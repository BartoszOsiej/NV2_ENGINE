use opensimplex2::smooth as simplex;

use super::block::BlockType;
use super::chunk::{CHUNK_D, CHUNK_H, CHUNK_W};
use super::vegetation::VegetationGenerator;
use super::worldgen::{WorldBlockWrite, WorldGenWriter};
use crate::settings::SharedSettings;

pub const SEA_LEVEL: usize = 46;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BiomeId {
    Ocean,
    Coast,
    Plains,
    Forest,
    DarkForest,
    Swamp,
    Taiga,
    Desert,
    Mountains,
}

pub type Biome = BiomeId;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TreeKind {
    Oak,
    Birch,
    Pine,
    DarkOak,
    DeadTree,
}

#[derive(Clone, Copy)]
pub struct BiomeDefinition {
    pub name: &'static str,
    pub temperature: f64,
    pub humidity: f64,
    pub tree_density: f64,
    pub grass_density: f64,
    pub flower_density: f64,
    pub shrub_density: f64,
    pub tree_types: &'static [TreeKind],
    pub surface_block: BlockType,
    pub ground_block: BlockType,
    pub shoreline_block: BlockType,
    pub cliff_block: BlockType,
    pub base_height: f64,
    pub relief: f64,
    pub ambient: [f32; 4],
    pub fog_color: [f32; 3],
    pub fog_density: f32,
    pub grade: [f32; 3],
    pub vegetation_tint: [f32; 3],
}

#[derive(Clone, Copy)]
struct ClimateSample {
    sample_x: f64,
    sample_z: f64,
    temperature: f64,
    humidity: f64,
    erosion: f64,
    variation: f64,
    landness: f64,
    mountainness: f64,
    swampiness: f64,
}

#[derive(Clone, Copy)]
pub(crate) struct ColumnSample {
    pub(crate) biome: BiomeId,
    pub(crate) definition: BiomeDefinition,
    pub(crate) surface: usize,
    pub(crate) water_top: usize,
    pub(crate) surface_block: BlockType,
    pub(crate) temperature: f64,
    pub(crate) humidity: f64,
    pub(crate) landness: f64,
    pub(crate) mountainness: f64,
}

#[derive(Clone, Copy, Debug)]
pub struct SurfaceVisuals {
    pub ambient: [f32; 4],
    pub fog_color: [f32; 3],
    pub fog_density: f32,
    pub grade: [f32; 3],
    pub vegetation_tint: [f32; 3],
    pub warmth: f32,
    pub moisture: f32,
    pub lushness: f32,
}

impl SurfaceVisuals {
    #[inline]
    pub fn foliage_color(self) -> [f32; 3] {
        self.vegetation_tint
    }
}

const NO_TREES: &[TreeKind] = &[];
const PLAINS_TREES: &[TreeKind] = &[TreeKind::Oak];
const FOREST_TREES: &[TreeKind] = &[TreeKind::Oak];
const DARK_FOREST_TREES: &[TreeKind] = &[TreeKind::DarkOak];
const SWAMP_TREES: &[TreeKind] = &[TreeKind::Oak, TreeKind::DeadTree];
const TAIGA_TREES: &[TreeKind] = &[TreeKind::Pine];
const MOUNTAIN_TREES: &[TreeKind] = &[TreeKind::Pine];

#[inline(always)]
fn n2(seed: i64, x: f64, z: f64) -> f64 {
    simplex::noise2(seed, x, z) as f64
}

#[inline(always)]
fn n3(seed: i64, x: f64, y: f64, z: f64) -> f64 {
    simplex::noise3_ImproveXZ(seed, x, y, z) as f64
}

#[inline(always)]
fn n3_01(seed: i64, x: f64, y: f64, z: f64) -> f64 {
    (n3(seed, x, y, z) + 1.0) * 0.5
}

fn fbm4(seed: i64, x: f64, z: f64) -> f64 {
    let value = n2(seed, x, z) * 1.000
        + n2(seed.wrapping_add(17), x * 2.03, z * 2.03) * 0.500
        + n2(seed.wrapping_add(31), x * 4.11, z * 4.11) * 0.250
        + n2(seed.wrapping_add(53), x * 8.23, z * 8.23) * 0.125;
    value / 1.875
}

#[inline(always)]
fn ridge(value: f64) -> f64 {
    1.0 - value.abs()
}

#[inline(always)]
fn smooth_step(value: f64) -> f64 {
    let clamped = value.clamp(0.0, 1.0);
    clamped * clamped * (3.0 - 2.0 * clamped)
}

#[inline(always)]
fn remap01(value: f64, min: f64, max: f64) -> f64 {
    if (max - min).abs() < f64::EPSILON {
        0.0
    } else {
        ((value - min) / (max - min)).clamp(0.0, 1.0)
    }
}

fn biome_definition(id: BiomeId) -> BiomeDefinition {
    match id {
        BiomeId::Ocean => BiomeDefinition {
            name: "ocean",
            temperature: 0.48,
            humidity: 0.88,
            tree_density: 0.0,
            grass_density: 0.0,
            flower_density: 0.0,
            shrub_density: 0.0,
            tree_types: NO_TREES,
            surface_block: BlockType::Sand,
            ground_block: BlockType::Clay,
            shoreline_block: BlockType::Sand,
            cliff_block: BlockType::Gravel,
            base_height: -18.0,
            relief: 1.8,
            ambient: [0.56, 0.72, 0.84, 0.88],
            fog_color: [0.54, 0.66, 0.82],
            fog_density: 0.96,
            grade: [0.97, 1.00, 1.05],
            vegetation_tint: [0.58, 0.82, 0.74],
        },
        BiomeId::Coast => BiomeDefinition {
            name: "coast",
            temperature: 0.62,
            humidity: 0.54,
            tree_density: 0.02,
            grass_density: 0.10,
            flower_density: 0.02,
            shrub_density: 0.04,
            tree_types: NO_TREES,
            surface_block: BlockType::Sand,
            ground_block: BlockType::Sand,
            shoreline_block: BlockType::Sand,
            cliff_block: BlockType::Gravel,
            base_height: -6.0,
            relief: 1.5,
            ambient: [0.84, 0.84, 0.68, 0.98],
            fog_color: [0.80, 0.77, 0.68],
            fog_density: 0.92,
            grade: [1.03, 1.00, 0.95],
            vegetation_tint: [0.78, 0.82, 0.48],
        },
        BiomeId::Plains => BiomeDefinition {
            name: "plains",
            temperature: 0.58,
            humidity: 0.46,
            tree_density: 0.05,
            grass_density: 0.72,
            flower_density: 0.16,
            shrub_density: 0.05,
            tree_types: PLAINS_TREES,
            surface_block: BlockType::Grass,
            ground_block: BlockType::Dirt,
            shoreline_block: BlockType::Gravel,
            cliff_block: BlockType::Stone,
            base_height: 4.0,
            relief: 2.8,
            ambient: [0.70, 0.88, 0.58, 0.96],
            fog_color: [0.68, 0.77, 0.79],
            fog_density: 0.90,
            grade: [1.02, 1.00, 0.97],
            vegetation_tint: [0.72, 0.92, 0.54],
        },
        BiomeId::Forest => BiomeDefinition {
            name: "forest",
            temperature: 0.54,
            humidity: 0.62,
            tree_density: 0.46,
            grass_density: 0.46,
            flower_density: 0.08,
            shrub_density: 0.12,
            tree_types: FOREST_TREES,
            surface_block: BlockType::Grass,
            ground_block: BlockType::Dirt,
            shoreline_block: BlockType::Gravel,
            cliff_block: BlockType::Stone,
            base_height: 6.5,
            relief: 3.8,
            ambient: [0.60, 0.80, 0.50, 0.90],
            fog_color: [0.58, 0.68, 0.66],
            fog_density: 1.00,
            grade: [0.98, 1.01, 0.97],
            vegetation_tint: [0.50, 0.86, 0.42],
        },
        BiomeId::DarkForest => BiomeDefinition {
            name: "dark_forest",
            temperature: 0.50,
            humidity: 0.74,
            tree_density: 0.74,
            grass_density: 0.18,
            flower_density: 0.02,
            shrub_density: 0.26,
            tree_types: DARK_FOREST_TREES,
            surface_block: BlockType::ForestFloor,
            ground_block: BlockType::Dirt,
            shoreline_block: BlockType::Clay,
            cliff_block: BlockType::Stone,
            base_height: 6.0,
            relief: 4.4,
            ambient: [0.48, 0.68, 0.42, 0.84],
            fog_color: [0.44, 0.56, 0.52],
            fog_density: 1.12,
            grade: [0.94, 0.99, 0.96],
            vegetation_tint: [0.38, 0.72, 0.34],
        },
        BiomeId::Swamp => BiomeDefinition {
            name: "swamp",
            temperature: 0.66,
            humidity: 0.90,
            tree_density: 0.28,
            grass_density: 0.26,
            flower_density: 0.04,
            shrub_density: 0.34,
            tree_types: SWAMP_TREES,
            surface_block: BlockType::Mud,
            ground_block: BlockType::PackedMud,
            shoreline_block: BlockType::Clay,
            cliff_block: BlockType::RootedSoil,
            base_height: -1.0,
            relief: 1.2,
            ambient: [0.52, 0.70, 0.60, 0.82],
            fog_color: [0.50, 0.60, 0.58],
            fog_density: 1.18,
            grade: [0.95, 0.99, 0.98],
            vegetation_tint: [0.42, 0.76, 0.52],
        },
        BiomeId::Taiga => BiomeDefinition {
            name: "taiga",
            temperature: 0.24,
            humidity: 0.52,
            tree_density: 0.58,
            grass_density: 0.18,
            flower_density: 0.03,
            shrub_density: 0.08,
            tree_types: TAIGA_TREES,
            surface_block: BlockType::Grass,
            ground_block: BlockType::Dirt,
            shoreline_block: BlockType::Gravel,
            cliff_block: BlockType::Andesite,
            base_height: 10.0,
            relief: 5.0,
            ambient: [0.68, 0.80, 0.86, 1.00],
            fog_color: [0.66, 0.76, 0.86],
            fog_density: 0.84,
            grade: [0.98, 1.00, 1.03],
            vegetation_tint: [0.58, 0.74, 0.60],
        },
        BiomeId::Desert => BiomeDefinition {
            name: "desert",
            temperature: 0.92,
            humidity: 0.10,
            tree_density: 0.0,
            grass_density: 0.0,
            flower_density: 0.0,
            shrub_density: 0.14,
            tree_types: NO_TREES,
            surface_block: BlockType::Sand,
            ground_block: BlockType::Sand,
            shoreline_block: BlockType::Sand,
            cliff_block: BlockType::Stone,
            base_height: 5.0,
            relief: 2.4,
            ambient: [0.90, 0.82, 0.58, 1.00],
            fog_color: [0.82, 0.72, 0.56],
            fog_density: 1.06,
            grade: [1.05, 0.98, 0.92],
            vegetation_tint: [0.82, 0.80, 0.36],
        },
        BiomeId::Mountains => BiomeDefinition {
            name: "mountains",
            temperature: 0.28,
            humidity: 0.34,
            tree_density: 0.12,
            grass_density: 0.10,
            flower_density: 0.01,
            shrub_density: 0.08,
            tree_types: MOUNTAIN_TREES,
            surface_block: BlockType::Grass,
            ground_block: BlockType::Gravel,
            shoreline_block: BlockType::Gravel,
            cliff_block: BlockType::Andesite,
            base_height: 18.0,
            relief: 9.5,
            ambient: [0.76, 0.84, 0.90, 1.02],
            fog_color: [0.70, 0.78, 0.90],
            fog_density: 0.80,
            grade: [0.98, 1.00, 1.03],
            vegetation_tint: [0.62, 0.78, 0.62],
        },
    }
}

pub struct BiomeGenerator {
    seed: u32,
    continent_seed: i64,
    temperature_seed: i64,
    humidity_seed: i64,
    erosion_seed: i64,
    peak_seed: i64,
    height_seed: i64,
    detail_seed: i64,
    warp_seed: i64,
    surface_seed: i64,
    cave_seed: i64,
    ore_seed: i64,
    water_seed: i64,
    settings: SharedSettings,
    vegetation: VegetationGenerator,
}

impl BiomeGenerator {
    pub fn new(seed: u32) -> Self {
        Self::new_with_settings(seed, SharedSettings::default())
    }

    pub fn new_with_settings(seed: u32, settings: SharedSettings) -> Self {
        let base = seed as i64;
        Self {
            seed,
            continent_seed: base.wrapping_add(1_111),
            temperature_seed: base.wrapping_add(2_222),
            humidity_seed: base.wrapping_add(3_333),
            erosion_seed: base.wrapping_add(4_444),
            peak_seed: base.wrapping_add(5_555),
            height_seed: base.wrapping_add(6_666),
            detail_seed: base.wrapping_add(7_777),
            warp_seed: base.wrapping_add(8_888),
            surface_seed: base.wrapping_add(9_999),
            cave_seed: base.wrapping_add(10_101),
            ore_seed: base.wrapping_add(11_111),
            water_seed: base.wrapping_add(12_121),
            settings,
            vegetation: VegetationGenerator::new(),
        }
    }

    pub fn seed(&self) -> u32 {
        self.seed
    }

    pub fn populate_chunk(
        &self,
        cx: i32,
        cz: i32,
        blocks: &mut Box<[[[BlockType; CHUNK_D]; CHUNK_H]; CHUNK_W]>,
        writes: &mut Vec<WorldBlockWrite>,
    ) {
        let mut column = [BlockType::Air; CHUNK_H];

        for x in 0..CHUNK_W {
            for z in 0..CHUNK_D {
                column.fill(BlockType::Air);
                let wx = cx * CHUNK_W as i32 + x as i32;
                let wz = cz * CHUNK_D as i32 + z as i32;
                self.fill_terrain_column(wx, wz, &mut column);

                for y in 0..CHUNK_H {
                    blocks[x][y][z] = column[y];
                }
            }
        }

        if !self.settings.low_end_pc() {
            let mut writer = WorldGenWriter::new(cx, cz, blocks);
            self.vegetation.populate_chunk(self, cx, cz, &mut writer);
            writes.extend(writer.finish());
            
            // ✅ AI VEGETATION (HEURISTIC) ✅
            self.populate_ai_vegetation(cx, cz, blocks);
        }
    }

    pub fn populate_world_trees_for_chunk(&self, world: &mut crate::world::World, cx: i32, cz: i32) {
        println!("[BIO-TREES-CALL] Chunk ({}, {}), low_end={}", cx, cz, self.settings.low_end_pc());
        if self.settings.low_end_pc() {
            println!("[BIO-TREES-SKIP] Low end mode - skipping");
            return;
        }
        self.vegetation.populate_world_trees_for_chunk(world, self, cx, cz);
    }
    
    /// AI vegetation using heuristics (no neural network, thread-safe)
    fn populate_ai_vegetation(
        &self,
        cx: i32,
        cz: i32,
        blocks: &mut Box<[[[BlockType; CHUNK_D]; CHUNK_H]; CHUNK_W]>,
    ) {
        let world_seed = self.seed() as i64;
        let mut placed = 0;
        
        // 4x4 grid per chunk
        for gy in 0..4 {
            for gx in 0..4 {
                let wx = cx * CHUNK_W as i32 + gx * 4;
                let wz = cz * CHUNK_D as i32 + gy * 4;
                
                let sample = self.sample_column(wx, wz);
                if sample.water_top > sample.surface { continue; }
                
                // Find surface Y
                let surface_y = sample.surface;
                let wy = surface_y + 1;
                
                if wy >= CHUNK_H { continue; }
                
                // Heuristic: High humidity -> Fern, otherwise -> Stick
                let block_type = if sample.humidity > 0.65 {
                    BlockType::Fern
                } else {
                    BlockType::Stick
                };
                
                // Place in chunk
                let lx = (wx & 15) as usize;
                let lz = (wz & 15) as usize;
                let ly = wy as usize;
                
                if ly < CHUNK_H && (cx * CHUNK_W as i32 + lx as i32) == wx && (cz * CHUNK_D as i32 + lz as i32) == wz {
                    blocks[lx][ly][lz] = block_type;
                    placed += 1;
                }
            }
        }
        
        if placed > 0 {
            println!("[AI-HEUR] Chunk ({}, {}): {} vegetation", cx, cz, placed);
        }
    }

    fn sample_climate(&self, wx: i32, wz: i32) -> ClimateSample {
        let x = wx as f64;
        let z = wz as f64;

        let warp_scale = 0.0012;
        let warp_x = fbm4(self.warp_seed, x * warp_scale, z * warp_scale) * 18.0;
        let warp_z = fbm4(
            self.warp_seed.wrapping_add(97),
            x * warp_scale + 37.0,
            z * warp_scale - 19.0,
        ) * 18.0;
        let sample_x = x + warp_x;
        let sample_z = z + warp_z;

        let continent = fbm4(self.continent_seed, sample_x * 0.00065, sample_z * 0.00065);
        let temperature = ((fbm4(
            self.temperature_seed,
            sample_x * 0.00185 + 48.0,
            sample_z * 0.00185 - 31.0,
        ) + 1.0) * 0.5)
            .clamp(0.0, 1.0);
        let humidity = ((fbm4(
            self.humidity_seed,
            sample_x * 0.00195 - 64.0,
            sample_z * 0.00195 + 22.0,
        ) + 1.0) * 0.5)
            .clamp(0.0, 1.0);
        let erosion = ((fbm4(self.erosion_seed, sample_x * 0.0028, sample_z * 0.0028) + 1.0) * 0.5)
            .clamp(0.0, 1.0);
        let ridges = ridge(fbm4(self.peak_seed, sample_x * 0.0048, sample_z * 0.0048)).clamp(0.0, 1.0);
        let variation = fbm4(self.detail_seed, sample_x * 0.0062, sample_z * 0.0062);
        let landness = smooth_step(remap01(continent, -0.24, 0.12));
        let mountainness = smooth_step(remap01(
            ridges * (1.0 - erosion * 0.55) + landness * 0.20,
            0.58,
            0.86,
        )) * landness;
        let lowlandness = (1.0 - mountainness) * (0.45 + erosion * 0.55);
        let swampiness = humidity * lowlandness * landness;

        ClimateSample {
            sample_x,
            sample_z,
            temperature,
            humidity,
            erosion,
            variation,
            landness,
            mountainness,
            swampiness,
        }
    }

    fn select_biome(&self, climate: ClimateSample) -> BiomeId {
        if climate.landness < 0.18 {
            return BiomeId::Ocean;
        }
        if climate.landness < 0.30 {
            return BiomeId::Coast;
        }
        if climate.mountainness > 0.68 {
            return BiomeId::Mountains;
        }
        if climate.temperature > 0.72 && climate.humidity < 0.28 {
            return BiomeId::Desert;
        }
        if climate.swampiness > 0.58 && climate.temperature > 0.46 {
            return BiomeId::Swamp;
        }
        if climate.temperature < 0.34 {
            return BiomeId::Taiga;
        }
        if climate.humidity > 0.68 && climate.variation > 0.04 {
            return BiomeId::DarkForest;
        }
        if climate.humidity > 0.48 {
            return BiomeId::Forest;
        }
        BiomeId::Plains
    }

    fn sample_surface_height(&self, climate: ClimateSample, definition: BiomeDefinition, biome: BiomeId) -> usize {
        let macro_noise = fbm4(self.height_seed, climate.sample_x * 0.0024, climate.sample_z * 0.0024);
        let local_noise = fbm4(
            self.detail_seed.wrapping_add(29),
            climate.sample_x * 0.0095,
            climate.sample_z * 0.0095,
        );
        let mountain_ridge = ridge(n2(
            self.peak_seed.wrapping_add(137),
            climate.sample_x * 0.0071,
            climate.sample_z * 0.0071,
        ))
        .clamp(0.0, 1.0);
        let dunes = if biome == BiomeId::Desert {
            ridge(n2(
                self.surface_seed,
                climate.sample_x * 0.015,
                climate.sample_z * 0.015,
            )) * 3.4 - 1.4
        } else {
            0.0
        };
        let coast_flatten = if biome == BiomeId::Coast {
            -4.0 + local_noise * 1.2
        } else {
            0.0
        };
        let swamp_flatten = if biome == BiomeId::Swamp {
            -2.2 + local_noise * 0.8
        } else {
            0.0
        };

        let base = SEA_LEVEL as f64 - 20.0 + climate.landness * 26.0 + definition.base_height;
        let rolling = macro_noise * (definition.relief * 1.6);
        let local = local_noise * definition.relief;
        let mountain = climate.mountainness
            * (18.0 + mountain_ridge * 22.0 + (1.0 - climate.erosion) * 10.0);

        (base + rolling + local + mountain + dunes + coast_flatten + swamp_flatten)
            .round()
            .clamp(3.0, (CHUNK_H - 3) as f64) as usize
    }

    fn sample_water_top(&self, climate: ClimateSample, biome: BiomeId, surface: usize) -> usize {
        if surface < SEA_LEVEL {
            return SEA_LEVEL;
        }

        if biome == BiomeId::Swamp && surface <= SEA_LEVEL + 2 {
            let pool = ((n2(
                self.water_seed,
                climate.sample_x * 0.045,
                climate.sample_z * 0.045,
            ) + 1.0) * 0.5)
                .clamp(0.0, 1.0);
            if climate.humidity > 0.66 && pool > 0.62 {
                return (surface + 1).min(SEA_LEVEL + 2);
            }
        }

        surface
    }

    fn snowline(&self, sample: &ColumnSample) -> usize {
        let cold = (1.0 - sample.temperature).clamp(0.0, 1.0);
        (94.0 - cold * 18.0 - sample.mountainness * 10.0)
            .round()
            .clamp(70.0, 110.0) as usize
    }

    fn choose_surface_block(&self, sample: &ColumnSample, wx: i32, wz: i32) -> BlockType {
        if sample.water_top > sample.surface {
            return sample.definition.shoreline_block;
        }

        let noise = fbm4(self.surface_seed, wx as f64 * 0.08, wz as f64 * 0.08);
        match sample.biome {
            BiomeId::Ocean | BiomeId::Coast => sample.definition.shoreline_block,
            BiomeId::Plains => {
                if noise > 0.52 {
                    BlockType::CoarseSoil
                } else if noise < -0.50 {
                    BlockType::BloomFloor
                } else {
                    sample.definition.surface_block
                }
            }
            BiomeId::Forest => {
                if noise > 0.42 {
                    BlockType::ForestFloor
                } else if noise < -0.38 {
                    BlockType::BloomFloor
                } else {
                    sample.definition.surface_block
                }
            }
            BiomeId::DarkForest => {
                if noise > -0.05 {
                    BlockType::ForestFloor
                } else {
                    BlockType::RootedSoil
                }
            }
            BiomeId::Swamp => {
                if noise > 0.18 {
                    BlockType::MossMat
                } else {
                    sample.definition.surface_block
                }
            }
            BiomeId::Taiga => {
                if sample.surface >= self.snowline(sample) {
                    BlockType::Snow
                } else if noise > 0.46 {
                    BlockType::CoarseSoil
                } else {
                    sample.definition.surface_block
                }
            }
            BiomeId::Desert => {
                if noise > 0.44 {
                    BlockType::Sand
                } else {
                    BlockType::CoarseSoil
                }
            }
            BiomeId::Mountains => {
                if sample.surface >= self.snowline(sample) {
                    BlockType::Snow
                } else if noise > 0.20 {
                    sample.definition.cliff_block
                } else {
                    sample.definition.surface_block
                }
            }
        }
    }

    fn filler_block(&self, sample: &ColumnSample, depth: usize) -> BlockType {
        match sample.biome {
            BiomeId::Ocean | BiomeId::Coast => {
                if depth <= 4 { BlockType::Sand } else { BlockType::Clay }
            }
            BiomeId::Swamp => {
                if depth <= 2 {
                    BlockType::Mud
                } else if depth <= 5 {
                    BlockType::PackedMud
                } else {
                    sample.definition.ground_block
                }
            }
            BiomeId::Desert => {
                if depth <= 6 { BlockType::Sand } else { BlockType::Clay }
            }
            BiomeId::Mountains => {
                if depth <= 2 {
                    BlockType::Gravel
                } else if depth <= 5 {
                    BlockType::Andesite
                } else {
                    sample.definition.cliff_block
                }
            }
            BiomeId::Taiga => {
                if depth <= 2 { BlockType::CoarseSoil } else { sample.definition.ground_block }
            }
            BiomeId::DarkForest => {
                if depth <= 2 { BlockType::RootedSoil } else { sample.definition.ground_block }
            }
            BiomeId::Plains | BiomeId::Forest => sample.definition.ground_block,
        }
    }

    fn deep_stone_block(&self, wx: i32, wy: i32, wz: i32) -> BlockType {
        if wy <= 18 {
            if n3_01(
                self.ore_seed.wrapping_add(61),
                wx as f64 * 0.09,
                wy as f64 * 0.11,
                wz as f64 * 0.09,
            ) > 0.84 {
                BlockType::Tuff
            } else {
                BlockType::SlateRock
            }
        } else if wy <= 40 {
            if n3_01(
                self.ore_seed.wrapping_add(131),
                wx as f64 * 0.05,
                wy as f64 * 0.05,
                wz as f64 * 0.05,
            ) > 0.70 {
                BlockType::Andesite
            } else {
                BlockType::Stone
            }
        } else {
            BlockType::Stone
        }
    }

    fn ore_block(&self, wx: i32, wy: i32, wz: i32, sample: &ColumnSample) -> Option<BlockType> {
        if wy < 2 || wy as usize >= sample.surface {
            return None;
        }

        let density = n3_01(
            self.ore_seed,
            wx as f64 * 0.045,
            wy as f64 * 0.045,
            wz as f64 * 0.045,
        );

        match () {
            _ if sample.biome == BiomeId::Mountains && (28..=80).contains(&wy) && density > 0.968 => {
                Some(BlockType::EmeraldOre)
            }
            _ if wy <= 16 && density > 0.970 => Some(BlockType::SlateDiamondOre),
            _ if wy <= 24 && density > 0.958 => Some(BlockType::RedstoneOre),
            _ if wy <= 32 && density > 0.952 => Some(BlockType::GoldOre),
            _ if wy <= 52 && density > 0.938 => Some(BlockType::IronOre),
            _ if wy <= 96 && density > 0.910 => Some(BlockType::CoalOre),
            _ => None,
        }
    }

    fn is_cave(&self, wx: i32, wy: i32, wz: i32, surface: usize) -> bool {
        if wy < 8 || wy >= surface as i32 - 6 {
            return false;
        }

        let depth = surface as i32 - wy;
        let depth_mask = smooth_step(remap01(depth as f64, 10.0, 52.0));
        if depth_mask <= 0.0 {
            return false;
        }

        let tunnel = ridge(n3(
            self.cave_seed,
            wx as f64 * 0.028,
            wy as f64 * 0.020,
            wz as f64 * 0.028,
        ));
        let chamber = ridge(n3(
            self.cave_seed.wrapping_add(311),
            wx as f64 * 0.018 + 41.0,
            wy as f64 * 0.016,
            wz as f64 * 0.018 - 17.0,
        ));

        (tunnel > 0.940 && chamber > 0.860 && depth_mask > 0.15)
            || (depth > 28 && chamber > 0.972)
    }

    fn fill_terrain_column(&self, wx: i32, wz: i32, column: &mut [BlockType; CHUNK_H]) {
        let sample = self.sample_column(wx, wz);
        column[0] = BlockType::Bedrock;

        for y in 1..sample.surface {
            let yi = y as i32;
            if self.is_cave(wx, yi, wz, sample.surface) {
                if sample.water_top > sample.surface && yi <= SEA_LEVEL as i32 - 3 {
                    column[y] = BlockType::Water;
                }
                continue;
            }

            if let Some(ore) = self.ore_block(wx, yi, wz, &sample) {
                column[y] = ore;
                continue;
            }

            let depth = sample.surface - y;
            column[y] = if depth <= 5 {
                self.filler_block(&sample, depth)
            } else {
                self.deep_stone_block(wx, yi, wz)
            };
        }

        column[sample.surface] = sample.surface_block;

        if sample.water_top > sample.surface {
            column[sample.surface] = sample.definition.shoreline_block;
            let top = sample.water_top.min(CHUNK_H - 1);
            for y in (sample.surface + 1)..=top {
                column[y] = BlockType::Water;
            }
        }
    }

    pub(crate) fn sample_column(&self, wx: i32, wz: i32) -> ColumnSample {
        let climate = self.sample_climate(wx, wz);
        let biome = self.select_biome(climate);
        let definition = biome_definition(biome);
        let surface = self.sample_surface_height(climate, definition, biome);
        let mut sample = ColumnSample {
            biome,
            definition,
            surface,
            water_top: surface,
            surface_block: definition.surface_block,
            temperature: climate.temperature,
            humidity: climate.humidity,
            landness: climate.landness,
            mountainness: climate.mountainness,
        };
        sample.water_top = self.sample_water_top(climate, biome, surface);
        sample.surface_block = self.choose_surface_block(&sample, wx, wz);
        sample
    }

    pub fn get_biome(&self, wx: i32, wz: i32) -> BiomeId {
        self.sample_column(wx, wz).biome
    }

    pub fn surface_height(&self, wx: i32, wz: i32) -> u32 {
        self.sample_column(wx, wz).surface as u32
    }

    pub fn visuals_at(&self, wx: i32, wz: i32) -> SurfaceVisuals {
        let sample = self.sample_column(wx, wz);
        let lushness = (sample.definition.grass_density * 0.45
            + sample.definition.tree_density * 0.55
            + sample.humidity * 0.25)
            .clamp(0.0, 1.2) as f32;

        SurfaceVisuals {
            ambient: sample.definition.ambient,
            fog_color: sample.definition.fog_color,
            fog_density: (sample.definition.fog_density * (0.92 + sample.humidity as f32 * 0.18))
                .clamp(0.78, 1.25),
            grade: sample.definition.grade,
            vegetation_tint: [
                (sample.definition.vegetation_tint[0] * (0.95 + sample.temperature as f32 * 0.08)).clamp(0.0, 1.25),
                (sample.definition.vegetation_tint[1] * (0.94 + lushness * 0.10)).clamp(0.0, 1.25),
                (sample.definition.vegetation_tint[2] * (0.92 + sample.humidity as f32 * 0.12)).clamp(0.0, 1.25),
            ],
            warmth: sample.temperature as f32,
            moisture: sample.humidity as f32,
            lushness,
        }
    }

    pub fn is_land_surface(&self, wx: i32, wz: i32) -> bool {
        let sample = self.sample_column(wx, wz);
        sample.water_top == sample.surface
            && sample.surface >= SEA_LEVEL + 3
            && !matches!(sample.biome, BiomeId::Ocean | BiomeId::Coast | BiomeId::Swamp)
    }

    pub fn is_spawn_candidate(&self, wx: i32, wz: i32) -> bool {
        let sample = self.sample_column(wx, wz);
        if sample.water_top != sample.surface {
            return false;
        }
        if !matches!(sample.biome, BiomeId::Plains | BiomeId::Forest | BiomeId::Taiga) {
            return false;
        }
        if sample.surface < SEA_LEVEL + 6 || sample.surface > 96 {
            return false;
        }
        if !matches!(
            sample.surface_block,
            BlockType::Grass
                | BlockType::ForestFloor
                | BlockType::BloomFloor
                | BlockType::RootedSoil
                | BlockType::CoarseSoil
                | BlockType::Snow
        ) {
            return false;
        }

        for (dx, dz) in [(8, 0), (-8, 0), (0, 8), (0, -8)] {
            let neighbour = self.sample_column(wx + dx, wz + dz);
            if neighbour.water_top != neighbour.surface
                || neighbour.surface < SEA_LEVEL + 3
                || matches!(neighbour.biome, BiomeId::Ocean | BiomeId::Coast | BiomeId::Swamp)
            {
                return false;
            }
        }

        true
    }

    pub fn smooth_surface_height(&self, wx: i32, wz: i32) -> usize {
        self.sample_column(wx, wz).surface
    }

    pub fn ambient_at(&self, wx: i32, wz: i32) -> [f32; 4] {
        self.visuals_at(wx, wz).ambient
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;

    #[test]
    fn samples_expected_biome_variety_across_known_seeds() {
        let seeds = [42_u32, 1_337_u32, 20_260_405_u32, 0x5eed_baad_u32];
        let mut seen = HashSet::new();

        for seed in seeds {
            let generator = BiomeGenerator::new(seed);
            for wz in (-768..=768).step_by(48) {
                for wx in (-768..=768).step_by(48) {
                    seen.insert(generator.get_biome(wx, wz));
                }
            }
        }

        for biome in [
            BiomeId::Ocean,
            BiomeId::Coast,
            BiomeId::Plains,
            BiomeId::Forest,
            BiomeId::DarkForest,
            BiomeId::Swamp,
            BiomeId::Taiga,
            BiomeId::Desert,
            BiomeId::Mountains,
        ] {
            assert!(seen.contains(&biome), "missing biome {:?}", biome);
        }
    }
}
