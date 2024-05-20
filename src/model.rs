use std::{collections::HashMap, path::PathBuf};

use log::{debug, info, trace};
use wgpu::util::DeviceExt;
use zerocopy::AsBytes;

use crate::{
    debug_overlay::DebugOverlay,
    resource_manager::ResourceManager,
    rmaterial::MaterialFile,
    rmodel::ModelFile,
    rshader2::{Shader2File, Shader2ObjectTypedInfo},
    rtexture::TextureFile,
    texture::Texture,
    DTIs,
};

pub struct Model {
    vertexbuf: wgpu::Buffer,
    indexbuf: wgpu::Buffer,

    debug_ids: Vec<wgpu::BindGroup>,

    // (vertex_stride, material_no, inputlayout)
    pipelines: HashMap<(u32, u32, u32), wgpu::RenderPipeline>,

    primitives: Vec<crate::rmodel::PrimitiveInfo>,
    textures: Vec<Option<Texture>>,
    mat_to_tex: Vec<Option<usize>>,
    parts_disp: Vec<bool>,

    joint_positions: Vec<glam::Vec3>,
}

impl Model {
    pub fn new(
        model_file: &ModelFile,
        material_file: &MaterialFile,
        shader2: &Shader2File,
        resource_manager: &ResourceManager,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        transform_bind_group_layout: &wgpu::BindGroupLayout,
        swapchain_format: wgpu::TextureFormat,
    ) -> anyhow::Result<Self> {
        let textures: Vec<_> = material_file
            .textures()
            .iter()
            .map(|path| {
                trace!("Loading texture {:?}", path);
                let mut file = resource_manager
                    .get_resource(&PathBuf::from(&path.replace('\\', "/")), &DTIs::rTexture)
                    .ok()?;
                let texture = TextureFile::new(&mut file).ok()?;

                Some(Texture::new(device, queue, texture))
            })
            .collect();

        let mat_to_tex: Vec<_> = model_file
            .material_names()
            .iter()
            .map(|name| {
                let info = material_file.material_by_name(name)?;

                // HACK: This is awful and stupid. But i need a proper way of
                // handling materials before i can do anything about it
                if info.mat_type().name() == "nDraw::MaterialToon" {
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
            contents: model_file.index_buf().as_bytes(),
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
                if true {
                    // HACK
                    let mat_name = &model_file.material_names()[prim.material_no() as usize];
                    let mat_info = material_file.material_by_name(mat_name).unwrap();
                    mat_info.mat_type().name() == "nDraw::MaterialToon"
                } else {
                    true
                }
            })
            .cloned()
            .collect();

        for primitive in primitives.iter() {
            let debug_id: u32 =
                model_file.boundary_infos()[primitive.boundary_num() as usize].joint();
            let debug_id_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("rModel debug id buffer"),
                contents: [debug_id].as_bytes(),
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
                        let layout = textures[tex_idx].as_ref().expect("no texture found!").bind_group_layout();
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
                    debug!(
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
                                    blend: Some(wgpu::BlendState {
                                        color: wgpu::BlendComponent { src_factor: wgpu::BlendFactor::SrcAlpha, dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha, operation: wgpu::BlendOperation::Add },
                                        alpha: wgpu::BlendComponent { src_factor: wgpu::BlendFactor::One, dst_factor: wgpu::BlendFactor::Zero, operation: wgpu::BlendOperation::Add },
                                    }),
                                })],
                            }),
                            primitive: wgpu::PrimitiveState {
                                topology: primitive.topology().to_wgpu(),
                                strip_index_format: Some(wgpu::IndexFormat::Uint16),
                                cull_mode: Some(wgpu::Face::Back),
                                ..Default::default()
                            },
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

                    render_pipeline
                });
        }

        let parts_disp = vec![true; primitives.len()];

        let joint_info = model_file.joint_info();

        Ok(Self {
            vertexbuf,
            indexbuf,
            pipelines,
            debug_ids,
            primitives,
            textures,
            mat_to_tex,
            parts_disp,
            joint_positions: joint_info
                .infos()
                .iter()
                .enumerate()
                .map(|(idx, info)| {
                    let o = info.offset();
                    glam::vec3(o.x, o.y, o.z)
                })
                .collect(),
        })
    }

    pub fn set_parts_disp(&mut self, parts_disp: &[bool]) {
        self.parts_disp = parts_disp.to_vec()
    }

    pub fn render<'a>(
        &'a self,
        rpass: &mut wgpu::RenderPass<'a>,
        queue: &wgpu::Queue,
        transform_bind_group: &'a wgpu::BindGroup,
        debug_overlay: &mut DebugOverlay,
    ) {
        rpass.set_bind_group(0, transform_bind_group, &[]);
        rpass.set_index_buffer(self.indexbuf.slice(..), wgpu::IndexFormat::Uint16);

        for joint_pos in &self.joint_positions {
            debug_overlay.add_cube(
                queue,
                *joint_pos * glam::Vec3::splat(0.01),
                glam::Vec3::splat(0.005),
            );
        }

        for (id, primitive) in self.primitives.iter().enumerate() {
            if !self.parts_disp[primitive.parts_no() as usize] {
                continue;
            }

            rpass.set_bind_group(1, &self.debug_ids[id], &[]);

            if let Some(tex_idx) = self.mat_to_tex[primitive.material_no() as usize] {
                rpass.set_bind_group(
                    2,
                    self.textures[tex_idx]
                        .as_ref()
                        .expect("no texture found")
                        .bind_group(),
                    &[],
                );
            };

            // TODO: Are these bounds correct?
            // XXX: What does vertex_ofs do
            let vertex_range = primitive.vertex_base() as u64
                ..(primitive.vertex_base() + (primitive.vertex_num() * primitive.vertex_stride()))
                    as u64;

            // trace!("drawing vertex range: {:?}", vertex_range);
            rpass.set_vertex_buffer(0, self.vertexbuf.slice(vertex_range));

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
