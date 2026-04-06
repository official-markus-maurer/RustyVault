use super::*;
use crate::settings::{get_settings, update_settings, Settings};
use tempfile::tempdir;

#[test]
fn test_populate_rv_dat_from_header_sets_extended_metadata() {
    let dir = tempdir().unwrap();
    let dat_path = dir.path().join("sample.dat");
    fs::write(&dat_path, "test").unwrap();

    let header = DatHeader {
        id: Some("id-1".to_string()),
        name: Some("SampleDat".to_string()),
        root_dir: Some("Roms".to_string()),
        description: Some("Desc".to_string()),
        category: Some("Cat".to_string()),
        version: Some("1.0".to_string()),
        date: Some("2026-01-01".to_string()),
        author: Some("Author".to_string()),
        email: Some("a@example.com".to_string()),
        homepage: Some("https://example.com".to_string()),
        url: Some("https://example.com/dat".to_string()),
        header: Some("nes".to_string()),
        compression: Some("zip".to_string()),
        merge_type: Some("split".to_string()),
        dir: Some("full".to_string()),
        ..Default::default()
    };

    let mut rv_dat = RvDat::new();
    DatUpdate::populate_rv_dat_from_header(&mut rv_dat, &header, &dat_path.to_string_lossy());

    assert_eq!(rv_dat.get_data(DatData::Id), Some("id-1".to_string()));
    assert_eq!(
        rv_dat.get_data(DatData::DatName),
        Some("SampleDat".to_string())
    );
    assert_eq!(
        rv_dat.get_data(DatData::DatRootFullName),
        Some(dat_path.to_string_lossy().to_string())
    );
    assert_eq!(rv_dat.get_data(DatData::RootDir), Some("Roms".to_string()));
    assert_eq!(
        rv_dat.get_data(DatData::Description),
        Some("Desc".to_string())
    );
    assert_eq!(rv_dat.get_data(DatData::Category), Some("Cat".to_string()));
    assert_eq!(rv_dat.get_data(DatData::Version), Some("1.0".to_string()));
    assert_eq!(rv_dat.get_data(DatData::Header), Some("nes".to_string()));
    assert_eq!(
        rv_dat.get_data(DatData::Compression),
        Some("zip".to_string())
    );
    assert_eq!(
        rv_dat.get_data(DatData::MergeType),
        Some("split".to_string())
    );
    assert_eq!(rv_dat.get_data(DatData::DirSetup), Some("full".to_string()));
    assert!(rv_dat.time_stamp > 0);
}

#[test]
fn test_update_dat_applies_header_file_type_from_dat_header_and_rule() {
    use crate::settings::{DatRule, HeaderType};
    use dat_reader::enums::HeaderFileType;

    let mut header = DatHeader {
        header: Some("No-Intro_A7800.xml".to_string()),
        ..Default::default()
    };
    header
        .base_dir
        .add_child(DatNode::new_file("game.bin".to_string(), FileType::File));

    let rule = DatRule {
        dir_key: "RustyVault".to_string(),
        header_type: HeaderType::Headered,
        ..Default::default()
    };

    DatUpdate::apply_header_rules(&mut header, &rule);

    let child = header.base_dir.children[0].file().unwrap();
    assert!(child.header_file_type.contains(HeaderFileType::A7800));
    assert!(child.header_file_type.contains(HeaderFileType::REQUIRED));
}

#[test]
fn test_map_dat_node_to_rv_file_marks_dat_sourced_flags() {
    let parent = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
    let dat_rc = Rc::new(RefCell::new(RvDat::new()));

    let mut dat_node = DatNode::new_file("rom.nes".to_string(), FileType::File);
    dat_node.date_modified = Some(12345);
    let file = dat_node.file_mut().unwrap();
    file.size = Some(1024);
    file.crc = Some(vec![1, 2, 3, 4]);
    file.sha1 = Some(vec![5; 20]);
    file.md5 = Some(vec![6; 16]);
    file.header_file_type = dat_reader::enums::HeaderFileType::NES;

    DatUpdate::map_dat_node_to_rv_file(
        Rc::clone(&parent),
        &dat_node,
        Rc::clone(&dat_rc),
        &mut Vec::new(),
    );

    let child_rc = Rc::clone(&parent.borrow().children[0]);
    let child = child_rc.borrow();
    assert!(child.file_status_is(FileStatus::SIZE_FROM_DAT));
    assert!(child.file_status_is(FileStatus::CRC_FROM_DAT));
    assert!(child.file_status_is(FileStatus::SHA1_FROM_DAT));
    assert!(child.file_status_is(FileStatus::MD5_FROM_DAT));
    assert!(child.file_status_is(FileStatus::HEADER_FILE_TYPE_FROM_DAT));
    assert!(child.file_status_is(FileStatus::DATE_FROM_DAT));
    assert_eq!(child.file_mod_time_stamp, 12345);
}

#[test]
fn test_map_dat_node_to_rv_file_preserves_existing_got_state_for_matching_node() {
    let parent = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
    let dat_rc = Rc::new(RefCell::new(RvDat::new()));

    let existing_child = Rc::new(RefCell::new(RvFile::new(FileType::File)));
    {
        let mut existing = existing_child.borrow_mut();
        existing.name = "rom.bin".to_string();
        existing.set_dat_got_status(DatStatus::InDatCollect, dat_reader::enums::GotStatus::Got);
        existing.tree_expanded = true;
        existing.tree_checked = crate::rv_file::TreeSelect::Locked;
    }

    let mut existing_children = vec![Rc::clone(&existing_child)];

    let mut dat_node = DatNode::new_file("rom.bin".to_string(), FileType::File);
    let file = dat_node.file_mut().unwrap();
    file.size = Some(4096);
    file.crc = Some(vec![1, 2, 3, 4]);

    DatUpdate::map_dat_node_to_rv_file(
        Rc::clone(&parent),
        &dat_node,
        Rc::clone(&dat_rc),
        &mut existing_children,
    );

    let mapped_child = {
        let parent_ref = parent.borrow();
        Rc::clone(&parent_ref.children[0])
    };
    let mapped = mapped_child.borrow();
    assert_eq!(mapped.got_status(), dat_reader::enums::GotStatus::Got);
    assert_eq!(mapped.rep_status(), crate::enums::RepStatus::Correct);
    assert!(mapped.tree_expanded);
    assert_eq!(mapped.tree_checked, crate::rv_file::TreeSelect::Locked);
    assert_eq!(mapped.size, Some(4096));
    assert_eq!(mapped.crc, Some(vec![1, 2, 3, 4]));
}

#[test]
fn test_map_dat_node_to_rv_file_preserves_existing_state_for_case_only_name_difference() {
    let parent = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
    let dat_rc = Rc::new(RefCell::new(RvDat::new()));

    let existing_child = Rc::new(RefCell::new(RvFile::new(FileType::File)));
    {
        let mut existing = existing_child.borrow_mut();
        existing.name = "ROM.BIN".to_string();
        existing.set_dat_got_status(DatStatus::InDatCollect, dat_reader::enums::GotStatus::Got);
        existing.tree_expanded = true;
    }

    let mut existing_children = vec![Rc::clone(&existing_child)];

    let mut dat_node = DatNode::new_file("rom.bin".to_string(), FileType::File);
    dat_node.file_mut().unwrap().size = Some(4096);

    DatUpdate::map_dat_node_to_rv_file(
        Rc::clone(&parent),
        &dat_node,
        Rc::clone(&dat_rc),
        &mut existing_children,
    );

    let mapped_child = {
        let parent_ref = parent.borrow();
        Rc::clone(&parent_ref.children[0])
    };
    let mapped = mapped_child.borrow();
    assert_eq!(mapped.name, "rom.bin");
    assert_eq!(mapped.got_status(), dat_reader::enums::GotStatus::Got);
    assert!(mapped.tree_expanded);
}

#[test]
fn test_update_dat_reuses_existing_dat_directory_for_case_only_name_difference() {
    let original_settings = get_settings();
    let temp = tempdir().unwrap();
    let dat_root = temp.path().join("DatRoot");
    fs::create_dir_all(&dat_root).unwrap();
    fs::write(
        dat_root.join("sample.dat"),
        r#"<?xml version="1.0"?>
<datafile>
  <header>
    <name>mydat</name>
  </header>
</datafile>"#,
    )
    .unwrap();

    let settings = Settings {
        dat_root: dat_root.to_string_lossy().into_owned(),
        ..Default::default()
    };
    update_settings(settings);

    let root = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
    let rustyvault = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
    rustyvault.borrow_mut().name = "RustyVault".to_string();
    rustyvault.borrow_mut().parent = Some(Rc::downgrade(&root));
    root.borrow_mut().child_add(Rc::clone(&rustyvault));

    let existing_dir = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
    existing_dir.borrow_mut().name = "MYDAT".to_string();
    existing_dir.borrow_mut().parent = Some(Rc::downgrade(&rustyvault));
    rustyvault.borrow_mut().child_add(Rc::clone(&existing_dir));

    DatUpdate::update_dat(Rc::clone(&root), &dat_root.to_string_lossy());
    update_settings(original_settings);

    let rustyvault_ref = rustyvault.borrow();
    assert_eq!(rustyvault_ref.children.len(), 1);
    let dat_dir = rustyvault_ref.children[0].borrow();
    assert_eq!(dat_dir.name, "MYDAT");
    assert_eq!(dat_dir.dir_dats.len(), 1);
}

#[test]
fn test_update_dat_creates_virtual_dir_and_category_and_uses_rule_dir_name_and_overrides() {
    use crate::settings::{DatRule, FilterType, HeaderType, MergeType};

    let original_settings = get_settings();
    let temp = tempdir().unwrap();
    let dat_root = temp.path().join("DatRoot");
    let nested = dat_root.join("Platforms").join("Nintendo");
    fs::create_dir_all(&nested).unwrap();

    fs::write(
        nested.join("sample.xml"),
        r#"<?xml version="1.0"?>
<datafile>
  <header>
    <name>MyDat</name>
    <description>DescDir</description>
    <category>CatDir</category>
    <id>ID-1</id>
    <compression>zip</compression>
    <merge>split</merge>
  </header>
</datafile>"#,
    )
    .unwrap();

    let mut settings = Settings {
        dat_root: dat_root.to_string_lossy().into_owned(),
        ..Default::default()
    };
    settings.dat_rules.items.push(DatRule {
        dir_key: "RustyVault\\Platforms\\Nintendo\\MyDat".to_string(),
        add_category_sub_dirs: true,
        use_description_as_dir_name: true,
        merge_override_dat: true,
        merge: MergeType::NonMerged,
        compression_override_dat: true,
        compression: dat_reader::enums::FileType::SevenZip,
        filter: FilterType::KeepAll,
        header_type: HeaderType::Optional,
        ..Default::default()
    });
    update_settings(settings);

    let root = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
    let rustyvault = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
    rustyvault.borrow_mut().name = "RustyVault".to_string();
    rustyvault.borrow_mut().parent = Some(Rc::downgrade(&root));
    root.borrow_mut().child_add(Rc::clone(&rustyvault));

    DatUpdate::update_dat(Rc::clone(&root), &dat_root.to_string_lossy());
    update_settings(original_settings);

    let rv = rustyvault.borrow();
    let platforms = rv
        .children
        .iter()
        .find(|c| c.borrow().name == "Platforms")
        .unwrap();
    let platforms = platforms.borrow();
    let nintendo = platforms
        .children
        .iter()
        .find(|c| c.borrow().name == "Nintendo")
        .unwrap();
    let nintendo = nintendo.borrow();
    let cat = nintendo
        .children
        .iter()
        .find(|c| c.borrow().name == "CatDir")
        .unwrap();
    let cat = cat.borrow();
    let dat_dir = cat
        .children
        .iter()
        .find(|c| c.borrow().name == "DescDir")
        .unwrap();
    let dat_dir = dat_dir.borrow();

    assert_eq!(dat_dir.dir_dats.len(), 1);
    let dat = dat_dir.dir_dats[0].borrow();
    assert_eq!(dat.get_data(DatData::Compression), Some("7z".to_string()));
    assert_eq!(
        dat.get_data(DatData::MergeType),
        Some("nonmerged".to_string())
    );
}

#[test]
fn test_map_dat_node_to_rv_file_preserves_existing_physical_metadata_for_matching_node() {
    let parent = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
    let dat_rc = Rc::new(RefCell::new(RvDat::new()));

    let existing_child = Rc::new(RefCell::new(RvFile::new(FileType::File)));
    {
        let mut existing = existing_child.borrow_mut();
        existing.name = "rom.bin".to_string();
        existing.set_dat_got_status(DatStatus::InDatCollect, dat_reader::enums::GotStatus::Got);
        existing.size = Some(8192);
        existing.crc = Some(vec![9, 9, 9, 9]);
        existing.sha1 = Some(vec![8; 20]);
        existing.file_mod_time_stamp = 777;
        existing.header_file_type = HeaderFileType::NES;
        existing.file_status_set(FileStatus::HEADER_FILE_TYPE_FROM_HEADER);
        existing.local_header_offset = Some(55);
    }

    let mut existing_children = vec![Rc::clone(&existing_child)];

    let mut dat_node = DatNode::new_file("rom.bin".to_string(), FileType::File);
    dat_node.date_modified = Some(12345);
    let file = dat_node.file_mut().unwrap();
    file.size = Some(4096);
    file.crc = Some(vec![1, 2, 3, 4]);
    file.header_file_type = HeaderFileType::SNES | HeaderFileType::REQUIRED;

    DatUpdate::map_dat_node_to_rv_file(
        Rc::clone(&parent),
        &dat_node,
        Rc::clone(&dat_rc),
        &mut existing_children,
    );

    let mapped_child = {
        let parent_ref = parent.borrow();
        Rc::clone(&parent_ref.children[0])
    };
    let mapped = mapped_child.borrow();
    assert_eq!(mapped.size, Some(8192));
    assert_eq!(mapped.crc, Some(vec![9, 9, 9, 9]));
    assert_eq!(mapped.sha1, Some(vec![8; 20]));
    assert_eq!(mapped.file_mod_time_stamp, 777);
    assert_eq!(mapped.local_header_offset, Some(55));
    assert_eq!(mapped.header_file_type(), HeaderFileType::NES);
    assert!(mapped.header_file_type_required());
    assert!(mapped.file_status_is(FileStatus::HEADER_FILE_TYPE_FROM_HEADER));
}

#[test]
fn test_map_dat_node_to_rv_file_preserves_existing_archive_state_for_matching_node() {
    let parent = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
    let dat_rc = Rc::new(RefCell::new(RvDat::new()));

    let existing_child = Rc::new(RefCell::new(RvFile::new(FileType::Zip)));
    {
        let mut existing = existing_child.borrow_mut();
        existing.name = "game.zip".to_string();
        existing.set_dat_got_status(DatStatus::InDatCollect, dat_reader::enums::GotStatus::Got);
        existing.zip_struct = dat_reader::enums::ZipStructure::ZipTrrnt;
        existing.file_mod_time_stamp = 123456;
    }

    let mut existing_children = vec![Rc::clone(&existing_child)];

    let dat_node = DatNode::new_dir("game.zip".to_string(), FileType::Zip);

    DatUpdate::map_dat_node_to_rv_file(
        Rc::clone(&parent),
        &dat_node,
        Rc::clone(&dat_rc),
        &mut existing_children,
    );

    let mapped_child = {
        let parent_ref = parent.borrow();
        Rc::clone(&parent_ref.children[0])
    };
    let mapped = mapped_child.borrow();
    assert_eq!(mapped.got_status(), dat_reader::enums::GotStatus::Got);
    assert_eq!(mapped.rep_status(), crate::enums::RepStatus::DirCorrect);
    assert_eq!(mapped.zip_struct, dat_reader::enums::ZipStructure::ZipTrrnt);
    assert_eq!(mapped.file_mod_time_stamp, 123456);
}

#[test]
fn test_map_dat_node_to_rv_file_preserves_unmatched_physical_child_as_not_in_dat() {
    let parent = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
    let dat_rc = Rc::new(RefCell::new(RvDat::new()));

    let existing_dir = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
    existing_dir.borrow_mut().name = "game".to_string();

    let orphan = Rc::new(RefCell::new(RvFile::new(FileType::File)));
    {
        let mut orphan_mut = orphan.borrow_mut();
        orphan_mut.name = "extra.bin".to_string();
        orphan_mut.set_dat_got_status(DatStatus::InDatCollect, dat_reader::enums::GotStatus::Got);
        orphan_mut.crc = Some(vec![1, 2, 3, 4]);
        orphan_mut.parent = Some(Rc::downgrade(&existing_dir));
    }
    existing_dir.borrow_mut().child_add(Rc::clone(&orphan));

    let mut existing_children = vec![Rc::clone(&existing_dir)];

    let dat_node = DatNode::new_dir("game".to_string(), FileType::Dir);

    DatUpdate::map_dat_node_to_rv_file(
        Rc::clone(&parent),
        &dat_node,
        Rc::clone(&dat_rc),
        &mut existing_children,
    );

    let mapped_dir = {
        let parent_ref = parent.borrow();
        Rc::clone(&parent_ref.children[0])
    };
    let dir_ref = mapped_dir.borrow();
    assert_eq!(dir_ref.children.len(), 1);
    let preserved = dir_ref.children[0].borrow();
    assert_eq!(preserved.name, "extra.bin");
    assert_eq!(preserved.dat_status(), DatStatus::NotInDat);
    assert_eq!(preserved.got_status(), dat_reader::enums::GotStatus::Got);
    assert_eq!(preserved.rep_status(), crate::enums::RepStatus::Unknown);
}

#[test]
fn test_check_all_dats_marks_matching_dir_dats_using_real_paths() {
    let root = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));

    let dat_a = Rc::new(RefCell::new(RvDat::new()));
    dat_a.borrow_mut().set_data(
        DatData::DatRootFullName,
        Some("DatRoot\\Arcade\\a.dat".to_string()),
    );

    let dat_b = Rc::new(RefCell::new(RvDat::new()));
    dat_b.borrow_mut().set_data(
        DatData::DatRootFullName,
        Some("DatRoot\\Console\\b.dat".to_string()),
    );

    root.borrow_mut().dir_dats.push(Rc::clone(&dat_a));
    root.borrow_mut().dir_dats.push(Rc::clone(&dat_b));

    DatUpdate::check_all_dats(Rc::clone(&root), "DatRoot\\Arcade");

    assert_eq!(dat_a.borrow().time_stamp, i64::MAX);
    assert_ne!(dat_b.borrow().time_stamp, i64::MAX);
}

#[test]
fn test_check_all_dats_marks_matching_directory_dat_using_real_path() {
    let root = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
    let child = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));

    let dat = Rc::new(RefCell::new(RvDat::new()));
    dat.borrow_mut().set_data(
        DatData::DatRootFullName,
        Some("DatRoot\\Console\\game.dat".to_string()),
    );
    child.borrow_mut().dat = Some(Rc::clone(&dat));
    root.borrow_mut().child_add(Rc::clone(&child));

    DatUpdate::check_all_dats(Rc::clone(&root), "DatRoot\\Console");

    assert_eq!(dat.borrow().time_stamp, i64::MAX);
}

#[test]
fn test_check_all_dats_normalizes_filter_separators() {
    let root = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));

    let dat = Rc::new(RefCell::new(RvDat::new()));
    dat.borrow_mut().set_data(
        DatData::DatRootFullName,
        Some("DatRoot\\Console\\game.dat".to_string()),
    );
    root.borrow_mut().dir_dats.push(Rc::clone(&dat));

    DatUpdate::check_all_dats(Rc::clone(&root), "DatRoot/Console");

    assert_eq!(dat.borrow().time_stamp, i64::MAX);
}

#[test]
fn test_check_all_dats_matches_case_insensitively_on_windows_style_paths() {
    let root = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));

    let dat = Rc::new(RefCell::new(RvDat::new()));
    dat.borrow_mut().set_data(
        DatData::DatRootFullName,
        Some("DatRoot\\Console\\game.dat".to_string()),
    );
    root.borrow_mut().dir_dats.push(Rc::clone(&dat));

    DatUpdate::check_all_dats(Rc::clone(&root), "datroot\\console");

    assert_eq!(dat.borrow().time_stamp, i64::MAX);
}

#[test]
fn test_check_all_dats_respects_path_segment_boundaries() {
    let root = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));

    let dat = Rc::new(RefCell::new(RvDat::new()));
    dat.borrow_mut().set_data(
        DatData::DatRootFullName,
        Some("DatRoot\\Arcade\\game.dat".to_string()),
    );
    root.borrow_mut().dir_dats.push(Rc::clone(&dat));

    DatUpdate::check_all_dats(Rc::clone(&root), "DatRoot\\Arc");

    assert_ne!(dat.borrow().time_stamp, i64::MAX);
}

#[test]
fn test_scan_dat_dir_uses_path_relative_parent_without_string_slicing() {
    let temp = tempdir().unwrap();
    let dat_root = temp.path().join("DatRoot");
    let nested = dat_root.join("Arcade").join("MAME");
    fs::create_dir_all(&nested).unwrap();
    fs::write(nested.join("set.dat"), "test").unwrap();

    let mut dats_found = Vec::new();
    DatUpdate::scan_dat_dir(&dat_root.to_string_lossy(), &mut dats_found);

    assert_eq!(dats_found.len(), 1);
    assert_eq!(
        Path::new(&dats_found[0].0),
        nested.join("set.dat").as_path()
    );
    assert_eq!(dats_found[0].1.replace('/', "\\"), "Arcade\\MAME");
}

#[test]
fn test_scan_dat_dir_keeps_paths_relative_to_configured_dat_root_when_scanning_subdirectory() {
    let original_settings = get_settings();
    let temp = tempdir().unwrap();
    let dat_root = temp.path().join("DatRoot");
    let nested = dat_root.join("Arcade").join("MAME");
    fs::create_dir_all(&nested).unwrap();
    fs::write(nested.join("set.dat"), "test").unwrap();

    let settings = Settings {
        dat_root: dat_root.to_string_lossy().into_owned(),
        ..Default::default()
    };
    update_settings(settings);

    let mut dats_found = Vec::new();
    DatUpdate::scan_dat_dir(&dat_root.join("Arcade").to_string_lossy(), &mut dats_found);
    update_settings(original_settings);

    assert_eq!(dats_found.len(), 1);
    assert_eq!(
        Path::new(&dats_found[0].0),
        nested.join("set.dat").as_path()
    );
    assert_eq!(dats_found[0].1.replace('/', "\\"), "Arcade\\MAME");
}
