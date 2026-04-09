use std::collections::HashSet;
use winit::event::{MouseButton, MouseScrollDelta};
use winit::keyboard::KeyCode;

#[derive(Default)]
pub struct InputState {
    pub keys_held: HashSet<KeyCode>,
    pub keys_pressed: HashSet<KeyCode>,
    pub mouse_buttons_held: HashSet<MouseButton>,
    pub mouse_buttons_pressed: HashSet<MouseButton>,
    pub mouse_buttons_released: HashSet<MouseButton>,
    pub mouse_dx: f64,
    pub mouse_dy: f64,
    pub scroll_lines: f32,
    pub cursor_position: Option<(f32, f32)>,
}

impl InputState {
    pub fn handle_key(&mut self, key: KeyCode, pressed: bool) {
        if pressed {
            if self.keys_held.insert(key) {
                self.keys_pressed.insert(key);
            }
        } else {
            self.keys_held.remove(&key);
        }
    }

    pub fn handle_mouse_button(&mut self, button: MouseButton, pressed: bool) {
        if pressed {
            if self.mouse_buttons_held.insert(button) {
                self.mouse_buttons_pressed.insert(button);
            }
        } else {
            if self.mouse_buttons_held.remove(&button) {
                self.mouse_buttons_released.insert(button);
            }
        }
    }

    pub fn accumulate_mouse(&mut self, dx: f64, dy: f64) {
        self.mouse_dx += dx;
        self.mouse_dy += dy;
    }

    pub fn set_cursor_position(&mut self, x: f32, y: f32) {
        self.cursor_position = Some((x, y));
    }

    pub fn accumulate_scroll(&mut self, delta: MouseScrollDelta) {
        match delta {
            MouseScrollDelta::LineDelta(_, y) => {
                self.scroll_lines += y;
            }
            MouseScrollDelta::PixelDelta(position) => {
                self.scroll_lines += (position.y as f32 / 40.0).clamp(-4.0, 4.0);
            }
        }
    }

    pub fn was_key_pressed(&self, key: KeyCode) -> bool {
        self.keys_pressed.contains(&key)
    }

    pub fn is_mouse_held(&self, button: MouseButton) -> bool {
        self.mouse_buttons_held.contains(&button)
    }

    pub fn was_mouse_pressed(&self, button: MouseButton) -> bool {
        self.mouse_buttons_pressed.contains(&button)
    }

    pub fn was_mouse_released(&self, button: MouseButton) -> bool {
        self.mouse_buttons_released.contains(&button)
    }

    pub fn clear_frame(&mut self) {
        self.keys_pressed.clear();
        self.mouse_buttons_pressed.clear();
        self.mouse_buttons_released.clear();
        self.mouse_dx = 0.0;
        self.mouse_dy = 0.0;
        self.scroll_lines = 0.0;
    }
}