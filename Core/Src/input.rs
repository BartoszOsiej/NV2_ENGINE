use std::collections::HashSet;
use winit::keyboard::KeyCode;

#[derive(Default)]
pub struct InputState {
    pub keys_held: HashSet<KeyCode>,
    pub mouse_dx:  f64,
    pub mouse_dy:  f64,
    pub mouse_captured: bool,
}

impl InputState {
    pub fn key_down(&mut self, key: KeyCode) { self.keys_held.insert(key); }
    pub fn key_up(&mut self, key: KeyCode)   { self.keys_held.remove(&key); }

    pub fn accumulate_mouse(&mut self, dx: f64, dy: f64) {
        self.mouse_dx += dx;
        self.mouse_dy += dy;
    }

    /// Drain mouse delta (call once per frame)
    pub fn take_mouse(&mut self) -> (f64, f64) {
        let d = (self.mouse_dx, self.mouse_dy);
        self.mouse_dx = 0.0;
        self.mouse_dy = 0.0;
        d
    }
}