use std::ops::RangeInclusive;

use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct WorldConfig {
    pub seed: Seed,
    pub chunks_cached: usize,
    pub pregen_chunks: RangeInclusive<i32>,
}

impl Default for WorldConfig {
    fn default() -> Self {
        Self {
            seed: Default::default(),
            chunks_cached: 4000,
            pregen_chunks: -12..=12,
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

impl Into<u32> for Seed {
    fn into(self) -> u32 {
        match self {
            Seed::Random => rand::random(),
            Seed::Set(s) => s,
        }
    }
}
