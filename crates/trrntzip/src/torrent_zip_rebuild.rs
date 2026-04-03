use std::io::{Read, Write};
use std::fs;
use std::path::Path;
use compress::i_compress::ICompress;
use compress::zip_enums::ZipReturn;
use compress::structured_archive::{ZipDateType, ZipStructure, get_compression_type, get_zip_comment_id, get_zip_date_time_type};
use compress::deflate_raw_best;
use crate::process_control::ProcessControl;
use crate::trrntzip_status::TrrntZipStatus;
use crate::zipped_file::ZippedFile;
use crc32fast::Hasher as Crc32Hasher;
use flate2::write::DeflateEncoder;
use flate2::Compression;
use sevenz_rust::compress_to_path as compress_to_7z_path;

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
    compressed_size: u32,
    uncompressed_size: u32,
    flags: u16,
    compression_method: u16,
    external_attributes: u32,
}

impl TorrentZipRebuild {
    const TORRENTZIP_DOS_TIME: u16 = 48128;
    const TORRENTZIP_DOS_DATE: u16 = 8600;

    fn torrentzip_flags(name: &str) -> u16 {
        0x0002 | if name.is_ascii() { 0 } else { 0x0800 }
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
            ZipDateType::TrrntZip => (Some(Self::TORRENTZIP_DOS_TIME), Some(Self::TORRENTZIP_DOS_DATE)),
            ZipDateType::None => (Some(0u16), Some(0u16)),
            ZipDateType::DateTime => (None, None),
            ZipDateType::Undefined => return false,
        };

        let version_needed: u16 = if expected_method == 93 { 63 } else { 20 };

        let mut local_offset = 0usize;
        while local_offset + 30 <= zip_bytes.len()
            && zip_bytes[local_offset..local_offset + 4] == local_header_signature
        {
            let flags = u16::from_le_bytes([zip_bytes[local_offset + 6], zip_bytes[local_offset + 7]]);
            let normalized_flags = 0x0002 | (flags & utf8_flag);

            zip_bytes[local_offset + 4..local_offset + 6].copy_from_slice(&version_needed.to_le_bytes());
            zip_bytes[local_offset + 6..local_offset + 8].copy_from_slice(&normalized_flags.to_le_bytes());
            zip_bytes[local_offset + 8..local_offset + 10].copy_from_slice(&expected_method.to_le_bytes());
            if let (Some(t), Some(d)) = (time_patch, date_patch) {
                zip_bytes[local_offset + 10..local_offset + 12].copy_from_slice(&t.to_le_bytes());
                zip_bytes[local_offset + 12..local_offset + 14].copy_from_slice(&d.to_le_bytes());
            }

            let name_len = u16::from_le_bytes([zip_bytes[local_offset + 26], zip_bytes[local_offset + 27]]) as usize;
            let extra_len = u16::from_le_bytes([zip_bytes[local_offset + 28], zip_bytes[local_offset + 29]]) as usize;
            let comp_size = u32::from_le_bytes([
                zip_bytes[local_offset + 18],
                zip_bytes[local_offset + 19],
                zip_bytes[local_offset + 20],
                zip_bytes[local_offset + 21],
            ]) as usize;

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

        let central_directory_size = u32::from_le_bytes([
            zip_bytes[eocd_offset + 12],
            zip_bytes[eocd_offset + 13],
            zip_bytes[eocd_offset + 14],
            zip_bytes[eocd_offset + 15],
        ]) as usize;
        let central_directory_offset = u32::from_le_bytes([
            zip_bytes[eocd_offset + 16],
            zip_bytes[eocd_offset + 17],
            zip_bytes[eocd_offset + 18],
            zip_bytes[eocd_offset + 19],
        ]) as usize;
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

            let flags = u16::from_le_bytes([zip_bytes[central_offset + 8], zip_bytes[central_offset + 9]]);
            let normalized_flags = 0x0002 | (flags & utf8_flag);

            zip_bytes[central_offset + 4..central_offset + 6].copy_from_slice(&0u16.to_le_bytes());
            zip_bytes[central_offset + 6..central_offset + 8].copy_from_slice(&version_needed.to_le_bytes());
            zip_bytes[central_offset + 8..central_offset + 10].copy_from_slice(&normalized_flags.to_le_bytes());
            zip_bytes[central_offset + 10..central_offset + 12].copy_from_slice(&expected_method.to_le_bytes());
            if let (Some(t), Some(d)) = (time_patch, date_patch) {
                zip_bytes[central_offset + 12..central_offset + 14].copy_from_slice(&t.to_le_bytes());
                zip_bytes[central_offset + 14..central_offset + 16].copy_from_slice(&d.to_le_bytes());
            }
            zip_bytes[central_offset + 32..central_offset + 34].copy_from_slice(&0u16.to_le_bytes());
            zip_bytes[central_offset + 34..central_offset + 36].copy_from_slice(&0u16.to_le_bytes());
            zip_bytes[central_offset + 36..central_offset + 38].copy_from_slice(&0u16.to_le_bytes());
            zip_bytes[central_offset + 38..central_offset + 40].copy_from_slice(&0u16.to_le_bytes());
            zip_bytes[central_offset + 40..central_offset + 42].copy_from_slice(&0u32.to_le_bytes());

            central_offset += 46 + file_name_length + extra_length + comment_length;
        }

        let mut crc = Crc32Hasher::new();
        crc.update(&zip_bytes[central_directory_offset..central_directory_offset + central_directory_size]);
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

    fn aborted_status(control: Option<&ProcessControl>) -> TrrntZipStatus {
        if control.is_some_and(|control| control.is_hard_stop_requested()) {
            TrrntZipStatus::USER_ABORTED_HARD
        } else {
            TrrntZipStatus::USER_ABORTED
        }
    }

    fn hard_stop_requested(control: Option<&ProcessControl>) -> bool {
        control.is_some_and(|control| {
            control.wait_one();
            control.is_soft_stop_requested()
        })
    }

    fn remove_tmp_if_present(tmp_filename: &Path) {
        if tmp_filename.exists() {
            let _ = fs::remove_file(tmp_filename);
        }
    }

    pub fn cleanup_samtmp_files(base_path: &Path, recursive: bool) -> usize {
        let mut removed = 0;

        let is_samtmp = base_path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.contains(".samtmp"));

        if is_samtmp {
            if base_path.is_dir() {
                if fs::remove_dir_all(base_path).is_ok() {
                    removed += 1;
                }
            } else if base_path.is_file() && fs::remove_file(base_path).is_ok() {
                removed += 1;
            }
            return removed;
        }

        let Ok(entries) = fs::read_dir(base_path) else {
            return 0;
        };

        for entry in entries.flatten() {
            let path = entry.path();
            let is_samtmp = path
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.contains(".samtmp"));
            if is_samtmp {
                if path.is_dir() {
                    if fs::remove_dir_all(&path).is_ok() {
                        removed += 1;
                    }
                } else if fs::remove_file(&path).is_ok() {
                    removed += 1;
                }
            } else if path.is_dir() && recursive {
                removed += Self::cleanup_samtmp_files(&path, true);
            }
        }

        removed
    }

    fn read_raw_zip_entry(zip_bytes: &[u8], entry_name: &str) -> Option<RawZipEntry> {
        let eocd_offset = zip_bytes
            .windows(4)
            .rposition(|window| window == [0x50, 0x4B, 0x05, 0x06])?;

        if eocd_offset + 22 > zip_bytes.len() {
            return None;
        }

        let central_directory_size = u32::from_le_bytes([
            zip_bytes[eocd_offset + 12],
            zip_bytes[eocd_offset + 13],
            zip_bytes[eocd_offset + 14],
            zip_bytes[eocd_offset + 15],
        ]) as usize;
        let central_directory_offset = u32::from_le_bytes([
            zip_bytes[eocd_offset + 16],
            zip_bytes[eocd_offset + 17],
            zip_bytes[eocd_offset + 18],
            zip_bytes[eocd_offset + 19],
        ]) as usize;

        if central_directory_offset + central_directory_size > zip_bytes.len() {
            return None;
        }

        let mut central_offset = central_directory_offset;
        while central_offset + 46 <= central_directory_offset + central_directory_size {
            if zip_bytes[central_offset..central_offset + 4] != [0x50, 0x4B, 0x01, 0x02] {
                return None;
            }

            let flags = u16::from_le_bytes([
                zip_bytes[central_offset + 8],
                zip_bytes[central_offset + 9],
            ]);
            let compression_method = u16::from_le_bytes([
                zip_bytes[central_offset + 10],
                zip_bytes[central_offset + 11],
            ]);
            let crc = u32::from_le_bytes([
                zip_bytes[central_offset + 16],
                zip_bytes[central_offset + 17],
                zip_bytes[central_offset + 18],
                zip_bytes[central_offset + 19],
            ]);
            let compressed_size = u32::from_le_bytes([
                zip_bytes[central_offset + 20],
                zip_bytes[central_offset + 21],
                zip_bytes[central_offset + 22],
                zip_bytes[central_offset + 23],
            ]);
            let uncompressed_size = u32::from_le_bytes([
                zip_bytes[central_offset + 24],
                zip_bytes[central_offset + 25],
                zip_bytes[central_offset + 26],
                zip_bytes[central_offset + 27],
            ]);
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
            let relative_offset = u32::from_le_bytes([
                zip_bytes[central_offset + 42],
                zip_bytes[central_offset + 43],
                zip_bytes[central_offset + 44],
                zip_bytes[central_offset + 45],
            ]) as usize;

            let name_start = central_offset + 46;
            let name_end = name_start + file_name_length;
            if name_end > zip_bytes.len() {
                return None;
            }

            let current_name = String::from_utf8_lossy(&zip_bytes[name_start..name_end]);
            if current_name == entry_name {
                if compression_method != 8 {
                    return None;
                }
                if (flags & 8) == 8 {
                    return None;
                }

                if relative_offset + 30 > zip_bytes.len()
                    || zip_bytes[relative_offset..relative_offset + 4] != [0x50, 0x4B, 0x03, 0x04]
                {
                    return None;
                }

                let local_name_length = u16::from_le_bytes([
                    zip_bytes[relative_offset + 26],
                    zip_bytes[relative_offset + 27],
                ]) as usize;
                let local_extra_length = u16::from_le_bytes([
                    zip_bytes[relative_offset + 28],
                    zip_bytes[relative_offset + 29],
                ]) as usize;
                let data_offset = relative_offset + 30 + local_name_length + local_extra_length;
                let data_end = data_offset + compressed_size as usize;

                if data_end > zip_bytes.len() {
                    return None;
                }

                return Some(RawZipEntry {
                    name: entry_name.to_string(),
                    compressed_data: zip_bytes[data_offset..data_end].to_vec(),
                    crc,
                    compressed_size,
                    uncompressed_size,
                    flags: 0x0002 | (flags & 0x0800),
                    compression_method: 8,
                    external_attributes: 0,
                });
            }

            central_offset += 46 + file_name_length + extra_length + comment_length;
        }

        None
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

        let mut entries = Vec::with_capacity(zipped_files.len());
        let source_bytes = fs::read(source_path).ok()?;

        for file in zipped_files {
            if file.is_dir || file.name.ends_with('/') {
                let name = if file.name.ends_with('/') {
                    file.name.clone()
                } else {
                    format!("{}/", file.name)
                };
                entries.push(RawZipEntry {
                    name: name.clone(),
                    compressed_data: Vec::new(),
                    crc: 0,
                    compressed_size: 0,
                    uncompressed_size: 0,
                    flags: Self::torrentzip_flags(&name),
                    compression_method: 0,
                    external_attributes: 0x10,
                });
                continue;
            }

            if let Some(mut raw) = Self::read_raw_zip_entry(&source_bytes, &file.name) {
                raw.flags = Self::torrentzip_flags(&file.name);
                raw.compression_method = 8;
                raw.external_attributes = 0;
                entries.push(raw);
                continue;
            }

            let mut entry_bytes = Vec::new();
            if file.size > 0 {
                let (mut read_stream, _) = original_zip_file
                    .zip_file_open_read_stream(file.index as usize)
                    .ok()?;
                read_stream.read_to_end(&mut entry_bytes).ok()?;
                let _ = original_zip_file.zip_file_close_read_stream();
            }

            let compressed_data = deflate_raw_best(&entry_bytes).or_else(|| {
                let mut encoder = DeflateEncoder::new(Vec::new(), Compression::best());
                encoder.write_all(&entry_bytes).ok()?;
                encoder.finish().ok()
            })?;

            let mut crc_hasher = Crc32Hasher::new();
            crc_hasher.update(&entry_bytes);
            let crc = crc_hasher.finalize();

            entries.push(RawZipEntry {
                name: file.name.clone(),
                compressed_size: compressed_data.len() as u32,
                uncompressed_size: entry_bytes.len() as u32,
                compressed_data,
                crc,
                flags: Self::torrentzip_flags(&file.name),
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
            ZipStructure::ZipTrrnt | ZipStructure::ZipZSTD => ".zip",
            _ => ".7z",
        };

        let tmp_filename = parent.join(format!("__{}.samtmp", path.file_name().unwrap().to_string_lossy()));
        let out_filename = parent.join(format!("{}{}", stem, out_ext));

        if path.extension().unwrap_or_default() != out_ext.trim_start_matches('.') {
            if out_filename.exists() {
                return TrrntZipStatus::REPEAT_FILES_FOUND;
            }
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
            let staging_dir = parent.join(format!("__{}.samtmp.dir", path.file_name().unwrap().to_string_lossy()));
            let _ = fs::remove_dir_all(&staging_dir);
            if fs::create_dir_all(&staging_dir).is_err() {
                original_zip_file.zip_file_close();
                return TrrntZipStatus::CATCH_ERROR;
            }

            for t in zipped_files {
                if Self::hard_stop_requested(control) {
                    let _ = fs::remove_dir_all(&staging_dir);
                    original_zip_file.zip_file_close();
                    Self::remove_tmp_if_present(&tmp_filename);
                    return Self::aborted_status(control);
                }

                let rel = t.name.replace('/', "\\");
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
            if compress_to_7z_path(&staging_dir, &tmp_filename).is_err() {
                let _ = fs::remove_dir_all(&staging_dir);
                original_zip_file.zip_file_close();
                Self::remove_tmp_if_present(&tmp_filename);
                return TrrntZipStatus::CATCH_ERROR;
            }

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
        let zr = zip_file_out.zip_file_create(&tmp_filename.to_string_lossy());
        
        if zr != ZipReturn::ZipGood {
            Self::remove_tmp_if_present(&tmp_filename);
            return TrrntZipStatus::CATCH_ERROR;
        }

        let output_compression_type = get_compression_type(output_type);

        let mut buffer = [0u8; 8192];

        for t in zipped_files {
            if Self::hard_stop_requested(control) {
                zip_file_out.zip_file_close_failed();
                original_zip_file.zip_file_close();
                Self::remove_tmp_if_present(&tmp_filename);
                return Self::aborted_status(control);
            }
            let mut read_stream: Box<dyn Read> = Box::new(std::io::empty());
            let stream_size = t.size;

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

            match zip_file_out.zip_file_open_write_stream(false, &t.name, stream_size, output_compression_type, None) {
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
                            if n == 0 { break; }
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

#[cfg(test)]
#[path = "tests/torrent_zip_rebuild_tests.rs"]
mod tests;
