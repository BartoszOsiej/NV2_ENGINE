use std::collections::HashMap;
use winit::window::Window;
use wgpu::util::DeviceExt;

mod camera;
mod vertices;
mod mesh;
mod texture_atlas;

use crate::world::World;
use crate::input::InputState;
use mesh::ChunkMesh;

const RENDER_RADIUS: i32 = 6;

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

    render_pipeline: wgpu::RenderPipeline,
    depth_texture:   wgpu::TextureView,

    atlas_bind_group:  wgpu::BindGroup,
    camera:            camera::Camera,
    camera_uniform:    camera::CameraUniform,
    camera_buffer:     wgpu::Buffer,
    camera_bind_group: wgpu::BindGroup,

    gpu_chunks: HashMap<(i32,i32), GpuChunk>,
}

impl State {
    pub async fn new(window: &'static Window) -> Self {
        let size = window.inner_size();
        let inst = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::VULKAN,
            ..Default::default()
        });
        let surface  = inst.create_surface(window).unwrap();
        let adapter  = inst.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: Some(&surface),
            force_fallback_adapter: false,
        }).await.unwrap();
        let (device, queue) = adapter.request_device(
            &wgpu::DeviceDescriptor {
                label: Some("NV_Device"),
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::default(),
            }, None,
        ).await.unwrap();

        let caps   = surface.get_capabilities(&adapter);
        let format = caps.formats[0];
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width: size.width, height: size.height,
            present_mode: wgpu::PresentMode::Fifo,
            alpha_mode: caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&device, &config);

        // ── ATLAS TEXTURE ──────────────────────────────────────────
        let atlas_bytes = include_bytes!("../../../Assets/Atlas/trawa_kamien.png");
        let atlas = texture_atlas::AtlasTexture::from_bytes(&device, &queue, atlas_bytes).unwrap();
        let atlas_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: None,
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0, visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        multisampled: false,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    }, count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1, visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
        let atlas_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &atlas_bgl,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&atlas.view) },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::Sampler(&atlas.sampler) },
            ],
            label: None,
        });

        // ── CAMERA ────────────────────────────────────────────────
        let camera = camera::Camera::new(size.width as f32 / size.height as f32);
        let mut camera_uniform = camera::CameraUniform::new();
        camera_uniform.update(&camera);
        let camera_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Camera Buffer"),
            contents: bytemuck::cast_slice(&[camera_uniform]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let camera_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: None,
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0, visibility: wgpu::ShaderStages::VERTEX,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false, min_binding_size: None,
                }, count: None,
            }],
        });
        let camera_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &camera_bgl,
            entries: &[wgpu::BindGroupEntry { binding: 0, resource: camera_buffer.as_entire_binding() }],
            label: None,
        });

        // ── DEPTH BUFFER ──────────────────────────────────────────
        let depth_texture = Self::make_depth(&device, &config);

        // ── PIPELINE ─────────────────────────────────────────────
        let shader = device.create_shader_module(wgpu::include_wgsl!("../shader.wgsl"));
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: None,
            bind_group_layouts: &[&atlas_bgl, &camera_bgl],
            push_constant_ranges: &[],
        });
        let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: None, layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader, entry_point: "vs_main",
                buffers: &[vertices::Vertex::desc()],
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader, entry_point: "fs_main",
                targets: &[Some(wgpu::ColorTargetState {
                    format, blend: Some(wgpu::BlendState::REPLACE),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            primitive: wgpu::PrimitiveState {
                cull_mode: Some(wgpu::Face::Back),
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth32Float,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::Less,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
        });

        Self {
            surface, device, queue, config, size, window,
            render_pipeline, depth_texture,
            atlas_bind_group,
            camera, camera_uniform, camera_buffer, camera_bind_group,
            gpu_chunks: HashMap::new(),
        }
    }

    fn make_depth(device: &wgpu::Device, config: &wgpu::SurfaceConfiguration) -> wgpu::TextureView {
        device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Depth"),
            size: wgpu::Extent3d { width: config.width, height: config.height, depth_or_array_layers: 1 },
            mip_level_count: 1, sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Depth32Float,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        }).create_view(&wgpu::TextureViewDescriptor::default())
    }

    pub fn resize(&mut self, new_size: winit::dpi::PhysicalSize<u32>) {
        if new_size.width > 0 && new_size.height > 0 {
            self.size = new_size;
            self.config.width  = new_size.width;
            self.config.height = new_size.height;
            self.camera.aspect = new_size.width as f32 / new_size.height as f32;
            self.surface.configure(&self.device, &self.config);
            self.depth_texture = Self::make_depth(&self.device, &self.config);
        }
    }

    /// Called each frame from main loop
    pub fn update(&mut self, world: &mut World, input: &mut InputState, dt: f32) {
        // Camera movement
        let (mdx, mdy) = input.take_mouse();
        if input.mouse_captured {
            self.camera.process_mouse(mdx, mdy, 0.0015);
        }
        self.camera.process_keys(&input.keys_held, dt);
        self.camera_uniform.update(&self.camera);
        self.queue.write_buffer(&self.camera_buffer, 0, bytemuck::cast_slice(&[self.camera_uniform]));

        // Load chunks around player
        let cx = (self.camera.position.x / 16.0).floor() as i32;
        let cz = (self.camera.position.z / 16.0).floor() as i32;
        world.load_around(cx, cz, RENDER_RADIUS);

        // Upload dirty chunk meshes to GPU
        for (&(chx, chz), chunk) in &mut world.chunks {
            if !chunk.dirty { continue; }
            chunk.dirty = false;

            let mesh = ChunkMesh::build(world, chx, chz);
            if mesh.indices.is_empty() { continue; }

            let vb = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: None,
                contents: bytemuck::cast_slice(&mesh.vertices),
                usage: wgpu::BufferUsages::VERTEX,
            });
            let ib = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: None,
                contents: bytemuck::cast_slice(&mesh.indices),
                usage: wgpu::BufferUsages::INDEX,
            });

            self.gpu_chunks.insert((chx, chz), GpuChunk {
                vertex_buffer: vb,
                index_buffer:  ib,
                num_indices:   mesh.indices.len() as u32,
            });
        }

        // Unload far chunks
        let keep_radius = RENDER_RADIUS + 2;
        self.gpu_chunks.retain(|&(chx, chz), _| {
            (chx - cx).abs() <= keep_radius && (chz - cz).abs() <= keep_radius
        });
    }

    pub fn render(&mut self) -> Result<(), wgpu::SurfaceError> {
        let output = self.surface.get_current_texture()?;
        let view   = output.texture.create_view(&wgpu::TextureViewDescriptor::default());
        let mut enc = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
        {
            let mut rpass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: None,
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view, resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color { r: 0.45, g: 0.68, b: 0.98, a: 1.0 }), // sky
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.depth_texture,
                    depth_ops: Some(wgpu::Operations { load: wgpu::LoadOp::Clear(1.0), store: wgpu::StoreOp::Store }),
                    stencil_ops: None,
                }),
                occlusion_query_set: None, timestamp_writes: None,
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