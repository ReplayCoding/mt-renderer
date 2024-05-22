use std::io::{Read, Seek};

use log::debug;
use zerocopy::{FromBytes, FromZeroes};

use crate::util;

#[repr(C, packed)]
#[derive(Debug, FromBytes, FromZeroes, Clone)]
struct SchedulerHeader {
    magic: u32,
    version: u16,
    track_num: u16,

    // UNCONFIRMED BEGIN
    crc: u32,
    bitfield_c: u32,
    base_track: u32,
    // UNCONFIRMED END
    pad_14: u32,
    field_18_ptr: u64,
    // track: [TRACK; ...],
}

#[derive(Debug)]
pub struct SchedulerFile {}

impl SchedulerFile {
    pub fn new<R: Read + Seek>(reader: &mut R) -> anyhow::Result<Self> {
        let header = util::read_struct::<SchedulerHeader, _>(reader)?;
        debug!("header: {:#?}", header);

        assert_eq!(header.magic.to_ne_bytes(), "SDL\0".as_bytes());
        assert_eq!({ header.version }, 0x16);

        Ok(Self {})
    }
}
