use std::fs;
use std::io;
use std::path::Path as StdPath;

pub struct Directory;

impl Directory {
    pub fn exists(path: &str) -> bool {
        StdPath::new(path).is_dir()
    }

    pub fn move_dir(source: &str, dest: &str) -> io::Result<()> {
        fs::rename(source, dest)
    }

    pub fn delete(path: &str) -> io::Result<()> {
        fs::remove_dir_all(path)
    }

    pub fn create_directory(path: &str) -> io::Result<()> {
        fs::create_dir_all(path)
    }

    pub fn get_files(path: &str) -> io::Result<Vec<String>> {
        let mut files = Vec::new();
        for entry in fs::read_dir(path)? {
            let entry = entry?;
            if entry.metadata()?.is_file() {
                files.push(entry.path().to_string_lossy().into_owned());
            }
        }
        Ok(files)
    }

    pub fn get_directories(path: &str) -> io::Result<Vec<String>> {
        let mut dirs = Vec::new();
        for entry in fs::read_dir(path)? {
            let entry = entry?;
            if entry.metadata()?.is_dir() {
                dirs.push(entry.path().to_string_lossy().into_owned());
            }
        }
        Ok(dirs)
    }
}
