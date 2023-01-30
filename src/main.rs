use std::net::SocketAddr;

use rand::Rng;
use valence_new::{
    client::{
        despawn_disconnected_clients,
        event::{default_event_handler, ChatMessage},
    },
    player_list::{remove_disconnected_clients_from_player_list, Entry},
    prelude::*,
};

const SPAWN_Y: i32 = 64;
const PLAYER_UUID_1: Uuid = Uuid::from_u128(1);
const PLAYER_UUID_2: Uuid = Uuid::from_u128(2);

pub fn main() {
    App::new()
        .add_plugin(ServerPlugin::new(MyCallbacks).with_connection_mode(ConnectionMode::Offline))
        .add_startup_system(setup)
        .add_system(init_clients)
        .add_system(update_player_list)
        .add_system(default_event_handler)
        .add_system(despawn_disconnected_clients)
        .add_system(remove_disconnected_clients_from_player_list)
        .add_system(chat_message)
        .run();
}

struct MyCallbacks;

#[async_trait]
impl AsyncCallbacks for MyCallbacks {
    async fn server_list_ping(
        &self,
        _shared: &SharedServer,
        remote_addr: SocketAddr,
        _protocol_version: i32,
    ) -> ServerListPing {
        ServerListPing::Respond {
            online_players: 42,
            max_players: 420,
            player_sample: vec![PlayerSampleEntry {
                name: "foobar".into(),
                id: Uuid::from_u128(12345),
            }],
            description: "Your IP address is ".into_text()
                + remote_addr.to_string().color(Color::GOLD),
            favicon_png: include_bytes!("../assets/logo-64x64.png"),
        }
    }

    // async fn login(&self, _shared: &SharedServer, _info: &NewClientInfo) -> Result<(), Text> {
    //     return Err("You are not meant to join this example".color(Color::RED));
    // }
}

fn setup(world: &mut World) {
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
            instance.set_block_state([x, SPAWN_Y, z], BlockState::LIGHT_GRAY_WOOL);
        }
    }

    world.spawn(instance);

    let mut player_list = world.resource_mut::<PlayerList>();

    player_list.insert(
        PLAYER_UUID_1,
        PlayerListEntry::new().with_display_name(Some("persistent entry with no ping")),
    );
}

fn init_clients(
    mut clients: Query<&mut Client, Added<Client>>,
    instances: Query<Entity, With<Instance>>,
    mut player_list: ResMut<PlayerList>,
) {
    let instance = instances.get_single().unwrap();

    for mut client in &mut clients {
        client.set_position([0.0, SPAWN_Y as f64 + 1.0, 0.0]);
        client.set_instance(instance);
        client.set_game_mode(GameMode::Creative);

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
            .with_display_name(Some("ඞ".color(Color::new(255, 87, 66))));

        player_list.insert(client.uuid(), entry);
    }
}

fn chat_message(
    mut clients: Query<(&mut Client, Option<&mut McEntity>)>,
    mut msg: EventReader<ChatMessage>,
) {
    while let Some(msg) = msg.iter().next() {
        let sender;

        {
            let (client, _) = clients.get(msg.client).unwrap();
            sender = client;
        }

        let username = match sender.player().get_custom_name() {
            Some(name) => name.clone(),
            None => Text::from(sender.username().to_string()),
        };

        for (mut c, _) in clients.iter_mut() {
            c.send_message(format!("{}: {}", username, msg.message));
        }
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

        let entry = player_list.get_mut(PLAYER_UUID_1).unwrap();
        let new_display_name = entry.display_name().unwrap().clone().color(color);
        entry.set_display_name(Some(new_display_name));
    }

    if tick % 20 == 0 {
        match player_list.entry(PLAYER_UUID_2) {
            Entry::Occupied(oe) => {
                oe.remove();
            }
            Entry::Vacant(ve) => {
                let entry = PlayerListEntry::new()
                    .with_display_name(Some("Hello!"))
                    .with_ping(300);

                ve.insert(entry);
            }
        }
    }
}
