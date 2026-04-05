use std::cell::RefCell;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom, Write};
use std::rc::Rc;

use crc32fast::Hasher as Crc32Hasher;

use crate::codepage_437;
use crate::structured_archive::{get_zip_comment_id, ZipStructure};
use crate::zip_enums::ZipReturn;
use crate::zip_extra_field;

use super::ZipFile;

pub(crate) trait ReadSeek: Read + Seek {}
impl<T: Read + Seek> ReadSeek for T {}

#[derive(Clone)]
pub(crate) struct CentralHeaderMeta {
    #[allow(dead_code)]
    pub(crate) flags: u16,
    pub(crate) compression_method: u16,
    pub(crate) compressed_size: u64,
    pub(crate) uncompressed_size: u64,
    pub(crate) crc32: u32,
    pub(crate) local_header_offset: u64,
    #[allow(dead_code)]
    pub(crate) header_last_modified: i64,
}

pub(crate) struct EocdLocator {
    pub(crate) local_files_count: u64,
    pub(crate) central_directory_size: u64,
    pub(crate) central_directory_offset: u64,
    pub(crate) central_directory_offset_correction: i64,
    pub(crate) comment_bytes: Vec<u8>,
    pub(crate) extra_data_found_on_end: bool,
}

pub(crate) enum ZipWriterFile {
    Memory(std::io::Cursor<Vec<u8>>),
}

impl Write for ZipWriterFile {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        match self {
            ZipWriterFile::Memory(c) => c.write(buf),
        }
    }

    fn flush(&mut self) -> std::io::Result<()> {
        match self {
            ZipWriterFile::Memory(c) => c.flush(),
        }
    }
}

impl Seek for ZipWriterFile {
    fn seek(&mut self, pos: SeekFrom) -> std::io::Result<u64> {
        match self {
            ZipWriterFile::Memory(c) => c.seek(pos),
        }
    }
}

pub(crate) struct PendingWrite {
    pub(crate) filename: String,
    pub(crate) compression_method: u16,
    pub(crate) mod_time: Option<i64>,
    pub(crate) uncompressed_size: u64,
    pub(crate) raw: bool,
    pub(crate) buffer: Rc<RefCell<Vec<u8>>>,
}

pub(crate) struct SharedBufferWriter {
    pub(crate) buffer: Rc<RefCell<Vec<u8>>>,
}

impl Write for SharedBufferWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.buffer.borrow_mut().extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

pub(crate) struct LocalFileHeaderInfo {
    pub(crate) flags: u16,
    pub(crate) compression_method: u16,
    pub(crate) compressed_size: u64,
    pub(crate) uncompressed_size: u64,
    pub(crate) data_offset: u64,
}

pub(crate) struct LocalHeaderFull {
    #[allow(dead_code)]
    pub(crate) flags: u16,
    pub(crate) compression_method: u16,
    pub(crate) crc32: u32,
    pub(crate) compressed_size: u64,
    pub(crate) uncompressed_size: u64,
    #[allow(dead_code)]
    pub(crate) header_last_modified: i64,
    #[allow(dead_code)]
    pub(crate) filename: String,
    #[allow(dead_code)]
    pub(crate) data_offset: u64,
}

include!("zip_file_internal/manual_writer.rs");
