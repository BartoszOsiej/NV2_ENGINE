/// Liquid simulation system — written from scratch.
///
/// ## Encoding (water_meta byte)
/// ┌──────────────────────────────────────────────────────────────────────┐
/// │  0  → empty (no liquid data)                                        │
/// │  1–7 → flowing water; level decreases with distance from source     │
/// │  8  → permanent source block (never consumed or drained)            │
/// └──────────────────────────────────────────────────────────────────────┘
///
/// ## Flow rules (one step per simulate call, throttled by caller)
/// 1. Active cells: water blocks that have at least one air neighbour.
/// 2. Gravity-first:   if the cell below is air → place level-7 water there.
/// 3. Lateral spread: if no downward path, spread to air cells at (level−1)
///    in the four cardinal directions.
/// 4. Source blocks (level 8) are never removed; they always stay in place.
/// 5. A hard cap of MAX_CHANGES_PER_STEP limits total block changes per tick
///    to prevent lag during initial world flooding.

use std::collections::{HashMap, HashSet};
use super::World;
use super::block::BlockType;
use super::chunk::{CHUNK_W, CHUNK_D, CHUNK_H};

/// Y-level below which terrain is flooded by the terrain generator.
pub const SEA_LEVEL: i32 = super::biomes::SEA_LEVEL as i32;

/// Level of a freshly placed source block.
pub const SOURCE_LEVEL: u8 = 8;

/// Maximum flow level a non-source block may have.
pub const FLOW_MAX: u8 = 7;

/// Maximum independent block changes applied in one simulation step.
/// Keeps the tick time bounded even with large exposed water bodies.
const MAX_CHANGES_PER_STEP: usize = 512;

// ──────────────────────────────────────────────────────────────────────────────
//  Public API
// ──────────────────────────────────────────────────────────────────────────────

/// Decode the raw water_meta byte into a level (0 = dry, 1-7 = flow, 8 = source).
#[inline]
pub fn decode_level(meta: u8) -> u8 {
    if meta == 0 { 0 } else { meta.min(SOURCE_LEVEL) }
}

/// Encode a level back to a water_meta byte.
#[inline]
pub fn encode_level(level: u8) -> u8 {
    level.min(SOURCE_LEVEL)
}

/// Perform one liquid simulation tick.
///
/// Called by `World::simulate_water()` at a throttled rate (≈0.5 s).
pub fn simulate_step(world: &mut World) {
    // ── Phase 1: collect active liquid cells ─────────────────────────────────
    // A cell is "active" when it has at least one directly adjacent air block,
    // OR when it is below the source level (so slightly-too-low flow blocks can
    // receive a refresh when a source nearby brings them up).
    let chunk_keys: Vec<(i32, i32)> = world.chunks.keys().cloned().collect();

    let mut active: Vec<(i32, i32, i32, u8)> = Vec::new(); // (wx, wy, wz, level)

    for (cx, cz) in &chunk_keys {
        let chunk = match world.chunks.get(&(*cx, *cz)) {
            Some(c) => c,
            None    => continue,
        };
        for lx in 0..CHUNK_W {
            for lz in 0..CHUNK_D {
                for ly in (0..CHUNK_H).rev() {               // top → bottom order
                    if *chunk.get(lx, ly, lz) != BlockType::Water { continue; }

                    let wx = cx * CHUNK_W as i32 + lx as i32;
                    let wy = ly as i32;
                    let wz = cz * CHUNK_D as i32 + lz as i32;

                    // Only take cells adjacent to air (active frontier)
                    let has_air_nb =
                        (wy > 0 && world.get_block(wx, wy - 1, wz) == BlockType::Air)
                        || world.get_block(wx + 1, wy, wz) == BlockType::Air
                        || world.get_block(wx - 1, wy, wz) == BlockType::Air
                        || world.get_block(wx, wy, wz + 1) == BlockType::Air
                        || world.get_block(wx, wy, wz - 1) == BlockType::Air;

                    if !has_air_nb { continue; }

                    let raw   = chunk.water_meta_get(lx, ly, lz);
                    // Legacy chunks may have 0x0F (old encoding) → treat as source
                    let level = if raw == 0 || raw > SOURCE_LEVEL { SOURCE_LEVEL } else { raw };
                    active.push((wx, wy, wz, level));
                }
            }
        }
    }

    // Sort highest-Y first so gravity propagates before lateral spread
    active.sort_by_key(|(_, y, _, _)| std::cmp::Reverse(*y));

    // ── Phase 2: compute changes ──────────────────────────────────────────────
    // `spawned` tracks positions that already received a write this tick so we
    // don't double-write.
    let mut spawned: HashSet<(i32, i32, i32)> = HashSet::new();
    let mut changes: Vec<(i32, i32, i32, u8)> = Vec::new(); // (wx, wy, wz, new_level)

    for &(wx, wy, wz, level) in &active {
        if changes.len() >= MAX_CHANGES_PER_STEP { break; }

        // Source blocks always confirm their own level (prevents erosion)
        if level == SOURCE_LEVEL {
            let key = (wx, wy, wz);
            if !spawned.contains(&key) {
                changes.push((wx, wy, wz, SOURCE_LEVEL));
                spawned.insert(key);
            }
        }

        // ── Gravity: try to fill the cell directly below ──────────────────
        if wy > 0 {
            let below = world.get_block(wx, wy - 1, wz);
            let key   = (wx, wy - 1, wz);
            if below == BlockType::Air && !spawned.contains(&key) {
                changes.push((wx, wy - 1, wz, FLOW_MAX));
                spawned.insert(key);
                // Gravity has priority — skip lateral spread this tick
                continue;
            }
        }

        // ── Lateral spread (only for levels > 1) ─────────────────────────
        if level <= 1 { continue; }
        let spread = level - 1;

        const DIRS: [(i32, i32); 4] = [(1, 0), (-1, 0), (0, 1), (0, -1)];
        for &(dx, dz) in &DIRS {
            if changes.len() >= MAX_CHANGES_PER_STEP { break; }
            let n  = (wx + dx, wy, wz + dz);
            if spawned.contains(&n) { continue; }
            if world.get_block(wx + dx, wy, wz + dz) == BlockType::Air {
                changes.push((wx + dx, wy, wz + dz, spread));
                spawned.insert(n);
            }
        }
    }

    // ── Phase 3: apply changes ────────────────────────────────────────────────
    for (wx, wy, wz, level) in changes {
        if level > 0 {
            world.set_block(wx, wy, wz, BlockType::Water);
            world.set_water_meta(wx, wy, wz, encode_level(level));
        }
    }
}

// ──────────────────────────────────────────────────────────────────────────────
//  Helper: place a water source block at world coordinates
// ──────────────────────────────────────────────────────────────────────────────

/// Place a permanent water source at (wx, wy, wz).
pub fn place_source(world: &mut World, wx: i32, wy: i32, wz: i32) {
    world.set_block(wx, wy, wz, BlockType::Water);
    world.set_water_meta(wx, wy, wz, encode_level(SOURCE_LEVEL));
}

/// Remove liquid at (wx, wy, wz) (player picks up water with a bucket, etc.)
pub fn remove_liquid(world: &mut World, wx: i32, wy: i32, wz: i32) {
    world.set_block(wx, wy, wz, BlockType::Air);
    world.set_water_meta(wx, wy, wz, 0);
}

/// Return the liquid level at world coordinates (0 = no liquid).
pub fn level_at(world: &World, wx: i32, wy: i32, wz: i32) -> u8 {
    let raw = world.get_water_meta(wx, wy, wz);
    decode_level(raw)
}

/// Return true if the block at (wx, wy, wz) is a permanent source block.
pub fn is_source(world: &World, wx: i32, wy: i32, wz: i32) -> bool {
    world.get_block(wx, wy, wz) == BlockType::Water
        && decode_level(world.get_water_meta(wx, wy, wz)) == SOURCE_LEVEL
}
