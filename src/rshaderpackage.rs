use std::{
    io::{Read, Seek},
    mem::size_of,
};

use log::{debug, trace};

use crate::{
    rshader2::{Shader2File, Shader2Object},
    util,
};

#[repr(C, packed)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct ShaderPackageHeader {
    magic: u32,
    shader_version: u32,
    version: u16,
    num_shaders: u16,
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

// Incomplete
#[repr(C, packed)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct ShaderPackageCore {
    field_0_ptr: u64,
    field_8_ptr: u64,
    ia_list: u64,
    vs_list: u64,
    ps_list: u64,
    gs_list: u64,
    hs_list: u64,
    ds_list: u64,
    cs_list: u64,
    tables: u64,
    ptable: [u64; 0x1000],
    // SHADERS vla begins here
}

#[repr(C, packed)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct RawShaderPackageShader {
    // stupid
    pad1: [u8; 0x20],
    pad2: [u8; 0x20],
    pad3: [u8; 0x20],
    pad4: [u8; 0x10],
}

#[repr(C, packed)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct RawShaderPackageShaderInput {
    layouts: [u32; 4], // SO_HANDLE[4]
    crc: u32,
    padding1: u32,
    playout: u64,
}

#[repr(C, packed)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct RawShaderPackageShaderCodeInfo {
    bitfield_0x0: u32,
    crc: u32,
    pcode: u64, // void*
}

#[derive(Debug)]
struct ShaderPackageShaderInput {
    layouts: [Option<Shader2Object>; 4], // SO_HANDLE[4]
    crc: u32,
}

#[derive(Debug)]
pub struct ShaderPackageFile {
    inputs: Vec<ShaderPackageShaderInput>,
}

impl ShaderPackageFile {
    pub fn new<R: Read + Seek>(reader: &mut R, shader2: &Shader2File) -> anyhow::Result<Self> {
        let header: ShaderPackageHeader = util::read_struct(reader)?;

        // CORE read?
        let mut core_bytes = vec![0u8; (header.body_offset - 0x30) as usize];
        reader.read_exact(&mut core_bytes)?;
        let core: &ShaderPackageCore =
            bytemuck::from_bytes(&core_bytes[..size_of::<ShaderPackageCore>()]);

        // body
        let mut body_bytes = vec![0u8; header.body_length as usize];
        reader.read_exact(&mut body_bytes)?;

        debug!("header {:#08x?}", header);
        debug!("core? {:#08x?}", core);

        let get_shaders = |num_shaders: u16, shaders_offs: u64, dump_prefix: &str| {
            (0..num_shaders).for_each(|idx| {
                let info_offs = shaders_offs as usize
                    + (idx as usize * size_of::<RawShaderPackageShaderCodeInfo>());
                let info_bytes =
                    &core_bytes[info_offs..info_offs + size_of::<RawShaderPackageShaderCodeInfo>()];
                let info: &RawShaderPackageShaderCodeInfo = bytemuck::from_bytes(info_bytes);

                let code_size = (info.bitfield_0x0 >> 10) as usize;
                let code_offs = info.pcode as usize;

                let code_bytes = &body_bytes[code_offs..code_offs + code_size];
                // std::fs::write(format!("shaders/{dump_prefix}_{idx}"), code_bytes).unwrap();

                trace!(
                    "shader info size {} offs {:08x}",
                    code_size,
                    (info.pcode as u64)
                );
            })
        };

        let inputs = (0..header.num_inputlayouts)
            .map(|idx| {
                let ia_offs = core.ia_list as usize
                    + (idx as usize * size_of::<RawShaderPackageShaderInput>());
                let ia_bytes =
                    &core_bytes[ia_offs..ia_offs + size_of::<RawShaderPackageShaderInput>()];
                let ia: &RawShaderPackageShaderInput = bytemuck::from_bytes(ia_bytes);

                let layouts: [_; 4] = ia
                    .layouts
                    .map(|layout| shader2.get_object_by_handle(layout).cloned());

                ShaderPackageShaderInput {
                    layouts,
                    crc: ia.crc,
                }
            })
            .collect();

        let vertex_shaders = get_shaders(header.num_vertexshaders, core.vs_list, "vs");
        let pixel_shaders = get_shaders(header.num_pixelshaders, core.ps_list, "ps");
        let geometry_shaders = get_shaders(header.num_geometryshaders, core.gs_list, "gs");
        let hull_shaders = get_shaders(header.num_hullshaders, core.hs_list, "hs");
        let domain_shaders = get_shaders(header.num_domainshaders, core.ds_list, "ds");
        let compute_shaders = get_shaders(header.num_computeshaders, core.cs_list, "ds");

        for shader_idx in 0..header.num_shaders {
            let shader_bytes_offs = size_of::<ShaderPackageCore>()
                + (shader_idx as usize * size_of::<RawShaderPackageShader>()); // VLA at end of CORE struct
            let shader_bytes = &core_bytes
                [shader_bytes_offs..shader_bytes_offs + size_of::<RawShaderPackageShader>()];
            let shader_info: &RawShaderPackageShader = bytemuck::from_bytes(shader_bytes);
            println!("{:#?}", shader_info);
        }

        Ok(Self { inputs })
    }
}

#[test]
fn test_struct_sizes() {
    assert_eq!(0x30, size_of::<ShaderPackageHeader>());
    assert_eq!(1 << 5, size_of::<RawShaderPackageShaderInput>());
    assert_eq!(1 << 4, size_of::<RawShaderPackageShaderCodeInfo>());
    assert_eq!(0x70, size_of::<RawShaderPackageShader>());
}
