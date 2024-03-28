struct VertexOutput {
    @builtin(position) position: vec4<f32>,
}

@group(0) @binding(0)
var<uniform> transform: mat4x4<f32>;

@vertex
fn vs_main(
        @location(0) position: vec3<f32>,
    ) -> VertexOutput {
    var out: VertexOutput;
    out.position = transform * vec4(position, 1.0);

    return out;
}

@group(1) @binding(0)
var<uniform> primitive_id: u32;

@fragment
fn fs_main(vertex: VertexOutput) -> @location(0) vec4<f32> {
    var colors = array<vec3<f32>, 20>(
        vec3f(215,62,103),
        vec3f(95,190,80),
        vec3f(133,95,213),
        vec3f(180,184,53),
        vec3f(213,87,180),
        vec3f(72,138,55),
        vec3f(145,79,158),
        vec3f(91,196,153),
        vec3f(206,78,55),
        vec3f(74,174,209),
        vec3f(225,133,58),
        vec3f(92,122,198),
        vec3f(207,162,81),
        vec3f(188,144,216),
        vec3f(152,173,92),
        vec3f(161,71,103),
        vec3f(53,133,98),
        vec3f(225,131,152),
        vec3f(111,111,40),
        vec3f(162,99,55)
    );

    return vec4(colors[primitive_id] / 255, 1.f);
}
