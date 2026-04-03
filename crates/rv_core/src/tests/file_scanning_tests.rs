    use super::*;
    use dat_reader::enums::GotStatus;

    #[test]
    fn test_file_scanning_integration() {
        let db_dir = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
        db_dir.borrow_mut().name = "TestDir".to_string();

        let mut existing_db_file = RvFile::new(FileType::File);
        existing_db_file.name = "exist.zip".to_string();
        existing_db_file.size = Some(100);
        existing_db_file.set_dat_got_status(dat_reader::enums::DatStatus::InDatCollect, GotStatus::NotGot);
        db_dir.borrow_mut().child_add(Rc::new(RefCell::new(existing_db_file)));

        let mut scanned_root = ScannedFile::new(FileType::Dir);
        scanned_root.name = "TestDir".to_string();

        let mut scan1 = ScannedFile::new(FileType::File);
        scan1.name = "exist.zip".to_string();
        scan1.size = Some(100);
        
        let mut scan2 = ScannedFile::new(FileType::File);
        scan2.name = "new_file.zip".to_string();
        scan2.size = Some(200);

        scanned_root.children.push(scan1);
        scanned_root.children.push(scan2);

        FileScanning::scan_dir(Rc::clone(&db_dir), &mut scanned_root);

        let dir = db_dir.borrow();
        assert_eq!(dir.children.len(), 2);
        
        // "exist.zip" should be matched and marked Got
        let c1 = dir.children[0].borrow();
        assert_eq!(c1.name, "exist.zip");
        assert_eq!(c1.got_status(), GotStatus::Got);
        assert_eq!(c1.dat_status(), dat_reader::enums::DatStatus::InDatCollect);

        // "new_file.zip" should be integrated as NotInDat but Got
        let c2 = dir.children[1].borrow();
        assert_eq!(c2.name, "new_file.zip");
        assert_eq!(c2.got_status(), GotStatus::Got);
        assert_eq!(c2.dat_status(), dat_reader::enums::DatStatus::NotInDat);
    }

    #[test]
    fn test_new_directory_found_is_fully_integrated_recursively() {
        let db_dir = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
        db_dir.borrow_mut().name = "TestDir".to_string();

        let mut scanned_root = ScannedFile::new(FileType::Dir);
        scanned_root.name = "TestDir".to_string();

        let mut scanned_subdir = ScannedFile::new(FileType::Dir);
        scanned_subdir.name = "SubDir".to_string();

        let mut nested_file = ScannedFile::new(FileType::File);
        nested_file.name = "nested.rom".to_string();
        nested_file.size = Some(123);

        scanned_subdir.children.push(nested_file);
        scanned_root.children.push(scanned_subdir);

        FileScanning::scan_dir(Rc::clone(&db_dir), &mut scanned_root);

        let dir = db_dir.borrow();
        assert_eq!(dir.children.len(), 1);

        let subdir = dir.children[0].borrow();
        assert_eq!(subdir.name, "SubDir");
        assert_eq!(subdir.children.len(), 1);
        assert_eq!(subdir.children[0].borrow().name, "nested.rom");
        assert_eq!(subdir.children[0].borrow().size, Some(123));
        assert_eq!(subdir.children[0].borrow().got_status(), GotStatus::Got);
        assert_eq!(subdir.children[0].borrow().dat_status(), dat_reader::enums::DatStatus::NotInDat);
    }

    #[test]
    fn test_match_found_updates_scanned_metadata_on_existing_file() {
        let db_dir = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
        db_dir.borrow_mut().name = "Root".to_string();

        let mut existing_db_file = RvFile::new(FileType::File);
        existing_db_file.name = "match.bin".to_string();
        existing_db_file.size = Some(2048);
        existing_db_file.file_mod_time_stamp = 123456;
        existing_db_file.set_dat_got_status(dat_reader::enums::DatStatus::InDatCollect, GotStatus::NotGot);
        let existing_db_file = Rc::new(RefCell::new(existing_db_file));
        db_dir.borrow_mut().child_add(Rc::clone(&existing_db_file));

        let mut scanned_root = ScannedFile::new(FileType::Dir);
        scanned_root.name = "Root".to_string();

        let mut scanned = ScannedFile::new(FileType::File);
        scanned.name = "match.bin".to_string();
        scanned.file_mod_time_stamp = 123456;
        scanned.size = Some(2048);
        scanned.crc = Some(vec![1, 2, 3, 4]);
        scanned.sha1 = Some(vec![5; 20]);
        scanned.md5 = Some(vec![6; 16]);
        scanned.alt_size = Some(2000);
        scanned.alt_crc = Some(vec![7, 8, 9, 10]);
        scanned.alt_sha1 = Some(vec![11; 20]);
        scanned.alt_md5 = Some(vec![12; 16]);
        scanned.local_header_offset = Some(42);
        scanned.header_file_type = dat_reader::enums::HeaderFileType::ZIP;
        scanned_root.children.push(scanned);

        FileScanning::scan_dir(Rc::clone(&db_dir), &mut scanned_root);

        let matched = existing_db_file.borrow();
        assert_eq!(matched.got_status(), GotStatus::Got);
        assert_eq!(matched.file_mod_time_stamp, 123456);
        assert_eq!(matched.size, Some(2048));
        assert_eq!(matched.crc, Some(vec![1, 2, 3, 4]));
        assert_eq!(matched.sha1, Some(vec![5; 20]));
        assert_eq!(matched.md5, Some(vec![6; 16]));
        assert_eq!(matched.alt_size, Some(2000));
        assert_eq!(matched.alt_crc, Some(vec![7, 8, 9, 10]));
        assert_eq!(matched.alt_sha1, Some(vec![11; 20]));
        assert_eq!(matched.alt_md5, Some(vec![12; 16]));
        assert_eq!(matched.local_header_offset, Some(42));
        assert_eq!(matched.header_file_type, dat_reader::enums::HeaderFileType::ZIP);
    }

    #[test]
    fn test_match_found_updates_archive_zip_structure() {
        let db_dir = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
        db_dir.borrow_mut().name = "Root".to_string();

        let mut existing_archive = RvFile::new(FileType::Zip);
        existing_archive.name = "game.zip".to_string();
        existing_archive.zip_struct = dat_reader::enums::ZipStructure::None;
        existing_archive.set_dat_got_status(dat_reader::enums::DatStatus::InDatCollect, GotStatus::NotGot);
        let existing_archive = Rc::new(RefCell::new(existing_archive));
        db_dir.borrow_mut().child_add(Rc::clone(&existing_archive));

        let mut scanned_root = ScannedFile::new(FileType::Dir);
        scanned_root.name = "Root".to_string();

        let mut scanned_archive = ScannedFile::new(FileType::Zip);
        scanned_archive.name = "game.zip".to_string();
        scanned_archive.zip_struct = dat_reader::enums::ZipStructure::ZipTrrnt;
        scanned_root.children.push(scanned_archive);

        FileScanning::scan_dir(Rc::clone(&db_dir), &mut scanned_root);

        let matched = existing_archive.borrow();
        assert_eq!(matched.got_status(), GotStatus::Got);
        assert_eq!(matched.zip_struct, dat_reader::enums::ZipStructure::ZipTrrnt);
    }

    #[test]
    fn test_match_found_marks_archive_member_file_types_got() {
        let db_dir = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
        db_dir.borrow_mut().name = "Root".to_string();

        let mut existing_db_file = RvFile::new(FileType::FileZip);
        existing_db_file.name = "member.bin".to_string();
        existing_db_file.size = Some(32);
        existing_db_file.set_dat_got_status(dat_reader::enums::DatStatus::InDatCollect, GotStatus::NotGot);
        let existing_db_file = Rc::new(RefCell::new(existing_db_file));
        db_dir.borrow_mut().child_add(Rc::clone(&existing_db_file));

        let mut scanned_root = ScannedFile::new(FileType::Dir);
        scanned_root.name = "Root".to_string();

        let mut scanned = ScannedFile::new(FileType::FileZip);
        scanned.name = "member.bin".to_string();
        scanned.size = Some(32);
        scanned_root.children.push(scanned);

        FileScanning::scan_dir(Rc::clone(&db_dir), &mut scanned_root);

        let matched = existing_db_file.borrow();
        assert_eq!(matched.got_status(), GotStatus::Got);
        assert_eq!(matched.dat_status(), dat_reader::enums::DatStatus::InDatCollect);
    }

    #[test]
    fn test_db_file_not_found_marks_archive_member_file_types_notgot() {
        let db_dir = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
        db_dir.borrow_mut().name = "Root".to_string();

        let mut existing_db_file = RvFile::new(FileType::FileSevenZip);
        existing_db_file.name = "member.bin".to_string();
        existing_db_file.size = Some(32);
        existing_db_file.set_dat_got_status(dat_reader::enums::DatStatus::InDatCollect, GotStatus::Got);
        let existing_db_file = Rc::new(RefCell::new(existing_db_file));
        db_dir.borrow_mut().child_add(Rc::clone(&existing_db_file));

        let mut scanned_root = ScannedFile::new(FileType::Dir);
        scanned_root.name = "Root".to_string();

        FileScanning::scan_dir(Rc::clone(&db_dir), &mut scanned_root);

        let missing = existing_db_file.borrow();
        assert_eq!(missing.got_status(), GotStatus::NotGot);
        assert_eq!(missing.rep_status(), crate::enums::RepStatus::Missing);
    }

    #[test]
    fn test_equal_name_hash_mismatch_marks_existing_file_corrupt_without_orphan() {
        let db_dir = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
        db_dir.borrow_mut().name = "Root".to_string();

        let mut existing_db_file = RvFile::new(FileType::File);
        existing_db_file.name = "bad.bin".to_string();
        existing_db_file.size = Some(10);
        existing_db_file.crc = Some(vec![0xAA, 0xBB, 0xCC, 0xDD]);
        existing_db_file.set_dat_got_status(dat_reader::enums::DatStatus::InDatCollect, GotStatus::NotGot);
        let existing_db_file = Rc::new(RefCell::new(existing_db_file));
        db_dir.borrow_mut().child_add(Rc::clone(&existing_db_file));

        let mut scanned_root = ScannedFile::new(FileType::Dir);
        scanned_root.name = "Root".to_string();

        let mut scanned = ScannedFile::new(FileType::File);
        scanned.name = "bad.bin".to_string();
        scanned.size = Some(10);
        scanned.crc = Some(vec![0x11, 0x22, 0x33, 0x44]);
        scanned.deep_scanned = true;
        scanned_root.children.push(scanned);

        FileScanning::scan_dir(Rc::clone(&db_dir), &mut scanned_root);

        let dir = db_dir.borrow();
        assert_eq!(dir.children.len(), 1);
        let matched = dir.children[0].borrow();
        assert_eq!(matched.name, "bad.bin");
        assert_eq!(matched.got_status(), GotStatus::Corrupt);
        assert_eq!(matched.dat_status(), dat_reader::enums::DatStatus::InDatCollect);
        assert_eq!(matched.crc, Some(vec![0x11, 0x22, 0x33, 0x44]));
    }

    #[test]
    fn test_equal_name_hash_mismatch_marks_fileonly_entry_corrupt_without_orphan() {
        let db_dir = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
        db_dir.borrow_mut().name = "Root".to_string();

        let mut existing_db_file = RvFile::new(FileType::FileOnly);
        existing_db_file.name = "bad.bin".to_string();
        existing_db_file.size = Some(10);
        existing_db_file.crc = Some(vec![0xAA, 0xBB, 0xCC, 0xDD]);
        existing_db_file.set_dat_got_status(dat_reader::enums::DatStatus::InDatCollect, GotStatus::NotGot);
        let existing_db_file = Rc::new(RefCell::new(existing_db_file));
        db_dir.borrow_mut().child_add(Rc::clone(&existing_db_file));

        let mut scanned_root = ScannedFile::new(FileType::Dir);
        scanned_root.name = "Root".to_string();

        let mut scanned = ScannedFile::new(FileType::File);
        scanned.name = "bad.bin".to_string();
        scanned.size = Some(10);
        scanned.crc = Some(vec![0x11, 0x22, 0x33, 0x44]);
        scanned.deep_scanned = true;
        scanned_root.children.push(scanned);

        FileScanning::scan_dir(Rc::clone(&db_dir), &mut scanned_root);

        let dir = db_dir.borrow();
        assert_eq!(dir.children.len(), 1);
        let matched = dir.children[0].borrow();
        assert_eq!(matched.name, "bad.bin");
        assert_eq!(matched.got_status(), GotStatus::Corrupt);
        assert_eq!(matched.dat_status(), dat_reader::enums::DatStatus::InDatCollect);
    }

    #[test]
    fn test_equal_name_hash_mismatch_marks_archive_member_entry_corrupt_without_orphan() {
        let db_dir = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
        db_dir.borrow_mut().name = "Root".to_string();

        let mut existing_db_file = RvFile::new(FileType::FileZip);
        existing_db_file.name = "bad.bin".to_string();
        existing_db_file.size = Some(10);
        existing_db_file.crc = Some(vec![0xAA, 0xBB, 0xCC, 0xDD]);
        existing_db_file.set_dat_got_status(dat_reader::enums::DatStatus::InDatCollect, GotStatus::NotGot);
        let existing_db_file = Rc::new(RefCell::new(existing_db_file));
        db_dir.borrow_mut().child_add(Rc::clone(&existing_db_file));

        let mut scanned_root = ScannedFile::new(FileType::Dir);
        scanned_root.name = "Root".to_string();

        let mut scanned = ScannedFile::new(FileType::File);
        scanned.name = "bad.bin".to_string();
        scanned.size = Some(10);
        scanned.crc = Some(vec![0x11, 0x22, 0x33, 0x44]);
        scanned.deep_scanned = true;
        scanned_root.children.push(scanned);

        FileScanning::scan_dir(Rc::clone(&db_dir), &mut scanned_root);

        let dir = db_dir.borrow();
        assert_eq!(dir.children.len(), 1);
        let matched = dir.children[0].borrow();
        assert_eq!(matched.name, "bad.bin");
        assert_eq!(matched.got_status(), GotStatus::Corrupt);
        assert_eq!(matched.dat_status(), dat_reader::enums::DatStatus::InDatCollect);
    }

    #[test]
    fn test_renamed_file_is_not_matched_during_scan_and_creates_orphan() {
        let db_dir = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
        db_dir.borrow_mut().name = "Root".to_string();

        let mut existing_db_file = RvFile::new(FileType::File);
        existing_db_file.name = "expected.bin".to_string();
        existing_db_file.size = Some(4);
        existing_db_file.crc = Some(vec![0xAD, 0xF3, 0xF3, 0x63]);
        existing_db_file.set_dat_got_status(dat_reader::enums::DatStatus::InDatCollect, GotStatus::NotGot);
        let existing_db_file = Rc::new(RefCell::new(existing_db_file));
        db_dir.borrow_mut().child_add(Rc::clone(&existing_db_file));

        let mut scanned_root = ScannedFile::new(FileType::Dir);
        scanned_root.name = "Root".to_string();

        let mut scanned = ScannedFile::new(FileType::File);
        scanned.name = "renamed.bin".to_string();
        scanned.size = Some(4);
        scanned.crc = Some(vec![0xAD, 0xF3, 0xF3, 0x63]);
        scanned.deep_scanned = true;
        scanned_root.children.push(scanned);

        FileScanning::scan_dir(Rc::clone(&db_dir), &mut scanned_root);

        let dir = db_dir.borrow();
        assert_eq!(dir.children.len(), 2);
        let expected = dir.children.iter().find(|child| child.borrow().name == "expected.bin").unwrap();
        let orphan = dir.children.iter().find(|child| child.borrow().name == "renamed.bin").unwrap();
        assert_eq!(expected.borrow().got_status(), GotStatus::NotGot);
        assert_eq!(expected.borrow().dat_status(), dat_reader::enums::DatStatus::InDatCollect);
        assert_eq!(orphan.borrow().got_status(), GotStatus::Got);
        assert_eq!(orphan.borrow().dat_status(), dat_reader::enums::DatStatus::NotInDat);
    }

    #[test]
    fn test_case_only_name_difference_matches_existing_file_on_windows_style_scan() {
        let db_dir = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
        db_dir.borrow_mut().name = "Root".to_string();

        let mut existing_db_file = RvFile::new(FileType::File);
        existing_db_file.name = "Game.zip".to_string();
        existing_db_file.size = Some(100);
        existing_db_file.file_mod_time_stamp = 123456;
        existing_db_file.set_dat_got_status(dat_reader::enums::DatStatus::InDatCollect, GotStatus::NotGot);
        let existing_db_file = Rc::new(RefCell::new(existing_db_file));
        db_dir.borrow_mut().child_add(Rc::clone(&existing_db_file));

        let mut scanned_root = ScannedFile::new(FileType::Dir);
        scanned_root.name = "Root".to_string();

        let mut scanned = ScannedFile::new(FileType::File);
        scanned.name = "game.zip".to_string();
        scanned.file_mod_time_stamp = 123456;
        scanned.size = Some(100);
        scanned_root.children.push(scanned);

        FileScanning::scan_dir(Rc::clone(&db_dir), &mut scanned_root);

        let dir = db_dir.borrow();
        assert_eq!(dir.children.len(), 1);
        let matched = dir.children[0].borrow();
        assert_eq!(matched.name, "Game.zip");
        assert_eq!(matched.got_status(), GotStatus::Got);
        assert_eq!(matched.dat_status(), dat_reader::enums::DatStatus::InDatCollect);
    }

    #[test]
    fn test_adjacent_file_candidate_is_realigned_before_orphaning() {
        let db_dir = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
        db_dir.borrow_mut().name = "Root".to_string();

        let mut expected = RvFile::new(FileType::File);
        expected.name = "target.bin".to_string();
        expected.size = Some(4);
        expected.crc = Some(vec![0xAD, 0xF3, 0xF3, 0x63]);
        expected.set_dat_got_status(dat_reader::enums::DatStatus::InDatCollect, GotStatus::NotGot);
        db_dir.borrow_mut().child_add(Rc::new(RefCell::new(expected)));

        let mut scanned_root = ScannedFile::new(FileType::Dir);
        scanned_root.name = "Root".to_string();

        let mut orphan = ScannedFile::new(FileType::File);
        orphan.name = "alpha.bin".to_string();
        orphan.size = Some(3);
        orphan.crc = Some(vec![1, 2, 3, 4]);
        orphan.deep_scanned = true;

        let mut renamed = ScannedFile::new(FileType::File);
        renamed.name = "zzz.bin".to_string();
        renamed.size = Some(4);
        renamed.crc = Some(vec![0xAD, 0xF3, 0xF3, 0x63]);
        renamed.deep_scanned = true;

        scanned_root.children.push(orphan);
        scanned_root.children.push(renamed);

        FileScanning::scan_dir(Rc::clone(&db_dir), &mut scanned_root);

        let dir = db_dir.borrow();
        assert_eq!(dir.children.len(), 3);
        let alpha = dir.children.iter().find(|child| child.borrow().name == "alpha.bin").unwrap();
        let target = dir.children.iter().find(|child| child.borrow().name == "target.bin").unwrap();
        let zzz = dir.children.iter().find(|child| child.borrow().name == "zzz.bin").unwrap();
        assert_eq!(alpha.borrow().dat_status(), dat_reader::enums::DatStatus::NotInDat);
        assert_eq!(zzz.borrow().dat_status(), dat_reader::enums::DatStatus::NotInDat);
        assert_eq!(target.borrow().got_status(), GotStatus::NotGot);
    }

    #[test]
    fn test_adjacent_db_candidate_is_realigned_before_marking_missing() {
        let db_dir = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
        db_dir.borrow_mut().name = "Root".to_string();

        let mut missing = RvFile::new(FileType::File);
        missing.name = "alpha.bin".to_string();
        missing.size = Some(2);
        missing.crc = Some(vec![9, 9, 9, 9]);
        missing.set_dat_got_status(dat_reader::enums::DatStatus::InDatCollect, GotStatus::NotGot);
        db_dir.borrow_mut().child_add(Rc::new(RefCell::new(missing)));

        let mut expected = RvFile::new(FileType::File);
        expected.name = "target.bin".to_string();
        expected.size = Some(4);
        expected.crc = Some(vec![0xAD, 0xF3, 0xF3, 0x63]);
        expected.set_dat_got_status(dat_reader::enums::DatStatus::InDatCollect, GotStatus::NotGot);
        db_dir.borrow_mut().child_add(Rc::new(RefCell::new(expected)));

        let mut scanned_root = ScannedFile::new(FileType::Dir);
        scanned_root.name = "Root".to_string();

        let mut renamed = ScannedFile::new(FileType::File);
        renamed.name = "renamed.bin".to_string();
        renamed.size = Some(4);
        renamed.crc = Some(vec![0xAD, 0xF3, 0xF3, 0x63]);
        renamed.deep_scanned = true;
        scanned_root.children.push(renamed);

        FileScanning::scan_dir(Rc::clone(&db_dir), &mut scanned_root);

        let dir = db_dir.borrow();
        assert_eq!(dir.children.len(), 3);
        let alpha = dir.children.iter().find(|child| child.borrow().name == "alpha.bin").unwrap();
        let target = dir.children.iter().find(|child| child.borrow().name == "target.bin").unwrap();
        assert_eq!(alpha.borrow().got_status(), GotStatus::NotGot);
        assert_eq!(target.borrow().got_status(), GotStatus::NotGot);
        let orphan = dir.children.iter().find(|child| child.borrow().name == "renamed.bin").unwrap();
        assert_eq!(orphan.borrow().dat_status(), dat_reader::enums::DatStatus::NotInDat);
    }

    #[test]
    fn test_two_step_file_candidate_window_is_realigned() {
        let db_dir = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
        db_dir.borrow_mut().name = "Root".to_string();

        let mut expected = RvFile::new(FileType::File);
        expected.name = "target.bin".to_string();
        expected.size = Some(4);
        expected.crc = Some(vec![0xAD, 0xF3, 0xF3, 0x63]);
        expected.set_dat_got_status(dat_reader::enums::DatStatus::InDatCollect, GotStatus::NotGot);
        db_dir.borrow_mut().child_add(Rc::new(RefCell::new(expected)));

        let mut scanned_root = ScannedFile::new(FileType::Dir);
        scanned_root.name = "Root".to_string();

        for name in ["alpha.bin", "beta.bin"] {
            let mut orphan = ScannedFile::new(FileType::File);
            orphan.name = name.to_string();
            orphan.size = Some(3);
            orphan.crc = Some(vec![1, 2, 3, 4]);
            orphan.deep_scanned = true;
            scanned_root.children.push(orphan);
        }

        let mut renamed = ScannedFile::new(FileType::File);
        renamed.name = "zzz.bin".to_string();
        renamed.size = Some(4);
        renamed.crc = Some(vec![0xAD, 0xF3, 0xF3, 0x63]);
        renamed.deep_scanned = true;
        scanned_root.children.push(renamed);

        FileScanning::scan_dir(Rc::clone(&db_dir), &mut scanned_root);

        let dir = db_dir.borrow();
        assert_eq!(dir.children.len(), 4);
        let target = dir.children.iter().find(|child| child.borrow().name == "target.bin").unwrap();
        assert_eq!(target.borrow().got_status(), GotStatus::NotGot);
    }

    #[test]
    fn test_two_step_db_candidate_window_is_realigned() {
        let db_dir = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
        db_dir.borrow_mut().name = "Root".to_string();

        for name in ["alpha.bin", "beta.bin"] {
            let mut missing = RvFile::new(FileType::File);
            missing.name = name.to_string();
            missing.size = Some(2);
            missing.crc = Some(vec![9, 9, 9, 9]);
            missing.set_dat_got_status(dat_reader::enums::DatStatus::InDatCollect, GotStatus::NotGot);
            db_dir.borrow_mut().child_add(Rc::new(RefCell::new(missing)));
        }

        let mut expected = RvFile::new(FileType::File);
        expected.name = "target.bin".to_string();
        expected.size = Some(4);
        expected.crc = Some(vec![0xAD, 0xF3, 0xF3, 0x63]);
        expected.set_dat_got_status(dat_reader::enums::DatStatus::InDatCollect, GotStatus::NotGot);
        db_dir.borrow_mut().child_add(Rc::new(RefCell::new(expected)));

        let mut scanned_root = ScannedFile::new(FileType::Dir);
        scanned_root.name = "Root".to_string();

        let mut renamed = ScannedFile::new(FileType::File);
        renamed.name = "renamed.bin".to_string();
        renamed.size = Some(4);
        renamed.crc = Some(vec![0xAD, 0xF3, 0xF3, 0x63]);
        renamed.deep_scanned = true;
        scanned_root.children.push(renamed);

        FileScanning::scan_dir(Rc::clone(&db_dir), &mut scanned_root);

        let dir = db_dir.borrow();
        assert_eq!(dir.children.len(), 4);
        let target = dir.children.iter().find(|child| child.borrow().name == "target.bin").unwrap();
        assert_eq!(target.borrow().got_status(), GotStatus::NotGot);
    }

    #[test]
    fn test_three_step_file_candidate_window_is_realigned() {
        let db_dir = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
        db_dir.borrow_mut().name = "Root".to_string();

        let mut expected = RvFile::new(FileType::File);
        expected.name = "target.bin".to_string();
        expected.size = Some(4);
        expected.crc = Some(vec![0xAD, 0xF3, 0xF3, 0x63]);
        expected.set_dat_got_status(dat_reader::enums::DatStatus::InDatCollect, GotStatus::NotGot);
        db_dir.borrow_mut().child_add(Rc::new(RefCell::new(expected)));

        let mut scanned_root = ScannedFile::new(FileType::Dir);
        scanned_root.name = "Root".to_string();

        for name in ["alpha.bin", "beta.bin", "gamma.bin"] {
            let mut orphan = ScannedFile::new(FileType::File);
            orphan.name = name.to_string();
            orphan.size = Some(3);
            orphan.crc = Some(vec![1, 2, 3, 4]);
            orphan.deep_scanned = true;
            scanned_root.children.push(orphan);
        }

        let mut renamed = ScannedFile::new(FileType::File);
        renamed.name = "zzz.bin".to_string();
        renamed.size = Some(4);
        renamed.crc = Some(vec![0xAD, 0xF3, 0xF3, 0x63]);
        renamed.deep_scanned = true;
        scanned_root.children.push(renamed);

        FileScanning::scan_dir(Rc::clone(&db_dir), &mut scanned_root);

        let dir = db_dir.borrow();
        assert_eq!(dir.children.len(), 5);
        let target = dir.children.iter().find(|child| child.borrow().name == "target.bin").unwrap();
        assert_eq!(target.borrow().got_status(), GotStatus::NotGot);
    }

    #[test]
    fn test_three_step_db_candidate_window_is_realigned() {
        let db_dir = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
        db_dir.borrow_mut().name = "Root".to_string();

        for name in ["alpha.bin", "beta.bin", "gamma.bin"] {
            let mut missing = RvFile::new(FileType::File);
            missing.name = name.to_string();
            missing.size = Some(2);
            missing.crc = Some(vec![9, 9, 9, 9]);
            missing.set_dat_got_status(dat_reader::enums::DatStatus::InDatCollect, GotStatus::NotGot);
            db_dir.borrow_mut().child_add(Rc::new(RefCell::new(missing)));
        }

        let mut expected = RvFile::new(FileType::File);
        expected.name = "target.bin".to_string();
        expected.size = Some(4);
        expected.crc = Some(vec![0xAD, 0xF3, 0xF3, 0x63]);
        expected.set_dat_got_status(dat_reader::enums::DatStatus::InDatCollect, GotStatus::NotGot);
        db_dir.borrow_mut().child_add(Rc::new(RefCell::new(expected)));

        let mut scanned_root = ScannedFile::new(FileType::Dir);
        scanned_root.name = "Root".to_string();

        let mut renamed = ScannedFile::new(FileType::File);
        renamed.name = "renamed.bin".to_string();
        renamed.size = Some(4);
        renamed.crc = Some(vec![0xAD, 0xF3, 0xF3, 0x63]);
        renamed.deep_scanned = true;
        scanned_root.children.push(renamed);

        FileScanning::scan_dir(Rc::clone(&db_dir), &mut scanned_root);

        let dir = db_dir.borrow();
        assert_eq!(dir.children.len(), 5);
        let target = dir.children.iter().find(|child| child.borrow().name == "target.bin").unwrap();
        assert_eq!(target.borrow().got_status(), GotStatus::NotGot);
        let orphan = dir.children.iter().find(|child| child.borrow().name == "renamed.bin").unwrap();
        assert_eq!(orphan.borrow().dat_status(), dat_reader::enums::DatStatus::NotInDat);
    }

    #[test]
    fn test_file_candidate_window_prefers_primary_match_over_nearer_alt_match() {
        let db_dir = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
        db_dir.borrow_mut().name = "Root".to_string();

        let mut expected = RvFile::new(FileType::File);
        expected.name = "target.bin".to_string();
        expected.size = Some(4);
        expected.crc = Some(vec![0xAA, 0xBB, 0xCC, 0xDD]);
        expected.alt_size = Some(4);
        expected.alt_crc = Some(vec![0x11, 0x22, 0x33, 0x44]);
        expected.set_dat_got_status(dat_reader::enums::DatStatus::InDatCollect, GotStatus::NotGot);
        db_dir.borrow_mut().child_add(Rc::new(RefCell::new(expected)));

        let mut scanned_root = ScannedFile::new(FileType::Dir);
        scanned_root.name = "Root".to_string();

        let mut alt_match = ScannedFile::new(FileType::File);
        alt_match.name = "alt.bin".to_string();
        alt_match.size = Some(4);
        alt_match.crc = Some(vec![0x11, 0x22, 0x33, 0x44]);
        alt_match.deep_scanned = true;

        let mut primary_match = ScannedFile::new(FileType::File);
        primary_match.name = "primary.bin".to_string();
        primary_match.size = Some(4);
        primary_match.crc = Some(vec![0xAA, 0xBB, 0xCC, 0xDD]);
        primary_match.deep_scanned = true;

        scanned_root.children.push(alt_match);
        scanned_root.children.push(primary_match);

        FileScanning::scan_dir(Rc::clone(&db_dir), &mut scanned_root);

        let dir = db_dir.borrow();
        assert_eq!(dir.children.len(), 3);
        let target = dir.children.iter().find(|child| child.borrow().name == "target.bin").unwrap();
        assert_eq!(target.borrow().got_status(), GotStatus::NotGot);
        let alt = dir.children.iter().find(|child| child.borrow().name == "alt.bin").unwrap();
        let primary = dir.children.iter().find(|child| child.borrow().name == "primary.bin").unwrap();
        assert_eq!(alt.borrow().dat_status(), dat_reader::enums::DatStatus::NotInDat);
        assert_eq!(primary.borrow().dat_status(), dat_reader::enums::DatStatus::NotInDat);
    }

    #[test]
    fn test_db_candidate_window_prefers_primary_match_over_nearer_alt_match() {
        let db_dir = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
        db_dir.borrow_mut().name = "Root".to_string();

        let mut alt_expected = RvFile::new(FileType::File);
        alt_expected.name = "alt_target.bin".to_string();
        alt_expected.size = Some(5);
        alt_expected.alt_size = Some(4);
        alt_expected.alt_crc = Some(vec![0x11, 0x22, 0x33, 0x44]);
        alt_expected.set_dat_got_status(dat_reader::enums::DatStatus::InDatCollect, GotStatus::NotGot);
        db_dir.borrow_mut().child_add(Rc::new(RefCell::new(alt_expected)));

        let mut primary_expected = RvFile::new(FileType::File);
        primary_expected.name = "primary_target.bin".to_string();
        primary_expected.size = Some(4);
        primary_expected.crc = Some(vec![0xAA, 0xBB, 0xCC, 0xDD]);
        primary_expected.set_dat_got_status(dat_reader::enums::DatStatus::InDatCollect, GotStatus::NotGot);
        db_dir.borrow_mut().child_add(Rc::new(RefCell::new(primary_expected)));

        let mut scanned_root = ScannedFile::new(FileType::Dir);
        scanned_root.name = "Root".to_string();

        let mut scanned = ScannedFile::new(FileType::File);
        scanned.name = "renamed.bin".to_string();
        scanned.size = Some(4);
        scanned.crc = Some(vec![0xAA, 0xBB, 0xCC, 0xDD]);
        scanned.alt_size = Some(4);
        scanned.alt_crc = Some(vec![0x11, 0x22, 0x33, 0x44]);
        scanned.deep_scanned = true;
        scanned_root.children.push(scanned);

        FileScanning::scan_dir(Rc::clone(&db_dir), &mut scanned_root);

        let dir = db_dir.borrow();
        let primary = dir
            .children
            .iter()
            .find(|child| child.borrow().name == "primary_target.bin")
            .unwrap();
        assert_eq!(primary.borrow().got_status(), GotStatus::NotGot);
        let alt = dir
            .children
            .iter()
            .find(|child| child.borrow().name == "alt_target.bin")
            .unwrap();
        assert_eq!(alt.borrow().got_status(), GotStatus::NotGot);
        let orphan = dir.children.iter().find(|child| child.borrow().name == "renamed.bin").unwrap();
        assert_eq!(orphan.borrow().dat_status(), dat_reader::enums::DatStatus::NotInDat);
    }

    #[test]
    fn test_file_candidate_window_prefers_primary_sha1_match_over_nearer_alt_sha1_match() {
        let db_dir = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
        db_dir.borrow_mut().name = "Root".to_string();

        let mut expected = RvFile::new(FileType::File);
        expected.name = "target.bin".to_string();
        expected.size = Some(4);
        expected.sha1 = Some(vec![0xAA, 0xBB, 0xCC, 0xDD]);
        expected.alt_size = Some(4);
        expected.alt_sha1 = Some(vec![0x11, 0x22, 0x33, 0x44]);
        expected.set_dat_got_status(dat_reader::enums::DatStatus::InDatCollect, GotStatus::NotGot);
        db_dir.borrow_mut().child_add(Rc::new(RefCell::new(expected)));

        let mut scanned_root = ScannedFile::new(FileType::Dir);
        scanned_root.name = "Root".to_string();

        let mut alt_match = ScannedFile::new(FileType::File);
        alt_match.name = "alt.bin".to_string();
        alt_match.size = Some(4);
        alt_match.sha1 = Some(vec![0x11, 0x22, 0x33, 0x44]);
        alt_match.deep_scanned = true;

        let mut primary_match = ScannedFile::new(FileType::File);
        primary_match.name = "primary.bin".to_string();
        primary_match.size = Some(4);
        primary_match.sha1 = Some(vec![0xAA, 0xBB, 0xCC, 0xDD]);
        primary_match.deep_scanned = true;

        scanned_root.children.push(alt_match);
        scanned_root.children.push(primary_match);

        FileScanning::scan_dir(Rc::clone(&db_dir), &mut scanned_root);

        let dir = db_dir.borrow();
        let target = dir.children.iter().find(|child| child.borrow().name == "target.bin").unwrap();
        assert_eq!(target.borrow().got_status(), GotStatus::NotGot);
        let alt = dir.children.iter().find(|child| child.borrow().name == "alt.bin").unwrap();
        let primary = dir.children.iter().find(|child| child.borrow().name == "primary.bin").unwrap();
        assert_eq!(alt.borrow().dat_status(), dat_reader::enums::DatStatus::NotInDat);
        assert_eq!(primary.borrow().dat_status(), dat_reader::enums::DatStatus::NotInDat);
    }

    #[test]
    fn test_db_candidate_window_prefers_primary_md5_match_over_nearer_alt_md5_match() {
        let db_dir = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
        db_dir.borrow_mut().name = "Root".to_string();

        let mut alt_expected = RvFile::new(FileType::File);
        alt_expected.name = "alt_target.bin".to_string();
        alt_expected.size = Some(5);
        alt_expected.alt_size = Some(4);
        alt_expected.alt_md5 = Some(vec![0x11, 0x22, 0x33, 0x44]);
        alt_expected.set_dat_got_status(dat_reader::enums::DatStatus::InDatCollect, GotStatus::NotGot);
        db_dir.borrow_mut().child_add(Rc::new(RefCell::new(alt_expected)));

        let mut primary_expected = RvFile::new(FileType::File);
        primary_expected.name = "primary_target.bin".to_string();
        primary_expected.size = Some(4);
        primary_expected.md5 = Some(vec![0xAA, 0xBB, 0xCC, 0xDD]);
        primary_expected.set_dat_got_status(dat_reader::enums::DatStatus::InDatCollect, GotStatus::NotGot);
        db_dir.borrow_mut().child_add(Rc::new(RefCell::new(primary_expected)));

        let mut scanned_root = ScannedFile::new(FileType::Dir);
        scanned_root.name = "Root".to_string();

        let mut scanned = ScannedFile::new(FileType::File);
        scanned.name = "renamed.bin".to_string();
        scanned.size = Some(4);
        scanned.md5 = Some(vec![0xAA, 0xBB, 0xCC, 0xDD]);
        scanned.alt_size = Some(4);
        scanned.alt_md5 = Some(vec![0x11, 0x22, 0x33, 0x44]);
        scanned.deep_scanned = true;
        scanned_root.children.push(scanned);

        FileScanning::scan_dir(Rc::clone(&db_dir), &mut scanned_root);

        let dir = db_dir.borrow();
        let primary = dir
            .children
            .iter()
            .find(|child| child.borrow().name == "primary_target.bin")
            .unwrap();
        assert_eq!(primary.borrow().got_status(), GotStatus::NotGot);
        let alt = dir
            .children
            .iter()
            .find(|child| child.borrow().name == "alt_target.bin")
            .unwrap();
        assert_eq!(alt.borrow().got_status(), GotStatus::NotGot);
        let orphan = dir.children.iter().find(|child| child.borrow().name == "renamed.bin").unwrap();
        assert_eq!(orphan.borrow().dat_status(), dat_reader::enums::DatStatus::NotInDat);
    }

    #[test]
    fn test_file_candidate_window_prefers_primary_md5_match_over_nearer_alt_md5_match() {
        let db_dir = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
        db_dir.borrow_mut().name = "Root".to_string();

        let mut expected = RvFile::new(FileType::File);
        expected.name = "target.bin".to_string();
        expected.size = Some(4);
        expected.md5 = Some(vec![0xAA, 0xBB, 0xCC, 0xDD]);
        expected.alt_size = Some(4);
        expected.alt_md5 = Some(vec![0x11, 0x22, 0x33, 0x44]);
        expected.set_dat_got_status(dat_reader::enums::DatStatus::InDatCollect, GotStatus::NotGot);
        db_dir.borrow_mut().child_add(Rc::new(RefCell::new(expected)));

        let mut scanned_root = ScannedFile::new(FileType::Dir);
        scanned_root.name = "Root".to_string();

        let mut alt_match = ScannedFile::new(FileType::File);
        alt_match.name = "alt.bin".to_string();
        alt_match.size = Some(4);
        alt_match.md5 = Some(vec![0x11, 0x22, 0x33, 0x44]);
        alt_match.deep_scanned = true;

        let mut primary_match = ScannedFile::new(FileType::File);
        primary_match.name = "primary.bin".to_string();
        primary_match.size = Some(4);
        primary_match.md5 = Some(vec![0xAA, 0xBB, 0xCC, 0xDD]);
        primary_match.deep_scanned = true;

        scanned_root.children.push(alt_match);
        scanned_root.children.push(primary_match);

        FileScanning::scan_dir(Rc::clone(&db_dir), &mut scanned_root);

        let dir = db_dir.borrow();
        let target = dir.children.iter().find(|child| child.borrow().name == "target.bin").unwrap();
        assert_eq!(target.borrow().got_status(), GotStatus::NotGot);
        let alt = dir.children.iter().find(|child| child.borrow().name == "alt.bin").unwrap();
        let primary = dir.children.iter().find(|child| child.borrow().name == "primary.bin").unwrap();
        assert_eq!(alt.borrow().dat_status(), dat_reader::enums::DatStatus::NotInDat);
        assert_eq!(primary.borrow().dat_status(), dat_reader::enums::DatStatus::NotInDat);
    }

    #[test]
    fn test_apply_scanned_metadata_preserves_dat_flags_and_updates_physical_flags() {
        let db_dir = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
        db_dir.borrow_mut().name = "Root".to_string();

        let mut existing_db_file = RvFile::new(FileType::File);
        existing_db_file.name = "headered.nes".to_string();
        existing_db_file.file_status_set(FileStatus::CRC_FROM_DAT);
        existing_db_file.set_dat_got_status(dat_reader::enums::DatStatus::InDatCollect, GotStatus::NotGot);
        let existing_db_file = Rc::new(RefCell::new(existing_db_file));
        db_dir.borrow_mut().child_add(Rc::clone(&existing_db_file));

        let mut scanned_root = ScannedFile::new(FileType::Dir);
        scanned_root.name = "Root".to_string();

        let mut scanned = ScannedFile::new(FileType::File);
        scanned.name = "headered.nes".to_string();
        scanned.header_file_type = dat_reader::enums::HeaderFileType::NES;
        scanned.status_flags = FileStatus::HEADER_FILE_TYPE_FROM_HEADER | FileStatus::ALT_CRC_FROM_HEADER;
        scanned.crc = Some(vec![1, 2, 3, 4]);
        scanned.deep_scanned = true;
        scanned_root.children.push(scanned);

        FileScanning::scan_dir(Rc::clone(&db_dir), &mut scanned_root);

        let matched = existing_db_file.borrow();
        assert!(matched.file_status_is(FileStatus::CRC_FROM_DAT));
        assert!(matched.file_status_is(FileStatus::HEADER_FILE_TYPE_FROM_HEADER));
        assert!(matched.file_status_is(FileStatus::ALT_CRC_FROM_HEADER));
    }

    #[test]
    fn test_match_found_refreshes_file_name_to_current_scanned_name() {
        let db_dir = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
        db_dir.borrow_mut().name = "Root".to_string();

        let mut existing_db_file = RvFile::new(FileType::File);
        existing_db_file.name = "Game.zip".to_string();
        existing_db_file.file_name = "old_name.zip".to_string();
        existing_db_file.size = Some(100);
        existing_db_file.crc = Some(vec![0xAA, 0xBB, 0xCC, 0xDD]);
        existing_db_file.set_dat_got_status(dat_reader::enums::DatStatus::InDatCollect, GotStatus::NotGot);
        let existing_db_file = Rc::new(RefCell::new(existing_db_file));
        db_dir.borrow_mut().child_add(Rc::clone(&existing_db_file));

        let mut scanned_root = ScannedFile::new(FileType::Dir);
        scanned_root.name = "Root".to_string();

        let mut scanned = ScannedFile::new(FileType::File);
        scanned.name = "game.zip".to_string();
        scanned.size = Some(100);
        scanned.crc = Some(vec![0xAA, 0xBB, 0xCC, 0xDD]);
        scanned.deep_scanned = true;
        scanned_root.children.push(scanned);

        FileScanning::scan_dir(Rc::clone(&db_dir), &mut scanned_root);

        let matched = existing_db_file.borrow();
        assert_eq!(matched.file_name, "game.zip");
        assert_eq!(matched.got_status(), GotStatus::Got);
    }

    #[test]
    fn test_scan_dir_handles_windows_style_case_insensitive_sort_order() {
        let db_dir = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
        db_dir.borrow_mut().name = "Root".to_string();

        let mut upper = RvFile::new(FileType::File);
        upper.name = "B.bin".to_string();
        upper.size = Some(4);
        upper.crc = Some(vec![0, 0, 0, 2]);
        upper.set_dat_got_status(dat_reader::enums::DatStatus::InDatCollect, GotStatus::NotGot);
        db_dir.borrow_mut().child_add(Rc::new(RefCell::new(upper)));

        let mut lower = RvFile::new(FileType::File);
        lower.name = "a.bin".to_string();
        lower.size = Some(4);
        lower.crc = Some(vec![0, 0, 0, 1]);
        lower.set_dat_got_status(dat_reader::enums::DatStatus::InDatCollect, GotStatus::NotGot);
        db_dir.borrow_mut().child_add(Rc::new(RefCell::new(lower)));

        let mut scanned_root = ScannedFile::new(FileType::Dir);
        scanned_root.name = "Root".to_string();

        let mut scanned_a = ScannedFile::new(FileType::File);
        scanned_a.name = "a.bin".to_string();
        scanned_a.size = Some(4);
        scanned_a.crc = Some(vec![0, 0, 0, 1]);
        scanned_root.children.push(scanned_a);

        let mut scanned_b = ScannedFile::new(FileType::File);
        scanned_b.name = "B.bin".to_string();
        scanned_b.size = Some(4);
        scanned_b.crc = Some(vec![0, 0, 0, 2]);
        scanned_root.children.push(scanned_b);

        FileScanning::scan_dir(Rc::clone(&db_dir), &mut scanned_root);

        let dir = db_dir.borrow();
        for child in &dir.children {
            assert_eq!(child.borrow().got_status(), GotStatus::Got);
        }
    }
