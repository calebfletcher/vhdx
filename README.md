# VHDX
[![Latest Version](https://img.shields.io/crates/v/vhdx.svg)](https://crates.io/crates/vhdx)
[![Rust Documentation](https://docs.rs/vhdx/badge.svg)](https://docs.rs/vhdx)
[![Actions Status](https://github.com/calebfletcher/vhdx/workflows/ci/badge.svg)](https://github.com/calebfletcher/vhdx/actions)
[![Unsafe Forbidden](https://img.shields.io/badge/unsafe-forbidden-brightgreen.svg)](https://img.shields.io/badge/unsafe-forbidden-brightgreen.svg)

An implementation of Microsoft's VHDX virtual hard disk format in Rust.

Based on Microsoft's Open Specification available at:
https://learn.microsoft.com/en-us/openspecs/windows_protocols/ms-vhdx

## Usage
```bash
cargo add vhdx
```
```toml
[dependencies]
vhdx = "0.1"
```

## Example
```rust,no_run
use std::io::Read;

let mut disk = vhdx::Vhdx::load("disk.vhdx");
let mut reader = disk.reader();

let mut buffer = [0; 512];
reader.read(&mut buffer).unwrap();
```