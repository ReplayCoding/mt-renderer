use std::{
    ffi::CStr,
    io::{Read, Seek},
};

use log::{debug, warn};

use crate::{dti, rshader2::Shader2};

#[repr(C, packed)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
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
#[derive(bytemuck::Pod, bytemuck::Zeroable, Debug, Copy, Clone)]
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
            .ok()
            .expect("failed to decode texture info path into CStr")
            .to_str()
            .expect("failed to convert texture info path into str")
    }

    fn dti(&self) -> Option<&'static str> {
        if self.dti_hash != 0 {
            Some(dti::from_hash(self.dti_hash).expect("invalid DTI hash in texture info"))
        } else {
            None
        }
    }
}

#[derive(strum::FromRepr, Debug)]
enum MaterialStateType {
    STATE_FUNCTION = 0,
    STATE_CBUFFER = 1,
    STATE_SAMPLER = 2,
    STATE_TEXTURE = 3,
    STATE_PROCEDURAL = 4,
}

#[repr(C, packed)]
#[derive(bytemuck::Pod, bytemuck::Zeroable, Debug, Copy, Clone)]
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
        MaterialStateType::from_repr((self.bitfield_0x0 & 0xf) as usize)
            .expect("invalid state type")
    }
    fn group(&self) -> u32 {
        (self.bitfield_0x0 >> 4) & 0xffff
    }
    fn index(&self) -> u32 {
        (self.bitfield_0x0 >> 20) & 0xfff
    }
}

#[repr(C, packed)]
#[derive(bytemuck::Pod, bytemuck::Zeroable, Debug, Copy, Clone)]
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
    fn dti(&self) -> &'static str {
        dti::from_hash(self.dti_hash).expect(
            format!(
                "invalid DTI hash in material info {:08x}",
                self.dti_hash as u32
            )
            .leak(),
        )
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
pub struct MaterialFile {}

impl MaterialFile {
    pub fn new<R: Read + Seek>(reader: &mut R, shader2: &Shader2) -> anyhow::Result<Self> {
        let mut header_bytes = [0u8; std::mem::size_of::<MaterialHeader>()];
        reader.read_exact(&mut header_bytes)?;
        let header: &MaterialHeader = bytemuck::from_bytes(&header_bytes);

        debug!("material header: {:#?}", header);

        reader.seek(std::io::SeekFrom::Start(header.textures))?;
        let textures: Vec<_> = (0..header.texture_num)
            .map(|i| {
                let mut texture_info_bytes = [0u8; std::mem::size_of::<RawTextureInfo>()];
                reader.read_exact(&mut texture_info_bytes).unwrap();
                let texture_info: &RawTextureInfo = bytemuck::from_bytes(&texture_info_bytes);

                let texture_path = texture_info.path();
                let texture_dti = texture_info.dti();
                assert_ne!(texture_dti, None);
                debug!(
                    "texture {}: dti {:?} path \"{}\"",
                    i, texture_dti, texture_path
                );

                texture_path.to_string()
            })
            .collect();

        for material_idx in 0..header.material_num {
            reader.seek(std::io::SeekFrom::Start(
                header.materials
                    + (material_idx as u64 * std::mem::size_of::<RawMaterialInfo>() as u64),
            ))?;

            let mut material_info_bytes = [0u8; std::mem::size_of::<RawMaterialInfo>()];
            reader.read_exact(&mut material_info_bytes)?;
            let material_info: &RawMaterialInfo = bytemuck::from_bytes(&material_info_bytes[..]);

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

            for state_idx in 0..material_info.state_num() {
                reader.seek(std::io::SeekFrom::Start(
                    material_info.states
                        + (state_idx as u64 * std::mem::size_of::<RawMaterialState>() as u64),
                ))?;

                let mut state_bytes = [0u8; std::mem::size_of::<RawMaterialState>()];
                reader.read_exact(&mut state_bytes)?;
                let state: &RawMaterialState = bytemuck::from_bytes(&state_bytes);

                let state_sh_obj = shader2.get_object_by_handle(state.sh_crc()).unwrap();
                debug!(
                    "gr {} idx {} st {:?} obj {:?}",
                    state.group(),
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
                        }
                    }
                    _ => {}
                }
            }
        }

        Ok(Self {})
    }
}

#[test]
fn test_struct_sizes() {
    assert_eq!(std::mem::size_of::<MaterialHeader>(), 0x28);
    assert_eq!(std::mem::size_of::<RawTextureInfo>(), 0x98);
    assert_eq!(std::mem::size_of::<RawMaterialInfo>(), 0x48);
    assert_eq!(std::mem::size_of::<RawMaterialState>(), 0x18);
}
