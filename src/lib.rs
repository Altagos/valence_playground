#![warn(clippy::pedantic)]
#![allow(clippy::module_name_repetitions)]
#![allow(clippy::needless_pass_by_value)]
#![allow(clippy::cast_precision_loss)]
#![allow(clippy::cast_possible_truncation)]
#![allow(clippy::cast_possible_wrap)]
#![allow(clippy::cast_sign_loss)]

pub mod gui;
pub mod minecraft;

#[macro_use]
extern crate tracing;

use std::{ops::RangeInclusive, sync::Mutex};

use lazy_static::lazy_static;
use valence::prelude::*;

pub const SPAWN_Y: i32 = 64;
pub const PLAYER_UUID_1: Uuid = Uuid::from_u128(1);
pub const PLAYER_UUID_2: Uuid = Uuid::from_u128(2);
pub const MAX_CONNECTIONS: usize = 20;
pub const SECTION_COUNT: usize = 24;
pub const PREGEN_CHUNKS: RangeInclusive<i32> = -12..=12;
pub const MAX_VIEW_DISTANCE: u8 = 10;

lazy_static! {
    pub static ref PLAYER_COUNT: Mutex<usize> = Mutex::new(0);
    pub static ref SPAWN_POS: Mutex<DVec3> = Mutex::new(DVec3::new(0.0, 200.0, 0.0));
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, SystemLabel)]
pub enum VPSystems {
    InitClients,
}
