// #![feature(stmt_expr_attributes)]
#![warn(clippy::pedantic)]
#![allow(
    clippy::module_name_repetitions,
    clippy::needless_pass_by_value,
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_sign_loss,
    clippy::let_underscore_untyped,
    clippy::similar_names
)]

pub mod config;
pub mod gui;
pub mod minecraft;
pub mod util;

#[macro_use]
extern crate tracing;

use std::sync::Mutex;

use config::Config;
use lazy_static::lazy_static;
use valence::prelude::*;

pub const SECTION_COUNT: usize = 24;
pub const REGION_SIZE: f64 = 16.0;

lazy_static! {
    pub static ref PLAYER_COUNT: Mutex<usize> = Mutex::new(0);
    pub static ref SPAWN_POS: Mutex<DVec3> = Mutex::new(DVec3::new(0.0, 200.0, 0.0));
    pub static ref CONFIG: Config = Config::from_current_dir().unwrap();
}
