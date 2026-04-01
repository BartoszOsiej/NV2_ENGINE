use cgmath::{Matrix4, Vector3, Point3, Deg, perspective, InnerSpace, EuclideanSpace};
use crate::input::InputState;
use winit::keyboard::KeyCode;

const PLAYER_HEIGHT: f32 = 1.8;
const PLAYER_HALF_HEIGHT: f32 = PLAYER_HEIGHT * 0.5;
const PLAYER_EYE_HEIGHT: f32 = 1.7;
const PLAYER_RADIUS: f32 = 0.3;
const CAMERA_NEAR_PLANE: f32 = 0.05;

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
        let b_min = Vector3::new(bx as f32, by as f32, bz as f32);
        let b_max = Vector3::new(bx as f32 + 1.0, by as f32 + 1.0, bz as f32 + 1.0);
        self.min.x < b_max.x && self.max.x > b_min.x
            && self.min.y < b_max.y && self.max.y > b_min.y
            && self.min.z < b_max.z && self.max.z > b_min.z
    }
}

pub struct Camera {
    pub position: Vector3<f32>,
    pub velocity: Vector3<f32>,
    pub yaw: f32,
    pub pitch: f32,
    pub on_ground: bool,
    pub in_water: bool,
}

impl Camera {
    pub fn new(position: Vector3<f32>) -> Self {
        Self {
            position,
            velocity: Vector3::new(0.0, 0.0, 0.0),
            yaw: -90.0f32.to_radians(),
            pitch: 0.0,
            on_ground: false,
            in_water: false,
        }
    }

    pub fn handle_input(&mut self, input: &InputState, _dt: f32) {
        let speed = 10.0;
        let mut move_vec = Vector3::new(0.0, 0.0, 0.0);
        
        let forward = Vector3::new(self.yaw.cos(), 0.0, self.yaw.sin()).normalize();
        let right = Vector3::new(-self.yaw.sin(), 0.0, self.yaw.cos()).normalize();

        if input.keys_held.contains(&KeyCode::KeyW) { move_vec += forward; }
        if input.keys_held.contains(&KeyCode::KeyS) { move_vec -= forward; }
        if input.keys_held.contains(&KeyCode::KeyA) { move_vec -= right; }
        if input.keys_held.contains(&KeyCode::KeyD) { move_vec += right; }

        if move_vec.magnitude() > 0.0 {
            let v = move_vec.normalize() * speed;
            self.velocity.x = v.x;
            self.velocity.z = v.z;
        }

        if input.keys_held.contains(&KeyCode::Space) {
            if self.on_ground {
                self.velocity.y = 10.0;
                self.on_ground = false;
            } else if self.in_water {
                // Swim upward while Space is held
                self.velocity.y = 4.0;
            }
        }

        self.yaw += (input.mouse_dx as f32) * 0.002;
        self.pitch -= (input.mouse_dy as f32) * 0.002;
        self.pitch = self.pitch.clamp(-1.5, 1.5);
    }

    fn eye_position(&self) -> Point3<f32> {
        Point3::from_vec(self.position + Vector3::new(0.0, PLAYER_EYE_HEIGHT, 0.0))
    }

    fn player_aabb(&self) -> AABB {
        AABB::new(
            self.position + Vector3::new(0.0, PLAYER_HALF_HEIGHT, 0.0),
            Vector3::new(PLAYER_RADIUS, PLAYER_HALF_HEIGHT, PLAYER_RADIUS),
        )
    }

    pub fn update_physics(&mut self, world: &crate::world::World, dt: f32) {
        // Detect water at player feet and mid-body
        let px = self.position.x.floor() as i32;
        let pz = self.position.z.floor() as i32;
        let foot_y = self.position.y as i32;
        let mid_y  = (self.position.y + PLAYER_HALF_HEIGHT) as i32;
        self.in_water =
            world.get_block(px, foot_y, pz) == crate::world::BlockType::Water
            || world.get_block(px, mid_y, pz) == crate::world::BlockType::Water;

        if self.in_water {
            // Buoyancy: much weaker gravity, strong terminal velocity clamp
            self.velocity.y -= 6.0 * dt;
            if self.velocity.y < -2.5 { self.velocity.y = -2.5; }
            // Water drag on all axes
            let drag = 0.07f32 * (dt * 60.0);
            self.velocity.x *= 1.0 - drag;
            self.velocity.z *= 1.0 - drag;
            self.velocity.y *= 1.0 - drag * 0.5;
        } else {
            self.velocity.y -= 32.0 * dt;
        }

        self.position.y += self.velocity.y * dt;
        self.on_ground = false;
        self.resolve_collisions(world, 1);

        self.position.x += self.velocity.x * dt;
        self.resolve_collisions(world, 0);

        self.position.z += self.velocity.z * dt;
        self.resolve_collisions(world, 2);

        self.velocity.x *= 0.8;
        self.velocity.z *= 0.8;
    }

    fn resolve_collisions(&mut self, world: &crate::world::World, axis: u8) {
        // Recompute AABB during checks so corrections immediately affect subsequent tests.
        for bx in (self.player_aabb().min.x.floor() as i32)..=(self.player_aabb().max.x.ceil() as i32) {
            for by in (self.player_aabb().min.y.floor() as i32)..=(self.player_aabb().max.y.ceil() as i32) {
                for bz in (self.player_aabb().min.z.floor() as i32)..=(self.player_aabb().max.z.ceil() as i32) {
                    let aabb = self.player_aabb();
                    if world.get_block(bx, by, bz).is_opaque() && aabb.intersects_block(bx, by, bz) {
                        match axis {
                            0 => {
                                // X axis penetration correction
                                if self.velocity.x > 0.0 {
                                    let penetration = aabb.max.x - (bx as f32);
                                    self.position.x -= penetration + 0.001;
                                } else {
                                    let penetration = (bx as f32 + 1.0) - aabb.min.x;
                                    self.position.x += penetration + 0.001;
                                }
                                self.velocity.x = 0.0;
                            }
                            1 => {
                                // Y axis (vertical) correction
                                if self.velocity.y > 0.0 {
                                    let penetration = aabb.max.y - (by as f32);
                                    self.position.y -= penetration + 0.001;
                                } else {
                                    let penetration = (by as f32 + 1.0) - aabb.min.y;
                                    self.position.y += penetration + 0.001;
                                    self.on_ground = true;
                                }
                                self.velocity.y = 0.0;
                            }
                            2 => {
                                // Z axis penetration correction
                                if self.velocity.z > 0.0 {
                                    let penetration = aabb.max.z - (bz as f32);
                                    self.position.z -= penetration + 0.001;
                                } else {
                                    let penetration = (bz as f32 + 1.0) - aabb.min.z;
                                    self.position.z += penetration + 0.001;
                                }
                                self.velocity.z = 0.0;
                            }
                            _ => {}
                        }
                    }
                }
            }
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
        let proj = perspective(Deg(90.0), aspect, CAMERA_NEAR_PLANE, 1000.0);
        let view = Matrix4::look_to_rh(
            camera.eye_position(),
            Vector3::new(camera.yaw.cos() * camera.pitch.cos(), camera.pitch.sin(), camera.yaw.sin() * camera.pitch.cos()).normalize(),
            Vector3::unit_y(),
        );
        self.view_proj = (proj * view).into();
    }
}