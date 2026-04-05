impl ZipFile {
    #[cfg(test)]
    fn read_local_header_offsets(zip_path: &str) -> Option<Vec<u64>> {
        let zip_bytes = fs::read(zip_path).ok()?;
        let eocd = Self::locate_eocd(&zip_bytes)?;
        let central_directory_size = eocd.central_directory_size as usize;
        let central_directory_offset = eocd.central_directory_offset as usize;
        let correction = eocd.central_directory_offset_correction as i128;
        if central_directory_offset + central_directory_size > zip_bytes.len() {
            return None;
        }

        let mut local_offsets = Vec::new();
        let mut central_offset = central_directory_offset;
        while central_offset + 46 <= central_directory_offset + central_directory_size {
            if zip_bytes[central_offset..central_offset + 4] != [0x50, 0x4B, 0x01, 0x02] {
                return None;
            }

            let compressed_size_u32 = u32::from_le_bytes([
                zip_bytes[central_offset + 20],
                zip_bytes[central_offset + 21],
                zip_bytes[central_offset + 22],
                zip_bytes[central_offset + 23],
            ]);
            let uncompressed_size_u32 = u32::from_le_bytes([
                zip_bytes[central_offset + 24],
                zip_bytes[central_offset + 25],
                zip_bytes[central_offset + 26],
                zip_bytes[central_offset + 27],
            ]);
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
            let relative_offset_u32 = u32::from_le_bytes([
                zip_bytes[central_offset + 42],
                zip_bytes[central_offset + 43],
                zip_bytes[central_offset + 44],
                zip_bytes[central_offset + 45],
            ]);

            let name_end = central_offset + 46 + file_name_length;
            let extra_end = name_end + extra_length;
            if extra_end > zip_bytes.len() {
                return None;
            }
            let extra = &zip_bytes[name_end..extra_end];
            let extra_info = zip_extra_field::parse_extra_fields(
                extra,
                true,
                uncompressed_size_u32,
                compressed_size_u32,
                relative_offset_u32,
            );

            let base_offset_u64 = if relative_offset_u32 == 0xFFFF_FFFF {
                extra_info.local_header_offset?
            } else {
                relative_offset_u32 as u64
            };
            let relative_offset_u64_i128 = (base_offset_u64 as i128).saturating_add(correction);
            if relative_offset_u64_i128 < 0 || relative_offset_u64_i128 > u64::MAX as i128 {
                return None;
            }
            let relative_offset_u64 = relative_offset_u64_i128 as u64;

            local_offsets.push(relative_offset_u64);
            central_offset += 46 + file_name_length + extra_length + comment_length;
        }

        Some(local_offsets)
    }

    fn read_local_file_header_at(
        file: &mut File,
        local_index_offset: u64,
    ) -> Result<LocalFileHeaderInfo, ZipReturn> {
        if file.seek(SeekFrom::Start(local_index_offset)).is_err() {
            return Err(ZipReturn::ZipErrorReadingFile);
        }

        let mut header = [0u8; 30];
        if file.read_exact(&mut header).is_err() {
            return Err(ZipReturn::ZipLocalFileHeaderError);
        }
        if header[0..4] != [0x50, 0x4B, 0x03, 0x04] {
            return Err(ZipReturn::ZipLocalFileHeaderError);
        }

        let flags = u16::from_le_bytes([header[6], header[7]]);
        let compression_method = u16::from_le_bytes([header[8], header[9]]);
        let compressed_size_u32 =
            u32::from_le_bytes([header[18], header[19], header[20], header[21]]);
        let uncompressed_size_u32 =
            u32::from_le_bytes([header[22], header[23], header[24], header[25]]);
        let file_name_length = u16::from_le_bytes([header[26], header[27]]) as u64;
        let extra_length = u16::from_le_bytes([header[28], header[29]]) as u64;

        let extra_start = local_index_offset
            .saturating_add(30)
            .saturating_add(file_name_length);
        let extra_end = extra_start.saturating_add(extra_length);
        let data_offset = extra_end;

        let mut extra = vec![0u8; extra_length as usize];
        if extra_length > 0 {
            if file.seek(SeekFrom::Start(extra_start)).is_err() {
                return Err(ZipReturn::ZipErrorReadingFile);
            }
            if file.read_exact(&mut extra).is_err() {
                return Err(ZipReturn::ZipErrorReadingFile);
            }
        }

        let extra_info = zip_extra_field::parse_extra_fields(
            &extra,
            false,
            uncompressed_size_u32,
            compressed_size_u32,
            0,
        );

        let compressed_size = if compressed_size_u32 == 0xFFFF_FFFF {
            extra_info
                .compressed_size
                .ok_or(ZipReturn::ZipLocalFileHeaderError)?
        } else {
            compressed_size_u32 as u64
        };
        let uncompressed_size = if uncompressed_size_u32 == 0xFFFF_FFFF {
            extra_info
                .uncompressed_size
                .ok_or(ZipReturn::ZipLocalFileHeaderError)?
        } else {
            uncompressed_size_u32 as u64
        };

        Ok(LocalFileHeaderInfo {
            flags,
            compression_method,
            compressed_size,
            uncompressed_size,
            data_offset,
        })
    }

    pub fn zip_file_open_read_stream_ex(
        &mut self,
        index: usize,
        raw: bool,
    ) -> Result<(Box<dyn Read>, u64, u16), ZipReturn> {
        if self.zip_open_type != ZipOpenType::OpenRead {
            return Err(ZipReturn::ZipReadingFromOutputFile);
        }
        let local_head = self
            .get_file_header(index)
            .and_then(|h| h.local_head)
            .ok_or(ZipReturn::ZipCannotFastOpen)?;
        self.zip_file_open_read_stream_from_local_header_pointer(local_head, raw)
    }

    pub fn zip_file_open_read_stream_from_local_header_pointer(
        &mut self,
        local_index_offset: u64,
        raw: bool,
    ) -> Result<(Box<dyn Read>, u64, u16), ZipReturn> {
        if self.zip_open_type != ZipOpenType::OpenRead {
            return Err(ZipReturn::ZipReadingFromOutputFile);
        }

        let mut file = File::open(&self.zip_filename).map_err(|_| ZipReturn::ZipErrorOpeningFile)?;
        let info = Self::read_local_file_header_at(&mut file, local_index_offset)?;
        if (info.flags & 8) == 8 {
            return Err(ZipReturn::ZipCannotFastOpen);
        }

        if file.seek(SeekFrom::Start(info.data_offset)).is_err() {
            return Err(ZipReturn::ZipErrorReadingFile);
        }
        let mut compressed = vec![0u8; info.compressed_size as usize];
        if file.read_exact(&mut compressed).is_err() {
            return Err(ZipReturn::ZipErrorReadingFile);
        }

        if raw {
            return Ok((
                Box::new(std::io::Cursor::new(compressed)),
                info.compressed_size,
                info.compression_method,
            ));
        }

        match info.compression_method {
            0 => Ok((
                Box::new(std::io::Cursor::new(compressed)),
                info.uncompressed_size,
                info.compression_method,
            )),
            8 => {
                let mut decoder = DeflateDecoder::new(std::io::Cursor::new(compressed));
                let mut out = Vec::with_capacity(info.uncompressed_size as usize);
                if decoder.read_to_end(&mut out).is_err() {
                    return Err(ZipReturn::ZipErrorGettingDataStream);
                }
                Ok((
                    Box::new(std::io::Cursor::new(out)),
                    info.uncompressed_size,
                    info.compression_method,
                ))
            }
            9 => {
                let cursor = std::io::Cursor::new(compressed);
                let mut decoder = Deflate64Decoder::new(BufReader::new(cursor));
                let mut out = Vec::with_capacity(info.uncompressed_size as usize);
                if decoder.read_to_end(&mut out).is_err() {
                    return Err(ZipReturn::ZipErrorGettingDataStream);
                }
                Ok((
                    Box::new(std::io::Cursor::new(out)),
                    info.uncompressed_size,
                    info.compression_method,
                ))
            }
            12 => {
                let mut decoder = BzDecoder::new(std::io::Cursor::new(compressed));
                let mut out = Vec::with_capacity(info.uncompressed_size as usize);
                if decoder.read_to_end(&mut out).is_err() {
                    return Err(ZipReturn::ZipErrorGettingDataStream);
                }
                Ok((
                    Box::new(std::io::Cursor::new(out)),
                    info.uncompressed_size,
                    info.compression_method,
                ))
            }
            93 => {
                let decoder = ZstdDecoder::new(std::io::Cursor::new(compressed))
                    .map_err(|_| ZipReturn::ZipErrorGettingDataStream)?;
                let mut decoder = decoder;
                let mut out = Vec::with_capacity(info.uncompressed_size as usize);
                if decoder.read_to_end(&mut out).is_err() {
                    return Err(ZipReturn::ZipErrorGettingDataStream);
                }
                Ok((
                    Box::new(std::io::Cursor::new(out)),
                    info.uncompressed_size,
                    info.compression_method,
                ))
            }
            _ => Err(ZipReturn::ZipUnsupportedCompression),
        }
    }

    fn read_central_directory_from_bytes(
        zip_bytes: &[u8],
    ) -> Option<(Vec<FileHeader>, Vec<CentralHeaderMeta>)> {
        let eocd = Self::locate_eocd(zip_bytes)?;
        let central_directory_size = eocd.central_directory_size as usize;
        let central_directory_offset = eocd.central_directory_offset as usize;
        let correction = eocd.central_directory_offset_correction as i128;
        if central_directory_offset + central_directory_size > zip_bytes.len() {
            return None;
        }

        let mut out = Vec::new();
        let mut meta = Vec::new();
        if eocd.local_files_count <= (usize::MAX as u64) {
            out.reserve(eocd.local_files_count as usize);
            meta.reserve(eocd.local_files_count as usize);
        }
        let mut central_offset = central_directory_offset;
        let central_end = central_directory_offset + central_directory_size;
        while central_offset + 46 <= central_end {
            if zip_bytes[central_offset..central_offset + 4] != [0x50, 0x4B, 0x01, 0x02] {
                return None;
            }

            let flags =
                u16::from_le_bytes([zip_bytes[central_offset + 8], zip_bytes[central_offset + 9]]);
            let compression_method = u16::from_le_bytes([
                zip_bytes[central_offset + 10],
                zip_bytes[central_offset + 11],
            ]);
            let last_mod_time = u16::from_le_bytes([
                zip_bytes[central_offset + 12],
                zip_bytes[central_offset + 13],
            ]);
            let last_mod_date = u16::from_le_bytes([
                zip_bytes[central_offset + 14],
                zip_bytes[central_offset + 15],
            ]);
            let crc32 = u32::from_le_bytes([
                zip_bytes[central_offset + 16],
                zip_bytes[central_offset + 17],
                zip_bytes[central_offset + 18],
                zip_bytes[central_offset + 19],
            ]);
            let compressed_size_u32 = u32::from_le_bytes([
                zip_bytes[central_offset + 20],
                zip_bytes[central_offset + 21],
                zip_bytes[central_offset + 22],
                zip_bytes[central_offset + 23],
            ]);
            let uncompressed_size_u32 = u32::from_le_bytes([
                zip_bytes[central_offset + 24],
                zip_bytes[central_offset + 25],
                zip_bytes[central_offset + 26],
                zip_bytes[central_offset + 27],
            ]);
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
            let relative_offset_u32 = u32::from_le_bytes([
                zip_bytes[central_offset + 42],
                zip_bytes[central_offset + 43],
                zip_bytes[central_offset + 44],
                zip_bytes[central_offset + 45],
            ]);

            let name_start = central_offset + 46;
            let name_end = name_start + file_name_length;
            let extra_end = name_end + extra_length;
            let record_end = extra_end + comment_length;
            if record_end > zip_bytes.len() {
                return None;
            }

            let file_name_bytes = &zip_bytes[name_start..name_end];
            let name = Self::decode_filename(file_name_bytes, flags)?;
            let is_directory = name.ends_with('/');

            let extra = &zip_bytes[name_end..extra_end];
            let extra_info = zip_extra_field::parse_extra_fields(
                extra,
                true,
                uncompressed_size_u32,
                compressed_size_u32,
                relative_offset_u32,
            );

            let base_relative_offset = if relative_offset_u32 == 0xFFFF_FFFF {
                extra_info.local_header_offset?
            } else {
                relative_offset_u32 as u64
            };
            let relative_offset_i128 = (base_relative_offset as i128).saturating_add(correction);
            if relative_offset_i128 < 0 || relative_offset_i128 > u64::MAX as i128 {
                return None;
            }
            let relative_offset = relative_offset_i128 as u64;
            let compressed_size = if compressed_size_u32 == 0xFFFF_FFFF {
                extra_info.compressed_size?
            } else {
                compressed_size_u32 as u64
            };
            let uncompressed_size = if uncompressed_size_u32 == 0xFFFF_FFFF {
                extra_info.uncompressed_size?
            } else {
                uncompressed_size_u32 as u64
            };

            let header_last_modified = ((last_mod_date as i64) << 16) | (last_mod_time as i64);

            let mut fh = FileHeader::new();
            fh.filename = name;
            fh.uncompressed_size = uncompressed_size;
            fh.is_directory = is_directory;
            fh.crc = Some(crc32.to_be_bytes().to_vec());
            fh.local_head = if (flags & 8) == 0 {
                Some(relative_offset)
            } else {
                None
            };
            fh.header_last_modified = header_last_modified;
            fh.modified_time = extra_info.modified_time_ticks;
            fh.accessed_time = extra_info.accessed_time_ticks;
            fh.created_time = extra_info.created_time_ticks;

            out.push(fh);
            meta.push(CentralHeaderMeta {
                flags,
                compression_method,
                compressed_size,
                uncompressed_size,
                crc32,
                local_header_offset: relative_offset,
                header_last_modified,
            });
            central_offset = record_end;
        }

        Some((out, meta))
    }

    fn read_central_directory(zip_path: &str) -> Option<(Vec<FileHeader>, Vec<CentralHeaderMeta>)> {
        let zip_bytes = fs::read(zip_path).ok()?;
        Self::read_central_directory_from_bytes(&zip_bytes)
    }

    fn read_local_file_header_full_from_bytes(
        zip_bytes: &[u8],
        local_offset: u64,
        central: &CentralHeaderMeta,
    ) -> Result<LocalHeaderFull, ZipReturn> {
        let local_offset_usize = local_offset as usize;
        if local_offset_usize + 30 > zip_bytes.len() {
            return Err(ZipReturn::ZipLocalFileHeaderError);
        }
        if zip_bytes[local_offset_usize..local_offset_usize + 4] != [0x50, 0x4B, 0x03, 0x04] {
            return Err(ZipReturn::ZipLocalFileHeaderError);
        }

        let flags = u16::from_le_bytes([
            zip_bytes[local_offset_usize + 6],
            zip_bytes[local_offset_usize + 7],
        ]);
        let compression_method = u16::from_le_bytes([
            zip_bytes[local_offset_usize + 8],
            zip_bytes[local_offset_usize + 9],
        ]);
        let last_mod_time = u16::from_le_bytes([
            zip_bytes[local_offset_usize + 10],
            zip_bytes[local_offset_usize + 11],
        ]);
        let last_mod_date = u16::from_le_bytes([
            zip_bytes[local_offset_usize + 12],
            zip_bytes[local_offset_usize + 13],
        ]);
        let header_last_modified = ((last_mod_date as i64) << 16) | (last_mod_time as i64);

        let crc32_local = u32::from_le_bytes([
            zip_bytes[local_offset_usize + 14],
            zip_bytes[local_offset_usize + 15],
            zip_bytes[local_offset_usize + 16],
            zip_bytes[local_offset_usize + 17],
        ]);
        let compressed_size_u32 = u32::from_le_bytes([
            zip_bytes[local_offset_usize + 18],
            zip_bytes[local_offset_usize + 19],
            zip_bytes[local_offset_usize + 20],
            zip_bytes[local_offset_usize + 21],
        ]);
        let uncompressed_size_u32 = u32::from_le_bytes([
            zip_bytes[local_offset_usize + 22],
            zip_bytes[local_offset_usize + 23],
            zip_bytes[local_offset_usize + 24],
            zip_bytes[local_offset_usize + 25],
        ]);
        let file_name_length = u16::from_le_bytes([
            zip_bytes[local_offset_usize + 26],
            zip_bytes[local_offset_usize + 27],
        ]) as usize;
        let extra_length = u16::from_le_bytes([
            zip_bytes[local_offset_usize + 28],
            zip_bytes[local_offset_usize + 29],
        ]) as usize;

        let name_start = local_offset_usize + 30;
        let name_end = name_start + file_name_length;
        let extra_end = name_end + extra_length;
        if extra_end > zip_bytes.len() {
            return Err(ZipReturn::ZipLocalFileHeaderError);
        }

        let file_name_bytes = &zip_bytes[name_start..name_end];
        let filename = Self::decode_filename(file_name_bytes, flags)
            .ok_or(ZipReturn::ZipLocalFileHeaderError)?;

        let extra_bytes = &zip_bytes[name_end..extra_end];
        let extra_info = zip_extra_field::parse_extra_fields(
            extra_bytes,
            false,
            uncompressed_size_u32,
            compressed_size_u32,
            0,
        );

        let mut compressed_size = if compressed_size_u32 == 0xFFFF_FFFF {
            extra_info
                .compressed_size
                .ok_or(ZipReturn::ZipLocalFileHeaderError)?
        } else {
            compressed_size_u32 as u64
        };
        let mut uncompressed_size = if uncompressed_size_u32 == 0xFFFF_FFFF {
            extra_info
                .uncompressed_size
                .ok_or(ZipReturn::ZipLocalFileHeaderError)?
        } else {
            uncompressed_size_u32 as u64
        };

        let mut crc32 = crc32_local;
        if (flags & 8) == 8 {
            crc32 = central.crc32;
            compressed_size = central.compressed_size;
            uncompressed_size = central.uncompressed_size;
        }

        let data_offset = extra_end as u64;

        Ok(LocalHeaderFull {
            flags,
            compression_method,
            crc32,
            compressed_size,
            uncompressed_size,
            header_last_modified,
            filename,
            data_offset,
        })
    }

    fn read_headers(&mut self) -> ZipReturn {
        let archive = match self.archive.as_mut() {
            Some(a) => a,
            None => return ZipReturn::ZipErrorOpeningFile,
        };

        self.file_headers.clear();
        self.central_meta.clear();

        let zip_bytes = self
            .zip_memory
            .clone()
            .or_else(|| fs::read(&self.zip_filename).ok());
        self.file_comment = zip_bytes
            .as_ref()
            .and_then(|b| Self::locate_eocd(b))
            .map(|e| codepage_437::decode(&e.comment_bytes))
            .unwrap_or_else(|| String::from_utf8_lossy(archive.comment()).to_string());

        let parsed = match zip_bytes
            .as_deref()
            .and_then(Self::read_central_directory_from_bytes)
            .or_else(|| {
                if self.zip_filename.is_empty() {
                    None
                } else {
                    Self::read_central_directory(&self.zip_filename)
                }
            }) {
            Some(v) => v,
            None => return ZipReturn::ZipCentralDirError,
        };
        self.file_headers = parsed.0;
        self.central_meta = parsed.1;

        let Some(zip_bytes) = zip_bytes.as_deref() else {
            return ZipReturn::ZipErrorReadingFile;
        };

        if self.file_headers.len() != self.central_meta.len() {
            return ZipReturn::ZipCentralDirError;
        }

        for central in &self.central_meta {
            let local = match Self::read_local_file_header_full_from_bytes(
                zip_bytes,
                central.local_header_offset,
                central,
            ) {
                Ok(v) => v,
                Err(z) => return z,
            };

            if central.compression_method != local.compression_method {
                return ZipReturn::ZipLocalFileHeaderError;
            }

            if !matches!(
                central.compression_method,
                0 | 1 | 2 | 3 | 4 | 5 | 6 | 8 | 9 | 12 | 14 | 20 | 93 | 98
            ) {
                return ZipReturn::ZipUnsupportedCompression;
            }

            if central.crc32 != local.crc32 {
                return ZipReturn::ZipLocalFileHeaderError;
            }
            if central.compressed_size != local.compressed_size {
                return ZipReturn::ZipLocalFileHeaderError;
            }
            if central.uncompressed_size != local.uncompressed_size {
                return ZipReturn::ZipLocalFileHeaderError;
            }
        }

        ZipReturn::ZipGood
    }
}
