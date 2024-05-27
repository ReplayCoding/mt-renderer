use std::path::PathBuf;

use mt_renderer::rarchive::cli_util::{repack_archive, unpack_archive};

fn main() -> anyhow::Result<()> {
    env_logger::init();

    let args: Vec<_> = std::env::args().collect();
    let path = PathBuf::from(&args[2]);

    match args[1].as_str() {
        "unpack" => {
            let out_dir = PathBuf::from(path.file_stem().unwrap());
            std::fs::create_dir(&out_dir)?;

            unpack_archive(&path, &out_dir)
        }
        "pack" => repack_archive(&path),

        unknown => panic!("unhandled command: {}", unknown),
    }
    .unwrap();

    Ok(())
}
