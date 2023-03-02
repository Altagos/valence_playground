pub mod building;
mod callbacks;
pub mod chat;
pub mod world_gen;

use bevy::prelude::Plugin;
use rand::Rng;
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

        let connection_mode = CONFIG.server.connection_mode.clone().into();

        app.add_plugin(ServerPlugin::new(VPCallbacks).with_connection_mode(connection_mode))
            .add_plugin(BuildingPlugin)
            .add_plugin(ChatPlugin)
            .add_plugin(WorldGenPlugin)
            .add_system_to_stage(EventLoop, default_event_handler)
            .add_system_set(PlayerList::default_system_set())
            .add_system(init_clients.label(VPSystems::InitClients))
            .add_system(update_player_list)
            .add_system(player_left)
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
    let spawn = *SPAWN_POS.lock().unwrap();
    let mut new_players = vec![];

    for mut client in &mut clients {
        client.set_position([spawn.x, spawn.y, spawn.z]);
        client.set_instance(instance);
        client.set_game_mode(GameMode::Creative);
        client.set_op_level(2);

        let mut rng = rand::thread_rng();
        let name_color = Color::new(
            rng.gen_range(0..=255),
            rng.gen_range(0..=255),
            rng.gen_range(0..=255),
        );

        let username = client.username().to_owned_username().color(name_color);
        client.player_mut().set_custom_name(Some(username.clone()));

        client.set_view_distance(CONFIG.server.max_view_distance);

        let entry = PlayerListEntry::new()
            .with_username(client.username())
            .with_properties(client.properties()) // For the player's skin and cape.
            .with_game_mode(client.game_mode())
            .with_ping(client.ping()) // Use negative values to indicate missing.
            .with_display_name(Some(
                client.username().to_owned_username().color(name_color),
            ));

        info!(target: "minecraft", "{} joined", client.username().to_string());
        new_players.push(username);
        player_list.insert(client.uuid(), entry);
        *PLAYER_COUNT.lock().unwrap() += 1;
    }

    clients.par_for_each_mut(16, |mut c| {
        for name in &new_players {
            c.send_message(name.clone() + " joined".to_string().color(Color::YELLOW));
        }
    });
}

fn update_player_list(mut player_list: ResMut<PlayerList>) {
    player_list.set_header("Just a normal minecraft server".into_text());
    player_list.set_footer(format!(
        "{}/{}",
        *PLAYER_COUNT.lock().unwrap(),
        CONFIG.server.max_connections
    ));
}

fn player_left(mut clients: Query<&mut Client>) {
    let mut players = vec![];

    for client in &clients {
        if client.is_disconnected() {
            let username = match client.player().get_custom_name() {
                Some(u) => u.clone(),
                None => client.username().to_string().into_text(),
            };
            players.push(username.clone());
            info!(target: "minecraft", "{} left", client.username().to_string());
            *PLAYER_COUNT.lock().unwrap() -= 1;
        }
    }

    clients.par_for_each_mut(16, |mut c| {
        for name in &players {
            c.send_message(name.clone() + " left".to_string().color(Color::YELLOW));
        }
    });
}

fn set_view_distance(mut clients: Query<&mut Client>) {
    clients.par_for_each_mut(16, |mut c| {
        if c.view_distance() > CONFIG.server.max_view_distance {
            c.set_view_distance(CONFIG.server.max_view_distance);
        }
    });
}
