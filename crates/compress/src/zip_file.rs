use std::cell::RefCell;
use std::fs::{self, File};
use std::io::{Read, Write, Seek, SeekFrom};
use std::path::Path;
use std::rc::Rc;

use crate::file_header::FileHeader;
use crate::i_compress::ICompress;
use crate::structured_archive::{ZipDateType, ZipStructure, get_compression_type, get_zip_date_time_type};
use crate::zip_enums::{ZipOpenType, ZipReturn};
use crate::codepage_437;
use crate::zip_extra_field;

use zip::write::FileOptions;
use zip::{CompressionMethod, DateTime, ZipArchive, ZipWriter};
use crc32fast::Hasher as Crc32Hasher;
use flate2::read::DeflateDecoder;

/// ICompress wrapper for `.zip` files.
/// 
/// `ZipFile` implements the `ICompress` trait for standard ZIP archives using the ecosystem
/// `zip` crate. It handles opening the archive, reading its internal Central Directory headers,
/// and streaming out the uncompressed byte payloads for the scanner.
/// 
/// Differences from C#:
/// - The C# `Compress.ZipFile` is a fully custom, hand-rolled ZIP parser. It allows for 
///   arbitrary byte-level injection, custom header formatting, and zero-copy streaming 
///   directly into a newly formatted `TorrentZip` output stream.
/// - This Rust implementation delegates to the standard `zip` crate. It perfectly supports 
///   extraction and hashing (`ZipFileOpenReadStream`), but does not yet implement the highly 
///   specialized in-place TorrentZip repacking APIs (`ZipFileOpenWriteStream`).
pub struct ZipFile {
    zip_filename: String,
    zip_open_type: ZipOpenType,
    time_stamp: i64,
    zip_struct: ZipStructure,
    
    archive: Option<ZipArchive<File>>,
    writer: Option<ZipWriter<File>>,
    
    file_headers: Vec<FileHeader>,
    file_comment: String,
    pending_write: Option<PendingWrite>,
}

struct PendingWrite {
    filename: String,
    compression_method: u16,
    mod_time: Option<i64>,
    buffer: Rc<RefCell<Vec<u8>>>,
}

struct SharedBufferWriter {
    buffer: Rc<RefCell<Vec<u8>>>,
}

impl Write for SharedBufferWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.buffer.borrow_mut().extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

struct LocalFileHeaderInfo {
    flags: u16,
    compression_method: u16,
    compressed_size: u64,
    uncompressed_size: u64,
    data_offset: u64,
}

#[cfg(test)]
#[path = "tests/zip_file_tests.rs"]
mod tests;

impl ZipFile {
    const TORRENTZIP_DOS_TIME: u16 = 48128;
    const TORRENTZIP_DOS_DATE: u16 = 8600;

    pub fn new() -> Self {
        Self {
            zip_filename: String::new(),
            zip_open_type: ZipOpenType::Closed,
            time_stamp: 0,
            zip_struct: ZipStructure::None,
            archive: None,
            writer: None,
            file_headers: Vec::new(),
            file_comment: String::new(),
            pending_write: None,
        }
    }

    fn ascii_lower(byte: u8) -> u8 {
        if byte >= b'A' && byte <= b'Z' {
            byte + 0x20
        } else {
            byte
        }
    }

    fn compare_ascii_casefolded_bytes(a: &[u8], b: &[u8]) -> i32 {
        let len = std::cmp::min(a.len(), b.len());
        for i in 0..len {
            let ca = Self::ascii_lower(a[i]);
            let cb = Self::ascii_lower(b[i]);
            if ca < cb {
                return -1;
            }
            if ca > cb {
                return 1;
            }
        }
        if a.len() < b.len() {
            -1
        } else if a.len() > b.len() {
            1
        } else {
            0
        }
    }

    fn trrntzip_string_compare(a: &str, b: &str) -> i32 {
        Self::compare_ascii_casefolded_bytes(a.as_bytes(), b.as_bytes())
    }

    fn decode_filename(file_name_bytes: &[u8], general_purpose_bit_flag: u16) -> Option<String> {
        if (general_purpose_bit_flag & (1 << 11)) != 0 {
            Some(std::str::from_utf8(file_name_bytes).ok()?.to_string())
        } else {
            Some(codepage_437::decode(file_name_bytes))
        }
    }

    fn validate_zip_structure(zip_path: &str) -> ZipStructure {
        let Ok(zip_bytes) = fs::read(zip_path) else {
            return ZipStructure::None;
        };

        let eocd_offset = zip_bytes
            .windows(4)
            .rposition(|window| window == [0x50, 0x4B, 0x05, 0x06]);

        let Some(eocd_offset) = eocd_offset else {
            return ZipStructure::None;
        };

        if eocd_offset + 22 > zip_bytes.len() {
            return ZipStructure::None;
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
        let comment_length = u16::from_le_bytes([
            zip_bytes[eocd_offset + 20],
            zip_bytes[eocd_offset + 21],
        ]) as usize;

        if eocd_offset + 22 + comment_length != zip_bytes.len() {
            return ZipStructure::None;
        }

        if central_directory_offset + central_directory_size > zip_bytes.len() {
            return ZipStructure::None;
        }

        let mut crc = Crc32Hasher::new();
        crc.update(&zip_bytes[central_directory_offset..central_directory_offset + central_directory_size]);
        let cd_crc = format!("{:08X}", crc.finalize());

        let comment_bytes = &zip_bytes[eocd_offset + 22..eocd_offset + 22 + comment_length];
        let comment = String::from_utf8_lossy(comment_bytes);

        let zip_struct = if comment.starts_with("TORRENTZIPPED-") {
            ZipStructure::ZipTrrnt
        } else if comment.starts_with("TDC-") {
            ZipStructure::ZipTDC
        } else if comment.starts_with("RVZSTD-") {
            ZipStructure::ZipZSTD
        } else {
            ZipStructure::None
        };

        if zip_struct == ZipStructure::None {
            return ZipStructure::None;
        }

        let expected_prefix = match zip_struct {
            ZipStructure::ZipTrrnt => "TORRENTZIPPED-",
            ZipStructure::ZipTDC => "TDC-",
            ZipStructure::ZipZSTD => "RVZSTD-",
            _ => "",
        };

        if comment.len() != expected_prefix.len() + 8 {
            return ZipStructure::None;
        }
        if &comment[expected_prefix.len()..] != cd_crc {
            return ZipStructure::None;
        }

        if !Self::validate_files_structure(&zip_bytes, central_directory_offset, central_directory_size, zip_struct) {
            return ZipStructure::None;
        }

        zip_struct
    }

    fn validate_files_structure(
        zip_bytes: &[u8],
        central_directory_offset: usize,
        central_directory_size: usize,
        zip_struct: ZipStructure,
    ) -> bool {
        let expected_compression = get_compression_type(zip_struct);
        let date_type = get_zip_date_time_type(zip_struct);
        let central_end = central_directory_offset + central_directory_size;
        let mut central_offset = central_directory_offset;

        let mut last_name: Option<String> = None;
        let saw_directory_entry_needing_check = matches!(zip_struct, ZipStructure::ZipTrrnt | ZipStructure::ZipZSTD);

        while central_offset + 46 <= central_end {
            if zip_bytes[central_offset..central_offset + 4] != [0x50, 0x4B, 0x01, 0x02] {
                return false;
            }

            let flags = u16::from_le_bytes([zip_bytes[central_offset + 8], zip_bytes[central_offset + 9]]);
            let compression_method = u16::from_le_bytes([zip_bytes[central_offset + 10], zip_bytes[central_offset + 11]]);
            let last_mod_time = u16::from_le_bytes([zip_bytes[central_offset + 12], zip_bytes[central_offset + 13]]);
            let last_mod_date = u16::from_le_bytes([zip_bytes[central_offset + 14], zip_bytes[central_offset + 15]]);
            let file_name_length = u16::from_le_bytes([zip_bytes[central_offset + 28], zip_bytes[central_offset + 29]]) as usize;
            let extra_length = u16::from_le_bytes([zip_bytes[central_offset + 30], zip_bytes[central_offset + 31]]) as usize;
            let comment_length = u16::from_le_bytes([zip_bytes[central_offset + 32], zip_bytes[central_offset + 33]]) as usize;
            let relative_offset = u32::from_le_bytes([
                zip_bytes[central_offset + 42],
                zip_bytes[central_offset + 43],
                zip_bytes[central_offset + 44],
                zip_bytes[central_offset + 45],
            ]) as usize;

            let name_start = central_offset + 46;
            let name_end = name_start + file_name_length;
            if name_end > zip_bytes.len() {
                return false;
            }
            let file_name_bytes = &zip_bytes[name_start..name_end];
            let name = match Self::decode_filename(file_name_bytes, flags) {
                Some(v) => v,
                None => return false,
            };

            if name.contains('\\') {
                return false;
            }

            let utf8_flag_set = (flags & (1 << 11)) != 0;
            let is_cp437 = codepage_437::is_code_page_437(&name);
            if is_cp437 == utf8_flag_set {
                return false;
            }

            if extra_length != 0 {
                return false;
            }

            if expected_compression != 8 && expected_compression != 93 {
                return false;
            }
            if compression_method != expected_compression {
                return false;
            }

            match date_type {
                ZipDateType::DateTime => {}
                ZipDateType::None => {
                    if last_mod_time != 0 || last_mod_date != 0 {
                        return false;
                    }
                }
                ZipDateType::TrrntZip => {
                    if last_mod_time != Self::TORRENTZIP_DOS_TIME || last_mod_date != Self::TORRENTZIP_DOS_DATE {
                        return false;
                    }
                }
                ZipDateType::Undefined => return false,
            }

            if relative_offset + 30 > zip_bytes.len() {
                return false;
            }
            if zip_bytes[relative_offset..relative_offset + 4] != [0x50, 0x4B, 0x03, 0x04] {
                return false;
            }

            let local_flags = u16::from_le_bytes([zip_bytes[relative_offset + 6], zip_bytes[relative_offset + 7]]);
            let local_compression = u16::from_le_bytes([zip_bytes[relative_offset + 8], zip_bytes[relative_offset + 9]]);
            let local_time = u16::from_le_bytes([zip_bytes[relative_offset + 10], zip_bytes[relative_offset + 11]]);
            let local_date = u16::from_le_bytes([zip_bytes[relative_offset + 12], zip_bytes[relative_offset + 13]]);
            let local_name_length = u16::from_le_bytes([zip_bytes[relative_offset + 26], zip_bytes[relative_offset + 27]]) as usize;
            let local_extra_length = u16::from_le_bytes([zip_bytes[relative_offset + 28], zip_bytes[relative_offset + 29]]) as usize;

            if local_extra_length != 0 {
                return false;
            }

            if local_name_length != file_name_length {
                return false;
            }

            if local_flags != flags || local_compression != compression_method || local_time != last_mod_time || local_date != last_mod_date {
                return false;
            }

            if let Some(prev) = last_name.as_ref() {
                if Self::trrntzip_string_compare(prev, &name) >= 0 {
                    return false;
                }
            }

            last_name = Some(name);

            central_offset += 46 + file_name_length + extra_length + comment_length;
        }

        if saw_directory_entry_needing_check {
            let mut central_offset = central_directory_offset;

            loop {
                if central_offset + 46 > central_end {
                    break;
                }
                if zip_bytes[central_offset..central_offset + 4] != [0x50, 0x4B, 0x01, 0x02] {
                    break;
                }

                let flags = u16::from_le_bytes([zip_bytes[central_offset + 8], zip_bytes[central_offset + 9]]);
                let file_name_length = u16::from_le_bytes([zip_bytes[central_offset + 28], zip_bytes[central_offset + 29]]) as usize;
                let extra_length = u16::from_le_bytes([zip_bytes[central_offset + 30], zip_bytes[central_offset + 31]]) as usize;
                let comment_length = u16::from_le_bytes([zip_bytes[central_offset + 32], zip_bytes[central_offset + 33]]) as usize;
                let name_start = central_offset + 46;
                let name_end = name_start + file_name_length;
                if name_end > zip_bytes.len() {
                    return false;
                }
                let Some(dir_name) = Self::decode_filename(&zip_bytes[name_start..name_end], flags) else {
                    return false;
                };

                let next_offset = central_offset + 46 + file_name_length + extra_length + comment_length;
                if next_offset + 46 > central_end {
                    break;
                }
                if zip_bytes[next_offset..next_offset + 4] != [0x50, 0x4B, 0x01, 0x02] {
                    break;
                }
                let next_flags = u16::from_le_bytes([zip_bytes[next_offset + 8], zip_bytes[next_offset + 9]]);
                let next_name_len = u16::from_le_bytes([zip_bytes[next_offset + 28], zip_bytes[next_offset + 29]]) as usize;
                let next_name_start = next_offset + 46;
                let next_name_end = next_name_start + next_name_len;
                if next_name_end > zip_bytes.len() {
                    return false;
                }
                let Some(next_name) = Self::decode_filename(&zip_bytes[next_name_start..next_name_end], next_flags) else {
                    return false;
                };

                if dir_name.ends_with('/') && next_name.len() > dir_name.len() {
                    let dir_bytes = dir_name.as_bytes();
                    let next_bytes = next_name.as_bytes();
                    if next_bytes.len() >= dir_bytes.len()
                        && Self::compare_ascii_casefolded_bytes(dir_bytes, &next_bytes[..dir_bytes.len()]) == 0
                    {
                        return false;
                    }
                }

                central_offset = next_offset;
            }
        }

        true
    }

    fn zip_datetime_from_i64(value: i64) -> Option<DateTime> {
        if value <= 0 {
            return None;
        }

        let year = (value / 10000000000) as u16;
        let month = ((value / 100000000) % 100) as u8;
        let day = ((value / 1000000) % 100) as u8;
        let hour = ((value / 10000) % 100) as u8;
        let minute = ((value / 100) % 100) as u8;
        let second = (value % 100) as u8;

        DateTime::from_date_and_time(year, month, day, hour, minute, second).ok()
    }

    fn compression_method_from_u16(value: u16) -> CompressionMethod {
        match value {
            0 => CompressionMethod::Stored,
            8 => CompressionMethod::Deflated,
            93 => CompressionMethod::Zstd,
            _ => CompressionMethod::Deflated,
        }
    }

    #[cfg(test)]
    fn read_local_header_offsets(zip_path: &str) -> Option<Vec<u64>> {
        let zip_bytes = fs::read(zip_path).ok()?;
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

        let mut local_offsets = Vec::new();
        let mut central_offset = central_directory_offset;
        while central_offset + 46 <= central_directory_offset + central_directory_size {
            if zip_bytes[central_offset..central_offset + 4] != [0x50, 0x4B, 0x01, 0x02] {
                return None;
            }

            let compressed_size_u32 = u32::from_le_bytes([
                zip_bytes[central_offset + 20],
                zip_bytes[central_offset + 21],
                zip_bytes[central_offset + 22],
                zip_bytes[central_offset + 23],
            ]);
            let uncompressed_size_u32 = u32::from_le_bytes([
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
            let relative_offset_u32 = u32::from_le_bytes([
                zip_bytes[central_offset + 42],
                zip_bytes[central_offset + 43],
                zip_bytes[central_offset + 44],
                zip_bytes[central_offset + 45],
            ]);

            let name_end = central_offset + 46 + file_name_length;
            let extra_end = name_end + extra_length;
            if extra_end > zip_bytes.len() {
                return None;
            }
            let extra = &zip_bytes[name_end..extra_end];
            let extra_info = zip_extra_field::parse_extra_fields(
                extra,
                true,
                uncompressed_size_u32,
                compressed_size_u32,
                relative_offset_u32,
            );

            let relative_offset_u64 = if relative_offset_u32 == 0xFFFF_FFFF {
                extra_info.local_header_offset?
            } else {
                relative_offset_u32 as u64
            };

            local_offsets.push(relative_offset_u64);
            central_offset += 46 + file_name_length + extra_length + comment_length;
        }

        Some(local_offsets)
    }

    fn read_local_file_header_at(file: &mut File, local_index_offset: u64) -> Result<LocalFileHeaderInfo, ZipReturn> {
        if file.seek(SeekFrom::Start(local_index_offset)).is_err() {
            return Err(ZipReturn::ZipErrorReadingFile);
        }

        let mut header = [0u8; 30];
        if file.read_exact(&mut header).is_err() {
            return Err(ZipReturn::ZipLocalFileHeaderError);
        }
        if header[0..4] != [0x50, 0x4B, 0x03, 0x04] {
            return Err(ZipReturn::ZipLocalFileHeaderError);
        }

        let flags = u16::from_le_bytes([header[6], header[7]]);
        let compression_method = u16::from_le_bytes([header[8], header[9]]);
        let compressed_size_u32 = u32::from_le_bytes([header[18], header[19], header[20], header[21]]);
        let uncompressed_size_u32 = u32::from_le_bytes([header[22], header[23], header[24], header[25]]);
        let file_name_length = u16::from_le_bytes([header[26], header[27]]) as u64;
        let extra_length = u16::from_le_bytes([header[28], header[29]]) as u64;

        let extra_start = local_index_offset
            .saturating_add(30)
            .saturating_add(file_name_length);
        let extra_end = extra_start.saturating_add(extra_length);
        let data_offset = extra_end;

        let mut extra = vec![0u8; extra_length as usize];
        if extra_length > 0 {
            if file.seek(SeekFrom::Start(extra_start)).is_err() {
                return Err(ZipReturn::ZipErrorReadingFile);
            }
            if file.read_exact(&mut extra).is_err() {
                return Err(ZipReturn::ZipErrorReadingFile);
            }
        }

        let extra_info = zip_extra_field::parse_extra_fields(
            &extra,
            false,
            uncompressed_size_u32,
            compressed_size_u32,
            0,
        );

        let compressed_size = if compressed_size_u32 == 0xFFFF_FFFF {
            extra_info
                .compressed_size
                .ok_or(ZipReturn::ZipLocalFileHeaderError)?
        } else {
            compressed_size_u32 as u64
        };
        let uncompressed_size = if uncompressed_size_u32 == 0xFFFF_FFFF {
            extra_info
                .uncompressed_size
                .ok_or(ZipReturn::ZipLocalFileHeaderError)?
        } else {
            uncompressed_size_u32 as u64
        };

        Ok(LocalFileHeaderInfo {
            flags,
            compression_method,
            compressed_size,
            uncompressed_size,
            data_offset,
        })
    }

    pub fn zip_file_open_read_stream_ex(
        &mut self,
        index: usize,
        raw: bool,
    ) -> Result<(Box<dyn Read>, u64, u16), ZipReturn> {
        if self.zip_open_type != ZipOpenType::OpenRead {
            return Err(ZipReturn::ZipReadingFromOutputFile);
        }
        let local_head = self
            .get_file_header(index)
            .and_then(|h| h.local_head)
            .ok_or(ZipReturn::ZipCannotFastOpen)?;
        self.zip_file_open_read_stream_from_local_header_pointer(local_head, raw)
    }

    pub fn zip_file_open_read_stream_from_local_header_pointer(
        &mut self,
        local_index_offset: u64,
        raw: bool,
    ) -> Result<(Box<dyn Read>, u64, u16), ZipReturn> {
        if self.zip_open_type != ZipOpenType::OpenRead {
            return Err(ZipReturn::ZipReadingFromOutputFile);
        }

        let mut file = File::open(&self.zip_filename).map_err(|_| ZipReturn::ZipErrorOpeningFile)?;
        let info = Self::read_local_file_header_at(&mut file, local_index_offset)?;
        if (info.flags & 8) == 8 {
            return Err(ZipReturn::ZipCannotFastOpen);
        }

        if file.seek(SeekFrom::Start(info.data_offset)).is_err() {
            return Err(ZipReturn::ZipErrorReadingFile);
        }
        let mut compressed = vec![0u8; info.compressed_size as usize];
        if file.read_exact(&mut compressed).is_err() {
            return Err(ZipReturn::ZipErrorReadingFile);
        }

        if raw {
            return Ok((
                Box::new(std::io::Cursor::new(compressed)),
                info.compressed_size,
                info.compression_method,
            ));
        }

        match info.compression_method {
            0 => Ok((
                Box::new(std::io::Cursor::new(compressed)),
                info.uncompressed_size,
                info.compression_method,
            )),
            8 => {
                let mut decoder = DeflateDecoder::new(std::io::Cursor::new(compressed));
                let mut out = Vec::with_capacity(info.uncompressed_size as usize);
                if decoder.read_to_end(&mut out).is_err() {
                    return Err(ZipReturn::ZipErrorGettingDataStream);
                }
                Ok((
                    Box::new(std::io::Cursor::new(out)),
                    info.uncompressed_size,
                    info.compression_method,
                ))
            }
            _ => Err(ZipReturn::ZipUnsupportedCompression),
        }
    }

    fn read_central_directory(zip_path: &str) -> Option<Vec<FileHeader>> {
        let zip_bytes = fs::read(zip_path).ok()?;
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

        let mut out = Vec::new();
        let mut central_offset = central_directory_offset;
        let central_end = central_directory_offset + central_directory_size;
        while central_offset + 46 <= central_end {
            if zip_bytes[central_offset..central_offset + 4] != [0x50, 0x4B, 0x01, 0x02] {
                return None;
            }

            let flags = u16::from_le_bytes([zip_bytes[central_offset + 8], zip_bytes[central_offset + 9]]);
            let last_mod_time = u16::from_le_bytes([zip_bytes[central_offset + 12], zip_bytes[central_offset + 13]]);
            let last_mod_date = u16::from_le_bytes([zip_bytes[central_offset + 14], zip_bytes[central_offset + 15]]);
            let crc32 = u32::from_le_bytes([
                zip_bytes[central_offset + 16],
                zip_bytes[central_offset + 17],
                zip_bytes[central_offset + 18],
                zip_bytes[central_offset + 19],
            ]);
            let compressed_size_u32 = u32::from_le_bytes([
                zip_bytes[central_offset + 20],
                zip_bytes[central_offset + 21],
                zip_bytes[central_offset + 22],
                zip_bytes[central_offset + 23],
            ]);
            let uncompressed_size_u32 = u32::from_le_bytes([
                zip_bytes[central_offset + 24],
                zip_bytes[central_offset + 25],
                zip_bytes[central_offset + 26],
                zip_bytes[central_offset + 27],
            ]);
            let file_name_length =
                u16::from_le_bytes([zip_bytes[central_offset + 28], zip_bytes[central_offset + 29]]) as usize;
            let extra_length =
                u16::from_le_bytes([zip_bytes[central_offset + 30], zip_bytes[central_offset + 31]]) as usize;
            let comment_length =
                u16::from_le_bytes([zip_bytes[central_offset + 32], zip_bytes[central_offset + 33]]) as usize;
            let relative_offset_u32 = u32::from_le_bytes([
                zip_bytes[central_offset + 42],
                zip_bytes[central_offset + 43],
                zip_bytes[central_offset + 44],
                zip_bytes[central_offset + 45],
            ]);

            let name_start = central_offset + 46;
            let name_end = name_start + file_name_length;
            let extra_end = name_end + extra_length;
            let record_end = extra_end + comment_length;
            if record_end > zip_bytes.len() {
                return None;
            }

            let file_name_bytes = &zip_bytes[name_start..name_end];
            let name = Self::decode_filename(file_name_bytes, flags)?;
            let is_directory = name.ends_with('/');

            let extra = &zip_bytes[name_end..extra_end];
            let extra_info = zip_extra_field::parse_extra_fields(
                extra,
                true,
                uncompressed_size_u32,
                compressed_size_u32,
                relative_offset_u32,
            );

            let relative_offset = if relative_offset_u32 == 0xFFFF_FFFF {
                extra_info.local_header_offset?
            } else {
                relative_offset_u32 as u64
            };
            let uncompressed_size = if uncompressed_size_u32 == 0xFFFF_FFFF {
                extra_info.uncompressed_size?
            } else {
                uncompressed_size_u32 as u64
            };

            let year = ((last_mod_date >> 9) & 0x7F) as i64 + 1980;
            let month = ((last_mod_date >> 5) & 0x0F) as i64;
            let day = (last_mod_date & 0x1F) as i64;
            let hour = ((last_mod_time >> 11) & 0x1F) as i64;
            let min = ((last_mod_time >> 5) & 0x3F) as i64;
            let sec = ((last_mod_time & 0x1F) as i64) * 2;

            let mut fh = FileHeader::new();
            fh.filename = name;
            fh.uncompressed_size = uncompressed_size;
            fh.is_directory = is_directory;
            fh.crc = Some(crc32.to_be_bytes().to_vec());
            fh.local_head = Some(relative_offset);
            fh.header_last_modified = year * 10000000000_i64
                + month * 100000000_i64
                + day * 1000000_i64
                + hour * 10000_i64
                + min * 100_i64
                + sec;
            fh.modified_time = extra_info.modified_time_ticks;
            fh.accessed_time = extra_info.accessed_time_ticks;
            fh.created_time = extra_info.created_time_ticks;

            out.push(fh);
            central_offset = record_end;
        }

        Some(out)
    }

    fn read_headers(&mut self) -> ZipReturn {
        let archive = match self.archive.as_mut() {
            Some(a) => a,
            None => return ZipReturn::ZipErrorOpeningFile,
        };

        self.file_headers.clear();
        let comment = archive.comment();
        self.file_comment = String::from_utf8_lossy(comment).to_string();

        let parsed = match Self::read_central_directory(&self.zip_filename) {
            Some(v) => v,
            None => return ZipReturn::ZipCentralDirError,
        };
        self.file_headers = parsed;

        ZipReturn::ZipGood
    }
}

impl ICompress for ZipFile {
    fn local_files_count(&self) -> usize {
        self.file_headers.len()
    }

    fn get_file_header(&self, index: usize) -> Option<&FileHeader> {
        self.file_headers.get(index)
    }

    fn zip_open_type(&self) -> ZipOpenType {
        self.zip_open_type
    }

    fn zip_file_open(&mut self, new_filename: &str, timestamp: i64, read_headers: bool) -> ZipReturn {
        self.zip_file_close();
        
        let path = Path::new(new_filename);
        if !path.exists() {
            return ZipReturn::ZipErrorFileNotFound;
        }

        let file = match File::open(path) {
            Ok(f) => f,
            Err(_) => return ZipReturn::ZipErrorOpeningFile,
        };

        let archive = match ZipArchive::new(file) {
            Ok(a) => a,
            Err(_) => return ZipReturn::ZipErrorOpeningFile,
        };

        self.zip_filename = new_filename.to_string();
        self.time_stamp = timestamp;
        self.zip_struct = Self::validate_zip_structure(&self.zip_filename);
        self.archive = Some(archive);
        self.zip_open_type = ZipOpenType::OpenRead;

        if read_headers {
            self.read_headers()
        } else {
            ZipReturn::ZipGood
        }
    }

    fn zip_file_close(&mut self) {
        self.archive = None;
        if let Some(mut w) = self.writer.take() {
            let _ = w.finish();
        }
        self.pending_write = None;
        self.zip_open_type = ZipOpenType::Closed;
        self.zip_struct = ZipStructure::None;
        self.file_headers.clear();
    }

    fn zip_file_open_read_stream(&mut self, index: usize) -> Result<(Box<dyn Read>, u64), ZipReturn> {
        if self.zip_open_type != ZipOpenType::OpenRead {
            return Err(ZipReturn::ZipReadingFromOutputFile);
        }

        let archive = match self.archive.as_mut() {
            Some(a) => a,
            None => return Err(ZipReturn::ZipErrorOpeningFile),
        };

        let file = match archive.by_index(index) {
            Ok(f) => f,
            Err(_) => return Err(ZipReturn::ZipErrorGettingDataStream),
        };

        let size = file.size();
        
        // zip crate's file stream borrows the archive, so returning a boxed reader is tricky in safe rust
        // For a full faithful port, we'd need a self-referential struct or read the whole file to memory
        // Since we are building the abstraction, we'll read to memory for now to satisfy the Box<dyn Read> with 'static lifetime
        let mut buffer = Vec::with_capacity(size as usize);
        let mut f = file;
        if let Err(_) = f.read_to_end(&mut buffer) {
            return Err(ZipReturn::ZipErrorGettingDataStream);
        }

        Ok((Box::new(std::io::Cursor::new(buffer)), size))
    }

    fn zip_file_close_read_stream(&mut self) -> ZipReturn {
        // Nothing to do since we read to memory
        ZipReturn::ZipGood
    }

    fn zip_struct(&self) -> ZipStructure {
        self.zip_struct
    }

    fn zip_filename(&self) -> &str {
        &self.zip_filename
    }

    fn time_stamp(&self) -> i64 {
        self.time_stamp
    }

    fn file_comment(&self) -> &str {
        &self.file_comment
    }

    fn zip_file_create(&mut self, new_filename: &str) -> ZipReturn {
        self.zip_file_close();
        
        let path = Path::new(new_filename);
        let file = match File::create(path) {
            Ok(f) => f,
            Err(_) => return ZipReturn::ZipErrorOpeningFile,
        };

        self.writer = Some(ZipWriter::new(file));
        self.zip_filename = new_filename.to_string();
        self.zip_open_type = ZipOpenType::OpenWrite;
        self.file_headers.clear();
        self.pending_write = None;
        self.zip_struct = ZipStructure::None;

        ZipReturn::ZipGood
    }

    fn zip_file_open_write_stream(
        &mut self,
        _raw: bool,
        _filename: &str,
        uncompressed_size: u64,
        _compression_method: u16,
        _mod_time: Option<i64>,
    ) -> Result<Box<dyn Write>, ZipReturn> {
        if self.zip_open_type != ZipOpenType::OpenWrite {
            return Err(ZipReturn::ZipWritingToInputFile);
        }

        if self.pending_write.is_some() {
            return Err(ZipReturn::ZipErrorOpeningFile);
        }

        if let Some(last) = self.file_headers.last() {
            // Enforce TorrentZip-style order when writing multiple files in a session
            // If the last entry is lexicographically after the new filename (case-insensitive),
            // declare incorrect order.
            if Self::compare_ascii_casefolded_bytes(
                last.filename.as_bytes(),
                _filename.as_bytes(),
            ) >= 0
            {
                return Err(ZipReturn::ZipTrrntzipIncorrectFileOrder);
            }
            // Prevent adding a directory marker that is immediately followed by an entry within that directory
            if last.filename.ends_with('/') {
                let dir_bytes = last.filename.as_bytes();
                let next_bytes = _filename.as_bytes();
                if next_bytes.len() > dir_bytes.len()
                    && Self::compare_ascii_casefolded_bytes(dir_bytes, &next_bytes[..dir_bytes.len()]) == 0
                {
                    return Err(ZipReturn::ZipTrrntzipIncorrectDirectoryAddedToZip);
                }
            }
        }

        let buffer = Rc::new(RefCell::new(Vec::with_capacity(uncompressed_size as usize)));
        self.pending_write = Some(PendingWrite {
            filename: _filename.to_string(),
            compression_method: _compression_method,
            mod_time: _mod_time,
            buffer: Rc::clone(&buffer),
        });

        Ok(Box::new(SharedBufferWriter { buffer }))
    }

    fn zip_file_close_write_stream(&mut self, _crc32: &[u8]) -> ZipReturn {
        let Some(pending_write) = self.pending_write.take() else {
            return ZipReturn::ZipErrorOpeningFile;
        };

        let Some(writer) = self.writer.as_mut() else {
            return ZipReturn::ZipErrorOpeningFile;
        };

        let mut options = FileOptions::default()
            .compression_method(Self::compression_method_from_u16(pending_write.compression_method));

        if let Some(mod_time) = pending_write.mod_time.and_then(Self::zip_datetime_from_i64) {
            options = options.last_modified_time(mod_time);
        }

        if writer.start_file(&pending_write.filename, options).is_err() {
            return ZipReturn::ZipErrorOpeningFile;
        }

        let buffer = pending_write.buffer.borrow();
        if writer.write_all(&buffer).is_err() {
            return ZipReturn::ZipErrorOpeningFile;
        }

        let mut fh = FileHeader::new();
        fh.filename = pending_write.filename;
        fh.uncompressed_size = buffer.len() as u64;
        if !_crc32.is_empty() {
            fh.crc = Some(_crc32.to_vec());
        }
        if let Some(mod_time) = pending_write.mod_time {
            fh.header_last_modified = mod_time;
        }
        self.file_headers.push(fh);

        ZipReturn::ZipGood
    }

    fn zip_file_close_failed(&mut self) {
        self.zip_file_close();
        // Delete file if creation failed
        if !self.zip_filename.is_empty() {
            let _ = std::fs::remove_file(&self.zip_filename);
        }
    }
}
