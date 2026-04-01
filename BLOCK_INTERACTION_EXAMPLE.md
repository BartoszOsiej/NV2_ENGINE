// BONUS: Block Interaction Implementation Guide
// ============================================
//
// Add this to Core/Src/input.rs after InputState:

use winit::event::MouseButton;

#[derive(Default, Clone, Copy, Debug)]
pub struct MouseState {
    pub left_pressed: bool,
    pub right_pressed: bool,
}

// Add to InputState struct:
pub struct InputState {
    pub keys_held: std::collections::HashSet<KeyCode>,
    pub mouse_dx:  f64,
    pub mouse_dy:  f64,
    pub mouse_captured: bool,
    pub mouse_state: MouseState,  // ADD THIS
}

impl InputState {
    pub fn handle_mouse_button(&mut self, button: MouseButton, pressed: bool) {
        match button {
            MouseButton::Left => self.mouse_state.left_pressed = pressed,
            MouseButton::Right => self.mouse_state.right_pressed = pressed,
            _ => {}
        }
    }
}

// =====================================================
// Add this to Core/Src/renderer/mod.rs State struct:

pub struct State {
    // ... existing fields ...
    pub selected_block: Option<camera::RaycastHit>,  // ADD THIS
    pub active_block: BlockType,                      // ADD THIS
}

impl State {
    pub async fn new(window: &'static Window) -> Self {
        let mut state = Self {
            // ... existing initialization ...
            selected_block: None,                      // ADD THIS
            active_block: BlockType::Stone,            // ADD THIS
        };
        state
    }
    
    pub fn update(&mut self, world: &mut World, input: &mut InputState, dt: f32) {
        // ... existing code ...
        
        // Raycast for block targeting
        self.selected_block = crate::world::raycast(
            self.camera.position,
            self.camera.get_forward(),
            5.0,
            world,
        );
        
        // Handle block breaking (left click)
        if input.mouse_state.left_pressed {
            if let Some(hit) = self.selected_block {
                let (bx, by, bz) = hit.block_pos;
                world.set_block(bx, by, bz, BlockType::Air);
                
                // TODO: Mark chunk for remeshing
                let cx = bx / 16;
                let cz = bz / 16;
                // invalidate_chunk_mesh((cx, cz));
                
                input.mouse_state.left_pressed = false; // one-time click
            }
        }
        
        // Handle block placement (right click)
        if input.mouse_state.right_pressed {
            if let Some(hit) = self.selected_block {
                let (bx, by, bz) = hit.block_pos;
                let (nx, ny, nz) = match hit.face {
                    0 => (bx, by + 1, bz),     // top -> place above
                    1 => (bx, by - 1, bz),     // bottom -> place below
                    2 => (bx, bz + 1, bz),     // front -> place forward
                    3 => (bx, by, bz - 1),     // back -> place back
                    4 => (bx + 1, by, bz),     // right -> place right
                    5 => (bx - 1, by, bz),     // left -> place left
                    _ => return,
                };
                
                // Don't place inside player
                let player_center = self.camera.position;
                let is_inside_player = (nx as f32 - player_center.x).abs() < 0.6 &&
                                      (ny as f32 - player_center.y).abs() < 1.8 &&
                                      (nz as f32 - player_center.z).abs() < 0.6;
                
                if !is_inside_player {
                    world.set_block(nx, ny, nz, self.active_block);
                    
                    // TODO: Mark chunk for remeshing
                }
                
                input.mouse_state.right_pressed = false; // one-time click
            }
        }
    }
    
    pub fn render(&mut self) -> Result<(), wgpu::SurfaceError> {
        // ... existing render code ...
        
        // TODO: Render selection box if block is targeted
        if let Some(hit) = self.selected_block {
            // Draw wireframe box around hit.block_pos
            // This requires additional vertex/index buffers for the wireframe
        }
    }
}

// =====================================================
// Add this to Core/Src/main.rs WindowEvent handler:

use winit::event::MouseButton;

WindowEvent::MouseInput { state, button, .. } => {
    match state {
        winit::event::ElementState::Pressed => {
            input.handle_mouse_button(button, true);
        }
        winit::event::ElementState::Released => {
            input.handle_mouse_button(button, false);
        }
    }
}

// =====================================================
// RENDERING SELECTION BOX - Advanced Implementation
// =====================================================
//
// To render a wireframe selection box around targeted blocks:
//
// 1. Create a new vertex buffer for the wireframe (in State::new):
//
//     let wireframe_vertices = vec![
//         // 8 corner vertices of a unit cube
//         [0.0, 0.0, 0.0], [1.0, 0.0, 0.0],
//         [1.0, 1.0, 0.0], [0.0, 1.0, 0.0],
//         [0.0, 0.0, 1.0], [1.0, 0.0, 1.0],
//         [1.0, 1.0, 1.0], [0.0, 1.0, 1.0],
//     ];
//     
//     let wireframe_indices = vec![
//         // Front face
//         0, 1, 1, 2, 2, 3, 3, 0,
//         // Back face
//         4, 5, 5, 6, 6, 7, 7, 4,
//         // Connecting edges
//         0, 4, 1, 5, 2, 6, 3, 7,
//     ];
//
// 2. Add bind group for wireframe model matrix
//
// 3. In render(), if selected_block is Some:
//     - Set wireframe pipeline
//     - Update model matrix to block position
//     - Draw wireframe mesh
//
// See shader.wgsl for required shader updates

// =====================================================
// CHUNK REMESHING STRATEGY
// =====================================================
//
// Track dirty chunks:
//
// pub struct State {
//     gpu_chunks: HashMap<(i32, i32), GpuChunk>,
//     dirty_chunks: HashSet<(i32, i32)>,  // ADD THIS
// }
//
// When setting blocks:
//     world.set_block(x, y, z, block);
//     
//     let cx = x / 16;
//     let cz = z / 16;
//     self.dirty_chunks.insert((cx, cz));
//     
//     // Mark adjacent chunks if block is on boundary
//     if x % 16 == 0 { self.dirty_chunks.insert((cx - 1, cz)); }
//     if z % 16 == 0 { self.dirty_chunks.insert((cx, cz - 1)); }
//
// In update loop, remesh dirty chunks:
//    for &coords in &self.dirty_chunks {
//        if let Some(chunk) = world.get_chunk(coords.0, coords.1) {
//            let mesh = mesh::ChunkMesh::generate(world, coords.0, coords.1);
//            // Update GPU buffers
//            self.gpu_chunks.insert(coords, GpuChunk {
//                vertex_buffer: /* ... */,
//                index_buffer: /* ... */,
//                num_indices: mesh.indices.len() as u32,
//            });
//        }
//    }
//    self.dirty_chunks.clear();
