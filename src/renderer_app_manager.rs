use log::trace;
use std::sync::Arc;
use winit::{
    event::{Event, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    window::Window,
};

pub trait RendererApp {
    fn setup(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        swapchain_format: wgpu::TextureFormat,
    ) -> anyhow::Result<Self>
    where
        Self: Sized;

    fn render(
        &mut self,
        manager: &RendererAppManagerInternal,
        frame_view: &wgpu::TextureView,
        encoder: &mut wgpu::CommandEncoder,
    ) -> anyhow::Result<()>;
}

/// Not the best name... contains render manager fields state that is accessible to apps
pub struct RendererAppManagerInternal {
    window: Arc<Window>,

    config: wgpu::SurfaceConfiguration,
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
}

impl RendererAppManagerInternal {
    pub fn config(&self) -> &wgpu::SurfaceConfiguration {
        &self.config
    }

    pub fn queue(&self) -> &wgpu::Queue {
        &self.queue
    }

    pub fn device(&self) -> &wgpu::Device {
        &self.device
    }
}

pub struct RendererAppManager<A: RendererApp> {
    internal: RendererAppManagerInternal,
    app: A,
}

impl<A> RendererAppManager<A>
where
    A: RendererApp,
{
    async fn create(window: Arc<Window>) -> anyhow::Result<Self> {
        let mut size = window.inner_size();
        size.width = size.width.max(1);
        size.height = size.height.max(1);

        let instance = wgpu::Instance::default();

        let surface = instance.create_surface(window.clone())?;
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
                    required_features: wgpu::Features::TEXTURE_COMPRESSION_BC,
                    // Make sure we use the texture resolution limits from the adapter, so we can support images the size of the swapchain.
                    required_limits: wgpu::Limits::downlevel_defaults()
                        .using_resolution(adapter.limits()),
                },
                None,
            )
            .await
            .expect("Failed to create device");

        let swapchain_capabilities = surface.get_capabilities(&adapter);
        let swapchain_format = *swapchain_capabilities
            .formats
            .iter()
            .find(|f| !f.is_srgb())
            .expect("couldn't get a non-srgb swapchain");

        let mut config = surface
            .get_default_config(&adapter, size.width, size.height)
            .unwrap();
        config.format = swapchain_format;
        surface.configure(&device, &config);

        let app = A::setup(&device, &queue, swapchain_format)?;

        Ok(RendererAppManager {
            internal: RendererAppManagerInternal {
                window,

                config,
                surface,
                device,
                queue,
            },

            app,
        })
    }

    fn resize(&mut self, new_size: &winit::dpi::PhysicalSize<u32>) {
        self.internal.config.width = new_size.width.max(1);
        self.internal.config.height = new_size.height.max(1);
        trace!("resize window: {:?}", new_size);
        self.internal
            .surface
            .configure(&self.internal.device, &self.internal.config);

        // On macos the window needs to be redrawn manually after resizing
        self.internal.window.request_redraw();
    }

    fn render(&mut self) -> anyhow::Result<()> {
        let frame = self
            .internal
            .surface
            .get_current_texture()
            .expect("Failed to acquire next swap chain texture");
        let frame_view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder = self
            .internal
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });

        self.app.render(&self.internal, &frame_view, &mut encoder)?;

        self.internal.queue.submit(Some(encoder.finish()));
        frame.present();

        self.internal.window.request_redraw();

        Ok(())
    }

    pub fn run() -> anyhow::Result<()> {
        let event_loop = EventLoop::new()?;

        event_loop.set_control_flow(ControlFlow::Poll);
        #[allow(unused_mut)]
        let mut builder = winit::window::WindowBuilder::new();
        let window = Arc::new(builder.build(&event_loop)?);

        env_logger::init();

        let mut manager = pollster::block_on(Self::create(window.clone()))?;

        event_loop.run(move |event, target| {
            if let Event::WindowEvent {
                window_id: _,
                event,
            } = event
            {
                match event {
                    WindowEvent::Resized(new_size) => {
                        manager.resize(&new_size);
                    }
                    WindowEvent::RedrawRequested => {
                        manager.render().unwrap();
                    }
                    WindowEvent::CloseRequested => target.exit(),
                    _ => {}
                };
            }
        })?;

        Ok(())
    }
}
