use std::sync::Arc;

use serde::{Deserialize, Serialize};
use valence::prelude::ConnectionMode as ValenceConnectionMode;

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ServerConfig {
    pub max_connections: usize,
    pub max_view_distance: u8,
    pub connection_mode: ConnectionMode,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            max_connections: 20,
            max_view_distance: 20,
            connection_mode: ConnectionMode::default(),
        }
    }
}

#[derive(Debug, Default, Deserialize, Serialize, Clone)]
pub enum ConnectionMode {
    #[default]
    Online,
    OnlineNoProxy,
    Offline,
    BungeeCord,
    Velocity {
        secret: String,
    },
}

impl From<ConnectionMode> for ValenceConnectionMode {
    fn from(val: ConnectionMode) -> Self {
        match val {
            ConnectionMode::Online => ValenceConnectionMode::Online {
                prevent_proxy_connections: false,
            },
            ConnectionMode::OnlineNoProxy => ValenceConnectionMode::Online {
                prevent_proxy_connections: true,
            },
            ConnectionMode::Offline => ValenceConnectionMode::Offline,
            ConnectionMode::BungeeCord => ValenceConnectionMode::BungeeCord,
            ConnectionMode::Velocity { secret } => ValenceConnectionMode::Velocity {
                secret: Arc::from(secret),
            },
        }
    }
}
