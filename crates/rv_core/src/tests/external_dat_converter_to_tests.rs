use super::*;

#[test]
fn test_convert_to_external_dat_flattens_archive_directories() {
    let root = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
    root.borrow_mut().name = "Root".to_string();

    let zip = Rc::new(RefCell::new(RvFile::new(FileType::Zip)));
    zip.borrow_mut().name = "game.zip".to_string();

    let subdir = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
    subdir.borrow_mut().name = "sub".to_string();

    let file = Rc::new(RefCell::new(RvFile::new(FileType::File)));
    {
        let mut f = file.borrow_mut();
        f.name = "inner.bin".to_string();
        f.size = Some(4);
        f.crc = Some(vec![1, 2, 3, 4]);
    }

    subdir.borrow_mut().child_add(Rc::clone(&file));
    zip.borrow_mut().child_add(Rc::clone(&subdir));
    root.borrow_mut().child_add(Rc::clone(&zip));

    let converter = ExternalDatConverterTo::new();
    let dat = converter.convert_to_external_dat(Rc::clone(&root)).unwrap();

    assert_eq!(dat.base_dir.children.len(), 1);
    let game_node = dat.base_dir.children[0].dir().unwrap();
    let child_names: Vec<_> = game_node
        .children
        .iter()
        .map(|child| child.name.clone())
        .collect();
    assert_eq!(
        child_names,
        vec!["sub/".to_string(), "sub/inner.bin".to_string()]
    );
    assert!(game_node.children[0].file().is_some());
    assert!(game_node.children[1].file().is_some());
}

#[test]
fn test_convert_to_external_dat_drops_empty_filtered_directories() {
    let root = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
    root.borrow_mut().name = "Root".to_string();

    let zip = Rc::new(RefCell::new(RvFile::new(FileType::Zip)));
    {
        let mut z = zip.borrow_mut();
        z.name = "game.zip".to_string();
    }

    let file = Rc::new(RefCell::new(RvFile::new(FileType::File)));
    {
        let mut f = file.borrow_mut();
        f.name = "present.bin".to_string();
        f.set_rep_status(RepStatus::Correct);
    }

    zip.borrow_mut().child_add(Rc::clone(&file));
    root.borrow_mut().child_add(Rc::clone(&zip));

    let mut converter = ExternalDatConverterTo::new();
    converter.filter_got = false;
    let dat = converter.convert_to_external_dat(Rc::clone(&root)).unwrap();

    assert!(dat.base_dir.children.is_empty());
}

#[test]
fn test_convert_to_external_dat_prunes_empty_archive_directory_markers() {
    let root = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
    root.borrow_mut().name = "Root".to_string();

    let zip = Rc::new(RefCell::new(RvFile::new(FileType::Zip)));
    zip.borrow_mut().name = "game.zip".to_string();

    let empty_dir = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
    empty_dir.borrow_mut().name = "empty".to_string();
    zip.borrow_mut().child_add(Rc::clone(&empty_dir));

    root.borrow_mut().child_add(Rc::clone(&zip));

    let converter = ExternalDatConverterTo::new();
    let dat = converter.convert_to_external_dat(Rc::clone(&root)).unwrap();

    assert!(dat.base_dir.children.is_empty());
}

#[test]
fn test_convert_to_external_dat_can_filter_loose_files_only() {
    let root = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
    root.borrow_mut().name = "Root".to_string();

    let loose = Rc::new(RefCell::new(RvFile::new(FileType::File)));
    {
        let mut f = loose.borrow_mut();
        f.name = "loose.bin".to_string();
        f.size = Some(1);
        f.crc = Some(vec![1, 2, 3, 4]);
        f.parent = Some(Rc::downgrade(&root));
    }
    root.borrow_mut().child_add(Rc::clone(&loose));

    let zip = Rc::new(RefCell::new(RvFile::new(FileType::Zip)));
    {
        let mut z = zip.borrow_mut();
        z.name = "game.zip".to_string();
        z.parent = Some(Rc::downgrade(&root));
    }
    root.borrow_mut().child_add(Rc::clone(&zip));

    let archive_file = Rc::new(RefCell::new(RvFile::new(FileType::File)));
    {
        let mut f = archive_file.borrow_mut();
        f.name = "inside.bin".to_string();
        f.size = Some(1);
        f.crc = Some(vec![5, 6, 7, 8]);
        f.parent = Some(Rc::downgrade(&zip));
    }
    zip.borrow_mut().child_add(Rc::clone(&archive_file));

    let mut converter = ExternalDatConverterTo::new();
    converter.filter_zips = false;
    let dat = converter.convert_to_external_dat(Rc::clone(&root)).unwrap();

    assert_eq!(dat.base_dir.children.len(), 1);
    assert_eq!(dat.base_dir.children[0].name, "loose.bin");
}

#[test]
fn test_convert_to_external_dat_can_filter_archive_files_only() {
    let root = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
    root.borrow_mut().name = "Root".to_string();

    let loose = Rc::new(RefCell::new(RvFile::new(FileType::File)));
    {
        let mut f = loose.borrow_mut();
        f.name = "loose.bin".to_string();
        f.size = Some(1);
        f.crc = Some(vec![1, 2, 3, 4]);
        f.parent = Some(Rc::downgrade(&root));
    }
    root.borrow_mut().child_add(Rc::clone(&loose));

    let zip = Rc::new(RefCell::new(RvFile::new(FileType::Zip)));
    {
        let mut z = zip.borrow_mut();
        z.name = "game.zip".to_string();
        z.parent = Some(Rc::downgrade(&root));
    }
    root.borrow_mut().child_add(Rc::clone(&zip));

    let archive_file = Rc::new(RefCell::new(RvFile::new(FileType::File)));
    {
        let mut f = archive_file.borrow_mut();
        f.name = "inside.bin".to_string();
        f.size = Some(1);
        f.crc = Some(vec![5, 6, 7, 8]);
        f.parent = Some(Rc::downgrade(&zip));
    }
    zip.borrow_mut().child_add(Rc::clone(&archive_file));

    let mut converter = ExternalDatConverterTo::new();
    converter.filter_files = false;
    let dat = converter.convert_to_external_dat(Rc::clone(&root)).unwrap();

    assert_eq!(dat.base_dir.children.len(), 1);
    let game = dat.base_dir.children[0].dir().unwrap();
    assert_eq!(dat.base_dir.children[0].name, "game");
    assert_eq!(game.children.len(), 1);
    assert_eq!(game.children[0].name, "inside.bin");
}

#[test]
fn test_convert_to_external_dat_round_trips_extended_header_fields() {
    let root = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
    root.borrow_mut().name = "Root".to_string();

    let dat = Rc::new(RefCell::new(crate::rv_dat::RvDat::new()));
    {
        let mut d = dat.borrow_mut();
        d.set_data(DatData::Id, Some("id-1".to_string()));
        d.set_data(DatData::DatName, Some("SampleDat".to_string()));
        d.set_data(DatData::RootDir, Some("Roms".to_string()));
        d.set_data(DatData::Header, Some("nes".to_string()));
        d.set_data(DatData::Compression, Some("zip".to_string()));
        d.set_data(DatData::MergeType, Some("split".to_string()));
        d.set_data(DatData::DirSetup, Some("full".to_string()));
    }
    root.borrow_mut().dir_dats.push(Rc::clone(&dat));

    let converter = ExternalDatConverterTo::new();
    let external = converter.convert_to_external_dat(Rc::clone(&root)).unwrap();

    assert_eq!(external.id, Some("id-1".to_string()));
    assert_eq!(external.name, Some("SampleDat".to_string()));
    assert_eq!(external.root_dir, Some("Roms".to_string()));
    assert_eq!(external.header, Some("nes".to_string()));
    assert_eq!(external.compression, Some("zip".to_string()));
    assert_eq!(external.merge_type, Some("split".to_string()));
    assert_eq!(external.dir, Some("full".to_string()));
}

#[test]
fn test_convert_to_external_dat_filter_merged_includes_unneeded_entries() {
    let root = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
    root.borrow_mut().name = "Root".to_string();

    let merged = Rc::new(RefCell::new(RvFile::new(FileType::File)));
    {
        let mut f = merged.borrow_mut();
        f.name = "merged.bin".to_string();
        f.size = Some(1);
        f.crc = Some(vec![1, 2, 3, 4]);
        f.set_rep_status(RepStatus::UnNeeded);
        f.parent = Some(Rc::downgrade(&root));
    }
    root.borrow_mut().child_add(Rc::clone(&merged));

    let mut converter = ExternalDatConverterTo::new();
    converter.filter_got = false;
    converter.filter_merged = true;
    let dat = converter.convert_to_external_dat(Rc::clone(&root)).unwrap();

    assert_eq!(dat.base_dir.children.len(), 1);
    assert_eq!(dat.base_dir.children[0].name, "merged.bin");
}

#[test]
fn test_convert_to_external_dat_filter_got_excludes_unneeded_entries_when_merged_and_fixable_filters_disabled(
) {
    let root = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
    root.borrow_mut().name = "Root".to_string();

    let merged = Rc::new(RefCell::new(RvFile::new(FileType::File)));
    {
        let mut f = merged.borrow_mut();
        f.name = "merged.bin".to_string();
        f.size = Some(1);
        f.crc = Some(vec![1, 2, 3, 4]);
        f.set_rep_status(RepStatus::UnNeeded);
        f.parent = Some(Rc::downgrade(&root));
    }
    root.borrow_mut().child_add(Rc::clone(&merged));

    let mut converter = ExternalDatConverterTo::new();
    converter.filter_got = true;
    converter.filter_merged = false;
    converter.filter_fixable = false;
    let dat = converter.convert_to_external_dat(Rc::clone(&root)).unwrap();

    assert!(dat.base_dir.children.is_empty());
}

#[test]
fn test_convert_to_external_dat_filter_missing_includes_corrupt_entries() {
    let root = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
    root.borrow_mut().name = "Root".to_string();

    let corrupt = Rc::new(RefCell::new(RvFile::new(FileType::File)));
    {
        let mut f = corrupt.borrow_mut();
        f.name = "bad.bin".to_string();
        f.size = Some(1);
        f.crc = Some(vec![1, 2, 3, 4]);
        f.set_rep_status(RepStatus::Corrupt);
        f.parent = Some(Rc::downgrade(&root));
    }
    root.borrow_mut().child_add(Rc::clone(&corrupt));

    let mut converter = ExternalDatConverterTo::new();
    converter.filter_got = false;
    converter.filter_missing = true;
    let dat = converter.convert_to_external_dat(Rc::clone(&root)).unwrap();

    assert_eq!(dat.base_dir.children.len(), 1);
    assert_eq!(dat.base_dir.children[0].name, "bad.bin");
}

#[test]
fn test_convert_to_external_dat_filter_missing_excludes_corrupt_entries_when_disabled() {
    let root = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
    root.borrow_mut().name = "Root".to_string();

    let corrupt = Rc::new(RefCell::new(RvFile::new(FileType::File)));
    {
        let mut f = corrupt.borrow_mut();
        f.name = "bad.bin".to_string();
        f.size = Some(1);
        f.crc = Some(vec![1, 2, 3, 4]);
        f.set_rep_status(RepStatus::Corrupt);
        f.parent = Some(Rc::downgrade(&root));
    }
    root.borrow_mut().child_add(Rc::clone(&corrupt));

    let mut converter = ExternalDatConverterTo::new();
    converter.filter_got = false;
    converter.filter_missing = false;
    let dat = converter.convert_to_external_dat(Rc::clone(&root)).unwrap();

    assert!(dat.base_dir.children.is_empty());
}

#[test]
fn test_convert_to_external_dat_filter_fixable_includes_action_status_entries() {
    let root = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
    root.borrow_mut().name = "Root".to_string();

    let pending = Rc::new(RefCell::new(RvFile::new(FileType::File)));
    {
        let mut f = pending.borrow_mut();
        f.name = "rename.bin".to_string();
        f.size = Some(1);
        f.crc = Some(vec![1, 2, 3, 4]);
        f.set_rep_status(RepStatus::Rename);
        f.parent = Some(Rc::downgrade(&root));
    }
    root.borrow_mut().child_add(Rc::clone(&pending));

    let mut converter = ExternalDatConverterTo::new();
    converter.filter_got = false;
    converter.filter_fixable = true;
    let dat = converter.convert_to_external_dat(Rc::clone(&root)).unwrap();

    assert_eq!(dat.base_dir.children.len(), 1);
    assert_eq!(dat.base_dir.children[0].name, "rename.bin");
}

#[test]
fn test_convert_to_external_dat_filter_fixable_excludes_action_status_entries_when_disabled() {
    let root = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
    root.borrow_mut().name = "Root".to_string();

    let pending = Rc::new(RefCell::new(RvFile::new(FileType::File)));
    {
        let mut f = pending.borrow_mut();
        f.name = "rename.bin".to_string();
        f.size = Some(1);
        f.crc = Some(vec![1, 2, 3, 4]);
        f.set_rep_status(RepStatus::Rename);
        f.parent = Some(Rc::downgrade(&root));
    }
    root.borrow_mut().child_add(Rc::clone(&pending));

    let mut converter = ExternalDatConverterTo::new();
    converter.filter_got = false;
    converter.filter_fixable = false;
    let dat = converter.convert_to_external_dat(Rc::clone(&root)).unwrap();

    assert!(dat.base_dir.children.is_empty());
}

#[test]
fn test_convert_to_external_dat_filter_fixable_includes_intosort_entries() {
    let root = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
    root.borrow_mut().name = "Root".to_string();

    let pending = Rc::new(RefCell::new(RvFile::new(FileType::File)));
    {
        let mut f = pending.borrow_mut();
        f.name = "sortme.bin".to_string();
        f.size = Some(1);
        f.crc = Some(vec![1, 2, 3, 4]);
        f.set_rep_status(RepStatus::InToSort);
        f.parent = Some(Rc::downgrade(&root));
    }
    root.borrow_mut().child_add(Rc::clone(&pending));

    let mut converter = ExternalDatConverterTo::new();
    converter.filter_got = false;
    converter.filter_fixable = true;
    let dat = converter.convert_to_external_dat(Rc::clone(&root)).unwrap();

    assert_eq!(dat.base_dir.children.len(), 1);
    assert_eq!(dat.base_dir.children[0].name, "sortme.bin");
}

#[test]
fn test_convert_to_external_dat_filter_fixable_includes_unneeded_entries() {
    let root = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
    root.borrow_mut().name = "Root".to_string();

    let pending = Rc::new(RefCell::new(RvFile::new(FileType::File)));
    {
        let mut f = pending.borrow_mut();
        f.name = "merged.bin".to_string();
        f.size = Some(1);
        f.crc = Some(vec![1, 2, 3, 4]);
        f.set_rep_status(RepStatus::UnNeeded);
        f.parent = Some(Rc::downgrade(&root));
    }
    root.borrow_mut().child_add(Rc::clone(&pending));

    let mut converter = ExternalDatConverterTo::new();
    converter.filter_got = false;
    converter.filter_merged = false;
    converter.filter_fixable = true;
    let dat = converter.convert_to_external_dat(Rc::clone(&root)).unwrap();

    assert_eq!(dat.base_dir.children.len(), 1);
    assert_eq!(dat.base_dir.children[0].name, "merged.bin");
}

#[test]
fn test_convert_to_external_dat_filter_fixable_excludes_intosort_entries_when_disabled() {
    let root = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
    root.borrow_mut().name = "Root".to_string();

    let pending = Rc::new(RefCell::new(RvFile::new(FileType::File)));
    {
        let mut f = pending.borrow_mut();
        f.name = "sortme.bin".to_string();
        f.size = Some(1);
        f.crc = Some(vec![1, 2, 3, 4]);
        f.set_rep_status(RepStatus::InToSort);
        f.parent = Some(Rc::downgrade(&root));
    }
    root.borrow_mut().child_add(Rc::clone(&pending));

    let mut converter = ExternalDatConverterTo::new();
    converter.filter_got = false;
    converter.filter_fixable = false;
    let dat = converter.convert_to_external_dat(Rc::clone(&root)).unwrap();

    assert!(dat.base_dir.children.is_empty());
}

#[test]
fn test_convert_to_external_dat_filter_fixable_excludes_unneeded_entries_when_both_fixable_and_merged_disabled(
) {
    let root = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
    root.borrow_mut().name = "Root".to_string();

    let pending = Rc::new(RefCell::new(RvFile::new(FileType::File)));
    {
        let mut f = pending.borrow_mut();
        f.name = "merged.bin".to_string();
        f.size = Some(1);
        f.crc = Some(vec![1, 2, 3, 4]);
        f.set_rep_status(RepStatus::UnNeeded);
        f.parent = Some(Rc::downgrade(&root));
    }
    root.borrow_mut().child_add(Rc::clone(&pending));

    let mut converter = ExternalDatConverterTo::new();
    converter.filter_got = false;
    converter.filter_merged = false;
    converter.filter_fixable = false;
    let dat = converter.convert_to_external_dat(Rc::clone(&root)).unwrap();

    assert!(dat.base_dir.children.is_empty());
}
