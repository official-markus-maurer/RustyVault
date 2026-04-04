impl Scanner {
    pub fn scan_archive_file(
        archive_type: FileType,
        filename: &str,
        time_stamp: i64,
        deep_scan: bool,
    ) -> Result<ScannedFile, ZipReturn> {
        let mut file: Box<dyn ICompress> = match archive_type {
            FileType::Zip => Box::new(ZipFile::new()),
            FileType::SevenZip => Box::new(SevenZipFile::new()),
            _ => Box::new(RawFile::new()),
        };

        let zr = file.zip_file_open(filename, time_stamp, true);
        if zr != ZipReturn::ZipGood {
            return Err(zr);
        }

        let mut scanned_archive = ScannedFile::new(archive_type);
        scanned_archive.name = filename.to_string();
        let z_struct = file.zip_struct();
        scanned_archive.zip_struct = match z_struct {
            compress::ZipStructure::ZipTrrnt => dat_reader::enums::ZipStructure::ZipTrrnt,
            compress::ZipStructure::ZipTDC => dat_reader::enums::ZipStructure::ZipTDC,
            compress::ZipStructure::SevenZipTrrnt => dat_reader::enums::ZipStructure::SevenZipTrrnt,
            compress::ZipStructure::ZipZSTD => dat_reader::enums::ZipStructure::ZipZSTD,
            compress::ZipStructure::SevenZipSLZMA => dat_reader::enums::ZipStructure::SevenZipSLZMA,
            compress::ZipStructure::SevenZipNLZMA => dat_reader::enums::ZipStructure::SevenZipNLZMA,
            compress::ZipStructure::SevenZipSZSTD => dat_reader::enums::ZipStructure::SevenZipSZSTD,
            compress::ZipStructure::SevenZipNZSTD => dat_reader::enums::ZipStructure::SevenZipNZSTD,
            _ => dat_reader::enums::ZipStructure::None,
        };
        scanned_archive.comment = file.file_comment().to_string();

        let files = Self::scan_files_in_archive(file.as_mut(), deep_scan);
        scanned_archive.children = files;

        file.zip_file_close();
        Ok(scanned_archive)
    }

    fn scan_files_in_archive(file: &mut dyn ICompress, deep_scan: bool) -> Vec<ScannedFile> {
        let file_count = file.local_files_count();
        let scanned_file_type = FileType::File;

        let mut file_headers = Vec::with_capacity(file_count);
        for i in 0..file_count {
            if let Some(lf) = file.get_file_header(i) {
                file_headers.push((i, lf.clone()));
            }
        }

        let mut results = Vec::with_capacity(file_headers.len());

        for (i, lf) in file_headers {
            let mut scanned_file = ScannedFile::new(scanned_file_type);
            let mut do_deep_scan = false;
            let mut lf_crc = None;
            let mut _lf_is_dir = false;

            scanned_file.name = lf.filename.clone();
            scanned_file.deep_scanned = deep_scan;
            scanned_file.index = i as i32;
            scanned_file.local_header_offset = lf.local_head;
            scanned_file.file_mod_time_stamp = lf.last_modified();

            _lf_is_dir = lf.is_directory;

            if lf.is_directory {
                scanned_file.header_file_type = HeaderFileType::NOTHING;
                scanned_file.got_status = GotStatus::Got;
                scanned_file.size = Some(0);
                scanned_file.crc = Some(vec![0, 0, 0, 0]);
                scanned_file.sha1 = Some(vec![
                    0xda, 0x39, 0xa3, 0xee, 0x5e, 0x6b, 0x4b, 0x0d, 0x32, 0x55, 0xbf, 0xef, 0x95, 0x60,
                    0x18, 0x90, 0xaf, 0xd8, 0x07, 0x09,
                ]);
                scanned_file.md5 = Some(vec![
                    0xd4, 0x1d, 0x8c, 0xd9, 0x8f, 0x00, 0xb2, 0x04, 0xe9, 0x80, 0x09, 0x98, 0xec, 0xf8,
                    0x42, 0x7e,
                ]);
                scanned_file.sha256 = Some(vec![
                    0xe3, 0xb0, 0xc4, 0x42, 0x98, 0xfc, 0x1c, 0x14, 0x9a, 0xfb, 0xf4, 0xc8, 0x99, 0x6f,
                    0xb9, 0x24, 0x27, 0xae, 0x41, 0xe4, 0x64, 0x9b, 0x93, 0x4c, 0xa4, 0x95, 0x99, 0x1b,
                    0x78, 0x52, 0xb8, 0x55,
                ]);
            } else {
                scanned_file.size = Some(lf.uncompressed_size);
                scanned_file.crc = lf.crc.clone();
                lf_crc = lf.crc.clone();
                do_deep_scan = deep_scan;
                scanned_file.status_flags.insert(FileStatus::SIZE_FROM_HEADER);
                if scanned_file.crc.is_some() {
                    scanned_file.status_flags.insert(FileStatus::CRC_FROM_HEADER);
                }
            }

            if !_lf_is_dir {
                if !do_deep_scan {
                    scanned_file.got_status = GotStatus::Got;
                    let stream_res = file.zip_file_open_read_stream(i);
                    match stream_res {
                        Ok((mut stream, _size)) => {
                            let mut alt_crc_hasher = Crc32Hasher::new();
                            let mut header_probe = Vec::with_capacity(512);
                            let mut header_file_type = HeaderFileType::NOTHING;
                            let mut header_size = 0usize;
                            let mut total_read = 0usize;

                            let mut buffer = [0u8; 32768];
                            loop {
                                match stream.read(&mut buffer) {
                                    Ok(0) => break,
                                    Ok(n) => {
                                        if header_probe.len() < 512 {
                                            let probe_take = std::cmp::min(512 - header_probe.len(), n);
                                            header_probe.extend_from_slice(&buffer[..probe_take]);
                                            let (detected_type, detected_size) =
                                                FileHeaders::get_file_type_from_buffer(&header_probe);
                                            header_file_type = detected_type;
                                            header_size = detected_size;
                                            if header_file_type == HeaderFileType::CHD
                                                && crate::settings::get_settings().check_chd_version
                                                && scanned_file.chd_version.is_none()
                                            {
                                                if let Some(info) = parse_chd_header_from_bytes(&header_probe) {
                                                    scanned_file.chd_version = Some(info.version);
                                                    if let Some(sha1) = info.sha1 {
                                                        scanned_file.alt_sha1 = Some(sha1);
                                                        scanned_file
                                                            .status_flags
                                                            .insert(FileStatus::ALT_SHA1_FROM_HEADER);
                                                    }
                                                    if let Some(md5) = info.md5 {
                                                        scanned_file.alt_md5 = Some(md5);
                                                        scanned_file
                                                            .status_flags
                                                            .insert(FileStatus::ALT_MD5_FROM_HEADER);
                                                    }
                                                }
                                            }
                                        }

                                        if header_size > 0 {
                                            let chunk_start = total_read;
                                            let chunk_end = total_read + n;
                                            if chunk_end > header_size {
                                                let alt_start = header_size.saturating_sub(chunk_start);
                                                alt_crc_hasher.update(&buffer[alt_start..n]);
                                            }
                                        }
                                        total_read += n;
                                    }
                                    Err(_) => {
                                        scanned_file.got_status = GotStatus::Corrupt;
                                        break;
                                    }
                                }
                            }

                            if scanned_file.got_status != GotStatus::Corrupt {
                                scanned_file.header_file_type = header_file_type;
                                if header_file_type != HeaderFileType::NOTHING {
                                    scanned_file
                                        .status_flags
                                        .insert(FileStatus::HEADER_FILE_TYPE_FROM_HEADER);
                                }
                                if header_size > 0 && scanned_file.size.unwrap_or(0) >= header_size as u64 {
                                    scanned_file.alt_size =
                                        Some(scanned_file.size.unwrap_or(0) - header_size as u64);
                                    scanned_file.alt_crc =
                                        Some(alt_crc_hasher.finalize().to_be_bytes().to_vec());
                                    scanned_file.status_flags.insert(
                                        FileStatus::ALT_SIZE_FROM_HEADER | FileStatus::ALT_CRC_FROM_HEADER,
                                    );
                                }
                            }

                            let _ = file.zip_file_close_read_stream();
                        }
                        Err(_) => {
                            scanned_file.got_status = GotStatus::Corrupt;
                            scanned_file.crc = lf_crc;
                        }
                    }
                } else {
                    let stream_res = file.zip_file_open_read_stream(i);
                    match stream_res {
                        Ok((mut stream, _size)) => {
                            let mut md5_hasher = Md5::new();
                            let mut sha1_hasher = Sha1::new();
                            let mut sha256_hasher = Sha256::new();
                            let mut crc_hasher = Crc32Hasher::new();
                            let mut alt_md5_hasher = Md5::new();
                            let mut alt_sha1_hasher = Sha1::new();
                            let mut alt_sha256_hasher = Sha256::new();
                            let mut alt_crc_hasher = Crc32Hasher::new();
                            let mut header_probe = Vec::with_capacity(512);
                            let mut header_file_type = HeaderFileType::NOTHING;
                            let mut header_size = 0usize;
                            let mut total_read = 0usize;

                            let mut buffer = [0u8; 32768];
                            loop {
                                match stream.read(&mut buffer) {
                                    Ok(0) => break,
                                    Ok(n) => {
                                        if header_probe.len() < 512 {
                                            let probe_take = std::cmp::min(512 - header_probe.len(), n);
                                            header_probe.extend_from_slice(&buffer[..probe_take]);
                                            let (detected_type, detected_size) =
                                                FileHeaders::get_file_type_from_buffer(&header_probe);
                                            header_file_type = detected_type;
                                            header_size = detected_size;
                                            if header_file_type == HeaderFileType::CHD
                                                && crate::settings::get_settings().check_chd_version
                                                && scanned_file.chd_version.is_none()
                                            {
                                                if let Some(info) = parse_chd_header_from_bytes(&header_probe) {
                                                    scanned_file.chd_version = Some(info.version);
                                                    if let Some(sha1) = info.sha1 {
                                                        scanned_file.alt_sha1 = Some(sha1);
                                                        scanned_file
                                                            .status_flags
                                                            .insert(FileStatus::ALT_SHA1_FROM_HEADER);
                                                    }
                                                    if let Some(md5) = info.md5 {
                                                        scanned_file.alt_md5 = Some(md5);
                                                        scanned_file
                                                            .status_flags
                                                            .insert(FileStatus::ALT_MD5_FROM_HEADER);
                                                    }
                                                }
                                            }
                                        }

                                        md5_hasher.update(&buffer[..n]);
                                        sha1_hasher.update(&buffer[..n]);
                                        sha256_hasher.update(&buffer[..n]);
                                        crc_hasher.update(&buffer[..n]);

                                        if header_size > 0 {
                                            let chunk_start = total_read;
                                            let chunk_end = total_read + n;
                                            if chunk_end > header_size {
                                                let alt_start = header_size.saturating_sub(chunk_start);
                                                alt_md5_hasher.update(&buffer[alt_start..n]);
                                                alt_sha1_hasher.update(&buffer[alt_start..n]);
                                                alt_sha256_hasher.update(&buffer[alt_start..n]);
                                                alt_crc_hasher.update(&buffer[alt_start..n]);
                                            }
                                        }
                                        total_read += n;
                                    }
                                    Err(_) => {
                                        scanned_file.got_status = GotStatus::Corrupt;
                                        break;
                                    }
                                }
                            }

                            if scanned_file.got_status != GotStatus::Corrupt {
                                let computed_crc = crc_hasher.finalize().to_be_bytes().to_vec();
                                if let Some(ref existing_crc) = scanned_file.crc {
                                    if existing_crc != &computed_crc {
                                        scanned_file.got_status = GotStatus::Corrupt;
                                    }
                                } else {
                                    scanned_file.crc = Some(computed_crc);
                                }
                                scanned_file.header_file_type = header_file_type;
                                if header_file_type != HeaderFileType::NOTHING {
                                    scanned_file
                                        .status_flags
                                        .insert(FileStatus::HEADER_FILE_TYPE_FROM_HEADER);
                                }
                                if header_size > 0 && scanned_file.size.unwrap_or(0) >= header_size as u64 {
                                    scanned_file.alt_size =
                                        Some(scanned_file.size.unwrap_or(0) - header_size as u64);
                                    scanned_file.alt_crc =
                                        Some(alt_crc_hasher.finalize().to_be_bytes().to_vec());
                                    scanned_file.alt_sha1 = Some(alt_sha1_hasher.finalize().to_vec());
                                    scanned_file.alt_md5 = Some(alt_md5_hasher.finalize().to_vec());
                                    scanned_file.alt_sha256 = Some(alt_sha256_hasher.finalize().to_vec());
                                    scanned_file.status_flags.insert(
                                        FileStatus::ALT_SIZE_FROM_HEADER
                                            | FileStatus::ALT_CRC_FROM_HEADER
                                            | FileStatus::ALT_SHA1_FROM_HEADER
                                            | FileStatus::ALT_MD5_FROM_HEADER
                                            | FileStatus::ALT_SHA256_FROM_HEADER,
                                    );
                                }
                                scanned_file.sha1 = Some(sha1_hasher.finalize().to_vec());
                                scanned_file.md5 = Some(md5_hasher.finalize().to_vec());
                                scanned_file.sha256 = Some(sha256_hasher.finalize().to_vec());
                                if scanned_file.got_status != GotStatus::Corrupt {
                                    scanned_file.got_status = GotStatus::Got;
                                }
                            }
                            let _ = file.zip_file_close_read_stream();
                        }
                        Err(_) => {
                            scanned_file.got_status = GotStatus::Corrupt;
                            scanned_file.crc = lf_crc;
                        }
                    }
                }
            }

            results.push(scanned_file);
        }

        results
    }
}
