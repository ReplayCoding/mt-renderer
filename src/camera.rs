use glam::{Mat4, Vec3};

use crate::input_state::InputState;

#[derive(Debug)]
pub struct Camera {
    position: Vec3,

    yaw: f32,
    pitch: f32,

    fov: f32,
}

impl Camera {
    // TODO: make this configurable
    const SENSITIVITY: f32 = 0.1;

    pub fn new(position: Vec3, yaw: f32, pitch: f32, fov: f32) -> Self {
        Self {
            position,
            yaw,
            pitch,
            fov,
        }
    }

    fn view(&self) -> Mat4 {
        let translation = glam::Mat4::from_translation(self.position);
        #[rustfmt::skip]
        let rotation =
            glam::Mat4::from_axis_angle(glam::vec3(1.,  0., 0.), self.pitch.to_radians()) *
            glam::Mat4::from_axis_angle(glam::vec3(0., 1., 0.), self.yaw.to_radians());

        (translation * rotation).inverse()
    }

    fn proj(&self, aspect: f32) -> Mat4 {
        // TODO: confirm that this is sane!
        glam::Mat4::perspective_rh(self.fov.to_radians(), aspect, 0.01, 50.0)
    }

    pub fn view_proj(&self, aspect: f32) -> Mat4 {
        self.proj(aspect) * self.view()
    }

    pub fn update(&mut self, input: &InputState) {
        let frame_mouse_delta = input.frame_mouse_delta();

        self.yaw -= Self::SENSITIVITY * frame_mouse_delta.x;
        self.pitch -= Self::SENSITIVITY * frame_mouse_delta.y;

        self.yaw %= 360.0;
        self.pitch = self.pitch.clamp(-89.0, 89.0);
    }
}
