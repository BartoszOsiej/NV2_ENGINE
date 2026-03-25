use super::block::BlockType;
use super::biomes::BiomeGenerator;

pub const CHUNK_W: usize = 16;
pub const CHUNK_H: usize = 256;
pub const CHUNK_D: usize = 16;

pub struct Chunk {
    /// blocks[x][y][z]
    pub blocks: Box<[[[BlockType; CHUNK_D]; CHUNK_H]; CHUNK_W]>,
    pub dirty: bool,
}

impl Chunk {
    pub fn generate(cx: i32, cz: i32, gen: &BiomeGenerator) -> Self {
        let mut blocks = Box::new(
            [[[BlockType::Air; CHUNK_D]; CHUNK_H]; CHUNK_W]
        );

        for x in 0..CHUNK_W {
            for z in 0..CHUNK_D {
                let wx = cx * CHUNK_W as i32 + x as i32;
                let wz = cz * CHUNK_D as i32 + z as i32;

                let mut column = [BlockType::Air; CHUNK_H];
                gen.fill_column(wx, wz, &mut column);

                for y in 0..CHUNK_H {
                    blocks[x][y][z] = column[y];
                }
            }
        }

        Self { blocks, dirty: true }
    }

    #[inline]
    pub fn get(&self, x: usize, y: usize, z: usize) -> BlockType {
        self.blocks[x][y][z]
    }

    /// Safe neighbour-aware get (returns Air for out-of-bounds y)
    #[inline]
    pub fn get_safe(&self, x: i32, y: i32, z: i32) -> Option<BlockType> {
        if x < 0 || x >= CHUNK_W as i32
        || y < 0 || y >= CHUNK_H as i32
        || z < 0 || z >= CHUNK_D as i32 {
            return None; // caller must query neighbour chunk
        }
        Some(self.blocks[x as usize][y as usize][z as usize])
    }
}