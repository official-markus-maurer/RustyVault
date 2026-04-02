use std::path::Path as StdPath;

use crate::name_fix::NameFix;

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
    pub fn dir_separator_char() -> char {
        if cfg!(unix) { '/' } else { '\\' }
    }

    pub fn fix_slash(path: &str) -> String {
        if cfg!(unix) {
            path.replace('\\', "/")
        } else {
            path.to_string()
        }
    }

    pub fn get_extension(path: &str) -> String {
        StdPath::new(&NameFix::add_long_path_prefix(path))
            .extension()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned()
    }

    pub fn get_file_name(path: &str) -> String {
        StdPath::new(&NameFix::add_long_path_prefix(path))
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned()
    }

    pub fn get_file_name_without_extension(path: &str) -> String {
        StdPath::new(&NameFix::add_long_path_prefix(path))
            .file_stem()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned()
    }

    pub fn get_directory_name(path: &str) -> String {
        if cfg!(unix) {
            return StdPath::new(path)
                .parent()
                .map(|p| p.to_string_lossy().into_owned())
                .unwrap_or_default();
        }

        let last_pos = path.rfind('\\');
        let last_pos1 = path.rfind('/');
        let use_pos = match (last_pos, last_pos1) {
            (None, None) => None,
            (Some(a), None) => Some(a),
            (None, Some(b)) => Some(b),
            (Some(a), Some(b)) => Some(a.max(b)),
        };
        let Some(i) = use_pos else { return String::new() };
        path[..i].to_string()
    }

    pub fn combine(path1: &str, path2: &str) -> String {
        if cfg!(unix) {
            return StdPath::new(path1).join(path2).to_string_lossy().into_owned();
        }

        if path2.is_empty() {
            return path1.to_string();
        }
        if path1.is_empty() {
            return path2.to_string();
        }
        if Self::is_path_rooted(path2) {
            return path2.to_string();
        }

        let ch = path1.chars().last().unwrap_or('\\');
        if ch != '\\' && ch != '/' && ch != ':' {
            format!("{}\\{}", path1, path2)
        } else {
            format!("{}{}", path1, path2)
        }
    }

    fn is_path_rooted(path: &str) -> bool {
        let bytes = path.as_bytes();
        if bytes.is_empty() {
            return false;
        }
        if bytes[0] == b'\\' || bytes[0] == b'/' {
            return true;
        }
        bytes.len() >= 2 && bytes[1] == b':'
    }
}
