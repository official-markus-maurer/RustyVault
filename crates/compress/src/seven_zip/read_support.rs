impl SevenZipFile {
    pub fn zip_file_open_stream<R: Read + Seek>(
        &mut self,
        mut stream: R,
        read_headers: bool,
    ) -> ZipReturn {
        self.zip_file_close();
        let mut bytes = Vec::new();
        if stream.seek(SeekFrom::Start(0)).is_err() {
            return ZipReturn::ZipErrorOpeningFile;
        }
        if stream.read_to_end(&mut bytes).is_err() {
            return ZipReturn::ZipErrorReadingFile;
        }

        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .ok()
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let tmp_path = std::env::temp_dir().join(format!("rv_7z_stream_{}.7z", unique));
        if fs::write(&tmp_path, bytes).is_err() {
            return ZipReturn::ZipErrorOpeningFile;
        }

        self.temp_open_path = Some(tmp_path.clone());
        self.zip_file_open(tmp_path.to_string_lossy().as_ref(), 0, read_headers)
    }

    fn verify_next_header_crc(file: &mut File) -> ZipReturn {
        let mut sig = [0u8; 32];
        if file.seek(SeekFrom::Start(0)).is_err() {
            return ZipReturn::ZipErrorReadingFile;
        }
        if file.read_exact(&mut sig).is_err() {
            return ZipReturn::ZipErrorReadingFile;
        }
        if sig[0..6] != [0x37, 0x7A, 0xBC, 0xAF, 0x27, 0x1C] {
            return ZipReturn::ZipSignatureError;
        }

        let next_header_offset = u64::from_le_bytes(sig[12..20].try_into().unwrap());
        let next_header_size = u64::from_le_bytes(sig[20..28].try_into().unwrap());
        let next_header_crc = u32::from_le_bytes(sig[28..32].try_into().unwrap());

        let header_pos = 32u64.saturating_add(next_header_offset);
        let Ok(file_len) = file.metadata().map(|m| m.len()) else {
            return ZipReturn::ZipErrorReadingFile;
        };
        if header_pos > file_len {
            return ZipReturn::ZipErrorReadingFile;
        }
        if next_header_size > (file_len - header_pos) {
            return ZipReturn::ZipErrorReadingFile;
        }

        if next_header_size == 0 {
            return ZipReturn::ZipGood;
        }

        if file.seek(SeekFrom::Start(header_pos)).is_err() {
            return ZipReturn::ZipErrorReadingFile;
        }
        let mut header_bytes = vec![0u8; next_header_size as usize];
        if file.read_exact(&mut header_bytes).is_err() {
            return ZipReturn::ZipErrorReadingFile;
        }
        let mut hasher = Crc32Hasher::new();
        hasher.update(&header_bytes);
        if hasher.finalize() != next_header_crc {
            return ZipReturn::Zip64EndOfCentralDirectoryError;
        }
        ZipReturn::ZipGood
    }

    fn read_headers(&mut self) -> ZipReturn {
        let archive = match self.archive.as_ref() {
            Some(a) => a,
            None => return ZipReturn::ZipErrorOpeningFile,
        };

        self.file_headers.clear();

        for file in &archive.files {
            let mut fh = FileHeader::new();
            let mut name = file.name().to_string();
            if file.is_directory() && !name.ends_with('/') {
                name.push('/');
            }
            fh.filename = name;
            fh.uncompressed_size = file.size();
            fh.is_directory = file.is_directory();

            if fh.is_directory {
                fh.crc = Some(vec![0, 0, 0, 0]);
            } else if file.has_crc {
                fh.crc = Some((file.crc as u32).to_be_bytes().to_vec());
            }

            let set_time = |nt: sevenz_rust::NtTime| -> Option<i64> {
                let st: std::time::SystemTime = nt.into();
                st.duration_since(std::time::UNIX_EPOCH)
                    .ok()
                    .map(|d| d.as_secs() as i64)
            };

            if file.has_last_modified_date {
                fh.modified_time = set_time(file.last_modified_date());
            }
            if file.has_creation_date {
                fh.created_time = set_time(file.creation_date());
            }
            if file.has_access_date {
                fh.accessed_time = set_time(file.access_date());
            }

            self.file_headers.push(fh);
        }

        ZipReturn::ZipGood
    }

    fn detect_zip_structure(&self) -> ZipStructure {
        let Ok(mut file) = File::open(&self.zip_filename) else {
            return ZipStructure::None;
        };
        let Ok(metadata) = file.metadata() else {
            return ZipStructure::None;
        };
        let len = metadata.len();
        if len < 6 {
            return ZipStructure::None;
        }

        let mut signature = [0u8; 6];
        if file.read_exact(&mut signature).is_err() {
            return ZipStructure::None;
        }
        if signature != [0x37, 0x7A, 0xBC, 0xAF, 0x27, 0x1C] {
            return ZipStructure::None;
        }

        let rv = self.detect_romvault7z(&mut file, len);
        if rv != ZipStructure::None {
            return rv;
        }

        self.detect_torrent7z(&mut file, len)
    }

    fn detect_romvault7z(&self, file: &mut File, len: u64) -> ZipStructure {
        if len < 32 {
            return ZipStructure::None;
        }

        let mut signature = [0u8; 32];
        if file.seek(SeekFrom::Start(0)).is_err() {
            return ZipStructure::None;
        }
        if file.read_exact(&mut signature).is_err() {
            return ZipStructure::None;
        }

        let next_header_offset = u64::from_le_bytes(signature[12..20].try_into().unwrap());
        let next_header_size = u64::from_le_bytes(signature[20..28].try_into().unwrap());
        let next_header_crc = u32::from_le_bytes(signature[28..32].try_into().unwrap());
        let header_pos = 32u64.saturating_add(next_header_offset);
        if next_header_size == 0 || header_pos > len {
            return ZipStructure::None;
        }

        if header_pos < 32 {
            return ZipStructure::None;
        }

        if file.seek(SeekFrom::Start(header_pos - 32)).is_err() {
            return ZipStructure::None;
        }
        let mut rv_hdr = [0u8; 32];
        if file.read_exact(&mut rv_hdr).is_err() {
            return ZipStructure::None;
        }
        if rv_hdr[..11] != *b"RomVault7Z0" {
            return ZipStructure::None;
        }

        let stored_crc = u32::from_le_bytes(rv_hdr[12..16].try_into().unwrap());
        let stored_header_offset = u64::from_le_bytes(rv_hdr[16..24].try_into().unwrap());
        let stored_header_size = u64::from_le_bytes(rv_hdr[24..32].try_into().unwrap());

        if stored_crc != next_header_crc
            || stored_header_offset != header_pos
            || stored_header_size != next_header_size
        {
            return ZipStructure::None;
        }

        match rv_hdr[11] {
            b'1' => ZipStructure::SevenZipSLZMA,
            b'2' => ZipStructure::SevenZipNLZMA,
            b'3' => ZipStructure::SevenZipSZSTD,
            b'4' => ZipStructure::SevenZipNZSTD,
            _ => ZipStructure::None,
        }
    }

    fn detect_torrent7z(&self, file: &mut File, len: u64) -> ZipStructure {
        const CRC_SZ: usize = 128;
        const T7Z_SIG_SIZE: usize = 34;
        const T7Z_FOOTER_SIZE: usize = T7Z_SIG_SIZE + 4;
        const BUFFER_SIZE: usize = 256 + 8 + T7Z_FOOTER_SIZE;

        if len < (T7Z_FOOTER_SIZE as u64) {
            return ZipStructure::None;
        }

        let mut buffer = vec![0u8; BUFFER_SIZE];

        if file.seek(std::io::SeekFrom::Start(0)).is_err() {
            return ZipStructure::None;
        }
        let mut first = vec![0u8; CRC_SZ];
        let read_first = file.read(&mut first).unwrap_or(0);
        buffer[..read_first.min(CRC_SZ)].copy_from_slice(&first[..read_first.min(CRC_SZ)]);

        let footer_offset = len.saturating_sub(T7Z_FOOTER_SIZE as u64);
        let start_last = footer_offset.saturating_sub(CRC_SZ as u64);
        let last_len = (footer_offset - start_last) as usize;
        if file.seek(std::io::SeekFrom::Start(start_last)).is_err() {
            return ZipStructure::None;
        }
        let mut last_block = vec![0u8; last_len];
        if file.read_exact(&mut last_block).is_err() {
            return ZipStructure::None;
        }
        buffer[CRC_SZ..CRC_SZ + last_len].copy_from_slice(&last_block);

        if file.seek(std::io::SeekFrom::Start(footer_offset)).is_err() {
            return ZipStructure::None;
        }
        let mut footer = vec![0u8; T7Z_FOOTER_SIZE];
        if file.read_exact(&mut footer).is_err() {
            return ZipStructure::None;
        }

        buffer[256..264].copy_from_slice(&footer_offset.to_le_bytes());
        buffer[264..264 + T7Z_FOOTER_SIZE].copy_from_slice(&footer);

        let sig_header = b"\xA9\x9F\xD1\x57\x08\xA9\xD7\xEA\x29\x64\xB2\x36\x1B\x83\x52\x33\x01torrent7z_0.9beta";
        if footer.len() < 4 + sig_header.len() {
            return ZipStructure::None;
        }
        let mut expected = sig_header.to_vec();
        expected[16] = footer[4 + 16];
        if footer[4..4 + expected.len()] != expected {
            return ZipStructure::None;
        }

        let in_crc32 = u32::from_le_bytes(footer[0..4].try_into().unwrap());
        buffer[264..268].fill(0xFF);

        let mut crc = crc32fast::Hasher::new();
        crc.update(&buffer);
        let calc = crc.finalize();
        if in_crc32 == calc {
            ZipStructure::SevenZipTrrnt
        } else {
            ZipStructure::None
        }
    }
}
