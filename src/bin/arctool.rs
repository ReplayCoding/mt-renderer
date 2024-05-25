use std::path::{Path, PathBuf};

use mt_renderer::{
    rarchive::{ArchiveFile, ArchiveWriter},
    DTI,
};

#[derive(serde::Serialize, serde::Deserialize)]
struct FileInfo {
    path: String,
    dti: String,
    quality: u32,
}

const FILE_INFO_PATH_NAME: &str = "info.json";

fn unpack(archive_path: &Path) -> anyhow::Result<()> {
    let file = Box::new(std::fs::File::open(&archive_path)?);
    let archive = ArchiveFile::new(file)?;

    let out_dir_name = PathBuf::from(archive_path.file_stem().unwrap());
    std::fs::create_dir(&out_dir_name)?;

    let mut file_infos = vec![];

    for resource in archive.resource_infos() {
        println!(
            "Extracting {:?} ({})",
            resource.path(),
            resource.dti().name()
        );

        let data = archive.get_resource_by_info(resource)?.unwrap();
        let out_path = out_dir_name.join(
            PathBuf::from(resource.path().replace("\\", "/"))
                .with_extension(resource.dti().file_ext().expect("DTI doesn't have an ext")),
        );

        std::fs::create_dir_all(out_path.parent().unwrap())?;
        std::fs::write(out_path, data)?;

        file_infos.push(FileInfo {
            path: resource.path().to_string(),
            dti: resource.dti().name().to_string(),
            quality: resource.quality(),
        });
    }

    std::fs::write(
        out_dir_name.join(FILE_INFO_PATH_NAME),
        serde_json::to_string_pretty(&file_infos)?.as_bytes(),
    )?;

    Ok(())
}

fn repack(archive_path: &Path) -> anyhow::Result<()> {
    let file_infos: Vec<FileInfo> = serde_json::from_reader(std::fs::File::open(
        &archive_path.join(FILE_INFO_PATH_NAME),
    )?)?;

    let mut out_file = std::fs::File::create(&PathBuf::from("test.arc"))?;
    let mut archive_writer = ArchiveWriter::new();

    for info in file_infos.iter() {
        let dti = DTI::from_str(&info.dti).expect("invalid dti");

        let fs_path = archive_path
            .join(info.path.replace("\\", "/"))
            .with_extension(dti.file_ext().expect("dti doesn't have file ext"));

        let data = std::fs::read(fs_path)?;

        archive_writer.add_file(&info.path, dti, info.quality, &data)?;
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
    }
    .unwrap();

    Ok(())
}
