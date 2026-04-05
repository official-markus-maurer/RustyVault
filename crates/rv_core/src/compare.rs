use crate::rv_file::RvFile;
use crate::scanned_file::ScannedFile;
use crate::settings::EScanLevel;
use dat_reader::enums::FileType;

/// Logic for comparing physically scanned files against database nodes.
///
/// `FileCompare` evaluates whether a `ScannedFile` (physical) correctly matches an `RvFile` (logical)
/// based on name, size, timestamps, and cryptographic hashes.
///
/// Implementation notes:
/// - `phase_1_test` performs exact matching.
/// - `phase_2_test` contains a limited set of fallback heuristics.
///
/// TODO: Expand phase-2 heuristics to handle more header/rename recovery cases safely.
pub struct FileCompare;

/// Performs a basic alphabetical name comparison between a DB file and a scanned file.
pub fn compare_db_to_file(db_file: &RvFile, file_c: &ScannedFile) -> i32 {
    let name_cmp = if cfg!(windows) {
        db_file
            .name
            .to_ascii_lowercase()
            .cmp(&file_c.name.to_ascii_lowercase())
    } else {
        db_file.name.cmp(&file_c.name)
    };
    match name_cmp {
        std::cmp::Ordering::Less => -1,
        std::cmp::Ordering::Equal => 0,
        std::cmp::Ordering::Greater => 1,
    }
}

impl FileCompare {
    fn compare_names(db_name: &str, test_name: &str, index_case: i32) -> std::cmp::Ordering {
        if index_case == 0 {
            db_name.cmp(test_name)
        } else if cfg!(windows) {
            db_name
                .to_ascii_lowercase()
                .cmp(&test_name.to_ascii_lowercase())
        } else {
            db_name.cmp(test_name)
        }
    }

    fn db_file_has_name_agnostic_identity(db_file: &RvFile) -> bool {
        db_file.crc.is_some()
            || db_file.sha1.is_some()
            || db_file.md5.is_some()
            || db_file.alt_crc.is_some()
            || db_file.alt_sha1.is_some()
            || db_file.alt_md5.is_some()
    }

    fn scanned_file_has_hash_identity(test_file: &ScannedFile) -> bool {
        test_file.crc.is_some()
            || test_file.sha1.is_some()
            || test_file.md5.is_some()
            || test_file.alt_crc.is_some()
            || test_file.alt_sha1.is_some()
            || test_file.alt_md5.is_some()
    }

    fn scanned_file_covers_required_hashes(db_file: &RvFile, test_file: &ScannedFile) -> bool {
        let has_crc = test_file.crc.is_some() || test_file.alt_crc.is_some();
        let has_sha1 = test_file.sha1.is_some() || test_file.alt_sha1.is_some();
        let has_md5 = test_file.md5.is_some() || test_file.alt_md5.is_some();

        ((db_file.crc.is_none() && db_file.alt_crc.is_none()) || has_crc)
            && ((db_file.sha1.is_none() && db_file.alt_sha1.is_none()) || has_sha1)
            && ((db_file.md5.is_none() && db_file.alt_md5.is_none()) || has_md5)
    }

    fn header_requirement_matches(db_file: &RvFile, test_file: &ScannedFile) -> bool {
        !db_file.header_file_type_required()
            || db_file.header_file_type() == test_file.header_file_type
    }

    fn phase_2_supported_leaf_type(file_type: FileType) -> bool {
        matches!(
            file_type,
            FileType::File | FileType::FileOnly | FileType::FileZip | FileType::FileSevenZip
        )
    }

    fn phase_1_compatible_type_pair(db_file_type: FileType, test_file_type: FileType) -> bool {
        db_file_type == test_file_type
            || (Self::phase_2_supported_leaf_type(db_file_type)
                && Self::phase_2_supported_leaf_type(test_file_type))
    }

    /// Core evaluation logic that matches physical metadata against logical expected metadata.
    ///
    /// This function strictly evaluates "Phase 1" equivalence: Exact File Name, Size, CRC,
    /// SHA1, and MD5 matching depending on the strictness of the current `EScanLevel` settings.
    pub fn phase_1_test(
        db_file: &RvFile,
        test_file: &ScannedFile,
        e_scan_level: EScanLevel,
        index_case: i32,
    ) -> (bool, bool) {
        let mut matched_alt = false;

        // Name comparison
        let retv = Self::compare_names(&db_file.name, &test_file.name, index_case);

        if retv != std::cmp::Ordering::Equal {
            return (false, matched_alt);
        }

        let db_file_type = db_file.file_type;
        let test_file_type = test_file.file_type;

        if !Self::phase_1_compatible_type_pair(db_file_type, test_file_type) {
            return (false, matched_alt);
        }

        // Directories and container nodes don't need deep hashing matches at this level
        if db_file_type == FileType::Dir
            || db_file_type == FileType::Zip
            || db_file_type == FileType::SevenZip
        {
            return (true, matched_alt);
        }

        if !Self::header_requirement_matches(db_file, test_file) {
            return (false, matched_alt);
        }

        // If the scanned file has any hash identity, we should do full hash matching
        if Self::scanned_file_has_hash_identity(test_file) {
            let matched = Self::compare_with_alt(db_file, test_file, &mut matched_alt);
            return (matched, matched_alt);
        }

        if e_scan_level != EScanLevel::Level1 && !db_file.is_deep_scanned() {
            return (false, matched_alt);
        }

        // Timestamp match
        if db_file.file_mod_time_stamp != test_file.file_mod_time_stamp {
            return (false, matched_alt);
        }

        if db_file.size == test_file.size {
            return (true, matched_alt);
        }

        let alt_test_size = test_file.alt_size.or(test_file.size);
        if test_file.alt_size.is_some() && db_file.size.is_some() && db_file.size == alt_test_size {
            matched_alt = true;
            return (true, matched_alt);
        }

        if db_file.alt_size.is_some() && db_file.alt_size == alt_test_size {
            matched_alt = true;
            return (true, matched_alt);
        }

        (false, matched_alt)
    }

    fn compare_with_alt(db_file: &RvFile, test_file: &ScannedFile, matched_alt: &mut bool) -> bool {
        let has_primary_identity = db_file.size.is_some()
            || db_file.crc.is_some()
            || db_file.sha1.is_some()
            || db_file.md5.is_some();

        // Standard compare
        let mut match_ok = has_primary_identity;
        if db_file.size.is_some() && db_file.size != test_file.size {
            match_ok = false;
        }
        if db_file.crc.is_some() && db_file.crc != test_file.crc {
            match_ok = false;
        }
        if db_file.sha1.is_some() && db_file.sha1 != test_file.sha1 {
            match_ok = false;
        }
        if db_file.md5.is_some() && db_file.md5 != test_file.md5 {
            match_ok = false;
        }

        if match_ok {
            *matched_alt = false;
            return true;
        }

        let alt_test_size = test_file.alt_size.or(test_file.size);
        let alt_test_crc = test_file.alt_crc.as_ref().or(test_file.crc.as_ref());
        let alt_test_sha1 = test_file.alt_sha1.as_ref().or(test_file.sha1.as_ref());
        let alt_test_md5 = test_file.alt_md5.as_ref().or(test_file.md5.as_ref());

        let scanned_has_alt_identity = test_file.alt_size.is_some()
            || test_file.alt_crc.is_some()
            || test_file.alt_sha1.is_some()
            || test_file.alt_md5.is_some();
        if scanned_has_alt_identity {
            let mut primary_vs_alt_ok = has_primary_identity;
            if db_file.size.is_some() && db_file.size != alt_test_size {
                primary_vs_alt_ok = false;
            }
            if db_file
                .crc
                .as_ref()
                .is_some_and(|v| Some(v) != alt_test_crc)
            {
                primary_vs_alt_ok = false;
            }
            if db_file
                .sha1
                .as_ref()
                .is_some_and(|v| Some(v) != alt_test_sha1)
            {
                primary_vs_alt_ok = false;
            }
            if db_file
                .md5
                .as_ref()
                .is_some_and(|v| Some(v) != alt_test_md5)
            {
                primary_vs_alt_ok = false;
            }

            if primary_vs_alt_ok {
                *matched_alt = true;
                return true;
            }
        }

        // Alt compare
        let mut alt_ok = true;
        if db_file.alt_size.is_some() && db_file.alt_size != alt_test_size {
            alt_ok = false;
        }
        if db_file
            .alt_crc
            .as_ref()
            .is_some_and(|v| Some(v) != alt_test_crc)
        {
            alt_ok = false;
        }
        if db_file
            .alt_sha1
            .as_ref()
            .is_some_and(|v| Some(v) != alt_test_sha1)
        {
            alt_ok = false;
        }
        if db_file
            .alt_md5
            .as_ref()
            .is_some_and(|v| Some(v) != alt_test_md5)
        {
            alt_ok = false;
        }

        if alt_ok
            && (db_file.alt_size.is_some()
                || db_file.alt_crc.is_some()
                || db_file.alt_sha1.is_some()
                || db_file.alt_md5.is_some())
        {
            *matched_alt = true;
            return true;
        }

        false
    }

    fn current_parent_physical_path(db_file: &RvFile) -> std::path::PathBuf {
        let mut path_parts = Vec::new();
        let mut current_parent = db_file.parent.as_ref().and_then(|p| p.upgrade());

        while let Some(parent) = current_parent {
            let parent_borrow = parent.borrow();
            if !parent_borrow.name_case().is_empty() {
                path_parts.push(parent_borrow.name_case().to_string());
            }
            current_parent = parent_borrow.parent.as_ref().and_then(|p| p.upgrade());
        }

        path_parts.reverse();
        let logical_path = path_parts.join("\\");
        let path = std::path::PathBuf::from(&logical_path);
        if path.is_absolute() {
            path
        } else if logical_path.is_empty() {
            std::path::PathBuf::new()
        } else {
            crate::settings::find_dir_mapping(&logical_path)
                .map(std::path::PathBuf::from)
                .unwrap_or(path)
        }
    }

    fn deep_scan_physical_file(db_file: &RvFile, test_file: &mut ScannedFile) {
        if !Self::phase_2_supported_leaf_type(test_file.file_type) {
            return;
        }

        if (!test_file.deep_scanned
            || !Self::scanned_file_has_hash_identity(test_file)
            || !Self::scanned_file_covers_required_hashes(db_file, test_file))
            && test_file.got_status != dat_reader::enums::GotStatus::FileLocked
        {
            let parent_path = Self::current_parent_physical_path(db_file);
            let full_path = if parent_path.as_os_str().is_empty() {
                test_file.name.clone()
            } else {
                parent_path
                    .join(&test_file.name)
                    .to_string_lossy()
                    .to_string()
            };

            if let Ok(mut sf) = crate::scanner::Scanner::scan_raw_file(&full_path) {
                test_file.file_mod_time_stamp = sf.file_mod_time_stamp;
                test_file.header_file_type = sf.header_file_type;
                test_file.status_flags = sf.status_flags;
                test_file.crc = sf.crc.take();
                test_file.sha1 = sf.sha1.take();
                test_file.md5 = sf.md5.take();
                test_file.size = sf.size.take();
                test_file.alt_size = sf.alt_size.take();
                test_file.alt_crc = sf.alt_crc.take();
                test_file.alt_sha1 = sf.alt_sha1.take();
                test_file.alt_md5 = sf.alt_md5.take();
                test_file.got_status = sf.got_status;
                test_file.deep_scanned = true;
            }
        }
    }

    /// Evaluates "Phase 2" equivalence using on-demand deep scanning.
    ///
    /// If Phase 1 fails but the name matches, this may compute missing hashes from disk
    /// and re-check equality against the expected DB identity.
    pub fn phase_2_test(
        db_file: &RvFile,
        test_file: &mut ScannedFile,
        index_case: i32,
    ) -> (bool, bool) {
        let mut matched_alt = false;

        // Name comparison
        let retv = Self::compare_names(&db_file.name, &test_file.name, index_case);

        if retv != std::cmp::Ordering::Equal {
            return (false, matched_alt);
        }

        let db_file_type = db_file.file_type;
        let test_file_type = test_file.file_type;

        if !Self::phase_2_supported_leaf_type(db_file_type)
            || !Self::phase_2_supported_leaf_type(test_file_type)
        {
            return (false, matched_alt);
        }

        Self::deep_scan_physical_file(db_file, test_file);

        if test_file.got_status == dat_reader::enums::GotStatus::FileLocked {
            return (false, matched_alt);
        }

        if !Self::header_requirement_matches(db_file, test_file) {
            return (false, matched_alt);
        }

        let matched = Self::compare_with_alt(db_file, test_file, &mut matched_alt);
        (matched, matched_alt)
    }

    pub fn phase_2_name_agnostic_test(
        db_file: &RvFile,
        test_file: &mut ScannedFile,
    ) -> (bool, bool) {
        let mut matched_alt = false;

        if !Self::phase_2_supported_leaf_type(db_file.file_type)
            || !Self::phase_2_supported_leaf_type(test_file.file_type)
        {
            return (false, matched_alt);
        }

        Self::deep_scan_physical_file(db_file, test_file);

        if test_file.got_status == dat_reader::enums::GotStatus::FileLocked {
            return (false, matched_alt);
        }

        if !Self::header_requirement_matches(db_file, test_file) {
            return (false, matched_alt);
        }

        if !Self::db_file_has_name_agnostic_identity(db_file) {
            let timestamp_confident = db_file.file_mod_time_stamp > 0
                && db_file.file_mod_time_stamp == test_file.file_mod_time_stamp;
            if !timestamp_confident {
                return (false, matched_alt);
            }
        }

        let matched = Self::compare_with_alt(db_file, test_file, &mut matched_alt);
        (matched, matched_alt)
    }
}

#[cfg(test)]
#[path = "tests/compare_tests.rs"]
mod tests;
