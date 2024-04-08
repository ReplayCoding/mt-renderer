struct VertexOutput {
    @builtin(position) pos: vec4f,
    @location(0) texcoord: vec2f,
}

@vertex
fn vs_main(
        @location(0) pos: vec2f
    ) -> VertexOutput {

    var out: VertexOutput;
    out.pos = vec4f(pos, 0., 1.);
    out.texcoord = (pos + 1) / 2;
    return out;
}

@group(0) @binding(0)
var tex_texture: texture_2d<f32>;
@group(0) @binding(1)
var tex_sampler: sampler;

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    return textureSample(tex_texture, tex_sampler, vec2f(in.texcoord.x, 1 - in.texcoord.y));
}
