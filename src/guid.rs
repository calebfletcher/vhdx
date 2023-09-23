#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Guid {
    data_1: u32,
    data_2: u16,
    data_3: u16,
    data_4: [u8; 8],
}

impl Guid {
    pub const ZERO: Guid = Guid::from_bytes([0; 16]);

    pub const fn new(data_1: u32, data_2: u16, data_3: u16, data_4: [u8; 8]) -> Self {
        Self {
            data_1,
            data_2,
            data_3,
            data_4,
        }
    }

    pub const fn from_bytes(bytes: [u8; 16]) -> Self {
        Self {
            data_1: u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]),
            data_2: u16::from_le_bytes([bytes[4], bytes[5]]),
            data_3: u16::from_le_bytes([bytes[6], bytes[7]]),
            data_4: [
                bytes[8], bytes[9], bytes[10], bytes[11], bytes[12], bytes[13], bytes[14],
                bytes[15],
            ],
        }
    }

    pub const fn from_str(value: &str) -> Self {
        let value = value.as_bytes();
        if value.len() != 36 {
            panic!("uuid has incorrect length");
        }

        let mut bytes = [0; 16];
        let mut skipped_chars = 0;

        let mut i = 0;
        while i < value.len() {
            let character = value[i];

            if character == b'-' {
                i += 1;
                skipped_chars += 1;
                continue;
            }

            let nibble = hex_digit_to_nibble(character);

            let nibble_index = i - skipped_chars;
            let buffer_idx = nibble_index / 2;
            let is_higher_nibble = nibble_index % 2 == 0;

            bytes[buffer_idx] |= if is_higher_nibble {
                nibble << 4
            } else {
                nibble
            };

            i += 1;
        }

        Self {
            data_1: u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]),
            data_2: u16::from_be_bytes([bytes[4], bytes[5]]),
            data_3: u16::from_be_bytes([bytes[6], bytes[7]]),
            data_4: [
                bytes[8], bytes[9], bytes[10], bytes[11], bytes[12], bytes[13], bytes[14],
                bytes[15],
            ],
        }
    }
}

impl std::fmt::Display for Guid {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{:08X}-{:04X}-{:04X}-{:02X}{:02X}-{:02X}{:02X}{:02X}{:02X}{:02X}{:02X}",
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

const fn hex_digit_to_nibble(input: u8) -> u8 {
    match input {
        b'0'..=b'9' => input - b'0',
        b'a'..=b'f' => input - b'a' + 10,
        b'A'..=b'F' => input - b'A' + 10,
        _ => panic!("unknown char"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn guid_parse() {
        let expected = Guid::new(
            0x2DC27766,
            0xF623,
            0x4200,
            [0x9D, 0x64, 0x11, 0x5E, 0x9B, 0xFD, 0x4A, 0x08],
        );

        let string = "2DC27766-F623-4200-9D64-115E9BFD4A08";

        assert_eq!(expected, Guid::from_str(string));
    }
}
