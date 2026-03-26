use noise::{NoiseFn, Perlin};
use super::block::BlockType;

pub struct BiomeGenerator {
    height_noise: Perlin,
    biome_noise:  Perlin,
}

impl BiomeGenerator {
    pub fn new(seed: u32) -> Self {
        Self {
            height_noise: Perlin::new(seed),
            biome_noise:  Perlin::new(seed.wrapping_add(1337)),
        }
    }

    pub fn surface_height(&self, wx: i32, wz: i32) -> u32 {
        let nx = wx as f64 * 0.008;
        let nz = wz as f64 * 0.008;

        let h = self.height_noise.get([nx, nz])
              + 0.5   * self.height_noise.get([nx * 2.0, nz * 2.0])
              + 0.25  * self.height_noise.get([nx * 4.0, nz * 4.0])
              + 0.125 * self.height_noise.get([nx * 8.0, nz * 8.0]);

        // remap [-1.875, 1.875] → [40, 120]
        let normalized = (h / 1.875 + 1.0) * 0.5;
        (40.0 + normalized * 80.0) as u32
    }

    pub fn temperature(&self, wx: i32, wz: i32) -> f64 {
        let nx = wx as f64 * 0.003;
        let nz = wz as f64 * 0.003;
        (self.biome_noise.get([nx, nz]) + 1.0) * 0.5
    }

    pub fn fill_column(&self, wx: i32, wz: i32, column: &mut [BlockType; 256]) {
        let surface = self.surface_height(wx, wz) as usize;
        let temp    = self.temperature(wx, wz);

        // Bedrock
        column[0] = BlockType::Stone;

        // Stone fill up to near surface
        for y in 1..surface.saturating_sub(4) {
            column[y] = BlockType::Stone;

            // Add ores using deterministic coordinate pattern so variants are used.
            if y > 4 && y < 50 {
                let hash = (wx.wrapping_mul(31).wrapping_add(wz).wrapping_add(y as i32)) as i32;
                let bucket = hash.abs() % 100;
                if bucket < 2 {
                    column[y] = BlockType::CoalOre;
                } else if bucket < 3 {
                    column[y] = BlockType::GoldOre;
                } else if bucket < 4 {
                    column[y] = BlockType::DiamondOre;
                }
            }
        }

        if surface >= 4 {
            let sub_start = surface - 4;
            let sub_end   = surface - 1;

            if temp < 0.25 {
                // Arctic
                for y in sub_start..sub_end { column[y] = BlockType::Stone; }
                column[sub_end] = BlockType::Snow;
                column[surface] = BlockType::Snow;
            } else if temp < 0.45 {
                // Cold highlands
                for y in sub_start..sub_end { column[y] = BlockType::Dirt; }
                column[sub_end] = BlockType::Dirt;
                column[surface] = BlockType::SnowGrass;
            } else if temp < 0.75 {
                // Temperate
                for y in sub_start..sub_end { column[y] = BlockType::Dirt; }
                column[sub_end] = BlockType::Dirt;
                column[surface] = BlockType::Grass;
            } else {
                // Desert
                for y in sub_start..=surface      { column[y] = BlockType::Sand; }
                for y in sub_start..sub_start + 2 { column[y] = BlockType::Gravel; }
            }
        }

        // Fill below sea level with water
        const SEA_LEVEL: usize = 50;
        for y in (surface + 1)..=SEA_LEVEL {
            if column[y] == BlockType::Air {
                column[y] = BlockType::Water;
            }
        }
    }
}