pub mod block;
pub mod chunk;
pub mod biomes;
pub mod generator;
pub mod raycast;
pub mod palette;
pub mod liquid;
pub mod vegetation;
pub mod worldgen;
pub mod ai_generator;
pub mod decorations;
pub mod decoration_ai;
pub mod online_trainer;

use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::io::BufWriter;
use std::path::Path;
use std::sync::Arc;
use std::sync::mpsc;
use anyhow::{Context, Result};
use cgmath::Vector3;
use serde::{Deserialize, Serialize};
use chunk::{Chunk, GeneratedChunk, CHUNK_W, CHUNK_D};
use biomes::{Biome, BiomeGenerator};
use biomes::SEA_LEVEL;
use generator::{ChunkGenerator, GeneratorMessage};
use worldgen::WorldBlockWrite;
use ai_generator::AISystem;
use decorations::DecorationManager;
use crate::{crafting::NVCrafterState, inventory::ItemStack, settings::SharedSettings};
pub use block::BlockType;
pub use raycast::RaycastHit;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WorldItemDrop {
    pub position: Vector3<i32>,
    pub stack: ItemStack,
}

pub struct World {
    pub chunks: HashMap<(i32, i32), Chunk>,
    generator: Arc<BiomeGenerator>,
    chunk_gen: ChunkGenerator,
    gen_receiver: mpsc::Receiver<GeneratorMessage>,
    pending_chunks: std::collections::HashSet<(i32, i32)>,
    pending_world_writes: HashMap<(i32, i32), Vec<WorldBlockWrite>>,
    tree_populated_chunks: HashSet<(i32, i32)>,
    nvcrafter_states: HashMap<(i32, i32, i32), NVCrafterState>,
    dropped_items: Vec<WorldItemDrop>,
    settings: SharedSettings,
    // ── AI System ────────────────────────────────────────────────────────
    pub ai_system: AISystem,
    ai_receiver: mpsc::Receiver<ai_generator::AIMessage>,
    // ── Decoration System (Phase 2) ──────────────────────────────────────
    pub decorations: DecorationManager,
}

#[derive(Serialize, Deserialize)]
struct ChunkSave {
    cx: i32,
    cz: i32,
    blocks: Vec<u8>,
}

#[derive(Serialize, Deserialize)]
struct NVCrafterSave {
    wx: i32,
    wy: i32,
    wz: i32,
    state: NVCrafterState,
}

#[derive(Serialize, Deserialize)]
struct WorldSave {
    seed: u32,
    chunks: Vec<ChunkSave>,
    #[serde(default)]
    nvcrafters: Vec<NVCrafterSave>,
}

impl World {
    pub fn new(seed: u32) -> Self {
        Self::new_with_settings(seed, SharedSettings::default())
    }

    pub fn new_with_settings(seed: u32, settings: SharedSettings) -> Self {
        let (chunk_gen, gen_receiver) = ChunkGenerator::new_with_seed_and_settings(seed, settings.clone());
        let generator = Arc::clone(chunk_gen.generator());
        let (ai_system, ai_receiver) = AISystem::new();
        Self {
            chunks: HashMap::new(),
            generator,
            chunk_gen,
            gen_receiver,
            pending_chunks: std::collections::HashSet::new(),
            pending_world_writes: HashMap::new(),
            tree_populated_chunks: HashSet::new(),
            nvcrafter_states: HashMap::new(),
            dropped_items: Vec::new(),
            settings,
            ai_system,
            ai_receiver,
            decorations: DecorationManager::new(),
        }
    }

    pub fn low_end_mode_enabled(&self) -> bool {
        self.settings.low_end_pc()
    }

    fn ensure_chunk_generated(&mut self, cx: i32, cz: i32) {
        if self.chunks.contains_key(&(cx, cz)) {
            return;
        }

        let generated = Chunk::generate(cx, cz, &self.generator);
        // Chunks materialized only because a canopy write crossed a chunk border
        // stay terrain-only here; the explicit load/insert path runs the tree pass.
        let _ = self.insert_generated_chunk_internal(cx, cz, generated, false);
    }

    fn insert_generated_chunk(&mut self, cx: i32, cz: i32, generated: GeneratedChunk) -> bool {
        self.insert_generated_chunk_internal(cx, cz, generated, true)
    }

    fn insert_generated_chunk_internal(
        &mut self,
        cx: i32,
        cz: i32,
        generated: GeneratedChunk,
        populate_trees: bool,
    ) -> bool {
        let key = (cx, cz);
        let GeneratedChunk { chunk, writes } = generated;

        if self.chunks.contains_key(&key) {
            self.pending_chunks.remove(&key);
            return false;
        }

        self.chunks.insert(key, chunk);

        self.pending_chunks.remove(&key);
        self.apply_generated_writes(writes);

        if let Some(pending) = self.pending_world_writes.remove(&key) {
            self.apply_generated_writes(pending);
        }

        if populate_trees {
            self.populate_world_trees_for_chunk(cx, cz);
        }

        true
    }

    // Runs the world-space tree pass exactly once for explicitly inserted chunks.
    pub(crate) fn populate_world_trees_for_chunk(&mut self, cx: i32, cz: i32) {
        let key = (cx, cz);
        if !self.chunks.contains_key(&key) || !self.tree_populated_chunks.insert(key) {
            return;
        }

        let generator = Arc::clone(&self.generator);
        generator.populate_world_trees_for_chunk(self, cx, cz);
    }

    fn apply_generated_writes(&mut self, writes: Vec<WorldBlockWrite>) {
        for write in writes {
            self.apply_generated_write(write);
        }
    }

    fn apply_generated_write(&mut self, write: WorldBlockWrite) {
        if write.wy <= 0 || write.wy >= chunk::CHUNK_H as i32 {
            return;
        }

        let cx = write.wx.div_euclid(CHUNK_W as i32);
        let cz = write.wz.div_euclid(CHUNK_D as i32);
        let lx = write.wx.rem_euclid(CHUNK_W as i32) as usize;
        let lz = write.wz.rem_euclid(CHUNK_D as i32) as usize;

        if let Some(chunk) = self.chunks.get_mut(&(cx, cz)) {
            let current = *chunk.get(lx, write.wy as usize, lz);
            if write.rule.allows(current) {
                chunk.set(lx, write.wy as usize, lz, write.block);
            }
            return;
        }

        self.pending_world_writes.entry((cx, cz)).or_default().push(write);
    }

    /// Process any completed chunks from background generation.
    ///
    /// Returns the coordinates of every chunk that was newly inserted this
    /// call so the renderer can rebuild seam meshes for their neighbours.
    pub fn process_generated_chunks(&mut self) -> Vec<(i32, i32)> {
        // Dispatch pending work onto rayon each frame.
        self.chunk_gen.flush();
        // Drain any completed chunks.
        let mut inserted = Vec::new();
        while let Ok(GeneratorMessage::ChunkReady(cx, cz, generated)) = self.gen_receiver.try_recv() {
            if self.insert_generated_chunk(cx, cz, generated) {
                inserted.push((cx, cz));
            }
        }
        inserted
    }

    /// Queue chunks for loading within `radius` of chunk (cx, cz)
    pub fn load_around(&mut self, cx: i32, cz: i32, radius: i32) {
        // Ensure the current chunk and its immediate neighbors exist before background loading.
        // This avoids the player moving into unloaded space and falling through visible terrain.
        let sync_radius = 1;

        if self.chunks.is_empty() {
            for dz in -sync_radius..=sync_radius {
                for dx in -sync_radius..=sync_radius {
                    let key = (cx + dx, cz + dz);
                    if !self.chunks.contains_key(&key) {
                        let generated = Chunk::generate(key.0, key.1, &self.generator);
                        self.insert_generated_chunk(key.0, key.1, generated);
                    } else {
                        self.populate_world_trees_for_chunk(key.0, key.1);
                    }
                }
            }
        }

        let mut to_generate = Vec::new();
        
        for dz in -radius..=radius {
            for dx in -radius..=radius {
                let key = (cx + dx, cz + dz);
                if self.chunks.contains_key(&key) {
                    self.populate_world_trees_for_chunk(key.0, key.1);
                    continue;
                }

                if dx.abs() <= sync_radius && dz.abs() <= sync_radius {
                    let generated = Chunk::generate(key.0, key.1, &self.generator);
                    self.insert_generated_chunk(key.0, key.1, generated);
                } else if !self.pending_chunks.contains(&key) {
                    to_generate.push(key);
                    self.pending_chunks.insert(key);
                }
            }
        }
        
        // Queue all distant chunks for background generation
        if !to_generate.is_empty() {
            self.chunk_gen.queue_chunks(&to_generate);
        }
    }

    /// Unload chunks outside the given radius
    pub fn unload_far_chunks(&mut self, cx: i32, cz: i32, radius: i32) {
        let unload_radius = radius + 2; // buffer
        let mut to_remove = Vec::new();
        for &(chunk_x, chunk_z) in self.chunks.keys() {
            let dx = (chunk_x - cx).abs();
            let dz = (chunk_z - cz).abs();
            if dx > unload_radius || dz > unload_radius {
                to_remove.push((chunk_x, chunk_z));
            }
        }
        for key in to_remove {
            self.chunks.remove(&key);
            self.pending_chunks.remove(&key);
            self.tree_populated_chunks.remove(&key);
        }
        self.nvcrafter_states.retain(|&(wx, _, wz), _| {
            let ccx = wx.div_euclid(CHUNK_W as i32);
            let ccz = wz.div_euclid(CHUNK_D as i32);
            (ccx - cx).abs() <= unload_radius && (ccz - cz).abs() <= unload_radius
        });
        self.dropped_items.retain(|drop| {
            let ccx = drop.position.x.div_euclid(CHUNK_W as i32);
            let ccz = drop.position.z.div_euclid(CHUNK_D as i32);
            (ccx - cx).abs() <= unload_radius && (ccz - cz).abs() <= unload_radius
        });
    }

    pub fn get_chunk(&self, cx: i32, cz: i32) -> Option<&Chunk> {
        self.chunks.get(&(cx, cz))
    }

    pub fn get_chunk_mut(&mut self, cx: i32, cz: i32) -> Option<&mut Chunk> {
        self.chunks.get_mut(&(cx, cz))
    }

    pub fn get_block(&self, wx: i32, wy: i32, wz: i32) -> BlockType {
        if wy < 0 || wy >= chunk::CHUNK_H as i32 {
            return BlockType::Air;
        }
        let cx = wx.div_euclid(CHUNK_W as i32);
        let cz = wz.div_euclid(CHUNK_D as i32);
        let lx = wx.rem_euclid(CHUNK_W as i32) as usize;
        let lz = wz.rem_euclid(CHUNK_D as i32) as usize;
        self.chunks
            .get(&(cx, cz))
            .map(|c| *c.get(lx, wy as usize, lz))
            .unwrap_or(BlockType::Air)
    }

    pub fn set_block(&mut self, wx: i32, wy: i32, wz: i32, block: BlockType) {
        if wy < 0 || wy >= chunk::CHUNK_H as i32 {
            return;
        }
        let previous = self.get_block(wx, wy, wz);
        let cx = wx.div_euclid(CHUNK_W as i32);
        let cz = wz.div_euclid(CHUNK_D as i32);
        let lx = wx.rem_euclid(CHUNK_W as i32) as usize;
        let lz = wz.rem_euclid(CHUNK_D as i32) as usize;

        self.ensure_chunk_generated(cx, cz);

        if let Some(chunk) = self.chunks.get_mut(&(cx, cz)) {
            chunk.set(lx, wy as usize, lz, block);
            if block != BlockType::Water {
                chunk.water_meta_set(lx, wy as usize, lz, 0);
            }
        }

        self.sync_block_state(Vector3::new(wx, wy, wz), previous, block);
    }

    fn sync_block_state(&mut self, position: Vector3<i32>, previous: BlockType, next: BlockType) {
        let key = (position.x, position.y, position.z);
        if previous == BlockType::NVCrafter && next != BlockType::NVCrafter {
            self.nvcrafter_states.remove(&key);
        }

        if previous != BlockType::NVCrafter && next == BlockType::NVCrafter {
            self.nvcrafter_states.entry(key).or_insert_with(NVCrafterState::new);
        }
    }

    pub fn ensure_nvcrafter_state(&mut self, position: Vector3<i32>) -> Option<&mut NVCrafterState> {
        if self.get_block(position.x, position.y, position.z) != BlockType::NVCrafter {
            return None;
        }

        let key = (position.x, position.y, position.z);
        self.nvcrafter_states.entry(key).or_insert_with(NVCrafterState::new);
        self.nvcrafter_states.get_mut(&key)
    }

    pub fn nvcrafter_state(&self, position: Vector3<i32>) -> Option<&NVCrafterState> {
        self.nvcrafter_states.get(&(position.x, position.y, position.z))
    }

    pub fn nvcrafter_state_mut(&mut self, position: Vector3<i32>) -> Option<&mut NVCrafterState> {
        self.ensure_nvcrafter_state(position)
    }

    fn take_nvcrafter_state(&mut self, position: Vector3<i32>) -> Option<NVCrafterState> {
        self.nvcrafter_states.remove(&(position.x, position.y, position.z))
    }

    fn push_item_drop(&mut self, position: Vector3<i32>, stack: ItemStack) {
        self.dropped_items.push(WorldItemDrop { position, stack });
    }

    pub fn queue_item_drop(&mut self, position: Vector3<i32>, stack: ItemStack) {
        self.push_item_drop(position, stack);
    }

    pub fn dropped_items(&self) -> &[WorldItemDrop] {
        &self.dropped_items
    }

    pub fn drain_item_drops_at(&mut self, position: Vector3<i32>) -> Vec<ItemStack> {
        let mut drained = Vec::new();
        let mut retained = Vec::with_capacity(self.dropped_items.len());

        for drop in self.dropped_items.drain(..) {
            if drop.position == position {
                drained.push(drop.stack);
            } else {
                retained.push(drop);
            }
        }

        self.dropped_items = retained;
        drained
    }

    pub fn raycast_block(&self, origin: Vector3<f32>, direction: Vector3<f32>) -> Option<RaycastHit> {
        raycast::raycast_solid_block(origin, direction, 5.0, self)
    }

    pub fn destroy_block(&mut self, pos: Vector3<i32>) -> Option<BlockType> {
        let block = self.get_block(pos.x, pos.y, pos.z);
        if matches!(block, BlockType::Air | BlockType::Water | BlockType::Bedrock) {
            return None;
        }
        if !block.is_solid() && block.movement_medium().is_none() {
            return None;
        }

        if block == BlockType::NVCrafter {
            if let Some(state) = self.take_nvcrafter_state(pos) {
                for idx in 0..state.grid.active_len() {
                    if let Some(stack) = state.grid.get_slot(idx).clone() {
                        self.push_item_drop(pos, stack);
                    }
                }
            }
        }

        self.set_block(pos.x, pos.y, pos.z, BlockType::Air);
        self.set_water_meta(pos.x, pos.y, pos.z, 0);
        Some(block)
    }

    pub fn place_block(&mut self, pos: Vector3<i32>, block: BlockType) -> bool {
        if !block.is_placeable_item() {
            return false;
        }

        if pos.y < 0 || pos.y >= chunk::CHUNK_H as i32 {
            return false;
        }

        if self.get_block(pos.x, pos.y, pos.z) != BlockType::Air {
            return false;
        }

        self.set_block(pos.x, pos.y, pos.z, block);
        true
    }

    pub(crate) fn set_block_if_allowed(
        &mut self,
        wx: i32,
        wy: i32,
        wz: i32,
        block: BlockType,
        rule: worldgen::BlockWriteRule,
    ) -> bool {
        if wy < 0 || wy >= chunk::CHUNK_H as i32 {
            return false;
        }

        let cx = wx.div_euclid(CHUNK_W as i32);
        let cz = wz.div_euclid(CHUNK_D as i32);
        let lx = wx.rem_euclid(CHUNK_W as i32) as usize;
        let lz = wz.rem_euclid(CHUNK_D as i32) as usize;

        self.ensure_chunk_generated(cx, cz);

        let current = self
            .chunks
            .get(&(cx, cz))
            .map(|chunk| *chunk.get(lx, wy as usize, lz))
            .unwrap_or(BlockType::Air);
        if !rule.allows(current) {
            return false;
        }

        self.set_block(wx, wy, wz, block);
        true
    }

    /// Read block by world coords and return the numeric block ID
    pub fn get_block_id(&self, wx: i32, wy: i32, wz: i32) -> u8 {
        self.get_block(wx, wy, wz).id()
    }

    /// Write block by world coords using numeric block ID
    pub fn set_block_id(&mut self, wx: i32, wy: i32, wz: i32, block_id: u8) {
        if let Some(block) = BlockType::from_id(block_id) {
            self.set_block(wx, wy, wz, block);
        }
    }

    /// Query ambient lighting color and multiplier at world coordinates.
    /// Returns `[r, g, b, multiplier]` where `multiplier` is applied to ambient term.
    pub fn ambient_at(&self, wx: i32, wz: i32) -> [f32; 4] {
        self.generator.ambient_at(wx, wz)
    }

    pub fn visuals_at(&self, wx: i32, wz: i32) -> biomes::SurfaceVisuals {
        self.generator.visuals_at(wx, wz)
    }

    pub fn biome_at(&self, wx: i32, wz: i32) -> Biome {
        self.generator.get_biome(wx, wz)
    }

    /// Return the surface height (y) at world coordinates using the biome generator.
    pub fn surface_height(&self, wx: i32, wz: i32) -> u32 {
        self.generator.surface_height(wx, wz)
    }

    /// Return the highest generated column top at world coordinates.
    /// This is water level where present, otherwise terrain surface.
    pub fn column_top_height(&self, wx: i32, wz: i32) -> u32 {
        self.generator.sample_column(wx, wz).water_top as u32
    }

    pub fn safe_teleport_position(&mut self, wx: i32, wy: i32, wz: i32) -> Result<(f32, f32, f32)> {
        let cx = wx.div_euclid(CHUNK_W as i32);
        let cz = wz.div_euclid(CHUNK_D as i32);
        self.load_around(cx, cz, 1);

        let min_y = 1;
        let max_y = chunk::CHUNK_H as i32 - 3;
        let start_y = wy.clamp(min_y, max_y);

        for y in start_y..=max_y {
            if self.player_column_is_clear(wx, y, wz) {
                return Ok((wx as f32 + 0.5, y as f32, wz as f32 + 0.5));
            }
        }

        for y in (min_y..start_y).rev() {
            if self.player_column_is_clear(wx, y, wz) {
                return Ok((wx as f32 + 0.5, y as f32, wz as f32 + 0.5));
            }
        }

        anyhow::bail!("No safe teleport space found near ({}, {}, {}).", wx, wy, wz)
    }

    fn player_column_is_clear(&self, wx: i32, wy: i32, wz: i32) -> bool {
        !self.get_block(wx, wy, wz).is_opaque() && !self.get_block(wx, wy + 1, wz).is_opaque()
    }

    fn spawn_volume_is_clear(&self, wx: i32, wy: i32, wz: i32) -> bool {
        self.spawn_volume_block_is_clear(self.get_block(wx, wy, wz))
            && self.spawn_volume_block_is_clear(self.get_block(wx, wy + 1, wz))
    }

    fn spawn_volume_block_is_clear(&self, block: BlockType) -> bool {
        !block.is_solid() && block.movement_medium().is_none() && !matches!(block, BlockType::Water)
    }

    fn find_spawn_support_y(&self, wx: i32, wz: i32) -> Option<i32> {
        let max_y = chunk::CHUNK_H as i32 - 3;

        for y in (0..=max_y).rev() {
            let block = self.get_block(wx, y, wz);
            if !block.is_solid() {
                continue;
            }

            if self.spawn_volume_is_clear(wx, y + 1, wz) {
                return Some(y);
            }
        }

        None
    }

    fn spawn_spiral_points(center_x: i32, center_z: i32, max_ring: i32) -> Vec<(i32, i32)> {
        let mut pts = vec![(center_x, center_z)];

        for ring in 1..=max_ring {
            let d = ring;
            for i in -ring..=ring {
                pts.push((center_x + i, center_z - d));
                pts.push((center_x + i, center_z + d));
            }
            for i in (-ring + 1)..ring {
                pts.push((center_x - d, center_z + i));
                pts.push((center_x + d, center_z + i));
            }
        }

        pts
    }

    fn find_spawn_position_near(&mut self, center_x: i32, center_z: i32, search_radius: i32) -> Option<(f32, f32, f32)> {
        let chunk_radius_x = (search_radius + CHUNK_W as i32 - 1) / CHUNK_W as i32;
        let chunk_radius_z = (search_radius + CHUNK_D as i32 - 1) / CHUNK_D as i32;
        let chunk_radius = chunk_radius_x.max(chunk_radius_z).max(1);
        let cx = center_x.div_euclid(CHUNK_W as i32);
        let cz = center_z.div_euclid(CHUNK_D as i32);
        self.load_around(cx, cz, chunk_radius);

        for (wx, wz) in Self::spawn_spiral_points(center_x, center_z, search_radius) {
            if let Some(solid_y) = self.find_spawn_support_y(wx, wz) {
                return Some((wx as f32 + 0.5, solid_y as f32 + 1.0, wz as f32 + 0.5));
            }
        }

        None
    }

    /// Find a safe land spawn point, scattered by the world seed.
    ///
    /// Candidate X/Z positions still come from the biome generator, but the
    /// final Y and clearance checks always use actual loaded world blocks so
    /// runtime canopy and water writes cannot place the player inside trees.
    pub fn find_spawn_point(&mut self) -> (f32, f32, f32) {
        // Derive five independent offsets from different hash rotations.
        let seed = self.generator.seed();
        let offsets: [(i32, i32); 5] = {
            let h0 = seed.wrapping_mul(0x9e3779b9_u32);
            let h1 = h0.wrapping_mul(0x517cc1b7_u32);
            let h2 = h1.wrapping_mul(0x6c62272e_u32);
            let h3 = h2.wrapping_mul(0xcc9e2d51_u32);
            let h4 = h3.wrapping_mul(0xac4c1b51_u32);
            let to_offset = |h: u32| {
                let ox = ((h & 0x7f) as i32 - 64) * 8;
                let oz = (((h >> 8) & 0x7f) as i32 - 64) * 8;
                (ox, oz)
            };
            [to_offset(h0), to_offset(h1), to_offset(h2), to_offset(h3), to_offset(h4)]
        };

        let step        = 4i32;
        let max_ring    = 128i32;   // ±512 blocks per offset
        let local_radius = 8i32;

        // Helper: spiral-ring iterator for a given centre.
        let ring_pts = |ox: i32, oz: i32| -> Vec<(i32, i32)> {
            let mut pts = vec![(ox, oz)];
            for ring in 1i32..=max_ring {
                let d = ring * step;
                for i in -ring..=ring {
                    pts.push((ox + i * step, oz - d));
                    pts.push((ox + i * step, oz + d));
                }
                for i in (-ring + 1)..ring {
                    pts.push((ox - d, oz + i * step));
                    pts.push((ox + d, oz + i * step));
                }
            }
            pts
        };

        // ── Phase 1: full candidate check ────────────────────────────────────
        for &(ox, oz) in &offsets {
            for (wx, wz) in ring_pts(ox, oz) {
                if self.generator.is_spawn_candidate(wx, wz) {
                    if let Some(spawn) = self.find_spawn_position_near(wx, wz, local_radius) {
                        return spawn;
                    }
                }
            }
        }

        // ── Phase 2: relaxed — any land surface (no biome filter) ────────────
        for &(ox, oz) in &offsets {
            for (wx, wz) in ring_pts(ox, oz) {
                if self.generator.is_land_surface(wx, wz) {
                    if let Some(spawn) = self.find_spawn_position_near(wx, wz, local_radius) {
                        return spawn;
                    }
                }
            }
        }

        // ── Phase 3: brute-force grid from world origin ───────────────────────
        eprintln!("⚠ spawn: all hashed offsets failed — scanning from origin");
        for ring in 0i32..=256 {
            let d = ring * 8;
            let pts: Vec<(i32, i32)> = if ring == 0 {
                vec![(0, 0)]
            } else {
                let mut v = Vec::new();
                for i in -ring..=ring {
                    v.push((i * 8, -d));
                    v.push((i * 8,  d));
                }
                for i in (-ring + 1)..ring {
                    v.push((-d, i * 8));
                    v.push(( d, i * 8));
                }
                v
            };
            for (wx, wz) in pts {
                if self.generator.is_land_surface(wx, wz) {
                    if let Some(spawn) = self.find_spawn_position_near(wx, wz, local_radius) {
                        return spawn;
                    }
                }
            }
        }

        eprintln!("⚠ spawn: land surface scan failed — brute forcing origin vicinity");
        if let Some(spawn) = self.find_spawn_position_near(0, 0, 96) {
            return spawn;
        }

        for search_radius in [160, 256, 384] {
            if let Some(spawn) = self.find_spawn_position_near(0, 0, search_radius) {
                eprintln!("⚠ spawn: using extended origin fallback with radius {}", search_radius);
                return spawn;
            }
        }

        panic!("spawn: unable to locate a valid solid block with clear runtime headroom");
    }

    /// Liquid simulation tick — delegates to the dedicated liquid module.
    /// Called at a throttled rate (~0.5 s) by the renderer update loop.
    pub fn simulate_water(&mut self) {
        liquid::simulate_step(self);
    }

    /// Get per-voxel water metadata (0 if none)
    pub fn get_water_meta(&self, wx: i32, wy: i32, wz: i32) -> u8 {
        if wy < 0 || wy >= chunk::CHUNK_H as i32 { return 0; }
        let cx = wx.div_euclid(CHUNK_W as i32);
        let cz = wz.div_euclid(CHUNK_D as i32);
        let lx = wx.rem_euclid(CHUNK_W as i32) as usize;
        let lz = wz.rem_euclid(CHUNK_D as i32) as usize;
        self.chunks
            .get(&(cx, cz))
            .map(|c| c.water_meta_get(lx, wy as usize, lz))
            .unwrap_or(0)
    }

    /// Set per-voxel water metadata (no-op if chunk missing)
    pub fn set_water_meta(&mut self, wx: i32, wy: i32, wz: i32, meta: u8) {
        if wy < 0 || wy >= chunk::CHUNK_H as i32 { return; }
        let cx = wx.div_euclid(CHUNK_W as i32);
        let cz = wz.div_euclid(CHUNK_D as i32);
        let lx = wx.rem_euclid(CHUNK_W as i32) as usize;
        let lz = wz.rem_euclid(CHUNK_D as i32) as usize;
        if let Some(chunk) = self.chunks.get_mut(&(cx, cz)) {
            chunk.water_meta_set(lx, wy as usize, lz, meta);
        }
    }

    pub fn save_to_file<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        let path = path.as_ref();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).context("create save directory")?;
        }

        let chunks = self.chunks.iter().map(|(&(cx, cz), chunk)| ChunkSave {
            cx,
            cz,
            blocks: chunk.flatten(),
        }).collect();

        let save = WorldSave {
            seed: self.generator.seed(),
            chunks,
            nvcrafters: self
                .nvcrafter_states
                .iter()
                .map(|(&(wx, wy, wz), state)| NVCrafterSave {
                    wx,
                    wy,
                    wz,
                    state: state.clone(),
                })
                .collect(),
        };

        let file = File::create(path).context("create save file")?;
        let writer = BufWriter::new(file);
        serde_json::to_writer(writer, &save).context("serialize world save")?;
        Ok(())
    }

    pub fn load_from_file<P: AsRef<Path>>(path: P) -> Result<Self> {
        Self::load_from_file_with_settings(path, SharedSettings::default())
    }

    pub fn load_from_file_with_settings<P: AsRef<Path>>(path: P, settings: SharedSettings) -> Result<Self> {
        let file = File::open(path.as_ref()).context("open save file")?;
        let save: WorldSave = serde_json::from_reader(file).context("deserialize world save")?;

        let mut world = World::new_with_settings(save.seed, settings);
        for chunk_save in save.chunks {
            let chunk = Chunk::from_flat(&chunk_save.blocks);
            world.chunks.insert((chunk_save.cx, chunk_save.cz), chunk);
        }
        for crafter in save.nvcrafters {
            world
                .nvcrafter_states
                .insert((crafter.wx, crafter.wy, crafter.wz), crafter.state);
        }
        world.tree_populated_chunks = world.chunks.keys().copied().collect();

        Ok(world)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spawn_position_near_skips_medium_only_columns() {
        let mut world = World::new(42);
        world.ensure_chunk_generated(0, 0);

        for y in 0..chunk::CHUNK_H as i32 {
            world.set_block(0, y, 0, BlockType::TreeLeaves);
        }

        let spawn = world
            .find_spawn_position_near(0, 0, 2)
            .expect("expected nearby runtime spawn above a solid block");

        let wx = spawn.0.floor() as i32;
        let wy = spawn.1 as i32;
        let wz = spawn.2.floor() as i32;

        assert_ne!((wx, wz), (0, 0), "spawn should move off medium-only columns");
        assert!(world.get_block(wx, wy - 1, wz).is_solid(), "spawn must sit above a solid block");
        assert!(world.spawn_volume_is_clear(wx, wy, wz), "spawn volume must avoid solids, water, and foliage mediums");
    }

    #[test]
    fn find_spawn_point_uses_loaded_runtime_blocks() {
        for seed in [42_u32, 1_337_u32, 7_654_321_u32] {
            let mut world = World::new(seed);
            let spawn = world.find_spawn_point();

            let wx = spawn.0.floor() as i32;
            let wy = spawn.1 as i32;
            let wz = spawn.2.floor() as i32;

            assert!(world.get_block(wx, wy - 1, wz).is_solid(), "spawn must resolve to a solid support block for seed {seed}");
            assert!(world.spawn_volume_is_clear(wx, wy, wz), "spawn must resolve to clear runtime space for seed {seed}");
        }
    }

    #[test]
    fn world_space_tree_canopies_cross_chunk_boundaries() {
        fn is_canopy_block(block: BlockType) -> bool {
            matches!(
                block,
                BlockType::TreeLeaves
                    | BlockType::NeedleCanopy
                    | BlockType::WetCanopy
                    | BlockType::PaleCanopy
                    | BlockType::DarkCanopy
            )
        }

        let mut found = false;

        'outer: for source_cz in -8..=8 {
            for source_cx in -8..=8 {
                let mut world = World::new(42);
                let generated = Chunk::generate(source_cx, source_cz, &world.generator);
                assert!(world.insert_generated_chunk(source_cx, source_cz, generated));

                for (&(dest_cx, dest_cz), chunk) in &world.chunks {
                    if (dest_cx, dest_cz) == (source_cx, source_cz) {
                        continue;
                    }
                    if world.tree_populated_chunks.contains(&(dest_cx, dest_cz)) {
                        continue;
                    }

                    let mut canopy_found = false;
                    for x in 0..CHUNK_W {
                        for y in 0..chunk::CHUNK_H {
                            for z in 0..CHUNK_D {
                                if is_canopy_block(*chunk.get(x, y, z)) {
                                    canopy_found = true;
                                    break;
                                }
                            }
                            if canopy_found {
                                break;
                            }
                        }
                        if canopy_found {
                            break;
                        }
                    }

                    if canopy_found {
                        assert_ne!((source_cx, source_cz), (dest_cx, dest_cz));
                        assert!(chunk.is_dirty, "neighbor chunk should be dirtied by world.set_block canopy writes");
                        assert!(
                            !world.tree_populated_chunks.contains(&(dest_cx, dest_cz)),
                            "cross-chunk canopy destination should stay terrain-only until explicitly loaded"
                        );
                        found = true;
                        break 'outer;
                    }
                }
            }
        }

        assert!(found, "expected to find at least one canopy spanning into a neighboring chunk");
    }

    #[test]
    fn raycast_hits_first_solid_block_and_skips_foliage() {
        let mut world = World::new(9001);
        world.set_block(0, 64, 1, BlockType::TreeLeaves);
        world.set_block(0, 64, 2, BlockType::Stone);

        let hit = world
            .raycast_block(Vector3::new(0.5, 64.5, 0.5), Vector3::new(0.0, 0.0, 1.0))
            .expect("expected raycast to hit the first solid block");

        assert_eq!(hit.block_pos, Vector3::new(0, 64, 2));
        assert_eq!(hit.face_normal, Vector3::new(0, 0, -1));
        assert_eq!(hit.block_type, BlockType::Stone);
    }

    #[test]
    fn destroyed_block_marks_chunk_dirty() {
        let mut world = World::new(1337);
        world.load_around(0, 0, 1);
        world.set_block(2, 60, 2, BlockType::Stone);

        if let Some(chunk) = world.get_chunk_mut(0, 0) {
            chunk.is_dirty = false;
        }

        let removed = world.destroy_block(Vector3::new(2, 60, 2));
        assert_eq!(removed, Some(BlockType::Stone));
        assert_eq!(world.get_block(2, 60, 2), BlockType::Air);
        assert!(world.get_chunk(0, 0).unwrap().is_dirty);
    }

    #[test]
    fn breaking_nvcrafter_releases_internal_items_into_world_drops() {
        let mut world = World::new(2026);
        let pos = Vector3::new(4, 64, 4);
        world.place_block(pos, BlockType::NVCrafter);

        let crafter = world
            .ensure_nvcrafter_state(pos)
            .expect("placed crafter should have state");
        crafter.grid.set_slot(
            0,
            Some(ItemStack::from_inventory_item(BlockType::Planks).expect("planks should exist")),
        );
        crafter.grid.set_slot(
            1,
            Some(ItemStack::from_inventory_item(BlockType::IronIngot).expect("ingot should exist")),
        );

        let removed = world.destroy_block(pos);

        assert_eq!(removed, Some(BlockType::NVCrafter));
        assert_eq!(world.get_block(pos.x, pos.y, pos.z), BlockType::Air);
        assert_eq!(world.dropped_items().len(), 2);
        assert!(world
            .dropped_items()
            .iter()
            .any(|drop| drop.stack.block_type == Some(BlockType::Planks)));
        assert!(world
            .dropped_items()
            .iter()
            .any(|drop| drop.stack.block_type == Some(BlockType::IronIngot)));
    }
}
