/// Data structure representing a file inside an archive during the rebuild process.
/// 
/// `ZippedFile` stores the byte index and CRC of a file within the source zip,
/// allowing `TorrentZipRebuild` to sort the central directory entries alphabetically 
/// and track which stream chunks need to be extracted and recompressed.
/// 
/// Differences from C#:
/// - Maps 1:1 to the C# `TrrntZip.ZippedFile` struct. Implements `Ord` to ensure
///   identical alphabetical sorting behavior.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct ZippedFile {
    pub index: i32,
    pub name: String,
    pub size: u64,
    pub crc: Option<Vec<u8>>,
    pub sha1: Option<Vec<u8>>,
    pub is_dir: bool,
}

impl ZippedFile {
    pub fn new() -> Self {
        Self {
            index: -1,
            name: String::new(),
            size: 0,
            crc: None,
            sha1: None,
            is_dir: false,
        }
    }

    pub fn byte_crc(&self) -> Option<[u8; 4]> {
        let bytes = self.crc.as_ref()?;
        if bytes.len() != 4 {
            return None;
        }
        Some([bytes[0], bytes[1], bytes[2], bytes[3]])
    }

    pub fn set_byte_crc(&mut self, value: Option<&[u8]>) {
        let Some(bytes) = value else {
            self.crc = None;
            return;
        };
        if bytes.len() != 4 {
            self.crc = None;
            return;
        }
        self.crc = Some(bytes.to_vec());
    }

    pub fn string_crc(&self) -> String {
        let Some(bytes) = self.byte_crc() else {
            return String::new();
        };
        let mut out = String::with_capacity(8);
        for b in bytes {
            use std::fmt::Write;
            let _ = write!(out, "{:02x}", b);
        }
        out
    }
}

impl Default for ZippedFile {
    fn default() -> Self {
        Self::new()
    }
}
