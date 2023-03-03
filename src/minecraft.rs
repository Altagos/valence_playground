pub mod building;
mod callbacks;
pub mod chat;
pub mod client;
pub mod world_gen;

use bevy::prelude::Plugin;
use valence::{client::event::default_event_handler, prelude::*};

use self::{building::BuildingPlugin, chat::ChatPlugin, world_gen::WorldGenPlugin};
use crate::{
    minecraft::{callbacks::VPCallbacks, client::ClientPlugin},
    CONFIG,
};

pub struct MinecraftPlugin;

impl Plugin for MinecraftPlugin {
    #[cfg(feature = "minecraft")]
    fn build(&self, app: &mut bevy::prelude::App) {
        let connection_mode = CONFIG.server.connection_mode.clone().into();

        app.add_plugin(ServerPlugin::new(VPCallbacks).with_connection_mode(connection_mode))
            .add_plugin(BuildingPlugin)
            .add_plugin(ChatPlugin)
            .add_plugin(ClientPlugin)
            .add_plugin(WorldGenPlugin)
            .add_system_to_stage(EventLoop, default_event_handler);
    }

    #[cfg(not(feature = "minecraft"))]
    fn build(&self, _app: &mut bevy::prelude::App) {}
}
