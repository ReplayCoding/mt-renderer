use mt_renderer::{rmaterial::MaterialFile, rshader2::Shader2};

fn main() -> anyhow::Result<()> {
    env_logger::init();

    let args: Vec<_> = std::env::args().collect();

    let mut shader_file = std::fs::File::open("/home/user/Desktop/WIN11-vm-folder/TGAAC-for-research/nativeDX11x64/custom_shaders/CustomShaderPackage.mfx")?;
    let shader2 = Shader2::new(&mut shader_file)?;

    let mut file = std::fs::File::open(&args[1])?;
    let material = MaterialFile::new(&mut file, &shader2)?;

    println!("{:#?}", material);

    Ok(())
}
