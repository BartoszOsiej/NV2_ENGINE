struct CameraUniform {
    view_proj: mat4x4<f32>,
};
@group(1) @binding(0)
var<uniform> camera: CameraUniform;

struct VertexInput {
    @location(0) position:   vec3<f32>,
    @location(1) tex_coords: vec2<f32>,
    @location(2) normal:     vec3<f32>,
    @location(3) brightness: f32,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) tex_coords: vec2<f32>,
    @location(1) light:      f32,
};

@vertex
fn vs_main(v: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    out.clip_position = camera.view_proj * vec4<f32>(v.position, 1.0);
    out.tex_coords    = v.tex_coords;

    // Directional sun light (from upper-right)
    let sun_dir   = normalize(vec3<f32>(0.6, 1.0, 0.4));
    let diffuse   = max(dot(v.normal, sun_dir), 0.0);
    let ambient   = 0.35;
    out.light     = (ambient + diffuse * 0.65) * v.brightness;
    return out;
}

@group(0) @binding(0) var t_atlas:  texture_2d<f32>;
@group(0) @binding(1) var s_atlas:  sampler;

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let color = textureSample(t_atlas, s_atlas, in.tex_coords);
    if color.a < 0.1 { discard; }

    // Apply lighting and a subtle fog toward horizon
    let lit = vec4<f32>(color.rgb * in.light, color.a);
    return lit;
}