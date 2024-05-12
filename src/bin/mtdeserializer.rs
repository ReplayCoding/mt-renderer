use mt_renderer::{dti, mtserializer::MtSerializer};

fn main() -> anyhow::Result<()> {
    env_logger::init();

    let args: Vec<_> = std::env::args().collect();

    let mut file = std::fs::File::open(&args[1])?;
    let deserialized = MtSerializer::deserialize(&mut file)?;

    println!("{:#?}", deserialized);

    Ok(())
}
