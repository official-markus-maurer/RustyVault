use crate::process_control::ProcessControl;
use crate::trrntzip_status::TrrntZipStatus;
use crate::zipped_file::ZippedFile;
use compress::codepage_437;
use compress::deflate_raw_best;
use compress::i_compress::ICompress;
use compress::structured_archive::{
    get_compression_type, get_zip_comment_id, get_zip_date_time_type, ZipDateType, ZipStructure,
};
use compress::zip_enums::ZipReturn;
use compress::zip_extra_field;
use crc32fast::Hasher as Crc32Hasher;
use sevenz_rust::encoder_options::{EncoderOptions, LzmaOptions, ZstandardOptions};
use sevenz_rust::{ArchiveEntry, ArchiveWriter, EncoderConfiguration, EncoderMethod, SourceReader};
use std::fs;
use std::io::{Read, Write};
use std::path::Path;

/// Core logic for rebuilding an archive into TorrentZip format.
///
/// `TorrentZipRebuild` is responsible for generating a deterministic `.zip` file.
/// It creates a temporary zip, copies the raw streams of the files from the source
/// zip in strict alphabetical order, forces the Deflate compression parameters,
/// sets timestamps to the TorrentZip epoch, and recomputes the structural hashes.
///
/// Differences from C#:
/// - The C# `TorrentZipRebuild` relies on a highly specialized `Compress.ZipFile` writer that handles
///   raw DEFLATE streams and deterministic TorrentZip local header offsets.
/// - The Rust version currently implements the structure and sorting pipeline, but relies on standard
///   file I/O writing streams that simulate the `ICompress` interface. Full TorrentZip deterministic
///   byte alignment is still pending a robust Rust Zip-streaming replacement crate.
pub struct TorrentZipRebuild;

struct RawZipEntry {
    name: String,
    compressed_data: Vec<u8>,
    crc: u32,
    compressed_size: u64,
    uncompressed_size: u64,
    flags: u16,
    compression_method: u16,
    external_attributes: u32,
}

include!("control.rs");
include!("raw_torrentzip.rs");

impl TorrentZipRebuild {
    const TORRENTZIP_DOS_TIME: u16 = 48128;
    const TORRENTZIP_DOS_DATE: u16 = 8600;

    fn structured_version_needed(expected_method: u16, is_zip64: bool) -> u16 {
        if expected_method == 93 {
            63
        } else if is_zip64 {
            45
        } else {
            20
        }
    }

    fn torrentzip_flags(name: &str) -> u16 {
        0x0002
            | if codepage_437::is_code_page_437(name) {
                0
            } else {
                0x0800
            }
    }

    fn ascii_lower(byte: u8) -> u8 {
        if byte.is_ascii_uppercase() {
            byte + 0x20
        } else {
            byte
        }
    }

    fn compare_trrntzip_names_case(left: &str, right: &str) -> std::cmp::Ordering {
        let bytes_a = left.as_bytes();
        let bytes_b = right.as_bytes();
        let len = std::cmp::min(bytes_a.len(), bytes_b.len());
        for i in 0..len {
            let ca = Self::ascii_lower(bytes_a[i]);
            let cb = Self::ascii_lower(bytes_b[i]);
            if ca < cb {
                return std::cmp::Ordering::Less;
            }
            if ca > cb {
                return std::cmp::Ordering::Greater;
            }
        }
        let res = bytes_a.len().cmp(&bytes_b.len());
        if res != std::cmp::Ordering::Equal {
            return res;
        }
        left.cmp(right)
    }

    fn normalize_zip_member_name(name: &str) -> String {
        if name.contains('\\') {
            name.replace('\\', "/")
        } else {
            name.to_string()
        }
    }

    fn remove_unneeded_directory_entries(mut files: Vec<ZippedFile>) -> Vec<ZippedFile> {
        let names: Vec<String> = files.iter().map(|f| f.name.clone()).collect();
        files.retain(|f| {
            if !(f.is_dir || f.name.ends_with('/')) {
                return true;
            }
            if !f.name.ends_with('/') {
                return true;
            }
            let dir_name = f.name.as_str();
            !names
                .iter()
                .any(|other| other.len() > dir_name.len() && other.starts_with(dir_name))
        });
        files
    }

    fn prepare_zip_file_list(
        zipped_files: &[ZippedFile],
        prune_dirs: bool,
    ) -> Result<Vec<ZippedFile>, TrrntZipStatus> {
        let mut out = Vec::with_capacity(zipped_files.len());
        for f in zipped_files {
            let mut z = f.clone();
            z.name = Self::normalize_zip_member_name(&z.name);
            if (z.is_dir || z.name.ends_with('/')) && !z.name.ends_with('/') {
                z.name.push('/');
            }
            out.push(z);
        }

        if prune_dirs {
            out = Self::remove_unneeded_directory_entries(out);
        }
        out.sort_by(|a, b| Self::compare_trrntzip_names_case(&a.name, &b.name));

        for i in 0..out.len().saturating_sub(1) {
            if out[i].name == out[i + 1].name {
                return Err(TrrntZipStatus::REPEAT_FILES_FOUND);
            }
        }

        Ok(out)
    }

    fn split_7zip_filename(filename: &str) -> (&str, &str, &str) {
        let dir_index = filename.rfind('/');
        let (path, name) = if let Some(i) = dir_index {
            (&filename[..i], &filename[i + 1..])
        } else {
            ("", filename)
        };

        let ext_index = name.rfind('.');
        if let Some(i) = ext_index {
            (path, &name[..i], &name[i + 1..])
        } else {
            (path, name, "")
        }
    }

    fn compare_trrnt7z_names(left: &str, right: &str) -> std::cmp::Ordering {
        let (path_a, name_a, ext_a) = Self::split_7zip_filename(left);
        let (path_b, name_b, ext_b) = Self::split_7zip_filename(right);
        let res = ext_a.cmp(ext_b);
        if res != std::cmp::Ordering::Equal {
            return res;
        }
        let res = name_a.cmp(name_b);
        if res != std::cmp::Ordering::Equal {
            return res;
        }
        path_a.cmp(path_b)
    }

    fn remove_unneeded_7z_directory_entries(mut files: Vec<ZippedFile>) -> Vec<ZippedFile> {
        let mut dir_sort_test = files.clone();
        dir_sort_test.sort_by(|a, b| a.name.cmp(&b.name));
        let mut i = 0usize;
        while i < dir_sort_test.len().saturating_sub(1) {
            if !dir_sort_test[i].name.ends_with('/') {
                i += 1;
                continue;
            }
            if dir_sort_test[i + 1].name.len() <= dir_sort_test[i].name.len() {
                i += 1;
                continue;
            }
            let dir_name = dir_sort_test[i].name.clone();
            if dir_sort_test[i + 1].name.starts_with(&dir_name) {
                files.retain(|f| f.name != dir_name);
                dir_sort_test.remove(i);
                continue;
            }
            i += 1;
        }
        files
    }

    fn prepare_seven_zip_file_list(
        zipped_files: &[ZippedFile],
    ) -> Result<Vec<ZippedFile>, TrrntZipStatus> {
        let mut out = Vec::with_capacity(zipped_files.len());
        for f in zipped_files {
            let mut z = f.clone();
            z.name = Self::normalize_zip_member_name(&z.name);
            if z.name.is_empty() || z.name == "/" {
                continue;
            }
            if (z.is_dir || z.name.ends_with('/')) && !z.name.ends_with('/') {
                z.name.push('/');
            }
            out.push(z);
        }

        out = Self::remove_unneeded_7z_directory_entries(out);
        out.sort_by(|a, b| Self::compare_trrnt7z_names(&a.name, &b.name));
        for i in 0..out.len().saturating_sub(1) {
            if out[i].name == out[i + 1].name {
                return Err(TrrntZipStatus::REPEAT_FILES_FOUND);
            }
        }

        Ok(out)
    }

    fn apply_structured_zip_metadata(zip_path: &Path, zip_struct: ZipStructure) -> bool {
        let Ok(mut zip_bytes) = fs::read(zip_path) else {
            return false;
        };

        let local_header_signature = [0x50, 0x4B, 0x03, 0x04];
        let central_header_signature = [0x50, 0x4B, 0x01, 0x02];
        let utf8_flag = 0x0800u16;

        let expected_method = get_compression_type(zip_struct);
        let date_type = get_zip_date_time_type(zip_struct);

        let (time_patch, date_patch) = match date_type {
            ZipDateType::TrrntZip => (
                Some(Self::TORRENTZIP_DOS_TIME),
                Some(Self::TORRENTZIP_DOS_DATE),
            ),
            ZipDateType::None => (Some(0u16), Some(0u16)),
            ZipDateType::DateTime => (None, None),
            ZipDateType::Undefined => return false,
        };

        let mut local_offset = 0usize;
        while local_offset + 30 <= zip_bytes.len()
            && zip_bytes[local_offset..local_offset + 4] == local_header_signature
        {
            let flags =
                u16::from_le_bytes([zip_bytes[local_offset + 6], zip_bytes[local_offset + 7]]);
            let normalized_flags = 0x0002 | (flags & utf8_flag);

            let name_len =
                u16::from_le_bytes([zip_bytes[local_offset + 26], zip_bytes[local_offset + 27]])
                    as usize;
            let extra_len =
                u16::from_le_bytes([zip_bytes[local_offset + 28], zip_bytes[local_offset + 29]])
                    as usize;

            let header_comp_size = u32::from_le_bytes([
                zip_bytes[local_offset + 18],
                zip_bytes[local_offset + 19],
                zip_bytes[local_offset + 20],
                zip_bytes[local_offset + 21],
            ]);
            let header_uncomp_size = u32::from_le_bytes([
                zip_bytes[local_offset + 22],
                zip_bytes[local_offset + 23],
                zip_bytes[local_offset + 24],
                zip_bytes[local_offset + 25],
            ]);

            let extra_start = local_offset + 30 + name_len;
            let extra_end = extra_start + extra_len;
            if extra_end > zip_bytes.len() {
                return false;
            }
            let extra_bytes = &zip_bytes[extra_start..extra_end];
            let extra_info = zip_extra_field::parse_extra_fields(
                extra_bytes,
                false,
                header_uncomp_size,
                header_comp_size,
                0,
            );
            let is_zip64 = extra_info.is_zip64
                || header_comp_size == 0xFFFF_FFFF
                || header_uncomp_size == 0xFFFF_FFFF;
            let version_needed = Self::structured_version_needed(expected_method, is_zip64);

            zip_bytes[local_offset + 4..local_offset + 6]
                .copy_from_slice(&version_needed.to_le_bytes());
            zip_bytes[local_offset + 6..local_offset + 8]
                .copy_from_slice(&normalized_flags.to_le_bytes());
            zip_bytes[local_offset + 8..local_offset + 10]
                .copy_from_slice(&expected_method.to_le_bytes());
            if let (Some(t), Some(d)) = (time_patch, date_patch) {
                zip_bytes[local_offset + 10..local_offset + 12].copy_from_slice(&t.to_le_bytes());
                zip_bytes[local_offset + 12..local_offset + 14].copy_from_slice(&d.to_le_bytes());
            }

            let comp_size = if header_comp_size == 0xFFFF_FFFF {
                let Some(v) = extra_info.compressed_size else {
                    return false;
                };
                let Ok(v) = usize::try_from(v) else {
                    return false;
                };
                v
            } else {
                header_comp_size as usize
            };

            let data_offset = local_offset + 30 + name_len + extra_len;
            local_offset = data_offset.saturating_add(comp_size);
        }

        let eocd_offset = zip_bytes
            .windows(4)
            .rposition(|window| window == [0x50, 0x4B, 0x05, 0x06]);
        let Some(eocd_offset) = eocd_offset else {
            return false;
        };
        if eocd_offset + 22 > zip_bytes.len() {
            return false;
        }

        let total_entries =
            u16::from_le_bytes([zip_bytes[eocd_offset + 10], zip_bytes[eocd_offset + 11]]);
        let central_directory_size_u32 = u32::from_le_bytes([
            zip_bytes[eocd_offset + 12],
            zip_bytes[eocd_offset + 13],
            zip_bytes[eocd_offset + 14],
            zip_bytes[eocd_offset + 15],
        ]);
        let central_directory_offset_u32 = u32::from_le_bytes([
            zip_bytes[eocd_offset + 16],
            zip_bytes[eocd_offset + 17],
            zip_bytes[eocd_offset + 18],
            zip_bytes[eocd_offset + 19],
        ]);

        let zip64_required = total_entries == 0xFFFF
            || central_directory_size_u32 == 0xFFFF_FFFF
            || central_directory_offset_u32 == 0xFFFF_FFFF;

        let zip64_info = if eocd_offset >= 20 {
            let locator_offset = eocd_offset - 20;
            if locator_offset + 20 <= zip_bytes.len()
                && zip_bytes[locator_offset..locator_offset + 4] == [0x50, 0x4B, 0x06, 0x07]
            {
                (|| {
                    let disk = u32::from_le_bytes(
                        zip_bytes[locator_offset + 4..locator_offset + 8]
                            .try_into()
                            .ok()?,
                    );
                    if disk != 0 {
                        return None;
                    }
                    let zip64_eocd_offset = u64::from_le_bytes(
                        zip_bytes[locator_offset + 8..locator_offset + 16]
                            .try_into()
                            .ok()?,
                    );
                    let total_disks = u32::from_le_bytes(
                        zip_bytes[locator_offset + 16..locator_offset + 20]
                            .try_into()
                            .ok()?,
                    );
                    if total_disks > 1 {
                        return None;
                    }
                    let zip64_eocd_offset_usize = zip64_eocd_offset as usize;
                    if zip64_eocd_offset_usize + 56 > zip_bytes.len() {
                        return None;
                    }
                    if zip_bytes[zip64_eocd_offset_usize..zip64_eocd_offset_usize + 4]
                        != [0x50, 0x4B, 0x06, 0x06]
                    {
                        return None;
                    }
                    let size_of_record = u64::from_le_bytes(
                        zip_bytes[zip64_eocd_offset_usize + 4..zip64_eocd_offset_usize + 12]
                            .try_into()
                            .ok()?,
                    );
                    if size_of_record != 44 {
                        return None;
                    }
                    let version_needed = u16::from_le_bytes(
                        zip_bytes[zip64_eocd_offset_usize + 14..zip64_eocd_offset_usize + 16]
                            .try_into()
                            .ok()?,
                    );
                    if version_needed != 45 {
                        return None;
                    }
                    let disk_num = u32::from_le_bytes(
                        zip_bytes[zip64_eocd_offset_usize + 16..zip64_eocd_offset_usize + 20]
                            .try_into()
                            .ok()?,
                    );
                    let disk_cd = u32::from_le_bytes(
                        zip_bytes[zip64_eocd_offset_usize + 20..zip64_eocd_offset_usize + 24]
                            .try_into()
                            .ok()?,
                    );
                    if disk_num != 0 || disk_cd != 0 {
                        return None;
                    }
                    let entries_on_disk = u64::from_le_bytes(
                        zip_bytes[zip64_eocd_offset_usize + 24..zip64_eocd_offset_usize + 32]
                            .try_into()
                            .ok()?,
                    );
                    let entries_total = u64::from_le_bytes(
                        zip_bytes[zip64_eocd_offset_usize + 32..zip64_eocd_offset_usize + 40]
                            .try_into()
                            .ok()?,
                    );
                    if entries_on_disk != entries_total {
                        return None;
                    }
                    let cd_size = u64::from_le_bytes(
                        zip_bytes[zip64_eocd_offset_usize + 40..zip64_eocd_offset_usize + 48]
                            .try_into()
                            .ok()?,
                    );
                    let cd_offset = u64::from_le_bytes(
                        zip_bytes[zip64_eocd_offset_usize + 48..zip64_eocd_offset_usize + 56]
                            .try_into()
                            .ok()?,
                    );
                    Some((entries_total, cd_size, cd_offset))
                })()
            } else {
                None
            }
        } else {
            None
        };

        if zip64_required && zip64_info.is_none() {
            return false;
        }

        let (central_directory_offset, central_directory_size) =
            if let Some((_, cd_size, cd_offset)) = zip64_info {
                let Ok(cd_offset) = usize::try_from(cd_offset) else {
                    return false;
                };
                let Ok(cd_size) = usize::try_from(cd_size) else {
                    return false;
                };
                (cd_offset, cd_size)
            } else {
                (
                    central_directory_offset_u32 as usize,
                    central_directory_size_u32 as usize,
                )
            };

        if central_directory_offset + central_directory_size > zip_bytes.len() {
            return false;
        }

        let central_end = central_directory_offset + central_directory_size;
        let mut central_offset = central_directory_offset;
        while central_offset + 46 <= central_end
            && zip_bytes[central_offset..central_offset + 4] == central_header_signature
        {
            let file_name_length = u16::from_le_bytes([
                zip_bytes[central_offset + 28],
                zip_bytes[central_offset + 29],
            ]) as usize;
            let extra_length = u16::from_le_bytes([
                zip_bytes[central_offset + 30],
                zip_bytes[central_offset + 31],
            ]) as usize;
            let comment_length = u16::from_le_bytes([
                zip_bytes[central_offset + 32],
                zip_bytes[central_offset + 33],
            ]) as usize;

            let flags =
                u16::from_le_bytes([zip_bytes[central_offset + 8], zip_bytes[central_offset + 9]]);
            let normalized_flags = 0x0002 | (flags & utf8_flag);

            let header_comp_size = u32::from_le_bytes([
                zip_bytes[central_offset + 20],
                zip_bytes[central_offset + 21],
                zip_bytes[central_offset + 22],
                zip_bytes[central_offset + 23],
            ]);
            let header_uncomp_size = u32::from_le_bytes([
                zip_bytes[central_offset + 24],
                zip_bytes[central_offset + 25],
                zip_bytes[central_offset + 26],
                zip_bytes[central_offset + 27],
            ]);
            let header_local_offset = u32::from_le_bytes([
                zip_bytes[central_offset + 42],
                zip_bytes[central_offset + 43],
                zip_bytes[central_offset + 44],
                zip_bytes[central_offset + 45],
            ]);

            let extra_start = central_offset + 46 + file_name_length;
            let extra_end = extra_start + extra_length;
            if extra_end > zip_bytes.len() {
                return false;
            }
            let extra_bytes = &zip_bytes[extra_start..extra_end];
            let extra_info = zip_extra_field::parse_extra_fields(
                extra_bytes,
                true,
                header_uncomp_size,
                header_comp_size,
                header_local_offset,
            );
            let is_zip64 = extra_info.is_zip64
                || header_comp_size == 0xFFFF_FFFF
                || header_uncomp_size == 0xFFFF_FFFF
                || header_local_offset == 0xFFFF_FFFF;
            let version_needed = Self::structured_version_needed(expected_method, is_zip64);

            zip_bytes[central_offset + 4..central_offset + 6].copy_from_slice(&0u16.to_le_bytes());
            zip_bytes[central_offset + 6..central_offset + 8]
                .copy_from_slice(&version_needed.to_le_bytes());
            zip_bytes[central_offset + 8..central_offset + 10]
                .copy_from_slice(&normalized_flags.to_le_bytes());
            zip_bytes[central_offset + 10..central_offset + 12]
                .copy_from_slice(&expected_method.to_le_bytes());
            if let (Some(t), Some(d)) = (time_patch, date_patch) {
                zip_bytes[central_offset + 12..central_offset + 14]
                    .copy_from_slice(&t.to_le_bytes());
                zip_bytes[central_offset + 14..central_offset + 16]
                    .copy_from_slice(&d.to_le_bytes());
            }
            zip_bytes[central_offset + 32..central_offset + 34]
                .copy_from_slice(&0u16.to_le_bytes());
            zip_bytes[central_offset + 34..central_offset + 36]
                .copy_from_slice(&0u16.to_le_bytes());
            zip_bytes[central_offset + 36..central_offset + 38]
                .copy_from_slice(&0u16.to_le_bytes());
            zip_bytes[central_offset + 38..central_offset + 42]
                .copy_from_slice(&0u32.to_le_bytes());

            central_offset += 46 + file_name_length + extra_length + comment_length;
        }

        let mut crc = Crc32Hasher::new();
        crc.update(
            &zip_bytes[central_directory_offset..central_directory_offset + central_directory_size],
        );
        let comment = format!("{}{:08X}", get_zip_comment_id(zip_struct), crc.finalize());

        let mut rebuilt = Vec::with_capacity(eocd_offset + 22 + comment.len());
        rebuilt.extend_from_slice(&zip_bytes[..eocd_offset + 22]);
        let comment_len = comment.len() as u16;
        let len_offset = eocd_offset + 20;
        rebuilt[len_offset] = (comment_len & 0xFF) as u8;
        rebuilt[len_offset + 1] = (comment_len >> 8) as u8;
        rebuilt.extend_from_slice(comment.as_bytes());

        fs::write(zip_path, rebuilt).is_ok()
    }

    fn rezip_torrentzip_raw(
        zipped_files: &[ZippedFile],
        original_zip_file: &mut dyn ICompress,
        tmp_filename: &Path,
        out_filename: &Path,
        source_path: &Path,
        control: Option<&ProcessControl>,
    ) -> Option<TrrntZipStatus> {
        if Self::hard_stop_requested(control) {
            Self::remove_tmp_if_present(tmp_filename);
            original_zip_file.zip_file_close();
            return Some(Self::aborted_status(control));
        }

        let prepared = match Self::prepare_zip_file_list(zipped_files, true) {
            Ok(v) => v,
            Err(status) => {
                Self::remove_tmp_if_present(tmp_filename);
                original_zip_file.zip_file_close();
                return Some(status);
            }
        };

        let mut entries = Vec::with_capacity(prepared.len());
        let source_is_torrentzip = original_zip_file.zip_struct() == ZipStructure::ZipTrrnt;

        for file in &prepared {
            let is_dir = file.is_dir || file.name.ends_with('/');
            let entry_name = if is_dir && !file.name.ends_with('/') {
                format!("{}/", file.name)
            } else {
                file.name.clone()
            };
            let flags = Self::torrentzip_flags(&entry_name);

            if source_is_torrentzip {
                if let Ok((mut read_stream, _, compression_method)) =
                    original_zip_file.zip_file_open_read_stream_ex(file.index as usize, true)
                {
                    if compression_method == 8 {
                        let mut compressed_data = Vec::new();
                        if read_stream.read_to_end(&mut compressed_data).is_ok() {
                            let _ = original_zip_file.zip_file_close_read_stream();

                            if let Some(header) =
                                original_zip_file.get_file_header(file.index as usize)
                            {
                                let compressed_size = compressed_data.len() as u64;
                                let uncompressed_size = header.uncompressed_size;
                                let crc = header
                                    .crc
                                    .as_ref()
                                    .and_then(|b| b.as_slice().try_into().ok())
                                    .map(u32::from_be_bytes)
                                    .unwrap_or(0);

                                entries.push(RawZipEntry {
                                    name: entry_name,
                                    compressed_size,
                                    uncompressed_size,
                                    compressed_data,
                                    crc,
                                    flags,
                                    compression_method: 8,
                                    external_attributes: 0,
                                });
                                continue;
                            }
                        }
                    }
                }
            }

            let mut entry_bytes = Vec::new();
            if !is_dir && file.size > 0 {
                let (mut read_stream, _) = original_zip_file
                    .zip_file_open_read_stream(file.index as usize)
                    .ok()?;
                read_stream.read_to_end(&mut entry_bytes).ok()?;
                let _ = original_zip_file.zip_file_close_read_stream();
            }

            let compressed_data = deflate_raw_best(&entry_bytes)?;

            let mut crc_hasher = Crc32Hasher::new();
            crc_hasher.update(&entry_bytes);
            let crc = crc_hasher.finalize();

            entries.push(RawZipEntry {
                name: entry_name,
                compressed_size: compressed_data.len() as u64,
                uncompressed_size: entry_bytes.len() as u64,
                compressed_data,
                crc,
                flags,
                compression_method: 8,
                external_attributes: 0,
            });
        }

        let built = Self::build_torrentzip_archive(&entries);
        if Self::hard_stop_requested(control) {
            Self::remove_tmp_if_present(tmp_filename);
            original_zip_file.zip_file_close();
            return Some(Self::aborted_status(control));
        }
        fs::write(tmp_filename, built).ok()?;

        original_zip_file.zip_file_close();
        if source_path.exists() {
            let _ = fs::remove_file(source_path);
        }
        let _ = fs::rename(tmp_filename, out_filename);
        Some(TrrntZipStatus::VALID_TRRNTZIP)
    }

    pub fn rezip_files(
        zipped_files: &[ZippedFile],
        original_zip_file: &mut dyn ICompress,
        output_type: ZipStructure,
    ) -> TrrntZipStatus {
        Self::rezip_files_with_control(zipped_files, original_zip_file, output_type, None)
    }

    pub fn rezip_files_with_control(
        zipped_files: &[ZippedFile],
        original_zip_file: &mut dyn ICompress,
        output_type: ZipStructure,
        control: Option<&ProcessControl>,
    ) -> TrrntZipStatus {
        let filename = original_zip_file.zip_filename().to_string();
        let path = Path::new(&filename);
        let parent = path.parent().unwrap_or(Path::new(""));
        let stem = path.file_stem().unwrap_or_default().to_string_lossy();

        let out_ext = match output_type {
            ZipStructure::ZipTrrnt | ZipStructure::ZipTDC | ZipStructure::ZipZSTD => ".zip",
            _ => ".7z",
        };

        let tmp_filename = parent.join(format!(
            "__{}.samtmp",
            path.file_name().unwrap().to_string_lossy()
        ));
        let out_filename = parent.join(format!("{}{}", stem, out_ext));

        if path.extension().unwrap_or_default() != out_ext.trim_start_matches('.')
            && out_filename.exists()
        {
            return TrrntZipStatus::REPEAT_FILES_FOUND;
        }

        Self::remove_tmp_if_present(&tmp_filename);

        if Self::hard_stop_requested(control) {
            original_zip_file.zip_file_close();
            return Self::aborted_status(control);
        }

        if output_type == ZipStructure::ZipTrrnt {
            if let Some(status) = Self::rezip_torrentzip_raw(
                zipped_files,
                original_zip_file,
                &tmp_filename,
                &out_filename,
                path,
                control,
            ) {
                return status;
            }
        }

        if matches!(
            output_type,
            ZipStructure::SevenZipSLZMA
                | ZipStructure::SevenZipNLZMA
                | ZipStructure::SevenZipSZSTD
                | ZipStructure::SevenZipNZSTD
        ) {
            let prepared = match Self::prepare_seven_zip_file_list(zipped_files) {
                Ok(v) => v,
                Err(status) => {
                    original_zip_file.zip_file_close();
                    return status;
                }
            };

            let staging_dir = parent.join(format!(
                "__{}.samtmp.dir",
                path.file_name().unwrap().to_string_lossy()
            ));
            let _ = fs::remove_dir_all(&staging_dir);
            if fs::create_dir_all(&staging_dir).is_err() {
                original_zip_file.zip_file_close();
                return TrrntZipStatus::CATCH_ERROR;
            }

            let sep = std::path::MAIN_SEPARATOR.to_string();
            for t in &prepared {
                if Self::hard_stop_requested(control) {
                    let _ = fs::remove_dir_all(&staging_dir);
                    original_zip_file.zip_file_close();
                    Self::remove_tmp_if_present(&tmp_filename);
                    return Self::aborted_status(control);
                }

                let rel = t.name.replace('/', &sep);
                let staged_path = staging_dir.join(rel);
                if t.is_dir || t.name.ends_with('/') {
                    let _ = fs::create_dir_all(&staged_path);
                    continue;
                }
                if let Some(parent_dir) = staged_path.parent() {
                    let _ = fs::create_dir_all(parent_dir);
                }

                let mut entry_bytes = Vec::new();
                if t.size > 0 {
                    match original_zip_file.zip_file_open_read_stream(t.index as usize) {
                        Ok((mut stream, _)) => {
                            if stream.read_to_end(&mut entry_bytes).is_err() {
                                let _ = fs::remove_dir_all(&staging_dir);
                                original_zip_file.zip_file_close();
                                Self::remove_tmp_if_present(&tmp_filename);
                                return TrrntZipStatus::CORRUPT_ZIP;
                            }
                        }
                        Err(_) => {
                            let _ = fs::remove_dir_all(&staging_dir);
                            original_zip_file.zip_file_close();
                            Self::remove_tmp_if_present(&tmp_filename);
                            return TrrntZipStatus::CORRUPT_ZIP;
                        }
                    }
                    let _ = original_zip_file.zip_file_close_read_stream();
                }

                if fs::write(&staged_path, &entry_bytes).is_err() {
                    let _ = fs::remove_dir_all(&staging_dir);
                    original_zip_file.zip_file_close();
                    Self::remove_tmp_if_present(&tmp_filename);
                    return TrrntZipStatus::CATCH_ERROR;
                }
            }

            Self::remove_tmp_if_present(&tmp_filename);
            let _ = fs::remove_file(&tmp_filename);
            let out_file = match fs::File::create(&tmp_filename) {
                Ok(f) => f,
                Err(_) => {
                    let _ = fs::remove_dir_all(&staging_dir);
                    original_zip_file.zip_file_close();
                    Self::remove_tmp_if_present(&tmp_filename);
                    return TrrntZipStatus::CATCH_ERROR;
                }
            };
            let mut writer = match ArchiveWriter::new(out_file) {
                Ok(w) => w,
                Err(_) => {
                    let _ = fs::remove_dir_all(&staging_dir);
                    original_zip_file.zip_file_close();
                    Self::remove_tmp_if_present(&tmp_filename);
                    return TrrntZipStatus::CATCH_ERROR;
                }
            };
            writer.set_encrypt_header(false);
            let solid = matches!(
                output_type,
                ZipStructure::SevenZipSLZMA | ZipStructure::SevenZipSZSTD
            );

            if solid {
                let config = match output_type {
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

                for t in prepared
                    .iter()
                    .filter(|t| t.is_dir || t.name.ends_with('/'))
                {
                    let rel = t.name.replace('/', &sep);
                    let staged_path = staging_dir.join(rel);
                    let entry = ArchiveEntry::from_path(&staged_path, t.name.clone());
                    if writer.push_archive_entry::<&[u8]>(entry, None).is_err() {
                        let _ = fs::remove_dir_all(&staging_dir);
                        original_zip_file.zip_file_close();
                        Self::remove_tmp_if_present(&tmp_filename);
                        return TrrntZipStatus::CATCH_ERROR;
                    }
                }

                let mut file_entries = Vec::new();
                let mut readers: Vec<SourceReader<fs::File>> = Vec::new();
                for t in prepared
                    .iter()
                    .filter(|t| !(t.is_dir || t.name.ends_with('/')))
                {
                    let rel = t.name.replace('/', &sep);
                    let staged_path = staging_dir.join(rel);
                    let entry = ArchiveEntry::from_path(&staged_path, t.name.clone());
                    let Ok(src) = fs::File::open(&staged_path) else {
                        let _ = fs::remove_dir_all(&staging_dir);
                        original_zip_file.zip_file_close();
                        Self::remove_tmp_if_present(&tmp_filename);
                        return TrrntZipStatus::CATCH_ERROR;
                    };
                    file_entries.push(entry);
                    readers.push(SourceReader::new(src));
                }
                if !file_entries.is_empty()
                    && writer.push_archive_entries(file_entries, readers).is_err()
                {
                    let _ = fs::remove_dir_all(&staging_dir);
                    original_zip_file.zip_file_close();
                    Self::remove_tmp_if_present(&tmp_filename);
                    return TrrntZipStatus::CATCH_ERROR;
                }
            } else {
                for t in &prepared {
                    let rel = t.name.replace('/', &sep);
                    let staged_path = staging_dir.join(rel);
                    let entry = ArchiveEntry::from_path(&staged_path, t.name.clone());
                    if t.is_dir || t.name.ends_with('/') {
                        if writer.push_archive_entry::<&[u8]>(entry, None).is_err() {
                            let _ = fs::remove_dir_all(&staging_dir);
                            original_zip_file.zip_file_close();
                            Self::remove_tmp_if_present(&tmp_filename);
                            return TrrntZipStatus::CATCH_ERROR;
                        }
                        continue;
                    }
                    let Ok(src) = fs::File::open(&staged_path) else {
                        let _ = fs::remove_dir_all(&staging_dir);
                        original_zip_file.zip_file_close();
                        Self::remove_tmp_if_present(&tmp_filename);
                        return TrrntZipStatus::CATCH_ERROR;
                    };
                    let config = match output_type {
                        ZipStructure::SevenZipSZSTD | ZipStructure::SevenZipNZSTD => {
                            EncoderConfiguration::new(EncoderMethod::ZSTD).with_options(
                                EncoderOptions::Zstd(ZstandardOptions::from_level(19)),
                            )
                        }
                        _ => {
                            let mut lz = LzmaOptions::from_level(9);
                            lz.set_dictionary_size(compress::seven_zip::seven_zip_dictionary_size_from_uncompressed_size(t.size));
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
                    if writer.push_archive_entry(entry, Some(src)).is_err() {
                        let _ = fs::remove_dir_all(&staging_dir);
                        original_zip_file.zip_file_close();
                        Self::remove_tmp_if_present(&tmp_filename);
                        return TrrntZipStatus::CATCH_ERROR;
                    }
                }
            }
            if writer.finish().is_err() {
                let _ = fs::remove_dir_all(&staging_dir);
                original_zip_file.zip_file_close();
                Self::remove_tmp_if_present(&tmp_filename);
                return TrrntZipStatus::CATCH_ERROR;
            }

            let _ = compress::seven_zip::apply_romvault7z_marker(&tmp_filename, output_type);

            original_zip_file.zip_file_close();
            if path.exists() {
                let _ = fs::remove_file(path);
            }
            let _ = fs::rename(&tmp_filename, &out_filename);
            let _ = fs::remove_dir_all(&staging_dir);
            return TrrntZipStatus::VALID_TRRNTZIP;
        }

        // Creating output archive
        // Note: For a fully faithful port, we would instantiate a ZipFile here.
        // For simplicity we will assume `original_zip_file` creates a new instance or we create a new standard zip writer.
        let mut zip_file_out = compress::zip_file::ZipFile::new();
        let zr = if matches!(output_type, ZipStructure::ZipZSTD | ZipStructure::ZipTDC) {
            zip_file_out
                .zip_file_create_with_structure(&tmp_filename.to_string_lossy(), output_type)
        } else {
            zip_file_out.zip_file_create(&tmp_filename.to_string_lossy())
        };

        if zr != ZipReturn::ZipGood {
            Self::remove_tmp_if_present(&tmp_filename);
            return TrrntZipStatus::CATCH_ERROR;
        }

        let output_compression_type = get_compression_type(output_type);

        let mut buffer = [0u8; 8192];

        let prepared = if matches!(output_type, ZipStructure::ZipZSTD) {
            match Self::prepare_zip_file_list(zipped_files, true) {
                Ok(v) => v,
                Err(status) => {
                    zip_file_out.zip_file_close_failed();
                    original_zip_file.zip_file_close();
                    Self::remove_tmp_if_present(&tmp_filename);
                    return status;
                }
            }
        } else if matches!(output_type, ZipStructure::ZipTDC) {
            match Self::prepare_zip_file_list(zipped_files, false) {
                Ok(v) => v,
                Err(status) => {
                    zip_file_out.zip_file_close_failed();
                    original_zip_file.zip_file_close();
                    Self::remove_tmp_if_present(&tmp_filename);
                    return status;
                }
            }
        } else {
            zipped_files.to_vec()
        };

        for t in &prepared {
            if Self::hard_stop_requested(control) {
                zip_file_out.zip_file_close_failed();
                original_zip_file.zip_file_close();
                Self::remove_tmp_if_present(&tmp_filename);
                return Self::aborted_status(control);
            }
            let stream_size = t.size;
            let source_is_structured = original_zip_file.zip_struct() == output_type;

            let mut mod_time = None;
            if output_type == ZipStructure::ZipTDC {
                if let Some(header) = original_zip_file.get_file_header(t.index as usize) {
                    mod_time = Some(header.header_last_modified);
                }
            }

            if source_is_structured
                && matches!(output_type, ZipStructure::ZipZSTD | ZipStructure::ZipTDC)
            {
                let mut uncompressed_size = 0u64;
                let mut crc_be: Option<Vec<u8>> = None;
                if let Some(header) = original_zip_file.get_file_header(t.index as usize) {
                    uncompressed_size = header.uncompressed_size;
                    crc_be = header
                        .crc
                        .as_ref()
                        .filter(|b| b.len() == 4)
                        .cloned()
                        .or_else(|| {
                            if uncompressed_size == 0 {
                                Some(vec![0, 0, 0, 0])
                            } else {
                                None
                            }
                        });
                }

                if let Some(crc_be) = crc_be.as_deref() {
                    if let Ok((mut raw_stream, _, method)) =
                        original_zip_file.zip_file_open_read_stream_ex(t.index as usize, true)
                    {
                        if method == output_compression_type {
                            let mut compressed = Vec::new();
                            if raw_stream.read_to_end(&mut compressed).is_ok() {
                                let _ = original_zip_file.zip_file_close_read_stream();
                                if let Ok(mut write_stream) = zip_file_out
                                    .zip_file_open_write_stream(
                                        true,
                                        &t.name,
                                        uncompressed_size,
                                        output_compression_type,
                                        mod_time,
                                    )
                                {
                                    let _ = write_stream.write_all(&compressed);
                                    let _ = write_stream.flush();
                                    let _ = zip_file_out.zip_file_close_write_stream(crc_be);
                                    continue;
                                }
                            } else {
                                let _ = original_zip_file.zip_file_close_read_stream();
                            }
                        } else {
                            let _ = original_zip_file.zip_file_close_read_stream();
                        }
                    }
                }
            }

            let mut read_stream: Box<dyn Read> = Box::new(std::io::empty());

            if t.size > 0 {
                match original_zip_file.zip_file_open_read_stream(t.index as usize) {
                    Ok((stream, _)) => {
                        read_stream = stream;
                    }
                    Err(_) => {
                        zip_file_out.zip_file_close_failed();
                        original_zip_file.zip_file_close();
                        Self::remove_tmp_if_present(&tmp_filename);
                        return TrrntZipStatus::CORRUPT_ZIP;
                    }
                }
            }

            match zip_file_out.zip_file_open_write_stream(
                false,
                &t.name,
                stream_size,
                output_compression_type,
                mod_time,
            ) {
                Ok(mut write_stream) => {
                    let mut crc_hasher = Crc32Hasher::new();
                    let mut size_to_go = stream_size;

                    while size_to_go > 0 {
                        if Self::hard_stop_requested(control) {
                            zip_file_out.zip_file_close_failed();
                            let _ = original_zip_file.zip_file_close_read_stream();
                            original_zip_file.zip_file_close();
                            Self::remove_tmp_if_present(&tmp_filename);
                            return Self::aborted_status(control);
                        }
                        let size_now = std::cmp::min(size_to_go as usize, buffer.len());
                        if let Ok(n) = read_stream.read(&mut buffer[..size_now]) {
                            if n == 0 {
                                break;
                            }
                            crc_hasher.update(&buffer[..n]);
                            let _ = write_stream.write_all(&buffer[..n]);
                            size_to_go -= n as u64;
                        } else {
                            break;
                        }
                    }

                    let _ = write_stream.flush();
                    let _ = original_zip_file.zip_file_close_read_stream();

                    let crc_bytes = crc_hasher.finalize().to_be_bytes();
                    let _ = zip_file_out.zip_file_close_write_stream(&crc_bytes);
                }
                Err(_) => {
                    zip_file_out.zip_file_close_failed();
                    original_zip_file.zip_file_close();
                    Self::remove_tmp_if_present(&tmp_filename);
                    return TrrntZipStatus::CORRUPT_ZIP;
                }
            }
        }

        zip_file_out.zip_file_close();
        original_zip_file.zip_file_close();

        if output_type == ZipStructure::ZipZSTD || output_type == ZipStructure::ZipTDC {
            let _ = Self::apply_structured_zip_metadata(&tmp_filename, output_type);
        }

        // Swap files
        if path.exists() {
            let _ = fs::remove_file(path);
        }

        let _ = fs::rename(&tmp_filename, &out_filename);

        TrrntZipStatus::VALID_TRRNTZIP
    }
}
