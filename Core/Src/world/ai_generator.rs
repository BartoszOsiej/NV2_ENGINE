/// AI-powered terrain and vegetation generator — powered by **MeMLP**
/// (Modular embedded Multi-layer Perceptron Model, see [`crate::world::memplp`]).
///
/// This module implements a lightweight, fast-learning neural stack that:
/// - Generates vegetation placement decisions autonomously (deep MLP head)
/// - Classifies biomes from climate features (modular biome head)
/// - Selects procedural texture styles (modular texture head)
/// - Learns from terrain features, biome characteristics AND player actions
/// - Trains asynchronously in the background
/// - Generates realistic textures on-the-fly (pure CPU, works on any hardware)
/// - Loads internet datasets with a graceful offline fallback
/// - Saves and loads model checkpoints (JSON, with legacy migration)
use std::sync::{Arc, Mutex};
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread;
use std::path::Path;
use ndarray::{Array1, Array2};
use serde::{Deserialize, Serialize};
use crate::world::block::BlockType;
use crate::world::memplp::{self, MeMLP, argmax, one_hot_n};
use crate::world::memplp::biome_heuristic_target as memlp_biome_target;
use crate::world::memplp::texture_heuristic_target as memlp_texture_target;

/// Default checkpoint path (relative to the working directory).
pub const DEFAULT_CHECKPOINT: &str = "checkpoints/ai_model.json";

/// Message type for AI background training
pub enum AIMessage {
    TrainingProgress { epoch: u32, loss: f32 },
    TextureGenerated { seed: u64, texture_data: Vec<u8> },
    VegetationDecision { wx: i32, wy: i32, wz: i32, block: BlockType, confidence: f32 },
    /// A player action was recorded and fed into the model.
    PlayerFeedback { samples: usize, loss: f32 },
    /// An online dataset batch was merged into training.
    OnlineDataset { source: String, samples: usize },
    /// The model was saved to disk.
    CheckpointSaved { path: String },
}

/// The NV2.0 AI model — a **MeMLP** (Modular embedded Multi-layer
/// Perceptron Model) wrapper.
///
/// `TerrainAI` keeps the engine-facing API stable while the actual network
/// is the modular [`MeMLP`]: a deep vegetation head (8→24→16→4) plus biome
/// (8→12→9) and texture (8→12→6) heads, all serialisable to one JSON
/// checkpoint. Legacy single-hidden-layer checkpoints are migrated
/// automatically on load.
#[derive(Serialize)]
pub struct TerrainAI {
    /// The modular neural model.
    model: MeMLP,

    // Biased random generator
    rng_state: u64,

    // Training parameters
    learning_rate: f32,
    training_samples: usize,
}

/// The pre-MeMLP checkpoint layout (kept for `Deserialize` migration).
#[derive(Deserialize)]
struct LegacyTerrainAI {
    #[serde(flatten)]
    legacy: memplp::LegacyCheckpoint,
    #[serde(default)]
    rng_state: u64,
    #[serde(default = "default_lr")]
    learning_rate: f32,
    #[serde(default)]
    training_samples: usize,
}

fn default_lr() -> f32 {
    0.01
}

impl<'de> Deserialize<'de> for TerrainAI {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value = serde_json::Value::deserialize(deserializer)?;

        // Current format: `{ "model": MeMLP, "rng_state": .., ... }`.
        if value.get("model").is_some() {
            #[derive(Deserialize)]
            struct NewFormat {
                model: MeMLP,
                #[serde(default)]
                rng_state: u64,
                #[serde(default = "default_lr")]
                learning_rate: f32,
                #[serde(default)]
                training_samples: usize,
            }
            let n: NewFormat =
                serde_json::from_value(value).map_err(serde::de::Error::custom)?;
            return Ok(Self {
                model: n.model,
                rng_state: n.rng_state,
                learning_rate: n.learning_rate,
                training_samples: n.training_samples,
            });
        }

        // Legacy format: flat w1/b1/w2/b2 — migrate to MeMLP.
        let legacy: LegacyTerrainAI =
            serde_json::from_value(value).map_err(serde::de::Error::custom)?;
        Ok(Self {
            model: MeMLP::from_legacy(legacy.legacy),
            rng_state: legacy.rng_state,
            learning_rate: legacy.learning_rate,
            training_samples: legacy.training_samples,
        })
    }
}

impl TerrainAI {
    /// Create a new AI with random initialization (fresh MeMLP).
    pub fn new() -> Self {
        Self {
            model: MeMLP::new(),
            rng_state: 42u64,
            learning_rate: 0.01,
            training_samples: 0,
        }
    }

    /// MeMLP checkpoint format version of this model.
    pub fn model_version(&self) -> u32 {
        self.model.version
    }

    /// Total trainable parameters across all MeMLP modules.
    pub fn model_param_count(&self) -> usize {
        self.model.param_count()
    }

    /// Number of training samples seen so far.
    pub fn training_samples(&self) -> usize {
        self.training_samples
    }

    /* ------------------------------------------------------------ */
    /* Vegetation head (deep MLP: 8 → 24 → 16 → 4)                  */
    /* ------------------------------------------------------------ */

    /// Forward pass: terrain features -> vegetation decision
    ///
    /// Input features (8):
    /// - terrain_height (normalized)
    /// - terrain_slope
    /// - biome_temperature
    /// - biome_humidity
    /// - nearby_water_distance
    /// - nearby_vegetation_count
    /// - light_level (0.0-1.0)
    /// - noise_seed_value (0.0-1.0)
    ///
    /// Returns a probability distribution over flower/fern/stick/pebble.
    pub fn forward(&self, features: &[f32; 8]) -> [f32; 4] {
        let probs = self.model.vegetation.forward(features);
        [probs[0], probs[1], probs[2], probs[3]]
    }

    /// Backward pass: train the vegetation head on observed terrain data.
    pub fn backward(&mut self, features: &[f32; 8], target: [f32; 4]) -> f32 {
        let loss = self
            .model
            .vegetation
            .train(features, &target, self.learning_rate);
        self.training_samples += 1;
        loss
    }

    /* ------------------------------------------------------------ */
    /* Biome head (8 → 12 → 9, matches BiomeId order)               */
    /* ------------------------------------------------------------ */

    /// Predict the most likely biome class (0..9) from climate features.
    pub fn predict_biome(&self, features: &[f32; 8]) -> usize {
        argmax(&self.model.biome.forward(features))
    }

    /// Train the biome head on one sample. `class` is 0..9 (BiomeId order).
    pub fn train_biome(&mut self, features: &[f32; 8], class: usize) -> f32 {
        let target = one_hot_n(class, memplp::BIOME_ARCH[memplp::BIOME_ARCH.len() - 1]);
        self.model.biome.train(features, &target, self.learning_rate)
    }

    /* ------------------------------------------------------------ */
    /* Texture head (8 → 12 → 6 texture-style classes)              */
    /* ------------------------------------------------------------ */

    /// Predict the most likely texture-style class (0..5) from climate.
    pub fn predict_texture_style(&self, features: &[f32; 8]) -> usize {
        argmax(&self.model.texture.forward(features))
    }

    /// Train the texture head on one sample. `class` is 0..5.
    pub fn train_texture(&mut self, features: &[f32; 8], class: usize) -> f32 {
        let target = one_hot_n(class, memplp::TEXTURE_CLASSES);
        self.model.texture.train(features, &target, self.learning_rate)
    }

    /* ------------------------------------------------------------ */
    /* Checkpoints                                                   */
    /* ------------------------------------------------------------ */

    /// Serialize the whole model to a JSON file (weights + biases).
    ///
    /// Works on any machine — the model is ~1.2 KB, so this is instant.
    pub fn save_checkpoint<P: AsRef<Path>>(&self, path: P) -> anyhow::Result<()> {
        let path = path.as_ref();
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent)?;
            }
        }
        let json = serde_json::to_string_pretty(self)?;
        std::fs::write(path, json)?;
        Ok(())
    }

    /// Load a model from a JSON checkpoint. Returns `None` if the file is
    /// missing or corrupt — the caller falls back to a fresh model.
    pub fn load_checkpoint<P: AsRef<Path>>(path: P) -> Option<Self> {
        let data = std::fs::read_to_string(path).ok()?;
        serde_json::from_str(&data).ok()
    }

    /* ------------------------------------------------------------ */
    /* Procedural textures (pure CPU, deterministic)                 */
    /* ------------------------------------------------------------ */

    /// Generate a 16×16 RGBA texture tile for a block, purely on the CPU.
    ///
    /// Deterministic given the seed: the same block + seed always produces
    /// the same tile, so a world is reproducible on any hardware (no GPU,
    /// no external assets required).
    pub fn generate_texture_for_block(&self, block: BlockType, seed: u64) -> Vec<u8> {
        let (palette, pattern) = texture_style_for_block(block);
        generate_tile(palette, pattern, seed)
    }
}

impl Default for TerrainAI {
    fn default() -> Self {
        Self::new()
    }
}

/* ================================================================ */
/*  Procedural texture generation (CPU-side, deterministic)          */
/* ================================================================ */

/// Base palette for a procedural texture: two colors that get blended by noise.
pub struct TexturePalette {
    pub c0: [u8; 3],
    pub c1: [u8; 3],
    /// Optional third color (ore speckles, leaf holes, plank seams…).
    pub c2: Option<[u8; 3]>,
    pub alpha: u8,
}

/// Which visual pattern to synthesize.
pub enum TexturePattern {
    /// Smooth value-noise blend between c0 and c1.
    Noise,
    /// c0 base with c1 speckle blobs (ores).
    Speckle,
    /// Vertical streaks (wood/log sides).
    Wood,
    /// Concentric rings (log top).
    Rings,
    /// Dense chaotic noise with translucent holes (leaves).
    Leaves,
    /// Horizontal boards with seams (planks).
    Planks,
    /// Smooth noise with slight transparency (water).
    Water,
    /// Hot glowing noise (lava).
    Lava,
    /// Smooth fine noise (stone, sand, dirt…).
    Stone,
}

/// Deterministic seed derived from a texture name (used by the atlas).
pub fn name_seed(name: &str) -> u64 {
    let mut h: u64 = 0x9E3779B97F4A7C15;
    for b in name.bytes() {
        h = h.wrapping_mul(0x100000001B3).wrapping_add(b as u64);
        h ^= h >> 29;
    }
    h
}

/// Look up a palette + pattern by a texture file name (e.g. `"oak_log"`, `"grass_block_top"`).
///
/// Used by the texture atlas as a procedural fallback when an asset file is
/// missing — so the game renders fine even on a stripped-down asset set.
pub fn texture_style_for_name(name: &str) -> (TexturePalette, TexturePattern) {
    // Reuse the block-based styles where the name maps to a known block.
    if let Some(block) = BlockType::from_name(name) {
        return texture_style_for_block(block);
    }
    // Fall back on keyword heuristics for names without a block mapping
    // (e.g. `oak_log_top`, `grass_block_side`).
    let n = name.to_ascii_lowercase();
    if n.contains("log") || n.contains("wood") || n.contains("trunk") {
        (
            TexturePalette { c0: [98, 74, 48], c1: [120, 92, 60], c2: Some([76, 56, 36]), alpha: 255 },
            TexturePattern::Wood,
        )
    } else if n.contains("leaves") || n.contains("canopy") {
        (
            TexturePalette { c0: [56, 108, 52], c1: [86, 142, 70], c2: Some([40, 82, 40]), alpha: 235 },
            TexturePattern::Leaves,
        )
    } else if n.contains("grass") {
        (
            TexturePalette { c0: [74, 128, 56], c1: [104, 158, 74], c2: Some([52, 96, 40]), alpha: 255 },
            TexturePattern::Noise,
        )
    } else if n.contains("sand") {
        (
            TexturePalette { c0: [219, 205, 156], c1: [232, 220, 180], c2: None, alpha: 255 },
            TexturePattern::Noise,
        )
    } else if n.contains("snow") {
        (
            TexturePalette { c0: [238, 242, 248], c1: [252, 253, 255], c2: None, alpha: 255 },
            TexturePattern::Noise,
        )
    } else if n.contains("water") {
        (
            TexturePalette { c0: [20, 60, 150], c1: [50, 100, 210], c2: None, alpha: 190 },
            TexturePattern::Water,
        )
    } else if n.contains("lava") || n.contains("ember") || n.contains("glow") {
        (
            TexturePalette { c0: [120, 60, 20], c1: [235, 120, 30], c2: Some([255, 200, 80]), alpha: 255 },
            TexturePattern::Lava,
        )
    } else if n.contains("ore") {
        (
            TexturePalette { c0: [116, 118, 124], c1: [235, 210, 90], c2: Some([180, 150, 50]), alpha: 255 },
            TexturePattern::Speckle,
        )
    } else if n.contains("planks") {
        (
            TexturePalette { c0: [160, 128, 86], c1: [178, 146, 100], c2: Some([134, 106, 70]), alpha: 255 },
            TexturePattern::Planks,
        )
    } else {
        (
            TexturePalette { c0: [132, 122, 108], c1: [150, 140, 124], c2: None, alpha: 255 },
            TexturePattern::Stone,
        )
    }
}

/// Generate a 16×16 RGBA tile from a palette + pattern + seed (public API
/// used by the texture atlas).
pub fn generate_tile_texture(palette: TexturePalette, pattern: TexturePattern, seed: u64) -> Vec<u8> {
    generate_tile(palette, pattern, seed)
}

/// Map a block to a palette + pattern for procedural generation.
pub fn texture_style_for_block(block: BlockType) -> (TexturePalette, TexturePattern) {
    use BlockType::*;
    match block {
        Grass | ForestFloor | BloomFloor | MossMat | MossCarpet | Fern | FernPlant => (
            TexturePalette { c0: [74, 128, 56], c1: [104, 158, 74], c2: Some([52, 96, 40]), alpha: 255 },
            TexturePattern::Noise,
        ),
        Dirt | CoarseSoil | RootedSoil | Mud | PackedMud => (
            TexturePalette { c0: [108, 74, 48], c1: [134, 96, 62], c2: Some([84, 58, 38]), alpha: 255 },
            TexturePattern::Noise,
        ),
        Sand => (
            TexturePalette { c0: [219, 205, 156], c1: [232, 220, 180], c2: None, alpha: 255 },
            TexturePattern::Noise,
        ),
        Gravel => (
            TexturePalette { c0: [116, 112, 108], c1: [140, 136, 130], c2: Some([90, 88, 86]), alpha: 255 },
            TexturePattern::Speckle,
        ),
        Snow => (
            TexturePalette { c0: [238, 242, 248], c1: [252, 253, 255], c2: None, alpha: 255 },
            TexturePattern::Noise,
        ),
        Stone | Andesite | Tuff | SlateRock | Cobblestone | CobbleMoss => (
            TexturePalette { c0: [116, 118, 124], c1: [138, 140, 148], c2: Some([94, 96, 102]), alpha: 255 },
            TexturePattern::Stone,
        ),
        Bedrock => (
            TexturePalette { c0: [48, 48, 52], c1: [72, 72, 78], c2: Some([30, 30, 34]), alpha: 255 },
            TexturePattern::Stone,
        ),
        Obsidian => (
            TexturePalette { c0: [24, 20, 42], c1: [38, 32, 60], c2: Some([14, 12, 26]), alpha: 255 },
            TexturePattern::Stone,
        ),
        StoneBricks => (
            TexturePalette { c0: [122, 124, 130], c1: [140, 142, 150], c2: Some([104, 106, 112]), alpha: 255 },
            TexturePattern::Planks,
        ),
        Clay => (
            TexturePalette { c0: [152, 156, 168], c1: [170, 174, 186], c2: None, alpha: 255 },
            TexturePattern::Stone,
        ),
        CoalOre | SlateCoalOre => (
            TexturePalette { c0: [110, 112, 118], c1: [70, 70, 74], c2: Some([30, 30, 32]), alpha: 255 },
            TexturePattern::Speckle,
        ),
        IronOre | SlateDiamondOre => (
            TexturePalette { c0: [116, 118, 124], c1: [214, 160, 132], c2: Some([180, 120, 90]), alpha: 255 },
            TexturePattern::Speckle,
        ),
        GoldOre => (
            TexturePalette { c0: [116, 118, 124], c1: [255, 224, 96], c2: Some([230, 190, 50]), alpha: 255 },
            TexturePattern::Speckle,
        ),
        DiamondOre => (
            TexturePalette { c0: [116, 118, 124], c1: [120, 235, 220], c2: Some([80, 200, 190]), alpha: 255 },
            TexturePattern::Speckle,
        ),
        EmeraldOre => (
            TexturePalette { c0: [116, 118, 124], c1: [90, 225, 120], c2: Some([50, 190, 80]), alpha: 255 },
            TexturePattern::Speckle,
        ),
        RedstoneOre => (
            TexturePalette { c0: [116, 118, 124], c1: [240, 80, 60], c2: Some([200, 50, 40]), alpha: 255 },
            TexturePattern::Speckle,
        ),
        TreeTrunk | NeedleWood | WarmWood | WetWood | PaleWood | DarkWood => (
            TexturePalette { c0: [98, 74, 48], c1: [120, 92, 60], c2: Some([76, 56, 36]), alpha: 255 },
            TexturePattern::Wood,
        ),
        TreeLeaves | NeedleCanopy | WarmCanopy | WetCanopy | PaleCanopy | BloomCanopy | DarkCanopy => (
            TexturePalette { c0: [56, 108, 52], c1: [86, 142, 70], c2: Some([40, 82, 40]), alpha: 235 },
            TexturePattern::Leaves,
        ),
        Planks => (
            TexturePalette { c0: [160, 128, 86], c1: [178, 146, 100], c2: Some([134, 106, 70]), alpha: 255 },
            TexturePattern::Planks,
        ),
        Water => (
            TexturePalette { c0: [20, 60, 150], c1: [50, 100, 210], c2: None, alpha: 190 },
            TexturePattern::Water,
        ),
        EmberRock | GlowRock => (
            TexturePalette { c0: [120, 60, 20], c1: [235, 120, 30], c2: Some([255, 200, 80]), alpha: 255 },
            TexturePattern::Lava,
        ),
        _ => (
            TexturePalette { c0: [132, 122, 108], c1: [150, 140, 124], c2: None, alpha: 255 },
            TexturePattern::Stone,
        ),
    }
}

/// Deterministic hash of (x, y) for value noise.
fn hash2(x: u32, y: u32, seed: u64) -> f32 {
    let mut h = seed;
    h = h.wrapping_mul(0x9E3779B97F4A7C15).wrapping_add(x as u64);
    h ^= h >> 29;
    h = h.wrapping_mul(0xBF58476D1CE4E5B9).wrapping_add(y as u64);
    h = h.wrapping_mul(0x94D049BB133111EB).wrapping_add(h >> 31);
    h ^= h >> 32;
    ((h & 0xFFFF) as f32) / 65535.0
}

/// Smoothstep interpolation.
fn smooth(t: f32) -> f32 {
    t * t * (3.0 - 2.0 * t)
}

/// 2D value noise (0..1) at arbitrary coordinates.
fn value_noise(x: f32, y: f32, seed: u64) -> f32 {
    let xi = x.floor();
    let yi = y.floor();
    let xf = x - xi;
    let yf = y - yi;
    let a = hash2(xi as u32, yi as u32, seed);
    let b = hash2(xi as u32 + 1, yi as u32, seed);
    let c = hash2(xi as u32, yi as u32 + 1, seed);
    let d = hash2(xi as u32 + 1, yi as u32 + 1, seed);
    let u = smooth(xf);
    let v = smooth(yf);
    a + (b - a) * u + (c - a) * v + (a - b - c + d) * u * v
}

/// Fractal Brownian motion: a few octaves of value noise.
fn fbm(x: f32, y: f32, seed: u64, octaves: u32) -> f32 {
    let mut total = 0.0;
    let mut amp = 0.5;
    let mut freq = 1.0;
    let mut max = 0.0;
    for _ in 0..octaves {
        total += value_noise(x * freq, y * freq, seed.wrapping_add(0x517CC1B7 * freq as u64)) * amp;
        max += amp;
        amp *= 0.5;
        freq *= 2.0;
    }
    total / max
}

fn lerp_u8(a: u8, b: u8, t: f32) -> u8 {
    (a as f32 + (b as f32 - a as f32) * t.clamp(0.0, 1.0)) as u8
}

fn blend(c0: [u8; 3], c1: [u8; 3], t: f32) -> [u8; 3] {
    [lerp_u8(c0[0], c1[0], t), lerp_u8(c0[1], c1[1], t), lerp_u8(c0[2], c1[2], t)]
}

/// Generate a 16×16 RGBA tile from a palette + pattern.
fn generate_tile(palette: TexturePalette, pattern: TexturePattern, seed: u64) -> Vec<u8> {
    const SIZE: u32 = 16;
    let mut out = vec![0u8; (SIZE * SIZE * 4) as usize];

    for y in 0..SIZE {
        for x in 0..SIZE {
            let fx = x as f32;
            let fy = y as f32;
            let idx = ((y * SIZE + x) * 4) as usize;
            let (rgb, alpha) = match pattern {
                TexturePattern::Noise => {
                    let n = fbm(fx / 16.0 + seed as f32, fy / 16.0, seed, 3);
                    let mut rgb = blend(palette.c0, palette.c1, n);
                    if let Some(c2) = palette.c2 {
                        if fbm(fx * 2.0, fy * 2.0, seed.wrapping_add(7), 2) > 0.75 {
                            rgb = blend(rgb, c2, 0.6);
                        }
                    }
                    (rgb, palette.alpha)
                }
                TexturePattern::Stone => {
                    let n = fbm(fx / 5.0 + 0.31, fy / 5.0, seed, 4);
                    let mut rgb = blend(palette.c0, palette.c1, n);
                    if let Some(c2) = palette.c2 {
                        // Fine cracks: thin high-frequency lines
                        let crack = fbm(fx * 0.9, fy * 0.9, seed.wrapping_add(13), 5);
                        if crack > 0.72 {
                            rgb = blend(rgb, c2, 0.8);
                        }
                    }
                    (rgb, palette.alpha)
                }
                TexturePattern::Speckle => {
                    // Base stone, then blobs of the accent color
                    let base = fbm(fx / 5.0 + 0.31, fy / 5.0, seed, 3);
                    let mut rgb = blend(palette.c0, [100, 102, 108], base);
                    let blob = fbm(fx / 3.0, fy / 3.0, seed.wrapping_add(99), 2);
                    if blob > 0.6 {
                        rgb = blend(rgb, palette.c1, (blob - 0.6) * 2.5);
                        if let Some(c2) = palette.c2 {
                            if blob > 0.82 {
                                rgb = blend(rgb, c2, 0.7);
                            }
                        }
                    }
                    (rgb, palette.alpha)
                }
                TexturePattern::Wood => {
                    // Vertical grain: vary along X with streaks
                    let grain = value_noise(fx * 0.55, fy * 0.22, seed.wrapping_add(5));
                    let streak = fbm(fx / 16.0 + seed as f32, fy * 1.7, seed.wrapping_add(3), 2);
                    let mut rgb = blend(palette.c0, palette.c1, grain * 0.5 + streak * 0.5);
                    if let Some(c2) = palette.c2 {
                        if streak > 0.7 {
                            rgb = blend(rgb, c2, 0.5);
                        }
                    }
                    (rgb, palette.alpha)
                }
                TexturePattern::Rings => {
                    // Concentric rings from the tile centre
                    let dx = fx - 7.5;
                    let dy = fy - 7.5;
                    let dist = (dx * dx + dy * dy).sqrt();
                    let ring = ((dist * 1.15 + fbm(fx * 0.5, fy * 0.5, seed, 2) * 0.6).fract()).abs();
                    let mut rgb = blend(palette.c0, palette.c1, ring);
                    if let Some(c2) = palette.c2 {
                        if ring > 0.85 {
                            rgb = blend(rgb, c2, 0.6);
                        }
                    }
                    (rgb, palette.alpha)
                }
                TexturePattern::Leaves => {
                    // Chaotic clumps with translucent holes
                    let n = fbm(fx * 1.8 + seed as f32 * 0.05, fy * 1.8, seed, 3);
                    if n < 0.16 {
                        // hole — fully transparent
                        (palette.c0, 0)
                    } else {
                        let rgb = blend(palette.c1, palette.c0, n);
                        let a = if n < 0.28 {
                            // edge of a hole: semi-transparent
                            lerp_u8(60, palette.alpha, (n - 0.16) / 0.12)
                        } else {
                            palette.alpha
                        };
                        (rgb, a)
                    }
                }
                TexturePattern::Planks => {
                    // Horizontal boards with dark seams every 4 px
                    let board = (fy / 4.0).floor() as u32;
                    let seam = fy % 4.0 < 0.5;
                    let grain = fbm(fx / 16.0 + board as f32 * 0.1, fy / 16.0, seed.wrapping_add(board as u64), 2);
                    let mut rgb = blend(palette.c0, palette.c1, grain);
                    if seam {
                        if let Some(c2) = palette.c2 {
                            rgb = c2;
                        } else {
                            rgb = blend(rgb, [60; 3], 0.5);
                        }
                    }
                    (rgb, palette.alpha)
                }
                TexturePattern::Water => {
                    let n = fbm(fx / 6.0 + 0.7, fy / 6.0, seed.wrapping_add(21), 3);
                    let rgb = blend(palette.c0, palette.c1, n);
                    (rgb, palette.alpha)
                }
                TexturePattern::Lava => {
                    let n = fbm(fx / 5.0 + 0.9, fy / 5.0, seed.wrapping_add(33), 4);
                    let mut rgb = blend(palette.c0, palette.c1, n);
                    if let Some(c2) = palette.c2 {
                        if n > 0.78 {
                            rgb = blend(rgb, c2, (n - 0.78) * 4.0);
                        }
                    }
                    (rgb, palette.alpha)
                }
            };
            out[idx] = rgb[0];
            out[idx + 1] = rgb[1];
            out[idx + 2] = rgb[2];
            out[idx + 3] = alpha;
        }
    }
    out
}

/* ================================================================ */
/*  AISystem: background training + feedback + checkpoints           */
/* ================================================================ */

/// A single (features, target) sample — used for player feedback and
/// dataset batches.
pub type TrainingSample = ([f32; 8], [f32; 4]);

/// Which of the 4 AI output classes a block maps to (if any).
/// 0 = flower, 1 = fern, 2 = stick, 3 = pebble.
pub fn vegetation_class(block: BlockType) -> Option<usize> {
    use BlockType::*;
    match block {
        Rose | DandelionFlower | TulipRed | TulipPink | TulipWhite | TulipOrange
        | Cornflower | Allium | AzaleaFlower | Flower => Some(0),
        Fern | FernPlant => Some(1),
        Stick | StickSmall => Some(2),
        Pebble1 | Pebble2 | Pebble3 => Some(3),
        _ => None,
    }
}

/// Build an 8-feature vector for a world position using the given climate
/// and terrain info. Kept cheap so the render loop can call it per action.
pub fn features_from_context(
    surface_height: f32,
    temperature: f32,
    humidity: f32,
) -> [f32; 8] {
    let height = (surface_height / 512.0).clamp(0.0, 1.0);
    [
        height,      // terrain_height
        0.25,        // terrain_slope (approx)
        temperature, // biome_temperature
        humidity,    // biome_humidity
        0.5,         // nearby_water_distance
        0.5,         // nearby_vegetation_count
        0.7,         // light_level
        0.5,         // noise_seed_value
    ]
}

/// One-hot target for a vegetation class index.
pub fn one_hot(class: usize) -> [f32; 4] {
    let mut target = [0.0f32; 4];
    if class < 4 {
        target[class] = 1.0;
    }
    target
}

/// AI system that runs in background thread
pub struct AISystem {
    ai: Arc<Mutex<TerrainAI>>,
    tx: Sender<AIMessage>,
    /// Player-action feedback buffer (shared with the render loop).
    feedback: Arc<Mutex<Vec<TrainingSample>>>,
    training_thread: Option<std::thread::JoinHandle<()>>,
    /// Checkpoint file to save/load the model from.
    checkpoint_path: String,
    /// Where online datasets are pulled from (empty = synthetic only).
    dataset_url: String,
}

impl AISystem {
    /// Create new AI system with background training thread.
    ///
    /// A checkpoint at `DEFAULT_CHECKPOINT` is loaded if present; the model
    /// is saved there every 20 epochs.
    pub fn new() -> (Self, Receiver<AIMessage>) {
        Self::new_with_checkpoint(DEFAULT_CHECKPOINT)
    }

    /// Same as [`AISystem::new`] but with an explicit checkpoint path.
    pub fn new_with_checkpoint(checkpoint_path: &str) -> (Self, Receiver<AIMessage>) {
        Self::new_inner(checkpoint_path, true)
    }

    /// Create a fresh AI with no checkpoint loading — used by tests and
    /// benchmark runs so the model always starts identical.
    pub fn new_clean() -> (Self, Receiver<AIMessage>) {
        Self::new_inner("", false)
    }

    fn new_inner(checkpoint_path: &str, load_checkpoint: bool) -> (Self, Receiver<AIMessage>) {
        let ai = if load_checkpoint {
            Arc::new(Mutex::new(
                TerrainAI::load_checkpoint(checkpoint_path).unwrap_or_else(TerrainAI::new),
            ))
        } else {
            Arc::new(Mutex::new(TerrainAI::new()))
        };
        let (tx, rx) = mpsc::channel();
        let feedback: Arc<Mutex<Vec<TrainingSample>>> = Arc::new(Mutex::new(Vec::new()));

        let ai_clone = Arc::clone(&ai);
        let tx_clone = tx.clone();
        let feedback_clone = Arc::clone(&feedback);
        let checkpoint_clone = checkpoint_path.to_string();

        let training_thread = thread::spawn(move || {
            Self::background_training_loop(ai_clone, tx_clone, feedback_clone, &checkpoint_clone);
        });

        let system = Self {
            ai,
            tx,
            feedback,
            training_thread: Some(training_thread),
            checkpoint_path: checkpoint_path.to_string(),
            dataset_url: String::new(),
        };

        (system, rx)
    }

    /// Record a player action as a training sample.
    ///
    /// The render loop calls this whenever the player places or breaks a
    /// block the AI cares about. Samples are drained by the background loop.
    pub fn record_player_action(&self, features: [f32; 8], target: [f32; 4]) {
        if let Ok(mut buf) = self.feedback.lock() {
            // Keep the buffer bounded; newest samples win.
            if buf.len() >= 4096 {
                buf.remove(0);
            }
            buf.push((features, target));
        }
    }

    /// Number of queued player-feedback samples.
    pub fn pending_feedback(&self) -> usize {
        self.feedback.lock().map(|b| b.len()).unwrap_or(0)
    }

    /// Save the current model weights to the checkpoint file immediately.
    pub fn save_checkpoint_now(&self) -> Option<String> {
        if let Ok(model) = self.ai.lock() {
            if model.save_checkpoint(&self.checkpoint_path).is_ok() {
                let _ = self.tx.send(AIMessage::CheckpointSaved {
                    path: self.checkpoint_path.clone(),
                });
                return Some(self.checkpoint_path.clone());
            }
        }
        None
    }

    /// Set the online dataset URL. Empty (default) keeps synthetic data.
    /// When set, the background loop periodically fetches and merges it.
    pub fn set_dataset_url(&mut self, url: impl Into<String>) {
        self.dataset_url = url.into();
    }

    /// Predict the biome class (0..9, BiomeId order) at these climate features.
    ///
    /// Runs the modular biome head of the MeMLP.
    pub fn predict_biome(&self, features: &[f32; 8]) -> usize {
        self.ai.lock().unwrap().predict_biome(features)
    }

    /// Predict the texture-style class (0..5) at these climate features.
    ///
    /// Runs the modular texture head of the MeMLP.
    pub fn predict_texture_style(&self, features: &[f32; 8]) -> usize {
        self.ai.lock().unwrap().predict_texture_style(features)
    }

    /// (total parameters, MeMLP version, training samples) of the live model.
    pub fn model_stats(&self) -> (usize, u32, usize) {
        let model = self.ai.lock().unwrap();
        (
            model.model_param_count(),
            model.model_version(),
            model.training_samples(),
        )
    }

    /// Background training loop: continuously improves model
    fn background_training_loop(
        ai: Arc<Mutex<TerrainAI>>,
        tx: Sender<AIMessage>,
        feedback: Arc<Mutex<Vec<TrainingSample>>>,
        checkpoint_path: &str,
    ) {
        let mut epoch = 0;
        let mut last_dataset_load: std::time::Instant = std::time::Instant::now();

        loop {
            epoch += 1;
            let mut total_loss = 0.0;
            let mut trained = 0usize;

            // 1) Drain player feedback first — human actions are the most
            //    valuable signal.
            let player_samples: Vec<TrainingSample> = feedback
                .lock()
                .map(|mut buf| std::mem::take(&mut *buf))
                .unwrap_or_default();
            for (features, target) in player_samples.iter() {
                if let Ok(mut model) = ai.lock() {
                    total_loss += model.backward(features, *target);
                    trained += 1;
                }
            }
            if !player_samples.is_empty() {
                let avg = total_loss / player_samples.len() as f32;
                let _ = tx.send(AIMessage::PlayerFeedback {
                    samples: player_samples.len(),
                    loss: avg,
                });
            }

            // 2) Online dataset (if configured and due) — refresh every ~60 s.
            if last_dataset_load.elapsed() >= std::time::Duration::from_secs(60) {
                let samples = crate::world::online_trainer::fetch_samples_blocking();
                if samples.len() > 8 {
                    for (features, target) in samples.iter().take(200) {
                        if let Ok(mut model) = ai.lock() {
                            total_loss += model.backward(features, *target);
                            trained += 1;
                        }
                    }
                    let _ = tx.send(AIMessage::OnlineDataset {
                        source: "internet".to_string(),
                        samples: samples.len().min(200),
                    });
                }
                last_dataset_load = std::time::Instant::now();
            }

            // 3) Synthetic samples to keep the vegetation head sharp.
            for i in 0..200 {
                let features = Self::generate_training_sample();
                let target = Self::target_vegetation(&features);
                if let Ok(mut model) = ai.lock() {
                    total_loss += model.backward(&features, target);
                    trained += 1;
                }
                if i % 50 == 0 && epoch <= 3 {
                    println!("[AI-TRAIN] Epoch {}, Sample {}: loss={:.4}", epoch, i, total_loss / trained.max(1) as f32);
                }
            }

            // 3b) Train the modular MeMLP heads (biome + texture) so the
            //     whole stack learns — not just vegetation.
            let mut head_loss = 0.0f32;
            let mut head_count = 0usize;
            for _ in 0..40 {
                let features = Self::generate_training_sample();
                if let Ok(mut model) = ai.lock() {
                    head_loss += model.train_biome(&features, memlp_biome_target(&features));
                    head_loss += model.train_texture(&features, memlp_texture_target(&features));
                    head_count += 2;
                }
            }
            if head_count > 0 {
                let avg = head_loss / head_count as f32;
                let stats = ai.lock().map(|m| (m.model_param_count(), m.model_version())).unwrap_or((0, 0));
                println!(
                    "[AI-MEMLP] Epoch {} | heads: biome+texture | loss={:.4} | params={} | version={}",
                    epoch, avg, stats.0, stats.1
                );
            }

            let avg_loss = if trained > 0 { total_loss / trained as f32 } else { 0.0 };
            let _ = tx.send(AIMessage::TrainingProgress { epoch, loss: avg_loss });

            // 4) Checkpoint every 20 epochs.
            if epoch % 20 == 0 {
                if let Ok(model) = ai.lock() {
                    if let Err(e) = model.save_checkpoint(checkpoint_path) {
                        eprintln!("[AI] checkpoint save failed: {e}");
                    } else {
                        println!("[AI] checkpoint saved ({})", model.training_samples());
                    }
                }
            }

            println!("[AI-EPOCH] {} completed | Avg Loss: {:.4} | samples: {}", epoch, avg_loss, trained);

            // Cooldown per 5 epochs
            if epoch % 5 == 0 {
                std::thread::sleep(std::time::Duration::from_millis(100));
            }
        }
    }

    /// Generate synthetic training data based on realistic terrain patterns
    fn generate_training_sample() -> [f32; 8] {
        use rand::Rng;
        let mut rng = rand::thread_rng();

        [
            rng.gen::<f32>(),           // terrain_height
            rng.gen::<f32>() * 0.5,     // terrain_slope
            rng.gen::<f32>(),           // biome_temperature
            rng.gen::<f32>(),           // biome_humidity
            rng.gen::<f32>(),           // nearby_water_distance
            rng.gen::<f32>(),           // nearby_vegetation_count
            rng.gen::<f32>(),           // light_level
            rng.gen::<f32>(),           // noise_seed_value
        ]
    }

    /// Determine target vegetation based on features (HEURISTIC)
    fn target_vegetation(features: &[f32; 8]) -> [f32; 4] {
        let height = features[0];
        let humidity = features[3];
        let light = features[6];

        // Heuristic rules for fast learning
        let mut probs = [0.0f32; 4];

        // Output 0: Flowers - high humidity + good light
        if humidity > 0.5f32 && light > 0.5f32 {
            probs[0] = (humidity * 0.7f32 + light * 0.3f32).min(1.0f32);
        }

        // Output 1: Ferns - VERY high humidity + low light (shady)
        if humidity > 0.7f32 && light < 0.4f32 {
            probs[1] = (humidity * 0.8f32).min(1.0f32);
        }

        // Output 2: Sticks - DEFAULT most places
        else if probs[0] < 0.3f32 && probs[1] < 0.3f32 {
            probs[2] = 0.6f32;
        }

        // Output 3: Pebbles - low height (valleys)
        if height < 0.2f32 {
            probs[3] = 0.7f32;
        }

        // Normalize to probability distribution
        let sum: f32 = probs.iter().sum();
        if sum > 0.0f32 {
            for p in probs.iter_mut() {
                *p /= sum;
            }
        } else {
            probs[2] = 1.0f32; // Default to sticks
        }

        probs
    }

    /// Get AI prediction for vegetation placement
    pub fn predict_vegetation(&self, features: &[f32; 8]) -> (BlockType, f32) {
        let ai = self.ai.lock().unwrap();
        let probs = ai.forward(features);

        // Find best choice
        let (idx, &confidence) = probs.iter()
            .enumerate()
            .max_by(|(_, a), (_, b)| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
            .unwrap_or((2, &0.0));

        let block = match idx {
            0 => BlockType::Rose,          // flower
            1 => BlockType::Fern,          // fern
            2 => BlockType::StickSmall,    // stick
            3 => BlockType::Pebble1,       // pebble
            _ => BlockType::Air,
        };

        (block, confidence)
    }

    /// Generate texture procedurally using AI guidance
    pub fn generate_texture(&self, seed: u64, width: u32, height: u32) -> Vec<u8> {
        // Default: a generic terrain tile scaled to the requested size.
        let tile = self.generate_block_texture(BlockType::Stone, seed);
        let mut out = Vec::with_capacity((width * height * 4) as usize);
        for y in 0..height {
            for x in 0..width {
                let sx = ((x % 16) * 4) as usize;
                let sy = ((y % 16) * 4) as usize;
                out.extend_from_slice(&tile[sy..sy + 4]);
            }
        }
        // If the request is exactly 16x16, return the tile directly.
        if width == 16 && height == 16 {
            return tile;
        }
        out
    }

    /// Generate a 16×16 tile for a specific block type (CPU, deterministic).
    pub fn generate_block_texture(&self, block: BlockType, seed: u64) -> Vec<u8> {
        self.ai.lock().unwrap().generate_texture_for_block(block, seed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_forward_pass() {
        let ai = TerrainAI::new();
        let features = [0.5, 0.2, 0.6, 0.7, 0.3, 0.4, 0.8, 0.5];
        let output = ai.forward(&features);

        // Check output is valid probability distribution
        let sum: f32 = output.iter().sum();
        assert!((sum - 1.0).abs() < 0.001);

        for &p in &output {
            assert!(p >= 0.0 && p <= 1.0);
        }
    }

    #[test]
    fn test_training() {
        let mut ai = TerrainAI::new();
        let features = [0.5, 0.2, 0.6, 0.7, 0.3, 0.4, 0.8, 0.5];
        let target = [1.0, 0.0, 0.0, 0.0]; // flower

        let loss1 = ai.backward(&features, target);
        let loss2 = ai.backward(&features, target);

        // Loss should decrease
        assert!(loss2 <= loss1 * 1.1); // Allow small variance
    }

    #[test]
    fn test_checkpoint_roundtrip() {
        let ai = TerrainAI::new();
        let path = std::env::temp_dir().join("nv2_ai_checkpoint_test.json");
        ai.save_checkpoint(&path).expect("save checkpoint");

        let loaded = TerrainAI::load_checkpoint(&path).expect("load checkpoint");
        let features = [0.1, 0.4, 0.7, 0.8, 0.2, 0.6, 0.9, 0.3];
        assert_eq!(ai.forward(&features), loaded.forward(&features));
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn test_procedural_texture_deterministic() {
        let ai = TerrainAI::new();
        let a = ai.generate_texture_for_block(BlockType::Grass, 1234);
        let b = ai.generate_texture_for_block(BlockType::Grass, 1234);
        assert_eq!(a, b, "same seed must produce identical tiles");
        assert_eq!(a.len(), 16 * 16 * 4);
    }

    #[test]
    fn test_procedural_texture_differs_by_seed() {
        let ai = TerrainAI::new();
        let a = ai.generate_texture_for_block(BlockType::Stone, 1);
        let b = ai.generate_texture_for_block(BlockType::Stone, 2);
        assert_ne!(a, b, "different seeds must produce different tiles");
    }

    #[test]
    fn test_legacy_checkpoint_migration() {
        // Pre-MeMLP checkpoint layout (flat w1/b1/w2/b2, ndarray serde).
        #[derive(Serialize)]
        struct Legacy {
            w1: Array2<f32>,
            b1: Array1<f32>,
            w2: Array2<f32>,
            b2: Array1<f32>,
            rng_state: u64,
            learning_rate: f32,
            training_samples: usize,
        }
        let legacy = Legacy {
            w1: Array2::ones((8, 16)),
            b1: Array1::zeros(16),
            w2: Array2::ones((16, 4)),
            b2: Array1::zeros(4),
            rng_state: 7,
            learning_rate: 0.01,
            training_samples: 5,
        };
        let path = std::env::temp_dir().join("nv2_legacy_checkpoint_test.json");
        std::fs::write(&path, serde_json::to_string(&legacy).unwrap()).unwrap();

        let ai = TerrainAI::load_checkpoint(&path).expect("legacy checkpoint must load");
        assert_eq!(ai.model_version(), memplp::MEMLP_VERSION);
        assert_eq!(ai.model.vegetation.arch(), &[8, 16, 4]);
        assert_eq!(ai.training_samples(), 5);
        assert_eq!(ai.model.vegetation.param_count(), 8 * 16 + 16 + 16 * 4 + 4);
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn test_real_checkpoint_migrates_if_present() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("checkpoints/ai_model.json");
        if !path.exists() {
            return; // repo without a trained checkpoint — nothing to verify
        }
        let ai = TerrainAI::load_checkpoint(&path).expect("shipped checkpoint must load");
        assert!(ai.model_param_count() > 0, "migrated model must have parameters");
    }

    /// QA benchmark — run with:
    /// `cargo test --release qa_benchmark_report -- --ignored --nocapture`
    ///
    /// Prints MeMLP inference/training throughput + model stats in markdown.
    #[test]
    #[ignore = "benchmark — run explicitly with --release"]
    fn qa_benchmark_report() {
        use std::time::Instant;

        let features = [0.52, 0.25, 0.61, 0.73, 0.30, 0.45, 0.80, 0.55];
        let target = [1.0, 0.0, 0.0, 0.0];

        println!("\n## NV2.0 QA — MeMLP benchmark\n");

        let ai = TerrainAI::load_checkpoint(DEFAULT_CHECKPOINT).unwrap_or_else(TerrainAI::new);
        let checkpoint_bytes =
            std::fs::metadata(DEFAULT_CHECKPOINT).map(|m| m.len()).unwrap_or(0);
        let _ = ai.save_checkpoint(DEFAULT_CHECKPOINT);

        println!("### Model\n");
        println!("| Metric | Value |");
        println!("|---|---|");
        println!("| Architecture | MeMLP (modular embedded Multi-layer Perceptron) |");
        println!("| Checkpoint version | v{} |", ai.model_version());
        println!("| Total parameters | {} |", ai.model_param_count());
        println!("| Vegetation head | 8 → 24 → 16 → 4 (deep MLP) |");
        println!("| Biome head | 8 → 12 → 9 |");
        println!("| Texture head | 8 → 12 → 6 |");
        println!("| Training samples seen | {} |", ai.training_samples());
        println!("| Checkpoint size | {checkpoint_bytes} B |");

        println!("\n### Inference (single-threaded, CPU)\n");
        println!("| Task | Latency | Throughput |");
        println!("|---|---|---|");

        let bench = |name: &str, iters: usize, mut f: Box<dyn FnMut() -> f32>| {
            let _ = f(); // warm-up
            let start = Instant::now();
            let mut sink = 0.0f32;
            for _ in 0..iters {
                sink += f();
            }
            let elapsed = start.elapsed();
            let ns = elapsed.as_nanos() as f64 / iters as f64;
            let per_sec = iters as f64 / elapsed.as_secs_f64();
            println!("| {name} | {ns:.1} ns/iter | {per_sec:.1} iter/s |");
            sink
        };

        let _ = bench("Vegetation head forward", 50_000, Box::new(|| {
            ai.forward(&features).iter().sum::<f32>()
        }));
        let _ = bench("Biome head forward", 50_000, Box::new(|| {
            ai.predict_biome(&features) as f32
        }));
        let _ = bench("Texture head forward", 50_000, Box::new(|| {
            ai.predict_texture_style(&features) as f32
        }));
        let _ = bench("16×16 procedural texture", 2_000, Box::new(|| {
            ai.generate_texture_for_block(BlockType::Grass, 12_345)[0] as f32
        }));

        println!("\n### Training (single-threaded, CPU)\n");
        println!("| Task | Throughput |");
        println!("|---|---|");

        let train_iters = 20_000;
        let mut model = TerrainAI::new();
        let mut loss = 0.0f32;

        let start = Instant::now();
        for i in 0..train_iters {
            let mut f = features;
            f[0] = (i as f32 / train_iters as f32).fract();
            loss += model.backward(&f, target);
        }
        let el = start.elapsed();
        println!("| Vegetation head | {:.1} samples/s |", train_iters as f64 / el.as_secs_f64());

        let start = Instant::now();
        for i in 0..train_iters {
            let mut f = features;
            f[2] = (i as f32 / train_iters as f32).fract();
            loss += model.train_biome(&f, i % 9);
        }
        let el = start.elapsed();
        println!("| Biome head | {:.1} samples/s |", train_iters as f64 / el.as_secs_f64());

        let start = Instant::now();
        for i in 0..train_iters {
            let mut f = features;
            f[3] = (i as f32 / train_iters as f32).fract();
            loss += model.train_texture(&f, i % 6);
        }
        let el = start.elapsed();
        println!("| Texture head | {:.1} samples/s |", train_iters as f64 / el.as_secs_f64());

        let (system, _rx) = AISystem::new_clean();
        system.record_player_action(features, target);
        let stats = system.model_stats();
        println!("\n### AISystem\n");
        println!("| Metric | Value |");
        println!("|---|---|");
        println!("| Pending player-feedback samples | {} |", system.pending_feedback());
        println!(
            "| Model stats | params={}, version={}, samples={} |",
            stats.0, stats.1, stats.2
        );
        println!("\nFinal loss sink: {loss:.6}\n");
    }

    #[test]
    fn test_player_feedback_buffer() {
        let (system, _rx) = AISystem::new_with_checkpoint(
            &std::env::temp_dir().join("nv2_ai_feedback_test.json").to_string_lossy(),
        );
        assert_eq!(system.pending_feedback(), 0);
        system.record_player_action([0.5; 8], [1.0, 0.0, 0.0, 0.0]);
        assert_eq!(system.pending_feedback(), 1);
        system.save_checkpoint_now();
    }
}
