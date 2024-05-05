use std::io::{Read, Seek};

use log::debug;
use zerocopy::{FromBytes, FromZeroes};

use crate::util;

#[repr(u32)]
#[derive(strum::FromRepr, Debug, Copy, Clone)]
#[allow(non_camel_case_types, unused)]
pub enum FormatType {
    FORMAT_R8G8B8A8_UNORM = 7,
    FORMAT_BC1_UNORM = 19,
    FORMAT_BC7_UNORM = 54,
}

impl FormatType {
    pub fn wgpu_type(&self) -> wgpu::TextureFormat {
        match self {
            Self::FORMAT_BC1_UNORM => wgpu::TextureFormat::Bc1RgbaUnorm,
            Self::FORMAT_R8G8B8A8_UNORM => wgpu::TextureFormat::Rgba8Unorm,
            Self::FORMAT_BC7_UNORM => wgpu::TextureFormat::Bc7RgbaUnorm,
        }
    }
}

#[repr(u32)]
#[derive(strum::FromRepr, Debug, PartialEq, Eq)]
#[allow(non_camel_case_types, unused)]
enum TextureType {
    TT_UNDEFINED = 0,
    TT_1D = 1,
    TT_2D = 2,
    TT_3D = 3,
    TT_1DARRAY = 4,
    TT_2DARRAY = 5,
    TT_CUBE = 6,
    TT_CUBEARRAY = 7,
    TT_2DMS = 8,
    TT_2DMSARRAY = 9,
}

// class HEADER	size(16):
// 	+---
//  0	| magic
//  4.	| version (bitstart=0,nbits=16)
//  4.	| attr (bitstart=16,nbits=8)
//  4.	| prebias (bitstart=24,nbits=4)
//  4.	| type (bitstart=28,nbits=4)
//  8.	| level_count (bitstart=0,nbits=6)
//  8.	| width (bitstart=6,nbits=13)
//  8.	| height (bitstart=19,nbits=13)
// 12.	| array_count (bitstart=0,nbits=8)
// 12.	| format (bitstart=8,nbits=8)
// 12.	| depth (bitstart=16,nbits=13)
// 12.	| auto_resize (bitstart=29,nbits=1)
// 12.	| render_target (bitstart=30,nbits=1)
// 12.	| use_vtf (bitstart=31,nbits=1)
// 	+---
#[repr(C, packed)]
#[derive(Debug, FromBytes, FromZeroes)]
struct TextureHeader {
    magic: u32,
    bitfield_4: u32,
    bitfield_8: u32,
    bitfield_c: u32,
}

impl TextureHeader {
    fn version(&self) -> u32 {
        self.bitfield_4 & 0xffff
    }
    fn prebias(&self) -> u32 {
        (self.bitfield_4 >> 24) & 0xf
    }
    fn width(&self) -> u32 {
        ((self.bitfield_8 >> 6) & 0x1fff) << self.prebias()
    }
    fn height(&self) -> u32 {
        ((self.bitfield_8 >> 19) & 0x1fff) << self.prebias()
    }
    fn image_type(&self) -> TextureType {
        let v = (self.bitfield_4 >> 28) & 0xf;

        TextureType::from_repr(v).unwrap()
    }
    fn format_raw(&self) -> u32 {
        (self.bitfield_c >> 8) & 0xff
    }
    fn format(&self) -> FormatType {
        FormatType::from_repr(self.format_raw()).unwrap()
    }
    fn array_count(&self) -> u32 {
        self.bitfield_c & 0xff
    }
    fn level_count(&self) -> u32 {
        self.bitfield_8 & 0x3f
    }
}

pub struct TextureFile {
    width: u32,
    height: u32,
    format: FormatType,

    data: Vec<u8>,
}

impl TextureFile {
    pub fn new<R: Read + Seek>(reader: &mut R) -> anyhow::Result<Self> {
        let header: TextureHeader = util::read_struct(reader)?;

        debug!("HEADER: {:#x?}", header);
        debug!(
            "v: {:04x} pb: {} w: {} h: {} t: {:?} f: {:?} ({}) ac: {} lc: {}",
            header.version(),
            header.prebias(),
            header.width(),
            header.height(),
            header.image_type(),
            header.format(),
            header.format_raw(),
            header.array_count(),
            header.level_count(),
        );

        assert_eq!(header.magic.to_ne_bytes(), "TEX\0".as_bytes());
        assert_eq!(header.image_type(), TextureType::TT_2D);

        // TODO: read SH data (cubemap)

        // TODO: is this what it is?
        let num_images = header.array_count() * header.level_count();
        let mut unk_offsets_bytes = vec![0u8; (num_images << 3) as usize];
        reader.read_exact(&mut unk_offsets_bytes)?;

        // this is stupid, it shouldn't be a Vec!
        let unk_offsets: Vec<u64> =
            util::read_struct_array::<u64>(&unk_offsets_bytes, num_images as usize)?
                .map(|o| *o.unwrap())
                .collect();

        debug!("texture offsets: {:08x?}", unk_offsets);

        // TEMP HACK
        // assert_eq!(unk_offsets.len(), 1);

        let offset = unk_offsets[0];
        reader.seek(std::io::SeekFrom::Start(offset))?;

        let mut image_data: Vec<u8> = vec![];
        reader.read_to_end(&mut image_data)?;

        Ok(Self {
            width: header.width(),
            height: header.height(),
            format: header.format(),
            data: image_data,
        })
    }

    pub fn width(&self) -> u32 {
        self.width
    }

    pub fn height(&self) -> u32 {
        self.height
    }

    pub fn format(&self) -> FormatType {
        self.format
    }

    pub fn data(&self) -> &[u8] {
        &self.data
    }
}

#[test]
fn test_struct_sizes() {
    use std::mem::size_of;
    assert_eq!(0x10, size_of::<TextureHeader>());
}
