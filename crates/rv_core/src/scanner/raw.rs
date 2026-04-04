impl Scanner {
    pub fn scan_raw_file(file_path: &str) -> Result<ScannedFile, std::io::Error> {
        let metadata = fs::metadata(file_path)?;
        let path = Path::new(file_path);
        let file_name = path.file_name().unwrap_or_default().to_string_lossy().to_string();

        let mut sf = ScannedFile::new(FileType::File);
        sf.name = file_name;
        if let Ok(mod_time) = metadata.modified() {
            if let Ok(dur) = mod_time.duration_since(std::time::UNIX_EPOCH) {
                sf.file_mod_time_stamp = dur.as_secs() as i64;
            }
        }
        sf.size = Some(metadata.len());

        let mut file = fs::File::open(file_path)?;
        let (header_file_type, header_size) =
            FileHeaders::get_file_type_from_stream(&mut file).unwrap_or((HeaderFileType::NOTHING, 0));
        file.seek(SeekFrom::Start(0))?;

        let mut md5_hasher = Md5::new();
        let mut sha1_hasher = Sha1::new();
        let mut sha256_hasher = Sha256::new();
        let mut crc_hasher = Crc32Hasher::new();
        let mut alt_md5_hasher = Md5::new();
        let mut alt_sha1_hasher = Sha1::new();
        let mut alt_sha256_hasher = Sha256::new();
        let mut alt_crc_hasher = Crc32Hasher::new();
        let mut total_read = 0usize;

        let mut buffer = [0u8; 8192];
        loop {
            let n = file.read(&mut buffer)?;
            if n == 0 {
                break;
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

        sf.header_file_type = header_file_type;
        if header_file_type != HeaderFileType::NOTHING {
            sf.status_flags.insert(FileStatus::HEADER_FILE_TYPE_FROM_HEADER);
        }
        sf.crc = Some(crc_hasher.finalize().to_be_bytes().to_vec());
        sf.sha1 = Some(sha1_hasher.finalize().to_vec());
        sf.md5 = Some(md5_hasher.finalize().to_vec());
        sf.sha256 = Some(sha256_hasher.finalize().to_vec());
        if header_size > 0 && metadata.len() >= header_size as u64 {
            sf.alt_size = Some(metadata.len() - header_size as u64);
            sf.alt_crc = Some(alt_crc_hasher.finalize().to_be_bytes().to_vec());
            sf.alt_sha1 = Some(alt_sha1_hasher.finalize().to_vec());
            sf.alt_md5 = Some(alt_md5_hasher.finalize().to_vec());
            sf.alt_sha256 = Some(alt_sha256_hasher.finalize().to_vec());
            sf.status_flags.insert(
                FileStatus::ALT_SIZE_FROM_HEADER
                    | FileStatus::ALT_CRC_FROM_HEADER
                    | FileStatus::ALT_SHA1_FROM_HEADER
                    | FileStatus::ALT_MD5_FROM_HEADER
                    | FileStatus::ALT_SHA256_FROM_HEADER,
            );
        }

        if header_file_type == HeaderFileType::CHD && crate::settings::get_settings().check_chd_version {
            file.seek(SeekFrom::Start(0))?;
            if let Some(info) = parse_chd_header_from_reader(&mut file) {
                sf.chd_version = Some(info.version);
                if let Some(sha1) = info.sha1 {
                    sf.alt_sha1 = Some(sha1);
                    sf.status_flags.insert(FileStatus::ALT_SHA1_FROM_HEADER);
                }
                if let Some(md5) = info.md5 {
                    sf.alt_md5 = Some(md5);
                    sf.status_flags.insert(FileStatus::ALT_MD5_FROM_HEADER);
                }
            } else {
                sf.got_status = GotStatus::Corrupt;
            }
        }
        sf.deep_scanned = true;
        if sf.got_status != GotStatus::Corrupt {
            sf.got_status = GotStatus::Got;
        }

        Ok(sf)
    }
}
