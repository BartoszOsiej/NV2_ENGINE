#![allow(unused)]

use std::env;
use std::path::PathBuf;
use std::time::{Instant, SystemTime, UNIX_EPOCH};
use cgmath::Vector3;
use winit::{
    application::ApplicationHandler,
    event::{DeviceEvent, DeviceId, WindowEvent},
    event_loop::{ActiveEventLoop, EventLoop},
    keyboard::{KeyCode, PhysicalKey},
    window::{CursorGrabMode, Window, WindowId},
};

mod commands;
mod crafting;
mod interaction;
mod inventory;
mod renderer;
mod world;
mod input;
mod assets;

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
enum AppMode {
    MainMenu,
    Playing,
    PauseMenu,
}

const MAIN_MENU_ITEMS: [&str; 3] = ["New Game", "Load/Save", "Quit"];
const PAUSE_MENU_ITEMS: [&str; 4] = ["Resume", "Save", "Save + Exit", "Exit"];

struct App {
    state:        Option<renderer::State>,
    world:        world::World,
    input:        input::InputState,
    mode:         AppMode,
    save_path:    PathBuf,
    status_message: String,
    command_input: Option<String>,
    main_menu_selection: usize,
    pause_menu_selection: usize,
    last_frame:   Instant,
}

impl App {
    fn format_command_prompt(buffer: &str) -> String {
        format!("/{}", buffer)
    }

    fn is_command_prompt_trigger(key: KeyCode, text: Option<&str>) -> bool {
        matches!(key, KeyCode::Slash | KeyCode::NumpadDivide)
            || text.is_some_and(|value| value.contains('/'))
    }

    fn default_save_path() -> PathBuf {
        let exe_dir = env::current_exe()
            .ok()
            .and_then(|path| path.parent().map(|p| p.to_path_buf()))
            .unwrap_or_else(|| env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
        exe_dir.join("saves").join("world.json")
    }

    fn title_text(&self) -> String {
        let mode_label = match self.mode {
            AppMode::MainMenu => "Main Menu",
            AppMode::Playing => "Playing",
            AppMode::PauseMenu => "Paused",
        };
        format!("NV2 Engine | {}", mode_label)
    }

    fn update_window_title(&self) {
        if let Some(state) = self.state.as_ref() {
            state.window.set_title(&self.title_text());
        }
    }

    fn set_status_message<S: Into<String>>(&mut self, message: S) {
        self.status_message = message.into();
    }

    fn show_console_message<S: Into<String>>(&mut self, message: S) {
        let message = message.into();
        println!("{}", message);
        self.status_message = message.clone();
        if let Some(state) = self.state.as_mut() {
            state.show_subtitle(&message);
        }
    }

    fn refresh_command_prompt(&mut self) {
        if let Some(buffer) = self.command_input.as_ref() {
            let prompt = Self::format_command_prompt(buffer);
            if let Some(state) = self.state.as_mut() {
                state.show_command_prompt(&prompt);
            }
        }
    }

    fn begin_command_input(&mut self) {
        self.command_input = Some(String::new());
        self.input = input::InputState::default();
        if let Some(state) = self.state.as_mut() {
            state.input_captured = false;
            state.window.set_cursor_visible(true);
            let _ = state.window.set_cursor_grab(CursorGrabMode::None);
        }
        self.refresh_command_prompt();
    }

    fn cancel_command_input(&mut self) {
        self.command_input = None;
        self.input = input::InputState::default();
        if let Some(state) = self.state.as_mut() {
            state.input_captured = true;
            state.window.set_cursor_visible(false);
            let _ = state.window.set_cursor_grab(CursorGrabMode::Locked);
            state.clear_command_prompt();
        }
        self.show_console_message("Command entry cancelled.");
    }

    fn execute_command_input(&mut self) {
        let command_text = match self.command_input.take() {
            Some(command) => command,
            None => return,
        };
        let command = format!("/{}", command_text);
        self.input = input::InputState::default();

        let player_origin = self
            .state
            .as_ref()
            .map(|state| {
                (
                    state.camera.position.x,
                    state.camera.position.y,
                    state.camera.position.z,
                )
            })
            .unwrap_or((0.0, 80.0, 0.0));

        let result = commands::execute(&mut self.world, player_origin, &command);

        if let Some(state) = self.state.as_mut() {
            state.input_captured = true;
            state.window.set_cursor_visible(false);
            let _ = state.window.set_cursor_grab(CursorGrabMode::Locked);
            state.clear_command_prompt();
        }

        match result {
            Ok(output) => {
                if let Some(target) = output.teleport_target {
                    self.apply_teleport(target);
                }
                self.show_console_message(output.message);
            }
            Err(error) => {
                self.show_console_message(error);
            }
        }
    }

    fn apply_teleport(&mut self, target: (f32, f32, f32)) {
        if let Some(state) = self.state.as_mut() {
            state.camera.position = Vector3::new(target.0, target.1, target.2);
            state.camera.velocity = Vector3::new(0.0, 0.0, 0.0);
            state.camera.on_ground = false;
            state.camera.in_water = false;
            state.camera_uniform.update_view_proj(
                &state.camera,
                state.config.width as f32 / state.config.height as f32,
            );
            state.queue.write_buffer(
                &state.camera_buffer,
                0,
                bytemuck::cast_slice(&[state.camera_uniform]),
            );
        }
    }

    fn reset_game_context(&mut self) {
        self.input = input::InputState::default();
        self.command_input = None;
        self.main_menu_selection = 0;
        self.pause_menu_selection = 0;
        if let Some(state) = self.state.as_mut() {
            state.reset_for_new_world();
            state.clear_command_prompt();
        }
    }

    fn new() -> Self {
        // Derive a well-distributed seed from wall-clock time.
        // Multiply+XOR folds both secs and nanos into all 32 output bits so
        // seeds remain distinct even when calls happen in the same second.
        let now  = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();
        let seed = (now.as_secs() as u32)
            .wrapping_mul(1_664_525)
            .wrapping_add(now.subsec_nanos())
            .wrapping_mul(1_013_904_223);
        Self {
            state:          None,
            world:          world::World::new(seed),
            input:          input::InputState::default(),
            mode:           AppMode::MainMenu,
            save_path:      Self::default_save_path(),
            status_message: String::from("Use Up/Down to choose, Enter to activate."),
            command_input:  None,
            main_menu_selection: 0,
            pause_menu_selection: 0,
            last_frame:     Instant::now(),
        }
    }

    fn start_new_game(&mut self) {
        self.reset_game_context();
        // Generate a well-distributed seed — different every call.
        let now  = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();
        let seed = (now.as_secs() as u32)
            .wrapping_mul(1_664_525)
            .wrapping_add(now.subsec_nanos())
            .wrapping_mul(1_013_904_223);
        self.world = world::World::new(seed);
        self.mode = AppMode::Playing;
        self.set_status_message("New game started. Press Esc for the pause menu.");
        self.update_window_title();
        let spawn = self.world.find_spawn_point();
        // If renderer is initialized, move the camera to the surface of the new world
        if let Some(state) = self.state.as_mut() {
            let (sx, sy, sz) = spawn;
            state.camera.position = Vector3::new(sx, sy, sz);
            state.camera.velocity = Vector3::new(0.0, 0.0, 0.0);
            state.camera.on_ground = true;
            state.camera_uniform.update_view_proj(&state.camera, state.config.width as f32 / state.config.height as f32);
            state.queue.write_buffer(&state.camera_buffer, 0, bytemuck::cast_slice(&[state.camera_uniform]));
        }
    }

    fn save_game(&mut self) {
        match self.world.save_to_file(&self.save_path) {
            Ok(_) => self.set_status_message(format!("World saved to {}", self.save_path.display())),
            Err(err) => self.set_status_message(format!("Save failed: {}", err)),
        }
        self.update_window_title();
    }

    fn load_game(&mut self) -> Result<(), String> {
        match world::World::load_from_file(&self.save_path) {
            Ok(world) => {
                self.reset_game_context();
                self.world = world;
                self.mode = AppMode::Playing;
                self.set_status_message(format!("Loaded save from {}", self.save_path.display()));
                self.update_window_title();
                Ok(())
            }
            Err(err) => {
                let message = format!("Load failed: {}", err);
                self.mode = AppMode::MainMenu;
                self.set_status_message(message.clone());
                self.update_window_title();
                Err(message)
            }
        }
    }

    fn enter_pause_menu(&mut self) {
        self.input = input::InputState::default();
        self.command_input = None;
        self.mode = AppMode::PauseMenu;
        self.pause_menu_selection = 0;
        self.set_status_message("Pause menu opened. Use Up/Down to choose and Enter to confirm.");
        self.update_window_title();
        if let Some(state) = self.state.as_mut() {
            state.clear_command_prompt();
        }
    }

    fn resume_game(&mut self) {
        self.input = input::InputState::default();
        self.mode = AppMode::Playing;
        self.set_status_message("Resumed. Press Esc to pause.");
        self.update_window_title();
    }

    fn exit_to_main_menu(&mut self, save: bool) {
        if save {
            self.save_game();
        }
        self.mode = AppMode::MainMenu;
        self.command_input = None;
        self.main_menu_selection = 0;
        self.set_status_message("Main menu opened. Use Up/Down to choose and Enter to confirm.");
        self.update_window_title();
        if let Some(state) = self.state.as_mut() {
            state.clear_command_prompt();
        }
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let window_attrs = Window::default_attributes().with_title("NV2 Engine");
        let window = event_loop.create_window(window_attrs).unwrap();
        
        // Robimy leak, żeby mieć &'static Window dla wgpu
        let window: &'static Window = Box::leak(Box::new(window));

        let mut state = pollster::block_on(renderer::State::new(window));
        if self.mode != AppMode::Playing {
            state.input_captured = false;
            state.window.set_cursor_visible(true);
            let _ = state.window.set_cursor_grab(CursorGrabMode::None);
        }
        // If we are resuming into an already-playing session (e.g. loaded save),
        // make sure the renderer camera is positioned on the world's surface.
        if self.mode == AppMode::Playing {
            let (sx, sy, sz) = self.world.find_spawn_point();
            state.camera.position = Vector3::new(sx, sy, sz);
            state.camera.velocity = Vector3::new(0.0, 0.0, 0.0);
            state.camera.on_ground = true;
            state.camera_uniform.update_view_proj(&state.camera, state.config.width as f32 / state.config.height as f32);
            state.queue.write_buffer(&state.camera_buffer, 0, bytemuck::cast_slice(&[state.camera_uniform]));
        }
        self.state = Some(state);
        self.update_window_title();
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        if self.state.is_none() {
            return;
        }

        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(physical_size) => {
                if let Some(state) = self.state.as_mut() {
                    state.resize(physical_size);
                }
            }
            WindowEvent::KeyboardInput { event, .. } => {
                if let PhysicalKey::Code(key) = event.physical_key {
                    let pressed = event.state.is_pressed();
                    let repeated = event.repeat;
                    let text = event.text.as_deref();

                    if self.mode == AppMode::Playing {
                        if self.command_input.is_some() {
                            if !pressed {
                                return;
                            }

                            match key {
                                KeyCode::Escape => {
                                    self.cancel_command_input();
                                }
                                KeyCode::Enter => {
                                    self.execute_command_input();
                                }
                                KeyCode::Backspace if !repeated => {
                                    if let Some(buffer) = self.command_input.as_mut() {
                                        if !buffer.is_empty() {
                                            buffer.pop();
                                        }
                                    }
                                    self.refresh_command_prompt();
                                }
                                _ => {
                                    if let Some(text) = text {
                                        if let Some(buffer) = self.command_input.as_mut() {
                                            for ch in text.chars() {
                                                if ch.is_control() {
                                                    continue;
                                                }
                                                buffer.push(ch);
                                            }
                                        }
                                        self.refresh_command_prompt();
                                    }
                                }
                            }
                            return;
                        }

                        if pressed && !repeated && Self::is_command_prompt_trigger(key, text) {
                            self.begin_command_input();
                            return;
                        }
                    }

                    let mut lock_cursor = false;
                    let mut unlock_cursor = false;

                    if pressed {
                        match self.mode {
                            AppMode::MainMenu => match key {
                                KeyCode::ArrowUp | KeyCode::KeyW => {
                                    self.main_menu_selection = (self.main_menu_selection + MAIN_MENU_ITEMS.len() - 1) % MAIN_MENU_ITEMS.len();
                                }
                                KeyCode::ArrowDown | KeyCode::KeyS => {
                                    self.main_menu_selection = (self.main_menu_selection + 1) % MAIN_MENU_ITEMS.len();
                                }
                                KeyCode::Enter | KeyCode::Space => {
                                    match self.main_menu_selection {
                                        0 => {
                                            self.start_new_game();
                                            lock_cursor = true;
                                        }
                                        1 => {
                                            if self.load_game().is_ok() {
                                                lock_cursor = true;
                                            }
                                        }
                                        2 => {
                                            event_loop.exit();
                                            return;
                                        }
                                        _ => {}
                                    }
                                }
                                KeyCode::KeyN => {
                                    self.start_new_game();
                                    lock_cursor = true;
                                }
                                KeyCode::KeyL => {
                                    if self.load_game().is_ok() {
                                        lock_cursor = true;
                                    }
                                }
                                KeyCode::KeyQ => {
                                    event_loop.exit();
                                    return;
                                }
                                KeyCode::Escape => {
                                    event_loop.exit();
                                    return;
                                }
                                KeyCode::KeyT => {
                                    if let Some(state) = self.state.as_mut() {
                                        state.next_texture_pack();
                                    }
                                }
                                _ => {}
                            },
                            AppMode::Playing => match key {
                                KeyCode::Escape => {
                                    if let Some(state) = self.state.as_mut() {
                                        if state.close_inventory(&mut self.world) {
                                            return;
                                        }
                                    }
                                    self.enter_pause_menu();
                                    unlock_cursor = true;
                                }
                                KeyCode::KeyE if !repeated => {
                                    if let Some(state) = self.state.as_mut() {
                                        let inventory_open = state.toggle_inventory(&mut self.world);
                                        self.set_status_message(if inventory_open {
                                            "Inventory opened."
                                        } else {
                                            "Inventory closed."
                                        });
                                    }
                                }
                                KeyCode::KeyF if !repeated => {
                                    if let Some(state) = self.state.as_mut() {
                                        let enabled = state.camera.toggle_flight();
                                        self.set_status_message(if enabled {
                                            String::from("Flight mode enabled.")
                                        } else {
                                            String::from("Flight mode disabled.")
                                        });
                                        self.update_window_title();
                                    }
                                }
                                KeyCode::KeyT => {
                                    if let Some(state) = self.state.as_mut() {
                                        state.next_texture_pack();
                                    }
                                }
                                _ => {}
                            },
                            AppMode::PauseMenu => match key {
                                KeyCode::ArrowUp | KeyCode::KeyW => {
                                    self.pause_menu_selection = (self.pause_menu_selection + PAUSE_MENU_ITEMS.len() - 1) % PAUSE_MENU_ITEMS.len();
                                }
                                KeyCode::ArrowDown | KeyCode::KeyS => {
                                    self.pause_menu_selection = (self.pause_menu_selection + 1) % PAUSE_MENU_ITEMS.len();
                                }
                                KeyCode::Enter | KeyCode::Space => {
                                    match self.pause_menu_selection {
                                        0 => {
                                            self.resume_game();
                                            lock_cursor = true;
                                        }
                                        1 => {
                                            self.save_game();
                                        }
                                        2 => {
                                            self.save_game();
                                            self.exit_to_main_menu(false);
                                            unlock_cursor = true;
                                        }
                                        3 => {
                                            self.exit_to_main_menu(false);
                                            unlock_cursor = true;
                                        }
                                        _ => {}
                                    }
                                }
                                KeyCode::Escape => {
                                    self.resume_game();
                                    lock_cursor = true;
                                }
                                KeyCode::KeyT => {
                                    if let Some(state) = self.state.as_mut() {
                                        state.next_texture_pack();
                                    }
                                }
                                _ => {}
                            },
                        }
                    }

                    if self.mode == AppMode::Playing && self.command_input.is_none() {
                        self.input.handle_key(key, pressed);
                    }

                    if let Some(state) = self.state.as_mut() {
                        if lock_cursor {
                            state.input_captured = true;
                            state.window.set_cursor_visible(false);
                            let _ = state.window.set_cursor_grab(CursorGrabMode::Locked);
                        } else if unlock_cursor {
                            state.input_captured = false;
                            state.window.set_cursor_visible(true);
                            let _ = state.window.set_cursor_grab(CursorGrabMode::None);
                        }
                    }
                }
            }
            WindowEvent::MouseInput { state: m_state, button, .. } => {
                let pressed = m_state == winit::event::ElementState::Pressed;
                if self.mode == AppMode::Playing && self.command_input.is_none() {
                    self.input.handle_mouse_button(button, pressed);

                    if let Some(state) = self.state.as_mut() {
                        if button == winit::event::MouseButton::Left
                            && pressed
                            && !state.input_captured
                            && !state.inventory_open()
                        {
                            state.input_captured = true;
                            state.window.set_cursor_visible(false);
                            let _ = state.window.set_cursor_grab(CursorGrabMode::Locked);
                        }
                    }
                }
            }
            WindowEvent::CursorMoved { position, .. } => {
                self.input.set_cursor_position(position.x as f32, position.y as f32);
            }
            WindowEvent::MouseWheel { delta, .. } => {
                if self.mode == AppMode::Playing && self.command_input.is_none() {
                    self.input.accumulate_scroll(delta);
                }
            }
            WindowEvent::RedrawRequested => {
                let now = Instant::now();
                let dt = now.duration_since(self.last_frame).as_secs_f32().min(0.1);
                self.last_frame = now;

                if let Some(state) = self.state.as_mut() {
                    if self.mode == AppMode::Playing {
                        state.update(&mut self.world, &mut self.input, dt);
                    }
                    self.input.clear_frame();

                    let (ui_mode, ui_selection) = match self.mode {
                        AppMode::MainMenu => (renderer::UiMode::MainMenu, Some(self.main_menu_selection)),
                        AppMode::PauseMenu => (renderer::UiMode::PauseMenu, Some(self.pause_menu_selection)),
                        AppMode::Playing => (renderer::UiMode::None, None),
                    };

                    if let Err(wgpu::SurfaceError::Lost) = state.render(&self.world, ui_mode, ui_selection) {
                        state.resize(state.size);
                    }

                    state.window.request_redraw();
                }
            }
            _ => {}
        }
    }

    fn device_event(&mut self, _: &ActiveEventLoop, _: DeviceId, event: DeviceEvent) {
        if let DeviceEvent::MouseMotion { delta: (dx, dy) } = event {
            let capture_mouse = self.mode == AppMode::Playing
                && self.command_input.is_none()
                && self.state.as_ref().map(|state| state.input_captured).unwrap_or(false);
            if capture_mouse {
                self.input.accumulate_mouse(dx, dy);
            }
        }
    }
}

fn main() {
    env_logger::init();
    let event_loop = EventLoop::new().unwrap();
    let mut app = App::new();
    let _ = event_loop.run_app(&mut app);
}