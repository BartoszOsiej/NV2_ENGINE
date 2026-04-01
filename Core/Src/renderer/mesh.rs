use crate::world::{World, chunk::{CHUNK_W, CHUNK_H, CHUNK_D}};
use crate::world::block::BlockType;
use super::vertices::Vertex;

const FACE_NORMALS: [[f32; 3]; 6] = [
    [ 0.0,  1.0,  0.0],
    [ 0.0, -1.0,  0.0],
    [ 0.0,  0.0,  1.0],
    [ 0.0,  0.0, -1.0],
    [ 1.0,  0.0,  0.0],
    [-1.0,  0.0,  0.0],
];

const FACE_OFFSETS: [[i32; 3]; 6] = [
    [ 0,  1,  0],
    [ 0, -1,  0],
    [ 0,  0,  1],
    [ 0,  0, -1],
    [ 1,  0,  0],
    [-1,  0,  0],
];

const FACE_QUADS: [[[f32; 3]; 4]; 6] = [
    [[0.0,1.0,0.0],[1.0,1.0,0.0],[1.0,1.0,1.0],[0.0,1.0,1.0]],
    [[0.0,0.0,1.0],[1.0,0.0,1.0],[1.0,0.0,0.0],[0.0,0.0,0.0]],
    [[0.0,0.0,1.0],[1.0,0.0,1.0],[1.0,1.0,1.0],[0.0,1.0,1.0]],
    [[1.0,0.0,0.0],[0.0,0.0,0.0],[0.0,1.0,0.0],[1.0,1.0,0.0]],
    [[1.0,0.0,1.0],[1.0,0.0,0.0],[1.0,1.0,0.0],[1.0,1.0,1.0]],
    [[0.0,0.0,0.0],[0.0,0.0,1.0],[0.0,1.0,1.0],[0.0,1.0,0.0]],
];

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

        // Sample biome tint once at the chunk center — reduces ambient_at() from 256 to 1
        // noise evaluation per chunk (~256x speedup for the most expensive per-chunk operation).
        let center_wx = cx * CHUNK_W as i32 + CHUNK_W as i32 / 2;
        let center_wz = cz * CHUNK_D as i32 + CHUNK_D as i32 / 2;
        let amb = world.ambient_at(center_wx, center_wz);
        let biome_tint = [amb[0], amb[1], amb[2]];

        for x in 0..CHUNK_W {
            for z in 0..CHUNK_D {

                for y in 0..CHUNK_H {
                    let block = chunk.get(x, y, z);
                    if !block.is_opaque() { continue; }

                    let face_uvs = block.face_uvs();

                    for face in 0..6usize {
                        let [dx, dy, dz] = FACE_OFFSETS[face];
                        let nx = cx * CHUNK_W as i32 + x as i32 + dx;
                        let ny = y as i32 + dy;
                        let nz = cz * CHUNK_D as i32 + z as i32 + dz;

                        let neighbour = world.get_block(nx, ny, nz);
                        if neighbour.is_opaque() { continue; }

                        let tile = face_uvs[face];
                        let uvs  = tile.uvs();

                        let brightness: f32 = match face {
                            0 => 1.00,
                            1 => 0.50,
                            2 | 3 => 0.80,
                            _ => 0.65,
                        };

                        let base = vertices.len() as u32;
                        let is_top_face = if tile.is_top { 1.0 } else { 0.0 };
                        for v in 0..4usize {
                            let [qx, qy, qz] = FACE_QUADS[face][v];
                            vertices.push(Vertex {
                                position:   [ox + x as f32 + qx, y as f32 + qy, oz + z as f32 + qz],
                                tex_coords: uvs[v],
                                normal:     FACE_NORMALS[face],
                                brightness,
                                is_top:     is_top_face,
                                biome_tint,
                            });
                        }
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

    // Build a mesh containing only water faces (for translucent water pass).
    // We generate faces for `BlockType::Water` where the neighbor is not water.
    pub fn build_water(world: &World, cx: i32, cz: i32) -> Self {
        let mut vertices = Vec::new();
        let mut indices  = Vec::new();

        let chunk = match world.get_chunk(cx, cz) {
            Some(c) => c,
            None    => return Self { vertices, indices },
        };

        let ox = (cx * CHUNK_W as i32) as f32;
        let oz = (cz * CHUNK_D as i32) as f32;

        let center_wx = cx * CHUNK_W as i32 + CHUNK_W as i32 / 2;
        let center_wz = cz * CHUNK_D as i32 + CHUNK_D as i32 / 2;
        let wamb = world.ambient_at(center_wx, center_wz);
        let biome_tint = [wamb[0], wamb[1], wamb[2]];

        for x in 0..CHUNK_W {
            for z in 0..CHUNK_D {

                for y in 0..CHUNK_H {
                    let block = chunk.get(x, y, z);
                    if *block != BlockType::Water { continue; }

                    let face_uvs = block.face_uvs();

                    for face in 0..6usize {
                        let [dx, dy, dz] = FACE_OFFSETS[face];
                        let nx = cx * CHUNK_W as i32 + x as i32 + dx;
                        let ny = y as i32 + dy;
                        let nz = cz * CHUNK_D as i32 + z as i32 + dz;

                        let neighbour = world.get_block(nx, ny, nz);
                        // Skip if neighbor is also water (internal face)
                        if neighbour == BlockType::Water { continue; }

                        let tile = face_uvs[face];
                        let uvs  = tile.uvs();

                        let brightness: f32 = match face {
                            0 => 0.95, // water surface slightly brighter
                            1 => 0.50,
                            2 | 3 => 0.70,
                            _ => 0.60,
                        };

                        let base = vertices.len() as u32;
                        let is_top_face = if tile.is_top { 1.0 } else { 0.0 };
                        for v in 0..4usize {
                            let [qx, qy, qz] = FACE_QUADS[face][v];
                            // Slightly lower the water top to avoid z-fighting with terrain
                            let qy_adjust = if face == 0 { -0.01 } else { 0.0 };
                            vertices.push(Vertex {
                                position:   [ox + x as f32 + qx, y as f32 + qy + qy_adjust, oz + z as f32 + qz],
                                tex_coords: uvs[v],
                                normal:     FACE_NORMALS[face],
                                brightness,
                                is_top:     is_top_face,
                                biome_tint,
                            });
                        }
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