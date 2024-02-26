struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) color: vec4<f32>,
}

@group(0) @binding(0)
var<uniform> rotation: mat4x4<f32>;

@vertex
fn vs_main(
        @location(0) position: vec3<f32>,
        @location(1) color: vec3<f32>
    ) -> VertexOutput {
    var out: VertexOutput;
    out.position = rotation * vec4(position, 1.0);
    out.color = vec4(color, 1.0);

    return out;
}

@fragment
fn fs_main(vertex: VertexOutput) -> @location(0) vec4<f32> {
    return vertex.color;
}
