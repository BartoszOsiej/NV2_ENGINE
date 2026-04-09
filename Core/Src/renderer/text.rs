use std::path::Path;

use anyhow::{Context, Result};
use fontdue::{
    layout::{CoordinateSystem, GlyphPosition, Layout, LayoutSettings, TextStyle as LayoutTextStyle},
    Font, FontSettings,
};
use wgpu::util::DeviceExt;

const BASE_TEXT_PX: f32 = 28.0;
const MIN_TEXT_PX: f32 = 18.0;
const SUBTITLE_BOTTOM_MARGIN_PX: f32 = 36.0;
const COMMAND_PROMPT_BOTTOM_MARGIN_PX: f32 = 36.0;
const DEFAULT_OUTLINE_THICKNESS_PX: f32 = 2.0;

const WHITE: [u8; 4] = [255, 255, 255, 255];
const BLACK: [u8; 4] = [0, 0, 0, 255];

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct TextVertex {
    position: [f32; 2],
    uv: [f32; 2],
}

impl TextVertex {
    fn desc<'a>() -> wgpu::VertexBufferLayout<'a> {
        use std::mem;

        wgpu::VertexBufferLayout {
            array_stride: mem::size_of::<TextVertex>() as wgpu::BufferAddress,
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
            ],
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum TextAlignment {
    Left,
    Center,
    Right,
}

#[derive(Clone, Copy)]
struct TextBounds {
    min_x: f32,
    min_y: f32,
    width: f32,
    height: f32,
}

type PositionedGlyph = GlyphPosition<()>;

struct TextLayout {
    glyphs: Vec<PositionedGlyph>,
    bounds: TextBounds,
    texture_width: u32,
    texture_height: u32,
    padding: u32,
}

struct PreparedLayer {
    bind_group: wgpu::BindGroup,
    vertex_buffer: wgpu::Buffer,
    index_buffer: wgpu::Buffer,
    index_count: u32,
}

struct PreparedText {
    _textures: Vec<wgpu::Texture>,
    _views: Vec<wgpu::TextureView>,
    layers: Vec<PreparedLayer>,
}

pub struct TextRenderer {
    screen_size: (u32, u32),
    font: Option<Font>,
    outline_thickness_px: f32,
    bind_group_layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    pipeline: wgpu::RenderPipeline,
    frame_texts: Vec<PreparedText>,
}

impl TextRenderer {
    pub fn new(
        device: &wgpu::Device,
        format: wgpu::TextureFormat,
        screen_size: (u32, u32),
    ) -> Self {
        let shader = device.create_shader_module(wgpu::include_wgsl!("text.wgsl"));

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("text bind group layout"),
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

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("text pipeline layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("text pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: "vs_main",
                buffers: &[TextVertex::desc()],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: "fs_main",
                targets: &[Some(wgpu::ColorTargetState {
                    format,
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
                format: super::texture::Texture::DEPTH_FORMAT,
                depth_write_enabled: false,
                depth_compare: wgpu::CompareFunction::Always,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
        });

        Self {
            screen_size,
            font: None,
            outline_thickness_px: DEFAULT_OUTLINE_THICKNESS_PX,
            bind_group_layout,
            sampler,
            pipeline,
            frame_texts: Vec::new(),
        }
    }

    pub fn resize(&mut self, screen_size: (u32, u32)) {
        self.screen_size = screen_size;
    }

    pub fn screen_size(&self) -> (u32, u32) {
        self.screen_size
    }

    pub fn measure_text_size(&self, text: &str, scale: f32) -> Option<(f32, f32)> {
        self.measure_text(text, scale)
            .map(|bounds| (bounds.width, bounds.height))
    }

    pub fn set_outline_thickness(&mut self, thickness_px: f32) {
        self.outline_thickness_px = thickness_px.max(0.0);
    }

    pub fn begin_frame(&mut self, screen_size: (u32, u32)) {
        self.screen_size = screen_size;
        self.frame_texts.clear();
    }

    pub fn load_font_from_path<P: AsRef<Path>>(&mut self, path: P) -> Result<()> {
        let path = path.as_ref();
        let bytes = std::fs::read(path)
            .with_context(|| format!("failed to read font from {}", path.display()))?;
        let font = Font::from_bytes(bytes, FontSettings::default())
            .map_err(|err| anyhow::anyhow!("failed to parse font {}: {}", path.display(), err))?;
        self.font = Some(font);
        Ok(())
    }

    pub fn draw_text(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        screen_x: f32,
        screen_y: f32,
        scale: f32,
        text: &str,
        alignment: TextAlignment,
    ) -> Result<()> {
        self.draw_text_tinted(device, queue, screen_x, screen_y, scale, text, alignment, WHITE)
    }

    pub(crate) fn draw_text_tinted(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        screen_x: f32,
        screen_y: f32,
        scale: f32,
        text: &str,
        alignment: TextAlignment,
        color: [u8; 4],
    ) -> Result<()> {
        let Some(layout) = self.layout_text(text, scale) else {
            return Ok(());
        };

        let left = match alignment {
            TextAlignment::Left => screen_x,
            TextAlignment::Center => screen_x - layout.bounds.width * 0.5,
            TextAlignment::Right => screen_x - layout.bounds.width,
        }
        .round();

        let top = screen_y.round();
        let base_x = left - layout.padding as f32;
        let base_y = top - layout.padding as f32;

        let outline_pixels = self.build_glyph_bitmap(&layout, BLACK)?;
        let main_pixels = self.build_glyph_bitmap(&layout, color)?;

        let (outline_texture, outline_view) = self.create_texture_resources(
            device,
            queue,
            &outline_pixels,
            layout.texture_width,
            layout.texture_height,
            "text outline",
        );
        let (main_texture, main_view) = self.create_texture_resources(
            device,
            queue,
            &main_pixels,
            layout.texture_width,
            layout.texture_height,
            "text color",
        );

        let mut layers = Vec::new();
        let width = layout.texture_width as f32;
        let height = layout.texture_height as f32;
        for (dx, dy) in self.outline_offsets() {
            layers.push(self.create_layer(
                device,
                base_x + dx as f32,
                base_y + dy as f32,
                width,
                height,
                &outline_view,
            ));
        }
        layers.push(self.create_layer(device, base_x, base_y, width, height, &main_view));

        self.frame_texts.push(PreparedText {
            _textures: vec![outline_texture, main_texture],
            _views: vec![outline_view, main_view],
            layers,
        });
        Ok(())
    }

    pub fn render_subtitle(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        text: &str,
    ) -> Result<()> {
        let Some(bounds) = self.measure_text(text, 1.0) else {
            return Ok(());
        };

        let screen_x = self.screen_size.0 as f32 * 0.5;
        let screen_y = self.screen_size.1 as f32 - SUBTITLE_BOTTOM_MARGIN_PX - bounds.height;
        self.draw_text(device, queue, screen_x, screen_y, 1.0, text, TextAlignment::Center)
    }

    pub fn render_command_prompt(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        text: &str,
    ) -> Result<()> {
        let Some(bounds) = self.measure_text(text, 1.2) else {
            return Ok(());
        };

        let screen_x = self.screen_size.0 as f32 * 0.5;
        let screen_y = self.screen_size.1 as f32 - COMMAND_PROMPT_BOTTOM_MARGIN_PX - bounds.height;
        self.draw_text(device, queue, screen_x, screen_y, 1.2, text, TextAlignment::Center)
    }

    pub fn draw<'a>(&'a self, rpass: &mut wgpu::RenderPass<'a>) {
        if self.frame_texts.is_empty() {
            return;
        }

        rpass.set_pipeline(&self.pipeline);
        for prepared in &self.frame_texts {
            for layer in &prepared.layers {
                rpass.set_bind_group(0, &layer.bind_group, &[]);
                rpass.set_vertex_buffer(0, layer.vertex_buffer.slice(..));
                rpass.set_index_buffer(layer.index_buffer.slice(..), wgpu::IndexFormat::Uint16);
                rpass.draw_indexed(0..layer.index_count, 0, 0..1);
            }
        }
    }

    fn measure_text(&self, text: &str, scale: f32) -> Option<TextBounds> {
        self.layout_text(text, scale).map(|layout| layout.bounds)
    }

    fn layout_text(&self, text: &str, scale: f32) -> Option<TextLayout> {
        let font = self.font.as_ref()?;
        if text.trim().is_empty() {
            return None;
        }

        let mut layout = Layout::new(CoordinateSystem::PositiveYDown);
        layout.reset(&LayoutSettings {
            x: 0.0,
            y: 0.0,
            ..Default::default()
        });
        layout.append(&[font], &LayoutTextStyle::new(text, self.pixel_height(scale), 0));

        let glyphs = layout.glyphs().to_vec();
        let mut min_x = f32::MAX;
        let mut min_y = f32::MAX;
        let mut max_x = f32::MIN;
        let mut max_y = f32::MIN;

        for glyph in &glyphs {
            if glyph.width == 0 || glyph.height == 0 {
                continue;
            }
            min_x = min_x.min(glyph.x);
            min_y = min_y.min(glyph.y);
            max_x = max_x.max(glyph.x + glyph.width as f32);
            max_y = max_y.max(glyph.y + glyph.height as f32);
        }

        if min_x == f32::MAX || min_y == f32::MAX {
            return None;
        }

        let padding = self.outline_padding();
        let width = (max_x - min_x).ceil().max(1.0);
        let height = (max_y - min_y).ceil().max(1.0);

        Some(TextLayout {
            glyphs,
            bounds: TextBounds {
                min_x,
                min_y,
                width,
                height,
            },
            texture_width: width as u32 + padding * 2,
            texture_height: height as u32 + padding * 2,
            padding,
        })
    }

    fn build_glyph_bitmap(&self, layout: &TextLayout, color: [u8; 4]) -> Result<Vec<u8>> {
        let font = self
            .font
            .as_ref()
            .context("text renderer font has not been loaded")?;
        let mut pixels = vec![0u8; layout.texture_width as usize * layout.texture_height as usize * 4];

        for glyph in &layout.glyphs {
            if glyph.width == 0 || glyph.height == 0 {
                continue;
            }

            let (_, bitmap) = font.rasterize_config(glyph.key);
            let dest_x = (glyph.x - layout.bounds.min_x).round() as i32 + layout.padding as i32;
            let dest_y = (glyph.y - layout.bounds.min_y).round() as i32 + layout.padding as i32;
            blit_glyph(
                &mut pixels,
                layout.texture_width,
                layout.texture_height,
                glyph.width,
                glyph.height,
                dest_x,
                dest_y,
                &bitmap,
                color,
            );
        }

        Ok(pixels)
    }

    fn create_texture_resources(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        pixels: &[u8],
        width: u32,
        height: u32,
        label: &str,
    ) -> (wgpu::Texture, wgpu::TextureView) {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some(label),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        queue.write_texture(
            wgpu::ImageCopyTexture {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            pixels,
            wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(width * 4),
                rows_per_image: Some(height),
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );

        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());

        (texture, view)
    }

    fn create_layer(
        &self,
        device: &wgpu::Device,
        screen_x: f32,
        screen_y: f32,
        width: f32,
        height: f32,
        texture_view: &wgpu::TextureView,
    ) -> PreparedLayer {
        let vertices = quad_vertices(self.screen_size, screen_x, screen_y, width, height);
        let indices: [u16; 6] = [0, 1, 2, 0, 2, 3];

        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("text layer bind group"),
            layout: &self.bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(texture_view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
            ],
        });

        let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("text vertex buffer"),
            contents: bytemuck::cast_slice(&vertices),
            usage: wgpu::BufferUsages::VERTEX,
        });
        let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("text index buffer"),
            contents: bytemuck::cast_slice(&indices),
            usage: wgpu::BufferUsages::INDEX,
        });

        PreparedLayer {
            bind_group,
            vertex_buffer,
            index_buffer,
            index_count: indices.len() as u32,
        }
    }

    fn outline_offsets(&self) -> Vec<(i32, i32)> {
        let radius = self.outline_thickness_px.round() as i32;
        if radius <= 0 {
            return Vec::new();
        }

        let mut offsets = Vec::new();
        let limit = (self.outline_thickness_px * self.outline_thickness_px).ceil() as i32;
        for dy in -radius..=radius {
            for dx in -radius..=radius {
                if dx == 0 && dy == 0 {
                    continue;
                }
                if dx * dx + dy * dy <= limit {
                    offsets.push((dx, dy));
                }
            }
        }
        offsets
    }

    fn outline_padding(&self) -> u32 {
        self.outline_thickness_px.ceil() as u32 + 1
    }

    fn pixel_height(&self, scale: f32) -> f32 {
        let screen_factor = (self.screen_size.1 as f32 / 720.0).max(1.0);
        (BASE_TEXT_PX * scale * screen_factor).round().max(MIN_TEXT_PX)
    }
}

fn quad_vertices(screen_size: (u32, u32), screen_x: f32, screen_y: f32, width: f32, height: f32) -> [TextVertex; 4] {
    let screen_w = screen_size.0.max(1) as f32;
    let screen_h = screen_size.1.max(1) as f32;

    let left = screen_x.round();
    let top = screen_y.round();
    let right = (screen_x + width).round();
    let bottom = (screen_y + height).round();

    let left_ndc = left / screen_w * 2.0 - 1.0;
    let right_ndc = right / screen_w * 2.0 - 1.0;
    let top_ndc = 1.0 - top / screen_h * 2.0;
    let bottom_ndc = 1.0 - bottom / screen_h * 2.0;

    [
        TextVertex {
            position: [left_ndc, top_ndc],
            uv: [0.0, 0.0],
        },
        TextVertex {
            position: [right_ndc, top_ndc],
            uv: [1.0, 0.0],
        },
        TextVertex {
            position: [right_ndc, bottom_ndc],
            uv: [1.0, 1.0],
        },
        TextVertex {
            position: [left_ndc, bottom_ndc],
            uv: [0.0, 1.0],
        },
    ]
}

fn blit_glyph(
    pixels: &mut [u8],
    dest_width: u32,
    dest_height: u32,
    glyph_width: usize,
    glyph_height: usize,
    dest_x: i32,
    dest_y: i32,
    bitmap: &[u8],
    color: [u8; 4],
) {
    for row in 0..glyph_height {
        for col in 0..glyph_width {
            let alpha = bitmap[row * glyph_width + col];
            if alpha == 0 {
                continue;
            }

            let x = dest_x + col as i32;
            let y = dest_y + row as i32;
            if x < 0 || y < 0 || x >= dest_width as i32 || y >= dest_height as i32 {
                continue;
            }

            let pixel_index = (y as usize * dest_width as usize + x as usize) * 4;
            let src_alpha = alpha.saturating_mul(color[3]) / 255;
            let src_alpha = (((src_alpha as f32 / 255.0).powf(0.82) * 255.0).round() as u8)
                .max(src_alpha);
            if src_alpha > pixels[pixel_index + 3] {
                pixels[pixel_index] = color[0];
                pixels[pixel_index + 1] = color[1];
                pixels[pixel_index + 2] = color[2];
                pixels[pixel_index + 3] = src_alpha;
            }
        }
    }
}