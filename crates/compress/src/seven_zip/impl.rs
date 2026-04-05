use std::cell::RefCell;
use std::collections::HashMap;
use std::fs;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::rc::Rc;

use crc32fast::Hasher as Crc32Hasher;

use crate::file_header::FileHeader;
use crate::i_compress::ICompress;
use crate::structured_archive::ZipStructure;
use crate::zip_enums::{ZipOpenType, ZipReturn};

use sevenz_rust::encoder_options::{EncoderOptions, LzmaOptions, ZstandardOptions};
use sevenz_rust::{
    Archive, ArchiveEntry, ArchiveWriter, EncoderConfiguration, EncoderMethod, Password,
    SourceReader,
};

use internals::{SevenZipPendingWrite, SharedFileWriter};

/// ICompress wrapper for `.7z` archives.
///
/// `SevenZipFile` implements the `ICompress` trait for 7z files, allowing the scanner to
/// open, read headers, and extract payloads from 7-Zip archives.
///
/// Implementation notes:
/// - Uses the `sevenz-rust` crate for archive reading/writing.
/// - Extraction currently materializes the file payload into memory for read streams.
///
/// TODO: Support streaming extraction for solid archives to reduce peak memory usage.
pub struct SevenZipFile {
    zip_filename: String,
    zip_open_type: ZipOpenType,
    time_stamp: i64,

    // In read mode, we hold the loaded archive
    archive: Option<Archive>,
    file: Option<File>,
    staging_dir: Option<PathBuf>,
    pending_write: Option<SevenZipPendingWrite>,
    temp_open_path: Option<PathBuf>,

    file_headers: Vec<FileHeader>,
    file_comment: String,
    zip_struct: ZipStructure,
}

include!("read_support.rs");
include!("write_support.rs");
include!("marker.rs");
include!("icompress_impl.rs");

impl SevenZipFile {
    pub fn new() -> Self {
        Self {
            zip_filename: String::new(),
            zip_open_type: ZipOpenType::Closed,
            time_stamp: 0,
            archive: None,
            file: None,
            staging_dir: None,
            pending_write: None,
            temp_open_path: None,
            file_headers: Vec::new(),
            file_comment: String::new(),
            zip_struct: ZipStructure::None,
        }
    }

    pub fn header_report(&self) -> String {
        String::new()
    }
}
