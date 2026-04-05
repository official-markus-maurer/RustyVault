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
            .unwrap_or_default();

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
        let mut effective_struct = self.zip_struct;
        if effective_struct != ZipStructure::None {
            let valid = self
                .manual_writer
                .as_ref()
                .map(|mw| Self::validate_structured_write(effective_struct, &mw.entries))
                .unwrap_or(true);
            if !valid {
                effective_struct = ZipStructure::None;
                self.file_comment.clear();
            }
        }

        if let Some(mut mw) = self.manual_writer.take() {
            let _ = mw.finish(effective_struct, &self.file_comment);
        }
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
        let (mut stream, out_size, _) =
            self.zip_file_open_read_stream_from_local_header_pointer(local_head, false)?;
        buffer.clear();
        buffer.reserve(out_size as usize);
        stream
            .read_to_end(&mut buffer)
            .map_err(|_| ZipReturn::ZipErrorGettingDataStream)?;
        Ok((Box::new(std::io::Cursor::new(buffer)), out_size))
    }

    fn zip_file_open_read_stream_ex(
        &mut self,
        index: usize,
        raw: bool,
    ) -> Result<(Box<dyn Read>, u64, u16), ZipReturn> {
        ZipFile::zip_file_open_read_stream_ex(self, index, raw)
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

        self.manual_writer = Some(ManualZipWriter::new(file));
        self.writer = None;
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
        self.open_write_stream_impl(raw, filename, uncompressed_size, compression_method, mod_time)
    }

    fn zip_file_close_write_stream(&mut self, crc32: &[u8]) -> ZipReturn {
        self.close_write_stream_impl(crc32)
    }

    fn zip_file_close_failed(&mut self) {
        self.zip_file_close();
        if !self.zip_filename.is_empty() {
            let _ = std::fs::remove_file(&self.zip_filename);
        }
    }
}

impl Default for ZipFile {
    fn default() -> Self {
        Self::new()
    }
}
