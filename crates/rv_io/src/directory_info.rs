use crate::file_info::FileInfo;
use crate::name_fix::NameFix;
use std::fs;
use std::path::Path as StdPath;

/// Object-oriented wrapper representing a specific directory on disk.
///
/// `DirectoryInfo` mimics the C# `System.IO.DirectoryInfo` class. It encapsulates
/// a directory path and provides methods to query its existence or retrieve its child
/// files/directories as objects.
///
/// Differences from C#:
/// - Internally delegates to `std::fs` and `PathBuf`.
#[derive(Debug, Clone)]
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
        let fixed = NameFix::add_long_path_prefix(path);
        let std_path = StdPath::new(&fixed);
        let name = std_path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned();
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

        let last_write_time = metadata
            .modified()
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| ticks_from_unix_duration(d.as_secs(), d.subsec_nanos()))
            .unwrap_or(0);

        let last_access_time = metadata
            .accessed()
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| ticks_from_unix_duration(d.as_secs(), d.subsec_nanos()))
            .unwrap_or(0);

        let creation_time = metadata
            .created()
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| ticks_from_unix_duration(d.as_secs(), d.subsec_nanos()))
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

        if let Ok(entries) = fs::read_dir(NameFix::add_long_path_prefix(&self.full_name)) {
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

    pub fn get_files(&self, search_pattern: &str) -> Vec<FileInfo> {
        let mut files = Vec::new();
        if !self.exists {
            return files;
        }

        let pattern = if search_pattern.is_empty() {
            "*"
        } else {
            search_pattern
        };
        if let Ok(entries) = fs::read_dir(NameFix::add_long_path_prefix(&self.full_name)) {
            for entry in entries.flatten() {
                if let Ok(metadata) = entry.metadata() {
                    if metadata.is_file() {
                        let name = entry.file_name().to_string_lossy().into_owned();
                        if wildcard_match(&name, pattern) {
                            files.push(FileInfo::new(&entry.path().to_string_lossy()));
                        }
                    }
                }
            }
        }
        files
    }
}

fn ticks_from_unix_duration(secs: u64, nanos: u32) -> i64 {
    const TICKS_AT_UNIX_EPOCH: i64 = 621355968000000000;
    const TICKS_PER_SECOND: i64 = 10_000_000;
    TICKS_AT_UNIX_EPOCH + (secs as i64) * TICKS_PER_SECOND + (nanos as i64) / 100
}

fn wildcard_match(name: &str, pattern: &str) -> bool {
    let (n_owned, p_owned);
    let (n, p) = if cfg!(windows) {
        n_owned = name.to_ascii_lowercase();
        p_owned = pattern.to_ascii_lowercase();
        (n_owned.as_str(), p_owned.as_str())
    } else {
        (name, pattern)
    };

    let n_bytes = n.as_bytes();
    let p_bytes = p.as_bytes();
    let mut ni = 0usize;
    let mut pi = 0usize;
    let mut star_pi: Option<usize> = None;
    let mut match_ni = 0usize;

    while ni < n_bytes.len() {
        if pi < p_bytes.len() && (p_bytes[pi] == b'?' || p_bytes[pi] == n_bytes[ni]) {
            ni += 1;
            pi += 1;
            continue;
        }
        if pi < p_bytes.len() && p_bytes[pi] == b'*' {
            star_pi = Some(pi);
            pi += 1;
            match_ni = ni;
            continue;
        }
        if let Some(sp) = star_pi {
            pi = sp + 1;
            match_ni += 1;
            ni = match_ni;
            continue;
        }
        return false;
    }

    while pi < p_bytes.len() && p_bytes[pi] == b'*' {
        pi += 1;
    }
    pi == p_bytes.len()
}
