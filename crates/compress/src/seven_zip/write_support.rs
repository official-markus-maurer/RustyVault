impl SevenZipFile {
    pub fn zip_file_create_with_structure(
        &mut self,
        new_filename: &str,
        zip_struct: ZipStructure,
    ) -> ZipReturn {
        if self.zip_open_type != ZipOpenType::Closed {
            return ZipReturn::ZipFileAlreadyOpen;
        }

        let path = Path::new(new_filename);
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() && fs::create_dir_all(parent).is_err() {
                return ZipReturn::ZipErrorOpeningFile;
            }
        }

        let staging_dir = PathBuf::from(format!("{}.rv7z.dir", new_filename));
        let _ = fs::remove_dir_all(&staging_dir);
        if fs::create_dir_all(&staging_dir).is_err() {
            return ZipReturn::ZipErrorOpeningFile;
        }

        self.zip_filename = new_filename.to_string();
        self.zip_open_type = ZipOpenType::OpenWrite;
        self.zip_struct = zip_struct;
        self.file_headers.clear();
        self.file_comment.clear();
        self.archive = None;
        self.file = None;
        self.staging_dir = Some(staging_dir);
        self.pending_write = None;

        ZipReturn::ZipGood
    }

    fn expected_compression_for_struct(zip_struct: ZipStructure) -> u16 {
        match zip_struct {
            ZipStructure::SevenZipSZSTD | ZipStructure::SevenZipNZSTD => 93,
            _ => 14,
        }
    }

    fn finalize_write(&mut self) -> ZipReturn {
        let Some(staging_dir) = self.staging_dir.as_ref() else {
            return ZipReturn::ZipErrorOpeningFile;
        };
        if self.zip_filename.is_empty() {
            return ZipReturn::ZipErrorOpeningFile;
        }

        let temp_path = format!("{}.rv7z.tmp", self.zip_filename);
        let _ = fs::remove_file(&temp_path);
        let mut planned: Vec<(String, bool, PathBuf)> = Vec::new();
        for fh in &self.file_headers {
            let mut name = fh.filename.replace('\\', "/");
            if name.is_empty() || name == "/" {
                continue;
            }
            if fh.is_directory && !name.ends_with('/') {
                name.push('/');
            }
            let disk_path = if fh.is_directory {
                staging_dir.join(name.trim_end_matches('/'))
            } else {
                staging_dir.join(&name)
            };
            planned.push((name, fh.is_directory, disk_path));
        }

        let mut dir_has_children: HashMap<String, bool> = HashMap::new();
        for (name, is_dir, _) in &planned {
            if *is_dir {
                continue;
            }
            if let Some(idx) = name.rfind('/') {
                dir_has_children.insert(format!("{}/", &name[..idx]), true);
            }
        }
        planned.retain(|(name, is_dir, _)| {
            if !*is_dir {
                return true;
            }
            !dir_has_children.get(name).copied().unwrap_or(false)
        });

        fn split_7zip_filename(filename: &str) -> (&str, &str, &str) {
            let dir_index = filename.rfind('/');
            let (path, name) = if let Some(i) = dir_index {
                (&filename[..i], &filename[i + 1..])
            } else {
                ("", filename)
            };
            let ext_index = name.rfind('.');
            if let Some(i) = ext_index {
                (path, &name[..i], &name[i + 1..])
            } else {
                (path, name, "")
            }
        }
        planned.sort_by(|(a, _, _), (b, _, _)| {
            let (path_a, name_a, ext_a) = split_7zip_filename(a);
            let (path_b, name_b, ext_b) = split_7zip_filename(b);
            let res = ext_a.cmp(ext_b);
            if res != std::cmp::Ordering::Equal {
                return res;
            }
            let res = name_a.cmp(name_b);
            if res != std::cmp::Ordering::Equal {
                return res;
            }
            path_a.cmp(path_b)
        });
        for i in 0..planned.len().saturating_sub(1) {
            if planned[i].0 == planned[i + 1].0 {
                let _ = fs::remove_file(&temp_path);
                return ZipReturn::ZipErrorWritingToOutputStream;
            }
        }

        let out_file = match File::create(&temp_path) {
            Ok(f) => f,
            Err(_) => {
                let _ = fs::remove_file(&temp_path);
                return ZipReturn::ZipErrorWritingToOutputStream;
            }
        };
        let mut writer = match ArchiveWriter::new(out_file) {
            Ok(w) => w,
            Err(_) => {
                let _ = fs::remove_file(&temp_path);
                return ZipReturn::ZipErrorWritingToOutputStream;
            }
        };
        writer.set_encrypt_header(false);

        let solid = matches!(
            self.zip_struct,
            ZipStructure::SevenZipSLZMA | ZipStructure::SevenZipSZSTD
        );
        if solid {
            let config = match self.zip_struct {
                ZipStructure::SevenZipSZSTD | ZipStructure::SevenZipNZSTD => {
                    EncoderConfiguration::new(EncoderMethod::ZSTD)
                        .with_options(EncoderOptions::Zstd(ZstandardOptions::from_level(19)))
                }
                _ => {
                    let mut lz = LzmaOptions::from_level(9);
                    lz.set_dictionary_size(1 << 24);
                    lz.set_num_fast_bytes(64);
                    lz.set_lc(4);
                    lz.set_lp(0);
                    lz.set_pb(2);
                    lz.set_mode_normal();
                    lz.set_match_finder_bt4();
                    EncoderConfiguration::new(EncoderMethod::LZMA).with_options(EncoderOptions::Lzma(lz))
                }
            };
            writer.set_content_methods(vec![config]);

            for (name, _is_dir, _) in planned.iter().filter(|(_, is_dir, _)| *is_dir) {
                if writer
                    .push_archive_entry::<&[u8]>(ArchiveEntry::new_directory(name), None)
                    .is_err()
                {
                    let _ = fs::remove_file(&temp_path);
                    return ZipReturn::ZipErrorWritingToOutputStream;
                }
            }

            let mut file_entries = Vec::new();
            let mut readers: Vec<SourceReader<File>> = Vec::new();
            for (name, _is_dir, disk_path) in planned.iter().filter(|(_, is_dir, _)| !*is_dir) {
                let Ok(src) = File::open(disk_path) else {
                    let _ = fs::remove_file(&temp_path);
                    return ZipReturn::ZipErrorWritingToOutputStream;
                };
                file_entries.push(ArchiveEntry::new_file(name));
                readers.push(SourceReader::new(src));
            }
            if !file_entries.is_empty() && writer.push_archive_entries(file_entries, readers).is_err() {
                let _ = fs::remove_file(&temp_path);
                return ZipReturn::ZipErrorWritingToOutputStream;
            }
        } else {
            for (name, is_dir, disk_path) in &planned {
                if *is_dir {
                    if writer
                        .push_archive_entry::<&[u8]>(ArchiveEntry::new_directory(name), None)
                        .is_err()
                    {
                        let _ = fs::remove_file(&temp_path);
                        return ZipReturn::ZipErrorWritingToOutputStream;
                    }
                    continue;
                }

                let Ok(src) = File::open(disk_path) else {
                    let _ = fs::remove_file(&temp_path);
                    return ZipReturn::ZipErrorWritingToOutputStream;
                };
                let config = match self.zip_struct {
                    ZipStructure::SevenZipSZSTD | ZipStructure::SevenZipNZSTD => {
                        EncoderConfiguration::new(EncoderMethod::ZSTD)
                            .with_options(EncoderOptions::Zstd(ZstandardOptions::from_level(19)))
                    }
                    _ => {
                        let mut lz = LzmaOptions::from_level(9);
                        lz.set_dictionary_size(seven_zip_dictionary_size_from_uncompressed_size(
                            src.metadata().map(|m| m.len()).unwrap_or(0),
                        ));
                        lz.set_num_fast_bytes(64);
                        lz.set_lc(4);
                        lz.set_lp(0);
                        lz.set_pb(2);
                        lz.set_mode_normal();
                        lz.set_match_finder_bt4();
                        EncoderConfiguration::new(EncoderMethod::LZMA).with_options(EncoderOptions::Lzma(lz))
                    }
                };
                writer.set_content_methods(vec![config]);
                if writer
                    .push_archive_entry(ArchiveEntry::new_file(name), Some(src))
                    .is_err()
                {
                    let _ = fs::remove_file(&temp_path);
                    return ZipReturn::ZipErrorWritingToOutputStream;
                }
            }
        }

        if writer.finish().is_err() {
            let _ = fs::remove_file(&temp_path);
            return ZipReturn::ZipErrorWritingToOutputStream;
        }

        let _ = apply_romvault7z_marker(Path::new(&temp_path), self.zip_struct);

        let _ = fs::remove_file(&self.zip_filename);
        if fs::rename(&temp_path, &self.zip_filename).is_err() {
            if fs::copy(&temp_path, &self.zip_filename).is_err() {
                let _ = fs::remove_file(&temp_path);
                return ZipReturn::ZipErrorWritingToOutputStream;
            }
            let _ = fs::remove_file(&temp_path);
        }

        ZipReturn::ZipGood
    }
}
