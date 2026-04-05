impl Scanner {
    pub fn scan_directory(path_str: &str) -> Vec<ScannedFile> {
        Self::scan_directory_with_level(path_str, crate::settings::EScanLevel::Level1)
    }

    pub fn scan_directory_with_level(
        path_str: &str,
        scan_level: crate::settings::EScanLevel,
    ) -> Vec<ScannedFile> {
        Self::scan_directory_with_level_and_ignore(path_str, scan_level, &[])
    }

    pub fn scan_directory_with_level_and_ignore(
        path_str: &str,
        scan_level: crate::settings::EScanLevel,
        extra_ignore_patterns: &[String],
    ) -> Vec<ScannedFile> {
        let deep_scan = matches!(
            scan_level,
            crate::settings::EScanLevel::Level2 | crate::settings::EScanLevel::Level3
        );

        let mut ignore_patterns = crate::settings::get_settings().ignore_files.items.clone();
        ignore_patterns.extend_from_slice(extra_ignore_patterns);
        let ignore_matcher = Arc::new(crate::patterns::PatternMatcher::from_scan_ignore_patterns(
            &ignore_patterns,
        ));

        Self::scan_directory_impl(Path::new(path_str), deep_scan, &ignore_matcher)
    }

    fn scan_directory_impl(
        path: &Path,
        deep_scan: bool,
        ignore_matcher: &Arc<crate::patterns::PatternMatcher>,
    ) -> Vec<ScannedFile> {
        let Ok(entries) = fs::read_dir(path) else {
            return Vec::new();
        };

        fn has_ext_case_insensitive(name: &str, ext: &str) -> bool {
            name.len() >= ext.len() && name[name.len() - ext.len()..].eq_ignore_ascii_case(ext)
        }

        entries
            .par_bridge()
            .filter_map(|entry| {
                let entry = entry.ok()?;
                let metadata = entry.metadata().ok()?;
                let file_name = entry.file_name().to_string_lossy().to_string();

                if !metadata.is_dir() && file_name.starts_with("__RomVault.") && file_name.ends_with(".tmp")
                {
                    let _ = fs::remove_file(entry.path());
                    return None;
                }

                if is_ignored_file_name(&file_name, ignore_matcher) {
                    return None;
                }

                let file_type = if metadata.is_dir() {
                    FileType::Dir
                } else if has_ext_case_insensitive(&file_name, ".zip") {
                    FileType::Zip
                } else if has_ext_case_insensitive(&file_name, ".7z") {
                    FileType::SevenZip
                } else {
                    FileType::File
                };

                let mut sf = ScannedFile::new(file_type);
                sf.name = file_name.clone();
                if let Ok(mod_time) = metadata.modified() {
                    if let Ok(dur) = mod_time.duration_since(std::time::UNIX_EPOCH) {
                        sf.file_mod_time_stamp = dur.as_secs() as i64;
                    }
                }

                if file_type == FileType::File {
                    if deep_scan {
                        let file_path = path.join(&file_name);
                        if let Ok(scanned_file) = Self::scan_raw_file(&file_path.to_string_lossy()) {
                            sf = scanned_file;
                        } else {
                            sf.size = Some(metadata.len());
                        }
                    } else {
                        sf.size = Some(metadata.len());
                    }
                } else if file_type == FileType::Dir {
                    let sub_path = path.join(&file_name);
                    sf.children = Self::scan_directory_impl(&sub_path, deep_scan, ignore_matcher);
                } else if file_type == FileType::Zip || file_type == FileType::SevenZip {
                    let archive_path = path.join(&file_name);
                    if let Ok(archive_sf) = Self::scan_archive_file(
                        file_type,
                        &archive_path.to_string_lossy(),
                        sf.file_mod_time_stamp,
                        deep_scan,
                    ) {
                        sf.children = archive_sf.children;
                    }
                }
                Some(sf)
            })
            .collect()
    }
}
