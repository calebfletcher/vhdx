use std::{
    fs::File,
    io::{Read, Seek, SeekFrom},
    path::Path,
};

static FILE_SIGNATURE: &str = "vhdxfile";
static HEADER_SIGNATURE: &str = "head";

const KB: usize = 1024;
const MB: usize = KB * KB;

#[derive(Debug)]
pub struct FileTypeIdentifier {
    signature: String,
    creator: String,
}

impl FileTypeIdentifier {
    /// Read a file type identifier from the current position in the file,
    /// advancing the file to beyond the file type identifier.
    pub fn read(file: &mut File) -> Self {
        let mut chunk = vec![0; KB];
        file.read_exact(&mut chunk).unwrap();
        let signature = String::from_utf8_lossy(&chunk[..8]).into_owned();
        assert_eq!(signature, FILE_SIGNATURE);

        let creator_iter = chunk[8..(8 + 512)]
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
pub struct Header {
    signature: String,
    checksum: [u8; 4],
    sequence_number: u64,
    file_write_guid: u128,
    data_write_guid: u128,
    log_guid: u128,
    log_version: u16,
    version: u16,
    log_length: u32,
    log_offset: u64,
}

impl Header {
    /// Read a header from the current position in the file, advancing the
    /// file to beyond the header.
    pub fn read(file: &mut File) -> Self {
        let mut header_buffer = vec![0; 128];
        file.read_exact(&mut header_buffer).unwrap();

        let signature = String::from_utf8(header_buffer[0..4].to_vec()).unwrap();
        let checksum = header_buffer[4..8].try_into().unwrap();
        let sequence_number = u64::from_le_bytes(header_buffer[8..16].try_into().unwrap());

        let file_write_guid = u128::from_le_bytes(header_buffer[16..32].try_into().unwrap());
        let data_write_guid = u128::from_le_bytes(header_buffer[32..48].try_into().unwrap());
        let log_guid = u128::from_le_bytes(header_buffer[48..64].try_into().unwrap());

        let log_version = u16::from_le_bytes(header_buffer[64..66].try_into().unwrap());
        let version = u16::from_le_bytes(header_buffer[66..68].try_into().unwrap());
        let log_length = u32::from_le_bytes(header_buffer[68..72].try_into().unwrap());
        let log_offset = u64::from_le_bytes(header_buffer[72..80].try_into().unwrap());

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
pub struct Vhdx {
    file_type_identifier: FileTypeIdentifier,
    header_1: Header,
    header_2: Header,
}

impl Vhdx {
    pub fn load(path: impl AsRef<Path>) -> Vhdx {
        let mut file = std::fs::File::open(path).unwrap();
        let file_type_identifier = FileTypeIdentifier::read(&mut file);
        file.seek(SeekFrom::Start(64 * KB as u64)).unwrap();
        let header_1 = Header::read(&mut file);
        file.seek(SeekFrom::Start(128 * KB as u64)).unwrap();
        let header_2 = Header::read(&mut file);

        Vhdx {
            file_type_identifier,
            header_1,
            header_2,
        }
    }
}

// #[cfg(test)]
// mod tests {
//     use super::*;

//     #[test]
//     fn it_works() {
//         let result = add(2, 2);
//         assert_eq!(result, 4);
//     }
// }
