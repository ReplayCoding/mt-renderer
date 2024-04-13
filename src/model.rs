use std::{collections::HashMap, path::PathBuf};

use log::info;
use wgpu::util::DeviceExt;

use crate::{
    rmaterial::MaterialFile,
    rmodel::ModelFile,
    rshader2::{Shader2File, Shader2ObjectTypedInfo},
    rtexture::TextureFile,
    texture::Texture,
};

pub struct Model {
    vertexbuf: wgpu::Buffer,
    indexbuf: wgpu::Buffer,

    debug_ids: Vec<wgpu::BindGroup>,

    // (vertex_stride, material_no, inputlayout)
    pipelines: HashMap<(u32, u32, u32), wgpu::RenderPipeline>,

    primitives: Vec<crate::rmodel::PrimitiveInfo>,
    textures: Vec<Texture>,
    mat_to_tex: Vec<Option<usize>>,
}

impl Model {
    pub fn new(
        model_file: &ModelFile,
        material_file: &MaterialFile,
        shader2: &Shader2File,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        transform_bind_group_layout: &wgpu::BindGroupLayout,
        swapchain_format: wgpu::TextureFormat,
    ) -> anyhow::Result<Self> {
        let textures: Vec<_> = material_file
            .textures()
            .iter()
            .map(|path| {
                // TODO: This is awful
                let path = PathBuf::from("/home/user/Desktop/WIN11-vm-folder/scripts/out/chr055")
                    .join(PathBuf::from(&path.replace('\\', "/")))
                    .with_extension("rTexture");
                info!("Loading texture {:?}", path);
                let mut file = std::fs::File::open(&path).unwrap();
                let texture = TextureFile::new(&mut file).unwrap();

                Texture::new(device, queue, texture)
            })
            .collect();

        let mat_to_tex: Vec<_> = model_file
            .material_names()
            .iter()
            .map(|name| {
                let info = material_file.material_by_name(name)?;

                if info.mat_type() == "nDraw::MaterialToon" {
                    info.albedo_texture_idx()
                } else {
                    None
                }
            })
            .collect();

        let vertexbuf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("rModel vertex buffer"),
            contents: model_file.vertex_buf(),
            usage: wgpu::BufferUsages::VERTEX,
        });

        let indexbuf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("rModel index buffer"),
            contents: bytemuck::cast_slice(model_file.index_buf()),
            usage: wgpu::BufferUsages::INDEX,
        });

        // Load the shaders from disk
        let textured_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: None,
            source: wgpu::ShaderSource::Wgsl(std::borrow::Cow::Borrowed(include_str!(
                "shaders/textured.wgsl"
            ))),
        });

        let debug_id_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: None,
            source: wgpu::ShaderSource::Wgsl(std::borrow::Cow::Borrowed(include_str!(
                "shaders/debug_ids.wgsl"
            ))),
        });

        let debug_id_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("rModel debug id bind group layout"),
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

        let mut pipelines = HashMap::new();
        let mut debug_ids: Vec<wgpu::BindGroup> = vec![];

        let primitives: Vec<_> = model_file
            .primitives()
            .iter()
            .filter(|prim| {
                let mat_name = &model_file.material_names()[prim.material_no() as usize];
                let mat_info = material_file.material_by_name(mat_name).unwrap();

                mat_info.mat_type() == "nDraw::MaterialToon"
            })
            .copied()
            .collect();

        for (_idx, primitive) in primitives.iter().enumerate() {
            let debug_id_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("rModel debug id buffer"),
                contents: bytemuck::cast_slice(&[(primitive.inputlayout() & 0xfffff000) >> 0xc]),
                usage: wgpu::BufferUsages::UNIFORM,
            });

            let debug_id_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("rModel debug id group"),
                layout: &debug_id_bind_group_layout,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: debug_id_buffer.as_entire_binding(),
                }],
            });

            debug_ids.push(debug_id_bind_group);

            // Create pipeline if needed
            pipelines
                .entry((primitive.vertex_stride(), primitive.material_no(), primitive.inputlayout()))
                .or_insert_with(|| {
                    let mut textured = false;
                    let mut bind_group_layouts =
                        vec![transform_bind_group_layout, &debug_id_bind_group_layout];

                    if let Some(tex_idx) = mat_to_tex[primitive.material_no() as usize] {
                        let layout = textures[tex_idx].bind_group_layout();
                        textured = true;
                        bind_group_layouts.push(layout);
                    };

                    let pipeline_layout =
                        device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                            label: None,
                            bind_group_layouts: &bind_group_layouts,
                            push_constant_ranges: &[],
                        });

                    let inputlayout_obj = shader2
                        .get_object_by_handle(primitive.inputlayout())
                        .unwrap_or_else(|| panic!("invalid inputlayout {:08x}",
                            (primitive.inputlayout() as u64)));

                    let inputlayout_specific = if let Shader2ObjectTypedInfo::InputLayout(spec) =
                        inputlayout_obj.obj_specific()
                    {
                        spec
                    } else {
                        unreachable!("primitive inputlayout isn't an inputlayout!")
                    };

                    let material_name = &model_file.material_names()[primitive.material_no() as usize];
                    let attributes =
                        Shader2File::create_vertex_buffer_elements(inputlayout_specific);
                    info!(
                        "Creating layout for {} {}: {:#?} (textured {}) (mat {}) (topo {:?})",
                        (primitive.inputlayout() & 0xfffff000) >> 0xc,
                        inputlayout_obj.name(),
                        attributes,
                        textured,
                        material_name,
                        primitive.topology()
                    );

                    let vertex_buffer_layouts = [wgpu::VertexBufferLayout {
                        array_stride: primitive.vertex_stride().into(),
                        step_mode: wgpu::VertexStepMode::Vertex,
                        attributes: &attributes,
                    }];

                    let shader = if textured && attributes.len() != 1 {
                        &textured_shader
                    } else {
                        &debug_id_shader
                    };

                    let render_pipeline =
                        device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                            label: Some(
                                format!(
                                    "rModel render pipeline for: stride {} textured {} inputlayout {} material {} topology {:?}",
                                    primitive.vertex_stride(),
                                    textured,
                                    inputlayout_obj.name(),
                                    material_name,
                                    primitive.inputlayout()
                                )
                                .leak(),
                            ),
                            layout: Some(&pipeline_layout),
                            vertex: wgpu::VertexState {
                                module: shader,
                                entry_point: "vs_main",
                                buffers: &vertex_buffer_layouts,
                            },
                            fragment: Some(wgpu::FragmentState {
                                module: shader,
                                entry_point: "fs_main",
                                targets: &[Some(wgpu::ColorTargetState {
                                    format: swapchain_format,
                                    write_mask: wgpu::ColorWrites::ALL,
                                    blend: None,
                                })],
                            }),
                            primitive: wgpu::PrimitiveState {
                                topology: primitive.topology().to_wgpu(),
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
            vertexbuf,
            indexbuf,
            pipelines,
            debug_ids,
            primitives,
            textures,
            mat_to_tex,
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

        rpass.set_bind_group(0, transform_bind_group, &[]);
        rpass.set_index_buffer(self.indexbuf.slice(..), wgpu::IndexFormat::Uint16);
        for (id, primitive) in self.primitives.iter().enumerate() {
            rpass.set_bind_group(1, &self.debug_ids[id], &[]);

            if let Some(tex_idx) = self.mat_to_tex[primitive.material_no() as usize] {
                rpass.set_bind_group(2, self.textures[tex_idx].bind_group(), &[]);
            };

            // TODO: Should we try to make this as small as possible?
            // XXX: What does vertex_ofs do
            rpass.set_vertex_buffer(0, self.vertexbuf.slice(primitive.vertex_base() as u64..));

            let pipeline = self
                .pipelines
                .get(&(
                    primitive.vertex_stride(),
                    primitive.material_no(),
                    primitive.inputlayout(),
                ))
                .unwrap();
            rpass.set_pipeline(pipeline);

            let index_ofs = primitive.index_ofs();
            let index_num = primitive.index_num();

            rpass.draw_indexed(
                index_ofs..(index_ofs + index_num),
                primitive.index_base() as i32,
                0..1,
            )
        }
    }
}
