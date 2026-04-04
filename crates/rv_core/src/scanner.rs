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

fn is_ignored_file_name(file_name: &str, patterns: &[String]) -> bool {
    #[cfg(windows)]
    let name = file_name.to_ascii_lowercase();
    #[cfg(not(windows))]
    let name = file_name.to_string();

    for pat in patterns {
        let Some(scan_pat) = crate::patterns::extract_scan_pattern(pat) else {
            continue;
        };
        if scan_pat.is_empty() {
            continue;
        }
        #[cfg(windows)]
        {
            let is_regex = scan_pat.len() >= 6 && scan_pat[..6].eq_ignore_ascii_case("regex:");
            if is_regex {
                if crate::patterns::matches_pattern(file_name, scan_pat) {
                    return true;
                }
            } else {
                let p = scan_pat.to_ascii_lowercase();
                if crate::patterns::matches_pattern(&name, &p) {
                    return true;
                }
            }
        }
        #[cfg(not(windows))]
        {
            if crate::patterns::matches_pattern(&name, scan_pat) {
                return true;
            }
        }
    }
    false
}

include!("scanner/archive.rs");
include!("scanner/raw.rs");
include!("scanner/directory.rs");

#[cfg(test)]
#[path = "tests/scanner_tests.rs"]
mod tests;
