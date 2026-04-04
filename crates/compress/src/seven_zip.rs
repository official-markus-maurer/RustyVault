use std::cell::RefCell;
use std::collections::HashMap;
use std::fs;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::rc::Rc;

use crc32fast::Hasher as Crc32Hasher;

use crate::file_header::FileHeader;
use crate::i_compress::ICompress;
use crate::structured_archive::ZipStructure;
use crate::zip_enums::{ZipOpenType, ZipReturn};

use sevenz_rust::encoder_options::{EncoderOptions, LzmaOptions, ZstandardOptions};
use sevenz_rust::{
    Archive, ArchiveEntry, ArchiveWriter, EncoderConfiguration, EncoderMethod, Password,
    SourceReader,
};

mod internals;
pub use internals::{extract_entry_bytes, seven_zip_dictionary_size_from_uncompressed_size};
use internals::{SevenZipPendingWrite, SharedFileWriter};

/// ICompress wrapper for `.7z` archives.
///
/// `SevenZipFile` implements the `ICompress` trait for 7z files, allowing the scanner to
/// open, read headers, and extract payloads from 7-Zip archives.
///
/// Differences from C#:
/// - The C# `Compress.SevenZip` library is a massively complex custom LZMA decoder built
///   specifically to handle solid-block streaming and chunked hashing without extracting
///   the entire solid block to disk.
/// - The Rust version utilizes the `sevenz-rust` crate. It successfully reads and extracts
///   files, but currently lacks the granular solid-block stream-hashing optimizations present
///   in the custom C# engine, meaning it may use more memory when extracting very large solid 7z files.
pub struct SevenZipFile {
    zip_filename: String,
    zip_open_type: ZipOpenType,
    time_stamp: i64,

    // In read mode, we hold the loaded archive
    archive: Option<Archive>,
    file: Option<File>,
    staging_dir: Option<PathBuf>,
    pending_write: Option<SevenZipPendingWrite>,
    temp_open_path: Option<PathBuf>,

    file_headers: Vec<FileHeader>,
    file_comment: String,
    zip_struct: ZipStructure,
}

impl SevenZipFile {
    pub fn new() -> Self {
        Self {
            zip_filename: String::new(),
            zip_open_type: ZipOpenType::Closed,
            time_stamp: 0,
            archive: None,
            file: None,
            staging_dir: None,
            pending_write: None,
            temp_open_path: None,
            file_headers: Vec::new(),
            file_comment: String::new(),
            zip_struct: ZipStructure::None,
        }
    }

    pub fn zip_file_open_stream<R: Read + Seek>(
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

        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .ok()
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let tmp_path = std::env::temp_dir().join(format!("rv_7z_stream_{}.7z", unique));
        if fs::write(&tmp_path, bytes).is_err() {
            return ZipReturn::ZipErrorOpeningFile;
        }

        self.temp_open_path = Some(tmp_path.clone());
        self.zip_file_open(tmp_path.to_string_lossy().as_ref(), 0, read_headers)
    }

    pub fn header_report(&self) -> String {
        String::new()
    }

    pub fn zip_file_create_with_structure(
        &mut self,
        new_filename: &str,
        zip_struct: ZipStructure,
    ) -> ZipReturn {
        if self.zip_open_type != ZipOpenType::Closed {
            return ZipReturn::ZipFileAlreadyOpen;
        }

        let path = Path::new(new_filename);
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() && fs::create_dir_all(parent).is_err() {
                return ZipReturn::ZipErrorOpeningFile;
            }
        }

        let staging_dir = PathBuf::from(format!("{}.rv7z.dir", new_filename));
        let _ = fs::remove_dir_all(&staging_dir);
        if fs::create_dir_all(&staging_dir).is_err() {
            return ZipReturn::ZipErrorOpeningFile;
        }

        self.zip_filename = new_filename.to_string();
        self.zip_open_type = ZipOpenType::OpenWrite;
        self.zip_struct = zip_struct;
        self.file_headers.clear();
        self.file_comment.clear();
        self.archive = None;
        self.file = None;
        self.staging_dir = Some(staging_dir);
        self.pending_write = None;

        ZipReturn::ZipGood
    }

    fn expected_compression_for_struct(zip_struct: ZipStructure) -> u16 {
        match zip_struct {
            ZipStructure::SevenZipSZSTD | ZipStructure::SevenZipNZSTD => 93,
            _ => 14,
        }
    }

    fn verify_next_header_crc(file: &mut File) -> ZipReturn {
        let mut sig = [0u8; 32];
        if file.seek(SeekFrom::Start(0)).is_err() {
            return ZipReturn::ZipErrorReadingFile;
        }
        if file.read_exact(&mut sig).is_err() {
            return ZipReturn::ZipErrorReadingFile;
        }
        if sig[0..6] != [0x37, 0x7A, 0xBC, 0xAF, 0x27, 0x1C] {
            return ZipReturn::ZipSignatureError;
        }

        let next_header_offset = u64::from_le_bytes(sig[12..20].try_into().unwrap());
        let next_header_size = u64::from_le_bytes(sig[20..28].try_into().unwrap());
        let next_header_crc = u32::from_le_bytes(sig[28..32].try_into().unwrap());

        let header_pos = 32u64.saturating_add(next_header_offset);
        let Ok(file_len) = file.metadata().map(|m| m.len()) else {
            return ZipReturn::ZipErrorReadingFile;
        };
        if header_pos > file_len {
            return ZipReturn::ZipErrorReadingFile;
        }
        if next_header_size > (file_len - header_pos) {
            return ZipReturn::ZipErrorReadingFile;
        }

        if next_header_size == 0 {
            return ZipReturn::ZipGood;
        }

        if file.seek(SeekFrom::Start(header_pos)).is_err() {
            return ZipReturn::ZipErrorReadingFile;
        }
        let mut header_bytes = vec![0u8; next_header_size as usize];
        if file.read_exact(&mut header_bytes).is_err() {
            return ZipReturn::ZipErrorReadingFile;
        }
        let mut hasher = Crc32Hasher::new();
        hasher.update(&header_bytes);
        if hasher.finalize() != next_header_crc {
            return ZipReturn::Zip64EndOfCentralDirectoryError;
        }
        ZipReturn::ZipGood
    }

    fn finalize_write(&mut self) -> ZipReturn {
        let Some(staging_dir) = self.staging_dir.as_ref() else {
            return ZipReturn::ZipErrorOpeningFile;
        };
        if self.zip_filename.is_empty() {
            return ZipReturn::ZipErrorOpeningFile;
        }

        let temp_path = format!("{}.rv7z.tmp", self.zip_filename);
        let _ = fs::remove_file(&temp_path);
        let mut planned: Vec<(String, bool, PathBuf)> = Vec::new();
        for fh in &self.file_headers {
            let mut name = fh.filename.replace('\\', "/");
            if name.is_empty() || name == "/" {
                continue;
            }
            if fh.is_directory && !name.ends_with('/') {
                name.push('/');
            }
            let disk_path = if fh.is_directory {
                staging_dir.join(name.trim_end_matches('/'))
            } else {
                staging_dir.join(&name)
            };
            planned.push((name, fh.is_directory, disk_path));
        }

        let mut dir_has_children: HashMap<String, bool> = HashMap::new();
        for (name, is_dir, _) in &planned {
            if *is_dir {
                continue;
            }
            if let Some(idx) = name.rfind('/') {
                dir_has_children.insert(format!("{}/", &name[..idx]), true);
            }
        }
        planned.retain(|(name, is_dir, _)| {
            if !*is_dir {
                return true;
            }
            !dir_has_children.get(name).copied().unwrap_or(false)
        });

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
        planned.sort_by(|(a, _, _), (b, _, _)| {
            let (path_a, name_a, ext_a) = split_7zip_filename(a);
            let (path_b, name_b, ext_b) = split_7zip_filename(b);
            let res = ext_a.cmp(ext_b);
            if res != std::cmp::Ordering::Equal {
                return res;
            }
            let res = name_a.cmp(name_b);
            if res != std::cmp::Ordering::Equal {
                return res;
            }
            path_a.cmp(path_b)
        });
        for i in 0..planned.len().saturating_sub(1) {
            if planned[i].0 == planned[i + 1].0 {
                let _ = fs::remove_file(&temp_path);
                return ZipReturn::ZipErrorWritingToOutputStream;
            }
        }

        let out_file = match File::create(&temp_path) {
            Ok(f) => f,
            Err(_) => {
                let _ = fs::remove_file(&temp_path);
                return ZipReturn::ZipErrorWritingToOutputStream;
            }
        };
        let mut writer = match ArchiveWriter::new(out_file) {
            Ok(w) => w,
            Err(_) => {
                let _ = fs::remove_file(&temp_path);
                return ZipReturn::ZipErrorWritingToOutputStream;
            }
        };
        writer.set_encrypt_header(false);

        let solid = matches!(
            self.zip_struct,
            ZipStructure::SevenZipSLZMA | ZipStructure::SevenZipSZSTD
        );
        if solid {
            let config = match self.zip_struct {
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

            for (name, _is_dir, _) in planned.iter().filter(|(_, is_dir, _)| *is_dir) {
                if writer
                    .push_archive_entry::<&[u8]>(ArchiveEntry::new_directory(name), None)
                    .is_err()
                {
                    let _ = fs::remove_file(&temp_path);
                    return ZipReturn::ZipErrorWritingToOutputStream;
                }
            }

            let mut file_entries = Vec::new();
            let mut readers: Vec<SourceReader<File>> = Vec::new();
            for (name, _is_dir, disk_path) in planned.iter().filter(|(_, is_dir, _)| !*is_dir) {
                let Ok(src) = File::open(disk_path) else {
                    let _ = fs::remove_file(&temp_path);
                    return ZipReturn::ZipErrorWritingToOutputStream;
                };
                file_entries.push(ArchiveEntry::new_file(name));
                readers.push(SourceReader::new(src));
            }
            if !file_entries.is_empty()
                && writer.push_archive_entries(file_entries, readers).is_err()
            {
                let _ = fs::remove_file(&temp_path);
                return ZipReturn::ZipErrorWritingToOutputStream;
            }
        } else {
            for (name, is_dir, disk_path) in &planned {
                if *is_dir {
                    if writer
                        .push_archive_entry::<&[u8]>(ArchiveEntry::new_directory(name), None)
                        .is_err()
                    {
                        let _ = fs::remove_file(&temp_path);
                        return ZipReturn::ZipErrorWritingToOutputStream;
                    }
                    continue;
                }

                let Ok(src) = File::open(disk_path) else {
                    let _ = fs::remove_file(&temp_path);
                    return ZipReturn::ZipErrorWritingToOutputStream;
                };
                let config = match self.zip_struct {
                    ZipStructure::SevenZipSZSTD | ZipStructure::SevenZipNZSTD => {
                        EncoderConfiguration::new(EncoderMethod::ZSTD)
                            .with_options(EncoderOptions::Zstd(ZstandardOptions::from_level(19)))
                    }
                    _ => {
                        let mut lz = LzmaOptions::from_level(9);
                        lz.set_dictionary_size(seven_zip_dictionary_size_from_uncompressed_size(
                            src.metadata().map(|m| m.len()).unwrap_or(0),
                        ));
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
                    .push_archive_entry(ArchiveEntry::new_file(name), Some(src))
                    .is_err()
                {
                    let _ = fs::remove_file(&temp_path);
                    return ZipReturn::ZipErrorWritingToOutputStream;
                }
            }
        }

        if writer.finish().is_err() {
            let _ = fs::remove_file(&temp_path);
            return ZipReturn::ZipErrorWritingToOutputStream;
        }

        let _ = apply_romvault7z_marker(Path::new(&temp_path), self.zip_struct);

        let _ = fs::remove_file(&self.zip_filename);
        if fs::rename(&temp_path, &self.zip_filename).is_err() {
            if fs::copy(&temp_path, &self.zip_filename).is_err() {
                let _ = fs::remove_file(&temp_path);
                return ZipReturn::ZipErrorWritingToOutputStream;
            }
            let _ = fs::remove_file(&temp_path);
        }

        ZipReturn::ZipGood
    }

    fn read_headers(&mut self) -> ZipReturn {
        let archive = match self.archive.as_ref() {
            Some(a) => a,
            None => return ZipReturn::ZipErrorOpeningFile,
        };

        self.file_headers.clear();

        for file in &archive.files {
            let mut fh = FileHeader::new();
            let mut name = file.name().to_string();
            if file.is_directory() && !name.ends_with('/') {
                name.push('/');
            }
            fh.filename = name;
            fh.uncompressed_size = file.size();
            fh.is_directory = file.is_directory();

            if fh.is_directory {
                fh.crc = Some(vec![0, 0, 0, 0]);
            } else if file.has_crc {
                fh.crc = Some((file.crc as u32).to_be_bytes().to_vec());
            }

            let set_time = |nt: sevenz_rust::NtTime| -> Option<i64> {
                let st: std::time::SystemTime = nt.into();
                st.duration_since(std::time::UNIX_EPOCH)
                    .ok()
                    .map(|d| d.as_secs() as i64)
            };

            if file.has_last_modified_date {
                fh.modified_time = set_time(file.last_modified_date());
            }
            if file.has_creation_date {
                fh.created_time = set_time(file.creation_date());
            }
            if file.has_access_date {
                fh.accessed_time = set_time(file.access_date());
            }

            self.file_headers.push(fh);
        }

        ZipReturn::ZipGood
    }

    fn detect_zip_structure(&self) -> ZipStructure {
        let Ok(mut file) = File::open(&self.zip_filename) else {
            return ZipStructure::None;
        };
        let Ok(metadata) = file.metadata() else {
            return ZipStructure::None;
        };
        let len = metadata.len();
        if len < 6 {
            return ZipStructure::None;
        }

        let mut signature = [0u8; 6];
        if file.read_exact(&mut signature).is_err() {
            return ZipStructure::None;
        }
        if signature != [0x37, 0x7A, 0xBC, 0xAF, 0x27, 0x1C] {
            return ZipStructure::None;
        }

        let rv = self.detect_romvault7z(&mut file, len);
        if rv != ZipStructure::None {
            return rv;
        }

        self.detect_torrent7z(&mut file, len)
    }

    fn detect_romvault7z(&self, file: &mut File, len: u64) -> ZipStructure {
        if len < 32 {
            return ZipStructure::None;
        }
        if file.seek(std::io::SeekFrom::Start(0)).is_err() {
            return ZipStructure::None;
        }
        let mut header = [0u8; 32];
        if file.read_exact(&mut header).is_err() {
            return ZipStructure::None;
        }
        if header[0..6] != [0x37, 0x7A, 0xBC, 0xAF, 0x27, 0x1C] {
            return ZipStructure::None;
        }

        let next_header_offset = u64::from_le_bytes(header[12..20].try_into().unwrap());
        let next_header_size = u64::from_le_bytes(header[20..28].try_into().unwrap());
        let next_header_crc = u32::from_le_bytes(header[28..32].try_into().unwrap());
        let header_pos = 32u64.saturating_add(next_header_offset);
        if header_pos < 32 || header_pos > len {
            return ZipStructure::None;
        }
        if header_pos < 32 {
            return ZipStructure::None;
        }
        let rv_pos = header_pos.saturating_sub(32);
        if file.seek(std::io::SeekFrom::Start(rv_pos)).is_err() {
            return ZipStructure::None;
        }
        let mut rv_hdr = [0u8; 32];
        if file.read_exact(&mut rv_hdr).is_err() {
            return ZipStructure::None;
        }

        let prefix = b"RomVault7Z0";
        if rv_hdr.len() < 12 {
            return ZipStructure::None;
        }
        if &rv_hdr[..11] != prefix {
            return ZipStructure::None;
        }

        let stored_crc = u32::from_le_bytes(rv_hdr[12..16].try_into().unwrap());
        let stored_header_offset = u64::from_le_bytes(rv_hdr[16..24].try_into().unwrap());
        let stored_header_size = u64::from_le_bytes(rv_hdr[24..32].try_into().unwrap());

        if stored_crc != next_header_crc
            || stored_header_offset != header_pos
            || stored_header_size != next_header_size
        {
            return ZipStructure::None;
        }

        match rv_hdr[11] {
            b'1' => ZipStructure::SevenZipSLZMA,
            b'2' => ZipStructure::SevenZipNLZMA,
            b'3' => ZipStructure::SevenZipSZSTD,
            b'4' => ZipStructure::SevenZipNZSTD,
            _ => ZipStructure::None,
        }
    }

    fn detect_torrent7z(&self, file: &mut File, len: u64) -> ZipStructure {
        const CRC_SZ: usize = 128;
        const T7Z_SIG_SIZE: usize = 34;
        const T7Z_FOOTER_SIZE: usize = T7Z_SIG_SIZE + 4;
        const BUFFER_SIZE: usize = 256 + 8 + T7Z_FOOTER_SIZE;

        if len < (T7Z_FOOTER_SIZE as u64) {
            return ZipStructure::None;
        }

        let mut buffer = vec![0u8; BUFFER_SIZE];

        if file.seek(std::io::SeekFrom::Start(0)).is_err() {
            return ZipStructure::None;
        }
        let mut first = vec![0u8; CRC_SZ];
        let read_first = file.read(&mut first).unwrap_or(0);
        buffer[..read_first.min(CRC_SZ)].copy_from_slice(&first[..read_first.min(CRC_SZ)]);

        let footer_offset = len.saturating_sub(T7Z_FOOTER_SIZE as u64);
        let start_last = footer_offset.saturating_sub(CRC_SZ as u64);
        let last_len = (footer_offset - start_last) as usize;
        if file.seek(std::io::SeekFrom::Start(start_last)).is_err() {
            return ZipStructure::None;
        }
        let mut last_block = vec![0u8; last_len];
        if file.read_exact(&mut last_block).is_err() {
            return ZipStructure::None;
        }
        buffer[CRC_SZ..CRC_SZ + last_len].copy_from_slice(&last_block);

        if file.seek(std::io::SeekFrom::Start(footer_offset)).is_err() {
            return ZipStructure::None;
        }
        let mut footer = vec![0u8; T7Z_FOOTER_SIZE];
        if file.read_exact(&mut footer).is_err() {
            return ZipStructure::None;
        }

        buffer[256..264].copy_from_slice(&footer_offset.to_le_bytes());
        buffer[264..264 + T7Z_FOOTER_SIZE].copy_from_slice(&footer);

        let sig_header = b"\xA9\x9F\xD1\x57\x08\xA9\xD7\xEA\x29\x64\xB2\x36\x1B\x83\x52\x33\x01torrent7z_0.9beta";
        if footer.len() < 4 + sig_header.len() {
            return ZipStructure::None;
        }
        let mut expected = sig_header.to_vec();
        expected[16] = footer[4 + 16];
        if footer[4..4 + expected.len()] != expected {
            return ZipStructure::None;
        }

        let in_crc32 = u32::from_le_bytes(footer[0..4].try_into().unwrap());
        buffer[264..268].fill(0xFF);

        let mut crc = crc32fast::Hasher::new();
        crc.update(&buffer);
        let calc = crc.finalize();
        if in_crc32 == calc {
            ZipStructure::SevenZipTrrnt
        } else {
            ZipStructure::None
        }
    }
}

pub fn apply_romvault7z_marker(path: &Path, zip_struct: ZipStructure) -> std::io::Result<()> {
    let variant = match zip_struct {
        ZipStructure::SevenZipSLZMA => b'1',
        ZipStructure::SevenZipNLZMA => b'2',
        ZipStructure::SevenZipSZSTD => b'3',
        ZipStructure::SevenZipNZSTD => b'4',
        _ => return Ok(()),
    };

    let mut input = File::open(path)?;
    let mut signature = [0u8; 32];
    input.read_exact(&mut signature)?;
    if signature[0..6] != [0x37, 0x7A, 0xBC, 0xAF, 0x27, 0x1C] {
        return Ok(());
    }

    let next_header_offset = u64::from_le_bytes(signature[12..20].try_into().unwrap());
    let next_header_size = u64::from_le_bytes(signature[20..28].try_into().unwrap());
    let next_header_crc = u32::from_le_bytes(signature[28..32].try_into().unwrap());

    let original_header_pos = 32u64.saturating_add(next_header_offset);
    let file_len = input.metadata()?.len();
    if original_header_pos > file_len {
        return Ok(());
    }

    let mut marker = [0u8; 32];
    marker[..11].copy_from_slice(b"RomVault7Z0");
    marker[11] = variant;

    let mut already_has_marker = false;
    if original_header_pos >= 32
        && input
            .seek(SeekFrom::Start(original_header_pos - 32))
            .is_ok()
    {
        let mut existing = [0u8; 32];
        if input.read_exact(&mut existing).is_ok() && existing[..11] == *b"RomVault7Z0" {
            already_has_marker = true;
        }
    }

    if already_has_marker {
        marker[12..16].copy_from_slice(&next_header_crc.to_le_bytes());
        marker[16..24].copy_from_slice(&original_header_pos.to_le_bytes());
        marker[24..32].copy_from_slice(&next_header_size.to_le_bytes());

        let mut io = File::options().write(true).open(path)?;
        io.seek(SeekFrom::Start(original_header_pos - 32))?;
        io.write_all(&marker)?;
        io.flush()?;
        return Ok(());
    }

    let new_next_header_offset = next_header_offset.saturating_add(32);
    let new_header_pos = 32u64.saturating_add(new_next_header_offset);

    marker[12..16].copy_from_slice(&next_header_crc.to_le_bytes());
    marker[16..24].copy_from_slice(&new_header_pos.to_le_bytes());
    marker[24..32].copy_from_slice(&next_header_size.to_le_bytes());

    signature[12..20].copy_from_slice(&new_next_header_offset.to_le_bytes());
    let mut crc = crc32fast::Hasher::new();
    crc.update(&signature[12..32]);
    signature[8..12].copy_from_slice(&crc.finalize().to_le_bytes());

    let tmp_path = path.with_extension(format!(
        "{}.rv7ztmp",
        path.extension().and_then(|e| e.to_str()).unwrap_or("")
    ));
    let mut output = File::create(&tmp_path)?;

    output.write_all(&signature)?;
    input.seek(SeekFrom::Start(32))?;
    let to_copy = original_header_pos.saturating_sub(32);
    std::io::copy(
        &mut std::io::Read::by_ref(&mut input).take(to_copy),
        &mut output,
    )?;
    output.write_all(&marker)?;
    input.seek(SeekFrom::Start(original_header_pos))?;
    std::io::copy(&mut input, &mut output)?;
    output.flush()?;
    drop(output);

    if std::fs::rename(&tmp_path, path).is_err() {
        std::fs::copy(&tmp_path, path)?;
        let _ = std::fs::remove_file(&tmp_path);
    }

    Ok(())
}

impl ICompress for SevenZipFile {
    fn local_files_count(&self) -> usize {
        self.file_headers.len()
    }

    fn get_file_header(&self, index: usize) -> Option<&FileHeader> {
        self.file_headers.get(index)
    }

    fn zip_open_type(&self) -> ZipOpenType {
        self.zip_open_type
    }

    fn zip_file_open(
        &mut self,
        new_filename: &str,
        timestamp: i64,
        read_headers: bool,
    ) -> ZipReturn {
        self.zip_file_close();

        let path = Path::new(new_filename);
        if !path.exists() {
            return ZipReturn::ZipErrorFileNotFound;
        }

        let file_secs = fs::metadata(path)
            .ok()
            .and_then(|m| m.modified().ok())
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);
        if timestamp > 0 && file_secs != timestamp {
            return ZipReturn::ZipErrorTimeStamp;
        }

        let mut file = match File::open(path) {
            Ok(f) => f,
            Err(e) => {
                let code = e.raw_os_error().unwrap_or(0);
                if code == 32 || code == 33 {
                    return ZipReturn::ZipFileLocked;
                }
                return ZipReturn::ZipErrorOpeningFile;
            }
        };

        let crc_status = Self::verify_next_header_crc(&mut file);
        if crc_status != ZipReturn::ZipGood {
            return crc_status;
        }

        let password = Password::empty();
        let archive = match sevenz_rust::Archive::read(&mut file, &password) {
            Ok(a) => a,
            Err(_) => return ZipReturn::ZipErrorOpeningFile,
        };

        self.zip_filename = new_filename.to_string();
        self.time_stamp = file_secs;
        self.archive = Some(archive);
        self.file = Some(file);
        self.staging_dir = None;
        self.pending_write = None;
        self.zip_open_type = ZipOpenType::OpenRead;
        self.zip_struct = self.detect_zip_structure();

        if read_headers {
            self.read_headers()
        } else {
            ZipReturn::ZipGood
        }
    }

    fn zip_file_close(&mut self) {
        if self.zip_open_type == ZipOpenType::OpenWrite {
            if let Some(pending) = self.pending_write.take() {
                let _ = pending.file.borrow_mut().flush();
            }
            let _ = self.finalize_write();
            if let Some(staging) = self.staging_dir.take() {
                let _ = fs::remove_dir_all(staging);
            }
        }

        self.archive = None;
        self.file = None;
        self.staging_dir = None;
        self.pending_write = None;
        if let Some(tmp) = self.temp_open_path.take() {
            let _ = fs::remove_file(tmp);
        }
        self.zip_open_type = ZipOpenType::Closed;
        self.file_headers.clear();
        self.zip_struct = ZipStructure::None;
        self.file_comment.clear();
    }

    fn zip_file_open_read_stream(
        &mut self,
        index: usize,
    ) -> Result<(Box<dyn Read>, u64), ZipReturn> {
        if self.zip_open_type != ZipOpenType::OpenRead {
            return Err(ZipReturn::ZipReadingFromOutputFile);
        }

        let archive = match self.archive.as_ref() {
            Some(a) => a,
            None => return Err(ZipReturn::ZipErrorOpeningFile),
        };

        let file_entry: &ArchiveEntry = match archive.files.get(index) {
            Some(f) => f,
            None => return Err(ZipReturn::ZipErrorGettingDataStream),
        };

        if file_entry.is_directory() {
            return Ok((Box::new(std::io::Cursor::new(Vec::new())), 0));
        }

        let Some(bytes) = extract_entry_bytes(&self.zip_filename, file_entry.name())? else {
            return Err(ZipReturn::ZipErrorGettingDataStream);
        };
        Ok((Box::new(std::io::Cursor::new(bytes)), file_entry.size()))
    }

    fn zip_file_close_read_stream(&mut self) -> ZipReturn {
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

    fn zip_file_create(&mut self, _new_filename: &str) -> ZipReturn {
        self.zip_file_create_with_structure(_new_filename, ZipStructure::SevenZipSLZMA)
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
        if self.pending_write.is_some() {
            return Err(ZipReturn::ZipErrorOpeningFile);
        }

        let expected = Self::expected_compression_for_struct(self.zip_struct);
        if compression_method != expected {
            return Err(ZipReturn::ZipTrrntzipIncorrectCompressionUsed);
        }

        let Some(staging_dir) = self.staging_dir.as_ref() else {
            return Err(ZipReturn::ZipErrorOpeningFile);
        };

        let is_directory = uncompressed_size == 0 && filename.ends_with('/');
        if is_directory {
            let mut fh = FileHeader::new();
            fh.filename = filename.trim_end_matches('/').to_string();
            fh.uncompressed_size = 0;
            fh.is_directory = true;
            if let Some(m) = mod_time {
                fh.header_last_modified = m;
            }
            self.file_headers.push(fh);
            return Ok(Box::new(std::io::sink()));
        }

        let staged_path = staging_dir.join(filename);
        if let Some(parent) = staged_path.parent() {
            if fs::create_dir_all(parent).is_err() {
                return Err(ZipReturn::ZipErrorOpeningFile);
            }
        }

        if uncompressed_size == 0 {
            if File::create(&staged_path).is_err() {
                return Err(ZipReturn::ZipErrorOpeningFile);
            }
            let mut fh = FileHeader::new();
            fh.filename = filename.to_string();
            fh.uncompressed_size = 0;
            fh.is_directory = false;
            if let Some(m) = mod_time {
                fh.header_last_modified = m;
            }
            self.file_headers.push(fh);
            return Ok(Box::new(std::io::sink()));
        }

        let file = match File::create(&staged_path) {
            Ok(f) => f,
            Err(_) => return Err(ZipReturn::ZipErrorOpeningFile),
        };

        let mut fh = FileHeader::new();
        fh.filename = filename.to_string();
        fh.uncompressed_size = uncompressed_size;
        fh.is_directory = false;
        if let Some(m) = mod_time {
            fh.header_last_modified = m;
        }
        self.file_headers.push(fh);
        let header_index = self.file_headers.len() - 1;

        let rc = Rc::new(RefCell::new(file));
        self.pending_write = Some(SevenZipPendingWrite {
            header_index,
            file: Rc::clone(&rc),
            mod_time,
        });

        Ok(Box::new(SharedFileWriter { file: rc }))
    }

    fn zip_file_close_write_stream(&mut self, _crc32: &[u8]) -> ZipReturn {
        if self.zip_open_type != ZipOpenType::OpenWrite {
            return ZipReturn::ZipWritingToInputFile;
        }
        let Some(pending) = self.pending_write.take() else {
            return ZipReturn::ZipErrorOpeningFile;
        };

        let _ = pending.file.borrow_mut().flush();

        if let Some(h) = self.file_headers.get_mut(pending.header_index) {
            if _crc32.len() == 4 {
                h.crc = Some(_crc32.to_vec());
            }
            if let Some(m) = pending.mod_time {
                h.header_last_modified = m;
            }
        }

        ZipReturn::ZipGood
    }

    fn zip_file_close_failed(&mut self) {
        if self.zip_open_type == ZipOpenType::OpenWrite {
            if let Some(pending) = self.pending_write.take() {
                let _ = pending.file.borrow_mut().flush();
            }
            if let Some(staging) = self.staging_dir.take() {
                let _ = fs::remove_dir_all(staging);
            }
            if !self.zip_filename.is_empty() {
                let _ = fs::remove_file(&self.zip_filename);
            }
        }
        self.archive = None;
        self.file = None;
        self.staging_dir = None;
        self.pending_write = None;
        self.zip_open_type = ZipOpenType::Closed;
        self.file_headers.clear();
        self.zip_struct = ZipStructure::None;
        self.file_comment.clear();
    }
}

impl Default for SevenZipFile {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
#[path = "tests/seven_zip_tests.rs"]
mod tests;
