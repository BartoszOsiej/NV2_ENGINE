use crate::world::{
    self,
    biomes::Biome,
    chunk::CHUNK_W,
    World,
};

pub const LOCATE_SEARCH_RADIUS: i32 = 2048;

const CHUNK_SAMPLE_OFFSETS: [(i32, i32); 9] = [
    (8, 8),
    (4, 4),
    (12, 4),
    (4, 12),
    (12, 12),
    (8, 4),
    (4, 8),
    (12, 8),
    (8, 12),
];

#[derive(Debug)]
pub struct CommandOutput {
    pub message: String,
    pub teleport_target: Option<(f32, f32, f32)>,
}

#[derive(Clone, Copy)]
struct BiomeQuery {
    description: &'static str,
    matches: fn(Biome) -> bool,
}

#[derive(Clone, Copy)]
struct LocateHit {
    wx: i32,
    wy: i32,
    wz: i32,
    matched_biome: Biome,
}

pub fn execute(world: &mut World, player_origin: (f32, f32, f32), raw: &str) -> Result<CommandOutput, String> {
    let command = raw.trim();
    if command.is_empty() {
        return Err(String::from("Empty command."));
    }

    let command = command.strip_prefix('/').unwrap_or(command);
    let parts: Vec<&str> = command.split_whitespace().collect();
    if parts.is_empty() {
        return Err(String::from("Empty command."));
    }

    match parts[0] {
        "locate" => execute_locate(world, player_origin, &parts),
        "tp" => execute_tp(world, &parts),
        other => Err(format!("Unknown command '/{}'. Supported commands: /locate, /tp.", other)),
    }
}

fn execute_locate(world: &mut World, player_origin: (f32, f32, f32), parts: &[&str]) -> Result<CommandOutput, String> {
    if parts.len() < 2 {
        return Err(String::from(
            "Usage: /locate <plains|forest|darkforest|mountains|swamp|taiga|desert|ocean|coast|beach|river|lake> [--tp]",
        ));
    }

    let requested_name = parts[1].trim().to_ascii_lowercase();
    let query = resolve_biome_query(parts[1]).ok_or_else(|| {
        format!(
            "Unsupported biome '{}'. Supported biomes: plains, forest, darkforest, mountains, swamp, taiga, desert, ocean, coast, beach, river, lake.",
            parts[1]
        )
    })?;

    let mut teleport_after_locate = false;
    for arg in &parts[2..] {
        if *arg == "--tp" {
            teleport_after_locate = true;
        } else {
            return Err(format!("Unknown flag '{}'. Only '--tp' is supported for /locate.", arg));
        }
    }

    let hit = locate_biome(
        world,
        player_origin.0.floor() as i32,
        player_origin.2.floor() as i32,
        query,
        LOCATE_SEARCH_RADIUS,
    )
    .ok_or_else(|| {
        format!(
            "No {} biome was found within {} blocks.",
            requested_name,
            LOCATE_SEARCH_RADIUS
        )
    })?;

    let base_message = format!(
        "Located {} at ({}, {}, {}) using {} sampling; matched {}.",
        requested_name,
        hit.wx,
        hit.wy,
        hit.wz,
        query.description,
        biome_label(hit.matched_biome)
    );

    if teleport_after_locate {
        let teleport_target = world
            .safe_teleport_position(hit.wx, hit.wy, hit.wz)
            .map_err(|err| err.to_string())?;
        return Ok(CommandOutput {
            message: format!(
                "{} Teleported to ({:.1}, {:.1}, {:.1}).",
                base_message,
                teleport_target.0,
                teleport_target.1,
                teleport_target.2
            ),
            teleport_target: Some(teleport_target),
        });
    }

    Ok(CommandOutput {
        message: base_message,
        teleport_target: None,
    })
}

fn execute_tp(world: &mut World, parts: &[&str]) -> Result<CommandOutput, String> {
    if parts.len() != 4 {
        return Err(String::from("Usage: /tp <x> <y> <z>"));
    }

    let x = parse_i32(parts[1], "x")?;
    let y = parse_i32(parts[2], "y")?;
    let z = parse_i32(parts[3], "z")?;
    let teleport_target = world
        .safe_teleport_position(x, y, z)
        .map_err(|err| err.to_string())?;

    Ok(CommandOutput {
        message: format!(
            "Teleported to ({:.1}, {:.1}, {:.1}).",
            teleport_target.0,
            teleport_target.1,
            teleport_target.2
        ),
        teleport_target: Some(teleport_target),
    })
}

fn parse_i32(text: &str, axis: &str) -> Result<i32, String> {
    text.parse::<i32>()
        .map_err(|_| format!("Invalid {} coordinate '{}'. Expected an integer.", axis, text))
}

fn locate_biome(world: &World, origin_x: i32, origin_z: i32, query: BiomeQuery, radius: i32) -> Option<LocateHit> {
    let origin_cx = origin_x.div_euclid(CHUNK_W as i32);
    let origin_cz = origin_z.div_euclid(CHUNK_W as i32);
    let max_ring = radius.div_euclid(CHUNK_W as i32);

    for ring in 0..=max_ring {
        let mut best_hit: Option<(i64, LocateHit)> = None;
        for (cx, cz) in chunk_ring(origin_cx, origin_cz, ring) {
            if let Some(hit) = scan_chunk_for_biome(world, cx, cz, query, origin_x, origin_z, radius) {
                let dx = hit.wx - origin_x;
                let dz = hit.wz - origin_z;
                let distance_sq = i64::from(dx) * i64::from(dx) + i64::from(dz) * i64::from(dz);
                let replace = match best_hit {
                    Some((best_distance_sq, _)) => distance_sq < best_distance_sq,
                    None => true,
                };
                if replace {
                    best_hit = Some((distance_sq, hit));
                }
            }
        }

        if let Some((_, hit)) = best_hit {
            return Some(hit);
        }
    }

    None
}

fn scan_chunk_for_biome(
    world: &World,
    cx: i32,
    cz: i32,
    query: BiomeQuery,
    origin_x: i32,
    origin_z: i32,
    radius: i32,
) -> Option<LocateHit> {
    let chunk_origin_x = cx * CHUNK_W as i32;
    let chunk_origin_z = cz * CHUNK_W as i32;
    let radius_sq = i64::from(radius) * i64::from(radius);

    for &(offset_x, offset_z) in &CHUNK_SAMPLE_OFFSETS {
        let wx = chunk_origin_x + offset_x;
        let wz = chunk_origin_z + offset_z;
        let dx = wx - origin_x;
        let dz = wz - origin_z;
        let distance_sq = i64::from(dx) * i64::from(dx) + i64::from(dz) * i64::from(dz);
        if distance_sq > radius_sq {
            continue;
        }

        let biome = world.biome_at(wx, wz);
        if !(query.matches)(biome) {
            continue;
        }

        let wy = world.column_top_height(wx, wz) as i32 + 1;
        return Some(LocateHit {
            wx,
            wy,
            wz,
            matched_biome: biome,
        });
    }

    None
}

fn chunk_ring(center_x: i32, center_z: i32, ring: i32) -> Vec<(i32, i32)> {
    if ring == 0 {
        return vec![(center_x, center_z)];
    }

    let mut coords = Vec::with_capacity((ring * 8) as usize);
    for dx in -ring..=ring {
        coords.push((center_x + dx, center_z - ring));
        coords.push((center_x + dx, center_z + ring));
    }
    for dz in (-ring + 1)..ring {
        coords.push((center_x - ring, center_z + dz));
        coords.push((center_x + ring, center_z + dz));
    }
    coords
}

fn resolve_biome_query(name: &str) -> Option<BiomeQuery> {
    let normalized = name.trim().to_ascii_lowercase();
    match normalized.as_str() {
        "forest" => Some(BiomeQuery {
            description: "temperate-forest biome",
            matches: |biome| matches!(biome, Biome::Forest | Biome::DarkForest),
        }),
        "plains" | "meadow" => Some(BiomeQuery {
            description: "open-grassland biome",
            matches: |biome| matches!(biome, Biome::Plains),
        }),
        "darkforest" | "rainwood" => Some(BiomeQuery {
            description: "dense-forest biome",
            matches: |biome| matches!(biome, Biome::DarkForest),
        }),
        "mountains" | "highland" => Some(BiomeQuery {
            description: "mountain biome",
            matches: |biome| matches!(biome, Biome::Mountains),
        }),
        "swamp" | "wetland" => Some(BiomeQuery {
            description: "wetland biome",
            matches: |biome| matches!(biome, Biome::Swamp),
        }),
        "taiga" | "tundra" => Some(BiomeQuery {
            description: "cold-forest biome",
            matches: |biome| matches!(biome, Biome::Taiga),
        }),
        "desert" => Some(BiomeQuery {
            description: "dry-biome",
            matches: |biome| matches!(biome, Biome::Desert),
        }),
        "ocean" => Some(BiomeQuery {
            description: "open-water alias",
            matches: |biome| matches!(biome, Biome::Ocean),
        }),
        "coast" | "beach" => Some(BiomeQuery {
            description: "shoreline alias",
            matches: |biome| matches!(biome, Biome::Coast),
        }),
        "river" => Some(BiomeQuery {
            description: "wet lowland alias",
            matches: |biome| matches!(biome, Biome::Swamp | Biome::Coast),
        }),
        "lake" => Some(BiomeQuery {
            description: "still-water alias",
            matches: |biome| matches!(biome, Biome::Swamp | Biome::Ocean | Biome::Coast),
        }),
        _ => None,
    }
}

fn biome_label(biome: Biome) -> &'static str {
    match biome {
        Biome::Ocean => "ocean",
        Biome::Coast => "coast",
        Biome::Plains => "plains",
        Biome::Forest => "forest",
        Biome::DarkForest => "dark_forest",
        Biome::Swamp => "swamp",
        Biome::Taiga => "taiga",
        Biome::Desert => "desert",
        Biome::Mountains => "mountains",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn supports_required_biome_names() {
        for name in [
            "forest",
            "plains",
            "meadow",
            "darkforest",
            "mountains",
            "highland",
            "swamp",
            "wetland",
            "rainwood",
            "taiga",
            "desert",
            "tundra",
            "ocean",
            "coast",
            "beach",
            "river",
            "lake",
        ] {
            assert!(resolve_biome_query(name).is_some(), "missing biome alias: {}", name);
        }
    }

    #[test]
    fn tp_requires_three_coordinates() {
        let mut world = World::new(1337);
        let error = execute(&mut world, (0.0, 80.0, 0.0), "/tp 100 80").unwrap_err();
        assert!(error.contains("Usage: /tp"));
    }

    #[test]
    fn executes_requested_command_flow() {
        let candidate_seeds = [1337_u32, 7_654_321, 42_4242, 0x1bad_b002, 0xdec0_de01];

        for seed in candidate_seeds {
            let mut world = World::new(seed);
            let origin = world.find_spawn_point();

            let locate_forest = execute(&mut world, origin, "/locate forest");
            let locate_highland = execute(&mut world, origin, "/locate highland --tp");
            if locate_forest.is_err() || locate_highland.is_err() {
                continue;
            }

            let locate_forest = locate_forest.unwrap();
            let locate_highland = locate_highland.unwrap();
            let tp = execute(&mut world, origin, "/tp 100 80 -200").unwrap();

            assert!(locate_forest.message.contains("Located forest"));
            assert!(locate_highland.message.contains("Located highland"));
            assert!(locate_highland.teleport_target.is_some());
            assert!(tp.message.contains("Teleported to"));
            assert!(tp.teleport_target.is_some());
            return;
        }

        panic!("No deterministic seed in the test set satisfied the locate forest/highland command flow.");
    }
}