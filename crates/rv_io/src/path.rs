use std::path::Path as StdPath;

/// Cross-platform wrapper for path string manipulation.
/// 
/// `Path` mimics the static methods of the C# `System.IO.Path` class,
/// providing standard wrappers for extracting file extensions, names, and directories
/// from string paths.
/// 
/// Differences from C#:
/// - Internally utilizes Rust's highly robust `std::path::Path` rather than raw string splitting.
pub struct Path;

impl Path {
    pub fn get_file_name(path: &str) -> String {
        StdPath::new(path)
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned()
    }

    pub fn combine(path1: &str, path2: &str) -> String {
        StdPath::new(path1)
            .join(path2)
            .to_string_lossy()
            .into_owned()
    }
}
