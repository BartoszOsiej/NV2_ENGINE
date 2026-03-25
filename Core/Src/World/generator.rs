pub struct WorldGenerator {
    pub seed: u32,
}

impl WorldGenerator {
    pub fn generate_chunk(&self, _chunk_x: i32, _chunk_z: i32) -> Vec<u32> {
        let mut blocks = vec![0; 16 * 16 * 256]; 
        
        for x in 0..16 {
            for z in 0..16 {
                for y in 0..256 {
                    let index = (y * 16 * 16) + (z * 16) + x;
                    if y == 64 { blocks[index] = 1; } // Trawa
                    else if y < 64 && y > 50 { blocks[index] = 2; } // Ziemia
                    else if y <= 50 { blocks[index] = 3; } // Kamień
                }
            }
        }
        blocks
    }
}