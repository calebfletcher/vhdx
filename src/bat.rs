use std::{fs::File, io::Read};

#[derive(Debug)]
pub enum PayloadBatEntryState {
    NotPresent,
    Undefined,
    Zero,
    Unmapped,
    FullyPresent,
    PartiallyPresent,
}

impl PayloadBatEntryState {
    fn from_bits(value: u8) -> Self {
        match value {
            0 => PayloadBatEntryState::NotPresent,
            1 => PayloadBatEntryState::Undefined,
            2 => PayloadBatEntryState::Zero,
            3 => PayloadBatEntryState::Unmapped,
            6 => PayloadBatEntryState::FullyPresent,
            7 => PayloadBatEntryState::PartiallyPresent,
            _ => panic!("unknown state: {value:b}"),
        }
    }
}

#[derive(Debug)]
pub struct BatEntry {
    state: PayloadBatEntryState,
    file_offset: u64,
}

impl BatEntry {
    fn read(file: &mut File) -> Self {
        let mut buffer = vec![0; 8];
        file.read_exact(&mut buffer).unwrap();

        let value = u64::from_le_bytes(buffer.try_into().unwrap());
        let state = PayloadBatEntryState::from_bits(value as u8 & 0b111);

        let mask = 0xFFFFFFFFFFF00000;
        let file_offset = value & mask;

        Self { state, file_offset }
    }
}

#[derive(Debug)]
pub struct Bat {
    entries: Vec<BatEntry>,
}

impl Bat {
    pub fn read(file: &mut File, metadata: &crate::Metadata) -> Self {
        let virt_disk_size = metadata.virtual_disk_size.virtual_disk_size();
        let logical_sector_size = metadata.logical_sector_size.logical_sector_size();
        let block_size = metadata.file_parameters.block_size();

        let chunk_ratio = (2 << 23) * logical_sector_size as u64 / block_size as u64;

        let payload_blocks_count = div_ceil(virt_disk_size, block_size as u64);

        let total_bat_entries =
            payload_blocks_count + div_floor(payload_blocks_count - 1, chunk_ratio);

        let entries = (0..total_bat_entries)
            .map(|_| BatEntry::read(file))
            .collect();

        Self { entries }
    }
}

const fn div_floor(dividend: u64, divisor: u64) -> u64 {
    dividend / divisor
}

const fn div_ceil(dividend: u64, divisor: u64) -> u64 {
    let d = dividend / divisor;
    let r = dividend % divisor;
    if r > 0 && divisor > 0 {
        d + 1
    } else {
        d
    }
}
