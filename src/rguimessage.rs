use std::{
    ffi::CStr,
    io::{Read, Seek}, mem::size_of,
};

use log::{debug, warn};
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
    message_buffer_size: u32,

    package_name_len: u32,
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

#[derive(Debug)]
pub struct GuiMessageFile {
    _edit_time: chrono::DateTime<chrono::Utc>,
    _messages: Vec<String>,
}

impl GuiMessageFile {
    pub fn new<R: Read + Seek>(reader: &mut R) -> anyhow::Result<Self> {
        let header = util::read_struct::<GuiMessageHeader, _>(reader)?;
        debug!("header {:#?}", header);

        assert_eq!(header.magic.to_ne_bytes(), "GMD\0".as_bytes());
        assert_eq!({ header.version }, 0x10302);

        // TODO: is this correct? dates seem to line up with original development...
        let edit_time = chrono::DateTime::from_timestamp(header.update_time as i64, 0);
        debug!("edit time: {:?}", edit_time.map(|e| e.to_rfc2822()));

        let mut package_name_buf = vec![0u8; (header.package_name_len + 1) as usize];
        reader.read_exact(&mut package_name_buf)?;
        let package_name = CStr::from_bytes_until_nul(&package_name_buf)?;
        debug!("package name {:#?}", package_name);

        let index = util::read_struct_array_stream::<GuiMessageIndex, _>(
            reader,
            header.index_num as usize,
        )?;

        if header.index_num != 0 {
            let mut hash_table = vec![0u8; 0x800];
            reader.read_exact(&mut hash_table)?;

            // debug!("hash_table \n{}", util::hexdump(&hash_table));
        }

        let mut index_name_buf = vec![0u8; header.index_name_buf_size as usize];
        reader.read_exact(&mut index_name_buf)?;

        let mut message_buf = vec![0u8; header.message_buffer_size as usize];
        reader.read_exact(&mut message_buf)?;

        for item in &index {
            parse_index_item(&index, item, &index_name_buf)?;
        }

        let mut messages: Vec<String> = vec![];

        let mut current_message_data = vec![];
        for current_char in &message_buf {
            if *current_char == 0 {
                let old = std::mem::take(&mut current_message_data);
                messages.push(String::from_utf8(old)?); // TODO: is it actually utf8?

                continue;
            }

            current_message_data.push(*current_char);
        }

        Ok(Self {
            _edit_time: edit_time.expect("failed to decode datetime"),
            _messages: messages,
        })
    }
}

fn parse_index_item(index: &[GuiMessageIndex], item: &GuiMessageIndex, index_name_buf: &[u8]) -> Result<(), anyhow::Error> {
    let _item_name = CStr::from_bytes_until_nul(&index_name_buf[item.offset as usize..])?;
    let hash = util::crc32(_item_name.to_bytes(), 0xffff_ffff);
    let hash_a = util::crc32(_item_name.to_bytes(), hash);
    let hash_b = util::crc32(_item_name.to_bytes(), hash_a);
    assert_eq!({ item.hash_a }, hash_a);
    assert_eq!({ item.hash_b }, hash_b);
    debug!("item: idx {} name {_item_name:?}", { item.index });
    if item.hash_link != 0 {
        assert_ne!({ item.hash_link }, -1i64 as u64); //hmmmm


        warn!(
            "TODO: hash link for item is nonzero: {:?} {}",
            _item_name,
            { item.hash_link }
        );

        let link_idx = item.hash_link as usize / size_of::<GuiMessageIndex>();
        parse_index_item(index, &index[link_idx], index_name_buf)?;

        debug!("done link");
    };

    Ok(())
}

#[test]
fn test_struct_sizes() {
    assert_eq!(size_of::<GuiMessageIndex>(), 1 << 5);
}
