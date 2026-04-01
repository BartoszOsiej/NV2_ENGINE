use anyhow::Result;
use std::path::Path;

use wgpu::util::DeviceExt;

pub struct TextRenderer {
    pub pipeline: wgpu::RenderPipeline,
    pub bind_group_layout: wgpu::BindGroupLayout,
    pub sampler: wgpu::Sampler,

    // Subtitles are explicit, shown only when set via `set_subtitle()`
    pub prepared_subtitles: Vec<PreparedText>,
    // Menu text entries (queued by menu code) and drawn only when menu is active
    pub prepared_menu_texts: Vec<PreparedText>,

    pub font: Option<fontdue::Font>,
    pub subtitle_texture_size: (u32, u32),
}

pub struct PreparedText {
    pub texture: wgpu::Texture,
    pub texture_view: wgpu::TextureView,
    pub bind_group: wgpu::BindGroup,
    pub vertex_buffer: wgpu::Buffer,
    pub index_buffer: wgpu::Buffer,
    pub index_count: u32,
}

impl TextRenderer {
    pub fn new(device: &wgpu::Device, config: &wgpu::SurfaceConfiguration) -> Self {
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("text texture layout"),
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

        let shader = device.create_shader_module(wgpu::include_wgsl!("text.wgsl"));
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("text pipeline layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        #[repr(C)]
        #[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
        struct TextVertex { position: [f32;2], uv: [f32;2] }

        let vertex_buffer_layout = wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<TextVertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                wgpu::VertexAttribute { offset: 0, shader_location: 0, format: wgpu::VertexFormat::Float32x2 },
                wgpu::VertexAttribute { offset: std::mem::size_of::<[f32;2]>() as wgpu::BufferAddress, shader_location: 1, format: wgpu::VertexFormat::Float32x2 },
            ],
        };

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("text pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs_main",
                buffers: &[vertex_buffer_layout],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: "fs_main",
                targets: &[Some(wgpu::ColorTargetState { format: config.format, blend: Some(wgpu::BlendState { color: wgpu::BlendComponent::OVER, alpha: wgpu::BlendComponent::OVER }), write_mask: wgpu::ColorWrites::ALL })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState { cull_mode: None, ..Default::default() },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: crate::renderer::texture::Texture::DEPTH_FORMAT,
                depth_write_enabled: false,
                depth_compare: wgpu::CompareFunction::Always,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
        });

        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("text sampler"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Nearest,
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            ..Default::default()
        });

        Self {
            pipeline,
            bind_group_layout,
            sampler,
            prepared_subtitles: Vec::new(),
            prepared_menu_texts: Vec::new(),
            font: None,
            subtitle_texture_size: (0,0),
        }
    }

    pub fn load_font_from_path<P: AsRef<Path>>(&mut self, path: P) -> Result<()> {
        let data = std::fs::read(path)?;
        match fontdue::Font::from_bytes(data.as_slice(), fontdue::FontSettings::default()) {
            Ok(f) => self.font = Some(f),
            Err(e) => return Err(anyhow::anyhow!("Failed to parse font: {:?}", e)),
        }
        Ok(())
    }

    /// Set subtitle text. Clears previous subtitle and replaces it.
    pub fn set_subtitle(&mut self, text: &str, device: &wgpu::Device, queue: &wgpu::Queue, screen_size: (u32,u32)) -> Result<()> {
        if self.font.is_none() { return Ok(()); }
        let font = self.font.as_ref().unwrap();

        // Clear any existing subtitle
        self.prepared_subtitles.clear();

        // Rasterize and upload texture (simple horizontal layout)
        let px_height = (screen_size.1 as f32 * 0.05).max(16.0);
        let mut glyphs: Vec<(fontdue::Metrics, Vec<u8>)> = Vec::new();
        let mut total_w: usize = 0;
        let mut max_h: usize = 0;
        for ch in text.chars() {
            let (metrics, bitmap) = font.rasterize(ch, px_height);
            total_w += metrics.width + 2;
            max_h = max_h.max(metrics.height);
            glyphs.push((metrics, bitmap));
        }
        if total_w == 0 || max_h == 0 { return Ok(()); }

        let width = total_w;
        let height = max_h;
        let mut img: Vec<u8> = vec![0u8; width * height * 4];
        let mut cursor = 0usize;
        for (metrics, bitmap) in glyphs {
            let w = metrics.width as usize; let h = metrics.height as usize;
            for y in 0..h {
                for x in 0..w {
                    let src = y * w + x;
                    let dst_x = cursor + x;
                    let dst_y = y;
                    let dst = (dst_y * width + dst_x) * 4;
                    let a = bitmap[src];
                    img[dst] = 255; img[dst+1] = 255; img[dst+2] = 255; img[dst+3] = a;
                }
            }
            cursor += w + 2;
        }

        let tex_size = wgpu::Extent3d { width: width as u32, height: height as u32, depth_or_array_layers: 1 };
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("subtitle texture"),
            size: tex_size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        queue.write_texture(
            wgpu::ImageCopyTexture { texture: &texture, mip_level: 0, origin: wgpu::Origin3d::ZERO, aspect: wgpu::TextureAspect::All },
            &img,
            wgpu::ImageDataLayout { offset: 0, bytes_per_row: Some((4 * width) as u32), rows_per_image: Some(height as u32) },
            tex_size,
        );

        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("subtitle bind group"),
            layout: &self.bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&view) },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::Sampler(&self.sampler) },
            ],
        });

        // Quad positioned near bottom (as before)
        let screen_w = screen_size.0 as f32; let screen_h = screen_size.1 as f32;
        let ndc_w = (width as f32 / screen_w) * 2.0; let ndc_h = (height as f32 / screen_h) * 2.0;
        let margin_px = (screen_h * 0.04).min(80.0);
        let y_px = margin_px;
        let x0 = -ndc_w * 0.5; let y0 = -1.0 + (y_px / screen_h) * 2.0; let x1 = x0 + ndc_w; let y1 = y0 + ndc_h;

        #[repr(C)] #[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)] struct TV { position: [f32;2], uv: [f32;2] }
        let verts: Vec<TV> = vec![ TV { position: [x0, y1], uv: [0.0,0.0] }, TV { position: [x1, y1], uv: [1.0,0.0] }, TV { position: [x1, y0], uv: [1.0,1.0] }, TV { position: [x0, y0], uv: [0.0,1.0] } ];
        let indices: Vec<u16> = vec![0,1,2, 0,2,3];

        let vb = device.create_buffer_init(&wgpu::util::BufferInitDescriptor { label: Some("subtitle vb"), contents: bytemuck::cast_slice(&verts), usage: wgpu::BufferUsages::VERTEX });
        let ib = device.create_buffer_init(&wgpu::util::BufferInitDescriptor { label: Some("subtitle ib"), contents: bytemuck::cast_slice(&indices), usage: wgpu::BufferUsages::INDEX });

        self.prepared_subtitles.push(PreparedText { texture, texture_view: view, bind_group, vertex_buffer: vb, index_buffer: ib, index_count: indices.len() as u32 });
        Ok(())
    }

    /// Queue a menu text entry at normal button label size (6% of screen height).
    pub fn queue_menu_text(&mut self, text: &str, device: &wgpu::Device, queue: &wgpu::Queue, screen_size: (u32,u32), center_y_px_top: f32) -> Result<()> {
        let px_height = (screen_size.1 as f32 * 0.06).max(14.0);
        self.queue_text_internal(text, device, queue, screen_size, center_y_px_top, px_height)
    }

    /// Queue a menu text entry at a custom size (scale relative to normal 6% height).
    pub fn queue_menu_text_sized(&mut self, text: &str, device: &wgpu::Device, queue: &wgpu::Queue, screen_size: (u32,u32), center_y_px_top: f32, scale: f32) -> Result<()> {
        let px_height = (screen_size.1 as f32 * 0.06 * scale).max(10.0);
        self.queue_text_internal(text, device, queue, screen_size, center_y_px_top, px_height)
    }

    fn queue_text_internal(&mut self, text: &str, device: &wgpu::Device, queue: &wgpu::Queue, screen_size: (u32,u32), center_y_px_top: f32, px_height: f32) -> Result<()> {
        let font = match self.font.as_ref() { Some(f) => f, None => return Ok(()) };

        // Rasterize all glyphs once; reuse alpha mask for both shadow and main layers.
        let mut glyphs: Vec<(fontdue::Metrics, Vec<u8>)> = Vec::new();
        let mut total_w: usize = 0;
        let mut max_h:   usize = 0;
        for ch in text.chars() {
            let (m, bm) = font.rasterize(ch, px_height);
            total_w += m.width.max(1) + 2;
            max_h    = max_h.max(m.height);
            glyphs.push((m, bm));
        }
        if total_w == 0 || max_h == 0 { return Ok(()); }
        let tw = total_w; let th = max_h;

        // Produce an RGBA pixel buffer with a given solid foreground colour (glyph alpha mask).
        let rasterize = |r: u8, g: u8, b: u8| -> Vec<u8> {
            let mut img = vec![0u8; tw * th * 4];
            let mut cur = 0usize;
            for (gm, bm) in &glyphs {
                let gw = gm.width as usize; let gh = gm.height as usize;
                for py in 0..gh {
                    for px in 0..gw {
                        let dx = cur + px; let dy = py;
                        if dx >= tw || dy >= th { continue; }
                        let d = (dy * tw + dx) * 4;
                        img[d] = r; img[d+1] = g; img[d+2] = b;
                        img[d+3] = bm[py * gw + px];
                    }
                }
                cur += gw.max(1) + 2;
            }
            img
        };

        let sw = screen_size.0 as f32; let sh = screen_size.1 as f32;
        let nw = (tw as f32 / sw) * 2.0;
        let nh = (th as f32 / sh) * 2.0;
        let cy  = 1.0 - (center_y_px_top / sh) * 2.0;
        let bx  = -nw * 0.5;
        let by0 = cy - nh * 0.5; let by1 = cy + nh * 0.5;
        // 3-pixel drop shadow in NDC units
        let sdx =  3.0 / sw * 2.0;
        let sdy = -3.0 / sh * 2.0;

        #[repr(C)] #[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
        struct TV { position: [f32;2], uv: [f32;2] }

        // Helper: upload one text layer as PreparedText.
        // Takes explicit refs so it does not borrow `self` (avoiding double-borrow).
        let make_layer = |img: &[u8], x_off: f32, y_off: f32,
                          device: &wgpu::Device, queue: &wgpu::Queue,
                          layout: &wgpu::BindGroupLayout,
                          sampler: &wgpu::Sampler| -> PreparedText {
            let tsz = wgpu::Extent3d { width: tw as u32, height: th as u32, depth_or_array_layers: 1 };
            let tex = device.create_texture(&wgpu::TextureDescriptor {
                label: None, size: tsz, mip_level_count: 1, sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8UnormSrgb,
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            });
            queue.write_texture(
                wgpu::ImageCopyTexture { texture: &tex, mip_level: 0, origin: wgpu::Origin3d::ZERO, aspect: wgpu::TextureAspect::All },
                img,
                wgpu::ImageDataLayout { offset: 0, bytes_per_row: Some((4 * tw) as u32), rows_per_image: Some(th as u32) },
                tsz,
            );
            let view = tex.create_view(&wgpu::TextureViewDescriptor::default());
            let bg   = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: None, layout,
                entries: &[
                    wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&view) },
                    wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::Sampler(sampler) },
                ],
            });
            let x0 = bx + x_off; let x1 = x0 + nw;
            let y0s = by0 + y_off; let y1s = by1 + y_off;
            let verts = [
                TV { position: [x0, y1s], uv: [0.0, 0.0] },
                TV { position: [x1, y1s], uv: [1.0, 0.0] },
                TV { position: [x1, y0s], uv: [1.0, 1.0] },
                TV { position: [x0, y0s], uv: [0.0, 1.0] },
            ];
            let idxs: [u16; 6] = [0, 1, 2, 0, 2, 3];
            let vb = device.create_buffer_init(&wgpu::util::BufferInitDescriptor { label: None, contents: bytemuck::cast_slice(&verts), usage: wgpu::BufferUsages::VERTEX });
            let ib = device.create_buffer_init(&wgpu::util::BufferInitDescriptor { label: None, contents: bytemuck::cast_slice(&idxs),  usage: wgpu::BufferUsages::INDEX  });
            PreparedText { texture: tex, texture_view: view, bind_group: bg, vertex_buffer: vb, index_buffer: ib, index_count: 6 }
        };

        // Shadow (dark, offset) is pushed first so it renders underneath.
        let shadow_img = rasterize(18, 18, 24);
        let shadow_pt  = make_layer(&shadow_img, sdx, sdy, device, queue, &self.bind_group_layout, &self.sampler);
        // Main white layer on top.
        let main_img   = rasterize(255, 255, 255);
        let main_pt    = make_layer(&main_img, 0.0, 0.0, device, queue, &self.bind_group_layout, &self.sampler);
        self.prepared_menu_texts.push(shadow_pt);
        self.prepared_menu_texts.push(main_pt);
        Ok(())
    }

    pub fn draw_menu<'a>(&'a self, rpass: &mut wgpu::RenderPass<'a>) {
        if self.prepared_menu_texts.is_empty() { return; }
        rpass.set_pipeline(&self.pipeline);
        for prepared in &self.prepared_menu_texts {
            rpass.set_bind_group(0, &prepared.bind_group, &[]);
            rpass.set_vertex_buffer(0, prepared.vertex_buffer.slice(..));
            rpass.set_index_buffer(prepared.index_buffer.slice(..), wgpu::IndexFormat::Uint16);
            rpass.draw_indexed(0..prepared.index_count, 0, 0..1);
        }
    }

    pub fn draw_subtitles<'a>(&'a self, rpass: &mut wgpu::RenderPass<'a>) {
        if self.prepared_subtitles.is_empty() { return; }
        rpass.set_pipeline(&self.pipeline);
        for prepared in &self.prepared_subtitles {
            rpass.set_bind_group(0, &prepared.bind_group, &[]);
            rpass.set_vertex_buffer(0, prepared.vertex_buffer.slice(..));
            rpass.set_index_buffer(prepared.index_buffer.slice(..), wgpu::IndexFormat::Uint16);
            rpass.draw_indexed(0..prepared.index_count, 0, 0..1);
        }
    }

    pub fn clear_menu_prepared(&mut self) { self.prepared_menu_texts.clear(); }
    pub fn clear_subtitle_prepared(&mut self) { self.prepared_subtitles.clear(); }
}
