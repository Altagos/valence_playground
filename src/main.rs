#![warn(clippy::pedantic)]
#![allow(clippy::module_name_repetitions)]
#![allow(clippy::needless_pass_by_value)]
#![allow(clippy::cast_precision_loss)]
#![allow(clippy::cast_possible_truncation)]
#![allow(clippy::cast_possible_wrap)]
#![allow(clippy::cast_sign_loss)]

mod gui;
mod minecraft;

#[macro_use]
extern crate tracing;

use std::{io, sync::Mutex};

use chrono::Local;
use gui::GuiPlugin;
use lazy_static::lazy_static;
use minecraft::MinecraftPlugin;
use tracing_subscriber::{fmt, prelude::*, EnvFilter};
use valence::prelude::*;

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

    let appender = tracing_appender::rolling::never(
        "./logs",
        format!("{}.log", Local::now().format("%d.%m.%Y_%H:%M:%S")),
    );
    let (non_blocking, _guard) = tracing_appender::non_blocking(appender);

    tracing_subscriber::registry()
        .with(EnvFilter::from_default_env())
        .with(fmt::layer().with_writer(io::stdout))
        .with(
            fmt::layer()
                .with_writer(non_blocking)
                .compact()
                .with_ansi(false),
        )
        .init();

    App::new()
        .add_plugin(MinecraftPlugin)
        .add_plugin(GuiPlugin)
        .run();
}
