// Fullscreen sky pass for NV-2.0.
// Draws the real sky: climate-tinted gradient, sun, moon, stars and
// procedurally animated clouds — so a desert world has a sun-baked sky,
// a rainforest humid overcast and a taiga a cold steel-blue vault.

struct SkyUniform {
    right: vec4<f32>,
    up: vec4<f32>,
    forward: vec4<f32>,
    view: vec4<f32>,    // x = tan(fov/2), y = aspect, z = time, w = day
    climate: vec4<f32>, // x = sun_phase, y = cloud_cover, z = weather kind, w = intensity
    atmosphere: vec4<f32>,
};
@group(0) @binding(0)
var<uniform> sky: SkyUniform;

struct VsOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) dir: vec3<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) vi: u32) -> VsOut {
    let uv = vec2<f32>(f32((vi << 1) & 2), f32(vi & 2));
    var out: VsOut;
    out.pos = vec4<f32>(uv * 2.0 - 1.0, 1.0, 1.0);
    // Reconstruct the view ray for this pixel.
    let ndc = uv * 2.0 - 1.0;
    let dir = normalize(
        sky.forward.xyz
        + sky.right.xyz * ndc.x * sky.view.x * sky.view.y
        + sky.up.xyz * ndc.y * sky.view.x
    );
    out.dir = dir;
    return out;
}

fn hash(p: vec2<f32>) -> f32 {
    return fract(sin(dot(p, vec2<f32>(127.1, 311.7))) * 43758.5453);
}

fn vnoise(p: vec2<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let u = f * f * (3.0 - 2.0 * f);
    let a = hash(i);
    let b = hash(i + vec2<f32>(1.0, 0.0));
    let c = hash(i + vec2<f32>(0.0, 1.0));
    let d = hash(i + vec2<f32>(1.0, 1.0));
    return mix(mix(a, b, u.x), mix(c, d, u.x), u.y);
}

fn fbm(p: vec2<f32>) -> f32 {
    var v = 0.0;
    var amp = 0.5;
    var q = p;
    for (var k = 0; k < 5; k = k + 1) {
        v = v + amp * vnoise(q);
        q = q * 2.03 + vec2<f32>(17.7, 9.2);
        amp = amp * 0.5;
    }
    return v;
}

@fragment
fn fs_main(in: VsOut) -> @location(0) vec4<f32> {
    let dir = normalize(in.dir);
    let day = sky.view.w;
    let time = sky.view.z;
    let sun_phase = sky.climate.x;
    let cloud_cover = clamp(sky.climate.y, 0.0, 1.0);

    let angle = (sun_phase - 0.25) * 6.283185307;
    let sun_elev = sin(angle);
    let sun_dir = normalize(vec3<f32>(-cos(angle), sin(angle), 0.18));

    // Base gradient: night → climate-tinted zenith → hazy horizon.
    let night_sky = vec3<f32>(0.008, 0.012, 0.050);
    let day_zenith = sky.atmosphere.xyz * 0.92 + vec3<f32>(0.05, 0.08, 0.12);
    let haze = mix(vec3<f32>(0.62, 0.70, 0.82), sky.atmosphere.xyz, 0.55);
    let horizon_t = pow(1.0 - max(dir.y, 0.0), 2.2);
    let day_grad = mix(day_zenith, haze, horizon_t);
    var col = mix(night_sky, day_grad, clamp(day * 1.1, 0.0, 1.0));

    // Darken below the horizon.
    col = mix(col, vec3<f32>(0.05, 0.06, 0.08), clamp(-dir.y * 4.0, 0.0, 1.0));

    // Sun disc + glow (only by day).
    let sun_dot = dot(dir, sun_dir);
    let sun_disc = smoothstep(0.99935, 0.9997, sun_dot);
    let sun_glow = exp(-(1.0 - sun_dot) * 260.0) * 0.55;
    col = col + vec3<f32>(1.0, 0.95, 0.82) * (sun_disc * day + sun_glow * day);

    // Moon at night.
    let moon_dot = dot(dir, -sun_dir);
    let moon_disc = smoothstep(0.9995, 0.9998, moon_dot);
    col = col + vec3<f32>(0.70, 0.75, 0.90) * moon_disc * (1.0 - day) * 0.8;

    // Stars at night.
    let star_noise = vnoise(dir.xz * 240.0 + dir.y * 13.0);
    let stars = step(0.997, star_noise) * (1.0 - day) * (1.0 - horizon_t * 0.9);
    col = col + vec3<f32>(0.9) * stars * 0.7;

    // Lightning during storms (rain with real intensity): a rolling noise
    // crosses a high threshold occasionally and flashes the whole sky.
    let storm = step(1.5, sky.climate.z) * step(0.45, sky.climate.w);
    let lf = vnoise(vec2<f32>(time * 0.83, 7.13));
    let flash = smoothstep(0.9965, 1.0, lf) * storm;
    col = col + vec3<f32>(0.85, 0.88, 1.0) * flash * 0.85;

    // Procedural clouds: planar projection, drifting with time, coverage
    // driven by the real climate cloud cover.
    let cloud_p = dir.xz / max(dir.y + 0.06, 0.06);
    let cloud_n = fbm(cloud_p * 0.9 + vec2<f32>(time * 0.012, time * 0.006));
    let cloud_t = smoothstep(0.42, 0.62, cloud_n);
    let cloud_amt = cloud_t * cloud_cover;
    let cloud_lit = clamp(0.55 + sun_dot * 0.45, 0.0, 1.0);
    let cloud_col = vec3<f32>(0.92, 0.94, 0.98) * (0.25 + 0.75 * cloud_lit * day);
    col = mix(col, cloud_col, cloud_amt * 0.9);

    return vec4<f32>(col, 1.0);
}
