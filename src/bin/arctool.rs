use std::path::PathBuf;

use mt_renderer::rarchive::ArchiveFile;

fn main() -> anyhow::Result<()> {
    env_logger::init();

    let args: Vec<_> = std::env::args().collect();
    let archive_path = PathBuf::from(&args[1]);
    let file = Box::new(std::fs::File::open(&archive_path)?);
    let archive = ArchiveFile::new(file)?;

    let out_dir_name = PathBuf::from(archive_path.file_stem().unwrap());

    for resource in archive.resource_infos() {
        println!(
            "Extracting {:?} ({})",
            resource.path(),
            resource.dti().name()
        );

        let data = archive.get_resource_by_info(resource).unwrap();
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
