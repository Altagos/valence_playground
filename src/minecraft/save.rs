use std::{
    collections::HashMap,
    fs::{FileType, OpenOptions as StdOpenOptions},
    io::{Read, Write},
};

use anyhow::Result;
use itertools::{iproduct, Itertools};
use rayon::prelude::*;
use tokio::{
    fs::{self, OpenOptions},
    io::AsyncWriteExt,
};
use valence::{
    prelude::{BlockState, Chunk},
    view::ChunkPos,
};
use walkdir::WalkDir;

use super::world_gen::chunk_worker::TerrainSettings;
use crate::{REGION_SIZE, SECTION_COUNT};

#[derive(PartialEq, Debug, serde::Deserialize, serde::Serialize)]
pub struct Region {
    pos: (i64, i64),
    settings: TerrainSettings,
    chunks: Vec<SaveChunk>,
}

impl Region {
    pub fn chunk(&self, pos: ChunkPos) -> Option<&SaveChunk> {
        self.chunks.iter().find(|&c| c.pos == (pos.x, pos.z))
    }

    pub fn region(regions: &Vec<Region>, pos: (i64, i64)) -> Option<&Region> {
        regions.iter().find(|&r| r.pos == pos)
    }

    pub fn chunk_from_regions(regions: &Vec<Region>, pos: ChunkPos) -> Option<&SaveChunk> {
        let (rpos_x, rpos_z) = chunkpos_to_regionpos(&pos);

        match Region::region(regions, (rpos_x, rpos_z)) {
            Some(r) => r.chunk(pos),
            None => None,
        }
    }
}

#[derive(PartialEq, Debug, serde::Deserialize, serde::Serialize)]
pub struct SaveChunk {
    pos: (i32, i32),
    blocks: Vec<Block>,
}

#[derive(PartialEq, Debug, serde::Deserialize, serde::Serialize)]
pub struct Block {
    x: usize,
    y: usize,
    z: usize,
    kind: u16,
}

pub fn chunkpos_to_regionpos(pos: &ChunkPos) -> (i64, i64) {
    let rpos_x = (pos.x as f64 / REGION_SIZE).floor() as i64;
    let rpos_z = (pos.z as f64 / REGION_SIZE).floor() as i64;

    (rpos_x, rpos_z)
}

pub fn overwrite_regions(chunks: &Vec<(ChunkPos, Chunk)>, settings: TerrainSettings) -> Result<()> {
    let mut regions = HashMap::new();

    for (pos, chunk) in chunks {
        let (rpos_x, rpos_z) = chunkpos_to_regionpos(pos);

        let mut region = match regions.get_mut(&(rpos_x, rpos_z)) {
            Some(r) => r,
            None => {
                let region = Region {
                    pos: (rpos_x, rpos_z),
                    settings: settings.clone(),
                    chunks: vec![],
                };
                regions.insert((rpos_x, rpos_z), region);
                regions.get_mut(&(rpos_x, rpos_z)).unwrap()
            }
        };

        let mut save_chunk = SaveChunk::from(chunk);
        save_chunk.pos = (pos.x, pos.z);
        region.chunks.push(save_chunk);
    }

    for (pos, region) in regions {
        let base_path = std::env::current_dir()?.join("world");
        std::fs::create_dir_all(&base_path)?;

        let path = base_path.join(format!("{}_{}.region", pos.0, pos.1));

        let mut file = StdOpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .open(path)?;
        let encoded: Vec<u8> = bincode::serialize(&region)?;
        file.write_all(encoded.as_slice())?;

        trace!(target: "minecraft::save", "saved {}_{}.region", pos.0, pos.1);
    }

    Result::Ok(())
}

pub fn save_chunk_to_region(chunk: Chunk, pos: ChunkPos, settings: TerrainSettings) -> Result<()> {
    let rpos = chunkpos_to_regionpos(&pos);
    let mut region = match load_region(rpos, &settings) {
        Ok(r) => r,
        Err(_) => Region {
            pos: rpos,
            settings,
            chunks: vec![],
        },
    };

    let mut save_chunk: SaveChunk = chunk.into();
    save_chunk.pos = (pos.x, pos.z);

    region.chunks.iter().map(|c| {
        if c.pos == save_chunk.pos {
            &save_chunk
        } else {
            c
        }
    });

    let base_path = std::env::current_dir()?.join("world");

    let path = base_path.join(format!("{}_{}.region", rpos.0, rpos.1));

    let mut file = StdOpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .open(path)?;
    let encoded: Vec<u8> = bincode::serialize(&region)?;
    file.write_all(encoded.as_slice())?;

    trace!(
        "saved chunk ({}, {}) to region {} {}",
        pos.x,
        pos.z,
        rpos.0,
        rpos.1
    );

    Result::Ok(())
}

pub fn load_region(pos: (i64, i64), settings: &TerrainSettings) -> Result<Region> {
    let base_path = std::env::current_dir()?.join("world");
    let path = base_path.join(format!("{}_{}.region", pos.0, pos.1));

    let mut buf = vec![];
    let mut file = StdOpenOptions::new().read(true).open(path)?;
    file.read_to_end(&mut buf);

    let region: Region = bincode::deserialize(&buf)?;
    if &region.settings == settings {
        Result::Ok(region)
    } else {
        Result::Err(anyhow::anyhow!("Terrain Settings don't match"))
    }
}

pub fn load_regions() -> Result<Vec<Region>> {
    let mut regions = vec![];

    let base_path = std::env::current_dir()?.join("world");
    for entry in WalkDir::new(base_path) {
        let entry = entry?;
        let file = entry.path().display();

        if entry.file_type().is_file() {
            if entry.path().extension().unwrap() == "region" {
                let mut buf = vec![];
                let mut file = StdOpenOptions::new().read(true).open(entry.path())?;
                file.read_to_end(&mut buf);

                let region: Region = bincode::deserialize(&buf)?;
                trace!(target: "minecraft::save", "loaded region {:?}", region.pos);

                regions.push(region);
            }
        }
    }

    Result::Ok(regions)
}

pub async fn save_chunk(chunk: Chunk, pos: ChunkPos) -> Result<()> {
    let base_path = std::env::current_dir()?.join("world");
    fs::create_dir_all(&base_path).await?;

    let path = base_path.join(format!("{}_{}.chunk", pos.x, pos.z));

    let mut file = OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .open(path)
        .await?;
    let mut save_chunk: SaveChunk = chunk.into();
    save_chunk.pos = (pos.x, pos.z);

    let encoded: Vec<u8> = bincode::serialize(&save_chunk)?;
    file.write_all(encoded.as_slice()).await?;

    Result::Ok(())
}

pub fn load_chunk(pos: &ChunkPos) -> Result<Chunk> {
    let base_path = std::env::current_dir()?.join("world");
    let path = base_path.join(format!("{}_{}.chunk", pos.x, pos.z));

    let mut buf = vec![];
    let mut file = StdOpenOptions::new().read(true).open(path)?;
    file.read_to_end(&mut buf);

    let save_chunk: SaveChunk = bincode::deserialize(&buf)?;

    Result::Ok(Chunk::from(save_chunk))
}

impl From<SaveChunk> for Chunk {
    fn from(value: SaveChunk) -> Self {
        let mut chunk = Chunk::new(SECTION_COUNT);

        value.blocks.iter().for_each(|c| {
            chunk.set_block_state(c.x, c.y, c.z, BlockState::from_raw(c.kind).unwrap());
        });

        chunk
    }
}

impl From<&SaveChunk> for Chunk {
    fn from(value: &SaveChunk) -> Self {
        let mut chunk = Chunk::new(SECTION_COUNT);

        value.blocks.iter().for_each(|c| {
            chunk.set_block_state(c.x, c.y, c.z, BlockState::from_raw(c.kind).unwrap());
        });

        chunk
    }
}

impl From<Chunk> for SaveChunk {
    fn from(value: Chunk) -> Self {
        let mut save_chunk = SaveChunk {
            pos: (0, 0),
            blocks: Vec::new(),
        };

        for (offset_z, offset_x) in iproduct!(0..16, 0..16) {
            for y in (0..value.section_count() * 16).rev() {
                let block = value.block_state(offset_x, y, offset_z);
                save_chunk.blocks.push(Block {
                    x: offset_x,
                    y,
                    z: offset_z,
                    kind: block.to_raw(),
                })
            }
        }

        save_chunk
    }
}

impl From<&Chunk> for SaveChunk {
    fn from(value: &Chunk) -> Self {
        let mut save_chunk = SaveChunk {
            pos: (0, 0),
            blocks: Vec::new(),
        };

        for (offset_z, offset_x) in iproduct!(0..16, 0..16) {
            for y in (0..value.section_count() * 16).rev() {
                let block = value.block_state(offset_x, y, offset_z);
                save_chunk.blocks.push(Block {
                    x: offset_x,
                    y,
                    z: offset_z,
                    kind: block.to_raw(),
                })
            }
        }

        save_chunk
    }
}
