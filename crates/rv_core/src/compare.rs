use crate::rv_file::RvFile;
use crate::scanned_file::ScannedFile;
use dat_reader::enums::FileType;
use crate::settings::EScanLevel;

/// Logic for comparing physically scanned files against database nodes.
/// 
/// `FileCompare` evaluates whether a `ScannedFile` (physical) correctly matches an `RvFile` (logical)
/// based on name, size, timestamps, and cryptographic hashes.
/// 
/// Differences from C#:
/// - The C# implementation contains deep logic for `Phase2Test`, which attempts to fuzzy-match files
///   that might have incorrect names or be stripped of extraneous headers.
/// - The Rust version currently implements `phase_1_test` for exact matches, and `phase_2_test` for CHD
///   version mismatch fallbacks and size-only exact-name matching.
pub struct FileCompare;

/// Performs a basic alphabetical name comparison between a DB file and a scanned file.
pub fn compare_db_to_file(db_file: &RvFile, file_c: &ScannedFile) -> i32 {
    let name_cmp = db_file.name.to_lowercase().cmp(&file_c.name.to_lowercase());
    match name_cmp {
        std::cmp::Ordering::Less => -1,
        std::cmp::Ordering::Equal => 0,
        std::cmp::Ordering::Greater => 1,
    }
}

impl FileCompare {
    fn db_file_requires_hash_match(db_file: &RvFile) -> bool {
        db_file.crc.is_some()
            || db_file.sha1.is_some()
            || db_file.md5.is_some()
            || db_file.alt_crc.is_some()
            || db_file.alt_sha1.is_some()
            || db_file.alt_md5.is_some()
    }

    fn db_file_has_name_agnostic_identity(db_file: &RvFile) -> bool {
        db_file.crc.is_some()
            || db_file.sha1.is_some()
            || db_file.md5.is_some()
            || db_file.alt_crc.is_some()
            || db_file.alt_sha1.is_some()
            || db_file.alt_md5.is_some()
    }

    fn header_requirement_matches(db_file: &RvFile, test_file: &ScannedFile) -> bool {
        !db_file.header_file_type_required() || db_file.header_file_type() == test_file.header_file_type
    }

    /// Core evaluation logic that matches physical metadata against logical expected metadata.
    /// 
    /// This function strictly evaluates "Phase 1" equivalence: Exact File Name, Size, CRC,
    /// SHA1, and MD5 matching depending on the strictness of the current `EScanLevel` settings.
    pub fn phase_1_test(db_file: &RvFile, test_file: &ScannedFile, e_scan_level: EScanLevel, index_case: i32) -> (bool, bool) {
        let mut matched_alt = false;

        // Name comparison
        let retv = if index_case == 0 {
            db_file.name.cmp(&test_file.name)
        } else {
            db_file.name.to_lowercase().cmp(&test_file.name.to_lowercase())
        };

        if retv != std::cmp::Ordering::Equal {
            return (false, matched_alt);
        }

        let db_file_type = db_file.file_type;
        let test_file_type = test_file.file_type;

        if db_file_type != test_file_type {
            return (false, matched_alt);
        }

        // Directories and Archives don't need deep hashing matches at this level
        if db_file_type == FileType::Dir || db_file_type == FileType::Zip || db_file_type == FileType::SevenZip || db_file_type == FileType::FileZip || db_file_type == FileType::FileSevenZip {
            return (true, matched_alt);
        }

        if !Self::header_requirement_matches(db_file, test_file) {
            return (false, matched_alt);
        }

        // If test file has CRC, we can do full hash matching
        if test_file.crc.is_some() {
            let matched = Self::compare_with_alt(db_file, test_file, &mut matched_alt);
            return (matched, matched_alt);
        }

        // If no hashes were scanned from the physical file, higher scan levels should still allow
        // timestamp/size fallback for DAT entries that also do not define cryptographic identity.
        if e_scan_level != EScanLevel::Level1 && Self::db_file_requires_hash_match(db_file) {
            return (false, matched_alt);
        }

        // Timestamp match
        if db_file.file_mod_time_stamp != test_file.file_mod_time_stamp {
            return (false, matched_alt);
        }

        if db_file.size == test_file.size {
            return (true, matched_alt);
        }

        (false, matched_alt)
    }

    fn compare_with_alt(db_file: &RvFile, test_file: &ScannedFile, matched_alt: &mut bool) -> bool {
        let has_primary_identity =
            db_file.size.is_some() || db_file.crc.is_some() || db_file.sha1.is_some() || db_file.md5.is_some();

        // Standard compare
        let mut match_ok = has_primary_identity;
        if db_file.size.is_some() && db_file.size != test_file.size { match_ok = false; }
        if db_file.crc.is_some() && db_file.crc != test_file.crc { match_ok = false; }
        if db_file.sha1.is_some() && db_file.sha1 != test_file.sha1 { match_ok = false; }
        if db_file.md5.is_some() && db_file.md5 != test_file.md5 { match_ok = false; }

        if match_ok {
            *matched_alt = false;
            return true;
        }

        // Alt compare
        let alt_test_size = test_file.alt_size.or(test_file.size);
        let alt_test_crc = test_file.alt_crc.as_ref().or(test_file.crc.as_ref());
        let alt_test_sha1 = test_file.alt_sha1.as_ref().or(test_file.sha1.as_ref());
        let alt_test_md5 = test_file.alt_md5.as_ref().or(test_file.md5.as_ref());

        let mut alt_ok = true;
        if db_file.alt_size.is_some() && db_file.alt_size != alt_test_size { alt_ok = false; }
        if db_file.alt_crc.as_ref().is_some_and(|v| Some(v) != alt_test_crc) { alt_ok = false; }
        if db_file.alt_sha1.as_ref().is_some_and(|v| Some(v) != alt_test_sha1) { alt_ok = false; }
        if db_file.alt_md5.as_ref().is_some_and(|v| Some(v) != alt_test_md5) { alt_ok = false; }

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

    fn deep_scan_physical_file(db_file: &RvFile, test_file: &mut ScannedFile) {
        if !test_file.deep_scanned && test_file.got_status != dat_reader::enums::GotStatus::FileLocked {
            let full_path = std::path::Path::new(&db_file.get_full_name())
                .parent()
                .map(|p| p.join(&test_file.name).to_string_lossy().to_string())
                .unwrap_or_else(|| test_file.name.clone());

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

    /// Evaluates "Phase 2" equivalence: This mimics the C# deep scan fallback.
    /// In C#, if Phase 1 fails, the scanner will execute `Populate.FromAFile` to perform
    /// a deep cryptographic hash of the loose file on disk right then and there.
    /// In our Rust version, if we get here, we check if the file needs deep scanning.
    pub fn phase_2_test(db_file: &RvFile, test_file: &mut ScannedFile, index_case: i32) -> (bool, bool) {
        let mut matched_alt = false;

        // Name comparison
        let retv = if index_case == 0 {
            db_file.name.cmp(&test_file.name)
        } else {
            db_file.name.to_lowercase().cmp(&test_file.name.to_lowercase())
        };

        if retv != std::cmp::Ordering::Equal {
            return (false, matched_alt);
        }

        let db_file_type = db_file.file_type;
        let test_file_type = test_file.file_type;

        if db_file_type != FileType::File || test_file_type != FileType::File {
            return (false, matched_alt);
        }

        Self::deep_scan_physical_file(db_file, test_file);

        if test_file.got_status == dat_reader::enums::GotStatus::FileLocked {
            return (true, matched_alt);
        }

        if !Self::header_requirement_matches(db_file, test_file) {
            return (false, matched_alt);
        }

        let matched = Self::compare_with_alt(db_file, test_file, &mut matched_alt);
        (matched, matched_alt)
    }

    pub fn phase_2_name_agnostic_test(db_file: &RvFile, test_file: &mut ScannedFile) -> (bool, bool) {
        let mut matched_alt = false;

        if db_file.file_type != FileType::File || test_file.file_type != FileType::File {
            return (false, matched_alt);
        }

        if !Self::db_file_has_name_agnostic_identity(db_file) {
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
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::fs;
    use std::rc::Rc;
    use tempfile::tempdir;

    #[test]
    fn test_compare_db_to_file() {
        let mut db_file = RvFile::new(FileType::File);
        db_file.name = "B_File.zip".to_string();

        let mut sc_file = ScannedFile::new(FileType::File);
        sc_file.name = "A_File.zip".to_string();

        assert_eq!(compare_db_to_file(&db_file, &sc_file), 1);
        
        sc_file.name = "C_File.zip".to_string();
        assert_eq!(compare_db_to_file(&db_file, &sc_file), -1);

        sc_file.name = "B_File.zip".to_string();
        assert_eq!(compare_db_to_file(&db_file, &sc_file), 0);
    }

    #[test]
    fn test_phase_1_test_hashes() {
        let mut db_file = RvFile::new(FileType::File);
        db_file.name = "rom.bin".to_string();
        db_file.size = Some(1024);
        db_file.crc = Some(vec![0xAA, 0xBB, 0xCC, 0xDD]);

        let mut sc_file = ScannedFile::new(FileType::File);
        sc_file.name = "rom.bin".to_string();
        sc_file.size = Some(1024);
        sc_file.crc = Some(vec![0xAA, 0xBB, 0xCC, 0xDD]);

        let (matched, alt) = FileCompare::phase_1_test(&db_file, &sc_file, EScanLevel::Level2, 0);
        assert!(matched);
        assert!(!alt);

        // Test Alt Match
        db_file.crc = Some(vec![0x11, 0x22, 0x33, 0x44]);
        db_file.alt_size = Some(1024);
        db_file.alt_crc = Some(vec![0xAA, 0xBB, 0xCC, 0xDD]);

        let (matched, alt) = FileCompare::phase_1_test(&db_file, &sc_file, EScanLevel::Level2, 0);
        assert!(matched);
        assert!(alt);

        // Test Mismatch
        sc_file.crc = Some(vec![0xFF, 0xFF, 0xFF, 0xFF]);
        let (matched, _) = FileCompare::phase_1_test(&db_file, &sc_file, EScanLevel::Level2, 0);
        assert!(!matched);
    }

    #[test]
    fn test_phase_1_test_does_not_match_archive_to_directory() {
        let mut db_file = RvFile::new(FileType::Zip);
        db_file.name = "game.zip".to_string();

        let mut sc_file = ScannedFile::new(FileType::Dir);
        sc_file.name = "game.zip".to_string();

        let (matched, alt) = FileCompare::phase_1_test(&db_file, &sc_file, EScanLevel::Level1, 0);
        assert!(!matched);
        assert!(!alt);
    }

    #[test]
    fn test_phase_1_test_level2_allows_timestamp_size_match_when_dat_has_no_hashes() {
        let mut db_file = RvFile::new(FileType::File);
        db_file.name = "rom.bin".to_string();
        db_file.size = Some(1024);
        db_file.file_mod_time_stamp = 123456;

        let mut sc_file = ScannedFile::new(FileType::File);
        sc_file.name = "rom.bin".to_string();
        sc_file.size = Some(1024);
        sc_file.file_mod_time_stamp = 123456;

        let (matched, alt) = FileCompare::phase_1_test(&db_file, &sc_file, EScanLevel::Level2, 0);
        assert!(matched);
        assert!(!alt);
    }

    #[test]
    fn test_phase_1_test_level2_rejects_timestamp_size_match_when_dat_has_hashes() {
        let mut db_file = RvFile::new(FileType::File);
        db_file.name = "rom.bin".to_string();
        db_file.size = Some(1024);
        db_file.crc = Some(vec![0xAA, 0xBB, 0xCC, 0xDD]);
        db_file.file_mod_time_stamp = 123456;

        let mut sc_file = ScannedFile::new(FileType::File);
        sc_file.name = "rom.bin".to_string();
        sc_file.size = Some(1024);
        sc_file.file_mod_time_stamp = 123456;

        let (matched, alt) = FileCompare::phase_1_test(&db_file, &sc_file, EScanLevel::Level2, 0);
        assert!(!matched);
        assert!(!alt);
    }

    #[test]
    fn test_phase_1_test_alt_match_uses_scanned_alt_hashes() {
        let mut db_file = RvFile::new(FileType::File);
        db_file.name = "rom.nes".to_string();
        db_file.alt_size = Some(4);
        db_file.alt_crc = Some(vec![0xAD, 0xF3, 0xF3, 0x63]);

        let mut sc_file = ScannedFile::new(FileType::File);
        sc_file.name = "rom.nes".to_string();
        sc_file.size = Some(20);
        sc_file.crc = Some(vec![0x00, 0x00, 0x00, 0x00]);
        sc_file.alt_size = Some(4);
        sc_file.alt_crc = Some(vec![0xAD, 0xF3, 0xF3, 0x63]);

        let (matched, alt) = FileCompare::phase_1_test(&db_file, &sc_file, EScanLevel::Level2, 0);
        assert!(matched);
        assert!(alt);
    }

    #[test]
    fn test_phase_2_name_agnostic_test_matches_renamed_file_by_hash() {
        let mut db_file = RvFile::new(FileType::File);
        db_file.name = "expected.bin".to_string();
        db_file.size = Some(4);
        db_file.crc = Some(vec![0xAD, 0xF3, 0xF3, 0x63]);

        let mut sc_file = ScannedFile::new(FileType::File);
        sc_file.name = "renamed.bin".to_string();
        sc_file.size = Some(4);
        sc_file.crc = Some(vec![0xAD, 0xF3, 0xF3, 0x63]);
        sc_file.deep_scanned = true;

        let (matched, alt) = FileCompare::phase_2_name_agnostic_test(&db_file, &mut sc_file);
        assert!(matched);
        assert!(!alt);
    }

    #[test]
    fn test_phase_2_name_agnostic_test_rejects_wrong_required_header_type() {
        let mut db_file = RvFile::new(FileType::File);
        db_file.name = "expected.nes".to_string();
        db_file.size = Some(4);
        db_file.crc = Some(vec![0xAD, 0xF3, 0xF3, 0x63]);
        db_file.header_file_type = dat_reader::enums::HeaderFileType::NES
            | dat_reader::enums::HeaderFileType::REQUIRED;

        let mut sc_file = ScannedFile::new(FileType::File);
        sc_file.name = "renamed.bin".to_string();
        sc_file.size = Some(4);
        sc_file.crc = Some(vec![0xAD, 0xF3, 0xF3, 0x63]);
        sc_file.header_file_type = dat_reader::enums::HeaderFileType::SNES;
        sc_file.deep_scanned = true;

        let (matched, alt) = FileCompare::phase_2_name_agnostic_test(&db_file, &mut sc_file);
        assert!(!matched);
        assert!(!alt);
    }

    #[test]
    fn test_phase_2_name_agnostic_test_propagates_deep_scan_status_flags() {
        let temp = tempdir().unwrap();
        let file_path = temp.path().join("renamed.nes");
        let mut bytes = vec![0x4E, 0x45, 0x53, 0x1A];
        bytes.extend_from_slice(&[0; 12]);
        bytes.extend_from_slice(b"DATA");
        let mut crc = crc32fast::Hasher::new();
        crc.update(&bytes);
        fs::write(&file_path, bytes).unwrap();

        let root = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
        root.borrow_mut().name = temp.path().to_string_lossy().to_string();

        let mut db_file = RvFile::new(FileType::File);
        db_file.name = "expected.nes".to_string();
        db_file.size = Some(20);
        db_file.crc = Some(crc.finalize().to_be_bytes().to_vec());
        db_file.parent = Some(Rc::downgrade(&root));

        let mut sc_file = ScannedFile::new(FileType::File);
        sc_file.name = "renamed.nes".to_string();

        let _ = FileCompare::phase_2_name_agnostic_test(&db_file, &mut sc_file);
        assert!(sc_file.status_flags.contains(crate::rv_file::FileStatus::HEADER_FILE_TYPE_FROM_HEADER));
        assert!(sc_file.status_flags.contains(crate::rv_file::FileStatus::ALT_CRC_FROM_HEADER));
    }

    #[test]
    fn test_phase_2_name_agnostic_test_rejects_size_only_identity() {
        let mut db_file = RvFile::new(FileType::File);
        db_file.name = "expected.bin".to_string();
        db_file.size = Some(1024);

        let mut sc_file = ScannedFile::new(FileType::File);
        sc_file.name = "renamed.bin".to_string();
        sc_file.size = Some(1024);
        sc_file.deep_scanned = true;

        let (matched, alt) = FileCompare::phase_2_name_agnostic_test(&db_file, &mut sc_file);
        assert!(!matched);
        assert!(!alt);
    }
}
