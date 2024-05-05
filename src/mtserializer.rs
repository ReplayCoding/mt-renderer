use std::io::{Read, Seek};

use log::debug;

use crate::{util, DTI};

#[repr(C, packed)]
#[derive(bytemuck::Zeroable, bytemuck::Pod, Clone, Copy, Debug)]
struct Header {
    magic: u32,
    major_version: u16,
    minor_version: u16,
    max_object_id: u32,

    _reserved: u32,

    object_num: u32,
    database_size: u32,
}

struct _RawObjectInfo {}

#[derive(Debug)]
pub struct MtSerializer {}

impl MtSerializer {
    pub fn new<R: Read + Seek>(reader: &mut R) -> anyhow::Result<Self> {
        let header: Header = util::read_struct(reader)?;

        assert_eq!(header.magic.to_le_bytes(), "XFS\0".as_bytes());
        assert_eq!((header.major_version as u16), 16);

        debug!("Header {:#?}", header);

        let mut database_bytes = vec![0u8; header.database_size as usize];
        reader.read_exact(&mut database_bytes)?;

        if header.object_num != 0 {
            let object_ptrs = (0..header.object_num).map(|object_idx| {
                let object_ptr_bytes =
                    &database_bytes[object_idx as usize * 8..(object_idx as usize + 1) * 8];
                let object_ptr: u64 = u64::from_le_bytes(object_ptr_bytes.try_into().unwrap());
                debug!("object ptr {}: {:08x}", object_idx, object_ptr);
                assert!(
                    object_ptr >= (header.object_num * 8).into(),
                    "object pointers overlap objects!"
                );

                object_ptr as usize
            });

            for object_ptr in object_ptrs {
                let dti_hash = u32::from_le_bytes(
                    database_bytes[object_ptr..object_ptr + 4]
                        .try_into()
                        .unwrap(),
                );
                debug!("DTI {:08x} {:?}", dti_hash, DTI::from_hash(dti_hash));
            }
        } else {
            todo!("handle 0 object_num");
        }

        Ok(Self {})
    }
}

#[test]
fn test_struct_sizes() {
    use std::mem::size_of;

    assert_eq!(0x18, size_of::<Header>());
}
