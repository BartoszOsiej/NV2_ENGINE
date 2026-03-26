use super::block::BlockType;
use super::biomes::BiomeGenerator;

pub const CHUNK_W: usize = 16;
pub const CHUNK_H: usize = 256;
pub const CHUNK_D: usize = 16;

pub struct Chunk {
    pub blocks: Box<[[[BlockType; CHUNK_D]; CHUNK_H]; CHUNK_W]>,
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

        Self { blocks }
    }

    #[inline]
    pub fn get(&self, x: usize, y: usize, z: usize) -> BlockType {
        self.blocks[x][y][z]
    }
}