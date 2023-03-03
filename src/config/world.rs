use std::ops::RangeInclusive;

use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct WorldConfig {
    pub seed: Seed,
    pub chunks_cached: usize,
    pub spawn: Option<[f64; 3]>,
    pub pregen_chunks: RangeInclusive<i32>,
}

impl Default for WorldConfig {
    fn default() -> Self {
        Self {
            seed: Seed::default(),
            chunks_cached: 4000,
            spawn: None,
            pregen_chunks: -22..=22,
        }
    }
}

#[derive(
    Debug, Default, Deserialize, Serialize, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash,
)]
pub enum Seed {
    #[default]
    Random,
    Set(u32),
}

impl From<Seed> for u32 {
    fn from(val: Seed) -> Self {
        match val {
            Seed::Random => rand::random(),
            Seed::Set(s) => s,
        }
    }
}
