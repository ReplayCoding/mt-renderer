use std::io::{Read, Seek};

use mt_renderer::mtserializer;

fn main() -> anyhow::Result<()> {
    env_logger::init();

    let args: Vec<_> = std::env::args().collect();

    let mut file = std::fs::File::open(&args[1])?;

    let mut magic_bytes = [0u8; 4];
    file.read_exact(&mut magic_bytes)?;
    let is_propparam = magic_bytes == "PRPZ".as_bytes();

    file.seek(std::io::SeekFrom::Start(0))?;

    let mut file_data = vec![];
    file.read_to_end(&mut file_data)?;

    let mut file_cursor = if is_propparam {
        std::io::Cursor::new(&file_data[12..]) // past header
    } else {
        std::io::Cursor::new(&file_data[..])
    };

    let deserialized = mtserializer::deserialize(&mut file_cursor)?;

    println!("{:#?}", deserialized);

    Ok(())
}
