impl DatUpdate {
    fn scan_dat_dir(path: &str, dats_found: &mut Vec<(String, String)>) {
        let scan_path = Path::new(path);
        let dat_root = crate::settings::get_settings().dat_root;
        let dat_root_path = Path::new(if dat_root.is_empty() { "DatRoot" } else { &dat_root });
        let base_path = if crate::settings::strip_physical_prefix(scan_path, dat_root_path).is_some() {
            dat_root_path
        } else {
            scan_path
        };

        Self::recursive_scan(base_path, scan_path, dats_found);
    }

    fn recursive_scan(base_path: &Path, current_path: &Path, dats_found: &mut Vec<(String, String)>) {
        if let Ok(entries) = fs::read_dir(current_path) {
            for entry in entries.filter_map(Result::ok) {
                let path = entry.path();
                if path.is_dir() {
                    Self::recursive_scan(base_path, &path, dats_found);
                } else if let Some(ext) = path.extension() {
                    let ext_str = ext.to_string_lossy().to_lowercase();
                    if ext_str == "dat" || ext_str == "xml" || ext_str == "datz" {
                        let full_path = path.to_string_lossy().into_owned();

                        let virtual_dir = path
                            .strip_prefix(base_path)
                            .ok()
                            .and_then(|relative| relative.parent())
                            .map(|parent| parent.to_string_lossy().replace('/', "\\"))
                            .filter(|parent| !parent.is_empty())
                            .unwrap_or_default();

                        dats_found.push((full_path, virtual_dir));
                    }
                }
            }
        }
    }

    /// Cleans up orphaned DB nodes whose underlying physical DAT files have been deleted.
    pub fn check_all_dats(db_file: Rc<RefCell<RvFile>>, dat_path: &str) {
        let db_dir = db_file.borrow();
        if !db_dir.is_directory() {
            return;
        }

        for dat in &db_dir.dir_dats {
            let dat_full_name = dat
                .borrow()
                .get_data(crate::rv_dat::DatData::DatRootFullName)
                .unwrap_or_default();
            if Self::dat_path_matches_filter(&dat_full_name, dat_path) {
                dat.borrow_mut().time_stamp = i64::MAX;
            }
        }

        if let Some(dat) = &db_dir.dat {
            let dat_full_name = dat
                .borrow()
                .get_data(crate::rv_dat::DatData::DatRootFullName)
                .unwrap_or_default();
            if Self::dat_path_matches_filter(&dat_full_name, dat_path) {
                dat.borrow_mut().time_stamp = i64::MAX;
            }
        }

        let children = db_dir.children.clone();
        drop(db_dir);

        for child in children {
            Self::check_all_dats(child, dat_path);
        }
    }
}
