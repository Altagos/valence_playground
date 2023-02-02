use bevy::prelude::{Resource, World};
use noise::{Clamp, NoiseFn, SuperSimplex};
use spinners::{Spinner, Spinners};
use valence_new::{
    bevy_app::Plugin,
    prelude::{App, BlockState, Chunk, DimensionId, Instance},
    server::Server,
};
use vek::Lerp;

use crate::SPAWN_Y;

#[derive(Resource, Clone, Copy, Debug)]
pub struct TerrainSettings {
    pub seed: u32,
}

impl Default for TerrainSettings {
    fn default() -> Self {
        let seed = rand::random();

        Self { seed }
    }
}

#[derive(Resource, Clone, Copy, Debug)]
pub struct TerrainGenerator {
    pub settings: TerrainSettings,
    density_noise: SuperSimplex,
    hilly_noise: SuperSimplex,
    stone_noise: SuperSimplex,
    gravel_noise: SuperSimplex,
    grass_noise: SuperSimplex,
}

impl Default for TerrainGenerator {
    fn default() -> Self {
        let settings = TerrainSettings::default();

        Self {
            settings,
            density_noise: SuperSimplex::new(settings.seed),
            hilly_noise: SuperSimplex::new(settings.seed.wrapping_add(1)),
            stone_noise: SuperSimplex::new(settings.seed.wrapping_add(2)),
            gravel_noise: SuperSimplex::new(settings.seed.wrapping_add(3)),
            grass_noise: SuperSimplex::new(settings.seed.wrapping_add(4)),
        }
    }
}

impl TerrainGenerator {
    pub fn gen(&self, world: &mut World) -> Instance {
        let mut sp = Spinner::new(Spinners::Noise, "Creating world".into());

        let mut instance = world
            .resource::<Server>()
            .new_instance(DimensionId::default());

        for z in -100..100 {
            for x in -100..100 {
                instance.insert_chunk([x, z], Chunk::default());
            }
        }

        for z in -500..500 {
            for x in -500..500 {
                let mut y = SPAWN_Y;

                if !(z == 0 && x == 0) {
                    y = (10.0 * (0.01 * x as f64).sin() + 5.0 * (0.03 * z as f64).cos()).round()
                        as i32
                        + SPAWN_Y;

                    let hilly = Lerp::lerp_unclamped(
                        0.0,
                        0.9,
                        noise01(&self.hilly_noise, [x, y, z].map(|a| a as f64 / 260.0)).powi(2),
                    );

                    let lower = 15.0 + 100.0 * hilly;
                    let upper = lower + 100.0 * hilly;

                    for ny in 0..=upper.round() as i32 {
                        // let noise_value =
                        //     fbm(&self.hilly_noise, [x, ny, z].map(|a| a as f64), 3, 2.0, 0.5)
                        //         .clamp(0.0, 0.9)
                        //         * 10.0;
                        let noise_value = Lerp::lerp_unclamped(
                            0.0,
                            0.9,
                            noise01(&self.hilly_noise, [x, ny, z].map(|a| a as f64 / 260.0))
                                .powi(1),
                        ) * 10.0;

                        // println!("{noise_value}");

                        let block = match noise_value.floor() as u64 {
                            0 => BlockState::BLACKSTONE,
                            1 => BlockState::NETHERITE_BLOCK,
                            2 => BlockState::BASALT,
                            3 => BlockState::CHISELED_STONE_BRICKS,
                            4 => BlockState::LODESTONE,
                            5 => BlockState::SMOOTH_STONE,
                            6 => BlockState::POLISHED_DIORITE,
                            7 => BlockState::WHITE_CONCRETE_POWDER,
                            8 => BlockState::SNOW_BLOCK,
                            _ => BlockState::WHITE_WOOL,
                        };

                        instance.set_block_state([x, ny, z], block);
                    }
                }
            }
        }

        instance.set_block_state([0, SPAWN_Y, 0], BlockState::WHITE_WOOL);
        instance.optimize();

        sp.stop();
        println!("");

        instance
    }
}

fn lerpstep(edge0: f64, edge1: f64, x: f64) -> f64 {
    if x <= edge0 {
        0.0
    } else if x >= edge1 {
        1.0
    } else {
        (x - edge0) / (edge1 - edge0)
    }
}

fn fbm(noise: &SuperSimplex, p: [f64; 3], octaves: u32, lacunarity: f64, persistence: f64) -> f64 {
    let mut freq = 1.0;
    let mut amp = 1.0;
    let mut amp_sum = 0.0;
    let mut sum = 0.0;

    for _ in 0..octaves {
        let n = noise01(noise, p.map(|a| a * freq));
        sum += n * amp;
        amp_sum += amp;

        freq *= lacunarity;
        amp *= persistence;
    }

    // Scale the output to [0, 1]
    sum / amp_sum
}

fn noise01(noise: &SuperSimplex, xyz: [f64; 3]) -> f64 { (noise.get(xyz) + 1.0) / 2.0 }

pub struct TerrainPlugin;

impl Plugin for TerrainPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<TerrainGenerator>()
            .add_startup_system(pre_gen_terrain);
    }
}

fn pre_gen_terrain(world: &mut World) {
    let generator = world.resource_mut::<TerrainGenerator>().to_owned();

    let instance = generator.gen(world);

    info!("World created and optimized");

    world.spawn(instance);
}
