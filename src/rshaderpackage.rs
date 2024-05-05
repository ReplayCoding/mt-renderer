use std::{
    io::{Read, Seek},
    mem::size_of,
};

use anyhow::anyhow;
use log::{debug, trace};
use zerocopy::{FromBytes, FromZeroes};

use crate::{
    rshader2::{Shader2File, Shader2Object},
    util,
};

#[repr(C, packed)]
#[derive(Debug, FromBytes, FromZeroes)]
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

// Incomplete?
#[repr(C, packed)]
#[derive(Debug, FromBytes, FromZeroes)]
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
#[derive(Debug, FromBytes, FromZeroes)]
struct RawShaderPackageShader {
    padding_0: [u8; 8],

    field_8: u64,

    padding_10: [u8; 0x18],

    field_28: u64,
    field_30: u64,
    field_38: u64,
    field_40: u64,
    field_48: u64,
    field_50: u64,
    field_58: u64,
    field_60: u64,

    padding_68: [u8; 8],
}

#[repr(C, packed)]
#[derive(Debug, FromBytes, FromZeroes)]
struct RawShaderPackageShaderInput {
    layouts: [u32; 4], // SO_HANDLE[4]
    crc: u32,
    padding1: u32,
    playout: u64,
}

#[repr(C, packed)]
#[derive(Debug, FromBytes, FromZeroes)]
struct RawShaderPackageShaderCodeInfo {
    bitfield_0x0: u32,
    crc: u32,
    pcode: u64, // void*
}

struct ShaderPackageCodeInfo {
    _code: Vec<u8>,
    _crc: u32,
}

#[derive(Debug)]
struct ShaderPackageShaderInput {
    _layouts: [Option<Shader2Object>; 4], // SO_HANDLE[4]
    _crc: u32,
}

#[derive(Debug)]
pub struct ShaderPackageFile {
    _inputs: Vec<ShaderPackageShaderInput>,
}

impl ShaderPackageFile {
    pub fn new<R: Read + Seek>(reader: &mut R, shader2: &Shader2File) -> anyhow::Result<Self> {
        let header: ShaderPackageHeader = util::read_struct(reader)?;

        // CORE read?
        let mut core_bytes = vec![0u8; (header.body_offset - 0x30) as usize];
        reader.read_exact(&mut core_bytes)?;
        let core = ShaderPackageCore::ref_from(&core_bytes[..size_of::<ShaderPackageCore>()])
            .expect("couldn't read CORE struct");

        // body
        let mut body_bytes = vec![0u8; header.body_length as usize];
        reader.read_exact(&mut body_bytes)?;

        debug!("header {:#08x?}", header);
        debug!("core? {:#08x?}", core);

        let get_shaders = |num_shaders: u16, shaders_offs: u64| -> Vec<ShaderPackageCodeInfo> {
            util::read_struct_array::<RawShaderPackageShaderCodeInfo>(
                &core_bytes[shaders_offs as usize..],
                num_shaders.into(),
            )
            .expect("couldn't read shader info list")
            .map(|info| {
                let info = info.expect("couldn't read shader info");

                let code_size = (info.bitfield_0x0 >> 10) as usize;
                let code_offs = info.pcode as usize;

                let code_bytes = &body_bytes[code_offs..code_offs + code_size];

                trace!("shader info size {} offs {:08x}", code_size, { info.pcode });

                ShaderPackageCodeInfo {
                    _code: code_bytes.to_vec(),
                    _crc: info.crc,
                }
            })
            .collect()
        };

        let inputs = util::read_struct_array::<RawShaderPackageShaderInput>(
            &core_bytes[core.ia_list as usize..],
            header.num_inputlayouts.into(),
        )
        .expect("couldn't read ia list")
        .map(|ia| {
            let ia = ia.expect("couldn't read shader input");

            let layouts: [_; 4] = ia
                .layouts
                .map(|layout| shader2.get_object_by_handle(layout).cloned());

            ShaderPackageShaderInput {
                _layouts: layouts,
                _crc: ia.crc,
            }
        })
        .collect();

        let _vertex_shaders = get_shaders(header.num_vertexshaders, core.vs_list);
        let _pixel_shaders = get_shaders(header.num_pixelshaders, core.ps_list);
        let _geometry_shaders = get_shaders(header.num_geometryshaders, core.gs_list);
        let _hull_shaders = get_shaders(header.num_hullshaders, core.hs_list);
        let _domain_shaders = get_shaders(header.num_domainshaders, core.ds_list);
        let _compute_shaders = get_shaders(header.num_computeshaders, core.cs_list);

        // VLA at end of CORE struct, which is why we add sizeof(CORE)
        for shader_info in util::read_struct_array::<RawShaderPackageShader>(
            &core_bytes[size_of::<ShaderPackageCore>()..],
            header.num_shaders.into(),
        )? {
            let _shader_info = shader_info.ok_or_else(|| anyhow!("couldn't read shader info"))?;
            debug!("{:#?}", shader_info);
        }

        Ok(Self { _inputs: inputs })
    }
}

#[test]
fn test_struct_sizes() {
    assert_eq!(0x30, size_of::<ShaderPackageHeader>());
    assert_eq!(1 << 5, size_of::<RawShaderPackageShaderInput>());
    assert_eq!(1 << 4, size_of::<RawShaderPackageShaderCodeInfo>());
    assert_eq!(0x70, size_of::<RawShaderPackageShader>());
}
