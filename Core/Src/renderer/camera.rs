use cgmath::{Matrix4, Vector3, Point3, Deg, perspective, InnerSpace, EuclideanSpace};
use crate::input::InputState;
use winit::keyboard::KeyCode;

const PLAYER_HEIGHT:      f32 = 1.8;
const PLAYER_HALF_HEIGHT: f32 = PLAYER_HEIGHT * 0.5;
const PLAYER_EYE_HEIGHT:  f32 = 1.7;
const PLAYER_RADIUS:      f32 = 0.3;
const CAMERA_NEAR_PLANE:  f32 = 0.05;
const WALK_SPEED:         f32 = 5.5;
const JUMP_SPEED:         f32 = 8.0;
const GRAVITY:            f32 = 20.0;
const WATER_GRAVITY:      f32 = 6.0;
const WATER_SINK_CAP:     f32 = 2.0;
const MOUSE_SENSITIVITY:  f32 = 0.002;

#[derive(Clone, Copy, Debug)]
pub struct AABB {
    pub min: Vector3<f32>,
    pub max: Vector3<f32>,
}

impl AABB {
    pub fn new(center: Vector3<f32>, half_extent: Vector3<f32>) -> Self {
        Self {
            min: center - half_extent,
            max: center + half_extent,
        }
    }

    pub fn intersects_block(&self, bx: i32, by: i32, bz: i32) -> bool {
        let b_min = Vector3::new(bx as f32,       by as f32,       bz as f32);
        let b_max = Vector3::new(bx as f32 + 1.0, by as f32 + 1.0, bz as f32 + 1.0);
        self.min.x < b_max.x && self.max.x > b_min.x
            && self.min.y < b_max.y && self.max.y > b_min.y
            && self.min.z < b_max.z && self.max.z > b_min.z
    }
}

pub struct Camera {
    pub position:  Vector3<f32>,
    pub velocity:  Vector3<f32>,
    pub yaw:       f32,
    pub pitch:     f32,
    pub on_ground: bool,
    pub in_water:  bool,
    bob_phase:     f32,
    bob_offset:    f32,
}

impl Camera {
    pub fn new(position: Vector3<f32>) -> Self {
        Self {
            position,
            velocity:  Vector3::new(0.0, 0.0, 0.0),
            yaw:       -90.0_f32.to_radians(),
            pitch:     0.0,
            on_ground: false,
            in_water:  false,
            bob_phase:  0.0,
            bob_offset: 0.0,
        }
    }

    pub fn handle_input(&mut self, input: &InputState, dt: f32) {
        let forward = Vector3::new(self.yaw.cos(), 0.0, self.yaw.sin()).normalize();
        let right   = Vector3::new(-self.yaw.sin(), 0.0, self.yaw.cos()).normalize();

        let mut move_dir = Vector3::new(0.0_f32, 0.0, 0.0);
        if input.keys_held.contains(&KeyCode::KeyW) { move_dir += forward; }
        if input.keys_held.contains(&KeyCode::KeyS) { move_dir -= forward; }
        if input.keys_held.contains(&KeyCode::KeyA) { move_dir -= right; }
        if input.keys_held.contains(&KeyCode::KeyD) { move_dir += right; }

        if move_dir.magnitude() > 0.0 {
            let v = move_dir.normalize() * WALK_SPEED;
            self.velocity.x = v.x;
            self.velocity.z = v.z;
        }

        if input.keys_held.contains(&KeyCode::Space) {
            if self.on_ground {
                self.velocity.y = JUMP_SPEED;
                self.on_ground  = false;
            } else if self.in_water {
                self.velocity.y = 3.0; // swim upward
            }
        }

        self.yaw   += input.mouse_dx as f32 * MOUSE_SENSITIVITY;
        self.pitch -= input.mouse_dy as f32 * MOUSE_SENSITIVITY;
        self.pitch  = self.pitch.clamp(-1.5, 1.5);

        // Subtle head bob while walking on solid ground.
        let walking = move_dir.magnitude() > 0.0 && self.on_ground && !self.in_water;
        if walking {
            self.bob_phase = (self.bob_phase + dt * 11.0) % std::f32::consts::TAU;
        } else {
            self.bob_phase *= (1.0 - dt * 8.0).max(0.0);
        }
        self.bob_offset = self.bob_phase.sin() * 0.035;
    }

    fn eye_position(&self) -> Point3<f32> {
        Point3::from_vec(self.position + Vector3::new(0.0, PLAYER_EYE_HEIGHT + self.bob_offset, 0.0))
    }

    fn player_aabb(&self) -> AABB {
        AABB::new(
            self.position + Vector3::new(0.0, PLAYER_HALF_HEIGHT, 0.0),
            Vector3::new(PLAYER_RADIUS, PLAYER_HALF_HEIGHT, PLAYER_RADIUS),
        )
    }

    pub fn update_physics(&mut self, world: &crate::world::World, dt: f32) {
        let px = self.position.x.floor() as i32;
        let pz = self.position.z.floor() as i32;

        self.in_water =
            world.get_block(px, self.position.y as i32, pz) == crate::world::BlockType::Water
            || world.get_block(px, (self.position.y + PLAYER_HALF_HEIGHT) as i32, pz) == crate::world::BlockType::Water;

        if self.in_water {
            self.velocity.y -= WATER_GRAVITY * dt;
            self.velocity.y  = self.velocity.y.max(-WATER_SINK_CAP);
            // Frame-rate-independent drag
            let drag_h = 0.93_f32.powf(dt * 60.0);
            let drag_v = 0.965_f32.powf(dt * 60.0);
            self.velocity.x *= drag_h;
            self.velocity.z *= drag_h;
            self.velocity.y *= drag_v;
        } else {
            self.velocity.y -= GRAVITY * dt;
        }

        // Integrate each axis separately so collisions are resolved per-axis.
        self.position.y += self.velocity.y * dt;
        self.on_ground   = false;
        self.resolve_collisions(world, 1);

        self.position.x += self.velocity.x * dt;
        self.resolve_collisions(world, 0);

        self.position.z += self.velocity.z * dt;
        self.resolve_collisions(world, 2);

        // Ground friction — exponential so it is frame-rate-independent.
        let ground_drag = 0.8_f32.powf(dt * 60.0);
        self.velocity.x *= ground_drag;
        self.velocity.z *= ground_drag;
    }

    fn resolve_collisions(&mut self, world: &crate::world::World, axis: u8) {
        let initial = self.player_aabb();
        let x_range = initial.min.x.floor() as i32 ..= initial.max.x.ceil() as i32;
        let y_range = initial.min.y.floor() as i32 ..= initial.max.y.ceil() as i32;
        let z_range = initial.min.z.floor() as i32 ..= initial.max.z.ceil() as i32;

        for bx in x_range {
            for by in y_range.clone() {
                for bz in z_range.clone() {
                    if !world.get_block(bx, by, bz).is_opaque() { continue; }

                    // Recompute AABB each iteration — earlier corrections shift position.
                    let aabb = self.player_aabb();
                    if !aabb.intersects_block(bx, by, bz) { continue; }

                    self.push_out_axis(aabb, bx, by, bz, axis);
                }
            }
        }
    }

    /// Push the player out of a solid block along the given axis.
    fn push_out_axis(&mut self, aabb: AABB, bx: i32, by: i32, bz: i32, axis: u8) {
        const EPSILON: f32 = 0.001;
        match axis {
            0 => {
                if self.velocity.x > 0.0 {
                    self.position.x -= aabb.max.x - bx as f32 + EPSILON;
                } else {
                    self.position.x += bx as f32 + 1.0 - aabb.min.x + EPSILON;
                }
                self.velocity.x = 0.0;
            }
            1 => {
                if self.velocity.y > 0.0 {
                    self.position.y -= aabb.max.y - by as f32 + EPSILON;
                } else {
                    self.position.y += by as f32 + 1.0 - aabb.min.y + EPSILON;
                    self.on_ground   = true;
                }
                self.velocity.y = 0.0;
            }
            2 => {
                if self.velocity.z > 0.0 {
                    self.position.z -= aabb.max.z - bz as f32 + EPSILON;
                } else {
                    self.position.z += bz as f32 + 1.0 - aabb.min.z + EPSILON;
                }
                self.velocity.z = 0.0;
            }
            _ => {}
        }
    }
}

#[repr(C)]
#[derive(Debug, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct CameraUniform {
    pub view_proj: [[f32; 4]; 4],
}

impl CameraUniform {
    pub fn new() -> Self {
        use cgmath::SquareMatrix;
        Self { view_proj: Matrix4::identity().into() }
    }

    pub fn update_view_proj(&mut self, camera: &Camera, aspect: f32) {
        let look_dir = Vector3::new(
            camera.yaw.cos() * camera.pitch.cos(),
            camera.pitch.sin(),
            camera.yaw.sin() * camera.pitch.cos(),
        ).normalize();

        let proj = perspective(Deg(90.0), aspect, CAMERA_NEAR_PLANE, 1000.0);
        let view = Matrix4::look_to_rh(camera.eye_position(), look_dir, Vector3::unit_y());
        self.view_proj = (proj * view).into();
    }
}