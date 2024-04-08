use std::{
    collections::HashMap,
    ffi::CStr,
    io::{Read, Seek},
};

use anyhow::anyhow;
use log::{debug, warn};

#[repr(C, packed)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable, Debug)]
struct Shader2Header {
    magic: u32,
    major_version: u16,
    minor_version: u16,
    shader_version: u32,
    num_objects: u32,

    stringtable_offs: u64, // char*
    /// Technically, this isn't really a member, but the
    /// start of an array. However, the index starts at 1,
    /// and so the game only loads num_objects - 1 objects
    pbojects: u64, // (OBJECT*)[]
}

#[repr(C, packed)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable, Debug)]
struct RawShader2Object {
    name_offs: u64,  // char*
    sname_offs: u64, // char*

    bitfield_1: u32,
}

impl RawShader2Object {
    fn obj_type(&self) -> u32 {
        self.bitfield_1 & 0x3f
    }
}

#[repr(C, packed)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable, Debug)]
struct RawShader2InputElement {
    name: u64,
    bitfield: u32,
    padding1: u32,
}

#[derive(strum::FromRepr, Debug, PartialEq, Eq)]
#[allow(non_camel_case_types, unused)]
enum InputElementFormat {
    IEF_UNDEFINED = 0,
    IEF_F32 = 1,
    IEF_F16 = 2,
    IEF_S16 = 3,
    IEF_U16 = 4,
    IEF_S16N = 5,
    IEF_U16N = 6,
    IEF_S8 = 7,
    IEF_U8 = 8,
    IEF_S8N = 9,
    IEF_U8N = 10,
    IEF_SCMP3N = 11,
    IEF_UCMP3N = 12,
    IEF_U8NL = 13,
    IEF_COLOR4N = 14,
    IEF_MAX = 15,
}

#[derive(Debug)]
struct Shader2InputElement {
    name: String,
    sindex: u32,
    format: InputElementFormat,
    count: u32,
    start: u32,
    offset: u32,
    instance: u32,
}

#[derive(Debug)]
pub struct Shader2ObjectInputLayoutInfo {
    elements: Vec<Shader2InputElement>,
}

#[derive(Debug)]
pub enum Shader2ObjectTypedInfo {
    None,
    InputLayout(Shader2ObjectInputLayoutInfo),
}

#[derive(Debug)]
pub struct Shader2Object {
    name: String,
    sname: Option<String>,
    obj_type: u32,
    hash: u32,

    obj_specific: Shader2ObjectTypedInfo,
}

impl Shader2Object {
    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn obj_specific(&self) -> &Shader2ObjectTypedInfo {
        &self.obj_specific
    }
}

pub struct Shader2 {
    hash_to_object: HashMap<u32, usize>,
    objects: Vec<Shader2Object>,
}

impl Shader2 {
    pub fn new<R: Read + Seek>(reader: &mut R) -> anyhow::Result<Self> {
        let mut file_data: Vec<u8> = vec![];
        reader.read_to_end(&mut file_data)?;

        let header: &Shader2Header =
            bytemuck::from_bytes(&file_data[..std::mem::size_of::<Shader2Header>()]);
        debug!("shader2 header: {:#?}", header);

        if header.magic != 0x58464d {
            let header_magic = header.magic;
            return Err(anyhow!("rShader2 magic incorrect: {:08x}", header_magic));
        };

        let stringtable_bytes = &file_data[header.stringtable_offs as usize..];

        let mut objects = vec![];

        let object_ptrs_bytes = &file_data[std::mem::size_of::<Shader2Header>()
            ..std::mem::size_of::<Shader2Header>() + ((header.num_objects as usize - 1) * 8)];
        let object_ptrs: &[u64] = bytemuck::cast_slice(&object_ptrs_bytes);
        for object_ptr in object_ptrs {
            let object_bytes = &file_data[*object_ptr as usize..];

            let object: &RawShader2Object =
                bytemuck::from_bytes(&object_bytes[..std::mem::size_of::<RawShader2Object>()]);

            let name_offs = object.name_offs; // :(
            assert_ne!(name_offs, 0);
            let name = CStr::from_bytes_until_nul(&stringtable_bytes[name_offs as usize..])?;

            let sname = if object.sname_offs != 0 {
                Some(CStr::from_bytes_until_nul(
                    &stringtable_bytes[object.sname_offs as usize..],
                )?)
            } else {
                None
            };
            let hash = crate::crc32(name.to_bytes(), 0xffff_ffff) & 0xfffff;

            let obj_specific = match object.obj_type() {
                // InputLayout
                9 => {
                    let element_count =
                        u16::from_le_bytes(object_bytes[0x28..0x28 + 2].try_into().unwrap());

                    let mut elements = vec![];
                    for i in 0..element_count {
                        let arr_offs =
                            0x38 + (std::mem::size_of::<RawShader2InputElement>() * i as usize);
                        let raw_element: &RawShader2InputElement = bytemuck::from_bytes(
                            &object_bytes[arr_offs
                                ..arr_offs + std::mem::size_of::<RawShader2InputElement>()],
                        );

                        let element_name = CStr::from_bytes_until_nul(
                            &stringtable_bytes[raw_element.name as usize..],
                        )?;

                        // 8.	| sindex (bitstart=0,nbits=6)
                        // 8.	| format (bitstart=6,nbits=5)
                        // 8.	| count (bitstart=11,nbits=7)
                        // 8.	| start (bitstart=18,nbits=4)
                        // 8.	| offset (bitstart=22,nbits=9)
                        // 8.	| instance (bitstart=31,nbits=1)
                        let element_parsed = Shader2InputElement {
                            name: element_name.to_string_lossy().to_string(),
                            sindex: raw_element.bitfield & 0x3f,
                            format: InputElementFormat::from_repr(
                                ((raw_element.bitfield >> 6) & 0x1f) as usize,
                            )
                            .unwrap(),
                            count: (raw_element.bitfield >> 11) & 0x7f,
                            start: (raw_element.bitfield >> 18) & 0x0f,
                            offset: (raw_element.bitfield >> 22) & 0x1ff,
                            instance: (raw_element.bitfield >> 31) & 0x01,
                        };

                        elements.push(element_parsed);
                    }
                    Shader2ObjectTypedInfo::InputLayout(Shader2ObjectInputLayoutInfo { elements })
                }
                _ => Shader2ObjectTypedInfo::None,
            };

            objects.push(Shader2Object {
                name: name.to_string_lossy().to_string(),
                sname: sname.map(|x| x.to_string_lossy().to_string()),
                obj_type: object.obj_type(),
                hash,
                obj_specific,
            });
        }

        let mut hash_to_object: HashMap<u32, usize> = HashMap::new();
        for (i, object) in objects.iter().enumerate() {
            assert!(
                !hash_to_object.contains_key(&object.hash),
                "Shader Object hash collision: {} and {}",
                object.name,
                objects[*hash_to_object.get(&object.hash).unwrap()].name
            );

            hash_to_object.insert(object.hash, i);
        }

        Ok(Self {
            objects,
            hash_to_object,
        })
    }

    pub fn objects(&self) -> &[Shader2Object] {
        &self.objects
    }

    pub fn get_object_by_handle<'a>(&'a self, handle: u32) -> Option<&'a Shader2Object> {
        let hash = (handle & 0xfffff000) >> 0xc;
        let idx = self.hash_to_object.get(&hash)?;

        Some(&self.objects[*idx])
    }

    pub fn create_vertex_buffer_elements(
        inputlayout: &Shader2ObjectInputLayoutInfo,
    ) -> Vec<wgpu::VertexAttribute> {
        let mut elements = vec![];

        for (shader_location, element) in inputlayout.elements.iter().enumerate() {
            if element.format == InputElementFormat::IEF_SCMP3N {
                warn!("Skipping element {:#?}", element);
                continue;
            }

            elements.push(wgpu::VertexAttribute {
                // TODO: verify this against nDraw::InputLayout::addVertexElement
                format: match element.format {
                    InputElementFormat::IEF_U8 => match element.count {
                        4 => wgpu::VertexFormat::Uint8x2,
                        _ => todo!("unhandled count: {:#?}", element),
                    },
                    InputElementFormat::IEF_U8N => match element.count {
                        1 => wgpu::VertexFormat::Unorm8x2,
                        4 => wgpu::VertexFormat::Unorm8x4,
                        _ => todo!("unhandled count: {:#?}", element),
                    },
                    InputElementFormat::IEF_S8N => {
                        match element.count {
                            1 => wgpu::VertexFormat::Snorm8x2, // There isn't a 8x1, so this is the closest we have
                            3 => wgpu::VertexFormat::Snorm8x4, // There isn't a 8x3, so this is the closest we have
                            4 => wgpu::VertexFormat::Snorm8x4,
                            _ => todo!("unhandled count: {:#?}", element),
                        }
                    }
                    InputElementFormat::IEF_S16N => {
                        match element.count {
                            1 => wgpu::VertexFormat::Snorm16x2, // There isn't a 16x1, so this is the closest we have
                            3 => wgpu::VertexFormat::Snorm16x4, // There isn't a 16x3, so this is the closest we have
                            _ => todo!("unhandled count: {:#?}", element),
                        }
                    }
                    InputElementFormat::IEF_S16 => {
                        match element.count {
                            1 => wgpu::VertexFormat::Sint16x2, // There isn't a 16x1, so this is the closest we have
                            _ => todo!("unhandled count: {:#?}", element),
                        }
                    }
                    InputElementFormat::IEF_U16 => match element.count {
                        2 => wgpu::VertexFormat::Uint16x2,
                        _ => todo!("unhandled count: {:#?}", element),
                    },
                    InputElementFormat::IEF_F16 => match element.count {
                        2 => wgpu::VertexFormat::Float16x2,
                        _ => todo!("unhandled count: {:#?}", element),
                    },
                    InputElementFormat::IEF_F32 => match element.count {
                        3 => wgpu::VertexFormat::Float32x3,
                        _ => todo!("unhandled count: {:#?}", element),
                    },
                    InputElementFormat::IEF_U8NL => match element.count {
                        3 => wgpu::VertexFormat::Unorm8x4,
                        _ => todo!("unhandled count: {:#?}", element),
                    },
                    _ => todo!("unimplemented input element format: {:#?}", element),
                },
                offset: element.offset.into(),
                shader_location: shader_location as u32,
            });
        }

        elements
    }
}

#[test]
fn test_struct_sizes() {
    assert_eq!(std::mem::size_of::<Shader2Header>(), 0x20);
    assert_eq!(std::mem::size_of::<RawShader2InputElement>(), 0x10);
}
