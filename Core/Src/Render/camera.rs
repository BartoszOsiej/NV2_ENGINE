use cgmath::*;
use winit::keyboard::KeyCode;
use std::collections::HashSet;

pub struct Camera {
    pub position: Point3<f32>,
    pub yaw:      f32, // radians
    pub pitch:    f32, // radians
    pub aspect:   f32,
    pub fovy:     f32,
    pub znear:    f32,
    pub zfar:     f32,
}

impl Camera {
    pub fn new(aspect: f32) -> Self {
        Self {
            position: Point3::new(8.0, 80.0, 8.0),
            yaw:   -std::f32::consts::FRAC_PI_2,
            pitch: -0.4,
            aspect,
            fovy:  70.0,
            znear: 0.05,
            zfar:  800.0,
        }
    }

    pub fn forward(&self) -> Vector3<f32> {
        Vector3::new(
            self.yaw.cos() * self.pitch.cos(),
            self.pitch.sin(),
            self.yaw.sin() * self.pitch.cos(),
        ).normalize()
    }

    pub fn right(&self) -> Vector3<f32> {
        self.forward().cross(Vector3::unit_y()).normalize()
    }

    pub fn build_view_proj(&self) -> Matrix4<f32> {
        let target = self.position + self.forward();
        let view   = Matrix4::look_at_rh(self.position, target, Vector3::unit_y());
        let proj   = perspective(Deg(self.fovy), self.aspect, self.znear, self.zfar);
        proj * view
    }

    pub fn process_mouse(&mut self, dx: f64, dy: f64, sensitivity: f32) {
        self.yaw   += dx as f32 * sensitivity;
        self.pitch -= dy as f32 * sensitivity;
        self.pitch  = self.pitch.clamp(-1.55, 1.55);
    }

    pub fn process_keys(&mut self, keys: &HashSet<KeyCode>, dt: f32) {
        let speed   = if keys.contains(&KeyCode::ShiftLeft) { 20.0 } else { 8.0 };
        let forward = self.forward();
        let right   = self.right();
        let flat_fwd = Vector3::new(forward.x, 0.0, forward.z).normalize();
        let flat_rgt = Vector3::new(right.x,   0.0, right.z  ).normalize();

        if keys.contains(&KeyCode::KeyW) { self.position += flat_fwd * speed * dt; }
        if keys.contains(&KeyCode::KeyS) { self.position -= flat_fwd * speed * dt; }
        if keys.contains(&KeyCode::KeyA) { self.position -= flat_rgt * speed * dt; }
        if keys.contains(&KeyCode::KeyD) { self.position += flat_rgt * speed * dt; }
        if keys.contains(&KeyCode::Space) {
            self.position.y += speed * dt;
        }
        if keys.contains(&KeyCode::ControlLeft) {
            self.position.y -= speed * dt;
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
        Self { view_proj: Matrix4::identity().into() }
    }
    pub fn update(&mut self, camera: &Camera) {
        self.view_proj = camera.build_view_proj().into();
    }
}