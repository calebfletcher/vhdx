use std::{
    fs::File,
    io::{Read, Seek, SeekFrom},
};

use crate::{guid::Guid, Error, KB, MB};

const LOG_ENTRY_SIGNATURE: &str = "loge";
const ZERO_DESCRIPTOR_SIGNATURE: &str = "zero";
const DATA_DESCRIPTOR_SIGNATURE: &str = "desc";
const DATA_SECTOR_SIGNATURE: &str = "data";

#[derive(Debug)]
pub struct LogEntryHeader {
    signature: String,
    checksum: [u8; 4],
    pub entry_length: u32,
    pub tail: u32,
    pub sequence_number: u64,
    descriptor_count: u32,
    log_guid: Guid,
    pub flushed_file_offset: u64,
    last_file_offset: u64,
}

impl LogEntryHeader {
    pub fn read(file: &mut File) -> Result<Self, Error> {
        let mut buffer = vec![0; 64];
        file.read_exact(&mut buffer)?;

        let signature = String::from_utf8(buffer[0..4].to_vec()).unwrap();
        let checksum = buffer[4..8].try_into().unwrap();
        let entry_length = u32::from_le_bytes(buffer[8..12].try_into().unwrap());
        let tail = u32::from_le_bytes(buffer[12..16].try_into().unwrap());
        let sequence_number = u64::from_le_bytes(buffer[16..24].try_into().unwrap());
        let descriptor_count = u32::from_le_bytes(buffer[24..28].try_into().unwrap());
        let log_guid = Guid::from_bytes(buffer[32..48].try_into().unwrap());
        let flushed_file_offset = u64::from_le_bytes(buffer[48..56].try_into().unwrap());
        let last_file_offset = u64::from_le_bytes(buffer[56..64].try_into().unwrap());

        if signature != LOG_ENTRY_SIGNATURE {
            return Err(Error::InvalidSignature);
        }
        assert_eq!(entry_length % (4 * KB as u32), 0);
        assert_eq!(tail % (4 * KB as u32), 0);
        assert!(sequence_number > 0);
        assert_eq!(flushed_file_offset % MB as u64, 0);
        assert_eq!(last_file_offset % MB as u64, 0);

        Ok(Self {
            signature,
            checksum,
            entry_length,
            tail,
            sequence_number,
            descriptor_count,
            log_guid,
            flushed_file_offset,
            last_file_offset,
        })
    }

    pub fn log_guid(&self) -> Guid {
        self.log_guid
    }
}

#[derive(Debug)]
pub enum Descriptor {
    Zero(ZeroDescriptor),
    Data(DataDescriptor),
}

#[derive(Debug)]
pub struct ZeroDescriptor {
    signature: String,
    zero_length: u64,
    file_offset: u64,
    sequence_number: u64,
}

impl ZeroDescriptor {
    pub fn read(file: &mut File) -> Result<Self, Error> {
        let mut buffer = vec![0; 32];
        file.read_exact(&mut buffer)?;

        let signature = String::from_utf8(buffer[0..4].to_vec()).unwrap();
        let zero_length = u64::from_le_bytes(buffer[8..16].try_into().unwrap());
        let file_offset = u64::from_le_bytes(buffer[16..24].try_into().unwrap());
        let sequence_number = u64::from_le_bytes(buffer[24..32].try_into().unwrap());

        assert_eq!(signature, ZERO_DESCRIPTOR_SIGNATURE);
        assert_eq!(zero_length % (4 * KB as u64), 0);
        assert_eq!(file_offset % (4 * KB as u64), 0);

        Ok(Self {
            signature,
            zero_length,
            file_offset,
            sequence_number,
        })
    }

    pub fn zero_length(&self) -> u64 {
        self.zero_length
    }

    pub fn file_offset(&self) -> u64 {
        self.file_offset
    }

    pub fn sequence_number(&self) -> u64 {
        self.sequence_number
    }
}

#[derive(Debug)]
pub struct DataDescriptor {
    signature: String,
    trailing_bytes: [u8; 4],
    leading_bytes: [u8; 8],
    file_offset: u64,
    sequence_number: u64,
}

impl DataDescriptor {
    pub fn read(file: &mut File) -> Result<Self, Error> {
        let mut buffer = vec![0; 32];
        file.read_exact(&mut buffer)?;

        let signature = String::from_utf8(buffer[0..4].to_vec()).unwrap();
        let trailing_bytes = buffer[4..8].try_into().unwrap();
        let leading_bytes = buffer[8..16].try_into().unwrap();
        let file_offset = u64::from_le_bytes(buffer[16..24].try_into().unwrap());
        let sequence_number = u64::from_le_bytes(buffer[24..32].try_into().unwrap());

        assert_eq!(signature, DATA_DESCRIPTOR_SIGNATURE);
        assert_eq!(file_offset % (4 * KB as u64), 0);

        Ok(Self {
            signature,
            trailing_bytes,
            leading_bytes,
            file_offset,
            sequence_number,
        })
    }

    pub fn sequence_number(&self) -> u64 {
        self.sequence_number
    }

    pub fn file_offset(&self) -> u64 {
        self.file_offset
    }

    pub fn trailing_bytes(&self) -> [u8; 4] {
        self.trailing_bytes
    }

    pub fn leading_bytes(&self) -> [u8; 8] {
        self.leading_bytes
    }
}

pub struct DataSector {
    signature: String,
    sequence_high: u32,
    data: Box<[u8; 4084]>,
    sequence_low: u32,
}

impl std::fmt::Debug for DataSector {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DataSector")
            .field("signature", &self.signature)
            .field("sequence_high", &self.sequence_high)
            .field("sequence_low", &self.sequence_low)
            .finish()
    }
}

impl DataSector {
    pub fn read(file: &mut File) -> Result<Self, Error> {
        let mut buffer = vec![0; 4096];
        file.read_exact(&mut buffer)?;

        let signature = String::from_utf8(buffer[0..4].to_vec()).unwrap();
        let sequence_high = u32::from_le_bytes(buffer[4..8].try_into().unwrap());
        let data = Box::<[u8]>::from(&buffer[8..4092]).try_into().unwrap();
        let sequence_low = u32::from_le_bytes(buffer[4092..4096].try_into().unwrap());

        assert_eq!(signature, DATA_SECTOR_SIGNATURE);

        Ok(Self {
            signature,
            sequence_high,
            data,
            sequence_low,
        })
    }

    pub fn data(&self) -> &[u8; 4084] {
        self.data.as_ref()
    }

    pub fn sequence_high(&self) -> u32 {
        self.sequence_high
    }

    pub fn sequence_low(&self) -> u32 {
        self.sequence_low
    }
}

#[derive(Debug)]
pub struct Entry {
    header: LogEntryHeader,
    descriptors: Vec<Descriptor>,
    data_sectors: Vec<DataSector>,
}

impl Entry {
    /// File cursor will be at the end of the entry after this function
    pub fn read(file: &mut File) -> Result<Self, Error> {
        let original_position = file.stream_position()?;

        let header = LogEntryHeader::read(file)?;
        let mut descriptors = Vec::with_capacity(header.descriptor_count as usize);
        let mut data_sectors = Vec::with_capacity(header.descriptor_count as usize);

        for _ in 0..header.descriptor_count {
            let mut buffer = vec![0; 4];
            file.read_exact(&mut buffer)?;
            let signature = std::str::from_utf8(&buffer[0..4]).unwrap();

            file.seek(std::io::SeekFrom::Current(-4))?;

            let descriptor: Descriptor = match signature {
                ZERO_DESCRIPTOR_SIGNATURE => {
                    let descriptor = ZeroDescriptor::read(file)?;
                    Descriptor::Zero(descriptor)
                }
                DATA_DESCRIPTOR_SIGNATURE => {
                    let descriptor = DataDescriptor::read(file)?;
                    Descriptor::Data(descriptor)
                }
                _ => Err(Error::InvalidSignature)?,
            };

            descriptors.push(descriptor);
        }

        // Align position to the next 4KB boundary
        let current_position = file.stream_position()?;
        file.seek(SeekFrom::Start(next_multiple_of(
            current_position,
            4 * KB as u64,
        )))?;

        let num_data_sectors = descriptors
            .iter()
            .filter(|desc| matches!(desc, Descriptor::Data(_)))
            .count();
        // println!(
        //     "entry had {} descriptors, with {} data sectors",
        //     descriptors.len(),
        //     num_data_sectors
        // );

        // Read all the data sectors, in order
        for _ in 0..num_data_sectors {
            data_sectors.push(DataSector::read(file)?);
        }

        // After reading the data sectors, the file position should be after the end of the entry
        let current_position = file.stream_position()?;
        assert_eq!(
            current_position,
            original_position + header.entry_length as u64
        );

        Ok(Self {
            header,
            descriptors,
            data_sectors,
        })
    }

    pub fn header(&self) -> &LogEntryHeader {
        &self.header
    }

    pub fn descriptors(&self) -> &[Descriptor] {
        self.descriptors.as_ref()
    }

    pub fn data_sectors(&self) -> &[DataSector] {
        self.data_sectors.as_ref()
    }
}

pub const fn next_multiple_of(value: u64, rhs: u64) -> u64 {
    let r = value % rhs;

    if r == 0 {
        value
    } else {
        value + (rhs - r)
    }
}
