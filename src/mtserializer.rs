use std::{
    ffi::CStr,
    io::{Read, Seek},
    mem::size_of,
};

use log::debug;
use zerocopy::{FromBytes, FromZeroes};

use crate::{
    dti::{self, PropType},
    util, DTI,
};

#[repr(C, packed)]
#[derive(FromBytes, FromZeroes, Debug)]
struct Header {
    magic: u32,
    major_version: u16,
    minor_version: u16,
    max_object_id: u32,

    _reserved: u32,

    object_num: u32,
    database_size: u32,
}

#[repr(C, packed)]
#[derive(FromBytes, FromZeroes, Debug)]
struct RawObjectInfo {
    dti_hash: u32,
    padding_0x4: u32, // original: union { hash: u32, dti: MtDTI* }

    /// prop_num: 15
    bitfield_0x8: u32,
    padding_0xc: u32,
}

#[repr(C, packed)]
#[derive(FromBytes, FromZeroes, Debug)]
struct RawPropertyInfo {
    name: u64, // char*

    /// type: 8
    /// attr: 8
    /// size: 15
    /// disabled: 1
    bitfield_0x8: u32,
    pad: [u8; 36],
}

#[derive(Debug)]
struct ObjectInfo {
    dti: &'static DTI,
    props: Vec<PropertyInfo>,
}

#[derive(Debug)]
struct PropertyInfo {
    name: String,
    prop_raw_type: u32,
    prop_attr: u32,
    prop_size: u32,

    is_dynamic: bool,
    prop_type: PropType,
    is_disabled: bool,
}

#[derive(Debug)]
pub enum PropertyValue {
    Class(Vec<Class>),
    U16(Vec<u16>),
}

#[derive(Debug)]
pub struct Class {
    class_type: &'static DTI,
    props: Vec<(String, PropertyValue)>,
}

impl Class {
    pub fn class_type(&self) -> &'static DTI {
        self.class_type
    }

    pub fn props(&self) -> &[(String, PropertyValue)] {
        &self.props
    }
}

fn read_static_prop<R: Read + Seek>(
    reader: &mut R,
    prop: &PropertyInfo,
    objects: &[ObjectInfo],
) -> anyhow::Result<PropertyValue> {
    // array len?
    let array_len = util::read_struct::<u32, _>(reader)?;
    debug!("read_static_prop len: {}", array_len);

    Ok(match prop.prop_type {
        PropType::class => PropertyValue::Class(
            (0..array_len)
                .map(|_| read_class(reader, objects))
                .collect::<anyhow::Result<Vec<Class>>>()?,
        ),

        PropType::u16 => PropertyValue::U16(util::read_struct_array_stream::<u16, _>(
            reader,
            array_len as usize,
        )?),

        _ => todo!("handle prop type: {:?}", prop.prop_type),
    })
}

fn read_dynamic_prop<R: Read + Seek>(
    reader: &mut R,
    prop: &PropertyInfo,
    objects: &[ObjectInfo],
) -> anyhow::Result<PropertyValue> {
    todo!()
}

fn read_class<R: Read + Seek>(reader: &mut R, objects: &[ObjectInfo]) -> anyhow::Result<Class> {
    // blah: 1
    // type: 15
    let class_info = util::read_struct::<u32, _>(reader)?;

    debug!("class_info: {:08x}", class_info);

    if class_info == 0xfffe {
        return Err(anyhow::anyhow!("this returns null"));
    }

    let object_info = &objects[((class_info >> 1) & 0x7fff) as usize];
    debug!("class object: {:08x?}", object_info);

    let _unused_value = util::read_struct::<u64, _>(reader)?;
    debug!("unused! : {:#?}", _unused_value); // What is this!

    let props = object_info
        .props
        .iter()
        .enumerate()
        .map(|(_idx, prop)| {
            debug!(
                "prop {} size {} type {:?} ({}) attr {} (dynamic {})",
                prop.name,
                prop.prop_size,
                prop.prop_type,
                prop.prop_raw_type,
                prop.prop_attr,
                prop.is_dynamic
            );

            if prop.is_disabled {
                todo!("disabled prop");
            }

            let value = if prop.is_dynamic {
                read_dynamic_prop(reader, prop, objects)
            } else {
                read_static_prop(reader, prop, objects)
            }?;

            Ok((prop.name.clone(), value))
        })
        .collect::<anyhow::Result<Vec<(String, PropertyValue)>>>()?;

    Ok(Class {
        class_type: object_info.dti,
        props,
    })
}

pub fn deserialize<R: Read + Seek>(reader: &mut R) -> anyhow::Result<Class> {
    let header: Header = util::read_struct(reader)?;

    assert_eq!(header.magic.to_le_bytes(), "XFS\0".as_bytes());
    assert_eq!((header.major_version as u16), 16);

    debug!("Header {:#?}", header);

    let mut database_bytes = vec![0u8; header.database_size as usize];
    reader.read_exact(&mut database_bytes)?;

    let objects: Vec<_> = if header.object_num != 0 {
        let object_ptrs = (0..header.object_num).map(|object_idx| {
            let object_ptr_bytes =
                &database_bytes[object_idx as usize * 8..(object_idx as usize + 1) * 8];
            let object_ptr: u64 = u64::from_le_bytes(object_ptr_bytes.try_into().unwrap());
            debug!("object ptr {}: {:08x}", object_idx, object_ptr);

            object_ptr as usize
        });

        object_ptrs
            .map(|object_ptr| {
                let object_bytes = &database_bytes[object_ptr..];
                let object = RawObjectInfo::ref_from(&object_bytes[..size_of::<RawObjectInfo>()])
                    .expect("couldn't read object");

                let dti =
                    DTI::from_hash(object.dti_hash.try_into().expect("DTI hash should be u32"))
                        .ok_or_else(|| {
                            anyhow::anyhow!("Couldn't get DTI for hash {:08x}", { object.dti_hash })
                        })?;
                let num_props = object.bitfield_0x8 & 0x7fff;
                let is_init = (object.bitfield_0x8 & 0x8000) != 0;
                assert!(!is_init, "TODO: handle this!");

                debug!(
                    "dti {:?} object {:?} propnum {}",
                    dti.name(),
                    object,
                    num_props
                );

                let props = util::read_struct_array::<RawPropertyInfo>(
                    &object_bytes[size_of::<RawObjectInfo>()..],
                    num_props as usize,
                )?
                .enumerate()
                .map(|(idx, prop)| {
                    let prop = prop.expect("couldn't read prop");
                    let prop_name_bytes = &database_bytes[prop.name as usize..];
                    let prop_name_cstr = CStr::from_bytes_until_nul(prop_name_bytes)
                        .expect("couldn't read prop name");

                    // Property names are encoded as SHIFT-JIS
                    let (prop_name, _encoding, _success) =
                        encoding_rs::SHIFT_JIS.decode(prop_name_cstr.to_bytes());

                    debug!("prop {} {}: {:x?}", idx, prop_name, prop);

                    let prop_raw_type = prop.bitfield_0x8 & 0xff;
                    let prop_attr = (prop.bitfield_0x8 >> 8) & 0xff;
                    let prop_size = (prop.bitfield_0x8 >> 16) & 0x7fff;

                    let is_dynamic = (prop_attr & dti::PROP_ATTR_DYNAMIC) != 0;
                    let prop_type = PropType::from(prop_raw_type);
                    // TODO: Is this correct?
                    let is_disabled = (prop.bitfield_0x8 & !0x7fff_ffff) != 0;

                    PropertyInfo {
                        name: prop_name.to_string(),
                        prop_raw_type,
                        prop_attr,
                        prop_size,
                        is_dynamic,
                        prop_type,
                        is_disabled,
                    }
                })
                .collect();

                Ok(ObjectInfo { dti, props })
            })
            .collect::<anyhow::Result<Vec<ObjectInfo>>>()?
    } else {
        todo!("handle 0 object_num?");
    };

    debug!("READING CLASSES");
    let class = read_class(reader, &objects)?;

    Ok(class)
}

#[test]
fn test_struct_sizes() {
    use std::mem::size_of;

    assert_eq!(0x18, size_of::<Header>());
    assert_eq!(0x30, size_of::<RawPropertyInfo>());
}
