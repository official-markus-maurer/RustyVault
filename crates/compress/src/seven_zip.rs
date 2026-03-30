use std::fs::File;
use std::io::{Read, Write};
use std::path::Path;

use crate::file_header::FileHeader;
use crate::i_compress::ICompress;
use crate::structured_archive::ZipStructure;
use crate::zip_enums::{ZipOpenType, ZipReturn};

// Using sevenz-rust crate
use sevenz_rust::{Archive, Password, SevenZArchiveEntry};

pub struct SevenZipFile {
    zip_filename: String,
    zip_open_type: ZipOpenType,
    time_stamp: i64,
    
    // In read mode, we hold the loaded archive
    archive: Option<Archive>,
    file: Option<File>,
    
    file_headers: Vec<FileHeader>,
    file_comment: String,
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
            
            let dt_opt: Option<sevenz_rust::nt_time::FileTime> = Some(file.last_modified_date());
            if let Some(_dt) = dt_opt {
                // Not standard DateTime struct, sevenz-rust uses nt_time::time::OffsetDateTime
                // Approximate fallback or use internal
                fh.header_last_modified = 0;
            }
            
            self.file_headers.push(fh);
        }

        ZipReturn::ZipGood
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
        
        let archive = match sevenz_rust::Archive::read(&mut file, 0, Password::empty().as_slice()) {
            Ok(a) => a,
            Err(_) => return ZipReturn::ZipErrorOpeningFile,
        };

        self.zip_filename = new_filename.to_string();
        self.time_stamp = timestamp;
        self.archive = Some(archive);
        self.file = Some(file);
        self.zip_open_type = ZipOpenType::OpenRead;

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
    }

    fn zip_file_open_read_stream(&mut self, index: usize) -> Result<(Box<dyn Read>, u64), ZipReturn> {
        if self.zip_open_type != ZipOpenType::OpenRead {
            return Err(ZipReturn::ZipReadingFromOutputFile);
        }

        let archive = match self.archive.as_ref() {
            Some(a) => a,
            None => return Err(ZipReturn::ZipErrorOpeningFile),
        };

        let file_entry: &SevenZArchiveEntry = match archive.files.get(index) {
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
        ZipStructure::None
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
