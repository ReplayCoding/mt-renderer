// Used by read_struct in util/read_struct.rs
#![allow(incomplete_features)]
#![feature(generic_const_exprs)]

pub mod dti;
pub use dti::generated as DTIs;
pub use dti::DTI;

pub mod input_state;
pub mod renderer_app_manager;
pub mod resource_manager;

pub mod mtserializer;
pub mod rarchive;
pub mod rguimessage;
pub mod rmaterial;
pub mod rmodel;
pub mod rshader2;
pub mod rshaderpackage;
pub mod rtexture;
pub mod rscheduler;

pub mod model;
pub mod texture;

pub mod util;

pub mod camera;
pub mod debug_overlay;
