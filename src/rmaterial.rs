use std::{
    ffi::CStr,
    io::{Read, Seek},
    mem::size_of,
};

use log::{debug, warn};
use zerocopy::{FromBytes, FromZeroes};

use crate::{rshader2::Shader2File, util, DTI};

#[repr(C, packed)]
#[derive(FromBytes, FromZeroes, Debug)]
struct MaterialHeader {
    magic: u32,
    version: u32,
    material_num: u32,
    texture_num: u32,
    shader_version: u32,
    _padding1: u32,

    textures: u64,
    materials: u64,
}

#[repr(C, packed)]
#[derive(FromBytes, FromZeroes, Debug)]
struct RawTextureInfo {
    dti_hash: u32,
    _padding: u32,

    _ptex: u64,
    _plut: u64,

    // TODO: make this a union with the LUT info
    path: [u8; 128],
}

impl RawTextureInfo {
    fn path(&self) -> &str {
        CStr::from_bytes_until_nul(&self.path)
            .expect("failed to decode texture info path into CStr")
            .to_str()
            .expect("failed to convert texture info path into str")
    }

    fn dti(&self) -> Option<&DTI> {
        if self.dti_hash != 0 {
            Some(DTI::from_hash(self.dti_hash).expect("invalid DTI hash in texture info"))
        } else {
            None
        }
    }
}

#[repr(u32)]
#[derive(strum::FromRepr, Debug)]
#[allow(non_camel_case_types)]
enum MaterialStateType {
    STATE_FUNCTION = 0,
    STATE_CBUFFER = 1,
    STATE_SAMPLER = 2,
    STATE_TEXTURE = 3,
    STATE_PROCEDURAL = 4,
}

#[repr(C, packed)]
#[derive(FromBytes, FromZeroes, Debug)]
struct RawMaterialState {
    bitfield_0x0: u32,
    _padding: u32,

    // SHADER_STATE, Maybe make this it's own struct?
    sh_value: u64,
    sh_crc: u32,
    _padding1: u32,
}
impl RawMaterialState {
    fn sh_value(&self) -> u64 {
        self.sh_value
    }
    fn sh_crc(&self) -> u32 {
        self.sh_crc
    }
    fn state_type(&self) -> MaterialStateType {
        MaterialStateType::from_repr(self.bitfield_0x0 & 0xf).expect("invalid state type")
    }
    fn group(&self) -> u32 {
        (self.bitfield_0x0 >> 4) & 0xffff
    }
    fn index(&self) -> u32 {
        (self.bitfield_0x0 >> 20) & 0xfff
    }
}

#[repr(C, packed)]
#[derive(FromBytes, FromZeroes, Debug)]
struct RawMaterialInfo {
    dti_hash: u32,
    _padding: u32,
    name_hash: u32,
    state_bufsize: u32,

    bsstate: u32,
    dsstate: u32,
    rsstate: u32,

    bitfield_0x1c: u32,
    bitfield_0x20: u32,

    blend_factor: [f32; 4],
    animation_bufsize: u32,

    states: u64,         // STATE*
    animation_list: u64, // ANIMATION_LIST*
}
impl RawMaterialInfo {
    fn dti(&self) -> &'static DTI {
        DTI::from_hash(self.dti_hash).unwrap_or_else(|| {
            panic!(
                "{}",
                format!("invalid DTI hash in material info {:08x}", {
                    self.dti_hash
                })
                .leak()
                .to_string()
            )
        })
    }
    fn name_hash(&self) -> u32 {
        self.name_hash
    }
    fn state_bufsize(&self) -> u32 {
        self.state_bufsize
    }
    fn bsstate(&self) -> u32 {
        self.bsstate
    }
    fn dsstate(&self) -> u32 {
        self.dsstate
    }
    fn rsstate(&self) -> u32 {
        self.rsstate
    }

    fn state_num(&self) -> u32 {
        self.bitfield_0x1c & 0xfff
    }
}

#[derive(Debug)]
pub struct MaterialInfo {
    name_hash: u32,
    mat_type: &'static DTI,
    albedo_texture_idx: Option<usize>, // HACK
}

impl MaterialInfo {
    pub fn name_hash(&self) -> u32 {
        self.name_hash
    }

    pub fn mat_type(&self) -> &DTI {
        self.mat_type
    }

    pub fn albedo_texture_idx(&self) -> Option<usize> {
        self.albedo_texture_idx
    }
}

#[derive(Debug)]
pub struct MaterialFile {
    textures: Vec<String>, // TODO: how does DTI affect this? This'll work fine for now i hope
    materials: Vec<MaterialInfo>,
}

impl MaterialFile {
    pub fn new<R: Read + Seek>(reader: &mut R, shader2: &Shader2File) -> anyhow::Result<Self> {
        let header: MaterialHeader = util::read_struct(reader)?;

        debug!("material header: {:#?}", header);

        reader.seek(std::io::SeekFrom::Start(header.textures))?;
        let textures: Vec<_> = (0..header.texture_num)
            .map(|i| {
                let texture_info: RawTextureInfo = util::read_struct(reader)?;

                let texture_path = texture_info.path();
                let texture_dti = texture_info.dti();
                assert_eq!(texture_dti.map(|d| d.name()), Some("rTexture")); // HACK
                debug!(
                    "texture {}: dti {:?} path \"{}\"",
                    i,
                    texture_dti.map(|d| d.name()),
                    texture_path
                );

                Ok(texture_path.to_string())
            })
            .collect::<anyhow::Result<Vec<String>>>()?;

        let materials: Vec<_> = (0..header.material_num).map(|material_idx | {
            reader.seek(std::io::SeekFrom::Start(
                header.materials
                    + (material_idx as u64 * size_of::<RawMaterialInfo>() as u64),
            )).unwrap();

            let material_info: RawMaterialInfo = util::read_struct(reader)?;

            debug!(
                "material {} dti {:?} namehash {:08x} state_bufsize {} state_num {} | bs {:?} ds {:?} rs {:?}",
                material_idx,
                material_info.dti(),
                material_info.name_hash(),
                material_info.state_bufsize(),
                material_info.state_num(),
                shader2
                    .get_object_by_handle(material_info.bsstate())
                    .unwrap()
                    .name(),
                shader2
                    .get_object_by_handle(material_info.dsstate())
                    .unwrap()
                    .name(),
                shader2
                    .get_object_by_handle(material_info.rsstate())
                    .unwrap()
                    .name()
            );
            // debug!("{:#?}", material_info);

            let mut albedo_texture_idx = None;
            for state_idx in 0..material_info.state_num() {
                reader.seek(std::io::SeekFrom::Start(
                    material_info.states
                        + (state_idx as u64 * size_of::<RawMaterialState>() as u64),
                )).unwrap();

                let state: RawMaterialState = util::read_struct(reader)?;

                let state_sh_obj = shader2.get_object_by_handle(state.sh_crc()).unwrap();
                debug!(
                    "gr {} idx {} st {:?} obj {:?}",
                    state.group(),
                    // What is this?
                    state.index(),
                    state.state_type(),
                    state_sh_obj.name()
                );

                match state.state_type() {
                    MaterialStateType::STATE_FUNCTION => {
                        let state_sh_value_obj = shader2
                            .get_object_by_handle(state.sh_value().try_into().unwrap())
                            .unwrap()
                            .name();
                        debug!("\t {}", state_sh_value_obj);
                    }
                    MaterialStateType::STATE_SAMPLER => {
                        let state_sh_value_obj = shader2
                            .get_object_by_handle(state.sh_value().try_into().unwrap())
                            .unwrap()
                            .name();
                        debug!("\t {}", state_sh_value_obj);
                    }
                    MaterialStateType::STATE_TEXTURE => {
                        if state.sh_value() == 0 {
                            warn!("TODO: handle STATE_TEXTURE with sh_value of 0");
                        } else {
                            debug!(
                                "\t tex_idx {} {}",
                                state.sh_value(),
                                textures[(state.sh_value() - 1) as usize]
                            );

                            if state_sh_obj.name() == "tAlbedoMap" {
                                albedo_texture_idx = Some((state.sh_value() - 1) as usize);
                            }
                        }
                    }
                    _ => {}
                }
            }

            Ok(MaterialInfo {
                name_hash: material_info.name_hash(),
                mat_type: material_info.dti(),
                albedo_texture_idx,
            })
        }).collect::<anyhow::Result<Vec<MaterialInfo>>>()?;

        Ok(Self {
            textures,
            materials,
        })
    }

    pub fn textures(&self) -> &[String] {
        &self.textures
    }

    pub fn materials(&self) -> &[MaterialInfo] {
        &self.materials
    }

    pub fn material_by_name(&self, name: &str) -> Option<&MaterialInfo> {
        let computed_hash = crate::crc32(name.as_bytes(), 0xffff_ffff);

        self.materials
            .iter()
            .find(|mat| mat.name_hash == computed_hash)
    }
}

#[test]
fn test_struct_sizes() {
    assert_eq!(size_of::<MaterialHeader>(), 0x28);
    assert_eq!(size_of::<RawTextureInfo>(), 0x98);
    assert_eq!(size_of::<RawMaterialInfo>(), 0x48);
    assert_eq!(size_of::<RawMaterialState>(), 0x18);
}
