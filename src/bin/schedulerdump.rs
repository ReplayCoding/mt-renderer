use mt_renderer::rscheduler::SchedulerFile;

fn main() -> anyhow::Result<()> {
    env_logger::init();

    let args: Vec<_> = std::env::args().collect();
    let mut file = std::fs::File::open(&args[1])?;
    let scheduler = SchedulerFile::new(&mut file)?;

    println!("{:#?}", scheduler);

    Ok(())
}

