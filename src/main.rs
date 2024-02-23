use winit::{event::*, event_loop::EventLoop, window::WindowBuilder};

fn main() {
    env_logger::init();

    let event_loop = EventLoop::new().unwrap();
    event_loop.set_control_flow(winit::event_loop::ControlFlow::Poll);
    let window = WindowBuilder::new().build(&event_loop).unwrap();

    event_loop
        .run(move |event, elwt| {
            match event {
                Event::WindowEvent { window_id, event } => {
                    if window_id == window.id() {
                        match event {
                            WindowEvent::CloseRequested => elwt.exit(),
                            _ => (),
                        }
                    }
                }
                _ => (),
            };
        })
        .unwrap();
}
