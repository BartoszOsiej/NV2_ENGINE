use crate::world::{World, chunk::{CHUNK_W, CHUNK_H, CHUNK_D}};
use crate::world::block::BlockType;
use super::vertices::Vertex;

// Water never appears above this Y level in natural terrain, so there is no
// point scanning the full 256-block column when building water meshes.
const WATER_SCAN_TOP: usize = 120;

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
// Index order: top-left, top-right, bottom-right, bottom-left (for CCW winding).
const FACE_QUADS: [[[f32; 3]; 4]; 6] = [
    [[0.0,1.0,0.0],[1.0,1.0,0.0],[1.0,1.0,1.0],[0.0,1.0,1.0]],
    [[0.0,0.0,1.0],[1.0,0.0,1.0],[1.0,0.0,0.0],[0.0,0.0,0.0]],
    [[0.0,0.0,1.0],[1.0,0.0,1.0],[1.0,1.0,1.0],[0.0,1.0,1.0]],
    [[1.0,0.0,0.0],[0.0,0.0,0.0],[0.0,1.0,0.0],[1.0,1.0,0.0]],
    [[1.0,0.0,1.0],[1.0,0.0,0.0],[1.0,1.0,0.0],[1.0,1.0,1.0]],
    [[0.0,0.0,0.0],[0.0,0.0,1.0],[0.0,1.0,1.0],[0.0,1.0,0.0]],
];

// Face brightness multipliers for solid blocks — gives a cheap ambient-occlusion feel.
const SOLID_BRIGHTNESS: [f32; 6] = [1.00, 0.50, 0.80, 0.80, 0.65, 0.65];

// Water faces are slightly darker overall; the surface is a touch brighter
// so it catches the light even at a shallow angle.
const WATER_BRIGHTNESS: [f32; 6] = [0.95, 0.50, 0.70, 0.70, 0.60, 0.60];

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
        let biome_tint = chunk_biome_tint(world, cx, cz);
        let ox = (cx * CHUNK_W as i32) as f32;
        let oz = (cz * CHUNK_D as i32) as f32;

        for x in 0..CHUNK_W {
            for z in 0..CHUNK_D {
                for y in 0..CHUNK_H {
                    let block = *chunk.get(x, y, z);
                    if !block.is_opaque() { continue; }

                    let face_uvs = block.face_uvs();

                    for face in 0..6_usize {
                        let [dx, dy, dz] = FACE_OFFSETS[face];
                        let wx = cx * CHUNK_W as i32 + x as i32 + dx;
                        let wy = y as i32 + dy;
                        let wz = cz * CHUNK_D as i32 + z as i32 + dz;

                        if world.get_block(wx, wy, wz).is_opaque() { continue; }

                        mesh.push_face(
                            ox + x as f32, y as f32, oz + z as f32,
                            face, face_uvs[face], SOLID_BRIGHTNESS[face], biome_tint,
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
        let biome_tint = chunk_biome_tint(world, cx, cz);
        let ox = (cx * CHUNK_W as i32) as f32;
        let oz = (cz * CHUNK_D as i32) as f32;
        let water_uvs = BlockType::Water.face_uvs();

        for x in 0..CHUNK_W {
            for z in 0..CHUNK_D {
                for y in 0..WATER_SCAN_TOP {
                    if *chunk.get(x, y, z) != BlockType::Water { continue; }

                    for face in 0..6_usize {
                        let [dx, dy, dz] = FACE_OFFSETS[face];
                        let wx = cx * CHUNK_W as i32 + x as i32 + dx;
                        let wy = y as i32 + dy;
                        let wz = cz * CHUNK_D as i32 + z as i32 + dz;

                        // Skip lateral faces adjacent to unloaded chunks — they cause
                        // hollow floating-cube artifacts at the render edge.
                        if face >= 2 {
                            let ncx = wx.div_euclid(CHUNK_W as i32);
                            let ncz = wz.div_euclid(CHUNK_D as i32);
                            if world.get_chunk(ncx, ncz).is_none() { continue; }
                        }

                        if world.get_block(wx, wy, wz) == BlockType::Water { continue; }

                        // Sink the water surface by 1 mm to avoid z-fighting with terrain above.
                        let y_offset = if face == 0 { -0.01 } else { 0.0 };

                        mesh.push_face(
                            ox + x as f32, y as f32 + y_offset, oz + z as f32,
                            face, water_uvs[face], WATER_BRIGHTNESS[face], biome_tint,
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

    /// Append one quad (4 vertices + 6 indices) for the given face.
    fn push_face(
        &mut self,
        bx: f32, by: f32, bz: f32,
        face: usize,
        tile: super::texture_atlas::TileUV,
        brightness: f32,
        biome_tint: [f32; 3],
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
            });
        }

        self.indices.extend_from_slice(&[base, base+1, base+2, base, base+2, base+3]);
    }
}

/// Sample the biome ambient tint once at the chunk centre.
/// Calling `ambient_at` for every individual voxel would be ~256× slower.
fn chunk_biome_tint(world: &World, cx: i32, cz: i32) -> [f32; 3] {
    let wx = cx * CHUNK_W as i32 + CHUNK_W as i32 / 2;
    let wz = cz * CHUNK_D as i32 + CHUNK_D as i32 / 2;
    let amb = world.ambient_at(wx, wz);
    [amb[0], amb[1], amb[2]]
}