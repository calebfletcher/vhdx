#![forbid(unsafe_code)]
#![allow(dead_code)]

use std::{
    fs::File,
    io::{Read, Seek, SeekFrom, Write},
    path::Path,
};

use metadata::MetadataItem;

use crate::guid::Guid;

mod bat;
mod guid;
mod metadata;

static FILE_SIGNATURE: &str = "vhdxfile";
static HEADER_SIGNATURE: &str = "head";
static REGION_TABLE_SIGNATURE: &str = "regi";
static METADATA_TABLE_SIGNATURE: &str = "metadata";

static REGION_GUID_BAT: Guid = Guid::from_str("2DC27766-F623-4200-9D64-115E9BFD4A08");
static REGION_GUID_METADATA: Guid = Guid::from_str("8B7CA206-4790-4B9A-B8FE-575F050F886E");

const KB: usize = 1024;
const MB: usize = KB * KB;

#[derive(Debug)]
struct FileTypeIdentifier {
    signature: String,
    creator: String,
}

impl FileTypeIdentifier {
    /// Read a file type identifier from the current position in the file,
    /// advancing the file to beyond the file type identifier.
    fn read(file: &mut File) -> Self {
        let mut buffer = vec![0; KB];
        file.read_exact(&mut buffer).unwrap();
        let signature = String::from_utf8_lossy(&buffer[..8]).into_owned();
        assert_eq!(signature, FILE_SIGNATURE);

        let creator_iter = buffer[8..(8 + 512)]
            .chunks_exact(2)
            .map(|bytes| u16::from_le_bytes(bytes.try_into().unwrap()))
            .take_while(|&ch| ch != 0);
        let creator = char::decode_utf16(creator_iter)
            .collect::<Result<String, _>>()
            .unwrap();

        Self { signature, creator }
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
    fn read(file: &mut File) -> Self {
        let mut buffer = vec![0; 128];
        file.read_exact(&mut buffer).unwrap();

        let signature = String::from_utf8(buffer[0..4].to_vec()).unwrap();
        let checksum = buffer[4..8].try_into().unwrap();
        let sequence_number = u64::from_le_bytes(buffer[8..16].try_into().unwrap());

        let file_write_guid = Guid::from_bytes(buffer[16..32].try_into().unwrap());
        let data_write_guid = Guid::from_bytes(buffer[32..48].try_into().unwrap());
        let log_guid = Guid::from_bytes(buffer[48..64].try_into().unwrap());

        let log_version = u16::from_le_bytes(buffer[64..66].try_into().unwrap());
        let version = u16::from_le_bytes(buffer[66..68].try_into().unwrap());
        let log_length = u32::from_le_bytes(buffer[68..72].try_into().unwrap());
        let log_offset = u64::from_le_bytes(buffer[72..80].try_into().unwrap());

        assert_eq!(signature, HEADER_SIGNATURE);
        assert_eq!(log_version, 0);
        assert_eq!(version, 1);
        assert_eq!(log_length % MB as u32, 0);
        assert_eq!(log_offset % MB as u64, 0);

        if log_guid != Guid::ZERO {
            unimplemented!("log replay");
        }

        Self {
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
        }
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
    fn read(file: &mut File) -> Self {
        let mut buffer = vec![0; 32];
        file.read_exact(&mut buffer).unwrap();

        let guid = Guid::from_bytes(buffer[0..16].try_into().unwrap());
        let file_offset = u64::from_le_bytes(buffer[16..24].try_into().unwrap());
        let length = u32::from_le_bytes(buffer[24..28].try_into().unwrap());
        let required = u32::from_le_bytes(buffer[28..32].try_into().unwrap());

        assert_eq!(file_offset % MB as u64, 0);
        assert!(file_offset > MB as u64);
        assert_eq!(length % MB as u32, 0);
        assert!(required == 0 || [REGION_GUID_BAT, REGION_GUID_METADATA].contains(&guid));

        Self {
            guid,
            file_offset,
            length,
            required,
        }
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
    fn read(file: &mut File) -> Self {
        let mut buffer = vec![0; 16];
        file.read_exact(&mut buffer).unwrap();

        let signature = String::from_utf8(buffer[0..4].to_vec()).unwrap();
        let checksum = buffer[4..8].try_into().unwrap();
        let entry_count = u32::from_le_bytes(buffer[8..12].try_into().unwrap());

        assert_eq!(signature, REGION_TABLE_SIGNATURE);
        assert!(entry_count <= 2047);

        let mut entries = Vec::with_capacity(entry_count as usize);
        for _ in 0..entry_count {
            entries.push(RegionTableEntry::read(file));
        }

        Self {
            signature,
            checksum,
            entries,
        }
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
    fn read(file: &mut File) -> Self {
        let mut buffer = vec![0; 32];
        file.read_exact(&mut buffer).unwrap();

        let item_id = Guid::from_bytes(buffer[0..16].try_into().unwrap());
        let offset = u32::from_le_bytes(buffer[16..20].try_into().unwrap());
        let length = u32::from_le_bytes(buffer[20..24].try_into().unwrap());
        let is_user = buffer[24] & 1 == 1;
        let is_virtual_disk = buffer[24] >> 1 & 1 == 1;
        let is_required = buffer[24] >> 2 & 1 == 1;

        assert!(offset >= 64 * KB as u32);
        assert!(length <= MB as u32);

        if length == 0 {
            assert_eq!(offset, 0);
        }
        let is_empty = length == 0;

        Self {
            item_id,
            offset,
            length,
            is_user,
            is_virtual_disk,
            is_required,
            is_empty,
        }
    }
}

#[derive(Debug)]
struct MetadataTable {
    signature: String,
    entries: Vec<MetadataTableEntry>,
}

impl MetadataTable {
    fn read(file: &mut File) -> Self {
        let mut buffer = vec![0; 32];
        file.read_exact(&mut buffer).unwrap();

        let signature = String::from_utf8(buffer[0..8].to_vec()).unwrap();
        let entry_count = u16::from_le_bytes(buffer[10..12].try_into().unwrap());

        assert_eq!(signature, METADATA_TABLE_SIGNATURE);
        assert!(entry_count <= 2047);

        let mut entries = Vec::with_capacity(entry_count as usize);
        for _ in 0..entry_count {
            entries.push(MetadataTableEntry::read(file));
        }

        Self { signature, entries }
    }

    fn get<T: MetadataItem>(&self, file: &mut File, offset: u64) -> Option<T> {
        self.entries
            .iter()
            .find(|e| e.item_id == T::GUID)
            .filter(|e| !e.is_empty)
            .map(|e| {
                file.seek(SeekFrom::Start(offset + e.offset as u64))
                    .unwrap();
                T::read(file)
            })
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
    fn read(file: &mut File) -> Self {
        let file_type_identifier = FileTypeIdentifier::read(file);
        file.seek(SeekFrom::Start(64 * KB as u64)).unwrap();
        let header_1 = Header::read(file);
        file.seek(SeekFrom::Start(128 * KB as u64)).unwrap();
        let header_2 = Header::read(file);
        file.seek(SeekFrom::Start(192 * KB as u64)).unwrap();
        let region_table_1 = RegionTable::read(file);
        file.seek(SeekFrom::Start(256 * KB as u64)).unwrap();
        let region_table_2 = RegionTable::read(file);

        Self {
            file_type_identifier,
            header_1,
            header_2,
            region_table_1,
            region_table_2,
        }
    }
}

/// Metadata parsed based on the metadata table
#[derive(Debug)]
pub struct Metadata {
    file_parameters: metadata::FileParameters,
    virtual_disk_size: metadata::VirtualDiskSize,
    virtual_disk_id: metadata::VirtualDiskId,
    logical_sector_size: metadata::LogicalSectorSize,
    physical_sector_size: metadata::PhysicalSectorSize,
    parent_locator: Option<metadata::ParentLocator>,
}
impl Metadata {
    fn from_table(file: &mut File, metadata_table: &MetadataTable, offset: u64) -> Self {
        let file_parameters = metadata_table
            .get::<metadata::FileParameters>(file, offset)
            .unwrap();
        let virtual_disk_size = metadata_table
            .get::<metadata::VirtualDiskSize>(file, offset)
            .unwrap();
        let virtual_disk_id = metadata_table
            .get::<metadata::VirtualDiskId>(file, offset)
            .unwrap();
        let logical_sector_size = metadata_table
            .get::<metadata::LogicalSectorSize>(file, offset)
            .unwrap();
        let physical_sector_size = metadata_table
            .get::<metadata::PhysicalSectorSize>(file, offset)
            .unwrap();
        let parent_locator = metadata_table.get::<metadata::ParentLocator>(file, offset);

        Self {
            file_parameters,
            virtual_disk_size,
            virtual_disk_id,
            logical_sector_size,
            physical_sector_size,
            parent_locator,
        }
    }
}

#[derive(Debug)]
pub struct Vhdx {
    file: File,
    header_section: HeaderSection,
    metadata_table: MetadataTable,
    metadata: Metadata,
    bat: bat::Bat,
}

impl Vhdx {
    pub fn load(path: impl AsRef<Path>) -> Vhdx {
        let mut file = std::fs::File::open(path).unwrap();
        let header_section = HeaderSection::read(&mut file);

        // Find the metadata table
        let metadata_table_section = header_section
            .region_table_1
            .entries
            .iter()
            .find(|entry| entry.guid == REGION_GUID_METADATA)
            .unwrap();

        file.seek(SeekFrom::Start(metadata_table_section.file_offset))
            .unwrap();
        let metadata_table = MetadataTable::read(&mut file);
        let metadata = Metadata::from_table(
            &mut file,
            &metadata_table,
            metadata_table_section.file_offset,
        );

        // Find the BAT table
        let bat_table_section = header_section
            .region_table_1
            .entries
            .iter()
            .find(|entry| entry.guid == REGION_GUID_BAT)
            .unwrap();
        file.seek(SeekFrom::Start(bat_table_section.file_offset))
            .unwrap();
        let bat = bat::Bat::read(&mut file, &metadata);

        Vhdx {
            file,
            header_section,
            metadata_table,
            metadata,
            bat,
        }
    }

    pub fn reader(self) -> Reader {
        Reader {
            disk: self,
            offset: 0,
        }
    }
}

#[derive(Debug)]
pub struct Reader {
    disk: Vhdx,
    offset: u64,
}

impl Read for Reader {
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

impl Seek for Reader {
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

impl Write for Reader {
    fn write(&mut self, _buf: &[u8]) -> std::io::Result<usize> {
        unimplemented!()
    }

    fn flush(&mut self) -> std::io::Result<()> {
        unimplemented!()
    }
}
