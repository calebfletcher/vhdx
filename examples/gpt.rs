fn main() {
    let disk = vhdx::Vhdx::load(std::env::args_os().nth(1).unwrap());
    let mut reader = disk.reader();

    let cfg = gpt::GptConfig::new().writable(false);
    let disk = cfg.open_from_device(Box::new(&mut reader)).unwrap();

    println!("Disk header: {:#?}", disk.primary_header());
    println!("Partition layout: {:#?}", disk.partitions());
}
