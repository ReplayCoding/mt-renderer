use std::path::PathBuf;

use mt_renderer::{
    resource_manager::ResourceManager, rmaterial::MaterialFile, rshader2::Shader2File, DTIs,
};

fn main() -> anyhow::Result<()> {
    env_logger::init();

    let args: Vec<_> = std::env::args().collect();
    let mut resource_manager = ResourceManager::new(&PathBuf::from(&args[1]));

    let mut shader_file = resource_manager.get_resource(
        &PathBuf::from("custom_shaders/CustomShaderPackage"),
        &DTIs::rShader2,
    )?;
    let shader2 = Shader2File::new(&mut shader_file)?;

    let mut file = resource_manager.get_resource_fancy(&args[2], &DTIs::rMaterial)?;
    let material = MaterialFile::new(&mut file, &shader2)?;

    println!("{:#?}", material);

    Ok(())
}
