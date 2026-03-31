/// Represents the header information for a file within an archive or on disk.
///
/// `FileHeader` is a unified structure that stores metadata (filename, size, CRC, timestamps)
/// for any file, regardless of whether it was extracted from a Zip, 7z, Gz, or is a raw file.
///
/// Differences from C#:
/// - Maps 1:1 to the C# `Compress.FileHeader` class.
#[derive(Debug, Clone, Default)]
pub struct FileHeader {
    pub filename: String,
    pub uncompressed_size: u64,
    pub crc: Option<Vec<u8>>,
    pub is_directory: bool,

    pub header_last_modified: i64,
    pub modified_time: Option<i64>,
    pub created_time: Option<i64>,
    pub accessed_time: Option<i64>,

    pub local_head: Option<u64>,
}

impl FileHeader {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn last_modified(&self) -> i64 {
        self.modified_time.unwrap_or(self.header_last_modified)
    }
}
