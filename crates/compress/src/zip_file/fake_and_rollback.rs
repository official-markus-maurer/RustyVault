impl ZipFile {
    /// Mutates a validator-tagged ZIP file so that it no longer validates as "clean".
    ///
    /// This helper looks for a trailing `TORRENTZIPPED-...` or `RVZSTD-...` comment and, when
    /// present, overwrites the last 8 bytes of the file with ASCII `0` bytes. The intent is to
    /// force downstream tools to treat the file as changed.
    ///
    /// If the file is not recognized as using a supported validator comment layout, this is a
    /// no-op that returns `Ok(())`.
    pub fn break_trrntzip(path: &str) -> std::io::Result<()> {
        let mut file = File::options().read(true).write(true).open(path)?;
        let mut bytes = Vec::new();
        file.read_to_end(&mut bytes)?;

        let len = bytes.len();
        if len < 22 {
            return Ok(());
        }

        let eocd_offset = bytes
            .windows(4)
            .rposition(|window| window == [0x50, 0x4B, 0x05, 0x06]);
        let Some(eocd_offset) = eocd_offset else {
            return Ok(());
        };
        if eocd_offset + 22 > len {
            return Ok(());
        }
        let comment_length =
            u16::from_le_bytes([bytes[eocd_offset + 20], bytes[eocd_offset + 21]]) as usize;
        if eocd_offset + 22 + comment_length != len {
            return Ok(());
        }

        let comment = std::str::from_utf8(&bytes[eocd_offset + 22..len]).unwrap_or("");
        let prefix_len = if comment.starts_with("TORRENTZIPPED-") {
            14
        } else if comment.starts_with("RVZSTD-") {
            7
        } else {
            return Ok(());
        };

        if len >= 8 && comment_length >= prefix_len + 8 {
            let start = len - 8;
            bytes[start..len].copy_from_slice(b"00000000");
            file.seek(SeekFrom::Start(0))?;
            file.write_all(&bytes)?;
            file.flush()?;
        }

        Ok(())
    }

    /// Puts the instance into "fake write" mode.
    ///
    /// Fake write mode allows building a ZIP in memory (via [`zip_file_fake_open_memory_stream`]
    /// and related APIs) without opening a physical output file.
    pub fn zip_create_fake(&mut self) {
        if self.zip_open_type != ZipOpenType::Closed {
            return;
        }
        self.zip_open_type = ZipOpenType::OpenFakeWrite;
        self.fake_write = true;
    }

    /// Starts a fake in-memory ZIP writer session.
    ///
    /// Returns [`ZipReturn::ZipWritingToInputFile`] if the instance is not in fake write mode.
    pub fn zip_file_fake_open_memory_stream(&mut self) -> ZipReturn {
        if self.zip_open_type != ZipOpenType::OpenFakeWrite {
            return ZipReturn::ZipWritingToInputFile;
        }
        self.writer = Some(ZipWriter::new(ZipWriterFile::Memory(std::io::Cursor::new(
            Vec::new(),
        ))));
        ZipReturn::ZipGood
    }

    /// Finishes a fake in-memory ZIP writer session and returns the produced bytes.
    ///
    /// Returns `None` when fake write mode is not active or when finalization fails.
    pub fn zip_file_fake_close_memory_stream(&mut self) -> Option<Vec<u8>> {
        if !self.fake_write {
            return None;
        }
        let w = self.writer.take()?;
        let cursor = w.finish().ok()?;
        self.zip_open_type = ZipOpenType::Closed;
        self.fake_write = false;
        match cursor {
            ZipWriterFile::Memory(c) => Some(c.into_inner()),
        }
    }

    #[allow(clippy::too_many_arguments)]
    /// Builds a local file header for a "fake" entry and records minimal metadata.
    ///
    /// This does not write any compressed payload. Callers provide the `file_offset`,
    /// sizes, CRC, and compression method to synthesize the header bytes.
    ///
    /// On success, returns the raw local-header bytes that should be written to the output stream.
    pub fn zip_file_add_fake(
        &mut self,
        filename: &str,
        file_offset: u64,
        uncompressed_size: u64,
        compressed_size: u64,
        crc32: &[u8],
        compression_method: u16,
        header_last_modified: i64,
    ) -> Result<Vec<u8>, ZipReturn> {
        if self.zip_open_type != ZipOpenType::OpenFakeWrite {
            return Err(ZipReturn::ZipWritingToInputFile);
        }

        let (extra, header_uncompressed_size, header_compressed_size, _header_local_offset) =
            zip_extra_field::write_zip64_extra(
                uncompressed_size,
                compressed_size,
                file_offset,
                false,
            );

        let is_zip64 = !extra.is_empty();

        let mut general_purpose_bit_flag: u16 = 2;
        let filename_bytes = if let Some(cp) = codepage_437::encode(filename) {
            cp
        } else {
            general_purpose_bit_flag |= 1 << 11;
            filename.as_bytes().to_vec()
        };

        if filename_bytes.len() > u16::MAX as usize || extra.len() > u16::MAX as usize {
            return Err(ZipReturn::ZipFileNameToLong);
        }

        let dt = Self::zip_datetime_from_i64(header_last_modified);
        let (dos_time, dos_date) = if let Some(dt) = dt {
            (dt.timepart(), dt.datepart())
        } else {
            (0u16, 0u16)
        };

        let crc_u32 = if crc32.len() == 4 {
            u32::from_be_bytes([crc32[0], crc32[1], crc32[2], crc32[3]])
        } else {
            0u32
        };

        let version_needed_to_extract: u16 = if compression_method == 93 {
            63
        } else if is_zip64 {
            45
        } else {
            20
        };

        let mut out = Vec::new();
        out.extend_from_slice(&[0x50, 0x4B, 0x03, 0x04]);
        out.extend_from_slice(&version_needed_to_extract.to_le_bytes());
        out.extend_from_slice(&general_purpose_bit_flag.to_le_bytes());
        out.extend_from_slice(&compression_method.to_le_bytes());
        out.extend_from_slice(&dos_time.to_le_bytes());
        out.extend_from_slice(&dos_date.to_le_bytes());
        out.extend_from_slice(&crc_u32.to_le_bytes());
        out.extend_from_slice(&header_compressed_size.to_le_bytes());
        out.extend_from_slice(&header_uncompressed_size.to_le_bytes());
        out.extend_from_slice(&(filename_bytes.len() as u16).to_le_bytes());
        out.extend_from_slice(&(extra.len() as u16).to_le_bytes());
        out.extend_from_slice(&filename_bytes);
        out.extend_from_slice(&extra);

        let mut fh = FileHeader::new();
        fh.filename = filename.to_string();
        fh.local_head = Some(file_offset);
        fh.uncompressed_size = uncompressed_size;
        fh.is_directory = filename.ends_with('/');
        fh.crc = if crc32.len() == 4 {
            Some(crc32.to_vec())
        } else {
            None
        };
        fh.header_last_modified = header_last_modified;
        self.file_headers.push(fh);

        Ok(out)
    }

    /// Rolls back the most recent write operation.
    ///
    /// If a write stream is currently pending, it is discarded. Otherwise the underlying writer
    /// is truncated back to the previous local header offset.
    pub fn zip_file_roll_back(&mut self) -> ZipReturn {
        if self.zip_open_type != ZipOpenType::OpenWrite {
            return ZipReturn::ZipWritingToInputFile;
        }
        if self.pending_write.is_some() {
            self.pending_write = None;
            return ZipReturn::ZipGood;
        }

        let Some(writer) = self.manual_writer.as_mut() else {
            return ZipReturn::ZipErrorRollBackFile;
        };
        let Some(last) = writer.entries.pop() else {
            return ZipReturn::ZipErrorRollBackFile;
        };

        let truncate_to = last.local_header_offset;
        if writer.file.set_len(truncate_to).is_err() {
            return ZipReturn::ZipErrorRollBackFile;
        }
        if writer.file.seek(SeekFrom::Start(truncate_to)).is_err() {
            return ZipReturn::ZipErrorRollBackFile;
        }

        let _ = self.file_headers.pop();
        ZipReturn::ZipGood
    }
}
