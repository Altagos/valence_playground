pub mod chunk_worker;

use std::{
    collections::{hash_map::Entry, HashMap},
    mem::size_of,
    num::NonZeroUsize,
    process,
    sync::{Arc, Mutex},
};

use bevy::{
    prelude::{Query, ResMut, Resource, World},
    window::Window,
};
use bevy_egui::egui;
use flume::{Receiver, Sender};
use indicatif::{ProgressBar, ProgressIterator, ProgressStyle};
use itertools::iproduct;
use lru::LruCache;
use noise::SuperSimplex;
use rayon::prelude::*;
use valence::{bevy_app::Plugin, prelude::*, server::Server};

use self::chunk_worker::{
    chunk_worker, gen_chunk, ChunkWorkerState, TerrainSettings, WorkerMessage, WorkerResponse,
};
use super::client::init_clients;
use crate::{minecraft::world_gen::chunk_worker::ChunkWorker, CONFIG, SPAWN_POS};

/// The order in which chunks should be processed by the thread pool. Smaller
/// values are sent first.
pub type Priority = u64;

/// World Gen sender
type WGSender = Sender<WorkerMessage>;

/// World Gen receiver
type WGReceiver = Receiver<WorkerResponse>;

#[derive(Resource, Clone, Debug)]
pub struct UpdateTerrainSettings(bool);

#[derive(Resource, Clone, Debug)]
pub struct Instances {
    pub terrain: Entity,
    pub wait: Entity,
}

#[derive(Resource)]
pub struct WorldGenState {
    /// Chunks that need to be generated. Chunks without a priority have already
    /// been sent to the thread pool.
    pending: HashMap<ChunkPos, Option<Priority>>,
    sender: WGSender,
    receiver: WGReceiver,
}

pub struct WorldGenPlugin;

impl Plugin for WorldGenPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<TerrainSettings>() // `ResourceInspectorPlugin` won't initialize the resource
            .register_type::<TerrainSettings>()
            .insert_resource(UpdateTerrainSettings(false)) // you need to register your type to display it
            .add_startup_system(setup)
            .add_system(set_terrain_settings)
            .add_system(remove_unviewed_chunks.after(init_clients))
            .add_system(update_client_views.after(remove_unviewed_chunks))
            .add_system(send_recv_chunks.after(update_client_views));
    }
}

fn setup(world: &mut World) {
    info!(target: "minecraft::world_gen", "Starting world generation...");

    let seed = CONFIG.world.seed.into();
    // let seed = 2968952028; // solid block at x:0 z:0

    info!(target: "minecraft::world_gen", "Current seed: {seed}");

    let pregen_chunks = CONFIG.world.pregen_chunks.clone();
    let num_pregen_chunks = pregen_chunks.clone().max().unwrap() * 2 + 1;
    let num_pregen_chunks = num_pregen_chunks * num_pregen_chunks;

    if num_pregen_chunks > CONFIG.world.chunks_cached.try_into().unwrap() {
        error!(target: "minecraft::world_gen",
            "Number of pregenerated chunks is higher than the chunk cache size. Please lower the \
             range of pregenerated chunks!"
        );
        process::exit(0);
    }

    let (finished_sender, finished_receiver) = flume::unbounded();
    let (pending_sender, pending_receiver) = flume::unbounded();
    let mut cache = LruCache::new(NonZeroUsize::new(CONFIG.world.chunks_cached).unwrap());
    let state = ChunkWorkerState {
        settings: TerrainSettings::default(),
        density: SuperSimplex::new(seed),
        hilly: SuperSimplex::new(seed.wrapping_add(1)),
        stone: SuperSimplex::new(seed.wrapping_add(2)),
        gravel: SuperSimplex::new(seed.wrapping_add(3)),
        grass: SuperSimplex::new(seed.wrapping_add(4)),
    };

    let mut pending_chunks = HashMap::new();
    for (x, z) in iproduct!(pregen_chunks.clone(), pregen_chunks.clone()) {
        let pos = ChunkPos::new(x, z);
        pending_chunks.insert(pos, Some((x + z) as u64));
    }

    let pb = ProgressBar::new(num_pregen_chunks as u64)
        .with_message("Pregenerating chunks...".to_string());

    pb.set_style(
        ProgressStyle::with_template(
            "{spinner:.green} [{elapsed_precise}] [{bar:.cyan/blue}] {pos}/{len} {msg} ({eta})",
        )
        .unwrap()
        .progress_chars("#>-"),
    );

    // let state = Arcstate));
    let state_clone = Arc::from(state.clone());

    let chunks = iproduct!(pregen_chunks.clone(), pregen_chunks)
        .progress_with(pb.clone())
        .par_bridge()
        .map(move |(x, z)| {
            let pos = ChunkPos::new(x, z);

            // let state = &state_clone;
            let chunk = gen_chunk(&state_clone, pos);
            // state.cache.push(pos, chunk.clone());
            // let _ = state.sender.try_send(WorkerResponse::Chunk(pos, chunk));
            (pos, chunk)
        })
        .collect::<Vec<(ChunkPos, Chunk)>>();

    chunks.iter().for_each(|(pos, chunk)| {
        cache.push(pos.to_owned(), chunk.to_owned());
    });

    drop(chunks);

    pb.finish_with_message("Chunks generated");

    let spawn_chunk = cache
        .get(&ChunkPos::new(0, 0))
        .expect("Should be generated");
    let mut y = spawn_chunk.section_count() * 16 - 1;

    if CONFIG.world.spawn.is_some() {
        let spawn = CONFIG.world.spawn.unwrap();
        *SPAWN_POS.lock().unwrap() = DVec3::new(spawn[0], spawn[1], spawn[2]);
        debug!(target: "minecraft::world_gen", "Spawn at {} {} {}", spawn[0], spawn[1], spawn[2]);
    } else {
        loop {
            let block = spawn_chunk.block_state(0, y, 0);
            if block.is_air() {
                y -= 1;
            } else if y == 0 {
                break;
            } else {
                y -= 50; // Blocks below 0 are treated as a bove 0
                *SPAWN_POS.lock().unwrap() = DVec3::new(0.0, y as f64, 0.0);
                debug!(target: "minecraft::world_gen", "Spawn height: {y}, Spawn block: {}", block);
                break;
            }
        }
    }

    println!("{}", size_of::<LruCache<ChunkPos, Chunk>>());

    // Chunks are generated in a thread pool for parallelism and to avoid blocking
    // the main tick loop. You can use your thread pool of choice here (rayon,
    // bevy_tasks, etc). Only the standard library is used in the example for the
    // sake of simplicity.
    //
    // If your chunk generation algorithm is inexpensive then there's no need to do
    // this.
    let worker = Arc::from(Mutex::from(ChunkWorker {
        sender: finished_sender,
        receiver: pending_receiver,
        cache,
        state,
    }));
    let metrics = tokio::runtime::Handle::current().metrics();
    for i in 0..metrics.num_workers() {
        let worker_clone = Arc::clone(&worker);

        let _ = tokio::task::Builder::new()
            .name(&format!("ChunkWorker_{}", i))
            .spawn(async move { chunk_worker(worker_clone, format!("ChunkWorker_{}", i)) });

        debug!(target: "minecraft::world_gen", "Started Chunk Worker {}", i);
    }

    world.insert_resource(WorldGenState {
        pending: pending_chunks,
        sender: pending_sender,
        receiver: finished_receiver,
    });

    world.insert_resource(TerrainSettings::default());

    let instance = world
        .resource::<Server>()
        .new_instance(DimensionId::default());

    let terrain_id = world.spawn(instance).id();

    // Creating waiting world
    let mut instance = world
        .resource::<Server>()
        .new_instance(DimensionId::default());

    for z in -5..5 {
        for x in -5..5 {
            instance.insert_chunk([x, z], Chunk::default());
        }
    }

    for z in -25..25 {
        for x in -25..25 {
            instance.set_block([x, 200, z], BlockState::STONE);
        }
    }

    let wait_id = world.spawn(instance).id();

    world.insert_resource(Instances {
        terrain: terrain_id,
        wait: wait_id,
    });

    info!(target: "minecraft::world_gen", "World generation started");
}

fn remove_unviewed_chunks(mut instances: Query<&mut Instance>, instances_list: Res<Instances>) {
    let mut instance = instances.get_mut(instances_list.terrain).unwrap();
    instance.retain_chunks(|_, chunk| chunk.is_viewed_mut());
}

fn update_client_views(
    instances: Query<&mut Instance>,
    instances_list: Res<Instances>,
    mut clients: Query<&mut Client>,
    mut state: ResMut<WorldGenState>,
) {
    let instance = instances.get(instances_list.terrain).unwrap();

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

fn send_recv_chunks(
    mut instances: Query<&mut Instance>,
    instances_list: Res<Instances>,
    state: ResMut<WorldGenState>,
    mut clients: Query<&mut Client>,
) {
    let mut instance = instances.get_mut(instances_list.terrain).unwrap();
    let state = state.into_inner();

    // Insert the chunks that are finished generating into the instance.
    for response in state.receiver.drain() {
        match response {
            WorkerResponse::Chunk(pos, chunk) => {
                instance.insert_chunk(pos, chunk);
                assert!(state.pending.remove(&pos).is_some());
            }
            WorkerResponse::GetTerrainSettings(_) => todo!("Not yet implemented"),
            WorkerResponse::TerrainSettingsSet => {
                clients.par_iter_mut().for_each_mut(|mut c| {
                    c.set_instance(instances_list.terrain);
                    c.send_message("Terrain Regenerated".color(Color::GREEN))
                });
            }
        }
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
        let _ = state.sender.try_send(WorkerMessage::Chunk(*pos));
    }
}

fn set_terrain_settings(
    settings: ResMut<TerrainSettings>,
    mut update: ResMut<UpdateTerrainSettings>,
    mut state: ResMut<WorldGenState>,
    mut instances: Query<&mut Instance>,
    instances_list: Res<Instances>,
    mut clients: Query<&mut Client>,
) {
    if update.0 {
        update.0 = false;
        let _ = state
            .sender
            .try_send(WorkerMessage::SetTerrainSettings(settings.clone()));
        let mut instance = instances.get_mut(instances_list.terrain).unwrap();

        instance.clear_chunks();

        for mut client in &mut clients {
            client.send_message("Regenerating terrain".color(Color::RED));
            client.set_instance(instances_list.wait);
            client.set_position([0., 203., 0.]);

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

            view.iter().for_each(queue_pos);
        }

        // Collect all the new chunks that need to be loaded this tick.
        let mut to_send = vec![];

        for (pos, priority) in &mut state.pending.iter_mut() {
            if let Some(pri) = priority.take() {
                to_send.push((pri, *pos));
            }
        }

        // Sort chunks by ascending priority.
        to_send.sort_unstable_by_key(|(pri, _)| *pri);

        // Send the sorted chunks to be loaded.
        for (_, pos) in to_send {
            let _ = state.sender.try_send(WorkerMessage::Chunk(pos));
        }
    }
}

pub fn inspector_ui(
    mut egui_context: ResMut<bevy_egui::EguiContext>,
    mut settings: ResMut<TerrainSettings>,
    mut update: ResMut<UpdateTerrainSettings>,
    mut windows: Query<(Entity, &mut Window)>,
) {
    let window = windows.single_mut();
    let ctx = egui_context.ctx_for_window_mut(window.0);

    egui::Window::new("Terrain Settings").show(&ctx, |ui| {
        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.collapsing("Gravel", |ui| {
                egui::Grid::new("gravel_settings").show(ui, |ui| {
                    ui.checkbox(&mut settings.enable_gravel, "Enable gravel");
                    ui.end_row();

                    ui.label("Point scaling");
                    ui.add(
                        egui::DragValue::new(&mut settings.gravel_height.point_scaleing).speed(0.1),
                    );
                    ui.end_row();

                    ui.label("Octaves");
                    ui.add(egui::DragValue::new(&mut settings.gravel_height.octaves).speed(0.1));
                    ui.end_row();

                    ui.label("Lacunarity");
                    ui.add(egui::DragValue::new(&mut settings.gravel_height.lacunarity).speed(0.1));
                    ui.end_row();

                    ui.label("Persistence");
                    ui.add(
                        egui::DragValue::new(&mut settings.gravel_height.persistence).speed(0.1),
                    );
                    ui.end_row();
                });
            });

            ui.collapsing("Sand", |ui| {
                egui::Grid::new("sand_settings").show(ui, |ui| {
                    ui.checkbox(&mut settings.enable_sand, "Enable sand");
                    ui.end_row();

                    ui.label("Sand offset");
                    ui.add(egui::DragValue::new(&mut settings.sand_offset).speed(0.1));
                    ui.end_row();

                    ui.label("Point scaling");
                    ui.add(
                        egui::DragValue::new(&mut settings.sand_height.point_scaleing).speed(0.1),
                    );
                    ui.end_row();

                    ui.label("Octaves");
                    ui.add(egui::DragValue::new(&mut settings.sand_height.octaves).speed(0.1));
                    ui.end_row();

                    ui.label("Lacunarity");
                    ui.add(egui::DragValue::new(&mut settings.sand_height.lacunarity).speed(0.1));
                    ui.end_row();

                    ui.label("Persistence");
                    ui.add(egui::DragValue::new(&mut settings.sand_height.persistence).speed(0.1));
                    ui.end_row();
                });
            });

            ui.collapsing("Stone", |ui| {
                egui::Grid::new("stone_settings").show(ui, |ui| {
                    ui.checkbox(&mut settings.enable_stone, "Enable stone");
                    ui.end_row();

                    ui.label("Point scaling");
                    ui.add(egui::DragValue::new(&mut settings.stone_point_scaleing).speed(0.1));
                    ui.end_row();
                });
            });

            ui.checkbox(&mut settings.enable_grass, "Enable grass");
            ui.checkbox(&mut settings.enable_water, "Enable water");
            ui.horizontal(|ui| {
                ui.label("Seed");
                ui.add(egui::DragValue::new(&mut settings.seed).speed(0.1));
            });

            ui.separator();

            ui.horizontal(|ui| {
                if ui.button("Update").clicked() {
                    update.0 = true;
                }

                if ui.button("Reset").clicked() {
                    *settings = TerrainSettings::default();
                    update.0 = true;
                }
            });
        });
    });
}
