use std::{ffi::OsString, path::PathBuf};

use mt_renderer::{rarchive::cli_util::unpack_archive, DTIs};
use walkdir::WalkDir;

fn main() -> anyhow::Result<()> {
    env_logger::init();

    let args: Vec<_> = std::env::args().collect();

    let game_root = PathBuf::from(&args[1]);

    let arc_extension = OsString::from(DTIs::rArchive.file_ext().unwrap());
    let walker = WalkDir::new(game_root).into_iter();

    for file in walker {
        let file = file?;

        if !(file.file_type().is_file() && file.path().extension() == Some(&arc_extension)) {
            continue;
        }


        let in_path = file.path().to_path_buf();
        let out_dir = in_path.with_file_name(in_path.file_stem().unwrap());

        println!("unpacking {:?} to {:?}...", in_path, out_dir);

        assert!(!out_dir.exists());
        std::fs::create_dir(&out_dir)?;

        unpack_archive(&in_path, &out_dir)?;

        std::fs::remove_file(in_path)?;
    }

    Ok(())
}
