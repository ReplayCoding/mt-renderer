struct VertexOutput {
    @builtin(position) pos: vec4f,
    @location(0) texcoord: vec2f,
}

@vertex
fn vs_main(
        @builtin(vertex_index) idx: u32
    ) -> VertexOutput {
    var verts = array<vec2f, 6>(
        vec2f(-1, -1),
        vec2f(-1, 1),
        vec2f(1, 1),

        vec2f(1, -1),
        vec2f(1, 1),
        vec2f(-1, -1),
    );

    var out: VertexOutput;
    out.pos = vec4f(verts[idx], 0., 1.);
    out.texcoord = (verts[idx] + 1) / 2;
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
