// Used by read_struct in util/read_struct.rs
#![feature(generic_const_exprs)]

pub mod crc;
pub use crc::crc32;

pub mod dti;
pub use dti::generated as DTIs;
pub use dti::DTI;

pub mod renderer_app_manager;
pub mod resource_manager;

pub mod mtserializer;
pub mod rarchive;
pub mod rmaterial;
pub mod rmodel;
pub mod rshader2;
pub mod rtexture;
pub mod rshaderpackage;

pub mod model;
pub mod texture;

mod util;
