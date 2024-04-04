use bytemuck::{Pod, Zeroable};
use log::debug;
use std::{
    borrow::Cow,
    collections::HashMap,
    ffi::CStr,
    io::{Read, Seek},
};
use wgpu::{util::DeviceExt, TextureFormat};

use crate::rshader2::{Shader2, Shader2ObjectTypedInfo};

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable, Debug)]
struct MtVector3 {
    x: f32,
    y: f32,
    z: f32,
    pad_: f32,
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable, Debug)]
struct MtAABB {
    minpos: MtVector3,
    maxpos: MtVector3,
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable, Debug)]
struct MtFloat3A {
    x: f32,
    y: f32,
    z: f32,
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable, Debug)]
struct MtSphere {
    pos: MtFloat3A,
    r: f32,
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable, Debug)]
struct MODEL_INFO {
    middist: i32,
    lowdist: i32,
    light_group: u32,
    memory: u16,
    reserved: u16,
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable, Debug)]
struct MODEL_HDR {
    magic: u32,
    version: u16,
    jnt_num: u16,
    primitive_num: u16,
    material_num: u16,
    vertex_num: u32,
    index_num: u32,
    polygon_num: u32,
    vertexbuf_size: u32,
    texture_num: u32,
    parts_num: u32,
    padding1: u32,
    joint_info: u64,
    parts_info: u64,
    material_info: u64,
    primitive_info: u64,
    vertex_data: u64,
    index_data: u64,
    rcn_data: u64,
    bounding_sphere: MtSphere,
    bounding_box: MtAABB,
    modelinfo: MODEL_INFO,
}

#[repr(C, packed)]
#[derive(Clone, Copy, Pod, Zeroable, Debug)]
struct PRIMITIVE_INFO {
    // u32 draw_mode:16;
    // u32 vertex_num:16;
    drawmode_vertexnum: u32,
    // u32 parts_no:12;
    // u32 material_no:12;
    // u32 lod:8;
    parts_material_lod: u32,

    // u32 disp:1;
    // u32 shape:1;
    // u32 sort:1;
    // u32 weight_num:5;
    // u32 alphapri:8;
    // u32 vertex_stride:8;
    // u32 topology:6;
    // u32 binormal_flip:1;
    // u32 bridge:1;
    very_large_bitfield: u32,

    vertex_ofs: u32,
    vertex_base: u32,
    inputlayout: u32, // SO_HANDLE
    index_ofs: u32,
    index_num: u32,
    index_base: u32,
    // u32 envelope:8;
    // u32 boundary_num:8;
    // u32 connect_id:16;
    envelope_boundary_connect: u32,
    // u32 min_index:16;
    // u32 max_index:16;
    min_max_index: u32,

    padding_: u32, // pointers are aligned to 8 bytes
    boundary: u64, // struct BOUNDARY_INFO *
}

impl PRIMITIVE_INFO {
    fn vertex_stride(&self) -> u32 {
        (self.very_large_bitfield >> 16) & 0xFF
    }
}

pub struct Model {
    primitives: Vec<PRIMITIVE_INFO>,

    vertexbuf: wgpu::Buffer,
    indexbuf: wgpu::Buffer,

    primitive_ids: Vec<wgpu::BindGroup>,

    pipelines: HashMap<u32, wgpu::RenderPipeline>,
}

impl Model {
    pub fn new<R: Read + Seek>(
        reader: &mut R,
        device: &wgpu::Device,
        shader2: &Shader2,
        transform_bind_group_layout: &wgpu::BindGroupLayout,
        swapchain_format: TextureFormat,
    ) -> anyhow::Result<Model> {
        let mut header_bytes: [u8; 0xa0] = [0; 0xa0];
        reader.read_exact(&mut header_bytes)?;

        let header: &MODEL_HDR = bytemuck::try_from_bytes(&header_bytes).unwrap();

        debug!("model header: {:#?}", header);

        let mut material_bytes = vec![0u8; header.material_num as usize * 128];
        reader.seek(std::io::SeekFrom::Start(header.material_info as u64))?;
        reader.read_exact(&mut material_bytes)?;
        let _materials: Vec<String> = (0..header.material_num as usize)
            .map(|material_idx| {
                let material_name_bytes =
                    &material_bytes[material_idx * 128..(material_idx + 1) * 128];
                let material_name = CStr::from_bytes_until_nul(material_name_bytes)
                    .unwrap()
                    .to_string_lossy();

                material_name.to_string()
            })
            .collect();

        let mut primitive_arr_bytes = vec![0u8; header.primitive_num as usize * 0x38];
        reader.seek(std::io::SeekFrom::Start(header.primitive_info as u64))?;
        reader.read_exact(&mut primitive_arr_bytes)?;
        let primitives: Vec<PRIMITIVE_INFO> = (0..header.primitive_num as usize)
            .map(|primitive_idx| {
                let primitive_bytes =
                    &primitive_arr_bytes[primitive_idx * 0x38..(primitive_idx + 1) * 0x38];
                let primitive: &PRIMITIVE_INFO =
                    bytemuck::try_from_bytes(&primitive_bytes).unwrap();

                let inputlayout_hash = (primitive.inputlayout & 0xfffff000) >> 0xc;
                let inputlayout_obj =
                    shader2
                        .get_object_by_hash(inputlayout_hash)
                        .expect(&format!(
                            "invalid inputlayout hash {:08x}",
                            inputlayout_hash
                        ));

                debug!(
                    "primitive {}: {} {} {:#?}",
                    primitive_idx,
                    primitive.vertex_stride(),
                    inputlayout_obj.name(),
                    primitive
                );
                primitive.clone()
            })
            .collect();

        let mut vertexbuf_bytes = vec![0u8; header.vertexbuf_size as usize];
        reader.seek(std::io::SeekFrom::Start(header.vertex_data))?;
        reader.read_exact(&mut vertexbuf_bytes)?;

        let mut indexbuf_bytes = vec![0u16; header.index_num as usize];
        reader.seek(std::io::SeekFrom::Start(header.index_data))?;
        reader.read_exact(&mut bytemuck::cast_slice_mut(&mut indexbuf_bytes))?;

        let vertexbuf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("rModel vertex buffer"),
            contents: &vertexbuf_bytes,
            usage: wgpu::BufferUsages::VERTEX,
        });

        let indexbuf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("rModel index buffer"),
            contents: bytemuck::cast_slice(&indexbuf_bytes),
            usage: wgpu::BufferUsages::INDEX,
        });

        // Load the shaders from disk
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: None,
            source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(include_str!("shader.wgsl"))),
        });

        let primitive_id_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("rModel primitive id bind group layout"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
            });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: None,
            bind_group_layouts: &[transform_bind_group_layout, &primitive_id_bind_group_layout],
            push_constant_ranges: &[],
        });

        let mut pipelines = HashMap::new();
        let mut primitive_ids: Vec<wgpu::BindGroup> = vec![];

        for (idx, primitive) in primitives.iter().enumerate() {
            let primitive_id_buffer =
                device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("rModel primitive id buffer"),
                    contents: bytemuck::cast_slice(&[idx as u32]),
                    usage: wgpu::BufferUsages::UNIFORM,
                });

            let primitive_id_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("rModel primitive id bind group"),
                layout: &primitive_id_bind_group_layout,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: primitive_id_buffer.as_entire_binding(),
                }],
            });

            primitive_ids.push(primitive_id_bind_group);

            // Create pipeline if needed
            pipelines
                .entry(primitive.vertex_stride())
                .or_insert_with(|| {
                    let inputlayout_hash = (primitive.inputlayout & 0xfffff000) >> 0xc;
                    let inputlayout_obj =
                        shader2
                            .get_object_by_hash(inputlayout_hash)
                            .expect(&format!(
                                "invalid inputlayout hash {:08x}",
                                inputlayout_hash
                            ));

                    let inputlayout_specific = if let Shader2ObjectTypedInfo::InputLayout(spec) = inputlayout_obj.obj_specific() {
                        spec
                    } else {
                        unreachable!("primitive inputlayout isn't an inputlayout!")
                    };

                    let attributes = Shader2::create_vertex_buffer_elements(&inputlayout_specific);
                    debug!("Creating layout for {}: {:#?}", inputlayout_obj.name(), attributes);
                    let vertex_buffer_layouts = [wgpu::VertexBufferLayout {
                        array_stride: primitive.vertex_stride().into(),
                        step_mode: wgpu::VertexStepMode::Vertex,
                        attributes: &attributes,
                    }];

                    let render_pipeline =
                        device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                            label: Some(
                                format!(
                                    "rModel render pipeline for: stride {}",
                                    primitive.vertex_stride()
                                )
                                .leak(),
                            ),
                            layout: Some(&pipeline_layout),
                            vertex: wgpu::VertexState {
                                module: &shader,
                                entry_point: "vs_main",
                                buffers: &vertex_buffer_layouts,
                            },
                            fragment: Some(wgpu::FragmentState {
                                module: &shader,
                                entry_point: "fs_main",
                                targets: &[Some(wgpu::ColorTargetState {
                                    format: swapchain_format,
                                    write_mask: wgpu::ColorWrites::ALL,
                                    blend: None,
                                })],
                            }),
                            primitive: wgpu::PrimitiveState {
                                topology: wgpu::PrimitiveTopology::TriangleStrip,
                                strip_index_format: Some(wgpu::IndexFormat::Uint16),
                                ..Default::default()
                            },
                            depth_stencil: Some(wgpu::DepthStencilState {
                                format: wgpu::TextureFormat::Depth24Plus,
                                depth_write_enabled: true,
                                depth_compare: wgpu::CompareFunction::Less,
                                stencil: wgpu::StencilState::default(),
                                bias: wgpu::DepthBiasState::default(),
                            }),
                            multisample: wgpu::MultisampleState::default(),
                            multiview: None,
                        });

                    render_pipeline
                });
        }

        Ok(Self {
            primitives,
            vertexbuf,
            indexbuf,
            pipelines,
            primitive_ids,
        })
    }

    pub fn render(
        &self,
        color_view: &wgpu::TextureView,
        depth_texture: &wgpu::TextureView,
        transform_bind_group: &wgpu::BindGroup,
        encoder: &mut wgpu::CommandEncoder,
    ) {
        let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: None,
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: color_view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::WHITE),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: depth_texture,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(1.0),
                    store: wgpu::StoreOp::Store,
                }),
                stencil_ops: None,
            }),
            timestamp_writes: None,
            occlusion_query_set: None,
        });

        rpass.set_bind_group(0, &transform_bind_group, &[]);
        rpass.set_index_buffer(self.indexbuf.slice(..), wgpu::IndexFormat::Uint16);
        for (id, primitive) in self.primitives.iter().enumerate() {
            rpass.set_bind_group(1, &self.primitive_ids[id], &[]);
            // TODO: Should we try to make this as small as possible?
            // XXX: What does vertex_ofs do
            rpass.set_vertex_buffer(0, self.vertexbuf.slice(primitive.vertex_base as u64..));

            let pipeline = self.pipelines.get(&primitive.vertex_stride()).unwrap();
            rpass.set_pipeline(pipeline);
            let index_ofs = primitive.index_ofs;
            let index_num = primitive.index_num;

            rpass.draw_indexed(
                index_ofs..(index_ofs + index_num),
                primitive.index_base as i32,
                0..1,
            )
        }
    }
}

#[test]
fn test_struct_sizes() {
    assert_eq!(std::mem::size_of::<MODEL_HDR>(), 0xa0);
    assert_eq!(std::mem::size_of::<PRIMITIVE_INFO>(), 0x38);
}
