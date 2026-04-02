use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

use compress::deflate_raw_best;
use compress::structured_archive::ZipStructure;
use crate::process_control::ProcessControl;
use crate::torrent_zip_check::TorrentZipCheck;
use crate::trrntzip_status::TrrntZipStatus;
use crc32fast::Hasher as Crc32Hasher;

pub struct TorrentZipMake;

struct RawZipEntry {
    name: String,
    compressed_data: Vec<u8>,
    crc: u32,
    compressed_size: u32,
    uncompressed_size: u32,
    flags: u16,
}

impl TorrentZipMake {
    const TORRENTZIP_DOS_TIME: u16 = 48128;
    const TORRENTZIP_DOS_DATE: u16 = 8600;

    fn flags_for(name: &str) -> u16 {
        0x0002 | if name.is_ascii() { 0 } else { 0x0800 }
    }

    fn build_torrentzip_archive(entries: &[RawZipEntry]) -> Vec<u8> {
        let mut archive_bytes = Vec::new();
        let mut central_directory = Vec::new();

        for entry in entries {
            let name_bytes = entry.name.as_bytes();
            let local_offset = archive_bytes.len() as u32;

            archive_bytes.extend_from_slice(&0x04034B50u32.to_le_bytes());
            archive_bytes.extend_from_slice(&20u16.to_le_bytes());
            archive_bytes.extend_from_slice(&entry.flags.to_le_bytes());
            archive_bytes.extend_from_slice(&8u16.to_le_bytes());
            archive_bytes.extend_from_slice(&Self::TORRENTZIP_DOS_TIME.to_le_bytes());
            archive_bytes.extend_from_slice(&Self::TORRENTZIP_DOS_DATE.to_le_bytes());
            archive_bytes.extend_from_slice(&entry.crc.to_le_bytes());
            archive_bytes.extend_from_slice(&entry.compressed_size.to_le_bytes());
            archive_bytes.extend_from_slice(&entry.uncompressed_size.to_le_bytes());
            archive_bytes.extend_from_slice(&(name_bytes.len() as u16).to_le_bytes());
            archive_bytes.extend_from_slice(&0u16.to_le_bytes());
            archive_bytes.extend_from_slice(name_bytes);
            archive_bytes.extend_from_slice(&entry.compressed_data);

            central_directory.extend_from_slice(&0x02014B50u32.to_le_bytes());
            central_directory.extend_from_slice(&0u16.to_le_bytes());
            central_directory.extend_from_slice(&20u16.to_le_bytes());
            central_directory.extend_from_slice(&entry.flags.to_le_bytes());
            central_directory.extend_from_slice(&8u16.to_le_bytes());
            central_directory.extend_from_slice(&Self::TORRENTZIP_DOS_TIME.to_le_bytes());
            central_directory.extend_from_slice(&Self::TORRENTZIP_DOS_DATE.to_le_bytes());
            central_directory.extend_from_slice(&entry.crc.to_le_bytes());
            central_directory.extend_from_slice(&entry.compressed_size.to_le_bytes());
            central_directory.extend_from_slice(&entry.uncompressed_size.to_le_bytes());
            central_directory.extend_from_slice(&(name_bytes.len() as u16).to_le_bytes());
            central_directory.extend_from_slice(&0u16.to_le_bytes());
            central_directory.extend_from_slice(&0u16.to_le_bytes());
            central_directory.extend_from_slice(&0u16.to_le_bytes());
            central_directory.extend_from_slice(&0u16.to_le_bytes());
            central_directory.extend_from_slice(&0u32.to_le_bytes());
            central_directory.extend_from_slice(&local_offset.to_le_bytes());
            central_directory.extend_from_slice(name_bytes);
        }

        let mut comment_crc = Crc32Hasher::new();
        comment_crc.update(&central_directory);
        let comment = format!("TORRENTZIPPED-{:08X}", comment_crc.finalize());

        let central_directory_offset = archive_bytes.len() as u32;
        let central_directory_size = central_directory.len() as u32;
        archive_bytes.extend_from_slice(&central_directory);
        archive_bytes.extend_from_slice(&0x06054B50u32.to_le_bytes());
        archive_bytes.extend_from_slice(&0u16.to_le_bytes());
        archive_bytes.extend_from_slice(&0u16.to_le_bytes());
        archive_bytes.extend_from_slice(&(entries.len() as u16).to_le_bytes());
        archive_bytes.extend_from_slice(&(entries.len() as u16).to_le_bytes());
        archive_bytes.extend_from_slice(&central_directory_size.to_le_bytes());
        archive_bytes.extend_from_slice(&central_directory_offset.to_le_bytes());
        archive_bytes.extend_from_slice(&(comment.len() as u16).to_le_bytes());
        archive_bytes.extend_from_slice(comment.as_bytes());
        archive_bytes
    }

    fn collect_files(root: &Path) -> (Vec<String>, Vec<String>) {
        let mut files = Vec::new();
        let mut dirs_with_files = std::collections::HashSet::new();
        let mut all_dirs = std::collections::HashSet::new();

        let mut stack = vec![PathBuf::from(root)];
        while let Some(path) = stack.pop() {
            if let Ok(read_dir) = fs::read_dir(&path) {
                let mut saw_file = false;
                for entry in read_dir.flatten() {
                    let p = entry.path();
                    if p.is_dir() {
                        stack.push(p);
                    } else if p.is_file() {
                        saw_file = true;
                        let rel = p.strip_prefix(root).unwrap().to_string_lossy().replace('\\', "/");
                        files.push(rel);
                        if let Some(parent) = p.parent() {
                            let mut dir_rel = parent.strip_prefix(root).unwrap().to_string_lossy().replace('\\', "/");
                            if !dir_rel.is_empty() && !dir_rel.ends_with('/') {
                                dir_rel.push('/');
                            }
                            if !dir_rel.is_empty() {
                                dirs_with_files.insert(dir_rel);
                            }
                        }
                    }
                }
                let mut dir_rel = path.strip_prefix(root).unwrap().to_string_lossy().replace('\\', "/");
                if !dir_rel.is_empty() && !dir_rel.ends_with('/') {
                    dir_rel.push('/');
                }
                if !dir_rel.is_empty() {
                    all_dirs.insert(dir_rel);
                }
                let _ = saw_file;
            }
        }

        files.sort_by(|a, b| TorrentZipCheck::trrnt_zip_string_compare(&crate::zipped_file::ZippedFile { index: 0, name: a.clone(), size: 0, crc: None, sha1: None, is_dir: false }, &crate::zipped_file::ZippedFile { index: 0, name: b.clone(), size: 0, crc: None, sha1: None, is_dir: false }).cmp(&0));

        let mut dir_markers: Vec<String> = all_dirs
            .into_iter()
            .filter(|d| !dirs_with_files.contains(d))
            .collect();
        dir_markers.sort_by(|a, b| TorrentZipCheck::trrnt_zip_string_compare(&crate::zipped_file::ZippedFile { index: 0, name: a.clone(), size: 0, crc: None, sha1: None, is_dir: true }, &crate::zipped_file::ZippedFile { index: 0, name: b.clone(), size: 0, crc: None, sha1: None, is_dir: true }).cmp(&0));

        (files, dir_markers)
    }

    pub fn zip_directory_with_control(dir: &str, output_type: ZipStructure, control: Option<&ProcessControl>) -> TrrntZipStatus {
        let path = Path::new(dir);
        if !path.exists() || !path.is_dir() {
            return TrrntZipStatus::CATCH_ERROR;
        }
        if output_type != ZipStructure::ZipTrrnt {
            return TrrntZipStatus::CATCH_ERROR;
        }

        let parent = path.parent().unwrap_or(Path::new(""));
        let stem = path.file_name().unwrap_or_default().to_string_lossy();
        let out_filename = parent.join(format!("{}.zip", stem));
        let tmp_filename = parent.join(format!("__{}.samtmp", stem));
        let _ = fs::remove_file(&tmp_filename);

        let (files, dirs) = Self::collect_files(path);
        let mut entries = Vec::with_capacity(files.len() + dirs.len());

        for d in dirs {
            let name = if d.ends_with('/') { d } else { format!("{}/", d) };
            entries.push(RawZipEntry {
                flags: Self::flags_for(&name),
                name,
                compressed_data: Vec::new(),
                crc: 0,
                compressed_size: 0,
                uncompressed_size: 0,
            });
        }

        for f in files {
            if let Some(c) = control {
                c.wait_one();
                if c.is_soft_stop_requested() {
                    let _ = fs::remove_file(&tmp_filename);
                    return if c.is_hard_stop_requested() {
                        TrrntZipStatus::USER_ABORTED_HARD
                    } else {
                        TrrntZipStatus::USER_ABORTED
                    };
                }
            }
            let p = path.join(&f);
            let mut data = Vec::new();
            if let Ok(mut file) = fs::File::open(&p) {
                let _ = file.read_to_end(&mut data);
            }
            let mut crc_hasher = Crc32Hasher::new();
            crc_hasher.update(&data);
            let crc = crc_hasher.finalize();
            let comp = match deflate_raw_best(&data) {
                Some(v) => v,
                None => return TrrntZipStatus::CATCH_ERROR,
            };
            entries.push(RawZipEntry {
                name: f.replace('\\', "/"),
                compressed_data: comp,
                crc,
                compressed_size: data.len() as u32, // will be overwritten below
                uncompressed_size: data.len() as u32,
                flags: Self::flags_for(&f),
            });
        }

        for e in entries.iter_mut() {
            if e.uncompressed_size > 0 {
                e.compressed_size = e.compressed_data.len() as u32;
            }
        }

        let built = Self::build_torrentzip_archive(&entries);
        if fs::write(&tmp_filename, built).is_err() {
            let _ = fs::remove_file(&tmp_filename);
            return TrrntZipStatus::CATCH_ERROR;
        }

        let _ = fs::remove_file(&out_filename);
        if fs::rename(&tmp_filename, &out_filename).is_err() {
            let _ = fs::copy(&tmp_filename, &out_filename);
            let _ = fs::remove_file(&tmp_filename);
        }

        TrrntZipStatus::VALID_TRRNTZIP
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::tempdir;
    use zip::ZipArchive;

    #[test]
    fn make_simple_torrentzip_from_dir() {
        let tmp = tempdir().unwrap();
        let root = tmp.path().join("dir");
        fs::create_dir_all(root.join("empty")).unwrap();
        fs::create_dir_all(root.join("sub")).unwrap();
        let mut f1 = fs::File::create(root.join("a.txt")).unwrap();
        f1.write_all(b"aaaa").unwrap();
        let mut f2 = fs::File::create(root.join("sub").join("b.bin")).unwrap();
        f2.write_all(b"bbbbbbbb").unwrap();

        let status = TorrentZipMake::zip_directory_with_control(root.to_string_lossy().as_ref(), ZipStructure::ZipTrrnt, None);
        assert_eq!(status, TrrntZipStatus::VALID_TRRNTZIP);

        let zip_path = tmp.path().join("dir.zip");
        let file = fs::File::open(&zip_path).unwrap();
        let archive = ZipArchive::new(file).unwrap();
        assert_eq!(archive.len(), 3);
    }
}
