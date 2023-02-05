use bevy::{
    prelude::{Query, ReflectResource, ResMut, Resource, World},
    reflect::Reflect,
    render::extract_resource::ExtractResource,
};
use bevy_inspector_egui::{inspector_options::std_options::NumberDisplay, prelude::*};
use noise::{NoiseFn, SuperSimplex};
use spinners::{Spinner, Spinners};
use valence_new::{
    bevy_app::Plugin,
    prelude::{App, BlockState, Chunk, DimensionId, Instance},
    server::Server,
};
use vek::{num_traits::ToPrimitive, Lerp};

use crate::SPAWN_Y;

#[derive(Reflect, Resource, Clone, Copy, Debug, ExtractResource, InspectorOptions)]
#[reflect(Resource)]
pub struct TerrainSettings {
    #[inspector(display = NumberDisplay::Drag)]
    pub seed: u32,
    pub noise_scaling: f64,
    #[inspector(display = NumberDisplay::Slider)]
    pub powi: u8,
    #[reflect(ignore)]
    pub update: bool,
}

impl Default for TerrainSettings {
    fn default() -> Self {
        let seed = rand::random();

        Self {
            seed,
            noise_scaling: 260.0,
            powi: 2,
            update: false,
        }
    }
}

#[derive(Resource, Clone, Copy, Debug)]
pub struct TerrainNoise {
    density_noise: SuperSimplex,
    hilly_noise: SuperSimplex,
    stone_noise: SuperSimplex,
    gravel_noise: SuperSimplex,
    grass_noise: SuperSimplex,
}

#[derive(Reflect, Resource, Clone, Copy, Debug, ExtractResource, InspectorOptions)]
#[reflect(Resource)]
pub struct TerrainGenerator {
    pub settings: TerrainSettings,
    #[reflect(ignore)]
    pub noise: TerrainNoise,
}

impl TerrainGenerator {
    pub fn set_settings(mut self, settings: TerrainSettings) -> Self {
        self.settings = settings;
        self
    }
}

impl Default for TerrainGenerator {
    fn default() -> Self {
        let settings = TerrainSettings::default();
        let noise = TerrainNoise {
            density_noise: SuperSimplex::new(settings.seed),
            hilly_noise: SuperSimplex::new(settings.seed.wrapping_add(1)),
            stone_noise: SuperSimplex::new(settings.seed.wrapping_add(2)),
            gravel_noise: SuperSimplex::new(settings.seed.wrapping_add(3)),
            grass_noise: SuperSimplex::new(settings.seed.wrapping_add(4)),
        };

        Self { settings, noise }
    }
}

impl TerrainGenerator {
    #[allow(clippy::cast_possible_truncation)]
    #[allow(clippy::cast_sign_loss)]
    pub fn gen(&self, instance: &mut Instance) {
        let mut sp = Spinner::new(Spinners::Noise, "Creating world".into());

        for z in -100..100 {
            for x in -100..100 {
                instance.insert_chunk([x, z], Chunk::default());
            }
        }

        for z in -500..500 {
            for x in -500..500 {
                let y = (10.0 * (0.01 * f64::from(x)).sin() + 5.0 * (0.03 * f64::from(z)).cos())
                    .round() as i32
                    + SPAWN_Y;

                let hilly = Lerp::lerp_unclamped(
                    0.0,
                    0.9,
                    noise01(
                        &self.noise.hilly_noise,
                        [x, y, z].map(|a| f64::from(a) / self.settings.noise_scaling),
                    )
                    .powi(self.settings.powi.into()),
                );

                let lower = 15.0 + 100.0 * hilly;
                let upper = lower + 100.0 * hilly;

                for ny in 0..=upper.round() as i32 {
                    // let noise_value =
                    //     fbm(&self.hilly_noise, [x, ny, z].map(|a| a as f64), 3, 2.0, 0.5)
                    //         .clamp(0.0, 0.9)
                    //         * 10.0;
                    let noise_value = hilly * 10.0;

                    // println!("{noise_value}");

                    let block = match noise_value.floor() as u32 {
                        0 => BlockState::BLACKSTONE,
                        1 => BlockState::NETHERITE_BLOCK,
                        2 => BlockState::BASALT,
                        3 => BlockState::CHISELED_STONE_BRICKS,
                        4 => BlockState::LODESTONE,
                        5 => BlockState::SMOOTH_STONE,
                        6 => BlockState::POLISHED_DIORITE,
                        7 => BlockState::WHITE_CONCRETE_POWDER,
                        8 => BlockState::SNOW_BLOCK,
                        _ => BlockState::AIR,
                    };

                    instance.set_block_state([x, ny, z], block);
                }
            }
        }

        // instance.set_block_state([0, SPAWN_Y, 0], BlockState::WHITE_WOOL);
        instance.optimize();

        sp.stop();
        println!();
    }
}

fn _lerpstep(edge0: f64, edge1: f64, x: f64) -> f64 {
    if x <= edge0 {
        0.0
    } else if x >= edge1 {
        1.0
    } else {
        (x - edge0) / (edge1 - edge0)
    }
}

fn _fbm(noise: &SuperSimplex, p: [f64; 3], octaves: u32, lacunarity: f64, persistence: f64) -> f64 {
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
        let terrain_settings = TerrainSettings::default();

        app.insert_resource(TerrainGenerator::default().set_settings(terrain_settings))
            .insert_resource(terrain_settings)
            .register_type::<TerrainGenerator>()
            .register_type::<TerrainSettings>()
            // .add_plugin(ResourceInspectorPlugin::<TerrainGenerator>::default())
            .add_startup_system(pre_gen_terrain)
            .add_system(update_terrain_settings);
    }
}

fn pre_gen_terrain(world: &mut World) {
    let generator = world.resource_mut::<TerrainGenerator>().to_owned();

    let mut instance = world
        .resource::<Server>()
        .new_instance(DimensionId::default());

    generator.gen(&mut instance);

    info!("World created and optimized");

    world.spawn(instance);
}

fn update_terrain_settings(
    mut generator: ResMut<TerrainGenerator>,
    mut settings: ResMut<TerrainSettings>,
    mut instances: Query<&mut Instance>,
) {
    if settings.update {
        let seed = settings.seed;
        settings.update = false;
        generator.settings = *settings;

        generator.noise.density_noise = SuperSimplex::new(seed);
        generator.noise.hilly_noise = SuperSimplex::new(seed.wrapping_add(1));
        generator.noise.stone_noise = SuperSimplex::new(seed.wrapping_add(2));
        generator.noise.gravel_noise = SuperSimplex::new(seed.wrapping_add(3));
        generator.noise.grass_noise = SuperSimplex::new(seed.wrapping_add(4));

        let mut instance = instances.get_single_mut().unwrap();
        generator.gen(instance.as_mut());
    }
}
