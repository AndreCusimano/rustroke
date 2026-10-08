// Draws tessellated meshes. Positions are in logical points, colors are
// linear with premultiplied alpha, and the atlas is sRGB so sampling returns
// linear values. The render target is sRGB as well, so blending is linear.

struct Uniforms {
    // Size of the render target in logical points.
    screen_size: vec2<f32>,
    _padding: vec2<f32>,
};

@group(0) @binding(0) var<uniform> uniforms: Uniforms;
@group(0) @binding(1) var atlas: texture_2d<f32>;
@group(0) @binding(2) var atlas_sampler: sampler;

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) color: vec4<f32>,
};

@vertex
fn vs_main(
    @location(0) pos: vec2<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) color: vec4<f32>,
) -> VertexOutput {
    var out: VertexOutput;
    out.position = vec4<f32>(
        2.0 * pos.x / uniforms.screen_size.x - 1.0,
        1.0 - 2.0 * pos.y / uniforms.screen_size.y,
        0.0,
        1.0,
    );
    out.uv = uv;
    out.color = color;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    return in.color * textureSample(atlas, atlas_sampler, in.uv);
}
