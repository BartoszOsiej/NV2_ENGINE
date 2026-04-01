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

const MAIN_MENU_ITEMS: [&str; 3] = ["New Game", "Load Save", "Quit"];
const MAIN_MENU_DESCRIPTIONS: [&str; 3] = [
    "Start a fresh world from scratch.",
    "Load the previously saved world from disk.",
    "Quit the application.",
];
const PAUSE_MENU_ITEMS: [&str; 4] = ["Resume", "Save", "Save + Exit", "Exit"];
const PAUSE_MENU_DESCRIPTIONS: [&str; 4] = [
    "Return to gameplay immediately.",
    "Save the current world to disk.",
    "Save the world and return to the main menu.",
    "Return to the main menu without saving.",
];

struct App {
    state:        Option<renderer::State>,
    world:        world::World,
    input:        input::InputState,
    mode:         AppMode,
    save_path:    PathBuf,
    status_message: String,
    main_menu_selection: usize,
    pause_menu_selection: usize,
    last_frame:   Instant,
}

impl App {
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

    fn reset_game_context(&mut self) {
        self.input = input::InputState::default();
        self.main_menu_selection = 0;
        self.pause_menu_selection = 0;
        if let Some(state) = self.state.as_mut() {
            state.reset_for_new_world();
        }
    }

    fn new() -> Self {
        // Use current time to seed world so each run varies unless user loads a save
        let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();
        let seed = ((now.as_secs() as u64) ^ (now.subsec_nanos() as u64)) as u32;
        Self {
            state:          None,
            world:          world::World::new(seed),
            input:          input::InputState::default(),
            mode:           AppMode::MainMenu,
            save_path:      Self::default_save_path(),
            status_message: String::from("Use Up/Down to choose, Enter to activate."),
            main_menu_selection: 0,
            pause_menu_selection: 0,
            last_frame:     Instant::now(),
        }
    }

    fn start_new_game(&mut self) {
        self.reset_game_context();
        // New random seed for each new game
        let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap();
        let seed = ((now.as_secs() as u64) ^ (now.subsec_nanos() as u64)) as u32;
        self.world = world::World::new(seed);
        self.mode = AppMode::Playing;
        self.status_message = String::from("New game started. Press Esc for the pause menu.");
        self.update_window_title();
        // If renderer is initialized, move the camera to the surface of the new world
        if let Some(state) = self.state.as_mut() {
            let spawn_y = self.world.surface_height(0, 0) as f32 + 1.8;
            state.camera.position = Vector3::new(0.0, spawn_y, 0.0);
            state.camera.velocity = Vector3::new(0.0, 0.0, 0.0);
            state.camera.on_ground = true;
            state.camera_uniform.update_view_proj(&state.camera, state.config.width as f32 / state.config.height as f32);
            state.queue.write_buffer(&state.camera_buffer, 0, bytemuck::cast_slice(&[state.camera_uniform]));
        }
    }

    fn save_game(&mut self) {
        match self.world.save_to_file(&self.save_path) {
            Ok(_) => self.status_message = format!("World saved to {}", self.save_path.display()),
            Err(err) => self.status_message = format!("Save failed: {}", err),
        }
        self.update_window_title();
    }

    fn load_game(&mut self) -> Result<(), String> {
        match world::World::load_from_file(&self.save_path) {
            Ok(world) => {
                self.reset_game_context();
                self.world = world;
                self.mode = AppMode::Playing;
                self.status_message = format!("Loaded save from {}", self.save_path.display());
                self.update_window_title();
                Ok(())
            }
            Err(err) => {
                let message = format!("Load failed: {}", err);
                self.mode = AppMode::MainMenu;
                self.status_message = message.clone();
                self.update_window_title();
                Err(message)
            }
        }
    }

    fn enter_pause_menu(&mut self) {
        self.input = input::InputState::default();
        self.mode = AppMode::PauseMenu;
        self.pause_menu_selection = 0;
        self.status_message = String::from("Pause menu opened. Use Up/Down to choose and Enter to confirm.");
        self.update_window_title();
    }

    fn resume_game(&mut self) {
        self.input = input::InputState::default();
        self.mode = AppMode::Playing;
        self.status_message = String::from("Resumed. Press Esc to pause.");
        self.update_window_title();
    }

    fn exit_to_main_menu(&mut self, save: bool) {
        if save {
            self.save_game();
        }
        self.mode = AppMode::MainMenu;
        self.main_menu_selection = 0;
        self.status_message = String::from("Main menu opened. Use Up/Down to choose and Enter to confirm.");
        self.update_window_title();
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
            let spawn_y = self.world.surface_height(0, 0) as f32 + 1.8;
            state.camera.position = Vector3::new(0.0, spawn_y, 0.0);
            state.camera.velocity = Vector3::new(0.0, 0.0, 0.0);
            state.camera.on_ground = true;
            state.camera_uniform.update_view_proj(&state.camera, state.config.width as f32 / state.config.height as f32);
            state.queue.write_buffer(&state.camera_buffer, 0, bytemuck::cast_slice(&[state.camera_uniform]));
        }
        self.state = Some(state);
        self.update_window_title();
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        let state = match self.state.as_mut() {
            Some(s) => s,
            None => return,
        };

        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(physical_size) => state.resize(physical_size),
            WindowEvent::KeyboardInput { event, .. } => {
                if let PhysicalKey::Code(key) = event.physical_key {
                    let pressed = event.state.is_pressed();
                    let mut lock_cursor = false;
                    let mut unlock_cursor = false;

                    if pressed {
                        match self.mode {
                            AppMode::MainMenu => match key {
                                KeyCode::ArrowUp | KeyCode::KeyW => {
                                    self.main_menu_selection = (self.main_menu_selection + MAIN_MENU_ITEMS.len() - 1) % MAIN_MENU_ITEMS.len();
                                    let label = MAIN_MENU_ITEMS[self.main_menu_selection];
                                    let description = MAIN_MENU_DESCRIPTIONS[self.main_menu_selection];
                                    self.status_message = format!("{}: {} Press Enter.", label, description);
                                    self.update_window_title();
                                }
                                KeyCode::ArrowDown | KeyCode::KeyS => {
                                    self.main_menu_selection = (self.main_menu_selection + 1) % MAIN_MENU_ITEMS.len();
                                    let label = MAIN_MENU_ITEMS[self.main_menu_selection];
                                    let description = MAIN_MENU_DESCRIPTIONS[self.main_menu_selection];
                                    self.status_message = format!("{}: {} Press Enter.", label, description);
                                    self.update_window_title();
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
                                _ => {}
                            },
                            AppMode::Playing => match key {
                                KeyCode::Escape => {
                                    self.enter_pause_menu();
                                    unlock_cursor = true;
                                }
                                _ => {}
                            },
                            AppMode::PauseMenu => match key {
                                KeyCode::ArrowUp | KeyCode::KeyW => {
                                    self.pause_menu_selection = (self.pause_menu_selection + PAUSE_MENU_ITEMS.len() - 1) % PAUSE_MENU_ITEMS.len();
                                    let label = PAUSE_MENU_ITEMS[self.pause_menu_selection];
                                    let description = PAUSE_MENU_DESCRIPTIONS[self.pause_menu_selection];
                                    self.status_message = format!("{}: {} Press Enter.", label, description);
                                    self.update_window_title();
                                }
                                KeyCode::ArrowDown | KeyCode::KeyS => {
                                    self.pause_menu_selection = (self.pause_menu_selection + 1) % PAUSE_MENU_ITEMS.len();
                                    let label = PAUSE_MENU_ITEMS[self.pause_menu_selection];
                                    let description = PAUSE_MENU_DESCRIPTIONS[self.pause_menu_selection];
                                    self.status_message = format!("{}: {} Press Enter.", label, description);
                                    self.update_window_title();
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
                                _ => {}
                            },
                        }
                    }

                    if self.mode == AppMode::Playing {
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
                if self.mode == AppMode::Playing {
                    self.input.handle_mouse_button(button, pressed);

                    if button == winit::event::MouseButton::Left && pressed && !state.input_captured {
                        state.input_captured = true;
                        state.window.set_cursor_visible(false);
                        let _ = state.window.set_cursor_grab(CursorGrabMode::Locked);
                    }
                }
            }
            WindowEvent::RedrawRequested => {
                let now = Instant::now();
                let dt = now.duration_since(self.last_frame).as_secs_f32().min(0.1);
                self.last_frame = now;

                if self.mode == AppMode::Playing {
                    state.update(&mut self.world, &mut self.input, dt);
                }
                self.input.clear_mouse();

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
            _ => {}
        }
    }

    fn device_event(&mut self, _: &ActiveEventLoop, _: DeviceId, event: DeviceEvent) {
        if let DeviceEvent::MouseMotion { delta: (dx, dy) } = event {
            self.input.accumulate_mouse(dx, dy);
        }
    }
}

fn main() {
    env_logger::init();
    let event_loop = EventLoop::new().unwrap();
    let mut app = App::new();
    let _ = event_loop.run_app(&mut app);
}