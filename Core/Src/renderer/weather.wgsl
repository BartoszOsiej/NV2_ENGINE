// Weather particles for NV-2.0: instanced rain streaks and snow flakes.
// Driven by the real climate weather (kind = 1 rain, 2 snow).

struct CameraUniform {
    view_proj: mat4x4<f32>,
};
@group(0) @binding(0)
var<uniform> camera: CameraUniform;

struct WeatherUniform {
    camera: vec4<f32>, // xyz eye position
    right: vec4<f32>,
    up: vec4<f32>,
    params: vec4<f32>, // x = time, y = intensity, z = kind (0 none, 1 rain, 2 snow), w = wind
};
@group(1) @binding(0)
var<uniform> weather: WeatherUniform;

struct Particle {
    pos: vec3<f32>,
    _pad0: f32,
    vel: vec3<f32>,
    _pad1: f32,
    size: vec2<f32>,
    phase: f32,
    _pad: f32,
};
@group(1) @binding(1)
var<storage, read> particles: array<Particle>;

struct VsOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) kind: f32,
};

@vertex
fn vs_main(@builtin(vertex_index) vi: u32, @builtin(instance_index) ii: u32) -> VsOut {
    let p = particles[ii];
    // Quad corner in -1..1 (two triangles via strip: verts 0..3).
    let uv = vec2<f32>(f32(vi & 1), f32((vi >> 1) & 1));
    let corner = uv * 2.0 - 1.0;
    // Rain streaks stretch along the world up axis; snow flakes stay square.
    let world = p.pos
        + weather.right.xyz * corner.x * p.size.x * 0.5
        + weather.up.xyz * corner.y * p.size.y * 0.5;
    var out: VsOut;
    out.pos = camera.view_proj * vec4<f32>(world, 1.0);
    out.uv = corner * 0.5 + 0.5;
    out.kind = weather.params.z;
    return out;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    if (in.kind < 1.5) {
        // Rain: pale blue streak, bright enough to read against the sky.
        let edge = smoothstep(0.10, 0.0, abs(in.uv.x - 0.5)) * 0.35;
        return vec4<f32>(0.82, 0.90, 1.0, 0.58 + edge);
    }
    // Snow: soft white flake with a feathered edge.
    let d = length(in.uv - vec2<f32>(0.5)) * 2.0;
    let a = 1.0 - smoothstep(0.68, 1.0, d);
    return vec4<f32>(1.0, 1.0, 1.0, a * 0.92);
}
