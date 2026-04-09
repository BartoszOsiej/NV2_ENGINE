use wgpu::util::DeviceExt;
use winit::{dpi::PhysicalSize, window::{CursorGrabMode, Window}};
use std::collections::HashMap;
// Tunable radii for loading/rendering
const LOAD_RADIUS: i32 = 4;
const RENDER_RADIUS: i32 = 4;
const CLEANUP_RADIUS: i32 = 5;

use crate::{
    assets,
    inventory::{HOTBAR_START, INVENTORY_SLOT_COUNT},
    interaction::{build_inventory_layout, GuiType, InteractionController, PanelRect, SlotRect, UiSlotId},
    world::{biomes::SEA_LEVEL, BlockType, World},
};

pub mod camera;
pub mod texture_atlas;
pub mod mesh;
pub mod texture;
pub mod vertices;
pub mod texture_registry;
pub mod instance;
pub mod menu;
mod text;
use camera::*;
use menu::MenuRenderer;
use vertices::Vertex;
use text::{TextAlignment, TextRenderer};
use texture_atlas::TileUV;

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct MaterialUniform {
    pub color_tint: [f32; 4],
}

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct BiomeUniform {
    /// xyz = ambient rgb tint, w = ambient multiplier
    pub ambient:    [f32; 4],
    /// xyz = fog rgb, w = fog density multiplier
    pub fog_color:  [f32; 4],
    /// xyz = scene grade, w = water animation time
    pub grade:      [f32; 4],
    /// x = day brightness, y = fog start, z = fog end, w = sun phase
    pub view_info:  [f32; 4],
    /// xyz = camera eye world-space position, w = sea level
    pub camera_pos: [f32; 4],
}

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct UiVertex {
    pub position: [f32; 2],
    pub color: [f32; 4],
}

impl UiVertex {
    pub fn desc<'a>() -> wgpu::VertexBufferLayout<'a> {
        use std::mem;
        wgpu::VertexBufferLayout {
            array_stride: mem::size_of::<UiVertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 0,
                    format: wgpu::VertexFormat::Float32x2,
                },
                wgpu::VertexAttribute {
                    offset: mem::size_of::<[f32; 2]>() as wgpu::BufferAddress,
                    shader_location: 1,
                    format: wgpu::VertexFormat::Float32x4,
                },
            ],
        }
    }
}

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct UiSpriteVertex {
    pub position: [f32; 2],
    pub uv: [f32; 2],
    pub color: [f32; 4],
}

impl UiSpriteVertex {
    pub fn desc<'a>() -> wgpu::VertexBufferLayout<'a> {
        use std::mem;
        wgpu::VertexBufferLayout {
            array_stride: mem::size_of::<UiSpriteVertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 0,
                    format: wgpu::VertexFormat::Float32x2,
                },
                wgpu::VertexAttribute {
                    offset: mem::size_of::<[f32; 2]>() as wgpu::BufferAddress,
                    shader_location: 1,
                    format: wgpu::VertexFormat::Float32x2,
                },
                wgpu::VertexAttribute {
                    offset: mem::size_of::<[f32; 4]>() as wgpu::BufferAddress,
                    shader_location: 2,
                    format: wgpu::VertexFormat::Float32x4,
                },
            ],
        }
    }
}
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum UiMode {
    None,
    MainMenu,
    PauseMenu,
}

#[derive(Debug, Copy, Clone)]
pub struct UiPanel {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
    pub fill: [f32; 4],
    pub border_color: [f32; 4],
    pub border_thickness: f32,
}

pub struct State {
    pub window: &'static Window,
    pub surface: wgpu::Surface<'static>,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub config: wgpu::SurfaceConfiguration,
    pub size: PhysicalSize<u32>,

    pub camera: Camera,
    pub camera_uniform: CameraUniform,
    pub camera_buffer: wgpu::Buffer,
    pub camera_bind_group: wgpu::BindGroup,
    pub material_buffer: wgpu::Buffer,
    pub material_bind_group: wgpu::BindGroup,
    pub top_tint: [f32; 3],
    pub biome_buffer: wgpu::Buffer,
    pub biome_bind_group: wgpu::BindGroup,

    pub depth_texture: texture::Texture,
    pub texture_atlas: texture_atlas::AtlasTexture,
    pub texture_bind_group: wgpu::BindGroup,

    pub render_pipeline: wgpu::RenderPipeline,
    pub water_pipeline: wgpu::RenderPipeline,
    pub ui_pipeline: wgpu::RenderPipeline,
    pub ui_sprite_pipeline: wgpu::RenderPipeline,
    pub text_renderer: TextRenderer,
    pub menu_renderer: MenuRenderer,
    pub interaction: InteractionController,

    pub chunk_meshes: HashMap<(i32, i32), mesh::ChunkMesh>,
    pub water_meshes: HashMap<(i32, i32), mesh::ChunkMesh>,
    pub current_vertices: Vec<Vertex>,
    pub current_indices: Vec<u32>,
    pub water_vertex_buffer: Option<wgpu::Buffer>,
    pub water_index_buffer: Option<wgpu::Buffer>,
    pub num_water_indices: u32,
    pub water_sim_timer: f32,
    pub prev_chunk: (i32, i32),
    pub vertex_buffer: Option<wgpu::Buffer>,
    pub index_buffer: Option<wgpu::Buffer>,
    pub num_indices: u32,
    pub block_models: HashMap<String, assets::NormalizedBlockModel>,

    pub input_captured: bool,
    water_sim_dirty: bool,
    /// Set when chunk meshes change; cleared after the next GPU buffer upload.
    needs_gpu_upload: bool,
    /// Accumulates dt; GPU buffer upload only runs when this exceeds 0.1 s.
    /// Resets to a high value on chunk-boundary cross to force immediate upload.
    mesh_rebuild_timer: f32,
    /// Throttles full water MESH rebuild to at most once per 2.5 s (decoupled from
    /// the 0.5 s simulation tick so traversal never rebuilds 81 chunks/frame).
    water_mesh_rebuild_timer: f32,
    /// Set when water meshes only need recombining, not a full geometry rebuild.
    needs_water_combine: bool,
    /// Chunk positions whose meshes have not been built yet; drained ≤4 per frame
    /// so mesh building never stalls the render thread for more than ~1 ms.
    meshes_to_build: std::collections::VecDeque<(i32, i32)>,
    crosshair_vertex_buffer: wgpu::Buffer,
    crosshair_index_buffer: wgpu::Buffer,
    crosshair_index_count: u32,
    subtitle_text: Option<String>,
    command_prompt_text: Option<String>,
    /// Monotonically increasing session time (seconds). Drives day/night + water anim.
    elapsed_time: f32,

    // ── Texture pack swapping ──────────────────────────────────────────────
    pub texture_bind_group_layout: wgpu::BindGroupLayout,
    /// Paths to available texture packs. Index 0 is always "default" (built-in compose).
    pub available_packs: Vec<String>,
    pub current_pack_index: usize,
}

// ── Module-level helpers ────────────────────────────────────────────────────

/// Pack all chunk meshes in a square of `radius` around `(cx, cz)` into
/// flattened vertex/index arrays ready for a GPU upload.
///
/// CPU frustum culling was intentionally removed here. The previous plane test
/// was over-culling camera-adjacent chunks, which made the spawn chunk disappear
/// and let the sky/fog show through. At a radius of 4 the upload set is small
/// enough that conservative submission is the safer choice.
fn combine_meshes(
    meshes:    &HashMap<(i32, i32), mesh::ChunkMesh>,
    cx: i32, cz: i32, radius: i32,
    _view_proj: &[[f32; 4]; 4],
) -> (Vec<Vertex>, Vec<u32>) {
    let mut verts = Vec::new();
    let mut idxs  = Vec::new();
    for dz in -radius..=radius {
        for dx in -radius..=radius {
            let (mx, mz) = (cx + dx, cz + dz);
            if let Some(m) = meshes.get(&(mx, mz)) {
                let base = verts.len() as u32;
                verts.extend_from_slice(&m.vertices);
                idxs.extend(m.indices.iter().map(|&i| i + base));
            }
        }
    }
    (verts, idxs)
}

/// Upload a vertex+index buffer pair to the GPU.
/// Returns `(None, None)` when `vertices` is empty.
fn upload_pair(
    device:   &wgpu::Device,
    vertices: &[Vertex],
    indices:  &[u32],
) -> (Option<wgpu::Buffer>, Option<wgpu::Buffer>) {
    if vertices.is_empty() {
        return (None, None);
    }
    let vb = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label:    None,
        contents: bytemuck::cast_slice(vertices),
        usage:    wgpu::BufferUsages::VERTEX,
    });
    let ib = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label:    None,
        contents: bytemuck::cast_slice(indices),
        usage:    wgpu::BufferUsages::INDEX,
    });
    (Some(vb), Some(ib))
}

fn build_crosshair_buffers(device: &wgpu::Device) -> (wgpu::Buffer, wgpu::Buffer, u32) {
    let vertices = [
        UiVertex { position: [-0.028,  0.004], color: [1.0, 1.0, 1.0, 0.85] },
        UiVertex { position: [ 0.028,  0.004], color: [1.0, 1.0, 1.0, 0.85] },
        UiVertex { position: [ 0.028, -0.004], color: [1.0, 1.0, 1.0, 0.85] },
        UiVertex { position: [-0.028, -0.004], color: [1.0, 1.0, 1.0, 0.85] },
        UiVertex { position: [-0.004,  0.044], color: [1.0, 1.0, 1.0, 0.85] },
        UiVertex { position: [ 0.004,  0.044], color: [1.0, 1.0, 1.0, 0.85] },
        UiVertex { position: [ 0.004, -0.044], color: [1.0, 1.0, 1.0, 0.85] },
        UiVertex { position: [-0.004, -0.044], color: [1.0, 1.0, 1.0, 0.85] },
    ];
    let indices: [u16; 12] = [0, 1, 2, 0, 2, 3, 4, 5, 6, 4, 6, 7];

    let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("crosshair vertex buffer"),
        contents: bytemuck::cast_slice(&vertices),
        usage: wgpu::BufferUsages::VERTEX,
    });
    let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("crosshair index buffer"),
        contents: bytemuck::cast_slice(&indices),
        usage: wgpu::BufferUsages::INDEX,
    });

    (vertex_buffer, index_buffer, indices.len() as u32)
}

fn push_ui_rect(
    vertices: &mut Vec<UiVertex>,
    indices: &mut Vec<u16>,
    screen_size: (u32, u32),
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    color: [f32; 4],
) {
    if width <= 0.0 || height <= 0.0 {
        return;
    }

    let screen_w = screen_size.0.max(1) as f32;
    let screen_h = screen_size.1.max(1) as f32;
    let left = x.round();
    let top = y.round();
    let right = (x + width).round();
    let bottom = (y + height).round();

    let left_ndc = left / screen_w * 2.0 - 1.0;
    let right_ndc = right / screen_w * 2.0 - 1.0;
    let top_ndc = 1.0 - top / screen_h * 2.0;
    let bottom_ndc = 1.0 - bottom / screen_h * 2.0;

    let base = vertices.len() as u16;
    vertices.push(UiVertex { position: [left_ndc, top_ndc], color });
    vertices.push(UiVertex { position: [right_ndc, top_ndc], color });
    vertices.push(UiVertex { position: [right_ndc, bottom_ndc], color });
    vertices.push(UiVertex { position: [left_ndc, bottom_ndc], color });
    indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
}

fn push_ui_panel(
    vertices: &mut Vec<UiVertex>,
    indices: &mut Vec<u16>,
    screen_size: (u32, u32),
    panel: UiPanel,
) {
    push_ui_rect(
        vertices,
        indices,
        screen_size,
        panel.x,
        panel.y,
        panel.width,
        panel.height,
        panel.fill,
    );

    let border = panel.border_thickness.max(0.0);
    if border <= 0.0 || panel.border_color[3] <= 0.0 {
        return;
    }

    push_ui_rect(
        vertices,
        indices,
        screen_size,
        panel.x,
        panel.y,
        panel.width,
        border,
        panel.border_color,
    );
    push_ui_rect(
        vertices,
        indices,
        screen_size,
        panel.x,
        panel.y + panel.height - border,
        panel.width,
        border,
        panel.border_color,
    );
    push_ui_rect(
        vertices,
        indices,
        screen_size,
        panel.x,
        panel.y,
        border,
        panel.height,
        panel.border_color,
    );
    push_ui_rect(
        vertices,
        indices,
        screen_size,
        panel.x + panel.width - border,
        panel.y,
        border,
        panel.height,
        panel.border_color,
    );
}

fn push_ui_sprite(
    vertices: &mut Vec<UiSpriteVertex>,
    indices: &mut Vec<u16>,
    screen_size: (u32, u32),
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    tile: TileUV,
    color: [f32; 4],
) {
    if width <= 0.0 || height <= 0.0 {
        return;
    }

    let screen_w = screen_size.0.max(1) as f32;
    let screen_h = screen_size.1.max(1) as f32;
    let left = x.round();
    let top = y.round();
    let right = (x + width).round();
    let bottom = (y + height).round();

    let left_ndc = left / screen_w * 2.0 - 1.0;
    let right_ndc = right / screen_w * 2.0 - 1.0;
    let top_ndc = 1.0 - top / screen_h * 2.0;
    let bottom_ndc = 1.0 - bottom / screen_h * 2.0;
    let uv = tile.uvs();

    let base = vertices.len() as u16;
    vertices.push(UiSpriteVertex { position: [left_ndc, top_ndc], uv: uv[0], color });
    vertices.push(UiSpriteVertex { position: [right_ndc, top_ndc], uv: uv[1], color });
    vertices.push(UiSpriteVertex { position: [right_ndc, bottom_ndc], uv: uv[2], color });
    vertices.push(UiSpriteVertex { position: [left_ndc, bottom_ndc], uv: uv[3], color });
    indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
}

fn gameplay_panel(rect: PanelRect, fill: [f32; 4], border_color: [f32; 4], border_thickness: f32) -> UiPanel {
    UiPanel {
        x: rect.x,
        y: rect.y,
        width: rect.width,
        height: rect.height,
        fill,
        border_color,
        border_thickness,
    }
}

fn block_icon_tile(block: BlockType) -> TileUV {
    let faces = block.face_uvs();
    faces
        .iter()
        .copied()
        .find(|tile| !tile.is_top)
        .unwrap_or(faces[0])
}

fn slot_group_bounds(rects: &[SlotRect], padding: f32) -> Option<PanelRect> {
    let first = *rects.first()?;
    let mut min_x = first.x;
    let mut min_y = first.y;
    let mut max_x = first.x + first.size;
    let mut max_y = first.y + first.size;

    for rect in &rects[1..] {
        min_x = min_x.min(rect.x);
        min_y = min_y.min(rect.y);
        max_x = max_x.max(rect.x + rect.size);
        max_y = max_y.max(rect.y + rect.size);
    }

    Some(PanelRect {
        x: min_x - padding,
        y: min_y - padding,
        width: (max_x - min_x) + padding * 2.0,
        height: (max_y - min_y) + padding * 2.0,
    })
}

fn slot_panel_rect(slot: SlotRect, padding: f32) -> PanelRect {
    PanelRect {
        x: slot.x - padding,
        y: slot.y - padding,
        width: slot.size + padding * 2.0,
        height: slot.size + padding * 2.0,
    }
}

/// Try font paths in priority order; stops after the first successful load.
fn load_font(tr: &mut TextRenderer) {
    let candidates: &[&str] = &[
        // Prefer readable UI fonts over decorative display faces.
        r"C:\Windows\Fonts\segoeui.ttf",
        r"C:\Windows\Fonts\arial.ttf",
        r"C:\Windows\Fonts\calibri.ttf",
        r"C:\Windows\Fonts\tahoma.ttf",
        r"C:\Windows\Fonts\verdana.ttf",
        "Core/Assets/Fonts/Subtitles/Doto-VariableFont_ROND,wght.ttf",
        "Assets/Fonts/Subtitles/Doto-VariableFont_ROND,wght.ttf",
        "../Assets/Fonts/Subtitles/Doto-VariableFont_ROND,wght.ttf",
        "../../Assets/Fonts/Subtitles/Doto-VariableFont_ROND,wght.ttf",
        // Decorative project font as a last resort.
        "Core/Assets/Fonts/Subtitles/RubikBurned-Regular.ttf",
        "Assets/Fonts/Subtitles/RubikBurned-Regular.ttf",
        "../Assets/Fonts/Subtitles/RubikBurned-Regular.ttf",
        "../../Assets/Fonts/Subtitles/RubikBurned-Regular.ttf",
    ];
    for path in candidates {
        if std::path::Path::new(path).exists() && tr.load_font_from_path(path).is_ok() {
            eprintln!("Loaded UI font: {}", path);
            return;
        }
    }
    // Last resort: let the asset system locate any available subtitle font.
    if let Ok(Some(path)) = crate::assets::ensure_subtitle_font() {
        if tr.load_font_from_path(&path).is_ok() {
            eprintln!("Loaded UI font: {}", path.display());
        }
    }
}

impl State {
    pub async fn new(window: &'static Window) -> Self {
        let size = window.inner_size();

        let backends = if cfg!(target_os = "windows") {
            // Prefer DirectX on Windows to avoid Vulkan validation layers
            wgpu::Backends::DX12 | wgpu::Backends::GL
        } else {
            wgpu::Backends::all()
        };

        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends,
            ..Default::default()
        });
        let surface = instance.create_surface(window).unwrap();

        let adapter = instance.request_adapter(&wgpu::RequestAdapterOptions {
            compatible_surface: Some(&surface),
            power_preference: wgpu::PowerPreference::HighPerformance,
            force_fallback_adapter: false,
        }).await.unwrap();

        let (device, queue) = adapter.request_device(&wgpu::DeviceDescriptor::default(), None).await.unwrap();

        let surface_caps = surface.get_capabilities(&adapter);
        let format = surface_caps.formats[0];

        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width: size.width,
            height: size.height,
            present_mode: wgpu::PresentMode::Fifo,
            alpha_mode: surface_caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 0,
        };
        surface.configure(&device, &config);

        let depth_texture = texture::Texture::create_depth_texture(&device, &config, "depth texture");

        let texture_atlas = texture_atlas::AtlasTexture::new(&device, &queue).await.unwrap();

        let texture_bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("texture layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });

        let texture_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("texture bind group"),
            layout: &texture_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&texture_atlas.view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&texture_atlas.sampler),
                },
            ],
        });

        let camera = Camera::new(cgmath::Vector3::new(0.0, 80.0, 0.0));
        let block_models = assets::BlockModelLoader::load_all("Assets/Models/Block/")
            .unwrap_or_default();
        let mut camera_uniform = CameraUniform::new();
        camera_uniform.update_view_proj(&camera, config.width as f32 / config.height as f32);

        let camera_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Camera Buffer"),
            contents: bytemuck::cast_slice(&[camera_uniform]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let camera_bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("camera layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });

        let camera_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("camera bind group"),
            layout: &camera_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: camera_buffer.as_entire_binding(),
            }],
        });

        let material_bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("material layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });

        let top_tint = [0.2, 0.8, 0.3];
        let material_uniform = MaterialUniform { color_tint: [top_tint[0], top_tint[1], top_tint[2], 0.0] };
        let material_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Material Uniform Buffer"),
            contents: bytemuck::cast_slice(&[material_uniform]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let material_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("material bind group"),
            layout: &material_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: material_buffer.as_entire_binding(),
            }],
        });

        // Biome ambient uniform: rgb tint + multiplier
        let biome_bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("biome layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });

        let default_biome = BiomeUniform {
            ambient: [1.0, 1.0, 1.0, 1.0],
            fog_color: [0.56, 0.72, 0.92, 1.0],
            grade: [1.0, 1.0, 1.0, 0.0],
            view_info: [1.0, 48.0, 84.0, 0.0],
            camera_pos: [0.0, 80.0, 0.0, SEA_LEVEL as f32],
        };
        let biome_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Biome Uniform Buffer"),
            contents: bytemuck::cast_slice(&[default_biome]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let biome_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("biome bind group"),
            layout: &biome_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: biome_buffer.as_entire_binding(),
            }],
        });

        let shader = device.create_shader_module(wgpu::include_wgsl!("shader.wgsl"));

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("pipeline layout"),
            bind_group_layouts: &[&camera_bind_group_layout, &material_bind_group_layout, &texture_bind_group_layout, &biome_bind_group_layout],
            push_constant_ranges: &[],
        });

        let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("pipeline"),
            layout: Some(&pipeline_layout),

            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs_main",
                buffers: &[Vertex::desc()],
                compilation_options: Default::default(),
            },

            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: "fs_main",
                targets: &[Some(config.format.into())],
                compilation_options: Default::default(),
            }),

            primitive: wgpu::PrimitiveState {
                cull_mode: None,
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: texture::Texture::DEPTH_FORMAT,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::Less,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
        });

        // Translucent water pipeline - same shader but with alpha blending
        let water_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("water pipeline"),
            layout: Some(&pipeline_layout),

            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs_main",
                buffers: &[Vertex::desc()],
                compilation_options: Default::default(),
            },

            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: "fs_main",
                targets: &[Some(wgpu::ColorTargetState {
                    format: config.format,
                    blend: Some(wgpu::BlendState {
                        color: wgpu::BlendComponent::OVER,
                        alpha: wgpu::BlendComponent::OVER,
                    }),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),

            primitive: wgpu::PrimitiveState {
                cull_mode: None,
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: texture::Texture::DEPTH_FORMAT,
                depth_write_enabled: false,
                depth_compare: wgpu::CompareFunction::LessEqual,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
        });

        let ui_shader = device.create_shader_module(wgpu::include_wgsl!("ui.wgsl"));
        let ui_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("ui pipeline layout"),
            bind_group_layouts: &[],
            push_constant_ranges: &[],
        });

        let ui_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("ui pipeline"),
            layout: Some(&ui_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &ui_shader,
                entry_point: "vs_main",
                buffers: &[UiVertex::desc()],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &ui_shader,
                entry_point: "fs_main",
                targets: &[Some(wgpu::ColorTargetState {
                    format: config.format,
                    blend: Some(wgpu::BlendState {
                        color: wgpu::BlendComponent::OVER,
                        alpha: wgpu::BlendComponent::OVER,
                    }),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                cull_mode: None,
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: texture::Texture::DEPTH_FORMAT,
                depth_write_enabled: false,
                depth_compare: wgpu::CompareFunction::Always,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
        });

        let ui_sprite_shader = device.create_shader_module(wgpu::include_wgsl!("ui_sprite.wgsl"));
        let ui_sprite_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("ui sprite pipeline layout"),
            bind_group_layouts: &[&texture_bind_group_layout],
            push_constant_ranges: &[],
        });

        let ui_sprite_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("ui sprite pipeline"),
            layout: Some(&ui_sprite_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &ui_sprite_shader,
                entry_point: "vs_main",
                buffers: &[UiSpriteVertex::desc()],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &ui_sprite_shader,
                entry_point: "fs_main",
                targets: &[Some(wgpu::ColorTargetState {
                    format: config.format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                cull_mode: None,
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: texture::Texture::DEPTH_FORMAT,
                depth_write_enabled: false,
                depth_compare: wgpu::CompareFunction::Always,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
        });

        let (crosshair_vertex_buffer, crosshair_index_buffer, crosshair_index_count) =
            build_crosshair_buffers(&device);
        let mut text_renderer = TextRenderer::new(
            &device,
            config.format,
            (config.width, config.height),
        );
        load_font(&mut text_renderer);

        Self {
            window,
            surface,
            device,
            queue,
            config,
            size,

            camera,
            camera_uniform,
            camera_buffer,
            camera_bind_group,
            material_buffer,
            material_bind_group,
            top_tint,

            biome_buffer,
            biome_bind_group,

            depth_texture,
            texture_atlas,
            texture_bind_group,

            render_pipeline,
            water_pipeline,
            ui_pipeline,
            ui_sprite_pipeline,

            chunk_meshes: HashMap::new(),
            water_meshes: HashMap::new(),
            current_vertices: Vec::new(),
            current_indices: Vec::new(),
            water_vertex_buffer: None,
            water_index_buffer: None,
            num_water_indices: 0,
            water_sim_timer: 0.0,
            vertex_buffer: None,
            index_buffer: None,
            num_indices: 0,
            prev_chunk: (i32::MIN, i32::MIN),
            block_models,

            input_captured: true,
            water_sim_dirty: false,
            needs_gpu_upload: false,
            mesh_rebuild_timer: 0.0,
            water_mesh_rebuild_timer: 0.0,
            needs_water_combine: false,
            meshes_to_build: std::collections::VecDeque::new(),
            crosshair_vertex_buffer,
            crosshair_index_buffer,
            crosshair_index_count,
            subtitle_text: None,
            command_prompt_text: None,
            elapsed_time: 0.0,
            text_renderer,
            menu_renderer: MenuRenderer::new(),
            interaction: InteractionController::default(),
            texture_bind_group_layout,
            available_packs: Self::scan_texture_packs(),
            current_pack_index: 0,
        }
    }

    fn ui_clear_color(mode: UiMode, elapsed: f32) -> wgpu::Color {
        match mode {
            UiMode::None      => Self::sky_color_for_time(elapsed),
            UiMode::MainMenu  => wgpu::Color { r: 0.06, g: 0.08, b: 0.12, a: 1.0 },
            UiMode::PauseMenu => wgpu::Color { r: 0.04, g: 0.05, b: 0.08, a: 1.0 },
        }
    }

    // ── Texture pack helpers ────────────────────────────────────────────────

    /// Return a sorted list of available pack paths. Index 0 is always "default".
    fn scan_texture_packs() -> Vec<String> {
        let mut packs = vec!["default".to_string()];
        let search_roots = ["Assets/TexturePacks", "../Assets/TexturePacks"];
        for root in &search_roots {
            if let Ok(entries) = std::fs::read_dir(root) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.is_dir() {
                        let atlas = path.join("atlas.png");
                        if atlas.exists() {
                            if let Some(s) = path.to_str() {
                                packs.push(s.replace('\\', "/"));
                            }
                        }
                    }
                }
            }
        }
        packs.sort_unstable();
        // Deduplicate (default is always first)
        packs.dedup();
        packs
    }

    /// Return the display name of the currently active texture pack.
    pub fn current_pack_name(&self) -> &str {
        self.available_packs
            .get(self.current_pack_index)
            .map(String::as_str)
            .unwrap_or("default")
    }

    /// Reload the atlas from the pack at `index` in `self.available_packs` and
    /// rebuild the texture bind group. Forces a full mesh rebuild.
    pub fn load_pack_by_index(&mut self, index: usize) {
        let path = match self.available_packs.get(index) {
            Some(p) => p.clone(),
            None => return,
        };
        self.current_pack_index = index;

        let atlas_result = if path == "default" {
            // Compose from individual block PNGs at runtime
            pollster::block_on(texture_atlas::AtlasTexture::new(&self.device, &self.queue))
        } else {
            let atlas_file = format!("{}/atlas.png", path);
            match std::fs::read(&atlas_file) {
                Ok(bytes) => {
                    let img = image::load_from_memory(&bytes)
                        .map(|i| i.to_rgba8());
                    match img {
                        Ok(rgba) => {
                            let (w, h) = rgba.dimensions();
                            texture_atlas::AtlasTexture::from_bytes(
                                &self.device,
                                &self.queue,
                                rgba.as_raw(),
                                w,
                                h,
                            )
                        }
                        Err(e) => {
                            log::warn!("Failed to decode atlas from pack '{}': {}", path, e);
                            return;
                        }
                    }
                }
                Err(e) => {
                    log::warn!("Failed to read atlas file '{}': {}", atlas_file, e);
                    return;
                }
            }
        };

        match atlas_result {
            Ok(new_atlas) => {
                self.texture_atlas = new_atlas;
                self.texture_bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("texture bind group"),
                    layout: &self.texture_bind_group_layout,
                    entries: &[
                        wgpu::BindGroupEntry {
                            binding: 0,
                            resource: wgpu::BindingResource::TextureView(&self.texture_atlas.view),
                        },
                        wgpu::BindGroupEntry {
                            binding: 1,
                            resource: wgpu::BindingResource::Sampler(&self.texture_atlas.sampler),
                        },
                    ],
                });
                // Force full mesh rebuild so new texture coordinates are applied.
                self.chunk_meshes.clear();
                self.water_meshes.clear();
                self.needs_gpu_upload = true;
                self.mesh_rebuild_timer = 999.0;
                self.water_mesh_rebuild_timer = 999.0;

                let name = self.available_packs[self.current_pack_index]
                    .rsplit('/')
                    .next()
                    .unwrap_or("default")
                    .to_string();
                self.show_subtitle(&format!("Texture pack: {}", name));
                log::info!("Loaded texture pack: {}", path);
            }
            Err(e) => {
                log::warn!("Failed to load texture pack '{}': {}", path, e);
            }
        }
    }

    /// Cycle to the next available texture pack.
    pub fn next_texture_pack(&mut self) {
        let n = self.available_packs.len();
        if n == 0 { return; }
        let next = (self.current_pack_index + 1) % n;
        self.load_pack_by_index(next);
    }

    /// Cycle to the previous available texture pack.
    pub fn prev_texture_pack(&mut self) {
        let n = self.available_packs.len();
        if n == 0 { return; }
        let prev = (self.current_pack_index + n - 1) % n;
        self.load_pack_by_index(prev);
    }

    /// Clear-color sky that matches the WGSL `sky_color(day, sun_phase)` function.
    /// Uses the same palette and blend math so the background seamlessly continues the fog.
    fn sky_color_for_time(t: f32) -> wgpu::Color {
        use std::f32::consts::TAU;
        let phase    = (t / 1200.0).fract();
        let day      = Self::day_brightness(t);
        let sun_elev = ((phase - 0.25) * TAU).sin();

        // Match shader sky_color() palette exactly
        let ch = |night: f32, zenith: f32, haze: f32, sunset: f32, twi: f32| -> f64 {
            let day_sky       = haze  + (zenith - haze)  * (day * 0.90).min(1.0);
            let base          = night + (day_sky - night) * (day * 1.20).min(1.0);
            let sunset_str    = (1.0 - sun_elev.abs() * 3.5).max(0.0);
            let sunset_col    = sunset + (twi - sunset) * (0.4 - sun_elev * 2.0).clamp(0.0, 1.0);
            (base + (sunset_col - base) * sunset_str * (day * 4.0).min(0.78)).clamp(0.0, 1.0) as f64
        };

        wgpu::Color {
            r: ch(0.006, 0.270, 0.560, 1.000, 0.560),
            g: ch(0.010, 0.520, 0.720, 0.340, 0.220),
            b: ch(0.048, 0.860, 0.920, 0.018, 0.460),
            a: 1.0,
        }
    }

    /// Day brightness: 0.15 at midnight, 1.0 at noon (drives shader diffuse/ambient).
    fn day_brightness(t: f32) -> f32 {
        use std::f32::consts::TAU;
        let phase = (t / 1200.0).fract();
        let v = ((phase * TAU - std::f32::consts::FRAC_PI_2).sin() * 0.5 + 0.5).max(0.0);
        0.15 + 0.85 * v.sqrt()
    }

    pub fn resize(&mut self, new_size: PhysicalSize<u32>) {
        if new_size.width > 0 && new_size.height > 0 {
            self.size = new_size;
            self.config.width = new_size.width;
            self.config.height = new_size.height;
            self.surface.configure(&self.device, &self.config);
            self.depth_texture = texture::Texture::create_depth_texture(&self.device, &self.config, "depth texture");
            self.text_renderer.resize((new_size.width, new_size.height));
        }
    }

    fn set_input_capture(&mut self, captured: bool) {
        self.input_captured = captured;
        self.window.set_cursor_visible(!captured);
        let grab_mode = if captured {
            CursorGrabMode::Locked
        } else {
            CursorGrabMode::None
        };
        let _ = self.window.set_cursor_grab(grab_mode);
    }

    pub fn inventory_open(&self) -> bool {
        self.interaction.inventory_open()
    }

    pub fn toggle_inventory(&mut self, world: &mut World) -> bool {
        let open = self.interaction.toggle_inventory(world);
        self.set_input_capture(!open);
        self.window.request_redraw();
        open
    }

    pub fn close_inventory(&mut self, world: &mut World) -> bool {
        if !self.interaction.close_inventory(world) {
            return false;
        }

        self.set_input_capture(true);
        self.window.request_redraw();
        true
    }

    pub fn reset_for_new_world(&mut self) {
        self.chunk_meshes.clear();
        self.water_meshes.clear();
        self.current_vertices.clear();
        self.current_indices.clear();
        self.vertex_buffer = None;
        self.index_buffer = None;
        self.water_vertex_buffer = None;
        self.water_index_buffer = None;
        self.num_water_indices = 0;
        self.num_indices = 0;
        self.prev_chunk = (i32::MIN, i32::MIN);
        self.camera = Camera::new(cgmath::Vector3::new(0.0, 80.0, 0.0));
        self.camera_uniform = CameraUniform::new();
        self.camera_uniform.update_view_proj(&self.camera, self.config.width as f32 / self.config.height as f32);
        self.queue.write_buffer(&self.camera_buffer, 0, bytemuck::cast_slice(&[self.camera_uniform]));
        self.water_sim_dirty = false;
        self.needs_gpu_upload = false;
        self.mesh_rebuild_timer = 0.0;
        self.water_mesh_rebuild_timer = 0.0;
        self.needs_water_combine = false;
        self.meshes_to_build.clear();
        self.subtitle_text = None;
        self.command_prompt_text = None;
        self.interaction = InteractionController::default();
        self.set_input_capture(true);
        // elapsed_time intentionally not reset — day/night cycle persists
    }

    pub fn update(&mut self, world: &mut World, input: &mut crate::input::InputState, dt: f32) {
        // Authoritative runtime movement path: event loop -> State::update -> Camera::tick_movement.
        let inventory_open = self.interaction.inventory_open();

        if !inventory_open {
            self.camera.tick_movement(world, input, dt);
        }
        self.camera_uniform.update_view_proj(&self.camera, self.config.width as f32 / self.config.height as f32);
        self.queue.write_buffer(&self.camera_buffer, 0, bytemuck::cast_slice(&[self.camera_uniform]));
        let material_uniform = MaterialUniform { color_tint: [self.top_tint[0], self.top_tint[1], self.top_tint[2], 0.0] };
        self.queue.write_buffer(&self.material_buffer, 0, bytemuck::cast_slice(&[material_uniform]));

        // Update atmosphere uniforms from the climate model at the camera position.
        let cam_x = self.camera.position.x.round() as i32;
        let cam_z = self.camera.position.z.round() as i32;
        let visuals = world.visuals_at(cam_x, cam_z);
        self.elapsed_time += dt;
        let water_time  = self.elapsed_time * 0.55;
        let day_bright  = Self::day_brightness(self.elapsed_time);
        let fog_density = visuals.fog_density.max(0.75);
        // Keep distance ranges stable here; density is applied once in the shader to avoid horizon bands.
        let fog_start = (RENDER_RADIUS as f32 * 10.0).clamp(28.0, 56.0);
        let fog_end   = (RENDER_RADIUS as f32 * 22.0).clamp(fog_start + 24.0, 120.0);
        let eye       = self.camera.position;
        let sun_phase = (self.elapsed_time / 1200.0).fract();
        let biome_uniform = BiomeUniform {
            ambient: visuals.ambient,
            fog_color: [visuals.fog_color[0], visuals.fog_color[1], visuals.fog_color[2], visuals.fog_density],
            grade: [visuals.grade[0], visuals.grade[1], visuals.grade[2], water_time],
            view_info: [day_bright, fog_start, fog_end, sun_phase],
            camera_pos: [eye.x, eye.y, eye.z, SEA_LEVEL as f32],
        };
        self.queue.write_buffer(&self.biome_buffer, 0, bytemuck::cast_slice(&[biome_uniform]));

        // Water simulation — throttled to 0.3 s intervals for responsive flow.
        // The new simulate_step() scales well with the increased MAX_CHANGES_PER_STEP.
        // Mesh rebuild is separately throttled to 1.5 s (see below).
        self.water_sim_timer += dt;
        if self.water_sim_timer >= 0.3 {
            self.water_sim_timer = 0.0;
            world.simulate_water();
            self.water_sim_dirty = true;
        }

        // Process any chunks that finished generating in background threads
        let new_chunk_coords = world.process_generated_chunks();

        // Queue chunk generation based on camera position
        let cx = (self.camera.position.x / 16.0).floor() as i32;
        let cz = (self.camera.position.z / 16.0).floor() as i32;

        world.load_around(cx, cz, LOAD_RADIUS);
        world.unload_far_chunks(cx, cz, LOAD_RADIUS);

        if inventory_open {
            self.interaction
                .update_inventory_input(world, input, (self.config.width, self.config.height));
        } else if self.input_captured {
            self.interaction.update(world, input, &self.camera, dt);
            // If a GUI opened mid-update (e.g. right-clicking an NVCrafter), release
            // the cursor so the player can see and interact with the new screen.
            if self.interaction.inventory_open() {
                self.set_input_capture(false);
                self.window.request_redraw();
            }
        }

        let chunk_moved = (cx, cz) != self.prev_chunk;

        let mut dirty_chunks = Vec::new();
        for (&key, chunk) in world.chunks.iter_mut() {
            if chunk.is_dirty {
                chunk.is_dirty = false;
                dirty_chunks.push(key);
            }
        }

        for &key in &dirty_chunks {
            if world.get_chunk(key.0, key.1).is_some()
                && !self.meshes_to_build.contains(&key)
            {
                self.meshes_to_build.push_front(key);
            }

            for (dx, dz) in [(-1i32, 0i32), (1, 0), (0, -1), (0, 1)] {
                let nkey = (key.0 + dx, key.1 + dz);
                if self.chunk_meshes.contains_key(&nkey)
                    && world.get_chunk(nkey.0, nkey.1).is_some()
                    && !self.meshes_to_build.contains(&nkey)
                {
                    self.meshes_to_build.push_back(nkey);
                }
            }
        }

        // ── Step 1: Enqueue any new/missing chunks that need mesh building ──
        // Never build inside this loop — just enqueue so we can rate-limit below.
        if !new_chunk_coords.is_empty() {
            for &(ncx, ncz) in &new_chunk_coords {
                // First-time build for the newly arrived chunk itself.
                let key = (ncx, ncz);
                if !self.chunk_meshes.contains_key(&key)
                    && !self.meshes_to_build.contains(&key)
                    && world.get_chunk(key.0, key.1).is_some()
                {
                    // High priority: push to front so initial terrain appears fast.
                    self.meshes_to_build.push_front(key);
                }

                // Seam repair: the 4 cardinal neighbours that are already meshed
                // will have wrong AO at the shared border because they were built
                // when this chunk didn't yet exist (neighbours were treated as Air).
                // Rebuild them now — the new mesh replaces the old one in-place,
                // so there is no visible pop or frame gap.
                for (dx, dz) in [(-1i32, 0i32), (1, 0), (0, -1), (0, 1)] {
                    let nkey = (ncx + dx, ncz + dz);
                    if self.chunk_meshes.contains_key(&nkey)
                        && !self.meshes_to_build.contains(&nkey)
                        && world.get_chunk(nkey.0, nkey.1).is_some()
                    {
                        // Low priority: push to back — seam repair yields to new chunks.
                        self.meshes_to_build.push_back(nkey);
                    }
                }
            }
        }

        // Synchronously generated chunks around the player are inserted directly
        // into `world.chunks`, so they never appear in `new_chunk_coords`.
        // Without this pass the spawn chunk can exist in the world but never be
        // queued for meshing, which makes the sky/fog show through at spawn.
        let mut missing_loaded = Vec::new();
        for dz in -RENDER_RADIUS..=RENDER_RADIUS {
            for dx in -RENDER_RADIUS..=RENDER_RADIUS {
                let key = (cx + dx, cz + dz);
                if world.get_chunk(key.0, key.1).is_none() {
                    continue;
                }
                if self.chunk_meshes.contains_key(&key) || self.meshes_to_build.contains(&key) {
                    continue;
                }
                let dist2 = dx * dx + dz * dz;
                missing_loaded.push((dist2, key));
            }
        }
        missing_loaded.sort_by_key(|&(dist2, _)| dist2);
        for (_, key) in missing_loaded {
            self.meshes_to_build.push_back(key);
        }

        // ── Drain build queue: ≤2 chunks per frame ─────────────────────────────
        // Limiting to 2 keeps each frame under ~30 ms budget even with complex
        // mountain geometry (each chunk build can take 5-15 ms).  Spreading more
        // work across frames is preferable to hitching every initial-load frame.
        let mut built_this_frame = 0u32;
        while built_this_frame < 2 {
            let key = match self.meshes_to_build.pop_front() {
                Some(k) => k,
                None => break,
            };
            // Chunk may have been unloaded while sitting in the queue — skip it.
            if world.get_chunk(key.0, key.1).is_none() { continue; }
            let m  = mesh::ChunkMesh::build(world, key.0, key.1);
            self.chunk_meshes.insert(key, m);
            let wm = mesh::ChunkMesh::build_water(world, key.0, key.1);
            self.water_meshes.insert(key, wm);
            built_this_frame += 1;
        }
        if built_this_frame > 0 {
            self.needs_gpu_upload    = true;
            self.needs_water_combine = true;
        }

        // On chunk boundary cross: evict stale cached meshes/queue entries, force upload.
        if chunk_moved {
            self.prev_chunk = (cx, cz);
            self.chunk_meshes.retain(|&(mx, mz), _| {
                (mx - cx).abs() <= CLEANUP_RADIUS && (mz - cz).abs() <= CLEANUP_RADIUS
            });
            self.water_meshes.retain(|&(mx, mz), _| {
                (mx - cx).abs() <= CLEANUP_RADIUS && (mz - cz).abs() <= CLEANUP_RADIUS
            });
            self.meshes_to_build.retain(|&(mx, mz)| {
                (mx - cx).abs() <= CLEANUP_RADIUS && (mz - cz).abs() <= CLEANUP_RADIUS
            });
            self.needs_gpu_upload = true;
            self.mesh_rebuild_timer = 1.0; // bypasses the 0.1-s cooldown below
            self.needs_water_combine = true;
        }

        // ── Step 2: GPU buffer upload — debounced to ≤10 Hz ──────────────────
        // Multiple chunk arrivals within a 100ms window are batched into ONE upload,
        // eliminating the per-chunk GPU-alloc spikes that caused the FPS drops.
        self.mesh_rebuild_timer += dt;
        if self.needs_gpu_upload && self.mesh_rebuild_timer >= 0.1 {
            self.mesh_rebuild_timer = 0.0;
            self.needs_gpu_upload = false;

            let (verts, idxs)    = combine_meshes(&self.chunk_meshes, cx, cz, RENDER_RADIUS, &self.camera_uniform.view_proj);
            self.num_indices      = idxs.len() as u32;
            let (vb, ib)          = upload_pair(&self.device, &verts, &idxs);
            self.vertex_buffer    = vb;
            self.index_buffer     = ib;
            // Water is handled separately; no full water rebuild necessary here.
        }

        // ── Water mesh FULL REBUILD (throttled: ≤ once per 1.5 s) ─────────────
        // Triggered by the liquid simulation tick. Geometry is rebuilt from the
        // current world state, then signals a buffer-combine below.
        self.water_mesh_rebuild_timer += dt;
        if self.water_sim_dirty && self.water_mesh_rebuild_timer >= 1.5 {
            self.water_mesh_rebuild_timer = 0.0;
            self.water_sim_dirty = false;
            self.water_meshes.clear();
            for dz in -RENDER_RADIUS..=RENDER_RADIUS {
                for dx in -RENDER_RADIUS..=RENDER_RADIUS {
                    let key = (cx + dx, cz + dz);
                    if world.get_chunk(key.0, key.1).is_some() {
                        let wm = mesh::ChunkMesh::build_water(world, key.0, key.1);
                        self.water_meshes.insert(key, wm);
                    }
                }
            }
            self.needs_water_combine = true;
        }

        // ── Water buffer COMBINE (fast: just re-pack existing cached meshes) ──
        // Runs after any incremental addition, eviction, or full rebuild above.
        if self.needs_water_combine {
            self.needs_water_combine = false;
            let (wverts, widxs)       = combine_meshes(&self.water_meshes, cx, cz, RENDER_RADIUS, &self.camera_uniform.view_proj);
            self.num_water_indices    = widxs.len() as u32;
            let (wvb, wib)            = upload_pair(&self.device, &wverts, &widxs);
            self.water_vertex_buffer  = wvb;
            self.water_index_buffer   = wib;
        }
    }

    /// Display a subtitle string using the queued text renderer.
    pub fn show_subtitle(&mut self, text: &str) {
        self.subtitle_text = (!text.trim().is_empty()).then(|| text.to_string());
        self.window.request_redraw();
    }

    pub fn clear_subtitle(&mut self) {
        self.subtitle_text = None;
        self.window.request_redraw();
    }

    pub fn show_command_prompt(&mut self, text: &str) {
        self.command_prompt_text = Some(text.to_string());
        self.window.request_redraw();
    }

    pub fn clear_command_prompt(&mut self) {
        self.command_prompt_text = None;
        self.window.request_redraw();
    }

    pub fn render(&mut self, world: &World, mode: UiMode, selection: Option<usize>) -> Result<(), wgpu::SurfaceError> {
        let output = self.surface.get_current_texture()?;
        let view = output.texture.create_view(&wgpu::TextureViewDescriptor::default());
        let clear_color = Self::ui_clear_color(mode, self.elapsed_time);
        let screen_size = (self.config.width, self.config.height);
        let gui_type = if mode == UiMode::None {
            self.interaction.gui_type()
        } else {
            None
        };
        let inventory_open = gui_type.is_some();

        let mut panel_vertices = Vec::new();
        let mut panel_indices = Vec::new();
        let mut sprite_vertices = Vec::new();
        let mut sprite_indices = Vec::new();

        match mode {
            UiMode::MainMenu | UiMode::PauseMenu => {
                for panel in self
                    .menu_renderer
                    .build_menu_panels(&self.text_renderer, mode, selection)
                {
                    push_ui_panel(&mut panel_vertices, &mut panel_indices, screen_size, panel);
                }
            }
            UiMode::None => {
                let layout = build_inventory_layout(screen_size, gui_type);
                let inventory = self.interaction.inventory();
                let active_slot = inventory.active_slot_index();
                let hovered_slot = self.interaction.hovered_slot();

                if inventory_open {
                    push_ui_panel(
                        &mut panel_vertices,
                        &mut panel_indices,
                        screen_size,
                        UiPanel {
                            x: 0.0,
                            y: 0.0,
                            width: self.config.width as f32,
                            height: self.config.height as f32,
                            fill: [0.02, 0.03, 0.04, 0.46],
                            border_color: [0.0, 0.0, 0.0, 0.0],
                            border_thickness: 0.0,
                        },
                    );
                    if let Some(panel) = layout.main_panel {
                        push_ui_panel(
                            &mut panel_vertices,
                            &mut panel_indices,
                            screen_size,
                            gameplay_panel(
                                panel,
                                [0.06, 0.08, 0.11, 0.92],
                                [0.88, 0.92, 0.98, 0.92],
                                2.0,
                            ),
                        );

                        let player_inventory_slots: Vec<SlotRect> = layout.player_slot_rects.iter().copied().collect();
                        if let Some(bounds) = slot_group_bounds(&player_inventory_slots, 12.0) {
                            push_ui_panel(
                                &mut panel_vertices,
                                &mut panel_indices,
                                screen_size,
                                gameplay_panel(
                                    bounds,
                                    [0.04, 0.06, 0.09, 0.52],
                                    [0.18, 0.24, 0.32, 0.78],
                                    1.0,
                                ),
                            );
                        }

                        match gui_type {
                            Some(GuiType::Inventory) => {
                                let crafting_slots: Vec<SlotRect> =
                                    layout.player_crafting_slots.iter().flatten().copied().collect();
                                if let Some(bounds) = slot_group_bounds(&crafting_slots, 12.0) {
                                    push_ui_panel(
                                        &mut panel_vertices,
                                        &mut panel_indices,
                                        screen_size,
                                        gameplay_panel(
                                            bounds,
                                            [0.05, 0.07, 0.11, 0.60],
                                            [0.24, 0.32, 0.44, 0.82],
                                            1.0,
                                        ),
                                    );
                                }
                                if let Some(output) = layout.player_crafting_output {
                                    push_ui_panel(
                                        &mut panel_vertices,
                                        &mut panel_indices,
                                        screen_size,
                                        gameplay_panel(
                                            slot_panel_rect(output, 12.0),
                                            [0.05, 0.10, 0.07, 0.64],
                                            [0.30, 0.52, 0.34, 0.88],
                                            1.0,
                                        ),
                                    );
                                }
                            }
                            Some(GuiType::NVCrafter) => {
                                let crafting_slots: Vec<SlotRect> =
                                    layout.nvcrafter_slots.iter().flatten().copied().collect();
                                if let Some(bounds) = slot_group_bounds(&crafting_slots, 12.0) {
                                    push_ui_panel(
                                        &mut panel_vertices,
                                        &mut panel_indices,
                                        screen_size,
                                        gameplay_panel(
                                            bounds,
                                            [0.05, 0.07, 0.11, 0.60],
                                            [0.24, 0.32, 0.44, 0.82],
                                            1.0,
                                        ),
                                    );
                                }
                                if let Some(output) = layout.nvcrafter_output {
                                    push_ui_panel(
                                        &mut panel_vertices,
                                        &mut panel_indices,
                                        screen_size,
                                        gameplay_panel(
                                            slot_panel_rect(output, 12.0),
                                            [0.05, 0.10, 0.07, 0.64],
                                            [0.30, 0.52, 0.34, 0.88],
                                            1.0,
                                        ),
                                    );
                                }
                            }
                            None => {}
                        }
                    }
                } else {
                    push_ui_panel(
                        &mut panel_vertices,
                        &mut panel_indices,
                        screen_size,
                        gameplay_panel(
                            layout.hotbar_panel,
                            [0.05, 0.07, 0.10, 0.84],
                            [0.85, 0.90, 0.96, 0.88],
                            2.0,
                        ),
                    );

                    let break_fraction = self.interaction.break_fraction();
                    if break_fraction > 0.0 {
                        let bar_width = layout.hotbar_panel.width - 48.0;
                        let bar_height = 9.0;
                        let bar_x = layout.hotbar_panel.x + 24.0;
                        let bar_y = layout.hotbar_panel.y - 18.0;
                        push_ui_panel(
                            &mut panel_vertices,
                            &mut panel_indices,
                            screen_size,
                            UiPanel {
                                x: bar_x,
                                y: bar_y,
                                width: bar_width,
                                height: bar_height,
                                fill: [0.07, 0.09, 0.12, 0.92],
                                border_color: [0.88, 0.92, 0.98, 0.75],
                                border_thickness: 1.0,
                            },
                        );
                        push_ui_panel(
                            &mut panel_vertices,
                            &mut panel_indices,
                            screen_size,
                            UiPanel {
                                x: bar_x + 1.0,
                                y: bar_y + 1.0,
                                width: (bar_width - 2.0) * break_fraction,
                                height: bar_height - 2.0,
                                fill: [0.92, 0.64, 0.20, 0.98],
                                border_color: [0.0, 0.0, 0.0, 0.0],
                                border_thickness: 0.0,
                            },
                        );
                    }
                }

                let visible_slots = layout.visible_slots(gui_type);

                for &(slot_id, rect) in &visible_slots {
                    let is_active = matches!(slot_id, UiSlotId::Inventory(index) if index == active_slot);
                    let is_hovered = hovered_slot == Some(slot_id);
                    let is_output = slot_id.is_output();

                    let (fill, border_color, border_thickness) = if is_active {
                        ([0.25, 0.18, 0.08, 0.95], [0.98, 0.82, 0.34, 1.0], 2.0)
                    } else if is_hovered {
                        ([0.14, 0.18, 0.24, 0.95], [0.84, 0.90, 0.98, 0.92], 2.0)
                    } else if is_output {
                        ([0.08, 0.13, 0.10, 0.92], [0.48, 0.72, 0.50, 0.96], 1.5)
                    } else {
                        ([0.08, 0.11, 0.15, 0.92], [0.20, 0.24, 0.30, 0.96], 1.0)
                    };

                    push_ui_panel(
                        &mut panel_vertices,
                        &mut panel_indices,
                        screen_size,
                        UiPanel {
                            x: rect.x,
                            y: rect.y,
                            width: rect.size,
                            height: rect.size,
                            fill,
                            border_color,
                            border_thickness,
                        },
                    );

                    if let Some(stack) = self.interaction.stack_for_slot(world, slot_id) {
                        if let Some(block) = stack.block_type {
                            push_ui_sprite(
                                &mut sprite_vertices,
                                &mut sprite_indices,
                                screen_size,
                                rect.x + 7.0,
                                rect.y + 7.0,
                                rect.size - 14.0,
                                rect.size - 14.0,
                                block_icon_tile(block),
                                [1.0, 1.0, 1.0, 1.0],
                            );
                        }
                    }
                }

                if let Some(stack) = self.interaction.dragged_stack() {
                    if let (Some(block), Some((cursor_x, cursor_y))) = (stack.block_type, self.interaction.cursor_position()) {
                        push_ui_sprite(
                            &mut sprite_vertices,
                            &mut sprite_indices,
                            screen_size,
                            cursor_x - 18.0,
                            cursor_y - 18.0,
                            36.0,
                            36.0,
                            block_icon_tile(block),
                            [1.0, 1.0, 1.0, 0.88],
                        );
                    }
                }

                if let Some(prompt) = self.command_prompt_text.as_deref() {
                    let (text_w, text_h) = self
                        .text_renderer
                        .measure_text_size(prompt, 1.2)
                        .unwrap_or((320.0, 36.0));
                    let width = (text_w + 52.0)
                        .max(300.0)
                        .min(self.config.width as f32 - 48.0);
                    let height = (text_h + 24.0).max(56.0);
                    let x = self.config.width as f32 * 0.5 - width * 0.5;
                    let y = self.config.height as f32 - 36.0 - text_h - 12.0;
                    push_ui_panel(
                        &mut panel_vertices,
                        &mut panel_indices,
                        screen_size,
                        UiPanel {
                            x,
                            y,
                            width,
                            height,
                            fill: [0.05, 0.08, 0.12, 0.86],
                            border_color: [0.86, 0.90, 0.98, 0.95],
                            border_thickness: 2.0,
                        },
                    );
                }
            }
        }

        let panel_buffers = if panel_vertices.is_empty() || panel_indices.is_empty() {
            None
        } else {
            Some((
                self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("ui panel vertex buffer"),
                    contents: bytemuck::cast_slice(&panel_vertices),
                    usage: wgpu::BufferUsages::VERTEX,
                }),
                self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("ui panel index buffer"),
                    contents: bytemuck::cast_slice(&panel_indices),
                    usage: wgpu::BufferUsages::INDEX,
                }),
                panel_indices.len() as u32,
            ))
        };

        let sprite_buffers = if sprite_vertices.is_empty() || sprite_indices.is_empty() {
            None
        } else {
            Some((
                self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("ui sprite vertex buffer"),
                    contents: bytemuck::cast_slice(&sprite_vertices),
                    usage: wgpu::BufferUsages::VERTEX,
                }),
                self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("ui sprite index buffer"),
                    contents: bytemuck::cast_slice(&sprite_indices),
                    usage: wgpu::BufferUsages::INDEX,
                }),
                sprite_indices.len() as u32,
            ))
        };

        self.text_renderer.begin_frame(screen_size);
        match mode {
            UiMode::None => {
                let layout = build_inventory_layout(screen_size, gui_type);
                let visible_slots = layout.visible_slots(gui_type);

                if inventory_open {
                    if let Some((title_x, title_y)) = layout.title_position {
                        let title = match gui_type {
                            Some(GuiType::Inventory) => "INVENTORY",
                            Some(GuiType::NVCrafter) => "NVCRAFTER",
                            None => "",
                        };
                        let _ = self.text_renderer.draw_text_tinted(
                            &self.device,
                            &self.queue,
                            title_x,
                            title_y,
                            0.72,
                            title,
                            TextAlignment::Center,
                            [242, 245, 250, 255],
                        );
                    }

                    let player_inventory_slots: Vec<SlotRect> = layout.player_slot_rects.iter().copied().collect();
                    if let Some(bounds) = slot_group_bounds(&player_inventory_slots, 12.0) {
                        let label = match gui_type {
                            Some(GuiType::NVCrafter) => "Inventory",
                            _ => "Inventory",
                        };
                        let _ = self.text_renderer.draw_text_tinted(
                            &self.device,
                            &self.queue,
                            bounds.x + 8.0,
                            bounds.y - 18.0,
                            0.40,
                            label,
                            TextAlignment::Left,
                            [180, 190, 204, 255],
                        );
                    }

                    match gui_type {
                        Some(GuiType::Inventory) => {
                            let crafting_slots: Vec<SlotRect> =
                                layout.player_crafting_slots.iter().flatten().copied().collect();
                            if let Some(bounds) = slot_group_bounds(&crafting_slots, 12.0) {
                                let _ = self.text_renderer.draw_text_tinted(
                                    &self.device,
                                    &self.queue,
                                    bounds.x + 8.0,
                                    bounds.y - 18.0,
                                    0.40,
                                    "Crafting",
                                    TextAlignment::Left,
                                    [180, 190, 204, 255],
                                );
                                if let Some(output) = layout.player_crafting_output {
                                    let arrow_x = bounds.x + bounds.width + (output.x - (bounds.x + bounds.width)) * 0.5;
                                    let arrow_y = output.y + output.size * 0.5 - 9.0;
                                    let _ = self.text_renderer.draw_text_tinted(
                                        &self.device,
                                        &self.queue,
                                        arrow_x,
                                        arrow_y,
                                        0.54,
                                        "->",
                                        TextAlignment::Center,
                                        [204, 214, 226, 255],
                                    );
                                    let _ = self.text_renderer.draw_text_tinted(
                                        &self.device,
                                        &self.queue,
                                        output.x + output.size * 0.5,
                                        output.y - 18.0,
                                        0.40,
                                        "Result",
                                        TextAlignment::Center,
                                        [168, 202, 172, 255],
                                    );
                                }
                            }
                        }
                        Some(GuiType::NVCrafter) => {
                            let crafting_slots: Vec<SlotRect> =
                                layout.nvcrafter_slots.iter().flatten().copied().collect();
                            if let Some(bounds) = slot_group_bounds(&crafting_slots, 12.0) {
                                let _ = self.text_renderer.draw_text_tinted(
                                    &self.device,
                                    &self.queue,
                                    bounds.x + 8.0,
                                    bounds.y - 18.0,
                                    0.40,
                                    "Crafting",
                                    TextAlignment::Left,
                                    [180, 190, 204, 255],
                                );
                                if let Some(output) = layout.nvcrafter_output {
                                    let arrow_x = bounds.x + bounds.width + (output.x - (bounds.x + bounds.width)) * 0.5;
                                    let arrow_y = output.y + output.size * 0.5 - 9.0;
                                    let _ = self.text_renderer.draw_text_tinted(
                                        &self.device,
                                        &self.queue,
                                        arrow_x,
                                        arrow_y,
                                        0.54,
                                        "->",
                                        TextAlignment::Center,
                                        [204, 214, 226, 255],
                                    );
                                    let _ = self.text_renderer.draw_text_tinted(
                                        &self.device,
                                        &self.queue,
                                        output.x + output.size * 0.5,
                                        output.y - 18.0,
                                        0.40,
                                        "Result",
                                        TextAlignment::Center,
                                        [168, 202, 172, 255],
                                    );
                                }
                            }
                        }
                        None => {}
                    }
                }

                for &(slot_id, rect) in &visible_slots {
                    if let Some(stack) = self.interaction.stack_for_slot(world, slot_id) {
                        // Stack count — shown for any count including 1 so item presence is clear.
                        let count_label = if stack.count > 1 {
                            format!("×{}", stack.count)
                        } else {
                            String::new()
                        };
                        if !count_label.is_empty() {
                            let _ = self.text_renderer.draw_text_tinted(
                                &self.device,
                                &self.queue,
                                rect.x + rect.size - 4.0,
                                rect.y + rect.size - 14.0,
                                0.52,
                                &count_label,
                                TextAlignment::Right,
                                [255, 255, 96, 255],
                            );
                        }
                    }
                }

                // Hover tooltip: item name near cursor position.
                if let Some(hovered) = self.interaction.hovered_slot() {
                    if let Some(stack) = self.interaction.stack_for_slot(world, hovered) {
                        if let Some(block) = stack.block_type {
                            if let Some((cursor_x, cursor_y)) = self.interaction.cursor_position() {
                                let name = block.display_name();
                                let tip_x = (cursor_x + 16.0).min(screen_size.0 as f32 - 8.0);
                                let tip_y = (cursor_y - 28.0).max(4.0);
                                let _ = self.text_renderer.draw_text_tinted(
                                    &self.device,
                                    &self.queue,
                                    tip_x,
                                    tip_y,
                                    0.54,
                                    name,
                                    TextAlignment::Left,
                                    [240, 240, 240, 255],
                                );
                            }
                        }
                    }
                }

                // Dragged-stack count label follows the cursor.
                if let Some(stack) = self.interaction.dragged_stack() {
                    if let Some((cursor_x, cursor_y)) = self.interaction.cursor_position() {
                        if stack.count > 1 {
                            let _ = self.text_renderer.draw_text_tinted(
                                &self.device,
                                &self.queue,
                                cursor_x + 26.0,
                                cursor_y + 14.0,
                                0.52,
                                &format!("×{}", stack.count),
                                TextAlignment::Right,
                                [255, 255, 96, 255],
                            );
                        }
                    }
                }

                if let Some(prompt) = self.command_prompt_text.as_deref() {
                    let _ = self.text_renderer.render_command_prompt(&self.device, &self.queue, prompt);
                } else if !inventory_open {
                    if let Some(subtitle) = self.subtitle_text.as_deref() {
                        let _ = self.text_renderer.render_subtitle(&self.device, &self.queue, subtitle);
                    }
                }
            }
            UiMode::MainMenu | UiMode::PauseMenu => {
                let _ = self.menu_renderer.render_menu(&mut self.text_renderer, &self.device, &self.queue, mode, selection);
            }
        }

        let mut encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("encoder") });

        {
            let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("render pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(clear_color),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.depth_texture.view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                occlusion_query_set: None,
                timestamp_writes: None,
            });

            if mode == UiMode::None {
                rpass.set_pipeline(&self.render_pipeline);
                rpass.set_bind_group(0, &self.camera_bind_group, &[]);
                rpass.set_bind_group(1, &self.material_bind_group, &[]);
                rpass.set_bind_group(2, &self.texture_bind_group, &[]);
                rpass.set_bind_group(3, &self.biome_bind_group, &[]);
                if let Some(ref vb) = self.vertex_buffer {
                    rpass.set_vertex_buffer(0, vb.slice(..));
                }
                if let Some(ref ib) = self.index_buffer {
                    rpass.set_index_buffer(ib.slice(..), wgpu::IndexFormat::Uint32);
                    rpass.draw_indexed(0..self.num_indices, 0, 0..1);
                }

                rpass.set_pipeline(&self.water_pipeline);
                rpass.set_bind_group(0, &self.camera_bind_group, &[]);
                rpass.set_bind_group(1, &self.material_bind_group, &[]);
                rpass.set_bind_group(2, &self.texture_bind_group, &[]);
                rpass.set_bind_group(3, &self.biome_bind_group, &[]);
                if let Some(ref wb) = self.water_vertex_buffer {
                    rpass.set_vertex_buffer(0, wb.slice(..));
                }
                if let Some(ref ib) = self.water_index_buffer {
                    rpass.set_index_buffer(ib.slice(..), wgpu::IndexFormat::Uint32);
                    if self.num_water_indices > 0 {
                        rpass.draw_indexed(0..self.num_water_indices, 0, 0..1);
                    }
                }
            }

            if let Some((ref panel_vb, ref panel_ib, panel_index_count)) = panel_buffers {
                rpass.set_pipeline(&self.ui_pipeline);
                rpass.set_vertex_buffer(0, panel_vb.slice(..));
                rpass.set_index_buffer(panel_ib.slice(..), wgpu::IndexFormat::Uint16);
                rpass.draw_indexed(0..panel_index_count, 0, 0..1);
            }

            if let Some((ref sprite_vb, ref sprite_ib, sprite_index_count)) = sprite_buffers {
                rpass.set_pipeline(&self.ui_sprite_pipeline);
                rpass.set_bind_group(0, &self.texture_bind_group, &[]);
                rpass.set_vertex_buffer(0, sprite_vb.slice(..));
                rpass.set_index_buffer(sprite_ib.slice(..), wgpu::IndexFormat::Uint16);
                rpass.draw_indexed(0..sprite_index_count, 0, 0..1);
            }

            self.text_renderer.draw(&mut rpass);

            if mode == UiMode::None && !inventory_open && self.crosshair_index_count > 0 {
                rpass.set_pipeline(&self.ui_pipeline);
                rpass.set_vertex_buffer(0, self.crosshair_vertex_buffer.slice(..));
                rpass.set_index_buffer(self.crosshair_index_buffer.slice(..), wgpu::IndexFormat::Uint16);
                rpass.draw_indexed(0..self.crosshair_index_count, 0, 0..1);
            }
        }

        self.queue.submit(Some(encoder.finish()));
        output.present();
        Ok(())
    }
}