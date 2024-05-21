use std::{
    ffi::CStr,
    io::{Read, Seek},
};

use log::debug;
use zerocopy::{FromBytes, FromZeroes};

use crate::util;

#[repr(C, packed)]
#[derive(Debug, FromBytes, FromZeroes)]
struct GuiMessageHeader {
    magic: u32,
    version: u32,
    language_id: u32,
    update_time: u64,
    index_num: u32,
    message_num: u32,
    index_name_buf_size: u32,
    buffer_size: u32,

    unk_size: u32,
}

#[repr(C, packed)]
#[derive(Debug, FromBytes, FromZeroes, Clone)]
struct GuiMessageIndex {
    index: u32,
    hash_a: u32,
    hash_b: u32,
    padding: u32,
    offset: u64,
    hash_link: u64,
}

pub struct GuiMessageFile {}

impl GuiMessageFile {
    pub fn new<R: Read + Seek>(reader: &mut R) -> anyhow::Result<Self> {
        let header = util::read_struct::<GuiMessageHeader, _>(reader)?;
        debug!("header {:#?}", header);

        assert_eq!(header.magic.to_ne_bytes(), "GMD\0".as_bytes());
        assert_eq!({ header.version }, 0x10302);

        // TODO: is this correct?
        let edit_time = chrono::DateTime::from_timestamp(header.update_time as i64, 0);
        debug!("edit time: {:?}", edit_time.map(|e| e.to_rfc2822()));

        let mut package_name_buf = vec![0u8; (header.unk_size + 1) as usize];
        reader.read_exact(&mut package_name_buf)?;
        let package_name = CStr::from_bytes_until_nul(&package_name_buf)?;
        debug!("str {:#?}", package_name);

        if header.index_num != 0 {
            let index = util::read_struct_array_stream::<GuiMessageIndex, _>(
                reader,
                header.index_num as usize,
            )?;

            debug!("index \n{:#08x?}", index);

            let mut hash_table = vec![0u8; 0x800];
            reader.read_exact(&mut hash_table)?;
            debug!("hash_table \n{}", util::hexdump(&hash_table));
        }

        if header.index_name_buf_size != 0 {
            let mut index_name_buf = vec![0u8; header.index_name_buf_size as usize];
            reader.read_exact(&mut index_name_buf)?;

            debug!("index names \n{}", util::hexdump(&index_name_buf));
        }

        let mut buf = vec![0u8; header.buffer_size as usize];
        reader.read_exact(&mut buf)?;
        debug!("buf \n{}", util::hexdump(&buf));

        // TOOD: index & string ptr splitting

        Ok(Self {})
    }
}

#[test]
fn test_struct_sizes() {
    use std::mem::size_of;

    assert_eq!(size_of::<GuiMessageIndex>(), 1 << 5);
}
