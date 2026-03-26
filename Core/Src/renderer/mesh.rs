use crate::world::{World, chunk::{CHUNK_W, CHUNK_H, CHUNK_D}};
use super::vertices::Vertex;

const FACE_NORMALS: [[f32; 3]; 6] = [[0.,1.,0.], [0.,-1.,0.], [0.,0.,1.], [0.,0.,-1.], [1.,0.,0.], [-1.,0.,0.]];
const FACE_OFFSETS: [[i32; 3]; 6] = [[0,1,0], [0,-1,0], [0,0,1], [0,0,-1], [1,0,0], [-1,0,0]];
const FACE_QUADS: [[[f32; 3]; 4]; 6] = [
    [[0.,1.,0.],[1.,1.,0.],[1.,1.,1.],[0.,1.,1.]],
    [[0.,0.,1.],[1.,0.,1.],[1.,0.,0.],[0.,0.,0.]],
    [[0.,0.,1.],[1.,0.,1.],[1.,1.,1.],[0.,1.,1.]],
    [[1.,0.,0.],[0.,0.,0.],[0.,1.,0.],[1.,1.,0.]],
    [[1.,0.,1.],[1.,0.,0.],[1.,1.,0.],[1.,1.,1.]],
    [[0.,0.,0.],[0.,0.,1.],[0.,1.,1.],[0.,1.,0.]],
];

pub struct ChunkMesh {
    pub vertices: Vec<Vertex>,
    pub indices:  Vec<u32>,
}

impl ChunkMesh {
    pub fn generate(world: &World, cx: i32, cz: i32) -> Self {
        let mut vertices = Vec::new();
        let mut indices  = Vec::new();
        let ox = (cx * CHUNK_W as i32) as f32;
        let oz = (cz * CHUNK_D as i32) as f32;

        for y in 0..CHUNK_H {
            for z in 0..CHUNK_D {
                for x in 0..CHUNK_W {
                    let bx = cx * CHUNK_W as i32 + x as i32;
                    let bz = cz * CHUNK_D as i32 + z as i32;
                    let block = world.get_block(bx, y as i32, bz);

                    if !block.is_opaque() { continue; }

                    // Pobieramy UV z BlockType (metoda zdefiniowana w world/block.rs)
                    let face_uvs = block.face_uvs();

                    for face in 0..6 {
                        let [dx, dy, dz] = FACE_OFFSETS[face];
                        let neighbour = world.get_block(bx + dx, y as i32 + dy, bz + dz);
                        if neighbour.is_opaque() { continue; }

                        let tile = face_uvs[face];
                        let uvs  = tile.uvs();
                        let brightness: f32 = match face { 0 => 1.0, 1 => 0.5, 2|3 => 0.8, _ => 0.65 };

                        let base = vertices.len() as u32;
                        for v in 0..4usize {
                            let [qx, qy, qz] = FACE_QUADS[face][v];
                            vertices.push(Vertex {
                                position:   [ox + x as f32 + qx, y as f32 + qy, oz + z as f32 + qz],
                                tex_coords: uvs[v],
                                normal:     FACE_NORMALS[face],
                                brightness,
                            });
                        }
                        indices.extend_from_slice(&[base, base+1, base+2, base, base+2, base+3]);
                    }
                }
            }
        }
        Self { vertices, indices }
    }
}