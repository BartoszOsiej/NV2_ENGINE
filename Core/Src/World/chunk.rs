use super::block::BlockType;
use super::biomes::BiomeGenerator;

pub const CHUNK_W: usize = 16;
pub const CHUNK_H: usize = 256;
pub const CHUNK_D: usize = 16;

pub struct Chunk {
    pub blocks: Box<[[[BlockType; CHUNK_D]; CHUNK_H]; CHUNK_W]>,
    // Per-voxel water metadata: 0 = no liquid, 1-7 = flowing (level), 8 = permanent source.
    pub water_meta: Box<[[[u8; CHUNK_D]; CHUNK_H]; CHUNK_W]>,
    pub is_dirty: bool,
}

impl Chunk {
    pub fn generate(cx: i32, cz: i32, gen: &BiomeGenerator) -> Self {
        let mut blocks = Box::new([[[BlockType::Air; CHUNK_D]; CHUNK_H]; CHUNK_W]);
        let mut water_meta = Box::new([[[0u8; CHUNK_D]; CHUNK_H]; CHUNK_W]);

        for x in 0..CHUNK_W {
            for z in 0..CHUNK_D {
                let wx = cx * CHUNK_W as i32 + x as i32;
                let wz = cz * CHUNK_D as i32 + z as i32;

                let mut column = [BlockType::Air; CHUNK_H];
                gen.fill_column(wx, wz, &mut column);

                for y in 0..CHUNK_H {
                    blocks[x][y][z] = column[y];
                }
                // Mark every generated water block as a permanent source (level 8).
                // Source blocks spread without draining, so generated rivers, lakes, and
                // oceans remain stable and replenish any adjacent flowing water.
                for y in 0..CHUNK_H {
                    if blocks[x][y][z] == BlockType::Water {
                        water_meta[x][y][z] = 8; // source block
                    }
                }
            }
        }

        Self {
            blocks,
            water_meta,
            is_dirty: false,
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