use std::io::{Read, Write};
use std::fs;
use std::path::Path;
use compress::i_compress::ICompress;
use compress::zip_enums::ZipReturn;
use compress::structured_archive::{ZipStructure, get_compression_type};
use crate::trrntzip_status::TrrntZipStatus;
use crate::zipped_file::ZippedFile;
use crc32fast::Hasher as Crc32Hasher;

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
}

impl TorrentZipRebuild {
    const TORRENTZIP_DOS_TIME: u16 = 48128;
    const TORRENTZIP_DOS_DATE: u16 = 8600;

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

    fn rezip_torrentzip_raw(
        zipped_files: &[ZippedFile],
        original_zip_file: &mut dyn ICompress,
        tmp_filename: &Path,
        out_filename: &Path,
        source_path: &Path,
    ) -> Option<TrrntZipStatus> {
        let zip_bytes = fs::read(source_path).ok()?;
        let mut entries = Vec::with_capacity(zipped_files.len());

        for file in zipped_files {
            if file.is_dir {
                return None;
            }
            let entry = Self::read_raw_zip_entry(&zip_bytes, &file.name)?;
            entries.push(entry);
        }

        let built = Self::build_torrentzip_archive(&entries);
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

        if tmp_filename.exists() {
            let _ = fs::remove_file(&tmp_filename);
        }

        if output_type == ZipStructure::ZipTrrnt {
            if let Some(status) = Self::rezip_torrentzip_raw(
                zipped_files,
                original_zip_file,
                &tmp_filename,
                &out_filename,
                path,
            ) {
                return status;
            }
        }

        // Creating output archive
        // Note: For a fully faithful port, we would instantiate a ZipFile here. 
        // For simplicity we will assume `original_zip_file` creates a new instance or we create a new standard zip writer.
        let mut zip_file_out = compress::zip_file::ZipFile::new();
        let zr = zip_file_out.zip_file_create(&tmp_filename.to_string_lossy());
        
        if zr != ZipReturn::ZipGood {
            return TrrntZipStatus::CATCH_ERROR;
        }

        let output_compression_type = get_compression_type(output_type);

        let mut buffer = [0u8; 8192];

        for t in zipped_files {
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
                        let _ = fs::remove_file(&tmp_filename);
                        return TrrntZipStatus::CORRUPT_ZIP;
                    }
                }
            }

            match zip_file_out.zip_file_open_write_stream(false, &t.name, stream_size, output_compression_type, None) {
                Ok(mut write_stream) => {
                    let mut crc_hasher = Crc32Hasher::new();
                    let mut size_to_go = stream_size;

                    while size_to_go > 0 {
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
                    let _ = fs::remove_file(&tmp_filename);
                    return TrrntZipStatus::CORRUPT_ZIP;
                }
            }
        }

        zip_file_out.zip_file_close();
        original_zip_file.zip_file_close();

        // Swap files
        if path.exists() {
            let _ = fs::remove_file(path);
        }
        
        let _ = fs::rename(&tmp_filename, &out_filename);

        TrrntZipStatus::VALID_TRRNTZIP
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use compress::i_compress::ICompress;
    use compress::zip_file::ZipFile;
    use tempfile::tempdir;
    use zip::write::FileOptions;
    use zip::{CompressionMethod, ZipArchive, ZipWriter};

    #[test]
    fn test_rezip_files_builds_torrentzip_with_raw_stream_reuse() {
        let temp = tempdir().unwrap();
        let source_path = temp.path().join("sample.zip");

        {
            let file = fs::File::create(&source_path).unwrap();
            let mut writer = ZipWriter::new(file);
            let options = FileOptions::default()
                .compression_method(CompressionMethod::Deflated)
                .compression_level(Some(9));
            writer.start_file("b.bin", options).unwrap();
            writer.write_all(b"bbbb").unwrap();
            writer.start_file("a.bin", options).unwrap();
            writer.write_all(b"aaaa").unwrap();
            writer.finish().unwrap();
        }

        let source_bytes = fs::read(&source_path).unwrap();
        let source_a = TorrentZipRebuild::read_raw_zip_entry(&source_bytes, "a.bin").unwrap();

        let mut zip_file = ZipFile::new();
        assert_eq!(
            zip_file.zip_file_open(&source_path.to_string_lossy(), 0, true),
            ZipReturn::ZipGood
        );

        let zipped_files = vec![
            ZippedFile {
                index: 1,
                name: "a.bin".to_string(),
                size: 4,
                crc: None,
                sha1: None,
                is_dir: false,
            },
            ZippedFile {
                index: 0,
                name: "b.bin".to_string(),
                size: 4,
                crc: None,
                sha1: None,
                is_dir: false,
            },
        ];

        let status = TorrentZipRebuild::rezip_files(&zipped_files, &mut zip_file, ZipStructure::ZipTrrnt);
        assert_eq!(status, TrrntZipStatus::VALID_TRRNTZIP);

        let rebuilt_bytes = fs::read(&source_path).unwrap();
        let rebuilt_a = TorrentZipRebuild::read_raw_zip_entry(&rebuilt_bytes, "a.bin").unwrap();
        assert_eq!(source_a.compressed_data, rebuilt_a.compressed_data);

        let archive = ZipArchive::new(fs::File::open(&source_path).unwrap()).unwrap();
        let eocd_offset = rebuilt_bytes
            .windows(4)
            .rposition(|window| window == [0x50, 0x4B, 0x05, 0x06])
            .unwrap();
        let central_directory_size = u32::from_le_bytes([
            rebuilt_bytes[eocd_offset + 12],
            rebuilt_bytes[eocd_offset + 13],
            rebuilt_bytes[eocd_offset + 14],
            rebuilt_bytes[eocd_offset + 15],
        ]) as usize;
        let central_directory_offset = u32::from_le_bytes([
            rebuilt_bytes[eocd_offset + 16],
            rebuilt_bytes[eocd_offset + 17],
            rebuilt_bytes[eocd_offset + 18],
            rebuilt_bytes[eocd_offset + 19],
        ]) as usize;
        let mut crc = Crc32Hasher::new();
        crc.update(
            &rebuilt_bytes[central_directory_offset..central_directory_offset + central_directory_size],
        );
        let expected_comment = format!("TORRENTZIPPED-{:08X}", crc.finalize());
        assert_eq!(String::from_utf8_lossy(archive.comment()), expected_comment);
    }
}
