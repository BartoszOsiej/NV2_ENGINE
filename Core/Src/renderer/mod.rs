use std::collections::HashMap;
use winit::window::Window;
use wgpu::util::DeviceExt;

pub mod camera;
pub mod vertices;
pub mod mesh;
pub mod texture_atlas;
pub mod texture;

use crate::world::World;
use crate::input::InputState;
use mesh::ChunkMesh;

pub const RENDER_RADIUS: i32 = 6;

struct GpuChunk {
    vertex_buffer: wgpu::Buffer,
    index_buffer:  wgpu::Buffer,
    num_indices:   u32,
}

pub struct State {
    pub surface:  wgpu::Surface<'static>,
    pub device:   wgpu::Device,
    pub queue:    wgpu::Queue,
    pub config:   wgpu::SurfaceConfiguration,
    pub size:     winit::dpi::PhysicalSize<u32>,
    pub window:   &'static Window,

    render_pipeline:   wgpu::RenderPipeline,
    depth_texture:     wgpu::TextureView,

    atlas_bind_group:  wgpu::BindGroup,
    pub camera:        camera::Camera,
    pub camera_uniform: camera::CameraUniform,
    pub camera_buffer:  wgpu::Buffer,
    camera_bind_group: wgpu::BindGroup,

    gpu_chunks: HashMap<(i32, i32), GpuChunk>,
}

impl State {
    pub async fn new(window: &'static Window) -> Self {
        let size = window.inner_size();
        // Force DirectX 12 backend for native Minecraft-like rendering on Windows.
        // If DX12 is not available, fall back to all available backends.
        let mut instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::DX12,
            dx12_shader_compiler: wgpu::Dx12Compiler::Fxc, // or Dxc if you prefer
            flags: wgpu::InstanceFlags::empty(),
            gles_minor_version: wgpu::Gles3MinorVersion::Version0,
        });
        let mut surface = instance.create_surface(window).unwrap();

        let mut adapter = instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: Some(&surface),
            force_fallback_adapter: false,
        }).await;

        if adapter.is_none() {
            eprintln!("DX12 adapter not found, falling back to all backend list.");
            instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
                backends: wgpu::Backends::all(),
                dx12_shader_compiler: wgpu::Dx12Compiler::Fxc,
                flags: wgpu::InstanceFlags::empty(),
                gles_minor_version: wgpu::Gles3MinorVersion::Version0,
            });
            surface = instance.create_surface(window).unwrap();
            adapter = instance.request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            }).await;
        }

        let adapter = adapter.expect("Failed to find any GPU adapter.");


        let (device, queue) = adapter.request_device(&wgpu::DeviceDescriptor {
            label: None,
            required_features: wgpu::Features::empty(),
            required_limits: wgpu::Limits::default(),
        }, None).await.unwrap();

        let surface_caps = surface.get_capabilities(&adapter);
        let surface_format = surface_caps.formats.iter().copied().find(|f| f.is_srgb()).unwrap_or(surface_caps.formats[0]);

        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width: size.width,
            height: size.height,
            present_mode: wgpu::PresentMode::Fifo,
            alpha_mode: surface_caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&device, &config);

        // Load a single atlas texture (trawa_kamien) and keep block UV logic simple.
        let atlas_texture = texture_atlas::AtlasTexture::from_bytes(&device, &queue, include_bytes!("../../../Assets/Atlas/trawa_kamien.png")).unwrap();

        let atlas_bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Atlas Bind Group Layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        multisampled: false,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
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

        let atlas_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &atlas_bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&atlas_texture.view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&atlas_texture.sampler),
                },
            ],
            label: Some("Atlas Bind Group"),
        });

        let camera = camera::Camera::new(config.width as f32 / config.height as f32);
        let mut camera_uniform = camera::CameraUniform::new();
        camera_uniform.update_view_proj(&camera);

        let camera_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Camera Buffer"),
            contents: bytemuck::cast_slice(&[camera_uniform]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST, // Naprawione usage
        });

        let camera_bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
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
            label: Some("camera_bind_group_layout"),
        });

        let camera_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &camera_bind_group_layout,
            entries: &[wgpu::BindGroupEntry { binding: 0, resource: camera_buffer.as_entire_binding() }],
            label: Some("camera_bind_group"),
        });

        let shader = device.create_shader_module(wgpu::include_wgsl!("../shader.wgsl"));
        let render_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Render Pipeline Layout"),
            bind_group_layouts: &[&atlas_bind_group_layout, &camera_bind_group_layout],
            push_constant_ranges: &[],
        });

        let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Render Pipeline"),
            layout: Some(&render_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs_main",
                buffers: &[vertices::Vertex::desc()],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: "fs_main",
                targets: &[Some(wgpu::ColorTargetState {
                    format: config.format,
                    blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None, // Wyłączone dla bezpieczeństwa testów
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

        let depth_texture = texture::Texture::create_depth_texture(&device, &config, "depth_texture");

        Self {
            surface, device, queue, config, size, window,
            render_pipeline,
            depth_texture: depth_texture.view,
            atlas_bind_group,
            camera, camera_uniform, camera_buffer, camera_bind_group,
            gpu_chunks: HashMap::new(),
        }
    }

    pub fn resize(&mut self, new_size: winit::dpi::PhysicalSize<u32>) {
        if new_size.width > 0 && new_size.height > 0 {
            self.size = new_size;
            self.config.width = new_size.width;
            self.config.height = new_size.height;
            self.surface.configure(&self.device, &self.config);
            self.depth_texture = texture::Texture::create_depth_texture(&self.device, &self.config, "depth_texture").view;
            self.camera.aspect = self.config.width as f32 / self.config.height as f32;
        }
    }

    pub fn update(&mut self, world: &World, input: &mut InputState, dt: f32) {
        // Mouse look and keyboard movement
        let (dx, dy) = input.take_mouse();
        self.camera.process_mouse(dx as f32, dy as f32);
        self.camera.process_keys(&input.keys_held, dt);
        self.camera.update_physics(dt, world);
        self.camera_uniform.update_view_proj(&self.camera);
        self.queue.write_buffer(&self.camera_buffer, 0, bytemuck::cast_slice(&[self.camera_uniform]));

        let (cx, cz) = (
            (self.camera.position.x / 16.0).floor() as i32,
            (self.camera.position.z / 16.0).floor() as i32,
        );

        for dz in -RENDER_RADIUS..=RENDER_RADIUS {
            for dx in -RENDER_RADIUS..=RENDER_RADIUS {
                let coords = (cx + dx, cz + dz);
                if !self.gpu_chunks.contains_key(&coords) {
                    if world.get_chunk(coords.0, coords.1).is_some() {
                        let mesh = ChunkMesh::generate(world, coords.0, coords.1);
                        if !mesh.indices.is_empty() {
                            let v_buf = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                                label: Some("Chunk VBuf"),
                                contents: bytemuck::cast_slice(&mesh.vertices),
                                usage: wgpu::BufferUsages::VERTEX,
                            });
                            let i_buf = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                                label: Some("Chunk IBuf"),
                                contents: bytemuck::cast_slice(&mesh.indices),
                                usage: wgpu::BufferUsages::INDEX,
                            });
                            self.gpu_chunks.insert(coords, GpuChunk {
                                vertex_buffer: v_buf,
                                index_buffer: i_buf,
                                num_indices: mesh.indices.len() as u32,
                            });
                        }
                    }
                }
            }
        }
    }

    pub fn render(&mut self) -> Result<(), wgpu::SurfaceError> {
        let output = self.surface.get_current_texture()?;
        let view = output.texture.create_view(&wgpu::TextureViewDescriptor::default());
        let mut enc = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("Render Encoder") });

        {
            let mut rpass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color { r: 0.5, g: 0.8, b: 1.0, a: 1.0 }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.depth_texture,
                    depth_ops: Some(wgpu::Operations { load: wgpu::LoadOp::Clear(1.0), store: wgpu::StoreOp::Store }),
                    stencil_ops: None,
                }),
                ..Default::default()
            });

            rpass.set_pipeline(&self.render_pipeline);
            rpass.set_bind_group(0, &self.atlas_bind_group, &[]);
            rpass.set_bind_group(1, &self.camera_bind_group, &[]);

            for chunk in self.gpu_chunks.values() {
                rpass.set_vertex_buffer(0, chunk.vertex_buffer.slice(..));
                rpass.set_index_buffer(chunk.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
                rpass.draw_indexed(0..chunk.num_indices, 0, 0..1);
            }
        }
        self.queue.submit(std::iter::once(enc.finish()));
        output.present();
        Ok(())
    }
}

