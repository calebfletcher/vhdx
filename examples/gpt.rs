use std::error::Error;

fn main() -> Result<(), Box<dyn Error>> {
    let disk_path = std::env::args_os().nth(1).unwrap();
    let mut disk = vhdx::Vhdx::load(disk_path)?;
    let mut reader = disk.reader();

    let cfg = gpt::GptConfig::new().writable(false);
    let disk = cfg.open_from_device(Box::new(&mut reader))?;

    println!("Disk header: {:#?}", disk.primary_header());
    println!("Partition layout: {:#?}", disk.partitions());

    Ok(())
}
