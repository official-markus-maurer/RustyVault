impl ZipFile {
    pub(crate) fn open_write_stream_impl(
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

        if !matches!(compression_method, 0 | 8 | 93) {
            return Err(ZipReturn::ZipUnsupportedCompression);
        }

        if self.pending_write.is_some() {
            return Err(ZipReturn::ZipErrorOpeningFile);
        }

        let expected_compression_method = get_compression_type(self.zip_struct);
        if self.zip_struct != ZipStructure::None && compression_method != expected_compression_method {
            return Err(ZipReturn::ZipTrrntzipIncorrectCompressionUsed);
        }

        let mut mod_time = mod_time;
        match get_zip_date_time_type(self.zip_struct) {
            ZipDateType::TrrntZip => {
                mod_time = Some(19961224233200);
            }
            ZipDateType::None => {
                mod_time = Some(0);
            }
            _ => {}
        }

        if matches!(
            self.zip_struct,
            ZipStructure::ZipTrrnt | ZipStructure::ZipTDC | ZipStructure::ZipZSTD
        ) {
            if let Some(last) = self.file_headers.last() {
                if Self::trrntzip_string_compare(&last.filename, filename) > 0 {
                    return Err(ZipReturn::ZipTrrntzipIncorrectFileOrder);
                }

                if matches!(self.zip_struct, ZipStructure::ZipTrrnt | ZipStructure::ZipZSTD)
                    && last.filename.ends_with('/')
                    && filename.len() > last.filename.len()
                    && filename.starts_with(&last.filename)
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
            uncompressed_size,
            raw,
            buffer: Rc::clone(&buffer),
        });

        Ok(Box::new(SharedBufferWriter { buffer }))
    }

    pub(crate) fn close_write_stream_impl(&mut self, crc32: &[u8]) -> ZipReturn {
        let Some(pending_write) = self.pending_write.take() else {
            return ZipReturn::ZipErrorOpeningFile;
        };

        let Some(writer) = self.manual_writer.as_mut() else {
            return ZipReturn::ZipErrorOpeningFile;
        };

        let buffer = pending_write.buffer.borrow();
        let mut uncompressed = Vec::new();
        let mut compressed = Vec::new();

        if pending_write.raw && crc32.len() != 4 {
            return ZipReturn::ZipErrorWritingToOutputStream;
        }

        let crc_be = if crc32.len() == 4 {
            crc32.to_vec()
        } else if pending_write.raw {
            Vec::new()
        } else {
            let mut hasher = Crc32Hasher::new();
            hasher.update(&buffer);
            hasher.finalize().to_be_bytes().to_vec()
        };

        let uncompressed_size = if pending_write.raw {
            pending_write.uncompressed_size
        } else {
            buffer.len() as u64
        };

        if !pending_write.raw && pending_write.uncompressed_size != uncompressed_size {
            return ZipReturn::ZipErrorWritingToOutputStream;
        }

        if pending_write.raw {
            compressed.extend_from_slice(&buffer);
        } else {
            uncompressed.extend_from_slice(&buffer);
            match pending_write.compression_method {
                0 => {
                    compressed = uncompressed.clone();
                }
                8 => {
                    compressed = match deflate_raw_best(&uncompressed) {
                        Some(v) => v,
                        None => return ZipReturn::ZipErrorWritingToOutputStream,
                    };
                }
                93 => {
                    let threads = crate::zstd_config::zstd_threads();
                    let mut encoder = match zstd::stream::write::Encoder::new(Vec::new(), 19) {
                        Ok(e) => e,
                        Err(_) => return ZipReturn::ZipErrorWritingToOutputStream,
                    };
                    if threads > 0 {
                        let _ = encoder.multithread(threads as u32);
                    }
                    if encoder.write_all(&uncompressed).is_err() {
                        return ZipReturn::ZipErrorWritingToOutputStream;
                    }
                    compressed = match encoder.finish() {
                        Ok(v) => v,
                        Err(_) => return ZipReturn::ZipErrorWritingToOutputStream,
                    };
                }
                _ => return ZipReturn::ZipUnsupportedCompression,
            }
        }

        if compressed.is_empty() && uncompressed_size != 0 {
            return ZipReturn::ZipErrorWritingToOutputStream;
        }

        let entry = match writer.write_local_entry(
            &pending_write.filename,
            pending_write.compression_method,
            pending_write.mod_time,
            &crc_be,
            uncompressed_size,
            compressed,
        ) {
            Ok(e) => e,
            Err(zr) => return zr,
        };

        writer.entries.push(entry.clone());

        let mut fh = FileHeader::new();
        fh.filename = entry.filename;
        fh.uncompressed_size = entry.uncompressed_size;
        fh.is_directory = entry.is_directory;
        if !crc_be.is_empty() {
            fh.crc = Some(crc_be);
        }
        if let Some(mod_time) = pending_write.mod_time {
            fh.header_last_modified = mod_time;
        }
        self.file_headers.push(fh);

        ZipReturn::ZipGood
    }
}
