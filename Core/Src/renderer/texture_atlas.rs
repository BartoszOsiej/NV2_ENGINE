use anyhow::*;
use image::GenericImageView;

pub struct AtlasTexture {
    pub view:    wgpu::TextureView,
    pub sampler: wgpu::Sampler,
}

impl AtlasTexture {
    pub fn from_bytes(device: &wgpu::Device, queue: &wgpu::Queue, bytes: &[u8]) -> Result<Self> {
        let img  = image::load_from_memory(bytes)?;
        let rgba = img.to_rgba8();
        let (w, h) = img.dimensions();

        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Atlas"),
            size: wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
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
            &rgba,
            wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(4 * w),
                rows_per_image: Some(h),
            },
            wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
        );

        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            mipmap_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });

        Ok(Self { view, sampler })
    }
}

/// Precomputed UV rect for a single tile in an atlas.
/// All values normalized [0.0, 1.0].
#[derive(Clone, Copy, Debug)]
pub struct TileUV {
    pub u0: f32, pub v0: f32,
    pub u1: f32, pub v1: f32,
}

impl TileUV {
    /// atlas_w/h = full atlas pixel size
    /// tx/ty     = tile top-left pixel
    /// tw/th     = tile pixel size
    pub fn new(atlas_w: f32, atlas_h: f32, tx: f32, ty: f32, tw: f32, th: f32) -> Self {
        Self {
            u0: tx / atlas_w,
            v0: ty / atlas_h,
            u1: (tx + tw) / atlas_w,
            v1: (ty + th) / atlas_h,
        }
    }

    pub fn uvs(&self) -> [[f32; 2]; 4] {
        [
            [self.u0, self.v0],
            [self.u1, self.v0],
            [self.u1, self.v1],
            [self.u0, self.v1],
        ]
    }
}

// ── trawa_kamien.png tile table (measured pixel-precisely) ───────────────────
// Grid: 4 cols × 4 rows
// Col x-starts: 11, 262, 517, 771   widths:  237, 241, 240, 240
// Row y-starts:  9, 249, 488, 724   heights: 227, 228, 225, 226
//
//  [0,0]=grass_top  [1,0]=grass_side  [2,0]=dirt       [3,0]=stone
//  [0,1]=sand       [1,1]=gravel      [2,1]=water       [3,1]=snow_side
//  [0,2]=snow_top   [1,2]=cobble      [2,2]=mossy_cob  [3,2]=bedrock
//  [0,3]=?          [1,3]=?           [2,3]=?           [3,3]=?

const TW: f32 = 1024.0;
const TH: f32 = 1024.0;

fn trawa(col: u32, row: u32) -> TileUV {
    let xs = [11.0f32, 262.0, 517.0, 771.0];
    let ws = [237.0f32, 241.0, 240.0, 240.0];
    let ys = [9.0f32,  249.0, 488.0, 724.0];
    let hs = [227.0f32, 228.0, 225.0, 226.0];
    TileUV::new(TW, TH, xs[col as usize], ys[row as usize], ws[col as usize], hs[row as usize])
}

// ── drewno_liscie.png tile table ─────────────────────────────────────────────
// 1024x1024, 4x4 grid
// Col x-starts: 133, 330, 530, 730   widths:  160, 162, 163, 160
// Row y-starts: 107, 287, 466, 642   heights: 136, 135, 132, 114
//
//  [0,0]=oak_bark    [1,0]=dark_bark   [2,0]=birch_bark  [3,0]=?
//  [0,1]=oak_top     [1,1]=dark_top    [2,1]=oak_top2    [3,1]=birch_top
//  [0,2]=oak_leaves  [1,2]=dark_leaves [2,2]=lime_leaves [3,2]=leaves4
//  [0,3]=oak_planks  [1,3]=dark_planks [2,3]=birch_plank [3,3]=gold_plank


// ── Public tile accessors used by block.rs ────────────────────────────────────

pub fn tile_grass_top()   -> TileUV { trawa(0, 0) }
pub fn tile_grass_side()  -> TileUV { trawa(1, 0) }
pub fn tile_dirt()        -> TileUV { trawa(2, 0) }
pub fn tile_stone()       -> TileUV { trawa(3, 0) }
pub fn tile_sand()        -> TileUV { trawa(0, 1) }
pub fn tile_gravel()      -> TileUV { trawa(1, 1) }
pub fn tile_water()       -> TileUV { trawa(2, 1) }
pub fn tile_snow_top()    -> TileUV { trawa(0, 2) }
pub fn tile_snow_side()   -> TileUV { trawa(3, 1) }
pub fn tile_coal_ore()    -> TileUV { trawa(3, 0) }
pub fn tile_gold_ore()    -> TileUV { trawa(3, 0) }
pub fn tile_diamond_ore() -> TileUV { trawa(3, 0) }