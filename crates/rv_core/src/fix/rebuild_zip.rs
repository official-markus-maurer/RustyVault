use std::cell::RefCell;
use std::collections::HashMap;
use std::fs;
use std::fs::File;
use std::io::Write;
use std::path::Path;
use std::rc::Rc;

use compress::structured_archive::get_compression_type;
use compress::i_compress::ICompress;
use compress::ZipReturn;
use compress::ZipStructure as CompressZipStructure;
use compress::zip_file::ZipFile as CompressZipFile;
use dat_reader::enums::{DatStatus, FileType, GotStatus, ZipStructure};
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipWriter};

use crate::enums::RepStatus;
use crate::rv_file::RvFile;

impl super::Fix {
    pub(super) fn compress_torrentzip_entry(
        name: &str,
        entry_bytes: &[u8],
    ) -> Option<super::TorrentZipBuiltEntry> {
        let compressed_data = compress::deflate_raw_best(entry_bytes).or_else(|| {
            let mut encoder = flate2::write::DeflateEncoder::new(Vec::new(), flate2::Compression::best());
            encoder.write_all(entry_bytes).ok()?;
            encoder.finish().ok()
        })?;

        let mut crc_hasher = crc32fast::Hasher::new();
        crc_hasher.update(entry_bytes);
        let crc = crc_hasher.finalize();

        Some(super::TorrentZipBuiltEntry {
            name: name.to_string(),
            compressed_size: compressed_data.len() as u32,
            uncompressed_size: entry_bytes.len() as u32,
            compressed_data,
            crc,
            flags: Self::torrentzip_flags(name),
            compression_method: 8,
            external_attributes: 0,
        })
    }

    fn maybe_reuse_torrentzip_stream_from_source(
        source_file: Rc<RefCell<RvFile>>,
    ) -> Option<super::TorrentZipBuiltEntry> {
        let (parent_archive, entry_name, parent_type) = Self::find_containing_archive(source_file)?;
        if parent_type != FileType::Zip {
            return None;
        }

        let archive_path = Self::get_existing_physical_path(Rc::clone(&parent_archive));
        let stored = Self::read_raw_zip_entry(&archive_path, &entry_name)?;

        Some(super::TorrentZipBuiltEntry {
            name: entry_name.clone(),
            compressed_data: stored.compressed_data,
            crc: stored.crc,
            compressed_size: stored.compressed_size,
            uncompressed_size: stored.uncompressed_size,
            flags: Self::torrentzip_flags(&entry_name),
            compression_method: 8,
            external_attributes: 0,
        })
    }

    fn maybe_reuse_torrentzip_stream_from_existing(
        zip_path: &str,
        entry_name: &str,
    ) -> Option<super::TorrentZipBuiltEntry> {
        let stored = Self::read_raw_zip_entry(zip_path, entry_name)?;
        Some(super::TorrentZipBuiltEntry {
            name: entry_name.to_string(),
            compressed_data: stored.compressed_data,
            crc: stored.crc,
            compressed_size: stored.compressed_size,
            uncompressed_size: stored.uncompressed_size,
            flags: Self::torrentzip_flags(entry_name),
            compression_method: 8,
            external_attributes: 0,
        })
    }

    fn build_torrentzip_directory_entry(name: &str) -> super::TorrentZipBuiltEntry {
        let entry_name = if name.ends_with('/') {
            name.to_string()
        } else {
            format!("{}/", name)
        };
        super::TorrentZipBuiltEntry {
            name: entry_name.clone(),
            compressed_data: Vec::new(),
            crc: 0,
            compressed_size: 0,
            uncompressed_size: 0,
            flags: Self::torrentzip_flags(&entry_name),
            compression_method: 0,
            external_attributes: 0x10,
        }
    }

    pub(super) fn build_torrentzip_archive(entries: &[super::TorrentZipBuiltEntry]) -> Option<Vec<u8>> {
        let mut archive_bytes = Vec::new();
        let mut central_directory = Vec::new();

        for entry in entries {
            let name_bytes = entry.name.as_bytes();
            let local_offset = archive_bytes.len() as u32;

            archive_bytes.extend_from_slice(&0x04034B50u32.to_le_bytes());
            archive_bytes.extend_from_slice(&20u16.to_le_bytes());
            archive_bytes.extend_from_slice(&entry.flags.to_le_bytes());
            archive_bytes.extend_from_slice(&entry.compression_method.to_le_bytes());
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
            central_directory.extend_from_slice(&entry.compression_method.to_le_bytes());
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
            central_directory.extend_from_slice(&entry.external_attributes.to_le_bytes());
            central_directory.extend_from_slice(&local_offset.to_le_bytes());
            central_directory.extend_from_slice(name_bytes);
        }

        let mut comment_crc = crc32fast::Hasher::new();
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

        Some(archive_bytes)
    }

    pub(super) fn rebuild_zip_archive(
        zip_file: Rc<RefCell<RvFile>>,
        queue: &mut Vec<Rc<RefCell<RvFile>>>,
        total_fixed: &mut i32,
        crc_map: &HashMap<(u64, Vec<u8>), Rc<RefCell<RvFile>>>,
        sha1_map: &HashMap<(u64, Vec<u8>), Rc<RefCell<RvFile>>>,
        md5_map: &HashMap<(u64, Vec<u8>), Rc<RefCell<RvFile>>>,
    ) -> bool {
        let desired_zip_struct = {
            let zf = zip_file.borrow();
            Self::effective_desired_zip_struct(zf.file_type, zf.new_zip_struct())
        };
        let zip_path = Self::get_existing_physical_path(Rc::clone(&zip_file));
        let temp_zip_path = format!("{}.rvfix.tmp", zip_path);
        let current_exists = Path::new(&zip_path).exists();
        let write_exact_torrentzip = matches!(desired_zip_struct, ZipStructure::ZipTrrnt);
        let write_structured_zip = matches!(desired_zip_struct, ZipStructure::ZipZSTD | ZipStructure::ZipTDC);
        let mut retained_entries = 0usize;
        let mut any_changes = current_exists && zip_file.borrow().zip_struct != desired_zip_struct;
        let mut entries = Vec::new();
        Self::collect_archive_rebuild_entries(Rc::clone(&zip_file), "", "", &mut entries, &mut any_changes);
        Self::sort_archive_rebuild_entries(&mut entries, desired_zip_struct);
        let mut torrentzip_entries: Vec<super::TorrentZipBuiltEntry> = Vec::new();
        let mut structured_writer = if write_structured_zip {
            let compress_struct = match desired_zip_struct {
                ZipStructure::ZipZSTD => CompressZipStructure::ZipZSTD,
                ZipStructure::ZipTDC => CompressZipStructure::ZipTDC,
                _ => CompressZipStructure::None,
            };
            let mut zip_out = CompressZipFile::new();
            if zip_out.zip_file_create_with_structure(&temp_zip_path, compress_struct) != ZipReturn::ZipGood {
                return false;
            }
            Some((zip_out, compress_struct))
        } else {
            None
        };

        let mut writer = if write_exact_torrentzip || write_structured_zip {
            None
        } else {
            let temp_file = match File::create(&temp_zip_path) {
                Ok(file) => file,
                Err(_) => return false,
            };
            Some(ZipWriter::new(temp_file))
        };
        let compression_method = match desired_zip_struct {
            ZipStructure::ZipZSTD => CompressionMethod::Zstd,
            _ => CompressionMethod::Deflated,
        };
        let mut options = SimpleFileOptions::default()
            .compression_method(compression_method)
            .compression_level(Some(9));
        if let Some(date_time) = Self::torrentzip_datetime() {
            options = options.last_modified_time(date_time);
        }

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

            if is_directory {
                match rep_status {
                    RepStatus::Delete | RepStatus::UnNeeded => {
                        any_changes = true;
                        continue;
                    }
                    RepStatus::MoveToSort | RepStatus::MoveToCorrupt => {
                        let target_path = Self::get_archive_member_tosort_path(
                            Path::new(&zip_path),
                            &existing_child_name,
                            if matches!(rep_status, RepStatus::MoveToCorrupt) {
                                "ToSort/Corrupt"
                            } else {
                                "ToSort"
                            },
                        );
                        if fs::create_dir_all(&target_path).is_err() {
                            let _ = fs::remove_file(&temp_zip_path);
                            return false;
                        }
                        any_changes = true;
                        continue;
                    }
                    RepStatus::Rename => {
                        if existing_child_name != child_name {
                            any_changes = true;
                        }
                    }
                    _ => {}
                }

                if write_exact_torrentzip {
                    torrentzip_entries.push(Self::build_torrentzip_directory_entry(&child_name));
                    retained_entries += 1;
                } else if let Some((zip_out, compress_struct)) = structured_writer.as_mut() {
                    let name = format!("{}/", child_name.trim_end_matches('/').replace('\\', "/"));
                    let compression_type = get_compression_type(*compress_struct);
                    let Ok(mut write_stream) =
                        zip_out.zip_file_open_write_stream(false, &name, 0, compression_type, None)
                    else {
                        let _ = fs::remove_file(&temp_zip_path);
                        return false;
                    };
                    let _ = write_stream.flush();
                    drop(write_stream);
                    if zip_out.zip_file_close_write_stream(&[]) != ZipReturn::ZipGood {
                        let _ = fs::remove_file(&temp_zip_path);
                        return false;
                    }
                    retained_entries += 1;
                } else if writer.as_mut().is_none_or(|writer| {
                    writer.add_directory(format!("{}/", child_name.trim_end_matches('/')), options).is_err()
                }) {
                    let _ = fs::remove_file(&temp_zip_path);
                    return false;
                } else {
                    retained_entries += 1;
                }
                continue;
            }

            let entry_bytes = match rep_status {
                RepStatus::Delete | RepStatus::UnNeeded => {
                    any_changes = true;
                    continue;
                }
                RepStatus::MoveToSort | RepStatus::MoveToCorrupt => {
                    let Some(bytes) = Self::read_zip_entry_bytes(&zip_path, &existing_child_name) else {
                        let _ = fs::remove_file(&temp_zip_path);
                        return false;
                    };
                    let target_path = Self::get_archive_member_tosort_path(
                        Path::new(&zip_path),
                        &existing_child_name,
                        if matches!(rep_status, RepStatus::MoveToCorrupt) {
                            "ToSort/Corrupt"
                        } else {
                            "ToSort"
                        },
                    );
                    if fs::write(&target_path, &bytes).is_err() {
                        let _ = fs::remove_file(&temp_zip_path);
                        return false;
                    }
                    any_changes = true;
                    continue;
                }
                RepStatus::Rename => {
                    let Some(bytes) = Self::read_zip_entry_bytes(&zip_path, &existing_child_name) else {
                        let _ = fs::remove_file(&temp_zip_path);
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
                        let _ = fs::remove_file(&temp_zip_path);
                        return false;
                    };

                    let bytes = Self::read_source_file_bytes(Rc::clone(&source_file));
                    let Some(bytes) = bytes else {
                        let _ = fs::remove_file(&temp_zip_path);
                        return false;
                    };

                    let source_is_read_only = {
                        let source_ref = source_file.borrow();
                        Self::is_fix_read_only(&source_ref)
                    };
                    let source_is_same_node = Rc::ptr_eq(&source_file, &entry.node);
                    let source_is_same_archive =
                        Self::source_uses_same_archive_path(Rc::clone(&source_file), Path::new(&zip_path));

                    if write_exact_torrentzip {
                        let built_entry = Self::maybe_reuse_torrentzip_stream_from_source(Rc::clone(&source_file))
                            .or_else(|| Self::compress_torrentzip_entry(&child_name.replace('\\', "/"), &bytes));
                        let Some(built_entry) = built_entry else {
                            let _ = fs::remove_file(&temp_zip_path);
                            return false;
                        };
                        if !source_is_read_only && !source_is_same_node && !source_is_same_archive {
                            Self::queue_source_cleanup(source_file, queue);
                        }
                        any_changes = true;
                        torrentzip_entries.push(built_entry);
                        retained_entries += 1;
                        continue;
                    }

                    if !source_is_read_only && !source_is_same_node && !source_is_same_archive {
                        Self::queue_source_cleanup(source_file, queue);
                    }

                    any_changes = true;
                    bytes
                }
                _ => {
                    if !current_exists {
                        if got_status == GotStatus::Got {
                            let _ = fs::remove_file(&temp_zip_path);
                            return false;
                        }
                        continue;
                    }

                    let Some(bytes) = Self::read_zip_entry_bytes(&zip_path, &existing_child_name) else {
                        let _ = fs::remove_file(&temp_zip_path);
                        return false;
                    };
                    bytes
                }
            };

            if write_exact_torrentzip {
                let built_entry = if existing_child_name == child_name {
                    Self::maybe_reuse_torrentzip_stream_from_existing(&zip_path, &existing_child_name)
                } else {
                    None
                }
                .or_else(|| Self::compress_torrentzip_entry(&child_name.replace('\\', "/"), &entry_bytes));
                let Some(built_entry) = built_entry else {
                    let _ = fs::remove_file(&temp_zip_path);
                    return false;
                };
                torrentzip_entries.push(built_entry);
                retained_entries += 1;
            } else if let Some((zip_out, compress_struct)) = structured_writer.as_mut() {
                let name = child_name.replace('\\', "/");
                let compression_type = get_compression_type(*compress_struct);
                let Ok(mut write_stream) = zip_out.zip_file_open_write_stream(
                    false,
                    &name,
                    entry_bytes.len() as u64,
                    compression_type,
                    None,
                ) else {
                    let _ = fs::remove_file(&temp_zip_path);
                    return false;
                };
                if write_stream.write_all(&entry_bytes).is_err() {
                    let _ = fs::remove_file(&temp_zip_path);
                    return false;
                }
                drop(write_stream);
                if zip_out.zip_file_close_write_stream(&[]) != ZipReturn::ZipGood {
                    let _ = fs::remove_file(&temp_zip_path);
                    return false;
                }
                retained_entries += 1;
            } else if writer.as_mut().is_none_or(|writer| {
                writer.start_file(child_name, options).is_err() || writer.write_all(&entry_bytes).is_err()
            }) {
                let _ = fs::remove_file(&temp_zip_path);
                return false;
            } else {
                retained_entries += 1;
            }
        }

        if write_exact_torrentzip {
            let Some(archive_bytes) = Self::build_torrentzip_archive(&torrentzip_entries) else {
                let _ = fs::remove_file(&temp_zip_path);
                return false;
            };
            if fs::write(&temp_zip_path, archive_bytes).is_err() {
                let _ = fs::remove_file(&temp_zip_path);
                return false;
            }
        } else if let Some((mut zip_out, compress_struct)) = structured_writer.take() {
            let _ = compress_struct;
            zip_out.zip_file_close();
        } else if writer.take().is_some_and(|writer| writer.finish().is_err()) {
            let _ = fs::remove_file(&temp_zip_path);
            return false;
        }

        if !any_changes {
            let _ = fs::remove_file(&temp_zip_path);
            return false;
        }

        if retained_entries == 0 {
            let _ = fs::remove_file(&temp_zip_path);
            if Path::new(&zip_path).exists() {
                let _ = fs::remove_file(&zip_path);
            }

            for entry in &entries {
                let mut child_ref = entry.node.borrow_mut();
                match child_ref.rep_status() {
                    RepStatus::Delete | RepStatus::UnNeeded => {
                        child_ref.set_got_status(GotStatus::NotGot);
                        child_ref.rep_status_reset();
                    }
                    RepStatus::MoveToSort => {
                        child_ref.set_dat_got_status(DatStatus::InToSort, GotStatus::Got);
                        child_ref.rep_status_reset();
                    }
                    RepStatus::MoveToCorrupt => {
                        child_ref.set_got_status(GotStatus::NotGot);
                        child_ref.rep_status_reset();
                    }
                    _ => {}
                }
            }

            let mut zip_mut = zip_file.borrow_mut();
            zip_mut.set_got_status(GotStatus::NotGot);
            zip_mut.rep_status_reset();
            zip_mut.cached_stats = None;
            return true;
        }

        if let Some(parent) = Path::new(&zip_path).parent() {
            let _ = fs::create_dir_all(parent);
        }

        if Path::new(&zip_path).exists() {
            let _ = fs::remove_file(&zip_path);
        }

        if fs::rename(&temp_zip_path, &zip_path).is_err() {
            let _ = fs::copy(&temp_zip_path, &zip_path);
            let _ = fs::remove_file(&temp_zip_path);
        }

        if matches!(desired_zip_struct, ZipStructure::ZipTrrnt) && !write_exact_torrentzip {
            let _ = Self::apply_torrentzip_metadata(&zip_path);
        }

        for entry in &entries {
            let mut child_ref = entry.node.borrow_mut();
            match child_ref.rep_status() {
                RepStatus::Delete | RepStatus::UnNeeded => {
                    child_ref.set_got_status(GotStatus::NotGot);
                    child_ref.rep_status_reset();
                }
                RepStatus::MoveToSort => {
                    child_ref.set_dat_got_status(DatStatus::InToSort, GotStatus::Got);
                    child_ref.rep_status_reset();
                }
                RepStatus::MoveToCorrupt => {
                    child_ref.set_got_status(GotStatus::NotGot);
                    child_ref.rep_status_reset();
                }
                RepStatus::CanBeFixed => {
                    child_ref.set_got_status(GotStatus::Got);
                    child_ref.rep_status_reset();
                    *total_fixed += 1;
                }
                RepStatus::CanBeFixedMIA => {
                    child_ref.set_got_status(GotStatus::Got);
                    child_ref.rep_status_reset();
                    *total_fixed += 1;
                }
                RepStatus::CorruptCanBeFixed => {
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

        let mut zip_mut = zip_file.borrow_mut();
        zip_mut.zip_struct = desired_zip_struct;
        zip_mut.set_got_status(GotStatus::Got);
        zip_mut.rep_status_reset();
        zip_mut.cached_stats = None;
        true
    }
}
