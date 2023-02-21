pub mod building;
pub mod chat;
pub mod world_gen;

use std::net::SocketAddr;

use bevy::prelude::Plugin;
use valence::{
    client::{despawn_disconnected_clients, event::default_event_handler},
    prelude::*,
};

use self::{building::BuildingPlugin, chat::ChatPlugin, world_gen::WorldGenPlugin};
use crate::{VPSystems, MAX_CONNECTIONS, MAX_VIEW_DISTANCE, PLAYER_COUNT, SPAWN_POS};

pub struct MinecraftPlugin;

impl Plugin for MinecraftPlugin {
    #[cfg(feature = "minecraft")]
    fn build(&self, app: &mut bevy::prelude::App) {
        let connection_mode;

        cfg_if::cfg_if! {
            if #[cfg(feature = "online")] {
                connection_mode = ConnectionMode::Online {
                    prevent_proxy_connections: false,
                }
            } else {
                connection_mode = ConnectionMode::Offline
            }
        }

        app.add_plugin(ServerPlugin::new(MyCallbacks).with_connection_mode(connection_mode))
            .add_plugin(BuildingPlugin)
            .add_plugin(ChatPlugin)
            .add_plugin(WorldGenPlugin)
            .add_system_to_stage(EventLoop, default_event_handler)
            .add_system_set(PlayerList::default_system_set())
            .add_system(init_clients.label(VPSystems::InitClients))
            .add_system(update_player_list)
            .add_system(update_player_count)
            .add_system(despawn_disconnected_clients);
    }

    #[cfg(not(feature = "minecraft"))]
    fn build(&self, _app: &mut bevy::prelude::App) {}
}

fn init_clients(
    mut clients: Query<&mut Client, Added<Client>>,
    instances: Query<Entity, With<Instance>>,
    mut player_list: ResMut<PlayerList>,
) {
    let instance = instances.get_single().unwrap();
    let spawn = SPAWN_POS.lock().unwrap().clone();

    for mut client in &mut clients {
        client.set_position([spawn.x, spawn.y, spawn.z]);
        client.set_instance(instance);
        client.set_game_mode(GameMode::Creative);
        client.set_op_level(2);

        client.set_view_distance(MAX_VIEW_DISTANCE);

        let entry = PlayerListEntry::new()
            .with_username(client.username())
            .with_properties(client.properties()) // For the player's skin and cape.
            .with_game_mode(client.game_mode())
            .with_ping(-1) // Use negative values to indicate missing.
            .with_display_name(Some(client.username().color(Color::new(255, 87, 66))));

        player_list.insert(client.uuid(), entry);
        *PLAYER_COUNT.lock().unwrap() += 1;
    }
}

fn update_player_list(mut player_list: ResMut<PlayerList>) {
    player_list.set_header("Just a normal minecraft server".into_text());
    player_list.set_footer(format!(
        "{}/{}",
        *PLAYER_COUNT.lock().unwrap(),
        MAX_CONNECTIONS
    ));
}

fn update_player_count(clients: Query<(Entity, &Client)>) {
    for (_entity, client) in &clients {
        if client.is_disconnected() {
            *PLAYER_COUNT.lock().unwrap() -= 1;
        }
    }
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
        _remote_addr: SocketAddr,
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
            return Ok(());
        }
        return Err("Server full".color(Color::RED));
    }
}
