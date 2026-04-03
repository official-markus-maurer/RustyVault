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
use bzip2::read::BzDecoder;
use zstd::stream::read::Decoder as ZstdDecoder;
use std::io::BufReader;
use deflate64::Deflate64Decoder;

trait ReadSeek: Read + Seek {}
impl<T: Read + Seek> ReadSeek for T {}

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
    
    archive: Option<ZipArchive<Box<dyn ReadSeek>>>,
    writer: Option<ZipWriter<ZipWriterFile>>,
    
    file_headers: Vec<FileHeader>,
    central_meta: Vec<CentralHeaderMeta>,
    file_comment: String,
    pending_write: Option<PendingWrite>,
    fake_write: bool,
    zip_memory: Option<Vec<u8>>,
}

#[derive(Clone)]
struct CentralHeaderMeta {
    #[allow(dead_code)]
    flags: u16,
    compression_method: u16,
    compressed_size: u64,
    uncompressed_size: u64,
    crc32: u32,
    local_header_offset: u64,
    #[allow(dead_code)]
    header_last_modified: i64,
}

struct EocdLocator {
    local_files_count: u64,
    central_directory_size: u64,
    central_directory_offset: u64,
    central_directory_offset_correction: i64,
    comment_bytes: Vec<u8>,
    extra_data_found_on_end: bool,
}

impl ZipFile {
    pub fn break_trrntzip(path: &str) -> std::io::Result<()> {
        let mut file = File::options().read(true).write(true).open(path)?;
        let mut bytes = Vec::new();
        file.read_to_end(&mut bytes)?;

        let len = bytes.len();
        if len < 22 {
            return Ok(());
        }

        let eocd_offset = bytes
            .windows(4)
            .rposition(|window| window == [0x50, 0x4B, 0x05, 0x06]);
        let Some(eocd_offset) = eocd_offset else { return Ok(()) };
        if eocd_offset + 22 > len {
            return Ok(());
        }
        let comment_length =
            u16::from_le_bytes([bytes[eocd_offset + 20], bytes[eocd_offset + 21]]) as usize;
        if eocd_offset + 22 + comment_length != len {
            return Ok(());
        }

        let comment = std::str::from_utf8(&bytes[eocd_offset + 22..len]).unwrap_or("");
        let prefix_len = if comment.starts_with("TORRENTZIPPED-") {
            14
        } else if comment.starts_with("RVZSTD-") {
            7
        } else {
            return Ok(());
        };

        if len >= 8 && comment_length >= prefix_len + 8 {
            let start = len - 8;
            bytes[start..len].copy_from_slice(b"00000000");
            file.seek(SeekFrom::Start(0))?;
            file.write_all(&bytes)?;
            file.flush()?;
        }

        Ok(())
    }

    pub fn zip_create_fake(&mut self) {
        if self.zip_open_type != ZipOpenType::Closed {
            return;
        }
        self.zip_open_type = ZipOpenType::OpenFakeWrite;
        self.fake_write = true;
    }

    pub fn zip_file_fake_open_memory_stream(&mut self) -> ZipReturn {
        if self.zip_open_type != ZipOpenType::OpenFakeWrite {
            return ZipReturn::ZipWritingToInputFile;
        }
        self.writer = Some(ZipWriter::new(ZipWriterFile::Memory(std::io::Cursor::new(Vec::new()))));
        ZipReturn::ZipGood
    }

    pub fn zip_file_fake_close_memory_stream(&mut self) -> Option<Vec<u8>> {
        if !self.fake_write {
            return None;
        }
        let w = self.writer.take()?;
        let cursor = w.finish().ok()?;
        self.zip_open_type = ZipOpenType::Closed;
        self.fake_write = false;
        match cursor {
            ZipWriterFile::Memory(c) => Some(c.into_inner()),
            ZipWriterFile::File(_) => None,
        }
    }

    pub fn zip_file_add_fake(
        &mut self,
        filename: &str,
        file_offset: u64,
        uncompressed_size: u64,
        compressed_size: u64,
        crc32: &[u8],
        compression_method: u16,
        header_last_modified: i64,
    ) -> Result<Vec<u8>, ZipReturn> {
        if self.zip_open_type != ZipOpenType::OpenFakeWrite {
            return Err(ZipReturn::ZipWritingToInputFile);
        }

        let (extra, header_uncompressed_size, header_compressed_size, _header_local_offset) =
            zip_extra_field::write_zip64_extra(uncompressed_size, compressed_size, file_offset, false);

        let filename_bytes = filename.as_bytes();
        if filename_bytes.len() > u16::MAX as usize || extra.len() > u16::MAX as usize {
            return Err(ZipReturn::ZipFileNameToLong);
        }

        let dt = Self::zip_datetime_from_i64(header_last_modified);
        let (dos_time, dos_date) = if let Some(dt) = dt {
            (dt.timepart(), dt.datepart())
        } else {
            (0u16, 0u16)
        };

        let crc_u32 = if crc32.len() == 4 {
            u32::from_be_bytes([crc32[0], crc32[1], crc32[2], crc32[3]])
        } else {
            0u32
        };

        let mut out = Vec::new();
        out.extend_from_slice(&[0x50, 0x4B, 0x03, 0x04]);
        out.extend_from_slice(&20u16.to_le_bytes());
        out.extend_from_slice(&0u16.to_le_bytes());
        out.extend_from_slice(&compression_method.to_le_bytes());
        out.extend_from_slice(&dos_time.to_le_bytes());
        out.extend_from_slice(&dos_date.to_le_bytes());
        out.extend_from_slice(&crc_u32.to_le_bytes());
        out.extend_from_slice(&header_compressed_size.to_le_bytes());
        out.extend_from_slice(&header_uncompressed_size.to_le_bytes());
        out.extend_from_slice(&(filename_bytes.len() as u16).to_le_bytes());
        out.extend_from_slice(&(extra.len() as u16).to_le_bytes());
        out.extend_from_slice(filename_bytes);
        out.extend_from_slice(&extra);

        let mut fh = FileHeader::new();
        fh.filename = filename.to_string();
        fh.local_head = Some(file_offset);
        fh.uncompressed_size = uncompressed_size;
        fh.is_directory = filename.ends_with('/');
        fh.crc = if crc32.len() == 4 { Some(crc32.to_vec()) } else { None };
        fh.header_last_modified = header_last_modified;
        self.file_headers.push(fh);

        Ok(out)
    }

    pub fn zip_file_roll_back(&mut self) -> ZipReturn {
        if self.zip_open_type != ZipOpenType::OpenWrite {
            return ZipReturn::ZipWritingToInputFile;
        }
        if self.pending_write.is_some() {
            self.pending_write = None;
            return ZipReturn::ZipGood;
        }
        ZipReturn::ZipErrorRollBackFile
    }
}

enum ZipWriterFile {
    File(File),
    Memory(std::io::Cursor<Vec<u8>>),
}

impl Write for ZipWriterFile {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        match self {
            ZipWriterFile::File(f) => f.write(buf),
            ZipWriterFile::Memory(c) => c.write(buf),
        }
    }

    fn flush(&mut self) -> std::io::Result<()> {
        match self {
            ZipWriterFile::File(f) => f.flush(),
            ZipWriterFile::Memory(c) => c.flush(),
        }
    }
}

impl Seek for ZipWriterFile {
    fn seek(&mut self, pos: SeekFrom) -> std::io::Result<u64> {
        match self {
            ZipWriterFile::File(f) => f.seek(pos),
            ZipWriterFile::Memory(c) => c.seek(pos),
        }
    }
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

struct LocalHeaderFull {
    #[allow(dead_code)]
    flags: u16,
    compression_method: u16,
    crc32: u32,
    compressed_size: u64,
    uncompressed_size: u64,
    #[allow(dead_code)]
    header_last_modified: i64,
    #[allow(dead_code)]
    filename: String,
    #[allow(dead_code)]
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
            central_meta: Vec::new(),
            file_comment: String::new(),
            pending_write: None,
            fake_write: false,
            zip_memory: None,
        }
    }

    pub fn zip_file_open_stream<R: Read + Seek + 'static>(
        &mut self,
        mut stream: R,
        read_headers: bool,
    ) -> ZipReturn {
        self.zip_file_close();
        let mut bytes = Vec::new();
        if stream.seek(SeekFrom::Start(0)).is_err() {
            return ZipReturn::ZipErrorOpeningFile;
        }
        if stream.read_to_end(&mut bytes).is_err() {
            return ZipReturn::ZipErrorReadingFile;
        }
        let archive = match ZipArchive::new(Box::new(std::io::Cursor::new(bytes.clone())) as Box<dyn ReadSeek>) {
            Ok(a) => a,
            Err(_) => return ZipReturn::ZipErrorOpeningFile,
        };

        self.zip_filename.clear();
        self.zip_memory = Some(bytes);
        self.time_stamp = 0;
        self.zip_struct = ZipStructure::None;
        self.archive = Some(archive);
        self.zip_open_type = ZipOpenType::OpenRead;

        if read_headers {
            self.read_headers()
        } else {
            ZipReturn::ZipGood
        }
    }

    pub fn get_crc(&self) -> Option<String> {
        let bytes = if let Some(b) = self.zip_memory.as_deref() {
            b.to_vec()
        } else if !self.zip_filename.is_empty() {
            fs::read(&self.zip_filename).ok()?
        } else {
            return None;
        };
        let eocd = Self::locate_eocd(&bytes)?;
        if eocd.central_directory_offset_correction != 0 || eocd.extra_data_found_on_end {
            return None;
        }
        let start = eocd.central_directory_offset as usize;
        let size = eocd.central_directory_size as usize;
        if start + size > bytes.len() {
            return None;
        }
        let mut crc = Crc32Hasher::new();
        crc.update(&bytes[start..start + size]);
        Some(format!("{:08X}", crc.finalize()))
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

    fn locate_eocd(zip_bytes: &[u8]) -> Option<EocdLocator> {
        let len = zip_bytes.len();
        if len < 22 {
            return None;
        }

        let max_back = std::cmp::min(len, 0xFFFF + 22);
        let search_start = len - max_back;
        let eocd_offset = zip_bytes[search_start..]
            .windows(4)
            .rposition(|window| window == [0x50, 0x4B, 0x05, 0x06])?
            + search_start;

        if eocd_offset + 22 > len {
            return None;
        }

        let number_of_this_disk = u16::from_le_bytes([zip_bytes[eocd_offset + 4], zip_bytes[eocd_offset + 5]]);
        let number_of_this_disk_center_dir = u16::from_le_bytes([zip_bytes[eocd_offset + 6], zip_bytes[eocd_offset + 7]]);
        if number_of_this_disk != 0 || number_of_this_disk_center_dir != 0 {
            return None;
        }

        let total_entries_disk = u16::from_le_bytes([zip_bytes[eocd_offset + 8], zip_bytes[eocd_offset + 9]]);
        let total_entries = u16::from_le_bytes([zip_bytes[eocd_offset + 10], zip_bytes[eocd_offset + 11]]);
        if total_entries_disk != total_entries {
            return None;
        }

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
        let comment_length = u16::from_le_bytes([zip_bytes[eocd_offset + 20], zip_bytes[eocd_offset + 21]]) as usize;

        let comment_start = eocd_offset + 22;
        let comment_end = comment_start + comment_length;
        if comment_end > len {
            return None;
        }
        let comment_bytes = zip_bytes[comment_start..comment_end].to_vec();
        let extra_data_found_on_end = comment_end != len;

        let zip64_required = central_directory_offset_u32 == 0xFFFF_FFFF
            || central_directory_size_u32 == 0xFFFF_FFFF
            || total_entries == 0xFFFF;

        let (local_files_count, central_directory_size, central_directory_offset, end_of_central_dir_offset) =
            if zip64_required && eocd_offset >= 20 {
                let locator_offset = eocd_offset - 20;
                if locator_offset + 20 > len {
                    return None;
                }
                if zip_bytes[locator_offset..locator_offset + 4] != [0x50, 0x4B, 0x06, 0x07] {
                    return None;
                }
                let disk = u32::from_le_bytes(zip_bytes[locator_offset + 4..locator_offset + 8].try_into().ok()?);
                if disk != 0 {
                    return None;
                }
                let zip64_eocd_offset =
                    u64::from_le_bytes(zip_bytes[locator_offset + 8..locator_offset + 16].try_into().ok()?);
                let total_disks =
                    u32::from_le_bytes(zip_bytes[locator_offset + 16..locator_offset + 20].try_into().ok()?);
                if total_disks > 1 {
                    return None;
                }
                let zip64_eocd_offset_usize = zip64_eocd_offset as usize;
                if zip64_eocd_offset_usize + 56 > len {
                    return None;
                }
                if zip_bytes[zip64_eocd_offset_usize..zip64_eocd_offset_usize + 4]
                    != [0x50, 0x4B, 0x06, 0x06]
                {
                    return None;
                }
                let size_of_record =
                    u64::from_le_bytes(zip_bytes[zip64_eocd_offset_usize + 4..zip64_eocd_offset_usize + 12].try_into().ok()?);
                if size_of_record != 44 {
                    return None;
                }
                let version_needed =
                    u16::from_le_bytes(zip_bytes[zip64_eocd_offset_usize + 14..zip64_eocd_offset_usize + 16].try_into().ok()?);
                if version_needed != 45 {
                    return None;
                }
                let disk_num =
                    u32::from_le_bytes(zip_bytes[zip64_eocd_offset_usize + 16..zip64_eocd_offset_usize + 20].try_into().ok()?);
                let disk_cd =
                    u32::from_le_bytes(zip_bytes[zip64_eocd_offset_usize + 20..zip64_eocd_offset_usize + 24].try_into().ok()?);
                if disk_num != 0 || disk_cd != 0 {
                    return None;
                }
                let entries_on_disk =
                    u64::from_le_bytes(zip_bytes[zip64_eocd_offset_usize + 24..zip64_eocd_offset_usize + 32].try_into().ok()?);
                let entries_total =
                    u64::from_le_bytes(zip_bytes[zip64_eocd_offset_usize + 32..zip64_eocd_offset_usize + 40].try_into().ok()?);
                if entries_on_disk != entries_total {
                    return None;
                }
                let cd_size =
                    u64::from_le_bytes(zip_bytes[zip64_eocd_offset_usize + 40..zip64_eocd_offset_usize + 48].try_into().ok()?);
                let cd_offset =
                    u64::from_le_bytes(zip_bytes[zip64_eocd_offset_usize + 48..zip64_eocd_offset_usize + 56].try_into().ok()?);

                (entries_total, cd_size, cd_offset, zip64_eocd_offset)
            } else {
                (total_entries as u64, central_directory_size_u32 as u64, central_directory_offset_u32 as u64, eocd_offset as u64)
            };

        let correction = (end_of_central_dir_offset as i128)
            .saturating_sub(central_directory_size as i128)
            .saturating_sub(central_directory_offset as i128);
        if correction < i64::MIN as i128 || correction > i64::MAX as i128 {
            return None;
        }
        let correction_i64 = correction as i64;
        let corrected_cd_offset_i128 = (central_directory_offset as i128).saturating_add(correction);
        if corrected_cd_offset_i128 < 0 || corrected_cd_offset_i128 > u64::MAX as i128 {
            return None;
        }
        let corrected_cd_offset = corrected_cd_offset_i128 as u64;

        Some(EocdLocator {
            local_files_count,
            central_directory_size,
            central_directory_offset: corrected_cd_offset,
            central_directory_offset_correction: correction_i64,
            comment_bytes,
            extra_data_found_on_end,
        })
    }

    fn validate_zip_structure(zip_path: &str) -> ZipStructure {
        let Ok(zip_bytes) = fs::read(zip_path) else {
            return ZipStructure::None;
        };

        let Some(eocd) = Self::locate_eocd(&zip_bytes) else {
            return ZipStructure::None;
        };

        if eocd.extra_data_found_on_end || eocd.central_directory_offset_correction != 0 {
            return ZipStructure::None;
        }

        let central_directory_offset = eocd.central_directory_offset as usize;
        let central_directory_size = eocd.central_directory_size as usize;
        if central_directory_offset + central_directory_size > zip_bytes.len() { return ZipStructure::None; }

        let mut crc = Crc32Hasher::new();
        crc.update(&zip_bytes[central_directory_offset..central_directory_offset + central_directory_size]);
        let cd_crc = format!("{:08X}", crc.finalize());

        let comment = codepage_437::decode(&eocd.comment_bytes);

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

        if value <= 0xFFFF_FFFF {
            let dos_date = ((value >> 16) & 0xFFFF) as u16;
            let dos_time = (value & 0xFFFF) as u16;

            let year = (((dos_date >> 9) & 0x7F) as u16).saturating_add(1980);
            let month = ((dos_date >> 5) & 0x0F) as u8;
            let day = (dos_date & 0x1F) as u8;

            let hour = ((dos_time >> 11) & 0x1F) as u8;
            let minute = ((dos_time >> 5) & 0x3F) as u8;
            let second = ((dos_time & 0x1F) as u8) * 2;

            return DateTime::from_date_and_time(year, month, day, hour, minute, second).ok();
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
        let eocd = Self::locate_eocd(&zip_bytes)?;
        let central_directory_size = eocd.central_directory_size as usize;
        let central_directory_offset = eocd.central_directory_offset as usize;
        let correction = eocd.central_directory_offset_correction as i128;
        if central_directory_offset + central_directory_size > zip_bytes.len() { return None; }

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

            let base_offset_u64 = if relative_offset_u32 == 0xFFFF_FFFF {
                extra_info.local_header_offset?
            } else {
                relative_offset_u32 as u64
            };
            let relative_offset_u64_i128 = (base_offset_u64 as i128).saturating_add(correction);
            if relative_offset_u64_i128 < 0 || relative_offset_u64_i128 > u64::MAX as i128 {
                return None;
            }
            let relative_offset_u64 = relative_offset_u64_i128 as u64;

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
            9 => {
                let cursor = std::io::Cursor::new(compressed);
                let mut decoder = Deflate64Decoder::new(BufReader::new(cursor));
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
            12 => {
                let mut decoder = BzDecoder::new(std::io::Cursor::new(compressed));
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
            93 => {
                let decoder = ZstdDecoder::new(std::io::Cursor::new(compressed))
                    .map_err(|_| ZipReturn::ZipErrorGettingDataStream)?;
                let mut decoder = decoder;
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

    fn read_central_directory_from_bytes(zip_bytes: &[u8]) -> Option<(Vec<FileHeader>, Vec<CentralHeaderMeta>)> {
        let eocd = Self::locate_eocd(zip_bytes)?;
        let central_directory_size = eocd.central_directory_size as usize;
        let central_directory_offset = eocd.central_directory_offset as usize;
        let correction = eocd.central_directory_offset_correction as i128;
        if central_directory_offset + central_directory_size > zip_bytes.len() {
            return None;
        }

        let mut out = Vec::new();
        let mut meta = Vec::new();
        if eocd.local_files_count <= (usize::MAX as u64) {
            out.reserve(eocd.local_files_count as usize);
            meta.reserve(eocd.local_files_count as usize);
        }
        let mut central_offset = central_directory_offset;
        let central_end = central_directory_offset + central_directory_size;
        while central_offset + 46 <= central_end {
            if zip_bytes[central_offset..central_offset + 4] != [0x50, 0x4B, 0x01, 0x02] {
                return None;
            }

            let flags = u16::from_le_bytes([zip_bytes[central_offset + 8], zip_bytes[central_offset + 9]]);
            let compression_method = u16::from_le_bytes([zip_bytes[central_offset + 10], zip_bytes[central_offset + 11]]);
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

            let base_relative_offset = if relative_offset_u32 == 0xFFFF_FFFF {
                extra_info.local_header_offset?
            } else {
                relative_offset_u32 as u64
            };
            let relative_offset_i128 = (base_relative_offset as i128).saturating_add(correction);
            if relative_offset_i128 < 0 || relative_offset_i128 > u64::MAX as i128 { return None; }
            let relative_offset = relative_offset_i128 as u64;
            let compressed_size = if compressed_size_u32 == 0xFFFF_FFFF {
                extra_info.compressed_size?
            } else {
                compressed_size_u32 as u64
            };
            let uncompressed_size = if uncompressed_size_u32 == 0xFFFF_FFFF {
                extra_info.uncompressed_size?
            } else {
                uncompressed_size_u32 as u64
            };

            let header_last_modified = ((last_mod_date as i64) << 16) | (last_mod_time as i64);

            let mut fh = FileHeader::new();
            fh.filename = name;
            fh.uncompressed_size = uncompressed_size;
            fh.is_directory = is_directory;
            fh.crc = Some(crc32.to_be_bytes().to_vec());
            fh.local_head = if (flags & 8) == 0 { Some(relative_offset) } else { None };
            fh.header_last_modified = header_last_modified;
            fh.modified_time = extra_info.modified_time_ticks;
            fh.accessed_time = extra_info.accessed_time_ticks;
            fh.created_time = extra_info.created_time_ticks;

            out.push(fh);
            meta.push(CentralHeaderMeta {
                flags,
                compression_method,
                compressed_size,
                uncompressed_size,
                crc32,
                local_header_offset: relative_offset,
                header_last_modified,
            });
            central_offset = record_end;
        }

        Some((out, meta))
    }

    fn read_central_directory(zip_path: &str) -> Option<(Vec<FileHeader>, Vec<CentralHeaderMeta>)> {
        let zip_bytes = fs::read(zip_path).ok()?;
        Self::read_central_directory_from_bytes(&zip_bytes)
    }

    fn read_local_file_header_full_from_bytes(
        zip_bytes: &[u8],
        local_offset: u64,
        central: &CentralHeaderMeta,
    ) -> Result<LocalHeaderFull, ZipReturn> {
        let local_offset_usize = local_offset as usize;
        if local_offset_usize + 30 > zip_bytes.len() {
            return Err(ZipReturn::ZipLocalFileHeaderError);
        }
        if zip_bytes[local_offset_usize..local_offset_usize + 4] != [0x50, 0x4B, 0x03, 0x04] {
            return Err(ZipReturn::ZipLocalFileHeaderError);
        }

        let flags = u16::from_le_bytes([zip_bytes[local_offset_usize + 6], zip_bytes[local_offset_usize + 7]]);
        let compression_method =
            u16::from_le_bytes([zip_bytes[local_offset_usize + 8], zip_bytes[local_offset_usize + 9]]);
        let last_mod_time =
            u16::from_le_bytes([zip_bytes[local_offset_usize + 10], zip_bytes[local_offset_usize + 11]]);
        let last_mod_date =
            u16::from_le_bytes([zip_bytes[local_offset_usize + 12], zip_bytes[local_offset_usize + 13]]);
        let header_last_modified = ((last_mod_date as i64) << 16) | (last_mod_time as i64);

        let crc32_local = u32::from_le_bytes([
            zip_bytes[local_offset_usize + 14],
            zip_bytes[local_offset_usize + 15],
            zip_bytes[local_offset_usize + 16],
            zip_bytes[local_offset_usize + 17],
        ]);
        let compressed_size_u32 = u32::from_le_bytes([
            zip_bytes[local_offset_usize + 18],
            zip_bytes[local_offset_usize + 19],
            zip_bytes[local_offset_usize + 20],
            zip_bytes[local_offset_usize + 21],
        ]);
        let uncompressed_size_u32 = u32::from_le_bytes([
            zip_bytes[local_offset_usize + 22],
            zip_bytes[local_offset_usize + 23],
            zip_bytes[local_offset_usize + 24],
            zip_bytes[local_offset_usize + 25],
        ]);
        let file_name_length = u16::from_le_bytes([
            zip_bytes[local_offset_usize + 26],
            zip_bytes[local_offset_usize + 27],
        ]) as usize;
        let extra_length = u16::from_le_bytes([
            zip_bytes[local_offset_usize + 28],
            zip_bytes[local_offset_usize + 29],
        ]) as usize;

        let name_start = local_offset_usize + 30;
        let name_end = name_start + file_name_length;
        let extra_end = name_end + extra_length;
        if extra_end > zip_bytes.len() {
            return Err(ZipReturn::ZipLocalFileHeaderError);
        }

        let file_name_bytes = &zip_bytes[name_start..name_end];
        let filename = Self::decode_filename(file_name_bytes, flags).ok_or(ZipReturn::ZipLocalFileHeaderError)?;

        let extra_bytes = &zip_bytes[name_end..extra_end];
        let extra_info = zip_extra_field::parse_extra_fields(
            extra_bytes,
            false,
            uncompressed_size_u32,
            compressed_size_u32,
            0,
        );

        let mut compressed_size = if compressed_size_u32 == 0xFFFF_FFFF {
            extra_info.compressed_size.ok_or(ZipReturn::ZipLocalFileHeaderError)?
        } else {
            compressed_size_u32 as u64
        };
        let mut uncompressed_size = if uncompressed_size_u32 == 0xFFFF_FFFF {
            extra_info.uncompressed_size.ok_or(ZipReturn::ZipLocalFileHeaderError)?
        } else {
            uncompressed_size_u32 as u64
        };

        let mut crc32 = crc32_local;
        if (flags & 8) == 8 {
            crc32 = central.crc32;
            compressed_size = central.compressed_size;
            uncompressed_size = central.uncompressed_size;
        }

        let data_offset = extra_end as u64;

        Ok(LocalHeaderFull {
            flags,
            compression_method,
            crc32,
            compressed_size,
            uncompressed_size,
            header_last_modified,
            filename,
            data_offset,
        })
    }

    fn read_headers(&mut self) -> ZipReturn {
        let archive = match self.archive.as_mut() {
            Some(a) => a,
            None => return ZipReturn::ZipErrorOpeningFile,
        };

        self.file_headers.clear();
        self.central_meta.clear();

        let zip_bytes = self
            .zip_memory
            .clone()
            .or_else(|| fs::read(&self.zip_filename).ok());
        self.file_comment = zip_bytes
            .as_ref()
            .and_then(|b| Self::locate_eocd(b))
            .map(|e| codepage_437::decode(&e.comment_bytes))
            .unwrap_or_else(|| String::from_utf8_lossy(archive.comment()).to_string());

        let parsed = match zip_bytes
            .as_deref()
            .and_then(Self::read_central_directory_from_bytes)
            .or_else(|| {
                if self.zip_filename.is_empty() {
                    None
                } else {
                    Self::read_central_directory(&self.zip_filename)
                }
            })
        {
            Some(v) => v,
            None => return ZipReturn::ZipCentralDirError,
        };
        self.file_headers = parsed.0;
        self.central_meta = parsed.1;

        let Some(zip_bytes) = zip_bytes.as_deref() else {
            return ZipReturn::ZipErrorReadingFile;
        };

        if self.file_headers.len() != self.central_meta.len() {
            return ZipReturn::ZipCentralDirError;
        }

        for central in &self.central_meta {
            let local = match Self::read_local_file_header_full_from_bytes(
                zip_bytes,
                central.local_header_offset,
                central,
            ) {
                Ok(v) => v,
                Err(z) => return z,
            };

            if central.compression_method != local.compression_method {
                return ZipReturn::ZipLocalFileHeaderError;
            }

            if !matches!(
                central.compression_method,
                0 | 1 | 2 | 3 | 4 | 5 | 6 | 8 | 9 | 12 | 14 | 20 | 93 | 98
            ) {
                return ZipReturn::ZipUnsupportedCompression;
            }

            if central.crc32 != local.crc32 {
                return ZipReturn::ZipLocalFileHeaderError;
            }
            if central.compressed_size != local.compressed_size {
                return ZipReturn::ZipLocalFileHeaderError;
            }
            if central.uncompressed_size != local.uncompressed_size {
                return ZipReturn::ZipLocalFileHeaderError;
            }
        }

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

        let file_secs = match fs::metadata(path)
            .ok()
            .and_then(|m| m.modified().ok())
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_secs() as i64)
        {
            Some(v) => v,
            None => 0,
        };

        if timestamp > 0 && file_secs != timestamp {
            return ZipReturn::ZipErrorTimeStamp;
        }

        let file = match File::open(path) {
            Ok(f) => f,
            Err(e) => {
                let code = e.raw_os_error().unwrap_or(0);
                if code == 32 || code == 33 {
                    return ZipReturn::ZipFileLocked;
                }
                return ZipReturn::ZipErrorOpeningFile;
            }
        };

        let archive = match ZipArchive::new(Box::new(file) as Box<dyn ReadSeek>) {
            Ok(a) => a,
            Err(_) => return ZipReturn::ZipErrorOpeningFile,
        };

        self.zip_filename = new_filename.to_string();
        self.zip_memory = None;
        self.time_stamp = file_secs;
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
        if let Some(w) = self.writer.take() {
            let _ = w.finish();
        }
        self.pending_write = None;
        self.zip_open_type = ZipOpenType::Closed;
        self.zip_struct = ZipStructure::None;
        self.file_headers.clear();
        self.central_meta.clear();
        self.file_comment.clear();
        self.fake_write = false;
        self.zip_memory = None;
    }

    fn zip_file_open_read_stream(&mut self, index: usize) -> Result<(Box<dyn Read>, u64), ZipReturn> {
        if self.zip_open_type != ZipOpenType::OpenRead {
            return Err(ZipReturn::ZipReadingFromOutputFile);
        }

        let local_head_fallback = self.get_file_header(index).and_then(|h| h.local_head);

        let mut buffer = Vec::new();
        let (read_ok, size) = {
            let archive = match self.archive.as_mut() {
                Some(a) => a,
                None => return Err(ZipReturn::ZipErrorOpeningFile),
            };

            let file = match archive.by_index(index) {
                Ok(f) => f,
                Err(_) => return Err(ZipReturn::ZipErrorGettingDataStream),
            };

            let size = file.size();
            buffer.reserve(size as usize);

            let mut f = file;
            let ok = f.read_to_end(&mut buffer).is_ok();
            (ok, size)
        };

        if read_ok {
            return Ok((Box::new(std::io::Cursor::new(buffer)), size));
        }

        let local_head = local_head_fallback.ok_or(ZipReturn::ZipErrorGettingDataStream)?;
        let (mut stream, out_size, _) = self.zip_file_open_read_stream_from_local_header_pointer(local_head, false)?;
        buffer.clear();
        buffer.reserve(out_size as usize);
        stream
            .read_to_end(&mut buffer)
            .map_err(|_| ZipReturn::ZipErrorGettingDataStream)?;
        Ok((Box::new(std::io::Cursor::new(buffer)), out_size))
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
        if self.zip_open_type != ZipOpenType::Closed {
            return ZipReturn::ZipFileAlreadyOpen;
        }
        
        let path = Path::new(new_filename);
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() && fs::create_dir_all(parent).is_err() {
                return ZipReturn::ZipErrorOpeningFile;
            }
        }
        let file = match File::create(path) {
            Ok(f) => f,
            Err(_) => return ZipReturn::ZipErrorOpeningFile,
        };

        self.writer = Some(ZipWriter::new(ZipWriterFile::File(file)));
        self.zip_filename = new_filename.to_string();
        self.zip_open_type = ZipOpenType::OpenWrite;
        self.file_headers.clear();
        self.central_meta.clear();
        self.file_comment.clear();
        self.pending_write = None;
        self.zip_struct = ZipStructure::None;

        ZipReturn::ZipGood
    }

    fn zip_file_open_write_stream(
        &mut self,
        raw: bool,
        filename: &str,
        uncompressed_size: u64,
        compression_method: u16,
        mod_time: Option<i64>,
    ) -> Result<Box<dyn Write>, ZipReturn> {
        if self.zip_open_type != ZipOpenType::OpenWrite {
            return Err(ZipReturn::ZipWritingToInputFile);
        }

        if raw {
            return Err(ZipReturn::ZipTrrntZipIncorrectDataStream);
        }

        if !matches!(compression_method, 0 | 8 | 93) {
            return Err(ZipReturn::ZipUnsupportedCompression);
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
                filename.as_bytes(),
            ) >= 0
            {
                return Err(ZipReturn::ZipTrrntzipIncorrectFileOrder);
            }
            // Prevent adding a directory marker that is immediately followed by an entry within that directory
            if last.filename.ends_with('/') {
                let dir_bytes = last.filename.as_bytes();
                let next_bytes = filename.as_bytes();
                if next_bytes.len() > dir_bytes.len()
                    && Self::compare_ascii_casefolded_bytes(dir_bytes, &next_bytes[..dir_bytes.len()]) == 0
                {
                    return Err(ZipReturn::ZipTrrntzipIncorrectDirectoryAddedToZip);
                }
            }
        }

        let buffer = Rc::new(RefCell::new(Vec::with_capacity(uncompressed_size as usize)));
        self.pending_write = Some(PendingWrite {
            filename: filename.to_string(),
            compression_method,
            mod_time,
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

        let mut options = FileOptions::<()>::default()
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
        fh.is_directory = fh.filename.ends_with('/');
        if fh.is_directory && fh.uncompressed_size != 0 {
            return ZipReturn::ZipErrorWritingToOutputStream;
        }
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
