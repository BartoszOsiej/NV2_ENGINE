use std::collections::HashSet;
use winit::keyboard::KeyCode;
use winit::event::MouseButton;

#[derive(Default)]
pub struct InputState {
    pub keys_held: HashSet<KeyCode>,
    pub mouse_buttons_held: HashSet<MouseButton>,
    pub mouse_dx: f64,
    pub mouse_dy: f64,
}

impl InputState {
    pub fn handle_key(&mut self, key: KeyCode, pressed: bool) {
        if pressed {
            self.keys_held.insert(key);
        } else {
            self.keys_held.remove(&key);
        }
    }

    pub fn handle_mouse_button(&mut self, button: MouseButton, pressed: bool) {
        if pressed {
            self.mouse_buttons_held.insert(button);
        } else {
            self.mouse_buttons_held.remove(&button);
        }
    }

    pub fn accumulate_mouse(&mut self, dx: f64, dy: f64) {
        self.mouse_dx += dx;
        self.mouse_dy += dy;
    }

    pub fn clear_mouse(&mut self) {
        self.mouse_dx = 0.0;
        self.mouse_dy = 0.0;
    }
}