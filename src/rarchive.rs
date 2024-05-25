use std::{
    ffi::CStr,
    io::{Cursor, Read, Seek, Write},
    mem::size_of,
    path::Path,
    sync::Mutex,
};

use flate2::{read::ZlibDecoder, write::ZlibEncoder};
use log::{debug, trace};
use zerocopy::{AsBytes, FromBytes, FromZeroes};

use crate::{util, DTI};

const ARCHIVE_MAGIC: u32 = u32::from_be(0x41524300); // "ARC\0"
const ARCHIVE_VERSION: u16 = 7;

const ORGSIZE_MASK: u32 = 2_u32.pow(29) - 1;
const QUALITY_MASK: u32 = 2_u32.pow(3) - 1;

const PATH_MAXLEN: usize = 127;

#[repr(C, packed)]
#[derive(Debug, FromBytes, FromZeroes, AsBytes)]
struct ArchiveHeader {
    magic: u32,
    version: u16,
    num_resources: u16,
}

#[repr(C, packed)]
#[derive(Debug, FromBytes, FromZeroes, AsBytes)]
struct RawResourceInfo {
    path: [u8; PATH_MAXLEN + 1], // + null byte
    dti_type: u32,
    size_compressed: u32,
    // orgsize: 29, quality: 3
    bitfield_orgsize_quality: u32,
    offset: u32,
}

#[derive(Debug)]
pub struct ResourceInfo {
    path: String,
    dti: &'static DTI,
    size_compressed: u32,
    size_uncompressed: u32,

    quality: u32,
    offset: u32,
}
impl ResourceInfo {
    pub fn path(&self) -> &str {
        &self.path
    }

    pub fn dti(&self) -> &'static DTI {
        self.dti
    }

    pub fn quality(&self) -> u32 {
        self.quality
    }
}

pub struct ArchiveFile<Backing: Read + Seek> {
    resources: Vec<ResourceInfo>,
    reader: Box<Mutex<Backing>>,
}

impl<Backing: Read + Seek> ArchiveFile<Backing> {
    pub fn new(mut reader: Backing) -> anyhow::Result<Self> {
        let header: ArchiveHeader = util::read_struct(&mut reader)?;

        debug!("archive header: {:#?}", header);

        assert_eq!({ header.magic }, ARCHIVE_MAGIC);
        assert_eq!({ header.version }, ARCHIVE_VERSION);

        let mut resources = vec![];

        for _ in 0..header.num_resources {
            let raw_resource_info: RawResourceInfo = util::read_struct(&mut reader)?;

            let path = CStr::from_bytes_until_nul(&raw_resource_info.path)?
                .to_string_lossy()
                .to_string();

            let dti = DTI::from_hash(raw_resource_info.dti_type).unwrap();

            let size_compressed = raw_resource_info.size_compressed;
            let size_uncompressed = raw_resource_info.bitfield_orgsize_quality & ORGSIZE_MASK;

            let quality = (raw_resource_info.bitfield_orgsize_quality >> 29) & QUALITY_MASK;

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
                quality,
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

    pub fn get_resource_by_info(&self, info: &ResourceInfo) -> anyhow::Result<Option<Vec<u8>>> {
        self.get_resource(&info.path, info.dti)
    }

    pub fn get_resource_with_path(
        &self,
        path: &Path,
        dti: &DTI,
    ) -> anyhow::Result<Option<Vec<u8>>> {
        let path = path.to_string_lossy().replace("/", "\\");

        self.get_resource(&path, dti)
    }

    pub fn get_resource(&self, path: &str, dti: &DTI) -> anyhow::Result<Option<Vec<u8>>> {
        trace!("getting resource {:?}", path);

        // hashmaps make everything go fast...
        let resource = self
            .resources
            .iter()
            .find(|resource| (resource.path == path) && (resource.dti == dti));

        let resource = if let Some(resource) = resource {
            resource
        } else {
            return Ok(None);
        };

        let mut reader = self.reader.lock().unwrap();

        reader.seek(std::io::SeekFrom::Start(resource.offset as u64))?;

        let mut content_compressed = vec![0u8; resource.size_compressed as usize];
        reader.read_exact(&mut content_compressed)?;

        drop(reader);

        let mut cursor = Cursor::new(&content_compressed);
        let mut decoder = ZlibDecoder::new(&mut cursor);

        let mut content_decompressed: Vec<u8> = vec![];
        let num_decompressed_bytes = decoder.read_to_end(&mut content_decompressed)?;

        assert_eq!(num_decompressed_bytes, resource.size_uncompressed as usize);

        Ok(Some(content_decompressed))
    }
}

// TODO: can this be combined with ResourceInfo
struct ArchiveResourceForWrite {
    path: String,
    quality: u32,
    size_uncompressed: u32,

    compressed_data: Vec<u8>,
    dti: &'static DTI,
}

pub struct ArchiveWriter {
    resources: Vec<ArchiveResourceForWrite>,
}

impl ArchiveWriter {
    pub fn new() -> Self {
        ArchiveWriter { resources: vec![] }
    }

    pub fn add_file(
        &mut self,
        path: &str,
        dti: &'static DTI,
        quality: u32,
        data: &[u8],
    ) -> anyhow::Result<()> {
        let mut encoder = ZlibEncoder::new(Vec::new(), flate2::Compression::default());
        encoder.write_all(data)?;

        let compressed_data = encoder.finish()?;

        let mut decoder = ZlibDecoder::new(Cursor::new(&compressed_data));
        let mut out = vec![];
        decoder.read_to_end(&mut out)?;

        assert_eq!(out.len(), data.len());

        trace!(
            "adding file {} unc {} comp {}",
            path,
            data.len(),
            compressed_data.len()
        );

        self.resources.push(ArchiveResourceForWrite {
            path: path.to_string(),
            quality,
            dti,

            compressed_data,
            size_uncompressed: data
                .len()
                .try_into()
                .expect("resource size doesn't fit into u32"),
        });

        Ok(())
    }

    pub fn save<W: Write>(&self, writer: &mut W) -> anyhow::Result<()> {
        let header = ArchiveHeader {
            magic: ARCHIVE_MAGIC,
            version: ARCHIVE_VERSION,
            num_resources: self.resources.len().try_into().unwrap(),
        };

        writer.write_all(header.as_bytes())?;

        let start_offset =
            size_of::<ArchiveHeader>() + (self.resources.len() * size_of::<RawResourceInfo>());
        let mut offset: u32 = start_offset.try_into().unwrap();

        for resource in &self.resources {
            trace!(
                "writing resource info: path {} comp {} unc {} quality {} dti {}",
                resource.path,
                resource.compressed_data.len(),
                resource.size_uncompressed,
                resource.quality,
                resource.dti.name()
            );

            assert!(resource.size_uncompressed <= ORGSIZE_MASK);
            assert!(resource.quality <= QUALITY_MASK);

            let bitfield_orgsize_quality = (resource.size_uncompressed & ORGSIZE_MASK)
                | ((resource.quality & QUALITY_MASK) << 29);

            let mut path_bytes = resource.path.as_bytes().to_vec();
            assert!(path_bytes.len() <= PATH_MAXLEN);

            path_bytes.resize(PATH_MAXLEN + 1, 0);

            let size_compressed = resource.compressed_data.len().try_into().unwrap();
            let info = RawResourceInfo {
                path: path_bytes.try_into().unwrap(),
                dti_type: resource.dti.hash(),
                size_compressed,
                bitfield_orgsize_quality,
                offset,
            };

            offset += size_compressed;

            writer.write_all(info.as_bytes())?;
        }

        for resource in &self.resources {
            writer.write_all(&resource.compressed_data)?;
        }

        Ok(())
    }
}

#[test]
fn test_struct_sizes() {
    use std::mem::size_of;

    assert_eq!(size_of::<ArchiveHeader>(), 8);
    assert_eq!(size_of::<RawResourceInfo>(), 0x90);
}
