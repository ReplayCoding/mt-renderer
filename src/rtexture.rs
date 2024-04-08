use std::io::{Read, Seek};

use bytemuck::{Pod, Zeroable};
use log::debug;

#[derive(strum::FromRepr, Debug, Copy, Clone)]
pub enum FormatType {
    FORMAT_UNKNOWN = 0,
    FORMAT_R32G32B32A32_FLOAT = 1,
    FORMAT_R16G16B16A16_FLOAT = 2,
    FORMAT_R16G16B16A16_UNORM = 3,
    FORMAT_R16G16B16A16_SNORM = 4,
    FORMAT_R32G32_FLOAT = 5,
    FORMAT_R10G10B10A2_UNORM = 6,
    FORMAT_R8G8B8A8_UNORM = 7,
    FORMAT_R8G8B8A8_SNORM = 8,
    FORMAT_R8G8B8A8_UNORM_SRGB = 9,
    FORMAT_B4G4R4A4_UNORM = 10,
    FORMAT_R16G16_FLOAT = 11,
    FORMAT_R16G16_UNORM = 12,
    FORMAT_R16G16_SNORM = 13,
    FORMAT_R32_FLOAT = 14,
    FORMAT_D24_UNORM_S8_UINT = 15,
    FORMAT_R16_FLOAT = 16,
    FORMAT_R16_UNORM = 17,
    FORMAT_A8_UNORM = 18,
    FORMAT_BC1_UNORM = 19,
    FORMAT_BC1_UNORM_SRGB = 20,
    FORMAT_BC2_UNORM = 21,
    FORMAT_BC2_UNORM_SRGB = 22,
    FORMAT_BC3_UNORM = 23,
    FORMAT_BC3_UNORM_SRGB = 24,
    FORMAT_BCX_GRAYSCALE = 25,
    FORMAT_BCX_ALPHA = 26,
    FORMAT_BC5_SNORM = 27,
    FORMAT_B5G6R5_UNORM = 28,
    FORMAT_B5G5R5A1_UNORM = 29,
    FORMAT_BCX_NM1 = 30,
    FORMAT_BCX_NM2 = 31,
    FORMAT_BCX_RGBI = 32,
    FORMAT_BCX_RGBY = 33,
    FORMAT_B8G8R8X8_UNORM = 34,
    FORMAT_BCX_RGBI_SRGB = 35,
    FORMAT_BCX_RGBY_SRGB = 36,
    FORMAT_BCX_NH = 37,
    FORMAT_R11G11B10_FLOAT = 38,
    FORMAT_B8G8R8A8_UNORM = 39,
    FORMAT_B8G8R8A8_UNORM_SRGB = 40,
    FORMAT_BCX_RGBNL = 41,
    FORMAT_BCX_YCCA = 42,
    FORMAT_BCX_YCCA_SRGB = 43,
    FORMAT_R8_UNORM = 44,
    FORMAT_B8G8R8A8_UNORM_LE = 45,
    FORMAT_B10G10R10A2_UNORM_LE = 46,
    FORMAT_BCX_SRGBA = 47,
    FORMAT_BC7_UNORM = 48,
    FORMAT_BC7_UNORM_SRGB = 49,
    FORMAT_SE5M9M9M9 = 50,
    FORMAT_R10G10B10A2_FLOAT = 51,
    FORMAT_YVU420P2_CSC1 = 52,
    FORMAT_R8A8_UNORM = 53,
    FORMAT_A8_UNORM_WHITE = 54,
}

#[derive(strum::FromRepr, Debug, PartialEq, Eq)]
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
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
struct HEADER {
    magic: u32,
    bitfield_4: u32,
    bitfield_8: u32,
    bitfield_c: u32,
}

impl HEADER {
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

        TextureType::from_repr(v as usize).unwrap()
    }
    fn format(&self) -> FormatType {
        let v = (self.bitfield_c >> 8) & 0xff;

        FormatType::from_repr(v as usize).unwrap()
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
        let mut header_bytes = [0u8; std::mem::size_of::<HEADER>()];
        reader.read_exact(&mut header_bytes)?;
        let header: &HEADER = bytemuck::from_bytes(&header_bytes);

        debug!("HEADER: {:#?}", header);
        debug!(
            "v: {:04x} pb: {} w: {} h: {} t: {:?} f: {:?} ac: {} lc: {}",
            header.version(),
            header.prebias(),
            header.width(),
            header.height(),
            header.image_type(),
            header.format(),
            header.array_count(),
            header.level_count(),
        );

        assert_eq!(header.magic.to_ne_bytes(), "TEX\0".as_bytes());
        assert_eq!(header.image_type(), TextureType::TT_2D);

        // TODO: read SH data

        let mut unk_offsets_bytes =
            vec![0u8; ((header.array_count() * header.level_count()) << 3) as usize];
        reader.read_exact(&mut unk_offsets_bytes)?;
        let unk_offsets: &[u64] = bytemuck::cast_slice(&unk_offsets_bytes);
        println!("offsets: {:08x?}", unk_offsets);

        // TEMP HACK
        assert_eq!(unk_offsets.len(), 1);

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
    assert_eq!(0x10, std::mem::size_of::<HEADER>());
}
