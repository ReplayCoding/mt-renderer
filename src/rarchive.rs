use std::{
    ffi::{CStr, OsString},
    io::{Cursor, Read, Seek},
    mem::size_of,
    path::{Path, PathBuf},
    sync::Mutex,
};

use flate2::read::ZlibDecoder;
use log::{debug, trace};

use crate::DTI;

#[repr(C, packed)]
#[derive(bytemuck::Pod, bytemuck::Zeroable, Debug, Clone, Copy)]
struct ArchiveHeader {
    magic: u32,
    version: u16,
    num_resources: u16,
}

#[repr(C, packed)]
#[derive(bytemuck::Pod, bytemuck::Zeroable, Debug, Clone, Copy)]
struct RawResourceInfo {
    path: [u8; 128],
    dti_type: u32,
    size_compressed: u32,
    // orgsize: 29, quality: 3
    bitfield_orgsize_quality: u32,
    offset: u32,
}

#[derive(Debug)]
pub struct ResourceInfo {
    path: PathBuf,
    dti: &'static DTI,
    size_compressed: u32,
    size_uncompressed: u32,
    offset: u32,

    // TODO: what exactly does this do?
    _quality: u8,
}
impl ResourceInfo {
    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn dti(&self) -> &DTI {
        self.dti
    }
}

pub struct ArchiveFile<Backing: Read + Seek> {
    resources: Vec<ResourceInfo>,
    reader: Box<Mutex<Backing>>,
}

impl<Backing: Read + Seek> ArchiveFile<Backing> {
    pub fn new(mut reader: Backing) -> anyhow::Result<Self> {
        let mut header_bytes = [0u8; size_of::<ArchiveHeader>()];
        reader.read_exact(&mut header_bytes)?;
        let header: &ArchiveHeader = bytemuck::from_bytes(&header_bytes);

        debug!("archive header: {:#?}", header);

        assert_eq!(header.magic.to_ne_bytes(), "ARC\0".as_bytes());

        let mut resources: Vec<ResourceInfo> = vec![];

        for _ in 0..header.num_resources {
            let mut raw_resource_info_bytes = [0u8; size_of::<RawResourceInfo>()];
            reader.read_exact(&mut raw_resource_info_bytes)?;
            let raw_resource_info: &RawResourceInfo =
                bytemuck::from_bytes(&raw_resource_info_bytes);

            let path = PathBuf::from(OsString::from(
                CStr::from_bytes_until_nul(&raw_resource_info.path)?
                    .to_string_lossy()
                    .to_string() // lol
                    .replace("\\", "/"),
            ));

            let dti = DTI::from_hash(raw_resource_info.dti_type).unwrap();

            let size_compressed = raw_resource_info.size_compressed;
            let size_uncompressed =
                raw_resource_info.bitfield_orgsize_quality & (2_u32.pow(29) - 1);

            let quality = (raw_resource_info.bitfield_orgsize_quality >> 29) & (2_u32.pow(3) - 1);

            let offset = raw_resource_info.offset;

            trace!(
                "resource: path {:?} dti {} size [c {} u {}] quality {} offset {:08x}",
                path,
                dti.name(),
                size_compressed,
                size_uncompressed,
                quality,
                offset
            );

            resources.push(ResourceInfo {
                path,
                dti,
                size_compressed,
                size_uncompressed,
                _quality: quality as u8, // only 3 bits, so it's fine
                offset,
            })
        }

        Ok(Self {
            resources,
            reader: Box::from(Mutex::from(reader)),
        })
    }

    pub fn resource_infos(&self) -> &[ResourceInfo] {
        &self.resources
    }

    pub fn get_resource_by_info(&self, info: &ResourceInfo) -> Option<Vec<u8>> {
        self.get_resource(&info.path, info.dti)
    }

    pub fn get_resource(&self, path: &Path, dti: &DTI) -> Option<Vec<u8>> {
        // hashmaps make everything go fast...
        let resource = self
            .resources
            .iter()
            .find(|resource| (resource.path == path) && (resource.dti == dti))?;

        let mut reader = self.reader.lock().unwrap();

        reader
            .seek(std::io::SeekFrom::Start(resource.offset.into()))
            .unwrap();

        let mut content_compressed = vec![0u8; resource.size_compressed as usize];
        reader.read_exact(&mut content_compressed).unwrap();

        let mut cursor = Cursor::new(&content_compressed);
        let mut decoder = ZlibDecoder::new(&mut cursor);

        let mut content_decompressed: Vec<u8> = vec![];
        let num_decompressed_bytes = decoder.read_to_end(&mut content_decompressed).unwrap();

        assert_eq!(num_decompressed_bytes, resource.size_uncompressed as usize);

        Some(content_decompressed)
    }
}

#[test]
fn test_struct_sizes() {
    assert_eq!(size_of::<ArchiveHeader>(), 8);
    assert_eq!(size_of::<RawResourceInfo>(), 0x90);
}
