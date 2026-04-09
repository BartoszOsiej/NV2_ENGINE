/// Liquid simulation system — written from scratch.
///
/// ## Encoding (water_meta byte)
/// ┌──────────────────────────────────────────────────────────────────────┐
/// │  0  → no dynamic liquid data; generated/static terrain water stays   │
/// │       at meta=0 and is ignored by the solver                         │
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
const MAX_CHANGES_PER_STEP: usize = 2048;

// ──────────────────────────────────────────────────────────────────────────────
//  Public API
// ──────────────────────────────────────────────────────────────────────────────

/// Decode the raw water_meta byte into a dynamic liquid level
/// (0 = no dynamic state, 1-7 = flow, 8 = source).
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
///
/// Algorithm:
///   1. Collect all water blocks in loaded chunks (scan up to SEA_LEVEL+60).
///   2. For each source block (level 8) propagate gravity + lateral spread.
///   3. For each non-source flow block, compute the *highest level reachable
///      from any horizontal/vertical neighbour* and either raise, keep, or remove
///      the block accordingly. This ensures flows decay when sources disappear.
pub fn simulate_step(world: &mut World) {
    let chunk_keys: Vec<(i32, i32)> = world.chunks.keys().cloned().collect();
    let scan_top = CHUNK_H.min(SEA_LEVEL as usize + 65);

    // ── Phase 1: snapshot all water cells ────────────────────────────────────
    // We read the whole liquid frontier into a flat list before mutating anything.
    let mut all_water: Vec<(i32, i32, i32, u8)> = Vec::new();

    for (cx, cz) in &chunk_keys {
        let chunk = match world.chunks.get(&(*cx, *cz)) {
            Some(c) => c,
            None    => continue,
        };
        for lx in 0..CHUNK_W {
            for lz in 0..CHUNK_D {
                for ly in 0..scan_top {
                    if *chunk.get(lx, ly, lz) != BlockType::Water { continue; }
                    let raw = chunk.water_meta_get(lx, ly, lz);
                    // Generated terrain water is static world water (meta=0).
                    // Only explicit liquid-sim cells participate in dynamic flow.
                    if raw == 0 { continue; }
                    let wx = cx * CHUNK_W as i32 + lx as i32;
                    let wy = ly as i32;
                    let wz = cz * CHUNK_D as i32 + lz as i32;
                    let level = if raw == 0 || raw > SOURCE_LEVEL { SOURCE_LEVEL } else { raw };
                    all_water.push((wx, wy, wz, level));
                }
            }
        }
    }

    // Sort highest-Y first so gravity naturally chains downward in one pass.
    all_water.sort_by_key(|(_, y, _, _)| std::cmp::Reverse(*y));

    // Build a fast lookup: position → current level
    let mut level_map: HashMap<(i32, i32, i32), u8> =
        all_water.iter().map(|&(x, y, z, l)| ((x, y, z), l)).collect();

    // ── Phase 2: propagation + decay ─────────────────────────────────────────
    let mut changes: Vec<(i32, i32, i32, u8)> = Vec::new(); // (wx,wy,wz, new_level 0=remove)
    let mut written: HashSet<(i32, i32, i32)> = HashSet::new();

    const DIRS: [(i32, i32); 4] = [(1, 0), (-1, 0), (0, 1), (0, -1)];

    for &(wx, wy, wz, level) in &all_water {
        if changes.len() >= MAX_CHANGES_PER_STEP { break; }

        // ── Sources always refresh themselves and spread aggressively ─────
        if level == SOURCE_LEVEL {
            // Confirm the source is still solid (not removed this tick)
            let key = (wx, wy, wz);
            if !written.contains(&key) {
                changes.push((wx, wy, wz, SOURCE_LEVEL));
                written.insert(key);
            }

            // Gravity spread: fill empty cell directly below at FLOW_MAX
            if wy > 0 {
                let bk = (wx, wy - 1, wz);
                let below = world.get_block(wx, wy - 1, wz);
                let cur   = level_map.get(&bk).cloned().unwrap_or(0);
                if (below == BlockType::Air || (below == BlockType::Water && cur < FLOW_MAX))
                    && !written.contains(&bk)
                    && changes.len() < MAX_CHANGES_PER_STEP
                {
                    changes.push((wx, wy - 1, wz, FLOW_MAX));
                    written.insert(bk);
                    level_map.insert(bk, FLOW_MAX);
                    continue; // gravity beats lateral spread
                }
            }

            // Lateral spread from source at FLOW_MAX - 1 (=6) to adjacent air/lower water
            let spread = FLOW_MAX - 1;
            for &(dx, dz) in &DIRS {
                if changes.len() >= MAX_CHANGES_PER_STEP { break; }
                let nk  = (wx + dx, wy, wz + dz);
                let nb  = world.get_block(wx + dx, wy, wz + dz);
                let cur = level_map.get(&nk).cloned().unwrap_or(0);
                if (nb == BlockType::Air || (nb == BlockType::Water && cur < spread))
                    && !written.contains(&nk)
                {
                    changes.push((wx + dx, wy, wz + dz, spread));
                    written.insert(nk);
                    level_map.insert(nk, spread);
                }
            }
            continue;
        }

        // ── Flowing (non-source) block: propagate and decay ──────────────
        // Downward gravity takes absolute priority.
        if wy > 0 {
            let bk = (wx, wy - 1, wz);
            let below = world.get_block(wx, wy - 1, wz);
            let cur   = level_map.get(&bk).cloned().unwrap_or(0);
            if (below == BlockType::Air || (below == BlockType::Water && cur < FLOW_MAX))
                && !written.contains(&bk)
                && changes.len() < MAX_CHANGES_PER_STEP
            {
                changes.push((wx, wy - 1, wz, FLOW_MAX));
                written.insert(bk);
                level_map.insert(bk, FLOW_MAX);
                // Don't continue here — still do lateral spread from this cell
            }
        }

        // Lateral spread: only if level > 1
        if level <= 1 { continue; }
        let spread = level - 1;
        for &(dx, dz) in &DIRS {
            if changes.len() >= MAX_CHANGES_PER_STEP { break; }
            let nk  = (wx + dx, wy, wz + dz);
            let nb  = world.get_block(wx + dx, wy, wz + dz);
            let cur = level_map.get(&nk).cloned().unwrap_or(0);
            if (nb == BlockType::Air || (nb == BlockType::Water && cur < spread))
                && !written.contains(&nk)
            {
                changes.push((wx + dx, wy, wz + dz, spread));
                written.insert(nk);
                level_map.insert(nk, spread);
            }
        }
    }

    // ── Phase 3: decay pass — remove or lower blocks that lost their source ──
    // For every flowing (non-source) water block that was NOT refreshed this tick,
    // check if any neighbour can still supply it at the recorded level. If not,
    // lower it by 1 (or remove it when it reaches 0).
    if changes.len() < MAX_CHANGES_PER_STEP {
        for &(wx, wy, wz, level) in &all_water {
            if level == SOURCE_LEVEL { continue; } // sources never decay
            if written.contains(&(wx, wy, wz)) { continue; } // already updated

            // Compute the best incoming level from all 6 neighbours
            let mut best_incoming: u8 = 0;

            // From above: a block directly above at any level feeds level 7 down
            if world.get_block(wx, wy + 1, wz) == BlockType::Water {
                let above_lvl = level_map.get(&(wx, wy + 1, wz)).cloned().unwrap_or(0);
                if above_lvl > 0 {
                    best_incoming = FLOW_MAX; // gravity-fed = always full
                }
            }

            // From horizontal neighbours
            if best_incoming < level {
                for &(dx, dz) in &DIRS {
                    let nk = (wx + dx, wy, wz + dz);
                    let nb = world.get_block(wx + dx, wy, wz + dz);
                    if nb == BlockType::Water {
                        let nlvl = level_map.get(&nk).cloned().unwrap_or(0);
                        if nlvl > 1 {
                            let can_give = nlvl - 1;
                            if can_give > best_incoming { best_incoming = can_give; }
                        }
                    }
                }
            }

            if changes.len() >= MAX_CHANGES_PER_STEP { break; }

            if best_incoming < level {
                // This block should decay
                let new_level = if level <= 1 || best_incoming == 0 { 0 } else { best_incoming };
                changes.push((wx, wy, wz, new_level));
                written.insert((wx, wy, wz));
            }
        }
    }

    // ── Phase 4: apply all changes ────────────────────────────────────────────
    for (wx, wy, wz, level) in changes {
        if level == 0 {
            world.set_block(wx, wy, wz, BlockType::Air);
            world.set_water_meta(wx, wy, wz, 0);
        } else {
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

/// Return the dynamic liquid level at world coordinates (0 = static or dry).
pub fn level_at(world: &World, wx: i32, wy: i32, wz: i32) -> u8 {
    let raw = world.get_water_meta(wx, wy, wz);
    decode_level(raw)
}

/// Return true if the block at (wx, wy, wz) is a permanent source block.
pub fn is_source(world: &World, wx: i32, wy: i32, wz: i32) -> bool {
    world.get_block(wx, wy, wz) == BlockType::Water
        && decode_level(world.get_water_meta(wx, wy, wz)) == SOURCE_LEVEL
}
