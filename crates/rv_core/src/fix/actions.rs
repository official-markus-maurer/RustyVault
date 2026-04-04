use std::path::Path;

impl super::Fix {
    pub(super) fn report_action(message: impl Into<String>) {
        if crate::settings::get_settings().detailed_fix_reporting {
            crate::task_reporter::task_log(message);
        }
    }

    pub(super) fn maybe_delete_old_cue_zip_in_tosort(tosort_path: &Path) {
        if !crate::settings::get_settings().delete_old_cue_files {
            return;
        }
        let Some(parent_dir) = tosort_path.parent() else {
            return;
        };
        let Some(file_name) = tosort_path.file_name().and_then(|s| s.to_str()) else {
            return;
        };

        let file_name_lc = file_name.to_ascii_lowercase();
        if file_name_lc.ends_with(".cue.zip") {
            return;
        }

        let base = tosort_path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or_default()
            .to_string();
        if base.is_empty() {
            return;
        }

        let candidate = parent_dir.join(format!("{base}.cue.zip"));
        if candidate.exists() && candidate != tosort_path {
            let _ = std::fs::remove_file(&candidate);
            Self::report_action(format!("Delete old cue zip: {}", candidate.display()));
        }
    }
}
