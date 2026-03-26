use cgmath::{Matrix4, Vector3, Point3, Deg, perspective, SquareMatrix, InnerSpace};
use std::collections::HashSet;
use winit::keyboard::KeyCode;

#[repr(C)]
#[derive(Debug, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct CameraUniform {
    view_proj: [[f32; 4]; 4],
}

impl CameraUniform {
    pub fn new() -> Self {
        Self {
            view_proj: Matrix4::identity().into(),
        }
    }

    pub fn update_view_proj(&mut self, camera: &Camera) {
        let view = Matrix4::look_at_rh(
            Point3::new(camera.position.x, camera.position.y, camera.position.z),
            Point3::new(
                camera.position.x + camera.yaw.sin() * camera.pitch.cos(),
                camera.position.y + camera.pitch.sin(),
                camera.position.z + camera.yaw.cos() * camera.pitch.cos(),
            ),
            Vector3::unit_y(),
        );
        let proj = perspective(Deg(45.0), camera.aspect, 0.1, 1000.0);
        self.view_proj = (proj * view).into();
    }
}

pub struct Camera {
    pub position: Vector3<f32>,
    pub yaw: f32,
    pub pitch: f32,
    pub aspect: f32,
    pub speed: f32,
    pub velocity: Vector3<f32>,
    pub on_ground: bool,
    pub jump_force: f32,
}

impl Camera {
    pub fn new(aspect: f32) -> Self {
        Self {
            position: Vector3::new(0.0, 120.0, 0.0),
            yaw: 0.0,
            pitch: 0.0,
            aspect,
            speed: 4.0, // walking speed
            velocity: Vector3::new(0.0, 0.0, 0.0),
            on_ground: false,
            jump_force: 8.0,
        }
    }

    pub fn process_keys(&mut self, keys: &HashSet<KeyCode>, dt: f32) {
        let mut forward = 0.0;
        let mut right = 0.0;
        let sprint = keys.contains(&KeyCode::ShiftLeft);

        if keys.contains(&KeyCode::KeyW) { forward += 1.0; }
        if keys.contains(&KeyCode::KeyS) { forward -= 1.0; }
        if keys.contains(&KeyCode::KeyA) { right -= 1.0; }
        if keys.contains(&KeyCode::KeyD) { right += 1.0; }

        let speed = if sprint { self.speed * 2.0 } else { self.speed };

        let mut dir = Vector3::new(
            self.yaw.sin() * forward + self.yaw.cos() * right,
            0.0, // horizontal movement only
            self.yaw.cos() * forward - self.yaw.sin() * right,
        );

        if dir.magnitude2() > 0.0 {
            dir = dir.normalize() * speed;
            self.velocity.x = dir.x;
            self.velocity.z = dir.z;
        } else {
            self.velocity.x = 0.0;
            self.velocity.z = 0.0;
        }

        // Jump
        if keys.contains(&KeyCode::Space) && self.on_ground {
            self.velocity.y = self.jump_force;
            self.on_ground = false;
        }
    }

    pub fn process_mouse(&mut self, dx: f32, dy: f32) {
        let sensitivity = 0.0025;
        self.yaw += dx * sensitivity;
        self.pitch -= dy * sensitivity;
        self.pitch = self.pitch.clamp(-1.55, 1.55);
    }

    pub fn update_physics(&mut self, dt: f32, world: &crate::world::World) {
        // Gravity
        if !self.on_ground {
            self.velocity.y -= 20.0 * dt; // gravity
        }

        // Apply velocity
        let new_pos = self.position + self.velocity * dt;

        // Collision detection
        let player_height = 1.8;
        let player_width = 0.6;

        // Check X movement
        let test_pos_x = Vector3::new(new_pos.x, self.position.y, self.position.z);
        if !self.collides_with_world(&test_pos_x, player_width, player_height, world) {
            self.position.x = test_pos_x.x;
        } else {
            self.velocity.x = 0.0;
        }

        // Check Z movement
        let test_pos_z = Vector3::new(self.position.x, self.position.y, new_pos.z);
        if !self.collides_with_world(&test_pos_z, player_width, player_height, world) {
            self.position.z = test_pos_z.z;
        } else {
            self.velocity.z = 0.0;
        }

        // Check Y movement
        let test_pos_y = Vector3::new(self.position.x, new_pos.y, self.position.z);
        if !self.collides_with_world(&test_pos_y, player_width, player_height, world) {
            self.position.y = test_pos_y.y;
            self.on_ground = false;
        } else {
            if self.velocity.y < 0.0 {
                self.on_ground = true;
            }
            self.velocity.y = 0.0;
        }

        // Prevent falling below world
        if self.position.y < 0.0 {
            self.position.y = 0.0;
            self.velocity.y = 0.0;
            self.on_ground = true;
        }
    }

    fn collides_with_world(&self, pos: &Vector3<f32>, width: f32, height: f32, world: &crate::world::World) -> bool {
        let half_width = width / 2.0;
        let corners = [
            (pos.x - half_width, pos.z - half_width),
            (pos.x + half_width, pos.z - half_width),
            (pos.x - half_width, pos.z + half_width),
            (pos.x + half_width, pos.z + half_width),
        ];

        for y in 0..(height as i32 + 1) {
            for (x, z) in corners.iter() {
                let wx = *x as i32;
                let wy = (pos.y as i32) + y;
                let wz = *z as i32;
                if world.get_block(wx, wy, wz).is_opaque() {
                    return true;
                }
            }
        }
        false
    }
}