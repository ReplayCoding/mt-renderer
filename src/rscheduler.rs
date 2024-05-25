use std::{
    ffi::CStr,
    io::{Read, Seek},
    mem::size_of,
};

use log::debug;
use zerocopy::{FromBytes, FromZeroes};

use crate::{dti, util, DTI};

#[repr(u8)]
#[allow(non_camel_case_types)]
#[derive(Debug, strum::FromRepr)]
enum SchedulerTrackType {
    TYPE_UNKNOWN = 0,
    TYPE_ROOT = 1,
    TYPE_UNIT = 2,
    TYPE_SYSTEM = 3,
    TYPE_SCHEDULER = 4,
    TYPE_OBJECT = 5,
    TYPE_INT = 6,
    TYPE_INT64 = 7,
    TYPE_VECTOR = 8,
    TYPE_FLOAT = 9,
    TYPE_FLOAT64 = 10,
    TYPE_BOOL = 11,
    TYPE_REF = 12,
    TYPE_RESOURCE = 13,
    TYPE_STRING = 14,
    TYPE_EVENT = 15,
    TYPE_MATRIX = 16,
}

#[repr(C, packed)]
#[derive(Debug, FromBytes, FromZeroes, Clone)]
struct SchedulerTrack {
    bitfield_0: u32,

    field_4: u32,         // parent/moveline TODO: what the fuck is a moveline
    track_prop_name: u64, // track/prop name ptr
    field_10: u32,        // prop idx/dti
    pad_14: u32,

    unit_group: u64,

    key_frame: u64, // KEY*
    key_value: u64, // u8*
}

impl SchedulerTrack {
    fn track_type(&self) -> u8 {
        (self.bitfield_0 & 0xff) as u8
    }

    fn prop_type(&self) -> u8 {
        ((self.bitfield_0 >> 8) & 0xff) as u8
    }

    fn key_num(&self) -> u16 {
        ((self.bitfield_0 >> 16) & 0xffff) as u16
    }
}

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
    metadata: u64,
    // track: [TRACK; ...],
}

#[derive(Debug)]
pub struct SchedulerFile {}

impl SchedulerFile {
    pub fn new<R: Read + Seek>(reader: &mut R) -> anyhow::Result<Self> {
        let mut file_data: Vec<u8> = vec![];
        reader.read_to_end(&mut file_data)?;

        let header =
            SchedulerHeader::read_from(&file_data[..size_of::<SchedulerHeader>()]).unwrap();
        debug!("header: {:#?}", header);

        assert_eq!(header.magic.to_ne_bytes(), "SDL\0".as_bytes());
        assert_eq!({ header.version }, 0x16);

        let tracks = util::read_struct_array::<SchedulerTrack>(
            &file_data[size_of::<SchedulerHeader>()..],
            header.track_num.into(),
        )?;

        for track in tracks {
            let track = track.unwrap();

            let name_bytes = &file_data[(header.metadata + track.track_prop_name) as usize..];
            let name = CStr::from_bytes_until_nul(name_bytes);

            let track_type = SchedulerTrackType::from_repr(track.track_type()).unwrap();
            let prop_type = dti::PropType::from_repr(track.prop_type().into()).unwrap(); // TODO: move this down

            debug!(
                "track type {:?} ptype {:?} keynum {} prop/track name {:?} \n{:?}",
                track_type,
                prop_type,
                { track.key_num() },
                name,
                track
            );

            match track_type {
                SchedulerTrackType::TYPE_ROOT | SchedulerTrackType::TYPE_OBJECT => {}

                SchedulerTrackType::TYPE_UNIT | SchedulerTrackType::TYPE_SYSTEM => {
                    let dti = DTI::from_hash(track.field_10);
                    debug!("dti {:?}", dti.map(|d| d.name()));
                }

                SchedulerTrackType::TYPE_INT
                | SchedulerTrackType::TYPE_INT64
                | SchedulerTrackType::TYPE_VECTOR
                | SchedulerTrackType::TYPE_FLOAT
                | SchedulerTrackType::TYPE_FLOAT64
                | SchedulerTrackType::TYPE_BOOL
                | SchedulerTrackType::TYPE_REF
                | SchedulerTrackType::TYPE_RESOURCE
                | SchedulerTrackType::TYPE_STRING
                | SchedulerTrackType::TYPE_EVENT
                | SchedulerTrackType::TYPE_MATRIX => {
                    let frame_infos = util::read_struct_array::<u32>(
                        &file_data[track.key_frame as usize..],
                        track.key_num() as usize,
                    )?;
                    let frame_values_bytes = &file_data[track.key_value as usize..];

                    for (idx, info) in frame_infos.enumerate() {
                        let info = info.unwrap();
                        debug!(
                            "frame no {} mode {:x}",
                            (info & 0xffffff),
                            (info >> 24) & 0xff
                        );

                        match prop_type {
                            dti::PropType::bool => {
                                debug!("value: {}", frame_values_bytes[idx]);
                            }
                            dti::PropType::u32 => {
                                let offs = idx * size_of::<u32>();
                                debug!(
                                    "value: {}",
                                    u32::from_le_bytes(
                                        frame_values_bytes[offs..offs + size_of::<u32>()]
                                            .try_into()
                                            .unwrap()
                                    )
                                );
                            }
                            _ => todo!("handle prop type {:?}", prop_type),
                        }
                    }
                }

                SchedulerTrackType::TYPE_UNKNOWN => todo!(),
                SchedulerTrackType::TYPE_SCHEDULER => todo!(),
            }
        }

        Ok(Self {})
    }
}

#[test]
fn test_struct_sizes() {
    assert_eq!(size_of::<SchedulerTrack>(), 0x30);
}
