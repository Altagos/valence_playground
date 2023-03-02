use std::{
    sync::{Arc, Mutex},
    time::Instant,
};

use bevy::prelude::{Reflect, Resource};
use bevy_inspector_egui::{prelude::ReflectInspectorOptions, InspectorOptions};
use flume::{Receiver, Sender};
use itertools::iproduct;
use lru::LruCache;
use noise::{NoiseFn, SuperSimplex};
use valence::{prelude::*, view::ChunkPos};

use crate::{CONFIG, SECTION_COUNT};

/// Chunk Worker sender
type CWSender = Sender<WorkerResponse>;

/// Chunk Worker receiver
type CWReceiver = Receiver<WorkerMessage>;

pub enum WorkerMessage {
    Chunk(ChunkPos),
    EmptyCache,
    GetTerrainSettings,
    SetTerrainSettings(TerrainSettings),
}

pub enum WorkerResponse {
    Chunk(ChunkPos, Chunk),
    GetTerrainSettings(TerrainSettings),
}

#[derive(Debug, Clone, Resource, Reflect, InspectorOptions)]
#[reflect(Resource, InspectorOptions)]
pub struct TerrainSettings {
    pub gravel_height: FBMSettings,
    pub sand_offset: i32,
    pub sand_height: FBMSettings,
    pub stone_point_scaleing: f64,
    pub seed: u32,
}

impl Default for TerrainSettings {
    fn default() -> Self {
        Self {
            gravel_height: FBMSettings::default_gravel(),
            sand_offset: 5,
            sand_height: FBMSettings::default_sand(),
            stone_point_scaleing: 15.0,
            seed: CONFIG.world.seed.into(),
        }
    }
}

#[derive(Debug, Default, Clone, Resource, Reflect, InspectorOptions)]
#[reflect(Resource, InspectorOptions)]
pub struct FBMSettings {
    #[inspector(min = 0.0)]
    pub point_scaleing: f64,
    pub octaves: u32,
    #[inspector(min = 0.0)]
    pub lacunarity: f64,
    #[inspector(min = 0.0)]
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

pub struct ChunkWorkerState {
    pub sender: CWSender,
    pub receiver: CWReceiver,
    pub cache: LruCache<ChunkPos, Chunk>,
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
pub fn chunk_worker(state: Arc<Mutex<ChunkWorkerState>>) {
    let mut state = state.lock().unwrap();
    while let Ok(msg) = state.receiver.recv() {
        match msg {
            WorkerMessage::Chunk(pos) => {
                let mut chunk = Chunk::new(SECTION_COUNT);
                let cached;
                let start = Instant::now();

                if state.cache.contains(&pos) {
                    chunk = state.cache.get_mut(&pos).unwrap().clone();
                    cached = true;
                } else {
                    gen_chunk(&state, &mut chunk, pos);
                    state.cache.push(pos, chunk.clone());
                    cached = false;
                }

                let duration = start.elapsed();
                let settings = &state.settings;
                trace!(target: "minecraft::world_gen", cached = cached,"Generated chunk at: {pos:?} ({duration:?}) settings = {settings:?}");

                let _ = state.sender.try_send(WorkerResponse::Chunk(pos, chunk));
            }
            WorkerMessage::GetTerrainSettings => {
                let settings = state.settings.clone();
                let _ = state
                    .sender
                    .try_send(WorkerResponse::GetTerrainSettings(settings));
            }
            WorkerMessage::SetTerrainSettings(new_settings) => {
                debug!(target: "minecraft::world_gen", "Updated terrain settings: {new_settings:?}");

                if new_settings.seed != state.settings.seed {
                    let seed = new_settings.seed;
                    state.density = SuperSimplex::new(seed);
                    state.hilly = SuperSimplex::new(seed.wrapping_add(1));
                    state.stone = SuperSimplex::new(seed.wrapping_add(2));
                    state.gravel = SuperSimplex::new(seed.wrapping_add(3));
                    state.grass = SuperSimplex::new(seed.wrapping_add(4));
                }

                state.settings = new_settings;
                state.cache.clear();
                debug!(target: "minecraft::world_gen", "Cache emptied");
            }
            WorkerMessage::EmptyCache => {
                state.cache.clear();
                debug!(target: "minecraft::world_gen", "Cache emptied");
            }
        }
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
                let n = noise01(&state.stone, p / state.settings.stone_point_scaleing);

                depth = (n * 5.0).round() as u64;

                if y < gravel_height {
                    BlockState::GRAVEL
                } else if y < sand_height {
                    BlockState::SAND
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
        } else if chunk.block_state(offset_x, y, offset_z).is_liquid()
            && chunk.block_state(offset_x, y - 1, offset_z) == BlockState::GRAVEL
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
