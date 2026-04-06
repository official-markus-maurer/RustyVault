    use super::*;
    use crate::file_scanning::FileScanning;
    use crate::scanner::Scanner;
    use crate::settings::{get_settings, set_dir_mapping, update_settings, DirMapping, Settings};
    use dat_reader::enums::FileType;
    use std::fs;
    use std::io::Write;
    use tempfile::tempdir;
    use zip::write::SimpleFileOptions;
    use zip::CompressionMethod;
    use zip::ZipWriter;

    #[test]
    fn test_find_fixes_exact_crc_match() {
        let root = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
        
        // Mock a ToSort directory with a Got file
        let to_sort = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
        to_sort.borrow_mut().set_dat_status(DatStatus::InToSort);
        
        let mut got_file = RvFile::new(FileType::File);
        got_file.name = "got_file.bin".to_string();
        got_file.size = Some(1024);
        got_file.crc = Some(vec![0xAA, 0xBB, 0xCC, 0xDD]);
        got_file.set_dat_status(DatStatus::InToSort);
        got_file.set_got_status(GotStatus::Got);
        let got_rc = Rc::new(RefCell::new(got_file));
        to_sort.borrow_mut().child_add(Rc::clone(&got_rc));
        
        // Mock a DatRoot directory with a Missing file
        let dat_root = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
        dat_root.borrow_mut().set_dat_status(DatStatus::InDatCollect);
        
        let mut missing_file = RvFile::new(FileType::File);
        missing_file.name = "missing_file.bin".to_string();
        missing_file.size = Some(1024);
        missing_file.crc = Some(vec![0xAA, 0xBB, 0xCC, 0xDD]); // Exact CRC Match
        missing_file.set_dat_status(DatStatus::InDatCollect);
        missing_file.set_got_status(GotStatus::NotGot);
        let missing_rc = Rc::new(RefCell::new(missing_file));
        dat_root.borrow_mut().child_add(Rc::clone(&missing_rc));

        root.borrow_mut().child_add(to_sort);
        root.borrow_mut().child_add(dat_root);

        FindFixes::scan_files(Rc::clone(&root));

        // Missing file should now be flagged as CanBeFixed
        assert_eq!(missing_rc.borrow().rep_status(), RepStatus::CanBeFixed);
        // Got file should be flagged as NeededForFix
        assert_eq!(got_rc.borrow().rep_status(), RepStatus::NeededForFix);
    }

    #[test]
    fn test_find_fixes_can_use_unselected_tosort_sources() {
        let root = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));

        let to_sort = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
        {
            let mut dir = to_sort.borrow_mut();
            dir.set_dat_status(DatStatus::InToSort);
            dir.tree_checked = TreeSelect::UnSelected;
        }

        let got_rc = Rc::new(RefCell::new(RvFile::new(FileType::File)));
        {
            let mut got_file = got_rc.borrow_mut();
            got_file.name = "got_file.bin".to_string();
            got_file.size = Some(1024);
            got_file.crc = Some(vec![0xAA, 0xBB, 0xCC, 0xDD]);
            got_file.set_dat_got_status(DatStatus::InToSort, GotStatus::Got);
            got_file.tree_checked = TreeSelect::UnSelected;
        }
        to_sort.borrow_mut().child_add(Rc::clone(&got_rc));

        let dat_root = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
        dat_root.borrow_mut().set_dat_status(DatStatus::InDatCollect);

        let missing_rc = Rc::new(RefCell::new(RvFile::new(FileType::File)));
        {
            let mut missing_file = missing_rc.borrow_mut();
            missing_file.name = "missing_file.bin".to_string();
            missing_file.size = Some(1024);
            missing_file.crc = Some(vec![0xAA, 0xBB, 0xCC, 0xDD]);
            missing_file.set_dat_got_status(DatStatus::InDatCollect, GotStatus::NotGot);
            missing_file.tree_checked = TreeSelect::Selected;
        }
        dat_root.borrow_mut().child_add(Rc::clone(&missing_rc));

        root.borrow_mut().child_add(to_sort);
        root.borrow_mut().child_add(dat_root);

        FindFixes::scan_files(Rc::clone(&root));

        assert_eq!(missing_rc.borrow().rep_status(), RepStatus::CanBeFixed);
        assert_eq!(got_rc.borrow().rep_status(), RepStatus::NeededForFix);
    }

    #[test]
    fn test_find_fixes_header_required_needs_header_from_header() {
        use crate::rv_file::FileStatus;
        use dat_reader::enums::HeaderFileType;

        let root = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));

        let got1 = Rc::new(RefCell::new(RvFile::new(FileType::File)));
        {
            let mut f = got1.borrow_mut();
            f.name = "got1.bin".to_string();
            f.size = Some(1024);
            f.crc = Some(vec![0xAA, 0xBB, 0xCC, 0xDD]);
            f.sha1 = Some(vec![1; 20]);
            f.set_header_file_type(HeaderFileType::NES);
            f.deep_scanned = true;
            f.set_dat_got_status(DatStatus::NotInDat, GotStatus::Got);
            f.rep_status_reset();
        }

        let got2 = Rc::new(RefCell::new(RvFile::new(FileType::File)));
        {
            let mut f = got2.borrow_mut();
            f.name = "got2.bin".to_string();
            f.size = Some(1024);
            f.crc = Some(vec![0xAA, 0xBB, 0xCC, 0xDD]);
            f.sha1 = Some(vec![1; 20]);
            f.set_header_file_type(HeaderFileType::NES);
            f.deep_scanned = true;
            f.file_status_set(FileStatus::HEADER_FILE_TYPE_FROM_HEADER);
            f.set_dat_got_status(DatStatus::NotInDat, GotStatus::Got);
            f.rep_status_reset();
        }

        let missing = Rc::new(RefCell::new(RvFile::new(FileType::File)));
        {
            let mut f = missing.borrow_mut();
            f.name = "missing.bin".to_string();
            f.size = Some(1024);
            f.crc = Some(vec![0xAA, 0xBB, 0xCC, 0xDD]);
            f.sha1 = Some(vec![1; 20]);
            f.set_header_file_type(HeaderFileType::NES | HeaderFileType::REQUIRED);
            f.file_status_set(FileStatus::SHA1_FROM_DAT);
            f.set_dat_got_status(DatStatus::InDatCollect, GotStatus::NotGot);
            f.rep_status_reset();
        }

        root.borrow_mut().child_add(Rc::clone(&got1));
        root.borrow_mut().child_add(Rc::clone(&got2));
        root.borrow_mut().child_add(Rc::clone(&missing));

        FindFixes::scan_files(Rc::clone(&root));

        assert_eq!(missing.borrow().rep_status(), RepStatus::CanBeFixed);
        assert_ne!(got1.borrow().rep_status(), RepStatus::NeededForFix);
        assert_eq!(got2.borrow().rep_status(), RepStatus::NeededForFix);
    }

    #[test]
    fn test_find_fixes_sets_rename_for_in_archive_match() {
        let root = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));

        let zip = Rc::new(RefCell::new(RvFile::new(FileType::Zip)));
        zip.borrow_mut().tree_checked = TreeSelect::Selected;
        zip.borrow_mut().set_dat_status(DatStatus::InDatCollect);

        let got = Rc::new(RefCell::new(RvFile::new(FileType::FileZip)));
        {
            let mut f = got.borrow_mut();
            f.name = "a.bin".to_string();
            f.size = Some(1024);
            f.crc = Some(vec![0x12, 0x34, 0x56, 0x78]);
            f.set_dat_got_status(DatStatus::NotInDat, GotStatus::Got);
            f.tree_checked = TreeSelect::Selected;
            f.rep_status_reset();
        }
        zip.borrow_mut().child_add(Rc::clone(&got));
        got.borrow_mut().parent = Some(Rc::downgrade(&zip));

        let missing = Rc::new(RefCell::new(RvFile::new(FileType::FileZip)));
        {
            let mut f = missing.borrow_mut();
            f.name = "b.bin".to_string();
            f.size = Some(1024);
            f.crc = Some(vec![0x12, 0x34, 0x56, 0x78]);
            f.set_dat_got_status(DatStatus::InDatCollect, GotStatus::NotGot);
            f.tree_checked = TreeSelect::Selected;
            f.rep_status_reset();
        }
        zip.borrow_mut().child_add(Rc::clone(&missing));
        missing.borrow_mut().parent = Some(Rc::downgrade(&zip));

        root.borrow_mut().child_add(Rc::clone(&zip));

        FindFixes::scan_files(Rc::clone(&root));

        assert_eq!(missing.borrow().rep_status(), RepStatus::CanBeFixed);
        assert_eq!(got.borrow().rep_status(), RepStatus::Rename);
    }

    #[test]
    fn test_find_fixes_candidate_selection_prefers_tosort_over_notindat() {
        let root = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));

        let to_sort = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
        to_sort.borrow_mut().set_dat_status(DatStatus::InToSort);
        to_sort.borrow_mut().tree_checked = TreeSelect::Selected;

        let tosort_got = Rc::new(RefCell::new(RvFile::new(FileType::File)));
        {
            let mut f = tosort_got.borrow_mut();
            f.name = "tosort.bin".to_string();
            f.size = Some(1024);
            f.crc = Some(vec![0x12, 0x34, 0x56, 0x78]);
            f.set_dat_got_status(DatStatus::InToSort, GotStatus::Got);
            f.tree_checked = TreeSelect::Selected;
            f.rep_status_reset();
        }
        to_sort.borrow_mut().child_add(Rc::clone(&tosort_got));

        let unknown_got = Rc::new(RefCell::new(RvFile::new(FileType::File)));
        {
            let mut f = unknown_got.borrow_mut();
            f.name = "unknown.bin".to_string();
            f.size = Some(1024);
            f.crc = Some(vec![0x12, 0x34, 0x56, 0x78]);
            f.set_dat_got_status(DatStatus::NotInDat, GotStatus::Got);
            f.tree_checked = TreeSelect::Selected;
            f.rep_status_reset();
        }

        let dat_root = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
        dat_root.borrow_mut().set_dat_status(DatStatus::InDatCollect);
        dat_root.borrow_mut().tree_checked = TreeSelect::Selected;

        let missing = Rc::new(RefCell::new(RvFile::new(FileType::File)));
        {
            let mut m = missing.borrow_mut();
            m.name = "missing.bin".to_string();
            m.size = Some(1024);
            m.crc = Some(vec![0x12, 0x34, 0x56, 0x78]);
            m.set_dat_got_status(DatStatus::InDatCollect, GotStatus::NotGot);
            m.tree_checked = TreeSelect::Selected;
            m.rep_status_reset();
        }
        dat_root.borrow_mut().child_add(Rc::clone(&missing));

        root.borrow_mut().child_add(Rc::clone(&to_sort));
        root.borrow_mut().child_add(Rc::clone(&dat_root));
        root.borrow_mut().child_add(Rc::clone(&unknown_got));

        FindFixes::scan_files(Rc::clone(&root));

        assert_eq!(missing.borrow().rep_status(), RepStatus::CanBeFixed);
        assert_eq!(tosort_got.borrow().rep_status(), RepStatus::NeededForFix);
        assert_eq!(unknown_got.borrow().rep_status(), RepStatus::MoveToSort);
    }

    #[test]
    fn test_find_fixes_merged_cleanup_uses_not_collected_when_shared_physical_backing_retained() {
        let original_settings = get_settings();
        let temp = tempdir().unwrap();
        let shared = temp.path().join("Shared");
        fs::create_dir_all(&shared).unwrap();

        let mut settings = Settings::default();
        settings.dir_mappings.items.clear();
        settings.dir_mappings.items.push(DirMapping {
            dir_key: "A".to_string(),
            dir_path: shared.to_string_lossy().into_owned(),
        });
        settings.dir_mappings.items.push(DirMapping {
            dir_key: "B".to_string(),
            dir_path: shared.to_string_lossy().into_owned(),
        });
        update_settings(settings);

        let root = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
        root.borrow_mut().name = "".to_string();

        let a_dir = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
        a_dir.borrow_mut().name = "A".to_string();
        a_dir.borrow_mut().tree_checked = TreeSelect::Selected;
        a_dir.borrow_mut().parent = Some(Rc::downgrade(&root));
        root.borrow_mut().child_add(Rc::clone(&a_dir));

        let b_dir = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
        b_dir.borrow_mut().name = "B".to_string();
        b_dir.borrow_mut().tree_checked = TreeSelect::Selected;
        b_dir.borrow_mut().parent = Some(Rc::downgrade(&root));
        root.borrow_mut().child_add(Rc::clone(&b_dir));

        let retained = Rc::new(RefCell::new(RvFile::new(FileType::File)));
        {
            let mut f = retained.borrow_mut();
            f.name = "same.bin".to_string();
            f.size = Some(1);
            f.crc = Some(vec![1, 2, 3, 4]);
            f.set_dat_got_status(DatStatus::InDatCollect, GotStatus::Got);
            f.tree_checked = TreeSelect::Selected;
            f.parent = Some(Rc::downgrade(&a_dir));
            f.rep_status_reset();
        }
        a_dir.borrow_mut().child_add(Rc::clone(&retained));

        let merged = Rc::new(RefCell::new(RvFile::new(FileType::File)));
        {
            let mut f = merged.borrow_mut();
            f.name = "same.bin".to_string();
            f.size = Some(1);
            f.crc = Some(vec![1, 2, 3, 4]);
            f.set_dat_got_status(DatStatus::InDatMerged, GotStatus::Got);
            f.tree_checked = TreeSelect::Selected;
            f.parent = Some(Rc::downgrade(&b_dir));
            f.rep_status_reset();
        }
        b_dir.borrow_mut().child_add(Rc::clone(&merged));

        FindFixes::scan_files(Rc::clone(&root));

        assert_eq!(retained.borrow().rep_status(), RepStatus::Correct);
        assert_eq!(merged.borrow().rep_status(), RepStatus::NotCollected);

        update_settings(original_settings);
    }

    #[test]
    fn test_find_fixes_zero_length_file_is_treated_as_redundant_match() {
        let root = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));

        let dat_zero = Rc::new(RefCell::new(RvFile::new(FileType::File)));
        {
            let mut f = dat_zero.borrow_mut();
            f.name = "zero.bin".to_string();
            f.size = Some(0);
            f.crc = Some(vec![0, 0, 0, 0]);
            f.sha1 = Some(vec![0xDA, 0x39, 0xA3, 0xEE, 0x5E, 0x6B, 0x4B, 0x0D, 0x32, 0x55, 0xBF, 0xEF, 0x95, 0x60, 0x18, 0x90, 0xAF, 0xD8, 0x07, 0x09]);
            f.md5 = Some(vec![0xD4, 0x1D, 0x8C, 0xD9, 0x8F, 0x00, 0xB2, 0x04, 0xE9, 0x80, 0x09, 0x98, 0xEC, 0xF8, 0x42, 0x7E]);
            f.deep_scanned = true;
            f.set_dat_got_status(DatStatus::InDatCollect, GotStatus::Got);
            f.tree_checked = TreeSelect::Selected;
            f.rep_status_reset();
        }

        let unknown_zero = Rc::new(RefCell::new(RvFile::new(FileType::File)));
        {
            let mut f = unknown_zero.borrow_mut();
            f.name = "unknown_zero.bin".to_string();
            f.size = Some(0);
            f.crc = Some(vec![0, 0, 0, 0]);
            f.sha1 = Some(vec![0xDA, 0x39, 0xA3, 0xEE, 0x5E, 0x6B, 0x4B, 0x0D, 0x32, 0x55, 0xBF, 0xEF, 0x95, 0x60, 0x18, 0x90, 0xAF, 0xD8, 0x07, 0x09]);
            f.md5 = Some(vec![0xD4, 0x1D, 0x8C, 0xD9, 0x8F, 0x00, 0xB2, 0x04, 0xE9, 0x80, 0x09, 0x98, 0xEC, 0xF8, 0x42, 0x7E]);
            f.deep_scanned = true;
            f.set_dat_got_status(DatStatus::NotInDat, GotStatus::Got);
            f.tree_checked = TreeSelect::Selected;
            f.rep_status_reset();
        }

        root.borrow_mut().child_add(Rc::clone(&dat_zero));
        root.borrow_mut().child_add(Rc::clone(&unknown_zero));

        FindFixes::scan_files(Rc::clone(&root));

        assert_eq!(dat_zero.borrow().rep_status(), RepStatus::Correct);
        assert_eq!(unknown_zero.borrow().rep_status(), RepStatus::Delete);
    }

    #[test]
    fn test_find_fixes_matching() {
        let root = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
        
        // Setup a missing ROM
        let missing = Rc::new(RefCell::new(RvFile::new(FileType::File)));
        {
            let mut m = missing.borrow_mut();
            m.name = "game.rom".to_string();
            m.size = Some(1024);
            m.crc = Some(vec![0x12, 0x34, 0x56, 0x78]);
            m.set_dat_got_status(DatStatus::InDatCollect, GotStatus::NotGot);
        }
        
        // Setup an unknown ROM that matches the missing ROM
        let unknown = Rc::new(RefCell::new(RvFile::new(FileType::File)));
        {
            let mut u = unknown.borrow_mut();
            u.name = "random.bin".to_string();
            u.size = Some(1024);
            u.crc = Some(vec![0x12, 0x34, 0x56, 0x78]);
            u.set_dat_got_status(DatStatus::NotInDat, GotStatus::Got);
        }

        root.borrow_mut().child_add(Rc::clone(&missing));
        root.borrow_mut().child_add(Rc::clone(&unknown));

        FindFixes::scan_files(Rc::clone(&root));

        // Missing should now be marked CanBeFixed
        assert_eq!(missing.borrow().rep_status(), RepStatus::CanBeFixed);
        
        // Unknown should be marked NeededForFix
        assert_eq!(unknown.borrow().rep_status(), RepStatus::NeededForFix);
    }

    #[test]
    fn test_find_fixes_unneeded() {
        let root = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
        
        // Setup an unknown ROM that DOES NOT match anything
        let unknown = Rc::new(RefCell::new(RvFile::new(FileType::File)));
        {
            let mut u = unknown.borrow_mut();
            u.name = "junk.txt".to_string();
            u.size = Some(123);
            u.crc = Some(vec![0xFF, 0xFF]);
            u.set_dat_got_status(DatStatus::NotInDat, GotStatus::Got);
        }

        root.borrow_mut().child_add(Rc::clone(&unknown));

        FindFixes::scan_files(Rc::clone(&root));

        // Unknown should be marked MoveToSort since it's not needed
        assert_eq!(unknown.borrow().rep_status(), RepStatus::MoveToSort);
    }

    #[test]
    fn test_find_fixes_fallback_sha1_match() {
        let root = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
        
        // Setup a missing ROM with NO CRC, only SHA1
        let missing = Rc::new(RefCell::new(RvFile::new(FileType::File)));
        {
            let mut m = missing.borrow_mut();
            m.name = "game.rom".to_string();
            m.size = Some(1024);
            m.crc = None; // No CRC
            m.sha1 = Some(vec![
                0xAA, 0xBB, 0xCC, 0xDD, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            ]);
            m.set_dat_got_status(DatStatus::InDatCollect, GotStatus::NotGot);
        }
        
        // Setup an unknown ROM with NO CRC, only SHA1
        let unknown = Rc::new(RefCell::new(RvFile::new(FileType::File)));
        {
            let mut u = unknown.borrow_mut();
            u.name = "random.bin".to_string();
            u.size = Some(1024);
            u.crc = None; // No CRC
            u.sha1 = Some(vec![
                0xAA, 0xBB, 0xCC, 0xDD, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            ]);
            u.set_dat_got_status(DatStatus::NotInDat, GotStatus::Got);
        }

        root.borrow_mut().child_add(Rc::clone(&missing));
        root.borrow_mut().child_add(Rc::clone(&unknown));

        FindFixes::scan_files(Rc::clone(&root));

        // Missing should now be marked CanBeFixed via SHA1 fallback
        assert_eq!(missing.borrow().rep_status(), RepStatus::CanBeFixed);
        
        // Unknown should be marked NeededForFix
        assert_eq!(unknown.borrow().rep_status(), RepStatus::NeededForFix);
    }

    #[test]
    fn test_find_fixes_fallback_md5_match() {
        let root = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
        
        // Setup a missing ROM with NO CRC/SHA1, only MD5
        let missing = Rc::new(RefCell::new(RvFile::new(FileType::File)));
        {
            let mut m = missing.borrow_mut();
            m.name = "game.rom".to_string();
            m.size = Some(1024);
            m.crc = None;
            m.sha1 = None;
            m.md5 = Some(vec![0x11, 0x22, 0x33, 0x44, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
            m.set_dat_got_status(DatStatus::InDatCollect, GotStatus::NotGot);
        }
        
        // Setup an unknown ROM with NO CRC/SHA1, only MD5
        let unknown = Rc::new(RefCell::new(RvFile::new(FileType::File)));
        {
            let mut u = unknown.borrow_mut();
            u.name = "random.bin".to_string();
            u.size = Some(1024);
            u.crc = None;
            u.sha1 = None;
            u.md5 = Some(vec![0x11, 0x22, 0x33, 0x44, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
            u.set_dat_got_status(DatStatus::NotInDat, GotStatus::Got);
        }

        root.borrow_mut().child_add(Rc::clone(&missing));
        root.borrow_mut().child_add(Rc::clone(&unknown));

        FindFixes::scan_files(Rc::clone(&root));

        // Missing should now be marked CanBeFixed via MD5 fallback
        assert_eq!(missing.borrow().rep_status(), RepStatus::CanBeFixed);
        
        // Unknown should be marked NeededForFix
        assert_eq!(unknown.borrow().rep_status(), RepStatus::NeededForFix);
    }

    // Zero-length edge cases are covered via fix pipeline; matching-only special-case
    // is deferred to deep scan contexts and in-dat comparisons.
    #[test]
    fn test_find_fixes_ignores_unselected_source_file() {
        let root = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));

        let source_dir = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
        source_dir.borrow_mut().tree_checked = TreeSelect::Selected;

        let source_file = Rc::new(RefCell::new(RvFile::new(FileType::File)));
        {
            let mut f = source_file.borrow_mut();
            f.name = "source.bin".to_string();
            f.size = Some(1024);
            f.crc = Some(vec![0x10, 0x20, 0x30, 0x40]);
            f.set_dat_got_status(DatStatus::NotInDat, GotStatus::Got);
            f.tree_checked = TreeSelect::UnSelected;
        }
        source_dir.borrow_mut().child_add(Rc::clone(&source_file));

        let target_dir = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
        target_dir.borrow_mut().tree_checked = TreeSelect::Selected;

        let missing_file = Rc::new(RefCell::new(RvFile::new(FileType::File)));
        {
            let mut f = missing_file.borrow_mut();
            f.name = "missing.bin".to_string();
            f.size = Some(1024);
            f.crc = Some(vec![0x10, 0x20, 0x30, 0x40]);
            f.set_dat_got_status(DatStatus::InDatCollect, GotStatus::NotGot);
            f.tree_checked = TreeSelect::Selected;
        }
        target_dir.borrow_mut().child_add(Rc::clone(&missing_file));

        root.borrow_mut().child_add(source_dir);
        root.borrow_mut().child_add(target_dir);

        FindFixes::scan_files(Rc::clone(&root));

        assert_eq!(missing_file.borrow().rep_status(), RepStatus::Missing);
        assert_eq!(source_file.borrow().rep_status(), RepStatus::Unknown);
    }

    #[test]
    fn test_find_fixes_allows_locked_source_branch() {
        let root = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));

        let source_dir = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
        source_dir.borrow_mut().tree_checked = TreeSelect::Selected;

        let source_file = Rc::new(RefCell::new(RvFile::new(FileType::File)));
        {
            let mut f = source_file.borrow_mut();
            f.name = "source.bin".to_string();
            f.size = Some(1024);
            f.crc = Some(vec![0x10, 0x20, 0x30, 0x40]);
            f.set_dat_got_status(DatStatus::NotInDat, GotStatus::Got);
            f.tree_checked = TreeSelect::Selected;
        }
        source_dir.borrow_mut().child_add(Rc::clone(&source_file));

        let target_dir = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
        target_dir.borrow_mut().tree_checked = TreeSelect::Selected;

        let missing_file = Rc::new(RefCell::new(RvFile::new(FileType::File)));
        {
            let mut f = missing_file.borrow_mut();
            f.name = "missing.bin".to_string();
            f.size = Some(1024);
            f.crc = Some(vec![0x10, 0x20, 0x30, 0x40]);
            f.set_dat_got_status(DatStatus::InDatCollect, GotStatus::NotGot);
            f.tree_checked = TreeSelect::Selected;
        }
        target_dir.borrow_mut().child_add(Rc::clone(&missing_file));

        root.borrow_mut().child_add(source_dir);
        root.borrow_mut().child_add(target_dir);

        FindFixes::scan_files(Rc::clone(&root));

        assert_eq!(missing_file.borrow().rep_status(), RepStatus::CanBeFixed);
        assert_eq!(source_file.borrow().rep_status(), RepStatus::NeededForFix);
    }

    #[test]
    fn test_find_fixes_prefers_existing_dat_file_over_tosort_duplicate_source() {
        let root = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));

        let correct_dir = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
        correct_dir.borrow_mut().tree_checked = TreeSelect::Selected;

        let correct_file = Rc::new(RefCell::new(RvFile::new(FileType::File)));
        {
            let mut f = correct_file.borrow_mut();
            f.name = "correct.bin".to_string();
            f.size = Some(1024);
            f.crc = Some(vec![0x10, 0x20, 0x30, 0x40]);
            f.set_dat_got_status(DatStatus::InDatCollect, GotStatus::Got);
            f.tree_checked = TreeSelect::Selected;
        }
        correct_dir.borrow_mut().child_add(Rc::clone(&correct_file));

        let tosort_dir = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
        tosort_dir.borrow_mut().tree_checked = TreeSelect::Selected;

        let tosort_file = Rc::new(RefCell::new(RvFile::new(FileType::File)));
        {
            let mut f = tosort_file.borrow_mut();
            f.name = "spare.bin".to_string();
            f.size = Some(1024);
            f.crc = Some(vec![0x10, 0x20, 0x30, 0x40]);
            f.set_dat_got_status(DatStatus::InToSort, GotStatus::Got);
            f.tree_checked = TreeSelect::Selected;
        }
        tosort_dir.borrow_mut().child_add(Rc::clone(&tosort_file));

        let target_dir = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
        target_dir.borrow_mut().tree_checked = TreeSelect::Selected;

        let missing_file = Rc::new(RefCell::new(RvFile::new(FileType::File)));
        {
            let mut f = missing_file.borrow_mut();
            f.name = "missing.bin".to_string();
            f.size = Some(1024);
            f.crc = Some(vec![0x10, 0x20, 0x30, 0x40]);
            f.set_dat_got_status(DatStatus::InDatCollect, GotStatus::NotGot);
            f.tree_checked = TreeSelect::Selected;
        }
        target_dir.borrow_mut().child_add(Rc::clone(&missing_file));

        root.borrow_mut().child_add(correct_dir);
        root.borrow_mut().child_add(tosort_dir);
        root.borrow_mut().child_add(target_dir);

        FindFixes::scan_files(Rc::clone(&root));

        assert_eq!(missing_file.borrow().rep_status(), RepStatus::CanBeFixed);
        assert_eq!(tosort_file.borrow().rep_status(), RepStatus::Delete);
        assert_eq!(correct_file.borrow().rep_status(), RepStatus::Correct);
    }

    #[test]
    fn test_find_fixes_prefers_existing_dat_file_over_merged_duplicate_source() {
        let root = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));

        let correct_dir = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
        correct_dir.borrow_mut().tree_checked = TreeSelect::Selected;

        let correct_file = Rc::new(RefCell::new(RvFile::new(FileType::File)));
        {
            let mut f = correct_file.borrow_mut();
            f.name = "correct.bin".to_string();
            f.size = Some(1024);
            f.crc = Some(vec![0x10, 0x20, 0x30, 0x40]);
            f.set_dat_got_status(DatStatus::InDatCollect, GotStatus::Got);
            f.tree_checked = TreeSelect::Selected;
        }
        correct_dir.borrow_mut().child_add(Rc::clone(&correct_file));

        let merged_dir = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
        merged_dir.borrow_mut().tree_checked = TreeSelect::Selected;

        let merged_file = Rc::new(RefCell::new(RvFile::new(FileType::File)));
        {
            let mut f = merged_file.borrow_mut();
            f.name = "merged.bin".to_string();
            f.size = Some(1024);
            f.crc = Some(vec![0x10, 0x20, 0x30, 0x40]);
            f.set_dat_got_status(DatStatus::InDatMerged, GotStatus::Got);
            f.tree_checked = TreeSelect::Selected;
        }
        merged_dir.borrow_mut().child_add(Rc::clone(&merged_file));

        let target_dir = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
        target_dir.borrow_mut().tree_checked = TreeSelect::Selected;

        let missing_file = Rc::new(RefCell::new(RvFile::new(FileType::File)));
        {
            let mut f = missing_file.borrow_mut();
            f.name = "missing.bin".to_string();
            f.size = Some(1024);
            f.crc = Some(vec![0x10, 0x20, 0x30, 0x40]);
            f.set_dat_got_status(DatStatus::InDatCollect, GotStatus::NotGot);
            f.tree_checked = TreeSelect::Selected;
        }
        target_dir.borrow_mut().child_add(Rc::clone(&missing_file));

        root.borrow_mut().child_add(correct_dir);
        root.borrow_mut().child_add(merged_dir);
        root.borrow_mut().child_add(target_dir);

        FindFixes::scan_files(Rc::clone(&root));

        assert_eq!(missing_file.borrow().rep_status(), RepStatus::CanBeFixed);
        assert_eq!(merged_file.borrow().rep_status(), RepStatus::UnNeeded);
        assert_eq!(correct_file.borrow().rep_status(), RepStatus::Correct);
    }

    #[test]
    fn test_find_fixes_prefers_tosort_source_over_notindat_source_when_priority_is_otherwise_equal() {
        let root = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));

        let tosort_dir = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
        tosort_dir.borrow_mut().tree_checked = TreeSelect::Selected;

        let tosort_source = Rc::new(RefCell::new(RvFile::new(FileType::File)));
        {
            let mut f = tosort_source.borrow_mut();
            f.name = "tosort.bin".to_string();
            f.size = Some(1024);
            f.crc = Some(vec![0x55, 0x66, 0x77, 0x88]);
            f.set_dat_got_status(DatStatus::InToSort, GotStatus::Got);
            f.tree_checked = TreeSelect::Selected;
        }
        tosort_dir.borrow_mut().child_add(Rc::clone(&tosort_source));

        let unknown_dir = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
        unknown_dir.borrow_mut().tree_checked = TreeSelect::Selected;

        let unknown_source = Rc::new(RefCell::new(RvFile::new(FileType::File)));
        {
            let mut f = unknown_source.borrow_mut();
            f.name = "unknown.bin".to_string();
            f.size = Some(1024);
            f.crc = Some(vec![0x55, 0x66, 0x77, 0x88]);
            f.set_dat_got_status(DatStatus::NotInDat, GotStatus::Got);
            f.tree_checked = TreeSelect::Selected;
        }
        unknown_dir.borrow_mut().child_add(Rc::clone(&unknown_source));

        let target_dir = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
        target_dir.borrow_mut().tree_checked = TreeSelect::Selected;

        let missing_file = Rc::new(RefCell::new(RvFile::new(FileType::File)));
        {
            let mut f = missing_file.borrow_mut();
            f.name = "missing.bin".to_string();
            f.size = Some(1024);
            f.crc = Some(vec![0x55, 0x66, 0x77, 0x88]);
            f.set_dat_got_status(DatStatus::InDatCollect, GotStatus::NotGot);
            f.tree_checked = TreeSelect::Selected;
        }
        target_dir.borrow_mut().child_add(Rc::clone(&missing_file));

        root.borrow_mut().child_add(tosort_dir);
        root.borrow_mut().child_add(unknown_dir);
        root.borrow_mut().child_add(target_dir);

        FindFixes::scan_files(Rc::clone(&root));

        assert_eq!(missing_file.borrow().rep_status(), RepStatus::CanBeFixed);
        assert_eq!(tosort_source.borrow().rep_status(), RepStatus::NeededForFix);
        assert_eq!(unknown_source.borrow().rep_status(), RepStatus::MoveToSort);
    }

    #[test]
    fn test_find_fixes_prefers_merged_source_over_notindat_source_when_priority_is_otherwise_equal() {
        let root = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));

        let merged_dir = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
        merged_dir.borrow_mut().tree_checked = TreeSelect::Selected;

        let merged_source = Rc::new(RefCell::new(RvFile::new(FileType::File)));
        {
            let mut f = merged_source.borrow_mut();
            f.name = "merged.bin".to_string();
            f.size = Some(1024);
            f.crc = Some(vec![0x41, 0x52, 0x63, 0x74]);
            f.set_dat_got_status(DatStatus::InDatMerged, GotStatus::Got);
            f.tree_checked = TreeSelect::Selected;
        }
        merged_dir.borrow_mut().child_add(Rc::clone(&merged_source));

        let unknown_dir = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
        unknown_dir.borrow_mut().tree_checked = TreeSelect::Selected;

        let unknown_source = Rc::new(RefCell::new(RvFile::new(FileType::File)));
        {
            let mut f = unknown_source.borrow_mut();
            f.name = "unknown.bin".to_string();
            f.size = Some(1024);
            f.crc = Some(vec![0x41, 0x52, 0x63, 0x74]);
            f.set_dat_got_status(DatStatus::NotInDat, GotStatus::Got);
            f.tree_checked = TreeSelect::Selected;
        }
        unknown_dir.borrow_mut().child_add(Rc::clone(&unknown_source));

        let target_dir = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
        target_dir.borrow_mut().tree_checked = TreeSelect::Selected;

        let missing_file = Rc::new(RefCell::new(RvFile::new(FileType::File)));
        {
            let mut f = missing_file.borrow_mut();
            f.name = "missing.bin".to_string();
            f.size = Some(1024);
            f.crc = Some(vec![0x41, 0x52, 0x63, 0x74]);
            f.set_dat_got_status(DatStatus::InDatCollect, GotStatus::NotGot);
            f.tree_checked = TreeSelect::Selected;
        }
        target_dir.borrow_mut().child_add(Rc::clone(&missing_file));

        root.borrow_mut().child_add(merged_dir);
        root.borrow_mut().child_add(unknown_dir);
        root.borrow_mut().child_add(target_dir);

        FindFixes::scan_files(Rc::clone(&root));

        assert_eq!(missing_file.borrow().rep_status(), RepStatus::CanBeFixed);
        assert_eq!(merged_source.borrow().rep_status(), RepStatus::NeededForFix);
        assert_eq!(unknown_source.borrow().rep_status(), RepStatus::MoveToSort);
    }

    #[test]
    fn test_find_fixes_keeps_corrupt_matching_source_in_movetocorrupt_state() {
        let root = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));

        let source_dir = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
        source_dir.borrow_mut().tree_checked = TreeSelect::Selected;

        let corrupt_source = Rc::new(RefCell::new(RvFile::new(FileType::File)));
        {
            let mut f = corrupt_source.borrow_mut();
            f.name = "corrupt_source.bin".to_string();
            f.size = Some(1024);
            f.crc = Some(vec![0x11, 0x22, 0x33, 0x44]);
            f.set_dat_got_status(DatStatus::InDatCollect, GotStatus::Corrupt);
            f.tree_checked = TreeSelect::Selected;
        }
        source_dir.borrow_mut().child_add(Rc::clone(&corrupt_source));

        let target_dir = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
        target_dir.borrow_mut().tree_checked = TreeSelect::Selected;

        let missing_file = Rc::new(RefCell::new(RvFile::new(FileType::File)));
        {
            let mut f = missing_file.borrow_mut();
            f.name = "missing.bin".to_string();
            f.size = Some(1024);
            f.crc = Some(vec![0x11, 0x22, 0x33, 0x44]);
            f.set_dat_got_status(DatStatus::InDatCollect, GotStatus::NotGot);
            f.tree_checked = TreeSelect::Selected;
        }
        target_dir.borrow_mut().child_add(Rc::clone(&missing_file));

        root.borrow_mut().child_add(source_dir);
        root.borrow_mut().child_add(target_dir);

        FindFixes::scan_files(Rc::clone(&root));

        assert_eq!(missing_file.borrow().rep_status(), RepStatus::CorruptCanBeFixed);
        assert_eq!(corrupt_source.borrow().rep_status(), RepStatus::MoveToCorrupt);
    }

    #[test]
    fn test_find_fixes_promotes_mia_matching_source_to_neededforfix() {
        let root = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));

        let source_dir = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
        source_dir.borrow_mut().tree_checked = TreeSelect::Selected;

        let mia_source = Rc::new(RefCell::new(RvFile::new(FileType::File)));
        {
            let mut f = mia_source.borrow_mut();
            f.name = "mia_source.bin".to_string();
            f.size = Some(1024);
            f.crc = Some(vec![0x21, 0x32, 0x43, 0x54]);
            f.set_dat_got_status(DatStatus::InDatMIA, GotStatus::Got);
            f.tree_checked = TreeSelect::Selected;
        }
        source_dir.borrow_mut().child_add(Rc::clone(&mia_source));

        let target_dir = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
        target_dir.borrow_mut().tree_checked = TreeSelect::Selected;

        let missing_file = Rc::new(RefCell::new(RvFile::new(FileType::File)));
        {
            let mut f = missing_file.borrow_mut();
            f.name = "missing.bin".to_string();
            f.size = Some(1024);
            f.crc = Some(vec![0x21, 0x32, 0x43, 0x54]);
            f.set_dat_got_status(DatStatus::InDatCollect, GotStatus::NotGot);
            f.tree_checked = TreeSelect::Selected;
        }
        target_dir.borrow_mut().child_add(Rc::clone(&missing_file));

        root.borrow_mut().child_add(source_dir);
        root.borrow_mut().child_add(target_dir);

        FindFixes::scan_files(Rc::clone(&root));

        assert_eq!(missing_file.borrow().rep_status(), RepStatus::CanBeFixed);
        assert_eq!(mia_source.borrow().rep_status(), RepStatus::CorrectMIA);
    }

    #[test]
    fn test_find_fixes_does_not_reuse_one_source_for_two_missing_files() {
        let root = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));

        let source_dir = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
        source_dir.borrow_mut().tree_checked = TreeSelect::Selected;

        let source_file = Rc::new(RefCell::new(RvFile::new(FileType::File)));
        {
            let mut f = source_file.borrow_mut();
            f.name = "spare.bin".to_string();
            f.size = Some(1024);
            f.crc = Some(vec![0x10, 0x20, 0x30, 0x40]);
            f.set_dat_got_status(DatStatus::InToSort, GotStatus::Got);
            f.tree_checked = TreeSelect::Selected;
        }
        source_dir.borrow_mut().child_add(Rc::clone(&source_file));

        let target_dir = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
        target_dir.borrow_mut().tree_checked = TreeSelect::Selected;

        let first_missing = Rc::new(RefCell::new(RvFile::new(FileType::File)));
        {
            let mut f = first_missing.borrow_mut();
            f.name = "missing1.bin".to_string();
            f.size = Some(1024);
            f.crc = Some(vec![0x10, 0x20, 0x30, 0x40]);
            f.set_dat_got_status(DatStatus::InDatCollect, GotStatus::NotGot);
            f.tree_checked = TreeSelect::Selected;
        }
        target_dir.borrow_mut().child_add(Rc::clone(&first_missing));

        let second_missing = Rc::new(RefCell::new(RvFile::new(FileType::File)));
        {
            let mut f = second_missing.borrow_mut();
            f.name = "missing2.bin".to_string();
            f.size = Some(1024);
            f.crc = Some(vec![0x10, 0x20, 0x30, 0x40]);
            f.set_dat_got_status(DatStatus::InDatCollect, GotStatus::NotGot);
            f.tree_checked = TreeSelect::Selected;
        }
        target_dir.borrow_mut().child_add(Rc::clone(&second_missing));

        root.borrow_mut().child_add(source_dir);
        root.borrow_mut().child_add(target_dir);

        FindFixes::scan_files(Rc::clone(&root));

        assert_eq!(source_file.borrow().rep_status(), RepStatus::NeededForFix);
        let first_rep = first_missing.borrow().rep_status();
        let second_rep = second_missing.borrow().rep_status();
        let fixable_count =
            usize::from(first_rep == RepStatus::CanBeFixed) + usize::from(second_rep == RepStatus::CanBeFixed);
        let missing_count =
            usize::from(first_rep == RepStatus::Missing) + usize::from(second_rep == RepStatus::Missing);
        assert_eq!(fixable_count, 1);
        assert_eq!(missing_count, 1);
    }

    #[test]
    fn test_find_fixes_allows_locked_source_to_fix_multiple_missing_files() {
        let root = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));

        let source_dir = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
        source_dir.borrow_mut().tree_checked = TreeSelect::Locked;

        let source_file = Rc::new(RefCell::new(RvFile::new(FileType::File)));
        {
            let mut f = source_file.borrow_mut();
            f.name = "locked_source.bin".to_string();
            f.size = Some(1024);
            f.crc = Some(vec![0x10, 0x20, 0x30, 0x40]);
            f.set_dat_got_status(DatStatus::NotInDat, GotStatus::Got);
            f.tree_checked = TreeSelect::Locked;
        }
        source_dir.borrow_mut().child_add(Rc::clone(&source_file));

        let target_dir = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
        target_dir.borrow_mut().tree_checked = TreeSelect::Selected;

        let first_missing = Rc::new(RefCell::new(RvFile::new(FileType::File)));
        {
            let mut f = first_missing.borrow_mut();
            f.name = "missing1.bin".to_string();
            f.size = Some(1024);
            f.crc = Some(vec![0x10, 0x20, 0x30, 0x40]);
            f.set_dat_got_status(DatStatus::InDatCollect, GotStatus::NotGot);
            f.tree_checked = TreeSelect::Selected;
        }
        target_dir.borrow_mut().child_add(Rc::clone(&first_missing));

        let second_missing = Rc::new(RefCell::new(RvFile::new(FileType::File)));
        {
            let mut f = second_missing.borrow_mut();
            f.name = "missing2.bin".to_string();
            f.size = Some(1024);
            f.crc = Some(vec![0x10, 0x20, 0x30, 0x40]);
            f.set_dat_got_status(DatStatus::InDatCollect, GotStatus::NotGot);
            f.tree_checked = TreeSelect::Selected;
        }
        target_dir.borrow_mut().child_add(Rc::clone(&second_missing));

        root.borrow_mut().child_add(source_dir);
        root.borrow_mut().child_add(target_dir);

        FindFixes::scan_files(Rc::clone(&root));

        assert_eq!(source_file.borrow().rep_status(), RepStatus::NeededForFix);
        assert_eq!(first_missing.borrow().rep_status(), RepStatus::CanBeFixed);
        assert_eq!(second_missing.borrow().rep_status(), RepStatus::CanBeFixed);
    }

    #[test]
    fn test_find_fixes_allows_correct_indat_source_to_fix_multiple_missing_files_without_consuming_it() {
        let root = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));

        let source_dir = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
        source_dir.borrow_mut().tree_checked = TreeSelect::Selected;

        let source_file = Rc::new(RefCell::new(RvFile::new(FileType::File)));
        {
            let mut f = source_file.borrow_mut();
            f.name = "correct_source.bin".to_string();
            f.size = Some(1024);
            f.crc = Some(vec![0x44, 0x55, 0x66, 0x77]);
            f.set_dat_got_status(DatStatus::InDatCollect, GotStatus::Got);
            f.tree_checked = TreeSelect::Selected;
        }
        source_dir.borrow_mut().child_add(Rc::clone(&source_file));

        let target_dir = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
        target_dir.borrow_mut().tree_checked = TreeSelect::Selected;

        for name in ["missing1.bin", "missing2.bin"] {
            let missing = Rc::new(RefCell::new(RvFile::new(FileType::File)));
            {
                let mut f = missing.borrow_mut();
                f.name = name.to_string();
                f.size = Some(1024);
                f.crc = Some(vec![0x44, 0x55, 0x66, 0x77]);
                f.set_dat_got_status(DatStatus::InDatCollect, GotStatus::NotGot);
                f.tree_checked = TreeSelect::Selected;
            }
            target_dir.borrow_mut().child_add(missing);
        }

        root.borrow_mut().child_add(source_dir);
        root.borrow_mut().child_add(Rc::clone(&target_dir));

        FindFixes::scan_files(Rc::clone(&root));

        let target_children = target_dir.borrow().children.clone();
        assert!(target_children.iter().all(|child| child.borrow().rep_status() == RepStatus::CanBeFixed));
        assert_eq!(source_file.borrow().rep_status(), RepStatus::Correct);
    }

    #[test]
    fn test_find_fixes_allows_mia_source_to_fix_multiple_missing_files_without_consuming_it() {
        let root = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));

        let source_dir = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
        source_dir.borrow_mut().tree_checked = TreeSelect::Selected;

        let source_file = Rc::new(RefCell::new(RvFile::new(FileType::File)));
        {
            let mut f = source_file.borrow_mut();
            f.name = "mia_source.bin".to_string();
            f.size = Some(1024);
            f.crc = Some(vec![0x31, 0x42, 0x53, 0x64]);
            f.set_dat_got_status(DatStatus::InDatMIA, GotStatus::Got);
            f.tree_checked = TreeSelect::Selected;
        }
        source_dir.borrow_mut().child_add(Rc::clone(&source_file));

        let target_dir = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
        target_dir.borrow_mut().tree_checked = TreeSelect::Selected;

        for name in ["missing1.bin", "missing2.bin"] {
            let missing = Rc::new(RefCell::new(RvFile::new(FileType::File)));
            {
                let mut f = missing.borrow_mut();
                f.name = name.to_string();
                f.size = Some(1024);
                f.crc = Some(vec![0x31, 0x42, 0x53, 0x64]);
                f.set_dat_got_status(DatStatus::InDatCollect, GotStatus::NotGot);
                f.tree_checked = TreeSelect::Selected;
            }
            target_dir.borrow_mut().child_add(missing);
        }

        root.borrow_mut().child_add(source_dir);
        root.borrow_mut().child_add(Rc::clone(&target_dir));

        FindFixes::scan_files(Rc::clone(&root));

        let target_children = target_dir.borrow().children.clone();
        assert!(target_children.iter().all(|child| child.borrow().rep_status() == RepStatus::CanBeFixed));
        assert_eq!(source_file.borrow().rep_status(), RepStatus::CorrectMIA);
    }

    #[test]
    fn test_find_fixes_prefers_consumable_source_over_locked_source_when_priority_is_otherwise_equal() {
        let root = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));

        let locked_dir = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
        locked_dir.borrow_mut().tree_checked = TreeSelect::Locked;

        let locked_source = Rc::new(RefCell::new(RvFile::new(FileType::File)));
        {
            let mut f = locked_source.borrow_mut();
            f.name = "locked_source.bin".to_string();
            f.size = Some(1024);
            f.crc = Some(vec![0x90, 0x80, 0x70, 0x60]);
            f.set_dat_got_status(DatStatus::NotInDat, GotStatus::Got);
            f.tree_checked = TreeSelect::Locked;
        }
        locked_dir.borrow_mut().child_add(Rc::clone(&locked_source));

        let consumable_dir = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
        consumable_dir.borrow_mut().tree_checked = TreeSelect::Selected;

        let consumable_source = Rc::new(RefCell::new(RvFile::new(FileType::File)));
        {
            let mut f = consumable_source.borrow_mut();
            f.name = "consumable_source.bin".to_string();
            f.size = Some(1024);
            f.crc = Some(vec![0x90, 0x80, 0x70, 0x60]);
            f.set_dat_got_status(DatStatus::NotInDat, GotStatus::Got);
            f.tree_checked = TreeSelect::Selected;
        }
        consumable_dir.borrow_mut().child_add(Rc::clone(&consumable_source));

        let target_dir = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
        target_dir.borrow_mut().tree_checked = TreeSelect::Selected;

        let missing = Rc::new(RefCell::new(RvFile::new(FileType::File)));
        {
            let mut f = missing.borrow_mut();
            f.name = "missing.bin".to_string();
            f.size = Some(1024);
            f.crc = Some(vec![0x90, 0x80, 0x70, 0x60]);
            f.set_dat_got_status(DatStatus::InDatCollect, GotStatus::NotGot);
            f.tree_checked = TreeSelect::Selected;
        }
        target_dir.borrow_mut().child_add(Rc::clone(&missing));

        root.borrow_mut().child_add(locked_dir);
        root.borrow_mut().child_add(consumable_dir);
        root.borrow_mut().child_add(target_dir);

        FindFixes::scan_files(Rc::clone(&root));

        assert_eq!(missing.borrow().rep_status(), RepStatus::CanBeFixed);
        assert_eq!(consumable_source.borrow().rep_status(), RepStatus::NeededForFix);
        assert_eq!(locked_source.borrow().rep_status(), RepStatus::MoveToSort);
    }

    #[test]
    fn test_find_fixes_promotes_matching_notindat_source_from_movetosort_to_neededforfix() {
        let root = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));

        let source_dir = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
        source_dir.borrow_mut().tree_checked = TreeSelect::Selected;

        let source_file = Rc::new(RefCell::new(RvFile::new(FileType::File)));
        {
            let mut f = source_file.borrow_mut();
            f.name = "source.bin".to_string();
            f.size = Some(1024);
            f.crc = Some(vec![0x10, 0x20, 0x30, 0x40]);
            f.set_dat_got_status(DatStatus::NotInDat, GotStatus::Got);
            f.tree_checked = TreeSelect::Selected;
        }
        source_dir.borrow_mut().child_add(Rc::clone(&source_file));

        let target_dir = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
        target_dir.borrow_mut().tree_checked = TreeSelect::Selected;

        let missing_file = Rc::new(RefCell::new(RvFile::new(FileType::File)));
        {
            let mut f = missing_file.borrow_mut();
            f.name = "target.bin".to_string();
            f.size = Some(1024);
            f.crc = Some(vec![0x10, 0x20, 0x30, 0x40]);
            f.set_dat_got_status(DatStatus::InDatCollect, GotStatus::NotGot);
            f.tree_checked = TreeSelect::Selected;
        }
        target_dir.borrow_mut().child_add(Rc::clone(&missing_file));

        root.borrow_mut().child_add(source_dir);
        root.borrow_mut().child_add(target_dir);

        FindFixes::scan_files(Rc::clone(&root));

        assert_eq!(missing_file.borrow().rep_status(), RepStatus::CanBeFixed);
        assert_eq!(source_file.borrow().rep_status(), RepStatus::NeededForFix);
    }

    #[test]
    fn test_find_fixes_allows_corrupt_indat_source_to_fix_multiple_missing_files_without_consuming_it() {
        let root = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));

        let source_dir = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
        source_dir.borrow_mut().tree_checked = TreeSelect::Selected;

        let source_file = Rc::new(RefCell::new(RvFile::new(FileType::File)));
        {
            let mut f = source_file.borrow_mut();
            f.name = "corrupt_source.bin".to_string();
            f.size = Some(1024);
            f.crc = Some(vec![0x71, 0x62, 0x53, 0x44]);
            f.set_dat_got_status(DatStatus::InDatCollect, GotStatus::Corrupt);
            f.tree_checked = TreeSelect::Selected;
        }
        source_dir.borrow_mut().child_add(Rc::clone(&source_file));

        let target_dir = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
        target_dir.borrow_mut().tree_checked = TreeSelect::Selected;

        for name in ["missing1.bin", "missing2.bin"] {
            let missing = Rc::new(RefCell::new(RvFile::new(FileType::File)));
            {
                let mut f = missing.borrow_mut();
                f.name = name.to_string();
                f.size = Some(1024);
                f.crc = Some(vec![0x71, 0x62, 0x53, 0x44]);
                f.set_dat_got_status(DatStatus::InDatCollect, GotStatus::NotGot);
                f.tree_checked = TreeSelect::Selected;
            }
            target_dir.borrow_mut().child_add(missing);
        }

        root.borrow_mut().child_add(source_dir);
        root.borrow_mut().child_add(Rc::clone(&target_dir));

        FindFixes::scan_files(Rc::clone(&root));

        let target_children = target_dir.borrow().children.clone();
        assert!(target_children.iter().all(|child| child.borrow().rep_status() == RepStatus::CorruptCanBeFixed));
        assert_eq!(source_file.borrow().rep_status(), RepStatus::MoveToCorrupt);
    }

    #[test]
    fn test_find_fixes_matches_missing_alt_crc_against_primary_source_crc() {
        let root = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));

        let source_dir = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
        source_dir.borrow_mut().tree_checked = TreeSelect::Selected;

        let source_file = Rc::new(RefCell::new(RvFile::new(FileType::File)));
        {
            let mut f = source_file.borrow_mut();
            f.name = "source.bin".to_string();
            f.size = Some(1024);
            f.crc = Some(vec![0xAA, 0xBB, 0xCC, 0xDD]);
            f.set_dat_got_status(DatStatus::NotInDat, GotStatus::Got);
            f.tree_checked = TreeSelect::Selected;
        }
        source_dir.borrow_mut().child_add(Rc::clone(&source_file));

        let target_dir = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
        target_dir.borrow_mut().tree_checked = TreeSelect::Selected;

        let missing_file = Rc::new(RefCell::new(RvFile::new(FileType::File)));
        {
            let mut f = missing_file.borrow_mut();
            f.name = "target.bin".to_string();
            f.alt_size = Some(1024);
            f.alt_crc = Some(vec![0xAA, 0xBB, 0xCC, 0xDD]);
            f.set_dat_got_status(DatStatus::InDatCollect, GotStatus::NotGot);
            f.tree_checked = TreeSelect::Selected;
        }
        target_dir.borrow_mut().child_add(Rc::clone(&missing_file));

        root.borrow_mut().child_add(source_dir);
        root.borrow_mut().child_add(target_dir);

        FindFixes::scan_files(Rc::clone(&root));

        assert_eq!(missing_file.borrow().rep_status(), RepStatus::CanBeFixed);
        assert_eq!(source_file.borrow().rep_status(), RepStatus::NeededForFix);
    }

    #[test]
    fn test_find_fixes_keeps_corrupt_notindat_source_in_delete_state_when_matched() {
        let root = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));

        let source_dir = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
        source_dir.borrow_mut().tree_checked = TreeSelect::Selected;

        let source_file = Rc::new(RefCell::new(RvFile::new(FileType::File)));
        {
            let mut f = source_file.borrow_mut();
            f.name = "corrupt_source.bin".to_string();
            f.size = Some(1024);
            f.crc = Some(vec![0xDE, 0xAD, 0xBE, 0xEF]);
            f.set_dat_got_status(DatStatus::NotInDat, GotStatus::Corrupt);
            f.tree_checked = TreeSelect::Selected;
        }
        source_dir.borrow_mut().child_add(Rc::clone(&source_file));

        let target_dir = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
        target_dir.borrow_mut().tree_checked = TreeSelect::Selected;

        let missing_file = Rc::new(RefCell::new(RvFile::new(FileType::File)));
        {
            let mut f = missing_file.borrow_mut();
            f.name = "target.bin".to_string();
            f.size = Some(1024);
            f.crc = Some(vec![0xDE, 0xAD, 0xBE, 0xEF]);
            f.set_dat_got_status(DatStatus::InDatCollect, GotStatus::NotGot);
            f.tree_checked = TreeSelect::Selected;
        }
        target_dir.borrow_mut().child_add(Rc::clone(&missing_file));

        root.borrow_mut().child_add(source_dir);
        root.borrow_mut().child_add(target_dir);

        FindFixes::scan_files(Rc::clone(&root));

        assert_eq!(missing_file.borrow().rep_status(), RepStatus::CorruptCanBeFixed);
        assert_eq!(source_file.borrow().rep_status(), RepStatus::Delete);
    }

    #[test]
    fn test_find_fixes_matches_primary_sha1_using_missing_alt_size() {
        let root = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));

        let source_dir = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
        source_dir.borrow_mut().tree_checked = TreeSelect::Selected;

        let source_file = Rc::new(RefCell::new(RvFile::new(FileType::File)));
        {
            let mut f = source_file.borrow_mut();
            f.name = "source.bin".to_string();
            f.size = Some(1024);
            f.sha1 = Some(vec![
                0x01, 0x23, 0x45, 0x67, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            ]);
            f.set_dat_got_status(DatStatus::NotInDat, GotStatus::Got);
            f.tree_checked = TreeSelect::Selected;
        }
        source_dir.borrow_mut().child_add(Rc::clone(&source_file));

        let target_dir = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
        target_dir.borrow_mut().tree_checked = TreeSelect::Selected;

        let missing_file = Rc::new(RefCell::new(RvFile::new(FileType::File)));
        {
            let mut f = missing_file.borrow_mut();
            f.name = "target.bin".to_string();
            f.alt_size = Some(1024);
            f.sha1 = Some(vec![
                0x01, 0x23, 0x45, 0x67, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            ]);
            f.set_dat_got_status(DatStatus::InDatCollect, GotStatus::NotGot);
            f.tree_checked = TreeSelect::Selected;
        }
        target_dir.borrow_mut().child_add(Rc::clone(&missing_file));

        root.borrow_mut().child_add(source_dir);
        root.borrow_mut().child_add(target_dir);

        FindFixes::scan_files(Rc::clone(&root));

        assert_eq!(missing_file.borrow().rep_status(), RepStatus::CanBeFixed);
        assert_eq!(source_file.borrow().rep_status(), RepStatus::NeededForFix);
    }

    #[test]
    fn test_find_fixes_matches_missing_alt_md5_against_primary_source_md5() {
        let root = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));

        let source_dir = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
        source_dir.borrow_mut().tree_checked = TreeSelect::Selected;

        let source_file = Rc::new(RefCell::new(RvFile::new(FileType::File)));
        {
            let mut f = source_file.borrow_mut();
            f.name = "source.bin".to_string();
            f.size = Some(1024);
            f.md5 = Some(vec![0x89, 0xAB, 0xCD, 0xEF, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
            f.set_dat_got_status(DatStatus::NotInDat, GotStatus::Got);
            f.tree_checked = TreeSelect::Selected;
        }
        source_dir.borrow_mut().child_add(Rc::clone(&source_file));

        let target_dir = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
        target_dir.borrow_mut().tree_checked = TreeSelect::Selected;

        let missing_file = Rc::new(RefCell::new(RvFile::new(FileType::File)));
        {
            let mut f = missing_file.borrow_mut();
            f.name = "target.bin".to_string();
            f.alt_size = Some(1024);
            f.alt_md5 =
                Some(vec![0x89, 0xAB, 0xCD, 0xEF, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
            f.set_dat_got_status(DatStatus::InDatCollect, GotStatus::NotGot);
            f.tree_checked = TreeSelect::Selected;
        }
        target_dir.borrow_mut().child_add(Rc::clone(&missing_file));

        root.borrow_mut().child_add(source_dir);
        root.borrow_mut().child_add(target_dir);

        FindFixes::scan_files(Rc::clone(&root));

        assert_eq!(missing_file.borrow().rep_status(), RepStatus::CanBeFixed);
        assert_eq!(source_file.borrow().rep_status(), RepStatus::NeededForFix);
    }

    #[test]
    fn test_find_fixes_matches_missing_alt_sha1_against_primary_source_sha1() {
        let root = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));

        let source_dir = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
        source_dir.borrow_mut().tree_checked = TreeSelect::Selected;

        let source_file = Rc::new(RefCell::new(RvFile::new(FileType::File)));
        {
            let mut f = source_file.borrow_mut();
            f.name = "source.bin".to_string();
            f.size = Some(1024);
            f.sha1 = Some(vec![
                0x89, 0xAB, 0xCD, 0xEF, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            ]);
            f.set_dat_got_status(DatStatus::NotInDat, GotStatus::Got);
            f.tree_checked = TreeSelect::Selected;
        }
        source_dir.borrow_mut().child_add(Rc::clone(&source_file));

        let target_dir = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
        target_dir.borrow_mut().tree_checked = TreeSelect::Selected;

        let missing_file = Rc::new(RefCell::new(RvFile::new(FileType::File)));
        {
            let mut f = missing_file.borrow_mut();
            f.name = "target.bin".to_string();
            f.alt_size = Some(1024);
            f.alt_sha1 = Some(vec![
                0x89, 0xAB, 0xCD, 0xEF, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            ]);
            f.set_dat_got_status(DatStatus::InDatCollect, GotStatus::NotGot);
            f.tree_checked = TreeSelect::Selected;
        }
        target_dir.borrow_mut().child_add(Rc::clone(&missing_file));

        root.borrow_mut().child_add(source_dir);
        root.borrow_mut().child_add(target_dir);

        FindFixes::scan_files(Rc::clone(&root));

        assert_eq!(missing_file.borrow().rep_status(), RepStatus::CanBeFixed);
        assert_eq!(source_file.borrow().rep_status(), RepStatus::NeededForFix);
    }

    #[test]
    fn test_find_fixes_matches_primary_md5_against_source_alt_md5() {
        let root = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));

        let source_dir = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
        source_dir.borrow_mut().tree_checked = TreeSelect::Selected;

        let source_file = Rc::new(RefCell::new(RvFile::new(FileType::File)));
        {
            let mut f = source_file.borrow_mut();
            f.name = "source.bin".to_string();
            f.alt_size = Some(1024);
            f.alt_md5 =
                Some(vec![0x98, 0x76, 0x54, 0x32, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
            f.set_dat_got_status(DatStatus::NotInDat, GotStatus::Got);
            f.tree_checked = TreeSelect::Selected;
        }
        source_dir.borrow_mut().child_add(Rc::clone(&source_file));

        let target_dir = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
        target_dir.borrow_mut().tree_checked = TreeSelect::Selected;

        let missing_file = Rc::new(RefCell::new(RvFile::new(FileType::File)));
        {
            let mut f = missing_file.borrow_mut();
            f.name = "target.bin".to_string();
            f.size = Some(1024);
            f.md5 = Some(vec![0x98, 0x76, 0x54, 0x32, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
            f.set_dat_got_status(DatStatus::InDatCollect, GotStatus::NotGot);
            f.tree_checked = TreeSelect::Selected;
        }
        target_dir.borrow_mut().child_add(Rc::clone(&missing_file));

        root.borrow_mut().child_add(source_dir);
        root.borrow_mut().child_add(target_dir);

        FindFixes::scan_files(Rc::clone(&root));

        assert_eq!(missing_file.borrow().rep_status(), RepStatus::CanBeFixed);
        assert_eq!(source_file.borrow().rep_status(), RepStatus::NeededForFix);
    }

    #[test]
    fn test_find_fixes_matches_primary_sha1_against_source_alt_sha1() {
        let root = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));

        let source_dir = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
        source_dir.borrow_mut().tree_checked = TreeSelect::Selected;

        let source_file = Rc::new(RefCell::new(RvFile::new(FileType::File)));
        {
            let mut f = source_file.borrow_mut();
            f.name = "source.bin".to_string();
            f.alt_size = Some(1024);
            f.alt_sha1 = Some(vec![
                0x98, 0x76, 0x54, 0x32, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            ]);
            f.set_dat_got_status(DatStatus::NotInDat, GotStatus::Got);
            f.tree_checked = TreeSelect::Selected;
        }
        source_dir.borrow_mut().child_add(Rc::clone(&source_file));

        let target_dir = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
        target_dir.borrow_mut().tree_checked = TreeSelect::Selected;

        let missing_file = Rc::new(RefCell::new(RvFile::new(FileType::File)));
        {
            let mut f = missing_file.borrow_mut();
            f.name = "target.bin".to_string();
            f.size = Some(1024);
            f.sha1 = Some(vec![
                0x98, 0x76, 0x54, 0x32, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            ]);
            f.set_dat_got_status(DatStatus::InDatCollect, GotStatus::NotGot);
            f.tree_checked = TreeSelect::Selected;
        }
        target_dir.borrow_mut().child_add(Rc::clone(&missing_file));

        root.borrow_mut().child_add(source_dir);
        root.borrow_mut().child_add(target_dir);

        FindFixes::scan_files(Rc::clone(&root));

        assert_eq!(missing_file.borrow().rep_status(), RepStatus::CanBeFixed);
        assert_eq!(source_file.borrow().rep_status(), RepStatus::NeededForFix);
    }

    #[test]
    fn test_find_fixes_matches_primary_crc_against_source_alt_crc() {
        let root = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));

        let source_dir = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
        source_dir.borrow_mut().tree_checked = TreeSelect::Selected;

        let source_file = Rc::new(RefCell::new(RvFile::new(FileType::File)));
        {
            let mut f = source_file.borrow_mut();
            f.name = "source.bin".to_string();
            f.alt_size = Some(1024);
            f.alt_crc = Some(vec![0x98, 0x76, 0x54, 0x32]);
            f.set_dat_got_status(DatStatus::NotInDat, GotStatus::Got);
            f.tree_checked = TreeSelect::Selected;
        }
        source_dir.borrow_mut().child_add(Rc::clone(&source_file));

        let target_dir = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
        target_dir.borrow_mut().tree_checked = TreeSelect::Selected;

        let missing_file = Rc::new(RefCell::new(RvFile::new(FileType::File)));
        {
            let mut f = missing_file.borrow_mut();
            f.name = "target.bin".to_string();
            f.size = Some(1024);
            f.crc = Some(vec![0x98, 0x76, 0x54, 0x32]);
            f.set_dat_got_status(DatStatus::InDatCollect, GotStatus::NotGot);
            f.tree_checked = TreeSelect::Selected;
        }
        target_dir.borrow_mut().child_add(Rc::clone(&missing_file));

        root.borrow_mut().child_add(source_dir);
        root.borrow_mut().child_add(target_dir);

        FindFixes::scan_files(Rc::clone(&root));

        assert_eq!(missing_file.borrow().rep_status(), RepStatus::CanBeFixed);
        assert_eq!(source_file.borrow().rep_status(), RepStatus::NeededForFix);
    }

    #[test]
    fn test_find_fixes_prefers_better_priority_alt_crc_source_over_worse_primary_crc_source() {
        let root = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));

        let locked_dir = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
        locked_dir.borrow_mut().tree_checked = TreeSelect::Selected;

        let locked_primary_source = Rc::new(RefCell::new(RvFile::new(FileType::File)));
        {
            let mut f = locked_primary_source.borrow_mut();
            f.name = "locked_primary.bin".to_string();
            f.size = Some(1024);
            f.crc = Some(vec![0x10, 0x20, 0x30, 0x40]);
            f.tree_checked = TreeSelect::Locked;
            f.set_dat_got_status(DatStatus::NotInDat, GotStatus::Got);
        }
        locked_dir.borrow_mut().child_add(Rc::clone(&locked_primary_source));

        let tosort_dir = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
        tosort_dir.borrow_mut().tree_checked = TreeSelect::Selected;

        let expendable_alt_source = Rc::new(RefCell::new(RvFile::new(FileType::File)));
        {
            let mut f = expendable_alt_source.borrow_mut();
            f.name = "tosort_alt.bin".to_string();
            f.alt_size = Some(1024);
            f.alt_crc = Some(vec![0x99, 0x88, 0x77, 0x66]);
            f.tree_checked = TreeSelect::Selected;
            f.set_dat_got_status(DatStatus::InToSort, GotStatus::Got);
        }
        tosort_dir.borrow_mut().child_add(Rc::clone(&expendable_alt_source));

        let target_dir = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
        target_dir.borrow_mut().tree_checked = TreeSelect::Selected;

        let missing_file = Rc::new(RefCell::new(RvFile::new(FileType::File)));
        {
            let mut f = missing_file.borrow_mut();
            f.name = "target.bin".to_string();
            f.size = Some(1024);
            f.crc = Some(vec![0x10, 0x20, 0x30, 0x40]);
            f.alt_size = Some(1024);
            f.alt_crc = Some(vec![0x99, 0x88, 0x77, 0x66]);
            f.set_dat_got_status(DatStatus::InDatCollect, GotStatus::NotGot);
            f.tree_checked = TreeSelect::Selected;
        }
        target_dir.borrow_mut().child_add(Rc::clone(&missing_file));

        root.borrow_mut().child_add(locked_dir);
        root.borrow_mut().child_add(tosort_dir);
        root.borrow_mut().child_add(target_dir);

        FindFixes::scan_files(Rc::clone(&root));

        assert_eq!(missing_file.borrow().rep_status(), RepStatus::CanBeFixed);
        assert_eq!(expendable_alt_source.borrow().rep_status(), RepStatus::NeededForFix);
        assert_eq!(locked_primary_source.borrow().rep_status(), RepStatus::MoveToSort);
    }

    #[test]
    fn test_find_fixes_leaves_unused_tosort_file_in_tosort() {
        let root = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));

        let tosort_file = Rc::new(RefCell::new(RvFile::new(FileType::File)));
        {
            let mut f = tosort_file.borrow_mut();
            f.name = "spare.bin".to_string();
            f.size = Some(1024);
            f.crc = Some(vec![0x10, 0x20, 0x30, 0x40]);
            f.set_dat_got_status(DatStatus::InToSort, GotStatus::Got);
            f.tree_checked = TreeSelect::Selected;
        }
        root.borrow_mut().child_add(Rc::clone(&tosort_file));

        FindFixes::scan_files(Rc::clone(&root));

        assert_eq!(tosort_file.borrow().rep_status(), RepStatus::InToSort);
    }

    #[test]
    fn test_find_fixes_deletes_unused_tosort_file_when_romroot_already_has_match() {
        let root = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));

        let romroot_file = Rc::new(RefCell::new(RvFile::new(FileType::File)));
        {
            let mut f = romroot_file.borrow_mut();
            f.name = "owned.bin".to_string();
            f.size = Some(1024);
            f.crc = Some(vec![0x10, 0x20, 0x30, 0x40]);
            f.set_dat_got_status(DatStatus::InDatCollect, GotStatus::Got);
            f.tree_checked = TreeSelect::Selected;
        }
        root.borrow_mut().child_add(Rc::clone(&romroot_file));

        let tosort_file = Rc::new(RefCell::new(RvFile::new(FileType::File)));
        {
            let mut f = tosort_file.borrow_mut();
            f.name = "spare.bin".to_string();
            f.size = Some(1024);
            f.crc = Some(vec![0x10, 0x20, 0x30, 0x40]);
            f.set_dat_got_status(DatStatus::InToSort, GotStatus::Got);
            f.tree_checked = TreeSelect::Selected;
        }
        root.borrow_mut().child_add(Rc::clone(&tosort_file));

        FindFixes::scan_files(Rc::clone(&root));

        assert_eq!(romroot_file.borrow().rep_status(), RepStatus::Correct);
        assert_eq!(tosort_file.borrow().rep_status(), RepStatus::Delete);
    }

    #[test]
    fn test_find_fixes_deletes_unused_notindat_duplicate_when_romroot_already_has_match() {
        let root = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));

        let romroot_file = Rc::new(RefCell::new(RvFile::new(FileType::File)));
        {
            let mut f = romroot_file.borrow_mut();
            f.name = "owned.bin".to_string();
            f.size = Some(1024);
            f.crc = Some(vec![0x10, 0x20, 0x30, 0x40]);
            f.set_dat_got_status(DatStatus::InDatCollect, GotStatus::Got);
            f.tree_checked = TreeSelect::Selected;
        }
        root.borrow_mut().child_add(Rc::clone(&romroot_file));

        let notindat_file = Rc::new(RefCell::new(RvFile::new(FileType::File)));
        {
            let mut f = notindat_file.borrow_mut();
            f.name = "owned_copy.bin".to_string();
            f.size = Some(1024);
            f.crc = Some(vec![0x10, 0x20, 0x30, 0x40]);
            f.set_dat_got_status(DatStatus::NotInDat, GotStatus::Got);
            f.tree_checked = TreeSelect::Selected;
        }
        root.borrow_mut().child_add(Rc::clone(&notindat_file));

        FindFixes::scan_files(Rc::clone(&root));

        assert_eq!(romroot_file.borrow().rep_status(), RepStatus::Correct);
        assert_eq!(notindat_file.borrow().rep_status(), RepStatus::Delete);
    }

    #[test]
    fn test_find_fixes_deletes_unused_tosort_file_when_romroot_archive_member_already_has_match() {
        let root = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));

        let archive = Rc::new(RefCell::new(RvFile::new(FileType::Zip)));
        {
            let mut a = archive.borrow_mut();
            a.name = "game.zip".to_string();
            a.tree_checked = TreeSelect::Selected;
            a.set_dat_got_status(DatStatus::InDatCollect, GotStatus::Got);
        }

        let archive_member = Rc::new(RefCell::new(RvFile::new(FileType::FileZip)));
        {
            let mut f = archive_member.borrow_mut();
            f.name = "game.a78".to_string();
            f.size = Some(131200);
            f.crc = Some(vec![0x10, 0x20, 0x30, 0x40]);
            f.set_dat_got_status(DatStatus::InDatCollect, GotStatus::Got);
            f.tree_checked = TreeSelect::Selected;
            f.parent = Some(Rc::downgrade(&archive));
        }
        archive.borrow_mut().child_add(Rc::clone(&archive_member));
        root.borrow_mut().child_add(Rc::clone(&archive));

        let tosort_file = Rc::new(RefCell::new(RvFile::new(FileType::File)));
        {
            let mut f = tosort_file.borrow_mut();
            f.name = "game.a78".to_string();
            f.size = Some(131200);
            f.crc = Some(vec![0x10, 0x20, 0x30, 0x40]);
            f.set_dat_got_status(DatStatus::InToSort, GotStatus::Got);
            f.tree_checked = TreeSelect::Selected;
        }
        root.borrow_mut().child_add(Rc::clone(&tosort_file));

        FindFixes::scan_files(Rc::clone(&root));

        assert_eq!(archive_member.borrow().rep_status(), RepStatus::Correct);
        assert_eq!(tosort_file.borrow().rep_status(), RepStatus::Delete);
    }

    #[test]
    fn test_find_fixes_realscan_deletes_tosort_duplicate_when_romroot_zip_member_exists() {
        let temp = tempdir().unwrap();
        let root = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
        root.borrow_mut().name = temp.path().to_string_lossy().to_string();

        let romroot_path = temp.path().join("RustyVault");
        let tosort_path = temp.path().join("ToSort");
        fs::create_dir_all(&romroot_path).unwrap();
        fs::create_dir_all(tosort_path.join("Atari - Atari 7800 (A78) (Aftermarket)")).unwrap();

        {
            let file = std::fs::File::create(romroot_path.join("1942 (Small) (World) (Aftermarket) (Un).zip")).unwrap();
            let mut writer = ZipWriter::new(file);
            let options = SimpleFileOptions::default().compression_method(CompressionMethod::Stored);
            writer.start_file("1942 (Small) (World) (Aftermarket) (Un).a78", options).unwrap();
            writer.write_all(&vec![0xAB; 131200]).unwrap();
            writer.finish().unwrap();
        }

        fs::write(
            tosort_path
                .join("Atari - Atari 7800 (A78) (Aftermarket)")
                .join("1942 (Small) (World) (Aftermarket) (Un).a78"),
            vec![0xAB; 131200],
        )
        .unwrap();

        let rustyvault_dir = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
        {
            let mut dir = rustyvault_dir.borrow_mut();
            dir.name = romroot_path.to_string_lossy().to_string();
            dir.tree_checked = TreeSelect::Selected;
            dir.parent = Some(Rc::downgrade(&root));
        }

        let game_zip = Rc::new(RefCell::new(RvFile::new(FileType::Zip)));
        {
            let mut a = game_zip.borrow_mut();
            a.name = "1942 (Small) (World) (Aftermarket) (Un).zip".to_string();
            a.tree_checked = TreeSelect::Selected;
            a.set_dat_got_status(DatStatus::InDatCollect, GotStatus::NotGot);
            a.parent = Some(Rc::downgrade(&rustyvault_dir));
        }

        let zip_member = Rc::new(RefCell::new(RvFile::new(FileType::FileZip)));
        {
            let mut f = zip_member.borrow_mut();
            f.name = "1942 (Small) (World) (Aftermarket) (Un).a78".to_string();
            f.size = Some(131200);
            f.tree_checked = TreeSelect::Selected;
            f.set_dat_got_status(DatStatus::InDatCollect, GotStatus::NotGot);
            f.parent = Some(Rc::downgrade(&game_zip));
        }
        game_zip.borrow_mut().child_add(Rc::clone(&zip_member));
        rustyvault_dir.borrow_mut().child_add(Rc::clone(&game_zip));
        root.borrow_mut().child_add(Rc::clone(&rustyvault_dir));

        let tosort_dir = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
        {
            let mut dir = tosort_dir.borrow_mut();
            dir.name = tosort_path.to_string_lossy().to_string();
            dir.tree_checked = TreeSelect::Selected;
            dir.set_dat_status(DatStatus::InToSort);
            dir.parent = Some(Rc::downgrade(&root));
        }
        root.borrow_mut().child_add(Rc::clone(&tosort_dir));

        let romroot_scan = Scanner::scan_directory_with_level(&rustyvault_dir.borrow().name, crate::settings::EScanLevel::Level2);
        let mut romroot_scanned = crate::scanned_file::ScannedFile::new(FileType::Dir);
        romroot_scanned.name = rustyvault_dir.borrow().name.clone();
        romroot_scanned.children = romroot_scan;
        FileScanning::scan_dir_with_level(Rc::clone(&rustyvault_dir), &mut romroot_scanned, crate::settings::EScanLevel::Level2);

        let tosort_scan = Scanner::scan_directory_with_level(&tosort_dir.borrow().name, crate::settings::EScanLevel::Level2);
        let mut tosort_scanned = crate::scanned_file::ScannedFile::new(FileType::Dir);
        tosort_scanned.name = tosort_dir.borrow().name.clone();
        tosort_scanned.children = tosort_scan;
        FileScanning::scan_dir_with_level(Rc::clone(&tosort_dir), &mut tosort_scanned, crate::settings::EScanLevel::Level2);

        FindFixes::scan_files(Rc::clone(&root));

        let scanned_tosort_file = {
            let tosort_borrow = tosort_dir.borrow();
            let category_dir = tosort_borrow.children.iter().find(|child| child.borrow().name == "Atari - Atari 7800 (A78) (Aftermarket)").unwrap().clone();
            let scanned_file = category_dir.borrow().children[0].clone();
            scanned_file
        };

        assert_eq!(zip_member.borrow().got_status(), GotStatus::Got);
        assert_eq!(zip_member.borrow().rep_status(), RepStatus::Correct);
        assert_eq!(scanned_tosort_file.borrow().dat_status(), DatStatus::InToSort);
        assert_eq!(scanned_tosort_file.borrow().rep_status(), RepStatus::Delete);
    }


    #[test]
    fn test_find_fixes_marks_indatmerged_got_file_as_unneeded() {
        let root = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));

        let merged_file = Rc::new(RefCell::new(RvFile::new(FileType::File)));
        {
            let mut f = merged_file.borrow_mut();
            f.name = "merged.bin".to_string();
            f.size = Some(1024);
            f.crc = Some(vec![0x01, 0x02, 0x03, 0x04]);
            f.set_dat_got_status(DatStatus::InDatMerged, GotStatus::Got);
            f.tree_checked = TreeSelect::Selected;
        }
        root.borrow_mut().child_add(Rc::clone(&merged_file));

        FindFixes::scan_files(Rc::clone(&root));

        assert_eq!(merged_file.borrow().rep_status(), RepStatus::UnNeeded);
    }

    #[test]
    fn test_find_fixes_marks_shared_destination_merged_view_as_not_collected() {
        let temp = tempdir().unwrap();
        let shared_path = temp.path().join("Shared");
        fs::create_dir_all(&shared_path).unwrap();
        let original_settings = get_settings();
        update_settings(Settings::default());
        set_dir_mapping(DirMapping {
            dir_key: "DatA".to_string(),
            dir_path: shared_path.to_string_lossy().into_owned(),
        });
        set_dir_mapping(DirMapping {
            dir_key: "DatB".to_string(),
            dir_path: shared_path.to_string_lossy().into_owned(),
        });

        let root = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));

        let dat_a = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
        {
            let mut dir = dat_a.borrow_mut();
            dir.name = "DatA".to_string();
            dir.tree_checked = TreeSelect::Selected;
            dir.parent = Some(Rc::downgrade(&root));
        }
        let kept_file = Rc::new(RefCell::new(RvFile::new(FileType::File)));
        {
            let mut file = kept_file.borrow_mut();
            file.name = "shared.bin".to_string();
            file.size = Some(1024);
            file.crc = Some(vec![0x01, 0x02, 0x03, 0x04]);
            file.set_dat_got_status(DatStatus::InDatCollect, GotStatus::Got);
            file.tree_checked = TreeSelect::Selected;
            file.parent = Some(Rc::downgrade(&dat_a));
        }
        dat_a.borrow_mut().child_add(Rc::clone(&kept_file));

        let dat_b = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
        {
            let mut dir = dat_b.borrow_mut();
            dir.name = "DatB".to_string();
            dir.tree_checked = TreeSelect::Selected;
            dir.parent = Some(Rc::downgrade(&root));
        }
        let merged_view = Rc::new(RefCell::new(RvFile::new(FileType::File)));
        {
            let mut file = merged_view.borrow_mut();
            file.name = "shared.bin".to_string();
            file.size = Some(1024);
            file.crc = Some(vec![0x01, 0x02, 0x03, 0x04]);
            file.set_dat_got_status(DatStatus::InDatMerged, GotStatus::Got);
            file.tree_checked = TreeSelect::Selected;
            file.parent = Some(Rc::downgrade(&dat_b));
        }
        dat_b.borrow_mut().child_add(Rc::clone(&merged_view));

        root.borrow_mut().child_add(Rc::clone(&dat_a));
        root.borrow_mut().child_add(Rc::clone(&dat_b));

        FindFixes::scan_files(Rc::clone(&root));

        assert_eq!(kept_file.borrow().rep_status(), RepStatus::Correct);
        assert_eq!(merged_view.borrow().rep_status(), RepStatus::NotCollected);

        update_settings(original_settings);
    }

    #[test]
    fn test_find_fixes_marks_shared_archive_member_merged_view_as_not_collected() {
        let temp = tempdir().unwrap();
        let shared_path = temp.path().join("Shared");
        fs::create_dir_all(&shared_path).unwrap();
        let original_settings = get_settings();
        update_settings(Settings::default());
        set_dir_mapping(DirMapping {
            dir_key: "DatA".to_string(),
            dir_path: shared_path.to_string_lossy().into_owned(),
        });
        set_dir_mapping(DirMapping {
            dir_key: "DatB".to_string(),
            dir_path: shared_path.to_string_lossy().into_owned(),
        });

        let root = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));

        let dat_a = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
        {
            let mut dir = dat_a.borrow_mut();
            dir.name = "DatA".to_string();
            dir.tree_checked = TreeSelect::Selected;
            dir.parent = Some(Rc::downgrade(&root));
        }
        let archive_a = Rc::new(RefCell::new(RvFile::new(FileType::Zip)));
        {
            let mut archive = archive_a.borrow_mut();
            archive.name = "shared.zip".to_string();
            archive.tree_checked = TreeSelect::Selected;
            archive.parent = Some(Rc::downgrade(&dat_a));
        }
        let kept_member = Rc::new(RefCell::new(RvFile::new(FileType::File)));
        {
            let mut file = kept_member.borrow_mut();
            file.name = "shared.bin".to_string();
            file.size = Some(1024);
            file.crc = Some(vec![0x0A, 0x0B, 0x0C, 0x0D]);
            file.set_dat_got_status(DatStatus::InDatCollect, GotStatus::Got);
            file.tree_checked = TreeSelect::Selected;
            file.parent = Some(Rc::downgrade(&archive_a));
        }
        archive_a.borrow_mut().child_add(Rc::clone(&kept_member));
        dat_a.borrow_mut().child_add(Rc::clone(&archive_a));

        let dat_b = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
        {
            let mut dir = dat_b.borrow_mut();
            dir.name = "DatB".to_string();
            dir.tree_checked = TreeSelect::Selected;
            dir.parent = Some(Rc::downgrade(&root));
        }
        let archive_b = Rc::new(RefCell::new(RvFile::new(FileType::Zip)));
        {
            let mut archive = archive_b.borrow_mut();
            archive.name = "shared.zip".to_string();
            archive.tree_checked = TreeSelect::Selected;
            archive.parent = Some(Rc::downgrade(&dat_b));
        }
        let merged_member = Rc::new(RefCell::new(RvFile::new(FileType::File)));
        {
            let mut file = merged_member.borrow_mut();
            file.name = "shared.bin".to_string();
            file.size = Some(1024);
            file.crc = Some(vec![0x0A, 0x0B, 0x0C, 0x0D]);
            file.set_dat_got_status(DatStatus::InDatMerged, GotStatus::Got);
            file.tree_checked = TreeSelect::Selected;
            file.parent = Some(Rc::downgrade(&archive_b));
        }
        archive_b.borrow_mut().child_add(Rc::clone(&merged_member));
        dat_b.borrow_mut().child_add(Rc::clone(&archive_b));

        root.borrow_mut().child_add(Rc::clone(&dat_a));
        root.borrow_mut().child_add(Rc::clone(&dat_b));

        FindFixes::scan_files(Rc::clone(&root));

        assert_eq!(kept_member.borrow().rep_status(), RepStatus::Correct);
        assert_eq!(merged_member.borrow().rep_status(), RepStatus::NotCollected);

        update_settings(original_settings);
    }

    #[test]
    fn test_find_fixes_prefers_existing_dat_file_over_tosort_alt_hash_duplicate() {
        let temp = tempdir().unwrap();
        let shared_path = temp.path().join("Shared");
        fs::create_dir_all(&shared_path).unwrap();
        let original_settings = get_settings();
        update_settings(Settings::default());
        set_dir_mapping(DirMapping {
            dir_key: "DatA".to_string(),
            dir_path: shared_path.to_string_lossy().into_owned(),
        });
        set_dir_mapping(DirMapping {
            dir_key: "DatB".to_string(),
            dir_path: shared_path.to_string_lossy().into_owned(),
        });

        let root = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));

        let target_archive = Rc::new(RefCell::new(RvFile::new(FileType::Zip)));
        {
            let mut archive = target_archive.borrow_mut();
            archive.name = "target.zip".to_string();
            archive.tree_checked = TreeSelect::Selected;
            archive.parent = Some(Rc::downgrade(&root));
        }
        let missing_member = Rc::new(RefCell::new(RvFile::new(FileType::File)));
        {
            let mut file = missing_member.borrow_mut();
            file.name = "rom.a78".to_string();
            file.size = Some(3);
            file.md5 = Some(vec![0x10, 0x20, 0x30, 0x40, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
            file.set_dat_got_status(DatStatus::InDatCollect, GotStatus::NotGot);
            file.tree_checked = TreeSelect::Selected;
            file.parent = Some(Rc::downgrade(&target_archive));
        }
        target_archive.borrow_mut().child_add(Rc::clone(&missing_member));
        root.borrow_mut().child_add(Rc::clone(&target_archive));

        let dat_a = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
        {
            let mut dir = dat_a.borrow_mut();
            dir.name = "DatA".to_string();
            dir.tree_checked = TreeSelect::Selected;
            dir.parent = Some(Rc::downgrade(&root));
        }
        let archive_a = Rc::new(RefCell::new(RvFile::new(FileType::Zip)));
        {
            let mut archive = archive_a.borrow_mut();
            archive.name = "shared.zip".to_string();
            archive.tree_checked = TreeSelect::Selected;
            archive.parent = Some(Rc::downgrade(&dat_a));
        }
        let kept_member = Rc::new(RefCell::new(RvFile::new(FileType::File)));
        {
            let mut file = kept_member.borrow_mut();
            file.name = "rom.a78".to_string();
            file.size = Some(3);
            file.md5 = Some(vec![0x10, 0x20, 0x30, 0x40, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
            file.set_dat_got_status(DatStatus::InDatCollect, GotStatus::Got);
            file.tree_checked = TreeSelect::Selected;
            file.parent = Some(Rc::downgrade(&archive_a));
        }
        archive_a.borrow_mut().child_add(Rc::clone(&kept_member));
        dat_a.borrow_mut().child_add(Rc::clone(&archive_a));
        root.borrow_mut().child_add(Rc::clone(&dat_a));

        let dat_b = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
        {
            let mut dir = dat_b.borrow_mut();
            dir.name = "DatB".to_string();
            dir.tree_checked = TreeSelect::Selected;
            dir.parent = Some(Rc::downgrade(&root));
        }
        let archive_b = Rc::new(RefCell::new(RvFile::new(FileType::Zip)));
        {
            let mut archive = archive_b.borrow_mut();
            archive.name = "shared.zip".to_string();
            archive.tree_checked = TreeSelect::Selected;
            archive.parent = Some(Rc::downgrade(&dat_b));
        }
        let shared_view = Rc::new(RefCell::new(RvFile::new(FileType::File)));
        {
            let mut file = shared_view.borrow_mut();
            file.name = "rom.a78".to_string();
            file.size = Some(3);
            file.md5 = Some(vec![0x10, 0x20, 0x30, 0x40, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
            file.set_dat_got_status(DatStatus::InDatMerged, GotStatus::Got);
            file.tree_checked = TreeSelect::Selected;
            file.parent = Some(Rc::downgrade(&archive_b));
        }
        archive_b.borrow_mut().child_add(Rc::clone(&shared_view));
        dat_b.borrow_mut().child_add(Rc::clone(&archive_b));
        root.borrow_mut().child_add(Rc::clone(&dat_b));

        let tosort_dir = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
        {
            let mut dir = tosort_dir.borrow_mut();
            dir.name = "ToSort".to_string();
            dir.tree_checked = TreeSelect::Selected;
            dir.parent = Some(Rc::downgrade(&root));
        }
        let tosort_source = Rc::new(RefCell::new(RvFile::new(FileType::File)));
        {
            let mut file = tosort_source.borrow_mut();
            file.name = "rom_headered.a78".to_string();
            file.alt_size = Some(3);
            file.alt_md5 =
                Some(vec![0x10, 0x20, 0x30, 0x40, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0]);
            file.set_dat_got_status(DatStatus::InToSort, GotStatus::Got);
            file.tree_checked = TreeSelect::Selected;
            file.parent = Some(Rc::downgrade(&tosort_dir));
        }
        tosort_dir.borrow_mut().child_add(Rc::clone(&tosort_source));
        root.borrow_mut().child_add(Rc::clone(&tosort_dir));

        FindFixes::scan_files(Rc::clone(&root));

        assert_eq!(missing_member.borrow().rep_status(), RepStatus::CanBeFixed);
        assert_eq!(tosort_source.borrow().rep_status(), RepStatus::Delete);
        assert_eq!(kept_member.borrow().rep_status(), RepStatus::Correct);
        assert_eq!(shared_view.borrow().rep_status(), RepStatus::NotCollected);

        update_settings(original_settings);
    }

    #[test]
    fn test_find_fixes_deletes_redundant_tosort_duplicate_in_same_pass_when_other_source_fixes_target() {
        let root = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));

        let target_archive = Rc::new(RefCell::new(RvFile::new(FileType::Zip)));
        {
            let mut archive = target_archive.borrow_mut();
            archive.name = "game.zip".to_string();
            archive.tree_checked = TreeSelect::Selected;
            archive.set_dat_got_status(DatStatus::InDatCollect, GotStatus::Got);
            archive.parent = Some(Rc::downgrade(&root));
        }

        let missing_child = Rc::new(RefCell::new(RvFile::new(FileType::File)));
        {
            let mut file = missing_child.borrow_mut();
            file.name = "game.a78".to_string();
            file.size = Some(4);
            file.crc = Some(vec![0xDE, 0xAD, 0xBE, 0xEF]);
            file.tree_checked = TreeSelect::Selected;
            file.set_dat_got_status(DatStatus::InDatCollect, GotStatus::NotGot);
            file.parent = Some(Rc::downgrade(&target_archive));
        }
        target_archive.borrow_mut().child_add(Rc::clone(&missing_child));
        root.borrow_mut().child_add(Rc::clone(&target_archive));

        let tosort_dir = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
        {
            let mut dir = tosort_dir.borrow_mut();
            dir.name = "ToSort".to_string();
            dir.tree_checked = TreeSelect::Selected;
            dir.parent = Some(Rc::downgrade(&root));
        }

        let primary_source = Rc::new(RefCell::new(RvFile::new(FileType::File)));
        {
            let mut file = primary_source.borrow_mut();
            file.name = "game.a78".to_string();
            file.size = Some(4);
            file.crc = Some(vec![0xDE, 0xAD, 0xBE, 0xEF]);
            file.tree_checked = TreeSelect::Selected;
            file.set_dat_got_status(DatStatus::InToSort, GotStatus::Got);
            file.parent = Some(Rc::downgrade(&tosort_dir));
        }

        let duplicate_source = Rc::new(RefCell::new(RvFile::new(FileType::File)));
        {
            let mut file = duplicate_source.borrow_mut();
            file.name = "game_copy.a78".to_string();
            file.size = Some(4);
            file.crc = Some(vec![0xDE, 0xAD, 0xBE, 0xEF]);
            file.tree_checked = TreeSelect::Selected;
            file.set_dat_got_status(DatStatus::InToSort, GotStatus::Got);
            file.parent = Some(Rc::downgrade(&tosort_dir));
        }

        tosort_dir.borrow_mut().child_add(Rc::clone(&primary_source));
        tosort_dir.borrow_mut().child_add(Rc::clone(&duplicate_source));
        root.borrow_mut().child_add(Rc::clone(&tosort_dir));

        FindFixes::scan_files(Rc::clone(&root));

        let source_statuses = [primary_source.borrow().rep_status(), duplicate_source.borrow().rep_status()];
        assert!(source_statuses.contains(&RepStatus::NeededForFix));
        assert!(source_statuses.contains(&RepStatus::Delete));
        assert_eq!(missing_child.borrow().rep_status(), RepStatus::CanBeFixed);
    }

    #[test]
    fn test_find_fixes_deletes_tosort_duplicate_when_matching_romroot_file_is_unselected() {
        let root = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));

        let romroot_dir = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
        {
            let mut dir = romroot_dir.borrow_mut();
            dir.name = "RomRoot".to_string();
            dir.tree_checked = TreeSelect::UnSelected;
            dir.parent = Some(Rc::downgrade(&root));
        }

        let romroot_file = Rc::new(RefCell::new(RvFile::new(FileType::File)));
        {
            let mut file = romroot_file.borrow_mut();
            file.name = "game.a78".to_string();
            file.size = Some(4);
            file.crc = Some(vec![0xDE, 0xAD, 0xBE, 0xEF]);
            file.tree_checked = TreeSelect::UnSelected;
            file.set_dat_got_status(DatStatus::InDatCollect, GotStatus::Got);
            file.parent = Some(Rc::downgrade(&romroot_dir));
        }
        romroot_dir.borrow_mut().child_add(Rc::clone(&romroot_file));
        root.borrow_mut().child_add(Rc::clone(&romroot_dir));

        let tosort_dir = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
        {
            let mut dir = tosort_dir.borrow_mut();
            dir.name = "ToSort".to_string();
            dir.tree_checked = TreeSelect::Selected;
            dir.parent = Some(Rc::downgrade(&root));
        }

        let tosort_file = Rc::new(RefCell::new(RvFile::new(FileType::File)));
        {
            let mut file = tosort_file.borrow_mut();
            file.name = "game_copy.a78".to_string();
            file.size = Some(4);
            file.crc = Some(vec![0xDE, 0xAD, 0xBE, 0xEF]);
            file.tree_checked = TreeSelect::Selected;
            file.set_dat_got_status(DatStatus::InToSort, GotStatus::Got);
            file.parent = Some(Rc::downgrade(&tosort_dir));
        }
        tosort_dir.borrow_mut().child_add(Rc::clone(&tosort_file));
        root.borrow_mut().child_add(Rc::clone(&tosort_dir));

        FindFixes::scan_files(Rc::clone(&root));

        assert_eq!(romroot_file.borrow().rep_status(), RepStatus::Correct);
        assert_eq!(tosort_file.borrow().rep_status(), RepStatus::Delete);
    }

    #[test]
    fn test_find_fixes_keeps_tosort_duplicate_when_romroot_copy_exists_on_disk_but_is_not_in_db_yet() {
        let temp = tempdir().unwrap();
        let root = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
        root.borrow_mut().name = temp.path().to_string_lossy().to_string();

        let romroot_dir = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
        {
            let mut dir = romroot_dir.borrow_mut();
            dir.name = temp.path().join("RomRoot").to_string_lossy().to_string();
            dir.tree_checked = TreeSelect::Selected;
            dir.parent = Some(Rc::downgrade(&root));
        }
        root.borrow_mut().child_add(Rc::clone(&romroot_dir));

        let tosort_dir = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
        {
            let mut dir = tosort_dir.borrow_mut();
            dir.name = temp.path().join("ToSort").to_string_lossy().to_string();
            dir.tree_checked = TreeSelect::Selected;
            dir.set_dat_status(DatStatus::InToSort);
            dir.parent = Some(Rc::downgrade(&root));
        }
        root.borrow_mut().child_add(Rc::clone(&tosort_dir));

        fs::create_dir_all(temp.path().join("RomRoot")).unwrap();
        fs::create_dir_all(temp.path().join("ToSort")).unwrap();
        fs::write(temp.path().join("RomRoot").join("game.a78"), b"data").unwrap();
        fs::write(temp.path().join("ToSort").join("game_copy.a78"), b"data").unwrap();

        let tosort_scan = Scanner::scan_directory_with_level(&tosort_dir.borrow().name, crate::settings::EScanLevel::Level2);
        let mut tosort_scanned = crate::scanned_file::ScannedFile::new(FileType::Dir);
        tosort_scanned.name = tosort_dir.borrow().name.clone();
        tosort_scanned.children = tosort_scan;
        FileScanning::scan_dir_with_level(Rc::clone(&tosort_dir), &mut tosort_scanned, crate::settings::EScanLevel::Level2);

        FindFixes::scan_files(Rc::clone(&root));

        let scanned_tosort_file = tosort_dir.borrow().children[0].clone();
        assert_eq!(scanned_tosort_file.borrow().rep_status(), RepStatus::InToSort);
    }

    #[test]
    fn test_find_fixes_level1_scan_hydrates_existing_dat_file_and_deletes_tosort_duplicate() {
        let temp = tempdir().unwrap();
        let root = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
        root.borrow_mut().name = temp.path().to_string_lossy().to_string();

        let romroot_dir = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
        {
            let mut dir = romroot_dir.borrow_mut();
            dir.name = temp.path().join("RomRoot").to_string_lossy().to_string();
            dir.tree_checked = TreeSelect::Selected;
            dir.parent = Some(Rc::downgrade(&root));
        }
        let romroot_file = Rc::new(RefCell::new(RvFile::new(FileType::File)));
        {
            let mut file = romroot_file.borrow_mut();
            file.name = "game.a78".to_string();
            file.size = Some(4);
            file.tree_checked = TreeSelect::Selected;
            file.set_dat_got_status(DatStatus::InDatCollect, GotStatus::NotGot);
            file.parent = Some(Rc::downgrade(&romroot_dir));
        }
        romroot_dir.borrow_mut().child_add(Rc::clone(&romroot_file));
        root.borrow_mut().child_add(Rc::clone(&romroot_dir));

        let tosort_dir = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
        {
            let mut dir = tosort_dir.borrow_mut();
            dir.name = temp.path().join("ToSort").to_string_lossy().to_string();
            dir.tree_checked = TreeSelect::Selected;
            dir.set_dat_status(DatStatus::InToSort);
            dir.parent = Some(Rc::downgrade(&root));
        }
        root.borrow_mut().child_add(Rc::clone(&tosort_dir));

        fs::create_dir_all(temp.path().join("RomRoot")).unwrap();
        fs::create_dir_all(temp.path().join("ToSort")).unwrap();
        fs::write(temp.path().join("RomRoot").join("game.a78"), b"data").unwrap();
        fs::write(temp.path().join("ToSort").join("game_copy.a78"), b"data").unwrap();

        let romroot_scan_l1 = Scanner::scan_directory_with_level(&romroot_dir.borrow().name, crate::settings::EScanLevel::Level1);
        let mut romroot_scanned_l1 = crate::scanned_file::ScannedFile::new(FileType::Dir);
        romroot_scanned_l1.name = romroot_dir.borrow().name.clone();
        romroot_scanned_l1.children = romroot_scan_l1;
        FileScanning::scan_dir_with_level(Rc::clone(&romroot_dir), &mut romroot_scanned_l1, crate::settings::EScanLevel::Level1);

        let tosort_scan_l2 = Scanner::scan_directory_with_level(&tosort_dir.borrow().name, crate::settings::EScanLevel::Level2);
        let mut tosort_scanned_l2 = crate::scanned_file::ScannedFile::new(FileType::Dir);
        tosort_scanned_l2.name = tosort_dir.borrow().name.clone();
        tosort_scanned_l2.children = tosort_scan_l2;
        FileScanning::scan_dir_with_level(Rc::clone(&tosort_dir), &mut tosort_scanned_l2, crate::settings::EScanLevel::Level2);

        FindFixes::scan_files(Rc::clone(&root));

        let scanned_tosort_file = tosort_dir.borrow().children[0].clone();
        assert_eq!(romroot_file.borrow().got_status(), GotStatus::Got);
        assert!(romroot_file.borrow().crc.is_some());
        assert_eq!(scanned_tosort_file.borrow().rep_status(), RepStatus::Delete);

        let romroot_scan_l2 = Scanner::scan_directory_with_level(&romroot_dir.borrow().name, crate::settings::EScanLevel::Level2);
        let mut romroot_scanned_l2 = crate::scanned_file::ScannedFile::new(FileType::Dir);
        romroot_scanned_l2.name = romroot_dir.borrow().name.clone();
        romroot_scanned_l2.children = romroot_scan_l2;
        FileScanning::scan_dir_with_level(Rc::clone(&romroot_dir), &mut romroot_scanned_l2, crate::settings::EScanLevel::Level2);

        FindFixes::scan_files(Rc::clone(&root));

        assert!(romroot_file.borrow().crc.is_some());
        assert_eq!(scanned_tosort_file.borrow().rep_status(), RepStatus::Delete);
    }

    #[test]
    fn test_find_fixes_deletes_tosort_duplicate_when_existing_dat_node_has_physical_file_but_no_cached_hashes() {
        let temp = tempdir().unwrap();
        let root = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
        root.borrow_mut().name = temp.path().to_string_lossy().to_string();

        let romroot_dir = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
        {
            let mut dir = romroot_dir.borrow_mut();
            dir.name = temp.path().join("RomRoot").to_string_lossy().to_string();
            dir.tree_checked = TreeSelect::Selected;
            dir.parent = Some(Rc::downgrade(&root));
        }
        let romroot_file = Rc::new(RefCell::new(RvFile::new(FileType::File)));
        {
            let mut file = romroot_file.borrow_mut();
            file.name = "game.a78".to_string();
            file.size = Some(4);
            file.tree_checked = TreeSelect::Selected;
            file.set_dat_got_status(DatStatus::InDatCollect, GotStatus::NotGot);
            file.parent = Some(Rc::downgrade(&romroot_dir));
        }
        romroot_dir.borrow_mut().child_add(Rc::clone(&romroot_file));
        root.borrow_mut().child_add(Rc::clone(&romroot_dir));

        let tosort_dir = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
        {
            let mut dir = tosort_dir.borrow_mut();
            dir.name = "ToSort".to_string();
            dir.tree_checked = TreeSelect::Selected;
            dir.parent = Some(Rc::downgrade(&root));
        }
        let tosort_file = Rc::new(RefCell::new(RvFile::new(FileType::File)));
        {
            let mut file = tosort_file.borrow_mut();
            file.name = "game_copy.a78".to_string();
            file.size = Some(4);
            file.crc = Some(vec![0x63, 0xf3, 0xf9, 0x95]);
            file.md5 = Some(vec![0x8d, 0x77, 0x7f, 0x38, 0x5d, 0x3d, 0xfe, 0xc8, 0x81, 0x5d, 0x20, 0xf7, 0x49, 0x60, 0x26, 0xdc]);
            file.sha1 = Some(vec![0xa1, 0x7c, 0x9a, 0xaa, 0x61, 0xe8, 0x0a, 0x1b, 0xf7, 0x1d, 0x0d, 0x85, 0x0a, 0xf4, 0xe5, 0xba, 0xa9, 0x80, 0x0b, 0xbd]);
            file.tree_checked = TreeSelect::Selected;
            file.set_dat_got_status(DatStatus::InToSort, GotStatus::Got);
            file.parent = Some(Rc::downgrade(&tosort_dir));
        }
        tosort_dir.borrow_mut().child_add(Rc::clone(&tosort_file));
        root.borrow_mut().child_add(Rc::clone(&tosort_dir));

        fs::create_dir_all(temp.path().join("RomRoot")).unwrap();
        fs::write(temp.path().join("RomRoot").join("game.a78"), b"data").unwrap();

        FindFixes::scan_files(Rc::clone(&root));

        assert_eq!(tosort_file.borrow().rep_status(), RepStatus::Delete);
    }

    #[test]
    fn test_find_fixes_marks_overlapping_mapping_merged_view_as_not_collected() {
        let temp = tempdir().unwrap();
        let shared_path = temp.path().join("Shared");
        let nested_path = shared_path.join("Nested");
        fs::create_dir_all(&nested_path).unwrap();
        let original_settings = get_settings();
        update_settings(Settings::default());
        set_dir_mapping(DirMapping {
            dir_key: "DatA".to_string(),
            dir_path: shared_path.to_string_lossy().into_owned(),
        });
        set_dir_mapping(DirMapping {
            dir_key: "DatA\\Subset".to_string(),
            dir_path: nested_path.to_string_lossy().into_owned(),
        });
        set_dir_mapping(DirMapping {
            dir_key: "DatB".to_string(),
            dir_path: nested_path.to_string_lossy().into_owned(),
        });

        let root = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));

        let dat_a = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
        {
            let mut dir = dat_a.borrow_mut();
            dir.name = "DatA".to_string();
            dir.tree_checked = TreeSelect::Selected;
            dir.parent = Some(Rc::downgrade(&root));
        }
        let dat_a_subset = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
        {
            let mut dir = dat_a_subset.borrow_mut();
            dir.name = "Subset".to_string();
            dir.tree_checked = TreeSelect::Selected;
            dir.parent = Some(Rc::downgrade(&dat_a));
        }
        let kept_file = Rc::new(RefCell::new(RvFile::new(FileType::File)));
        {
            let mut file = kept_file.borrow_mut();
            file.name = "shared.bin".to_string();
            file.size = Some(1024);
            file.crc = Some(vec![0x01, 0x02, 0x03, 0x04]);
            file.set_dat_got_status(DatStatus::InDatCollect, GotStatus::Got);
            file.tree_checked = TreeSelect::Selected;
            file.parent = Some(Rc::downgrade(&dat_a_subset));
        }
        dat_a_subset.borrow_mut().child_add(Rc::clone(&kept_file));
        dat_a.borrow_mut().child_add(Rc::clone(&dat_a_subset));

        let dat_b = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
        {
            let mut dir = dat_b.borrow_mut();
            dir.name = "DatB".to_string();
            dir.tree_checked = TreeSelect::Selected;
            dir.parent = Some(Rc::downgrade(&root));
        }
        let merged_view = Rc::new(RefCell::new(RvFile::new(FileType::File)));
        {
            let mut file = merged_view.borrow_mut();
            file.name = "shared.bin".to_string();
            file.size = Some(1024);
            file.crc = Some(vec![0x01, 0x02, 0x03, 0x04]);
            file.set_dat_got_status(DatStatus::InDatMerged, GotStatus::Got);
            file.tree_checked = TreeSelect::Selected;
            file.parent = Some(Rc::downgrade(&dat_b));
        }
        dat_b.borrow_mut().child_add(Rc::clone(&merged_view));

        root.borrow_mut().child_add(Rc::clone(&dat_a));
        root.borrow_mut().child_add(Rc::clone(&dat_b));

        FindFixes::scan_files(Rc::clone(&root));

        assert_eq!(kept_file.borrow().rep_status(), RepStatus::Correct);
        assert_eq!(merged_view.borrow().rep_status(), RepStatus::NotCollected);

        update_settings(original_settings);
    }

    #[test]
    fn test_find_fixes_marks_case_only_shared_archive_member_view_as_not_collected() {
        let temp = tempdir().unwrap();
        let shared_path = temp.path().join("Shared");
        fs::create_dir_all(&shared_path).unwrap();
        let original_settings = get_settings();
        update_settings(Settings::default());
        set_dir_mapping(DirMapping {
            dir_key: "DatA".to_string(),
            dir_path: shared_path.to_string_lossy().into_owned(),
        });
        set_dir_mapping(DirMapping {
            dir_key: "DatB".to_string(),
            dir_path: shared_path.to_string_lossy().into_owned(),
        });

        let root = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));

        let dat_a = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
        {
            let mut dir = dat_a.borrow_mut();
            dir.name = "DatA".to_string();
            dir.tree_checked = TreeSelect::Selected;
            dir.parent = Some(Rc::downgrade(&root));
        }
        let archive_a = Rc::new(RefCell::new(RvFile::new(FileType::Zip)));
        {
            let mut archive = archive_a.borrow_mut();
            archive.name = "shared.zip".to_string();
            archive.tree_checked = TreeSelect::Selected;
            archive.parent = Some(Rc::downgrade(&dat_a));
        }
        let kept_member = Rc::new(RefCell::new(RvFile::new(FileType::File)));
        {
            let mut file = kept_member.borrow_mut();
            file.name = "rom.bin".to_string();
            file.file_name = "ROM.BIN".to_string();
            file.size = Some(1024);
            file.crc = Some(vec![0x01, 0x02, 0x03, 0x04]);
            file.set_dat_got_status(DatStatus::InDatCollect, GotStatus::Got);
            file.tree_checked = TreeSelect::Selected;
            file.parent = Some(Rc::downgrade(&archive_a));
        }
        archive_a.borrow_mut().child_add(Rc::clone(&kept_member));
        dat_a.borrow_mut().child_add(Rc::clone(&archive_a));

        let dat_b = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
        {
            let mut dir = dat_b.borrow_mut();
            dir.name = "DatB".to_string();
            dir.tree_checked = TreeSelect::Selected;
            dir.parent = Some(Rc::downgrade(&root));
        }
        let archive_b = Rc::new(RefCell::new(RvFile::new(FileType::Zip)));
        {
            let mut archive = archive_b.borrow_mut();
            archive.name = "shared.zip".to_string();
            archive.file_name = "SHARED.ZIP".to_string();
            archive.tree_checked = TreeSelect::Selected;
            archive.parent = Some(Rc::downgrade(&dat_b));
        }
        let merged_member = Rc::new(RefCell::new(RvFile::new(FileType::File)));
        {
            let mut file = merged_member.borrow_mut();
            file.name = "rom.bin".to_string();
            file.file_name = "rom.bin".to_string();
            file.size = Some(1024);
            file.crc = Some(vec![0x01, 0x02, 0x03, 0x04]);
            file.set_dat_got_status(DatStatus::InDatMerged, GotStatus::Got);
            file.tree_checked = TreeSelect::Selected;
            file.parent = Some(Rc::downgrade(&archive_b));
        }
        archive_b.borrow_mut().child_add(Rc::clone(&merged_member));
        dat_b.borrow_mut().child_add(Rc::clone(&archive_b));

        root.borrow_mut().child_add(Rc::clone(&dat_a));
        root.borrow_mut().child_add(Rc::clone(&dat_b));

        FindFixes::scan_files(Rc::clone(&root));

        assert_eq!(kept_member.borrow().rep_status(), RepStatus::Correct);
        assert_eq!(merged_member.borrow().rep_status(), RepStatus::NotCollected);

        update_settings(original_settings);
    }

    #[test]
    fn test_find_fixes_does_not_delete_notindat_when_crc_matches_but_verified_sha1_differs() {
        let root = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));

        let mut dat_file = RvFile::new(FileType::File);
        dat_file.name = "dat.bin".to_string();
        dat_file.size = Some(4);
        dat_file.crc = Some(vec![0, 0, 0, 1]);
        dat_file.sha1 = Some(vec![1; 20]);
        dat_file.deep_scanned = true;
        dat_file.set_dat_got_status(DatStatus::InDatCollect, GotStatus::Got);
        dat_file.tree_checked = TreeSelect::Selected;
        let dat_file = Rc::new(RefCell::new(dat_file));
        root.borrow_mut().child_add(Rc::clone(&dat_file));

        let mut unknown = RvFile::new(FileType::File);
        unknown.name = "unknown.bin".to_string();
        unknown.size = Some(4);
        unknown.crc = Some(vec![0, 0, 0, 1]);
        unknown.sha1 = Some(vec![2; 20]);
        unknown.deep_scanned = true;
        unknown.set_dat_got_status(DatStatus::NotInDat, GotStatus::Got);
        unknown.tree_checked = TreeSelect::Selected;
        let unknown = Rc::new(RefCell::new(unknown));
        root.borrow_mut().child_add(Rc::clone(&unknown));

        FindFixes::scan_files(Rc::clone(&root));

        assert_eq!(dat_file.borrow().rep_status(), RepStatus::Correct);
        assert_eq!(unknown.borrow().rep_status(), RepStatus::MoveToSort);
    }

    #[test]
    fn test_find_fixes_deletes_notindat_when_verified_alt_sha1_matches_existing_dat_sha1() {
        let root = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));

        let mut dat_file = RvFile::new(FileType::File);
        dat_file.name = "dat.bin".to_string();
        dat_file.size = Some(4);
        dat_file.crc = Some(vec![0, 0, 0, 1]);
        dat_file.sha1 = Some(vec![1; 20]);
        dat_file.deep_scanned = true;
        dat_file.set_dat_got_status(DatStatus::InDatCollect, GotStatus::Got);
        dat_file.tree_checked = TreeSelect::Selected;
        let dat_file = Rc::new(RefCell::new(dat_file));
        root.borrow_mut().child_add(Rc::clone(&dat_file));

        let mut unknown = RvFile::new(FileType::File);
        unknown.name = "unknown.bin".to_string();
        unknown.size = Some(4);
        unknown.crc = Some(vec![0, 0, 0, 1]);
        unknown.sha1 = Some(vec![2; 20]);
        unknown.alt_sha1 = Some(vec![1; 20]);
        unknown.deep_scanned = true;
        unknown.set_dat_got_status(DatStatus::NotInDat, GotStatus::Got);
        unknown.tree_checked = TreeSelect::Selected;
        let unknown = Rc::new(RefCell::new(unknown));
        root.borrow_mut().child_add(Rc::clone(&unknown));

        FindFixes::scan_files(Rc::clone(&root));

        assert_eq!(dat_file.borrow().rep_status(), RepStatus::Correct);
        assert_eq!(unknown.borrow().rep_status(), RepStatus::Delete);
    }

    #[test]
    fn test_find_fixes_marks_indatnodump_missing_file_as_not_collected() {
        let root = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));

        let nodump_file = Rc::new(RefCell::new(RvFile::new(FileType::File)));
        {
            let mut f = nodump_file.borrow_mut();
            f.name = "nodump_missing.bin".to_string();
            f.size = Some(1024);
            f.crc = Some(vec![0x01, 0x02, 0x03, 0x04]);
            f.set_dat_got_status(DatStatus::InDatNoDump, GotStatus::NotGot);
            f.tree_checked = TreeSelect::Selected;
        }
        root.borrow_mut().child_add(Rc::clone(&nodump_file));

        FindFixes::scan_files(Rc::clone(&root));

        assert_eq!(nodump_file.borrow().rep_status(), RepStatus::NotCollected);
    }

    #[test]
    fn test_find_fixes_marks_indatnodump_corrupt_file_as_unneeded() {
        let root = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));

        let nodump_file = Rc::new(RefCell::new(RvFile::new(FileType::File)));
        {
            let mut f = nodump_file.borrow_mut();
            f.name = "nodump.bin".to_string();
            f.size = Some(1024);
            f.crc = Some(vec![0x01, 0x02, 0x03, 0x04]);
            f.set_dat_got_status(DatStatus::InDatNoDump, GotStatus::Corrupt);
            f.tree_checked = TreeSelect::Selected;
        }
        root.borrow_mut().child_add(Rc::clone(&nodump_file));

        FindFixes::scan_files(Rc::clone(&root));

        assert_eq!(nodump_file.borrow().rep_status(), RepStatus::UnNeeded);
    }
