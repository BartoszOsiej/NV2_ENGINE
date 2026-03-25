pub mod block;
pub mod chunk;
pub mod biomes;

use std::collections::HashMap;
use chunk::{Chunk, CHUNK_W, CHUNK_D};
use biomes::BiomeGenerator;
pub use block::BlockType;

pub struct World {
    pub chunks: HashMap<(i32, i32), Chunk>,
    generator: BiomeGenerator,
}

impl World {
    pub fn new(seed: u32) -> Self {
        Self {
            chunks: HashMap::new(),
            generator: BiomeGenerator::new(seed),
        }
    }

    /// Ensure chunks in radius around (cx, cz) are loaded.
    pub fn load_around(&mut self, cx: i32, cz: i32, radius: i32) {
        for dz in -radius..=radius {
            for dx in -radius..=radius {
                let key = (cx + dx, cz + dz);
                self.chunks.entry(key).or_insert_with(|| {
                    Chunk::generate(key.0, key.1, &self.generator)
                });
            }
        }
    }

    pub fn get_chunk(&self, cx: i32, cz: i32) -> Option<&Chunk> {
        self.chunks.get(&(cx, cz))
    }

    /// World-space block get.
    pub fn get_block(&self, wx: i32, wy: i32, wz: i32) -> BlockType {
        if wy < 0 || wy >= chunk::CHUNK_H as i32 { return BlockType::Air; }
        let cx = wx.div_euclid(CHUNK_W as i32);
        let cz = wz.div_euclid(CHUNK_D as i32);
        let lx = wx.rem_euclid(CHUNK_W as i32) as usize;
        let lz = wz.rem_euclid(CHUNK_D as i32) as usize;
        self.chunks
            .get(&(cx, cz))
            .map(|c| c.get(lx, wy as usize, lz))
            .unwrap_or(BlockType::Air)
    }
}