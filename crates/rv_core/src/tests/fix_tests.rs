use super::*;
use crate::find_fixes::FindFixes;
use crate::settings::{get_settings, set_dir_mapping, update_settings, DirMapping, Settings};
use dat_reader::enums::FileType;
use dat_reader::enums::ZipStructure;
use std::cell::RefCell;
use std::rc::Rc;
use tempfile::tempdir;

#[test]
fn test_get_physical_path() {
    let root = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
    root.borrow_mut().name = "RustyVault".to_string();

    let folder = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
    folder.borrow_mut().name = "Nintendo".to_string();
    folder.borrow_mut().parent = Some(Rc::downgrade(&root));

    let file = Rc::new(RefCell::new(RvFile::new(FileType::File)));
    file.borrow_mut().name = "game.zip".to_string();
    file.borrow_mut().parent = Some(Rc::downgrade(&folder));

    folder.borrow_mut().child_add(Rc::clone(&file));
    root.borrow_mut().child_add(Rc::clone(&folder));

    let path = Fix::get_physical_path(Rc::clone(&file));
    assert_eq!(path, "RomRoot/Nintendo/game.zip");
}

#[test]
fn test_get_physical_path_prefers_longest_dir_mapping_prefix() {
    let original_settings = get_settings();
    update_settings(Settings::default());
    set_dir_mapping(DirMapping {
        dir_key: "RustyVault\\Nintendo".to_string(),
        dir_path: r"C:\Mapped\Nintendo".to_string(),
    });

    let root = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
    root.borrow_mut().name = "RustyVault".to_string();

    let folder = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
    folder.borrow_mut().name = "Nintendo".to_string();
    folder.borrow_mut().parent = Some(Rc::downgrade(&root));

    let file = Rc::new(RefCell::new(RvFile::new(FileType::File)));
    file.borrow_mut().name = "game.zip".to_string();
    file.borrow_mut().parent = Some(Rc::downgrade(&folder));

    folder.borrow_mut().child_add(Rc::clone(&file));
    root.borrow_mut().child_add(Rc::clone(&folder));

    let path = Fix::get_physical_path(Rc::clone(&file));
    update_settings(original_settings);

    assert_eq!(
        std::path::PathBuf::from(path),
        std::path::PathBuf::from(r"C:\Mapped\Nintendo\game.zip")
    );
}

#[test]
fn test_get_tosort_path_uses_mapped_tosort_root_for_mapped_source_path() {
    let temp = tempdir().unwrap();
    let vault_path = temp.path().join("MappedVault");
    let tosort_root = temp.path().join("Sorted");
    let original_settings = get_settings();
    update_settings(Settings::default());
    set_dir_mapping(DirMapping {
        dir_key: "RustyVault".to_string(),
        dir_path: vault_path.to_string_lossy().into_owned(),
    });
    set_dir_mapping(DirMapping {
        dir_key: "ToSort".to_string(),
        dir_path: tosort_root.to_string_lossy().into_owned(),
    });

    let source_path = vault_path.join("Nintendo").join("game.zip");
    let tosort_path = Fix::get_tosort_path(&source_path.to_string_lossy(), "ToSort");
    update_settings(original_settings);

    assert_eq!(
        std::path::PathBuf::from(tosort_path),
        tosort_root.join("Nintendo").join("game.zip")
    );
}

#[test]
fn test_get_archive_member_tosort_path_uses_mapped_tosort_root_for_mapped_archive() {
    let temp = tempdir().unwrap();
    let vault_path = temp.path().join("MappedVault");
    let tosort_root = temp.path().join("Sorted");
    let original_settings = get_settings();
    update_settings(Settings::default());
    set_dir_mapping(DirMapping {
        dir_key: "RustyVault".to_string(),
        dir_path: vault_path.to_string_lossy().into_owned(),
    });
    set_dir_mapping(DirMapping {
        dir_key: "ToSort".to_string(),
        dir_path: tosort_root.to_string_lossy().into_owned(),
    });

    let archive_path = vault_path.join("Nintendo").join("game.zip");
    let tosort_path = Fix::get_archive_member_tosort_path(&archive_path, "sub/rom.bin", "ToSort");
    update_settings(original_settings);

    assert_eq!(
        tosort_path,
        tosort_root
            .join("Nintendo")
            .join("game.zip")
            .join("sub")
            .join("rom.bin")
    );
}

#[test]
fn test_get_tosort_path_uses_unmapped_logical_target_root_when_source_is_mapped() {
    let temp = tempdir().unwrap();
    let vault_path = temp.path().join("MappedVault");
    let original_settings = get_settings();
    update_settings(Settings::default());
    set_dir_mapping(DirMapping {
        dir_key: "RustyVault".to_string(),
        dir_path: vault_path.to_string_lossy().into_owned(),
    });

    let source_path = vault_path
        .join("Nintendo")
        .join("unmapped_target_root_when_source_mapped_unique.zip");
    let tosort_path = Fix::get_tosort_path(&source_path.to_string_lossy(), "UniqueToSortRoot");
    update_settings(original_settings);

    let tosort_path = std::path::PathBuf::from(tosort_path);
    assert_eq!(
        tosort_path.parent().unwrap(),
        PathBuf::from("UniqueToSortRoot").join("Nintendo").as_path()
    );
    let file_name = tosort_path.file_name().unwrap().to_string_lossy();
    assert!(file_name.starts_with("unmapped_target_root_when_source_mapped_unique"));
    assert!(file_name.ends_with(".zip"));
}

#[test]
fn test_get_archive_member_tosort_path_uses_unmapped_logical_target_root_when_source_is_mapped() {
    let temp = tempdir().unwrap();
    let vault_path = temp.path().join("MappedVault");
    let original_settings = get_settings();
    update_settings(Settings::default());
    set_dir_mapping(DirMapping {
        dir_key: "RustyVault".to_string(),
        dir_path: vault_path.to_string_lossy().into_owned(),
    });

    let archive_path = vault_path
        .join("Nintendo")
        .join("unmapped_target_root_when_source_mapped_archive_unique.zip");
    let tosort_path =
        Fix::get_archive_member_tosort_path(&archive_path, "sub/rom.bin", "UniqueToSortRoot");
    update_settings(original_settings);

    assert_eq!(
        tosort_path,
        PathBuf::from("UniqueToSortRoot")
            .join("Nintendo")
            .join("unmapped_target_root_when_source_mapped_archive_unique.zip")
            .join("sub")
            .join("rom.bin")
    );
}

#[test]
fn test_get_tosort_path_handles_case_mismatched_windows_source_root() {
    let temp = tempdir().unwrap();
    let vault_path = temp.path().join("MappedVault");
    let tosort_root = temp.path().join("Sorted");
    let original_settings = get_settings();
    update_settings(Settings::default());
    set_dir_mapping(DirMapping {
        dir_key: "RustyVault".to_string(),
        dir_path: vault_path.to_string_lossy().into_owned(),
    });
    set_dir_mapping(DirMapping {
        dir_key: "ToSort".to_string(),
        dir_path: tosort_root.to_string_lossy().into_owned(),
    });

    let source_path = vault_path
        .join("Nintendo")
        .join("game.zip")
        .to_string_lossy()
        .to_lowercase();
    let tosort_path = Fix::get_tosort_path(&source_path, "ToSort");
    update_settings(original_settings);

    assert_eq!(
        std::path::PathBuf::from(tosort_path),
        tosort_root.join("nintendo").join("game.zip")
    );
}

#[test]
fn test_get_tosort_path_avoids_duplicate_corrupt_segment_with_case_mismatched_keys() {
    let temp = tempdir().unwrap();
    let tosort_root = temp.path().join("Sorted");
    let original_settings = get_settings();
    update_settings(Settings::default());
    set_dir_mapping(DirMapping {
        dir_key: "tosort".to_string(),
        dir_path: tosort_root.to_string_lossy().into_owned(),
    });

    let source_path = tosort_root
        .join("Corrupt")
        .join("game.zip")
        .to_string_lossy()
        .to_lowercase();
    let tosort_path = Fix::get_tosort_path(&source_path, "tosort/corrupt");
    update_settings(original_settings);

    let result_path = std::path::PathBuf::from(tosort_path);
    let corrupt_count = result_path
        .components()
        .filter(|component| {
            component
                .as_os_str()
                .to_string_lossy()
                .eq_ignore_ascii_case("Corrupt")
        })
        .count();
    assert_eq!(corrupt_count, 1);
    assert_eq!(result_path, tosort_root.join("corrupt").join("game.zip"));
}

#[test]
fn test_get_tosort_path_normalizes_unmapped_base_dir_separators_and_case() {
    let source_path = PathBuf::from("ToSort")
        .join("Corrupt")
        .join("unmapped_corrupt_case_unique.zip");

    let tosort_path = Fix::get_tosort_path(&source_path.to_string_lossy(), "tosort\\corrupt");

    let result_path = PathBuf::from(tosort_path);
    let corrupt_count = result_path
        .components()
        .filter(|component| {
            component
                .as_os_str()
                .to_string_lossy()
                .eq_ignore_ascii_case("Corrupt")
        })
        .count();
    assert_eq!(corrupt_count, 1);
    assert_eq!(
        result_path.parent().unwrap(),
        PathBuf::from("ToSort").join("Corrupt").as_path()
    );
    let file_name = result_path.file_name().unwrap().to_string_lossy();
    assert!(file_name.starts_with("unmapped_corrupt_case_unique"));
    assert!(file_name.ends_with(".zip"));
}

#[test]
fn test_get_archive_member_tosort_path_normalizes_unmapped_base_dir_separators_and_case() {
    let archive_path = PathBuf::from("ToSort")
        .join("Corrupt")
        .join("unmapped_archive_corrupt_case_unique.zip");

    let tosort_path =
        Fix::get_archive_member_tosort_path(&archive_path, "sub/rom.bin", "tosort\\corrupt");

    let corrupt_count = tosort_path
        .components()
        .filter(|component| {
            component
                .as_os_str()
                .to_string_lossy()
                .eq_ignore_ascii_case("Corrupt")
        })
        .count();
    assert_eq!(corrupt_count, 1);
    assert_eq!(
        tosort_path,
        PathBuf::from("ToSort")
            .join("Corrupt")
            .join("unmapped_archive_corrupt_case_unique.zip")
            .join("sub")
            .join("rom.bin")
    );
}

#[test]
fn test_fix_file_status_changes() {
    let mut queue = Vec::new();
    let mut total_fixed = 0;

    let crc_map = HashMap::new();
    let sha1_map = HashMap::new();
    let md5_map = HashMap::new();

    // Test MoveToSort status change
    let file_to_sort = Rc::new(RefCell::new(RvFile::new(FileType::File)));
    file_to_sort.borrow_mut().name = "test.zip".to_string();
    file_to_sort
        .borrow_mut()
        .set_dat_got_status(dat_reader::enums::DatStatus::NotInDat, GotStatus::Got);
    file_to_sort
        .borrow_mut()
        .set_rep_status(RepStatus::MoveToSort);
    Fix::fix_a_file(
        Rc::clone(&file_to_sort),
        &mut queue,
        &mut total_fixed,
        &crc_map,
        &sha1_map,
        &md5_map,
    );
    assert_eq!(file_to_sort.borrow().rep_status(), RepStatus::InToSort);
    assert_eq!(
        file_to_sort.borrow().dat_status(),
        dat_reader::enums::DatStatus::InToSort
    );
    file_to_sort.borrow_mut().rep_status_reset();
    assert_eq!(file_to_sort.borrow().rep_status(), RepStatus::InToSort);

    // Test Delete status change
    let file_delete = Rc::new(RefCell::new(RvFile::new(FileType::File)));
    file_delete.borrow_mut().name = "test.zip".to_string();
    file_delete.borrow_mut().set_got_status(GotStatus::Got);
    file_delete.borrow_mut().set_rep_status(RepStatus::Delete);
    Fix::fix_a_file(
        Rc::clone(&file_delete),
        &mut queue,
        &mut total_fixed,
        &crc_map,
        &sha1_map,
        &md5_map,
    );
    assert_eq!(file_delete.borrow().rep_status(), RepStatus::Deleted);
    assert_eq!(file_delete.borrow().got_status(), GotStatus::NotGot);

    // Test CanBeFixed status change (without actual source file mapping for copy)
    let file_fix = Rc::new(RefCell::new(RvFile::new(FileType::File)));
    file_fix.borrow_mut().name = "test.zip".to_string();
    file_fix.borrow_mut().set_rep_status(RepStatus::CanBeFixed);
    Fix::fix_a_file(
        Rc::clone(&file_fix),
        &mut queue,
        &mut total_fixed,
        &crc_map,
        &sha1_map,
        &md5_map,
    );
    assert_eq!(file_fix.borrow().rep_status(), RepStatus::CanBeFixed);
}

#[test]
fn test_fix_file_move_to_corrupt_marks_file_notgot() {
    let mut queue = Vec::new();
    let mut total_fixed = 0;
    let crc_map = HashMap::new();
    let sha1_map = HashMap::new();
    let md5_map = HashMap::new();

    let file_move = Rc::new(RefCell::new(RvFile::new(FileType::File)));
    file_move.borrow_mut().name = "bad.zip".to_string();
    file_move.borrow_mut().set_got_status(GotStatus::Corrupt);
    file_move
        .borrow_mut()
        .set_rep_status(RepStatus::MoveToCorrupt);

    Fix::fix_a_file(
        Rc::clone(&file_move),
        &mut queue,
        &mut total_fixed,
        &crc_map,
        &sha1_map,
        &md5_map,
    );

    assert_eq!(file_move.borrow().rep_status(), RepStatus::Deleted);
    assert_eq!(file_move.borrow().got_status(), GotStatus::NotGot);
}

#[test]
fn test_fix_file_move_to_corrupt_preserves_merged_source_as_not_collected() {
    let mut queue = Vec::new();
    let mut total_fixed = 0;
    let crc_map = HashMap::new();
    let sha1_map = HashMap::new();
    let md5_map = HashMap::new();

    let file_move = Rc::new(RefCell::new(RvFile::new(FileType::File)));
    {
        let mut file = file_move.borrow_mut();
        file.name = "bad.zip".to_string();
        file.set_dat_got_status(
            dat_reader::enums::DatStatus::InDatMerged,
            GotStatus::Corrupt,
        );
        file.set_rep_status(RepStatus::MoveToCorrupt);
    }

    Fix::fix_a_file(
        Rc::clone(&file_move),
        &mut queue,
        &mut total_fixed,
        &crc_map,
        &sha1_map,
        &md5_map,
    );

    assert_eq!(file_move.borrow().got_status(), GotStatus::NotGot);
    assert_eq!(file_move.borrow().rep_status(), RepStatus::NotCollected);
}

#[test]
fn test_fix_archive_move_to_sort_updates_dat_status_and_survives_reset() {
    let temp = tempdir().unwrap();
    let root = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
    root.borrow_mut().name = temp.path().to_string_lossy().to_string();

    let archive = Rc::new(RefCell::new(RvFile::new(FileType::Zip)));
    {
        let mut a = archive.borrow_mut();
        a.name = "sortme.zip".to_string();
        a.tree_checked = TreeSelect::Selected;
        a.set_dat_got_status(dat_reader::enums::DatStatus::NotInDat, GotStatus::Got);
        a.set_rep_status(RepStatus::MoveToSort);
        a.parent = Some(Rc::downgrade(&root));
    }
    root.borrow_mut().child_add(Rc::clone(&archive));

    Fix::fix_archive_node(Rc::clone(&archive));

    assert_eq!(archive.borrow().rep_status(), RepStatus::InToSort);
    assert_eq!(
        archive.borrow().dat_status(),
        dat_reader::enums::DatStatus::InToSort
    );
    archive.borrow_mut().rep_status_reset();
    assert_eq!(archive.borrow().rep_status(), RepStatus::InToSort);
}

#[test]
fn test_fix_file_process_queue() {
    let root = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
    let mut queue = Vec::new();
    let mut total_fixed = 0;

    // Setup source file
    let src_file = Rc::new(RefCell::new(RvFile::new(FileType::File)));
    {
        let mut f = src_file.borrow_mut();
        f.name = "source.zip".to_string();
        f.size = Some(1024);
        f.crc = Some(vec![0xAA, 0xBB, 0xCC, 0xDD]);
        f.set_dat_got_status(dat_reader::enums::DatStatus::NotInDat, GotStatus::Got);
        f.set_rep_status(RepStatus::NeededForFix);
    }

    // Setup maps with source file
    let mut crc_map = HashMap::new();
    crc_map.insert((1024, vec![0xAA, 0xBB, 0xCC, 0xDD]), Rc::clone(&src_file));
    let sha1_map = HashMap::new();
    let md5_map = HashMap::new();

    // Setup destination file that needs fix
    let dst_file = Rc::new(RefCell::new(RvFile::new(FileType::File)));
    {
        let mut f = dst_file.borrow_mut();
        f.name = "dest.zip".to_string();
        f.size = Some(1024);
        f.crc = Some(vec![0xAA, 0xBB, 0xCC, 0xDD]);
        f.set_dat_got_status(
            dat_reader::enums::DatStatus::InDatCollect,
            GotStatus::NotGot,
        );
        f.set_rep_status(RepStatus::CanBeFixed);
    }

    // Add to tree so get_physical_path doesn't panic
    root.borrow_mut().child_add(Rc::clone(&src_file));
    root.borrow_mut().child_add(Rc::clone(&dst_file));

    // Trigger fix on the destination file
    Fix::fix_a_file(
        Rc::clone(&dst_file),
        &mut queue,
        &mut total_fixed,
        &crc_map,
        &sha1_map,
        &md5_map,
    );

    // 1. Destination file should be marked Correct
    assert_eq!(dst_file.borrow().rep_status(), RepStatus::Correct);
    // 2. Total fixed should be incremented
    assert_eq!(total_fixed, 1);
    // 3. Source file should be queued for deletion in the next tick
    assert_eq!(queue.len(), 1);
    assert_eq!(src_file.borrow().rep_status(), RepStatus::Delete);
    // Ensure the queued item is actually the source file
    assert_eq!(Rc::as_ptr(&queue[0]), Rc::as_ptr(&src_file));
}

#[test]
fn test_fix_file_process_queue_matches_alt_sha1_source() {
    let root = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
    let mut queue = Vec::new();
    let mut total_fixed = 0;

    let src_file = Rc::new(RefCell::new(RvFile::new(FileType::File)));
    {
        let mut f = src_file.borrow_mut();
        f.name = "source.zip".to_string();
        f.size = Some(1024);
        f.sha1 = Some(vec![0x11, 0x22, 0x33, 0x44]);
        f.set_dat_got_status(dat_reader::enums::DatStatus::NotInDat, GotStatus::Got);
        f.set_rep_status(RepStatus::NeededForFix);
    }

    let crc_map = HashMap::new();
    let mut sha1_map = HashMap::new();
    sha1_map.insert((1024, vec![0x11, 0x22, 0x33, 0x44]), Rc::clone(&src_file));
    let md5_map = HashMap::new();

    let dst_file = Rc::new(RefCell::new(RvFile::new(FileType::File)));
    {
        let mut f = dst_file.borrow_mut();
        f.name = "dest.zip".to_string();
        f.alt_size = Some(1024);
        f.alt_sha1 = Some(vec![0x11, 0x22, 0x33, 0x44]);
        f.set_dat_got_status(
            dat_reader::enums::DatStatus::InDatCollect,
            GotStatus::NotGot,
        );
        f.set_rep_status(RepStatus::CanBeFixed);
    }

    root.borrow_mut().child_add(Rc::clone(&src_file));
    root.borrow_mut().child_add(Rc::clone(&dst_file));

    Fix::fix_a_file(
        Rc::clone(&dst_file),
        &mut queue,
        &mut total_fixed,
        &crc_map,
        &sha1_map,
        &md5_map,
    );

    assert_eq!(dst_file.borrow().rep_status(), RepStatus::Correct);
    assert_eq!(total_fixed, 1);
    assert_eq!(queue.len(), 1);
    assert_eq!(src_file.borrow().rep_status(), RepStatus::Delete);
}

#[test]
fn test_find_source_file_matches_primary_sha1_using_alt_size() {
    let source_file = Rc::new(RefCell::new(RvFile::new(FileType::File)));
    {
        let mut f = source_file.borrow_mut();
        f.name = "source.bin".to_string();
        f.size = Some(1024);
        f.sha1 = Some(vec![0x12, 0x34, 0x56, 0x78]);
    }

    let target_file = Rc::new(RefCell::new(RvFile::new(FileType::File)));
    {
        let mut f = target_file.borrow_mut();
        f.name = "target.bin".to_string();
        f.alt_size = Some(1024);
        f.sha1 = Some(vec![0x12, 0x34, 0x56, 0x78]);
    }

    let crc_map = HashMap::new();
    let mut sha1_map = HashMap::new();
    sha1_map.insert(
        (1024, vec![0x12, 0x34, 0x56, 0x78]),
        Rc::clone(&source_file),
    );
    let md5_map = HashMap::new();

    let found = Fix::find_source_file(&target_file.borrow(), &crc_map, &sha1_map, &md5_map);
    assert!(found.is_some());
    assert_eq!(Rc::as_ptr(&found.unwrap()), Rc::as_ptr(&source_file));
}

#[test]
fn test_fix_file_process_queue_preserves_merged_source_as_not_collected_after_delete() {
    let root = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
    let mut queue = Vec::new();
    let mut total_fixed = 0;

    let src_file = Rc::new(RefCell::new(RvFile::new(FileType::File)));
    {
        let mut f = src_file.borrow_mut();
        f.name = "merged_source.zip".to_string();
        f.size = Some(1024);
        f.crc = Some(vec![0x0A, 0x1B, 0x2C, 0x3D]);
        f.set_dat_got_status(dat_reader::enums::DatStatus::InDatMerged, GotStatus::Got);
        f.set_rep_status(RepStatus::NeededForFix);
    }

    let mut crc_map = HashMap::new();
    crc_map.insert((1024, vec![0x0A, 0x1B, 0x2C, 0x3D]), Rc::clone(&src_file));
    let sha1_map = HashMap::new();
    let md5_map = HashMap::new();

    let dst_file = Rc::new(RefCell::new(RvFile::new(FileType::File)));
    {
        let mut f = dst_file.borrow_mut();
        f.name = "dest.zip".to_string();
        f.size = Some(1024);
        f.crc = Some(vec![0x0A, 0x1B, 0x2C, 0x3D]);
        f.set_dat_got_status(
            dat_reader::enums::DatStatus::InDatCollect,
            GotStatus::NotGot,
        );
        f.set_rep_status(RepStatus::CanBeFixed);
    }

    root.borrow_mut().child_add(Rc::clone(&src_file));
    root.borrow_mut().child_add(Rc::clone(&dst_file));

    Fix::fix_a_file(
        Rc::clone(&dst_file),
        &mut queue,
        &mut total_fixed,
        &crc_map,
        &sha1_map,
        &md5_map,
    );
    assert_eq!(queue.len(), 1);

    let queued_source = queue.remove(0);
    Fix::fix_a_file(
        queued_source,
        &mut queue,
        &mut total_fixed,
        &crc_map,
        &sha1_map,
        &md5_map,
    );

    assert_eq!(src_file.borrow().got_status(), GotStatus::NotGot);
    assert_eq!(src_file.borrow().rep_status(), RepStatus::NotCollected);
}

#[test]
fn test_fix_file_process_queue_preserves_mia_and_sets_got_status() {
    let root = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
    let mut queue = Vec::new();
    let mut total_fixed = 0;

    let src_file = Rc::new(RefCell::new(RvFile::new(FileType::File)));
    {
        let mut f = src_file.borrow_mut();
        f.name = "source_mia.zip".to_string();
        f.size = Some(1024);
        f.crc = Some(vec![0xAB, 0xBC, 0xCD, 0xDE]);
        f.set_rep_status(RepStatus::NeededForFix);
    }

    let mut crc_map = HashMap::new();
    crc_map.insert((1024, vec![0xAB, 0xBC, 0xCD, 0xDE]), Rc::clone(&src_file));
    let sha1_map = HashMap::new();
    let md5_map = HashMap::new();

    let dst_file = Rc::new(RefCell::new(RvFile::new(FileType::File)));
    {
        let mut f = dst_file.borrow_mut();
        f.name = "dest_mia.zip".to_string();
        f.size = Some(1024);
        f.crc = Some(vec![0xAB, 0xBC, 0xCD, 0xDE]);
        f.set_dat_got_status(dat_reader::enums::DatStatus::InDatMIA, GotStatus::NotGot);
        f.set_rep_status(RepStatus::CanBeFixedMIA);
    }

    root.borrow_mut().child_add(Rc::clone(&src_file));
    root.borrow_mut().child_add(Rc::clone(&dst_file));

    Fix::fix_a_file(
        Rc::clone(&dst_file),
        &mut queue,
        &mut total_fixed,
        &crc_map,
        &sha1_map,
        &md5_map,
    );

    assert_eq!(dst_file.borrow().rep_status(), RepStatus::CorrectMIA);
    assert_eq!(dst_file.borrow().got_status(), GotStatus::Got);
    dst_file.borrow_mut().rep_status_reset();
    assert_eq!(dst_file.borrow().rep_status(), RepStatus::CorrectMIA);
    assert_eq!(total_fixed, 1);
    assert_eq!(queue.len(), 1);
}

#[test]
fn test_fix_respects_tree_selection_state() {
    let root = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));

    let selected_dir = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
    {
        let mut dir = selected_dir.borrow_mut();
        dir.name = "Selected".to_string();
        dir.tree_checked = TreeSelect::Selected;
        dir.parent = Some(Rc::downgrade(&root));
    }

    let selected_file = Rc::new(RefCell::new(RvFile::new(FileType::File)));
    {
        let mut file = selected_file.borrow_mut();
        file.name = "selected.zip".to_string();
        file.tree_checked = TreeSelect::Selected;
        file.set_dat_got_status(dat_reader::enums::DatStatus::InDatCollect, GotStatus::Got);
        file.set_rep_status(RepStatus::Rename);
        file.parent = Some(Rc::downgrade(&selected_dir));
    }

    selected_dir
        .borrow_mut()
        .child_add(Rc::clone(&selected_file));
    root.borrow_mut().child_add(Rc::clone(&selected_dir));

    let unselected_dir = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
    {
        let mut dir = unselected_dir.borrow_mut();
        dir.name = "UnSelected".to_string();
        dir.tree_checked = TreeSelect::UnSelected;
        dir.parent = Some(Rc::downgrade(&root));
    }

    let unselected_file = Rc::new(RefCell::new(RvFile::new(FileType::File)));
    {
        let mut file = unselected_file.borrow_mut();
        file.name = "unselected.zip".to_string();
        file.tree_checked = TreeSelect::Selected;
        file.set_dat_got_status(dat_reader::enums::DatStatus::InDatCollect, GotStatus::Got);
        file.set_rep_status(RepStatus::Rename);
        file.parent = Some(Rc::downgrade(&unselected_dir));
    }

    let skipped_file = Rc::new(RefCell::new(RvFile::new(FileType::File)));
    {
        let mut file = skipped_file.borrow_mut();
        file.name = "skipped.zip".to_string();
        file.tree_checked = TreeSelect::UnSelected;
        file.set_rep_status(RepStatus::Rename);
        file.parent = Some(Rc::downgrade(&unselected_dir));
    }

    unselected_dir
        .borrow_mut()
        .child_add(Rc::clone(&unselected_file));
    unselected_dir
        .borrow_mut()
        .child_add(Rc::clone(&skipped_file));
    root.borrow_mut().child_add(Rc::clone(&unselected_dir));

    Fix::perform_fixes(Rc::clone(&root));

    assert_eq!(selected_file.borrow().rep_status(), RepStatus::Correct);
    assert_eq!(unselected_file.borrow().rep_status(), RepStatus::Correct);
    assert_eq!(skipped_file.borrow().rep_status(), RepStatus::Rename);
}

#[test]
fn test_fix_processes_selected_archive_members_inside_unselected_archive() {
    let temp = tempdir().unwrap();
    let root = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
    root.borrow_mut().name = temp.path().to_string_lossy().to_string();

    let archive = Rc::new(RefCell::new(RvFile::new(FileType::Zip)));
    {
        let mut a = archive.borrow_mut();
        a.name = "game.zip".to_string();
        a.tree_checked = TreeSelect::UnSelected;
        a.zip_struct = ZipStructure::ZipTDC;
        a.parent = Some(Rc::downgrade(&root));
    }

    let child = Rc::new(RefCell::new(RvFile::new(FileType::File)));
    {
        let mut f = child.borrow_mut();
        f.name = "new.bin".to_string();
        f.file_name = "old.bin".to_string();
        f.size = Some(4);
        f.tree_checked = TreeSelect::Selected;
        f.set_dat_got_status(dat_reader::enums::DatStatus::InDatCollect, GotStatus::Got);
        f.set_rep_status(RepStatus::Rename);
        f.parent = Some(Rc::downgrade(&archive));
    }
    archive.borrow_mut().child_add(Rc::clone(&child));
    root.borrow_mut().child_add(Rc::clone(&archive));

    let archive_path = temp.path().join("game.zip");
    {
        let file = File::create(&archive_path).unwrap();
        let mut writer = ZipWriter::new(file);
        writer
            .start_file("old.bin", SimpleFileOptions::default())
            .unwrap();
        writer.write_all(b"data").unwrap();
        writer.finish().unwrap();
    }

    Fix::perform_fixes(Rc::clone(&root));

    let file = File::open(&archive_path).unwrap();
    let mut zip = ZipArchive::new(file).unwrap();
    assert!(zip.by_name("new.bin").is_ok());
    assert!(zip.by_name("old.bin").is_err());
    assert_eq!(child.borrow().rep_status(), RepStatus::Correct);
}

#[test]
fn test_fix_perform_fixes_deletes_redundant_tosort_duplicate_after_find_fixes() {
    let temp = tempdir().unwrap();
    let root = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
    root.borrow_mut().name = temp.path().to_string_lossy().to_string();

    let vault_dir = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
    {
        let mut dir = vault_dir.borrow_mut();
        dir.name = "RomRoot".to_string();
        dir.tree_checked = TreeSelect::Selected;
        dir.parent = Some(Rc::downgrade(&root));
    }

    let correct_file = Rc::new(RefCell::new(RvFile::new(FileType::File)));
    {
        let mut file = correct_file.borrow_mut();
        file.name = "owned.bin".to_string();
        file.size = Some(4);
        file.crc = Some(vec![0xAD, 0xF3, 0xF3, 0x63]);
        file.tree_checked = TreeSelect::Selected;
        file.set_dat_got_status(dat_reader::enums::DatStatus::InDatCollect, GotStatus::Got);
        file.parent = Some(Rc::downgrade(&vault_dir));
    }
    vault_dir.borrow_mut().child_add(Rc::clone(&correct_file));

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
        file.name = "spare.bin".to_string();
        file.size = Some(4);
        file.crc = Some(vec![0xAD, 0xF3, 0xF3, 0x63]);
        file.tree_checked = TreeSelect::Selected;
        file.set_dat_got_status(dat_reader::enums::DatStatus::InToSort, GotStatus::Got);
        file.parent = Some(Rc::downgrade(&tosort_dir));
    }
    tosort_dir.borrow_mut().child_add(Rc::clone(&tosort_file));

    root.borrow_mut().child_add(Rc::clone(&vault_dir));
    root.borrow_mut().child_add(Rc::clone(&tosort_dir));

    let vault_path = temp.path().join("RomRoot");
    let tosort_path = temp.path().join("ToSort");
    fs::create_dir_all(&vault_path).unwrap();
    fs::create_dir_all(&tosort_path).unwrap();
    let correct_path = vault_path.join("owned.bin");
    let spare_path = tosort_path.join("spare.bin");
    fs::write(&correct_path, b"data").unwrap();
    fs::write(&spare_path, b"data").unwrap();

    FindFixes::scan_files(Rc::clone(&root));
    assert_eq!(correct_file.borrow().rep_status(), RepStatus::Correct);
    assert_eq!(tosort_file.borrow().rep_status(), RepStatus::Delete);

    Fix::perform_fixes(Rc::clone(&root));

    assert!(correct_path.exists());
    assert!(!spare_path.exists());
    assert_eq!(correct_file.borrow().rep_status(), RepStatus::Correct);
    assert_eq!(tosort_file.borrow().rep_status(), RepStatus::Deleted);
    assert_eq!(tosort_file.borrow().got_status(), GotStatus::NotGot);
}

#[test]
fn test_fix_perform_fixes_deletes_unneeded_loose_file() {
    let temp = tempdir().unwrap();
    let root = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
    root.borrow_mut().name = temp.path().to_string_lossy().to_string();

    let unneeded_file = Rc::new(RefCell::new(RvFile::new(FileType::File)));
    {
        let mut file = unneeded_file.borrow_mut();
        file.name = "merged.bin".to_string();
        file.tree_checked = TreeSelect::Selected;
        file.set_dat_got_status(dat_reader::enums::DatStatus::InDatMerged, GotStatus::Got);
        file.set_rep_status(RepStatus::UnNeeded);
        file.parent = Some(Rc::downgrade(&root));
    }
    root.borrow_mut().child_add(Rc::clone(&unneeded_file));

    let file_path = temp.path().join("merged.bin");
    fs::write(&file_path, b"data").unwrap();

    Fix::perform_fixes(Rc::clone(&root));

    assert!(!file_path.exists());
    assert_eq!(unneeded_file.borrow().got_status(), GotStatus::NotGot);
    assert_eq!(unneeded_file.borrow().rep_status(), RepStatus::NotCollected);
}

#[test]
fn test_fix_perform_fixes_deletes_unneeded_archive() {
    let temp = tempdir().unwrap();
    let root = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
    root.borrow_mut().name = temp.path().to_string_lossy().to_string();

    let unneeded_archive = Rc::new(RefCell::new(RvFile::new(FileType::Zip)));
    {
        let mut archive = unneeded_archive.borrow_mut();
        archive.name = "merged.zip".to_string();
        archive.tree_checked = TreeSelect::Selected;
        archive.set_dat_got_status(dat_reader::enums::DatStatus::InDatMerged, GotStatus::Got);
        archive.set_rep_status(RepStatus::UnNeeded);
        archive.parent = Some(Rc::downgrade(&root));
    }
    root.borrow_mut().child_add(Rc::clone(&unneeded_archive));

    let archive_path = temp.path().join("merged.zip");
    fs::write(&archive_path, b"zip").unwrap();

    Fix::perform_fixes(Rc::clone(&root));

    assert!(!archive_path.exists());
    assert_eq!(unneeded_archive.borrow().got_status(), GotStatus::NotGot);
    assert_eq!(
        unneeded_archive.borrow().rep_status(),
        RepStatus::NotCollected
    );
}

#[test]
fn test_fix_archive_rebuild_removes_unneeded_zip_member() {
    let temp = tempdir().unwrap();
    let root = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
    root.borrow_mut().name = temp.path().to_string_lossy().to_string();

    let target_archive = Rc::new(RefCell::new(RvFile::new(FileType::Zip)));
    {
        let mut archive = target_archive.borrow_mut();
        archive.name = "target.zip".to_string();
        archive.tree_checked = TreeSelect::Selected;
        archive.set_dat_status(dat_reader::enums::DatStatus::InDatCollect);
        archive.set_zip_dat_struct(ZipStructure::ZipTrrnt, true);
        archive.parent = Some(Rc::downgrade(&root));
    }

    let keep_child = Rc::new(RefCell::new(RvFile::new(FileType::File)));
    {
        let mut file = keep_child.borrow_mut();
        file.name = "keep.bin".to_string();
        file.file_name = "keep.bin".to_string();
        file.size = Some(4);
        file.tree_checked = TreeSelect::Selected;
        file.set_dat_got_status(dat_reader::enums::DatStatus::InDatCollect, GotStatus::Got);
        file.set_rep_status(RepStatus::Correct);
        file.parent = Some(Rc::downgrade(&target_archive));
    }

    let unneeded_child = Rc::new(RefCell::new(RvFile::new(FileType::File)));
    {
        let mut file = unneeded_child.borrow_mut();
        file.name = "drop.bin".to_string();
        file.file_name = "drop.bin".to_string();
        file.size = Some(4);
        file.tree_checked = TreeSelect::Selected;
        file.set_dat_got_status(dat_reader::enums::DatStatus::InDatMerged, GotStatus::Got);
        file.set_rep_status(RepStatus::UnNeeded);
        file.parent = Some(Rc::downgrade(&target_archive));
    }

    target_archive
        .borrow_mut()
        .child_add(Rc::clone(&keep_child));
    target_archive
        .borrow_mut()
        .child_add(Rc::clone(&unneeded_child));
    root.borrow_mut().child_add(Rc::clone(&target_archive));

    let target_path = temp.path().join("target.zip");
    {
        let file = File::create(&target_path).unwrap();
        let mut writer = ZipWriter::new(file);
        let options = SimpleFileOptions::default();
        writer.start_file("keep.bin", options).unwrap();
        writer.write_all(b"keep").unwrap();
        writer.start_file("drop.bin", options).unwrap();
        writer.write_all(b"drop").unwrap();
        writer.finish().unwrap();
    }

    Fix::perform_fixes(Rc::clone(&root));

    assert_eq!(
        Fix::read_zip_entry_bytes(&target_path.to_string_lossy(), "keep.bin").unwrap(),
        b"keep"
    );
    assert!(Fix::read_zip_entry_bytes(&target_path.to_string_lossy(), "drop.bin").is_none());
    assert_eq!(unneeded_child.borrow().got_status(), GotStatus::NotGot);
    assert_eq!(
        unneeded_child.borrow().rep_status(),
        RepStatus::NotCollected
    );
}

#[test]
fn test_fix_archive_rebuild_removes_unneeded_sevenzip_member() {
    let temp = tempdir().unwrap();
    let root = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
    root.borrow_mut().name = temp.path().to_string_lossy().to_string();

    let target_archive = Rc::new(RefCell::new(RvFile::new(FileType::SevenZip)));
    {
        let mut archive = target_archive.borrow_mut();
        archive.name = "target.7z".to_string();
        archive.tree_checked = TreeSelect::Selected;
        archive.set_dat_status(dat_reader::enums::DatStatus::InDatCollect);
        archive.set_zip_dat_struct(ZipStructure::SevenZipSLZMA, true);
        archive.parent = Some(Rc::downgrade(&root));
    }

    let keep_child = Rc::new(RefCell::new(RvFile::new(FileType::File)));
    {
        let mut file = keep_child.borrow_mut();
        file.name = "keep.bin".to_string();
        file.file_name = "keep.bin".to_string();
        file.size = Some(4);
        file.tree_checked = TreeSelect::Selected;
        file.set_dat_got_status(dat_reader::enums::DatStatus::InDatCollect, GotStatus::Got);
        file.set_rep_status(RepStatus::Correct);
        file.parent = Some(Rc::downgrade(&target_archive));
    }

    let unneeded_child = Rc::new(RefCell::new(RvFile::new(FileType::File)));
    {
        let mut file = unneeded_child.borrow_mut();
        file.name = "drop.bin".to_string();
        file.file_name = "drop.bin".to_string();
        file.size = Some(4);
        file.tree_checked = TreeSelect::Selected;
        file.set_dat_got_status(dat_reader::enums::DatStatus::InDatMerged, GotStatus::Got);
        file.set_rep_status(RepStatus::UnNeeded);
        file.parent = Some(Rc::downgrade(&target_archive));
    }

    target_archive
        .borrow_mut()
        .child_add(Rc::clone(&keep_child));
    target_archive
        .borrow_mut()
        .child_add(Rc::clone(&unneeded_child));
    root.borrow_mut().child_add(Rc::clone(&target_archive));

    let stage_dir = temp.path().join("stage_7z_unneeded");
    fs::create_dir_all(&stage_dir).unwrap();
    fs::write(stage_dir.join("keep.bin"), b"keep").unwrap();
    fs::write(stage_dir.join("drop.bin"), b"drop").unwrap();
    let target_path = temp.path().join("target.7z");
    sevenz_rust::compress_to_path(&stage_dir, &target_path).unwrap();

    Fix::perform_fixes(Rc::clone(&root));

    assert_eq!(
        Fix::read_seven_zip_entry_bytes(&target_path.to_string_lossy(), "keep.bin").unwrap(),
        b"keep"
    );
    assert!(Fix::read_seven_zip_entry_bytes(&target_path.to_string_lossy(), "drop.bin").is_none());
    assert_eq!(unneeded_child.borrow().got_status(), GotStatus::NotGot);
    assert_eq!(
        unneeded_child.borrow().rep_status(),
        RepStatus::NotCollected
    );
}

#[test]
fn test_fix_shared_destination_keeps_physical_file_for_other_dat_branch() {
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
    let keep_file = Rc::new(RefCell::new(RvFile::new(FileType::File)));
    {
        let mut file = keep_file.borrow_mut();
        file.name = "shared.bin".to_string();
        file.tree_checked = TreeSelect::Selected;
        file.set_dat_got_status(dat_reader::enums::DatStatus::InDatCollect, GotStatus::Got);
        file.set_rep_status(RepStatus::Correct);
        file.parent = Some(Rc::downgrade(&dat_a));
    }
    dat_a.borrow_mut().child_add(Rc::clone(&keep_file));

    let dat_b = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
    {
        let mut dir = dat_b.borrow_mut();
        dir.name = "DatB".to_string();
        dir.tree_checked = TreeSelect::Selected;
        dir.parent = Some(Rc::downgrade(&root));
    }
    let duplicate_view = Rc::new(RefCell::new(RvFile::new(FileType::File)));
    {
        let mut file = duplicate_view.borrow_mut();
        file.name = "shared.bin".to_string();
        file.tree_checked = TreeSelect::Selected;
        file.set_dat_got_status(dat_reader::enums::DatStatus::InDatMerged, GotStatus::Got);
        file.set_rep_status(RepStatus::UnNeeded);
        file.parent = Some(Rc::downgrade(&dat_b));
    }
    dat_b.borrow_mut().child_add(Rc::clone(&duplicate_view));

    root.borrow_mut().child_add(Rc::clone(&dat_a));
    root.borrow_mut().child_add(Rc::clone(&dat_b));

    let file_path = shared_path.join("shared.bin");
    fs::write(&file_path, b"data").unwrap();

    Fix::perform_fixes(Rc::clone(&root));

    assert!(file_path.exists());
    assert_eq!(keep_file.borrow().got_status(), GotStatus::Got);
    assert_eq!(keep_file.borrow().rep_status(), RepStatus::Correct);
    assert_eq!(duplicate_view.borrow().got_status(), GotStatus::NotGot);
    assert_eq!(
        duplicate_view.borrow().rep_status(),
        RepStatus::NotCollected
    );

    update_settings(original_settings);
}

#[test]
fn test_fix_perform_fixes_deletes_redundant_tosort_duplicate_when_romroot_archive_member_exists() {
    let temp = tempdir().unwrap();
    let root = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
    root.borrow_mut().name = temp.path().to_string_lossy().to_string();

    let vault_dir = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
    {
        let mut dir = vault_dir.borrow_mut();
        dir.name = "RomRoot".to_string();
        dir.tree_checked = TreeSelect::Selected;
        dir.parent = Some(Rc::downgrade(&root));
    }

    let archive = Rc::new(RefCell::new(RvFile::new(FileType::Zip)));
    {
        let mut a = archive.borrow_mut();
        a.name = "game.zip".to_string();
        a.tree_checked = TreeSelect::Selected;
        a.set_dat_got_status(dat_reader::enums::DatStatus::InDatCollect, GotStatus::Got);
        a.parent = Some(Rc::downgrade(&vault_dir));
    }

    let archive_member = Rc::new(RefCell::new(RvFile::new(FileType::FileZip)));
    {
        let mut f = archive_member.borrow_mut();
        f.name = "game.a78".to_string();
        f.size = Some(131200);
        f.crc = Some(vec![0x10, 0x20, 0x30, 0x40]);
        f.tree_checked = TreeSelect::Selected;
        f.set_dat_got_status(dat_reader::enums::DatStatus::InDatCollect, GotStatus::Got);
        f.parent = Some(Rc::downgrade(&archive));
    }
    archive.borrow_mut().child_add(Rc::clone(&archive_member));
    vault_dir.borrow_mut().child_add(Rc::clone(&archive));
    root.borrow_mut().child_add(Rc::clone(&vault_dir));

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
        file.name = "game.a78".to_string();
        file.size = Some(131200);
        file.crc = Some(vec![0x10, 0x20, 0x30, 0x40]);
        file.tree_checked = TreeSelect::Selected;
        file.set_dat_got_status(dat_reader::enums::DatStatus::InToSort, GotStatus::Got);
        file.parent = Some(Rc::downgrade(&tosort_dir));
    }
    tosort_dir.borrow_mut().child_add(Rc::clone(&tosort_file));
    root.borrow_mut().child_add(Rc::clone(&tosort_dir));

    let romroot_path = temp.path().join("RomRoot");
    let tosort_path = temp.path().join("ToSort");
    fs::create_dir_all(&romroot_path).unwrap();
    fs::create_dir_all(&tosort_path).unwrap();
    fs::write(romroot_path.join("game.zip"), b"zip-placeholder").unwrap();
    let spare_path = tosort_path.join("game.a78");
    fs::write(&spare_path, b"data").unwrap();

    FindFixes::scan_files(Rc::clone(&root));
    assert_eq!(archive_member.borrow().rep_status(), RepStatus::Correct);
    assert_eq!(tosort_file.borrow().rep_status(), RepStatus::Delete);

    Fix::perform_fixes(Rc::clone(&root));

    assert!(!spare_path.exists());
    assert_eq!(tosort_file.borrow().rep_status(), RepStatus::Deleted);
    assert_eq!(tosort_file.borrow().got_status(), GotStatus::NotGot);
}

#[test]
fn test_fix_single_pass_deletes_redundant_tosort_duplicate_when_other_source_fixes_target() {
    let temp = tempdir().unwrap();
    let root = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
    root.borrow_mut().name = temp.path().to_string_lossy().to_string();

    let target_archive = Rc::new(RefCell::new(RvFile::new(FileType::Zip)));
    {
        let mut archive = target_archive.borrow_mut();
        archive.name = "game.zip".to_string();
        archive.tree_checked = TreeSelect::Selected;
        archive.set_dat_got_status(dat_reader::enums::DatStatus::InDatCollect, GotStatus::Got);
        archive.parent = Some(Rc::downgrade(&root));
    }

    let missing_child = Rc::new(RefCell::new(RvFile::new(FileType::File)));
    {
        let mut file = missing_child.borrow_mut();
        file.name = "game.a78".to_string();
        file.size = Some(4);
        file.crc = Some(vec![0xDE, 0xAD, 0xBE, 0xEF]);
        file.tree_checked = TreeSelect::Selected;
        file.set_dat_got_status(
            dat_reader::enums::DatStatus::InDatCollect,
            GotStatus::NotGot,
        );
        file.set_rep_status(RepStatus::Missing);
        file.parent = Some(Rc::downgrade(&target_archive));
    }
    target_archive
        .borrow_mut()
        .child_add(Rc::clone(&missing_child));
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
        file.set_dat_got_status(dat_reader::enums::DatStatus::InToSort, GotStatus::Got);
        file.parent = Some(Rc::downgrade(&tosort_dir));
    }

    let duplicate_source = Rc::new(RefCell::new(RvFile::new(FileType::File)));
    {
        let mut file = duplicate_source.borrow_mut();
        file.name = "game_copy.a78".to_string();
        file.size = Some(4);
        file.crc = Some(vec![0xDE, 0xAD, 0xBE, 0xEF]);
        file.tree_checked = TreeSelect::Selected;
        file.set_dat_got_status(dat_reader::enums::DatStatus::InToSort, GotStatus::Got);
        file.parent = Some(Rc::downgrade(&tosort_dir));
    }

    tosort_dir
        .borrow_mut()
        .child_add(Rc::clone(&primary_source));
    tosort_dir
        .borrow_mut()
        .child_add(Rc::clone(&duplicate_source));
    root.borrow_mut().child_add(Rc::clone(&tosort_dir));

    let target_path = temp.path().join("game.zip");
    {
        let file = File::create(&target_path).unwrap();
        let writer = ZipWriter::new(file);
        writer.finish().unwrap();
    }
    let tosort_path = temp.path().join("ToSort");
    fs::create_dir_all(&tosort_path).unwrap();
    let primary_source_path = tosort_path.join("game.a78");
    let duplicate_source_path = tosort_path.join("game_copy.a78");
    fs::write(&primary_source_path, b"data").unwrap();
    fs::write(&duplicate_source_path, b"data").unwrap();

    FindFixes::scan_files(Rc::clone(&root));
    let source_statuses = [
        primary_source.borrow().rep_status(),
        duplicate_source.borrow().rep_status(),
    ];
    assert!(source_statuses.contains(&RepStatus::NeededForFix));
    assert!(source_statuses.contains(&RepStatus::Delete));

    Fix::perform_fixes(Rc::clone(&root));

    assert!(target_path.exists());
    assert!(
        primary_source.borrow().got_status() == GotStatus::NotGot
            || duplicate_source.borrow().got_status() == GotStatus::NotGot
    );
    assert!(!primary_source_path.exists());
    assert!(!duplicate_source_path.exists());
    assert_eq!(primary_source.borrow().got_status(), GotStatus::NotGot);
    assert_eq!(duplicate_source.borrow().got_status(), GotStatus::NotGot);
}

#[test]
fn test_fix_deletes_tosort_duplicate_when_matching_romroot_file_is_unselected() {
    let temp = tempdir().unwrap();
    let root = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
    root.borrow_mut().name = temp.path().to_string_lossy().to_string();

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
        file.set_dat_got_status(dat_reader::enums::DatStatus::InDatCollect, GotStatus::Got);
        file.set_rep_status(RepStatus::Correct);
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
        file.set_dat_got_status(dat_reader::enums::DatStatus::InToSort, GotStatus::Got);
        file.parent = Some(Rc::downgrade(&tosort_dir));
    }
    tosort_dir.borrow_mut().child_add(Rc::clone(&tosort_file));
    root.borrow_mut().child_add(Rc::clone(&tosort_dir));

    fs::create_dir_all(temp.path().join("RomRoot")).unwrap();
    fs::create_dir_all(temp.path().join("ToSort")).unwrap();
    fs::write(temp.path().join("RomRoot").join("game.a78"), b"data").unwrap();
    let tosort_path = temp.path().join("ToSort").join("game_copy.a78");
    fs::write(&tosort_path, b"data").unwrap();

    FindFixes::scan_files(Rc::clone(&root));
    assert_eq!(tosort_file.borrow().rep_status(), RepStatus::Delete);

    Fix::perform_fixes(Rc::clone(&root));

    assert!(temp.path().join("RomRoot").join("game.a78").exists());
    assert!(!tosort_path.exists());
    assert_eq!(romroot_file.borrow().got_status(), GotStatus::Got);
    assert_eq!(tosort_file.borrow().got_status(), GotStatus::NotGot);
}

#[test]
fn test_fix_uses_locked_source_without_deleting_it() {
    let root = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
    let mut queue = Vec::new();
    let mut total_fixed = 0;

    let src_file = Rc::new(RefCell::new(RvFile::new(FileType::File)));
    {
        let mut f = src_file.borrow_mut();
        f.name = "locked_source.zip".to_string();
        f.size = Some(1024);
        f.crc = Some(vec![0xAA, 0xBB, 0xCC, 0xDD]);
        f.tree_checked = TreeSelect::Locked;
        f.set_dat_got_status(dat_reader::enums::DatStatus::NotInDat, GotStatus::Got);
        f.set_rep_status(RepStatus::NeededForFix);
        f.parent = Some(Rc::downgrade(&root));
    }

    let mut crc_map = HashMap::new();
    crc_map.insert((1024, vec![0xAA, 0xBB, 0xCC, 0xDD]), Rc::clone(&src_file));
    let sha1_map = HashMap::new();
    let md5_map = HashMap::new();

    let dst_file = Rc::new(RefCell::new(RvFile::new(FileType::File)));
    {
        let mut f = dst_file.borrow_mut();
        f.name = "dest.zip".to_string();
        f.size = Some(1024);
        f.crc = Some(vec![0xAA, 0xBB, 0xCC, 0xDD]);
        f.tree_checked = TreeSelect::Selected;
        f.set_dat_got_status(
            dat_reader::enums::DatStatus::InDatCollect,
            GotStatus::NotGot,
        );
        f.set_rep_status(RepStatus::CanBeFixed);
        f.parent = Some(Rc::downgrade(&root));
    }

    root.borrow_mut().child_add(Rc::clone(&src_file));
    root.borrow_mut().child_add(Rc::clone(&dst_file));

    Fix::fix_a_file(
        Rc::clone(&dst_file),
        &mut queue,
        &mut total_fixed,
        &crc_map,
        &sha1_map,
        &md5_map,
    );

    assert_eq!(dst_file.borrow().rep_status(), RepStatus::Correct);
    assert_eq!(src_file.borrow().rep_status(), RepStatus::NeededForFix);
    assert!(queue.is_empty());
    assert_eq!(total_fixed, 1);
}

#[test]
fn test_fix_deletes_unselected_neededforfix_source_after_using_it() {
    let temp = tempdir().unwrap();
    let root = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
    root.borrow_mut().name = temp.path().to_string_lossy().to_string();

    let src_file = Rc::new(RefCell::new(RvFile::new(FileType::File)));
    {
        let mut f = src_file.borrow_mut();
        f.name = "dupe.bin".to_string();
        f.size = Some(4);
        f.crc = Some(vec![0xAD, 0xF3, 0xF3, 0x63]);
        f.tree_checked = TreeSelect::UnSelected;
        f.set_dat_got_status(dat_reader::enums::DatStatus::NotInDat, GotStatus::Got);
        f.set_rep_status(RepStatus::NeededForFix);
        f.parent = Some(Rc::downgrade(&root));
    }

    let dst_file = Rc::new(RefCell::new(RvFile::new(FileType::File)));
    {
        let mut f = dst_file.borrow_mut();
        f.name = "needed.bin".to_string();
        f.size = Some(4);
        f.crc = Some(vec![0xAD, 0xF3, 0xF3, 0x63]);
        f.tree_checked = TreeSelect::Selected;
        f.set_dat_got_status(
            dat_reader::enums::DatStatus::InDatCollect,
            GotStatus::NotGot,
        );
        f.set_rep_status(RepStatus::CanBeFixed);
        f.parent = Some(Rc::downgrade(&root));
    }

    root.borrow_mut().child_add(Rc::clone(&src_file));
    root.borrow_mut().child_add(Rc::clone(&dst_file));

    let src_path = temp.path().join("dupe.bin");
    let dst_path = temp.path().join("needed.bin");
    fs::write(&src_path, b"data").unwrap();
    assert!(src_path.exists());
    assert!(!dst_path.exists());

    Fix::perform_fixes(Rc::clone(&root));

    assert_eq!(dst_file.borrow().rep_status(), RepStatus::Correct);
    assert_eq!(src_file.borrow().rep_status(), RepStatus::Deleted);
    assert_eq!(src_file.borrow().got_status(), GotStatus::NotGot);
    assert!(!src_path.exists());
    assert!(dst_path.exists());
    assert_eq!(fs::read(&dst_path).unwrap(), b"data");
}

#[test]
fn test_fix_can_be_fixed_avoids_self_cleanup_when_source_and_target_differ_only_by_case() {
    let temp = tempdir().unwrap();
    let root = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
    root.borrow_mut().name = temp.path().to_string_lossy().to_string();

    let src_file = Rc::new(RefCell::new(RvFile::new(FileType::File)));
    {
        let mut f = src_file.borrow_mut();
        f.name = "new.bin".to_string();
        f.file_name = "new.bin".to_string();
        f.size = Some(4);
        f.crc = Some(vec![0xAD, 0xF3, 0xF3, 0x63]);
        f.tree_checked = TreeSelect::Selected;
        f.set_dat_got_status(dat_reader::enums::DatStatus::NotInDat, GotStatus::Got);
        f.set_rep_status(RepStatus::NeededForFix);
        f.parent = Some(Rc::downgrade(&root));
    }

    let dst_file = Rc::new(RefCell::new(RvFile::new(FileType::File)));
    {
        let mut f = dst_file.borrow_mut();
        f.name = "New.bin".to_string();
        f.size = Some(4);
        f.crc = Some(vec![0xAD, 0xF3, 0xF3, 0x63]);
        f.tree_checked = TreeSelect::Selected;
        f.set_dat_got_status(
            dat_reader::enums::DatStatus::InDatCollect,
            GotStatus::NotGot,
        );
        f.set_rep_status(RepStatus::CanBeFixed);
        f.parent = Some(Rc::downgrade(&root));
    }

    root.borrow_mut().child_add(Rc::clone(&src_file));
    root.borrow_mut().child_add(Rc::clone(&dst_file));
    fs::write(temp.path().join("new.bin"), b"data").unwrap();

    let mut queue = Vec::new();
    let mut total_fixed = 0;
    let mut crc_map = HashMap::new();
    crc_map.insert((4, vec![0xAD, 0xF3, 0xF3, 0x63]), Rc::clone(&src_file));
    let sha1_map = HashMap::new();
    let md5_map = HashMap::new();

    Fix::fix_a_file(
        Rc::clone(&dst_file),
        &mut queue,
        &mut total_fixed,
        &crc_map,
        &sha1_map,
        &md5_map,
    );

    let entry_names: Vec<String> = fs::read_dir(temp.path())
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    assert_eq!(entry_names, vec!["New.bin".to_string()]);
    assert!(queue.is_empty());
    assert_eq!(dst_file.borrow().rep_status(), RepStatus::Correct);
    assert_eq!(src_file.borrow().rep_status(), RepStatus::NeededForFix);
    assert_eq!(total_fixed, 1);
}

#[test]
fn test_effective_desired_zip_struct_uses_global_seven_zip_default_when_none() {
    let original_settings = get_settings();
    update_settings(Settings {
        seven_z_default_struct: 2,
        ..Settings::default()
    });

    assert_eq!(
        Fix::effective_desired_zip_struct(FileType::SevenZip, ZipStructure::None),
        ZipStructure::SevenZipSZSTD
    );

    update_settings(original_settings);
}

#[test]
fn test_fix_skips_locked_targets() {
    let root = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));

    let locked_file = Rc::new(RefCell::new(RvFile::new(FileType::File)));
    {
        let mut file = locked_file.borrow_mut();
        file.name = "locked.zip".to_string();
        file.tree_checked = TreeSelect::Locked;
        file.set_rep_status(RepStatus::Rename);
        file.parent = Some(Rc::downgrade(&root));
    }

    root.borrow_mut().child_add(Rc::clone(&locked_file));

    Fix::perform_fixes(Rc::clone(&root));

    assert_eq!(locked_file.borrow().rep_status(), RepStatus::Rename);
}

#[test]
fn test_fix_rename_physically_renames_file_using_file_name_as_source() {
    let temp = tempdir().unwrap();
    let root = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
    root.borrow_mut().name = temp.path().to_string_lossy().to_string();

    let file = Rc::new(RefCell::new(RvFile::new(FileType::File)));
    {
        let mut f = file.borrow_mut();
        f.name = "new.bin".to_string();
        f.file_name = "old.bin".to_string();
        f.tree_checked = TreeSelect::Selected;
        f.set_dat_got_status(dat_reader::enums::DatStatus::InDatCollect, GotStatus::Got);
        f.set_rep_status(RepStatus::Rename);
        f.parent = Some(Rc::downgrade(&root));
    }
    root.borrow_mut().child_add(Rc::clone(&file));

    fs::write(temp.path().join("old.bin"), b"rename-me").unwrap();

    let mut queue = Vec::new();
    let mut total_fixed = 0;
    let crc_map = HashMap::new();
    let sha1_map = HashMap::new();
    let md5_map = HashMap::new();
    Fix::fix_a_file(
        Rc::clone(&file),
        &mut queue,
        &mut total_fixed,
        &crc_map,
        &sha1_map,
        &md5_map,
    );

    assert!(!temp.path().join("old.bin").exists());
    assert!(temp.path().join("new.bin").exists());
    assert_eq!(file.borrow().rep_status(), RepStatus::Correct);
    assert_eq!(file.borrow().file_name, "new.bin");
}

#[test]
fn test_fix_rename_physically_renames_file_when_name_differs_only_by_case() {
    let temp = tempdir().unwrap();
    let root = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
    root.borrow_mut().name = temp.path().to_string_lossy().to_string();

    let file = Rc::new(RefCell::new(RvFile::new(FileType::File)));
    {
        let mut f = file.borrow_mut();
        f.name = "New.bin".to_string();
        f.file_name = "new.bin".to_string();
        f.tree_checked = TreeSelect::Selected;
        f.set_dat_got_status(dat_reader::enums::DatStatus::InDatCollect, GotStatus::Got);
        f.set_rep_status(RepStatus::Rename);
        f.parent = Some(Rc::downgrade(&root));
    }
    root.borrow_mut().child_add(Rc::clone(&file));

    fs::write(temp.path().join("new.bin"), b"rename-me").unwrap();

    let mut queue = Vec::new();
    let mut total_fixed = 0;
    let crc_map = HashMap::new();
    let sha1_map = HashMap::new();
    let md5_map = HashMap::new();
    Fix::fix_a_file(
        Rc::clone(&file),
        &mut queue,
        &mut total_fixed,
        &crc_map,
        &sha1_map,
        &md5_map,
    );

    assert!(temp.path().join("New.bin").exists());
    let entry_names: Vec<String> = fs::read_dir(temp.path())
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    assert_eq!(entry_names, vec!["New.bin".to_string()]);
    assert_eq!(file.borrow().rep_status(), RepStatus::Correct);
    assert_eq!(file.borrow().file_name, "New.bin");
}

#[test]
fn test_fix_rename_preserves_correctmia_for_loose_file() {
    let temp = tempdir().unwrap();
    let root = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
    root.borrow_mut().name = temp.path().to_string_lossy().to_string();

    let file = Rc::new(RefCell::new(RvFile::new(FileType::File)));
    {
        let mut f = file.borrow_mut();
        f.name = "mia.bin".to_string();
        f.file_name = "old_mia.bin".to_string();
        f.tree_checked = TreeSelect::Selected;
        f.set_dat_got_status(dat_reader::enums::DatStatus::InDatMIA, GotStatus::Got);
        f.set_rep_status(RepStatus::Rename);
        f.parent = Some(Rc::downgrade(&root));
    }
    root.borrow_mut().child_add(Rc::clone(&file));

    fs::write(temp.path().join("old_mia.bin"), b"rename-me").unwrap();

    let mut queue = Vec::new();
    let mut total_fixed = 0;
    let crc_map = HashMap::new();
    let sha1_map = HashMap::new();
    let md5_map = HashMap::new();
    Fix::fix_a_file(
        Rc::clone(&file),
        &mut queue,
        &mut total_fixed,
        &crc_map,
        &sha1_map,
        &md5_map,
    );

    assert!(temp.path().join("mia.bin").exists());
    assert_eq!(file.borrow().rep_status(), RepStatus::CorrectMIA);
    assert_eq!(file.borrow().file_name, "mia.bin");
}

#[test]
fn test_fix_rename_physically_renames_archive_node_using_file_name_as_source() {
    let temp = tempdir().unwrap();
    let root = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
    root.borrow_mut().name = temp.path().to_string_lossy().to_string();

    let archive = Rc::new(RefCell::new(RvFile::new(FileType::Zip)));
    {
        let mut a = archive.borrow_mut();
        a.name = "new.zip".to_string();
        a.file_name = "old.zip".to_string();
        a.tree_checked = TreeSelect::Selected;
        a.set_dat_got_status(dat_reader::enums::DatStatus::InDatCollect, GotStatus::Got);
        a.set_rep_status(RepStatus::Rename);
        a.parent = Some(Rc::downgrade(&root));
    }
    root.borrow_mut().child_add(Rc::clone(&archive));

    fs::write(temp.path().join("old.zip"), b"zip-bytes").unwrap();

    Fix::fix_archive_node(Rc::clone(&archive));

    assert!(!temp.path().join("old.zip").exists());
    assert!(temp.path().join("new.zip").exists());
    assert_eq!(archive.borrow().rep_status(), RepStatus::Correct);
    assert_eq!(archive.borrow().file_name, "new.zip");
}

#[test]
fn test_fix_rename_physically_renames_archive_node_when_name_differs_only_by_case() {
    let temp = tempdir().unwrap();
    let root = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
    root.borrow_mut().name = temp.path().to_string_lossy().to_string();

    let archive = Rc::new(RefCell::new(RvFile::new(FileType::Zip)));
    {
        let mut a = archive.borrow_mut();
        a.name = "New.zip".to_string();
        a.file_name = "new.zip".to_string();
        a.tree_checked = TreeSelect::Selected;
        a.set_dat_got_status(dat_reader::enums::DatStatus::InDatCollect, GotStatus::Got);
        a.set_rep_status(RepStatus::Rename);
        a.parent = Some(Rc::downgrade(&root));
    }
    root.borrow_mut().child_add(Rc::clone(&archive));

    fs::write(temp.path().join("new.zip"), b"zip-bytes").unwrap();

    Fix::fix_archive_node(Rc::clone(&archive));

    assert!(temp.path().join("New.zip").exists());
    let entry_names: Vec<String> = fs::read_dir(temp.path())
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    assert_eq!(entry_names, vec!["New.zip".to_string()]);
    assert_eq!(archive.borrow().rep_status(), RepStatus::Correct);
    assert_eq!(archive.borrow().file_name, "New.zip");
}

#[test]
fn test_fix_rename_preserves_correctmia_for_archive_node() {
    let temp = tempdir().unwrap();
    let root = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
    root.borrow_mut().name = temp.path().to_string_lossy().to_string();

    let archive = Rc::new(RefCell::new(RvFile::new(FileType::Zip)));
    {
        let mut a = archive.borrow_mut();
        a.name = "mia.zip".to_string();
        a.file_name = "old_mia.zip".to_string();
        a.tree_checked = TreeSelect::Selected;
        a.set_dat_got_status(dat_reader::enums::DatStatus::InDatMIA, GotStatus::Got);
        a.set_rep_status(RepStatus::Rename);
        a.parent = Some(Rc::downgrade(&root));
    }
    root.borrow_mut().child_add(Rc::clone(&archive));

    fs::write(temp.path().join("old_mia.zip"), b"zip-bytes").unwrap();

    Fix::fix_archive_node(Rc::clone(&archive));

    assert!(temp.path().join("mia.zip").exists());
    assert_eq!(archive.borrow().rep_status(), RepStatus::CorrectMIA);
    assert_eq!(archive.borrow().file_name, "mia.zip");
}

#[test]
fn test_fix_selected_directory_renames_case_using_existing_file_name() {
    let temp = tempdir().unwrap();
    let root = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
    root.borrow_mut().name = temp.path().to_string_lossy().to_string();

    let dir = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
    {
        let mut d = dir.borrow_mut();
        d.name = "NewDir".to_string();
        d.file_name = "olddir".to_string();
        d.tree_checked = TreeSelect::Selected;
        d.parent = Some(Rc::downgrade(&root));
    }
    root.borrow_mut().child_add(Rc::clone(&dir));

    fs::create_dir_all(temp.path().join("olddir")).unwrap();

    Fix::perform_fixes(Rc::clone(&root));

    assert!(!temp.path().join("olddir").exists());
    assert!(temp.path().join("NewDir").exists());
    assert_eq!(dir.borrow().file_name, "NewDir");
}

#[test]
fn test_physical_path_eq_for_rename_matches_platform_semantics() {
    let left = Path::new("C:\\Root\\Folder");
    let right = Path::new("c:\\root\\folder");

    #[cfg(windows)]
    assert!(Fix::physical_path_eq_for_rename(left, right));
    #[cfg(not(windows))]
    assert!(!Fix::physical_path_eq_for_rename(left, right));
}

#[test]
fn test_fix_zip_move_moves_whole_archive() {
    let temp = tempdir().unwrap();
    let root = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
    root.borrow_mut().name = temp.path().to_string_lossy().to_string();

    let source_archive = Rc::new(RefCell::new(RvFile::new(FileType::Zip)));
    {
        let mut archive = source_archive.borrow_mut();
        archive.name = "source.zip".to_string();
        archive.tree_checked = TreeSelect::Selected;
        archive.parent = Some(Rc::downgrade(&root));
    }

    let source_child = Rc::new(RefCell::new(RvFile::new(FileType::File)));
    {
        let mut file = source_child.borrow_mut();
        file.name = "game.bin".to_string();
        file.size = Some(4);
        file.crc = Some(vec![0x12, 0x34, 0x56, 0x78]);
        file.tree_checked = TreeSelect::Selected;
        file.set_rep_status(RepStatus::NeededForFix);
        file.parent = Some(Rc::downgrade(&source_archive));
    }
    source_archive
        .borrow_mut()
        .child_add(Rc::clone(&source_child));

    let target_archive = Rc::new(RefCell::new(RvFile::new(FileType::Zip)));
    {
        let mut archive = target_archive.borrow_mut();
        archive.name = "target.zip".to_string();
        archive.tree_checked = TreeSelect::Selected;
        archive.set_dat_status(dat_reader::enums::DatStatus::InDatMIA);
        archive.parent = Some(Rc::downgrade(&root));
    }

    let target_child = Rc::new(RefCell::new(RvFile::new(FileType::File)));
    {
        let mut file = target_child.borrow_mut();
        file.name = "game.bin".to_string();
        file.size = Some(4);
        file.crc = Some(vec![0x12, 0x34, 0x56, 0x78]);
        file.tree_checked = TreeSelect::Selected;
        file.set_dat_got_status(
            dat_reader::enums::DatStatus::InDatCollect,
            dat_reader::enums::GotStatus::NotGot,
        );
        file.set_rep_status(RepStatus::CanBeFixed);
        file.parent = Some(Rc::downgrade(&target_archive));
    }
    target_archive
        .borrow_mut()
        .child_add(Rc::clone(&target_child));

    root.borrow_mut().child_add(Rc::clone(&source_archive));
    root.borrow_mut().child_add(Rc::clone(&target_archive));

    let source_path = temp.path().join("source.zip");
    std::fs::write(&source_path, b"ZIPDATA").unwrap();

    let mut queue = Vec::new();
    let mut total_fixed = 0;
    let mut crc_map = HashMap::new();
    crc_map.insert((4, vec![0x12, 0x34, 0x56, 0x78]), Rc::clone(&source_child));
    let sha1_map = HashMap::new();
    let md5_map = HashMap::new();

    assert!(Fix::find_source_file(&target_child.borrow(), &crc_map, &sha1_map, &md5_map).is_some());
    Fix::fix_a_zip(
        Rc::clone(&target_archive),
        &mut queue,
        &mut total_fixed,
        &crc_map,
        &sha1_map,
        &md5_map,
    );

    assert_eq!(queue.len(), 1);
    Fix::fix_archive_node(Rc::clone(&queue.remove(0)));

    let target_path = temp.path().join("target.zip");
    assert!(target_path.exists());
    assert!(!source_path.exists());
    assert_eq!(std::fs::read(&target_path).unwrap(), b"ZIPDATA");
    assert_eq!(target_child.borrow().rep_status(), RepStatus::Correct);
    assert_eq!(source_archive.borrow().rep_status(), RepStatus::Deleted);
    assert_eq!(total_fixed, 1);
}

#[test]
fn test_fix_zip_move_does_not_treat_case_only_archive_path_difference_as_distinct_source() {
    let temp = tempdir().unwrap();
    let root = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
    root.borrow_mut().name = temp.path().to_string_lossy().to_string();

    let source_archive = Rc::new(RefCell::new(RvFile::new(FileType::Zip)));
    {
        let mut archive = source_archive.borrow_mut();
        archive.name = "source.zip".to_string();
        archive.tree_checked = TreeSelect::Selected;
        archive.parent = Some(Rc::downgrade(&root));
    }

    let source_child = Rc::new(RefCell::new(RvFile::new(FileType::File)));
    {
        let mut file = source_child.borrow_mut();
        file.name = "game.bin".to_string();
        file.file_name = "game.bin".to_string();
        file.size = Some(4);
        file.crc = Some(vec![0x12, 0x34, 0x56, 0x78]);
        file.tree_checked = TreeSelect::Selected;
        file.set_rep_status(RepStatus::NeededForFix);
        file.parent = Some(Rc::downgrade(&source_archive));
    }
    source_archive
        .borrow_mut()
        .child_add(Rc::clone(&source_child));

    let target_archive = Rc::new(RefCell::new(RvFile::new(FileType::Zip)));
    {
        let mut archive = target_archive.borrow_mut();
        archive.name = "Source.zip".to_string();
        archive.tree_checked = TreeSelect::Selected;
        archive.parent = Some(Rc::downgrade(&root));
    }

    let target_child = Rc::new(RefCell::new(RvFile::new(FileType::File)));
    {
        let mut file = target_child.borrow_mut();
        file.name = "game.bin".to_string();
        file.file_name = "game.bin".to_string();
        file.size = Some(4);
        file.crc = Some(vec![0x12, 0x34, 0x56, 0x78]);
        file.tree_checked = TreeSelect::Selected;
        file.set_dat_got_status(
            dat_reader::enums::DatStatus::InDatCollect,
            dat_reader::enums::GotStatus::NotGot,
        );
        file.set_rep_status(RepStatus::CanBeFixed);
        file.parent = Some(Rc::downgrade(&target_archive));
    }
    target_archive
        .borrow_mut()
        .child_add(Rc::clone(&target_child));

    root.borrow_mut().child_add(Rc::clone(&source_archive));
    root.borrow_mut().child_add(Rc::clone(&target_archive));

    let source_path = temp.path().join("source.zip");
    {
        let file = File::create(&source_path).unwrap();
        let mut writer = ZipWriter::new(file);
        writer
            .start_file("game.bin", SimpleFileOptions::default())
            .unwrap();
        writer.write_all(b"data").unwrap();
        writer.finish().unwrap();
    }

    let mut queue = Vec::new();
    let mut total_fixed = 0;
    let mut crc_map = HashMap::new();
    crc_map.insert((4, vec![0x12, 0x34, 0x56, 0x78]), Rc::clone(&source_child));
    let sha1_map = HashMap::new();
    let md5_map = HashMap::new();

    assert!(Fix::rebuild_zip_archive(
        Rc::clone(&target_archive),
        &mut queue,
        &mut total_fixed,
        &crc_map,
        &sha1_map,
        &md5_map,
    ));

    let target_path = temp.path().join("Source.zip");
    let mut data = Vec::new();
    ZipArchive::new(File::open(&target_path).unwrap())
        .unwrap()
        .by_name("game.bin")
        .unwrap()
        .read_to_end(&mut data)
        .unwrap();
    assert_eq!(data, b"data");
    assert!(queue.is_empty());
    assert_eq!(target_child.borrow().rep_status(), RepStatus::Correct);
    assert_eq!(target_child.borrow().got_status(), GotStatus::Got);
    assert_eq!(total_fixed, 1);
}

#[test]
fn test_fix_zip_move_moves_whole_archive_for_indatmerged_target_entry() {
    let temp = tempdir().unwrap();
    let root = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
    root.borrow_mut().name = temp.path().to_string_lossy().to_string();

    let source_archive = Rc::new(RefCell::new(RvFile::new(FileType::Zip)));
    {
        let mut archive = source_archive.borrow_mut();
        archive.name = "source.zip".to_string();
        archive.tree_checked = TreeSelect::Selected;
        archive.parent = Some(Rc::downgrade(&root));
    }

    let source_child = Rc::new(RefCell::new(RvFile::new(FileType::File)));
    {
        let mut file = source_child.borrow_mut();
        file.name = "game.bin".to_string();
        file.size = Some(4);
        file.crc = Some(vec![0x12, 0x34, 0x56, 0x78]);
        file.tree_checked = TreeSelect::Selected;
        file.set_rep_status(RepStatus::NeededForFix);
        file.parent = Some(Rc::downgrade(&source_archive));
    }
    source_archive
        .borrow_mut()
        .child_add(Rc::clone(&source_child));

    let target_archive = Rc::new(RefCell::new(RvFile::new(FileType::Zip)));
    {
        let mut archive = target_archive.borrow_mut();
        archive.name = "target.zip".to_string();
        archive.tree_checked = TreeSelect::Selected;
        archive.set_dat_status(dat_reader::enums::DatStatus::InDatMIA);
        archive.parent = Some(Rc::downgrade(&root));
    }

    let target_child = Rc::new(RefCell::new(RvFile::new(FileType::File)));
    {
        let mut file = target_child.borrow_mut();
        file.name = "game.bin".to_string();
        file.size = Some(4);
        file.crc = Some(vec![0x12, 0x34, 0x56, 0x78]);
        file.tree_checked = TreeSelect::Selected;
        file.set_dat_got_status(
            dat_reader::enums::DatStatus::InDatMerged,
            dat_reader::enums::GotStatus::NotGot,
        );
        file.set_rep_status(RepStatus::CanBeFixed);
        file.parent = Some(Rc::downgrade(&target_archive));
    }
    target_archive
        .borrow_mut()
        .child_add(Rc::clone(&target_child));

    root.borrow_mut().child_add(Rc::clone(&source_archive));
    root.borrow_mut().child_add(Rc::clone(&target_archive));

    let source_path = temp.path().join("source.zip");
    std::fs::write(&source_path, b"ZIPDATA").unwrap();

    let mut queue = Vec::new();
    let mut total_fixed = 0;
    let mut crc_map = HashMap::new();
    crc_map.insert((4, vec![0x12, 0x34, 0x56, 0x78]), Rc::clone(&source_child));
    let sha1_map = HashMap::new();
    let md5_map = HashMap::new();

    Fix::fix_a_zip(
        Rc::clone(&target_archive),
        &mut queue,
        &mut total_fixed,
        &crc_map,
        &sha1_map,
        &md5_map,
    );

    assert_eq!(queue.len(), 1);
    Fix::fix_archive_node(Rc::clone(&queue.remove(0)));

    let target_path = temp.path().join("target.zip");
    assert!(target_path.exists());
    assert!(!source_path.exists());
    assert_eq!(std::fs::read(&target_path).unwrap(), b"ZIPDATA");
    assert_eq!(target_child.borrow().rep_status(), RepStatus::Correct);
    assert_eq!(source_archive.borrow().rep_status(), RepStatus::Deleted);
    assert_eq!(total_fixed, 1);
}

#[test]
fn test_fix_zip_move_moves_whole_archive_with_nested_directory_members() {
    let temp = tempdir().unwrap();
    let root = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
    root.borrow_mut().name = temp.path().to_string_lossy().to_string();

    let source_archive = Rc::new(RefCell::new(RvFile::new(FileType::Zip)));
    {
        let mut archive = source_archive.borrow_mut();
        archive.name = "source.zip".to_string();
        archive.tree_checked = TreeSelect::Selected;
        archive.parent = Some(Rc::downgrade(&root));
    }

    let source_dir = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
    {
        let mut dir = source_dir.borrow_mut();
        dir.name = "sub".to_string();
        dir.tree_checked = TreeSelect::Selected;
        dir.parent = Some(Rc::downgrade(&source_archive));
    }

    let source_child = Rc::new(RefCell::new(RvFile::new(FileType::File)));
    {
        let mut file = source_child.borrow_mut();
        file.name = "game.bin".to_string();
        file.size = Some(4);
        file.crc = Some(vec![0x12, 0x34, 0x56, 0x78]);
        file.tree_checked = TreeSelect::Selected;
        file.set_rep_status(RepStatus::NeededForFix);
        file.parent = Some(Rc::downgrade(&source_dir));
    }
    source_dir.borrow_mut().child_add(Rc::clone(&source_child));
    source_archive
        .borrow_mut()
        .child_add(Rc::clone(&source_dir));

    let target_archive = Rc::new(RefCell::new(RvFile::new(FileType::Zip)));
    {
        let mut archive = target_archive.borrow_mut();
        archive.name = "target.zip".to_string();
        archive.tree_checked = TreeSelect::Selected;
        archive.set_dat_status(dat_reader::enums::DatStatus::InDatMIA);
        archive.parent = Some(Rc::downgrade(&root));
    }

    let target_dir = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
    {
        let mut dir = target_dir.borrow_mut();
        dir.name = "sub".to_string();
        dir.tree_checked = TreeSelect::Selected;
        dir.parent = Some(Rc::downgrade(&target_archive));
    }

    let target_child = Rc::new(RefCell::new(RvFile::new(FileType::File)));
    {
        let mut file = target_child.borrow_mut();
        file.name = "game.bin".to_string();
        file.size = Some(4);
        file.crc = Some(vec![0x12, 0x34, 0x56, 0x78]);
        file.tree_checked = TreeSelect::Selected;
        file.set_dat_got_status(
            dat_reader::enums::DatStatus::InDatCollect,
            dat_reader::enums::GotStatus::NotGot,
        );
        file.set_rep_status(RepStatus::CanBeFixed);
        file.parent = Some(Rc::downgrade(&target_dir));
    }
    target_dir.borrow_mut().child_add(Rc::clone(&target_child));
    target_archive
        .borrow_mut()
        .child_add(Rc::clone(&target_dir));

    root.borrow_mut().child_add(Rc::clone(&source_archive));
    root.borrow_mut().child_add(Rc::clone(&target_archive));

    let source_path = temp.path().join("source.zip");
    std::fs::write(&source_path, b"ZIPDATA").unwrap();

    let mut queue = Vec::new();
    let mut total_fixed = 0;
    let mut crc_map = HashMap::new();
    crc_map.insert((4, vec![0x12, 0x34, 0x56, 0x78]), Rc::clone(&source_child));
    let sha1_map = HashMap::new();
    let md5_map = HashMap::new();

    Fix::fix_a_zip(
        Rc::clone(&target_archive),
        &mut queue,
        &mut total_fixed,
        &crc_map,
        &sha1_map,
        &md5_map,
    );

    assert_eq!(queue.len(), 1);
    Fix::fix_archive_node(Rc::clone(&queue.remove(0)));

    let target_path = temp.path().join("target.zip");
    assert!(target_path.exists());
    assert!(!source_path.exists());
    assert_eq!(std::fs::read(&target_path).unwrap(), b"ZIPDATA");
    assert_eq!(target_child.borrow().rep_status(), RepStatus::Correct);
    assert_eq!(source_archive.borrow().rep_status(), RepStatus::Deleted);
    assert_eq!(total_fixed, 1);
}

#[test]
fn test_fix_zip_move_copies_locked_source_archive() {
    let temp = tempdir().unwrap();
    let root = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
    root.borrow_mut().name = temp.path().to_string_lossy().to_string();

    let source_archive = Rc::new(RefCell::new(RvFile::new(FileType::Zip)));
    {
        let mut archive = source_archive.borrow_mut();
        archive.name = "source.zip".to_string();
        archive.tree_checked = TreeSelect::Locked;
        archive.parent = Some(Rc::downgrade(&root));
    }

    let source_child = Rc::new(RefCell::new(RvFile::new(FileType::File)));
    {
        let mut file = source_child.borrow_mut();
        file.name = "game.bin".to_string();
        file.size = Some(4);
        file.crc = Some(vec![0x12, 0x34, 0x56, 0x78]);
        file.tree_checked = TreeSelect::Selected;
        file.set_rep_status(RepStatus::NeededForFix);
        file.parent = Some(Rc::downgrade(&source_archive));
    }
    source_archive
        .borrow_mut()
        .child_add(Rc::clone(&source_child));

    let target_archive = Rc::new(RefCell::new(RvFile::new(FileType::Zip)));
    {
        let mut archive = target_archive.borrow_mut();
        archive.name = "target.zip".to_string();
        archive.tree_checked = TreeSelect::Selected;
        archive.parent = Some(Rc::downgrade(&root));
    }

    let target_child = Rc::new(RefCell::new(RvFile::new(FileType::File)));
    {
        let mut file = target_child.borrow_mut();
        file.name = "game.bin".to_string();
        file.size = Some(4);
        file.crc = Some(vec![0x12, 0x34, 0x56, 0x78]);
        file.tree_checked = TreeSelect::Selected;
        file.set_dat_got_status(
            dat_reader::enums::DatStatus::InDatCollect,
            dat_reader::enums::GotStatus::NotGot,
        );
        file.set_rep_status(RepStatus::CanBeFixed);
        file.parent = Some(Rc::downgrade(&target_archive));
    }
    target_archive
        .borrow_mut()
        .child_add(Rc::clone(&target_child));

    root.borrow_mut().child_add(Rc::clone(&source_archive));
    root.borrow_mut().child_add(Rc::clone(&target_archive));

    let source_path = temp.path().join("source.zip");
    std::fs::write(&source_path, b"ZIPDATA").unwrap();

    let mut queue = Vec::new();
    let mut total_fixed = 0;
    let mut crc_map = HashMap::new();
    crc_map.insert((4, vec![0x12, 0x34, 0x56, 0x78]), Rc::clone(&source_child));
    let sha1_map = HashMap::new();
    let md5_map = HashMap::new();

    assert!(Fix::find_source_file(&target_child.borrow(), &crc_map, &sha1_map, &md5_map).is_some());
    Fix::fix_a_zip(
        Rc::clone(&target_archive),
        &mut queue,
        &mut total_fixed,
        &crc_map,
        &sha1_map,
        &md5_map,
    );

    let target_path = temp.path().join("target.zip");
    assert!(target_path.exists());
    assert!(source_path.exists());
    assert_eq!(std::fs::read(&target_path).unwrap(), b"ZIPDATA");
    assert_eq!(source_archive.borrow().rep_status(), RepStatus::UnSet);
    assert!(queue.is_empty());
    assert_eq!(total_fixed, 1);
}

#[test]
fn test_fix_zip_partial_rebuild_preserves_existing_and_adds_missing() {
    let temp = tempdir().unwrap();
    let root = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
    root.borrow_mut().name = temp.path().to_string_lossy().to_string();

    let source_dir = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
    {
        let mut dir = source_dir.borrow_mut();
        dir.name = "ToSort".to_string();
        dir.tree_checked = TreeSelect::Selected;
        dir.parent = Some(Rc::downgrade(&root));
    }

    let source_file = Rc::new(RefCell::new(RvFile::new(FileType::File)));
    {
        let mut file = source_file.borrow_mut();
        file.name = "missing.bin".to_string();
        file.size = Some(3);
        file.crc = Some(vec![0x00, 0x00, 0x00, 0x03]);
        file.tree_checked = TreeSelect::Selected;
        file.set_rep_status(RepStatus::NeededForFix);
        file.parent = Some(Rc::downgrade(&source_dir));
    }
    source_dir.borrow_mut().child_add(Rc::clone(&source_file));

    let target_archive = Rc::new(RefCell::new(RvFile::new(FileType::Zip)));
    {
        let mut archive = target_archive.borrow_mut();
        archive.name = "target.zip".to_string();
        archive.tree_checked = TreeSelect::Selected;
        archive.parent = Some(Rc::downgrade(&root));
    }

    let keep_child = Rc::new(RefCell::new(RvFile::new(FileType::File)));
    {
        let mut file = keep_child.borrow_mut();
        file.name = "keep.bin".to_string();
        file.size = Some(4);
        file.tree_checked = TreeSelect::Selected;
        file.set_dat_got_status(dat_reader::enums::DatStatus::InDatCollect, GotStatus::Got);
        file.set_rep_status(RepStatus::Correct);
        file.parent = Some(Rc::downgrade(&target_archive));
    }

    let missing_child = Rc::new(RefCell::new(RvFile::new(FileType::File)));
    {
        let mut file = missing_child.borrow_mut();
        file.name = "missing.bin".to_string();
        file.size = Some(3);
        file.crc = Some(vec![0x00, 0x00, 0x00, 0x03]);
        file.tree_checked = TreeSelect::Selected;
        file.set_dat_got_status(
            dat_reader::enums::DatStatus::InDatCollect,
            GotStatus::NotGot,
        );
        file.set_rep_status(RepStatus::CanBeFixed);
        file.parent = Some(Rc::downgrade(&target_archive));
    }

    target_archive
        .borrow_mut()
        .child_add(Rc::clone(&keep_child));
    target_archive
        .borrow_mut()
        .child_add(Rc::clone(&missing_child));

    root.borrow_mut().child_add(Rc::clone(&source_dir));
    root.borrow_mut().child_add(Rc::clone(&target_archive));

    fs::create_dir_all(temp.path().join("ToSort")).unwrap();
    fs::write(temp.path().join("ToSort").join("missing.bin"), b"new").unwrap();

    let target_path = temp.path().join("target.zip");
    {
        let file = File::create(&target_path).unwrap();
        let mut writer = ZipWriter::new(file);
        writer
            .start_file("keep.bin", SimpleFileOptions::default())
            .unwrap();
        writer.write_all(b"keep").unwrap();
        writer.finish().unwrap();
    }

    let mut queue = Vec::new();
    let mut total_fixed = 0;
    let mut crc_map = HashMap::new();
    crc_map.insert((3, vec![0x00, 0x00, 0x00, 0x03]), Rc::clone(&source_file));
    let sha1_map = HashMap::new();
    let md5_map = HashMap::new();

    assert!(Fix::rebuild_zip_archive(
        Rc::clone(&target_archive),
        &mut queue,
        &mut total_fixed,
        &crc_map,
        &sha1_map,
        &md5_map,
    ));
    assert_eq!(queue.len(), 1);
    let mut cleanup_queue = Vec::new();
    let mut cleanup_total_fixed = 0;
    Fix::fix_a_file(
        queue.remove(0),
        &mut cleanup_queue,
        &mut cleanup_total_fixed,
        &crc_map,
        &sha1_map,
        &md5_map,
    );

    let file = File::open(&target_path).unwrap();
    let mut archive = ZipArchive::new(file).unwrap();
    let mut keep_data = Vec::new();
    archive
        .by_name("keep.bin")
        .unwrap()
        .read_to_end(&mut keep_data)
        .unwrap();
    let mut missing_data = Vec::new();
    archive
        .by_name("missing.bin")
        .unwrap()
        .read_to_end(&mut missing_data)
        .unwrap();

    assert_eq!(keep_data, b"keep");
    assert_eq!(missing_data, b"new");
    assert_eq!(missing_child.borrow().rep_status(), RepStatus::Correct);
    assert_eq!(source_file.borrow().rep_status(), RepStatus::Deleted);
    assert_eq!(total_fixed, 1);
}

#[test]
fn test_fix_zip_partial_rebuild_canbefixedmia_survives_reset() {
    let temp = tempdir().unwrap();
    let root = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
    root.borrow_mut().name = temp.path().to_string_lossy().to_string();

    let source_dir = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
    {
        let mut dir = source_dir.borrow_mut();
        dir.name = "ToSort".to_string();
        dir.tree_checked = TreeSelect::Selected;
        dir.parent = Some(Rc::downgrade(&root));
    }

    let source_file = Rc::new(RefCell::new(RvFile::new(FileType::File)));
    {
        let mut file = source_file.borrow_mut();
        file.name = "missing.bin".to_string();
        file.size = Some(3);
        file.crc = Some(vec![0x00, 0x00, 0x00, 0x03]);
        file.tree_checked = TreeSelect::Selected;
        file.set_rep_status(RepStatus::NeededForFix);
        file.parent = Some(Rc::downgrade(&source_dir));
    }
    source_dir.borrow_mut().child_add(Rc::clone(&source_file));

    let target_archive = Rc::new(RefCell::new(RvFile::new(FileType::Zip)));
    {
        let mut archive = target_archive.borrow_mut();
        archive.name = "target.zip".to_string();
        archive.tree_checked = TreeSelect::Selected;
        archive.parent = Some(Rc::downgrade(&root));
    }

    let missing_child = Rc::new(RefCell::new(RvFile::new(FileType::File)));
    {
        let mut file = missing_child.borrow_mut();
        file.name = "missing.bin".to_string();
        file.size = Some(3);
        file.crc = Some(vec![0x00, 0x00, 0x00, 0x03]);
        file.tree_checked = TreeSelect::Selected;
        file.set_dat_got_status(dat_reader::enums::DatStatus::InDatMIA, GotStatus::NotGot);
        file.set_rep_status(RepStatus::CanBeFixedMIA);
        file.parent = Some(Rc::downgrade(&target_archive));
    }

    target_archive
        .borrow_mut()
        .child_add(Rc::clone(&missing_child));
    root.borrow_mut().child_add(Rc::clone(&source_dir));
    root.borrow_mut().child_add(Rc::clone(&target_archive));

    fs::create_dir_all(temp.path().join("ToSort")).unwrap();
    fs::write(temp.path().join("ToSort").join("missing.bin"), b"new").unwrap();

    let target_path = temp.path().join("target.zip");
    {
        let file = File::create(&target_path).unwrap();
        let writer = ZipWriter::new(file);
        writer.finish().unwrap();
    }

    let mut queue = Vec::new();
    let mut total_fixed = 0;
    let mut crc_map = HashMap::new();
    crc_map.insert((3, vec![0x00, 0x00, 0x00, 0x03]), Rc::clone(&source_file));
    let sha1_map = HashMap::new();
    let md5_map = HashMap::new();

    assert!(Fix::rebuild_zip_archive(
        Rc::clone(&target_archive),
        &mut queue,
        &mut total_fixed,
        &crc_map,
        &sha1_map,
        &md5_map,
    ));

    assert_eq!(missing_child.borrow().rep_status(), RepStatus::CorrectMIA);
    missing_child.borrow_mut().rep_status_reset();
    assert_eq!(missing_child.borrow().rep_status(), RepStatus::CorrectMIA);
    assert_eq!(missing_child.borrow().got_status(), GotStatus::Got);
}

#[test]
fn test_fix_zip_partial_rebuild_matches_alt_sha1_source() {
    let temp = tempdir().unwrap();
    let root = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
    root.borrow_mut().name = temp.path().to_string_lossy().to_string();

    let source_dir = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
    {
        let mut dir = source_dir.borrow_mut();
        dir.name = "ToSort".to_string();
        dir.tree_checked = TreeSelect::Selected;
        dir.parent = Some(Rc::downgrade(&root));
    }

    let source_file = Rc::new(RefCell::new(RvFile::new(FileType::File)));
    {
        let mut file = source_file.borrow_mut();
        file.name = "missing.bin".to_string();
        file.size = Some(3);
        file.sha1 = Some(vec![0x01, 0x23, 0x45, 0x67]);
        file.tree_checked = TreeSelect::Selected;
        file.set_rep_status(RepStatus::NeededForFix);
        file.parent = Some(Rc::downgrade(&source_dir));
    }
    source_dir.borrow_mut().child_add(Rc::clone(&source_file));

    let target_archive = Rc::new(RefCell::new(RvFile::new(FileType::Zip)));
    {
        let mut archive = target_archive.borrow_mut();
        archive.name = "target.zip".to_string();
        archive.tree_checked = TreeSelect::Selected;
        archive.parent = Some(Rc::downgrade(&root));
    }

    let missing_child = Rc::new(RefCell::new(RvFile::new(FileType::File)));
    {
        let mut file = missing_child.borrow_mut();
        file.name = "missing.bin".to_string();
        file.alt_size = Some(3);
        file.alt_sha1 = Some(vec![0x01, 0x23, 0x45, 0x67]);
        file.tree_checked = TreeSelect::Selected;
        file.set_dat_got_status(
            dat_reader::enums::DatStatus::InDatCollect,
            GotStatus::NotGot,
        );
        file.set_rep_status(RepStatus::CanBeFixed);
        file.parent = Some(Rc::downgrade(&target_archive));
    }

    target_archive
        .borrow_mut()
        .child_add(Rc::clone(&missing_child));
    root.borrow_mut().child_add(Rc::clone(&source_dir));
    root.borrow_mut().child_add(Rc::clone(&target_archive));

    fs::create_dir_all(temp.path().join("ToSort")).unwrap();
    fs::write(temp.path().join("ToSort").join("missing.bin"), b"new").unwrap();

    let target_path = temp.path().join("target.zip");
    {
        let file = File::create(&target_path).unwrap();
        let writer = ZipWriter::new(file);
        writer.finish().unwrap();
    }

    let mut queue = Vec::new();
    let mut total_fixed = 0;
    let crc_map = HashMap::new();
    let mut sha1_map = HashMap::new();
    sha1_map.insert((3, vec![0x01, 0x23, 0x45, 0x67]), Rc::clone(&source_file));
    let md5_map = HashMap::new();

    assert!(Fix::rebuild_zip_archive(
        Rc::clone(&target_archive),
        &mut queue,
        &mut total_fixed,
        &crc_map,
        &sha1_map,
        &md5_map,
    ));
    assert_eq!(queue.len(), 1);
    let mut cleanup_queue = Vec::new();
    let mut cleanup_total_fixed = 0;
    Fix::fix_a_file(
        queue.remove(0),
        &mut cleanup_queue,
        &mut cleanup_total_fixed,
        &crc_map,
        &sha1_map,
        &md5_map,
    );

    let file = File::open(&target_path).unwrap();
    let mut archive = ZipArchive::new(file).unwrap();
    let mut missing_data = Vec::new();
    archive
        .by_name("missing.bin")
        .unwrap()
        .read_to_end(&mut missing_data)
        .unwrap();

    assert_eq!(missing_data, b"new");
    assert_eq!(missing_child.borrow().rep_status(), RepStatus::Correct);
    assert_eq!(source_file.borrow().rep_status(), RepStatus::Deleted);
    assert_eq!(total_fixed, 1);
}

#[test]
fn test_fix_sevenzip_partial_rebuild_matches_alt_md5_source() {
    let temp = tempdir().unwrap();
    let root = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
    root.borrow_mut().name = temp.path().to_string_lossy().to_string();

    let source_dir = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
    {
        let mut dir = source_dir.borrow_mut();
        dir.name = "ToSort".to_string();
        dir.tree_checked = TreeSelect::Selected;
        dir.parent = Some(Rc::downgrade(&root));
    }

    let source_file = Rc::new(RefCell::new(RvFile::new(FileType::File)));
    {
        let mut file = source_file.borrow_mut();
        file.name = "missing.bin".to_string();
        file.size = Some(3);
        file.md5 = Some(vec![0x01, 0x23, 0x45, 0x67]);
        file.tree_checked = TreeSelect::Selected;
        file.set_rep_status(RepStatus::NeededForFix);
        file.parent = Some(Rc::downgrade(&source_dir));
    }
    source_dir.borrow_mut().child_add(Rc::clone(&source_file));

    let target_archive = Rc::new(RefCell::new(RvFile::new(FileType::SevenZip)));
    {
        let mut archive = target_archive.borrow_mut();
        archive.name = "target.7z".to_string();
        archive.tree_checked = TreeSelect::Selected;
        archive.set_zip_dat_struct(ZipStructure::SevenZipSLZMA, true);
        archive.parent = Some(Rc::downgrade(&root));
    }

    let missing_child = Rc::new(RefCell::new(RvFile::new(FileType::File)));
    {
        let mut file = missing_child.borrow_mut();
        file.name = "missing.bin".to_string();
        file.alt_size = Some(3);
        file.alt_md5 = Some(vec![0x01, 0x23, 0x45, 0x67]);
        file.tree_checked = TreeSelect::Selected;
        file.set_dat_got_status(
            dat_reader::enums::DatStatus::InDatCollect,
            GotStatus::NotGot,
        );
        file.set_rep_status(RepStatus::CanBeFixed);
        file.parent = Some(Rc::downgrade(&target_archive));
    }

    target_archive
        .borrow_mut()
        .child_add(Rc::clone(&missing_child));
    root.borrow_mut().child_add(Rc::clone(&source_dir));
    root.borrow_mut().child_add(Rc::clone(&target_archive));

    fs::create_dir_all(temp.path().join("ToSort")).unwrap();
    fs::write(temp.path().join("ToSort").join("missing.bin"), b"new").unwrap();

    let stage_dir = temp.path().join("stage_alt_md5_7z");
    fs::create_dir_all(&stage_dir).unwrap();
    let target_path = temp.path().join("target.7z");
    sevenz_rust::compress_to_path(&stage_dir, &target_path).unwrap();

    let mut queue = Vec::new();
    let mut total_fixed = 0;
    let crc_map = HashMap::new();
    let sha1_map = HashMap::new();
    let mut md5_map = HashMap::new();
    md5_map.insert((3, vec![0x01, 0x23, 0x45, 0x67]), Rc::clone(&source_file));

    assert!(Fix::rebuild_seven_zip_archive(
        Rc::clone(&target_archive),
        &mut queue,
        &mut total_fixed,
        &crc_map,
        &sha1_map,
        &md5_map,
    ));
    assert_eq!(queue.len(), 1);
    let mut cleanup_queue = Vec::new();
    let mut cleanup_total_fixed = 0;
    Fix::fix_a_file(
        queue.remove(0),
        &mut cleanup_queue,
        &mut cleanup_total_fixed,
        &crc_map,
        &sha1_map,
        &md5_map,
    );

    assert_eq!(
        Fix::read_seven_zip_entry_bytes(&target_path.to_string_lossy(), "missing.bin").unwrap(),
        b"new"
    );
    assert_eq!(missing_child.borrow().rep_status(), RepStatus::Correct);
    assert_eq!(source_file.borrow().rep_status(), RepStatus::Deleted);
    assert_eq!(total_fixed, 1);
}

#[test]
fn test_fix_zip_partial_rebuild_matches_alt_md5_source() {
    let temp = tempdir().unwrap();
    let root = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
    root.borrow_mut().name = temp.path().to_string_lossy().to_string();

    let source_dir = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
    {
        let mut dir = source_dir.borrow_mut();
        dir.name = "ToSort".to_string();
        dir.tree_checked = TreeSelect::Selected;
        dir.parent = Some(Rc::downgrade(&root));
    }

    let source_file = Rc::new(RefCell::new(RvFile::new(FileType::File)));
    {
        let mut file = source_file.borrow_mut();
        file.name = "missing.bin".to_string();
        file.size = Some(3);
        file.md5 = Some(vec![0x10, 0x20, 0x30, 0x40]);
        file.tree_checked = TreeSelect::Selected;
        file.set_rep_status(RepStatus::NeededForFix);
        file.parent = Some(Rc::downgrade(&source_dir));
    }
    source_dir.borrow_mut().child_add(Rc::clone(&source_file));

    let target_archive = Rc::new(RefCell::new(RvFile::new(FileType::Zip)));
    {
        let mut archive = target_archive.borrow_mut();
        archive.name = "target.zip".to_string();
        archive.tree_checked = TreeSelect::Selected;
        archive.parent = Some(Rc::downgrade(&root));
    }

    let missing_child = Rc::new(RefCell::new(RvFile::new(FileType::File)));
    {
        let mut file = missing_child.borrow_mut();
        file.name = "missing.bin".to_string();
        file.alt_size = Some(3);
        file.alt_md5 = Some(vec![0x10, 0x20, 0x30, 0x40]);
        file.tree_checked = TreeSelect::Selected;
        file.set_dat_got_status(
            dat_reader::enums::DatStatus::InDatCollect,
            GotStatus::NotGot,
        );
        file.set_rep_status(RepStatus::CanBeFixed);
        file.parent = Some(Rc::downgrade(&target_archive));
    }

    target_archive
        .borrow_mut()
        .child_add(Rc::clone(&missing_child));
    root.borrow_mut().child_add(Rc::clone(&source_dir));
    root.borrow_mut().child_add(Rc::clone(&target_archive));

    fs::create_dir_all(temp.path().join("ToSort")).unwrap();
    fs::write(temp.path().join("ToSort").join("missing.bin"), b"new").unwrap();

    let target_path = temp.path().join("target.zip");
    {
        let file = File::create(&target_path).unwrap();
        let writer = ZipWriter::new(file);
        writer.finish().unwrap();
    }

    let mut queue = Vec::new();
    let mut total_fixed = 0;
    let crc_map = HashMap::new();
    let sha1_map = HashMap::new();
    let mut md5_map = HashMap::new();
    md5_map.insert((3, vec![0x10, 0x20, 0x30, 0x40]), Rc::clone(&source_file));

    assert!(Fix::rebuild_zip_archive(
        Rc::clone(&target_archive),
        &mut queue,
        &mut total_fixed,
        &crc_map,
        &sha1_map,
        &md5_map,
    ));
    assert_eq!(queue.len(), 1);
    let mut cleanup_queue = Vec::new();
    let mut cleanup_total_fixed = 0;
    Fix::fix_a_file(
        queue.remove(0),
        &mut cleanup_queue,
        &mut cleanup_total_fixed,
        &crc_map,
        &sha1_map,
        &md5_map,
    );

    let file = File::open(&target_path).unwrap();
    let mut archive = ZipArchive::new(file).unwrap();
    let mut missing_data = Vec::new();
    archive
        .by_name("missing.bin")
        .unwrap()
        .read_to_end(&mut missing_data)
        .unwrap();

    assert_eq!(missing_data, b"new");
    assert_eq!(missing_child.borrow().rep_status(), RepStatus::Correct);
    assert_eq!(source_file.borrow().rep_status(), RepStatus::Deleted);
    assert_eq!(total_fixed, 1);
}

#[test]
fn test_fix_sevenzip_partial_rebuild_matches_alt_sha1_source() {
    let temp = tempdir().unwrap();
    let root = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
    root.borrow_mut().name = temp.path().to_string_lossy().to_string();

    let source_dir = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
    {
        let mut dir = source_dir.borrow_mut();
        dir.name = "ToSort".to_string();
        dir.tree_checked = TreeSelect::Selected;
        dir.parent = Some(Rc::downgrade(&root));
    }

    let source_file = Rc::new(RefCell::new(RvFile::new(FileType::File)));
    {
        let mut file = source_file.borrow_mut();
        file.name = "missing.bin".to_string();
        file.size = Some(3);
        file.sha1 = Some(vec![0x10, 0x20, 0x30, 0x40]);
        file.tree_checked = TreeSelect::Selected;
        file.set_rep_status(RepStatus::NeededForFix);
        file.parent = Some(Rc::downgrade(&source_dir));
    }
    source_dir.borrow_mut().child_add(Rc::clone(&source_file));

    let target_archive = Rc::new(RefCell::new(RvFile::new(FileType::SevenZip)));
    {
        let mut archive = target_archive.borrow_mut();
        archive.name = "target.7z".to_string();
        archive.tree_checked = TreeSelect::Selected;
        archive.set_zip_dat_struct(ZipStructure::SevenZipSLZMA, true);
        archive.parent = Some(Rc::downgrade(&root));
    }

    let missing_child = Rc::new(RefCell::new(RvFile::new(FileType::File)));
    {
        let mut file = missing_child.borrow_mut();
        file.name = "missing.bin".to_string();
        file.alt_size = Some(3);
        file.alt_sha1 = Some(vec![0x10, 0x20, 0x30, 0x40]);
        file.tree_checked = TreeSelect::Selected;
        file.set_dat_got_status(
            dat_reader::enums::DatStatus::InDatCollect,
            GotStatus::NotGot,
        );
        file.set_rep_status(RepStatus::CanBeFixed);
        file.parent = Some(Rc::downgrade(&target_archive));
    }

    target_archive
        .borrow_mut()
        .child_add(Rc::clone(&missing_child));
    root.borrow_mut().child_add(Rc::clone(&source_dir));
    root.borrow_mut().child_add(Rc::clone(&target_archive));

    fs::create_dir_all(temp.path().join("ToSort")).unwrap();
    fs::write(temp.path().join("ToSort").join("missing.bin"), b"new").unwrap();

    let stage_dir = temp.path().join("stage_alt_sha1_7z");
    fs::create_dir_all(&stage_dir).unwrap();
    let target_path = temp.path().join("target.7z");
    sevenz_rust::compress_to_path(&stage_dir, &target_path).unwrap();

    let mut queue = Vec::new();
    let mut total_fixed = 0;
    let crc_map = HashMap::new();
    let mut sha1_map = HashMap::new();
    sha1_map.insert((3, vec![0x10, 0x20, 0x30, 0x40]), Rc::clone(&source_file));
    let md5_map = HashMap::new();

    assert!(Fix::rebuild_seven_zip_archive(
        Rc::clone(&target_archive),
        &mut queue,
        &mut total_fixed,
        &crc_map,
        &sha1_map,
        &md5_map,
    ));
    assert_eq!(queue.len(), 1);
    let mut cleanup_queue = Vec::new();
    let mut cleanup_total_fixed = 0;
    Fix::fix_a_file(
        queue.remove(0),
        &mut cleanup_queue,
        &mut cleanup_total_fixed,
        &crc_map,
        &sha1_map,
        &md5_map,
    );

    assert_eq!(
        Fix::read_seven_zip_entry_bytes(&target_path.to_string_lossy(), "missing.bin").unwrap(),
        b"new"
    );
    assert_eq!(missing_child.borrow().rep_status(), RepStatus::Correct);
    assert_eq!(source_file.borrow().rep_status(), RepStatus::Deleted);
    assert_eq!(total_fixed, 1);
}

#[test]
fn test_fix_zip_partial_rebuild_matches_primary_md5_against_source_alt_md5() {
    let temp = tempdir().unwrap();
    let root = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
    root.borrow_mut().name = temp.path().to_string_lossy().to_string();

    let source_dir = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
    {
        let mut dir = source_dir.borrow_mut();
        dir.name = "ToSort".to_string();
        dir.tree_checked = TreeSelect::Selected;
        dir.parent = Some(Rc::downgrade(&root));
    }

    let source_file = Rc::new(RefCell::new(RvFile::new(FileType::File)));
    {
        let mut file = source_file.borrow_mut();
        file.name = "missing.bin".to_string();
        file.alt_size = Some(3);
        file.alt_md5 = Some(vec![0x10, 0x20, 0x30, 0x40]);
        file.tree_checked = TreeSelect::Selected;
        file.set_rep_status(RepStatus::NeededForFix);
        file.parent = Some(Rc::downgrade(&source_dir));
    }
    source_dir.borrow_mut().child_add(Rc::clone(&source_file));

    let target_archive = Rc::new(RefCell::new(RvFile::new(FileType::Zip)));
    {
        let mut archive = target_archive.borrow_mut();
        archive.name = "target.zip".to_string();
        archive.tree_checked = TreeSelect::Selected;
        archive.parent = Some(Rc::downgrade(&root));
    }

    let missing_child = Rc::new(RefCell::new(RvFile::new(FileType::File)));
    {
        let mut file = missing_child.borrow_mut();
        file.name = "missing.bin".to_string();
        file.size = Some(3);
        file.md5 = Some(vec![0x10, 0x20, 0x30, 0x40]);
        file.tree_checked = TreeSelect::Selected;
        file.set_dat_got_status(
            dat_reader::enums::DatStatus::InDatCollect,
            GotStatus::NotGot,
        );
        file.set_rep_status(RepStatus::CanBeFixed);
        file.parent = Some(Rc::downgrade(&target_archive));
    }

    target_archive
        .borrow_mut()
        .child_add(Rc::clone(&missing_child));
    root.borrow_mut().child_add(Rc::clone(&source_dir));
    root.borrow_mut().child_add(Rc::clone(&target_archive));

    fs::create_dir_all(temp.path().join("ToSort")).unwrap();
    fs::write(temp.path().join("ToSort").join("missing.bin"), b"new").unwrap();

    let target_path = temp.path().join("target.zip");
    {
        let file = File::create(&target_path).unwrap();
        let writer = ZipWriter::new(file);
        writer.finish().unwrap();
    }

    let mut queue = Vec::new();
    let mut total_fixed = 0;
    let crc_map = HashMap::new();
    let sha1_map = HashMap::new();
    let mut md5_map = HashMap::new();
    md5_map.insert((3, vec![0x10, 0x20, 0x30, 0x40]), Rc::clone(&source_file));

    assert!(Fix::rebuild_zip_archive(
        Rc::clone(&target_archive),
        &mut queue,
        &mut total_fixed,
        &crc_map,
        &sha1_map,
        &md5_map,
    ));
    assert_eq!(queue.len(), 1);
    let mut cleanup_queue = Vec::new();
    let mut cleanup_total_fixed = 0;
    Fix::fix_a_file(
        queue.remove(0),
        &mut cleanup_queue,
        &mut cleanup_total_fixed,
        &crc_map,
        &sha1_map,
        &md5_map,
    );

    let file = File::open(&target_path).unwrap();
    let mut archive = ZipArchive::new(file).unwrap();
    let mut missing_data = Vec::new();
    archive
        .by_name("missing.bin")
        .unwrap()
        .read_to_end(&mut missing_data)
        .unwrap();

    assert_eq!(missing_data, b"new");
    assert_eq!(missing_child.borrow().rep_status(), RepStatus::Correct);
    assert_eq!(source_file.borrow().rep_status(), RepStatus::Deleted);
    assert_eq!(total_fixed, 1);
}

#[test]
fn test_fix_zip_partial_rebuild_prefers_target_primary_identity_when_target_has_primary_and_alt_md5(
) {
    let temp = tempdir().unwrap();
    let root = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
    root.borrow_mut().name = temp.path().to_string_lossy().to_string();

    let source_dir = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
    {
        let mut dir = source_dir.borrow_mut();
        dir.name = "ToSort".to_string();
        dir.tree_checked = TreeSelect::Selected;
        dir.parent = Some(Rc::downgrade(&root));
    }

    let source_file = Rc::new(RefCell::new(RvFile::new(FileType::File)));
    {
        let mut file = source_file.borrow_mut();
        file.name = "missing.bin".to_string();
        file.size = Some(3);
        file.md5 = Some(vec![0x10, 0x20, 0x30, 0x40]);
        file.alt_size = Some(3);
        file.alt_md5 = Some(vec![0xAA, 0xBB, 0xCC, 0xDD]);
        file.tree_checked = TreeSelect::Selected;
        file.set_rep_status(RepStatus::NeededForFix);
        file.parent = Some(Rc::downgrade(&source_dir));
    }
    source_dir.borrow_mut().child_add(Rc::clone(&source_file));

    let target_archive = Rc::new(RefCell::new(RvFile::new(FileType::Zip)));
    {
        let mut archive = target_archive.borrow_mut();
        archive.name = "target.zip".to_string();
        archive.tree_checked = TreeSelect::Selected;
        archive.parent = Some(Rc::downgrade(&root));
    }

    let missing_child = Rc::new(RefCell::new(RvFile::new(FileType::File)));
    {
        let mut file = missing_child.borrow_mut();
        file.name = "missing.bin".to_string();
        file.size = Some(3);
        file.md5 = Some(vec![0x10, 0x20, 0x30, 0x40]);
        file.alt_size = Some(3);
        file.alt_md5 = Some(vec![0xAA, 0xBB, 0xCC, 0xDD]);
        file.tree_checked = TreeSelect::Selected;
        file.set_dat_got_status(
            dat_reader::enums::DatStatus::InDatCollect,
            GotStatus::NotGot,
        );
        file.set_rep_status(RepStatus::CanBeFixed);
        file.parent = Some(Rc::downgrade(&target_archive));
    }

    target_archive
        .borrow_mut()
        .child_add(Rc::clone(&missing_child));
    root.borrow_mut().child_add(Rc::clone(&source_dir));
    root.borrow_mut().child_add(Rc::clone(&target_archive));

    fs::create_dir_all(temp.path().join("ToSort")).unwrap();
    fs::write(temp.path().join("ToSort").join("missing.bin"), b"new").unwrap();

    let target_path = temp.path().join("target.zip");
    {
        let file = File::create(&target_path).unwrap();
        let writer = ZipWriter::new(file);
        writer.finish().unwrap();
    }

    let mut queue = Vec::new();
    let mut total_fixed = 0;
    let crc_map = HashMap::new();
    let sha1_map = HashMap::new();
    let mut md5_map = HashMap::new();
    md5_map.insert((3, vec![0x10, 0x20, 0x30, 0x40]), Rc::clone(&source_file));
    md5_map.insert((3, vec![0xAA, 0xBB, 0xCC, 0xDD]), Rc::clone(&source_file));

    assert!(Fix::rebuild_zip_archive(
        Rc::clone(&target_archive),
        &mut queue,
        &mut total_fixed,
        &crc_map,
        &sha1_map,
        &md5_map,
    ));
    assert_eq!(queue.len(), 1);
    let mut cleanup_queue = Vec::new();
    let mut cleanup_total_fixed = 0;
    Fix::fix_a_file(
        queue.remove(0),
        &mut cleanup_queue,
        &mut cleanup_total_fixed,
        &crc_map,
        &sha1_map,
        &md5_map,
    );

    let file = File::open(&target_path).unwrap();
    let mut archive = ZipArchive::new(file).unwrap();
    let mut missing_data = Vec::new();
    archive
        .by_name("missing.bin")
        .unwrap()
        .read_to_end(&mut missing_data)
        .unwrap();

    assert_eq!(missing_data, b"new");
    assert_eq!(missing_child.borrow().rep_status(), RepStatus::Correct);
    assert_eq!(source_file.borrow().rep_status(), RepStatus::Deleted);
    assert_eq!(total_fixed, 1);
}

#[test]
fn test_fix_zip_partial_rebuild_removes_consumed_source_entry() {
    let temp = tempdir().unwrap();
    let root = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
    root.borrow_mut().name = temp.path().to_string_lossy().to_string();

    let source_archive = Rc::new(RefCell::new(RvFile::new(FileType::Zip)));
    {
        let mut archive = source_archive.borrow_mut();
        archive.name = "source.zip".to_string();
        archive.tree_checked = TreeSelect::Selected;
        archive.parent = Some(Rc::downgrade(&root));
    }

    let source_keep = Rc::new(RefCell::new(RvFile::new(FileType::File)));
    {
        let mut file = source_keep.borrow_mut();
        file.name = "keep.bin".to_string();
        file.size = Some(4);
        file.tree_checked = TreeSelect::Selected;
        file.set_dat_got_status(dat_reader::enums::DatStatus::InDatCollect, GotStatus::Got);
        file.set_rep_status(RepStatus::Correct);
        file.parent = Some(Rc::downgrade(&source_archive));
    }

    let source_move = Rc::new(RefCell::new(RvFile::new(FileType::File)));
    {
        let mut file = source_move.borrow_mut();
        file.name = "move.bin".to_string();
        file.size = Some(4);
        file.crc = Some(vec![0x00, 0x00, 0x00, 0x04]);
        file.tree_checked = TreeSelect::Selected;
        file.set_rep_status(RepStatus::NeededForFix);
        file.parent = Some(Rc::downgrade(&source_archive));
    }

    source_archive
        .borrow_mut()
        .child_add(Rc::clone(&source_keep));
    source_archive
        .borrow_mut()
        .child_add(Rc::clone(&source_move));

    let target_archive = Rc::new(RefCell::new(RvFile::new(FileType::Zip)));
    {
        let mut archive = target_archive.borrow_mut();
        archive.name = "target.zip".to_string();
        archive.tree_checked = TreeSelect::Selected;
        archive.parent = Some(Rc::downgrade(&root));
    }

    let target_child = Rc::new(RefCell::new(RvFile::new(FileType::File)));
    {
        let mut file = target_child.borrow_mut();
        file.name = "move.bin".to_string();
        file.size = Some(4);
        file.crc = Some(vec![0x00, 0x00, 0x00, 0x04]);
        file.tree_checked = TreeSelect::Selected;
        file.set_dat_got_status(
            dat_reader::enums::DatStatus::InDatCollect,
            GotStatus::NotGot,
        );
        file.set_rep_status(RepStatus::CanBeFixed);
        file.parent = Some(Rc::downgrade(&target_archive));
    }

    target_archive
        .borrow_mut()
        .child_add(Rc::clone(&target_child));

    root.borrow_mut().child_add(Rc::clone(&source_archive));
    root.borrow_mut().child_add(Rc::clone(&target_archive));

    let source_path = temp.path().join("source.zip");
    {
        let file = File::create(&source_path).unwrap();
        let mut writer = ZipWriter::new(file);
        writer
            .start_file("keep.bin", SimpleFileOptions::default())
            .unwrap();
        writer.write_all(b"keep").unwrap();
        writer
            .start_file("move.bin", SimpleFileOptions::default())
            .unwrap();
        writer.write_all(b"move").unwrap();
        writer.finish().unwrap();
    }

    let mut queue = Vec::new();
    let mut total_fixed = 0;
    let mut crc_map = HashMap::new();
    crc_map.insert((4, vec![0x00, 0x00, 0x00, 0x04]), Rc::clone(&source_move));
    let sha1_map = HashMap::new();
    let md5_map = HashMap::new();

    assert!(Fix::rebuild_zip_archive(
        Rc::clone(&target_archive),
        &mut queue,
        &mut total_fixed,
        &crc_map,
        &sha1_map,
        &md5_map,
    ));
    assert_eq!(queue.len(), 1);
    let mut cleanup_queue = Vec::new();
    let mut cleanup_total_fixed = 0;
    Fix::fix_a_zip(
        queue.remove(0),
        &mut cleanup_queue,
        &mut cleanup_total_fixed,
        &crc_map,
        &sha1_map,
        &md5_map,
    );

    let target_path = temp.path().join("target.zip");
    let file = File::open(&target_path).unwrap();
    let mut target_zip = ZipArchive::new(file).unwrap();
    let mut moved_data = Vec::new();
    target_zip
        .by_name("move.bin")
        .unwrap()
        .read_to_end(&mut moved_data)
        .unwrap();
    assert_eq!(moved_data, b"move");

    let file = File::open(&source_path).unwrap();
    let mut source_zip = ZipArchive::new(file).unwrap();
    assert!(source_zip.by_name("move.bin").is_err());
    let mut kept_data = Vec::new();
    source_zip
        .by_name("keep.bin")
        .unwrap()
        .read_to_end(&mut kept_data)
        .unwrap();
    assert_eq!(kept_data, b"keep");
    assert_eq!(source_move.borrow().rep_status(), RepStatus::Deleted);
    assert_eq!(total_fixed, 1);
}

#[test]
fn test_fix_zip_partial_rebuild_renames_existing_entry() {
    let temp = tempdir().unwrap();
    let root = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
    root.borrow_mut().name = temp.path().to_string_lossy().to_string();

    let target_archive = Rc::new(RefCell::new(RvFile::new(FileType::Zip)));
    {
        let mut archive = target_archive.borrow_mut();
        archive.name = "target.zip".to_string();
        archive.tree_checked = TreeSelect::Selected;
        archive.parent = Some(Rc::downgrade(&root));
    }

    let renamed_child = Rc::new(RefCell::new(RvFile::new(FileType::File)));
    {
        let mut file = renamed_child.borrow_mut();
        file.name = "new.bin".to_string();
        file.file_name = "old.bin".to_string();
        file.size = Some(4);
        file.tree_checked = TreeSelect::Selected;
        file.set_dat_got_status(dat_reader::enums::DatStatus::InDatCollect, GotStatus::Got);
        file.set_rep_status(RepStatus::Rename);
        file.parent = Some(Rc::downgrade(&target_archive));
    }

    target_archive
        .borrow_mut()
        .child_add(Rc::clone(&renamed_child));
    root.borrow_mut().child_add(Rc::clone(&target_archive));

    let target_path = temp.path().join("target.zip");
    {
        let file = File::create(&target_path).unwrap();
        let mut writer = ZipWriter::new(file);
        writer
            .start_file("old.bin", SimpleFileOptions::default())
            .unwrap();
        writer.write_all(b"data").unwrap();
        writer.finish().unwrap();
    }

    let mut queue = Vec::new();
    let mut total_fixed = 0;
    let crc_map = HashMap::new();
    let sha1_map = HashMap::new();
    let md5_map = HashMap::new();

    assert!(Fix::rebuild_zip_archive(
        Rc::clone(&target_archive),
        &mut queue,
        &mut total_fixed,
        &crc_map,
        &sha1_map,
        &md5_map,
    ));

    let file = File::open(&target_path).unwrap();
    let mut archive = ZipArchive::new(file).unwrap();
    let mut data = Vec::new();
    archive
        .by_name("new.bin")
        .unwrap()
        .read_to_end(&mut data)
        .unwrap();
    assert!(archive.by_name("old.bin").is_err());
    assert_eq!(data, b"data");
    assert_eq!(renamed_child.borrow().rep_status(), RepStatus::Correct);
    assert_eq!(renamed_child.borrow().file_name, "new.bin");
}
#[test]
fn test_fix_zip_partial_rebuild_rename_preserves_correctmia() {
    let temp = tempdir().unwrap();
    let root = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
    root.borrow_mut().name = temp.path().to_string_lossy().to_string();

    let target_archive = Rc::new(RefCell::new(RvFile::new(FileType::Zip)));
    {
        let mut archive = target_archive.borrow_mut();
        archive.name = "target.zip".to_string();
        archive.tree_checked = TreeSelect::Selected;
        archive.set_dat_status(dat_reader::enums::DatStatus::InDatMIA);
        archive.parent = Some(Rc::downgrade(&root));
    }

    let renamed_child = Rc::new(RefCell::new(RvFile::new(FileType::File)));
    {
        let mut file = renamed_child.borrow_mut();
        file.name = "new.bin".to_string();
        file.file_name = "old.bin".to_string();
        file.size = Some(4);
        file.tree_checked = TreeSelect::Selected;
        file.set_dat_got_status(dat_reader::enums::DatStatus::InDatMIA, GotStatus::Got);
        file.set_rep_status(RepStatus::Rename);
        file.parent = Some(Rc::downgrade(&target_archive));
    }

    target_archive
        .borrow_mut()
        .child_add(Rc::clone(&renamed_child));
    root.borrow_mut().child_add(Rc::clone(&target_archive));

    let target_path = temp.path().join("target.zip");
    {
        let file = File::create(&target_path).unwrap();
        let mut writer = ZipWriter::new(file);
        writer
            .start_file("old.bin", SimpleFileOptions::default())
            .unwrap();
        writer.write_all(b"data").unwrap();
        writer.finish().unwrap();
    }

    let mut queue = Vec::new();
    let mut total_fixed = 0;
    let crc_map = HashMap::new();
    let sha1_map = HashMap::new();
    let md5_map = HashMap::new();

    assert!(Fix::rebuild_zip_archive(
        Rc::clone(&target_archive),
        &mut queue,
        &mut total_fixed,
        &crc_map,
        &sha1_map,
        &md5_map,
    ));

    assert_eq!(renamed_child.borrow().rep_status(), RepStatus::CorrectMIA);
    assert_eq!(renamed_child.borrow().file_name, "new.bin");
    assert_eq!(target_archive.borrow().rep_status(), RepStatus::CorrectMIA);
}

#[test]
fn test_fix_zip_partial_rebuild_marks_moved_entry_in_tosort() {
    let temp = tempdir().unwrap();
    let root = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
    root.borrow_mut().name = temp.path().to_string_lossy().to_string();

    let target_archive = Rc::new(RefCell::new(RvFile::new(FileType::Zip)));
    {
        let mut archive = target_archive.borrow_mut();
        archive.name = "target.zip".to_string();
        archive.tree_checked = TreeSelect::Selected;
        archive.parent = Some(Rc::downgrade(&root));
    }

    let moved_child = Rc::new(RefCell::new(RvFile::new(FileType::File)));
    {
        let mut file = moved_child.borrow_mut();
        file.name = "move.bin".to_string();
        file.size = Some(4);
        file.tree_checked = TreeSelect::Selected;
        file.set_dat_got_status(dat_reader::enums::DatStatus::InDatCollect, GotStatus::Got);
        file.set_rep_status(RepStatus::MoveToSort);
        file.parent = Some(Rc::downgrade(&target_archive));
    }

    target_archive
        .borrow_mut()
        .child_add(Rc::clone(&moved_child));
    root.borrow_mut().child_add(Rc::clone(&target_archive));

    let target_path = temp.path().join("target.zip");
    {
        let file = File::create(&target_path).unwrap();
        let mut writer = ZipWriter::new(file);
        writer
            .start_file("move.bin", SimpleFileOptions::default())
            .unwrap();
        writer.write_all(b"data").unwrap();
        writer.finish().unwrap();
    }

    let mut queue = Vec::new();
    let mut total_fixed = 0;
    let crc_map = HashMap::new();
    let sha1_map = HashMap::new();
    let md5_map = HashMap::new();

    assert!(Fix::rebuild_zip_archive(
        Rc::clone(&target_archive),
        &mut queue,
        &mut total_fixed,
        &crc_map,
        &sha1_map,
        &md5_map,
    ));

    let moved_path = Fix::get_archive_member_tosort_path(&target_path, "move.bin", "ToSort");
    assert!(moved_path.exists());
    assert_eq!(fs::read(&moved_path).unwrap(), b"data");
    assert!(!target_path.exists());
    assert_eq!(moved_child.borrow().rep_status(), RepStatus::InToSort);
    assert_eq!(moved_child.borrow().got_status(), GotStatus::Got);
    assert_eq!(
        moved_child.borrow().dat_status(),
        dat_reader::enums::DatStatus::InToSort
    );
    moved_child.borrow_mut().rep_status_reset();
    assert_eq!(moved_child.borrow().rep_status(), RepStatus::InToSort);
    assert_eq!(target_archive.borrow().rep_status(), RepStatus::Deleted);
    assert_eq!(target_archive.borrow().got_status(), GotStatus::NotGot);
}

#[test]
fn test_fix_zip_partial_rebuild_marks_removed_entry_missing_and_clears_got_when_archive_remains() {
    let temp = tempdir().unwrap();
    let root = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
    root.borrow_mut().name = temp.path().to_string_lossy().to_string();

    let target_archive = Rc::new(RefCell::new(RvFile::new(FileType::Zip)));
    {
        let mut archive = target_archive.borrow_mut();
        archive.name = "target.zip".to_string();
        archive.tree_checked = TreeSelect::Selected;
        archive.set_dat_status(dat_reader::enums::DatStatus::InDatCollect);
        archive.parent = Some(Rc::downgrade(&root));
    }

    let keep_child = Rc::new(RefCell::new(RvFile::new(FileType::File)));
    {
        let mut file = keep_child.borrow_mut();
        file.name = "keep.bin".to_string();
        file.file_name = "keep.bin".to_string();
        file.size = Some(4);
        file.tree_checked = TreeSelect::Selected;
        file.set_dat_got_status(dat_reader::enums::DatStatus::InDatCollect, GotStatus::Got);
        file.parent = Some(Rc::downgrade(&target_archive));
    }

    let moved_child = Rc::new(RefCell::new(RvFile::new(FileType::File)));
    {
        let mut file = moved_child.borrow_mut();
        file.name = "bad.bin".to_string();
        file.file_name = "bad.bin".to_string();
        file.size = Some(4);
        file.tree_checked = TreeSelect::Selected;
        file.set_dat_got_status(dat_reader::enums::DatStatus::InDatCollect, GotStatus::Got);
        file.set_rep_status(RepStatus::MoveToCorrupt);
        file.parent = Some(Rc::downgrade(&target_archive));
    }

    target_archive
        .borrow_mut()
        .child_add(Rc::clone(&keep_child));
    target_archive
        .borrow_mut()
        .child_add(Rc::clone(&moved_child));
    root.borrow_mut().child_add(Rc::clone(&target_archive));

    let target_path = temp.path().join("target.zip");
    {
        let file = File::create(&target_path).unwrap();
        let mut writer = ZipWriter::new(file);
        writer
            .start_file("keep.bin", SimpleFileOptions::default())
            .unwrap();
        writer.write_all(b"keep").unwrap();
        writer
            .start_file("bad.bin", SimpleFileOptions::default())
            .unwrap();
        writer.write_all(b"data").unwrap();
        writer.finish().unwrap();
    }

    let mut queue = Vec::new();
    let mut total_fixed = 0;
    let crc_map = HashMap::new();
    let sha1_map = HashMap::new();
    let md5_map = HashMap::new();

    assert!(Fix::rebuild_zip_archive(
        Rc::clone(&target_archive),
        &mut queue,
        &mut total_fixed,
        &crc_map,
        &sha1_map,
        &md5_map,
    ));

    let moved_path = Fix::get_archive_member_tosort_path(&target_path, "bad.bin", "ToSort/Corrupt");
    assert!(moved_path.exists());
    assert_eq!(fs::read(&moved_path).unwrap(), b"data");
    assert_eq!(
        Fix::read_zip_entry_bytes(&target_path.to_string_lossy(), "keep.bin").unwrap(),
        b"keep"
    );
    assert!(Fix::read_zip_entry_bytes(&target_path.to_string_lossy(), "bad.bin").is_none());
    assert_eq!(moved_child.borrow().rep_status(), RepStatus::Missing);
    assert_eq!(moved_child.borrow().got_status(), GotStatus::NotGot);
    assert_eq!(target_archive.borrow().got_status(), GotStatus::Got);
}

#[test]
fn test_fix_zip_partial_rebuild_sorts_entries_for_torrentzip() {
    let temp = tempdir().unwrap();
    let root = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
    root.borrow_mut().name = temp.path().to_string_lossy().to_string();

    let source_dir = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
    {
        let mut dir = source_dir.borrow_mut();
        dir.name = "ToSort".to_string();
        dir.tree_checked = TreeSelect::Selected;
        dir.parent = Some(Rc::downgrade(&root));
    }

    let source_file = Rc::new(RefCell::new(RvFile::new(FileType::File)));
    {
        let mut file = source_file.borrow_mut();
        file.name = "a.bin".to_string();
        file.size = Some(1);
        file.crc = Some(vec![0x00, 0x00, 0x00, 0x01]);
        file.tree_checked = TreeSelect::Selected;
        file.set_rep_status(RepStatus::NeededForFix);
        file.parent = Some(Rc::downgrade(&source_dir));
    }
    source_dir.borrow_mut().child_add(Rc::clone(&source_file));

    let target_archive = Rc::new(RefCell::new(RvFile::new(FileType::Zip)));
    {
        let mut archive = target_archive.borrow_mut();
        archive.name = "target.zip".to_string();
        archive.tree_checked = TreeSelect::Selected;
        archive.set_dat_status(dat_reader::enums::DatStatus::InDatCollect);
        archive.set_zip_dat_struct(ZipStructure::ZipTrrnt, true);
        archive.parent = Some(Rc::downgrade(&root));
    }

    let b_child = Rc::new(RefCell::new(RvFile::new(FileType::File)));
    {
        let mut file = b_child.borrow_mut();
        file.name = "b.bin".to_string();
        file.size = Some(1);
        file.tree_checked = TreeSelect::Selected;
        file.set_dat_got_status(dat_reader::enums::DatStatus::InDatCollect, GotStatus::Got);
        file.set_rep_status(RepStatus::Correct);
        file.parent = Some(Rc::downgrade(&target_archive));
    }

    let a_child = Rc::new(RefCell::new(RvFile::new(FileType::File)));
    {
        let mut file = a_child.borrow_mut();
        file.name = "a.bin".to_string();
        file.size = Some(1);
        file.crc = Some(vec![0x00, 0x00, 0x00, 0x01]);
        file.tree_checked = TreeSelect::Selected;
        file.set_dat_got_status(
            dat_reader::enums::DatStatus::InDatCollect,
            GotStatus::NotGot,
        );
        file.set_rep_status(RepStatus::CanBeFixed);
        file.parent = Some(Rc::downgrade(&target_archive));
    }

    target_archive.borrow_mut().child_add(Rc::clone(&b_child));
    target_archive.borrow_mut().child_add(Rc::clone(&a_child));

    root.borrow_mut().child_add(Rc::clone(&source_dir));
    root.borrow_mut().child_add(Rc::clone(&target_archive));

    fs::create_dir_all(temp.path().join("ToSort")).unwrap();
    fs::write(temp.path().join("ToSort").join("a.bin"), b"a").unwrap();

    let target_path = temp.path().join("target.zip");
    {
        let file = File::create(&target_path).unwrap();
        let mut writer = ZipWriter::new(file);
        writer
            .start_file("b.bin", SimpleFileOptions::default())
            .unwrap();
        writer.write_all(b"b").unwrap();
        writer.finish().unwrap();
    }

    let mut queue = Vec::new();
    let mut total_fixed = 0;
    let mut crc_map = HashMap::new();
    crc_map.insert((1, vec![0x00, 0x00, 0x00, 0x01]), Rc::clone(&source_file));
    let sha1_map = HashMap::new();
    let md5_map = HashMap::new();

    assert!(Fix::rebuild_zip_archive(
        Rc::clone(&target_archive),
        &mut queue,
        &mut total_fixed,
        &crc_map,
        &sha1_map,
        &md5_map,
    ));

    let file = File::open(&target_path).unwrap();
    let mut archive = ZipArchive::new(file).unwrap();
    {
        let entry0 = archive.by_index(0).unwrap();
        assert_eq!(entry0.name(), "a.bin");
        let dt0 = entry0.last_modified().unwrap();
        assert_eq!(dt0.year(), 1996);
        assert_eq!(dt0.month(), 12);
        assert_eq!(dt0.day(), 24);
        assert_eq!(dt0.hour(), 23);
        assert_eq!(dt0.minute(), 32);
        assert_eq!(dt0.second(), 0);
    }
    assert_eq!(archive.by_index(1).unwrap().name(), "b.bin");
    let zip_bytes = fs::read(&target_path).unwrap();
    assert_eq!(&zip_bytes[0..4], &[0x50, 0x4B, 0x03, 0x04]);
    assert_eq!(u16::from_le_bytes([zip_bytes[4], zip_bytes[5]]), 20);
    assert_eq!(u16::from_le_bytes([zip_bytes[6], zip_bytes[7]]), 2);
    assert_eq!(u16::from_le_bytes([zip_bytes[8], zip_bytes[9]]), 8);
    assert_eq!(
        u16::from_le_bytes([zip_bytes[10], zip_bytes[11]]),
        Fix::TORRENTZIP_DOS_TIME
    );
    assert_eq!(
        u16::from_le_bytes([zip_bytes[12], zip_bytes[13]]),
        Fix::TORRENTZIP_DOS_DATE
    );
    let eocd_offset = zip_bytes
        .windows(4)
        .rposition(|window| window == [0x50, 0x4B, 0x05, 0x06])
        .unwrap();
    let central_directory_size = u32::from_le_bytes([
        zip_bytes[eocd_offset + 12],
        zip_bytes[eocd_offset + 13],
        zip_bytes[eocd_offset + 14],
        zip_bytes[eocd_offset + 15],
    ]) as usize;
    let central_directory_offset = u32::from_le_bytes([
        zip_bytes[eocd_offset + 16],
        zip_bytes[eocd_offset + 17],
        zip_bytes[eocd_offset + 18],
        zip_bytes[eocd_offset + 19],
    ]) as usize;
    let mut crc_hasher = crc32fast::Hasher::new();
    crc_hasher.update(
        &zip_bytes[central_directory_offset..central_directory_offset + central_directory_size],
    );
    let expected_comment = format!("TORRENTZIPPED-{:08X}", crc_hasher.finalize());
    assert_eq!(String::from_utf8_lossy(archive.comment()), expected_comment);
    assert_eq!(
        &zip_bytes[central_directory_offset..central_directory_offset + 4],
        &[0x50, 0x4B, 0x01, 0x02]
    );
    assert_eq!(
        u16::from_le_bytes([
            zip_bytes[central_directory_offset + 4],
            zip_bytes[central_directory_offset + 5],
        ]),
        0
    );
    assert_eq!(
        u16::from_le_bytes([
            zip_bytes[central_directory_offset + 6],
            zip_bytes[central_directory_offset + 7],
        ]),
        20
    );
    assert_eq!(
        u16::from_le_bytes([
            zip_bytes[central_directory_offset + 8],
            zip_bytes[central_directory_offset + 9],
        ]),
        2
    );
    assert_eq!(
        u16::from_le_bytes([
            zip_bytes[central_directory_offset + 10],
            zip_bytes[central_directory_offset + 11],
        ]),
        8
    );
    assert_eq!(target_archive.borrow().zip_struct, ZipStructure::ZipTrrnt);
}

#[test]
fn test_fix_torrentzip_rebuild_preserves_existing_raw_streams() {
    let temp = tempdir().unwrap();
    let root = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
    root.borrow_mut().name = temp.path().to_string_lossy().to_string();

    let source_dir = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
    {
        let mut dir = source_dir.borrow_mut();
        dir.name = "ToSort".to_string();
        dir.tree_checked = TreeSelect::Selected;
        dir.parent = Some(Rc::downgrade(&root));
    }

    let source_file = Rc::new(RefCell::new(RvFile::new(FileType::File)));
    {
        let mut file = source_file.borrow_mut();
        file.name = "a.bin".to_string();
        file.size = Some(4);
        file.crc = Some(vec![0x00, 0x00, 0x00, 0x04]);
        file.tree_checked = TreeSelect::Selected;
        file.set_rep_status(RepStatus::NeededForFix);
        file.parent = Some(Rc::downgrade(&source_dir));
    }
    source_dir.borrow_mut().child_add(Rc::clone(&source_file));

    let target_archive = Rc::new(RefCell::new(RvFile::new(FileType::Zip)));
    {
        let mut archive = target_archive.borrow_mut();
        archive.name = "target.zip".to_string();
        archive.tree_checked = TreeSelect::Selected;
        archive.zip_struct = ZipStructure::ZipTrrnt;
        archive.set_dat_status(dat_reader::enums::DatStatus::InDatCollect);
        archive.set_zip_dat_struct(ZipStructure::ZipTrrnt, true);
        archive.parent = Some(Rc::downgrade(&root));
    }

    let keep_child = Rc::new(RefCell::new(RvFile::new(FileType::File)));
    {
        let mut file = keep_child.borrow_mut();
        file.name = "b.bin".to_string();
        file.size = Some(4);
        file.tree_checked = TreeSelect::Selected;
        file.set_dat_got_status(dat_reader::enums::DatStatus::InDatCollect, GotStatus::Got);
        file.set_rep_status(RepStatus::Correct);
        file.parent = Some(Rc::downgrade(&target_archive));
    }

    let missing_child = Rc::new(RefCell::new(RvFile::new(FileType::File)));
    {
        let mut file = missing_child.borrow_mut();
        file.name = "a.bin".to_string();
        file.size = Some(4);
        file.crc = Some(vec![0x00, 0x00, 0x00, 0x04]);
        file.tree_checked = TreeSelect::Selected;
        file.set_dat_got_status(
            dat_reader::enums::DatStatus::InDatCollect,
            GotStatus::NotGot,
        );
        file.set_rep_status(RepStatus::CanBeFixed);
        file.parent = Some(Rc::downgrade(&target_archive));
    }

    target_archive
        .borrow_mut()
        .child_add(Rc::clone(&keep_child));
    target_archive
        .borrow_mut()
        .child_add(Rc::clone(&missing_child));

    root.borrow_mut().child_add(Rc::clone(&source_dir));
    root.borrow_mut().child_add(Rc::clone(&target_archive));

    fs::create_dir_all(temp.path().join("ToSort")).unwrap();
    fs::write(temp.path().join("ToSort").join("a.bin"), b"aaaa").unwrap();

    let target_path = temp.path().join("target.zip");
    let initial_bytes =
        Fix::build_torrentzip_archive(&[Fix::compress_torrentzip_entry("b.bin", b"bbbb").unwrap()])
            .unwrap();
    fs::write(&target_path, initial_bytes).unwrap();

    let before = Fix::read_raw_zip_entry(&target_path.to_string_lossy(), "b.bin").unwrap();

    let mut queue = Vec::new();
    let mut total_fixed = 0;
    let mut crc_map = HashMap::new();
    crc_map.insert((4, vec![0x00, 0x00, 0x00, 0x04]), Rc::clone(&source_file));
    let sha1_map = HashMap::new();
    let md5_map = HashMap::new();

    assert!(Fix::rebuild_zip_archive(
        Rc::clone(&target_archive),
        &mut queue,
        &mut total_fixed,
        &crc_map,
        &sha1_map,
        &md5_map,
    ));

    let after = Fix::read_raw_zip_entry(&target_path.to_string_lossy(), "b.bin").unwrap();
    assert_eq!(before.compressed_data, after.compressed_data);
    assert_eq!(before.crc, after.crc);
}

#[test]
fn test_fix_torrentzip_rebuild_reuses_deflate_stream_from_standard_zip_source() {
    let temp = tempdir().unwrap();
    let root = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
    root.borrow_mut().name = temp.path().to_string_lossy().to_string();

    let source_archive = Rc::new(RefCell::new(RvFile::new(FileType::Zip)));
    {
        let mut archive = source_archive.borrow_mut();
        archive.name = "source.zip".to_string();
        archive.tree_checked = TreeSelect::Selected;
        archive.parent = Some(Rc::downgrade(&root));
    }

    let source_child = Rc::new(RefCell::new(RvFile::new(FileType::File)));
    {
        let mut file = source_child.borrow_mut();
        file.name = "a.bin".to_string();
        file.size = Some(6);
        file.crc = Some(vec![0x00, 0x00, 0x00, 0x06]);
        file.tree_checked = TreeSelect::Selected;
        file.set_rep_status(RepStatus::NeededForFix);
        file.parent = Some(Rc::downgrade(&source_archive));
    }
    source_archive
        .borrow_mut()
        .child_add(Rc::clone(&source_child));

    let target_archive = Rc::new(RefCell::new(RvFile::new(FileType::Zip)));
    {
        let mut archive = target_archive.borrow_mut();
        archive.name = "target.zip".to_string();
        archive.tree_checked = TreeSelect::Selected;
        archive.set_dat_status(dat_reader::enums::DatStatus::InDatCollect);
        archive.set_zip_dat_struct(ZipStructure::ZipTrrnt, true);
        archive.parent = Some(Rc::downgrade(&root));
    }

    let target_child = Rc::new(RefCell::new(RvFile::new(FileType::File)));
    {
        let mut file = target_child.borrow_mut();
        file.name = "a.bin".to_string();
        file.size = Some(6);
        file.crc = Some(vec![0x00, 0x00, 0x00, 0x06]);
        file.tree_checked = TreeSelect::Selected;
        file.set_dat_got_status(
            dat_reader::enums::DatStatus::InDatCollect,
            GotStatus::NotGot,
        );
        file.set_rep_status(RepStatus::CanBeFixed);
        file.parent = Some(Rc::downgrade(&target_archive));
    }
    target_archive
        .borrow_mut()
        .child_add(Rc::clone(&target_child));

    root.borrow_mut().child_add(Rc::clone(&source_archive));
    root.borrow_mut().child_add(Rc::clone(&target_archive));

    let source_path = temp.path().join("source.zip");
    {
        let file = File::create(&source_path).unwrap();
        let mut writer = ZipWriter::new(file);
        let options = SimpleFileOptions::default()
            .compression_method(CompressionMethod::Deflated)
            .compression_level(Some(9));
        writer.start_file("a.bin", options).unwrap();
        writer.write_all(b"aaaaaa").unwrap();
        writer.finish().unwrap();
    }

    let source_raw = Fix::read_raw_zip_entry(&source_path.to_string_lossy(), "a.bin").unwrap();

    let mut queue = Vec::new();
    let mut total_fixed = 0;
    let mut crc_map = HashMap::new();
    crc_map.insert((6, vec![0x00, 0x00, 0x00, 0x06]), Rc::clone(&source_child));
    let sha1_map = HashMap::new();
    let md5_map = HashMap::new();

    assert!(Fix::rebuild_zip_archive(
        Rc::clone(&target_archive),
        &mut queue,
        &mut total_fixed,
        &crc_map,
        &sha1_map,
        &md5_map,
    ));

    let target_path = temp.path().join("target.zip");
    let target_raw = Fix::read_raw_zip_entry(&target_path.to_string_lossy(), "a.bin").unwrap();
    assert_eq!(source_raw.compressed_data, target_raw.compressed_data);
    assert_eq!(source_raw.crc, target_raw.crc);
    assert_eq!(source_raw.uncompressed_size, target_raw.uncompressed_size);
}

#[test]
fn test_fix_zip_rebuild_runs_for_structure_only_change() {
    let temp = tempdir().unwrap();
    let root = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
    root.borrow_mut().name = temp.path().to_string_lossy().to_string();

    let target_archive = Rc::new(RefCell::new(RvFile::new(FileType::Zip)));
    {
        let mut archive = target_archive.borrow_mut();
        archive.name = "target.zip".to_string();
        archive.tree_checked = TreeSelect::Selected;
        archive.zip_struct = ZipStructure::ZipTDC;
        archive.set_dat_status(dat_reader::enums::DatStatus::InDatCollect);
        archive.set_zip_dat_struct(ZipStructure::ZipTrrnt, true);
        archive.parent = Some(Rc::downgrade(&root));
    }

    let target_child = Rc::new(RefCell::new(RvFile::new(FileType::File)));
    {
        let mut file = target_child.borrow_mut();
        file.name = "a.bin".to_string();
        file.size = Some(6);
        file.crc = Some(vec![0x00, 0x00, 0x00, 0x06]);
        file.tree_checked = TreeSelect::Selected;
        file.set_dat_got_status(dat_reader::enums::DatStatus::InDatCollect, GotStatus::Got);
        file.set_rep_status(RepStatus::Correct);
        file.parent = Some(Rc::downgrade(&target_archive));
    }
    target_archive
        .borrow_mut()
        .child_add(Rc::clone(&target_child));
    root.borrow_mut().child_add(Rc::clone(&target_archive));

    let target_path = temp.path().join("target.zip");
    {
        let file = File::create(&target_path).unwrap();
        let mut writer = ZipWriter::new(file);
        let options = SimpleFileOptions::default()
            .compression_method(CompressionMethod::Deflated)
            .compression_level(Some(9));
        writer.start_file("a.bin", options).unwrap();
        writer.write_all(b"aaaaaa").unwrap();
        writer.finish().unwrap();
    }

    let mut queue = Vec::new();
    let mut total_fixed = 0;
    let crc_map = HashMap::new();
    let sha1_map = HashMap::new();
    let md5_map = HashMap::new();

    assert!(Fix::rebuild_zip_archive(
        Rc::clone(&target_archive),
        &mut queue,
        &mut total_fixed,
        &crc_map,
        &sha1_map,
        &md5_map,
    ));
    assert!(Fix::read_raw_zip_entry(&target_path.to_string_lossy(), "a.bin").is_some());
    assert_eq!(target_archive.borrow().zip_struct, ZipStructure::ZipTrrnt);
    assert_eq!(target_archive.borrow().rep_status(), RepStatus::Correct);
    assert_eq!(target_archive.borrow().got_status(), GotStatus::Got);
}

#[test]
fn test_fix_zip_rebuild_supports_nested_directory_members() {
    let temp = tempdir().unwrap();
    let root = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
    root.borrow_mut().name = temp.path().to_string_lossy().to_string();

    let target_archive = Rc::new(RefCell::new(RvFile::new(FileType::Zip)));
    {
        let mut archive = target_archive.borrow_mut();
        archive.name = "target.zip".to_string();
        archive.tree_checked = TreeSelect::Selected;
        archive.zip_struct = ZipStructure::ZipTDC;
        archive.set_dat_status(dat_reader::enums::DatStatus::InDatCollect);
        archive.set_zip_dat_struct(ZipStructure::ZipTrrnt, true);
        archive.parent = Some(Rc::downgrade(&root));
    }

    let folder = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
    {
        let mut dir = folder.borrow_mut();
        dir.name = "sub".to_string();
        dir.tree_checked = TreeSelect::Selected;
        dir.parent = Some(Rc::downgrade(&target_archive));
    }

    let target_child = Rc::new(RefCell::new(RvFile::new(FileType::File)));
    {
        let mut file = target_child.borrow_mut();
        file.name = "a.bin".to_string();
        file.size = Some(4);
        file.tree_checked = TreeSelect::Selected;
        file.set_dat_got_status(dat_reader::enums::DatStatus::InDatCollect, GotStatus::Got);
        file.set_rep_status(RepStatus::Correct);
        file.parent = Some(Rc::downgrade(&folder));
    }

    folder.borrow_mut().child_add(Rc::clone(&target_child));
    target_archive.borrow_mut().child_add(Rc::clone(&folder));
    root.borrow_mut().child_add(Rc::clone(&target_archive));

    let target_path = temp.path().join("target.zip");
    {
        let file = File::create(&target_path).unwrap();
        let mut writer = ZipWriter::new(file);
        writer
            .start_file("sub/a.bin", SimpleFileOptions::default())
            .unwrap();
        writer.write_all(b"data").unwrap();
        writer.finish().unwrap();
    }

    let mut queue = Vec::new();
    let mut total_fixed = 0;
    let crc_map = HashMap::new();
    let sha1_map = HashMap::new();
    let md5_map = HashMap::new();

    assert!(Fix::rebuild_zip_archive(
        Rc::clone(&target_archive),
        &mut queue,
        &mut total_fixed,
        &crc_map,
        &sha1_map,
        &md5_map,
    ));

    let file = File::open(&target_path).unwrap();
    let mut archive = ZipArchive::new(file).unwrap();
    let mut data = Vec::new();
    archive
        .by_name("sub/a.bin")
        .unwrap()
        .read_to_end(&mut data)
        .unwrap();
    assert_eq!(data, b"data");
    assert_eq!(target_child.borrow().rep_status(), RepStatus::Correct);
    assert_eq!(target_archive.borrow().rep_status(), RepStatus::Correct);
}

#[test]
fn test_fix_sevenzip_partial_rebuild_preserves_existing_and_adds_missing() {
    let temp = tempdir().unwrap();
    let root = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
    root.borrow_mut().name = temp.path().to_string_lossy().to_string();

    let source_dir = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
    {
        let mut dir = source_dir.borrow_mut();
        dir.name = "ToSort".to_string();
        dir.tree_checked = TreeSelect::Selected;
        dir.parent = Some(Rc::downgrade(&root));
    }

    let source_file = Rc::new(RefCell::new(RvFile::new(FileType::File)));
    {
        let mut file = source_file.borrow_mut();
        file.name = "missing.bin".to_string();
        file.size = Some(3);
        file.crc = Some(vec![0x00, 0x00, 0x00, 0x03]);
        file.tree_checked = TreeSelect::Selected;
        file.set_rep_status(RepStatus::NeededForFix);
        file.parent = Some(Rc::downgrade(&source_dir));
    }
    source_dir.borrow_mut().child_add(Rc::clone(&source_file));

    let target_archive = Rc::new(RefCell::new(RvFile::new(FileType::SevenZip)));
    {
        let mut archive = target_archive.borrow_mut();
        archive.name = "target.7z".to_string();
        archive.tree_checked = TreeSelect::Selected;
        archive.set_dat_status(dat_reader::enums::DatStatus::InDatMIA);
        archive.set_zip_dat_struct(ZipStructure::SevenZipSLZMA, true);
        archive.parent = Some(Rc::downgrade(&root));
    }

    let keep_child = Rc::new(RefCell::new(RvFile::new(FileType::File)));
    {
        let mut file = keep_child.borrow_mut();
        file.name = "keep.bin".to_string();
        file.size = Some(4);
        file.tree_checked = TreeSelect::Selected;
        file.set_dat_got_status(dat_reader::enums::DatStatus::InDatCollect, GotStatus::Got);
        file.set_rep_status(RepStatus::Correct);
        file.parent = Some(Rc::downgrade(&target_archive));
    }

    let missing_child = Rc::new(RefCell::new(RvFile::new(FileType::File)));
    {
        let mut file = missing_child.borrow_mut();
        file.name = "missing.bin".to_string();
        file.size = Some(3);
        file.crc = Some(vec![0x00, 0x00, 0x00, 0x03]);
        file.tree_checked = TreeSelect::Selected;
        file.set_dat_got_status(
            dat_reader::enums::DatStatus::InDatCollect,
            GotStatus::NotGot,
        );
        file.set_rep_status(RepStatus::CanBeFixed);
        file.parent = Some(Rc::downgrade(&target_archive));
    }

    target_archive
        .borrow_mut()
        .child_add(Rc::clone(&keep_child));
    target_archive
        .borrow_mut()
        .child_add(Rc::clone(&missing_child));

    root.borrow_mut().child_add(Rc::clone(&source_dir));
    root.borrow_mut().child_add(Rc::clone(&target_archive));

    fs::create_dir_all(temp.path().join("ToSort")).unwrap();
    fs::write(temp.path().join("ToSort").join("missing.bin"), b"new").unwrap();

    let stage_dir = temp.path().join("stage_7z");
    fs::create_dir_all(&stage_dir).unwrap();
    fs::write(stage_dir.join("keep.bin"), b"keep").unwrap();
    let target_path = temp.path().join("target.7z");
    sevenz_rust::compress_to_path(&stage_dir, &target_path).unwrap();

    let mut queue = Vec::new();
    let mut total_fixed = 0;
    let mut crc_map = HashMap::new();
    crc_map.insert((3, vec![0x00, 0x00, 0x00, 0x03]), Rc::clone(&source_file));
    let sha1_map = HashMap::new();
    let md5_map = HashMap::new();

    assert!(Fix::rebuild_seven_zip_archive(
        Rc::clone(&target_archive),
        &mut queue,
        &mut total_fixed,
        &crc_map,
        &sha1_map,
        &md5_map,
    ));
    assert_eq!(queue.len(), 1);
    let mut cleanup_queue = Vec::new();
    let mut cleanup_total_fixed = 0;
    Fix::fix_a_file(
        queue.remove(0),
        &mut cleanup_queue,
        &mut cleanup_total_fixed,
        &crc_map,
        &sha1_map,
        &md5_map,
    );

    assert_eq!(
        Fix::read_seven_zip_entry_bytes(&target_path.to_string_lossy(), "keep.bin").unwrap(),
        b"keep"
    );
    assert_eq!(
        Fix::read_seven_zip_entry_bytes(&target_path.to_string_lossy(), "missing.bin").unwrap(),
        b"new"
    );
    assert_eq!(missing_child.borrow().rep_status(), RepStatus::Correct);
    assert_eq!(source_file.borrow().rep_status(), RepStatus::Deleted);
    assert_eq!(
        target_archive.borrow().zip_struct,
        ZipStructure::SevenZipSLZMA
    );
    assert_eq!(total_fixed, 1);
}

#[test]
fn test_fix_sevenzip_rebuild_runs_for_structure_only_change() {
    let temp = tempdir().unwrap();
    let root = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
    root.borrow_mut().name = temp.path().to_string_lossy().to_string();

    let target_archive = Rc::new(RefCell::new(RvFile::new(FileType::SevenZip)));
    {
        let mut archive = target_archive.borrow_mut();
        archive.name = "target.7z".to_string();
        archive.tree_checked = TreeSelect::Selected;
        archive.zip_struct = ZipStructure::SevenZipNLZMA;
        archive.set_dat_status(dat_reader::enums::DatStatus::InDatCollect);
        archive.set_zip_dat_struct(ZipStructure::SevenZipSLZMA, true);
        archive.parent = Some(Rc::downgrade(&root));
    }

    let target_child = Rc::new(RefCell::new(RvFile::new(FileType::File)));
    {
        let mut file = target_child.borrow_mut();
        file.name = "a.bin".to_string();
        file.size = Some(4);
        file.tree_checked = TreeSelect::Selected;
        file.set_dat_got_status(dat_reader::enums::DatStatus::InDatCollect, GotStatus::Got);
        file.set_rep_status(RepStatus::Correct);
        file.parent = Some(Rc::downgrade(&target_archive));
    }
    target_archive
        .borrow_mut()
        .child_add(Rc::clone(&target_child));
    root.borrow_mut().child_add(Rc::clone(&target_archive));

    let stage_dir = temp.path().join("stage_7z_structure");
    fs::create_dir_all(&stage_dir).unwrap();
    fs::write(stage_dir.join("a.bin"), b"data").unwrap();
    let target_path = temp.path().join("target.7z");
    sevenz_rust::compress_to_path(&stage_dir, &target_path).unwrap();

    let mut queue = Vec::new();
    let mut total_fixed = 0;
    let crc_map = HashMap::new();
    let sha1_map = HashMap::new();
    let md5_map = HashMap::new();

    assert!(Fix::rebuild_seven_zip_archive(
        Rc::clone(&target_archive),
        &mut queue,
        &mut total_fixed,
        &crc_map,
        &sha1_map,
        &md5_map,
    ));
    assert_eq!(
        Fix::read_seven_zip_entry_bytes(&target_path.to_string_lossy(), "a.bin").unwrap(),
        b"data"
    );
    assert_eq!(
        target_archive.borrow().zip_struct,
        ZipStructure::SevenZipSLZMA
    );
    let mut check = compress::SevenZipFile::new();
    assert_eq!(
        compress::ICompress::zip_file_open(&mut check, &target_path.to_string_lossy(), 0, true),
        compress::ZipReturn::ZipGood
    );
    assert_eq!(
        compress::ICompress::zip_struct(&check),
        compress::ZipStructure::SevenZipSLZMA
    );
    compress::ICompress::zip_file_close(&mut check);
    assert_eq!(target_archive.borrow().rep_status(), RepStatus::Correct);
    assert_eq!(target_archive.borrow().got_status(), GotStatus::Got);
}

#[test]
fn test_fix_sevenzip_rebuild_supports_nested_directory_members() {
    let temp = tempdir().unwrap();
    let root = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
    root.borrow_mut().name = temp.path().to_string_lossy().to_string();

    let target_archive = Rc::new(RefCell::new(RvFile::new(FileType::SevenZip)));
    {
        let mut archive = target_archive.borrow_mut();
        archive.name = "target.7z".to_string();
        archive.tree_checked = TreeSelect::Selected;
        archive.zip_struct = ZipStructure::SevenZipNLZMA;
        archive.set_dat_status(dat_reader::enums::DatStatus::InDatCollect);
        archive.set_zip_dat_struct(ZipStructure::SevenZipSLZMA, true);
        archive.parent = Some(Rc::downgrade(&root));
    }

    let folder = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
    {
        let mut dir = folder.borrow_mut();
        dir.name = "sub".to_string();
        dir.tree_checked = TreeSelect::Selected;
        dir.parent = Some(Rc::downgrade(&target_archive));
    }

    let target_child = Rc::new(RefCell::new(RvFile::new(FileType::File)));
    {
        let mut file = target_child.borrow_mut();
        file.name = "a.bin".to_string();
        file.size = Some(4);
        file.tree_checked = TreeSelect::Selected;
        file.set_dat_got_status(dat_reader::enums::DatStatus::InDatCollect, GotStatus::Got);
        file.set_rep_status(RepStatus::Correct);
        file.parent = Some(Rc::downgrade(&folder));
    }

    folder.borrow_mut().child_add(Rc::clone(&target_child));
    target_archive.borrow_mut().child_add(Rc::clone(&folder));
    root.borrow_mut().child_add(Rc::clone(&target_archive));

    let stage_dir = temp.path().join("stage_7z_nested");
    fs::create_dir_all(stage_dir.join("sub")).unwrap();
    fs::write(stage_dir.join("sub").join("a.bin"), b"data").unwrap();
    let target_path = temp.path().join("target.7z");
    sevenz_rust::compress_to_path(&stage_dir, &target_path).unwrap();

    let mut queue = Vec::new();
    let mut total_fixed = 0;
    let crc_map = HashMap::new();
    let sha1_map = HashMap::new();
    let md5_map = HashMap::new();

    assert!(Fix::rebuild_seven_zip_archive(
        Rc::clone(&target_archive),
        &mut queue,
        &mut total_fixed,
        &crc_map,
        &sha1_map,
        &md5_map,
    ));

    assert_eq!(
        Fix::read_seven_zip_entry_bytes(&target_path.to_string_lossy(), "sub/a.bin").unwrap(),
        b"data"
    );
    assert_eq!(target_child.borrow().rep_status(), RepStatus::Correct);
    assert_eq!(target_archive.borrow().rep_status(), RepStatus::Correct);
}

#[test]
fn test_fix_sevenzip_move_moves_whole_archive_with_nested_directory_members() {
    let temp = tempdir().unwrap();
    let root = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
    root.borrow_mut().name = temp.path().to_string_lossy().to_string();

    let source_archive = Rc::new(RefCell::new(RvFile::new(FileType::SevenZip)));
    {
        let mut archive = source_archive.borrow_mut();
        archive.name = "source.7z".to_string();
        archive.tree_checked = TreeSelect::Selected;
        archive.parent = Some(Rc::downgrade(&root));
    }

    let source_dir = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
    {
        let mut dir = source_dir.borrow_mut();
        dir.name = "sub".to_string();
        dir.tree_checked = TreeSelect::Selected;
        dir.parent = Some(Rc::downgrade(&source_archive));
    }

    let source_child = Rc::new(RefCell::new(RvFile::new(FileType::File)));
    {
        let mut file = source_child.borrow_mut();
        file.name = "game.bin".to_string();
        file.size = Some(4);
        file.crc = Some(vec![0x12, 0x34, 0x56, 0x78]);
        file.tree_checked = TreeSelect::Selected;
        file.set_rep_status(RepStatus::NeededForFix);
        file.parent = Some(Rc::downgrade(&source_dir));
    }
    source_dir.borrow_mut().child_add(Rc::clone(&source_child));
    source_archive
        .borrow_mut()
        .child_add(Rc::clone(&source_dir));

    let target_archive = Rc::new(RefCell::new(RvFile::new(FileType::SevenZip)));
    {
        let mut archive = target_archive.borrow_mut();
        archive.name = "target.7z".to_string();
        archive.tree_checked = TreeSelect::Selected;
        archive.parent = Some(Rc::downgrade(&root));
    }

    let target_dir = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
    {
        let mut dir = target_dir.borrow_mut();
        dir.name = "sub".to_string();
        dir.tree_checked = TreeSelect::Selected;
        dir.parent = Some(Rc::downgrade(&target_archive));
    }

    let target_child = Rc::new(RefCell::new(RvFile::new(FileType::File)));
    {
        let mut file = target_child.borrow_mut();
        file.name = "game.bin".to_string();
        file.size = Some(4);
        file.crc = Some(vec![0x12, 0x34, 0x56, 0x78]);
        file.tree_checked = TreeSelect::Selected;
        file.set_dat_got_status(
            dat_reader::enums::DatStatus::InDatCollect,
            dat_reader::enums::GotStatus::NotGot,
        );
        file.set_rep_status(RepStatus::CanBeFixed);
        file.parent = Some(Rc::downgrade(&target_dir));
    }
    target_dir.borrow_mut().child_add(Rc::clone(&target_child));
    target_archive
        .borrow_mut()
        .child_add(Rc::clone(&target_dir));

    root.borrow_mut().child_add(Rc::clone(&source_archive));
    root.borrow_mut().child_add(Rc::clone(&target_archive));

    let stage_dir = temp.path().join("source_7z_nested_move");
    fs::create_dir_all(stage_dir.join("sub")).unwrap();
    fs::write(stage_dir.join("sub").join("game.bin"), b"data").unwrap();
    let source_path = temp.path().join("source.7z");
    sevenz_rust::compress_to_path(&stage_dir, &source_path).unwrap();

    let mut queue = Vec::new();
    let mut total_fixed = 0;
    let mut crc_map = HashMap::new();
    crc_map.insert((4, vec![0x12, 0x34, 0x56, 0x78]), Rc::clone(&source_child));
    let sha1_map = HashMap::new();
    let md5_map = HashMap::new();

    Fix::fix_a_zip(
        Rc::clone(&target_archive),
        &mut queue,
        &mut total_fixed,
        &crc_map,
        &sha1_map,
        &md5_map,
    );

    assert_eq!(queue.len(), 1);
    Fix::fix_archive_node(Rc::clone(&queue.remove(0)));

    let target_path = temp.path().join("target.7z");
    assert!(target_path.exists());
    assert!(!source_path.exists());
    assert_eq!(
        Fix::read_seven_zip_entry_bytes(&target_path.to_string_lossy(), "sub/game.bin").unwrap(),
        b"data"
    );
    assert_eq!(target_child.borrow().rep_status(), RepStatus::Correct);
    assert_eq!(source_archive.borrow().rep_status(), RepStatus::Deleted);
    assert_eq!(total_fixed, 1);
}

#[test]
fn test_fix_zip_rebuild_preserves_empty_directory_entries() {
    let temp = tempdir().unwrap();
    let root = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
    root.borrow_mut().name = temp.path().to_string_lossy().to_string();

    let target_archive = Rc::new(RefCell::new(RvFile::new(FileType::Zip)));
    {
        let mut archive = target_archive.borrow_mut();
        archive.name = "target.zip".to_string();
        archive.tree_checked = TreeSelect::Selected;
        archive.zip_struct = ZipStructure::ZipTDC;
        archive.set_dat_status(dat_reader::enums::DatStatus::InDatCollect);
        archive.set_zip_dat_struct(ZipStructure::ZipTrrnt, true);
        archive.parent = Some(Rc::downgrade(&root));
    }

    let empty_dir = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
    {
        let mut dir = empty_dir.borrow_mut();
        dir.name = "empty".to_string();
        dir.tree_checked = TreeSelect::Selected;
        dir.set_rep_status(RepStatus::Correct);
        dir.parent = Some(Rc::downgrade(&target_archive));
    }
    target_archive.borrow_mut().child_add(Rc::clone(&empty_dir));
    root.borrow_mut().child_add(Rc::clone(&target_archive));

    let target_path = temp.path().join("target.zip");
    {
        let file = File::create(&target_path).unwrap();
        let mut writer = ZipWriter::new(file);
        writer
            .add_directory("empty/", SimpleFileOptions::default())
            .unwrap();
        writer.finish().unwrap();
    }

    let mut queue = Vec::new();
    let mut total_fixed = 0;
    let crc_map = HashMap::new();
    let sha1_map = HashMap::new();
    let md5_map = HashMap::new();

    assert!(Fix::rebuild_zip_archive(
        Rc::clone(&target_archive),
        &mut queue,
        &mut total_fixed,
        &crc_map,
        &sha1_map,
        &md5_map,
    ));

    let file = File::open(&target_path).unwrap();
    let mut archive = ZipArchive::new(file).unwrap();
    assert!(archive.by_name("empty/").is_ok());
    assert_eq!(target_archive.borrow().rep_status(), RepStatus::Correct);
}

#[test]
fn test_fix_zip_partial_rebuild_does_not_queue_cleanup_when_source_is_same_member() {
    let temp = tempdir().unwrap();
    let root = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
    root.borrow_mut().name = temp.path().to_string_lossy().to_string();

    let target_archive = Rc::new(RefCell::new(RvFile::new(FileType::Zip)));
    {
        let mut archive = target_archive.borrow_mut();
        archive.name = "target.zip".to_string();
        archive.tree_checked = TreeSelect::Selected;
        archive.set_dat_status(dat_reader::enums::DatStatus::InDatCollect);
        archive.set_zip_dat_struct(ZipStructure::ZipTrrnt, true);
        archive.parent = Some(Rc::downgrade(&root));
    }

    let child = Rc::new(RefCell::new(RvFile::new(FileType::File)));
    {
        let mut file = child.borrow_mut();
        file.name = "keep.bin".to_string();
        file.file_name = "keep.bin".to_string();
        file.size = Some(4);
        file.crc = Some(vec![0xAD, 0xF3, 0xF3, 0x63]);
        file.tree_checked = TreeSelect::Selected;
        file.set_dat_got_status(
            dat_reader::enums::DatStatus::InDatCollect,
            GotStatus::NotGot,
        );
        file.set_rep_status(RepStatus::CanBeFixed);
        file.parent = Some(Rc::downgrade(&target_archive));
    }

    target_archive.borrow_mut().child_add(Rc::clone(&child));
    root.borrow_mut().child_add(Rc::clone(&target_archive));

    let target_path = temp.path().join("target.zip");
    {
        let file = File::create(&target_path).unwrap();
        let mut writer = ZipWriter::new(file);
        writer
            .start_file("keep.bin", SimpleFileOptions::default())
            .unwrap();
        writer.write_all(b"data").unwrap();
        writer.finish().unwrap();
    }

    let mut queue = Vec::new();
    let mut total_fixed = 0;
    let mut crc_map = HashMap::new();
    crc_map.insert((4, vec![0xAD, 0xF3, 0xF3, 0x63]), Rc::clone(&child));
    let sha1_map = HashMap::new();
    let md5_map = HashMap::new();

    assert!(Fix::rebuild_zip_archive(
        Rc::clone(&target_archive),
        &mut queue,
        &mut total_fixed,
        &crc_map,
        &sha1_map,
        &md5_map,
    ));

    let mut data = Vec::new();
    ZipArchive::new(File::open(&target_path).unwrap())
        .unwrap()
        .by_name("keep.bin")
        .unwrap()
        .read_to_end(&mut data)
        .unwrap();
    assert_eq!(data, b"data");
    assert!(queue.is_empty());
    assert_eq!(child.borrow().rep_status(), RepStatus::Correct);
    assert_eq!(child.borrow().got_status(), GotStatus::Got);
    assert_eq!(total_fixed, 1);
}

#[test]
fn test_fix_sevenzip_partial_rebuild_renames_existing_entry() {
    let temp = tempdir().unwrap();
    let root = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
    root.borrow_mut().name = temp.path().to_string_lossy().to_string();

    let target_archive = Rc::new(RefCell::new(RvFile::new(FileType::SevenZip)));
    {
        let mut archive = target_archive.borrow_mut();
        archive.name = "target.7z".to_string();
        archive.tree_checked = TreeSelect::Selected;
        archive.set_dat_status(dat_reader::enums::DatStatus::InDatMIA);
        archive.set_zip_dat_struct(ZipStructure::SevenZipSLZMA, true);
        archive.parent = Some(Rc::downgrade(&root));
    }

    let renamed_child = Rc::new(RefCell::new(RvFile::new(FileType::File)));
    {
        let mut file = renamed_child.borrow_mut();
        file.name = "new.bin".to_string();
        file.file_name = "old.bin".to_string();
        file.size = Some(4);
        file.tree_checked = TreeSelect::Selected;
        file.set_dat_got_status(dat_reader::enums::DatStatus::InDatCollect, GotStatus::Got);
        file.set_rep_status(RepStatus::Rename);
        file.parent = Some(Rc::downgrade(&target_archive));
    }

    target_archive
        .borrow_mut()
        .child_add(Rc::clone(&renamed_child));
    root.borrow_mut().child_add(Rc::clone(&target_archive));

    let stage_dir = temp.path().join("stage_7z");
    fs::create_dir_all(&stage_dir).unwrap();
    fs::write(stage_dir.join("old.bin"), b"data").unwrap();
    let target_path = temp.path().join("target.7z");
    sevenz_rust::compress_to_path(&stage_dir, &target_path).unwrap();

    let mut queue = Vec::new();
    let mut total_fixed = 0;
    let crc_map = HashMap::new();
    let sha1_map = HashMap::new();
    let md5_map = HashMap::new();

    assert!(Fix::rebuild_seven_zip_archive(
        Rc::clone(&target_archive),
        &mut queue,
        &mut total_fixed,
        &crc_map,
        &sha1_map,
        &md5_map,
    ));

    assert_eq!(
        Fix::read_seven_zip_entry_bytes(&target_path.to_string_lossy(), "new.bin").unwrap(),
        b"data"
    );
    assert!(Fix::read_seven_zip_entry_bytes(&target_path.to_string_lossy(), "old.bin").is_none());
    assert_eq!(renamed_child.borrow().rep_status(), RepStatus::Correct);
    assert_eq!(renamed_child.borrow().file_name, "new.bin");
}
#[test]
fn test_fix_sevenzip_partial_rebuild_rename_preserves_correctmia() {
    let temp = tempdir().unwrap();
    let root = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
    root.borrow_mut().name = temp.path().to_string_lossy().to_string();

    let target_archive = Rc::new(RefCell::new(RvFile::new(FileType::SevenZip)));
    {
        let mut archive = target_archive.borrow_mut();
        archive.name = "target.7z".to_string();
        archive.tree_checked = TreeSelect::Selected;
        archive.set_dat_status(dat_reader::enums::DatStatus::InDatMIA);
        archive.set_zip_dat_struct(ZipStructure::SevenZipSLZMA, true);
        archive.parent = Some(Rc::downgrade(&root));
    }

    let renamed_child = Rc::new(RefCell::new(RvFile::new(FileType::File)));
    {
        let mut file = renamed_child.borrow_mut();
        file.name = "new.bin".to_string();
        file.file_name = "old.bin".to_string();
        file.size = Some(4);
        file.tree_checked = TreeSelect::Selected;
        file.set_dat_got_status(dat_reader::enums::DatStatus::InDatMIA, GotStatus::Got);
        file.set_rep_status(RepStatus::Rename);
        file.parent = Some(Rc::downgrade(&target_archive));
    }

    target_archive
        .borrow_mut()
        .child_add(Rc::clone(&renamed_child));
    root.borrow_mut().child_add(Rc::clone(&target_archive));

    let stage_dir = temp.path().join("stage_7z");
    fs::create_dir_all(&stage_dir).unwrap();
    fs::write(stage_dir.join("old.bin"), b"data").unwrap();
    let target_path = temp.path().join("target.7z");
    sevenz_rust::compress_to_path(&stage_dir, &target_path).unwrap();

    let mut queue = Vec::new();
    let mut total_fixed = 0;
    let crc_map = HashMap::new();
    let sha1_map = HashMap::new();
    let md5_map = HashMap::new();

    assert!(Fix::rebuild_seven_zip_archive(
        Rc::clone(&target_archive),
        &mut queue,
        &mut total_fixed,
        &crc_map,
        &sha1_map,
        &md5_map,
    ));

    assert_eq!(renamed_child.borrow().rep_status(), RepStatus::CorrectMIA);
    assert_eq!(renamed_child.borrow().file_name, "new.bin");
    assert_eq!(target_archive.borrow().rep_status(), RepStatus::CorrectMIA);
}

#[test]
fn test_fix_sevenzip_rebuild_does_not_queue_cleanup_when_source_archive_path_differs_only_by_case()
{
    let temp = tempdir().unwrap();
    let root = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
    root.borrow_mut().name = temp.path().to_string_lossy().to_string();

    let source_archive = Rc::new(RefCell::new(RvFile::new(FileType::SevenZip)));
    {
        let mut archive = source_archive.borrow_mut();
        archive.name = "source.7z".to_string();
        archive.tree_checked = TreeSelect::Selected;
        archive.parent = Some(Rc::downgrade(&root));
    }

    let source_child = Rc::new(RefCell::new(RvFile::new(FileType::File)));
    {
        let mut file = source_child.borrow_mut();
        file.name = "game.bin".to_string();
        file.file_name = "game.bin".to_string();
        file.size = Some(4);
        file.crc = Some(vec![0x12, 0x34, 0x56, 0x78]);
        file.tree_checked = TreeSelect::Selected;
        file.set_rep_status(RepStatus::NeededForFix);
        file.parent = Some(Rc::downgrade(&source_archive));
    }
    source_archive
        .borrow_mut()
        .child_add(Rc::clone(&source_child));

    let target_archive = Rc::new(RefCell::new(RvFile::new(FileType::SevenZip)));
    {
        let mut archive = target_archive.borrow_mut();
        archive.name = "Source.7z".to_string();
        archive.tree_checked = TreeSelect::Selected;
        archive.set_dat_status(dat_reader::enums::DatStatus::InDatCollect);
        archive.set_zip_dat_struct(ZipStructure::SevenZipSLZMA, true);
        archive.parent = Some(Rc::downgrade(&root));
    }

    let target_child = Rc::new(RefCell::new(RvFile::new(FileType::File)));
    {
        let mut file = target_child.borrow_mut();
        file.name = "game.bin".to_string();
        file.file_name = "game.bin".to_string();
        file.size = Some(4);
        file.crc = Some(vec![0x12, 0x34, 0x56, 0x78]);
        file.tree_checked = TreeSelect::Selected;
        file.set_dat_got_status(
            dat_reader::enums::DatStatus::InDatCollect,
            GotStatus::NotGot,
        );
        file.set_rep_status(RepStatus::CanBeFixed);
        file.parent = Some(Rc::downgrade(&target_archive));
    }
    target_archive
        .borrow_mut()
        .child_add(Rc::clone(&target_child));

    root.borrow_mut().child_add(Rc::clone(&source_archive));
    root.borrow_mut().child_add(Rc::clone(&target_archive));

    let stage_dir = temp.path().join("stage_same_archive_case_7z");
    fs::create_dir_all(&stage_dir).unwrap();
    fs::write(stage_dir.join("game.bin"), b"data").unwrap();
    let source_path = temp.path().join("source.7z");
    sevenz_rust::compress_to_path(&stage_dir, &source_path).unwrap();

    let mut queue = Vec::new();
    let mut total_fixed = 0;
    let mut crc_map = HashMap::new();
    crc_map.insert((4, vec![0x12, 0x34, 0x56, 0x78]), Rc::clone(&source_child));
    let sha1_map = HashMap::new();
    let md5_map = HashMap::new();

    assert!(Fix::rebuild_seven_zip_archive(
        Rc::clone(&target_archive),
        &mut queue,
        &mut total_fixed,
        &crc_map,
        &sha1_map,
        &md5_map,
    ));

    let target_path = temp.path().join("Source.7z");
    assert_eq!(
        Fix::read_seven_zip_entry_bytes(&target_path.to_string_lossy(), "game.bin").unwrap(),
        b"data"
    );
    assert!(queue.is_empty());
    assert_eq!(target_child.borrow().rep_status(), RepStatus::Correct);
    assert_eq!(target_child.borrow().got_status(), GotStatus::Got);
    assert_eq!(total_fixed, 1);
}

#[test]
fn test_fix_sevenzip_partial_rebuild_does_not_queue_cleanup_when_source_is_same_member() {
    let temp = tempdir().unwrap();
    let root = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
    root.borrow_mut().name = temp.path().to_string_lossy().to_string();

    let target_archive = Rc::new(RefCell::new(RvFile::new(FileType::SevenZip)));
    {
        let mut archive = target_archive.borrow_mut();
        archive.name = "target.7z".to_string();
        archive.tree_checked = TreeSelect::Selected;
        archive.set_dat_status(dat_reader::enums::DatStatus::InDatCollect);
        archive.set_zip_dat_struct(ZipStructure::SevenZipSLZMA, true);
        archive.parent = Some(Rc::downgrade(&root));
    }

    let child = Rc::new(RefCell::new(RvFile::new(FileType::File)));
    {
        let mut file = child.borrow_mut();
        file.name = "keep.bin".to_string();
        file.file_name = "keep.bin".to_string();
        file.size = Some(4);
        file.crc = Some(vec![0xAD, 0xF3, 0xF3, 0x63]);
        file.tree_checked = TreeSelect::Selected;
        file.set_dat_got_status(
            dat_reader::enums::DatStatus::InDatCollect,
            GotStatus::NotGot,
        );
        file.set_rep_status(RepStatus::CanBeFixed);
        file.parent = Some(Rc::downgrade(&target_archive));
    }

    target_archive.borrow_mut().child_add(Rc::clone(&child));
    root.borrow_mut().child_add(Rc::clone(&target_archive));

    let stage_dir = temp.path().join("stage_same_member_7z");
    fs::create_dir_all(&stage_dir).unwrap();
    fs::write(stage_dir.join("keep.bin"), b"data").unwrap();
    let target_path = temp.path().join("target.7z");
    sevenz_rust::compress_to_path(&stage_dir, &target_path).unwrap();

    let mut queue = Vec::new();
    let mut total_fixed = 0;
    let mut crc_map = HashMap::new();
    crc_map.insert((4, vec![0xAD, 0xF3, 0xF3, 0x63]), Rc::clone(&child));
    let sha1_map = HashMap::new();
    let md5_map = HashMap::new();

    assert!(Fix::rebuild_seven_zip_archive(
        Rc::clone(&target_archive),
        &mut queue,
        &mut total_fixed,
        &crc_map,
        &sha1_map,
        &md5_map,
    ));

    assert_eq!(
        Fix::read_seven_zip_entry_bytes(&target_path.to_string_lossy(), "keep.bin").unwrap(),
        b"data"
    );
    assert!(queue.is_empty());
    assert_eq!(child.borrow().rep_status(), RepStatus::Correct);
    assert_eq!(child.borrow().got_status(), GotStatus::Got);
    assert_eq!(total_fixed, 1);
}

#[test]
fn test_fix_sevenzip_partial_rebuild_marks_removed_entry_missing_and_clears_got() {
    let temp = tempdir().unwrap();
    let root = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
    root.borrow_mut().name = temp.path().to_string_lossy().to_string();

    let target_archive = Rc::new(RefCell::new(RvFile::new(FileType::SevenZip)));
    {
        let mut archive = target_archive.borrow_mut();
        archive.name = "target.7z".to_string();
        archive.tree_checked = TreeSelect::Selected;
        archive.set_dat_status(dat_reader::enums::DatStatus::InDatCollect);
        archive.set_zip_dat_struct(ZipStructure::SevenZipSLZMA, true);
        archive.parent = Some(Rc::downgrade(&root));
    }

    let moved_child = Rc::new(RefCell::new(RvFile::new(FileType::File)));
    {
        let mut file = moved_child.borrow_mut();
        file.name = "bad.bin".to_string();
        file.size = Some(4);
        file.tree_checked = TreeSelect::Selected;
        file.set_dat_got_status(dat_reader::enums::DatStatus::InDatCollect, GotStatus::Got);
        file.set_rep_status(RepStatus::MoveToCorrupt);
        file.parent = Some(Rc::downgrade(&target_archive));
    }

    target_archive
        .borrow_mut()
        .child_add(Rc::clone(&moved_child));
    root.borrow_mut().child_add(Rc::clone(&target_archive));

    let stage_dir = temp.path().join("stage_7z_corrupt");
    fs::create_dir_all(&stage_dir).unwrap();
    fs::write(stage_dir.join("bad.bin"), b"data").unwrap();
    let target_path = temp.path().join("target.7z");
    sevenz_rust::compress_to_path(&stage_dir, &target_path).unwrap();

    let mut queue = Vec::new();
    let mut total_fixed = 0;
    let crc_map = HashMap::new();
    let sha1_map = HashMap::new();
    let md5_map = HashMap::new();

    assert!(Fix::rebuild_seven_zip_archive(
        Rc::clone(&target_archive),
        &mut queue,
        &mut total_fixed,
        &crc_map,
        &sha1_map,
        &md5_map,
    ));

    let moved_path = Fix::get_archive_member_tosort_path(&target_path, "bad.bin", "ToSort/Corrupt");
    assert!(moved_path.exists());
    assert_eq!(fs::read(&moved_path).unwrap(), b"data");
    assert!(!target_path.exists());
    assert_eq!(moved_child.borrow().rep_status(), RepStatus::Missing);
    assert_eq!(moved_child.borrow().got_status(), GotStatus::NotGot);
    assert_eq!(target_archive.borrow().rep_status(), RepStatus::Missing);
    assert_eq!(target_archive.borrow().got_status(), GotStatus::NotGot);
}

#[test]
fn test_fix_sevenzip_partial_rebuild_marks_removed_entry_missing_and_clears_got_when_archive_remains(
) {
    let temp = tempdir().unwrap();
    let root = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
    root.borrow_mut().name = temp.path().to_string_lossy().to_string();

    let target_archive = Rc::new(RefCell::new(RvFile::new(FileType::SevenZip)));
    {
        let mut archive = target_archive.borrow_mut();
        archive.name = "target.7z".to_string();
        archive.tree_checked = TreeSelect::Selected;
        archive.set_dat_status(dat_reader::enums::DatStatus::InDatCollect);
        archive.set_zip_dat_struct(ZipStructure::SevenZipSLZMA, true);
        archive.parent = Some(Rc::downgrade(&root));
    }

    let keep_child = Rc::new(RefCell::new(RvFile::new(FileType::File)));
    {
        let mut file = keep_child.borrow_mut();
        file.name = "keep.bin".to_string();
        file.file_name = "keep.bin".to_string();
        file.size = Some(4);
        file.tree_checked = TreeSelect::Selected;
        file.set_dat_got_status(dat_reader::enums::DatStatus::InDatCollect, GotStatus::Got);
        file.parent = Some(Rc::downgrade(&target_archive));
    }

    let moved_child = Rc::new(RefCell::new(RvFile::new(FileType::File)));
    {
        let mut file = moved_child.borrow_mut();
        file.name = "bad.bin".to_string();
        file.file_name = "bad.bin".to_string();
        file.size = Some(4);
        file.tree_checked = TreeSelect::Selected;
        file.set_dat_got_status(dat_reader::enums::DatStatus::InDatCollect, GotStatus::Got);
        file.set_rep_status(RepStatus::MoveToCorrupt);
        file.parent = Some(Rc::downgrade(&target_archive));
    }

    target_archive
        .borrow_mut()
        .child_add(Rc::clone(&keep_child));
    target_archive
        .borrow_mut()
        .child_add(Rc::clone(&moved_child));
    root.borrow_mut().child_add(Rc::clone(&target_archive));

    let stage_dir = temp.path().join("stage_7z_corrupt_keep");
    fs::create_dir_all(&stage_dir).unwrap();
    fs::write(stage_dir.join("keep.bin"), b"keep").unwrap();
    fs::write(stage_dir.join("bad.bin"), b"data").unwrap();
    let target_path = temp.path().join("target.7z");
    sevenz_rust::compress_to_path(&stage_dir, &target_path).unwrap();

    let mut queue = Vec::new();
    let mut total_fixed = 0;
    let crc_map = HashMap::new();
    let sha1_map = HashMap::new();
    let md5_map = HashMap::new();

    assert!(Fix::rebuild_seven_zip_archive(
        Rc::clone(&target_archive),
        &mut queue,
        &mut total_fixed,
        &crc_map,
        &sha1_map,
        &md5_map,
    ));

    let moved_path = Fix::get_archive_member_tosort_path(&target_path, "bad.bin", "ToSort/Corrupt");
    assert!(moved_path.exists());
    assert_eq!(fs::read(&moved_path).unwrap(), b"data");
    assert_eq!(
        Fix::read_seven_zip_entry_bytes(&target_path.to_string_lossy(), "keep.bin").unwrap(),
        b"keep"
    );
    assert!(Fix::read_seven_zip_entry_bytes(&target_path.to_string_lossy(), "bad.bin").is_none());
    assert_eq!(moved_child.borrow().rep_status(), RepStatus::Missing);
    assert_eq!(moved_child.borrow().got_status(), GotStatus::NotGot);
    assert_eq!(target_archive.borrow().got_status(), GotStatus::Got);
}

#[test]
fn test_fix_loose_file_from_zip_source_with_existing_archive_name_and_rebuilds_source_archive() {
    let temp = tempdir().unwrap();
    let root = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
    root.borrow_mut().name = temp.path().to_string_lossy().to_string();

    let source_archive = Rc::new(RefCell::new(RvFile::new(FileType::Zip)));
    {
        let mut archive = source_archive.borrow_mut();
        archive.name = "source.zip".to_string();
        archive.file_name = "oldsource.zip".to_string();
        archive.tree_checked = TreeSelect::Selected;
        archive.parent = Some(Rc::downgrade(&root));
    }

    let source_keep = Rc::new(RefCell::new(RvFile::new(FileType::File)));
    {
        let mut file = source_keep.borrow_mut();
        file.name = "keep.bin".to_string();
        file.size = Some(4);
        file.tree_checked = TreeSelect::Selected;
        file.set_dat_got_status(dat_reader::enums::DatStatus::InDatCollect, GotStatus::Got);
        file.set_rep_status(RepStatus::Correct);
        file.parent = Some(Rc::downgrade(&source_archive));
    }

    let source_move = Rc::new(RefCell::new(RvFile::new(FileType::File)));
    {
        let mut file = source_move.borrow_mut();
        file.name = "move.bin".to_string();
        file.size = Some(4);
        file.crc = Some(vec![0x00, 0x00, 0x00, 0x04]);
        file.tree_checked = TreeSelect::Selected;
        file.set_rep_status(RepStatus::NeededForFix);
        file.parent = Some(Rc::downgrade(&source_archive));
    }

    source_archive
        .borrow_mut()
        .child_add(Rc::clone(&source_keep));
    source_archive
        .borrow_mut()
        .child_add(Rc::clone(&source_move));

    let target_file = Rc::new(RefCell::new(RvFile::new(FileType::File)));
    {
        let mut file = target_file.borrow_mut();
        file.name = "move.bin".to_string();
        file.size = Some(4);
        file.crc = Some(vec![0x00, 0x00, 0x00, 0x04]);
        file.tree_checked = TreeSelect::Selected;
        file.set_dat_got_status(
            dat_reader::enums::DatStatus::InDatCollect,
            GotStatus::NotGot,
        );
        file.set_rep_status(RepStatus::CanBeFixed);
        file.parent = Some(Rc::downgrade(&root));
    }

    root.borrow_mut().child_add(Rc::clone(&source_archive));
    root.borrow_mut().child_add(Rc::clone(&target_file));

    let source_path = temp.path().join("oldsource.zip");
    {
        let file = File::create(&source_path).unwrap();
        let mut writer = ZipWriter::new(file);
        writer
            .start_file("keep.bin", SimpleFileOptions::default())
            .unwrap();
        writer.write_all(b"keep").unwrap();
        writer
            .start_file("move.bin", SimpleFileOptions::default())
            .unwrap();
        writer.write_all(b"move").unwrap();
        writer.finish().unwrap();
    }

    let mut queue = Vec::new();
    let mut total_fixed = 0;
    let mut crc_map = HashMap::new();
    crc_map.insert((4, vec![0x00, 0x00, 0x00, 0x04]), Rc::clone(&source_move));
    let sha1_map = HashMap::new();
    let md5_map = HashMap::new();

    Fix::fix_a_file(
        Rc::clone(&target_file),
        &mut queue,
        &mut total_fixed,
        &crc_map,
        &sha1_map,
        &md5_map,
    );

    assert_eq!(fs::read(temp.path().join("move.bin")).unwrap(), b"move");
    assert_eq!(queue.len(), 1);

    let mut cleanup_queue = Vec::new();
    let mut cleanup_total_fixed = 0;
    Fix::fix_a_zip(
        queue.remove(0),
        &mut cleanup_queue,
        &mut cleanup_total_fixed,
        &crc_map,
        &sha1_map,
        &md5_map,
    );

    assert!(Fix::read_zip_entry_bytes(&source_path.to_string_lossy(), "move.bin").is_none());
    assert_eq!(
        Fix::read_zip_entry_bytes(&source_path.to_string_lossy(), "keep.bin").unwrap(),
        b"keep"
    );
    assert_eq!(target_file.borrow().rep_status(), RepStatus::Correct);
    assert_eq!(source_move.borrow().rep_status(), RepStatus::Deleted);
    assert_eq!(total_fixed, 1);
}

#[test]
fn test_fix_loose_file_from_sevenzip_source_with_existing_archive_name_and_rebuilds_source_archive()
{
    let temp = tempdir().unwrap();
    let root = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
    root.borrow_mut().name = temp.path().to_string_lossy().to_string();

    let source_archive = Rc::new(RefCell::new(RvFile::new(FileType::SevenZip)));
    {
        let mut archive = source_archive.borrow_mut();
        archive.name = "source.7z".to_string();
        archive.file_name = "oldsource.7z".to_string();
        archive.tree_checked = TreeSelect::Selected;
        archive.set_dat_status(dat_reader::enums::DatStatus::InDatCollect);
        archive.set_zip_dat_struct(ZipStructure::SevenZipSLZMA, true);
        archive.parent = Some(Rc::downgrade(&root));
    }

    let source_keep = Rc::new(RefCell::new(RvFile::new(FileType::File)));
    {
        let mut file = source_keep.borrow_mut();
        file.name = "keep.bin".to_string();
        file.size = Some(4);
        file.tree_checked = TreeSelect::Selected;
        file.set_dat_got_status(dat_reader::enums::DatStatus::InDatCollect, GotStatus::Got);
        file.set_rep_status(RepStatus::Correct);
        file.parent = Some(Rc::downgrade(&source_archive));
    }

    let source_move = Rc::new(RefCell::new(RvFile::new(FileType::File)));
    {
        let mut file = source_move.borrow_mut();
        file.name = "move.bin".to_string();
        file.size = Some(4);
        file.crc = Some(vec![0x00, 0x00, 0x00, 0x04]);
        file.tree_checked = TreeSelect::Selected;
        file.set_rep_status(RepStatus::NeededForFix);
        file.parent = Some(Rc::downgrade(&source_archive));
    }

    source_archive
        .borrow_mut()
        .child_add(Rc::clone(&source_keep));
    source_archive
        .borrow_mut()
        .child_add(Rc::clone(&source_move));

    let target_file = Rc::new(RefCell::new(RvFile::new(FileType::File)));
    {
        let mut file = target_file.borrow_mut();
        file.name = "move.bin".to_string();
        file.size = Some(4);
        file.crc = Some(vec![0x00, 0x00, 0x00, 0x04]);
        file.tree_checked = TreeSelect::Selected;
        file.set_dat_got_status(
            dat_reader::enums::DatStatus::InDatCollect,
            GotStatus::NotGot,
        );
        file.set_rep_status(RepStatus::CanBeFixed);
        file.parent = Some(Rc::downgrade(&root));
    }

    root.borrow_mut().child_add(Rc::clone(&source_archive));
    root.borrow_mut().child_add(Rc::clone(&target_file));

    let source_stage = temp.path().join("source_stage_existing_name");
    fs::create_dir_all(&source_stage).unwrap();
    fs::write(source_stage.join("keep.bin"), b"keep").unwrap();
    fs::write(source_stage.join("move.bin"), b"move").unwrap();
    let source_path = temp.path().join("oldsource.7z");
    sevenz_rust::compress_to_path(&source_stage, &source_path).unwrap();

    let mut queue = Vec::new();
    let mut total_fixed = 0;
    let mut crc_map = HashMap::new();
    crc_map.insert((4, vec![0x00, 0x00, 0x00, 0x04]), Rc::clone(&source_move));
    let sha1_map = HashMap::new();
    let md5_map = HashMap::new();

    Fix::fix_a_file(
        Rc::clone(&target_file),
        &mut queue,
        &mut total_fixed,
        &crc_map,
        &sha1_map,
        &md5_map,
    );

    assert_eq!(fs::read(temp.path().join("move.bin")).unwrap(), b"move");
    assert_eq!(queue.len(), 1);

    let mut cleanup_queue = Vec::new();
    let mut cleanup_total_fixed = 0;
    Fix::fix_a_zip(
        queue.remove(0),
        &mut cleanup_queue,
        &mut cleanup_total_fixed,
        &crc_map,
        &sha1_map,
        &md5_map,
    );

    assert!(Fix::read_seven_zip_entry_bytes(&source_path.to_string_lossy(), "move.bin").is_none());
    assert_eq!(
        Fix::read_seven_zip_entry_bytes(&source_path.to_string_lossy(), "keep.bin").unwrap(),
        b"keep"
    );
    assert_eq!(target_file.borrow().rep_status(), RepStatus::Correct);
    assert_eq!(source_move.borrow().rep_status(), RepStatus::Deleted);
    assert_eq!(total_fixed, 1);
}

#[test]
fn test_fix_loose_file_from_sevenzip_source_and_rebuilds_source_archive() {
    let temp = tempdir().unwrap();
    let root = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
    root.borrow_mut().name = temp.path().to_string_lossy().to_string();

    let source_archive = Rc::new(RefCell::new(RvFile::new(FileType::SevenZip)));
    {
        let mut archive = source_archive.borrow_mut();
        archive.name = "source.7z".to_string();
        archive.tree_checked = TreeSelect::Selected;
        archive.set_dat_status(dat_reader::enums::DatStatus::InDatCollect);
        archive.set_zip_dat_struct(ZipStructure::SevenZipSLZMA, true);
        archive.parent = Some(Rc::downgrade(&root));
    }

    let source_keep = Rc::new(RefCell::new(RvFile::new(FileType::File)));
    {
        let mut file = source_keep.borrow_mut();
        file.name = "keep.bin".to_string();
        file.size = Some(4);
        file.tree_checked = TreeSelect::Selected;
        file.set_dat_got_status(dat_reader::enums::DatStatus::InDatCollect, GotStatus::Got);
        file.set_rep_status(RepStatus::Correct);
        file.parent = Some(Rc::downgrade(&source_archive));
    }

    let source_move = Rc::new(RefCell::new(RvFile::new(FileType::File)));
    {
        let mut file = source_move.borrow_mut();
        file.name = "move.bin".to_string();
        file.size = Some(4);
        file.crc = Some(vec![0x00, 0x00, 0x00, 0x04]);
        file.tree_checked = TreeSelect::Selected;
        file.set_rep_status(RepStatus::NeededForFix);
        file.parent = Some(Rc::downgrade(&source_archive));
    }

    source_archive
        .borrow_mut()
        .child_add(Rc::clone(&source_keep));
    source_archive
        .borrow_mut()
        .child_add(Rc::clone(&source_move));

    let target_file = Rc::new(RefCell::new(RvFile::new(FileType::File)));
    {
        let mut file = target_file.borrow_mut();
        file.name = "move.bin".to_string();
        file.size = Some(4);
        file.crc = Some(vec![0x00, 0x00, 0x00, 0x04]);
        file.tree_checked = TreeSelect::Selected;
        file.set_dat_got_status(
            dat_reader::enums::DatStatus::InDatCollect,
            GotStatus::NotGot,
        );
        file.set_rep_status(RepStatus::CanBeFixed);
        file.parent = Some(Rc::downgrade(&root));
    }

    root.borrow_mut().child_add(Rc::clone(&source_archive));
    root.borrow_mut().child_add(Rc::clone(&target_file));

    let source_stage = temp.path().join("source_stage");
    fs::create_dir_all(&source_stage).unwrap();
    fs::write(source_stage.join("keep.bin"), b"keep").unwrap();
    fs::write(source_stage.join("move.bin"), b"move").unwrap();
    let source_path = temp.path().join("source.7z");
    sevenz_rust::compress_to_path(&source_stage, &source_path).unwrap();

    let mut queue = Vec::new();
    let mut total_fixed = 0;
    let mut crc_map = HashMap::new();
    crc_map.insert((4, vec![0x00, 0x00, 0x00, 0x04]), Rc::clone(&source_move));
    let sha1_map = HashMap::new();
    let md5_map = HashMap::new();

    Fix::fix_a_file(
        Rc::clone(&target_file),
        &mut queue,
        &mut total_fixed,
        &crc_map,
        &sha1_map,
        &md5_map,
    );

    assert_eq!(fs::read(temp.path().join("move.bin")).unwrap(), b"move");
    assert_eq!(queue.len(), 1);

    let mut cleanup_queue = Vec::new();
    let mut cleanup_total_fixed = 0;
    Fix::fix_a_zip(
        queue.remove(0),
        &mut cleanup_queue,
        &mut cleanup_total_fixed,
        &crc_map,
        &sha1_map,
        &md5_map,
    );

    assert!(Fix::read_seven_zip_entry_bytes(&source_path.to_string_lossy(), "move.bin").is_none());
    assert_eq!(
        Fix::read_seven_zip_entry_bytes(&source_path.to_string_lossy(), "keep.bin").unwrap(),
        b"keep"
    );
    assert_eq!(target_file.borrow().rep_status(), RepStatus::Correct);
    assert_eq!(source_move.borrow().rep_status(), RepStatus::Deleted);
    assert_eq!(total_fixed, 1);
}

#[test]
fn test_read_zip_entry_bytes_matches_case_insensitively_on_windows_style_names() {
    let temp = tempdir().unwrap();
    let zip_path = temp.path().join("source.zip");
    {
        let file = File::create(&zip_path).unwrap();
        let mut writer = ZipWriter::new(file);
        writer
            .start_file("MOVE.BIN", SimpleFileOptions::default())
            .unwrap();
        writer.write_all(b"move").unwrap();
        writer.finish().unwrap();
    }

    assert_eq!(
        Fix::read_zip_entry_bytes(&zip_path.to_string_lossy(), "move.bin").unwrap(),
        b"move"
    );
    assert!(Fix::read_raw_zip_entry(&zip_path.to_string_lossy(), "move.bin").is_some());
}

#[test]
fn test_read_seven_zip_entry_bytes_matches_case_insensitively_on_windows_style_names() {
    let temp = tempdir().unwrap();
    let stage_dir = temp.path().join("source_7z_case");
    fs::create_dir_all(&stage_dir).unwrap();
    fs::write(stage_dir.join("MOVE.BIN"), b"move").unwrap();
    let source_path = temp.path().join("source.7z");
    sevenz_rust::compress_to_path(&stage_dir, &source_path).unwrap();

    assert_eq!(
        Fix::read_seven_zip_entry_bytes(&source_path.to_string_lossy(), "move.bin").unwrap(),
        b"move"
    );
}

#[test]
#[cfg(windows)]
fn test_fix_case_only_dir_duplicates_do_not_crash_and_allow_file_creation() {
    let original_settings = get_settings();
    update_settings(Settings {
        cache_save_timer_enabled: false,
        ..Default::default()
    });

    let temp = tempdir().unwrap();
    let root = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
    root.borrow_mut().name = temp.path().to_string_lossy().to_string();

    let rom_root = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
    {
        let mut rr = rom_root.borrow_mut();
        rr.name = "RomRoot".to_string();
        rr.tree_checked = TreeSelect::Selected;
        rr.set_dat_got_status(dat_reader::enums::DatStatus::NotInDat, GotStatus::Got);
        rr.rep_status_reset();
        rr.parent = Some(Rc::downgrade(&root));
    }
    root.borrow_mut().child_add(Rc::clone(&rom_root));

    let to_sort = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
    {
        let mut ts = to_sort.borrow_mut();
        ts.name = "ToSort".to_string();
        ts.tree_checked = TreeSelect::Selected;
        ts.set_dat_got_status(dat_reader::enums::DatStatus::InToSort, GotStatus::Got);
        ts.rep_status_reset();
        ts.parent = Some(Rc::downgrade(&root));
    }
    root.borrow_mut().child_add(Rc::clone(&to_sort));

    fs::create_dir_all(temp.path().join("RomRoot")).unwrap();
    fs::create_dir_all(temp.path().join("ToSort")).unwrap();

    let game_dir = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
    {
        let mut game = game_dir.borrow_mut();
        game.name = "!!!_I_AM_A_NAUGHTY_BUG".to_string();
        game.tree_checked = TreeSelect::Selected;
        game.set_dat_got_status(
            dat_reader::enums::DatStatus::InDatCollect,
            GotStatus::NotGot,
        );
        game.rep_status_reset();
        game.parent = Some(Rc::downgrade(&rom_root));
    }
    rom_root.borrow_mut().child_add(Rc::clone(&game_dir));

    let dir_d = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
    {
        let mut d = dir_d.borrow_mut();
        d.name = "d".to_string();
        d.tree_checked = TreeSelect::Selected;
        d.set_dat_got_status(
            dat_reader::enums::DatStatus::InDatCollect,
            GotStatus::NotGot,
        );
        d.rep_status_reset();
        d.parent = Some(Rc::downgrade(&game_dir));
    }
    game_dir.borrow_mut().child_add(Rc::clone(&dir_d));

    let dir_upper_d = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
    {
        let mut d = dir_upper_d.borrow_mut();
        d.name = "D".to_string();
        d.tree_checked = TreeSelect::Selected;
        d.set_dat_got_status(
            dat_reader::enums::DatStatus::InDatCollect,
            GotStatus::NotGot,
        );
        d.rep_status_reset();
        d.parent = Some(Rc::downgrade(&game_dir));
    }
    game_dir.borrow_mut().child_add(Rc::clone(&dir_upper_d));

    let target_file = Rc::new(RefCell::new(RvFile::new(FileType::File)));
    {
        let mut f = target_file.borrow_mut();
        f.name = "C".to_string();
        f.size = Some(0);
        f.crc = Some(vec![0, 0, 0, 0]);
        f.tree_checked = TreeSelect::Selected;
        f.set_dat_got_status(
            dat_reader::enums::DatStatus::InDatCollect,
            GotStatus::NotGot,
        );
        f.set_rep_status(RepStatus::CanBeFixed);
        f.parent = Some(Rc::downgrade(&dir_d));
    }
    dir_d.borrow_mut().child_add(Rc::clone(&target_file));

    let tosort_d = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
    {
        let mut d = tosort_d.borrow_mut();
        d.name = "d".to_string();
        d.tree_checked = TreeSelect::Selected;
        d.set_dat_got_status(dat_reader::enums::DatStatus::InToSort, GotStatus::Got);
        d.rep_status_reset();
        d.parent = Some(Rc::downgrade(&to_sort));
    }
    to_sort.borrow_mut().child_add(Rc::clone(&tosort_d));

    let source_file = Rc::new(RefCell::new(RvFile::new(FileType::File)));
    {
        let mut f = source_file.borrow_mut();
        f.name = "C".to_string();
        f.size = Some(0);
        f.crc = Some(vec![0, 0, 0, 0]);
        f.tree_checked = TreeSelect::Locked;
        f.set_dat_got_status(dat_reader::enums::DatStatus::InToSort, GotStatus::Got);
        f.set_rep_status(RepStatus::NeededForFix);
        f.parent = Some(Rc::downgrade(&tosort_d));
    }
    tosort_d.borrow_mut().child_add(Rc::clone(&source_file));

    let source_path = temp.path().join("ToSort").join("d").join("C");
    fs::create_dir_all(source_path.parent().unwrap()).unwrap();
    fs::write(&source_path, b"").unwrap();

    Fix::perform_fixes(Rc::clone(&root));

    let target_path = temp
        .path()
        .join("RomRoot")
        .join("!!!_I_AM_A_NAUGHTY_BUG")
        .join("d")
        .join("C");
    assert!(target_path.exists());
    assert_eq!(fs::metadata(target_path).unwrap().len(), 0);

    update_settings(original_settings);
}
