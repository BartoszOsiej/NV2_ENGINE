struct CameraUniform {
    view_proj: mat4x4<f32>,
};
@group(0) @binding(0)
var<uniform> camera: CameraUniform;

struct MaterialUniform {
    color_tint: vec4<f32>,
};
@group(1) @binding(0)
var<uniform> material: MaterialUniform;

struct BiomeUniform {
    ambient_and_mul: vec4<f32>, // xyz = ambient tint, w = ambient multiplier
    time_info:       vec4<f32>, // x = water_time, y = day_brightness (0.15–1.0), z/w reserved
};
@group(3) @binding(0)
var<uniform> biome: BiomeUniform;

struct VertexInput {
    @location(0) position:   vec3<f32>,
    @location(1) tex_coords: vec2<f32>,
    @location(2) normal:     vec3<f32>,
    @location(3) brightness: f32,
    @location(4) is_top:     f32,
    @location(5) biome_tint: vec3<f32>,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) tex_coords:  vec2<f32>,
    @location(1) light:       f32,
    @location(2) is_top:      f32,
    @location(3) biome_tint:  vec3<f32>,
};

@vertex
fn vs_main(v: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    out.clip_position = camera.view_proj * vec4<f32>(v.position, 1.0);
    out.tex_coords    = v.tex_coords;
    out.is_top        = v.is_top;
    out.biome_tint    = v.biome_tint;

    // Directional sun light (from upper-right); scaled by day brightness
    let sun_dir    = normalize(vec3<f32>(0.6, 1.0, 0.4));
    let diffuse    = max(dot(v.normal, sun_dir), 0.0);
    let day        = biome.time_info.y;           // 0.15 night … 1.0 noon
    let amb_str    = biome.ambient_and_mul.w * 0.35;
    // Moonlight floor keeps the world slightly visible at night
    let ambient    = mix(max(amb_str * 0.25, 0.05), amb_str, day);
    out.light      = (ambient + diffuse * 0.65 * day) * v.brightness;
    return out;
}

@group(2) @binding(0) var t_atlas: texture_2d<f32>;
@group(2) @binding(1) var s_atlas: sampler;

@fragment
fn fs_main(fs_input: VertexOutput) -> @location(0) vec4<f32> {
    let atlas_tile = vec2<f32>(16.0 / 512.0, 16.0 / 320.0);
    var tex_coords = fs_input.tex_coords;

    // Recover which atlas tile and in-tile UV from the interpolated coordinate.
    // NOTE: do NOT flip tex_coords.y before this — that would map tile row 0
    // to row 19 (empty) and break all textures. We flip *within* the local tile
    // below so that face-quad v0=bottom still shows the top of the image.
    let tile_index = floor(tex_coords / atlas_tile);
    var local_uv = fract(tex_coords / atlas_tile);

    // Face quads assign v0 (top of atlas tile) to the block BOTTOM vertex and v1
    // to the TOP vertex, making textures appear upside-down. Flip V within the
    // tile here to correct that without disturbing tile_index.
    local_uv.y = 1.0 - local_uv.y;

    if (fs_input.is_top > 0.5) {
        local_uv = vec2<f32>(local_uv.y, 1.0 - local_uv.x);
    }

    tex_coords = tile_index * atlas_tile + local_uv * atlas_tile;

    let color = textureSample(t_atlas, s_atlas, tex_coords);
    // If texture is effectively transparent, skip
    if (color.a < 0.01) { discard; }

    var sampled = color.rgb;

    // Detect water tiles by their atlas indices (col 10/11, row 0)
    let is_water = (tile_index.x == 10.0 && tile_index.y == 0.0) || (tile_index.x == 11.0 && tile_index.y == 0.0);
    if (is_water) {
        // Animate the water tile UV with sinusoidal ripple waves
        let wt = biome.time_info.x;
        let wu = fract(local_uv.x + sin(local_uv.y * 12.0 + wt * 2.1) * 0.07 + cos(wt * 0.9) * 0.03);
        let wv = fract(local_uv.y + sin(local_uv.x * 10.0 + wt * 1.7) * 0.07 + sin(wt * 1.1) * 0.03);
        let wavy_tc  = tile_index * atlas_tile + vec2<f32>(wu, wv) * atlas_tile;
        let wcolor   = textureSample(t_atlas, s_atlas, wavy_tc);
        // Tint water with per-vertex biome color then light
        let water_tint = fs_input.biome_tint * vec3<f32>(0.50, 0.78, 1.12);
        let ws = mix(wcolor.rgb, water_tint, 0.28);
        // Gently pulse alpha to suggest surface movement
        let alpha = 0.58 + sin(wt * 1.8 + local_uv.x * 5.0 + local_uv.y * 4.0) * 0.07;
        return vec4<f32>(ws * fs_input.light, alpha);
    }

    // Grass-top tile (atlas col 0, row 0): tint with biome grass color.
    // Only this specific tile gets the grass push — oak log tops and other
    // is_top tiles keep their original texture color.
    let is_grass_top = (tile_index.x == 0.0 && tile_index.y == 0.0);
    if (is_grass_top) {
        // Combine per-vertex biome tint with a green-push to produce biome grass color.
        // Plains ~(0.55,0.73,0.31), Forest darker, Desert yellower, Snowy grayish.
        let grass_color = fs_input.biome_tint * vec3<f32>(0.52, 0.90, 0.38);
        sampled *= grass_color;
    } else {
        // Oak leaves (atlas col 14, row 0): foliage tint from biome
        let is_leaves = (tile_index.x == 14.0 && tile_index.y == 0.0);
        if (is_leaves) {
            sampled *= fs_input.biome_tint * vec3<f32>(0.56, 0.88, 0.42);
        } else {
            // All other blocks: subtle per-vertex biome ambient tint (environmental lighting)
            sampled *= fs_input.biome_tint;
        }
    }

    let lit = vec4<f32>(sampled * fs_input.light, color.a);
    return lit;
}