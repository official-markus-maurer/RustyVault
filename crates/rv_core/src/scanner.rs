use std::fs;
use std::io::Read;
use std::path::Path;
use md5::{Md5, Digest};
use sha1::Sha1;
use crc32fast::Hasher as Crc32Hasher;

use dat_reader::enums::{FileType, GotStatus, HeaderFileType};
use compress::i_compress::ICompress;
use compress::zip_file::ZipFile;
use compress::seven_zip::SevenZipFile;
use compress::raw_file::RawFile;
use compress::zip_enums::ZipReturn;

use crate::scanned_file::ScannedFile;

pub struct Scanner;

impl Scanner {
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
        let mut results = Vec::new();
        let file_count = file.local_files_count();

        let scanned_file_type = FileType::File; 

        for i in 0..file_count {
            let mut scanned_file = ScannedFile::new(scanned_file_type);
            let mut do_deep_scan = false;
            let mut lf_crc = None;
            let mut _lf_is_dir = false;
            
            if let Some(lf) = file.get_file_header(i) {
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
                } else {
                    scanned_file.size = Some(lf.uncompressed_size);
                    scanned_file.crc = lf.crc.clone();
                    lf_crc = lf.crc.clone();
                    do_deep_scan = deep_scan;
                }
            } else {
                continue;
            }

            if !_lf_is_dir {
                if !do_deep_scan {
                    scanned_file.got_status = GotStatus::Got;
                } else {
                    // Deep Scan logic: stream and hash
                    let stream_res = file.zip_file_open_read_stream(i);
                    match stream_res {
                        Ok((mut stream, _size)) => {
                            let mut md5_hasher = Md5::new();
                            let mut sha1_hasher = Sha1::new();
                            let mut crc_hasher = Crc32Hasher::new();
                            
                            let mut buffer = [0u8; 8192];
                            loop {
                                match stream.read(&mut buffer) {
                                    Ok(0) => break,
                                    Ok(n) => {
                                        md5_hasher.update(&buffer[..n]);
                                        sha1_hasher.update(&buffer[..n]);
                                        crc_hasher.update(&buffer[..n]);
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
                                scanned_file.sha1 = Some(sha1_hasher.finalize().to_vec());
                                scanned_file.md5 = Some(md5_hasher.finalize().to_vec());
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
        let mut md5_hasher = Md5::new();
        let mut sha1_hasher = Sha1::new();
        let mut crc_hasher = Crc32Hasher::new();
        
        let mut buffer = [0u8; 8192];
        loop {
            let n = file.read(&mut buffer)?;
            if n == 0 { break; }
            md5_hasher.update(&buffer[..n]);
            sha1_hasher.update(&buffer[..n]);
            crc_hasher.update(&buffer[..n]);
        }
        
        sf.crc = Some(crc_hasher.finalize().to_be_bytes().to_vec());
        sf.sha1 = Some(sha1_hasher.finalize().to_vec());
        sf.md5 = Some(md5_hasher.finalize().to_vec());
        sf.got_status = GotStatus::Got;
        
        Ok(sf)
    }

    pub fn scan_directory(path_str: &str) -> Vec<ScannedFile> {
        let mut results = Vec::new();
        let path = Path::new(path_str);
        
        if let Ok(entries) = fs::read_dir(path) {
            for entry in entries.flatten() {
                let metadata = entry.metadata().unwrap();
                let file_name = entry.file_name().to_string_lossy().to_string();
                
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
                    sf.size = Some(metadata.len());
                } else if file_type == FileType::Dir {
                    // Recursively scan directories
                    let sub_path = path.join(&file_name);
                    sf.children = Self::scan_directory(&sub_path.to_string_lossy());
                } else if file_type == FileType::Zip || file_type == FileType::SevenZip {
                    // Quick scan archive contents without deep scan
                    let archive_path = path.join(&file_name);
                    if let Ok(archive_sf) = Self::scan_archive_file(file_type, &archive_path.to_string_lossy(), sf.file_mod_time_stamp, false) {
                        sf.children = archive_sf.children;
                    }
                }
                
                results.push(sf);
            }
        }
        
        results
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_scan_raw_file_hashing() {
        let mut temp_file = NamedTempFile::new().unwrap();
        let test_data = b"Hello, RustyVault!";
        temp_file.write_all(test_data).unwrap();

        let scanned = Scanner::scan_raw_file(temp_file.path().to_str().unwrap()).unwrap();
        
        // Size Check
        assert_eq!(scanned.size, Some(test_data.len() as u64));
        
        // Hashing checks
        let mut expected_md5 = Md5::new();
        expected_md5.update(test_data);
        assert_eq!(scanned.md5, Some(expected_md5.finalize().to_vec()));

        let mut expected_sha1 = Sha1::new();
        expected_sha1.update(test_data);
        assert_eq!(scanned.sha1, Some(expected_sha1.finalize().to_vec()));

        let mut expected_crc = Crc32Hasher::new();
        expected_crc.update(test_data);
        assert_eq!(scanned.crc, Some(expected_crc.finalize().to_be_bytes().to_vec()));

        assert_eq!(scanned.got_status, GotStatus::Got);
    }
}
