use cgmath::{Deg, EuclideanSpace, InnerSpace, Matrix4, Point3, Vector3, perspective};
use crate::input::InputState;
use crate::world::block::{MovementMedium, MovementMediumKind};
use crate::world::{self, BlockType};
use winit::keyboard::KeyCode;

const PLAYER_HEIGHT:      f32 = 1.8;
const PLAYER_HALF_HEIGHT: f32 = PLAYER_HEIGHT * 0.5;
const PLAYER_EYE_HEIGHT:  f32 = 1.7;
const PLAYER_RADIUS:      f32 = 0.3;
const CAMERA_NEAR_PLANE:  f32 = 0.05;
const WALK_SPEED:         f32 = 5.5;
const FLY_SPEED:          f32 = 9.0;
const FLY_VERTICAL_SPEED: f32 = 8.5;
const SPRINT_MULTIPLIER:  f32 = 1.75;
const JUMP_SPEED:         f32 = 8.0;
const GRAVITY:            f32 = 20.0;
const WATER_GRAVITY:      f32 = 6.0;
const WATER_SINK_CAP:     f32 = 2.0;
const MOUSE_SENSITIVITY:  f32 = 0.002;
const MAX_FALL_SPEED:     f32 = 32.0;
const GROUND_ACCELERATION:f32 = 8.0;
const MEDIUM_SPEED_SETTLE:f32 = 12.0;

#[derive(Clone, Copy, Debug)]
pub struct MovementTuning {
    pub walk_speed: f32,
    pub run_speed: f32,
    pub air_control: f32,
    pub jump_force: f32,
    pub friction_ground: f32,
    pub friction_air: f32,
}

impl Default for MovementTuning {
    fn default() -> Self {
        Self {
            walk_speed: WALK_SPEED,
            run_speed: WALK_SPEED * SPRINT_MULTIPLIER,
            air_control: 0.35,
            jump_force: JUMP_SPEED,
            friction_ground: 14.0,
            friction_air: 1.75,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ActiveMovementModifiers {
    pub speed_multiplier: f32,
    pub sprint_multiplier: f32,
    pub fall_multiplier: f32,
    pub sound_dampening: f32,
}

impl Default for ActiveMovementModifiers {
    fn default() -> Self {
        Self {
            speed_multiplier: 1.0,
            sprint_multiplier: 1.0,
            fall_multiplier: 1.0,
            sound_dampening: 1.0,
        }
    }
}

impl ActiveMovementModifiers {
    fn apply_medium(&mut self, medium: MovementMedium) {
        self.speed_multiplier *= medium.movement_speed_multiplier;
        self.sprint_multiplier *= medium.sprint_speed_multiplier;
        self.fall_multiplier *= medium.fall_speed_multiplier;
        self.sound_dampening *= medium.sound_dampening;
    }

    fn clamp(self) -> Self {
        Self {
            speed_multiplier: self.speed_multiplier.max(0.05),
            sprint_multiplier: self.sprint_multiplier.max(0.05),
            fall_multiplier: self.fall_multiplier.max(0.05),
            sound_dampening: self.sound_dampening.max(0.05),
        }
    }
}

#[derive(Clone, Copy, Debug)]
struct MovementIntent {
    move_dir: Vector3<f32>,
    sprinting: bool,
    jump_pressed: bool,
}

impl Default for MovementIntent {
    fn default() -> Self {
        Self {
            move_dir: Vector3::new(0.0, 0.0, 0.0),
            sprinting: false,
            jump_pressed: false,
        }
    }
}

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
    pub movement:  MovementTuning,
    pub movement_modifiers: ActiveMovementModifiers,
    pub on_ground: bool,
    pub in_water:  bool,
    pub in_foliage_medium: bool,
    pub footstep_volume: f32,
    pub is_flying: bool,
    bob_phase:     f32,
    bob_offset:    f32,
    intent:        MovementIntent,
}

impl Camera {
    pub fn new(position: Vector3<f32>) -> Self {
        Self {
            position,
            velocity:  Vector3::new(0.0, 0.0, 0.0),
            yaw:       -90.0_f32.to_radians(),
            pitch:     0.0,
            movement:  MovementTuning::default(),
            movement_modifiers: ActiveMovementModifiers::default(),
            on_ground: false,
            in_water:  false,
            in_foliage_medium: false,
            footstep_volume: 1.0,
            is_flying: false,
            bob_phase:  0.0,
            bob_offset: 0.0,
            intent: MovementIntent::default(),
        }
    }

    pub fn tick_movement(&mut self, world: &world::World, input: &InputState, dt: f32) {
        self.capture_input_intent(input);
        self.integrate_movement(world, dt);
    }

    pub fn look_direction(&self) -> Vector3<f32> {
        Vector3::new(
            self.yaw.cos() * self.pitch.cos(),
            self.pitch.sin(),
            self.yaw.sin() * self.pitch.cos(),
        )
        .normalize()
    }

    pub fn interaction_origin(&self) -> Vector3<f32> {
        self.eye_position().to_vec()
    }

    pub fn player_bounds(&self) -> AABB {
        self.player_aabb()
    }

    pub fn toggle_flight(&mut self) -> bool {
        self.is_flying = !self.is_flying;
        self.velocity = Vector3::new(0.0, 0.0, 0.0);
        self.on_ground = false;
        self.in_water = false;
        self.in_foliage_medium = false;
        self.footstep_volume = 1.0;
        self.movement_modifiers = ActiveMovementModifiers::default();
        self.bob_phase = 0.0;
        self.bob_offset = 0.0;
        self.intent = MovementIntent::default();
        self.is_flying
    }

    fn capture_input_intent(&mut self, input: &InputState) {
        let forward = Vector3::new(self.yaw.cos(), 0.0, self.yaw.sin()).normalize();
        let right   = Vector3::new(-self.yaw.sin(), 0.0, self.yaw.cos()).normalize();
        let sprinting =
            input.keys_held.contains(&KeyCode::ShiftLeft)
            || input.keys_held.contains(&KeyCode::ShiftRight);

        self.yaw   += input.mouse_dx as f32 * MOUSE_SENSITIVITY;
        self.pitch -= input.mouse_dy as f32 * MOUSE_SENSITIVITY;
        self.pitch  = self.pitch.clamp(-1.5, 1.5);

        if self.is_flying {
            let mut planar_dir = Vector3::new(0.0_f32, 0.0, 0.0);
            if input.keys_held.contains(&KeyCode::KeyW) { planar_dir += forward; }
            if input.keys_held.contains(&KeyCode::KeyS) { planar_dir -= forward; }
            if input.keys_held.contains(&KeyCode::KeyA) { planar_dir -= right; }
            if input.keys_held.contains(&KeyCode::KeyD) { planar_dir += right; }

            let mut vertical = 0.0_f32;
            if input.keys_held.contains(&KeyCode::Space) { vertical += 1.0; }
            if input.keys_held.contains(&KeyCode::ControlLeft) || input.keys_held.contains(&KeyCode::ControlRight) {
                vertical -= 1.0;
            }

            let speed = if sprinting {
                SPRINT_MULTIPLIER
            } else {
                1.0
            };

            if planar_dir.magnitude2() > 0.0 {
                let planar_vel = planar_dir.normalize() * (FLY_SPEED * speed);
                self.velocity.x = planar_vel.x;
                self.velocity.z = planar_vel.z;
            } else {
                self.velocity.x = 0.0;
                self.velocity.z = 0.0;
            }
            self.velocity.y = vertical * FLY_VERTICAL_SPEED * speed;
            self.intent = MovementIntent::default();
            self.bob_phase = 0.0;
            self.bob_offset = 0.0;
            return;
        }

        let mut move_dir = Vector3::new(0.0_f32, 0.0, 0.0);
        if input.keys_held.contains(&KeyCode::KeyW) { move_dir += forward; }
        if input.keys_held.contains(&KeyCode::KeyS) { move_dir -= forward; }
        if input.keys_held.contains(&KeyCode::KeyA) { move_dir -= right; }
        if input.keys_held.contains(&KeyCode::KeyD) { move_dir += right; }

        if move_dir.magnitude2() > 0.0 {
            move_dir = move_dir.normalize();
        }

        self.intent = MovementIntent {
            move_dir,
            sprinting,
            jump_pressed: input.keys_held.contains(&KeyCode::Space),
        };
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

    fn integrate_movement(&mut self, world: &world::World, dt: f32) {
        if self.is_flying {
            self.position += self.velocity * dt;
            self.on_ground = false;
            self.in_water = false;
            self.in_foliage_medium = false;
            self.footstep_volume = 1.0;
            self.movement_modifiers = ActiveMovementModifiers::default();
            return;
        }

        self.refresh_environment_state(world);
        self.apply_horizontal_friction(dt);

        let target_speed = self.target_move_speed();
        self.accelerate_planar(target_speed, dt);
        self.settle_planar_speed(target_speed, dt);

        if self.intent.jump_pressed {
            if self.on_ground {
                self.velocity.y = self.movement.jump_force;
                self.on_ground = false;
            } else if self.in_water {
                self.velocity.y = 3.0;
            }
        }

        if self.in_water {
            self.velocity.y -= WATER_GRAVITY * dt;
            self.velocity.y  = self.velocity.y.max(-WATER_SINK_CAP);
            let drag_h = 0.93_f32.powf(dt * 60.0);
            let drag_v = 0.965_f32.powf(dt * 60.0);
            self.velocity.x *= drag_h;
            self.velocity.z *= drag_h;
            self.velocity.y *= drag_v;
        } else {
            let gravity = if self.velocity.y < 0.0 {
                GRAVITY * self.movement_modifiers.fall_multiplier
            } else {
                GRAVITY
            };
            self.velocity.y -= gravity * dt;

            let fall_speed_cap = (MAX_FALL_SPEED * self.movement_modifiers.fall_multiplier).max(1.0);
            self.velocity.y = self.velocity.y.max(-fall_speed_cap);
        }

        self.position.y += self.velocity.y * dt;
        self.on_ground   = false;
        self.resolve_collisions(world, 1);

        self.position.x += self.velocity.x * dt;
        self.resolve_collisions(world, 0);

        self.position.z += self.velocity.z * dt;
        self.resolve_collisions(world, 2);

        self.refresh_environment_state(world);
        self.update_head_bob(dt);
    }

    fn refresh_environment_state(&mut self, world: &world::World) {
        let (movement_modifiers, in_foliage_medium) = self.sample_movement_modifiers(world);
        self.movement_modifiers = movement_modifiers;
        self.in_foliage_medium = in_foliage_medium;
        self.footstep_volume = movement_modifiers.sound_dampening;
        self.in_water = self.intersects_block_matching(world, |block| block == BlockType::Water);
    }

    fn sample_movement_modifiers(&self, world: &world::World) -> (ActiveMovementModifiers, bool) {
        let aabb = self.player_aabb();
        let x_range = aabb.min.x.floor() as i32 ..= aabb.max.x.ceil() as i32;
        let y_range = aabb.min.y.floor() as i32 ..= aabb.max.y.ceil() as i32;
        let z_range = aabb.min.z.floor() as i32 ..= aabb.max.z.ceil() as i32;

        let mut modifiers = ActiveMovementModifiers::default();
        let mut applied_kinds = Vec::new();
        let mut in_foliage_medium = false;

        for bx in x_range {
            for by in y_range.clone() {
                for bz in z_range.clone() {
                    let block = world.get_block(bx, by, bz);
                    let Some(medium) = block.movement_medium() else { continue; };
                    if !aabb.intersects_block(bx, by, bz) {
                        continue;
                    }

                    if medium.kind == MovementMediumKind::Foliage {
                        in_foliage_medium = true;
                    }

                    if applied_kinds.contains(&medium.kind) {
                        continue;
                    }

                    modifiers.apply_medium(medium);
                    applied_kinds.push(medium.kind);
                }
            }
        }

        (modifiers.clamp(), in_foliage_medium)
    }

    fn intersects_block_matching<F>(&self, world: &world::World, mut predicate: F) -> bool
    where
        F: FnMut(BlockType) -> bool,
    {
        let aabb = self.player_aabb();
        let x_range = aabb.min.x.floor() as i32 ..= aabb.max.x.ceil() as i32;
        let y_range = aabb.min.y.floor() as i32 ..= aabb.max.y.ceil() as i32;
        let z_range = aabb.min.z.floor() as i32 ..= aabb.max.z.ceil() as i32;

        for bx in x_range {
            for by in y_range.clone() {
                for bz in z_range.clone() {
                    let block = world.get_block(bx, by, bz);
                    if predicate(block) && aabb.intersects_block(bx, by, bz) {
                        return true;
                    }
                }
            }
        }

        false
    }

    fn apply_horizontal_friction(&mut self, dt: f32) {
        let friction = if self.on_ground {
            self.movement.friction_ground
        } else {
            self.movement.friction_air
        };
        let decay = (-friction * dt).exp();

        self.velocity.x *= decay;
        self.velocity.z *= decay;

        if self.velocity.x.abs() < 0.0005 {
            self.velocity.x = 0.0;
        }
        if self.velocity.z.abs() < 0.0005 {
            self.velocity.z = 0.0;
        }
    }

    fn target_move_speed(&self) -> f32 {
        if self.intent.move_dir.magnitude2() == 0.0 {
            return 0.0;
        }

        let mut speed = if self.intent.sprinting {
            self.movement.run_speed
        } else {
            self.movement.walk_speed
        };

        speed *= self.movement_modifiers.speed_multiplier;
        if self.intent.sprinting {
            speed *= self.movement_modifiers.sprint_multiplier;
        }

        speed
    }

    fn accelerate_planar(&mut self, target_speed: f32, dt: f32) {
        if target_speed <= 0.0 || self.intent.move_dir.magnitude2() == 0.0 {
            return;
        }

        let current_speed = self.velocity.x * self.intent.move_dir.x + self.velocity.z * self.intent.move_dir.z;
        let add_speed = target_speed - current_speed;
        if add_speed <= 0.0 {
            return;
        }

        let acceleration = if self.on_ground {
            GROUND_ACCELERATION
        } else {
            GROUND_ACCELERATION * self.movement.air_control.max(0.0)
        };
        let accel_speed = (acceleration * dt * target_speed).min(add_speed);

        self.velocity.x += self.intent.move_dir.x * accel_speed;
        self.velocity.z += self.intent.move_dir.z * accel_speed;
    }

    fn settle_planar_speed(&mut self, target_speed: f32, dt: f32) {
        if target_speed <= 0.0 || (!self.on_ground && !self.in_foliage_medium) {
            return;
        }

        let planar_speed = self.planar_speed();
        if planar_speed <= target_speed {
            return;
        }

        let reduction = 1.0 - (-MEDIUM_SPEED_SETTLE * dt).exp();
        let new_speed = planar_speed - (planar_speed - target_speed) * reduction;
        let scale = new_speed / planar_speed;

        self.velocity.x *= scale;
        self.velocity.z *= scale;
    }

    fn planar_speed(&self) -> f32 {
        Vector3::new(self.velocity.x, 0.0, self.velocity.z).magnitude()
    }

    fn update_head_bob(&mut self, dt: f32) {
        let planar_speed = self.planar_speed();
        let walking = planar_speed > 0.1 && self.on_ground && !self.in_water;

        if walking {
            let cadence = 7.5 + planar_speed * 0.55;
            self.bob_phase = (self.bob_phase + dt * cadence) % std::f32::consts::TAU;
        } else {
            self.bob_phase *= (1.0 - dt * 8.0).max(0.0);
        }

        let bob_strength = (planar_speed / self.movement.run_speed.max(1.0)).clamp(0.0, 1.0);
        self.bob_offset = self.bob_phase.sin() * 0.035 * bob_strength;
    }

    fn resolve_collisions(&mut self, world: &world::World, axis: u8) {
        let initial = self.player_aabb();
        let x_range = initial.min.x.floor() as i32 ..= initial.max.x.ceil() as i32;
        let y_range = initial.min.y.floor() as i32 ..= initial.max.y.ceil() as i32;
        let z_range = initial.min.z.floor() as i32 ..= initial.max.z.ceil() as i32;

        for bx in x_range {
            for by in y_range.clone() {
                for bz in z_range.clone() {
                    if !world.get_block(bx, by, bz).is_solid() { continue; }

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
        let look_dir = camera.look_direction();

        let proj = perspective(Deg(90.0), aspect, CAMERA_NEAR_PLANE, 1000.0);
        let view = Matrix4::look_to_rh(camera.eye_position(), look_dir, Vector3::unit_y());
        self.view_proj = (proj * view).into();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::World;

    #[test]
    fn foliage_medium_updates_player_modifiers() {
        let mut world = World::new(7);
        world.set_block(0, 200, 0, BlockType::TreeLeaves);

        let mut camera = Camera::new(Vector3::new(0.5, 199.35, 0.5));
        camera.refresh_environment_state(&world);

        assert!(camera.in_foliage_medium);
        assert_eq!(camera.movement_modifiers.speed_multiplier, 0.55);
        assert_eq!(camera.movement_modifiers.sprint_multiplier, 0.65);
        assert_eq!(camera.movement_modifiers.fall_multiplier, 0.35);
        assert_eq!(camera.footstep_volume, 0.6);
        assert!(!camera.on_ground);
    }

    #[test]
    fn grounded_movement_accelerates_instead_of_snapping() {
        let mut world = World::new(11);
        world.set_block(0, 199, 0, BlockType::Stone);

        let mut camera = Camera::new(Vector3::new(0.5, 200.0, 0.5));
        camera.on_ground = true;

        let mut input = InputState::default();
        input.handle_key(KeyCode::KeyW, true);

        camera.tick_movement(&world, &input, 0.1);
        let first_speed = camera.planar_speed();

        assert!(first_speed > 0.0);
        assert!(first_speed < camera.movement.walk_speed);
        assert!(camera.on_ground);

        camera.tick_movement(&world, &input, 0.1);
        let second_speed = camera.planar_speed();

        assert!(second_speed > first_speed);
        assert!(second_speed <= camera.movement.walk_speed + 0.1);
    }

    #[test]
    fn falling_through_foliage_stays_ungrounded_and_slows() {
        let mut world = World::new(23);
        world.set_block(0, 198, 0, BlockType::NeedleCanopy);
        world.set_block(0, 199, 0, BlockType::NeedleCanopy);
        world.set_block(0, 200, 0, BlockType::NeedleCanopy);

        let mut camera = Camera::new(Vector3::new(0.5, 199.2, 0.5));
        camera.velocity.y = -20.0;

        camera.tick_movement(&world, &InputState::default(), 0.05);

        assert!(camera.in_foliage_medium);
        assert!(!camera.on_ground);
        assert!(camera.velocity.y > -20.0);
    }
}