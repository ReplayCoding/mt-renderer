use std::{
    collections::HashMap,
    ffi::CStr,
    io::{Read, Seek},
    mem::size_of,
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

    bitfield_0x10: u32,
    bitfield_0x14: u32,

    hash: u32, // ?
    padding1: u32,
    annotations: u64, // VARIABLE*
}

impl RawShader2Object {
    fn obj_type(&self) -> u32 {
        self.bitfield_0x10 & 0x3f
    }

    fn annotation_num(&self) -> u32 {
        self.bitfield_0x10 >> 0x16
    }
}

#[repr(C, packed)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable, Debug)]
struct RawShader2InputElement {
    name: u64,
    bitfield: u32,
    padding1: u32,
}

#[repr(u32)]
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
#[allow(unused)] // TODO
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
    stride: u32,
    elements: Vec<Shader2InputElement>,
}

#[derive(Debug)]
pub struct Shader2ObjectStructInfo {
    variables: Vec<Shader2Variable>,
}

#[derive(Debug)]
pub struct Shader2ObjectCBufferInfo {
    crc: u32,
    variables: Vec<Shader2Variable>,
}

#[derive(Debug)]
pub enum Shader2ObjectTypedInfo {
    None,
    InputLayout(Shader2ObjectInputLayoutInfo),
    Struct(Shader2ObjectStructInfo),
    CBuffer(Shader2ObjectCBufferInfo),
}

#[repr(u32)]
#[derive(strum::FromRepr, Debug)]
#[allow(non_camel_case_types)]
enum ObjectType {
    OT_CBUFFER = 0,
    OT_TEXTURE = 1,
    OT_FUNCTION = 2,
    OT_SAMPLER = 3,
    OT_BLEND = 4,
    OT_DEPTHSTENCIL = 5,
    OT_RASTERIZER = 6,
    OT_TECHNIQUE = 7,
    OT_STRUCT = 8,
    OT_INPUTLAYOUT = 9,
    OT_SAMPLERCMP = 10,
    OT_POINTSTREAM = 11,
    OT_LINESTREAM = 12,
    OT_TRIANGLESTREAM = 13,
    OT_INPUTPATCH = 14,
    OT_OUTPUTPATCH = 15,

    OT_UNKNOWN_16 = 16,
    OT_UNKNOWN_17 = 17, // related to compute?
}

#[derive(Debug)]
#[allow(unused)] // TODO
pub struct Shader2Object {
    name: String,
    sname: Option<String>,
    annotations: Option<Vec<Shader2Variable>>,
    obj_type: ObjectType,
    name_hash: u32,

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

#[repr(C, packed)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable, Debug)]
struct RawShader2InputLayout {
    bitfield_0: u32,
    padding1: u32,
    pdefaultvalues: u64, // is this ever used?
                         // elements: [RawShader2InputElement; ...]
}

#[repr(C, packed)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable, Debug)]
struct RawShader2Struct {
    bitfield_0: u32,
    padding1: u32,
    members: u64, // VARIABLE*
}

#[repr(C, packed)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable, Debug)]
struct RawShader2CBuffer {
    bitfield_0: u32,
    crc: u32,
    variables: u64,   // VARIABLE*
    pinitvalues: u64, // void*
}

#[repr(C, packed)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable, Debug)]
struct RawShader2Variable {
    name: u64, // MT_CSTR
    bitfield_0x8: u32,
    field_4: u32, // anonymous enum
    sname: u64,   // MT_CSTR
    bitfield_0x18: u32,
    padding1: u32,
    annotations: u64, // VARIABLE*
    pinitvalues: u64, // void*
}

#[repr(u32)]
#[derive(strum::FromRepr, Debug)]
#[allow(non_camel_case_types)]
enum ClassType {
    CT_UNDEFINED = 0,
    CT_VOID = 1,
    CT_SCALAR = 2,
    CT_VECTOR = 3,
    CT_MATRIX = 4,
    CT_STRUCT = 5,
    CT_OBJECT = 6,
}

#[derive(Debug)]
struct Shader2Variable {
    name: String,
    sname: String,
    ctype: ClassType,
    size: u32,
    annotations: Option<Vec<Shader2Variable>>,
    sindex: u32,
    offset: u32,
}

pub struct Shader2File {
    name_hash_to_object: HashMap<u32, usize>,
    objects: Vec<Shader2Object>,
}

fn parse_variables(
    variables_offset: u64,
    variables_num: u32,
    file_data: &[u8],
    stringtable_bytes: &[u8],
) -> Vec<Shader2Variable> {
    (0..variables_num)
        .map(|member_idx| {
            let variable_offset =
                (member_idx as usize * size_of::<RawShader2Variable>()) + variables_offset as usize;
            let variable_bytes =
                &file_data[variable_offset..variable_offset + size_of::<RawShader2Variable>()];
            let variable: &RawShader2Variable = bytemuck::from_bytes(variable_bytes);

            let name = CStr::from_bytes_until_nul(&stringtable_bytes[variable.name as usize..])
                .expect("Unable to decode variable name for struct");
            let sname = CStr::from_bytes_until_nul(&stringtable_bytes[variable.sname as usize..])
                .expect("Unable to decode variable name for struct");

            assert_eq!(variable.padding1 as u32, 0);

            debug!("member #{} name {:?}", member_idx, name);

            // TODO: handle attr
            let _attr = variable.bitfield_0x8 & 0x7ffff;
            let ctype = (variable.bitfield_0x8 >> 19) & 0x7;
            let size = (variable.bitfield_0x8 >> 22) & 0x3ff;

            let sindex = (variable.bitfield_0x18) & 0xff;
            let offset = (variable.bitfield_0x18 >> 8) & 0x3ff;
            // TODO: what is this for?
            let _svalue = (variable.bitfield_0x18 >> 18) & 0x3f;
            let annotation_num = (variable.bitfield_0x18 >> 24) & 0xff;

            let annotations = if variable.annotations != 0 {
                Some(parse_variables(
                    variable.annotations,
                    annotation_num,
                    file_data,
                    stringtable_bytes,
                ))
            } else {
                None
            };

            Shader2Variable {
                name: name.to_string_lossy().to_string(),
                sname: sname.to_string_lossy().to_string(),
                ctype: ClassType::from_repr(ctype).expect("invalid ctype"),
                size,
                sindex,
                offset,
                annotations,
            }
        })
        .collect()
}

impl Shader2File {
    pub fn new<R: Read + Seek>(reader: &mut R) -> anyhow::Result<Self> {
        let mut file_data: Vec<u8> = vec![];
        reader.read_to_end(&mut file_data)?;

        let header: &Shader2Header = bytemuck::from_bytes(&file_data[..size_of::<Shader2Header>()]);
        debug!("shader2 header: {:#?}", header);

        if header.magic != 0x58464d {
            let header_magic = header.magic;
            return Err(anyhow!("rShader2 magic incorrect: {:08x}", header_magic));
        };

        let stringtable_bytes = &file_data[header.stringtable_offs as usize..];

        let mut objects = vec![];

        let object_ptrs_bytes = &file_data[size_of::<Shader2Header>()
            ..size_of::<Shader2Header>() + ((header.num_objects as usize - 1) * 8)];
        let object_ptrs: &[u64] = bytemuck::cast_slice(object_ptrs_bytes);
        for object_ptr in object_ptrs {
            let object_bytes = &file_data[*object_ptr as usize..];

            let object: &RawShader2Object =
                bytemuck::from_bytes(&object_bytes[..size_of::<RawShader2Object>()]);

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

            let name_hash = crate::crc32(name.to_bytes(), 0xffff_ffff) & 0xfffff;
            debug!("object {:?} {:?} {}", name, object, object.obj_type());

            let annotations = if object.annotations != 0 {
                Some(parse_variables(
                    object.annotations,
                    object.annotation_num(),
                    &file_data,
                    &stringtable_bytes,
                ))
            } else {
                None
            };

            let obj_type = ObjectType::from_repr(object.obj_type()).expect("Unknown object type");
            let obj_specific_bytes = &object_bytes[size_of::<RawShader2Object>()..];
            let obj_specific = match obj_type {
                ObjectType::OT_CBUFFER => {
                    let raw_cbuffer: &RawShader2CBuffer =
                        bytemuck::from_bytes(&obj_specific_bytes[..size_of::<RawShader2CBuffer>()]);

                    let num_variables = (raw_cbuffer.bitfield_0 >> 16) & 0xffff;

                    Shader2ObjectTypedInfo::CBuffer(Shader2ObjectCBufferInfo {
                        crc: raw_cbuffer.crc,
                        variables: parse_variables(
                            raw_cbuffer.variables,
                            num_variables,
                            &file_data,
                            stringtable_bytes,
                        ),
                    })
                }
                ObjectType::OT_STRUCT => {
                    let raw_struct: &RawShader2Struct =
                        bytemuck::from_bytes(&obj_specific_bytes[..size_of::<RawShader2Struct>()]);

                    let num_members = (raw_struct.bitfield_0 >> 0xa) & 0xfff;

                    debug!("{:#?} member#: {}", raw_struct, num_members);
                    let variables = parse_variables(
                        raw_struct.members,
                        num_members,
                        &file_data,
                        &stringtable_bytes,
                    );

                    Shader2ObjectTypedInfo::Struct(Shader2ObjectStructInfo { variables })
                }

                ObjectType::OT_INPUTLAYOUT => {
                    let raw_inputlayout: &RawShader2InputLayout = bytemuck::from_bytes(
                        &obj_specific_bytes[..size_of::<RawShader2InputLayout>()],
                    );

                    let element_count = raw_inputlayout.bitfield_0 & 0xffff;
                    let stride = (raw_inputlayout.bitfield_0 >> 16) & 0xffff;

                    let mut elements = vec![];
                    for i in 0..element_count {
                        let arr_offs = size_of::<RawShader2InputLayout>()
                            + (size_of::<RawShader2InputElement>() * i as usize);
                        let raw_element: &RawShader2InputElement = bytemuck::from_bytes(
                            &obj_specific_bytes
                                [arr_offs..arr_offs + size_of::<RawShader2InputElement>()],
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
                                (raw_element.bitfield >> 6) & 0x1f,
                            )
                            .unwrap(),
                            count: (raw_element.bitfield >> 11) & 0x7f,
                            start: (raw_element.bitfield >> 18) & 0x0f,
                            offset: (raw_element.bitfield >> 22) & 0x1ff,
                            instance: (raw_element.bitfield >> 31) & 0x01,
                        };

                        elements.push(element_parsed);
                    }
                    Shader2ObjectTypedInfo::InputLayout(Shader2ObjectInputLayoutInfo {
                        stride,
                        elements,
                    })
                }
                _ => Shader2ObjectTypedInfo::None,
            };

            objects.push(Shader2Object {
                name: name.to_string_lossy().to_string(),
                sname: sname.map(|x| x.to_string_lossy().to_string()),
                obj_type,
                annotations,
                name_hash,
                obj_specific,
            });
        }

        let mut name_hash_to_object: HashMap<u32, usize> = HashMap::new();
        for (i, object) in objects.iter().enumerate() {
            assert!(
                !name_hash_to_object.contains_key(&object.name_hash),
                "Shader Object name hash collision: {} and {}",
                object.name,
                objects[*name_hash_to_object.get(&object.name_hash).unwrap()].name
            );

            name_hash_to_object.insert(object.name_hash, i);
        }

        Ok(Self {
            objects,
            name_hash_to_object,
        })
    }

    pub fn objects(&self) -> &[Shader2Object] {
        &self.objects
    }

    pub fn get_object_by_handle(&self, handle: u32) -> Option<&Shader2Object> {
        let hash = (handle & 0xfffff000) >> 0xc;
        let idx = self.name_hash_to_object.get(&hash)?;

        Some(&self.objects[*idx])
    }

    pub fn create_vertex_buffer_elements(
        inputlayout: &Shader2ObjectInputLayoutInfo,
    ) -> Vec<wgpu::VertexAttribute> {
        debug!("Creating inputlayout {:#?}", inputlayout.elements);
        let mut elements = vec![];

        for element in inputlayout.elements.iter() {
            let shader_location = match element.name.as_str() {
                "Position" => 0,
                "TexCoord" => 1,
                _ => continue,
            };

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
    assert_eq!(size_of::<Shader2Header>(), 0x20);
    assert_eq!(size_of::<RawShader2Object>(), 0x28);
    assert_eq!(size_of::<RawShader2InputElement>(), 0x10);
    assert_eq!(size_of::<RawShader2InputLayout>(), 16);
    assert_eq!(size_of::<RawShader2Struct>(), 16);
    assert_eq!(size_of::<RawShader2Variable>(), 0x30);
    assert_eq!(size_of::<RawShader2CBuffer>(), 24);
}
