mod rmodel;
use glam::Mat4;
use rmodel::Model;
use std::{borrow::Cow, sync::Arc, time::Instant};
use wgpu::util::DeviceExt;
use winit::{
    event::{Event, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    window::Window,
};

struct App {
    window: Arc<Window>,

    time_start: Instant,

    config: wgpu::SurfaceConfiguration,
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,

    model: Model,

    transform_buf: wgpu::Buffer,
    transform_bind_group: wgpu::BindGroup,

    depth_texture: Option<wgpu::Texture>,
    depth_texture_view: Option<wgpu::TextureView>,
}

impl App {
    async fn new(window: Arc<Window>) -> App {
        let mut size = window.inner_size();
        size.width = size.width.max(1);
        size.height = size.height.max(1);

        let instance = wgpu::Instance::default();

        let surface = instance.create_surface(window.clone()).unwrap();
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::default(),
                force_fallback_adapter: false,
                // Request an adapter which can render to our surface
                compatible_surface: Some(&surface),
            })
            .await
            .expect("Failed to find an appropriate adapter");

        // Create the logical device and command queue
        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: None,
                    required_features: wgpu::Features::empty(),
                    // Make sure we use the texture resolution limits from the adapter, so we can support images the size of the swapchain.
                    required_limits: wgpu::Limits::downlevel_webgl2_defaults()
                        .using_resolution(adapter.limits()),
                },
                None,
            )
            .await
            .expect("Failed to create device");

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

        let swapchain_capabilities = surface.get_capabilities(&adapter);
        let swapchain_format = swapchain_capabilities.formats[0];

        let mut fil = std::fs::File::open("/home/user/Desktop/WIN11-vm-folder/scripts/out/place0100/GO/place/p0100/p0100.rModel").unwrap();
        let model = Model::new(&mut fil, &device, swapchain_format).unwrap();

        let config = surface
            .get_default_config(&adapter, size.width, size.height)
            .unwrap();
        surface.configure(&device, &config);

        let time_start = Instant::now();

        App {
            window,

            time_start,
            config,
            surface,
            device,
            queue,

            model,

            transform_buf,
            transform_bind_group,

            depth_texture: None,
            depth_texture_view: None,
        }
    }

    fn resize(&mut self, new_size: &winit::dpi::PhysicalSize<u32>) {
        self.config.width = new_size.width.max(1);
        self.config.height = new_size.height.max(1);
        self.surface.configure(&self.device, &self.config);

        // On macos the window needs to be redrawn manually after resizing
        self.window.request_redraw();
    }

    fn update_depth_texture(&mut self) {
        if let Some(depth_texture) = &self.depth_texture {
            if depth_texture.width() != self.config.width
                || depth_texture.height() != self.config.height
            {
                self.depth_texture = None;
                self.depth_texture_view = None;
            }
        }

        if self.depth_texture.is_none() {
            let depth_texture = self.device.create_texture(&wgpu::TextureDescriptor {
                label: Some("depth texture"),
                size: wgpu::Extent3d {
                    width: self.config.width,
                    height: self.config.height,
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

    fn render(&mut self) {
        let frame = self
            .surface
            .get_current_texture()
            .expect("Failed to acquire next swap chain texture");
        let view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });

        let current_time = self.time_start.elapsed().as_secs_f32();
        let transform_mat = compute_mat(
            current_time,
            self.config.width as f32 / self.config.height as f32,
        );

        self.update_depth_texture();

        self.queue.write_buffer(
            &self.transform_buf,
            0,
            bytemuck::cast_slice(transform_mat.as_ref()),
        );

        {
            self.model.render(
                &view,
                &self.depth_texture_view.as_ref().unwrap(),
                &mut encoder,
            );
            // rpass.set_pipeline(&self.render_pipeline);
            // rpass.set_bind_group(0, &self.transform_bind_group, &[]);
            // rpass.set_vertex_buffer(0, self.vertex_buf.slice(..));
            // // rpass.set_index_buffer(self.index_buf.slice(..), wgpu::IndexFormat::Uint16);
            // rpass.draw(0..36, 0..1);
        }

        self.queue.submit(Some(encoder.finish()));
        frame.present();

        self.window.request_redraw();
    }
}

fn compute_mat(deg: f32, aspect: f32) -> Mat4 {
    let model = glam::Mat4::from_rotation_x(deg) * glam::Mat4::from_rotation_y(deg);
    let view = glam::Mat4::from_translation(glam::Vec3::new(0., 0., -3.));
    let proj = glam::Mat4::perspective_rh(70.0_f32.to_radians(), aspect, 0.1, 100.0);

    proj * view * model
}

pub fn main() -> anyhow::Result<()> {
    let event_loop = EventLoop::new()?;

    event_loop.set_control_flow(ControlFlow::Poll);
    #[allow(unused_mut)]
    let mut builder = winit::window::WindowBuilder::new();
    let window = Arc::new(builder.build(&event_loop)?);

    env_logger::init();

    let mut app = pollster::block_on(App::new(window));

    event_loop.run(move |event, target| {
        if let Event::WindowEvent {
            window_id: _,
            event,
        } = event
        {
            match event {
                WindowEvent::Resized(new_size) => {
                    app.resize(&new_size);
                }
                WindowEvent::RedrawRequested => {
                    app.render();
                }
                WindowEvent::CloseRequested => target.exit(),
                _ => {}
            };
        }
    })?;

    Ok(())
}
