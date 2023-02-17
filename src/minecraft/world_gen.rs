use std::{
    collections::{hash_map::Entry, HashMap},
    num::NonZeroUsize,
    sync::{Arc, Mutex},
    thread,
    time::Instant,
};

use bevy::prelude::{Query, ResMut, Resource, World};
use flume::{Receiver, Sender};
use indicatif::{ProgressBar, ProgressStyle};
use itertools::iproduct;
use lru::LruCache;
use noise::{NoiseFn, SuperSimplex};
use valence::{bevy_app::Plugin, prelude::*, server::Server};

use crate::{VPSystems, SECTION_COUNT};

pub struct ChunkWorkerState {
    pub sender: Sender<(ChunkPos, Chunk)>,
    pub receiver: Receiver<ChunkPos>,
    pub cache: LruCache<ChunkPos, Chunk>,
    // Noise functions
    pub density: SuperSimplex,
    pub hilly: SuperSimplex,
    pub stone: SuperSimplex,
    pub gravel: SuperSimplex,
    pub grass: SuperSimplex,
}

#[derive(Resource)]
pub struct WorldGenState {
    /// Chunks that need to be generated. Chunks without a priority have already
    /// been sent to the thread pool.
    pending: HashMap<ChunkPos, Option<Priority>>,
    sender: Sender<ChunkPos>,
    receiver: Receiver<(ChunkPos, Chunk)>,
}

/// The order in which chunks should be processed by the thread pool. Smaller
/// values are sent first.
pub type Priority = u64;

pub struct WorldGenPlugin;

impl Plugin for WorldGenPlugin {
    fn build(&self, app: &mut App) {
        app.add_startup_system(setup)
            .add_system(remove_unviewed_chunks.after(VPSystems::InitClients))
            .add_system(update_client_views.after(remove_unviewed_chunks))
            .add_system(send_recv_chunks.after(update_client_views));
    }
}

fn setup(world: &mut World) {
    info!(target: "minecraft::world_gen", "Starting world generation...");

    let seed = rand::random();

    info!(target: "minecraft::world_gen", "Current seed: {seed}");

    let mut num_pregen_chunks = 0;
    for (..) in iproduct!(-22..22, -22..22) {
        num_pregen_chunks += 1;
    }

    let (finished_sender, finished_receiver) = flume::unbounded();
    let (pending_sender, pending_receiver) = flume::unbounded();
    let cache = LruCache::new(NonZeroUsize::new(num_pregen_chunks + 100).unwrap());

    let mut state = ChunkWorkerState {
        sender: finished_sender,
        receiver: pending_receiver,
        cache,
        density: SuperSimplex::new(seed),
        hilly: SuperSimplex::new(seed.wrapping_add(1)),
        stone: SuperSimplex::new(seed.wrapping_add(2)),
        gravel: SuperSimplex::new(seed.wrapping_add(3)),
        grass: SuperSimplex::new(seed.wrapping_add(4)),
    };

    let mut pending_chunks = HashMap::new();

    {
        let pb = ProgressBar::new(num_pregen_chunks as u64)
            .with_message(format!("Pregenerating chunks..."));
        pb.set_style(
            ProgressStyle::with_template(
                "{spinner:.green} [{elapsed_precise}] [{bar:.cyan/blue}] {pos}/{len} {msg} ({eta})",
            )
            .unwrap()
            .progress_chars("#>-"),
        );

        for (x, z) in iproduct!(-22..=22, -22..=22) {
            let pos = ChunkPos::new(x, z);
            let mut chunk = Chunk::new(SECTION_COUNT);

            gen_chunk(&state, &mut chunk, pos);

            state.cache.push(pos, chunk.clone());
            let _ = state.sender.try_send((pos, chunk));
            pb.inc(1);

            pending_chunks.insert(pos, Some((x + z) as u64));
        }

        pb.finish();
        pb.set_message("Pregeneration complete");
    }

    // Chunks are generated in a thread pool for parallelism and to avoid blocking
    // the main tick loop. You can use your thread pool of choice here (rayon,
    // bevy_tasks, etc). Only the standard library is used in the example for the
    // sake of simplicity.
    //
    // If your chunk generation algorithm is inexpensive then there's no need to do
    // this.
    let state = Arc::new(Mutex::new(state));
    for _ in 0..thread::available_parallelism().unwrap().get() {
        let state = state.clone();
        thread::spawn(move || chunk_worker(state));
    }

    world.insert_resource(WorldGenState {
        pending: pending_chunks,
        sender: pending_sender,
        receiver: finished_receiver,
    });

    let instance = world
        .resource::<Server>()
        .new_instance(DimensionId::default());

    world.spawn(instance);

    info!(target: "minecraft::world_gen", "World generation started");
}

fn remove_unviewed_chunks(mut instances: Query<&mut Instance>) {
    instances
        .single_mut()
        .retain_chunks(|_, chunk| chunk.is_viewed_mut());
}

fn update_client_views(
    mut instances: Query<&mut Instance>,
    mut clients: Query<&mut Client>,
    mut state: ResMut<WorldGenState>,
) {
    let instance = instances.single_mut();

    for client in &mut clients {
        let view = client.view();
        let queue_pos = |pos| {
            if instance.chunk(pos).is_none() {
                match state.pending.entry(pos) {
                    Entry::Occupied(mut oe) => {
                        if let Some(priority) = oe.get_mut() {
                            let dist = view.pos.distance_squared(pos);
                            *priority = (*priority).min(dist);
                        }
                    }
                    Entry::Vacant(ve) => {
                        let dist = view.pos.distance_squared(pos);
                        ve.insert(Some(dist));
                    }
                }
            }
        };

        // Queue all the new chunks in the view to be sent to the thread pool.
        if client.is_added() {
            view.iter().for_each(queue_pos);
        } else {
            let old_view = client.old_view();
            if old_view != view {
                view.diff(old_view).for_each(queue_pos);
            }
        }
    }
}

fn send_recv_chunks(mut instances: Query<&mut Instance>, state: ResMut<WorldGenState>) {
    let mut instance = instances.single_mut();
    let state = state.into_inner();

    // Insert the chunks that are finished generating into the instance.
    for (pos, chunk) in state.receiver.drain() {
        instance.insert_chunk(pos, chunk);
        assert!(state.pending.remove(&pos).is_some());
    }

    // Collect all the new chunks that need to be loaded this tick.
    let mut to_send = vec![];

    for (pos, priority) in &mut state.pending.iter_mut() {
        if let Some(pri) = priority.take() {
            to_send.push((pri, pos));
        }
    }

    // Sort chunks by ascending priority.
    to_send.sort_unstable_by_key(|(pri, _)| *pri);

    // Send the sorted chunks to be loaded.
    for (_, pos) in to_send {
        let _ = state.sender.try_send(*pos);
    }
}

fn chunk_worker(state: Arc<Mutex<ChunkWorkerState>>) {
    let mut state = state.lock().unwrap();
    while let Ok(pos) = state.receiver.recv() {
        let mut chunk = Chunk::new(SECTION_COUNT);
        let cached;
        let start = Instant::now();

        if state.cache.contains(&pos) {
            chunk = state.cache.get_mut(&pos).unwrap().to_owned();
            cached = true;
        } else {
            gen_chunk(&state, &mut chunk, pos);
            state.cache.push(pos, chunk.clone());
            cached = false;
        }

        let duration = start.elapsed();
        trace!(target: "minecraft::world_gen", cached = cached,"Generated chunk at: {pos:?} ({duration:?})");

        let _ = state.sender.try_send((pos, chunk));
    }
}

#[inline]
pub fn gen_chunk(state: &ChunkWorkerState, chunk: &mut Chunk, pos: ChunkPos) {
    for (offset_z, offset_x) in iproduct!(0..16, 0..16) {
        let x = offset_x as i32 + pos.x * 16;
        let z = offset_z as i32 + pos.z * 16;

        gen_block(state, chunk, x, z, offset_x, offset_z);
    }
}

#[inline]
pub fn gen_chunk_fors(state: &ChunkWorkerState, chunk: &mut Chunk, pos: ChunkPos) {
    for offset_z in 0..16 {
        for offset_x in 0..16 {
            let x = offset_x as i32 + pos.x * 16;
            let z = offset_z as i32 + pos.z * 16;

            gen_block(state, chunk, x, z, offset_x, offset_z);
        }
    }
}

fn gen_block(
    state: &ChunkWorkerState,
    chunk: &mut Chunk,
    x: i32,
    z: i32,
    offset_x: usize,
    offset_z: usize,
) {
    let mut in_terrain = false;
    let mut depth = 0;

    // Fill in the terrain column.
    for y in (0..chunk.section_count() as i32 * 16).rev() {
        const WATER_HEIGHT: i32 = 120;

        let p = DVec3::new(f64::from(x), f64::from(y), f64::from(z));

        let block = if has_terrain_at(&state, p) {
            let gravel_height =
                WATER_HEIGHT - 1 - (fbm(&state.gravel, p / 10.0, 3, 2.0, 0.5) * 6.0).floor() as i32;

            if in_terrain {
                if depth > 0 {
                    depth -= 1;
                    if y < gravel_height {
                        BlockState::GRAVEL
                    } else {
                        BlockState::DIRT
                    }
                } else {
                    BlockState::STONE
                }
            } else {
                in_terrain = true;
                let n = noise01(&state.stone, p / 15.0);

                depth = (n * 5.0).round() as u64;

                if y < gravel_height {
                    BlockState::GRAVEL
                } else if y < WATER_HEIGHT - 1 {
                    BlockState::DIRT
                } else {
                    BlockState::GRASS_BLOCK
                }
            }
        } else {
            in_terrain = false;
            depth = 0;
            if y < WATER_HEIGHT {
                BlockState::WATER
            } else {
                BlockState::AIR
            }
        };

        chunk.set_block_state(offset_x, y as usize, offset_z, block);
    }

    // Add grass on top of grass blocks.
    for y in (0..chunk.section_count() * 16).rev() {
        if chunk.block_state(offset_x, y, offset_z).is_air()
            && chunk.block_state(offset_x, y - 1, offset_z) == BlockState::GRASS_BLOCK
        {
            let p = DVec3::new(f64::from(x), y as f64, f64::from(z));
            let density = fbm(&state.grass, p / 5.0, 4, 2.0, 0.7);

            if density > 0.55 {
                if density > 0.7 && chunk.block_state(offset_x, y + 1, offset_z).is_air() {
                    let upper = BlockState::TALL_GRASS.set(PropName::Half, PropValue::Upper);
                    let lower = BlockState::TALL_GRASS.set(PropName::Half, PropValue::Lower);

                    chunk.set_block_state(offset_x, y + 1, offset_z, upper);
                    chunk.set_block_state(offset_x, y, offset_z, lower);
                } else {
                    chunk.set_block_state(offset_x, y, offset_z, BlockState::GRASS);
                }
            }
        }
    }
}

fn has_terrain_at(state: &ChunkWorkerState, p: DVec3) -> bool {
    let hilly = lerp(0.1, 1.0, noise01(&state.hilly, p / 400.0)).powi(2);

    let lower = 64.0 + 100.0 * hilly;
    let upper = lower + 100.0 * hilly;

    if p.y <= lower {
        return true;
    } else if p.y >= upper {
        return false;
    }

    let density = 1.0 - lerpstep(lower, upper, p.y);

    let n = fbm(&state.density, p / 100.0, 4, 2.0, 0.5);

    n < density
}

fn lerp(a: f64, b: f64, t: f64) -> f64 { a * (1.0 - t) + b * t }

fn lerpstep(edge0: f64, edge1: f64, x: f64) -> f64 {
    if x <= edge0 {
        0.0
    } else if x >= edge1 {
        1.0
    } else {
        (x - edge0) / (edge1 - edge0)
    }
}

fn fbm(noise: &SuperSimplex, p: DVec3, octaves: u32, lacunarity: f64, persistence: f64) -> f64 {
    let mut freq = 1.0;
    let mut amp = 1.0;
    let mut amp_sum = 0.0;
    let mut sum = 0.0;

    for _ in 0..octaves {
        let n = noise01(noise, p * freq);
        sum += n * amp;
        amp_sum += amp;

        freq *= lacunarity;
        amp *= persistence;
    }

    // Scale the output to [0, 1]
    sum / amp_sum
}

fn noise01(noise: &SuperSimplex, p: DVec3) -> f64 { (noise.get(p.to_array()) + 1.0) / 2.0 }
