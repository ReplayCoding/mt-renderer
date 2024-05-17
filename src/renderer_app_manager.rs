use log::trace;
use std::{
    sync::Arc,
    time::{Duration, Instant},
};
use winit::{
    event::{DeviceEvent, Event, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    window::Window,
};

use crate::input_state::{InputState, KeyState};

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
        manager: &RendererAppManagerPublic,
        frame_view: &wgpu::TextureView,
        encoder: &mut wgpu::CommandEncoder,
    ) -> anyhow::Result<()>;
}

pub struct RendererAppManagerPublic {
    window: Arc<Window>,

    config: wgpu::SurfaceConfiguration,
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,

    input: InputState,

    frame_time: Duration,
}

impl RendererAppManagerPublic {
    pub fn config(&self) -> &wgpu::SurfaceConfiguration {
        &self.config
    }

    pub fn queue(&self) -> &wgpu::Queue {
        &self.queue
    }

    pub fn device(&self) -> &wgpu::Device {
        &self.device
    }

    pub fn input(&self) -> &InputState {
        &self.input
    }

    pub fn frame_time(&self) -> Duration {
        self.frame_time
    }
}

pub struct RendererAppManager<A: RendererApp> {
    public: RendererAppManagerPublic,
    app: A,

    last_frame: Instant,
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
            public: RendererAppManagerPublic {
                window,

                config,
                surface,
                device,
                queue,
                input: InputState::new(),
                frame_time: Duration::ZERO,
            },

            app,
            last_frame: Instant::now(),
        })
    }

    fn resize(&mut self, new_size: &winit::dpi::PhysicalSize<u32>) {
        self.public.config.width = new_size.width.max(1);
        self.public.config.height = new_size.height.max(1);
        trace!("resize window: {:?}", new_size);
        self.public
            .surface
            .configure(&self.public.device, &self.public.config);

        // On macos the window needs to be redrawn manually after resizing
        self.public.window.request_redraw();
    }

    fn render(&mut self) -> anyhow::Result<()> {
        let this_frame = Instant::now();
        self.public.frame_time = this_frame.duration_since(self.last_frame);
        self.last_frame = this_frame;

        let frame = self
            .public
            .surface
            .get_current_texture()
            .expect("Failed to acquire next swap chain texture");
        let frame_view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder = self
            .public
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });

        self.app.render(&self.public, &frame_view, &mut encoder)?;

        self.public.input.next_frame();

        self.public.queue.submit(Some(encoder.finish()));
        frame.present();

        self.public.window.request_redraw();

        Ok(())
    }

    fn on_mouse_moved(&mut self, x: f64, y: f64) {
        let event_delta = glam::vec2(x as f32, y as f32);

        self.public.input.add_mouse_movement(event_delta);
    }

    pub fn run() -> anyhow::Result<()> {
        let event_loop = EventLoop::new()?;

        event_loop.set_control_flow(ControlFlow::Poll);
        #[allow(unused_mut)]
        let mut builder = winit::window::WindowBuilder::new();
        let window = Arc::new(builder.build(&event_loop)?);

        env_logger::init();

        let mut manager = pollster::block_on(Self::create(window.clone()))?;

        window.set_cursor_grab(winit::window::CursorGrabMode::Confined)?;
        window.set_cursor_visible(false);

        event_loop.run(move |event, target| match event {
            Event::WindowEvent {
                window_id: _,
                event,
            } => {
                match event {
                    WindowEvent::Resized(new_size) => {
                        manager.resize(&new_size);
                    }
                    WindowEvent::RedrawRequested => {
                        manager.render().unwrap();
                    }
                    WindowEvent::CloseRequested => target.exit(),

                    WindowEvent::KeyboardInput {
                        device_id: _,
                        event,
                        is_synthetic: _,
                    } => {
                        let translated_key =
                            if let winit::keyboard::PhysicalKey::Code(key) = event.physical_key {
                                match key {
                                    winit::keyboard::KeyCode::KeyW => KeyState::W,
                                    winit::keyboard::KeyCode::KeyA => KeyState::A,
                                    winit::keyboard::KeyCode::KeyS => KeyState::S,
                                    winit::keyboard::KeyCode::KeyD => KeyState::D,
                                    _ => KeyState::empty(),
                                }
                            } else {
                                KeyState::empty()
                            };

                        match event.state {
                            winit::event::ElementState::Pressed => {
                                manager.public.input.set_key(translated_key)
                            }
                            winit::event::ElementState::Released => {
                                manager.public.input.unset_key(translated_key)
                            }
                        };
                    }
                    _ => {}
                };
            }
            Event::DeviceEvent {
                device_id: _,
                event,
            } => match event {
                DeviceEvent::MouseMotion { delta } => {
                    manager.on_mouse_moved(delta.0, delta.1);
                }
                _ => {}
            },
            _ => (),
        })?;

        Ok(())
    }
}
