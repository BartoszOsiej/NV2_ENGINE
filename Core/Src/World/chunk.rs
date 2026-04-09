use super::block::BlockType;
use super::biomes::BiomeGenerator;
use super::worldgen::WorldBlockWrite;

pub const CHUNK_W: usize = 16;
pub const CHUNK_H: usize = 256;
pub const CHUNK_D: usize = 16;

pub struct GeneratedChunk {
    pub chunk: Chunk,
    pub writes: Vec<WorldBlockWrite>,
}

pub struct Chunk {
    pub blocks: Box<[[[BlockType; CHUNK_D]; CHUNK_H]; CHUNK_W]>,
    // Per-voxel water metadata: 0 = no dynamic liquid state (also used by
    // generated/static water), 1-7 = flowing level, 8 = permanent source.
    pub water_meta: Box<[[[u8; CHUNK_D]; CHUNK_H]; CHUNK_W]>,
    pub is_dirty: bool,
}

impl Chunk {
    pub fn generate(cx: i32, cz: i32, gen: &BiomeGenerator) -> GeneratedChunk {
        let mut blocks = Box::new([[[BlockType::Air; CHUNK_D]; CHUNK_H]; CHUNK_W]);
        let mut water_meta = Box::new([[[0u8; CHUNK_D]; CHUNK_H]; CHUNK_W]);
        let mut writes = Vec::new();
        gen.populate_chunk(cx, cz, &mut blocks, &mut writes);

        GeneratedChunk {
            chunk: Self {
                blocks,
                water_meta,
                is_dirty: false,
            },
            writes,
        }
    }

    #[inline]
    pub fn get(&self, x: usize, y: usize, z: usize) -> &BlockType {
        &self.blocks[x][y][z]
    }

    #[inline]
    pub fn set(&mut self, x: usize, y: usize, z: usize, block: BlockType) {
        if x < CHUNK_W && y < CHUNK_H && z < CHUNK_D {
            self.blocks[x][y][z] = block;
            self.is_dirty = true;
        }
    }

    /// Water metadata helpers
    #[inline]
    pub fn water_meta_get(&self, x: usize, y: usize, z: usize) -> u8 {
        self.water_meta[x][y][z]
    }

    #[inline]
    pub fn water_meta_set(&mut self, x: usize, y: usize, z: usize, meta: u8) {
        if x < CHUNK_W && y < CHUNK_H && z < CHUNK_D {
            self.water_meta[x][y][z] = meta;
            self.is_dirty = true;
        }
    }

    /// Return the liquid level at local coordinates (0 = none, 1-7 = flow, 8 = source).
    #[inline]
    pub fn water_level(&self, x: usize, y: usize, z: usize) -> u8 {
        self.water_meta[x][y][z].min(8)
    }

    /// Set the liquid level (0-8) at local coordinates.
    #[inline]
    pub fn set_water_level(&mut self, x: usize, y: usize, z: usize, level: u8) {
        self.water_meta[x][y][z] = level.min(8);
        self.is_dirty = true;
    }

    pub fn flatten(&self) -> Vec<u8> {
        let expected = CHUNK_W * CHUNK_H * CHUNK_D;
        let mut data = Vec::with_capacity(expected * 2);
        // First write block ids
        for x in 0..CHUNK_W {
            for y in 0..CHUNK_H {
                for z in 0..CHUNK_D {
                    data.push(self.blocks[x][y][z].id());
                }
            }
        }
        // Then write water metadata for each cell in the same ordering
        for x in 0..CHUNK_W {
            for y in 0..CHUNK_H {
                for z in 0..CHUNK_D {
                    data.push(self.water_meta[x][y][z]);
                }
            }
        }
        data
    }

    pub fn from_flat(bytes: &[u8]) -> Self {
        let expected_cells = CHUNK_W * CHUNK_H * CHUNK_D;
        let expected_len = expected_cells * 2;
        assert_eq!(bytes.len(), expected_len, "chunk flat data has wrong length (expected blocks + water_meta)");

        let mut blocks = Box::new([[[BlockType::Air; CHUNK_D]; CHUNK_H]; CHUNK_W]);
        let mut water_meta = Box::new([[[0u8; CHUNK_D]; CHUNK_H]; CHUNK_W]);
        let mut idx = 0;
        for x in 0..CHUNK_W {
            for y in 0..CHUNK_H {
                for z in 0..CHUNK_D {
                    blocks[x][y][z] = BlockType::from_id_or_air(bytes[idx]);
                    idx += 1;
                }
            }
        }
        for x in 0..CHUNK_W {
            for y in 0..CHUNK_H {
                for z in 0..CHUNK_D {
                    water_meta[x][y][z] = bytes[idx];
                    idx += 1;
                }
            }
        }

        Self {
            blocks,
            water_meta,
            is_dirty: false,
        }
    }
}