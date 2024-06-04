use std::{mem::size_of, path::PathBuf};

use glam::Mat4;
use mt_renderer::{
    camera::Camera,
    debug_overlay::DebugOverlay,
    get_enum_value,
    model::Model,
    mtserializer::{self, PropertyValue},
    renderer_app_manager::{RendererApp, RendererAppManager, RendererAppManagerPublic},
    resource_manager::ResourceManager,
    rmaterial::MaterialFile,
    rmodel::ModelFile,
    rshader2::Shader2File,
    DTIs,
};
use zerocopy::AsBytes;

struct ModelViewerApp {
    model: Model,
    debug_overlay: DebugOverlay,

    transform_buf: wgpu::Buffer,
    transform_bind_group: wgpu::BindGroup,

    depth_texture: Option<wgpu::Texture>,
    depth_texture_view: Option<wgpu::TextureView>,

    camera: Camera,
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
        public: &RendererAppManagerPublic,
        swapchain_format: wgpu::TextureFormat,
    ) -> anyhow::Result<Self> {
        public
            .window()
            .set_cursor_grab(winit::window::CursorGrabMode::Confined)?;
        public.window().set_cursor_visible(false);

        let args: Vec<_> = std::env::args().collect();

        let mut resource_manager = ResourceManager::new(&PathBuf::from(&args[1]));

        let mut shader_file = resource_manager
            .get_resource_fancy("custom_shaders/CustomShaderPackage", &DTIs::rShader2)?;
        let shader2 = Shader2File::new(&mut shader_file)?;

        let transform_buf = public.device().create_buffer(&wgpu::BufferDescriptor {
            label: Some("transform buffer"),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            size: size_of::<Mat4>() as u64,
            mapped_at_creation: false,
        });

        let transform_bind_group_layout =
            public
                .device()
                .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
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

        let transform_bind_group = public
            .device()
            .create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("transform binding group"),
                layout: &transform_bind_group_layout,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: transform_buf.as_entire_binding(),
                }],
            });

        let mut character_file =
            resource_manager.get_resource_fancy(&args[2], &DTIs::nGO__rCharacter)?;
        let character_info = mtserializer::deserialize(&mut character_file)?;

        let model_path: &String = &get_enum_value!(
            &&character_info
                .get_prop("mpModel")
                .expect("couldn't find mpModel")
                .values()[0],
            PropertyValue::Custom
        )[1]; // resource custom: (type, custom) TODO? handle customs properly
        let model_path = PathBuf::from(model_path.replace("\\", "/"));

        let parts_disp: Vec<bool> = character_info
            .get_prop("PartsDisp")
            .expect("couldn't find partsdisp")
            .values()
            .iter()
            .map(|val| *get_enum_value!(val, PropertyValue::Bool))
            .collect();

        let mut model_resource = resource_manager.get_resource(&model_path, &DTIs::rModel)?;
        let model_file = ModelFile::new(&mut model_resource)?;

        let mut material_resource = resource_manager.get_resource(&model_path, &DTIs::rMaterial)?;

        let material = MaterialFile::new(&mut material_resource, &shader2)?;

        let mut model = Model::new(
            &model_file,
            &material,
            &shader2,
            &resource_manager,
            public.device(),
            public.queue(),
            &transform_bind_group_layout,
            swapchain_format,
        )?;

        model.set_parts_disp(&parts_disp);

        let debug_overlay = DebugOverlay::new(public.device(), swapchain_format);

        Ok(ModelViewerApp {
            model,

            transform_buf,
            transform_bind_group,

            depth_texture: None,
            depth_texture_view: None,
            camera: Camera::new(glam::vec3(0., 0., 1.), 0., 0., 50.),
            debug_overlay,
        })
    }

    fn render(
        &mut self,
        manager: &RendererAppManagerPublic,
        frame_view: &wgpu::TextureView,
        encoder: &mut wgpu::CommandEncoder,
    ) -> anyhow::Result<()> {
        // FIXME: this should probably be handled by manager
        self.update_depth_texture(
            manager.device(),
            manager.config().width,
            manager.config().height,
        );
        let depth_view = self
            .depth_texture_view
            .as_ref()
            .expect("should never be None here");

        let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: None,
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: frame_view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::WHITE),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: depth_view,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(1.0),
                    store: wgpu::StoreOp::Store,
                }),
                stencil_ops: None,
            }),
            timestamp_writes: None,
            occlusion_query_set: None,
        });

        self.camera.update(
            manager.input(),
            manager.config().width as f32 / manager.config().height as f32,
        );

        let transform_mat = self.camera.view_proj();

        manager
            .queue()
            .write_buffer(&self.transform_buf, 0, transform_mat.as_ref().as_bytes());

        self.model.render(
            &mut rpass,
            manager.queue(),
            &self.transform_bind_group,
            &mut self.debug_overlay,
        );

        self.debug_overlay
            .render(&mut rpass, manager.queue(), &self.camera);

        Ok(())
    }

    fn post_render(&mut self) -> anyhow::Result<()> {
        self.debug_overlay.clear();

        Ok(())
    }
}

pub fn main() -> anyhow::Result<()> {
    RendererAppManager::<ModelViewerApp>::run()
}
