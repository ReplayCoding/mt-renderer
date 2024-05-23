use std::path::{Path, PathBuf};

use mt_renderer::rarchive::{ArchiveFile, ArchiveWriter};

fn unpack(archive_path: &Path) -> anyhow::Result<()> {
    let file = Box::new(std::fs::File::open(&archive_path)?);
    let archive = ArchiveFile::new(file)?;

    let out_dir_name = PathBuf::from(archive_path.file_stem().unwrap());

    for resource in archive.resource_infos() {
        println!(
            "Extracting {:?} ({})",
            resource.path(),
            resource.dti().name()
        );

        let data = archive.get_resource_by_info(resource)?.unwrap();
        let out_path = out_dir_name.join(
            resource
                .path()
                .with_extension(resource.dti().file_ext().expect("DTI doesn't have an ext")),
        );

        std::fs::create_dir_all(out_path.parent().unwrap())?;
        std::fs::write(out_path, data)?;
    }

    Ok(())
}

fn repack(archive_path: &Path) -> anyhow::Result<()> {
    let in_file = Box::new(std::fs::File::open(&archive_path)?);
    let mut out_file = std::fs::File::create(&PathBuf::from("test.arc"))?;

    let archive = ArchiveFile::new(in_file)?;
    let mut archive_writer = ArchiveWriter::new();

    for resource_info in archive.resource_infos() {
        let resource = archive
            .get_resource_by_info(resource_info)?
            .expect("resource in archive is not available? what!?");

        archive_writer.add_file(
            &resource_info.path().to_str().unwrap().replace("/", "\\"),
            resource_info.dti(),
            resource_info.quality(),
            &resource,
        )?;
    }

    archive_writer.save(&mut out_file)?;
    Ok(())
}

fn main() -> anyhow::Result<()> {
    env_logger::init();

    let args: Vec<_> = std::env::args().collect();
    let archive_path = PathBuf::from(&args[2]);

    match args[1].as_str() {
        "unpack" => unpack(&archive_path),
        "pack" => repack(&archive_path),

        unknown => panic!("unhandled command: {}", unknown),
    }?;

    Ok(())
}
