struct VertexInput {
    @location(0) position: vec2<f32>,
    @location(1) uv:       vec2<f32>,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs_main(input: VertexInput) -> VertexOutput {
    var out: VertexOutput;
    out.clip_position = vec4<f32>(input.position, 0.0, 1.0);
    out.uv = input.uv;
    return out;
}

@group(0) @binding(0)
var subtitle_tex: texture_2d<f32>;

@group(0) @binding(1)
var subtitle_sampler: sampler;

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    let s = textureSample(subtitle_tex, subtitle_sampler, input.uv);
    // s.r is glyph coverage (we upload white color with alpha as coverage)
    return vec4<f32>(s.rgb, s.a);
}
