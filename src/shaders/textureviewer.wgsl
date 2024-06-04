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
    // r0.xyzw = tGUIBaseMap.Sample(SSGUI_s, r0.xy).xzwy;
    var r0: vec4f = textureSample(tex_texture, tex_sampler, vec2f(in.texcoord.x, 1 - in.texcoord.y)).xzwy;

    var r1: vec4f = r0.xyxy - 0.482353002;
    r1.y = -r1.y * 0.344139993 + r0.z;
    r0.y = -r1.z * 0.714139998 + r1.y;

    let r0xz: vec2f = (r1.xw * vec2f(1.40199995,1.77199996)) + r0.zz;
    r0.x = r0xz.x;
    r0.z = r0xz.y;

    return r0;
}
