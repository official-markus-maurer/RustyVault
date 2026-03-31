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
}
