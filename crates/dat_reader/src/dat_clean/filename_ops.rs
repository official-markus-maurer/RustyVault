impl DatClean {
    pub fn remove_date_time(d_dir: &mut DatDir) {
        for node in &mut d_dir.children {
            if node.file().is_some() {
                node.date_modified = None;
            }
            if let Some(ddir) = node.dir_mut() {
                Self::remove_date_time(ddir);
            }
        }
    }

    pub fn remove_md5(d_dir: &mut DatDir) {
        for node in &mut d_dir.children {
            if let Some(df) = node.file_mut() {
                df.md5 = None;
            }
            if let Some(ddir) = node.dir_mut() {
                Self::remove_md5(ddir);
            }
        }
    }

    pub fn remove_sha256(d_dir: &mut DatDir) {
        for node in &mut d_dir.children {
            if let Some(df) = node.file_mut() {
                df.sha256 = None;
            }
            if let Some(ddir) = node.dir_mut() {
                Self::remove_sha256(ddir);
            }
        }
    }

    pub fn clean_filenames(d_dir: &mut DatDir) {
        for node in &mut d_dir.children {
            Self::clean_filename(node);
            if let Some(ddir) = node.dir_mut() {
                Self::clean_filenames(ddir);
            }
        }
    }

    pub fn clean_file_names_full(in_dat: &mut DatNode) {
        let Some(d_dir) = in_dat.dir_mut() else {
            return;
        };
        let mut children = Vec::new();
        children.append(&mut d_dir.children);

        for mut child in children {
            match child.file_type {
                FileType::UnSet | FileType::File | FileType::Dir => {
                    child.name = child.name.trim_start_matches(' ').to_string();
                    child.name = child.name.trim_end_matches(['.', ' ']).to_string();
                }
                FileType::Zip | FileType::SevenZip => {
                    child.name = child.name.trim_start_matches(' ').to_string();
                    child.name = child.name.trim_end_matches(' ').to_string();
                }
                _ => {}
            }
            if child.name.trim().is_empty() {
                child.name = "_".to_string();
            }

            if child.is_dir() && child.file_type == FileType::Dir {
                Self::clean_file_names_full(&mut child);
            }

            d_dir.add_child(child);
        }
    }

    pub fn fix_dupes(d_dir: &mut DatDir) {
        d_dir.children.sort_by(|a, b| {
            let t = a.file_type.cmp(&b.file_type);
            if t != std::cmp::Ordering::Equal {
                return t;
            }
            let la = a.name.to_ascii_lowercase();
            let lb = b.name.to_ascii_lowercase();
            la.cmp(&lb).then(a.name.cmp(&b.name))
        });

        let mut last_name = String::new();
        let mut last_type = FileType::UnSet;
        let mut match_count = 0usize;

        for node in &mut d_dir.children {
            let this_name = node.name.clone();
            let ft = node.file_type;

            if ft == last_type && last_name.eq_ignore_ascii_case(&this_name) {
                match ft {
                    FileType::Dir | FileType::Zip | FileType::SevenZip => {
                        node.name = format!("{}_{}", this_name, match_count);
                    }
                    _ => {
                        let ext = std::path::Path::new(&this_name)
                            .extension()
                            .map(|e| format!(".{}", e.to_string_lossy()))
                            .unwrap_or_default();
                        let stem = if ext.is_empty() {
                            this_name.clone()
                        } else {
                            this_name[..this_name.len().saturating_sub(ext.len())].to_string()
                        };
                        node.name = format!("{}_{}{}", stem, match_count, ext);
                    }
                }
                match_count += 1;
            } else {
                match_count = 0;
                last_name = this_name;
                last_type = ft;
            }

            if let Some(ddir) = node.dir_mut() {
                Self::fix_dupes(ddir);
            }
        }
    }
}
