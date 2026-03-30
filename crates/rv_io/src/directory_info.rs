use std::fs;
use std::path::Path as StdPath;
use crate::file_info::FileInfo;

pub struct DirectoryInfo {
    pub name: String,
    pub full_name: String,
    pub last_write_time: i64,
    pub last_access_time: i64,
    pub creation_time: i64,
    pub exists: bool,
}

impl DirectoryInfo {
    pub fn new(path: &str) -> Self {
        let std_path = StdPath::new(path);
        let name = std_path.file_name().unwrap_or_default().to_string_lossy().into_owned();
        let full_name = path.to_string();

        if !std_path.is_dir() {
            return Self {
                name,
                full_name,
                last_write_time: 0,
                last_access_time: 0,
                creation_time: 0,
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
            exists: true,
        }
    }

    pub fn get_directories(&self) -> Vec<DirectoryInfo> {
        let mut dirs = Vec::new();
        if !self.exists {
            return dirs;
        }

        if let Ok(entries) = fs::read_dir(&self.full_name) {
            for entry in entries.flatten() {
                if let Ok(metadata) = entry.metadata() {
                    if metadata.is_dir() {
                        dirs.push(DirectoryInfo::new(&entry.path().to_string_lossy()));
                    }
                }
            }
        }
        dirs
    }

    pub fn get_files(&self, _search_pattern: &str) -> Vec<FileInfo> {
        // Simplified search pattern handling. A full port might use the glob crate
        let mut files = Vec::new();
        if !self.exists {
            return files;
        }

        if let Ok(entries) = fs::read_dir(&self.full_name) {
            for entry in entries.flatten() {
                if let Ok(metadata) = entry.metadata() {
                    if metadata.is_file() {
                        files.push(FileInfo::new(&entry.path().to_string_lossy()));
                    }
                }
            }
        }
        files
    }
}
