#![doc = include_str!("../README.md")]
#![forbid(unsafe_code)]
#![allow(dead_code)]

use std::{
    fs::File,
    io::{Read, Seek, SeekFrom, Write},
    path::Path,
};
use thiserror::Error;

use metadata::MetadataItem;

use crate::guid::Guid;

mod bat;
mod guid;
mod log;
mod metadata;

static FILE_SIGNATURE: &str = "vhdxfile";
static HEADER_SIGNATURE: &str = "head";
static REGION_TABLE_SIGNATURE: &str = "regi";
static METADATA_TABLE_SIGNATURE: &str = "metadata";

static REGION_GUID_BAT: Guid = Guid::from_str("2DC27766-F623-4200-9D64-115E9BFD4A08");
static REGION_GUID_METADATA: Guid = Guid::from_str("8B7CA206-4790-4B9A-B8FE-575F050F886E");

const KB: usize = 1024;
const MB: usize = KB * KB;

static ZEROS: [u8; 4 * KB] = [0; 4 * KB];

#[derive(Error, Debug)]
pub enum Error {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("invalid signature")]
    InvalidSignature,
}

#[derive(Debug)]
struct FileTypeIdentifier {
    signature: String,
    creator: String,
}

impl FileTypeIdentifier {
    /// Read a file type identifier from the current position in the file,
    /// advancing the file to beyond the file type identifier.
    fn read(file: &mut File) -> Result<Self, Error> {
        let mut buffer = vec![0; KB];
        file.read_exact(&mut buffer)?;
        let signature = String::from_utf8_lossy(&buffer[..8]).into_owned();
        assert_eq!(signature, FILE_SIGNATURE);

        let creator_iter = buffer[8..(8 + 512)]
            .chunks_exact(2)
            .map(|bytes| u16::from_le_bytes(bytes.try_into().unwrap()))
            .take_while(|&ch| ch != 0);
        let creator = char::decode_utf16(creator_iter)
            .collect::<Result<String, _>>()
            .unwrap();

        Ok(Self { signature, creator })
    }
}

#[derive(Debug)]
struct Header {
    signature: String,
    checksum: [u8; 4],
    sequence_number: u64,
    file_write_guid: Guid,
    data_write_guid: Guid,
    log_guid: Guid,
    log_version: u16,
    version: u16,
    log_length: u32,
    log_offset: u64,
}

impl Header {
    /// Read a header from the current position in the file, advancing the
    /// file to beyond the header.
    fn read(file: &mut File) -> Result<Self, Error> {
        let mut buffer = vec![0; 128];
        file.read_exact(&mut buffer)?;

        let signature = String::from_utf8(buffer[0..4].to_vec()).unwrap();
        let checksum = buffer[4..8].try_into().expect("infallible");
        let sequence_number = u64::from_le_bytes(buffer[8..16].try_into().expect("infallible"));

        let file_write_guid = Guid::from_bytes(buffer[16..32].try_into().expect("infallible"));
        let data_write_guid = Guid::from_bytes(buffer[32..48].try_into().expect("infallible"));
        let log_guid = Guid::from_bytes(buffer[48..64].try_into().expect("infallible"));

        let log_version = u16::from_le_bytes(buffer[64..66].try_into().expect("infallible"));
        let version = u16::from_le_bytes(buffer[66..68].try_into().expect("infallible"));
        let log_length = u32::from_le_bytes(buffer[68..72].try_into().expect("infallible"));
        let log_offset = u64::from_le_bytes(buffer[72..80].try_into().expect("infallible"));

        assert_eq!(signature, HEADER_SIGNATURE);
        assert_eq!(log_version, 0);
        assert_eq!(version, 1);
        assert_eq!(log_length % MB as u32, 0);
        assert_eq!(log_offset % MB as u64, 0);

        Ok(Self {
            signature,
            checksum,
            sequence_number,
            data_write_guid,
            file_write_guid,
            log_guid,
            log_version,
            version,
            log_length,
            log_offset,
        })
    }
}

#[derive(Debug)]
struct RegionTableEntry {
    guid: Guid,
    file_offset: u64,
    length: u32,
    required: u32,
}

impl RegionTableEntry {
    fn read(file: &mut File) -> Result<Self, Error> {
        let mut buffer = vec![0; 32];
        file.read_exact(&mut buffer)?;

        let guid = Guid::from_bytes(buffer[0..16].try_into().expect("infallible"));
        let file_offset = u64::from_le_bytes(buffer[16..24].try_into().expect("infallible"));
        let length = u32::from_le_bytes(buffer[24..28].try_into().expect("infallible"));
        let required = u32::from_le_bytes(buffer[28..32].try_into().expect("infallible"));

        assert_eq!(file_offset % MB as u64, 0);
        assert!(file_offset > MB as u64);
        assert_eq!(length % MB as u32, 0);
        assert!(required == 0 || [REGION_GUID_BAT, REGION_GUID_METADATA].contains(&guid));

        Ok(Self {
            guid,
            file_offset,
            length,
            required,
        })
    }
}

#[derive(Debug)]
struct RegionTable {
    signature: String,
    checksum: [u8; 4],
    entries: Vec<RegionTableEntry>,
}

impl RegionTable {
    /// Read a region table from the current position in the file, advancing
    /// the file to beyond the region table.
    fn read(file: &mut File) -> Result<Self, Error> {
        let mut buffer = vec![0; 16];
        file.read_exact(&mut buffer)?;

        let signature = String::from_utf8(buffer[0..4].to_vec()).unwrap();
        let checksum = buffer[4..8].try_into().expect("infallible");
        let entry_count = u32::from_le_bytes(buffer[8..12].try_into().expect("infallible"));

        assert_eq!(signature, REGION_TABLE_SIGNATURE);
        assert!(entry_count <= 2047);

        let mut entries = Vec::with_capacity(entry_count as usize);
        for _ in 0..entry_count {
            entries.push(RegionTableEntry::read(file)?);
        }

        Ok(Self {
            signature,
            checksum,
            entries,
        })
    }
}

#[derive(Debug)]
struct MetadataTableEntry {
    item_id: Guid,
    offset: u32,
    length: u32,
    is_user: bool,
    is_virtual_disk: bool,
    is_required: bool,
    is_empty: bool,
}

impl MetadataTableEntry {
    fn read(file: &mut File) -> Result<Self, Error> {
        let mut buffer = vec![0; 32];
        file.read_exact(&mut buffer)?;

        let item_id = Guid::from_bytes(buffer[0..16].try_into().expect("infallible"));
        let offset = u32::from_le_bytes(buffer[16..20].try_into().expect("infallible"));
        let length = u32::from_le_bytes(buffer[20..24].try_into().expect("infallible"));
        let is_user = buffer[24] & 1 == 1;
        let is_virtual_disk = buffer[24] >> 1 & 1 == 1;
        let is_required = buffer[24] >> 2 & 1 == 1;

        assert!(offset >= 64 * KB as u32);
        assert!(length <= MB as u32);

        if length == 0 {
            assert_eq!(offset, 0);
        }
        let is_empty = length == 0;

        Ok(Self {
            item_id,
            offset,
            length,
            is_user,
            is_virtual_disk,
            is_required,
            is_empty,
        })
    }
}

#[derive(Debug)]
struct MetadataTable {
    signature: String,
    entries: Vec<MetadataTableEntry>,
}

impl MetadataTable {
    fn read(file: &mut File) -> Result<Self, Error> {
        let mut buffer = vec![0; 32];
        file.read_exact(&mut buffer)?;

        let signature = String::from_utf8(buffer[0..8].to_vec()).unwrap();
        let entry_count = u16::from_le_bytes(buffer[10..12].try_into().expect("infallible"));

        assert_eq!(signature, METADATA_TABLE_SIGNATURE);
        assert!(entry_count <= 2047);

        let mut entries = Vec::with_capacity(entry_count as usize);
        for _ in 0..entry_count {
            entries.push(MetadataTableEntry::read(file)?);
        }

        Ok(Self { signature, entries })
    }

    fn get<T: MetadataItem>(&self, file: &mut File, offset: u64) -> Result<Option<T>, Error> {
        self.entries
            .iter()
            .find(|e| e.item_id == T::GUID)
            .filter(|e| !e.is_empty)
            .map(|e| {
                file.seek(SeekFrom::Start(offset + e.offset as u64))?;
                T::read(file)
            })
            .transpose()
    }
}

#[derive(Debug)]
struct HeaderSection {
    file_type_identifier: FileTypeIdentifier,
    header_1: Header,
    header_2: Header,
    region_table_1: RegionTable,
    region_table_2: RegionTable,
}

impl HeaderSection {
    fn read(file: &mut File) -> Result<Self, Error> {
        let file_type_identifier = FileTypeIdentifier::read(file)?;
        file.seek(SeekFrom::Start(64 * KB as u64))?;
        let header_1 = Header::read(file)?;
        file.seek(SeekFrom::Start(128 * KB as u64))?;
        let header_2 = Header::read(file)?;
        file.seek(SeekFrom::Start(192 * KB as u64))?;
        let region_table_1 = RegionTable::read(file)?;
        file.seek(SeekFrom::Start(256 * KB as u64))?;
        let region_table_2 = RegionTable::read(file)?;

        Ok(Self {
            file_type_identifier,
            header_1,
            header_2,
            region_table_1,
            region_table_2,
        })
    }
}

/// Metadata parsed based on the metadata table
#[derive(Debug)]
struct Metadata {
    file_parameters: metadata::FileParameters,
    virtual_disk_size: metadata::VirtualDiskSize,
    virtual_disk_id: metadata::VirtualDiskId,
    logical_sector_size: metadata::LogicalSectorSize,
    physical_sector_size: metadata::PhysicalSectorSize,
    parent_locator: Option<metadata::ParentLocator>,
}
impl Metadata {
    fn from_table(
        file: &mut File,
        metadata_table: &MetadataTable,
        offset: u64,
    ) -> Result<Self, Error> {
        let file_parameters = metadata_table
            .get::<metadata::FileParameters>(file, offset)?
            .unwrap();
        let virtual_disk_size = metadata_table
            .get::<metadata::VirtualDiskSize>(file, offset)?
            .unwrap();
        let virtual_disk_id = metadata_table
            .get::<metadata::VirtualDiskId>(file, offset)?
            .unwrap();
        let logical_sector_size = metadata_table
            .get::<metadata::LogicalSectorSize>(file, offset)?
            .unwrap();
        let physical_sector_size = metadata_table
            .get::<metadata::PhysicalSectorSize>(file, offset)?
            .unwrap();
        let parent_locator = metadata_table.get::<metadata::ParentLocator>(file, offset)?;

        Ok(Self {
            file_parameters,
            virtual_disk_size,
            virtual_disk_id,
            logical_sector_size,
            physical_sector_size,
            parent_locator,
        })
    }
}

/// A VHDX file with all metadata loaded in-memory.
#[derive(Debug)]
pub struct Vhdx {
    file: File,
    header_section: HeaderSection,
    metadata_table: MetadataTable,
    metadata: Metadata,
    bat: bat::Bat,
}

impl Vhdx {
    /// Load a VHDX file from the filesystem.
    ///
    /// Through opening the file, if there is a log to be replayed it will be
    /// applied during this function.
    pub fn load(path: impl AsRef<Path>) -> Result<Self, Error> {
        let mut file = File::options().read(true).write(true).open(path)?;
        let header_section = HeaderSection::read(&mut file)?;

        // Find the metadata table
        let metadata_table_section = header_section
            .region_table_1
            .entries
            .iter()
            .find(|entry| entry.guid == REGION_GUID_METADATA)
            .unwrap();

        file.seek(SeekFrom::Start(metadata_table_section.file_offset))?;
        let metadata_table = MetadataTable::read(&mut file)?;
        let metadata = Metadata::from_table(
            &mut file,
            &metadata_table,
            metadata_table_section.file_offset,
        )?;

        // Find the BAT table
        let bat_table_section = header_section
            .region_table_1
            .entries
            .iter()
            .find(|entry| entry.guid == REGION_GUID_BAT)
            .unwrap();
        file.seek(SeekFrom::Start(bat_table_section.file_offset))?;
        let bat = bat::Bat::read(&mut file, &metadata)?;

        let mut disk = Vhdx {
            file,
            header_section,
            metadata_table,
            metadata,
            bat,
        };
        disk.try_replay_log()?;

        Ok(disk)
    }

    /// Use the disk as a [`Reader`] that implements [`std::io::Read`] and [`std::io::Seek`].
    pub fn reader(&mut self) -> Reader {
        Reader {
            disk: self,
            offset: 0,
        }
    }

    /// Find the active sequence of the log.
    ///
    /// This function does not care if the log is empty or has no valid entries,
    /// and may not return valid entries if it is called in this state.
    fn find_log(&mut self) -> Result<LogSequence, Error> {
        let current_header = self.current_header();
        let log_guid = current_header.log_guid;
        let log_offset = current_header.log_offset;
        let log_length = current_header.log_length;
        println!("log length {} at offset 0x{:X}", log_length, log_offset);

        // From 2.3.3 Log Replay
        // Tail is earlier on in the file, head is later
        // Tail is oldest (lowest sequence number)

        // Step 1
        let mut candidate = LogSequence {
            sequence_number: 0,
            entries: Vec::new(),
        };
        let mut current_tail = log_offset;
        let mut old_tail = log_offset;

        loop {
            // Step 2
            let mut current = LogSequence {
                sequence_number: 0,
                entries: Vec::new(),
            };
            let mut head_value = current_tail;
            self.file.seek(SeekFrom::Start(current_tail))?;

            // Step 3
            loop {
                let entry_offset = self.file.stream_position()?;
                //println!("Attempting to read entry at offset {}", entry_offset);
                match log::Entry::read(&mut self.file) {
                    Ok(entry) => {
                        // Check if the entry matches the guid in the file header
                        if entry.header().log_guid() != log_guid {
                            break;
                        }
                        if current.is_empty() {
                            // Extend the current sequence to include the log entry
                            current.sequence_number = entry.header().sequence_number;
                            current.entries.push((entry_offset - log_offset, entry));
                            head_value = entry_offset;
                        } else if entry.header().sequence_number
                            == current.head().unwrap().header().sequence_number + 1
                        {
                            // Extend the current sequence to include the log entry
                            current.entries.push((entry_offset - log_offset, entry));
                            head_value = entry_offset;
                        }
                    }
                    Err(Error::InvalidSignature) => {
                        // Not a valid entry, stop searching
                        break;
                    }
                    Err(e) => {
                        // Unexpected error, propogate
                        Err(e)?;
                    }
                }
            }

            // Step 4
            let is_current_sequence_valid = current.is_valid();

            // Step 5
            let is_current_sequence_empty = current.is_empty();
            if is_current_sequence_valid && current.sequence_number > candidate.sequence_number {
                candidate = current;
                //println!("new candidate sequence: {}", candidate.sequence_number);
            }

            // Step 6
            if is_current_sequence_empty || !is_current_sequence_valid {
                // Step forward one sector if we didn't have a valid sequence
                current_tail += 4 * KB as u64;
                if current_tail >= log_offset + log_length as u64 {
                    current_tail -= log_length as u64;
                }
            } else {
                // Sequence is valid and non-empty, skip to the end
                current_tail = head_value;
            }

            // Step 7
            if current_tail < old_tail {
                // Stop
                break;
            }
            old_tail = current_tail;
        }

        if candidate.is_empty() {
            panic!("no valid log sequences, file is corrupt");
        }

        // Check if the file has been truncated since the log was written
        let file_size = self.file.seek(SeekFrom::End(0))?;
        if file_size < candidate.head().unwrap().header().flushed_file_offset {
            panic!("file has been truncated, cannot open");
        }

        let active_sequence = candidate;
        println!(
            "Found active sequence with {} entries ({} -> {})",
            active_sequence.entries.len(),
            active_sequence.tail().unwrap().header().sequence_number,
            active_sequence.head().unwrap().header().sequence_number,
        );

        Ok(active_sequence)
    }

    fn try_replay_log(&mut self) -> Result<(), Error> {
        // Check if we should replay the log
        let current_header = self.current_header();
        if current_header.log_guid == Guid::ZERO {
            return Ok(());
        }

        println!("replaying log");
        let sequence = self.find_log()?;

        // Replay the log
        for entry in sequence.iter() {
            let mut data_sector_offset = 0;
            for desc in entry.descriptors() {
                match desc {
                    log::Descriptor::Zero(desc) => {
                        if desc.sequence_number() != entry.header().sequence_number {
                            panic!("descriptor does not have the correct sequence number");
                        }

                        // TODO: Do we need to expand the file?
                        let file_length = self.file.seek(SeekFrom::End(0))?;
                        if desc.file_offset() >= file_length {
                            panic!("zeros write start is greater than file length");
                        }
                        if desc.file_offset() + desc.zero_length() >= file_length {
                            panic!("zeros write start is greater than file length");
                        }

                        self.file.seek(SeekFrom::Start(desc.file_offset()))?;
                        let num_sectors = desc.zero_length() / (4 * KB as u64);
                        for _ in 0..num_sectors {
                            self.file.write_all(&ZEROS)?;
                        }
                    }
                    log::Descriptor::Data(desc) => {
                        if desc.sequence_number() != entry.header().sequence_number {
                            panic!("descriptor does not have the correct sequence number");
                        }

                        let data_sector = &entry.data_sectors()[data_sector_offset];

                        // TODO: Do we need to expand the file?
                        let file_length = self.file.seek(SeekFrom::End(0))?;
                        if desc.file_offset() >= file_length {
                            panic!("data write start is greater than file length");
                        }
                        if desc.file_offset() + 4 * KB as u64 >= file_length {
                            panic!("data write end is greater than file length");
                        }
                        self.file.seek(SeekFrom::Start(desc.file_offset()))?;
                        self.file.write_all(&desc.leading_bytes())?;
                        self.file.write_all(data_sector.data())?;
                        self.file.write_all(&desc.trailing_bytes())?;

                        data_sector_offset += 1;
                    }
                }
            }
        }

        Ok(())
    }

    fn debug_log_sectors(&mut self, log_offset: u64, log_length: u32) -> Result<(), Error> {
        let mut entry_offset = log_offset;
        let stride = 4 * KB as u64;
        while entry_offset - log_offset < log_length as u64 {
            self.file.seek(SeekFrom::Start(entry_offset))?;
            let mut buffer = vec![0; 64];
            self.file.read_exact(&mut buffer)?;
            let signature = String::from_utf8(buffer[0..4].to_vec()).unwrap();
            if !buffer.iter().all(|c| *c == 0) {
                println!(
                    "Entry {}: '{}' (offset {})",
                    (entry_offset - log_offset) / stride,
                    signature,
                    entry_offset,
                );
            }
            entry_offset += stride;
        }
        Ok(())
    }

    fn current_header(&self) -> &Header {
        std::cmp::max_by_key(
            &self.header_section.header_1,
            &self.header_section.header_2,
            |header| header.sequence_number,
        )
    }
}

struct LogSequence {
    sequence_number: u64,
    /// Log entries in order from tail to head,
    ///
    /// The offset is from the start of the log, not the file
    entries: Vec<(u64, log::Entry)>,
}

impl LogSequence {
    fn tail(&self) -> Option<&log::Entry> {
        self.entries.first().map(|(_, entry)| entry)
    }

    fn head(&self) -> Option<&log::Entry> {
        self.entries.last().map(|(_, entry)| entry)
    }

    /// A sequence is valid if it is both non-empty and that the head entry's tail
    /// is also part of the sequence.
    ///
    /// Note that this function does not check for consecutive sequence numbers
    fn is_valid(&self) -> bool {
        let Some(head) = self.head() else {
            return false;
        };
        let current_sequence_head_entry_tail = head.header().tail as u64;
        self.entries
            .iter()
            .any(|(offset, _)| *offset == current_sequence_head_entry_tail)
    }

    fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Iterate over the sequence in order from tail to head.
    fn iter(&self) -> impl Iterator<Item = &log::Entry> {
        self.entries.iter().map(|(_, entry)| entry)
    }
}

/// A higher-level abstraction to a VHDX disk that implements [`std::io::Read`]
/// and [`std::io::Seek`].
#[derive(Debug)]
pub struct Reader<'a> {
    disk: &'a mut Vhdx,
    offset: u64,
}

impl Read for Reader<'_> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        // Read at most to the end of this block
        let (entry, offset) = self.disk.bat.offset_to_entry(self.offset);
        let block_size = self.disk.metadata.file_parameters.block_size() as usize;
        let bytes_remaining_in_block = block_size as u64 - offset;
        let num_to_read = buf.len().min(bytes_remaining_in_block as usize);
        let dest_slice = &mut buf[..num_to_read];

        self.offset += num_to_read as u64;

        use bat::PayloadBatEntryState::*;
        let num_actually_read = match entry.state() {
            NotPresent | Undefined | Zero | Unmapped => {
                // Return zeros
                dest_slice.fill(0);
                num_to_read
            }
            FullyPresent => {
                // Read from file
                self.disk
                    .file
                    .seek(SeekFrom::Start(entry.file_offset() + offset))?;
                self.disk.file.read(dest_slice)?
            }
            PartiallyPresent => unimplemented!("differential disks"),
        };

        Ok(num_actually_read)
    }
}

impl Seek for Reader<'_> {
    fn seek(&mut self, pos: SeekFrom) -> std::io::Result<u64> {
        match pos {
            SeekFrom::Start(offset) => self.offset = offset,
            SeekFrom::End(from_end) => {
                self.offset = self
                    .disk
                    .metadata
                    .virtual_disk_size
                    .virtual_disk_size()
                    .checked_add_signed(from_end)
                    .unwrap()
            }
            SeekFrom::Current(offset) => {
                self.offset = self.offset.checked_add_signed(offset).unwrap()
            }
        }
        Ok(self.offset)
    }
}

impl Write for Reader<'_> {
    fn write(&mut self, _buf: &[u8]) -> std::io::Result<usize> {
        unimplemented!()
    }

    fn flush(&mut self) -> std::io::Result<()> {
        unimplemented!()
    }
}
