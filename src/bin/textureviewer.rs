use mt_renderer::{
    renderer_app_manager::{RendererApp, RendererAppManager, RendererAppManagerPublic},
    rtexture::TextureFile,
    texture::Texture,
};
use std::{borrow::Cow, mem::size_of};
use wgpu::util::DeviceExt;
use zerocopy::AsBytes;

struct TextureViewerApp {
    texture: Texture,
    pipeline: wgpu::RenderPipeline,
    vertex_buffer: wgpu::Buffer,

    image_size: glam::Vec2,

    scale_buf: wgpu::Buffer,
    scale_bg: wgpu::BindGroup,
}

fn compute_scale(image_size: glam::Vec2, window_size: glam::Vec2) -> glam::Vec2 {
    dbg!(image_size);
    dbg!(window_size);

    if image_size.max_element() > window_size.max_element() {
        glam::Vec2::splat(1.) // TODO
    }
    else {
        glam::vec2(image_size.x / window_size.x, image_size.y / window_size.y)
    }
}

impl RendererApp for TextureViewerApp {
    fn setup(
        manager: &RendererAppManagerPublic,
        swapchain_format: wgpu::TextureFormat,
    ) -> anyhow::Result<Self> {
        let args: Vec<_> = std::env::args().collect();

        let mut f = std::fs::File::open(&args[1]).unwrap();
        let texture_resource = TextureFile::new(&mut f).unwrap();

        let image_size = glam::vec2(
            texture_resource.width() as f32,
            texture_resource.height() as f32,
        );

        let texture = Texture::new(manager.device(), manager.queue(), texture_resource);

        #[rustfmt::skip]
        let vertex_buf_data: [f32; 6 * 2] = [
            -1., -1.,
            -1.,  1.,
             1.,  1.,
             1., -1.,
             1.,  1.,
            -1., -1.,
        ];

        let vertex_buffer =
            manager
                .device()
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("vertex buffer"),
                    contents: vertex_buf_data.as_bytes(),
                    usage: wgpu::BufferUsages::VERTEX,
                });

        let shader = manager
            .device()
            .create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("shader"),
                source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(include_str!(
                    "../shaders/textureviewer.wgsl"
                ))),
            });

        let scale_buf = manager.device().create_buffer(&wgpu::BufferDescriptor {
            label: Some("texture scale buffer"),
            size: size_of::<f32>() as u64 * 2,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let scale_bgl =
            manager
                .device()
                .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                    label: Some("texture scale bind group layout"),
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

        let scale_bg = manager
            .device()
            .create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("texture scale bind group"),
                layout: &scale_bgl,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                        buffer: &scale_buf,
                        offset: 0,
                        size: None,
                    }),
                }],
            });

        let pipeline_layout =
            manager
                .device()
                .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                    label: Some("render pipeline layout"),
                    bind_group_layouts: &[texture.bind_group_layout(), &scale_bgl],
                    push_constant_ranges: &[],
                });

        let pipeline = manager
            .device()
            .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("render pipeline"),
                layout: Some(&pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: "vs_main",
                    buffers: &[wgpu::VertexBufferLayout {
                        array_stride: (size_of::<f32>() * 2) as u64,
                        step_mode: wgpu::VertexStepMode::Vertex,
                        attributes: &[wgpu::VertexAttribute {
                            format: wgpu::VertexFormat::Float32x2,
                            offset: 0,
                            shader_location: 0,
                        }],
                    }],
                },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: "fs_main",
                    targets: &[Some(wgpu::ColorTargetState {
                        format: swapchain_format,
                        blend: None,
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                }),
                primitive: wgpu::PrimitiveState {
                    topology: wgpu::PrimitiveTopology::TriangleList,
                    strip_index_format: None,
                    ..Default::default()
                },
                depth_stencil: None,
                multisample: wgpu::MultisampleState::default(),
                multiview: None,
            });

        Ok(TextureViewerApp {
            texture,
            pipeline,
            vertex_buffer,
            scale_buf,
            scale_bg,
            image_size,
        })
    }

    fn render(
        &mut self,
        manager: &RendererAppManagerPublic,
        frame_view: &wgpu::TextureView,
        encoder: &mut wgpu::CommandEncoder,
    ) -> anyhow::Result<()> {
        let scale = compute_scale(
            self.image_size,
            glam::vec2(
                manager.config().width as f32,
                manager.config().height as f32,
            ),
        );

        manager
            .queue()
            .write_buffer(&self.scale_buf, 0, scale.as_ref().as_bytes());

        let mut rpass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("main render pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: frame_view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color {
                        r: 0.2,
                        g: 0.3,
                        b: 0.4,
                        a: 1.0,
                    }),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });

        rpass.set_pipeline(&self.pipeline);
        rpass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
        rpass.set_bind_group(0, self.texture.bind_group(), &[]);
        rpass.set_bind_group(1, &self.scale_bg, &[]);

        rpass.draw(0..6, 0..1);

        Ok(())
    }
}

pub fn main() -> anyhow::Result<()> {
    RendererAppManager::<TextureViewerApp>::run()
}
