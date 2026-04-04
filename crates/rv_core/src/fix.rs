use std::cell::RefCell;
use std::collections::HashMap;
use std::fs;
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::{SystemTime, UNIX_EPOCH};

#[cfg(test)]
use std::io::Write;

use crate::enums::RepStatus;
use crate::rv_file::{RvFile, TreeSelect};
use dat_reader::enums::{DatStatus, FileType, GotStatus, ZipStructure};
use tracing::{debug, info, trace};
use zip::{DateTime as ZipDateTime, ZipArchive};

#[cfg(test)]
use zip::write::SimpleFileOptions;
#[cfg(test)]
use zip::CompressionMethod;
#[cfg(test)]
use zip::ZipWriter;

mod actions;
mod rebuild_common;
mod rebuild_sevenzip;
mod rebuild_zip;
mod zip_ops;

pub struct Fix;

struct StoredZipEntry {
    compressed_data: Vec<u8>,
    crc: u32,
    compressed_size: u32,
    uncompressed_size: u32,
}

struct ArchiveRebuildEntry {
    node: Rc<RefCell<RvFile>>,
    target_name: String,
    existing_name: String,
    is_directory: bool,
}

struct ArchiveMatchEntry {
    node: Rc<RefCell<RvFile>>,
    logical_name: String,
}

struct TorrentZipBuiltEntry {
    name: String,
    compressed_data: Vec<u8>,
    crc: u32,
    compressed_size: u32,
    uncompressed_size: u32,
    flags: u16,
    compression_method: u16,
    external_attributes: u32,
}

include!("fix/engine.rs");

#[cfg(test)]
#[path = "tests/fix_tests.rs"]
mod tests;
