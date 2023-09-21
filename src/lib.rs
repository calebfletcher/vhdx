#![forbid(unsafe_code)]
#![allow(dead_code)]

use std::{
    fs::File,
    io::{Read, Seek, SeekFrom},
    path::Path,
};

use crate::guid::Guid;

mod guid;

static FILE_SIGNATURE: &str = "vhdxfile";
static HEADER_SIGNATURE: &str = "head";
static REGION_TABLE_SIGNATURE: &str = "regi";

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
struct RegionEntry {
    guid: Guid,
    file_offset: u64,
    length: u32,
    required: u32,
}

impl RegionEntry {
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
    entries: Vec<RegionEntry>,
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
            entries.push(RegionEntry::read(file));
        }

        Self {
            signature,
            checksum,
            entries,
        }
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

#[derive(Debug)]
pub struct Vhdx {
    header_section: HeaderSection,
}

impl Vhdx {
    pub fn load(path: impl AsRef<Path>) -> Vhdx {
        let mut file = std::fs::File::open(path).unwrap();
        let header_section = HeaderSection::read(&mut file);
        Vhdx { header_section }
    }
}
