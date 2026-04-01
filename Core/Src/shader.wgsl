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
struct VertexInput {
    @location(0) position:   vec3<f32>,
    @location(1) tex_coords: vec2<f32>,
    @location(2) normal:     vec3<f32>,
    @location(3) brightness: f32,
    @location(4) is_top:     f32,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) tex_coords: vec2<f32>,
    @location(1) light:      f32,
    @location(2) is_top:     f32,
};

@vertex
fn vs_main(v: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    out.clip_position = camera.view_proj * vec4<f32>(v.position, 1.0);
    out.tex_coords    = v.tex_coords;
    out.is_top        = v.is_top;

    // Directional sun light (from upper-right)
    let sun_dir   = normalize(vec3<f32>(0.6, 1.0, 0.4));
    let diffuse   = max(dot(v.normal, sun_dir), 0.0);
    let ambient   = 0.35;
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
    if (color.a < 0.1) { discard; }

    var sampled = color.rgb;
    if (fs_input.is_top > 0.5) {
        sampled *= vec3<f32>(0.2, 0.7, 0.3);
    }

    let lit = vec4<f32>(sampled * fs_input.light, color.a);
    return lit;
}