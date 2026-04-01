use cgmath::{Vector3, InnerSpace};

#[derive(Debug, Clone, Copy)]
pub struct RaycastHit {
    pub pos: Vector3<i32>,
    pub face: Vector3<i32>,
}

pub fn raycast(origin: Vector3<f32>, direction: Vector3<f32>, max_dist: f32, world: &crate::world::World) -> Option<RaycastHit> {
    let mut curr = origin;
    let step = direction.normalize() * 0.1;
    let mut dist_traveled = 0.0;

    while dist_traveled < max_dist {
        let bx = curr.x.floor() as i32;
        let by = curr.y.floor() as i32;
        let bz = curr.z.floor() as i32;

        if world.get_block(bx, by, bz).is_opaque() {
            let face = determine_face(curr, bx, by, bz);
            return Some(RaycastHit { pos: Vector3::new(bx, by, bz), face });
        }

        curr += step;
        dist_traveled += 0.1;
    }
    None
}

fn determine_face(hit_pos: Vector3<f32>, bx: i32, by: i32, bz: i32) -> Vector3<i32> {
    let dx = (hit_pos.x - (bx as f32 + 0.5)).abs();
    let dy = (hit_pos.y - (by as f32 + 0.5)).abs();
    let dz = (hit_pos.z - (bz as f32 + 0.5)).abs();

    // Wybieramy oś, na której uderzenie jest najbliżej krawędzi bloku
    if dx > dy && dx > dz {
        if hit_pos.x > bx as f32 + 0.5 { Vector3::new(1, 0, 0) } else { Vector3::new(-1, 0, 0) }
    } else if dy > dx && dy > dz {
        if hit_pos.y > by as f32 + 0.5 { Vector3::new(0, 1, 0) } else { Vector3::new(0, -1, 0) }
    } else {
        if hit_pos.z > bz as f32 + 0.5 { Vector3::new(0, 0, 1) } else { Vector3::new(0, 0, -1) }
    }
}