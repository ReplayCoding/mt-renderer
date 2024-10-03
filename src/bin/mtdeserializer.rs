use mt_renderer::mtserializer::{self, prp_file_to_mtserializer};

fn main() -> anyhow::Result<()> {
    env_logger::init();

    let args: Vec<_> = std::env::args().collect();

    let mut file = std::fs::File::open(&args[1])?;

    let mut file_cursor = prp_file_to_mtserializer(&mut file)?;
    let deserialized = mtserializer::deserialize(&mut file_cursor)?;

    println!("{:#?}", deserialized);

    Ok(())
}
