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
    fn test_compare_db_to_file_uses_windows_style_case_insensitive_ordering() {
        let mut db_file = RvFile::new(FileType::File);
        db_file.name = "B_File.zip".to_string();

        let mut sc_file = ScannedFile::new(FileType::File);
        sc_file.name = "a_file.zip".to_string();

        assert_eq!(compare_db_to_file(&db_file, &sc_file), 1);
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
    fn test_phase_1_test_uses_sha1_identity_even_without_crc() {
        let mut db_file = RvFile::new(FileType::File);
        db_file.name = "rom.bin".to_string();
        db_file.size = Some(1024);
        db_file.sha1 = Some(vec![0xAA, 0xBB, 0xCC, 0xDD]);
        db_file.file_mod_time_stamp = 123456;

        let mut sc_file = ScannedFile::new(FileType::File);
        sc_file.name = "rom.bin".to_string();
        sc_file.size = Some(1024);
        sc_file.sha1 = Some(vec![0x11, 0x22, 0x33, 0x44]);
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
    fn test_phase_1_test_primary_match_can_use_scanned_alt_hashes() {
        let mut db_file = RvFile::new(FileType::File);
        db_file.name = "rom.nes".to_string();
        db_file.size = Some(4);
        db_file.crc = Some(vec![0xAD, 0xF3, 0xF3, 0x63]);

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
    fn test_phase_1_test_primary_match_can_use_scanned_alt_sha1() {
        let mut db_file = RvFile::new(FileType::File);
        db_file.name = "rom.nes".to_string();
        db_file.size = Some(4);
        db_file.sha1 = Some(vec![0xAD, 0xF3, 0xF3, 0x63]);

        let mut sc_file = ScannedFile::new(FileType::File);
        sc_file.name = "rom.nes".to_string();
        sc_file.size = Some(20);
        sc_file.sha1 = Some(vec![0x00, 0x00, 0x00, 0x00]);
        sc_file.alt_size = Some(4);
        sc_file.alt_sha1 = Some(vec![0xAD, 0xF3, 0xF3, 0x63]);

        let (matched, alt) = FileCompare::phase_1_test(&db_file, &sc_file, EScanLevel::Level2, 0);
        assert!(matched);
        assert!(alt);
    }

    #[test]
    fn test_phase_2_name_agnostic_test_matches_renamed_file_by_alt_md5() {
        let mut db_file = RvFile::new(FileType::File);
        db_file.name = "expected.bin".to_string();
        db_file.alt_size = Some(4);
        db_file.alt_md5 = Some(vec![0x11, 0x22, 0x33, 0x44]);

        let mut sc_file = ScannedFile::new(FileType::File);
        sc_file.name = "renamed.bin".to_string();
        sc_file.size = Some(20);
        sc_file.md5 = Some(vec![0x00, 0x00, 0x00, 0x00]);
        sc_file.alt_size = Some(4);
        sc_file.alt_md5 = Some(vec![0x11, 0x22, 0x33, 0x44]);
        sc_file.deep_scanned = true;

        let (matched, alt) = FileCompare::phase_2_name_agnostic_test(&db_file, &mut sc_file);
        assert!(matched);
        assert!(alt);
    }

    #[test]
    fn test_phase_2_name_agnostic_test_matches_renamed_file_by_alt_sha1() {
        let mut db_file = RvFile::new(FileType::File);
        db_file.name = "expected.bin".to_string();
        db_file.alt_size = Some(4);
        db_file.alt_sha1 = Some(vec![0x11, 0x22, 0x33, 0x44]);

        let mut sc_file = ScannedFile::new(FileType::File);
        sc_file.name = "renamed.bin".to_string();
        sc_file.size = Some(20);
        sc_file.sha1 = Some(vec![0x00, 0x00, 0x00, 0x00]);
        sc_file.alt_size = Some(4);
        sc_file.alt_sha1 = Some(vec![0x11, 0x22, 0x33, 0x44]);
        sc_file.deep_scanned = true;

        let (matched, alt) = FileCompare::phase_2_name_agnostic_test(&db_file, &mut sc_file);
        assert!(matched);
        assert!(alt);
    }

    #[test]
    fn test_phase_1_test_prefers_primary_lane_when_scanned_file_has_both_primary_and_alt_crc() {
        let mut db_file = RvFile::new(FileType::File);
        db_file.name = "rom.bin".to_string();
        db_file.size = Some(4);
        db_file.crc = Some(vec![0xAA, 0xBB, 0xCC, 0xDD]);
        db_file.alt_size = Some(4);
        db_file.alt_crc = Some(vec![0x11, 0x22, 0x33, 0x44]);

        let mut sc_file = ScannedFile::new(FileType::File);
        sc_file.name = "rom.bin".to_string();
        sc_file.size = Some(4);
        sc_file.crc = Some(vec![0xAA, 0xBB, 0xCC, 0xDD]);
        sc_file.alt_size = Some(4);
        sc_file.alt_crc = Some(vec![0x11, 0x22, 0x33, 0x44]);

        let (matched, alt) = FileCompare::phase_1_test(&db_file, &sc_file, EScanLevel::Level2, 0);
        assert!(matched);
        assert!(!alt);
    }

    #[test]
    fn test_phase_2_name_agnostic_test_rejects_timestamp_fallback_when_alt_sha1_is_present_but_wrong() {
        let mut db_file = RvFile::new(FileType::File);
        db_file.name = "expected.bin".to_string();
        db_file.file_mod_time_stamp = 123456;
        db_file.alt_size = Some(4);
        db_file.alt_sha1 = Some(vec![0x11, 0x22, 0x33, 0x44]);

        let mut sc_file = ScannedFile::new(FileType::File);
        sc_file.name = "renamed.bin".to_string();
        sc_file.size = Some(20);
        sc_file.file_mod_time_stamp = 123456;
        sc_file.alt_size = Some(4);
        sc_file.alt_sha1 = Some(vec![0xAA, 0xBB, 0xCC, 0xDD]);
        sc_file.deep_scanned = true;

        let (matched, alt) = FileCompare::phase_2_name_agnostic_test(&db_file, &mut sc_file);
        assert!(!matched);
        assert!(!alt);
    }

    #[test]
    fn test_phase_1_test_level2_rejects_timestamp_fallback_when_alt_md5_is_present_but_wrong() {
        let mut db_file = RvFile::new(FileType::File);
        db_file.name = "rom.bin".to_string();
        db_file.file_mod_time_stamp = 123456;
        db_file.alt_size = Some(1024);
        db_file.alt_md5 = Some(vec![0x11, 0x22, 0x33, 0x44]);

        let mut sc_file = ScannedFile::new(FileType::File);
        sc_file.name = "rom.bin".to_string();
        sc_file.size = Some(2048);
        sc_file.alt_size = Some(1024);
        sc_file.file_mod_time_stamp = 123456;
        sc_file.alt_md5 = Some(vec![0xAA, 0xBB, 0xCC, 0xDD]);

        let (matched, alt) = FileCompare::phase_1_test(&db_file, &sc_file, EScanLevel::Level2, 0);
        assert!(!matched);
        assert!(!alt);
    }

    #[test]
    fn test_phase_1_test_level2_allows_alt_size_only_timestamp_match_without_hashes() {
        let mut db_file = RvFile::new(FileType::File);
        db_file.name = "rom.bin".to_string();
        db_file.alt_size = Some(1024);
        db_file.file_mod_time_stamp = 123456;

        let mut sc_file = ScannedFile::new(FileType::File);
        sc_file.name = "rom.bin".to_string();
        sc_file.size = Some(1024);
        sc_file.file_mod_time_stamp = 123456;

        let (matched, alt) = FileCompare::phase_1_test(&db_file, &sc_file, EScanLevel::Level2, 0);
        assert!(matched);
        assert!(alt);
    }

    #[test]
    fn test_phase_1_test_level2_allows_scanned_alt_size_only_timestamp_match_without_hashes() {
        let mut db_file = RvFile::new(FileType::File);
        db_file.name = "rom.bin".to_string();
        db_file.alt_size = Some(1024);
        db_file.file_mod_time_stamp = 123456;

        let mut sc_file = ScannedFile::new(FileType::File);
        sc_file.name = "rom.bin".to_string();
        sc_file.size = Some(1040);
        sc_file.alt_size = Some(1024);
        sc_file.file_mod_time_stamp = 123456;

        let (matched, alt) = FileCompare::phase_1_test(&db_file, &sc_file, EScanLevel::Level2, 0);
        assert!(matched);
        assert!(alt);
    }

    #[test]
    fn test_phase_1_test_level2_allows_primary_size_to_match_scanned_alt_size_without_hashes() {
        let mut db_file = RvFile::new(FileType::File);
        db_file.name = "rom.bin".to_string();
        db_file.size = Some(1024);
        db_file.file_mod_time_stamp = 123456;

        let mut sc_file = ScannedFile::new(FileType::File);
        sc_file.name = "rom.bin".to_string();
        sc_file.size = Some(1040);
        sc_file.alt_size = Some(1024);
        sc_file.file_mod_time_stamp = 123456;

        let (matched, alt) = FileCompare::phase_1_test(&db_file, &sc_file, EScanLevel::Level2, 0);
        assert!(matched);
        assert!(alt);
    }

    #[test]
    fn test_phase_1_test_matches_case_only_name_difference_when_index_case_enabled() {
        let mut db_file = RvFile::new(FileType::File);
        db_file.name = "ROM.BIN".to_string();
        db_file.size = Some(4);
        db_file.crc = Some(vec![0xAD, 0xF3, 0xF3, 0x63]);

        let mut sc_file = ScannedFile::new(FileType::File);
        sc_file.name = "rom.bin".to_string();
        sc_file.size = Some(4);
        sc_file.crc = Some(vec![0xAD, 0xF3, 0xF3, 0x63]);

        let (matched, alt) = FileCompare::phase_1_test(&db_file, &sc_file, EScanLevel::Level2, 1);
        assert!(matched);
        assert!(!alt);
    }

    #[test]
    fn test_phase_1_test_allows_archive_member_db_type_to_match_scanned_file_type() {
        let mut db_file = RvFile::new(FileType::FileZip);
        db_file.name = "rom.bin".to_string();
        db_file.size = Some(4);
        db_file.file_mod_time_stamp = 0;

        let sc_file = ScannedFile::new(FileType::File);

        let mut sc_file = sc_file;
        sc_file.name = "rom.bin".to_string();
        sc_file.size = Some(4);

        let (matched, alt) = FileCompare::phase_1_test(&db_file, &sc_file, EScanLevel::Level2, 0);
        assert!(matched);
        assert!(!alt);
    }

    #[test]
    fn test_phase_1_test_allows_fileonly_db_type_to_match_scanned_file_type() {
        let mut db_file = RvFile::new(FileType::FileOnly);
        db_file.name = "rom.bin".to_string();
        db_file.size = Some(4);
        db_file.file_mod_time_stamp = 0;

        let mut sc_file = ScannedFile::new(FileType::File);
        sc_file.name = "rom.bin".to_string();
        sc_file.size = Some(4);

        let (matched, alt) = FileCompare::phase_1_test(&db_file, &sc_file, EScanLevel::Level2, 0);
        assert!(matched);
        assert!(!alt);
    }

    #[test]
    fn test_phase_1_test_allows_plain_file_db_type_to_match_specialized_scanned_leaf_type() {
        let mut db_file = RvFile::new(FileType::File);
        db_file.name = "rom.bin".to_string();
        db_file.size = Some(4);
        db_file.crc = Some(vec![0xAA, 0xBB, 0xCC, 0xDD]);

        let mut sc_file = ScannedFile::new(FileType::FileZip);
        sc_file.name = "rom.bin".to_string();
        sc_file.size = Some(4);
        sc_file.crc = Some(vec![0xAA, 0xBB, 0xCC, 0xDD]);
        sc_file.deep_scanned = true;

        let (matched, alt) = FileCompare::phase_1_test(&db_file, &sc_file, EScanLevel::Level2, 0);
        assert!(matched);
        assert!(!alt);
    }

    #[test]
    fn test_phase_1_test_rejects_archive_member_hash_mismatch_against_scanned_file_type() {
        let mut db_file = RvFile::new(FileType::FileZip);
        db_file.name = "rom.bin".to_string();
        db_file.size = Some(4);
        db_file.crc = Some(vec![0xAA, 0xBB, 0xCC, 0xDD]);

        let mut sc_file = ScannedFile::new(FileType::File);
        sc_file.name = "rom.bin".to_string();
        sc_file.size = Some(4);
        sc_file.crc = Some(vec![0x11, 0x22, 0x33, 0x44]);

        let (matched, alt) = FileCompare::phase_1_test(&db_file, &sc_file, EScanLevel::Level2, 0);
        assert!(!matched);
        assert!(!alt);
    }

    #[test]
    fn test_phase_2_test_matches_case_only_name_difference_when_index_case_enabled() {
        let mut db_file = RvFile::new(FileType::File);
        db_file.name = "ROM.BIN".to_string();
        db_file.size = Some(4);
        db_file.crc = Some(vec![0xAD, 0xF3, 0xF3, 0x63]);

        let mut sc_file = ScannedFile::new(FileType::File);
        sc_file.name = "rom.bin".to_string();
        sc_file.size = Some(4);
        sc_file.crc = Some(vec![0xAD, 0xF3, 0xF3, 0x63]);
        sc_file.deep_scanned = true;

        let (matched, alt) = FileCompare::phase_2_test(&db_file, &mut sc_file, 1);
        assert!(matched);
        assert!(!alt);
    }

    #[test]
    fn test_phase_2_test_rejects_locked_file_instead_of_auto_matching() {
        let mut db_file = RvFile::new(FileType::File);
        db_file.name = "rom.bin".to_string();
        db_file.size = Some(4);
        db_file.crc = Some(vec![0xAD, 0xF3, 0xF3, 0x63]);

        let mut sc_file = ScannedFile::new(FileType::File);
        sc_file.name = "rom.bin".to_string();
        sc_file.got_status = dat_reader::enums::GotStatus::FileLocked;
        sc_file.deep_scanned = true;

        let (matched, alt) = FileCompare::phase_2_test(&db_file, &mut sc_file, 0);
        assert!(!matched);
        assert!(!alt);
    }

    #[test]
    fn test_phase_2_test_allows_size_only_exact_name_match_without_hash_identity() {
        let mut db_file = RvFile::new(FileType::File);
        db_file.name = "rom.bin".to_string();
        db_file.size = Some(1024);

        let mut sc_file = ScannedFile::new(FileType::File);
        sc_file.name = "rom.bin".to_string();
        sc_file.size = Some(1024);
        sc_file.deep_scanned = true;

        let (matched, alt) = FileCompare::phase_2_test(&db_file, &mut sc_file, 0);
        assert!(matched);
        assert!(!alt);
    }

    #[test]
    fn test_phase_2_test_allows_alt_size_only_exact_name_match_without_hash_identity() {
        let mut db_file = RvFile::new(FileType::File);
        db_file.name = "rom.bin".to_string();
        db_file.alt_size = Some(1024);

        let mut sc_file = ScannedFile::new(FileType::File);
        sc_file.name = "rom.bin".to_string();
        sc_file.size = Some(1024);
        sc_file.deep_scanned = true;

        let (matched, alt) = FileCompare::phase_2_test(&db_file, &mut sc_file, 0);
        assert!(matched);
        assert!(alt);
    }

    #[test]
    fn test_phase_2_test_allows_fileonly_size_only_exact_name_match_without_hash_identity() {
        let mut db_file = RvFile::new(FileType::FileOnly);
        db_file.name = "rom.bin".to_string();
        db_file.size = Some(1024);

        let mut sc_file = ScannedFile::new(FileType::FileOnly);
        sc_file.name = "rom.bin".to_string();
        sc_file.size = Some(1024);
        sc_file.deep_scanned = true;

        let (matched, alt) = FileCompare::phase_2_test(&db_file, &mut sc_file, 0);
        assert!(matched);
        assert!(!alt);
    }

    #[test]
    fn test_phase_2_test_allows_archive_member_size_only_exact_name_match_without_hash_identity() {
        let mut db_file = RvFile::new(FileType::FileZip);
        db_file.name = "rom.bin".to_string();
        db_file.size = Some(1024);

        let mut sc_file = ScannedFile::new(FileType::FileZip);
        sc_file.name = "rom.bin".to_string();
        sc_file.size = Some(1024);
        sc_file.deep_scanned = true;

        let (matched, alt) = FileCompare::phase_2_test(&db_file, &mut sc_file, 0);
        assert!(matched);
        assert!(!alt);
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
    fn test_phase_2_name_agnostic_test_matches_fileonly_renamed_file_by_hash() {
        let mut db_file = RvFile::new(FileType::FileOnly);
        db_file.name = "expected.bin".to_string();
        db_file.size = Some(4);
        db_file.crc = Some(vec![0xAD, 0xF3, 0xF3, 0x63]);

        let mut sc_file = ScannedFile::new(FileType::FileOnly);
        sc_file.name = "renamed.bin".to_string();
        sc_file.size = Some(4);
        sc_file.crc = Some(vec![0xAD, 0xF3, 0xF3, 0x63]);
        sc_file.deep_scanned = true;

        let (matched, alt) = FileCompare::phase_2_name_agnostic_test(&db_file, &mut sc_file);
        assert!(matched);
        assert!(!alt);
    }

    #[test]
    fn test_phase_2_name_agnostic_test_matches_archive_member_renamed_file_by_hash() {
        let mut db_file = RvFile::new(FileType::FileZip);
        db_file.name = "expected.bin".to_string();
        db_file.size = Some(4);
        db_file.crc = Some(vec![0xAD, 0xF3, 0xF3, 0x63]);

        let mut sc_file = ScannedFile::new(FileType::FileZip);
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
    fn test_phase_2_name_agnostic_test_deep_scans_using_existing_parent_directory_name() {
        let temp = tempdir().unwrap();
        let existing_dir = temp.path().join("olddir");
        fs::create_dir_all(&existing_dir).unwrap();
        fs::write(existing_dir.join("renamed.bin"), b"data").unwrap();

        let root = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
        root.borrow_mut().name = temp.path().to_string_lossy().to_string();

        let parent_dir = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
        {
            let mut dir = parent_dir.borrow_mut();
            dir.name = "NewDir".to_string();
            dir.file_name = "olddir".to_string();
            dir.parent = Some(Rc::downgrade(&root));
        }

        let mut db_file = RvFile::new(FileType::File);
        db_file.name = "expected.bin".to_string();
        db_file.size = Some(4);
        db_file.crc = Some(vec![0xAD, 0xF3, 0xF3, 0x63]);
        db_file.parent = Some(Rc::downgrade(&parent_dir));

        let mut sc_file = ScannedFile::new(FileType::File);
        sc_file.name = "renamed.bin".to_string();

        let (matched, alt) = FileCompare::phase_2_name_agnostic_test(&db_file, &mut sc_file);
        assert!(matched);
        assert!(!alt);
        assert!(sc_file.deep_scanned);
        assert_eq!(sc_file.crc, Some(vec![0xAD, 0xF3, 0xF3, 0x63]));
    }

    #[test]
    fn test_phase_2_test_deep_scans_specialized_scanned_leaf_type() {
        let temp = tempdir().unwrap();
        let file_path = temp.path().join("rom.bin");
        fs::write(&file_path, b"data").unwrap();

        let root = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
        root.borrow_mut().name = temp.path().to_string_lossy().to_string();

        let mut db_file = RvFile::new(FileType::FileZip);
        db_file.name = "rom.bin".to_string();
        db_file.size = Some(4);
        db_file.crc = Some(vec![0xAD, 0xF3, 0xF3, 0x63]);
        db_file.parent = Some(Rc::downgrade(&root));

        let mut sc_file = ScannedFile::new(FileType::FileZip);
        sc_file.name = "rom.bin".to_string();

        let (matched, alt) = FileCompare::phase_2_test(&db_file, &mut sc_file, 0);
        assert!(matched);
        assert!(!alt);
        assert!(sc_file.deep_scanned);
        assert_eq!(sc_file.crc, Some(vec![0xAD, 0xF3, 0xF3, 0x63]));
    }

    #[test]
    fn test_phase_2_test_rescans_hashless_deep_scanned_leaf() {
        let temp = tempdir().unwrap();
        let file_path = temp.path().join("rom.bin");
        fs::write(&file_path, b"data").unwrap();

        let root = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
        root.borrow_mut().name = temp.path().to_string_lossy().to_string();

        let mut db_file = RvFile::new(FileType::File);
        db_file.name = "rom.bin".to_string();
        db_file.size = Some(4);
        db_file.crc = Some(vec![0xAD, 0xF3, 0xF3, 0x63]);
        db_file.parent = Some(Rc::downgrade(&root));

        let mut sc_file = ScannedFile::new(FileType::File);
        sc_file.name = "rom.bin".to_string();
        sc_file.deep_scanned = true;

        let (matched, alt) = FileCompare::phase_2_test(&db_file, &mut sc_file, 0);
        assert!(matched);
        assert!(!alt);
        assert_eq!(sc_file.crc, Some(vec![0xAD, 0xF3, 0xF3, 0x63]));
    }

    #[test]
    fn test_phase_2_test_rescans_when_required_sha1_is_missing_from_scanned_identity() {
        let temp = tempdir().unwrap();
        let file_path = temp.path().join("rom.bin");
        fs::write(&file_path, b"data").unwrap();
        let scanned = crate::scanner::Scanner::scan_raw_file(&file_path.to_string_lossy()).unwrap();

        let root = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
        root.borrow_mut().name = temp.path().to_string_lossy().to_string();

        let mut db_file = RvFile::new(FileType::File);
        db_file.name = "rom.bin".to_string();
        db_file.size = Some(4);
        db_file.sha1 = scanned.sha1.clone();
        db_file.parent = Some(Rc::downgrade(&root));

        let mut sc_file = ScannedFile::new(FileType::File);
        sc_file.name = "rom.bin".to_string();
        sc_file.size = Some(4);
        sc_file.crc = scanned.crc.clone();
        sc_file.deep_scanned = true;

        let (matched, alt) = FileCompare::phase_2_test(&db_file, &mut sc_file, 0);
        assert!(matched);
        assert!(!alt);
        assert_eq!(sc_file.sha1, scanned.sha1);
    }

    #[test]
    fn test_phase_2_name_agnostic_test_rejects_size_only_identity_without_timestamp_confidence() {
        let mut db_file = RvFile::new(FileType::File);
        db_file.name = "expected.bin".to_string();
        db_file.size = Some(1024);
        db_file.file_mod_time_stamp = 123;

        let mut sc_file = ScannedFile::new(FileType::File);
        sc_file.name = "renamed.bin".to_string();
        sc_file.size = Some(1024);
        sc_file.file_mod_time_stamp = 456;
        sc_file.deep_scanned = true;

        let (matched, alt) = FileCompare::phase_2_name_agnostic_test(&db_file, &mut sc_file);
        assert!(!matched);
        assert!(!alt);
    }

    #[test]
    fn test_phase_2_name_agnostic_test_allows_size_only_identity_with_timestamp_confidence() {
        let mut db_file = RvFile::new(FileType::File);
        db_file.name = "expected.bin".to_string();
        db_file.alt_size = Some(1024);
        db_file.file_mod_time_stamp = 123456;

        let mut sc_file = ScannedFile::new(FileType::File);
        sc_file.name = "renamed.bin".to_string();
        sc_file.size = Some(1024);
        sc_file.file_mod_time_stamp = 123456;
        sc_file.deep_scanned = true;

        let (matched, alt) = FileCompare::phase_2_name_agnostic_test(&db_file, &mut sc_file);
        assert!(matched);
        assert!(alt);
    }
