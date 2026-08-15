use opensimplex2::smooth as simplex;

use super::block::BlockType;
use super::chunk::{CHUNK_D, CHUNK_H, CHUNK_W};
use super::vegetation::VegetationGenerator;
use super::worldgen::{WorldBlockWrite, WorldGenWriter};
use crate::settings::SharedSettings;

pub const SEA_LEVEL: usize = 46;

/// Degrees of real latitude per world block — the world maps onto a real
/// patch of Earth, so walking north genuinely gets colder. 0.0006°/block
/// means ±8192 blocks ≈ ±4.9° ≈ 540 km of real climate gradient.
const LAT_PER_BLOCK: f64 = 0.0006;

/// Real-world climate zone (Whittaker-style classification from measured
/// annual temperature / precipitation). The zone decides which biome
/// *dominates* a region; noise only creates local pockets of variety.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClimateZone {
    /// Warmest month < 10 °C — polar: snowfields, dwarf scrub.
    Tundra,
    /// Cold (< 6 °C mean) — boreal conifer forest (taiga).
    Boreal,
    /// Mild + moderate rain — temperate forest / grassland mix.
    Temperate,
    /// Warm + low rain — dry grassland / steppe.
    Dry,
    /// Arid (< 1.2 mm/day) — desert.
    Desert,
    /// Hot + wet — rainforest / jungle.
    TropicalWet,
}

/// Regional sky/fog tint for a real climate zone. This is what makes a
/// desert world look sun-baked and a rainforest look humid — the sky and
/// haze follow the measured climate, not a generic blue.
fn climate_atmosphere(zone: ClimateZone, t_c: f64, p_mm: f64) -> [f32; 3] {
    // Temperature shifts the tint warm→cool; precipitation adds haze.
    let warmth = ((t_c + 12.0) / 42.0).clamp(0.0, 1.0) as f32;
    let wet = (p_mm / 12.0).clamp(0.0, 1.0) as f32;
    let base: [f32; 3] = match zone {
        ClimateZone::Desert => [0.86, 0.76, 0.58], // dusty warm sky
        ClimateZone::TropicalWet => [0.58, 0.74, 0.80], // humid teal
        ClimateZone::Tundra => [0.62, 0.70, 0.84], // pale polar steel
        ClimateZone::Boreal => [0.64, 0.74, 0.90], // cool northern blue
        ClimateZone::Dry => [0.80, 0.78, 0.64], // warm steppe haze
        ClimateZone::Temperate => [0.56, 0.72, 0.92], // classic blue
    };
    // blend toward warm on hot days, toward grey-blue when wet
    let warm_tint = [0.86, 0.78, 0.60];
    let wet_tint = [0.62, 0.70, 0.80];
    let mut out = [0.0; 3];
    for i in 0..3 {
        let t = base[i] + (warm_tint[i] - base[i]) * warmth * 0.30;
        out[i] = (t + (wet_tint[i] - t) * wet * 0.18).clamp(0.0, 1.0);
    }
    out
}

/// Whittaker-style zone from real annual temperature, warmest-month
/// temperature and annual precipitation.
fn whittaker_zone(t_c: f64, warmest: f64, p_mm: f64) -> ClimateZone {
    if warmest < 10.0 {
        ClimateZone::Tundra
    } else if t_c < 6.0 {
        ClimateZone::Boreal
    } else if p_mm < 1.2 {
        ClimateZone::Desert
    } else if t_c >= 18.0 && p_mm > 6.0 {
        ClimateZone::TropicalWet
    } else if p_mm < 3.5 && t_c > 12.0 {
        ClimateZone::Dry
    } else {
        ClimateZone::Temperate
    }
}

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
    zone: ClimateZone,
    /// Real annual temperature (°C) at this column's Earth location.
    real_temp_c: f64,
    /// Real annual precipitation (mm/day) at this column's Earth location.
    real_precip_mm: f64,
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
    /// Regional sky/fog tint from the real climate at this location — a
    /// desert haze, humid rainforest teal or cold boreal steel.
    pub atmosphere: [f32; 3],
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
    /// Real (NASA POWER) or synthetic climate baseline for this world.
    meteo: super::meteo::MeteoData,
}

impl BiomeGenerator {
    pub fn new(seed: u32) -> Self {
        Self::new_with_settings(seed, SharedSettings::default())
    }

    pub fn new_with_settings(seed: u32, settings: SharedSettings) -> Self {
        let base = seed as i64;
        let meteo = super::meteo::meteo_for_seed(seed);
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
            meteo,
        }
    }

    /// The real-world climate baseline (NASA POWER or synthetic fallback).
    pub fn meteo(&self) -> super::meteo::MeteoData {
        self.meteo
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

            // Biome-aware ground cover (grass tufts, ferns, dead bushes,
            // cactus) — noise-jittered so it never forms a visible grid.
            self.place_ground_cover(cx, cz, blocks);
        }
    }

    pub fn populate_world_trees_for_chunk(&self, world: &mut crate::world::World, cx: i32, cz: i32) {
        if self.settings.low_end_pc() {
            return;
        }
        self.vegetation.populate_world_trees_for_chunk(world, self, cx, cz);
    }

    /// Per-block ground cover placed during chunk generation. Density and
    /// block type follow the biome; placement is jittered by noise so the
    /// terrain reads as natural scatter instead of a stamped grid.
    fn place_ground_cover(
        &self,
        cx: i32,
        cz: i32,
        blocks: &mut Box<[[[BlockType; CHUNK_D]; CHUNK_H]; CHUNK_W]>,
    ) {
        let world_seed = self.seed() as i64;

        for z in 0..CHUNK_D {
            for x in 0..CHUNK_W {
                let wx = cx * CHUNK_W as i32 + x as i32;
                let wz = cz * CHUNK_D as i32 + z as i32;
                let sample = self.sample_column(wx, wz);
                if sample.water_top != sample.surface {
                    continue;
                }
                if !self.supports_cover_surface(sample.surface_block) {
                    continue;
                }

                // density by biome (fraction of surface blocks that get cover)
                let density = match sample.biome {
                    BiomeId::Plains => 0.30,
                    BiomeId::Forest => 0.26,
                    BiomeId::DarkForest => 0.10,
                    BiomeId::Swamp => 0.14,
                    BiomeId::Taiga => 0.16,
                    BiomeId::Mountains => 0.08,
                    BiomeId::Desert => 0.04,
                    _ => 0.0,
                };
                if density <= 0.0 {
                    continue;
                }

                let noise = n2(
                    world_seed.wrapping_add(7_777),
                    wx as f64 * 0.11,
                    wz as f64 * 0.11,
                ) as f64;
                // [0,1] noise, higher values place cover → density fraction
                if (noise + 1.0) * 0.5 > density {
                    continue;
                }

                let block = match sample.biome {
                    BiomeId::Desert => {
                        if sample.surface_block == BlockType::Sand {
                            BlockType::DeadBush
                        } else {
                            BlockType::Cactus
                        }
                    }
                    BiomeId::Swamp => {
                        if sample.humidity > 0.75 {
                            BlockType::Fern
                        } else {
                            BlockType::TallGrass
                        }
                    }
                    BiomeId::Taiga | BiomeId::Mountains => BlockType::TallGrass,
                    _ => {
                        if sample.humidity > 0.62 {
                            BlockType::Fern
                        } else {
                            BlockType::TallGrass
                        }
                    }
                };

                let wy = sample.surface + 1;
                if wy < CHUNK_H {
                    blocks[x][wy][z] = block;
                }
            }
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
        // NV-2.0 realism: each column maps to a real Earth coordinate (the
        // seed's anchor ± latitude/longitude offset) and the *measured*
        // temperature / precipitation there picks the climate zone, which
        // decides the dominant biome (desert stays desert, rainforest stays
        // rainforest, taiga stays taiga). The noise temperature/humidity
        // stay untouched — they carve local variety *inside* the zone
        // (oases, clearings, forest patches, swamps).
        let (rlat, rlon) = self.earth_position(x, z);
        let grid = super::meteo::climate_grid();
        let (real_temp_c, real_precip_mm) = grid.annual_at(rlat, rlon);
        let warmest = grid
            .monthly_temps(rlat, rlon)
            .iter()
            .copied()
            .fold(f64::MIN, f64::max);
        let zone = whittaker_zone(real_temp_c, warmest, real_precip_mm);
        let erosion = ((fbm4(self.erosion_seed, sample_x * 0.0028, sample_z * 0.0028) + 1.0) * 0.5)
            .clamp(0.0, 1.0);
        let ridges = ridge(fbm4(self.peak_seed, sample_x * 0.0048, sample_z * 0.0048)).clamp(0.0, 1.0);
        let variation = fbm4(self.detail_seed, sample_x * 0.0062, sample_z * 0.0062);
        // No smooth_step here: it pushes mid-range noise to the 0/1 extremes,
        // which turns the map into flat ocean + flat plateau. A linear remap
        // keeps continents rolling with real coastlines.
        let landness = remap01(continent, -0.55, 0.45);
        let mountainness = smooth_step(remap01(
            ridges * (1.0 - erosion * 0.55) + landness * 0.20,
            0.62,
            0.92,
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
            zone,
            real_temp_c,
            real_precip_mm,
        }
    }

    /// Earth coordinates (lat, lon) for a world column: the seed's anchor
    /// plus a per-block offset so the real climate varies across the world.
    fn earth_position(&self, x: f64, z: f64) -> (f64, f64) {
        let (alat, alon) = super::meteo::world_coordinates(self.seed);
        let lat = (alat + z * LAT_PER_BLOCK).clamp(-80.0, 80.0);
        let lon = (alon + x * LAT_PER_BLOCK).rem_euclid(360.0);
        (lat, lon)
    }

    /// Real-climate-driven biome selection: the Whittaker zone (measured
    /// temperature / precipitation) decides which biome dominates, noise
    /// only carves local pockets (oases in deserts, forest patches in
    /// steppe, clearings in rainforest). Ocean / Coast / Mountains always
    /// win where the terrain says so.
    fn select_biome(&self, climate: ClimateSample) -> BiomeId {
        if climate.landness < 0.15 {
            return BiomeId::Ocean;
        }
        if climate.landness < 0.27 {
            return BiomeId::Coast;
        }
        if climate.mountainness > 0.68 {
            return BiomeId::Mountains;
        }
        let t = climate.temperature;
        let h = climate.humidity;
        let v = climate.variation;

        match climate.zone {
            ClimateZone::Tundra => {
                // Polar: treeless snowfields (taiga biome, snow-covered via
                // the real-climate snowline).
                BiomeId::Taiga
            }
            ClimateZone::Boreal => {
                // Cold conifer belt; wetter/warmer pockets turn to forest.
                if h > 0.6 && t > 0.48 {
                    BiomeId::Forest
                } else if h > 0.5 && v > 0.04 {
                    BiomeId::Forest
                } else {
                    BiomeId::Taiga
                }
            }
            ClimateZone::Temperate => {
                // Classic four-season mix.
                if climate.swampiness > 0.34 && t > 0.46 && climate.landness < 0.72 {
                    BiomeId::Swamp
                } else if h > 0.68 && v > 0.04 {
                    BiomeId::DarkForest
                } else if h > 0.48 {
                    BiomeId::Forest
                } else {
                    BiomeId::Plains
                }
            }
            ClimateZone::Dry => {
                // Steppe / grassland; riversides and wet pockets turn green.
                if climate.swampiness > 0.34 && t > 0.46 {
                    BiomeId::Swamp
                } else if h > 0.56 && t > 0.5 {
                    BiomeId::Forest
                } else {
                    BiomeId::Plains
                }
            }
            ClimateZone::Desert => {
                // Arid: desert everywhere except rare oasis/scrub pockets
                // where local noise spikes humidity.
                if h > 0.68 && t > 0.5 {
                    BiomeId::Plains
                } else {
                    BiomeId::Desert
                }
            }
            ClimateZone::TropicalWet => {
                // Rainforest; swamps in the lowlands, jungle everywhere.
                if climate.swampiness > 0.34 && climate.landness < 0.72 {
                    BiomeId::Swamp
                } else if h > 0.6 {
                    BiomeId::DarkForest
                } else {
                    BiomeId::Forest
                }
            }
        }
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

        // Continental shelf profile: the ocean floor stays deep, then the
        // shelf ramps up to a beach near sea level, then land climbs gently
        // inland. landness ∈ [0, 1] (0 = deep ocean, 1 = high continent).
        let sea = SEA_LEVEL as f64;
        // Shelf ramps slowly and keeps rising across most of the landness
        // range, so lowlands, hills and highlands all exist (a fast ramp
        // collapses everything into one flat plateau height).
        let shelf = smooth_step(remap01(climate.landness, 0.10, 0.52));
        // deep ocean floor, slowly rising across the shelf
        let base = sea - 32.0 + shelf * 52.0;
        // biome base offsets push highlands up / lowlands down from the shelf
        let base = base + definition.base_height * shelf;
        // Deep ocean gets its own strong rolling so the floor isn't a flat
        // slab; land keeps full per-biome relief so it isn't a plateau.
        let rolling = macro_noise * (definition.relief * 1.6) * (0.25 + shelf * 0.75)
            + (1.0 - shelf) * macro_noise * 5.0;
        let local = local_noise * definition.relief * (0.25 + shelf * 0.75);
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

    /// Real-climate snowline: the altitude (in world y) above which the
    /// ground stays snow-covered. Driven by the measured annual temperature
    /// at the column's Earth location — polar worlds snow at sea level,
    /// deserts/rainforests almost never.
    fn snowline(&self, wx: i32, wz: i32) -> usize {
        let (rlat, rlon) = self.earth_position(wx as f64, wz as f64);
        let (t_c, _) = super::meteo::climate_grid().annual_at(rlat, rlon);
        // -15 °C → sea-level snow (y ≈ 42); +20 °C → only high peaks (y ≈ 98)
        let coldness = ((-t_c) / 15.0).clamp(0.0, 1.0);
        let warmness = (t_c / 20.0).clamp(0.0, 1.0);
        let line = 90.0 - coldness * 48.0 + warmness * 8.0;
        line.round().clamp(42.0, 100.0) as usize
    }

    /// True when a ground-cover block (grass tuft, fern, dead bush…) may
    /// sit on top of this surface block.
    fn supports_cover_surface(&self, block: BlockType) -> bool {
        matches!(
            block,
            BlockType::Grass
                | BlockType::ForestFloor
                | BlockType::BloomFloor
                | BlockType::CoarseSoil
                | BlockType::RootedSoil
                | BlockType::MossMat
                | BlockType::Mud
                | BlockType::Sand
        )
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
                if sample.surface >= self.snowline(wx, wz) {
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
                if sample.surface >= self.snowline(wx, wz) {
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

        // Regional atmosphere from the real climate at this column — the
        // sky and fog follow the measured climate of the whole region.
        let (rlat, rlon) = self.earth_position(wx as f64, wz as f64);
        let grid = super::meteo::climate_grid();
        let (atmos_temp_c, atmos_precip_mm) = grid.annual_at(rlat, rlon);
        let atmos_zone = whittaker_zone(
            atmos_temp_c,
            grid.monthly_temps(rlat, rlon)
                .iter()
                .copied()
                .fold(f64::MIN, f64::max),
            atmos_precip_mm,
        );
        let atmosphere = climate_atmosphere(atmos_zone, atmos_temp_c, atmos_precip_mm);

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
            atmosphere,
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

    /// Seeds whose Earth location lands in a specific real climate zone
    /// (see `meteo::world_coordinates`). Used so tests span the full range
    /// of real climates instead of all landing in the Arctic.
    const TEMPERATE_SEED: u32 = 627_736_576; // 51°N 0°E — London
    const TROPICAL_SEED: u32 = 2_411_746_645; // 3°S 60°W — Amazon
    const DESERT_SEED: u32 = 1_519_749_802; // 24°N 15°E — Sahara
    const BOREAL_SEED: u32 = 330_416_127; // 60°N 90°E — Siberia

    fn samples_expected_biome_variety_across_known_seeds() {
        // The union across real climate zones must cover every biome:
        // tropical rainforests, deserts, boreal taiga and temperate forest.
        let seeds = [TEMPERATE_SEED, TROPICAL_SEED, DESERT_SEED, BOREAL_SEED];
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

    #[test]
    fn world_dominant_biome_matches_its_real_climate() {
        // The core realism guarantee: a Sahara seed must be mostly desert,
        // an Amazon seed mostly rainforest, a Siberian seed mostly taiga.
        let cases = [
            (DESERT_SEED, BiomeId::Desert, 0.30, false),
            (TROPICAL_SEED, BiomeId::Forest, 0.45, true), // Forest+DarkForest = rainforest
            (BOREAL_SEED, BiomeId::Taiga, 0.40, false),
        ];

        for (seed, dominant, min_share, plus_dark) in cases {
            let generator = BiomeGenerator::new(seed);
            let mut counts = std::collections::HashMap::new();
            let mut land = 0usize;
            for wz in (-768..=768).step_by(32) {
                for wx in (-768..=768).step_by(32) {
                    let b = generator.get_biome(wx, wz);
                    if matches!(b, BiomeId::Ocean | BiomeId::Coast) {
                        continue;
                    }
                    land += 1;
                    *counts.entry(b).or_insert(0usize) += 1;
                }
            }
            let mut share = *counts.get(&dominant).unwrap_or(&0) as f64 / land as f64;
            if plus_dark {
                share += *counts.get(&BiomeId::DarkForest).unwrap_or(&0) as f64 / land as f64;
            }
            assert!(
                share >= min_share,
                "seed {seed}: {:?} share {share:.0}% < {min_share}% — world does not match its real climate",
                dominant
            );
        }
    }

    #[test]
    fn terrain_has_healthy_distribution_and_no_flat_plateau() {
        // Regression guard for the worldgen overhaul: the map must not be a
        // giant flat ocean/plateau (bimodal landness used to clamp most of
        // the map to a single height) and water must not swallow the world.
        let mut water_share = 0.0f64;

        for seed in [42_u32, 1_337_u32, 20_260_405_u32] {
            let generator = BiomeGenerator::new(seed);
            let mut seed_water = 0usize;
            let mut seed_total = 0usize;
            let mut seed_hist = [0usize; CHUNK_H.div_ceil(8)];

            for wz in (-640..=640).step_by(8) {
                for wx in (-640..=640).step_by(8) {
                    let sample = generator.sample_column(wx, wz);
                    seed_total += 1;
                    if sample.water_top > sample.surface {
                        seed_water += 1;
                    }
                    let bucket = (sample.surface / 8).min(seed_hist.len() - 1);
                    seed_hist[bucket] += 1;
                }
            }

            // no single 8-block height band may dominate the map
            for (bucket, &count) in seed_hist.iter().enumerate() {
                let share = count as f64 / seed_total as f64;
                if share > 0.45 {
                    panic!(
                        "seed {seed}: bucket y={} holds {:.0}% of columns — flat plateau, bucket share must stay < 45%",
                        bucket * 8,
                        share * 100.0,
                    );
                }
            }

            let share = seed_water as f64 / seed_total as f64;
            assert!(
                share < 0.40,
                "seed {seed}: {:.0}% underwater — must stay under 40%",
                share * 100.0
            );
            assert!(
                share > 0.02,
                "seed {seed}: only {:.0}% underwater — oceans should exist for variety",
                share * 100.0
            );
            water_share += share;
        }

        let _avg = water_share / 3.0;
    }

    #[test]
    fn cover_vegetation_respects_biome_surface() {
        // The ground-cover pass must only place on biome-appropriate surfaces
        // (no sticks floating over water / sand-in-plains-style nonsense).
        // Temperate world — the classic mix of plains/forest with cover.
        let generator = BiomeGenerator::new(TEMPERATE_SEED);
        let mut cover_count = 0;
        let mut unsupported = 0usize;
        for cz in -1..=1 {
            for cx in -1..=1 {
                let mut blocks = Box::new([[[BlockType::Air; CHUNK_D]; CHUNK_H]; CHUNK_W]);
                let mut writes = Vec::new();
                generator.populate_chunk(cx, cz, &mut blocks, &mut writes);

                for x in 0..CHUNK_W {
                    for z in 0..CHUNK_D {
                        // find the topmost non-air block in this column
                        let mut top = None;
                        for y in (1..CHUNK_H).rev() {
                            if blocks[x][y][z] != BlockType::Air {
                                top = Some(y);
                                break;
                            }
                        }
                        if let Some(y) = top {
                            let block = blocks[x][y][z];
                            if matches!(
                                block,
                                BlockType::TallGrass
                                    | BlockType::Fern
                                    | BlockType::DeadBush
                                    | BlockType::Cactus
                                    | BlockType::Stick
                                    | BlockType::StickSmall
                            ) {
                                cover_count += 1;
                                // cover must sit on a supported surface
                                assert_ne!(y, 0);
                                if !matches!(
                                    blocks[x][y - 1][z],
                                    BlockType::Grass
                                        | BlockType::ForestFloor
                                        | BlockType::BloomFloor
                                        | BlockType::CoarseSoil
                                        | BlockType::RootedSoil
                                        | BlockType::MossMat
                                        | BlockType::Mud
                                        | BlockType::Sand
                                ) {
                                    unsupported += 1;
                                }
                            }
                        }
                    }
                }
            }
        }
        assert!(cover_count > 0, "expected ground cover in the chunks");
        assert_eq!(unsupported, 0, "cover placed on unsupported surface");
    }
}

