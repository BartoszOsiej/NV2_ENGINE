// Voxel wildlife for NV-2.0: instanced textured cubes (body, head, legs)
// that wander the world. Each animal is a few instances of this unit cube,
// shaded with a procedural fur tile from the shared atlas.

struct CameraUniform {
    view_proj: mat4x4<f32>,
};
@group(0) @binding(0)
var<uniform> camera: CameraUniform;

struct Animal {
    pos: vec3<f32>,
    _pad0: f32,
    size: vec3<f32>,
    _pad1: f32,
    color: vec3<f32>,
    _pad2: f32,
    rot: f32,
    _pad3: f32,
    uv: vec2<f32>,
};
@group(1) @binding(0)
var<storage, read> animals: array<Animal>;

@group(2) @binding(0)
var t_atlas: texture_2d<f32>;
@group(2) @binding(1)
var s_atlas: sampler;

// Atlas layout: 512×320 px, 16 px tiles (32 cols × 20 rows).
const TILE_U: f32 = 16.0 / 512.0;
const TILE_V: f32 = 16.0 / 320.0;

struct VsIn {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
    @builtin(instance_index) ii: u32,
};

struct VsOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) normal: vec3<f32>,
    @location(1) color: vec3<f32>,
    @location(2) uv: vec2<f32>,
};

@vertex
fn vs_main(in: VsIn) -> VsOut {
    let a = animals[in.ii];
    // Rotate the unit cube around Y by the animal's heading.
    let c = cos(a.rot);
    let s = sin(a.rot);
    let lx = in.position.x * c - in.position.z * s;
    let lz = in.position.x * s + in.position.z * c;
    let world = a.pos + vec3<f32>(lx * a.size.x, in.position.y * a.size.y, lz * a.size.z);
    var out: VsOut;
    out.pos = camera.view_proj * vec4<f32>(world, 1.0);
    out.normal = in.normal;
    out.color = a.color;
    // Map the cube-face UV onto the instance's fur tile in the atlas.
    out.uv = a.uv + vec2<f32>(in.uv.x * TILE_U, in.uv.y * TILE_V);
    return out;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let fur = textureSample(t_atlas, s_atlas, in.uv).rgb;
    // Voxel shading: top faces bright, sides darker, belly slightly darker.
    let shade = 0.60 + 0.40 * abs(in.normal.y);
    return vec4<f32>(fur * in.color * shade, 1.0);
}
