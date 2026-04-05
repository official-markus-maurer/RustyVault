use std::cell::RefCell;
use std::fs::{self, File};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::Path;
use std::rc::Rc;

use crate::codepage_437;
use crate::deflate_raw_best;
use crate::file_header::FileHeader;
use crate::i_compress::ICompress;
use crate::structured_archive::{
    get_compression_type, get_zip_date_time_type, ZipDateType, ZipStructure,
};
use crate::zip_enums::{ZipOpenType, ZipReturn};
use crate::zip_extra_field;

use bzip2::read::BzDecoder;
use crc32fast::Hasher as Crc32Hasher;
use deflate64::Deflate64Decoder;
use flate2::read::DeflateDecoder;
use std::io::BufReader;
use zip::{DateTime, ZipArchive, ZipWriter};
use zstd::stream::read::Decoder as ZstdDecoder;

mod zip_file_internal;
use zip_file_internal::{
    CentralHeaderMeta, EocdLocator, LocalFileHeaderInfo, LocalHeaderFull, ManualCentralEntry,
    ManualZipWriter, PendingWrite, ReadSeek, SharedBufferWriter, ZipWriterFile,
};

/// [`ICompress`](crate::i_compress::ICompress) wrapper for `.zip` archives.
///
/// `ZipFile` supports both reading and writing ZIPs:
/// - Read mode uses the ecosystem [`zip`] crate to enumerate entries and extract payloads.
/// - Write mode uses a manual writer to support validator-oriented constraints such as
///   deterministic ordering and structure normalization.
///
/// The same type can also operate on in-memory ZIP data via [`ZipFile::zip_file_open_stream`],
/// which is useful for scanners that already have the archive bytes available.
///
/// In write paths, the `zip_struct` field may be set to a non-`None` value to enforce extra
/// invariants (compression method, timestamps, and directory ordering rules).
pub struct ZipFile {
    zip_filename: String,
    zip_open_type: ZipOpenType,
    time_stamp: i64,
    zip_struct: ZipStructure,

    archive: Option<ZipArchive<Box<dyn ReadSeek>>>,
    writer: Option<ZipWriter<ZipWriterFile>>,
    manual_writer: Option<ManualZipWriter>,

    file_headers: Vec<FileHeader>,
    central_meta: Vec<CentralHeaderMeta>,
    file_comment: String,
    pending_write: Option<PendingWrite>,
    fake_write: bool,
    zip_memory: Option<Vec<u8>>,
}

include!("fake_and_rollback.rs");
include!("write_stream.rs");

impl ZipFile {
    const TORRENTZIP_DOS_TIME: u16 = 48128;
    const TORRENTZIP_DOS_DATE: u16 = 8600;

    /// Creates a new `ZipFile` in the `Closed` state.
    ///
    /// This constructor does not open any backing file or allocate archive state beyond a few
    /// empty vectors. Use the [`ICompress`](crate::i_compress::ICompress) methods (or
    /// [`ZipFile::zip_file_open_stream`]) to open an archive for reading/writing.
    pub fn new() -> Self {
        Self {
            zip_filename: String::new(),
            zip_open_type: ZipOpenType::Closed,
            time_stamp: 0,
            zip_struct: ZipStructure::None,
            archive: None,
            writer: None,
            manual_writer: None,
            file_headers: Vec::new(),
            central_meta: Vec::new(),
            file_comment: String::new(),
            pending_write: None,
            fake_write: false,
            zip_memory: None,
        }
    }

    /// Creates a new ZIP file on disk and assigns an expected structure profile.
    ///
    /// This is equivalent to calling `zip_file_create(new_filename)` via the
    /// [`ICompress`](crate::i_compress::ICompress) implementation and, on success, setting
    /// `self.zip_struct` to `zip_struct` so subsequent writes can be validated against it.
    pub fn zip_file_create_with_structure(
        &mut self,
        new_filename: &str,
        zip_struct: ZipStructure,
    ) -> ZipReturn {
        let zr = self.zip_file_create(new_filename);
        if zr == ZipReturn::ZipGood {
            self.zip_struct = zip_struct;
        }
        zr
    }

    fn validate_structured_write(zip_struct: ZipStructure, entries: &[ManualCentralEntry]) -> bool {
        let expected_compression = get_compression_type(zip_struct);
        if expected_compression != 8 && expected_compression != 93 {
            return false;
        }

        for entry in entries {
            if entry.compression_method != expected_compression {
                return false;
            }
            if entry.filename.contains('\\') {
                return false;
            }

            let utf8_flag_set = (entry.flags & (1 << 11)) != 0;
            let is_cp437 = codepage_437::is_code_page_437(&entry.filename);
            if is_cp437 == utf8_flag_set {
                return false;
            }

            match get_zip_date_time_type(zip_struct) {
                ZipDateType::DateTime => {}
                ZipDateType::None => {
                    if entry.dos_time != 0 || entry.dos_date != 0 {
                        return false;
                    }
                }
                ZipDateType::TrrntZip => {
                    if entry.dos_time != Self::TORRENTZIP_DOS_TIME
                        || entry.dos_date != Self::TORRENTZIP_DOS_DATE
                    {
                        return false;
                    }
                }
                ZipDateType::Undefined => return false,
            }
        }

        for i in 0..entries.len().saturating_sub(1) {
            if Self::trrntzip_string_compare(&entries[i].filename, &entries[i + 1].filename) >= 0 {
                return false;
            }
        }

        if matches!(zip_struct, ZipStructure::ZipTrrnt | ZipStructure::ZipZSTD) {
            for i in 0..entries.len().saturating_sub(1) {
                let filename0 = &entries[i].filename;
                if !filename0.ends_with('/') {
                    continue;
                }
                let filename1 = &entries[i + 1].filename;
                if filename1.len() <= filename0.len() {
                    continue;
                }

                if filename1.starts_with(filename0) {
                    return false;
                }
            }
        }

        true
    }

    /// Opens a ZIP archive from a seekable stream by buffering it into memory.
    ///
    /// This method reads the entire stream into a `Vec<u8>`, constructs a [`zip::ZipArchive`]
    /// over an in-memory cursor, and transitions the instance into `OpenRead`.
    ///
    /// When `read_headers` is `true`, file headers are parsed immediately.
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
        let archive = match ZipArchive::new(
            Box::new(std::io::Cursor::new(bytes.clone())) as Box<dyn ReadSeek>
        ) {
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

    /// Computes the CRC32 of the central directory for "clean" ZIPs.
    ///
    /// This is used as a lightweight fingerprint for validator workflows. The CRC is only
    /// returned when:
    /// - The End of Central Directory can be located
    /// - There is no trailing extra data
    /// - No offset correction is needed
    ///
    /// The returned string is an uppercase 8-hex-digit value (e.g. `"1A2B3C4D"`).
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
        if byte.is_ascii_uppercase() {
            byte + 0x20
        } else {
            byte
        }
    }

    fn trrntzip_string_compare(a: &str, b: &str) -> i32 {
        let ab = a.as_bytes();
        let bb = b.as_bytes();
        let len = std::cmp::min(ab.len(), bb.len());
        for i in 0..len {
            let ca = Self::ascii_lower(ab[i]);
            let cb = Self::ascii_lower(bb[i]);
            if ca < cb {
                return -1;
            }
            if ca > cb {
                return 1;
            }
        }
        if ab.len() < bb.len() {
            return -1;
        }
        if ab.len() > bb.len() {
            return 1;
        }
        // Tie-break with ordinal/byte-wise comparison
        for i in 0..len {
            if ab[i] < bb[i] {
                return -1;
            }
            if ab[i] > bb[i] {
                return 1;
            }
        }
        0
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

        let number_of_this_disk =
            u16::from_le_bytes([zip_bytes[eocd_offset + 4], zip_bytes[eocd_offset + 5]]);
        let number_of_this_disk_center_dir =
            u16::from_le_bytes([zip_bytes[eocd_offset + 6], zip_bytes[eocd_offset + 7]]);
        if number_of_this_disk != 0 || number_of_this_disk_center_dir != 0 {
            return None;
        }

        let total_entries_disk =
            u16::from_le_bytes([zip_bytes[eocd_offset + 8], zip_bytes[eocd_offset + 9]]);
        let total_entries =
            u16::from_le_bytes([zip_bytes[eocd_offset + 10], zip_bytes[eocd_offset + 11]]);
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
        let comment_length =
            u16::from_le_bytes([zip_bytes[eocd_offset + 20], zip_bytes[eocd_offset + 21]]) as usize;

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

        let zip64_info = if eocd_offset >= 20 {
            let locator_offset = eocd_offset - 20;
            if locator_offset + 20 <= len
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
                    if zip64_eocd_offset_usize + 56 > len {
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

                    Some((entries_total, cd_size, cd_offset, zip64_eocd_offset))
                })()
            } else {
                None
            }
        } else {
            None
        };

        if zip64_required && zip64_info.is_none() {
            return None;
        }

        let (
            local_files_count,
            central_directory_size,
            central_directory_offset,
            end_of_central_dir_offset,
        ) = if let Some((entries_total, cd_size, cd_offset, zip64_eocd_offset)) = zip64_info {
            (entries_total, cd_size, cd_offset, zip64_eocd_offset)
        } else {
            (
                total_entries as u64,
                central_directory_size_u32 as u64,
                central_directory_offset_u32 as u64,
                eocd_offset as u64,
            )
        };

        let correction = (end_of_central_dir_offset as i128)
            .saturating_sub(central_directory_size as i128)
            .saturating_sub(central_directory_offset as i128);
        if correction < i64::MIN as i128 || correction > i64::MAX as i128 {
            return None;
        }
        let correction_i64 = correction as i64;
        let corrected_cd_offset_i128 =
            (central_directory_offset as i128).saturating_add(correction);
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
        if central_directory_offset + central_directory_size > zip_bytes.len() {
            return ZipStructure::None;
        }

        let mut crc = Crc32Hasher::new();
        crc.update(
            &zip_bytes[central_directory_offset..central_directory_offset + central_directory_size],
        );
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
        if comment[expected_prefix.len()..] != cd_crc {
            return ZipStructure::None;
        }

        if !Self::validate_files_structure(
            &zip_bytes,
            central_directory_offset,
            central_directory_size,
            zip_struct,
        ) {
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
        let saw_directory_entry_needing_check =
            matches!(zip_struct, ZipStructure::ZipTrrnt | ZipStructure::ZipZSTD);

        while central_offset + 46 <= central_end {
            if zip_bytes[central_offset..central_offset + 4] != [0x50, 0x4B, 0x01, 0x02] {
                return false;
            }

            let flags =
                u16::from_le_bytes([zip_bytes[central_offset + 8], zip_bytes[central_offset + 9]]);
            let compression_method = u16::from_le_bytes([
                zip_bytes[central_offset + 10],
                zip_bytes[central_offset + 11],
            ]);
            let last_mod_time = u16::from_le_bytes([
                zip_bytes[central_offset + 12],
                zip_bytes[central_offset + 13],
            ]);
            let last_mod_date = u16::from_le_bytes([
                zip_bytes[central_offset + 14],
                zip_bytes[central_offset + 15],
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

            let extra_end = name_end + extra_length;
            if extra_end > zip_bytes.len() {
                return false;
            }
            let extra_bytes = &zip_bytes[name_end..extra_end];
            let extra_info = zip_extra_field::parse_extra_fields(
                extra_bytes,
                true,
                uncompressed_size_u32,
                compressed_size_u32,
                relative_offset_u32,
            );
            if extra_info.extra_data_found {
                return false;
            }

            let relative_offset_u64 = if relative_offset_u32 == 0xFFFF_FFFF {
                let Some(v) = extra_info.local_header_offset else {
                    return false;
                };
                v
            } else {
                relative_offset_u32 as u64
            };

            if name.contains('\\') {
                return false;
            }

            let utf8_flag_set = (flags & (1 << 11)) != 0;
            let is_cp437 = codepage_437::is_code_page_437(&name);
            if is_cp437 == utf8_flag_set {
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
                    if last_mod_time != Self::TORRENTZIP_DOS_TIME
                        || last_mod_date != Self::TORRENTZIP_DOS_DATE
                    {
                        return false;
                    }
                }
                ZipDateType::Undefined => return false,
            }

            let relative_offset = match usize::try_from(relative_offset_u64) {
                Ok(v) => v,
                Err(_) => return false,
            };
            if relative_offset + 30 > zip_bytes.len() {
                return false;
            }
            if zip_bytes[relative_offset..relative_offset + 4] != [0x50, 0x4B, 0x03, 0x04] {
                return false;
            }

            let local_flags = u16::from_le_bytes([
                zip_bytes[relative_offset + 6],
                zip_bytes[relative_offset + 7],
            ]);
            let local_compression = u16::from_le_bytes([
                zip_bytes[relative_offset + 8],
                zip_bytes[relative_offset + 9],
            ]);
            let local_time = u16::from_le_bytes([
                zip_bytes[relative_offset + 10],
                zip_bytes[relative_offset + 11],
            ]);
            let local_date = u16::from_le_bytes([
                zip_bytes[relative_offset + 12],
                zip_bytes[relative_offset + 13],
            ]);
            let local_name_length = u16::from_le_bytes([
                zip_bytes[relative_offset + 26],
                zip_bytes[relative_offset + 27],
            ]) as usize;
            let local_extra_length = u16::from_le_bytes([
                zip_bytes[relative_offset + 28],
                zip_bytes[relative_offset + 29],
            ]) as usize;

            if local_name_length != file_name_length {
                return false;
            }

            if local_flags != flags
                || local_compression != compression_method
                || local_time != last_mod_time
                || local_date != last_mod_date
            {
                return false;
            }

            let local_name_start = relative_offset + 30;
            let local_name_end = local_name_start + local_name_length;
            if local_name_end > zip_bytes.len() {
                return false;
            }
            let local_extra_end = local_name_end + local_extra_length;
            if local_extra_end > zip_bytes.len() {
                return false;
            }
            let local_extra_bytes = &zip_bytes[local_name_end..local_extra_end];
            let local_compressed_size_u32 = u32::from_le_bytes([
                zip_bytes[relative_offset + 18],
                zip_bytes[relative_offset + 19],
                zip_bytes[relative_offset + 20],
                zip_bytes[relative_offset + 21],
            ]);
            let local_uncompressed_size_u32 = u32::from_le_bytes([
                zip_bytes[relative_offset + 22],
                zip_bytes[relative_offset + 23],
                zip_bytes[relative_offset + 24],
                zip_bytes[relative_offset + 25],
            ]);
            let local_extra_info = zip_extra_field::parse_extra_fields(
                local_extra_bytes,
                false,
                local_uncompressed_size_u32,
                local_compressed_size_u32,
                0,
            );
            if local_extra_info.extra_data_found {
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

                let flags = u16::from_le_bytes([
                    zip_bytes[central_offset + 8],
                    zip_bytes[central_offset + 9],
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
                let name_start = central_offset + 46;
                let name_end = name_start + file_name_length;
                if name_end > zip_bytes.len() {
                    return false;
                }
                let Some(dir_name) = Self::decode_filename(&zip_bytes[name_start..name_end], flags)
                else {
                    return false;
                };

                let next_offset =
                    central_offset + 46 + file_name_length + extra_length + comment_length;
                if next_offset + 46 > central_end {
                    break;
                }
                if zip_bytes[next_offset..next_offset + 4] != [0x50, 0x4B, 0x01, 0x02] {
                    break;
                }
                let next_flags =
                    u16::from_le_bytes([zip_bytes[next_offset + 8], zip_bytes[next_offset + 9]]);
                let next_name_len =
                    u16::from_le_bytes([zip_bytes[next_offset + 28], zip_bytes[next_offset + 29]])
                        as usize;
                let next_name_start = next_offset + 46;
                let next_name_end = next_name_start + next_name_len;
                if next_name_end > zip_bytes.len() {
                    return false;
                }
                let Some(next_name) =
                    Self::decode_filename(&zip_bytes[next_name_start..next_name_end], next_flags)
                else {
                    return false;
                };

                if dir_name.ends_with('/')
                    && next_name.len() > dir_name.len()
                    && next_name.starts_with(&dir_name)
                {
                    return false;
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

            let year = ((dos_date >> 9) & 0x7F).saturating_add(1980);
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
}

include!("read_support.rs");

include!("icompress_impl.rs");
