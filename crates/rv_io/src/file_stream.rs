use std::fs;
use std::io;

use crate::name_fix::NameFix;

pub struct FileStream;

impl FileStream {
    pub const BUF_SIZE_MAX: usize = 128 * 4096;

    pub fn open_file_read(path: &str) -> io::Result<fs::File> {
        fs::File::open(NameFix::add_long_path_prefix(path))
    }

    pub fn open_file_read_with_buffer(path: &str, _buffer_size: usize) -> io::Result<fs::File> {
        fs::File::open(NameFix::add_long_path_prefix(path))
    }

    pub fn open_file_write(path: &str, _buffer_size: usize) -> io::Result<fs::File> {
        fs::File::create(NameFix::add_long_path_prefix(path))
    }
}

