use std::{collections::HashMap, path::PathBuf};

use glam::Mat4;
use log::info;
use mt_renderer::{
    renderer_app_manager::{RendererApp, RendererAppManager, RendererAppManagerInternal},
    rmaterial::MaterialFile,
    rmodel::ModelFile,
    rshader2::{Shader2File, Shader2ObjectTypedInfo},
    rtexture::TextureFile,
    texture::Texture,
};

use wgpu::util::DeviceExt;

struct Model {
    vertexbuf: wgpu::Buffer,
    indexbuf: wgpu::Buffer,

    debug_ids: Vec<wgpu::BindGroup>,

    // (vertex_stride, material_no)
    pipelines: HashMap<(u32, u32), wgpu::RenderPipeline>,

    primitives: Vec<mt_renderer::rmodel::PrimitiveInfo>,
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
                let path = PathBuf::from("/home/user/Desktop/WIN11-vm-folder/scripts/out/chr040_eng")
                    .join(PathBuf::from(&path.replace("\\", "/")))
                    .with_extension("rTexture");
                info!("Loading texture {:?}", path);
                let mut file = std::fs::File::open(&path).unwrap();
                let texture = TextureFile::new(&mut file).unwrap();
                let texture = Texture::new(device, queue, texture);

                texture
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
            contents: &model_file.vertex_buf(),
            usage: wgpu::BufferUsages::VERTEX,
        });

        let indexbuf = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("rModel index buffer"),
            contents: bytemuck::cast_slice(&model_file.index_buf()),
            usage: wgpu::BufferUsages::INDEX,
        });

        // Load the shaders from disk
        let textured_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: None,
            source: wgpu::ShaderSource::Wgsl(std::borrow::Cow::Borrowed(include_str!(
                "../shaders/textured.wgsl"
            ))),
        });

        let debug_id_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: None,
            source: wgpu::ShaderSource::Wgsl(std::borrow::Cow::Borrowed(include_str!(
                "../shaders/debug_ids.wgsl"
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

        for (_idx, primitive) in model_file.primitives().iter().enumerate() {
            let debug_id_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("rModel primitive id buffer"),
                contents: bytemuck::cast_slice(&[primitive.material_no() as u32]),
                usage: wgpu::BufferUsages::UNIFORM,
            });

            let debug_id_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("rModel primitive debug id group"),
                layout: &debug_id_bind_group_layout,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: debug_id_buffer.as_entire_binding(),
                }],
            });

            debug_ids.push(debug_id_bind_group);

            // Create pipeline if needed
            pipelines
                .entry((primitive.vertex_stride(), primitive.material_no()))
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
                        .expect(&format!(
                            "invalid inputlayout {:08x}",
                            (primitive.inputlayout() as u64) // blah
                        ));

                    let inputlayout_specific = if let Shader2ObjectTypedInfo::InputLayout(spec) =
                        inputlayout_obj.obj_specific()
                    {
                        spec
                    } else {
                        unreachable!("primitive inputlayout isn't an inputlayout!")
                    };

                    let attributes =
                        Shader2File::create_vertex_buffer_elements(&inputlayout_specific);
                    info!(
                        "Creating layout for {}: {:#?} (textured {})",
                        inputlayout_obj.name(),
                        attributes,
                        textured,
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
                                    "rModel render pipeline for: stride {} textured {} inputlayout {}",
                                    primitive.vertex_stride(),
                                    textured,
                                    inputlayout_obj.name()
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
                                topology: wgpu::PrimitiveTopology::TriangleStrip, // TODO: Use primitive topology
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
            primitives: model_file.primitives().to_vec(),
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

        rpass.set_bind_group(0, &transform_bind_group, &[]);
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
                .get(&(primitive.vertex_stride(), primitive.material_no()))
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

struct ModelViewerApp {
    model: Model,

    transform_buf: wgpu::Buffer,
    transform_bind_group: wgpu::BindGroup,

    depth_texture: Option<wgpu::Texture>,
    depth_texture_view: Option<wgpu::TextureView>,
}

impl ModelViewerApp {
    fn update_depth_texture(&mut self, device: &wgpu::Device, width: u32, height: u32) {
        if let Some(depth_texture) = &self.depth_texture {
            if depth_texture.width() != width || depth_texture.height() != height {
                self.depth_texture = None;
                self.depth_texture_view = None;
            }
        }

        if self.depth_texture.is_none() {
            let depth_texture = device.create_texture(&wgpu::TextureDescriptor {
                label: Some("depth texture"),
                size: wgpu::Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Depth24Plus,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                view_formats: &[],
            });

            self.depth_texture_view =
                Some(depth_texture.create_view(&wgpu::TextureViewDescriptor::default()));
            self.depth_texture = Some(depth_texture);
        }
    }
}

impl RendererApp for ModelViewerApp {
    fn setup(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        swapchain_format: wgpu::TextureFormat,
    ) -> anyhow::Result<Self> {
        let args: Vec<_> = std::env::args().collect();

        let mut model_file = std::fs::File::open(&args[1])?;
        let mut material_file = std::fs::File::open(&args[2])?;
        let mut shader_file = std::fs::File::open("/home/user/Desktop/WIN11-vm-folder/TGAAC-for-research/nativeDX11x64/custom_shaders/CustomShaderPackage.mfx")?;
        let shader2 = Shader2File::new(&mut shader_file)?;
        let material = MaterialFile::new(&mut material_file, &shader2)?;

        let transform_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("transform buffer"),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            size: std::mem::size_of::<Mat4>() as u64,
            mapped_at_creation: false,
        });

        let transform_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("transform binding group layout"),
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
            label: Some("transform binding group"),
            layout: &transform_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: transform_buf.as_entire_binding(),
            }],
        });

        let model_file = ModelFile::new(&mut model_file)?;

        let model = Model::new(
            &model_file,
            &material,
            &shader2,
            device,
            queue,
            &transform_bind_group_layout,
            swapchain_format,
        )?;

        Ok(ModelViewerApp {
            model,

            transform_buf,
            transform_bind_group,

            depth_texture: None,
            depth_texture_view: None,
        })
    }

    fn render(
        &mut self,
        manager: &RendererAppManagerInternal,
        frame_view: &wgpu::TextureView,
        encoder: &mut wgpu::CommandEncoder,
    ) -> anyhow::Result<()> {
        // FIXME: this should probably be handled by manager
        self.update_depth_texture(
            manager.device(),
            manager.config().width,
            manager.config().height,
        );

        let transform_mat =
            compute_mat(manager.config().width as f32 / manager.config().height as f32);
        manager.queue().write_buffer(
            &self.transform_buf,
            0,
            bytemuck::cast_slice(transform_mat.as_ref()),
        );

        self.model.render(
            frame_view,
            &self.depth_texture_view.as_ref().unwrap(),
            &self.transform_bind_group,
            encoder,
        );

        Ok(())
    }
}

fn compute_mat(aspect: f32) -> Mat4 {
    let model = glam::Mat4::IDENTITY; // glam::Mat4::from_scale(glam::vec3(10.,10.,10.));

    let view = {
        let camera_pos = glam::vec3(0., 0.5, 1.);
        let camera_target = glam::vec3(0.0, 0.0, 0.0);
        let camera_direction = (camera_pos - camera_target).normalize();

        let up = glam::vec3(0., 1., 0.);
        let camera_right = up.cross(camera_direction).normalize();
        let camera_up = camera_direction.cross(camera_right).normalize();

        let camera_front = glam::vec3(0., 0., -1.);

        glam::Mat4::look_at_lh(camera_pos, camera_pos + camera_front, camera_up)
    };
    let proj = glam::Mat4::perspective_lh(70.0_f32.to_radians(), aspect, 0.01, 5.0);

    proj * view * model
}

pub fn main() -> anyhow::Result<()> {
    RendererAppManager::<ModelViewerApp>::run()
}
