use std::{
    ffi::CStr,
    io::{Read, Write},
};

use log::debug;
use serde::{Deserialize, Serialize};
use zerocopy::{AsBytes, FromBytes, FromZeroes};

use crate::util;

const HASH_TABLE_LEN: usize = 256;

#[repr(C, packed)]
#[derive(Debug, FromBytes, FromZeroes, AsBytes)]
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
#[derive(Debug, FromBytes, FromZeroes, AsBytes, Clone)]
struct RawGuiMessageIndexItem {
    message_index: u32,
    hash_a: u32,
    hash_b: u32,
    padding: u32,

    label_offset: u64,
    // NOTE: 0 is already used for nullptr, so -1 marks the 0th index
    hash_link: u64,
}

#[derive(Debug, Serialize, Deserialize)]
struct GuiMessageIndexItem {
    label: String,
    // NOTE: this is assuming that no items have duplicate message indices, which is theoretically possible
    message: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GuiMessageFile {
    update_time: chrono::DateTime<chrono::Utc>,
    language_id: u32,
    package_name: String,

    messages: Vec<GuiMessageIndexItem>,
}

impl GuiMessageFile {
    pub fn new<R: Read>(reader: &mut R) -> anyhow::Result<Self> {
        let header = util::read_struct::<GuiMessageHeader, _>(reader)?;
        debug!("header {:#?}", header);

        assert_eq!(header.magic.to_ne_bytes(), "GMD\0".as_bytes());
        assert_eq!({ header.version }, 0x10302);

        // TODO: is this correct? dates seem to line up with original development...
        let update_time = chrono::DateTime::from_timestamp(header.update_time as i64, 0);
        debug!("update time: {:?}", update_time.map(|e| e.to_rfc2822()));

        let mut package_name_buf = vec![0u8; (header.package_name_len + 1) as usize];
        reader.read_exact(&mut package_name_buf)?;
        let package_name = CStr::from_bytes_until_nul(&package_name_buf)?;
        debug!("package name {:#?}", package_name);

        let index = util::read_struct_array_stream::<RawGuiMessageIndexItem, _>(
            reader,
            header.index_num as usize,
        )?;

        if header.index_num != 0 {
            let hash_table = util::read_struct_array_stream::<u64, _>(reader, HASH_TABLE_LEN)?;
            debug!("hash_table \n{:016x?}", hash_table);
        }

        let mut index_name_buf = vec![0u8; header.index_name_buf_size as usize];
        reader.read_exact(&mut index_name_buf)?;

        let mut message_buf = vec![0u8; header.message_buffer_size as usize];
        reader.read_exact(&mut message_buf)?;

        let mut messages: Vec<String> = vec![];

        let mut current_message_data = vec![];
        for current_char in &message_buf {
            if *current_char == 0 {
                let old = std::mem::take(&mut current_message_data);
                messages.push(String::from_utf8(old)?); // TODO: is it actually utf8? seems to be in DGS1 on 3DS

                continue;
            }

            current_message_data.push(*current_char);
        }

        let messages_index_mapped =  index.iter().enumerate().map(|(item_idx, item)| {
            let index_name_buf: &[u8] = &index_name_buf;
            let item_name =
                CStr::from_bytes_until_nul(&index_name_buf[item.label_offset as usize..])?;

            let hash = util::crc32(item_name.to_bytes(), 0xffff_ffff);
            let hash_a = util::crc32(item_name.to_bytes(), hash);
            let hash_b = util::crc32(item_name.to_bytes(), hash_a);
            assert_eq!({ item.hash_a }, hash_a);
            assert_eq!({ item.hash_b }, hash_b);
            debug!("item {item_idx}: message index {} name {item_name:?} hash {:02x} hasha {hash_a:08x} hashb {hash_b:08x} link idx {}", { item.message_index }, hash & 0xff, {item.hash_link});

            Ok(GuiMessageIndexItem {
                label: String::from_utf8(item_name.to_bytes().to_vec())?,
                message: messages[item.message_index as usize].clone(),
            })
        }).collect::<anyhow::Result<Vec<GuiMessageIndexItem>>>()?;

        Ok(Self {
            update_time: update_time.expect("failed to decode datetime"),
            messages: messages_index_mapped,
            language_id: header.language_id,
            package_name: String::from_utf8(package_name.to_bytes().to_vec())?,
        })
    }

    pub fn save<W: Write>(&self, writer: &mut W) -> anyhow::Result<()> {
        // build message & label buffers
        let mut label_offsets = vec![0];
        let mut current_label_offset = 0;

        let mut label_buf: Vec<u8> = vec![];
        let mut message_buf: Vec<u8> = vec![];

        for message in &self.messages {
            let label_bytes = message.label.as_bytes();
            label_buf.extend_from_slice(label_bytes);
            label_buf.push(0); // nullbyte

            current_label_offset += label_bytes.len() + 1; // + null
            label_offsets.push(current_label_offset);

            message_buf.extend_from_slice(message.message.as_bytes());
            message_buf.push(0); // nullbyte
        }

        // build index & hash table
        let mut hash_table = [0u64; HASH_TABLE_LEN];
        let mut index = vec![];
        for (idx, message) in self.messages.iter().enumerate() {
            let label_bytes = message.label.as_bytes();

            let hash = util::crc32(label_bytes, 0xffff_ffff);
            let hash_a = util::crc32(label_bytes, hash);
            let hash_b = util::crc32(label_bytes, hash_a);

            let truncated_hash = hash & 0xff;

            if hash_table[truncated_hash as usize] != 0 {
                todo!("handle hash collision");
            }

            hash_table[truncated_hash as usize] = if idx != 0 { idx as u64 } else { -1_i64 as u64 };

            index.push(RawGuiMessageIndexItem {
                message_index: idx as u32,
                hash_a,
                hash_b,
                padding: 0xcdcdcdcd,
                label_offset: label_offsets[idx] as u64,
                hash_link: 0, // TODO
            });
        }

        let header = GuiMessageHeader {
            magic: u32::from_ne_bytes("GMD\0".as_bytes().try_into().unwrap()),
            version: 0x10302,
            language_id: self.language_id,
            update_time: self.update_time.timestamp() as u64,
            index_num: self.messages.len() as u32,
            message_num: self.messages.len() as u32,
            index_name_buf_size: label_buf.len() as u32,
            message_buffer_size: message_buf.len() as u32,
            package_name_len: self.package_name.len() as u32,
        };

        writer.write_all(header.as_bytes())?;
        writer.write_all(self.package_name.as_bytes())?;
        writer.write_all(&[0])?; // nullbyte

        for item in index {
            writer.write_all(item.as_bytes())?;
        }

        writer.write_all(hash_table.as_bytes())?;

        writer.write_all(&label_buf)?;
        writer.write_all(&message_buf)?;

        Ok(())
    }
}

#[test]
fn test_struct_sizes() {
    use std::mem::size_of;

    assert_eq!(size_of::<RawGuiMessageIndexItem>(), 1 << 5);
}
