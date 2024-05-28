use mt_renderer::rguimessage::GuiMessageFile;

fn main() -> anyhow::Result<()> {
    env_logger::init();

    let args: Vec<_> = std::env::args().collect();
    let mut file = std::fs::File::open(&args[1])?;
    let gmd = GuiMessageFile::new(&mut file)?;

    println!("{}", serde_json::to_string_pretty(&gmd)?);

    Ok(())
}
