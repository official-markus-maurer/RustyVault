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
        let path = Path::new(path_str);
        let deep_scan = matches!(
            scan_level,
            crate::settings::EScanLevel::Level2 | crate::settings::EScanLevel::Level3
        );

        // TODO(perf): avoid collecting `read_dir` into an intermediate `Vec` when possible; consider a work-stealing
        // producer/consumer that feeds entries to rayon.
        // TODO(perf): precompile ignore patterns (or use globset) instead of repeatedly matching strings.
        // TODO(threading): add a shared cancellation token so long scans can be interrupted from the UI.
        if let Ok(entries) = fs::read_dir(path) {
            let entry_list: Vec<_> = entries.flatten().collect();
            let mut ignore_patterns = crate::settings::get_settings().ignore_files.items.clone();
            ignore_patterns.extend_from_slice(extra_ignore_patterns);
            let ignore_patterns = Arc::new(ignore_patterns);

            let results: Vec<ScannedFile> = entry_list
                .into_par_iter()
                .filter_map(|entry| {
                    let metadata = entry.metadata().ok()?;
                    let file_name = entry.file_name().to_string_lossy().to_string();

                    if !metadata.is_dir()
                        && file_name.starts_with("__RomVault.")
                        && file_name.ends_with(".tmp")
                    {
                        let _ = fs::remove_file(entry.path());
                        return None;
                    }

                    if is_ignored_file_name(&file_name, &ignore_patterns) {
                        return None;
                    }

                    let file_type = if metadata.is_dir() {
                        FileType::Dir
                    } else {
                        let lower_name = file_name.to_lowercase();
                        if lower_name.ends_with(".zip") {
                            FileType::Zip
                        } else if lower_name.ends_with(".7z") {
                            FileType::SevenZip
                        } else {
                            FileType::File
                        }
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
                        sf.children =
                            Self::scan_directory_with_level(&sub_path.to_string_lossy(), scan_level);
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
                .collect();

            return results;
        }

        Vec::new()
    }
}
