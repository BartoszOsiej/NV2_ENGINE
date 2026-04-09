use opensimplex2::smooth as simplex;

use super::biomes::{BiomeGenerator, BiomeId, ColumnSample, TreeKind};
use super::block::BlockType;
use super::chunk::{CHUNK_D, CHUNK_H, CHUNK_W};
use super::worldgen::{BlockWriteRule, WorldGenWriter};
use super::World;

const TREE_CELL_SIZE: i32 = 6;
const GRASS_CELL_SIZE: i32 = 2;
const FLOWER_CELL_SIZE: i32 = 4;
const SHRUB_CELL_SIZE: i32 = 3;

const DARK_OAK_LAYERS: &[(i32, i32)] = &[(-2, 2), (-1, 3), (0, 4), (1, 3), (2, 2)];
const SWAMP_LAYERS: &[(i32, i32)] = &[(-1, 2), (0, 3), (1, 3), (2, 2)];

pub struct VegetationGenerator;

#[derive(Clone, Copy)]
struct TreeDefinition {
    trunk_block: BlockType,
    has_canopy: bool,
    min_height: i32,
    max_height: i32,
    max_slope: f64,
}

#[derive(Clone, Copy)]
struct BlockPlacement {
    wx: i32,
    wy: i32,
    wz: i32,
    block: BlockType,
    rule: BlockWriteRule,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
struct WorldPos {
    x: i32,
    y: i32,
    z: i32,
}

impl WorldPos {
    fn new(x: i32, y: i32, z: i32) -> Self {
        Self { x, y, z }
    }
}

#[derive(Clone, Copy)]
struct CanopyAnchor {
    root: WorldPos,
    top: WorldPos,
    height: i32,
}

#[derive(Clone, Copy)]
struct CanopyRng {
    seed: i64,
}

impl CanopyRng {
    fn new(seed: i64) -> Self {
        Self { seed }
    }

    fn for_anchor(world_seed: i64, anchor: CanopyAnchor) -> Self {
        let seed = world_seed
            .wrapping_add((anchor.root.x as i64).wrapping_mul(31))
            .wrapping_add((anchor.root.y as i64).wrapping_mul(131))
            .wrapping_add((anchor.root.z as i64).wrapping_mul(17))
            .wrapping_add((anchor.top.x as i64).wrapping_mul(47))
            .wrapping_add((anchor.top.y as i64).wrapping_mul(97))
            .wrapping_add((anchor.top.z as i64).wrapping_mul(53))
            .wrapping_add((anchor.height as i64).wrapping_mul(173));
        Self::new(seed)
    }

    fn sample2(self, salt: i64, wx: i32, wz: i32, scale: f64) -> f64 {
        noise2_01(self.seed.wrapping_add(salt), wx, wz, scale)
    }

    fn sample3(self, salt: i64, wx: i32, wy: i32, wz: i32, scale: f64) -> f64 {
        noise3_01(self.seed.wrapping_add(salt), wx, wy, wz, scale)
    }
}

struct TreePlan {
    anchor: CanopyAnchor,
    trunk: Vec<BlockPlacement>,
    extras: Vec<BlockPlacement>,
}

impl TreePlan {
    fn new(anchor: CanopyAnchor) -> Self {
        Self {
            anchor,
            trunk: Vec::new(),
            extras: Vec::new(),
        }
    }
}

impl VegetationGenerator {
    pub fn new() -> Self {
        Self
    }

    pub fn populate_chunk(
        &self,
        generator: &BiomeGenerator,
        cx: i32,
        cz: i32,
        writer: &mut WorldGenWriter<'_>,
    ) {
        self.place_grass(generator, cx, cz, writer);
        self.place_flowers(generator, cx, cz, writer);
        self.place_shrubs(generator, cx, cz, writer);
    }

    pub fn populate_world_trees_for_chunk(
        &self,
        world: &mut World,
        generator: &BiomeGenerator,
        cx: i32,
        cz: i32,
    ) {
        self.place_trees(world, generator, cx, cz);
    }

    fn place_trees(&self, world: &mut World, generator: &BiomeGenerator, cx: i32, cz: i32) {
        let world_seed = generator.seed() as i64;

        for_each_chunk_cell(cx, cz, TREE_CELL_SIZE, world_seed, 1_003, |wx, wz| {
            let sample = generator.sample_column(wx, wz);
            if sample.definition.tree_types.is_empty() || sample.water_top != sample.surface {
                return;
            }
            if !supports_tree_surface(sample.surface_block) {
                return;
            }

            let density = noise2_01(world_seed.wrapping_add(1_041), wx, wz, 0.19);
            if density > sample.definition.tree_density {
                return;
            }

            let slope = slope_at(generator, wx, wz);
            let kind = choose_tree_kind(sample.definition.tree_types, world_seed.wrapping_add(1_101), wx, wz);
            let definition = tree_definition(kind, sample.biome);
            if slope > definition.max_slope {
                return;
            }

            let root_y = surface_y_from_sample(&sample);
            let Some(plan) = build_trunk_plan(kind, definition, world_seed, wx, root_y, wz) else {
                return;
            };

            let canopy = if definition.has_canopy {
                let rng = CanopyRng::for_anchor(world_seed.wrapping_add(5_101), plan.anchor);
                canopy_builder::build_canopy(plan.anchor, sample.biome, rng)
            } else {
                Vec::new()
            };

            if !trunk_plan_is_valid(generator, &plan) {
                return;
            }

            apply_trunk_plan(world, &plan);
            apply_canopy(world, generator, sample.biome, canopy);
        });
    }

    fn place_grass(
        &self,
        generator: &BiomeGenerator,
        cx: i32,
        cz: i32,
        writer: &mut WorldGenWriter<'_>,
    ) {
        let world_seed = generator.seed() as i64;

        for_each_chunk_cell(cx, cz, GRASS_CELL_SIZE, world_seed, 2_003, |wx, wz| {
            let sample = generator.sample_column(wx, wz);
            if sample.water_top != sample.surface || !supports_grass_surface(sample.surface_block) {
                return;
            }
            if noise2_01(world_seed.wrapping_add(2_031), wx, wz, 0.47) > sample.definition.grass_density {
                return;
            }

            let Some(surface_y) = surface_y_at(generator, wx, wz) else {
                return;
            };
            let block = match sample.biome {
                BiomeId::Swamp if noise2_01(world_seed.wrapping_add(2_071), wx, wz, 0.81) > 0.60 => BlockType::Bush,
                _ => BlockType::TallGrass,
            };
            writer.set_block(wx, surface_y + 1, wz, block, BlockWriteRule::GroundCover);
        });
    }

    fn place_flowers(
        &self,
        generator: &BiomeGenerator,
        cx: i32,
        cz: i32,
        writer: &mut WorldGenWriter<'_>,
    ) {
        let world_seed = generator.seed() as i64;

        for_each_chunk_cell(cx, cz, FLOWER_CELL_SIZE, world_seed, 3_003, |wx, wz| {
            let sample = generator.sample_column(wx, wz);
            if sample.water_top != sample.surface || !supports_flower_surface(sample.surface_block) {
                return;
            }
            if noise2_01(world_seed.wrapping_add(3_031), wx, wz, 0.39) > sample.definition.flower_density {
                return;
            }

            let Some(surface_y) = surface_y_at(generator, wx, wz) else {
                return;
            };
            writer.set_block(wx, surface_y + 1, wz, BlockType::Flower, BlockWriteRule::GroundCover);
        });
    }

    fn place_shrubs(
        &self,
        generator: &BiomeGenerator,
        cx: i32,
        cz: i32,
        writer: &mut WorldGenWriter<'_>,
    ) {
        let world_seed = generator.seed() as i64;

        for_each_chunk_cell(cx, cz, SHRUB_CELL_SIZE, world_seed, 4_003, |wx, wz| {
            let sample = generator.sample_column(wx, wz);
            if sample.water_top != sample.surface || !supports_shrub_surface(sample.surface_block) {
                return;
            }
            if noise2_01(world_seed.wrapping_add(4_031), wx, wz, 0.31) > sample.definition.shrub_density {
                return;
            }

            let Some(surface_y) = surface_y_at(generator, wx, wz) else {
                return;
            };

            match sample.biome {
                BiomeId::Desert => {
                    if noise2_01(world_seed.wrapping_add(4_101), wx, wz, 0.73) > 0.72 {
                        let height = choose_range(world_seed.wrapping_add(4_131), 2, 4, wx, wz, 0.51);
                        for offset in 1..=height {
                            writer.set_block(wx, surface_y + offset, wz, BlockType::Cactus, BlockWriteRule::GroundCover);
                        }
                    } else {
                        writer.set_block(wx, surface_y + 1, wz, BlockType::DeadBush, BlockWriteRule::GroundCover);
                    }
                }
                BiomeId::Swamp => {
                    let block = if noise2_01(world_seed.wrapping_add(4_171), wx, wz, 0.67) > 0.55 {
                        BlockType::RootLattice
                    } else {
                        BlockType::Bush
                    };
                    writer.set_block(wx, surface_y + 1, wz, block, BlockWriteRule::GroundCover);
                }
                _ => {
                    writer.set_block(wx, surface_y + 1, wz, BlockType::Bush, BlockWriteRule::GroundCover);
                }
            }
        });
    }
}

fn tree_definition(kind: TreeKind, biome: BiomeId) -> TreeDefinition {
    match kind {
        TreeKind::Oak => TreeDefinition {
            trunk_block: BlockType::TreeTrunk,
            has_canopy: true,
            min_height: if biome == BiomeId::Plains { 3 } else { 4 },
            max_height: if biome == BiomeId::Plains { 5 } else { 6 },
            max_slope: if biome == BiomeId::Swamp { 2.4 } else { 3.5 },
        },
        TreeKind::Birch => TreeDefinition {
            trunk_block: BlockType::PaleWood,
            has_canopy: true,
            min_height: 5,
            max_height: 7,
            max_slope: 3.0,
        },
        TreeKind::Pine => TreeDefinition {
            trunk_block: BlockType::NeedleWood,
            has_canopy: true,
            min_height: 7,
            max_height: 10,
            max_slope: 5.0,
        },
        TreeKind::DarkOak => TreeDefinition {
            trunk_block: BlockType::DarkWood,
            has_canopy: true,
            min_height: 6,
            max_height: 8,
            max_slope: 3.0,
        },
        TreeKind::DeadTree => TreeDefinition {
            trunk_block: BlockType::WarmWood,
            has_canopy: false,
            min_height: 4,
            max_height: 7,
            max_slope: 2.6,
        },
    }
}

fn build_trunk_plan(
    kind: TreeKind,
    definition: TreeDefinition,
    world_seed: i64,
    wx: i32,
    root_y: i32,
    wz: i32,
) -> Option<TreePlan> {
    let height = choose_range(
        world_seed.wrapping_add(5_003),
        definition.min_height,
        definition.max_height,
        wx,
        wz,
        0.27,
    );
    if root_y + height + 8 >= CHUNK_H as i32 {
        return None;
    }

    let root = WorldPos::new(wx, root_y, wz);
    let mut plan = TreePlan::new(CanopyAnchor {
        root,
        top: root,
        height,
    });
    let mut top_x = wx;
    let mut top_y = root_y;
    let mut top_z = wz;

    for step in 1..=height {
        let (offset_x, offset_z) = trunk_offset(kind, world_seed, step, height, wx, wz);
        let trunk_x = wx + offset_x;
        let trunk_y = root_y + step;
        let trunk_z = wz + offset_z;
        top_x = trunk_x;
        top_y = trunk_y;
        top_z = trunk_z;

        plan.trunk.push(BlockPlacement {
            wx: trunk_x,
            wy: trunk_y,
            wz: trunk_z,
            block: definition.trunk_block,
            rule: BlockWriteRule::TreeTrunk,
        });
    }

    plan.anchor.top = WorldPos::new(top_x, top_y, top_z);

    if kind == TreeKind::DeadTree {
        let (dir_x, dir_z) = direction_from_seed(world_seed.wrapping_add(5_211), wx, wz);
        let branch_y = root_y + height.saturating_sub(1);
        for reach in 1..=2 {
            plan.extras.push(BlockPlacement {
                wx: top_x + dir_x * reach,
                wy: branch_y + if reach == 2 { 1 } else { 0 },
                wz: top_z + dir_z * reach,
                block: definition.trunk_block,
                rule: BlockWriteRule::TreeTrunk,
            });
        }
    }

    Some(plan)
}

fn trunk_plan_is_valid(generator: &BiomeGenerator, plan: &TreePlan) -> bool {
    plan.trunk.iter().all(|block| trunk_position_is_valid(generator, block.wx, block.wy, block.wz))
        && plan.extras.iter().all(|block| trunk_position_is_valid(generator, block.wx, block.wy, block.wz))
}

fn apply_trunk_plan(world: &mut World, plan: &TreePlan) {
    for placement in &plan.trunk {
        world.set_block_if_allowed(placement.wx, placement.wy, placement.wz, placement.block, placement.rule);
    }
    for placement in &plan.extras {
        world.set_block_if_allowed(placement.wx, placement.wy, placement.wz, placement.block, placement.rule);
    }
}

fn apply_canopy(world: &mut World, generator: &BiomeGenerator, biome: BiomeId, canopy: Vec<WorldPos>) {
    let canopy_block = leaf_block_for_biome(biome);
    for position in canopy {
        if canopy_position_is_valid(generator, position.x, position.y, position.z) {
            world.set_block_if_allowed(
                position.x,
                position.y,
                position.z,
                canopy_block,
                BlockWriteRule::TreeCanopy,
            );
        }
    }
}

fn leaf_block_for_biome(biome: BiomeId) -> BlockType {
    match biome {
        BiomeId::DarkForest => BlockType::DarkCanopy,
        BiomeId::Swamp => BlockType::WetCanopy,
        BiomeId::Taiga | BiomeId::Mountains => BlockType::NeedleCanopy,
        _ => BlockType::TreeLeaves,
    }
}

mod canopy_builder {
    use super::{dedup_positions, collect_conical, collect_irregular_blob, collect_layered_discs, collect_sphere};
    use super::{BiomeId, CanopyAnchor, CanopyRng, WorldPos, DARK_OAK_LAYERS, SWAMP_LAYERS};

    pub(super) fn build_canopy(anchor: CanopyAnchor, biome: BiomeId, rng: CanopyRng) -> Vec<WorldPos> {
        let mut canopy = Vec::new();

        match biome {
            BiomeId::Plains => {
                collect_sphere(anchor.top.x, anchor.top.y, anchor.top.z, 1, 1, 1, &mut canopy);
                canopy.push(WorldPos::new(anchor.top.x, anchor.top.y + 1, anchor.top.z));
            }
            BiomeId::Forest => {
                collect_irregular_blob(anchor.top.x, anchor.top.y, anchor.top.z, 2, 2, 2, 0.24, rng, &mut canopy);
                canopy.push(WorldPos::new(anchor.top.x, anchor.top.y + 1, anchor.top.z));
            }
            BiomeId::DarkForest => {
                collect_layered_discs(anchor.top.x, anchor.top.y, anchor.top.z, DARK_OAK_LAYERS, &mut canopy);
                canopy.push(WorldPos::new(anchor.top.x, anchor.top.y + 1, anchor.top.z));
            }
            BiomeId::Swamp => {
                collect_layered_discs(anchor.top.x, anchor.top.y - 1, anchor.top.z, SWAMP_LAYERS, &mut canopy);
                if rng.sample2(211, anchor.top.x, anchor.top.z, 0.61) > 0.48 {
                    canopy.push(WorldPos::new(anchor.top.x, anchor.top.y + 1, anchor.top.z));
                }
            }
            BiomeId::Taiga | BiomeId::Mountains => {
                let cone_height = anchor.height.clamp(4, 6);
                let base_radius = if anchor.height >= 9 { 3 } else { 2 };
                collect_conical(anchor.top.x, anchor.top.y, anchor.top.z, cone_height, base_radius, &mut canopy);
            }
            _ => {
                collect_sphere(anchor.top.x, anchor.top.y, anchor.top.z, 2, 1, 2, &mut canopy);
            }
        }

        dedup_positions(&mut canopy);
        canopy
    }
}

fn collect_sphere(
    center_x: i32,
    center_y: i32,
    center_z: i32,
    radius_x: i32,
    radius_y: i32,
    radius_z: i32,
    out: &mut Vec<WorldPos>,
) {
    for dy in -radius_y..=radius_y {
        for dz in -radius_z..=radius_z {
            for dx in -radius_x..=radius_x {
                let nx = dx as f64 / radius_x.max(1) as f64;
                let ny = dy as f64 / radius_y.max(1) as f64;
                let nz = dz as f64 / radius_z.max(1) as f64;
                if nx * nx + ny * ny + nz * nz <= 1.12 {
                    out.push(WorldPos::new(center_x + dx, center_y + dy, center_z + dz));
                }
            }
        }
    }
}

fn collect_layered_discs(
    center_x: i32,
    center_y: i32,
    center_z: i32,
    layers: &[(i32, i32)],
    out: &mut Vec<WorldPos>,
) {
    for (dy, radius) in layers.iter().copied() {
        for dz in -radius..=radius {
            for dx in -radius..=radius {
                if dx * dx + dz * dz <= radius * radius + 1 {
                    out.push(WorldPos::new(center_x + dx, center_y + dy, center_z + dz));
                }
            }
        }
    }
}

fn collect_conical(
    center_x: i32,
    center_y: i32,
    center_z: i32,
    height: i32,
    base_radius: i32,
    out: &mut Vec<WorldPos>,
) {
    for layer in 0..height {
        let radius = ((height - layer) * base_radius + height - 1) / height;
        let y = center_y - layer;
        for dz in -radius..=radius {
            for dx in -radius..=radius {
                if dx * dx + dz * dz <= radius * radius + 1 {
                    out.push(WorldPos::new(center_x + dx, y, center_z + dz));
                }
            }
        }
    }
    out.push(WorldPos::new(center_x, center_y + 1, center_z));
}

fn collect_irregular_blob(
    center_x: i32,
    center_y: i32,
    center_z: i32,
    radius_x: i32,
    radius_y: i32,
    radius_z: i32,
    roughness: f64,
    rng: CanopyRng,
    out: &mut Vec<WorldPos>,
) {
    for dy in -radius_y..=radius_y {
        for dz in -radius_z..=radius_z {
            for dx in -radius_x..=radius_x {
                let nx = dx as f64 / radius_x.max(1) as f64;
                let ny = dy as f64 / radius_y.max(1) as f64;
                let nz = dz as f64 / radius_z.max(1) as f64;
                let distance = nx * nx + ny * ny + nz * nz;
                if distance > 1.25 {
                    continue;
                }
                let keep = rng.sample3(337, center_x + dx, center_y + dy, center_z + dz, 0.37);
                if distance <= 0.88 || keep > roughness {
                    out.push(WorldPos::new(center_x + dx, center_y + dy, center_z + dz));
                }
            }
        }
    }
}

fn dedup_positions(positions: &mut Vec<WorldPos>) {
    positions.sort_unstable_by_key(|pos| (pos.x, pos.y, pos.z));
    positions.dedup();
}

fn trunk_offset(kind: TreeKind, world_seed: i64, step: i32, height: i32, wx: i32, wz: i32) -> (i32, i32) {
    match kind {
        TreeKind::DarkOak => (0, 0),
        TreeKind::DeadTree => {
            if step > height / 2 {
                let (dx, dz) = direction_from_seed(world_seed.wrapping_add(5_361), wx, wz);
                let bend = ((step - height / 2) as f64 / (height - height / 2).max(1) as f64).round() as i32;
                (dx * bend.min(1), dz * bend.min(1))
            } else {
                (0, 0)
            }
        }
        _ => (0, 0),
    }
}

fn choose_tree_kind(tree_types: &'static [TreeKind], seed: i64, wx: i32, wz: i32) -> TreeKind {
    if tree_types.len() == 1 {
        return tree_types[0];
    }

    let index = choose_range(seed, 0, tree_types.len() as i32 - 1, wx, wz, 0.73) as usize;
    tree_types[index]
}

fn supports_tree_surface(block: BlockType) -> bool {
    matches!(
        block,
        BlockType::Grass
            | BlockType::ForestFloor
            | BlockType::BloomFloor
            | BlockType::CoarseSoil
            | BlockType::RootedSoil
            | BlockType::MossMat
            | BlockType::Mud
            | BlockType::Snow
    )
}

fn supports_grass_surface(block: BlockType) -> bool {
    matches!(
        block,
        BlockType::Grass
            | BlockType::ForestFloor
            | BlockType::BloomFloor
            | BlockType::CoarseSoil
            | BlockType::MossMat
            | BlockType::RootedSoil
    )
}

fn supports_flower_surface(block: BlockType) -> bool {
    matches!(block, BlockType::Grass | BlockType::BloomFloor | BlockType::ForestFloor)
}

fn supports_shrub_surface(block: BlockType) -> bool {
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
            | BlockType::Snow
    )
}

fn trunk_position_is_valid(generator: &BiomeGenerator, wx: i32, wy: i32, wz: i32) -> bool {
    if wy <= 0 || wy >= CHUNK_H as i32 {
        return false;
    }

    let sample = generator.sample_column(wx, wz);
    sample.water_top == sample.surface && wy > surface_y_from_sample(&sample)
}

fn canopy_position_is_valid(generator: &BiomeGenerator, wx: i32, wy: i32, wz: i32) -> bool {
    if wy <= 0 || wy >= CHUNK_H as i32 {
        return false;
    }

    let sample = generator.sample_column(wx, wz);
    wy > surface_y_from_sample(&sample).max(sample.water_top as i32)
}

fn slope_at(generator: &BiomeGenerator, wx: i32, wz: i32) -> f64 {
    let center = generator.surface_height(wx, wz) as i32;
    let neighbors = [
        generator.surface_height(wx + 1, wz) as i32,
        generator.surface_height(wx - 1, wz) as i32,
        generator.surface_height(wx, wz + 1) as i32,
        generator.surface_height(wx, wz - 1) as i32,
    ];

    neighbors
        .into_iter()
        .map(|value| (value - center).abs() as f64)
        .fold(0.0, f64::max)
}

fn chunk_bounds(cx: i32, cz: i32) -> (i32, i32, i32, i32) {
    let min_x = cx * CHUNK_W as i32;
    let max_x = min_x + CHUNK_W as i32;
    let min_z = cz * CHUNK_D as i32;
    let max_z = min_z + CHUNK_D as i32;
    (min_x, max_x, min_z, max_z)
}

fn for_each_chunk_cell<F>(
    cx: i32,
    cz: i32,
    cell_size: i32,
    world_seed: i64,
    seed_offset: i64,
    mut f: F,
) where
    F: FnMut(i32, i32),
{
    let (min_x, max_x, min_z, max_z) = chunk_bounds(cx, cz);
    let min_cell_x = min_x.div_euclid(cell_size);
    let max_cell_x = (max_x - 1).div_euclid(cell_size);
    let min_cell_z = min_z.div_euclid(cell_size);
    let max_cell_z = (max_z - 1).div_euclid(cell_size);

    for cell_z in min_cell_z..=max_cell_z {
        for cell_x in min_cell_x..=max_cell_x {
            let (wx, wz) = cell_anchor(
                world_seed.wrapping_add(seed_offset),
                cell_x,
                cell_z,
                cell_size,
                1,
            );
            if wx >= min_x && wx < max_x && wz >= min_z && wz < max_z {
                f(wx, wz);
            }
        }
    }
}

fn surface_y_from_sample(sample: &ColumnSample) -> i32 {
    sample.surface as i32
}

fn surface_y_at(generator: &BiomeGenerator, wx: i32, wz: i32) -> Option<i32> {
    let sample = generator.sample_column(wx, wz);
    if sample.water_top == sample.surface {
        Some(sample.surface as i32)
    } else {
        None
    }
}

fn cell_anchor(seed: i64, cell_x: i32, cell_z: i32, cell_size: i32, margin: i32) -> (i32, i32) {
    let min_x = cell_x * cell_size + margin;
    let max_x = cell_x * cell_size + cell_size - margin - 1;
    let min_z = cell_z * cell_size + margin;
    let max_z = cell_z * cell_size + cell_size - margin - 1;
    (
        choose_range(seed, min_x, max_x.max(min_x), cell_x, cell_z, 0.73),
        choose_range(seed.wrapping_add(19), min_z, max_z.max(min_z), cell_x, cell_z, 0.79),
    )
}

fn choose_range(seed: i64, min: i32, max: i32, wx: i32, wz: i32, scale: f64) -> i32 {
    if max <= min {
        return min;
    }
    let t = noise2_01(seed, wx, wz, scale);
    min + ((max - min) as f64 * t).round() as i32
}

fn direction_from_seed(seed: i64, wx: i32, wz: i32) -> (i32, i32) {
    match choose_range(seed, 0, 3, wx, wz, 0.91) {
        0 => (1, 0),
        1 => (-1, 0),
        2 => (0, 1),
        _ => (0, -1),
    }
}

fn noise2_01(seed: i64, wx: i32, wz: i32, scale: f64) -> f64 {
    ((simplex::noise2(seed, wx as f64 * scale, wz as f64 * scale) as f64) + 1.0) * 0.5
}

fn noise3_01(seed: i64, wx: i32, wy: i32, wz: i32, scale: f64) -> f64 {
    ((simplex::noise3_ImproveXZ(
        seed,
        wx as f64 * scale,
        wy as f64 * scale,
        wz as f64 * scale,
    ) as f64)
        + 1.0)
        * 0.5
}

fn is_tree_block(block: BlockType) -> bool {
    matches!(
        block,
        BlockType::TreeTrunk
            | BlockType::TreeLeaves
            | BlockType::NeedleWood
            | BlockType::NeedleCanopy
            | BlockType::PaleWood
            | BlockType::PaleCanopy
            | BlockType::DarkWood
            | BlockType::DarkCanopy
            | BlockType::WarmWood
            | BlockType::WetCanopy
    )
}

fn is_cover_block(block: BlockType) -> bool {
    matches!(
        block,
        BlockType::Bush
            | BlockType::TallGrass
            | BlockType::Flower
            | BlockType::DeadBush
            | BlockType::Cactus
            | BlockType::RootLattice
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::World;
    use std::collections::HashSet;

    #[test]
    fn dark_oak_trunks_stay_centered_under_canopy() {
        for step in 1..=8 {
            assert_eq!(trunk_offset(TreeKind::DarkOak, 42, step, 8, 0, 0), (0, 0));
        }
    }

    #[test]
    fn dark_forest_canopy_builds_a_gap_free_crown() {
        let anchor = CanopyAnchor {
            root: WorldPos::new(0, 64, 0),
            top: WorldPos::new(0, 71, 0),
            height: 7,
        };
        let canopy = canopy_builder::build_canopy(anchor, BiomeId::DarkForest, CanopyRng::for_anchor(42, anchor));
        let canopy: HashSet<_> = canopy.into_iter().collect();

        for (dy, radius) in DARK_OAK_LAYERS.iter().copied() {
            for dz in -radius..=radius {
                for dx in -radius..=radius {
                    if dx * dx + dz * dz <= radius * radius + 1 {
                        assert!(
                            canopy.contains(&WorldPos::new(anchor.top.x + dx, anchor.top.y + dy, anchor.top.z + dz)),
                            "missing dark canopy block at layer {dy} offset ({dx}, {dz})"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn biome_driven_vegetation_produces_expected_features() {
        let mut tree_blocks = 0usize;
        let mut cover_blocks = 0usize;

        for seed in [42_u32, 1_337_u32, 20_260_405_u32] {
            let mut world = World::new(seed);
            for cz in -4..=4 {
                for cx in -4..=4 {
                    world.load_around(cx, cz, 0);
                }
            }

            for chunk in world.chunks.values() {
                for x in 0..CHUNK_W {
                    for y in 0..CHUNK_H {
                        for z in 0..CHUNK_D {
                            let block = chunk.blocks[x][y][z];
                            if is_tree_block(block) {
                                tree_blocks += 1;
                            }
                            if is_cover_block(block) {
                                cover_blocks += 1;
                            }
                        }
                    }
                }
            }
        }

        assert!(tree_blocks > 1_000, "expected world-space tree pass to produce trees, got {tree_blocks}");
        assert!(cover_blocks > 400, "expected vegetation rewrite to produce cover, got {cover_blocks}");
    }
}
