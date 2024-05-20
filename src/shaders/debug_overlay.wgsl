struct VertexOutput {
    @builtin(position) position: vec4<f32>,
}

struct VertexPosInput {
    @location(1) a: vec4f,
    @location(2) b: vec4f,
    @location(3) c: vec4f,
    @location(4) d: vec4f,
}

@group(0) @binding(0)
var<uniform> camera_transform: mat4x4<f32>;

@vertex
fn vs_main(
        @location(0) base_position: vec3<f32>,
        position: VertexPosInput,
    ) -> VertexOutput {
    var out: VertexOutput;
    let position_matrix = mat4x4<f32>(position.a, position.b, position.c, position.d);

    out.position = camera_transform * position_matrix * vec4f(base_position, 1.f);

    return out;
}

@fragment
fn fs_main(vertex: VertexOutput) -> @location(0) vec4<f32> {
    return vec4(0.1f, 0.2f, 0.3f, 1.f);
}
