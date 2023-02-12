#![warn(clippy::pedantic)]
#![allow(clippy::module_name_repetitions)]
#![allow(clippy::needless_pass_by_value)]

mod gui;
mod minecraft;

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
use flume::{Receiver, Sender};
use gui::GuiPlugin;
use lazy_static::lazy_static;
use minecraft::MinecraftPlugin;
use noise::{NoiseFn, SuperSimplex};
use rand::Rng;
use valence::{
    client::{
        despawn_disconnected_clients,
        event::{default_event_handler, ChatCommand, FinishDigging, StartDigging, UseItemOnBlock},
    },
    prelude::*,
    protocol::types::Hand,
};

pub const SPAWN_Y: i32 = 64;
pub const PLAYER_UUID_1: Uuid = Uuid::from_u128(1);
pub const PLAYER_UUID_2: Uuid = Uuid::from_u128(2);
pub const MAX_CONNECTIONS: usize = 20;
pub const SECTION_COUNT: usize = 24;

lazy_static! {
    pub static ref PLAYER_COUNT: Mutex<usize> = Mutex::new(0);
    pub static ref SPAWN_POS: DVec3 = DVec3::new(0.0, 200.0, 0.0);
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, SystemLabel)]
pub enum VPSystems {
    InitClients,
}

pub fn main() {
    dotenv::dotenv().ok();

    App::new()
        .add_plugin(MinecraftPlugin)
        .add_plugin(GuiPlugin)
        .run();
}
