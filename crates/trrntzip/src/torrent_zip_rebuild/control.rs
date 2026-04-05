impl TorrentZipRebuild {
    fn aborted_status(control: Option<&ProcessControl>) -> TrrntZipStatus {
        if control.is_some_and(|control| control.is_hard_stop_requested()) {
            TrrntZipStatus::USER_ABORTED_HARD
        } else {
            TrrntZipStatus::USER_ABORTED
        }
    }

    fn hard_stop_requested(control: Option<&ProcessControl>) -> bool {
        control.is_some_and(|control| {
            control.wait_one();
            control.is_soft_stop_requested()
        })
    }

    fn remove_tmp_if_present(tmp_filename: &Path) {
        if tmp_filename.exists() {
            let _ = fs::remove_file(tmp_filename);
        }
    }

    pub fn cleanup_samtmp_files(base_path: &Path, recursive: bool) -> usize {
        let mut removed = 0;

        let is_samtmp = base_path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.contains(".samtmp"));

        if is_samtmp {
            if base_path.is_dir() {
                if fs::remove_dir_all(base_path).is_ok() {
                    removed += 1;
                }
            } else if base_path.is_file() && fs::remove_file(base_path).is_ok() {
                removed += 1;
            }
            return removed;
        }

        let Ok(entries) = fs::read_dir(base_path) else {
            return 0;
        };

        for entry in entries.flatten() {
            let path = entry.path();
            let is_samtmp = path
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.contains(".samtmp"));
            if is_samtmp {
                if path.is_dir() {
                    if fs::remove_dir_all(&path).is_ok() {
                        removed += 1;
                    }
                } else if fs::remove_file(&path).is_ok() {
                    removed += 1;
                }
            } else if path.is_dir() && recursive {
                removed += Self::cleanup_samtmp_files(&path, true);
            }
        }

        removed
    }
}
