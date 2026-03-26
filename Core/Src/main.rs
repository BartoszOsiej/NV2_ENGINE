use std::time::Instant;
use winit::{
    application::ApplicationHandler,
    event::{DeviceEvent, DeviceId, WindowEvent},
    event_loop::{ActiveEventLoop, EventLoop},
    keyboard::{KeyCode, PhysicalKey},
    window::{Window, WindowId},
};

mod renderer;
mod world;
mod input;

struct App {
    state:      Option<renderer::State>,
    world:      world::World,
    input:      input::InputState,
    last_frame: Instant,
}

impl App {
    fn new() -> Self {
        Self {
            state:      None,
            world:      world::World::new(42),
            input:      input::InputState::default(),
            last_frame: Instant::now(),
        }
    }
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let window: &'static Window = Box::leak(Box::new(
            event_loop.create_window(
                Window::default_attributes().with_title("NV_ENGINE")
            ).unwrap()
        ));
        let state = pollster::block_on(renderer::State::new(window));
        self.world.load_around(0, 0, renderer::RENDER_RADIUS);
        self.state = Some(state);
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        _id: WindowId,
        event: WindowEvent,
    ) {
        let state = match self.state.as_mut() {
            Some(s) => s,
            None    => return,
        };

        match event {
            WindowEvent::CloseRequested => event_loop.exit(),

            WindowEvent::KeyboardInput {
                event: winit::event::KeyEvent {
                    physical_key: PhysicalKey::Code(key),
                    state: elem_state,
                    ..
                }, ..
            } => {
                match elem_state {
                    winit::event::ElementState::Pressed => {
                        self.input.key_down(key);
                        if key == KeyCode::Escape {
                            self.input.mouse_captured = !self.input.mouse_captured;
                            state.window.set_cursor_visible(!self.input.mouse_captured);
                            let _ = state.window.set_cursor_grab(
                                if self.input.mouse_captured {
                                    winit::window::CursorGrabMode::Confined
                                } else {
                                    winit::window::CursorGrabMode::None
                                }
                            );
                        }
                    }
                    winit::event::ElementState::Released => {
                        self.input.key_up(key);
                    }
                }
            }

            WindowEvent::Resized(s) => state.resize(s),

            WindowEvent::RedrawRequested => {
                let now = Instant::now();
                let dt  = now.duration_since(self.last_frame).as_secs_f32().min(0.05);
                self.last_frame = now;

                state.update(&mut self.world, &mut self.input, dt);

                match state.render() {
                    Ok(_) => {}
                    Err(wgpu::SurfaceError::Lost) => state.resize(state.size),
                    Err(wgpu::SurfaceError::OutOfMemory) => event_loop.exit(),
                    Err(e) => eprintln!("{e:?}"),
                }
                state.window.request_redraw();
            }

            _ => {}
        }
    }

    fn device_event(
        &mut self,
        _event_loop: &ActiveEventLoop,
        _device_id: DeviceId,
        event: DeviceEvent,
    ) {
        if let DeviceEvent::MouseMotion { delta: (dx, dy) } = event {
            self.input.accumulate_mouse(dx, dy);
        }
    }
}

fn main() {
    env_logger::init();
    let event_loop = EventLoop::new().unwrap();
    let mut app = App::new();
    event_loop.run_app(&mut app).unwrap();
}