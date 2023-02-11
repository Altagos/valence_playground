#![warn(clippy::pedantic)]
#![allow(clippy::module_name_repetitions)]
#![allow(clippy::needless_pass_by_value)]

mod chat;
mod gui;
mod terrain;

#[macro_use]
extern crate tracing;

use std::{
    collections::{hash_map::Entry, HashMap},
    net::SocketAddr,
    sync::{Arc, Mutex},
    thread,
    time::SystemTime,
};

use bevy::{
    prelude::{Camera3dBundle, Transform, Vec3},
    tasks::AsyncComputeTaskPool,
};
use chat::ChatPlugin;
use flume::{Receiver, Sender};
use gui::GuiPlugin;
use lazy_static::lazy_static;
use noise::{NoiseFn, SuperSimplex};
use rand::Rng;
use terrain::TerrainPlugin;
use valence::{
    client::{
        despawn_disconnected_clients,
        event::{default_event_handler, ChatCommand, FinishDigging, StartDigging, UseItemOnBlock},
    },
    player_list::Entry as PLEntry,
    prelude::*,
    protocol::types::Hand,
};

const SPAWN_Y: i32 = 64;
const PLAYER_UUID_1: Uuid = Uuid::from_u128(1);
const PLAYER_UUID_2: Uuid = Uuid::from_u128(2);
const MAX_CONNECTIONS: usize = 20;
const SECTION_COUNT: usize = 24;

lazy_static! {
    static ref PLAYER_COUNT: Mutex<usize> = Mutex::new(0);
    static ref SPAWN_POS: DVec3 = DVec3::new(0.0, 200.0, 0.0);
}

struct ChunkWorkerState {
    sender: Sender<(ChunkPos, Chunk)>,
    receiver: Receiver<ChunkPos>,
    // Noise functions
    density: SuperSimplex,
    hilly: SuperSimplex,
    stone: SuperSimplex,
    gravel: SuperSimplex,
    grass: SuperSimplex,
}

#[derive(Resource)]
struct GameState {
    /// Chunks that need to be generated. Chunks without a priority have already
    /// been sent to the thread pool.
    pending: HashMap<ChunkPos, Option<Priority>>,
    sender: Sender<ChunkPos>,
    receiver: Receiver<(ChunkPos, Chunk)>,
}

/// The order in which chunks should be processed by the thread pool. Smaller
/// values are sent first.
type Priority = u64;

pub fn main() {
    dotenv::dotenv().ok();

    App::new()
        .add_plugin(
            ServerPlugin::new(MyCallbacks).with_connection_mode(ConnectionMode::Online {
                prevent_proxy_connections: false,
            }),
        )
        // .add_plugin(TerrainPlugin)
        .add_plugin(ChatPlugin)
        .add_plugin(GuiPlugin)
        // .add_plugin(WorldInspectorPlugin)
        .add_system_to_stage(EventLoop, default_event_handler)
        .add_system_to_stage(EventLoop, interpret_command)
        .add_system_to_stage(EventLoop, digging_creative_mode)
        .add_system_to_stage(EventLoop, digging_survival_mode)
        .add_system_to_stage(EventLoop, place_blocks)
        .add_system_set(PlayerList::default_system_set())
        .add_startup_system(setup_camera)
        .add_startup_system(setup)
        .add_system(init_clients)
        .add_system(update_player_list)
        .add_system(remove_unviewed_chunks.after(init_clients))
        .add_system(update_client_views.after(remove_unviewed_chunks))
        .add_system(send_recv_chunks.after(update_client_views))
        .add_system(despawn_disconnected_clients)
        .add_system(update_player_count)
        .run();
}

#[derive(Default)]
struct MyCallbacks;

#[async_trait]
impl AsyncCallbacks for MyCallbacks {
    #[allow(clippy::cast_possible_truncation)]
    #[allow(clippy::cast_possible_wrap)]
    async fn server_list_ping(
        &self,
        _shared: &SharedServer,
        remote_addr: SocketAddr,
        _protocol_version: i32,
    ) -> ServerListPing {
        ServerListPing::Respond {
            online_players: *PLAYER_COUNT.lock().unwrap() as i32,
            max_players: MAX_CONNECTIONS as i32,
            player_sample: vec![],
            description: "Just a minecraft server".color(Color::WHITE),
            favicon_png: include_bytes!("../assets/logo-64x64.png"),
        }
    }

    async fn login(&self, _shared: &SharedServer, _info: &NewClientInfo) -> Result<(), Text> {
        // return Err("You are not meant to join this example".color(Color::RED));

        if MAX_CONNECTIONS > *PLAYER_COUNT.lock().unwrap() {
            *PLAYER_COUNT.lock().unwrap() += 1;
            return Ok(());
        }
        return Err("Server full".color(Color::RED));
    }
}

fn setup(world: &mut World) {
    info!("Starting minecraft server...");

    let seed = rand::random();

    info!("current seed: {seed}");

    let (finished_sender, finished_receiver) = flume::unbounded();
    let (pending_sender, pending_receiver) = flume::unbounded();

    let state = Arc::new(ChunkWorkerState {
        sender: finished_sender,
        receiver: pending_receiver,
        density: SuperSimplex::new(seed),
        hilly: SuperSimplex::new(seed.wrapping_add(1)),
        stone: SuperSimplex::new(seed.wrapping_add(2)),
        gravel: SuperSimplex::new(seed.wrapping_add(3)),
        grass: SuperSimplex::new(seed.wrapping_add(4)),
    });

    // Chunks are generated in a thread pool for parallelism and to avoid blocking
    // the main tick loop. You can use your thread pool of choice here (rayon,
    // bevy_tasks, etc). Only the standard library is used in the example for the
    // sake of simplicity.
    //
    // If your chunk generation algorithm is inexpensive then there's no need to do
    // this.
    for _ in 0..thread::available_parallelism().unwrap().get() {
        let state = state.clone();
        thread::spawn(move || chunk_worker(state));
    }

    world.insert_resource(GameState {
        pending: HashMap::new(),
        sender: pending_sender,
        receiver: finished_receiver,
    });

    let instance = world
        .resource::<Server>()
        .new_instance(DimensionId::default());

    world.spawn(instance);

    info!("Minecraft server started");
}

fn setup_camera(mut commands: Commands) {
    commands.spawn(Camera3dBundle {
        transform: Transform::from_xyz(-2.0, 2.5, 5.0).looking_at(Vec3::ZERO, Vec3::Y),
        ..Default::default()
    });
}

fn init_clients(
    mut clients: Query<&mut Client, Added<Client>>,
    instances: Query<Entity, With<Instance>>,
    mut player_list: ResMut<PlayerList>,
) {
    let instance = instances.get_single().unwrap();

    for mut client in &mut clients {
        client.set_position([SPAWN_POS.x, SPAWN_POS.y, SPAWN_POS.z]);
        client.set_instance(instance);
        client.set_game_mode(GameMode::Creative);
        client.set_op_level(2);
        client.set_view_distance(20);

        client.send_message(
            "Please open your player list (tab key)."
                .italic()
                .color(Color::WHITE),
        );

        let entry = PlayerListEntry::new()
            .with_username(client.username())
            .with_properties(client.properties()) // For the player's skin and cape.
            .with_game_mode(client.game_mode())
            .with_ping(0) // Use negative values to indicate missing.
            .with_display_name(Some(client.username().color(Color::new(255, 87, 66))));

        player_list.insert(client.uuid(), entry);
    }
}

fn update_player_list(mut player_list: ResMut<PlayerList>, server: Res<Server>) {
    let tick = server.current_tick();

    player_list.set_header("Current tick: ".into_text() + tick);
    player_list
        .set_footer("Current tick but in purple: ".into_text() + tick.color(Color::LIGHT_PURPLE));

    if tick % 5 == 0 {
        let mut rng = rand::thread_rng();
        let color = Color::new(rng.gen(), rng.gen(), rng.gen());

        match player_list.get_mut(PLAYER_UUID_1) {
            Some(entry) => {
                let new_display_name = entry.display_name().unwrap().clone().color(color);
                entry.set_display_name(Some(new_display_name));
            }
            None => {
                player_list.insert(
                    PLAYER_UUID_1,
                    PlayerListEntry::new().with_display_name(Some("persistent entry with no ping")),
                );
            }
        };
    }

    if tick % 20 == 0 {
        match player_list.entry(PLAYER_UUID_2) {
            PLEntry::Occupied(oe) => {
                oe.remove();
            }
            PLEntry::Vacant(ve) => {
                let entry = PlayerListEntry::new()
                    .with_display_name(Some("Hello!"))
                    .with_ping(300);

                ve.insert(entry);
            }
        }
    }
}

fn update_player_count(clients: Query<(Entity, &Client)>) {
    for (_entity, client) in &clients {
        if client.is_disconnected() {
            *PLAYER_COUNT.lock().unwrap() -= 1;
        }
    }
}

fn interpret_command(mut clients: Query<&mut Client>, mut events: EventReader<ChatCommand>) {
    for event in events.iter() {
        let Ok(mut client) = clients.get_component_mut::<Client>(event.client) else {
            continue;
        };

        let mut args = event.command.split_whitespace();
        let command = args.next().unwrap_or_default();

        if command == "gamemode" {
            if client.op_level() < 2 {
                // not enough permissions to use gamemode command
                client.send_message("Not enough permissions to use gamemode command.".italic());
                continue;
            }

            let mode = args.next().unwrap_or_default();
            let mode = match mode {
                "adventure" => GameMode::Adventure,
                "creative" => GameMode::Creative,
                "survival" => GameMode::Survival,
                "spectator" => GameMode::Spectator,
                _ => {
                    client.send_message("Invalid gamemode.".italic());
                    continue;
                }
            };
            client.set_game_mode(mode);
            client.send_message(format!("Set gamemode to {mode:?}.").italic());
        } else {
            client.send_message("Invalid command.".italic());
        }
    }
}

fn digging_creative_mode(
    clients: Query<&Client>,
    mut instances: Query<&mut Instance>,
    mut events: EventReader<StartDigging>,
) {
    let mut instance = instances.single_mut();

    for event in events.iter() {
        let Ok(client) = clients.get_component::<Client>(event.client) else {
            continue;
        };
        if client.game_mode() == GameMode::Creative {
            instance.set_block_state(event.position, BlockState::AIR);
        }
    }
}

fn digging_survival_mode(
    clients: Query<&Client>,
    mut instances: Query<&mut Instance>,
    mut events: EventReader<FinishDigging>,
) {
    let mut instance = instances.single_mut();

    for event in events.iter() {
        let Ok(client) = clients.get_component::<Client>(event.client) else {
            continue;
        };
        if client.game_mode() == GameMode::Survival {
            instance.set_block_state(event.position, BlockState::AIR);
        }
    }
}

fn place_blocks(
    mut clients: Query<(&Client, &mut Inventory)>,
    mut instances: Query<&mut Instance>,
    mut events: EventReader<UseItemOnBlock>,
) {
    let mut instance = instances.single_mut();

    for event in events.iter() {
        let Ok((client, mut inventory)) = clients.get_mut(event.client) else {
            continue;
        };
        if event.hand != Hand::Main {
            continue;
        }

        // get the held item
        let slot_id = client.held_item_slot();
        let Some(stack) = inventory.slot(slot_id) else {
            // no item in the slot
            continue;
        };

        let Some(block_kind) = stack.item.to_block_kind() else {
            // can't place this item as a block
            continue;
        };

        if client.game_mode() == GameMode::Survival {
            // check if the player has the item in their inventory and remove
            // it.
            let slot = if stack.count() > 1 {
                let mut stack = stack.clone();
                stack.set_count(stack.count() - 1);
                Some(stack)
            } else {
                None
            };
            inventory.replace_slot(slot_id, slot);
        }
        let real_pos = event.position.get_in_direction(event.face);
        instance.set_block_state(real_pos, block_kind.to_state());
    }
}

fn remove_unviewed_chunks(mut instances: Query<&mut Instance>) {
    instances
        .single_mut()
        .retain_chunks(|_, chunk| chunk.is_viewed_mut());
}

fn update_client_views(
    mut instances: Query<&mut Instance>,
    mut clients: Query<&mut Client>,
    mut state: ResMut<GameState>,
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

fn send_recv_chunks(mut instances: Query<&mut Instance>, state: ResMut<GameState>) {
    let mut instance = instances.single_mut();
    let state = state.into_inner();

    // Insert the chunks that are finished generating into the instance.
    for (pos, chunk) in state.receiver.drain() {
        instance.insert_chunk(pos, chunk);
        assert!(state.pending.remove(&pos).is_some());
    }

    // Collect all the new chunks that need to be loaded this tick.
    let mut to_send = vec![];

    for (pos, priority) in &mut state.pending {
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

fn chunk_worker(state: Arc<ChunkWorkerState>) {
    while let Ok(pos) = state.receiver.recv() {
        let mut chunk = Chunk::new(SECTION_COUNT);

        for offset_z in 0..16 {
            for offset_x in 0..16 {
                let x = offset_x as i32 + pos.x * 16;
                let z = offset_z as i32 + pos.z * 16;

                let mut in_terrain = false;
                let mut depth = 0;

                // Fill in the terrain column.
                for y in (0..chunk.section_count() as i32 * 16).rev() {
                    const WATER_HEIGHT: i32 = 120;

                    let p = DVec3::new(x as f64, y as f64, z as f64);

                    let block = if has_terrain_at(&state, p) {
                        let gravel_height = WATER_HEIGHT
                            - 1
                            - (fbm(&state.gravel, p / 10.0, 3, 2.0, 0.5) * 6.0).floor() as i32;

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

                            depth = (n * 5.0).round() as u32;

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
                        let p = DVec3::new(x as f64, y as f64, z as f64);
                        let density = fbm(&state.grass, p / 5.0, 4, 2.0, 0.7);

                        if density > 0.55 {
                            if density > 0.7
                                && chunk.block_state(offset_x, y + 1, offset_z).is_air()
                            {
                                let upper =
                                    BlockState::TALL_GRASS.set(PropName::Half, PropValue::Upper);
                                let lower =
                                    BlockState::TALL_GRASS.set(PropName::Half, PropValue::Lower);

                                chunk.set_block_state(offset_x, y + 1, offset_z, upper);
                                chunk.set_block_state(offset_x, y, offset_z, lower);
                            } else {
                                chunk.set_block_state(offset_x, y, offset_z, BlockState::GRASS);
                            }
                        }
                    }
                }
            }
        }

        let _ = state.sender.try_send((pos, chunk));
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
