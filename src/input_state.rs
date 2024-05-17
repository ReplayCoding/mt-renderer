// TODO: rename to input_state/InputState

use bitflags::bitflags;
use log::trace;

bitflags! {
    #[derive(Debug, Copy, Clone)]
    pub struct KeyState: u32 {
        const W = 1 << 0;
        const A = 1 << 1;
        const S = 1 << 2;
        const D = 1 << 3;
    }
}

pub struct InputState {
    frame_mouse_delta: glam::Vec2,

    current_key_state: KeyState,
}

impl InputState {
    pub fn new() -> Self {
        Self {
            frame_mouse_delta: glam::vec2(0., 0.),
            current_key_state: KeyState::empty(),
        }
    }

    pub fn next_frame(&mut self) {
        self.frame_mouse_delta = glam::vec2(0., 0.); // frame is over, reset mouse delta
    }

    pub fn add_mouse_movement(&mut self, event_delta: glam::Vec2) {
        self.frame_mouse_delta += event_delta;
    }

    pub fn frame_mouse_delta(&self) -> glam::Vec2 {
        self.frame_mouse_delta
    }

    pub fn set_key(&mut self, key: KeyState) {
        self.current_key_state |= key;
    }

    pub fn unset_key(&mut self, key: KeyState) {
        self.current_key_state -= key;
    }

    pub fn has_key(&self, key: KeyState) -> bool {
        self.current_key_state.contains(key)
    }
}

#[test]
fn test_mouse() {
    let mut input = InputState::new();

    input.add_mouse_movement(glam::vec2(1., 1.));
    assert_eq!(input.frame_mouse_delta(), glam::vec2(1., 1.));

    input.add_mouse_movement(glam::vec2(0., -0.5));
    assert_eq!(input.frame_mouse_delta(), glam::vec2(1., 0.5));

    input.next_frame();
    assert_eq!(input.frame_mouse_delta(), glam::vec2(0., 0.));
}

#[test]
fn test_key() {
    let mut input = InputState::new();

    input.set_key(KeyState::W);
    assert!(input.has_key(KeyState::W));

    input.set_key(KeyState::A);
    assert!(input.has_key(KeyState::W) && input.has_key(KeyState::A));
    assert!(!input.has_key(KeyState::D));

    input.unset_key(KeyState::A);
    assert!(input.has_key(KeyState::W));
}
