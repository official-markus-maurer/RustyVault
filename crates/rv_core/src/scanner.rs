use std::fs;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;
use std::sync::Arc;

use crc32fast::Hasher as Crc32Hasher;
use md5::{Digest, Md5};
use rayon::prelude::*;
use sha1::Sha1;
use sha2::Sha256;

use compress::i_compress::ICompress;
use compress::raw_file::RawFile;
use compress::seven_zip::SevenZipFile;
use compress::zip_enums::ZipReturn;
use compress::zip_file::ZipFile;
use dat_reader::enums::{FileType, GotStatus, HeaderFileType};
use file_header_reader::FileHeaders;

use crate::chd::{parse_chd_header_from_bytes, parse_chd_header_from_reader};
use crate::rv_file::FileStatus;
use crate::scanned_file::ScannedFile;

pub struct Scanner;

fn is_ignored_file_name(file_name: &str, matcher: &crate::patterns::PatternMatcher) -> bool {
    matcher.is_match(file_name)
}

include!("scanner/archive.rs");
include!("scanner/raw.rs");
include!("scanner/directory.rs");

#[cfg(test)]
#[path = "tests/scanner_tests.rs"]
mod tests;
