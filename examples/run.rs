use std::error::Error;

fn main() -> Result<(), Box<dyn Error>> {
    let disk_path = std::env::args_os().nth(1).unwrap();
    let disk = vhdx::Vhdx::load(disk_path)?;
    dbg!(disk);
    Ok(())
}
