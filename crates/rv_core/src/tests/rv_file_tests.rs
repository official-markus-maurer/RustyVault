use super::*;
use crate::settings::{get_settings, set_dir_mapping, update_settings, DirMapping, Settings};
use std::cell::RefCell;
use std::panic::AssertUnwindSafe;
use std::rc::Rc;

#[test]
fn test_rvfile_hierarchy() {
    let mut root = RvFile::new(FileType::Dir);
    root.name = "Root".to_string();
    let root_rc = Rc::new(RefCell::new(root));

    let child1 = Rc::new(RefCell::new(RvFile::new(FileType::File)));
    child1.borrow_mut().name = "File1.zip".to_string();
    child1.borrow_mut().parent = Some(Rc::downgrade(&root_rc));

    let child2 = Rc::new(RefCell::new(RvFile::new(FileType::File)));
    child2.borrow_mut().name = "File2.zip".to_string();
    child2.borrow_mut().parent = Some(Rc::downgrade(&root_rc));

    root_rc.borrow_mut().child_add(Rc::clone(&child1));
    root_rc.borrow_mut().child_add(Rc::clone(&child2));

    assert_eq!(root_rc.borrow().children.len(), 2);

    // Test parent linking
    assert!(Rc::ptr_eq(
        &child1.borrow().parent.as_ref().unwrap().upgrade().unwrap(),
        &root_rc
    ));

    // Remove child
    root_rc.borrow_mut().child_remove(0);
    assert_eq!(root_rc.borrow().children.len(), 1);
    assert_eq!(root_rc.borrow().children[0].borrow().name, "File2.zip");
}

#[test]
fn test_file_status_flags() {
    let mut file = RvFile::new(FileType::File);
    assert!(!file.file_status_is(FileStatus::SIZE_FROM_HEADER));

    file.file_status_set(FileStatus::SIZE_FROM_HEADER);
    assert!(file.file_status_is(FileStatus::SIZE_FROM_HEADER));

    file.file_status_clear(FileStatus::SIZE_FROM_HEADER);
    assert!(!file.file_status_is(FileStatus::SIZE_FROM_HEADER));
}

#[test]
fn test_mark_as_missing() {
    let mut root = RvFile::new(FileType::Dir);
    root.dat_status = DatStatus::InDatCollect;

    let child1 = Rc::new(RefCell::new(RvFile::new(FileType::File)));
    child1.borrow_mut().dat_status = DatStatus::NotInDat;

    let child2 = Rc::new(RefCell::new(RvFile::new(FileType::File)));
    child2.borrow_mut().dat_status = DatStatus::InDatCollect;

    root.child_add(Rc::clone(&child1));
    root.child_add(Rc::clone(&child2));

    root.mark_as_missing();

    assert_eq!(root.children.len(), 1);
    assert_eq!(root.children[0].borrow().rep_status(), RepStatus::Missing);
}

#[test]
fn test_get_full_name_uses_dir_mappings() {
    let original_settings = get_settings();
    update_settings(Settings::default());
    set_dir_mapping(DirMapping {
        dir_key: "RustyVault".to_string(),
        dir_path: r"C:\Mapped\Vault".to_string(),
    });

    let root = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
    root.borrow_mut().name = "RustyVault".to_string();

    let folder = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
    folder.borrow_mut().name = "Nintendo".to_string();
    folder.borrow_mut().parent = Some(Rc::downgrade(&root));

    let file = Rc::new(RefCell::new(RvFile::new(FileType::File)));
    file.borrow_mut().name = "game.zip".to_string();
    file.borrow_mut().parent = Some(Rc::downgrade(&folder));

    let full_name = file.borrow().get_full_name();
    update_settings(original_settings);

    assert_eq!(
        std::path::PathBuf::from(full_name),
        std::path::PathBuf::from(r"C:\Mapped\Vault\Nintendo\game.zip")
    );
}

#[test]
fn test_rep_status_reset_uses_rule_ignore_list_without_falling_back_to_global() {
    let original_settings = get_settings();
    let mut settings = Settings::default();
    settings.ignore_files.items = vec!["ignored.bin".to_string()];
    settings.dat_rules.items = vec![crate::settings::DatRule {
        dir_key: "RomVault".to_string(),
        ..Default::default()
    }];
    update_settings(settings);

    let parent = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
    parent.borrow_mut().name = "RomVault".to_string();

    let mut file = RvFile::new(FileType::File);
    file.name = "ignored.bin".to_string();
    file.dat_status = DatStatus::NotInDat;
    file.got_status = GotStatus::Got;
    file.parent = Some(Rc::downgrade(&parent));
    file.rep_status_reset();
    assert_ne!(file.rep_status(), RepStatus::Ignore);

    update_settings(Settings {
        ignore_files: crate::settings::IgnoreFilesWrapper {
            items: vec!["ignored.bin".to_string()],
        },
        ..Settings::default()
    });

    let parent = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
    parent.borrow_mut().name = "NoRule".to_string();

    let mut file = RvFile::new(FileType::File);
    file.name = "ignored.bin".to_string();
    file.dat_status = DatStatus::NotInDat;
    file.got_status = GotStatus::Got;
    file.parent = Some(Rc::downgrade(&parent));
    file.rep_status_reset();
    assert_eq!(file.rep_status(), RepStatus::Ignore);

    update_settings(original_settings);
}

#[test]
fn test_rep_status_reset_treats_indatmerged_missing_file_as_not_collected() {
    let mut file = RvFile::new(FileType::File);
    file.dat_status = DatStatus::InDatMerged;
    file.got_status = dat_reader::enums::GotStatus::NotGot;

    file.rep_status_reset();

    assert_eq!(file.rep_status(), RepStatus::NotCollected);
}

#[test]
fn test_rep_status_reset_treats_indatnodump_corrupt_file_as_unneeded() {
    let mut file = RvFile::new(FileType::File);
    file.dat_status = DatStatus::InDatNoDump;
    file.got_status = dat_reader::enums::GotStatus::Corrupt;

    file.rep_status_reset();

    assert_eq!(file.rep_status(), RepStatus::Corrupt);
}

#[test]
fn test_rep_status_reset_treats_indatmerged_got_file_as_unneeded() {
    let mut file = RvFile::new(FileType::File);
    file.dat_status = DatStatus::InDatMerged;
    file.got_status = dat_reader::enums::GotStatus::Got;

    file.rep_status_reset();

    assert_eq!(file.rep_status(), RepStatus::UnNeeded);
}

#[test]
fn test_rep_status_reset_treats_indatmerged_directory_like_other_indat_directories() {
    let mut dir = RvFile::new(FileType::Dir);
    dir.dat_status = DatStatus::InDatMerged;
    dir.got_status = dat_reader::enums::GotStatus::Got;

    dir.rep_status_reset();

    assert_eq!(dir.rep_status(), RepStatus::DirCorrect);
}

#[test]
fn test_rep_status_reset_treats_corrupt_indat_directory_as_dircorrupt() {
    let mut dir = RvFile::new(FileType::Dir);
    dir.dat_status = DatStatus::InDatCollect;
    dir.got_status = dat_reader::enums::GotStatus::Corrupt;

    dir.rep_status_reset();

    assert_eq!(dir.rep_status(), RepStatus::DirCorrupt);
}

#[test]
fn test_status_mutation_invalidates_ancestor_cached_stats() {
    let root = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
    let child_dir = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
    child_dir.borrow_mut().parent = Some(Rc::downgrade(&root));
    let leaf = Rc::new(RefCell::new(RvFile::new(FileType::File)));
    leaf.borrow_mut().parent = Some(Rc::downgrade(&child_dir));

    root.borrow_mut().child_add(Rc::clone(&child_dir));
    child_dir.borrow_mut().child_add(Rc::clone(&leaf));

    root.borrow_mut().cached_stats = Some(crate::repair_status::RepairStatus::new());
    child_dir.borrow_mut().cached_stats = Some(crate::repair_status::RepairStatus::new());
    leaf.borrow_mut().cached_stats = Some(crate::repair_status::RepairStatus::new());

    leaf.borrow_mut().set_rep_status(RepStatus::Missing);

    assert!(leaf.borrow().cached_stats.is_none());
    assert!(child_dir.borrow().cached_stats.is_none());
    assert!(root.borrow().cached_stats.is_none());
}

#[test]
fn test_status_mutation_invalidates_ancestor_dir_status_to_unknown() {
    let root = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
    let child_dir = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
    child_dir.borrow_mut().parent = Some(Rc::downgrade(&root));
    let leaf = Rc::new(RefCell::new(RvFile::new(FileType::File)));
    leaf.borrow_mut().parent = Some(Rc::downgrade(&child_dir));

    root.borrow_mut().child_add(Rc::clone(&child_dir));
    child_dir.borrow_mut().child_add(Rc::clone(&leaf));

    root.borrow_mut().dir_status = Some(ReportStatus::Correct);
    child_dir.borrow_mut().dir_status = Some(ReportStatus::Correct);

    leaf.borrow_mut().set_rep_status(RepStatus::Missing);

    assert_eq!(child_dir.borrow().dir_status, Some(ReportStatus::Unknown));
    assert_eq!(root.borrow().dir_status, Some(ReportStatus::Unknown));
}

#[test]
fn test_status_mutation_does_not_panic_when_ancestor_is_already_borrowed() {
    let root = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
    let child_dir = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
    child_dir.borrow_mut().parent = Some(Rc::downgrade(&root));
    let leaf = Rc::new(RefCell::new(RvFile::new(FileType::File)));
    leaf.borrow_mut().parent = Some(Rc::downgrade(&child_dir));

    root.borrow_mut().child_add(Rc::clone(&child_dir));
    child_dir.borrow_mut().child_add(Rc::clone(&leaf));

    let _borrowed_parent = child_dir.borrow_mut();
    let result = std::panic::catch_unwind(AssertUnwindSafe(|| {
        leaf.borrow_mut().set_rep_status(RepStatus::Missing);
    }));

    assert!(result.is_ok());
    assert_eq!(leaf.borrow().rep_status(), RepStatus::Missing);
}
