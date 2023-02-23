use std::net::SocketAddr;

use valence::prelude::*;

use crate::{CONFIG, PLAYER_COUNT};

#[derive(Default)]
pub struct VPCallbacks;

#[async_trait]
impl AsyncCallbacks for VPCallbacks {
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
            max_players: CONFIG.server.max_connections as i32,
            player_sample: vec![],
            description: "Just a minecraft server".color(Color::WHITE),
            favicon_png: include_bytes!("../../assets/logo-64x64.png"),
        }
    }

    async fn login(&self, _shared: &SharedServer, _info: &NewClientInfo) -> Result<(), Text> {
        // return Err("You are not meant to join this example".color(Color::RED));

        if CONFIG.server.max_connections > *PLAYER_COUNT.lock().unwrap() {
            return Ok(());
        }
        return Err("Server full".color(Color::RED));
    }
}
