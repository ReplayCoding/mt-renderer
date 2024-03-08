struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(1) texcoord: vec2<f32>,
}

@group(0) @binding(0)
var<uniform> transform: mat4x4<f32>;

@vertex
fn vs_main(
        @location(0) position: vec3<f32>,
        @location(2) texcoord: vec2<f32>,
    ) -> VertexOutput {
    var out: VertexOutput;
    out.position = transform * vec4(position, 1.0);
    out.texcoord = texcoord;

    return out;
}

@group(1) @binding(0)
var awesome_texture: texture_2d<f32>;

@group(1) @binding(1)
var awesome_sampler: sampler;

@fragment
fn fs_main(vertex: VertexOutput) -> @location(0) vec4<f32> {
    return textureSample(awesome_texture, awesome_sampler, vertex.texcoord);
}
