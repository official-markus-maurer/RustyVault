use crate::name_fix::NameFix;
use std::fs;
use std::io;
use std::path::Path as StdPath;

/// Cross-platform wrapper for file operations.
///
/// `File` mimics the static methods of the C# `System.IO.File` class,
/// providing standard wrappers for moving, copying, and deleting physical files.
///
/// Differences from C#:
/// - Similar to `Directory`, this bypasses the need for the legacy C# `RVIO` long-path
///   workarounds by relying on Rust's `std::fs` abstractions.
pub struct File;

impl File {
    pub fn exists(path: &str) -> bool {
        StdPath::new(&NameFix::add_long_path_prefix(path)).is_file()
    }

    pub fn delete(path: &str) -> io::Result<()> {
        fs::remove_file(NameFix::add_long_path_prefix(path))
    }

    pub fn move_file(source: &str, dest: &str) -> io::Result<()> {
        fs::rename(
            NameFix::add_long_path_prefix(source),
            NameFix::add_long_path_prefix(dest),
        )
    }

    pub fn copy(source: &str, dest: &str, overwrite: bool) -> io::Result<u64> {
        if !overwrite && StdPath::new(&NameFix::add_long_path_prefix(dest)).exists() {
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                "Destination file already exists",
            ));
        }
        fs::copy(
            NameFix::add_long_path_prefix(source),
            NameFix::add_long_path_prefix(dest),
        )
    }

    pub fn read_all_bytes(path: &str) -> io::Result<Vec<u8>> {
        fs::read(NameFix::add_long_path_prefix(path))
    }

    pub fn write_all_bytes(path: &str, bytes: &[u8]) -> io::Result<()> {
        fs::write(NameFix::add_long_path_prefix(path), bytes)
    }

    pub fn get_last_write_time(path: &str) -> io::Result<u64> {
        let metadata = fs::metadata(NameFix::add_long_path_prefix(path))?;
        if let Ok(time) = metadata.modified() {
            if let Ok(dur) = time.duration_since(std::time::UNIX_EPOCH) {
                const TICKS_AT_UNIX_EPOCH: i64 = 621355968000000000;
                const TICKS_PER_SECOND: i64 = 10_000_000;
                let ticks = TICKS_AT_UNIX_EPOCH
                    + (dur.as_secs() as i64) * TICKS_PER_SECOND
                    + (dur.subsec_nanos() as i64) / 100;
                return Ok(ticks.max(0) as u64);
            }
        }
        Ok(0)
    }
}
