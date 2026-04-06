use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use crate::{Crc32, ICompress, SevenZipFile, ZipFile, ZipReturn};

pub type MessageBack = Box<dyn FnMut(&str)>;

pub struct ArchiveExtract {
    pub message_callback: Option<MessageBack>,
}

impl ArchiveExtract {
    pub fn new() -> Self {
        Self {
            message_callback: None,
        }
    }

    pub fn with_callback(message_callback: MessageBack) -> Self {
        Self {
            message_callback: Some(message_callback),
        }
    }

    fn msg(&mut self, message: &str) {
        if let Some(cb) = self.message_callback.as_mut() {
            cb(message);
        }
    }

    fn make_out_path(out_dir: &str, archive_name: &str) -> Option<PathBuf> {
        let norm = archive_name.replace('/', "\\");
        let rel = Path::new(&norm);
        let mut safe = PathBuf::new();
        for comp in rel.components() {
            match comp {
                std::path::Component::Normal(p) => safe.push(p),
                std::path::Component::CurDir => {}
                std::path::Component::ParentDir
                | std::path::Component::RootDir
                | std::path::Component::Prefix(_) => {
                    return None;
                }
            }
        }
        Some(Path::new(out_dir).join(safe))
    }

    pub fn full_extract(&mut self, filename: &str, out_dir: &str) -> bool {
        self.msg(&format!("Processing file: {}", filename));
        if !out_dir.is_empty() {
            self.msg(&format!("Output dir: {}", out_dir));
        }

        let ext = Path::new(filename)
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();

        let mut archive: Box<dyn ICompress> = match ext.as_str() {
            "zip" => Box::new(ZipFile::new()),
            "7z" => Box::new(SevenZipFile::new()),
            _ => {
                self.msg(&format!("Unknown file type .{}", ext));
                return false;
            }
        };

        let zret = archive.zip_file_open(filename, 0, true);
        if zret != ZipReturn::ZipGood {
            self.msg(&format!("Error opening archive {:?}", zret));
            return false;
        }

        for i in 0..archive.local_files_count() {
            let (filename_in_archive, is_directory, uncompressed_size, expected_crc) = {
                let Some(lf) = archive.get_file_header(i) else {
                    self.msg("Error reading file header");
                    archive.zip_file_close();
                    return false;
                };
                (
                    lf.filename.clone(),
                    lf.is_directory,
                    lf.uncompressed_size,
                    lf.crc.clone(),
                )
            };

            if is_directory {
                let dir_name = filename_in_archive.trim_end_matches('/');
                let Some(out_full_dir) = Self::make_out_path(out_dir, dir_name) else {
                    self.msg(&format!("Invalid archive path {}", filename_in_archive));
                    archive.zip_file_close();
                    return false;
                };
                if fs::create_dir_all(&out_full_dir).is_err() {
                    self.msg(&format!("Error creating directory {:?}", out_full_dir));
                    archive.zip_file_close();
                    return false;
                }
                continue;
            }

            self.msg(&format!("Extracting {}", filename_in_archive));
            let Some(out_path) = Self::make_out_path(out_dir, &filename_in_archive) else {
                self.msg(&format!("Invalid archive path {}", filename_in_archive));
                archive.zip_file_close();
                return false;
            };
            if let Some(parent) = out_path.parent() {
                if !parent.as_os_str().is_empty() && fs::create_dir_all(parent).is_err() {
                    self.msg(&format!("Error creating directory {:?}", parent));
                    archive.zip_file_close();
                    return false;
                }
            }

            let mut s_write = match fs::File::create(&out_path) {
                Ok(f) => f,
                Err(_) => {
                    self.msg(&format!("Error opening outputfile {:?}", out_path));
                    archive.zip_file_close();
                    return false;
                }
            };

            let (mut s_read, _) = match archive.zip_file_open_read_stream(i) {
                Ok(v) => v,
                Err(e) => {
                    self.msg(&format!("Error opening read stream {:?}", e));
                    archive.zip_file_close();
                    return false;
                }
            };

            let mut crc = Crc32::new();
            let mut remaining = uncompressed_size;
            let mut buffer = vec![0u8; 409_600];
            while remaining > 0 {
                let want = std::cmp::min(remaining, buffer.len() as u64) as usize;
                let n = match s_read.read(&mut buffer[..want]) {
                    Ok(0) => break,
                    Ok(n) => n,
                    Err(_) => {
                        archive.zip_file_close();
                        return false;
                    }
                };
                crc.slurp_block(&buffer[..n]);
                if s_write.write_all(&buffer[..n]).is_err() {
                    archive.zip_file_close();
                    return false;
                }
                remaining = remaining.saturating_sub(n as u64);
            }

            let _ = archive.zip_file_close_read_stream();

            if let Some(expected) = expected_crc.as_deref() {
                let found = crc.crc32_result_be_bytes();
                if expected.len() != 4
                    || expected[0] != found[0]
                    || expected[1] != found[1]
                    || expected[2] != found[2]
                    || expected[3] != found[3]
                {
                    self.msg(&format!(
                        "CRC error. Expected {} found {}",
                        crate::to_hex(Some(expected)),
                        crate::to_hex(Some(&found))
                    ));
                    archive.zip_file_close();
                    return false;
                }
            }
        }

        archive.zip_file_close();
        true
    }
}

impl Default for ArchiveExtract {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
#[path = "tests/archive_extract_tests.rs"]
mod tests;
