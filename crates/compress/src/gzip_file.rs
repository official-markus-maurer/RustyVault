use std::cell::RefCell;
use std::fs::{self, File};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::Path;
use std::rc::Rc;

use crc32fast::Hasher as Crc32Hasher;
use flate2::read::DeflateDecoder;
use flate2::write::DeflateEncoder;
use flate2::Compression;

use crate::codepage_437;
use crate::file_header::FileHeader;
use crate::i_compress::ICompress;
use crate::structured_archive::ZipStructure;
use crate::zip_enums::{ZipOpenType, ZipReturn};

enum GZipWriteInner {
    Raw(File),
    Deflate(DeflateEncoder<File>),
    Zstd(zstd::stream::write::Encoder<'static, File>),
}

pub struct GZipFile {
    filename: String,
    zip_open_type: ZipOpenType,
    time_stamp: i64,

    file: Option<File>,
    write_inner: Option<Rc<RefCell<GZipWriteInner>>>,

    crc_be: Option<[u8; 4]>,
    uncompressed_size: u64,
    compressed_size: u64,
    compression_method: u8,
    mtime: u32,
    extra_data: Option<Vec<u8>>,
    file_comment: String,

    header_start_pos: u64,
    data_start_pos: u64,

    header: Option<FileHeader>,
}

pub struct SharedGZipWriter {
    inner: Rc<RefCell<GZipWriteInner>>,
}

impl Write for SharedGZipWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let mut inner = self.inner.borrow_mut();
        match &mut *inner {
            GZipWriteInner::Raw(f) => f.write(buf),
            GZipWriteInner::Deflate(e) => e.write(buf),
            GZipWriteInner::Zstd(e) => e.write(buf),
        }
    }

    fn flush(&mut self) -> std::io::Result<()> {
        let mut inner = self.inner.borrow_mut();
        match &mut *inner {
            GZipWriteInner::Raw(f) => f.flush(),
            GZipWriteInner::Deflate(e) => e.flush(),
            GZipWriteInner::Zstd(e) => e.flush(),
        }
    }
}

impl GZipFile {
    pub fn new() -> Self {
        Self {
            filename: String::new(),
            zip_open_type: ZipOpenType::Closed,
            time_stamp: 0,
            file: None,
            write_inner: None,
            crc_be: None,
            uncompressed_size: 0,
            compressed_size: 0,
            compression_method: 0,
            mtime: 0,
            extra_data: None,
            file_comment: String::new(),
            header_start_pos: 0,
            data_start_pos: 0,
            header: None,
        }
    }

    pub fn zip_file_open_stream<R: Read + Seek>(&mut self, mut stream: R) -> ZipReturn {
        self.zip_file_close();
        let mut bytes = Vec::new();
        if stream.seek(SeekFrom::Start(0)).is_err() {
            return ZipReturn::ZipErrorOpeningFile;
        }
        if stream.read_to_end(&mut bytes).is_err() {
            return ZipReturn::ZipErrorReadingFile;
        }
        let mut cursor = std::io::Cursor::new(bytes);
        self.file = None;
        self.filename.clear();
        self.time_stamp = 0;
        self.zip_open_type = ZipOpenType::OpenRead;
        self.parse_headers_from_reader(&mut cursor)
    }

    pub fn zip_file_roll_back(&mut self) -> ZipReturn {
        if self.zip_open_type != ZipOpenType::OpenWrite {
            return ZipReturn::ZipWritingToInputFile;
        }
        let Some(file) = self.file.as_mut() else {
            return ZipReturn::ZipErrorOpeningFile;
        };
        if file.seek(SeekFrom::Start(self.data_start_pos)).is_err() {
            return ZipReturn::ZipErrorWritingToOutputStream;
        }
        ZipReturn::ZipGood
    }

    fn unix_seconds_to_utc_ticks(secs: i32) -> i64 {
        const EPOCH_TIME_TO_UTC_TIME: i64 = 621_355_968_000_000_000;
        const TICKS_PER_SECOND: i64 = 10_000_000;
        EPOCH_TIME_TO_UTC_TIME.saturating_add((secs as i64).saturating_mul(TICKS_PER_SECOND))
    }

    fn parse_headers_from_reader<R: Read + Seek>(&mut self, r: &mut R) -> ZipReturn {
        let mut fixed = [0u8; 10];
        if r.read_exact(&mut fixed).is_err() {
            return ZipReturn::ZipErrorReadingFile;
        }
        if fixed[0] != 0x1f || fixed[1] != 0x8b {
            return ZipReturn::ZipSignatureError;
        }

        let cm = fixed[2];
        if cm != 8 && cm != 93 {
            return ZipReturn::ZipUnsupportedCompression;
        }
        self.compression_method = cm;
        let flg = fixed[3];
        self.mtime = u32::from_le_bytes([fixed[4], fixed[5], fixed[6], fixed[7]]);

        self.extra_data = None;
        self.crc_be = None;
        self.uncompressed_size = 0;
        self.file_comment.clear();

        if (flg & 0x04) == 0x04 {
            let mut xlen_bytes = [0u8; 2];
            if r.read_exact(&mut xlen_bytes).is_err() {
                return ZipReturn::ZipErrorReadingFile;
            }
            let xlen = u16::from_le_bytes(xlen_bytes) as usize;
            let mut extra = vec![0u8; xlen];
            if r.read_exact(&mut extra).is_err() {
                return ZipReturn::ZipErrorReadingFile;
            }
            match xlen {
                12 => {
                    self.crc_be = Some([extra[0], extra[1], extra[2], extra[3]]);
                    self.uncompressed_size = u64::from_le_bytes(extra[4..12].try_into().unwrap());
                }
                28 | 77 => {
                    self.crc_be = Some([extra[16], extra[17], extra[18], extra[19]]);
                    self.uncompressed_size = u64::from_le_bytes(extra[20..28].try_into().unwrap());
                }
                _ => {}
            }
            self.extra_data = Some(extra);
        }

        if (flg & 0x08) == 0x08 {
            loop {
                let mut b = [0u8; 1];
                if r.read_exact(&mut b).is_err() {
                    return ZipReturn::ZipErrorReadingFile;
                }
                if b[0] == 0 {
                    break;
                }
            }
        }

        if (flg & 0x10) == 0x10 {
            let mut comment = Vec::new();
            loop {
                let mut b = [0u8; 1];
                if r.read_exact(&mut b).is_err() {
                    return ZipReturn::ZipErrorReadingFile;
                }
                if b[0] == 0 {
                    break;
                }
                comment.push(b[0]);
            }
            self.file_comment = codepage_437::decode(&comment);
        }

        if (flg & 0x02) == 0x02 {
            let mut crc16 = [0u8; 2];
            if r.read_exact(&mut crc16).is_err() {
                return ZipReturn::ZipErrorReadingFile;
            }
        }

        let data_start = match r.stream_position() {
            Ok(v) => v,
            Err(_) => return ZipReturn::ZipErrorReadingFile,
        };
        let file_len = match r.seek(SeekFrom::End(0)) {
            Ok(v) => v,
            Err(_) => return ZipReturn::ZipErrorReadingFile,
        };
        if file_len < data_start + 8 {
            return ZipReturn::ZipDecodeError;
        }

        self.data_start_pos = data_start;
        self.compressed_size = (file_len - data_start) - 8;

        if r.seek(SeekFrom::End(-8)).is_err() {
            return ZipReturn::ZipErrorReadingFile;
        }
        let mut trailer = [0u8; 8];
        if r.read_exact(&mut trailer).is_err() {
            return ZipReturn::ZipErrorReadingFile;
        }
        let gz_crc_le = [trailer[0], trailer[1], trailer[2], trailer[3]];
        let gz_isize = u32::from_le_bytes([trailer[4], trailer[5], trailer[6], trailer[7]]);

        let trailer_crc_be = [gz_crc_le[3], gz_crc_le[2], gz_crc_le[1], gz_crc_le[0]];
        if let Some(existing) = self.crc_be {
            if existing != trailer_crc_be {
                return ZipReturn::ZipDecodeError;
            }
        } else {
            self.crc_be = Some(trailer_crc_be);
        }

        if self.uncompressed_size != 0 {
            if gz_isize != (self.uncompressed_size as u32) {
                return ZipReturn::ZipDecodeError;
            }
        } else {
            self.uncompressed_size = gz_isize as u64;
        }

        ZipReturn::ZipGood
    }
}

impl ICompress for GZipFile {
    fn local_files_count(&self) -> usize {
        1
    }

    fn get_file_header(&self, index: usize) -> Option<&FileHeader> {
        if index == 0 {
            self.header.as_ref()
        } else {
            None
        }
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

        let file_secs = fs::metadata(path)
            .ok()
            .and_then(|m| m.modified().ok())
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);

        if timestamp > 0 && timestamp != file_secs {
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

        self.filename = new_filename.to_string();
        self.time_stamp = file_secs;
        self.zip_open_type = ZipOpenType::OpenRead;

        if !read_headers {
            self.file = Some(file);
            return ZipReturn::ZipGood;
        }

        if file.seek(SeekFrom::Start(0)).is_err() {
            return ZipReturn::ZipErrorReadingFile;
        }
        let status = self.parse_headers_from_reader(&mut file);
        if status != ZipReturn::ZipGood {
            return status;
        }

        let mut fh = FileHeader::new();
        fh.filename = path.file_name().unwrap_or_default().to_string_lossy().to_string();
        fh.uncompressed_size = self.uncompressed_size;
        fh.is_directory = false;
        fh.crc = self.crc_be.map(|c| c.to_vec());
        fh.modified_time = if self.mtime == 0 {
            None
        } else {
            Some(Self::unix_seconds_to_utc_ticks(self.mtime as i32))
        };

        self.header = Some(fh);
        self.file = Some(file);

        ZipReturn::ZipGood
    }

    fn zip_file_close(&mut self) {
        self.file = None;
        self.write_inner = None;
        self.zip_open_type = ZipOpenType::Closed;
        self.header = None;
        self.crc_be = None;
        self.uncompressed_size = 0;
        self.compressed_size = 0;
        self.compression_method = 0;
        self.mtime = 0;
        self.extra_data = None;
        self.file_comment.clear();
        self.header_start_pos = 0;
        self.data_start_pos = 0;
    }

    fn zip_file_open_read_stream(&mut self, index: usize) -> Result<(Box<dyn Read>, u64), ZipReturn> {
        if self.zip_open_type != ZipOpenType::OpenRead {
            return Err(ZipReturn::ZipReadingFromOutputFile);
        }
        if index != 0 {
            return Err(ZipReturn::ZipErrorGettingDataStream);
        }

        let file = self
            .file
            .as_ref()
            .ok_or(ZipReturn::ZipErrorGettingDataStream)?
            .try_clone()
            .map_err(|_| ZipReturn::ZipErrorGettingDataStream)?;
        let mut file = file;
        if file.seek(SeekFrom::Start(self.data_start_pos)).is_err() {
            return Err(ZipReturn::ZipErrorGettingDataStream);
        }
        let take = file.take(self.compressed_size);

        if self.compression_method == 8 {
            Ok((Box::new(DeflateDecoder::new(take)), self.uncompressed_size))
        } else if self.compression_method == 93 {
            let decoder = zstd::stream::read::Decoder::new(take)
                .map_err(|_| ZipReturn::ZipUnsupportedCompression)?;
            Ok((Box::new(decoder), self.uncompressed_size))
        } else {
            Err(ZipReturn::ZipUnsupportedCompression)
        }
    }

    fn zip_file_close_read_stream(&mut self) -> ZipReturn {
        ZipReturn::ZipGood
    }

    fn zip_struct(&self) -> ZipStructure {
        ZipStructure::None
    }

    fn zip_filename(&self) -> &str {
        &self.filename
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

        self.file = Some(file);
        self.filename = new_filename.to_string();
        self.zip_open_type = ZipOpenType::OpenWrite;
        self.crc_be = None;
        self.uncompressed_size = 0;
        self.compressed_size = 0;
        self.compression_method = 0;
        self.mtime = 0;
        self.extra_data = None;
        self.file_comment.clear();
        self.header_start_pos = 0;
        self.data_start_pos = 0;

        ZipReturn::ZipGood
    }

    fn zip_file_open_write_stream(
        &mut self,
        raw: bool,
        _filename: &str,
        uncompressed_size: u64,
        compression_method: u16,
        _mod_time: Option<i64>,
    ) -> Result<Box<dyn Write>, ZipReturn> {
        if self.zip_open_type != ZipOpenType::OpenWrite {
            return Err(ZipReturn::ZipWritingToInputFile);
        }
        if self.write_inner.is_some() {
            return Err(ZipReturn::ZipErrorOpeningFile);
        }
        let Some(file) = self.file.as_mut() else {
            return Err(ZipReturn::ZipErrorWritingToOutputStream);
        };

        let cm = if compression_method == 93 { 93u8 } else { 8u8 };
        if cm != 8 && cm != 93 {
            return Err(ZipReturn::ZipUnsupportedCompression);
        }
        self.compression_method = cm;
        self.uncompressed_size = uncompressed_size;

        if file.seek(SeekFrom::Start(0)).is_err() {
            return Err(ZipReturn::ZipErrorWritingToOutputStream);
        }

        file.write_all(&[0x1f, 0x8b, cm, 0x04]) // ID1, ID2, CM, FLG
            .map_err(|_| ZipReturn::ZipErrorWritingToOutputStream)?;
        file.write_all(&0u32.to_le_bytes())
            .map_err(|_| ZipReturn::ZipErrorWritingToOutputStream)?;
        file.write_all(&[0x00, 0xff])
            .map_err(|_| ZipReturn::ZipErrorWritingToOutputStream)?;

        let extra = self.extra_data.clone();
        if let Some(extra) = &extra {
            if extra.len() > u16::MAX as usize {
                return Err(ZipReturn::ZipErrorWritingToOutputStream);
            }
            file.write_all(&(extra.len() as u16).to_le_bytes())
                .map_err(|_| ZipReturn::ZipErrorWritingToOutputStream)?;
            self.header_start_pos = file.stream_position().map_err(|_| ZipReturn::ZipErrorWritingToOutputStream)?;
            file.write_all(extra)
                .map_err(|_| ZipReturn::ZipErrorWritingToOutputStream)?;
        } else {
            file.write_all(&(12u16).to_le_bytes())
                .map_err(|_| ZipReturn::ZipErrorWritingToOutputStream)?;
            self.header_start_pos = file.stream_position().map_err(|_| ZipReturn::ZipErrorWritingToOutputStream)?;
            file.write_all(&[0u8; 12])
                .map_err(|_| ZipReturn::ZipErrorWritingToOutputStream)?;
        }

        self.data_start_pos = file.stream_position().map_err(|_| ZipReturn::ZipErrorWritingToOutputStream)?;

        let cloned = file
            .try_clone()
            .map_err(|_| ZipReturn::ZipErrorWritingToOutputStream)?;
        let inner = if raw {
            GZipWriteInner::Raw(cloned)
        } else if cm == 8 {
            GZipWriteInner::Deflate(DeflateEncoder::new(cloned, Compression::best()))
        } else {
            let enc = zstd::stream::write::Encoder::new(cloned, 19)
                .map_err(|_| ZipReturn::ZipUnsupportedCompression)?;
            GZipWriteInner::Zstd(enc)
        };

        let rc = Rc::new(RefCell::new(inner));
        self.write_inner = Some(Rc::clone(&rc));
        Ok(Box::new(SharedGZipWriter { inner: rc }))
    }

    fn zip_file_close_write_stream(&mut self, crc32_be: &[u8]) -> ZipReturn {
        if self.zip_open_type != ZipOpenType::OpenWrite {
            return ZipReturn::ZipWritingToInputFile;
        }
        let Some(rc) = self.write_inner.take() else {
            return ZipReturn::ZipGood;
        };

        let inner = Rc::try_unwrap(rc)
            .ok()
            .map(|c| c.into_inner());
        let Some(inner) = inner else {
            return ZipReturn::ZipErrorWritingToOutputStream;
        };

        let mut file = match inner {
            GZipWriteInner::Raw(f) => f,
            GZipWriteInner::Deflate(e) => match e.finish() {
                Ok(f) => f,
                Err(_) => return ZipReturn::ZipErrorWritingToOutputStream,
            },
            GZipWriteInner::Zstd(e) => match e.finish() {
                Ok(f) => f,
                Err(_) => return ZipReturn::ZipErrorWritingToOutputStream,
            },
        };

        if file.flush().is_err() {
            return ZipReturn::ZipErrorWritingToOutputStream;
        }

        let end_pos = match file.stream_position() {
            Ok(v) => v,
            Err(_) => return ZipReturn::ZipErrorWritingToOutputStream,
        };
        if end_pos < self.data_start_pos {
            return ZipReturn::ZipErrorWritingToOutputStream;
        }
        self.compressed_size = end_pos - self.data_start_pos;

        let crc_be = if crc32_be.len() == 4 {
            [crc32_be[0], crc32_be[1], crc32_be[2], crc32_be[3]]
        } else {
            let mut hasher = Crc32Hasher::new();
            hasher.update(&[]);
            let crc = hasher.finalize();
            crc.to_be_bytes()
        };
        self.crc_be = Some(crc_be);

        let trailer_le = [crc_be[3], crc_be[2], crc_be[1], crc_be[0]];
        if file.write_all(&trailer_le).is_err() {
            return ZipReturn::ZipErrorWritingToOutputStream;
        }
        if file
            .write_all(&(self.uncompressed_size as u32).to_le_bytes())
            .is_err()
        {
            return ZipReturn::ZipErrorWritingToOutputStream;
        }

        let final_end = match file.stream_position() {
            Ok(v) => v,
            Err(_) => return ZipReturn::ZipErrorWritingToOutputStream,
        };

        if file.seek(SeekFrom::Start(self.header_start_pos)).is_err() {
            return ZipReturn::ZipErrorWritingToOutputStream;
        }

        if self.extra_data.is_none() {
            if file.write_all(&crc_be).is_err() {
                return ZipReturn::ZipErrorWritingToOutputStream;
            }
            if file.write_all(&self.uncompressed_size.to_le_bytes()).is_err() {
                return ZipReturn::ZipErrorWritingToOutputStream;
            }
        } else if let Some(extra) = self.extra_data.as_ref() {
            if file.write_all(extra).is_err() {
                return ZipReturn::ZipErrorWritingToOutputStream;
            }
        }

        if file.seek(SeekFrom::Start(final_end)).is_err() {
            return ZipReturn::ZipErrorWritingToOutputStream;
        }
        if file.flush().is_err() {
            return ZipReturn::ZipErrorWritingToOutputStream;
        }

        self.file = Some(file);
        ZipReturn::ZipGood
    }

    fn zip_file_close_failed(&mut self) {
        let path = self.filename.clone();
        self.zip_file_close();
        if !path.is_empty() {
            let _ = fs::remove_file(path);
        }
    }
}

#[cfg(test)]
#[path = "tests/gzip_file_tests.rs"]
mod tests;
