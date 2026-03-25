use crate::world::{World, BlockType, chunk::{CHUNK_W, CHUNK_H, CHUNK_D}};
use super::vertices::Vertex;

/// Six faces: +Y, -Y, +Z, -Z, +X, -X
const FACE_NORMALS: [[f32; 3]; 6] = [
    [ 0.0,  1.0,  0.0], // Top
    [ 0.0, -1.0,  0.0], // Bottom
    [ 0.0,  0.0,  1.0], // Front  (+Z)
    [ 0.0,  0.0, -1.0], // Back   (-Z)
    [ 1.0,  0.0,  0.0], // Right  (+X)
    [-1.0,  0.0,  0.0], // Left   (-X)
];

/// Neighbour offsets per face
const FACE_OFFSETS: [[i32; 3]; 6] = [
    [ 0,  1,  0],
    [ 0, -1,  0],
    [ 0,  0,  1],
    [ 0,  0, -1],
    [ 1,  0,  0],
    [-1,  0,  0],
];

/// Quad vertices per face (local offsets, before block position)
/// Each face = 4 vertices, CCW winding, Y-up
const FACE_QUADS: [[[f32; 3]; 4]; 6] = [
    // Top (+Y)
    [[0.0,1.0,0.0],[1.0,1.0,0.0],[1.0,1.0,1.0],[0.0,1.0,1.0]],
    // Bottom (-Y)
    [[0.0,0.0,1.0],[1.0,0.0,1.0],[1.0,0.0,0.0],[0.0,0.0,0.0]],
    // Front (+Z)
    [[0.0,0.0,1.0],[1.0,0.0,1.0],[1.0,1.0,1.0],[0.0,1.0,1.0]],
    // Back (-Z)
    [[1.0,0.0,0.0],[0.0,0.0,0.0],[0.0,1.0,0.0],[1.0,1.0,0.0]],
    // Right (+X)
    [[1.0,0.0,1.0],[1.0,0.0,0.0],[1.0,1.0,0.0],[1.0,1.0,1.0]],
    // Left (-X)
    [[0.0,0.0,0.0],[0.0,0.0,1.0],[0.0,1.0,1.0],[0.0,1.0,0.0]],
];

const ATLAS_COLS: f32 = 4.0;
const ATLAS_ROWS: f32 = 4.0;

pub struct ChunkMesh {
    pub vertices: Vec<Vertex>,
    pub indices:  Vec<u32>,
}

impl ChunkMesh {
    pub fn build(world: &World, cx: i32, cz: i32) -> Self {
        let mut vertices = Vec::new();
        let mut indices  = Vec::new();

        let chunk = match world.get_chunk(cx, cz) {
            Some(c) => c,
            None    => return Self { vertices, indices },
        };

        let ox = (cx * CHUNK_W as i32) as f32;
        let oz = (cz * CHUNK_D as i32) as f32;

        for x in 0..CHUNK_W {
            for y in 0..CHUNK_H {
                for z in 0..CHUNK_D {
                    let block = chunk.get(x, y, z);
                    if !block.is_opaque() { continue; }

                    let face_uvs = block.face_uvs();

                    for face in 0..6 {
                        // Check if neighbour is opaque → skip face
                        let [dx, dy, dz] = FACE_OFFSETS[face];
                        let nx = x as i32 + dx;
                        let ny = y as i32 + dy;
                        let nz = z as i32 + dz;

                        let neighbour = world.get_block(
                            cx * CHUNK_W as i32 + nx,
                            ny,
                            cz * CHUNK_D as i32 + nz,
                        );
                        if neighbour.is_opaque() { continue; }

                        // Atlas UV for this face
                        let (ac, ar) = face_uvs[face];
                        let u0 = ac as f32 / ATLAS_COLS;
                        let v0 = ar as f32 / ATLAS_ROWS;
                        let u1 = u0 + 1.0 / ATLAS_COLS;
                        let v1 = v0 + 1.0 / ATLAS_ROWS;
                        let uvs = [[u0,v0],[u1,v0],[u1,v1],[u0,v1]];

                        let normal = FACE_NORMALS[face];

                        // Face brightness (simple directional shading)
                        let brightness = match face {
                            0 => 1.00f32, // top
                            1 => 0.50f32, // bottom
                            2 | 3 => 0.80f32, // front/back
                            _ => 0.65f32, // sides
                        };

                        let base = vertices.len() as u32;
                        for v in 0..4 {
                            let [qx, qy, qz] = FACE_QUADS[face][v];
                            vertices.push(Vertex {
                                position: [
                                    ox + x as f32 + qx,
                                    y as f32 + qy,
                                    oz + z as f32 + qz,
                                ],
                                tex_coords: uvs[v],
                                normal,
                                brightness,
                            });
                        }
                        // Two triangles per quad
                        indices.extend_from_slice(&[
                            base, base+1, base+2,
                            base, base+2, base+3,
                        ]);
                    }
                }
            }
        }

        Self { vertices, indices }
    }
}