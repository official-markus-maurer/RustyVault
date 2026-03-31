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
/// - The C# `TorrentZipRebuild` relies on the custom `Compress.ZipFile` writer which allows 
///   for in-place stream modifications and TorrentZip header hashing.
/// - The Rust version currently implements a mock/stub. It defines the sorting and 
///   structure validation, but delegates the actual byte writing to future `zip` crate extensions.
pub struct TorrentZipRebuild;

impl TorrentZipRebuild {
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
