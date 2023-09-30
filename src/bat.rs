use std::{fs::File, io::Read};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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

    pub fn file_offset(&self) -> u64 {
        self.file_offset
    }

    pub fn state(&self) -> PayloadBatEntryState {
        self.state
    }
}

#[derive(Debug)]
pub struct Bat {
    block_size: u64,
    chunk_ratio: u64,
    entries: Vec<BatEntry>,
}

impl Bat {
    pub(crate) fn read(file: &mut File, metadata: &crate::Metadata) -> Self {
        let virt_disk_size = metadata.virtual_disk_size.virtual_disk_size();
        let logical_sector_size = metadata.logical_sector_size.logical_sector_size();
        let block_size = metadata.file_parameters.block_size() as u64;
        let chunk_ratio = (2 << 23) * logical_sector_size as u64 / block_size;
        let payload_blocks_count = div_ceil(virt_disk_size, block_size);
        let total_bat_entries = payload_blocks_count + (payload_blocks_count - 1) / chunk_ratio;

        if total_bat_entries - payload_blocks_count != 0 {
            unimplemented!("sector bitmap blocks");
        }

        let entries = (0..total_bat_entries)
            .map(|_| BatEntry::read(file))
            .collect();

        Self {
            block_size,
            chunk_ratio,
            entries,
        }
    }

    /// Get the associated entry for a given disk offset.
    ///
    /// Returns both the entry that contains the offset, as well as the offset
    /// within that entry.
    pub fn offset_to_entry(&self, offset: u64) -> (&BatEntry, u64) {
        let payload_block_index = offset / self.block_size;
        let sector_bitmap_blocks = payload_block_index / self.chunk_ratio;
        let bat_index = payload_block_index + sector_bitmap_blocks;
        let entry = self.entries.get(bat_index as usize).unwrap();
        let base_address = payload_block_index * self.block_size;
        (entry, offset - base_address)
    }
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
