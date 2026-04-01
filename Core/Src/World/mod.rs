pub mod block;
pub mod chunk;
pub mod biomes;
pub mod generator;
pub mod raycast;
pub mod palette;
pub mod liquid;

use std::collections::HashMap;
use std::fs::File;
use std::io::BufWriter;
use std::path::Path;
use std::sync::Arc;
use std::sync::mpsc;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use chunk::{Chunk, CHUNK_W, CHUNK_D};
use biomes::BiomeGenerator;
use generator::{ChunkGenerator, GeneratorMessage};
pub use block::BlockType;

pub struct World {
    pub chunks: HashMap<(i32, i32), Chunk>,
    generator: Arc<BiomeGenerator>,
    chunk_gen: ChunkGenerator,
    gen_receiver: mpsc::Receiver<GeneratorMessage>,
    pending_chunks: std::collections::HashSet<(i32, i32)>,
}

#[derive(Serialize, Deserialize)]
struct ChunkSave {
    cx: i32,
    cz: i32,
    blocks: Vec<u8>,
}

#[derive(Serialize, Deserialize)]
struct WorldSave {
    seed: u32,
    chunks: Vec<ChunkSave>,
}

impl World {
    pub fn new(seed: u32) -> Self {
        let gen = Arc::new(BiomeGenerator::new(seed));
        let (chunk_gen, gen_receiver) = ChunkGenerator::new_with_seed(seed);
        
        Self {
            chunks: HashMap::new(),
            generator: Arc::clone(&gen),
            chunk_gen,
            gen_receiver,
            pending_chunks: std::collections::HashSet::new(),
        }
    }

    /// Process any completed chunks from background generation
    ///
    /// Returns true if at least one chunk was inserted.
    pub fn process_generated_chunks(&mut self) -> bool {
        let mut inserted = false;
        // Non-blocking: try to receive all ready chunks
        while let Ok(GeneratorMessage::ChunkReady(cx, cz, chunk)) = self.gen_receiver.try_recv() {
            self.chunks.insert((cx, cz), chunk);
            self.pending_chunks.remove(&(cx, cz));
            inserted = true;
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
                        let chunk = Chunk::generate(key.0, key.1, &self.generator);
                        self.chunks.insert(key, chunk);
                    }
                }
            }
        }

        let mut to_generate = Vec::new();
        
        for dz in -radius..=radius {
            for dx in -radius..=radius {
                let key = (cx + dx, cz + dz);
                if self.chunks.contains_key(&key) {
                    continue;
                }

                if dx.abs() <= sync_radius && dz.abs() <= sync_radius {
                    let chunk = Chunk::generate(key.0, key.1, &self.generator);
                    self.chunks.insert(key, chunk);
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
        }
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
        let cx = wx.div_euclid(CHUNK_W as i32);
        let cz = wz.div_euclid(CHUNK_D as i32);
        let lx = wx.rem_euclid(CHUNK_W as i32) as usize;
        let lz = wz.rem_euclid(CHUNK_D as i32) as usize;

        if let Some(chunk) = self.chunks.get_mut(&(cx, cz)) {
            chunk.set(lx, wy as usize, lz, block);
        }
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

    /// Return the surface height (y) at world coordinates using the biome generator.
    pub fn surface_height(&self, wx: i32, wz: i32) -> u32 {
        self.generator.surface_height(wx, wz)
    }

    /// Find the nearest land spawn point to world origin (0, 0).
    ///
    /// Spirals outward in 8-block increments until a solid, above-sea-level, non-river
    /// surface is found. Falls back to Y=80 if no land is found within 512 blocks.
    pub fn find_spawn_point(&self) -> (f32, f32, f32) {
        // Check origin first
        if self.generator.is_land_surface(0, 0) {
            let sy = self.generator.surface_height(0, 0);
            return (0.5, sy as f32 + 1.8, 0.5);
        }

        let step = 8i32;
        for ring in 1i32..=64 {          // max radius = 64 * 8 = 512 blocks
            let d = ring * step;
            // Walk the perimeter of a square ring at Manhattan distance `d`
            let mut pts: Vec<(i32, i32)> = Vec::new();
            for i in -ring..=ring {
                pts.push((i * step, -d));
                pts.push((i * step,  d));
            }
            for i in (-ring + 1)..ring {
                pts.push((-d, i * step));
                pts.push(( d, i * step));
            }
            for (wx, wz) in pts {
                if self.generator.is_land_surface(wx, wz) {
                    let sy = self.generator.surface_height(wx, wz);
                    return (wx as f32 + 0.5, sy as f32 + 1.8, wz as f32 + 0.5);
                }
            }
        }

        // Fallback: above the sea level at origin
        (0.5, 80.0, 0.5)
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
        };

        let file = File::create(path).context("create save file")?;
        let writer = BufWriter::new(file);
        serde_json::to_writer(writer, &save).context("serialize world save")?;
        Ok(())
    }

    pub fn load_from_file<P: AsRef<Path>>(path: P) -> Result<Self> {
        let file = File::open(path.as_ref()).context("open save file")?;
        let save: WorldSave = serde_json::from_reader(file).context("deserialize world save")?;

        let mut world = World::new(save.seed);
        for chunk_save in save.chunks {
            let chunk = Chunk::from_flat(&chunk_save.blocks);
            world.chunks.insert((chunk_save.cx, chunk_save.cz), chunk);
        }

        Ok(world)
    }
}