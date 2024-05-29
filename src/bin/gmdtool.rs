use mt_renderer::rguimessage::GuiMessageFile;

fn main() -> anyhow::Result<()> {
    env_logger::init();

    let args: Vec<_> = std::env::args().collect();

    match args[1].as_str() {
        "dump" => {
            let mut file = std::fs::File::open(&args[2])?;
            let gmd = GuiMessageFile::new(&mut file)?;

            println!("{}", serde_json::to_string_pretty(&gmd)?);
        }
        "rebuild" => {
            let in_file = std::fs::File::open(&args[2])?;
            let gmd: GuiMessageFile = serde_json::from_reader(in_file)?;

            let mut out_file = std::fs::File::create("out.gmd")?;
            gmd.save(&mut out_file)?;
        }

        unhandled => panic!("unhandled option: {unhandled}"),
    }

    Ok(())
}
