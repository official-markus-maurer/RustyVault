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
