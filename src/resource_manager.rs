use std::{
    collections::HashMap,
    fs::File,
    io::{Cursor, Read, Seek},
    path::{Path, PathBuf},
};

use crate::{rarchive::ArchiveFile, DTIs, DTI};
use anyhow::anyhow;
use log::trace;

enum ResourceInner {
    FileBacked(File),
    ArchiveBacked(Cursor<Vec<u8>>),
}

pub struct Resource(ResourceInner);

impl Read for Resource {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        match &mut self.0 {
            ResourceInner::FileBacked(ref mut r) => r.read(buf),
            ResourceInner::ArchiveBacked(ref mut r) => r.read(buf),
        }
    }
}

impl Seek for Resource {
    fn seek(&mut self, pos: std::io::SeekFrom) -> std::io::Result<u64> {
        match &mut self.0 {
            ResourceInner::FileBacked(ref mut r) => r.seek(pos),
            ResourceInner::ArchiveBacked(ref mut r) => r.seek(pos),
        }
    }
}

pub struct ResourceManager {
    base_path: PathBuf,
    loaded_archives: HashMap<PathBuf, ArchiveFile<File>>,
}

impl ResourceManager {
    pub fn new(base_path: &Path) -> Self {
        Self {
            base_path: base_path.to_path_buf(),
            loaded_archives: HashMap::new(),
        }
    }

    pub fn add_archive(&mut self, path: &Path) -> anyhow::Result<()> {
        if self.loaded_archives.contains_key(path) {
            return Ok(());
        }

        let file = std::fs::File::open(
            &self
                .base_path
                .join(path.with_extension(DTIs::rArchive.file_ext().unwrap())),
        )?;

        let archive = ArchiveFile::new(file)?;

        self.loaded_archives.insert(path.to_path_buf(), archive);

        Ok(())
    }

    /// Terrible name. If the path is formatted as "<archive>:<path>", then load
    /// the resource from that archive
    pub fn get_resource_fancy(&mut self, path: &str, dti: &DTI) -> anyhow::Result<Resource> {
        let (archive_path, path): (Option<&str>, &str) = path
            .split_once(":")
            .map(|(a, p)| (Some(a), p))
            .or_else(|| Some((None, path)))
            .unwrap();

        if let Some(archive_path) = archive_path {
            self.add_archive(&PathBuf::from(archive_path))?;
        }

        self.get_resource(&PathBuf::from(path), dti)
    }

    pub fn get_resource(&self, path: &Path, dti: &DTI) -> anyhow::Result<Resource> {
        let file_ext = dti
            .file_ext()
            .ok_or_else(|| anyhow!("DTI {} doesn't have a file extension", dti.name(),))?;

        let fs_path = &self.base_path.join(path.with_extension(file_ext));
        trace!(
            "Attempting to load resource {:?} ({}) from file path {:?}",
            path,
            dti.name(),
            fs_path
        );
        let file = std::fs::File::open(fs_path);

        if let Ok(file) = file {
            Ok(Resource(ResourceInner::FileBacked(file)))
        } else {
            for (_archive_path, archive) in &self.loaded_archives {
                if let Some(resource_data) = archive.get_resource(path, dti) {
                    return Ok(Resource(ResourceInner::ArchiveBacked(Cursor::new(
                        resource_data,
                    ))));
                }
            }

            Err(anyhow!(
                "Couldn't find resource {:?} ({})",
                fs_path,
                dti.name()
            ))
        }
    }
}
