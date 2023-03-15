use std::{
    sync::{Arc, LockResult, Mutex, MutexGuard},
    time::Instant,
};

use anyhow::Result;
use bevy::prelude::{Reflect, Resource};
use flume::{Receiver, Sender};
use itertools::{iproduct, Itertools};
use lru::LruCache;
use noise::{NoiseFn, SuperSimplex};
use rayon::prelude::*;
use valence::{prelude::*, view::ChunkPos};

use crate::{
    minecraft::save::{
        chunkpos_to_regionpos, load_region, overwrite_regions, save_chunk_to_region,
    },
    util::*,
    CONFIG, SECTION_COUNT,
};

/// Chunk Worker sender
type CWSender = Sender<WorkerResponse>;

/// Chunk Worker receiver
type CWReceiver = Receiver<WorkerMessage>;

#[derive(Debug, Clone)]
pub enum WorkerMessage {
    Chunk(ChunkPos),
    EmptyCache,
    GetTerrainSettings,
    SetTerrainSettings(TerrainSettings),
}

#[derive(Debug, Clone)]
pub enum WorkerResponse {
    Chunk(ChunkPos, Chunk),
    GetTerrainSettings(TerrainSettings),
    TerrainSettingsSet,
}

#[derive(Debug, Clone, Resource, Reflect)]
#[reflect(Resource)]
pub struct TerrainSettings {
    pub enable_gravel: bool,
    pub gravel_height: FBMSettings,
    pub enable_sand: bool,
    pub sand_offset: i32,
    pub sand_height: FBMSettings,
    pub enable_stone: bool,
    pub stone_point_scaleing: f64,
    pub enable_grass: bool,
    pub enable_water: bool,
    pub seed: u32,
}

impl Default for TerrainSettings {
    fn default() -> Self {
        Self {
            enable_gravel: true,
            gravel_height: FBMSettings::default_gravel(),
            enable_sand: true,
            sand_offset: 5,
            sand_height: FBMSettings::default_sand(),
            enable_stone: true,
            stone_point_scaleing: 15.0,
            enable_grass: true,
            enable_water: true,
            seed: CONFIG.world.seed.into(),
        }
    }
}

#[derive(Debug, Default, Clone, Resource, Reflect)]
#[reflect(Resource)]
pub struct FBMSettings {
    pub point_scaleing: f64,
    pub octaves: u32,
    pub lacunarity: f64,
    pub persistence: f64,
}

#[allow(clippy::must_use_candidate)]
impl FBMSettings {
    pub fn call(&self, noise: &SuperSimplex, p: DVec3) -> f64 {
        fbm(
            noise,
            p / self.point_scaleing,
            self.octaves,
            self.lacunarity,
            self.persistence,
        )
    }

    pub fn default_gravel() -> Self {
        Self {
            point_scaleing: 10.0,
            octaves: 3,
            lacunarity: 2.,
            persistence: -1.5,
        }
    }

    pub fn default_sand() -> Self {
        Self {
            point_scaleing: 10.0,
            octaves: 1,
            lacunarity: 2.0,
            persistence: 0.5,
        }
    }
}

pub struct ChunkWorker {
    pub sender: CWSender,
    pub receiver: CWReceiver,
    pub cache: LruCache<ChunkPos, Chunk>,
    pub state: ChunkWorkerState,
}

#[derive(Clone)]
pub struct ChunkWorkerState {
    pub settings: TerrainSettings,
    // Noise functions
    pub density: SuperSimplex,
    pub hilly: SuperSimplex,
    pub stone: SuperSimplex,
    pub gravel: SuperSimplex,
    pub grass: SuperSimplex,
}

/// # Panics
/// - if state is not accesible
pub fn chunk_worker(worker: Arc<Mutex<ChunkWorker>>, worker_name: String) -> Result<()> {
    let mut w = worker.lock().ignore_poison();

    while let Ok(msg) = w.receiver.recv() {
        match msg {
            WorkerMessage::Chunk(pos) => {
                handle_chunk(&mut w, &worker_name, pos)?;
            }
            WorkerMessage::GetTerrainSettings => {
                let settings = w.state.settings.clone();
                let _ = w
                    .sender
                    .try_send(WorkerResponse::GetTerrainSettings(settings));
            }
            WorkerMessage::SetTerrainSettings(new_settings) => {
                debug!(target: "minecraft::world_gen::worker", "Updated terrain settings: {new_settings:?}");

                if new_settings.seed != w.state.settings.seed {
                    let seed = new_settings.seed;
                    w.state.density = SuperSimplex::new(seed);
                    w.state.hilly = SuperSimplex::new(seed.wrapping_add(1));
                    w.state.stone = SuperSimplex::new(seed.wrapping_add(2));
                    w.state.gravel = SuperSimplex::new(seed.wrapping_add(3));
                    w.state.grass = SuperSimplex::new(seed.wrapping_add(4));
                }

                w.state.settings = new_settings;
                w.cache.clear();
                debug!(target: "minecraft::world_gen::worker", "Cache emptied");

                let _ = w.sender.send(WorkerResponse::TerrainSettingsSet);
            }
            WorkerMessage::EmptyCache => {
                w.cache.clear();
                debug!(target: "minecraft::world_gen::worker", "Cache emptied");
            }
        }
    }

    anyhow::Ok(())
}

fn handle_chunk(
    worker: &mut MutexGuard<ChunkWorker>,
    worker_name: &str,
    pos: ChunkPos,
) -> Result<()> {
    let chunk;
    let cached;
    let saved;
    let start = Instant::now();

    if worker.cache.contains(&pos) {
        chunk = worker.cache.get_mut(&pos).unwrap().clone();
        cached = true;
        saved = true;
    } else {
        chunk = {
            if let Ok(region) = load_region(chunkpos_to_regionpos(&pos)) {
                match region.chunk(pos) {
                    Some(c) => {
                        saved = true;
                        c.into()
                    }
                    None => {
                        saved = false;
                        let chunk = gen_chunk(&worker.state, pos);
                        let chunk_clone = chunk.clone();
                        let pos_clone = pos.clone();
                        tokio::task::Builder::new().spawn_blocking(move || {
                            save_chunk_to_region(chunk_clone, pos_clone).unwrap()
                        });
                        chunk
                    }
                }
            } else {
                saved = false;
                let chunk = gen_chunk(&worker.state, pos);
                let chunk_clone = chunk.clone();
                let pos_clone = pos.clone();
                tokio::task::Builder::new()
                    .spawn_blocking(move || save_chunk_to_region(chunk_clone, pos_clone).unwrap());
                chunk
            }
        };

        // chunk = gen_chunk(&worker.state, pos);
        worker.cache.push(pos, chunk.clone());
        cached = false;
    }

    let _ = worker.sender.try_send(WorkerResponse::Chunk(pos, chunk));

    let duration = start.elapsed();
    let settings = &worker.state.settings;
    trace!(
        target: "minecraft::world_gen::worker",
        cached = cached,
        saved = saved,
        worker = worker_name,
        "Generated chunk at: {pos:?} ({duration:?}) settings = {settings:?}"
    );

    anyhow::Ok(())
}

#[inline]
pub fn gen_chunk(state: &ChunkWorkerState, pos: ChunkPos) -> Chunk {
    let mut chunk = Chunk::new(SECTION_COUNT);

    let range = 0..16;
    let range_2 = 0..16;

    for (offset_z, offset_x) in range.cartesian_product(range_2) {
        let x = offset_x as i32 + pos.x * 16;
        let z = offset_z as i32 + pos.z * 16;

        gen_block(state, &mut chunk, x, z, offset_x, offset_z);
    }

    chunk
}

#[inline]
pub fn gen_chunk_fors(state: &ChunkWorkerState, pos: ChunkPos) -> Chunk {
    let mut chunk = Chunk::new(SECTION_COUNT);

    for offset_z in 0..16 {
        for offset_x in 0..16 {
            let x = offset_x as i32 + pos.x * 16;
            let z = offset_z as i32 + pos.z * 16;

            gen_block(state, &mut chunk, x, z, offset_x, offset_z);
        }
    }

    chunk
}

pub fn gen_block(
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

        let block = if has_terrain_at(state, p) {
            let gravel_fbm = state.settings.gravel_height.call(&state.gravel, p);
            let gravel_height = WATER_HEIGHT - 1 - (gravel_fbm * 6.0).floor() as i32;

            let sand_fbm = state.settings.sand_height.call(&state.gravel, p);
            let sand_height =
                gravel_height + state.settings.sand_offset + (sand_fbm * 6.0).floor() as i32;

            if in_terrain {
                if depth > 0 {
                    depth -= 1;
                    if y < gravel_height && state.settings.enable_gravel {
                        BlockState::GRAVEL
                    } else if state.settings.enable_grass {
                        BlockState::DIRT
                    } else {
                        BlockState::AIR
                    }
                } else if state.settings.enable_stone {
                    BlockState::STONE
                } else {
                    BlockState::AIR
                }
            } else {
                in_terrain = true;
                let n = noise01(&state.stone, p / state.settings.stone_point_scaleing);

                depth = (n * 5.0).round() as u64;

                if y < gravel_height && state.settings.enable_gravel {
                    BlockState::GRAVEL
                } else if y >= gravel_height && y < sand_height && state.settings.enable_sand {
                    BlockState::SAND
                } else if state.settings.enable_grass {
                    BlockState::GRASS_BLOCK
                } else {
                    BlockState::AIR
                }
            }
        } else {
            in_terrain = false;
            depth = 0;
            if y < WATER_HEIGHT && state.settings.enable_water {
                BlockState::WATER
            } else {
                BlockState::AIR
            }
        };

        chunk.set_block_state(offset_x, y as usize, offset_z, block);
    }

    // Add grass on top of grass blocks.
    if (state.settings.enable_water && state.settings.enable_gravel) || state.settings.enable_grass
    {
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
            } else if chunk.block_state(offset_x, y, offset_z).is_liquid()
                && chunk.block_state(offset_x, y - 1, offset_z) == BlockState::GRAVEL
                && state.settings.enable_water
                && state.settings.enable_gravel
            {
                let p = DVec3::new(f64::from(x), y as f64, f64::from(z));
                let density = fbm(&state.grass, p / 5.0, 4, 2.0, 0.7);

                if density > 0.55 {
                    if density > 0.7 && chunk.block_state(offset_x, y + 1, offset_z).is_liquid() {
                        let upper = BlockState::TALL_SEAGRASS.set(PropName::Half, PropValue::Upper);
                        let lower = BlockState::TALL_SEAGRASS.set(PropName::Half, PropValue::Lower);

                        chunk.set_block_state(offset_x, y + 1, offset_z, upper);
                        chunk.set_block_state(offset_x, y, offset_z, lower);
                    } else {
                        chunk.set_block_state(offset_x, y, offset_z, BlockState::SEAGRASS);
                    }
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
