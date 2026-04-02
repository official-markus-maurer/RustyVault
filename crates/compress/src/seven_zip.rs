use std::fs::File;
use std::io::{Read, Write, Seek};
use std::path::Path;

use crate::file_header::FileHeader;
use crate::i_compress::ICompress;
use crate::structured_archive::ZipStructure;
use crate::zip_enums::{ZipOpenType, ZipReturn};

use sevenz_rust::{Archive, ArchiveEntry, Password};

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
            file_headers: Vec::new(),
            file_comment: String::new(),
            zip_struct: ZipStructure::None,
        }
    }

    fn read_headers(&mut self) -> ZipReturn {
        let archive = match self.archive.as_ref() {
            Some(a) => a,
            None => return ZipReturn::ZipErrorOpeningFile,
        };

        self.file_headers.clear();

        for file in &archive.files {
            let mut fh = FileHeader::new();
            fh.filename = file.name().to_string();
            fh.uncompressed_size = file.size();
            fh.is_directory = file.is_directory();
            
            // sevenz_rust entry does not expose CRC directly on the entry struct
            // We would need to read it or see if it's available in future versions
            // Currently skipping CRC population at header level for 7z
            
            if file.has_last_modified_date {
                fh.header_last_modified = 0;
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

        if stored_crc != next_header_crc || stored_header_offset != header_pos || stored_header_size != next_header_size {
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
        if &footer[4..4 + expected.len()] != expected {
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

    fn zip_file_open(&mut self, new_filename: &str, timestamp: i64, read_headers: bool) -> ZipReturn {
        self.zip_file_close();
        
        let path = Path::new(new_filename);
        if !path.exists() {
            return ZipReturn::ZipErrorFileNotFound;
        }

        let mut file = match File::open(path) {
            Ok(f) => f,
            Err(_) => return ZipReturn::ZipErrorOpeningFile,
        };
        
        let password = Password::empty();
        let archive = match sevenz_rust::Archive::read(&mut file, &password) {
            Ok(a) => a,
            Err(_) => return ZipReturn::ZipErrorOpeningFile,
        };

        self.zip_filename = new_filename.to_string();
        self.time_stamp = timestamp;
        self.archive = Some(archive);
        self.file = Some(file);
        self.zip_open_type = ZipOpenType::OpenRead;
        self.zip_struct = self.detect_zip_structure();

        if read_headers {
            self.read_headers()
        } else {
            ZipReturn::ZipGood
        }
    }

    fn zip_file_close(&mut self) {
        self.archive = None;
        self.file = None;
        self.zip_open_type = ZipOpenType::Closed;
        self.file_headers.clear();
        self.zip_struct = ZipStructure::None;
    }

    fn zip_file_open_read_stream(&mut self, index: usize) -> Result<(Box<dyn Read>, u64), ZipReturn> {
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

        let _size = file_entry.size();
        
        let _file = std::fs::File::open(&self.zip_filename).map_err(|_| ZipReturn::ZipErrorGettingDataStream)?;
        let mut buffer = Vec::new();
        sevenz_rust::decompress_file_with_extract_fn(
            &self.zip_filename,
            "tmp",
            |entry, reader, _dest| {
                if entry.name() == file_entry.name() {
                    std::io::copy(reader, &mut buffer)?;
                    Ok(true)
                } else {
                    Ok(false)
                }
            }
        ).map_err(|_| ZipReturn::ZipErrorGettingDataStream)?;

        Ok((Box::new(std::io::Cursor::new(buffer)), file_entry.size()))
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
        // sevenz-rust crate currently only supports reading, not writing
        // For faithful port we'd need to implement or wrap a 7z writer,
        // or just return unsupported for now.
        ZipReturn::ZipWritingToInputFile
    }

    fn zip_file_open_write_stream(
        &mut self,
        _raw: bool,
        _filename: &str,
        _uncompressed_size: u64,
        _compression_method: u16,
        _mod_time: Option<i64>,
    ) -> Result<Box<dyn Write>, ZipReturn> {
        Err(ZipReturn::ZipWritingToInputFile)
    }

    fn zip_file_close_write_stream(&mut self, _crc32: &[u8]) -> ZipReturn {
        ZipReturn::ZipWritingToInputFile
    }

    fn zip_file_close_failed(&mut self) {
        self.zip_file_close();
    }
}

#[cfg(test)]
#[path = "tests/seven_zip_tests.rs"]
mod tests;
