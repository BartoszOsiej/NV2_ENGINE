use crate::world::{World, chunk::{CHUNK_W, CHUNK_H, CHUNK_D}};
use crate::world::block::BlockType;
use crate::assets::BlockModelLoader;
use super::texture_atlas::{self, TileUV};
use super::vertices::Vertex;

#[derive(Clone, Copy)]
struct ColumnVisuals {
    biome_tint: [f32; 3],
    surface_data: [f32; 4],
}

#[derive(Clone, Copy)]
struct BlockAppearance {
    face_uvs: [TileUV; 6],
    foliage_tint: f32,
}

// Scan the full chunk height so high rivers, lakes, or later terrain features
// never disappear above an arbitrary cutoff.
const WATER_SCAN_TOP: usize = CHUNK_H;

const FACE_NORMALS: [[f32; 3]; 6] = [
    [ 0.0,  1.0,  0.0], // top
    [ 0.0, -1.0,  0.0], // bottom
    [ 0.0,  0.0,  1.0], // south  (+Z)
    [ 0.0,  0.0, -1.0], // north  (-Z)
    [ 1.0,  0.0,  0.0], // east   (+X)
    [-1.0,  0.0,  0.0], // west   (-X)
];

const FACE_OFFSETS: [[i32; 3]; 6] = [
    [ 0,  1,  0],
    [ 0, -1,  0],
    [ 0,  0,  1],
    [ 0,  0, -1],
    [ 1,  0,  0],
    [-1,  0,  0],
];

// Counter-clockwise quad vertices (position offsets within a unit cube).
const FACE_QUADS: [[[f32; 3]; 4]; 6] = [
    [[0.0,1.0,0.0],[1.0,1.0,0.0],[1.0,1.0,1.0],[0.0,1.0,1.0]],
    [[0.0,0.0,1.0],[1.0,0.0,1.0],[1.0,0.0,0.0],[0.0,0.0,0.0]],
    [[0.0,0.0,1.0],[1.0,0.0,1.0],[1.0,1.0,1.0],[0.0,1.0,1.0]],
    [[1.0,0.0,0.0],[0.0,0.0,0.0],[0.0,1.0,0.0],[1.0,1.0,0.0]],
    [[1.0,0.0,1.0],[1.0,0.0,0.0],[1.0,1.0,0.0],[1.0,1.0,1.0]],
    [[0.0,0.0,0.0],[0.0,0.0,1.0],[0.0,1.0,1.0],[0.0,1.0,0.0]],
];

// Face brightness multipliers (directional lighting, pre-AO).
const SOLID_BRIGHTNESS: [f32; 6] = [1.00, 0.50, 0.80, 0.80, 0.65, 0.65];
const WATER_BRIGHTNESS: [f32; 6] = [0.95, 0.50, 0.70, 0.70, 0.60, 0.60];

// ── Ambient Occlusion sample offsets ─────────────────────────────────────────
// For each face (6) × vertex (4) → [side1 offset, side2 offset, corner offset]
// Derived from FACE_QUADS vertex positions and face normals.
// All offsets are relative to the block being meshed.
const AO_SAMPLES: [[[[i32; 3]; 3]; 4]; 6] = [
    // Face 0 — top (+Y)  quad [[0,1,0],[1,1,0],[1,1,1],[0,1,1]]
    [[[-1,1, 0],[0,1,-1],[-1,1,-1]],
     [[ 1,1, 0],[0,1,-1],[ 1,1,-1]],
     [[ 1,1, 0],[0,1, 1],[ 1,1, 1]],
     [[-1,1, 0],[0,1, 1],[-1,1, 1]]],
    // Face 1 — bottom (-Y)  quad [[0,0,1],[1,0,1],[1,0,0],[0,0,0]]
    [[[-1,-1, 0],[0,-1, 1],[-1,-1, 1]],
     [[ 1,-1, 0],[0,-1, 1],[ 1,-1, 1]],
     [[ 1,-1, 0],[0,-1,-1],[ 1,-1,-1]],
     [[-1,-1, 0],[0,-1,-1],[-1,-1,-1]]],
    // Face 2 — south (+Z)  quad [[0,0,1],[1,0,1],[1,1,1],[0,1,1]]
    [[[-1, 0,1],[0,-1,1],[-1,-1,1]],
     [[ 1, 0,1],[0,-1,1],[ 1,-1,1]],
     [[ 1, 0,1],[0, 1,1],[ 1, 1,1]],
     [[-1, 0,1],[0, 1,1],[-1, 1,1]]],
    // Face 3 — north (-Z)  quad [[1,0,0],[0,0,0],[0,1,0],[1,1,0]]
    [[[ 1, 0,-1],[0,-1,-1],[ 1,-1,-1]],
     [[-1, 0,-1],[0,-1,-1],[-1,-1,-1]],
     [[-1, 0,-1],[0, 1,-1],[-1, 1,-1]],
     [[ 1, 0,-1],[0, 1,-1],[ 1, 1,-1]]],
    // Face 4 — east (+X)  quad [[1,0,1],[1,0,0],[1,1,0],[1,1,1]]
    [[[ 1, 0, 1],[ 1,-1, 0],[ 1,-1, 1]],
     [[ 1, 0,-1],[ 1,-1, 0],[ 1,-1,-1]],
     [[ 1, 0,-1],[ 1, 1, 0],[ 1, 1,-1]],
     [[ 1, 0, 1],[ 1, 1, 0],[ 1, 1, 1]]],
    // Face 5 — west (-X)  quad [[0,0,0],[0,0,1],[0,1,1],[0,1,0]]
    [[[-1, 0,-1],[-1,-1, 0],[-1,-1,-1]],
     [[-1, 0, 1],[-1,-1, 0],[-1,-1, 1]],
     [[-1, 0, 1],[-1, 1, 0],[-1, 1, 1]],
     [[-1, 0,-1],[-1, 1, 0],[-1, 1,-1]]],
];

pub struct ChunkMesh {
    pub vertices: Vec<Vertex>,
    pub indices:  Vec<u32>,
}

impl ChunkMesh {
    /// Build the opaque solid mesh for the chunk at (cx, cz).
    pub fn build(world: &World, cx: i32, cz: i32) -> Self {
        let chunk = match world.get_chunk(cx, cz) {
            Some(c) => c,
            None    => return Self::empty(),
        };

        let mut mesh = Self::empty();
        let column_visuals = sample_chunk_visuals(world, cx, cz);
        let hide_dense_foliage = world.low_end_mode_enabled();
        let ox = (cx * CHUNK_W as i32) as f32;
        let oz = (cz * CHUNK_D as i32) as f32;

        for x in 0..CHUNK_W {
            for z in 0..CHUNK_D {
                let column_visual = column_visuals[x * CHUNK_D + z];
                for y in 0..CHUNK_H {
                    let block = *chunk.get(x, y, z);
                    if hide_dense_foliage && block.hide_on_low_end() { continue; }
                    
                    // ✅ SPRITE-LIKE BLOCKS (Stick, Bush, Flower, Fern) ✅
                    if block.is_sprite_like() {
                        let appearance = resolve_block_appearance(block);
                        let wx = cx * CHUNK_W as i32 + x as i32;
                        let wy = y as i32;
                        let wz = cz * CHUNK_D as i32 + z as i32;
                        let biome_tint = column_visual.biome_tint;
                        
                        // Render as 2 crossed quads (like flowers in Minecraft)
                        mesh.push_sprite(
                            ox + x as f32, y as f32, oz + z as f32,
                            appearance.face_uvs[0],
                            0.6,
                            biome_tint,
                        );
                        continue;
                    }
                    
                    if !block.is_cube_meshed() { continue; }

                    let appearance = resolve_block_appearance(block);
                    let wx = cx * CHUNK_W as i32 + x as i32;
                    let wy = y as i32;
                    let wz = cz * CHUNK_D as i32 + z as i32;
                    let biome_tint = if appearance.foliage_tint > 0.5 {
                        world.visuals_at(wx, wz).foliage_color()
                    } else {
                        column_visual.biome_tint
                    };

                    for face in 0..6_usize {
                        let [dx, dy, dz] = FACE_OFFSETS[face];
                        if world.get_block(wx + dx, wy + dy, wz + dz).is_opaque() { continue; }

                        let mut surface_data = column_visual.surface_data;
                        surface_data[3] = face_variation_seed(wx, wy, wz, face);

                        mesh.push_face(
                            ox + x as f32, y as f32, oz + z as f32,
                            face, appearance.face_uvs[face], SOLID_BRIGHTNESS[face], biome_tint, appearance.foliage_tint, surface_data,
                            world, wx, wy, wz,
                        );
                    }
                }
            }
        }

        mesh
    }

    /// Build the translucent water mesh for the chunk at (cx, cz).
    pub fn build_water(world: &World, cx: i32, cz: i32) -> Self {
        let chunk = match world.get_chunk(cx, cz) {
            Some(c) => c,
            None    => return Self::empty(),
        };

        let mut mesh = Self::empty();
        let column_visuals = sample_chunk_visuals(world, cx, cz);
        let ox = (cx * CHUNK_W as i32) as f32;
        let oz = (cz * CHUNK_D as i32) as f32;
        let water_uvs = BlockType::Water.face_uvs();

        for x in 0..CHUNK_W {
            for z in 0..CHUNK_D {
                let column_visual = column_visuals[x * CHUNK_D + z];
                for y in 0..WATER_SCAN_TOP {
                    if *chunk.get(x, y, z) != BlockType::Water { continue; }

                    let meta  = chunk.water_meta_get(x, y, z);
                    let level = if meta == 0 || meta > 8 { 8u8 } else { meta };
                    let fill  = level as f32 / 8.0;
                    let top_y_offset = fill - 1.0;

                    for face in 0..6_usize {
                        // Bottom face (face 1) is always occluded by solid
                        // blocks below water and causes dark artefacts with
                        // alpha blending — skip it unconditionally.
                        if face == 1 { continue; }

                        let [dx, dy, dz] = FACE_OFFSETS[face];
                        let wx = cx * CHUNK_W as i32 + x as i32 + dx;
                        let wy = y as i32 + dy;
                        let wz = cz * CHUNK_D as i32 + z as i32 + dz;

                        if face >= 2 {
                            let ncx = wx.div_euclid(CHUNK_W as i32);
                            let ncz = wz.div_euclid(CHUNK_D as i32);
                            if world.get_chunk(ncx, ncz).is_none() { continue; }
                        }

                        let nb = world.get_block(wx, wy, wz);
                        // Skip interior water-water faces AND faces hidden inside solid blocks.
                        if nb == BlockType::Water || nb.is_opaque() { continue; }

                        let y_offset = if face == 0 { top_y_offset - 0.01 } else { 0.0 };

                        let mut surface_data = column_visual.surface_data;
                        surface_data[3] = face_variation_seed(wx, y as i32, wz, face);

                        // Water uses flat shading — no AO needed on translucent surfaces.
                        mesh.push_face_flat(
                            ox + x as f32, y as f32 + y_offset, oz + z as f32,
                            face, water_uvs[face], WATER_BRIGHTNESS[face], column_visual.biome_tint, 0.0, surface_data,
                        );
                    }
                }
            }
        }

        mesh
    }

    fn empty() -> Self {
        Self { vertices: Vec::new(), indices: Vec::new() }
    }

    /// Push one quad with **per-vertex ambient occlusion** baked into brightness.
    ///
    /// Checks the three neighbouring blocks at each quad corner to calculate how
    /// much each vertex is "tucked away" in a crevice.  Vertices in deeply occluded
    /// corners receive up to ~65% brightness reduction.  The quad triangulation is
    /// flipped when the AO gradient would otherwise create visible discontinuities.
    fn push_face(
        &mut self,
        bx: f32, by: f32, bz: f32,
        face: usize,
        tile: TileUV,
        base_brightness: f32,
        biome_tint: [f32; 3],
        foliage_tint: f32,
        surface_data: [f32; 4],
        world: &World,
        wx: i32, wy: i32, wz: i32,
    ) {
        let base   = self.vertices.len() as u32;
        let uvs    = tile.uvs();
        let is_top = if tile.is_top { 1.0_f32 } else { 0.0 };
        let normal = FACE_NORMALS[face];

        // Per-vertex AO: 0.0 = fully occluded corner, 1.0 = open
        let ao = [
            vertex_ao(world, wx, wy, wz, face, 0),
            vertex_ao(world, wx, wy, wz, face, 1),
            vertex_ao(world, wx, wy, wz, face, 2),
            vertex_ao(world, wx, wy, wz, face, 3),
        ];

        // Point light from adjacent emissive blocks (GlowRock, etc.).
        // Checked once per face rather than per vertex for performance.
        let point_light = adjacent_point_light(world, wx, wy, wz);

        for v in 0..4_usize {
            let [qx, qy, qz] = FACE_QUADS[face][v];
            // AO floor = 0.35 keeps deep corners visible (pure black looks wrong).
            // Point light is not attenuated by AO — it fills dark corners.
            let brightness = (base_brightness * (0.35 + 0.65 * ao[v]) + point_light).min(1.0);
            self.vertices.push(Vertex {
                position:   [bx + qx, by + qy, bz + qz],
                tex_coords: uvs[v],
                normal,
                brightness,
                is_top,
                biome_tint,
                surface_data,
                foliage_tint,
            });
        }

        // Flip the diagonal so the darker gradient runs the "right" way.
        // Without this, AO values at opposing corners create a visible crease.
        if ao[0] + ao[2] < ao[1] + ao[3] {
            self.indices.extend_from_slice(&[base, base+1, base+3, base+1, base+2, base+3]);
        } else {
            self.indices.extend_from_slice(&[base, base+1, base+2, base, base+2, base+3]);
        }
    }

    /// Push one quad with **uniform brightness** (no AO). Used for water and
    /// other translucent geometry where AO would look incorrect.
    fn push_face_flat(
        &mut self,
        bx: f32, by: f32, bz: f32,
        face: usize,
        tile: TileUV,
        brightness: f32,
        biome_tint: [f32; 3],
        foliage_tint: f32,
        surface_data: [f32; 4],
    ) {
        let base   = self.vertices.len() as u32;
        let uvs    = tile.uvs();
        let is_top = if tile.is_top { 1.0_f32 } else { 0.0 };
        let normal = FACE_NORMALS[face];

        for v in 0..4_usize {
            let [qx, qy, qz] = FACE_QUADS[face][v];
            self.vertices.push(Vertex {
                position:   [bx + qx, by + qy, bz + qz],
                tex_coords: uvs[v],
                normal,
                brightness,
                is_top,
                biome_tint,
                surface_data,
                foliage_tint,
            });
        }

        self.indices.extend_from_slice(&[base, base+1, base+2, base, base+2, base+3]);
    }

    /// Render sprite-like block (flower, stick, bush) as 2 crossed quads
    fn push_sprite(
        &mut self,
        x: f32,
        y: f32,
        z: f32,
        uv: TileUV,
        size: f32,
        tint: [f32; 3],
    ) {
        let uvs = uv.uvs();
        let base = self.vertices.len() as u32;
        let half = size * 0.5;
        let brightness = 0.8; // Sprites get consistent brightness
        
        // Quad 1: North-South (X-axis rotation)
        let positions_1 = [
            [x - half, y, z],
            [x + half, y, z],
            [x + half, y + size, z],
            [x - half, y + size, z],
        ];
        
        for (pos, uv) in positions_1.iter().zip(uvs.iter()) {
            self.vertices.push(Vertex {
                position: *pos,
                tex_coords: *uv,
                normal: [1.0, 0.0, 0.0],
                brightness,
                is_top: 0.0,
                biome_tint: tint,
                surface_data: [0.0, 0.0, 0.0, 0.0],
                foliage_tint: 1.0,
            });
        }
        self.indices.extend_from_slice(&[base, base+1, base+2, base, base+2, base+3]);
        
        // Quad 2: East-West (Z-axis rotation) 
        let positions_2 = [
            [x, y, z - half],
            [x, y, z + half],
            [x, y + size, z + half],
            [x, y + size, z - half],
        ];
        
        let base2 = self.vertices.len() as u32;
        for (pos, uv) in positions_2.iter().zip(uvs.iter()) {
            self.vertices.push(Vertex {
                position: *pos,
                tex_coords: *uv,
                normal: [0.0, 0.0, 1.0],
                brightness,
                is_top: 0.0,
                biome_tint: tint,
                surface_data: [0.0, 0.0, 0.0, 0.0],
                foliage_tint: 1.0,
            });
        }
        self.indices.extend_from_slice(&[base2, base2+1, base2+2, base2, base2+2, base2+3]);
    }
}

// ─────────────────────────────────────────────────────────────────────────────
//  Emissive block light helpers
// ─────────────────────────────────────────────────────────────────────────────

/// Light emission factor (0.0 – 1.0) for blocks that glow.
#[inline]
fn block_emission(block: BlockType) -> f32 {
    match block {
        BlockType::GlowRock => 1.0,
        BlockType::EmberRock => 0.12,
        _ => 0.0,
    }
}

/// Check the 6 cardinal neighbours of a block for emissive sources and
/// return an additive light contribution (0.0 – 1.0) to apply to the face.
///
/// Using a 1-block range keeps the per-face cost to exactly 6 world lookups
/// while still illuminating cave walls next to lava / glowstone pockets.
/// The result is not attenuated by AO so emissive light fills dark corners.
#[inline]
fn adjacent_point_light(world: &World, wx: i32, wy: i32, wz: i32) -> f32 {
    let neighbors: [(i32, i32, i32); 6] = [
        ( 0,  1,  0), ( 0, -1,  0),
        ( 0,  0,  1), ( 0,  0, -1),
        ( 1,  0,  0), (-1,  0,  0),
    ];
    let mut light = 0.0f32;
    for (dx, dy, dz) in neighbors {
        let e = block_emission(world.get_block(wx + dx, wy + dy, wz + dz));
        if e > light { light = e; }
    }
    // Scale down: 1-block-distance glow should illuminate but not overexpose.
    light * 0.55
}

// ─────────────────────────────────────────────────────────────────────────────
//  AO helper
// ─────────────────────────────────────────────────────────────────────────────

/// Compute the ambient occlusion factor (0.0 … 1.0) for a single vertex.
///
/// Checks the two side-neighbours and the diagonal corner.  If both sides are
/// solid the vertex is fully occluded (returns 0.0) regardless of the corner.
#[inline]
fn vertex_ao(world: &World, bx: i32, by: i32, bz: i32, face: usize, vert: usize) -> f32 {
    let [[dx1,dy1,dz1], [dx2,dy2,dz2], [dx3,dy3,dz3]] = AO_SAMPLES[face][vert];
    let s1 = world.get_block(bx + dx1, by + dy1, bz + dz1).is_opaque();
    let s2 = world.get_block(bx + dx2, by + dy2, bz + dz2).is_opaque();
    let s3 = world.get_block(bx + dx3, by + dy3, bz + dz3).is_opaque();
    // Both sides solid → maximum occlusion; otherwise count all solids.
    let count = if s1 && s2 { 3 } else { s1 as u32 + s2 as u32 + s3 as u32 };
    (3 - count) as f32 / 3.0
}

fn sample_chunk_visuals(world: &World, cx: i32, cz: i32) -> Vec<ColumnVisuals> {
    let mut visuals = Vec::with_capacity(CHUNK_W * CHUNK_D);
    for x in 0..CHUNK_W {
        for z in 0..CHUNK_D {
            let wx = cx * CHUNK_W as i32 + x as i32;
            let wz = cz * CHUNK_D as i32 + z as i32;
            let sample = world.visuals_at(wx, wz);
            visuals.push(ColumnVisuals {
                biome_tint: sample.vegetation_tint,
                surface_data: [sample.warmth, sample.moisture, sample.lushness, 0.0],
            });
        }
    }
    visuals
}

fn resolve_block_appearance(block: BlockType) -> BlockAppearance {
    if !block.is_foliage() {
        return BlockAppearance {
            face_uvs: block.face_uvs(),
            foliage_tint: 0.0,
        };
    }

    let face_uvs = resolve_model_face_uvs(block)
        .or_else(|| resolve_foliage_cube_uvs(block))
        .unwrap_or([texture_atlas::tile_tree_leaves(); 6]);

    BlockAppearance {
        face_uvs,
        foliage_tint: 1.0,
    }
}

fn resolve_model_face_uvs(block: BlockType) -> Option<[TileUV; 6]> {
    if !block.has_model() {
        return None;
    }

    let model = BlockModelLoader::get_model(block.texture_name())?;
    let mut face_uvs = [texture_atlas::tile_tree_leaves(); 6];

    for (face_index, texture_name) in model.textures.iter().enumerate() {
        let is_top = face_index < 2;
        face_uvs[face_index] = texture_atlas::tile_by_texture_name(texture_name, is_top)?;
    }

    Some(face_uvs)
}

fn resolve_foliage_cube_uvs(block: BlockType) -> Option<[TileUV; 6]> {
    if !block.has_registered_texture() {
        return None;
    }

    let tile = texture_atlas::tile_by_texture_name(block.texture_name(), false)?;
    Some([tile; 6])
}

#[inline]
fn face_variation_seed(wx: i32, wy: i32, wz: i32, face: usize) -> f32 {
    let mut hash = (wx as u32).wrapping_mul(0x9E37_79B9);
    hash ^= (wy as u32).wrapping_mul(0x85EB_CA6B);
    hash ^= (wz as u32).wrapping_mul(0xC2B2_AE35);
    hash ^= (face as u32).wrapping_mul(0x27D4_EB2D);
    hash ^= hash >> 15;
    hash = hash.wrapping_mul(0x85EB_CA6B);
    hash ^= hash >> 13;
    hash = hash.wrapping_mul(0xC2B2_AE35);
    hash ^= hash >> 16;
    (hash & 0x00FF_FFFF) as f32 / 16_777_215.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tree_leaves_blocks_build_visible_mesh_geometry() {
        let mut world = World::new(1234);
        world.set_block(0, 120, 0, BlockType::TreeLeaves);

        let mesh = ChunkMesh::build(&world, 0, 0);

        assert!(!mesh.vertices.is_empty());
        assert!(mesh.vertices.iter().any(|vertex| vertex.foliage_tint > 0.5));
    }

    #[test]
    fn cross_chunk_foliage_blocks_mesh_on_both_sides() {
        let mut world = World::new(4321);
        let y = 120;
        let edge_x = CHUNK_W as i32 - 1;

        world.set_block(edge_x, y, 0, BlockType::TreeLeaves);
        world.set_block(edge_x + 1, y, 0, BlockType::TreeLeaves);

        let left_mesh = ChunkMesh::build(&world, 0, 0);
        let right_mesh = ChunkMesh::build(&world, 1, 0);

        assert!(!left_mesh.vertices.is_empty());
        assert!(!right_mesh.vertices.is_empty());
    }
}

