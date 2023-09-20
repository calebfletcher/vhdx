#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Guid {
    data_1: u32,
    data_2: u16,
    data_3: u16,
    data_4: [u8; 8],
}

impl Guid {
    pub const fn new(data_1: u32, data_2: u16, data_3: u16, data_4: [u8; 8]) -> Self {
        Self {
            data_1,
            data_2,
            data_3,
            data_4,
        }
    }

    pub fn from_bytes(bytes: [u8; 16]) -> Self {
        Self {
            data_1: u32::from_le_bytes(bytes[0..4].try_into().unwrap()),
            data_2: u16::from_le_bytes(bytes[4..6].try_into().unwrap()),
            data_3: u16::from_le_bytes(bytes[6..8].try_into().unwrap()),
            data_4: bytes[8..16].try_into().unwrap(),
        }
    }
}

impl std::fmt::Display for Guid {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{:04X}-{:04X}-{:04X}-{:02X}{:02X}-{:02X}{:02X}{:02X}{:02X}{:02X}{:02X}",
            self.data_1,
            self.data_2,
            self.data_3,
            self.data_4[0],
            self.data_4[1],
            self.data_4[2],
            self.data_4[3],
            self.data_4[4],
            self.data_4[5],
            self.data_4[6],
            self.data_4[7]
        )
    }
}

impl std::fmt::Debug for Guid {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        <Guid as std::fmt::Display>::fmt(self, f)
    }
}
