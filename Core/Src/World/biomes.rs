use noise::{NoiseFn, Perlin};
use super::block::BlockType;

/// Y level below which terrain is flooded with water.
pub const SEA_LEVEL: usize = 44;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Biome {
    Plains,
    ForestDense,
    ForestSparse,
    Mountains,
    Beach,
    Desert,
    Snowy,
    Swamp,
    River,
    Glade,
    /// Boreal cold forest (spruce-like)
    Taiga,
    /// Hot, flat, dry grassland
    Savanna,
    /// Deep open sea — surface well below SEA_LEVEL
    Ocean,
    /// Cold, flat, barren permafrost
    Tundra,
}

/// Advanced biome generator with Minecraft-style terrain features
/// Features: caves, ores, trees, varied terrain, biome transitions
pub struct BiomeGenerator {
    // Terrain noise sources
    height_noise: Perlin,      // Primary terrain height
    biome_noise: Perlin,       // Biome classification (temp/humidity)
    detail_noise: Perlin,      // Local terrain variation
        river_noise: Perlin,       // Large-scale river mask
        glade_noise: Perlin,       // Clearings inside forests
    
    // Feature generation
    cave_noise: Perlin,        // Cave system generation
    ore_noise: Perlin,         // Ore vein distribution
    tree_noise: Perlin,        // Tree placement
    
    seed: u32,
}

impl BiomeGenerator {
    pub fn new(seed: u32) -> Self {
        Self {
            height_noise: Perlin::new(seed),
            biome_noise: Perlin::new(seed.wrapping_add(1337)),
            detail_noise: Perlin::new(seed.wrapping_add(2555)),
            cave_noise: Perlin::new(seed.wrapping_add(3773)),
            ore_noise: Perlin::new(seed.wrapping_add(4991)),
            tree_noise: Perlin::new(seed.wrapping_add(6209)),
            river_noise: Perlin::new(seed.wrapping_add(7439)),
            glade_noise: Perlin::new(seed.wrapping_add(8597)),
            seed,
        }
    }

    pub fn seed(&self) -> u32 {
        self.seed
    }

    /// Determine biome at world coordinates based on temperature and humidity
    pub fn get_biome(&self, wx: i32, wz: i32) -> Biome {
        let nx = wx as f64 * 0.0025;
        let nz = wz as f64 * 0.0025;

        let temp = self.biome_noise.get([nx, nz]);
        let humidity = self.biome_noise.get([nx + 100.0, nz + 100.0]);
        let elev = (self.base_height(wx, wz) / 1.875 + 1.0) * 0.5;

        // Rivers take absolute precedence when the river mask is near zero
        let rnoise = self.river_noise.get([wx as f64 * 0.0015, wz as f64 * 0.0015]);
        if rnoise.abs() < 0.018 {
            return Biome::River;
        }

        // Deep ocean: very low elevation
        if elev < 0.18 {
            return Biome::Ocean;
        }

        // Build feature vector and find nearest biome centre
        let feats = (temp, humidity, elev);
        let mut best = Biome::Plains;
        let mut best_score = f64::INFINITY;

        for (b, cent) in Self::biome_feature_centers().iter() {
            let d2 = (feats.0 - cent.0).powi(2)
                   + (feats.1 - cent.1).powi(2)
                   + (feats.2 - cent.2).powi(2);
            if d2 < best_score { best_score = d2; best = *b; }
        }

        // Heuristic overrides
        if feats.2 > 0.82 {
            return if feats.0 < -0.10 { Biome::Snowy } else { Biome::Mountains };
        }
        if feats.1 > 0.55 && feats.2 < 0.25 { return Biome::Swamp; }
        if feats.0 < -0.55 && feats.1 < 0.10 { return Biome::Tundra; }

        best
    }

    /// Returns a list of (Biome, feature_center) where each center is (temp, humidity, elev)
    fn biome_feature_centers() -> Vec<(Biome, (f64, f64, f64))> {
        vec![
            (Biome::Plains,       ( 0.0,  0.0,  0.47)),
            (Biome::ForestDense,  ( 0.1,  0.55, 0.52)),
            (Biome::ForestSparse, ( 0.05, 0.30, 0.50)),
            (Biome::Mountains,    (-0.2,  0.0,  0.85)),
            (Biome::Beach,        ( 0.4,  0.0,  0.22)),
            (Biome::Desert,       ( 0.6, -0.6,  0.38)),
            (Biome::Snowy,        (-0.9,  0.0,  0.94)),
            (Biome::Swamp,        ( 0.0,  0.8,  0.28)),
            (Biome::River,        ( 0.0,  0.6,  0.28)),
            (Biome::Glade,        ( 0.1,  0.4,  0.48)),
            (Biome::Taiga,        (-0.5,  0.3,  0.55)),
            (Biome::Savanna,      ( 0.5, -0.4,  0.46)),
            (Biome::Ocean,        (-0.1,  0.3,  0.14)),
            (Biome::Tundra,       (-0.7, -0.2,  0.45)),
        ]
    }

    /// Calculate base height using fractal Brownian motion (fbm)
    /// This creates more interesting terrain than simple noise
    fn base_height(&self, wx: i32, wz: i32) -> f64 {
        let nx = wx as f64 * 0.008;
        let nz = wz as f64 * 0.008;

        // Fractal Brownian Motion: combine multiple octaves of noise
        let h1 = self.height_noise.get([nx, nz]) * 1.0;
        let h2 = self.height_noise.get([nx * 2.0, nz * 2.0]) * 0.5;
        let h3 = self.height_noise.get([nx * 4.0, nz * 4.0]) * 0.25;
        let h4 = self.height_noise.get([nx * 8.0, nz * 8.0]) * 0.125;

        h1 + h2 + h3 + h4
    }

    /// Get surface height with biome-specific terrain variations
    pub fn surface_height(&self, wx: i32, wz: i32) -> u32 {
        let base = self.base_height(wx, wz);
        let detail = self.detail_noise.get([wx as f64 * 0.02, wz as f64 * 0.02]);
        let n = (base / 1.875 + 1.0) * 0.5; // normalised 0..1

        let height = match self.get_biome(wx, wz) {
            Biome::Plains       => 47.0 + n * 24.0,  // 47-71
            Biome::ForestDense  => 50.0 + n * 28.0,  // 50-78
            Biome::ForestSparse => 48.0 + n * 26.0,  // 48-74
            Biome::Mountains    => 60.0 + n * 80.0,  // 60-140
            Biome::Beach        => 38.0 + n * 7.0,   // 38-45  (straddles SEA_LEVEL=44)
            Biome::Desert       => 48.0 + n * 22.0,  // 48-70
            Biome::Snowy        => 55.0 + n * 35.0,  // 55-90
            Biome::Swamp        => 40.0 + n * 12.0,  // 40-52  (partially below SEA)
            Biome::River        => 37.0 + n * 7.0,   // 37-44  (below to at SEA)
            Biome::Glade        => 48.0 + n * 25.0,  // 48-73
            Biome::Taiga        => 52.0 + n * 30.0,  // 52-82
            Biome::Savanna      => 46.0 + n * 22.0,  // 46-68
            Biome::Ocean        => 20.0 + n * 16.0,  // 20-36  (all below SEA)
            Biome::Tundra       => 44.0 + n * 8.0,   // 44-52  (flat, near SEA)
        };

        let variation = 0.92 + (detail + 1.0) * 0.04;
        ((height * variation) as u32).max(2).min(254)
    }

    /// Check if there's a cave at this position using 3D Perlin noise
    fn is_cave(&self, wx: i32, wy: i32, wz: i32, surface: u32) -> bool {
        // Only caves below surface
        if wy as u32 >= surface {
            return false;
        }
        
        // Don't create caves too close to bedrock
        if wy < 5 {
            return false;
        }

        let nx = wx as f64 * 0.05;
        let ny = wy as f64 * 0.05;
        let nz = wz as f64 * 0.05;

        let cave1 = (self.cave_noise.get([nx, ny, nz]) + 1.0) * 0.5;
        let cave2 = (self.cave_noise.get([nx + 100.0, ny, nz + 100.0]) + 1.0) * 0.5;

        // Create cave network - threshold adjusted for desired cave size
        cave1 > 0.45 && cave2 > 0.45
    }

    /// Determine ore type based on depth and noise
    fn get_ore_at(&self, wy: i32, wx: i32, wz: i32, surface: u32) -> Option<BlockType> {
        if wy as u32 >= surface || wy < 5 {
            return None;
        }

        let depth_pct = 1.0 - (wy as f64 / surface as f64);
        let ore_val = (self.ore_noise.get([wx as f64 * 0.04, wy as f64 * 0.04, wz as f64 * 0.04]) + 1.0) * 0.5;

        // Ore distribution by depth
        match () {
            _ if ore_val > 0.92 && depth_pct < 0.1 => Some(BlockType::CoalOre),
            _ if ore_val > 0.94 && depth_pct < 0.3 => Some(BlockType::IronOre),
            _ if ore_val > 0.96 && depth_pct < 0.5 => Some(BlockType::GoldOre),
            _ if ore_val > 0.985 && depth_pct > 0.6 => Some(BlockType::DiamondOre),
            _ if ore_val > 0.97 && depth_pct > 0.4 => Some(BlockType::RedstoneOre),
            _ => None,
        }
    }

    /// Check if there should be a tree at this location
    fn should_place_tree(&self, wx: i32, wz: i32, biome: Biome) -> bool {
        let v = self.tree_noise.get([wx as f64 * 0.03, wz as f64 * 0.03]);
        match biome {
            Biome::ForestDense  => v > 0.20,
            Biome::ForestSparse => v > 0.50,
            Biome::Taiga        => v > 0.45,
            Biome::Glade        => v > 0.72,
            _ => false,
        }
    }

    /// Place a tree at the given surface location
    fn place_tree(&self, wx: i32, surface: i32, wz: i32, column: &mut [BlockType; 256]) {
        if (surface as u32 + 10) >= 256 {
            return; // Not enough space
        }

        // Vary height with noise (4-9)
        let raw = (self.tree_noise.get([wx as f64 * 0.08, wz as f64 * 0.08]) + 1.0) * 0.5;
        let height = 4 + (raw * 5.0) as usize; // 4..9

        // Trunk
        for y in 1..=height {
            if (surface + (y as i32)) < 256 {
                column[(surface + (y as i32)) as usize] = BlockType::OakLog;
            }
        }

        // Simple canopy
        let leaf_base = surface + height as i32 - 1;
        for dy in -2i32..=2i32 {
            for dx in -2i32..=2i32 {
                let dist = dx.abs() + dy.abs();
                if dist > 3 { continue; }
                let y = leaf_base + dy as i32 + 1;
                if y >= 0 && y < 256 {
                    column[y as usize] = BlockType::OakLeaves;
                }
            }
        }
    }

    /// Fill a vertical column with blocks based on biome and features.
    /// Columns below SEA_LEVEL are flooded with water.
    pub fn fill_column(&self, wx: i32, wz: i32, column: &mut [BlockType; 256]) {
        let base_surface = self.surface_height(wx, wz) as usize;
        let base_biome   = self.get_biome(wx, wz);

        // River carving: lower the bed near river centrelines
        let rnoise = self.river_noise.get([wx as f64 * 0.0015, wz as f64 * 0.0015]);
        let is_river = rnoise.abs() < 0.018;
        let (surface, biome) = if is_river {
            let carve = 3 + ((1.0 - rnoise.abs() / 0.018) * 4.0) as usize;
            (base_surface.saturating_sub(carve), Biome::River)
        } else {
            (base_surface, base_biome)
        };

        // ── Bedrock ──────────────────────────────────────────────────────────
        column[0] = BlockType::Bedrock;

        // ── Underground fill ─────────────────────────────────────────────────
        for y in 1..surface {
            if self.is_cave(wx, y as i32, wz, surface as u32) {
                // leave Air in caves
                continue;
            }
            if let Some(ore) = self.get_ore_at(y as i32, wx, wz, surface as u32) {
                column[y] = ore;
                continue;
            }
            let depth = surface - y;
            column[y] = match depth {
                1..=3 => match biome {
                    Biome::Desert | Biome::Beach | Biome::Ocean => BlockType::Sand,
                    _ => BlockType::Dirt,
                },
                4..=19 => match biome {
                    Biome::Desert | Biome::Beach | Biome::Ocean => BlockType::Sand,
                    _ => BlockType::Dirt,
                },
                _ => BlockType::Stone,
            };
        }

        // Deepslate + Tuff in the lowest 30 blocks
        for y in 1..surface.min(30) {
            if column[y] == BlockType::Stone {
                let v = (self.ore_noise.get([wx as f64 * 0.01, y as f64 * 0.01, wz as f64 * 0.01]) + 1.0) * 0.5;
                column[y] = if y < 10 { BlockType::Deepslate }
                            else if v > 0.5 { BlockType::Tuff }
                            else { BlockType::Deepslate };
            }
        }

        // ── Surface block ─────────────────────────────────────────────────────
        if surface < 256 {
            column[surface] = match biome {
                Biome::Plains | Biome::Glade | Biome::Savanna         => BlockType::Grass,
                Biome::ForestDense | Biome::ForestSparse              => BlockType::Grass,
                Biome::Taiga   => if surface > 90 { BlockType::Snow } else { BlockType::Grass },
                Biome::Mountains => if surface > 100 { BlockType::Snow } else { BlockType::Stone },
                Biome::Beach | Biome::Ocean => BlockType::Sand,
                Biome::Desert  => BlockType::Sand,
                Biome::Snowy | Biome::Tundra => BlockType::Snow,
                Biome::Swamp   => BlockType::Dirt,
                Biome::River   => BlockType::Water,
            };
        }

        // ── Sea-level flooding ────────────────────────────────────────────────
        if surface < SEA_LEVEL {
            // Replace dry surface with appropriate underwater bed material
            if surface < 256 && column[surface] != BlockType::Water {
                column[surface] = match biome {
                    Biome::Desert | Biome::Beach | Biome::Ocean => BlockType::Sand,
                    _ => BlockType::Gravel,
                };
            }
            // Water column from surface+1 up to SEA_LEVEL
            for y in (surface + 1)..=SEA_LEVEL {
                if y < 256 { column[y] = BlockType::Water; }
            }
        }

        // ── Trees (above-sea-level columns only) ──────────────────────────────
        if surface >= SEA_LEVEL && surface < 250 && self.should_place_tree(wx, wz, biome) {
            self.place_tree(wx, surface as i32, wz, column);
        }
    }

    /// Returns true if (wx, wz) is solid, walkable land above sea level.
    pub fn is_land_surface(&self, wx: i32, wz: i32) -> bool {
        let surf  = self.surface_height(wx, wz) as usize;
        let biome = self.get_biome(wx, wz);
        surf > SEA_LEVEL && !matches!(biome, Biome::River | Biome::Ocean)
    }

    /// Ambient colour and multiplier for the sky-light at world position.
    pub fn ambient_at(&self, wx: i32, wz: i32) -> [f32; 4] {
        let nx = wx as f64 * 0.0025;
        let nz = wz as f64 * 0.0025;
        let temp     = self.biome_noise.get([nx, nz]);
        let humidity = self.biome_noise.get([nx + 100.0, nz + 100.0]);
        let elev     = (self.base_height(wx, wz) / 1.875 + 1.0) * 0.5;

        let mut accum  = [0.0f64; 4];
        let sigma = 0.25;
        let mut total_w = 0.0;
        for (b, cent) in Self::biome_feature_centers().iter() {
            let d2 = (temp - cent.0).powi(2) + (humidity - cent.1).powi(2) + (elev - cent.2).powi(2);
            let w = (-d2 / (2.0 * sigma * sigma)).exp();
            total_w += w;
            let amb: [f64; 4] = match b {
                Biome::Plains       => [0.95, 1.00, 0.90, 0.95],
                Biome::ForestDense  => [0.80, 0.95, 0.75, 0.85],
                Biome::ForestSparse => [0.90, 1.00, 0.88, 0.95],
                Biome::Mountains    => [0.92, 0.98, 1.00, 1.00],
                Biome::Snowy        => [1.05, 1.05, 1.02, 1.05],
                Biome::Beach        => [1.02, 0.98, 0.90, 0.98],
                Biome::Desert       => [1.05, 0.95, 0.82, 1.00],
                Biome::Swamp        => [0.60, 0.75, 0.55, 0.75],
                Biome::River        => [0.78, 0.90, 1.05, 0.85],
                Biome::Glade        => [0.98, 1.05, 0.95, 1.05],
                Biome::Taiga        => [0.78, 0.90, 0.98, 0.88],
                Biome::Savanna      => [1.00, 0.90, 0.75, 1.00],
                Biome::Ocean        => [0.65, 0.80, 1.10, 0.80],
                Biome::Tundra       => [0.95, 0.98, 1.05, 0.98],
            };
            for i in 0..4 { accum[i] += amb[i] * w; }
        }
        if total_w <= 0.0 { return [1.0, 1.0, 1.0, 1.0]; }
        [
            (accum[0] / total_w) as f32,
            (accum[1] / total_w) as f32,
            (accum[2] / total_w) as f32,
            (accum[3] / total_w) as f32,
        ]
    }
}