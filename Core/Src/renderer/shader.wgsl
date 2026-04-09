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
    ambient: vec4<f32>,
    fog_color: vec4<f32>,
    grade: vec4<f32>,
    view_info: vec4<f32>,
    camera_pos: vec4<f32>,
};
@group(3) @binding(0)
var<uniform> biome: BiomeUniform;

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) tex_coords: vec2<f32>,
    @location(2) normal: vec3<f32>,
    @location(3) brightness: f32,
    @location(4) is_top: f32,
    @location(5) biome_tint: vec3<f32>,
    @location(6) surface_data: vec4<f32>,
    @location(7) foliage_tint: f32,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) tex_coords: vec2<f32>,
    @location(1) light: f32,
    @location(2) is_top: f32,
    @location(3) biome_tint: vec3<f32>,
    @location(4) world_pos: vec3<f32>,
    @location(5) world_normal: vec3<f32>,
    @location(6) surface_data: vec4<f32>,
    @location(7) foliage_tint: f32,
};

@group(2) @binding(0)
var t_atlas: texture_2d<f32>;
@group(2) @binding(1)
var s_atlas: sampler;

fn sun_direction(sun_phase: f32) -> vec3<f32> {
    let angle = (sun_phase - 0.25) * 6.283185307;
    return normalize(vec3<f32>(-cos(angle), sin(angle), 0.25));
}

fn sky_color(day: f32, sun_phase: f32) -> vec3<f32> {
    let sun_elev = sin((sun_phase - 0.25) * 6.283185307);
    let night_sky = vec3<f32>(0.006, 0.010, 0.048);
    let day_zenith = vec3<f32>(0.270, 0.520, 0.860);
    let haze_blue = vec3<f32>(0.560, 0.720, 0.920);
    let sunset_low = vec3<f32>(1.000, 0.340, 0.018);
    let twilight = vec3<f32>(0.560, 0.220, 0.460);

    let day_sky = mix(haze_blue, day_zenith, clamp(day * 0.90, 0.0, 1.0));
    let sunset_strength = clamp(1.0 - abs(sun_elev) * 3.5, 0.0, 1.0);
    let sunset_col = mix(sunset_low, twilight, clamp(0.4 - sun_elev * 2.0, 0.0, 1.0));
    let base = mix(night_sky, day_sky, clamp(day * 1.20, 0.0, 1.0));
    return mix(base, sunset_col, sunset_strength * clamp(day * 4.0, 0.0, 0.78));
}

fn canonical_tile_uv(local_uv: vec2<f32>, is_top: f32) -> vec2<f32> {
    var uv = vec2<f32>(local_uv.x, 1.0 - local_uv.y);
    if (is_top > 0.5) {
        uv = vec2<f32>(uv.y, 1.0 - uv.x);
    }

    return uv;
}

fn varied_tile_uv(local_uv: vec2<f32>, variation: f32) -> vec2<f32> {
    var uv = local_uv;
    let variant = u32(floor(clamp(variation, 0.0, 0.99999) * 8.0));
    switch variant {
        case 1u: {
            uv = vec2<f32>(1.0 - uv.x, uv.y);
        }
        case 2u: {
            uv = vec2<f32>(uv.x, 1.0 - uv.y);
        }
        case 3u: {
            uv = vec2<f32>(1.0 - uv.x, 1.0 - uv.y);
        }
        case 4u: {
            uv = vec2<f32>(uv.y, uv.x);
        }
        case 5u: {
            uv = vec2<f32>(1.0 - uv.y, uv.x);
        }
        case 6u: {
            uv = vec2<f32>(uv.y, 1.0 - uv.x);
        }
        case 7u: {
            uv = vec2<f32>(1.0 - uv.y, 1.0 - uv.x);
        }
        default: {}
    }

    return uv;
}

fn grass_top_tile(tile_x: i32, tile_y: i32) -> bool {
    return tile_x == 0 && tile_y == 0;
}

fn leaf_tile(tile_x: i32, tile_y: i32) -> bool {
    return (tile_y == 0 && tile_x == 14) || (tile_y == 3 && tile_x >= 8 && tile_x <= 15);
}

fn emissive_tile(tile_x: i32, tile_y: i32) -> bool {
    let lava = tile_y == 0 && (tile_x == 15 || tile_x == 16);
    let glow = tile_x == 15 && tile_y == 1;
    let shroom = tile_x == 15 && tile_y == 4;
    return lava || glow || shroom;
}

fn water_tile(tile_x: i32, tile_y: i32) -> bool {
    return tile_y == 0 && (tile_x == 10 || tile_x == 11);
}

fn flower_tile(tile_x: i32, tile_y: i32) -> bool {
    return tile_y == 5 && (tile_x == 0 || tile_x == 2 || tile_x == 3);
}

fn log_top_tile(tile_x: i32, tile_y: i32) -> bool {
    return (tile_y == 0 && tile_x == 13) || (tile_y == 5 && tile_x == 5);
}

fn rotation_blacklist(tile_x: i32, tile_y: i32) -> bool {
    return grass_top_tile(tile_x, tile_y)
        || leaf_tile(tile_x, tile_y)
        || flower_tile(tile_x, tile_y)
        || log_top_tile(tile_x, tile_y)
        || water_tile(tile_x, tile_y);
}

fn rotation_whitelist(tile_x: i32, tile_y: i32) -> bool {
    let bark_side = (tile_y == 0 && tile_x == 12) || (tile_y == 3 && tile_x >= 1 && tile_x <= 7);
    let natural_ground =
        (tile_y == 0 && (tile_x == 3 || tile_x == 4 || tile_x == 5 || tile_x == 6))
        || (tile_y == 4 && (tile_x == 0 || tile_x == 4 || tile_x == 5 || tile_x == 8));
    return bark_side || natural_ground;
}

fn local_grade(surface_data: vec4<f32>) -> vec3<f32> {
    let warmth = surface_data.x;
    let moisture = surface_data.y;
    let lushness = surface_data.z;
    let arid = max(warmth - moisture, 0.0);

    return vec3<f32>(
        0.93 + warmth * 0.10 + arid * 0.03,
        0.92 + lushness * 0.09,
        0.90 + moisture * 0.16,
    );
}

fn vegetation_grade(surface_data: vec4<f32>, biome_tint: vec3<f32>) -> vec3<f32> {
    let warmth = surface_data.x;
    let moisture = surface_data.y;
    let lushness = surface_data.z;
    let climate_bias = vec3<f32>(
        0.90 + warmth * 0.14,
        0.88 + lushness * 0.18,
        0.86 + moisture * 0.24,
    );
    return biome_tint * climate_bias;
}

@vertex
fn vs_main(v: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    out.clip_position = camera.view_proj * vec4<f32>(v.position, 1.0);
    out.tex_coords = v.tex_coords;
    out.is_top = v.is_top;
    out.biome_tint = v.biome_tint;
    out.world_pos = v.position;
    out.world_normal = v.normal;
    out.surface_data = v.surface_data;
    out.foliage_tint = v.foliage_tint;

    let sun_phase = biome.view_info.w;
    let sun_dir = sun_direction(sun_phase);
    let sun_elev = sin((sun_phase - 0.25) * 6.283185307);
    let day = biome.view_info.x;
    let diffuse = max(dot(v.normal, sun_dir), 0.0);
    let moon_diffuse = max(dot(v.normal, -sun_dir), 0.0)
        * 0.09 * clamp(-sun_elev * 2.0, 0.0, 1.0);
    let ambient_strength = biome.ambient.w * 0.42;
    let ambient = mix(max(ambient_strength * 0.28, 0.07), ambient_strength, day);

    out.light = (ambient + diffuse * 0.72 * day + moon_diffuse) * v.brightness;
    return out;
}

@fragment
fn fs_main(fs_input: VertexOutput) -> @location(0) vec4<f32> {
    let atlas_tile = vec2<f32>(16.0 / 512.0, 16.0 / 320.0);
    let tile_space = fs_input.tex_coords / atlas_tile;
    let tile_index = floor(tile_space);
    let tile_x = i32(tile_index.x);
    let tile_y = i32(tile_index.y);
    let tile_eps = 0.5 / 16.0;

    var local_uv = canonical_tile_uv(fract(tile_space), fs_input.is_top);
    if (rotation_whitelist(tile_x, tile_y) && !rotation_blacklist(tile_x, tile_y)) {
        local_uv = varied_tile_uv(local_uv, fs_input.surface_data.w);
    }
    local_uv = clamp(local_uv, vec2<f32>(tile_eps), vec2<f32>(1.0 - tile_eps));

    let tex_coords = tile_index * atlas_tile + local_uv * atlas_tile;
    let color = textureSample(t_atlas, s_atlas, tex_coords);
    if (color.a < 0.01) {
        discard;
    }

    let day = biome.view_info.x;
    let fog_start = biome.view_info.y;
    let fog_end = biome.view_info.z;
    let sun_phase = biome.view_info.w;
    let sun_dir = sun_direction(sun_phase);
    let sky_col = sky_color(day, sun_phase);
    let fog_col = mix(sky_col, biome.fog_color.xyz, 0.68);
    let view_dist = length(fs_input.world_pos - biome.camera_pos.xyz);
    let fog_density = max(biome.fog_color.w, 0.001);
    let fog_band = smoothstep(fog_start * 0.55, fog_end, view_dist);
    let dist_fog = 1.0 - exp(-fog_band * fog_band * (1.35 + fog_density * 1.85));

    let h_above = max(0.0, fs_input.world_pos.y - biome.camera_pos.w);
    let mist_strength = (0.04 + fs_input.surface_data.y * 0.06) * fog_density;
    let h_fog = 1.0 - exp(
        -exp(-h_above * (0.068 + (1.0 - fs_input.surface_data.x) * 0.028))
        * mist_strength
        * (0.85 + fog_band * 1.35)
    );

    if (emissive_tile(tile_x, tile_y)) {
        let emissive = color.rgb * mix(1.55, 1.0, day);
        return vec4<f32>(mix(emissive, fog_col, dist_fog * 0.5), color.a);
    }

    if (water_tile(tile_x, tile_y)) {
        let water_uv = canonical_tile_uv(fract(tile_space), fs_input.is_top);
        let wt = biome.grade.w;
        let wu1 = clamp(
            water_uv.x + sin(water_uv.y * 14.0 + wt * 2.4) * 0.035
                + cos(water_uv.x * 8.0 + wt * 1.3) * 0.018,
            tile_eps,
            1.0 - tile_eps,
        );
        let wv1 = clamp(
            water_uv.y + cos(water_uv.x * 12.0 + wt * 1.9) * 0.035
                + sin(water_uv.y * 9.0 + wt * 2.6) * 0.018,
            tile_eps,
            1.0 - tile_eps,
        );
        let wu2 = clamp(
            water_uv.x - sin(water_uv.y * 7.0 + wt * 1.5) * 0.026
                + cos(wt * 0.7) * 0.012,
            tile_eps,
            1.0 - tile_eps,
        );
        let wv2 = clamp(
            water_uv.y - cos(water_uv.x * 6.0 + wt * 1.1) * 0.026
                + sin(wt * 0.9) * 0.012,
            tile_eps,
            1.0 - tile_eps,
        );
        let tc1 = tile_index * atlas_tile + vec2<f32>(wu1, wv1) * atlas_tile;
        let tc2 = tile_index * atlas_tile + vec2<f32>(wu2, wv2) * atlas_tile;
        let c1 = textureSample(t_atlas, s_atlas, tc1).rgb;
        let c2 = textureSample(t_atlas, s_atlas, tc2).rgb;
        let wc = (c1 + c2) * 0.5;

        let water_grade = vec3<f32>(
            0.14 + fs_input.surface_data.y * 0.10,
            0.26 + fs_input.surface_data.z * 0.14,
            0.42 + fs_input.surface_data.y * 0.16,
        );
        let deep = mix(vec3<f32>(0.08, 0.16, 0.30), fs_input.biome_tint * water_grade, 0.58);
        var ws = mix(
            wc * deep,
            deep * (0.68 + fs_input.surface_data.y * 0.08),
            0.22 + fs_input.surface_data.z * 0.05,
        );

        let view_d = normalize(biome.camera_pos.xyz - fs_input.world_pos);
        let up = vec3<f32>(0.0, 1.0, 0.0);
        let cos_theta = max(dot(view_d, up), 0.0);
        let fresnel = 0.02 + 0.22 * pow(1.0 - cos_theta, 5.0);
        let half_v = normalize(sun_dir + view_d);
        let spec = pow(max(dot(up, half_v), 0.0), 88.0) * 0.10 * day;
        let sky_refl = fog_col * fresnel * 0.10;
        let water_light = 0.46 + fs_input.light * 0.24;

        let alpha = clamp(
            0.54 + fresnel * 0.12 + sin(wt * 2.2 + water_uv.x * 6.0) * 0.02,
            0.46,
            0.82,
        );
        ws = ws * water_light + spec + sky_refl;
        ws = mix(ws, fog_col, dist_fog);
        return vec4<f32>(ws, alpha);
    }

    let scene_grade = mix(vec3<f32>(1.0), biome.grade.xyz * local_grade(fs_input.surface_data), 0.48);
    let ambient_tint = mix(vec3<f32>(1.0), biome.ambient.xyz, 0.22);
    let variation = 0.96 + fs_input.surface_data.w * 0.08;
    var sampled = color.rgb * scene_grade * ambient_tint * variation;

    if (grass_top_tile(tile_x, tile_y)) {
        sampled *= mix(vec3<f32>(1.0), vegetation_grade(fs_input.surface_data, fs_input.biome_tint), 0.92);
    } else if (fs_input.foliage_tint > 0.5) {
        sampled *= mix(
            vec3<f32>(1.0),
            vegetation_grade(fs_input.surface_data, fs_input.biome_tint) * vec3<f32>(0.94, 0.98, 0.88),
            0.86,
        );
    }

    let view_d2 = normalize(biome.camera_pos.xyz - fs_input.world_pos);
    let half_vec = normalize(sun_dir + view_d2);
    let spec2 = pow(max(dot(fs_input.world_normal, half_vec), 0.0), 28.0) * 0.045 * day;
    let lit_rgb = sampled * fs_input.light + spec2;
    let mist_col = mix(fog_col, biome.fog_color.xyz * biome.grade.xyz, 0.28);
    let fog_amount = clamp(1.0 - (1.0 - dist_fog) * (1.0 - h_fog), 0.0, 1.0);
    let final_rgb = mix(lit_rgb, mist_col, fog_amount);

    return vec4<f32>(final_rgb, color.a);
}