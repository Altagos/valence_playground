pub mod building;
mod callbacks;
pub mod chat;
pub mod world_gen;

use bevy::prelude::Plugin;
use valence::{
    client::{despawn_disconnected_clients, event::default_event_handler},
    prelude::*,
};

use self::{building::BuildingPlugin, chat::ChatPlugin, world_gen::WorldGenPlugin};
use crate::{VPSystems, CONFIG, PLAYER_COUNT, SPAWN_POS};

pub struct MinecraftPlugin;

impl Plugin for MinecraftPlugin {
    #[cfg(feature = "minecraft")]
    fn build(&self, app: &mut bevy::prelude::App) {
        use crate::minecraft::callbacks::VPCallbacks;

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

        app.add_plugin(ServerPlugin::new(VPCallbacks).with_connection_mode(connection_mode))
            .add_plugin(BuildingPlugin)
            .add_plugin(ChatPlugin)
            .add_plugin(WorldGenPlugin)
            .add_system_to_stage(EventLoop, default_event_handler)
            .add_system_set(PlayerList::default_system_set())
            .add_system(init_clients.label(VPSystems::InitClients))
            .add_system(update_player_list)
            .add_system(update_player_count)
            .add_system(despawn_disconnected_clients)
            .add_system(set_view_distance);
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

        client.set_view_distance(CONFIG.server.max_view_distance);

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
        CONFIG.server.max_connections
    ));
}

fn update_player_count(clients: Query<&Client>) {
    for client in &clients {
        if client.is_disconnected() {
            *PLAYER_COUNT.lock().unwrap() -= 1;
        }
    }
}

fn set_view_distance(mut clients: Query<&mut Client>) {
    clients.par_for_each_mut(16, |mut c| {
        if c.view_distance() > CONFIG.server.max_view_distance {
            c.set_view_distance(CONFIG.server.max_view_distance)
        }
    });
}
