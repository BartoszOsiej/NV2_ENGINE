use anyhow::*;
use image::{GenericImageView, RgbaImage};
use std::path::Path;
use std::result::Result::Ok;

pub struct AtlasTexture {
    pub view:    wgpu::TextureView,
    pub sampler: wgpu::Sampler,
}

impl AtlasTexture {
    pub async fn new(device: &wgpu::Device, queue: &wgpu::Queue) -> Result<Self> {
        let atlas_paths = [
            Path::new("Assets/Atlas/atlas.png"),
            Path::new("Assets/Atlas/terrain.png"),
            Path::new("../Assets/Atlas/atlas.png"),
            Path::new("../Assets/Atlas/terrain.png"),
            Path::new("../../Assets/Atlas/atlas.png"),
            Path::new("../../Assets/Atlas/terrain.png"),
            Path::new("../../../Assets/Atlas/atlas.png"),
            Path::new("../../../Assets/Atlas/terrain.png"),
        ];

        // Build atlas image from file, composed tiles, or fallback solid colour.
        let mut atlas_img: RgbaImage = 'load: {
            for atlas_path in atlas_paths {
                if let Ok(img) = image::open(atlas_path) {
                    let rgba = img.to_rgba8();
                    let (w, h) = img.dimensions();
                    break 'load if w != 512 || h != 320 {
                        let dyn_img = image::DynamicImage::ImageRgba8(rgba);
                        image::imageops::resize(&dyn_img, 512, 320, image::imageops::FilterType::Nearest)
                    } else {
                        rgba
                    };
                }
            }
            if let Some(composed) = Self::compose_from_blocks() {
                break 'load composed;
            }
            // Last-resort fallback: solid white checkerboard
            let mut fb = RgbaImage::new(512, 320);
            for y in 0..320u32 {
                for x in 0..512u32 {
                    let v = if (x / 16 + y / 16) % 2 == 0 { 255 } else { 200 };
                    fb.put_pixel(x, y, image::Rgba([v, v, v, 255]));
                }
            }
            fb
        };

        // Overwrite water tile slots with procedurally-generated textures so that
        // water always looks correct regardless of which atlas was loaded.
        Self::inject_water_tiles(&mut atlas_img);

        let (w, h) = atlas_img.dimensions();
        let bytes = atlas_img.into_raw();
        Self::from_bytes(device, queue, &bytes, w, h)
    }

    pub fn from_bytes(device: &wgpu::Device, queue: &wgpu::Queue, rgba_bytes: &[u8], width: u32, height: u32) -> Result<Self> {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Atlas"),
            size: wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
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
            rgba_bytes,
            wgpu::ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(width * 4),
                rows_per_image: Some(height),
            },
            wgpu::Extent3d { width, height, depth_or_array_layers: 1 },
        );

        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::Repeat,
            address_mode_w: wgpu::AddressMode::Repeat,
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            mipmap_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });

        Ok(Self { view, sampler })
    }

    pub fn from_raw_bytes(device: &wgpu::Device, queue: &wgpu::Queue, bytes: &[u8]) -> Result<Self> {
        let img  = image::load_from_memory(bytes)?;
        let rgba = img.to_rgba8();
        let (w, h) = img.dimensions();

        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Atlas"),
            size: wgpu::Extent3d { width: w, height: h, depth_or_array_layers: 1 },
            mip_level_count: 1, sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8UnormSrgb,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        queue.write_texture(
            wgpu::ImageCopyTexture {
                texture: &texture, mip_level: 0,
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

        let view    = texture.create_view(&wgpu::TextureViewDescriptor::default());
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            address_mode_u: wgpu::AddressMode::Repeat,
            address_mode_v: wgpu::AddressMode::Repeat,
            address_mode_w: wgpu::AddressMode::Repeat,
            mag_filter:     wgpu::FilterMode::Nearest,
            min_filter:     wgpu::FilterMode::Nearest,
            mipmap_filter:  wgpu::FilterMode::Nearest,
            ..Default::default()
        });
        Ok(Self { view, sampler })
    }

    // ─────────────────────────────────────────────────────────────────────────
    //  Procedural water textures
    // ─────────────────────────────────────────────────────────────────────────

    /// Overwrite atlas tile positions (10,0) and (11,0) with mathematically
    /// generated water_still and water_flow textures.
    ///
    /// Atlas layout: 32 columns × 20 rows of 16×16 px tiles on a 512×320 canvas.
    /// Tile (col, row) starts at pixel (col*16, row*16).
    fn inject_water_tiles(atlas: &mut RgbaImage) {
        for ty in 0u32..16 {
            for tx in 0u32..16 {
                atlas.put_pixel(160 + tx, ty, Self::water_still_pixel(tx, ty)); // col 10
                atlas.put_pixel(176 + tx, ty, Self::water_flow_pixel(tx, ty));  // col 11
            }
        }
    }

    /// Generate one pixel of the *still water* tile.
    ///
    /// Combines four sine/cosine wave functions at different frequencies and
    /// phases to produce a rippled surface pattern with deep Minecraft-blue tones.
    fn water_still_pixel(x: u32, y: u32) -> image::Rgba<u8> {
        use std::f32::consts::TAU;
        let fx = x as f32 * TAU / 16.0;
        let fy = y as f32 * TAU / 16.0;

        // Four overlapping waves at varied frequencies and phases
        let w1 = (fx * 2.5 + fy * 1.3).sin();
        let w2 = (fx * 1.7 - fy * 2.1 + 1.2).sin();
        let w3 = (fx * 0.7 + fy * 3.1 + 0.5).cos();
        let w4 = ((fx - std::f32::consts::PI).sin() * (fy - std::f32::consts::PI).sin()).abs();

        // Weighted sum → gamma-corrected [0,1]
        let c = ((w1 * 0.40 + w2 * 0.30 + w3 * 0.20 + w4 * 0.10) * 0.5 + 0.5)
            .powf(0.75)
            .clamp(0.0, 1.0);

        // Deep blue–teal palette
        image::Rgba([
            (14.0 + c * 38.0) as u8,   // R: 14 – 52
            (42.0 + c * 78.0) as u8,   // G: 42 – 120
            (135.0 + c * 78.0) as u8,  // B: 135 – 213
            190,                        // A: semi-transparent
        ])
    }

    /// Generate one pixel of the *flowing water* tile.
    ///
    /// Uses vertical stream-lines with superimposed foam and eddy functions to
    /// suggest downward motion. Slightly brighter and more turquoise than still.
    fn water_flow_pixel(x: u32, y: u32) -> image::Rgba<u8> {
        use std::f32::consts::TAU;
        let fx = x as f32 * TAU / 16.0;
        let fy = y as f32 * TAU / 16.0;

        let stream = (fx * 1.5 + fy * 3.0).sin();                   // downward flow lines
        let foam   = ((fx * 4.0).sin() * (fy * 2.5 + 0.7).cos()).abs(); // foam bubbles
        let eddy   = (fx * 2.0 - fy * 1.5 + 2.1).cos();             // lateral turbulence

        let c = ((stream * 0.45 + foam * 0.35 + eddy * 0.20) * 0.5 + 0.5)
            .powf(0.80)
            .clamp(0.0, 1.0);

        image::Rgba([
            (12.0 + c * 35.0) as u8,   // R: 12 – 47
            (38.0 + c * 65.0) as u8,   // G: 38 – 103
            (128.0 + c * 85.0) as u8,  // B: 128 – 213
            172,                        // A: slightly more transparent than still
        ])
    }

    fn compose_from_blocks() -> Option<RgbaImage> {
        let possible_roots = vec![
            Path::new("Assets/Blocks").to_path_buf(),
            Path::new("../Assets/Blocks").to_path_buf(),
            Path::new("../../Assets/Blocks").to_path_buf(),
            Path::new("../../../Assets/Blocks").to_path_buf(),
            Path::new("../../../../Assets/Blocks").to_path_buf(),
        ];

        let in_root = possible_roots.into_iter().find(|path| path.exists())?;
        // Create an atlas large enough for common blocks (32x32 grid = 512x512 @ 16x16 per tile)
        let mut atlas = RgbaImage::new(512, 320);

        // Map block names to atlas positions (col, row)
        let src_names = vec![
            ((0,0), "grass_block_top"),
            ((1,0), "grass_block_side"),
            ((3,0), "dirt"),
            ((4,0), "stone"),
            ((5,0), "sand"),
            ((6,0), "gravel"),
            ((7,0), "snow"),
            ((8,0), "cobblestone"),
            ((9,0), "bedrock"),
            ((10,0), "water_still"),
            ((11,0), "water_flow"),
            ((12,0), "oak_log"),
            ((13,0), "oak_log_top"),
            ((14,0), "oak_leaves"),
            ((15,0), "lava_still"),
            ((16,0), "lava_flow"),
            
            ((8,1), "stone_bricks"),
            ((11,1), "andesite"),
            ((14,1), "netherrack"),
            ((15,1), "glowstone"),
            ((0,1), "coal_ore"),
            ((1,1), "iron_ore"),
            ((2,1), "gold_ore"),
            ((3,1), "diamond_ore"),
            ((4,1), "emerald_ore"),
            ((5,1), "redstone_ore"),
            ((6,1), "copper_ore"),
            ((7,1), "lapis_ore"),
            
            ((0,2), "deepslate"),
            ((1,2), "deepslate_coal_ore"),
            ((2,2), "deepslate_iron_ore"),
            ((3,2), "deepslate_copper_ore"),
            ((4,2), "deepslate_gold_ore"),
            ((5,2), "deepslate_diamond_ore"),
            ((6,2), "deepslate_emerald_ore"),
            ((7,2), "deepslate_lapis_ore"),
            ((8,2), "deepslate_redstone_ore"),
            ((9,2), "deepslate_bricks"),
            ((10,2), "tuff"),
            ((11,2), "tuff_bricks"),
            ((12,2), "obsidian"),
            ((13,2), "crying_obsidian"),
            ((14,2), "end_stone"),
            ((15,2), "end_stone_bricks"),
            
            ((0,3), "oak_planks"),
            ((1,3), "spruce_log"),
            ((2,3), "birch_log"),
            ((3,3), "jungle_log"),
            ((4,3), "acacia_log"),
            ((5,3), "dark_oak_log"),
            ((6,3), "mangrove_log"),
            ((7,3), "pale_oak_log"),
            ((8,3), "spruce_leaves"),
            ((9,3), "birch_leaves"),
            ((10,3), "jungle_leaves"),
            ((11,3), "acacia_leaves"),
            ((12,3), "dark_oak_leaves"),
            ((13,3), "mangrove_leaves"),
            ((14,3), "pale_oak_leaves"),
            ((15,3), "azalea_leaves"),
            
            ((0,4), "clay"),
            ((1,4), "mycelium_top"),
            ((2,4), "podzol_top"),
            ((3,4), "rooted_dirt"),
            ((4,4), "moss_block"),
            ((5,4), "mud"),
            ((6,4), "packed_mud"),
            ((7,4), "muddy_mangrove_roots_top"),
            ((8,4), "coarse_dirt"),
            ((9,4), "farmland"),
            ((10,4), "farmland_moist"),
            ((11,4), "soul_sand"),
            ((12,4), "soul_soil"),
            ((13,4), "nether_wart_block"),
            ((14,4), "warped_wart_block"),
            ((15,4), "shroomlight"),
        ];

        let mut placed = 0;
        for ((col, row), name) in &src_names {
            // Try multiple variations of the filename
            let paths = vec![
                in_root.join(format!("{}.png", name)),
                in_root.join(format!("{}_1.png", name)),
                in_root.join(format!("{}_2.png", name)),
                in_root.join(format!("{}_top.png", name)),
            ];
            
            let mut loaded = false;
            for path in paths {
                if let Ok(img) = image::open(&path) {
                    let tile = img.resize_exact(16, 16, image::imageops::FilterType::Nearest).to_rgba8();
                    for y in 0..16 {
                        for x in 0..16 {
                            let px = tile.get_pixel(x, y);
                            atlas.put_pixel((col * 16 + x) as u32, (row * 16 + y) as u32, *px);
                        }
                    }
                    loaded = true;
                    placed += 1;
                    break;
                }
            }
            
            if !loaded {
                eprintln!("⚠️ Could not load texture for block: {}", name);
                // Fill with placeholder color (checkerboard pattern)
                for y in 0..16 {
                    for x in 0..16 {
                        let pattern = ((x / 2 + y / 2) % 2) * 128;
                        atlas.put_pixel(
                            (col * 16 + x) as u32, 
                            (row * 16 + y) as u32, 
                            image::Rgba([128 + pattern as u8, 64, 128 + pattern as u8, 255])
                        );
                    }
                }
            }
        }
        
        eprintln!("✓ Successfully loaded {}/{} block textures to atlas", placed, src_names.len());
        Some(atlas)
    }
}

const ATLAS_W: f32 = 512.0;  // Updated: 32 columns x 16px = 512px
const ATLAS_H: f32 = 320.0;  // Updated: 20 rows x 16px = 320px (we use ~5 rows)
const TILE:    f32 = 16.0;

#[derive(Clone, Copy, Debug)]
pub struct TileUV {
    pub u0: f32, pub v0: f32,
    pub u1: f32, pub v1: f32,
    pub is_top: bool,
}

impl TileUV {
    pub fn new(col: u32, row: u32) -> Self {
        let eps = 0.5 / ATLAS_W;
        let u0  = col as f32 * TILE / ATLAS_W + eps;
        let v0  = row as f32 * TILE / ATLAS_H + eps;
        let u1  = u0 + TILE / ATLAS_W - eps * 2.0;
        let v1  = v0 + TILE / ATLAS_H - eps * 2.0;
        Self { u0, v0, u1, v1, is_top: false }
    }

    pub fn new_top(col: u32, row: u32) -> Self {
        let mut tile = Self::new(col, row);
        tile.is_top = true;
        tile
    }

    pub fn uvs(&self) -> [[f32; 2]; 4] {
        [[self.u0, self.v0], [self.u1, self.v0], [self.u1, self.v1], [self.u0, self.v1]]
    }
}

// Tile accessors
pub fn tile_grass_top()           -> TileUV { TileUV::new_top( 0, 0) }
pub fn tile_grass_side()          -> TileUV { TileUV::new( 1, 0) }
pub fn tile_dirt()                -> TileUV { TileUV::new( 3, 0) }
pub fn tile_stone()               -> TileUV { TileUV::new( 4, 0) }
pub fn tile_sand()                -> TileUV { TileUV::new( 5, 0) }
pub fn tile_gravel()              -> TileUV { TileUV::new( 6, 0) }
pub fn tile_snow()                -> TileUV { TileUV::new( 7, 0) }
pub fn tile_cobblestone()         -> TileUV { TileUV::new( 8, 0) }
pub fn tile_bedrock()             -> TileUV { TileUV::new( 9, 0) }
pub fn tile_water()               -> TileUV { TileUV::new(10, 0) }
pub fn tile_water_flow()          -> TileUV { TileUV::new(11, 0) }
pub fn tile_oak_log_side()        -> TileUV { TileUV::new(12, 0) }
pub fn tile_oak_log_top()         -> TileUV { TileUV::new_top(13, 0) }
pub fn tile_oak_leaves()          -> TileUV { TileUV::new(14, 0) }
pub fn tile_lava_still()          -> TileUV { TileUV::new(15, 0) }
pub fn tile_lava_flow()           -> TileUV { TileUV::new(16, 0) }

pub fn tile_stone_bricks()        -> TileUV { TileUV::new( 8, 1) }
pub fn tile_andesite()            -> TileUV { TileUV::new(11, 1) }
pub fn tile_netherrack()          -> TileUV { TileUV::new(14, 1) }
pub fn tile_glowstone()           -> TileUV { TileUV::new(15, 1) }

pub fn tile_coal_ore()            -> TileUV { TileUV::new( 0, 1) }
pub fn tile_iron_ore()            -> TileUV { TileUV::new( 1, 1) }
pub fn tile_gold_ore()            -> TileUV { TileUV::new( 2, 1) }
pub fn tile_diamond_ore()         -> TileUV { TileUV::new( 3, 1) }
pub fn tile_emerald_ore()         -> TileUV { TileUV::new( 4, 1) }
pub fn tile_redstone_ore()        -> TileUV { TileUV::new( 5, 1) }
pub fn tile_copper_ore()          -> TileUV { TileUV::new( 6, 1) }
pub fn tile_lapis_ore()           -> TileUV { TileUV::new( 7, 1) }

pub fn tile_deepslate()           -> TileUV { TileUV::new( 0, 2) }
pub fn tile_deepslate_coal_ore()  -> TileUV { TileUV::new( 1, 2) }
pub fn tile_deepslate_iron_ore()  -> TileUV { TileUV::new( 2, 2) }
pub fn tile_deepslate_copper_ore()-> TileUV { TileUV::new( 3, 2) }
pub fn tile_deepslate_gold_ore()  -> TileUV { TileUV::new( 4, 2) }
pub fn tile_deepslate_diamond()   -> TileUV { TileUV::new( 5, 2) }
pub fn tile_deepslate_emerald()   -> TileUV { TileUV::new( 6, 2) }
pub fn tile_deepslate_lapis()     -> TileUV { TileUV::new( 7, 2) }
pub fn tile_deepslate_redstone()  -> TileUV { TileUV::new( 8, 2) }
pub fn tile_deepslate_bricks()    -> TileUV { TileUV::new( 9, 2) }
pub fn tile_tuff()                -> TileUV { TileUV::new(10, 2) }
pub fn tile_tuff_bricks()         -> TileUV { TileUV::new(11, 2) }
pub fn tile_obsidian()            -> TileUV { TileUV::new(12, 2) }
pub fn tile_crying_obsidian()     -> TileUV { TileUV::new(13, 2) }
pub fn tile_end_stone()           -> TileUV { TileUV::new(14, 2) }
pub fn tile_end_stone_bricks()    -> TileUV { TileUV::new(15, 2) }

pub fn tile_oak_planks()          -> TileUV { TileUV::new( 0, 3) }
pub fn tile_spruce_log_side()     -> TileUV { TileUV::new( 1, 3) }
pub fn tile_birch_log_side()      -> TileUV { TileUV::new( 2, 3) }
pub fn tile_jungle_log_side()     -> TileUV { TileUV::new( 3, 3) }
pub fn tile_acacia_log_side()     -> TileUV { TileUV::new( 4, 3) }
pub fn tile_dark_oak_log_side()   -> TileUV { TileUV::new( 5, 3) }
pub fn tile_mangrove_log_side()   -> TileUV { TileUV::new( 6, 3) }
pub fn tile_pale_oak_log_side()   -> TileUV { TileUV::new( 7, 3) }
pub fn tile_spruce_leaves()       -> TileUV { TileUV::new( 8, 3) }
pub fn tile_birch_leaves()        -> TileUV { TileUV::new( 9, 3) }
pub fn tile_jungle_leaves()       -> TileUV { TileUV::new(10, 3) }
pub fn tile_acacia_leaves()       -> TileUV { TileUV::new(11, 3) }
pub fn tile_dark_oak_leaves()     -> TileUV { TileUV::new(12, 3) }
pub fn tile_mangrove_leaves()     -> TileUV { TileUV::new(13, 3) }
pub fn tile_pale_oak_leaves()     -> TileUV { TileUV::new(14, 3) }
pub fn tile_azalea_leaves()       -> TileUV { TileUV::new(15, 3) }

pub fn tile_clay()                -> TileUV { TileUV::new( 0, 4) }
pub fn tile_mycelium()            -> TileUV { TileUV::new_top( 1, 4) }
pub fn tile_podzol()              -> TileUV { TileUV::new_top( 2, 4) }
pub fn tile_rooted_dirt()         -> TileUV { TileUV::new( 3, 4) }
pub fn tile_moss_block()          -> TileUV { TileUV::new( 4, 4) }
pub fn tile_mud()                 -> TileUV { TileUV::new( 5, 4) }
pub fn tile_packed_mud()          -> TileUV { TileUV::new( 6, 4) }
pub fn tile_muddy_mangrove_roots()->TileUV { TileUV::new_top( 7, 4) }
pub fn tile_coarse_dirt()         -> TileUV { TileUV::new( 8, 4) }
pub fn tile_farmland()            -> TileUV { TileUV::new( 9, 4) }
pub fn tile_farmland_moist()      -> TileUV { TileUV::new(10, 4) }
pub fn tile_soul_sand()           -> TileUV { TileUV::new(11, 4) }
pub fn tile_soul_soil()           -> TileUV { TileUV::new(12, 4) }
pub fn tile_nether_wart_block()   -> TileUV { TileUV::new(13, 4) }
pub fn tile_warped_wart_block()   -> TileUV { TileUV::new(14, 4) }
pub fn tile_shroomlight()         -> TileUV { TileUV::new(15, 4) }