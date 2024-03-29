mod chunk;

use std::{
    collections::HashMap,
    fs::OpenOptions as StdOpenOptions,
    io::{Read, Write},
};

use anyhow::Result;
use tokio::{
    fs::{self, OpenOptions},
    io::AsyncWriteExt,
};
use valence::{prelude::Chunk, view::ChunkPos};
use walkdir::WalkDir;

pub use self::chunk::*;
use super::world_gen::chunk_worker::TerrainSettings;
use crate::REGION_SIZE;

#[derive(PartialEq, Debug, serde::Deserialize, serde::Serialize)]
pub struct Region {
    pos: (i64, i64),
    settings: TerrainSettings,
    chunks: Vec<SaveChunk>,
}

impl IntoIterator for Region {
    type IntoIter = std::vec::IntoIter<Self::Item>;
    type Item = SaveChunk;

    fn into_iter(self) -> Self::IntoIter { self.chunks.into_iter() }
}

impl Region {
    #[must_use]
    pub fn chunk(&self, pos: ChunkPos) -> Option<&SaveChunk> {
        self.chunks.iter().find(|&c| c.pos == (pos.x, pos.z))
    }

    #[must_use]
    pub fn region(regions: &Vec<Region>, pos: (i64, i64)) -> Option<&Region> {
        regions.iter().find(|&r| r.pos == pos)
    }

    #[must_use]
    pub fn chunk_from_regions(regions: &Vec<Region>, pos: ChunkPos) -> Option<&SaveChunk> {
        let (rpos_x, rpos_z) = chunkpos_to_regionpos(&pos);

        match Region::region(regions, (rpos_x, rpos_z)) {
            Some(r) => r.chunk(pos),
            None => None,
        }
    }
}

#[must_use]
pub fn chunkpos_to_regionpos(pos: &ChunkPos) -> (i64, i64) {
    let rpos_x = (f64::from(pos.x) / REGION_SIZE).floor() as i64;
    let rpos_z = (f64::from(pos.z) / REGION_SIZE).floor() as i64;

    (rpos_x, rpos_z)
}

pub fn overwrite_regions(chunks: &Vec<(ChunkPos, Chunk)>, settings: TerrainSettings) -> Result<()> {
    let mut regions = HashMap::new();

    for (pos, chunk) in chunks {
        let (rpos_x, rpos_z) = chunkpos_to_regionpos(pos);

        let region = match regions.get_mut(&(rpos_x, rpos_z)) {
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

    let _ = region
        .chunks
        .iter()
        .map(|c| {
            if c.pos == save_chunk.pos {
                &save_chunk
            } else {
                c
            }
        })
        .collect::<Vec<_>>();

    if let Some(mut c) = region.chunks.iter_mut().find(|c| c.pos == save_chunk.pos) {
        std::mem::swap(&mut c, &mut &mut save_chunk);
    } else {
        region.chunks.push(save_chunk);
    }

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
    let _ = file.read_to_end(&mut buf);

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
        let _file = entry.path().display();

        if entry.file_type().is_file() && entry.path().extension().unwrap() == "region" {
            let mut buf = vec![];
            let mut file = StdOpenOptions::new().read(true).open(entry.path())?;
            let _ = file.read_to_end(&mut buf);

            let region: Region = bincode::deserialize(&buf)?;
            trace!(target: "minecraft::save", "loaded region {:?}", region.pos);

            regions.push(region);
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
    let _ = file.read_to_end(&mut buf);

    let save_chunk: SaveChunk = bincode::deserialize(&buf)?;

    Result::Ok(Chunk::from(save_chunk))
}
