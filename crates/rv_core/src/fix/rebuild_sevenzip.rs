use std::cell::RefCell;
use std::collections::HashMap;
use std::fs;
use std::fs::File;
use std::path::Path;
use std::rc::Rc;

use compress::apply_romvault7z_marker;
use compress::ZipStructure as CompressZipStructure;
use dat_reader::enums::{GotStatus, ZipStructure};
use sevenz_rust::encoder_options::{EncoderOptions, LzmaOptions, ZstandardOptions};
use sevenz_rust::{ArchiveEntry, ArchiveWriter, EncoderConfiguration, EncoderMethod, SourceReader};
use trrntzip::torrent_zip_check::TorrentZipCheck;
use trrntzip::zipped_file::ZippedFile;

use crate::enums::RepStatus;
use crate::rv_file::RvFile;

impl super::Fix {
    pub(super) fn rebuild_seven_zip_archive(
        archive_file: Rc<RefCell<RvFile>>,
        queue: &mut Vec<Rc<RefCell<RvFile>>>,
        total_fixed: &mut i32,
        crc_map: &HashMap<crate::hash_keys::CrcKey, Rc<RefCell<RvFile>>>,
        sha1_map: &HashMap<crate::hash_keys::Sha1Key, Rc<RefCell<RvFile>>>,
        md5_map: &HashMap<crate::hash_keys::Md5Key, Rc<RefCell<RvFile>>>,
    ) -> bool {
        let desired_zip_struct = {
            let af = archive_file.borrow();
            Self::effective_desired_zip_struct(af.file_type, af.new_zip_struct())
        };
        let archive_path = Self::get_existing_physical_path(Rc::clone(&archive_file));
        let temp_archive_path = format!("{}.rvfix.tmp", archive_path);
        let current_exists = Path::new(&archive_path).exists();
        let mut retained_entries = 0usize;
        let mut any_changes =
            current_exists && archive_file.borrow().zip_struct != desired_zip_struct;
        let mut entries = Vec::new();
        Self::collect_archive_rebuild_entries(
            Rc::clone(&archive_file),
            "",
            "",
            &mut entries,
            &mut any_changes,
        );
        Self::sort_archive_rebuild_entries(&mut entries, desired_zip_struct);

        let mut dir_has_children: HashMap<String, bool> = HashMap::new();
        for e in &entries {
            if e.is_directory {
                continue;
            }
            let name = e.target_name.replace('\\', "/");
            if let Some(idx) = name.rfind('/') {
                dir_has_children.insert(format!("{}/", &name[..idx]), true);
            }
        }

        struct SevenZipPlannedEntry {
            archive_name: String,
            is_directory: bool,
            payload: Option<Vec<u8>>,
        }

        let mut planned: Vec<SevenZipPlannedEntry> = Vec::new();

        for entry in &entries {
            let (child_name, existing_child_name, rep_status, got_status, is_directory) = {
                let child_ref = entry.node.borrow();
                (
                    entry.target_name.clone(),
                    entry.existing_name.clone(),
                    child_ref.rep_status(),
                    child_ref.got_status(),
                    entry.is_directory,
                )
            };

            let archive_name = if is_directory {
                let mut d = child_name.trim_end_matches('/').replace('\\', "/");
                d.push('/');
                d
            } else {
                child_name.replace('\\', "/")
            };

            if is_directory {
                if dir_has_children
                    .get(&archive_name)
                    .copied()
                    .unwrap_or(false)
                {
                    any_changes = true;
                    continue;
                }
                match rep_status {
                    RepStatus::Delete | RepStatus::UnNeeded => {
                        any_changes = true;
                        continue;
                    }
                    RepStatus::MoveToSort | RepStatus::MoveToCorrupt => {
                        any_changes = true;
                        continue;
                    }
                    RepStatus::Rename => {
                        if existing_child_name.replace('\\', "/") != archive_name {
                            any_changes = true;
                        }
                    }
                    _ => {}
                }

                planned.push(SevenZipPlannedEntry {
                    archive_name,
                    is_directory: true,
                    payload: None,
                });
                retained_entries += 1;
                continue;
            }

            let entry_bytes = match rep_status {
                RepStatus::Delete | RepStatus::UnNeeded => {
                    any_changes = true;
                    continue;
                }
                RepStatus::MoveToSort | RepStatus::MoveToCorrupt => {
                    let Some(bytes) =
                        Self::read_seven_zip_entry_bytes(&archive_path, &existing_child_name)
                    else {
                        return false;
                    };
                    let target_path = Self::get_archive_member_tosort_path(
                        Path::new(&archive_path),
                        &existing_child_name,
                        if matches!(rep_status, RepStatus::MoveToCorrupt) {
                            "ToSort/Corrupt"
                        } else {
                            "ToSort"
                        },
                    );
                    if fs::write(&target_path, &bytes).is_err() {
                        return false;
                    }
                    any_changes = true;
                    continue;
                }
                RepStatus::Rename => {
                    let Some(bytes) =
                        Self::read_seven_zip_entry_bytes(&archive_path, &existing_child_name)
                    else {
                        return false;
                    };
                    if existing_child_name != child_name {
                        any_changes = true;
                    }
                    bytes
                }
                RepStatus::CanBeFixed | RepStatus::CanBeFixedMIA | RepStatus::CorruptCanBeFixed => {
                    let source_file = {
                        let child_ref = entry.node.borrow();
                        Self::find_source_file(&child_ref, crc_map, sha1_map, md5_map)
                    };
                    let Some(source_file) = source_file else {
                        return false;
                    };

                    let Some(bytes) = Self::read_source_file_bytes(Rc::clone(&source_file)) else {
                        return false;
                    };

                    let source_is_read_only = {
                        let source_ref = source_file.borrow();
                        Self::is_fix_read_only(&source_ref)
                    };
                    let source_is_same_node = Rc::ptr_eq(&source_file, &entry.node);
                    let source_is_same_archive = Self::source_uses_same_archive_path(
                        Rc::clone(&source_file),
                        Path::new(&archive_path),
                    );

                    if !source_is_read_only && !source_is_same_node && !source_is_same_archive {
                        Self::queue_source_cleanup(source_file, queue);
                    }

                    any_changes = true;
                    bytes
                }
                _ => {
                    if !current_exists {
                        if got_status == GotStatus::Got {
                            return false;
                        }
                        continue;
                    }

                    let Some(bytes) =
                        Self::read_seven_zip_entry_bytes(&archive_path, &existing_child_name)
                    else {
                        return false;
                    };
                    bytes
                }
            };

            planned.push(SevenZipPlannedEntry {
                archive_name,
                is_directory: false,
                payload: Some(entry_bytes),
            });
            retained_entries += 1;
        }

        if !any_changes {
            return false;
        }

        if retained_entries == 0 {
            let _ = fs::remove_file(&temp_archive_path);
            if Path::new(&archive_path).exists() {
                let _ = fs::remove_file(&archive_path);
            }

            for entry in &entries {
                let mut child_ref = entry.node.borrow_mut();
                match child_ref.rep_status() {
                    RepStatus::Delete | RepStatus::UnNeeded => {
                        child_ref.set_got_status(GotStatus::NotGot);
                        child_ref.rep_status_reset();
                    }
                    RepStatus::MoveToSort => {
                        child_ref.set_dat_got_status(
                            dat_reader::enums::DatStatus::InToSort,
                            GotStatus::Got,
                        );
                        child_ref.rep_status_reset();
                    }
                    RepStatus::MoveToCorrupt => {
                        child_ref.set_got_status(GotStatus::NotGot);
                        child_ref.rep_status_reset();
                    }
                    _ => {}
                }
            }

            let mut archive_mut = archive_file.borrow_mut();
            archive_mut.set_got_status(GotStatus::NotGot);
            archive_mut.rep_status_reset();
            archive_mut.cached_stats = None;
            return true;
        }

        let _ = fs::remove_file(&temp_archive_path);
        let compress_struct = match desired_zip_struct {
            ZipStructure::SevenZipSLZMA => CompressZipStructure::SevenZipSLZMA,
            ZipStructure::SevenZipNLZMA => CompressZipStructure::SevenZipNLZMA,
            ZipStructure::SevenZipSZSTD => CompressZipStructure::SevenZipSZSTD,
            ZipStructure::SevenZipNZSTD => CompressZipStructure::SevenZipNZSTD,
            _ => CompressZipStructure::None,
        };

        planned.sort_by(|a, b| {
            let zf_a = ZippedFile {
                index: 0,
                name: a.archive_name.clone(),
                size: a.payload.as_ref().map(|p| p.len() as u64).unwrap_or(0),
                crc: None,
                sha1: None,
                is_dir: a.is_directory,
            };
            let zf_b = ZippedFile {
                index: 0,
                name: b.archive_name.clone(),
                size: b.payload.as_ref().map(|p| p.len() as u64).unwrap_or(0),
                crc: None,
                sha1: None,
                is_dir: b.is_directory,
            };
            TorrentZipCheck::trrnt_7zip_string_compare(&zf_a, &zf_b).cmp(&0)
        });

        for i in 0..planned.len().saturating_sub(1) {
            if planned[i].archive_name == planned[i + 1].archive_name {
                let _ = fs::remove_file(&temp_archive_path);
                return false;
            }
        }

        let file = match File::create(&temp_archive_path) {
            Ok(f) => f,
            Err(_) => {
                let _ = fs::remove_file(&temp_archive_path);
                return false;
            }
        };

        let mut writer = match ArchiveWriter::new(file) {
            Ok(w) => w,
            Err(_) => {
                let _ = fs::remove_file(&temp_archive_path);
                return false;
            }
        };
        writer.set_encrypt_header(false);

        let solid = matches!(
            desired_zip_struct,
            ZipStructure::SevenZipSLZMA | ZipStructure::SevenZipSZSTD
        );
        if solid {
            let config = match desired_zip_struct {
                ZipStructure::SevenZipSZSTD | ZipStructure::SevenZipNZSTD => {
                    EncoderConfiguration::new(EncoderMethod::ZSTD)
                        .with_options(EncoderOptions::Zstd(ZstandardOptions::from_level(19)))
                }
                _ => {
                    let mut lz = LzmaOptions::from_level(9);
                    lz.set_dictionary_size(1 << 24);
                    lz.set_num_fast_bytes(64);
                    lz.set_lc(4);
                    lz.set_lp(0);
                    lz.set_pb(2);
                    lz.set_mode_normal();
                    lz.set_match_finder_bt4();
                    EncoderConfiguration::new(EncoderMethod::LZMA)
                        .with_options(EncoderOptions::Lzma(lz))
                }
            };
            writer.set_content_methods(vec![config]);

            for p in planned.iter().filter(|p| p.is_directory) {
                if writer
                    .push_archive_entry::<&[u8]>(ArchiveEntry::new_directory(&p.archive_name), None)
                    .is_err()
                {
                    let _ = fs::remove_file(&temp_archive_path);
                    return false;
                }
            }

            let mut file_entries = Vec::new();
            let mut readers: Vec<SourceReader<std::io::Cursor<Vec<u8>>>> = Vec::new();
            for p in planned.iter().filter(|p| !p.is_directory) {
                let payload = p.payload.clone().unwrap_or_default();
                file_entries.push(ArchiveEntry::new_file(&p.archive_name));
                readers.push(SourceReader::new(std::io::Cursor::new(payload)));
            }
            if !file_entries.is_empty()
                && writer.push_archive_entries(file_entries, readers).is_err()
            {
                let _ = fs::remove_file(&temp_archive_path);
                return false;
            }
        } else {
            for p in &planned {
                if p.is_directory {
                    if writer
                        .push_archive_entry::<&[u8]>(
                            ArchiveEntry::new_directory(&p.archive_name),
                            None,
                        )
                        .is_err()
                    {
                        let _ = fs::remove_file(&temp_archive_path);
                        return false;
                    }
                    continue;
                }
                let payload = p.payload.clone().unwrap_or_default();
                let config = match desired_zip_struct {
                    ZipStructure::SevenZipSZSTD | ZipStructure::SevenZipNZSTD => {
                        EncoderConfiguration::new(EncoderMethod::ZSTD)
                            .with_options(EncoderOptions::Zstd(ZstandardOptions::from_level(19)))
                    }
                    _ => {
                        let mut lz = LzmaOptions::from_level(9);
                        lz.set_dictionary_size(
                            compress::seven_zip::seven_zip_dictionary_size_from_uncompressed_size(
                                payload.len() as u64,
                            ),
                        );
                        lz.set_num_fast_bytes(64);
                        lz.set_lc(4);
                        lz.set_lp(0);
                        lz.set_pb(2);
                        lz.set_mode_normal();
                        lz.set_match_finder_bt4();
                        EncoderConfiguration::new(EncoderMethod::LZMA)
                            .with_options(EncoderOptions::Lzma(lz))
                    }
                };
                writer.set_content_methods(vec![config]);
                if writer
                    .push_archive_entry(
                        ArchiveEntry::new_file(&p.archive_name),
                        Some(std::io::Cursor::new(payload)),
                    )
                    .is_err()
                {
                    let _ = fs::remove_file(&temp_archive_path);
                    return false;
                }
            }
        }

        if writer.finish().is_err() {
            let _ = fs::remove_file(&temp_archive_path);
            return false;
        }

        let _ = apply_romvault7z_marker(Path::new(&temp_archive_path), compress_struct);

        if Path::new(&archive_path).exists() {
            let _ = fs::remove_file(&archive_path);
        }

        if fs::rename(&temp_archive_path, &archive_path).is_err() {
            let _ = fs::copy(&temp_archive_path, &archive_path);
            let _ = fs::remove_file(&temp_archive_path);
        }

        for entry in &entries {
            let mut child_ref = entry.node.borrow_mut();
            match child_ref.rep_status() {
                RepStatus::Delete | RepStatus::UnNeeded => {
                    child_ref.set_got_status(GotStatus::NotGot);
                    child_ref.rep_status_reset();
                }
                RepStatus::MoveToSort => {
                    child_ref
                        .set_dat_got_status(dat_reader::enums::DatStatus::InToSort, GotStatus::Got);
                    child_ref.rep_status_reset();
                }
                RepStatus::MoveToCorrupt => {
                    child_ref.set_got_status(GotStatus::NotGot);
                    child_ref.rep_status_reset();
                }
                RepStatus::CanBeFixed | RepStatus::CanBeFixedMIA | RepStatus::CorruptCanBeFixed => {
                    child_ref.set_got_status(GotStatus::Got);
                    child_ref.rep_status_reset();
                    *total_fixed += 1;
                }
                RepStatus::Rename => {
                    child_ref.file_name = child_ref.name.clone();
                    child_ref.set_got_status(GotStatus::Got);
                    child_ref.rep_status_reset();
                }
                _ => {}
            }
        }

        let mut archive_mut = archive_file.borrow_mut();
        archive_mut.zip_struct = desired_zip_struct;
        archive_mut.set_got_status(GotStatus::Got);
        archive_mut.rep_status_reset();
        archive_mut.cached_stats = None;
        true
    }
}
