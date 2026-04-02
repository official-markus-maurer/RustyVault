use std::fs;
use std::io;
use std::path::Path as StdPath;
use crate::name_fix::NameFix;

/// Cross-platform wrapper for directory operations.
/// 
/// `Directory` mimics the static methods of the C# `System.IO.Directory` class,
/// providing standard wrappers for checking existence, creating directories, and
/// enumerating files/folders.
/// 
/// Differences from C#:
/// - Internally delegates to `std::fs` rather than requiring P/Invoke `kernel32.dll` 
///   long-path hacks, since Rust supports modern Windows `\\?\` natively.
pub struct Directory;

impl Directory {
    pub fn exists(path: &str) -> bool {
        StdPath::new(&NameFix::add_long_path_prefix(path)).is_dir()
    }

    pub fn move_dir(source: &str, dest: &str) -> io::Result<()> {
        fs::rename(NameFix::add_long_path_prefix(source), NameFix::add_long_path_prefix(dest))
    }

    pub fn delete(path: &str) -> io::Result<()> {
        fs::remove_dir_all(NameFix::add_long_path_prefix(path))
    }

    pub fn create_directory(path: &str) -> io::Result<()> {
        fs::create_dir_all(NameFix::add_long_path_prefix(path))
    }

    pub fn get_files(path: &str) -> io::Result<Vec<String>> {
        let mut files = Vec::new();
        for entry in fs::read_dir(NameFix::add_long_path_prefix(path))? {
            let entry = entry?;
            if entry.metadata()?.is_file() {
                files.push(entry.path().to_string_lossy().into_owned());
            }
        }
        Ok(files)
    }

    pub fn get_directories(path: &str) -> io::Result<Vec<String>> {
        let mut dirs = Vec::new();
        for entry in fs::read_dir(NameFix::add_long_path_prefix(path))? {
            let entry = entry?;
            if entry.metadata()?.is_dir() {
                dirs.push(entry.path().to_string_lossy().into_owned());
            }
        }
        Ok(dirs)
    }
}
