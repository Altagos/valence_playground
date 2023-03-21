use itertools::iproduct;
use valence::prelude::{BlockState, Chunk};

use crate::SECTION_COUNT;

pub type OffsetBlockPos = (usize, usize, usize);
pub type SaveChunkIteratorItem = (OffsetBlockPos, BlockState);

#[derive(PartialEq, Debug, serde::Deserialize, serde::Serialize, Clone)]
pub struct SaveChunk {
    pub pos: (i32, i32),
    pub blocks: Vec<Block>,
}

impl IntoIterator for SaveChunk {
    type IntoIter = SaveChunkIterator;
    type Item = SaveChunkIteratorItem;

    fn into_iter(self) -> Self::IntoIter {
        SaveChunkIterator {
            blocks: self.blocks,
            next: SaveChunkId(0),
        }
    }
}

pub struct SaveChunkId(usize);

pub struct SaveChunkIterator {
    blocks: Vec<Block>,
    next: SaveChunkId,
}

impl Iterator for SaveChunkIterator {
    type Item = SaveChunkIteratorItem;

    fn next(&mut self) -> Option<Self::Item> {
        let next = self.next.0;
        let Some(block) = self.blocks.get(next) else { return None; };

        self.next.0 = next + 1;

        Some((
            (block.x, block.y, block.z),
            BlockState::from_raw(block.kind)?,
        ))
    }
}

#[derive(PartialEq, Debug, serde::Deserialize, serde::Serialize, Clone, Copy)]
pub struct Block {
    pub x: usize,
    pub y: usize,
    pub z: usize,
    pub kind: u16,
}

impl From<SaveChunk> for Chunk {
    fn from(value: SaveChunk) -> Self {
        let mut chunk = Chunk::new(SECTION_COUNT);

        for (pos, block) in value.into_iter() {
            chunk.set_block_state(pos.0, pos.1, pos.2, block);
        }

        chunk
    }
}

impl From<&SaveChunk> for Chunk {
    fn from(value: &SaveChunk) -> Self {
        let mut chunk = Chunk::new(SECTION_COUNT);

        for (pos, block) in value.clone().into_iter() {
            chunk.set_block_state(pos.0, pos.1, pos.2, block);
        }

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
