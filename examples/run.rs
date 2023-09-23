fn main() {
    let disk = vhdx::Vhdx::load(std::env::args_os().nth(1).unwrap());
    dbg!(disk);
}
