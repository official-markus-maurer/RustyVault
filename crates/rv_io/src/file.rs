use std::fs;
use std::io::{self, Read, Write};
use std::path::Path as StdPath;

pub struct File;

impl File {
    pub fn exists(path: &str) -> bool {
        StdPath::new(path).is_file()
    }

    pub fn delete(path: &str) -> io::Result<()> {
        fs::remove_file(path)
    }

    pub fn move_file(source: &str, dest: &str) -> io::Result<()> {
        fs::rename(source, dest)
    }

    pub fn copy(source: &str, dest: &str, overwrite: bool) -> io::Result<u64> {
        if !overwrite && StdPath::new(dest).exists() {
            return Err(io::Error::new(io::ErrorKind::AlreadyExists, "Destination file already exists"));
        }
        fs::copy(source, dest)
    }

    pub fn read_all_bytes(path: &str) -> io::Result<Vec<u8>> {
        fs::read(path)
    }

    pub fn write_all_bytes(path: &str, bytes: &[u8]) -> io::Result<()> {
        fs::write(path, bytes)
    }

    pub fn get_last_write_time(path: &str) -> io::Result<u64> {
        let metadata = fs::metadata(path)?;
        if let Ok(time) = metadata.modified() {
            if let Ok(dur) = time.duration_since(std::time::UNIX_EPOCH) {
                return Ok(dur.as_secs());
            }
        }
        Ok(0)
    }
}
