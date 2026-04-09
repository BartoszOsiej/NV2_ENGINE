use cgmath::{InnerSpace, Vector3};

use crate::world::block::BlockType;
use crate::world::World;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RaycastHit {
    pub block_pos: Vector3<i32>,
    pub face_normal: Vector3<i32>,
    pub block_type: BlockType,
}

pub(crate) fn raycast_solid_block(
    origin: Vector3<f32>,
    direction: Vector3<f32>,
    max_dist: f32,
    world: &World,
) -> Option<RaycastHit> {
    if direction.magnitude2() <= f32::EPSILON {
        return None;
    }

    let dir = direction.normalize();
    let mut bx = origin.x.floor() as i32;
    let mut by = origin.y.floor() as i32;
    let mut bz = origin.z.floor() as i32;

    let step_x = dir.x.signum() as i32;
    let step_y = dir.y.signum() as i32;
    let step_z = dir.z.signum() as i32;

    let inv_x = if dir.x.abs() > f32::EPSILON { 1.0 / dir.x.abs() } else { f32::INFINITY };
    let inv_y = if dir.y.abs() > f32::EPSILON { 1.0 / dir.y.abs() } else { f32::INFINITY };
    let inv_z = if dir.z.abs() > f32::EPSILON { 1.0 / dir.z.abs() } else { f32::INFINITY };

    let mut side_x = initial_side_distance(origin.x, bx, dir.x, inv_x);
    let mut side_y = initial_side_distance(origin.y, by, dir.y, inv_y);
    let mut side_z = initial_side_distance(origin.z, bz, dir.z, inv_z);

    let current = world.get_block(bx, by, bz);
    if current.is_solid() {
        return Some(RaycastHit {
            block_pos: Vector3::new(bx, by, bz),
            face_normal: Vector3::new(0, 0, 0),
            block_type: current,
        });
    }

    loop {
        let (travel, face_normal) = if side_x <= side_y && side_x <= side_z {
            bx += step_x;
            let dist = side_x;
            side_x += inv_x;
            (dist, Vector3::new(-step_x, 0, 0))
        } else if side_y <= side_z {
            by += step_y;
            let dist = side_y;
            side_y += inv_y;
            (dist, Vector3::new(0, -step_y, 0))
        } else {
            bz += step_z;
            let dist = side_z;
            side_z += inv_z;
            (dist, Vector3::new(0, 0, -step_z))
        };

        if travel > max_dist {
            return None;
        }

        let block = world.get_block(bx, by, bz);
        if block.is_solid() {
            return Some(RaycastHit {
                block_pos: Vector3::new(bx, by, bz),
                face_normal,
                block_type: block,
            });
        }
    }
}

fn initial_side_distance(origin: f32, block: i32, dir: f32, inv: f32) -> f32 {
    if !inv.is_finite() {
        return f32::INFINITY;
    }

    if dir >= 0.0 {
        ((block + 1) as f32 - origin) * inv
    } else {
        (origin - block as f32) * inv
    }
}