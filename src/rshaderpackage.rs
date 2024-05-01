use std::{
    io::{Read, Seek},
    mem::size_of,
};

use log::debug;

use crate::{rshader2::Shader2File, util};

#[repr(C, packed)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct ShaderPackageHeader {
    magic: u32,
    shader_version: u32,
    version: u16,
    field_a: u16,
    num_vertexshaders: u16,
    num_pixelshaders: u16,
    num_geometryshaders: u16,
    num_hullshaders: u16,
    num_domainshaders: u16,
    num_computeshaders: u16,
    num_inputlayouts: u16,
    // -------- UNKNOWN --------
    field_1a: u16,
    field_1c: u32,
    field_20: u32,
    // -------- UNKNOWN END --------
    body_length: u32,
    body_offset: u64,
}

#[derive(Debug)]
pub struct ShaderPackageFile {}

impl ShaderPackageFile {
    pub fn new<R: Read + Seek>(reader: &mut R, shader2: &Shader2File) -> anyhow::Result<Self> {
        let header: ShaderPackageHeader = util::read_struct(reader)?;

        // CORE read?
        let mut core_bytes = vec![0u8; (header.body_offset - 0x30) as usize];
        reader.read_exact(&mut core_bytes)?;

        debug!("header {:#08x?}", header);

        Ok(Self {})
    }
}

#[test]
fn test_struct_sizes() {
    assert_eq!(0x30, size_of::<ShaderPackageHeader>());
}
