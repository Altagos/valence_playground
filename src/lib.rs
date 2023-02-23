#![warn(clippy::pedantic)]
#![allow(clippy::module_name_repetitions)]
#![allow(clippy::needless_pass_by_value)]
#![allow(clippy::cast_precision_loss)]
#![allow(clippy::cast_possible_truncation)]
#![allow(clippy::cast_possible_wrap)]
#![allow(clippy::cast_sign_loss)]

pub mod config;
pub mod gui;
pub mod minecraft;

#[macro_use]
extern crate tracing;

use std::sync::Mutex;

use config::Config;
use lazy_static::lazy_static;
use valence::prelude::*;

pub const SECTION_COUNT: usize = 24;

lazy_static! {
    pub static ref PLAYER_COUNT: Mutex<usize> = Mutex::new(0);
    pub static ref SPAWN_POS: Mutex<DVec3> = Mutex::new(DVec3::new(0.0, 200.0, 0.0));
    pub static ref CONFIG: Config = Config::from_current_dir().unwrap();
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, SystemLabel)]
pub enum VPSystems {
    InitClients,
}
