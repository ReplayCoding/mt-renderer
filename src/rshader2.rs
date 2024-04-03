use std::{
    collections::HashMap,
    ffi::CStr,
    io::{Read, Seek},
};

use anyhow::anyhow;
use log::debug;

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

#[derive(Debug)]
pub struct Shader2ObjectInputLayoutInfo {
    elements: Vec<String>,
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

            let object: &RawShader2Object = bytemuck::from_bytes(&object_bytes[..std::mem::size_of::<RawShader2Object>()]);

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
                9 => Shader2ObjectTypedInfo::None,
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

    pub fn get_object_by_hash<'a>(&'a self, hash: u32) -> Option<&'a Shader2Object> {
        let idx = self.hash_to_object.get(&hash)?;

        Some(&self.objects[*idx])
    }

    pub fn objects(&self) -> &[Shader2Object] {
        &self.objects
    }
}

#[test]
fn test_struct_sizes() {
    assert_eq!(std::mem::size_of::<Shader2Header>(), 0x20);
}
