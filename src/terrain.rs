use bevy::prelude::{Resource, World};
use noise::SuperSimplex;
use valence_new::{
    bevy_app::Plugin,
    prelude::{App, BlockState, Chunk, DimensionId},
    server::Server,
};

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

pub struct TerrainPlugin;

impl Plugin for TerrainPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<TerrainGenerator>()
            .add_startup_system(pre_gen_terrain);
    }
}

fn pre_gen_terrain(world: &mut World) {
    let generator = world.resource_mut::<TerrainGenerator>();
    let mut instance = world
        .resource::<Server>()
        .new_instance(DimensionId::default());

    info!("Creating world");

    for z in -100..100 {
        for x in -100..100 {
            instance.insert_chunk([x, z], Chunk::default());
        }
    }

    for z in -500..500 {
        for x in -500..500 {
            let mut y = SPAWN_Y;

            if !(z == 0 && x == 0) {
                y = (10.0 * (0.01 * x as f64).sin() + 5.0 * (0.03 * z as f64).cos()).round() as i32
                    + SPAWN_Y;
            }

            for ny in 0..=y {
                instance.set_block_state([x, ny, z], BlockState::LIGHT_GRAY_WOOL);
            }
        }
    }

    instance.optimize();

    info!("World created and optimized");

    world.spawn(instance);
}
