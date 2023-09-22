use std::io::Read;

fn main() {
    let disk = vhdx::Vhdx::load(std::env::args_os().nth(1).unwrap());
    let mut reader = disk.reader();

    let mut buffer = vec![0; 2048];
    dbg!(reader.read(&mut buffer).unwrap());
    println!("Buffer: {}", String::from_utf8_lossy(&buffer));
}
