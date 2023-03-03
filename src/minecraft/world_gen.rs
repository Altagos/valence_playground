pub mod chunk_worker;

use std::{
    collections::{hash_map::Entry, HashMap},
    mem::size_of,
    num::NonZeroUsize,
    process,
    sync::{Arc, Mutex},
};

use bevy::prelude::{Query, ResMut, Resource, World};
use bevy_inspector_egui::{bevy_egui, egui};
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
use crate::{minecraft::world_gen::chunk_worker::ChunkWorker, VPLabel, CONFIG, SPAWN_POS};

/// The order in which chunks should be processed by the thread pool. Smaller
/// values are sent first.
pub type Priority = u64;

/// World Gen sender
type WGSender = Sender<WorkerMessage>;

/// World Gen receiver
type WGReceiver = Receiver<WorkerResponse>;

#[derive(Resource, Clone, Debug)]
struct UpdateTerrainSettings(bool);

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
            .add_system(remove_unviewed_chunks.after(VPLabel::InitClients))
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

    let id = world.spawn(instance).id();

    let mut boat = McEntity::new(EntityKind::Boat, id);
    boat.set_position([0., 200., 0.]);
    world.spawn(boat);

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
    for response in state.receiver.drain() {
        match response {
            WorkerResponse::Chunk(pos, chunk) => {
                instance.insert_chunk(pos, chunk);
                assert!(state.pending.remove(&pos).is_some());
            }
            WorkerResponse::GetTerrainSettings(_) => todo!("Not yet implemented"),
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
    mut clients: Query<&mut Client>,
) {
    if update.0 {
        update.0 = false;
        let _ = state
            .sender
            .try_send(WorkerMessage::SetTerrainSettings(settings.clone()));
        let mut instance = instances.single_mut();

        instance.clear_chunks();

        for mut client in &mut clients {
            client.send_message("Regenerating terrain".color(Color::RED));

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

pub fn inspector_ui(world: &mut World) {
    // the usual `ResourceInspector` code
    let egui_context = world
        .resource_mut::<bevy_egui::EguiContext>()
        .ctx_mut()
        .clone();

    egui::Window::new("Terrain Settings").show(&egui_context, |ui| {
        egui::ScrollArea::vertical().show(ui, |ui| {
            bevy_inspector_egui::bevy_inspector::ui_for_resource::<TerrainSettings>(world, ui);

            ui.separator();

            ui.horizontal(|ui| {
                if ui.button("Update").clicked() {
                    let mut update = world.resource_mut::<UpdateTerrainSettings>();
                    update.0 = true;
                }

                if ui.button("Reset").clicked() {
                    {
                        let mut settings = world.resource_mut::<TerrainSettings>();
                        *settings = TerrainSettings::default();
                    }

                    let mut update = world.resource_mut::<UpdateTerrainSettings>();
                    update.0 = true;
                }
            });
        });
    });
}
