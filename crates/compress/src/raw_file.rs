use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::Path;

use crate::file_header::FileHeader;
use crate::i_compress::ICompress;
use crate::structured_archive::ZipStructure;
use crate::zip_enums::{ZipOpenType, ZipReturn};

pub struct RawFile {
    filename: String,
    zip_open_type: ZipOpenType,
    time_stamp: i64,
    
    file: Option<File>,
    header: Option<FileHeader>,
}

impl RawFile {
    pub fn new() -> Self {
        Self {
            filename: String::new(),
            zip_open_type: ZipOpenType::Closed,
            time_stamp: 0,
            file: None,
            header: None,
        }
    }
}

impl ICompress for RawFile {
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

    fn zip_file_open(&mut self, new_filename: &str, timestamp: i64, _read_headers: bool) -> ZipReturn {
        self.zip_file_close();

        let path = Path::new(new_filename);
        if !path.exists() {
            return ZipReturn::ZipErrorFileNotFound;
        }

        let metadata = match fs::metadata(path) {
            Ok(m) => m,
            Err(_) => return ZipReturn::ZipErrorOpeningFile,
        };

        let file = match File::open(path) {
            Ok(f) => f,
            Err(_) => return ZipReturn::ZipErrorOpeningFile,
        };

        self.filename = new_filename.to_string();
        self.time_stamp = timestamp;
        self.file = Some(file);
        self.zip_open_type = ZipOpenType::OpenRead;

        let mut fh = FileHeader::new();
        fh.filename = path.file_name().unwrap_or_default().to_string_lossy().to_string();
        fh.uncompressed_size = metadata.len();
        fh.is_directory = metadata.is_dir();
        
        if let Ok(mod_time) = metadata.modified() {
            if let Ok(dur) = mod_time.duration_since(std::time::UNIX_EPOCH) {
                fh.header_last_modified = dur.as_secs() as i64;
                fh.modified_time = Some(dur.as_secs() as i64);
            }
        }

        self.header = Some(fh);

        ZipReturn::ZipGood
    }

    fn zip_file_close(&mut self) {
        self.file = None;
        self.zip_open_type = ZipOpenType::Closed;
        self.header = None;
    }

    fn zip_file_open_read_stream(&mut self, index: usize) -> Result<(Box<dyn Read>, u64), ZipReturn> {
        if self.zip_open_type != ZipOpenType::OpenRead {
            return Err(ZipReturn::ZipReadingFromOutputFile);
        }

        if index != 0 {
            return Err(ZipReturn::ZipErrorGettingDataStream);
        }

        let file = self.file.as_ref().unwrap().try_clone().map_err(|_| ZipReturn::ZipErrorGettingDataStream)?;
        let size = self.header.as_ref().unwrap().uncompressed_size;
        
        Ok((Box::new(file), size))
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
        ""
    }

    fn zip_file_create(&mut self, new_filename: &str) -> ZipReturn {
        self.zip_file_close();
        
        let path = Path::new(new_filename);
        let file = match File::create(path) {
            Ok(f) => f,
            Err(_) => return ZipReturn::ZipErrorOpeningFile,
        };

        self.file = Some(file);
        self.filename = new_filename.to_string();
        self.zip_open_type = ZipOpenType::OpenWrite;

        ZipReturn::ZipGood
    }

    fn zip_file_open_write_stream(
        &mut self,
        _raw: bool,
        _filename: &str,
        _uncompressed_size: u64,
        _compression_method: u16,
        _mod_time: Option<i64>,
    ) -> Result<Box<dyn Write>, ZipReturn> {
        if self.zip_open_type != ZipOpenType::OpenWrite {
            return Err(ZipReturn::ZipWritingToInputFile);
        }

        let file = self.file.as_ref().unwrap().try_clone().map_err(|_| ZipReturn::ZipErrorWritingToOutputStream)?;
        Ok(Box::new(file))
    }

    fn zip_file_close_write_stream(&mut self, _crc32: &[u8]) -> ZipReturn {
        ZipReturn::ZipGood
    }

    fn zip_file_close_failed(&mut self) {
        self.zip_file_close();
        if !self.filename.is_empty() {
            let _ = fs::remove_file(&self.filename);
        }
    }
}
