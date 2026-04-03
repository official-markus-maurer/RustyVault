use std::fs;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;
use std::sync::Arc;
use md5::{Md5, Digest};
use sha1::Sha1;
use sha2::Sha256;
use crc32fast::Hasher as Crc32Hasher;
use rayon::prelude::*;
use crate::chd::{parse_chd_header_from_reader, parse_chd_header_from_bytes};

use dat_reader::enums::{FileType, GotStatus, HeaderFileType};
use compress::i_compress::ICompress;
use compress::zip_file::ZipFile;
use compress::seven_zip::SevenZipFile;
use compress::raw_file::RawFile;
use compress::zip_enums::ZipReturn;
use file_header_reader::FileHeaders;

use crate::rv_file::FileStatus;
use crate::scanned_file::ScannedFile;

/// Core physical file scanning and hashing engine.
/// 
/// `Scanner` is responsible for interacting with the physical disk and the `compress` crate 
/// to open files and archives, extract their headers, and perform deep cryptographic hashing
/// (CRC32, SHA1, MD5) on their contents.
/// 
/// Differences from C#:
/// - The C# `Scanner` is highly integrated with multi-threaded ThreadPools (`ThreadWorker`) to
///   concurrently hash large files and chunks of ZIPs in memory.
/// - The Rust version utilizes the `rayon` parallel iterator ecosystem to automatically distribute
///   recursive directory scanning and file hashing across all available CPU cores, providing 
///   equivalent or faster throughput without manual thread pool management.
pub struct Scanner;

fn wildcard_match(pattern: &str, text: &str) -> bool {
    let p = pattern.as_bytes();
    let t = text.as_bytes();
    let mut pi = 0usize;
    let mut ti = 0usize;
    let mut star: Option<usize> = None;
    let mut star_match_ti = 0usize;

    while ti < t.len() {
        if pi < p.len() && (p[pi] == b'?' || p[pi] == t[ti]) {
            pi += 1;
            ti += 1;
            continue;
        }
        if pi < p.len() && p[pi] == b'*' {
            star = Some(pi);
            pi += 1;
            star_match_ti = ti;
            continue;
        }
        if let Some(star_pi) = star {
            pi = star_pi + 1;
            star_match_ti += 1;
            ti = star_match_ti;
            continue;
        }
        return false;
    }

    while pi < p.len() && p[pi] == b'*' {
        pi += 1;
    }
    pi == p.len()
}

fn is_ignored_file_name(file_name: &str, patterns: &[String]) -> bool {
    #[cfg(windows)]
    let name = file_name.to_ascii_lowercase();
    #[cfg(not(windows))]
    let name = file_name.to_string();

    for pat in patterns {
        #[cfg(windows)]
        let p = pat.to_ascii_lowercase();
        #[cfg(not(windows))]
        let p = pat.to_string();

        if p.contains('*') || p.contains('?') {
            if wildcard_match(&p, &name) {
                return true;
            }
        } else if p == name {
            return true;
        }
    }
    false
}

impl Scanner {
    /// Opens an archive (or raw file) and scans its internal directory structure.
    /// If `deep_scan` is true, it will also calculate cryptographic hashes for every file inside.
    pub fn scan_archive_file(
        archive_type: FileType,
        filename: &str,
        time_stamp: i64,
        deep_scan: bool,
    ) -> Result<ScannedFile, ZipReturn> {
        let mut file: Box<dyn ICompress> = match archive_type {
            FileType::Zip => Box::new(ZipFile::new()),
            FileType::SevenZip => Box::new(SevenZipFile::new()),
            _ => Box::new(RawFile::new()),
        };

        let zr = file.zip_file_open(filename, time_stamp, true);
        if zr != ZipReturn::ZipGood {
            return Err(zr);
        }

        let mut scanned_archive = ScannedFile::new(archive_type);
        scanned_archive.name = filename.to_string();
        let z_struct = file.zip_struct();
        scanned_archive.zip_struct = match z_struct {
            compress::ZipStructure::ZipTrrnt => dat_reader::enums::ZipStructure::ZipTrrnt,
            compress::ZipStructure::ZipTDC => dat_reader::enums::ZipStructure::ZipTDC,
            compress::ZipStructure::SevenZipTrrnt => dat_reader::enums::ZipStructure::SevenZipTrrnt,
            compress::ZipStructure::ZipZSTD => dat_reader::enums::ZipStructure::ZipZSTD,
            compress::ZipStructure::SevenZipSLZMA => dat_reader::enums::ZipStructure::SevenZipSLZMA,
            compress::ZipStructure::SevenZipNLZMA => dat_reader::enums::ZipStructure::SevenZipNLZMA,
            compress::ZipStructure::SevenZipSZSTD => dat_reader::enums::ZipStructure::SevenZipSZSTD,
            compress::ZipStructure::SevenZipNZSTD => dat_reader::enums::ZipStructure::SevenZipNZSTD,
            _ => dat_reader::enums::ZipStructure::None,
        };
        scanned_archive.comment = file.file_comment().to_string();

        let files = Self::scan_files_in_archive(file.as_mut(), deep_scan);
        scanned_archive.children = files;

        file.zip_file_close();
        Ok(scanned_archive)
    }

    fn scan_files_in_archive(file: &mut dyn ICompress, deep_scan: bool) -> Vec<ScannedFile> {
        let file_count = file.local_files_count();
        let scanned_file_type = FileType::File; 

        // Extract all headers sequentially first because `file` is mutable and not Sync
        let mut file_headers = Vec::with_capacity(file_count);
        for i in 0..file_count {
            if let Some(lf) = file.get_file_header(i) {
                // Clone the FileHeader so we drop the immutable borrow on `file` immediately
                file_headers.push((i, lf.clone()));
            }
        }

        // We can't parallelize the stream reading across multiple threads for the SAME archive 
        // because ICompress stream access is stateful.
        // However, we CAN parallelize the outer `scan_directory` loop which processes MULTIPLE archives at once.
        let mut results = Vec::with_capacity(file_headers.len());

        for (i, lf) in file_headers {
            let mut scanned_file = ScannedFile::new(scanned_file_type);
            let mut do_deep_scan = false;
            let mut lf_crc = None;
            let mut _lf_is_dir = false;
            
            scanned_file.name = lf.filename.clone();
            scanned_file.deep_scanned = deep_scan;
            scanned_file.index = i as i32;
            scanned_file.local_header_offset = lf.local_head;
            scanned_file.file_mod_time_stamp = lf.last_modified();
            
            _lf_is_dir = lf.is_directory;

            if lf.is_directory {
                scanned_file.header_file_type = HeaderFileType::NOTHING;
                scanned_file.got_status = GotStatus::Got;
                scanned_file.size = Some(0);
                scanned_file.crc = Some(vec![0, 0, 0, 0]);
                scanned_file.sha1 = Some(vec![0xda, 0x39, 0xa3, 0xee, 0x5e, 0x6b, 0x4b, 0x0d, 0x32, 0x55, 0xbf, 0xef, 0x95, 0x60, 0x18, 0x90, 0xaf, 0xd8, 0x07, 0x09]);
                scanned_file.md5 = Some(vec![0xd4, 0x1d, 0x8c, 0xd9, 0x8f, 0x00, 0xb2, 0x04, 0xe9, 0x80, 0x09, 0x98, 0xec, 0xf8, 0x42, 0x7e]);
                scanned_file.sha256 = Some(vec![
                    0xe3, 0xb0, 0xc4, 0x42, 0x98, 0xfc, 0x1c, 0x14, 0x9a, 0xfb, 0xf4, 0xc8, 0x99, 0x6f,
                    0xb9, 0x24, 0x27, 0xae, 0x41, 0xe4, 0x64, 0x9b, 0x93, 0x4c, 0xa4, 0x95, 0x99, 0x1b,
                    0x78, 0x52, 0xb8, 0x55,
                ]);
            } else {
                scanned_file.size = Some(lf.uncompressed_size);
                scanned_file.crc = lf.crc.clone();
                lf_crc = lf.crc.clone();
                do_deep_scan = deep_scan;
                scanned_file.status_flags.insert(FileStatus::SIZE_FROM_HEADER);
                if scanned_file.crc.is_some() {
                    scanned_file.status_flags.insert(FileStatus::CRC_FROM_HEADER);
                }
            }

            if !_lf_is_dir {
                if !do_deep_scan {
                    scanned_file.got_status = GotStatus::Got;
                    let stream_res = file.zip_file_open_read_stream(i);
                    match stream_res {
                        Ok((mut stream, _size)) => {
                            let mut alt_crc_hasher = Crc32Hasher::new();
                            let mut header_probe = Vec::with_capacity(512);
                            let mut header_file_type = HeaderFileType::NOTHING;
                            let mut header_size = 0usize;
                            let mut total_read = 0usize;

                            let mut buffer = [0u8; 32768];
                            loop {
                                match stream.read(&mut buffer) {
                                    Ok(0) => break,
                                    Ok(n) => {
                                        if header_probe.len() < 512 {
                                            let probe_take =
                                                std::cmp::min(512 - header_probe.len(), n);
                                            header_probe.extend_from_slice(&buffer[..probe_take]);
                                            let (detected_type, detected_size) =
                                                FileHeaders::get_file_type_from_buffer(&header_probe);
                                            header_file_type = detected_type;
                                            header_size = detected_size;
                                            if header_file_type == HeaderFileType::CHD
                                                && crate::settings::get_settings().check_chd_version
                                                && scanned_file.chd_version.is_none()
                                            {
                                                if let Some(info) = parse_chd_header_from_bytes(&header_probe) {
                                                    scanned_file.chd_version = Some(info.version);
                                                    if let Some(sha1) = info.sha1 {
                                                        scanned_file.alt_sha1 = Some(sha1);
                                                        scanned_file.status_flags.insert(FileStatus::ALT_SHA1_FROM_HEADER);
                                                    }
                                                    if let Some(md5) = info.md5 {
                                                        scanned_file.alt_md5 = Some(md5);
                                                        scanned_file.status_flags.insert(FileStatus::ALT_MD5_FROM_HEADER);
                                                    }
                                                }
                                            }
                                        }

                                        if header_size > 0 {
                                            let chunk_start = total_read;
                                            let chunk_end = total_read + n;
                                            if chunk_end > header_size {
                                                let alt_start = header_size.saturating_sub(chunk_start);
                                                alt_crc_hasher.update(&buffer[alt_start..n]);
                                            }
                                        }
                                        total_read += n;
                                    }
                                    Err(_) => {
                                        scanned_file.got_status = GotStatus::Corrupt;
                                        break;
                                    }
                                }
                            }

                            if scanned_file.got_status != GotStatus::Corrupt {
                                scanned_file.header_file_type = header_file_type;
                                if header_file_type != HeaderFileType::NOTHING {
                                    scanned_file
                                        .status_flags
                                        .insert(FileStatus::HEADER_FILE_TYPE_FROM_HEADER);
                                }
                                if header_size > 0
                                    && scanned_file.size.unwrap_or(0) >= header_size as u64
                                {
                                    scanned_file.alt_size = Some(
                                        scanned_file.size.unwrap_or(0) - header_size as u64,
                                    );
                                    scanned_file.alt_crc =
                                        Some(alt_crc_hasher.finalize().to_be_bytes().to_vec());
                                    scanned_file
                                        .status_flags
                                        .insert(FileStatus::ALT_SIZE_FROM_HEADER | FileStatus::ALT_CRC_FROM_HEADER);
                                }
                            }

                            let _ = file.zip_file_close_read_stream();
                        }
                        Err(_) => {
                            scanned_file.got_status = GotStatus::Corrupt;
                            scanned_file.crc = lf_crc;
                        }
                    }
                } else {
                    // Deep Scan logic: stream and hash
                    let stream_res = file.zip_file_open_read_stream(i);
                    match stream_res {
                        Ok((mut stream, _size)) => {
                            let mut md5_hasher = Md5::new();
                            let mut sha1_hasher = Sha1::new();
                            let mut sha256_hasher = Sha256::new();
                            let mut crc_hasher = Crc32Hasher::new();
                            let mut alt_md5_hasher = Md5::new();
                            let mut alt_sha1_hasher = Sha1::new();
                            let mut alt_sha256_hasher = Sha256::new();
                            let mut alt_crc_hasher = Crc32Hasher::new();
                            let mut header_probe = Vec::with_capacity(512);
                            let mut header_file_type = HeaderFileType::NOTHING;
                            let mut header_size = 0usize;
                            let mut total_read = 0usize;
                            
                            let mut buffer = [0u8; 32768]; // Increased buffer size for faster hashing
                            loop {
                                match stream.read(&mut buffer) {
                                    Ok(0) => break,
                                    Ok(n) => {
                                        if header_probe.len() < 512 {
                                            let probe_take = std::cmp::min(512 - header_probe.len(), n);
                                            header_probe.extend_from_slice(&buffer[..probe_take]);
                                            let (detected_type, detected_size) =
                                                FileHeaders::get_file_type_from_buffer(&header_probe);
                                            header_file_type = detected_type;
                                            header_size = detected_size;
                                            if header_file_type == HeaderFileType::CHD
                                                && crate::settings::get_settings().check_chd_version
                                                && scanned_file.chd_version.is_none()
                                            {
                                                if let Some(info) = parse_chd_header_from_bytes(&header_probe) {
                                                    scanned_file.chd_version = Some(info.version);
                                                    if let Some(sha1) = info.sha1 {
                                                        scanned_file.alt_sha1 = Some(sha1);
                                                        scanned_file.status_flags.insert(FileStatus::ALT_SHA1_FROM_HEADER);
                                                    }
                                                    if let Some(md5) = info.md5 {
                                                        scanned_file.alt_md5 = Some(md5);
                                                        scanned_file.status_flags.insert(FileStatus::ALT_MD5_FROM_HEADER);
                                                    }
                                                }
                                            }
                                        }

                                        md5_hasher.update(&buffer[..n]);
                                        sha1_hasher.update(&buffer[..n]);
                                        sha256_hasher.update(&buffer[..n]);
                                        crc_hasher.update(&buffer[..n]);

                                        if header_size > 0 {
                                            let chunk_start = total_read;
                                            let chunk_end = total_read + n;
                                            if chunk_end > header_size {
                                                let alt_start = header_size.saturating_sub(chunk_start);
                                                alt_md5_hasher.update(&buffer[alt_start..n]);
                                                alt_sha1_hasher.update(&buffer[alt_start..n]);
                                                alt_sha256_hasher.update(&buffer[alt_start..n]);
                                                alt_crc_hasher.update(&buffer[alt_start..n]);
                                            }
                                        }
                                        total_read += n;
                                    }
                                    Err(_) => {
                                        scanned_file.got_status = GotStatus::Corrupt;
                                        break;
                                    }
                                }
                            }
                            
                            if scanned_file.got_status != GotStatus::Corrupt {
                                let computed_crc = crc_hasher.finalize().to_be_bytes().to_vec();
                                if let Some(ref existing_crc) = scanned_file.crc {
                                    if existing_crc != &computed_crc {
                                        scanned_file.got_status = GotStatus::Corrupt;
                                    }
                                } else {
                                    scanned_file.crc = Some(computed_crc);
                                }
                                scanned_file.header_file_type = header_file_type;
                                if header_file_type != HeaderFileType::NOTHING {
                                    scanned_file
                                        .status_flags
                                        .insert(FileStatus::HEADER_FILE_TYPE_FROM_HEADER);
                                }
                                if header_size > 0 && scanned_file.size.unwrap_or(0) >= header_size as u64 {
                                    scanned_file.alt_size = Some(scanned_file.size.unwrap_or(0) - header_size as u64);
                                    scanned_file.alt_crc = Some(alt_crc_hasher.finalize().to_be_bytes().to_vec());
                                    scanned_file.alt_sha1 = Some(alt_sha1_hasher.finalize().to_vec());
                                    scanned_file.alt_md5 = Some(alt_md5_hasher.finalize().to_vec());
                                    scanned_file.alt_sha256 = Some(alt_sha256_hasher.finalize().to_vec());
                                    scanned_file.status_flags.insert(
                                        FileStatus::ALT_SIZE_FROM_HEADER
                                            | FileStatus::ALT_CRC_FROM_HEADER
                                            | FileStatus::ALT_SHA1_FROM_HEADER
                                            | FileStatus::ALT_MD5_FROM_HEADER
                                            | FileStatus::ALT_SHA256_FROM_HEADER,
                                    );
                                }
                                scanned_file.sha1 = Some(sha1_hasher.finalize().to_vec());
                                scanned_file.md5 = Some(md5_hasher.finalize().to_vec());
                                scanned_file.sha256 = Some(sha256_hasher.finalize().to_vec());
                                if scanned_file.got_status != GotStatus::Corrupt {
                                    scanned_file.got_status = GotStatus::Got;
                                }
                            }
                            let _ = file.zip_file_close_read_stream();
                        },
                        Err(_) => {
                            scanned_file.got_status = GotStatus::Corrupt;
                            scanned_file.crc = lf_crc;
                        }
                    }
                }
            }

            results.push(scanned_file);
        }

        results
    }

    /// Scans a single uncompressed file on disk.
    pub fn scan_raw_file(file_path: &str) -> Result<ScannedFile, std::io::Error> {
        let metadata = fs::metadata(file_path)?;
        let path = Path::new(file_path);
        let file_name = path.file_name().unwrap_or_default().to_string_lossy().to_string();
        
        let mut sf = ScannedFile::new(FileType::File);
        sf.name = file_name;
        if let Ok(mod_time) = metadata.modified() {
            if let Ok(dur) = mod_time.duration_since(std::time::UNIX_EPOCH) {
                sf.file_mod_time_stamp = dur.as_secs() as i64;
            }
        }
        sf.size = Some(metadata.len());

        let mut file = fs::File::open(file_path)?;
        let (header_file_type, header_size) = FileHeaders::get_file_type_from_stream(&mut file)
            .unwrap_or((HeaderFileType::NOTHING, 0));
        file.seek(SeekFrom::Start(0))?;

        let mut md5_hasher = Md5::new();
        let mut sha1_hasher = Sha1::new();
        let mut sha256_hasher = Sha256::new();
        let mut crc_hasher = Crc32Hasher::new();
        let mut alt_md5_hasher = Md5::new();
        let mut alt_sha1_hasher = Sha1::new();
        let mut alt_sha256_hasher = Sha256::new();
        let mut alt_crc_hasher = Crc32Hasher::new();
        let mut total_read = 0usize;
        
        let mut buffer = [0u8; 8192];
        loop {
            let n = file.read(&mut buffer)?;
            if n == 0 { break; }
            md5_hasher.update(&buffer[..n]);
            sha1_hasher.update(&buffer[..n]);
            sha256_hasher.update(&buffer[..n]);
            crc_hasher.update(&buffer[..n]);

            if header_size > 0 {
                let chunk_start = total_read;
                let chunk_end = total_read + n;
                if chunk_end > header_size {
                    let alt_start = header_size.saturating_sub(chunk_start);
                    alt_md5_hasher.update(&buffer[alt_start..n]);
                    alt_sha1_hasher.update(&buffer[alt_start..n]);
                    alt_sha256_hasher.update(&buffer[alt_start..n]);
                    alt_crc_hasher.update(&buffer[alt_start..n]);
                }
            }
            total_read += n;
        }
        
        sf.header_file_type = header_file_type;
        if header_file_type != HeaderFileType::NOTHING {
            sf.status_flags.insert(FileStatus::HEADER_FILE_TYPE_FROM_HEADER);
        }
        sf.crc = Some(crc_hasher.finalize().to_be_bytes().to_vec());
        sf.sha1 = Some(sha1_hasher.finalize().to_vec());
        sf.md5 = Some(md5_hasher.finalize().to_vec());
        sf.sha256 = Some(sha256_hasher.finalize().to_vec());
        if header_size > 0 && metadata.len() >= header_size as u64 {
            sf.alt_size = Some(metadata.len() - header_size as u64);
            sf.alt_crc = Some(alt_crc_hasher.finalize().to_be_bytes().to_vec());
            sf.alt_sha1 = Some(alt_sha1_hasher.finalize().to_vec());
            sf.alt_md5 = Some(alt_md5_hasher.finalize().to_vec());
            sf.alt_sha256 = Some(alt_sha256_hasher.finalize().to_vec());
            sf.status_flags.insert(
                FileStatus::ALT_SIZE_FROM_HEADER
                    | FileStatus::ALT_CRC_FROM_HEADER
                    | FileStatus::ALT_SHA1_FROM_HEADER
                    | FileStatus::ALT_MD5_FROM_HEADER
                    | FileStatus::ALT_SHA256_FROM_HEADER,
            );
        }

        if header_file_type == HeaderFileType::CHD && crate::settings::get_settings().check_chd_version {
            file.seek(SeekFrom::Start(0))?;
            if let Some(info) = parse_chd_header_from_reader(&mut file) {
                sf.chd_version = Some(info.version);
                if let Some(sha1) = info.sha1 {
                    sf.alt_sha1 = Some(sha1);
                    sf.status_flags.insert(FileStatus::ALT_SHA1_FROM_HEADER);
                }
                if let Some(md5) = info.md5 {
                    sf.alt_md5 = Some(md5);
                    sf.status_flags.insert(FileStatus::ALT_MD5_FROM_HEADER);
                }
            } else {
                sf.got_status = GotStatus::Corrupt;
            }
        }
        sf.deep_scanned = true;
        if sf.got_status != GotStatus::Corrupt {
            sf.got_status = GotStatus::Got;
        }
        
        Ok(sf)
    }

    /// Recursively scans a physical directory.
    pub fn scan_directory(path_str: &str) -> Vec<ScannedFile> {
        Self::scan_directory_with_level(path_str, crate::settings::EScanLevel::Level1)
    }

    pub fn scan_directory_with_level(
        path_str: &str,
        scan_level: crate::settings::EScanLevel,
    ) -> Vec<ScannedFile> {
        let path = Path::new(path_str);
        let deep_scan = matches!(
            scan_level,
            crate::settings::EScanLevel::Level2 | crate::settings::EScanLevel::Level3
        );
        
        if let Ok(entries) = fs::read_dir(path) {
            let entry_list: Vec<_> = entries.flatten().collect();
            let ignore_patterns = Arc::new(crate::settings::get_settings().ignore_files.items.clone());

            // Parallelize directory entry processing!
            let results: Vec<ScannedFile> = entry_list
                .into_par_iter()
                .filter_map(|entry| {
                let metadata = entry.metadata().unwrap();
                let file_name = entry.file_name().to_string_lossy().to_string();

                if !metadata.is_dir() && file_name.starts_with("__RomVault.") && file_name.ends_with(".tmp") {
                    let _ = fs::remove_file(entry.path());
                    return None;
                }

                if is_ignored_file_name(&file_name, &ignore_patterns) {
                    return None;
                }
                
                let file_type = if metadata.is_dir() {
                    FileType::Dir
                } else {
                    let lower_name = file_name.to_lowercase();
                    if lower_name.ends_with(".zip") {
                        FileType::Zip
                    } else if lower_name.ends_with(".7z") {
                        FileType::SevenZip
                    } else {
                        FileType::File
                    }
                };

                let mut sf = ScannedFile::new(file_type);
                sf.name = file_name.clone();
                if let Ok(mod_time) = metadata.modified() {
                    if let Ok(dur) = mod_time.duration_since(std::time::UNIX_EPOCH) {
                        sf.file_mod_time_stamp = dur.as_secs() as i64;
                    }
                }
                
                if file_type == FileType::File {
                    if deep_scan {
                        let file_path = path.join(&file_name);
                        if let Ok(scanned_file) = Self::scan_raw_file(&file_path.to_string_lossy()) {
                            sf = scanned_file;
                        } else {
                            sf.size = Some(metadata.len());
                        }
                    } else {
                        sf.size = Some(metadata.len());
                    }
                } else if file_type == FileType::Dir {
                    // Recursively scan directories
                    let sub_path = path.join(&file_name);
                    sf.children = Self::scan_directory_with_level(&sub_path.to_string_lossy(), scan_level);
                } else if file_type == FileType::Zip || file_type == FileType::SevenZip {
                    // Quick scan archive contents without deep scan, or fully hash contents for deeper levels
                    let archive_path = path.join(&file_name);
                    if let Ok(archive_sf) = Self::scan_archive_file(
                        file_type,
                        &archive_path.to_string_lossy(),
                        sf.file_mod_time_stamp,
                        deep_scan,
                    ) {
                        sf.children = archive_sf.children;
                    }
                }
                Some(sf)
            })
            .collect();
            
            return results;
        }
        
        Vec::new()
    }
}

#[cfg(test)]
#[path = "tests/scanner_tests.rs"]
mod tests;
