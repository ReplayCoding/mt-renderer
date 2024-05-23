use std::mem::size_of;

use wgpu::util::DeviceExt;
use zerocopy::AsBytes;

use crate::camera::Camera;

#[rustfmt::skip]
// position: vec3f
const CUBE_VERTS: [f32; 3 * 8] = [
    1.0,  1.0, -1.0,
    1.0, -1.0, -1.0,
    1.0,  1.0,  1.0,
    1.0, -1.0,  1.0,
    -1.0,  1.0, -1.0,
    -1.0, -1.0, -1.0,
    -1.0,  1.0,  1.0,
    -1.0, -1.0,  1.0,
];

#[rustfmt::skip]
const CUBE_INDICES: [u16; 3 * 12] = [
    4, 2, 0,
    2, 7, 3,
    6, 5, 7,
    1, 7, 5,
    0, 3, 1,
    4, 1, 5,
    4, 6, 2,
    2, 6, 7,
    6, 4, 5,
    1, 3, 7,
    0, 2, 3,
    4, 0, 1,
];

type CubeMat = [f32; 16];

pub struct DebugOverlay {
    cubes: Vec<CubeMat>,

    cube_vertex_buffer: wgpu::Buffer,
    cube_index_buffer: wgpu::Buffer,
    cube_pipeline: wgpu::RenderPipeline,

    cube_position_buffer: wgpu::Buffer,

    transform_bind_group: wgpu::BindGroup,
    transform_buffer: wgpu::Buffer,
}

impl DebugOverlay {
    const MIN_ALLOC_POSITIONS: u64 = 1024;

    pub fn new(
        device: &wgpu::Device,
        swapchain_format: wgpu::TextureFormat,
    ) -> Self {
        let cube_index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("debug overlay - cube index buffer"),
            contents: &CUBE_INDICES.as_bytes(),
            usage: wgpu::BufferUsages::INDEX,
        });
        let cube_vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("debug overlay - cube vertex buffer"),
            contents: &CUBE_VERTS.as_bytes(),
            usage: wgpu::BufferUsages::VERTEX,
        });

        let cube_position_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("debug overlay - cube position buffer"),
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            size: Self::MIN_ALLOC_POSITIONS * (4 * 3),
            mapped_at_creation: false,
        });

        let debug_overlay_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("debug overlay - shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/debug_overlay.wgsl").into()),
        });

        let transform_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("debug overlay - transform buffer"),
            size: size_of::<glam::Mat4>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let transform_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("debug overlay - transform bind group layout"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
            });

        let transform_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("debug overlay - transform bind group"),
            layout: &transform_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: transform_buffer.as_entire_binding(),
            }],
        });

        let cube_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("debug overlay - cube pipeline layout"),
            bind_group_layouts: &[&transform_bind_group_layout],
            push_constant_ranges: &[],
        });

        let cube_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("debug overlay - cube pipeline"),
            layout: Some(&cube_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &debug_overlay_shader,
                entry_point: "vs_main",
                buffers: &[
                    // Cube Data
                    wgpu::VertexBufferLayout {
                        array_stride: 3 * 4, // 3 * sizeof(f32)
                        step_mode: wgpu::VertexStepMode::Vertex,
                        attributes: &[wgpu::VertexAttribute {
                            format: wgpu::VertexFormat::Float32x3,
                            offset: 0,
                            shader_location: 0,
                        }],
                    },
                    // Cube Positions
                    wgpu::VertexBufferLayout {
                        array_stride: size_of::<CubeMat>() as u64,
                        step_mode: wgpu::VertexStepMode::Instance,
                        attributes: &[
                            wgpu::VertexAttribute {
                                format: wgpu::VertexFormat::Float32x4,
                                offset: 0,
                                shader_location: 1,
                            },
                            wgpu::VertexAttribute {
                                format: wgpu::VertexFormat::Float32x4,
                                offset: 16,
                                shader_location: 2,
                            },
                            wgpu::VertexAttribute {
                                format: wgpu::VertexFormat::Float32x4,
                                offset: 32,
                                shader_location: 3,
                            },
                            wgpu::VertexAttribute {
                                format: wgpu::VertexFormat::Float32x4,
                                offset: 48,
                                shader_location: 4,
                            },
                        ],
                    },
                ],
            },
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                cull_mode: Some(wgpu::Face::Back),
                ..Default::default()
            },
            fragment: Some(wgpu::FragmentState {
                module: &debug_overlay_shader,
                entry_point: "fs_main",
                targets: &[Some(wgpu::ColorTargetState {
                    format: swapchain_format,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth24Plus,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::LessEqual,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
        });

        Self {
            cubes: vec![],
            cube_vertex_buffer,
            cube_index_buffer,
            cube_pipeline,
            cube_position_buffer,

            transform_bind_group,
            transform_buffer,
        }
    }

    pub fn render<'a>(
        &'a self,
        rpass: &mut wgpu::RenderPass<'a>,
        queue: &wgpu::Queue,
        camera: &Camera,
    ) {
        let transform = camera.view_proj();

        queue.write_buffer(&self.transform_buffer, 0, transform.as_ref().as_bytes());

        rpass.set_pipeline(&self.cube_pipeline);
        rpass.set_vertex_buffer(0, self.cube_vertex_buffer.slice(..));
        rpass.set_vertex_buffer(1, self.cube_position_buffer.slice(..));

        rpass.set_index_buffer(self.cube_index_buffer.slice(..), wgpu::IndexFormat::Uint16);

        rpass.set_bind_group(0, &self.transform_bind_group, &[]);

        rpass.draw_indexed(0..CUBE_INDICES.len() as u32, 0, 0..self.cubes.len() as u32);
    }

    pub fn clear(&mut self) {
        self.cubes.clear();
    }

    pub fn add_cube(&mut self, queue: &wgpu::Queue, position: glam::Vec3, scale: glam::Vec3) {
        self.cubes.push(
            *glam::Mat4::from_scale_rotation_translation(scale, glam::Quat::IDENTITY, position)
                .as_ref(),
        );

        let cube_pos_buf_size = (self.cubes.len() * size_of::<CubeMat>()) as u64;
        if self.cube_position_buffer.size() < cube_pos_buf_size {
            todo!("resize cube position buffer");
        } else {
            queue.write_buffer(
                &self.cube_position_buffer,
                0,
                self.cubes.as_slice().as_bytes(),
            );
        }
    }
}
