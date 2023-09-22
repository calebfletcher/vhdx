use std::{fs::File, io::Read};

use crate::guid::Guid;

static PARENT_LOCATOR_TYPE: Guid = Guid::from_str("B04AEFB7-D19E-4A81-B789-25B8E9445913");

pub trait MetadataItem {
    const GUID: Guid;

    fn read(file: &mut File) -> Self;
}

#[derive(Debug)]
pub struct FileParameters {
    block_size: u32,
    leave_block_allocated: bool,
    has_parent: bool,
}

impl FileParameters {
    pub fn block_size(&self) -> u32 {
        self.block_size
    }
}

impl MetadataItem for FileParameters {
    const GUID: Guid = Guid::from_str("CAA16737-FA36-4D43-B3B6-33F0AA44E76B");

    fn read(file: &mut File) -> Self {
        let mut buffer = vec![0; 8];
        file.read_exact(&mut buffer).unwrap();

        let block_size = u32::from_le_bytes(buffer[0..4].try_into().unwrap());
        let leave_block_allocated = buffer[4] >> 7 & 1 == 1;
        let has_parent = buffer[4] >> 6 & 1 == 1;

        Self {
            block_size,
            leave_block_allocated,
            has_parent,
        }
    }
}

#[derive(Debug)]
pub struct VirtualDiskSize {
    virtual_disk_size: u64,
}

impl VirtualDiskSize {
    pub fn virtual_disk_size(&self) -> u64 {
        self.virtual_disk_size
    }
}

impl MetadataItem for VirtualDiskSize {
    const GUID: Guid = Guid::from_str("2FA54224-CD1B-4876-B211-5DBED83BF4B8");

    fn read(file: &mut File) -> Self {
        let mut buffer = vec![0; 8];
        file.read_exact(&mut buffer).unwrap();

        let virtual_disk_size = u64::from_le_bytes(buffer[0..8].try_into().unwrap());

        Self { virtual_disk_size }
    }
}

#[derive(Debug)]
pub struct VirtualDiskId {
    virtual_disk_id: Guid,
}

impl VirtualDiskId {
    pub fn virtual_disk_id(&self) -> Guid {
        self.virtual_disk_id
    }
}

impl MetadataItem for VirtualDiskId {
    const GUID: Guid = Guid::from_str("BECA12AB-B2E6-4523-93EF-C309E000C746");

    fn read(file: &mut File) -> Self {
        let mut buffer = vec![0; 16];
        file.read_exact(&mut buffer).unwrap();

        let virtual_disk_id = Guid::from_bytes(buffer[0..16].try_into().unwrap());

        Self { virtual_disk_id }
    }
}

#[derive(Debug)]
pub struct LogicalSectorSize {
    logical_sector_size: u32,
}

impl LogicalSectorSize {
    pub fn logical_sector_size(&self) -> u32 {
        self.logical_sector_size
    }
}

impl MetadataItem for LogicalSectorSize {
    const GUID: Guid = Guid::from_str("8141BF1D-A96F-4709-BA47-F233A8FAAB5F");

    fn read(file: &mut File) -> Self {
        let mut buffer = vec![0; 4];
        file.read_exact(&mut buffer).unwrap();

        let logical_sector_size = u32::from_le_bytes(buffer[0..4].try_into().unwrap());
        assert!([512, 4096].contains(&logical_sector_size));

        Self {
            logical_sector_size,
        }
    }
}

#[derive(Debug)]
pub struct PhysicalSectorSize {
    physical_sector_size: u32,
}

impl PhysicalSectorSize {
    pub fn physical_sector_size(&self) -> u32 {
        self.physical_sector_size
    }
}

impl MetadataItem for PhysicalSectorSize {
    const GUID: Guid = Guid::from_str("CDA348C7-445D-4471-9CC9-E9885251C556");

    fn read(file: &mut File) -> Self {
        let mut buffer = vec![0; 4];
        file.read_exact(&mut buffer).unwrap();

        let physical_sector_size = u32::from_le_bytes(buffer[0..4].try_into().unwrap());
        assert!([512, 4096].contains(&physical_sector_size));

        Self {
            physical_sector_size,
        }
    }
}

#[derive(Debug)]
pub struct ParentLocator {
    locator_type: Guid,
    key_value_count: u16,
}

impl MetadataItem for ParentLocator {
    const GUID: Guid = Guid::from_str("A8D35F2D-B30B-454D-ABF7-D3D84834AB0C");

    fn read(file: &mut File) -> Self {
        let mut buffer = vec![0; 20];
        file.read_exact(&mut buffer).unwrap();

        let locator_type = Guid::from_bytes(buffer[0..16].try_into().unwrap());
        let key_value_count = u16::from_le_bytes(buffer[18..20].try_into().unwrap());
        // TODO: Read the key-value data to find the parent

        assert_eq!(locator_type, PARENT_LOCATOR_TYPE);

        Self {
            locator_type,
            key_value_count,
        }
    }
}
