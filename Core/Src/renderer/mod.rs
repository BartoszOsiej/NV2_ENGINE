use wgpu::util::DeviceExt;
use winit::{window::Window, dpi::PhysicalSize};
use std::collections::HashMap;
// Tunable radii for loading/rendering
const LOAD_RADIUS: i32 = 6;
const RENDER_RADIUS: i32 = 4;
const CLEANUP_RADIUS: i32 = 6;

use crate::{assets, world::World};

pub mod camera;
pub mod texture_atlas;
pub mod mesh;
pub mod texture;
pub mod vertices;
pub mod texture_registry;
pub mod instance;
mod text;
use camera::*;
use vertices::Vertex;
use text::TextRenderer;

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct MaterialUniform {
    pub color_tint: [f32; 4],
}

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct BiomeUniform {
    /// xyz = ambient rgb tint, w = ambient multiplier
    pub ambient:   [f32; 4],
    /// x = water animation time, y = day brightness (0.15 night … 1.0 noon), z/w reserved
    pub time_info: [f32; 4],
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

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum UiMode {
    None,
    MainMenu,
    PauseMenu,
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
    pub text_renderer: Option<TextRenderer>,

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
    pub last_ui_mode: UiMode,
    pub last_ui_selection: Option<usize>,
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
    // Cached menu GPU buffers — rebuilt only when mode or selection changes
    cached_ui_vertex_buffer: Option<wgpu::Buffer>,
    cached_ui_index_buffer: Option<wgpu::Buffer>,
    cached_ui_index_count: u32,
    /// Monotonically increasing session time (seconds). Drives day/night + water anim.
    elapsed_time: f32,
}

// ── Module-level helpers ────────────────────────────────────────────────────

/// Pack all visible chunk meshes in a square of `radius` around `(cx, cz)`
/// into flattened vertex/index arrays ready for a GPU upload.
fn combine_meshes(
    meshes: &HashMap<(i32, i32), mesh::ChunkMesh>,
    cx: i32, cz: i32, radius: i32,
) -> (Vec<Vertex>, Vec<u32>) {
    let mut verts = Vec::new();
    let mut idxs  = Vec::new();
    for dz in -radius..=radius {
        for dx in -radius..=radius {
            if let Some(m) = meshes.get(&(cx + dx, cz + dz)) {
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

/// Try font paths in priority order; stops after the first successful load.
fn load_font(tr: &mut TextRenderer) {
    let candidates: &[&str] = &[
        // Project display font (paths relative to the working directory)
        "Core/Assets/Fonts/Subtitles/RubikBurned-Regular.ttf",
        "Assets/Fonts/Subtitles/RubikBurned-Regular.ttf",
        "../Assets/Fonts/Subtitles/RubikBurned-Regular.ttf",
        "../../Assets/Fonts/Subtitles/RubikBurned-Regular.ttf",
        // Readable Windows system fonts
        r"C:\Windows\Fonts\segoeui.ttf",
        r"C:\Windows\Fonts\arial.ttf",
        r"C:\Windows\Fonts\calibri.ttf",
        r"C:\Windows\Fonts\tahoma.ttf",
        r"C:\Windows\Fonts\verdana.ttf",
    ];
    for path in candidates {
        if std::path::Path::new(path).exists() && tr.load_font_from_path(path).is_ok() {
            return;
        }
    }
    // Last resort: let the asset system locate any available subtitle font.
    if let Ok(Some(path)) = crate::assets::ensure_subtitle_font() {
        let _ = tr.load_font_from_path(&path);
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

        let default_biome = BiomeUniform { ambient: [1.0, 1.0, 1.0, 1.0], time_info: [0.0, 1.0, 0.0, 0.0] };
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

        let mut text_renderer = TextRenderer::new(&device, &config);
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
            last_ui_mode: UiMode::None,
            last_ui_selection: None,
            water_sim_dirty: false,
            needs_gpu_upload: false,
            mesh_rebuild_timer: 0.0,
            water_mesh_rebuild_timer: 0.0,
            needs_water_combine: false,
            meshes_to_build: std::collections::VecDeque::new(),
            cached_ui_vertex_buffer: None,
            cached_ui_index_buffer: None,
            cached_ui_index_count: 0,
            elapsed_time: 0.0,
            text_renderer: Some(text_renderer),
        }
    }

    fn ui_clear_color(mode: UiMode, elapsed: f32) -> wgpu::Color {
        match mode {
            UiMode::None      => Self::sky_color_for_time(elapsed),
            UiMode::MainMenu  => wgpu::Color { r: 0.06, g: 0.08, b: 0.12, a: 1.0 },
            UiMode::PauseMenu => wgpu::Color { r: 0.04, g: 0.05, b: 0.08, a: 1.0 },
        }
    }

    /// Smooth day/night sky gradient. `t` is session seconds; 1200-second full day cycle.
    fn sky_color_for_time(t: f32) -> wgpu::Color {
        use std::f32::consts::TAU;
        let phase = (t / 1200.0).fract();
        // noon_blend: 0.0 at midnight, 1.0 at noon
        let noon = ((phase * TAU - std::f32::consts::FRAC_PI_2).sin() * 0.5 + 0.5).max(0.0_f32);
        // dawn_blend: peak around t=0.25 and t=0.75
        let dawn = (1.0 - (phase * 2.0 - 0.5).abs().min(1.0)).powf(2.0) * 0.70;
        let r = (0.02 + noon * 0.33 + dawn * 0.55).min(1.0) as f64;
        let g = (0.03 + noon * 0.62 + dawn * 0.22).min(1.0) as f64;
        let b = (0.08 + noon * 0.82 + dawn * 0.05).min(1.0) as f64;
        wgpu::Color { r, g, b, a: 1.0 }
    }

    /// Day brightness: 0.15 at midnight, 1.0 at noon (drives shader diffuse/ambient).
    fn day_brightness(t: f32) -> f32 {
        use std::f32::consts::TAU;
        let phase = (t / 1200.0).fract();
        let v = ((phase * TAU - std::f32::consts::FRAC_PI_2).sin() * 0.5 + 0.5).max(0.0);
        0.15 + 0.85 * v.sqrt()
    }

    fn build_menu_vertices(mode: UiMode, selected_index: Option<usize>) -> (Vec<UiVertex>, Vec<u16>) {
        let mut vertices = Vec::new();
        let mut indices: Vec<u16> = Vec::new();

        fn add_rect(vertices: &mut Vec<UiVertex>, indices: &mut Vec<u16>, x0: f32, y0: f32, x1: f32, y1: f32, color: [f32; 4]) {
            let base = vertices.len() as u16;
            vertices.push(UiVertex { position: [x0, y0], color });
            vertices.push(UiVertex { position: [x1, y0], color });
            vertices.push(UiVertex { position: [x1, y1], color });
            vertices.push(UiVertex { position: [x0, y1], color });
            indices.extend_from_slice(&[base, base+1, base+2, base, base+2, base+3]);
        }

        fn add_button(vertices: &mut Vec<UiVertex>, indices: &mut Vec<u16>, sel: Option<usize>, idx: usize, x0: f32, y0: f32, x1: f32, y1: f32, color: [f32; 4]) {
            if sel == Some(idx) {
                add_rect(vertices, indices, x0 - 0.02, y0 + 0.02, x1 + 0.02, y1 - 0.02, [0.16, 0.48, 0.95, 0.35]);
            }
            add_rect(vertices, indices, x0, y0, x1, y1, color);
        }

        // Shared background pane drawn only for menu modes;
        // UiMode::None gets a crosshair instead.
        match mode {
            UiMode::MainMenu => {
                // Background overlay
                add_rect(&mut vertices, &mut indices, -0.76, 0.74, 0.76, -0.74, [0.04, 0.05, 0.08, 0.92]);
                add_rect(&mut vertices, &mut indices, -0.72, 0.70, 0.72, -0.70, [0.09, 0.10, 0.14, 0.94]);
                // Title bar
                add_rect(&mut vertices, &mut indices, -0.60, 0.62, 0.60, 0.44, [0.15, 0.50, 0.90, 1.0]);
                // Buttons: evenly spaced in NDC space
                add_button(&mut vertices, &mut indices, selected_index, 0, -0.55, 0.36, 0.55, 0.22, [0.20, 0.62, 0.32, 1.0]);
                add_button(&mut vertices, &mut indices, selected_index, 1, -0.55, 0.14, 0.55, 0.00, [0.88, 0.65, 0.18, 1.0]);
                add_button(&mut vertices, &mut indices, selected_index, 2, -0.55, -0.08, 0.55, -0.22, [0.80, 0.20, 0.28, 1.0]);
            }
            UiMode::PauseMenu => {
                // Background overlay
                add_rect(&mut vertices, &mut indices, -0.76, 0.74, 0.76, -0.74, [0.04, 0.05, 0.08, 0.92]);
                add_rect(&mut vertices, &mut indices, -0.72, 0.70, 0.72, -0.70, [0.09, 0.10, 0.14, 0.94]);
                // Title bar
                add_rect(&mut vertices, &mut indices, -0.60, 0.62, 0.60, 0.46, [0.60, 0.30, 0.90, 1.0]);
                // Buttons
                add_button(&mut vertices, &mut indices, selected_index, 0, -0.55, 0.38, 0.55, 0.24, [0.28, 0.70, 0.25, 1.0]);
                add_button(&mut vertices, &mut indices, selected_index, 1, -0.55, 0.16, 0.55, 0.02, [0.92, 0.74, 0.20, 1.0]);
                add_button(&mut vertices, &mut indices, selected_index, 2, -0.55, -0.06, 0.55, -0.20, [0.85, 0.28, 0.30, 1.0]);
                add_button(&mut vertices, &mut indices, selected_index, 3, -0.55, -0.28, 0.55, -0.42, [0.45, 0.45, 0.55, 1.0]);
            }
            UiMode::None => {
                // Crosshair: two thin white rectangles crossing at screen centre.
                add_rect(&mut vertices, &mut indices, -0.028,  0.004, 0.028, -0.004, [1.0, 1.0, 1.0, 0.85]); // horizontal bar
                add_rect(&mut vertices, &mut indices, -0.004,  0.044, 0.004, -0.044, [1.0, 1.0, 1.0, 0.85]); // vertical bar
            }
        }

        (vertices, indices)
    }

    pub fn resize(&mut self, new_size: PhysicalSize<u32>) {
        if new_size.width > 0 && new_size.height > 0 {
            self.size = new_size;
            self.config.width = new_size.width;
            self.config.height = new_size.height;
            self.surface.configure(&self.device, &self.config);
            self.depth_texture = texture::Texture::create_depth_texture(&self.device, &self.config, "depth texture");
        }
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
        self.last_ui_mode = UiMode::None;
        self.last_ui_selection = None;
        self.water_sim_dirty = false;
        self.needs_gpu_upload = false;
        self.mesh_rebuild_timer = 0.0;
        self.water_mesh_rebuild_timer = 0.0;
        self.needs_water_combine = false;
        self.meshes_to_build.clear();
        self.cached_ui_vertex_buffer = None;
        self.cached_ui_index_buffer = None;
        self.cached_ui_index_count = 0;
        // elapsed_time intentionally not reset — day/night cycle persists
    }

    pub fn update(&mut self, world: &mut World, input: &mut crate::input::InputState, dt: f32) {
        self.camera.handle_input(input, dt);
        self.camera.update_physics(world, dt);
        self.camera_uniform.update_view_proj(&self.camera, self.config.width as f32 / self.config.height as f32);
        self.queue.write_buffer(&self.camera_buffer, 0, bytemuck::cast_slice(&[self.camera_uniform]));
        let material_uniform = MaterialUniform { color_tint: [self.top_tint[0], self.top_tint[1], self.top_tint[2], 0.0] };
        self.queue.write_buffer(&self.material_buffer, 0, bytemuck::cast_slice(&[material_uniform]));

        // Update biome ambient + time uniforms
        let cam_x = self.camera.position.x.round() as i32;
        let cam_z = self.camera.position.z.round() as i32;
        let ambient = world.ambient_at(cam_x, cam_z);
        self.elapsed_time += dt;
        let water_time  = self.elapsed_time * 0.55;
        let day_bright  = Self::day_brightness(self.elapsed_time);
        let biome_uniform = BiomeUniform { ambient, time_info: [water_time, day_bright, 0.0, 0.0] };
        self.queue.write_buffer(&self.biome_buffer, 0, bytemuck::cast_slice(&[biome_uniform]));

        // Water simulation — throttled to 2.0 s intervals.
        // The simulate_step() scan cost is proportional to loaded chunk count;
        // running it less frequently keeps frame pacing smooth.
        // Mesh rebuild is separately throttled to 2.5 s (see below).
        self.water_sim_timer += dt;
        if self.water_sim_timer >= 2.0 {
            self.water_sim_timer = 0.0;
            world.simulate_water();
            self.water_sim_dirty = true;
        }

        // Process any chunks that finished generating in background threads
        let new_chunks = world.process_generated_chunks();

        // Queue chunk generation based on camera position
        let cx = (self.camera.position.x / 16.0).floor() as i32;
        let cz = (self.camera.position.z / 16.0).floor() as i32;

        world.load_around(cx, cz, LOAD_RADIUS);
        world.unload_far_chunks(cx, cz, LOAD_RADIUS);

        let chunk_moved = (cx, cz) != self.prev_chunk;

        // ── Step 1: Enqueue any new/missing chunks that need mesh building ──
        // Never build inside this loop — just enqueue so we can rate-limit below.
        if new_chunks {
            for dz in -RENDER_RADIUS..=RENDER_RADIUS {
                for dx in -RENDER_RADIUS..=RENDER_RADIUS {
                    let key = (cx + dx, cz + dz);
                    if !self.chunk_meshes.contains_key(&key)
                        && !self.meshes_to_build.contains(&key)
                        && world.get_chunk(key.0, key.1).is_some()
                    {
                        self.meshes_to_build.push_back(key);
                    }
                }
            }
        }

        // ── Drain build queue: ≤4 chunks per frame ─────────────────────────────
        // Spreading work across frames prevents the multi-second freeze when
        // entering a new world or walking into an unloaded area.
        let mut built_this_frame = 0u32;
        while built_this_frame < 4 {
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

            let (verts, idxs)    = combine_meshes(&self.chunk_meshes, cx, cz, RENDER_RADIUS);
            self.num_indices      = idxs.len() as u32;
            let (vb, ib)          = upload_pair(&self.device, &verts, &idxs);
            self.vertex_buffer    = vb;
            self.index_buffer     = ib;
            self.current_vertices = verts;
            self.current_indices  = idxs;
            // Water is handled separately; no full water rebuild necessary here.
        }

        // ── Water mesh FULL REBUILD (throttled: ≤ once per 2.5 s) ─────────────
        // Triggered by the liquid simulation tick. Geometry is rebuilt from the
        // current world state, then signals a buffer-combine below.
        self.water_mesh_rebuild_timer += dt;
        if self.water_sim_dirty && self.water_mesh_rebuild_timer >= 2.5 {
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
            let (wverts, widxs)       = combine_meshes(&self.water_meshes, cx, cz, RENDER_RADIUS);
            self.num_water_indices    = widxs.len() as u32;
            let (wvb, wib)            = upload_pair(&self.device, &wverts, &widxs);
            self.water_vertex_buffer  = wvb;
            self.water_index_buffer   = wib;
        }
    }

    /// Display a subtitle string using the loaded font (if any).
    pub fn show_subtitle(&mut self, text: &str) {
        if let Some(ref mut tr) = self.text_renderer {
            let _ = tr.set_subtitle(text, &self.device, &self.queue, (self.config.width, self.config.height));
        }
    }

    pub fn render(&mut self, _world: &World, mode: UiMode, selection: Option<usize>) -> Result<(), wgpu::SurfaceError> {
        let output = self.surface.get_current_texture()?;
        let view = output.texture.create_view(&wgpu::TextureViewDescriptor::default());
        let clear_color = Self::ui_clear_color(mode, self.elapsed_time);

        // Rebuild UI geometry only when mode or selection changes
        let ui_changed = mode != self.last_ui_mode || selection != self.last_ui_selection;
        if ui_changed {
            // build_menu_vertices returns crosshair geometry for None mode too
            let (ui_vertices, ui_indices) = Self::build_menu_vertices(mode, selection);
            if !ui_vertices.is_empty() && !ui_indices.is_empty() {
                self.cached_ui_vertex_buffer = Some(self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("ui vertex buffer"),
                    contents: bytemuck::cast_slice(&ui_vertices),
                    usage: wgpu::BufferUsages::VERTEX,
                }));
                self.cached_ui_index_buffer = Some(self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("ui index buffer"),
                    contents: bytemuck::cast_slice(&ui_indices),
                    usage: wgpu::BufferUsages::INDEX,
                }));
                self.cached_ui_index_count = ui_indices.len() as u32;
            } else {
                self.cached_ui_vertex_buffer = None;
                self.cached_ui_index_buffer = None;
                self.cached_ui_index_count = 0;
            }

            // Rebuild font text only when mode or selection changes
            if let Some(ref mut tr) = self.text_renderer {
                if mode == UiMode::None {
                    if self.last_ui_mode != UiMode::None {
                        tr.clear_menu_prepared();
                    }
                } else {
                    tr.clear_menu_prepared();
                    let screen_h = self.config.height as f32;
                    let screen_sz = (self.config.width, self.config.height);
                    match mode {
                        UiMode::MainMenu => {
                            // Title
                            let title_py = (1.0 - 0.53f32) * screen_h * 0.5;
                            let _ = tr.queue_menu_text_sized("NV  ENGINE", &self.device, &self.queue, screen_sz, title_py, 1.4);
                            // Button labels
                            let ndc_centers = [0.29, 0.07, -0.15];
                            let labels = ["NEW GAME", "LOAD SAVE", "QUIT"];
                            for (i, lbl) in labels.iter().enumerate() {
                                let py = (1.0 - ndc_centers[i]) * screen_h * 0.5;
                                let _ = tr.queue_menu_text(lbl, &self.device, &self.queue, screen_sz, py);
                            }
                            // Description for the selected button
                            let descriptions = [
                                "Start a fresh world from scratch.",
                                "Load the previously saved world from disk.",
                                "Quit the application.",
                            ];
                            let sel = selection.unwrap_or(0);
                            if sel < descriptions.len() {
                                let desc_py = (1.0 - (-0.57f32)) * screen_h * 0.5;
                                let _ = tr.queue_menu_text_sized(descriptions[sel], &self.device, &self.queue, screen_sz, desc_py, 0.55);
                            }
                        }
                        UiMode::PauseMenu => {
                            // Title
                            let title_py = (1.0 - 0.54f32) * screen_h * 0.5;
                            let _ = tr.queue_menu_text_sized("PAUSED", &self.device, &self.queue, screen_sz, title_py, 1.4);
                            // Button labels
                            let ndc_centers = [0.31, 0.09, -0.13, -0.35];
                            let labels = ["RESUME", "SAVE", "SAVE + EXIT", "EXIT"];
                            for (i, lbl) in labels.iter().enumerate() {
                                let py = (1.0 - ndc_centers[i]) * screen_h * 0.5;
                                let _ = tr.queue_menu_text(lbl, &self.device, &self.queue, screen_sz, py);
                            }
                            // Description for the selected button
                            let descriptions = [
                                "Return to gameplay immediately.",
                                "Save the current world to disk.",
                                "Save the world and return to the main menu.",
                                "Return to the main menu without saving.",
                            ];
                            let sel = selection.unwrap_or(0);
                            if sel < descriptions.len() {
                                let desc_py = (1.0 - (-0.57f32)) * screen_h * 0.5;
                                let _ = tr.queue_menu_text_sized(descriptions[sel], &self.device, &self.queue, screen_sz, desc_py, 0.55);
                            }
                        }
                        UiMode::None => {}
                    }
                }
            }

            self.last_ui_mode = mode;
            self.last_ui_selection = selection;
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

            // Opaque world geometry
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

            // Translucent water pass
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

            // UI overlay (menu)
            if let (Some(ref uvb), Some(ref uib)) = (&self.cached_ui_vertex_buffer, &self.cached_ui_index_buffer) {
                if self.cached_ui_index_count > 0 {
                    rpass.set_pipeline(&self.ui_pipeline);
                    rpass.set_vertex_buffer(0, uvb.slice(..));
                    rpass.set_index_buffer(uib.slice(..), wgpu::IndexFormat::Uint16);
                    rpass.draw_indexed(0..self.cached_ui_index_count, 0, 0..1);
                }
            }

            if let Some(ref tr) = self.text_renderer {
                if mode != UiMode::None {
                    tr.draw_menu(&mut rpass);
                } else {
                    tr.draw_subtitles(&mut rpass);
                }
            }
        }

        self.queue.submit(Some(encoder.finish()));
        output.present();
        Ok(())
    }
}