use std::time::Instant;
use winit::{
    event::*,
    event_loop::EventLoop,
    keyboard::{KeyCode, PhysicalKey},
    window::WindowBuilder,
};

mod renderer;
mod world;
mod input;

fn main() {
    env_logger::init();
    let event_loop = EventLoop::new().unwrap();
    let window: &'static _ = Box::leak(Box::new(
        WindowBuilder::new()
            .with_title("NV_ENGINE")
            .with_inner_size(winit::dpi::LogicalSize::new(1280, 720))
            .build(&event_loop).unwrap()
    ));

    let mut state = pollster::block_on(renderer::State::new(window));
    let mut world = world::World::new(42);
    let mut input = input::InputState::default();

    // Pre-generate spawn chunks so first frame isn't blank
    world.load_around(0, 0, renderer::RENDER_RADIUS);  // make RENDER_RADIUS pub

    let mut last_frame = Instant::now();

    event_loop.run(move |event, target| {
        match event {
            Event::WindowEvent { event: ref wev, window_id }
                if window_id == window.id() =>
            {
                match wev {
                    WindowEvent::CloseRequested => target.exit(),

                    WindowEvent::KeyboardInput { event: KeyEvent { physical_key: PhysicalKey::Code(key), state: elem_state, .. }, .. } => {
                        match elem_state {
                            ElementState::Pressed  => {
                                input.key_down(*key);
                                // Toggle mouse capture with Escape
                                if *key == KeyCode::Escape {
                                    input.mouse_captured = !input.mouse_captured;
                                    window.set_cursor_visible(!input.mouse_captured);
                                    let _ = window.set_cursor_grab(
                                        if input.mouse_captured {
                                            winit::window::CursorGrabMode::Confined
                                        } else {
                                            winit::window::CursorGrabMode::None
                                        }
                                    );
                                }
                            }
                            ElementState::Released => input.key_up(*key),
                        }
                    }

                    WindowEvent::Resized(s) => state.resize(*s),

                    WindowEvent::RedrawRequested => {
                        let now = Instant::now();
                        let dt  = now.duration_since(last_frame).as_secs_f32();
                        last_frame = now;

                        state.update(&mut world, &mut input, dt);

                        match state.render() {
                            Ok(_) => {}
                            Err(wgpu::SurfaceError::Lost) => state.resize(state.size),
                            Err(wgpu::SurfaceError::OutOfMemory) => target.exit(),
                            Err(e) => eprintln!("{e:?}"),
                        }
                    }

                    _ => {}
                }
            }

            Event::DeviceEvent { event: DeviceEvent::MouseMotion { delta: (dx, dy) }, .. } => {
                input.accumulate_mouse(dx, dy);
            }

            Event::AboutToWait => window.request_redraw(),

            _ => {}
        }
    }).unwrap();
}