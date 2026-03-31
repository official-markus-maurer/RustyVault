use std::cell::RefCell;
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::Path;
use std::rc::Rc;

use crate::file_header::FileHeader;
use crate::i_compress::ICompress;
use crate::structured_archive::ZipStructure;
use crate::zip_enums::{ZipOpenType, ZipReturn};

use zip::write::FileOptions;
use zip::{CompressionMethod, DateTime, ZipArchive, ZipWriter};

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

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn unique_temp_zip(name: &str) -> String {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir()
            .join(format!("{}_{}.zip", name, unique))
            .to_string_lossy()
            .to_string()
    }

    #[test]
    fn test_zip_file_write_stream_persists_written_data() {
        let path = unique_temp_zip("compress_zip_write");
        let mut zip_file = ZipFile::new();

        assert_eq!(zip_file.zip_file_create(&path), ZipReturn::ZipGood);
        let mut stream = zip_file
            .zip_file_open_write_stream(false, "hello.txt", 5, 8, Some(19961224233200))
            .unwrap();
        stream.write_all(b"hello").unwrap();
        drop(stream);
        assert_eq!(
            zip_file.zip_file_close_write_stream(&[0x36, 0x10, 0xA6, 0x86]),
            ZipReturn::ZipGood
        );
        zip_file.zip_file_close();

        let mut reopened = ZipFile::new();
        assert_eq!(reopened.zip_file_open(&path, 0, true), ZipReturn::ZipGood);
        assert_eq!(reopened.local_files_count(), 1);
        assert_eq!(reopened.get_file_header(0).unwrap().filename, "hello.txt");

        let (mut reader, size) = reopened.zip_file_open_read_stream(0).unwrap();
        let mut data = Vec::new();
        reader.read_to_end(&mut data).unwrap();
        assert_eq!(size, 5);
        assert_eq!(data, b"hello");

        reopened.zip_file_close();
        let _ = fs::remove_file(path);
    }

    #[test]
    fn test_zip_file_detects_torrentzip_comment() {
        let path = unique_temp_zip("compress_zip_comment");
        {
            let file = File::create(&path).unwrap();
            let mut writer = ZipWriter::new(file);
            let options = FileOptions::default().compression_method(CompressionMethod::Deflated);
            writer.start_file("a.txt", options).unwrap();
            writer.write_all(b"a").unwrap();
            writer.set_comment("TORRENTZIPPED-12345678");
            writer.finish().unwrap();
        }

        let mut zip_file = ZipFile::new();
        assert_eq!(zip_file.zip_file_open(&path, 0, true), ZipReturn::ZipGood);
        assert_eq!(zip_file.zip_struct(), ZipStructure::ZipTrrnt);
        zip_file.zip_file_close();

        let _ = fs::remove_file(path);
    }

    #[test]
    fn test_zip_file_reads_local_header_offsets() {
        let path = unique_temp_zip("compress_zip_offsets");
        {
            let file = File::create(&path).unwrap();
            let mut writer = ZipWriter::new(file);
            let options = FileOptions::default().compression_method(CompressionMethod::Deflated);
            writer.start_file("a.txt", options).unwrap();
            writer.write_all(b"aaa").unwrap();
            writer.start_file("b.txt", options).unwrap();
            writer.write_all(b"bbbb").unwrap();
            writer.finish().unwrap();
        }

        let mut zip_file = ZipFile::new();
        assert_eq!(zip_file.zip_file_open(&path, 0, true), ZipReturn::ZipGood);
        let first = zip_file.get_file_header(0).unwrap().local_head.unwrap();
        let second = zip_file.get_file_header(1).unwrap().local_head.unwrap();
        assert_eq!(first, 0);
        assert!(second > first);
        zip_file.zip_file_close();

        let _ = fs::remove_file(path);
    }
}

impl ZipFile {
    pub fn new() -> Self {
        Self {
            zip_filename: String::new(),
            zip_open_type: ZipOpenType::Closed,
            time_stamp: 0,
            archive: None,
            writer: None,
            file_headers: Vec::new(),
            file_comment: String::new(),
            pending_write: None,
        }
    }

    fn detect_zip_structure(&self) -> ZipStructure {
        if self.file_comment.starts_with("TORRENTZIPPED-") {
            return ZipStructure::ZipTrrnt;
        }
        if self.file_comment.starts_with("TDC-") {
            return ZipStructure::ZipTDC;
        }
        if self.file_comment.starts_with("RVZSTD-") {
            return ZipStructure::ZipZSTD;
        }
        ZipStructure::None
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
            ]) as u64;

            local_offsets.push(relative_offset);
            central_offset += 46 + file_name_length + extra_length + comment_length;
        }

        Some(local_offsets)
    }

    fn read_headers(&mut self) -> ZipReturn {
        let local_offsets = Self::read_local_header_offsets(&self.zip_filename);
        let archive = match self.archive.as_mut() {
            Some(a) => a,
            None => return ZipReturn::ZipErrorOpeningFile,
        };

        self.file_headers.clear();
        let comment = archive.comment();
        self.file_comment = String::from_utf8_lossy(comment).to_string();

        for i in 0..archive.len() {
            if let Ok(file) = archive.by_index(i) {
                let mut fh = FileHeader::new();
                fh.filename = file.name().to_string();
                fh.uncompressed_size = file.size();
                fh.is_directory = file.is_dir();
                fh.local_head = local_offsets
                    .as_ref()
                    .and_then(|offsets| offsets.get(i).copied());
                
                // Read CRC if available (usually only after reading for some methods, but zip crate exposes it)
                fh.crc = Some(file.crc32().to_be_bytes().to_vec());
                
                let dt = file.last_modified();
                let year: u16 = dt.year();
                let year_i64: i64 = year as i64;
                let month = dt.month() as i64;
                let day = dt.day() as i64;
                let hour = dt.hour() as i64;
                let min = dt.minute() as i64;
                let sec = dt.second() as i64;
                
                // Standard DateTime integer conversion used by TrrntZip
                fh.header_last_modified = year_i64 * 10000000000_i64 + month * 100000000_i64 + day * 1000000_i64 + hour * 10000_i64 + min * 100_i64 + sec;
                
                self.file_headers.push(fh);
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
        self.detect_zip_structure()
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
