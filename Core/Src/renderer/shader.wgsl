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

    // Directional sun light (from upper-right)
    let sun_dir   = normalize(vec3<f32>(0.6, 1.0, 0.4));
    let diffuse   = max(dot(v.normal, sun_dir), 0.0);
    let ambient   = biome.ambient_and_mul.w * 0.35;
    out.light     = (ambient + diffuse * 0.65) * v.brightness;
    return out;
}

@group(2) @binding(0) var t_atlas: texture_2d<f32>;
@group(2) @binding(1) var s_atlas: sampler;

@fragment
fn fs_main(fs_input: VertexOutput) -> @location(0) vec4<f32> {
    let atlas_tile = vec2<f32>(16.0 / 512.0, 16.0 / 320.0);
    var tex_coords = fs_input.tex_coords;

    // Vulkan coordinate correction
    tex_coords.y = 1.0 - tex_coords.y;

    let tile_index = floor(tex_coords / atlas_tile);
    var local_uv = fract(tex_coords / atlas_tile);

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
        // Tint water with per-vertex biome color (river = cooler, swamp = murky)
        let water_tint = fs_input.biome_tint * vec3<f32>(0.50, 0.75, 1.10);
        sampled = mix(sampled, water_tint, 0.30);
        let lit = vec4<f32>(sampled * fs_input.light, 0.60);
        return lit;
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