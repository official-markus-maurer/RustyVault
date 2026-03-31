use std::fs;
use std::path::Path as StdPath;

/// Object-oriented wrapper representing a specific file on disk.
/// 
/// `FileInfo` mimics the C# `System.IO.FileInfo` class. It encapsulates 
/// a file path and provides methods to query its metadata (size, last modified time).
/// 
/// Differences from C#:
/// - Internally delegates to `std::fs::metadata`.
#[derive(Debug, Clone)]
pub struct FileInfo {
    pub name: String,
    pub full_name: String,
    pub last_write_time: i64,
    pub last_access_time: i64,
    pub creation_time: i64,
    pub length: u64,
    pub exists: bool,
}

impl FileInfo {
    pub fn new(path: &str) -> Self {
        let std_path = StdPath::new(path);
        let name = std_path.file_name().unwrap_or_default().to_string_lossy().into_owned();
        let full_name = path.to_string();

        if !std_path.is_file() {
            return Self {
                name,
                full_name,
                last_write_time: 0,
                last_access_time: 0,
                creation_time: 0,
                length: 0,
                exists: false,
            };
        }

        let metadata = fs::metadata(std_path).unwrap();
        
        let last_write_time = metadata.modified()
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);
            
        let last_access_time = metadata.accessed()
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);
            
        let creation_time = metadata.created()
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);

        Self {
            name,
            full_name,
            last_write_time,
            last_access_time,
            creation_time,
            length: metadata.len(),
            exists: true,
        }
    }
}
