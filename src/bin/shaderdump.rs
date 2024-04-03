use mt_renderer::rshader2::Shader2;

fn main() ->anyhow::Result<()> {
    env_logger::init();

    let args: Vec<_> = std::env::args().collect();
    let mut file = std::fs::File::open(&args[1])?;
    let shader2 = Shader2::new(&mut file)?;

    for object in shader2.objects() {
        println!("{:#?}", object);
    }

    Ok(())
}
