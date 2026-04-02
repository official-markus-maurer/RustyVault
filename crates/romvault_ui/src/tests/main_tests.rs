    use super::*;
    use rv_core::rv_file::RvFile;
    use dat_reader::enums::FileType;
    use std::fs;
    use std::fs::File;
    use std::path::Path;
    use std::rc::Rc;
    use std::cell::RefCell;
    use sevenz_rust::{Archive, EncoderMethod, Password};
    use tempfile::tempdir;
    use crate::utils::get_full_node_path;

    #[test]
    fn test_get_full_node_path() {
        let root = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
        root.borrow_mut().name = "RustyVault".to_string();

        let sub_dir = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
        sub_dir.borrow_mut().name = "MAME".to_string();
        sub_dir.borrow_mut().parent = Some(Rc::downgrade(&root));
        
        let game = Rc::new(RefCell::new(RvFile::new(FileType::Zip)));
        game.borrow_mut().name = "pacman.zip".to_string();
        game.borrow_mut().parent = Some(Rc::downgrade(&sub_dir));

        sub_dir.borrow_mut().child_add(Rc::clone(&game));
        root.borrow_mut().child_add(Rc::clone(&sub_dir));

        let path = get_full_node_path(Rc::clone(&game));
        assert_eq!(path, "RustyVault\\MAME\\pacman.zip");
    }

    #[test]
    fn test_branch_has_selected_nodes_finds_selected_descendant() {
        let root = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
        root.borrow_mut().tree_checked = TreeSelect::UnSelected;

        let child_dir = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
        child_dir.borrow_mut().tree_checked = TreeSelect::UnSelected;

        let selected_file = Rc::new(RefCell::new(RvFile::new(FileType::File)));
        selected_file.borrow_mut().tree_checked = TreeSelect::Selected;

        child_dir.borrow_mut().child_add(Rc::clone(&selected_file));
        root.borrow_mut().child_add(Rc::clone(&child_dir));

        assert!(RomVaultApp::branch_has_selected_nodes(&root.borrow()));
    }

    #[test]
    fn test_ui_missing_count_excludes_unknown_and_not_collected() {
        let mut stats = rv_core::repair_status::RepairStatus::new();
        stats.roms_missing = 2;
        stats.roms_missing_mia = 1;
        stats.roms_fixes = 1;
        stats.roms_unknown = 2;
        stats.roms_not_collected = 3;

        assert_eq!(ui_missing_count(&stats), 3);
    }

    #[test]
    fn test_ui_fixable_count_includes_unneeded_but_excludes_not_collected() {
        let mut stats = rv_core::repair_status::RepairStatus::new();
        stats.roms_fixes = 1;
        stats.roms_unknown = 2;
        stats.roms_not_collected = 3;
        stats.roms_unneeded = 4;

        assert_eq!(ui_fixable_count(&stats), 7);
    }

    #[test]
    fn test_current_fixable_count_counts_only_selected_actionable_statuses() {
        let root = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
        root.borrow_mut().tree_checked = TreeSelect::Selected;

        let selected_fix = Rc::new(RefCell::new(RvFile::new(FileType::File)));
        {
            let mut file = selected_fix.borrow_mut();
            file.tree_checked = TreeSelect::Selected;
            file.set_rep_status(rv_core::enums::RepStatus::Delete);
            file.parent = Some(Rc::downgrade(&root));
        }

        let selected_source = Rc::new(RefCell::new(RvFile::new(FileType::File)));
        {
            let mut file = selected_source.borrow_mut();
            file.tree_checked = TreeSelect::Selected;
            file.set_rep_status(rv_core::enums::RepStatus::NeededForFix);
            file.parent = Some(Rc::downgrade(&root));
        }

        let unselected_fix = Rc::new(RefCell::new(RvFile::new(FileType::File)));
        {
            let mut file = unselected_fix.borrow_mut();
            file.tree_checked = TreeSelect::UnSelected;
            file.set_rep_status(rv_core::enums::RepStatus::Delete);
            file.parent = Some(Rc::downgrade(&root));
        }

        let selected_unneeded = Rc::new(RefCell::new(RvFile::new(FileType::File)));
        {
            let mut file = selected_unneeded.borrow_mut();
            file.tree_checked = TreeSelect::Selected;
            file.set_rep_status(rv_core::enums::RepStatus::UnNeeded);
            file.parent = Some(Rc::downgrade(&root));
        }

        root.borrow_mut().child_add(Rc::clone(&selected_fix));
        root.borrow_mut().child_add(Rc::clone(&selected_source));
        root.borrow_mut().child_add(Rc::clone(&unselected_fix));
        root.borrow_mut().child_add(Rc::clone(&selected_unneeded));

        assert_eq!(current_fixable_count(Rc::clone(&root)), 2);
    }

    #[test]
    fn test_collect_sam_work_items_supports_mixed_sources_and_recursion() {
        let temp = tempdir().unwrap();
        let dir_source = temp.path().join("SetDir");
        let nested_dir = temp.path().join("Nested");
        fs::create_dir_all(&dir_source).unwrap();
        fs::create_dir_all(&nested_dir).unwrap();
        fs::write(temp.path().join("game.zip"), b"zip").unwrap();
        fs::write(temp.path().join("game.7z"), b"7z").unwrap();
        fs::write(nested_dir.join("child.zip"), b"zip").unwrap();

        let mut items = Vec::new();
        let mut seen = std::collections::HashSet::new();
        RomVaultApp::collect_sam_work_items(
            temp.path(),
            true,
            crate::dialogs::SamInputKind::Mixed,
            &mut items,
            &mut seen,
        );

        assert!(items.iter().any(|item| item.ends_with("SetDir")));
        assert!(items.iter().any(|item| item.ends_with("game.zip")));
        assert!(items.iter().any(|item| item.ends_with("game.7z")));
        assert!(items.iter().any(|item| item.ends_with("child.zip")));
    }

    #[test]
    fn test_sam_output_path_uses_expected_extension_for_directory_source() {
        let output = RomVaultApp::sam_output_path(
            Path::new("C:\\Output"),
            Path::new("C:\\Input\\SetDir"),
            crate::dialogs::SamOutputKind::ZipZstd,
        )
        .unwrap();

        assert!(output.ends_with("SetDir.zip"));
    }

    #[test]
    fn test_sam_output_path_uses_7z_extension_for_sevenzip_lzma() {
        let output = RomVaultApp::sam_output_path(
            Path::new("C:\\Output"),
            Path::new("C:\\Input\\SetDir"),
            crate::dialogs::SamOutputKind::SevenZipLzma,
        )
        .unwrap();

        assert!(output.ends_with("SetDir.7z"));
        assert_eq!(
            RomVaultApp::sam_output_extension(crate::dialogs::SamOutputKind::SevenZipZstd),
            Some("7z")
        );
    }

    #[test]
    fn test_sam_output_root_for_origin_mode_uses_source_parent() {
        let file_root = RomVaultApp::sam_output_root_for_source(
            Path::new("C:\\Input\\game.zip"),
            "C:\\Output",
            true,
        )
        .unwrap();
        let dir_root = RomVaultApp::sam_output_root_for_source(
            Path::new("C:\\Input\\SetDir"),
            "C:\\Output",
            true,
        )
        .unwrap();

        assert_eq!(file_root, Path::new("C:\\Input"));
        assert_eq!(dir_root, Path::new("C:\\Input"));
    }

    #[test]
    fn test_sam_process_7z_item_creates_archive_from_directory_source() {
        let temp = tempdir().unwrap();
        let source_dir = temp.path().join("SetDir");
        fs::create_dir_all(source_dir.join("sub")).unwrap();
        fs::write(source_dir.join("a.bin"), b"aaaa").unwrap();
        fs::write(source_dir.join("sub").join("b.bin"), b"bbbb").unwrap();
        let output_path = temp.path().join("SetDir.7z");

        let status = RomVaultApp::sam_process_7z_item(
            &source_dir,
            SamSourceKind::Directory,
            &output_path,
            crate::dialogs::SamOutputKind::SevenZipLzma,
            true,
            &ProcessControl::new(),
        )
        .unwrap();

        assert_eq!(status, "SEVENZIP_LZMA_CREATED");
        assert!(output_path.exists());
        assert!(RomVaultApp::sam_verify_7z_output(&output_path).is_ok());

        let mut archive_file = File::open(&output_path).unwrap();
        let password = Password::empty();
        let archive = Archive::read(&mut archive_file, &password).unwrap();
        assert!(archive.is_solid);
        assert_eq!(archive.blocks[0].coders[0].encoder_method_id(), EncoderMethod::ID_LZMA);
    }

    #[test]
    fn test_sam_process_7z_zstd_item_creates_archive_from_directory_source() {
        let temp = tempdir().unwrap();
        let source_dir = temp.path().join("SetDir");
        fs::create_dir_all(source_dir.join("sub")).unwrap();
        fs::write(source_dir.join("a.bin"), b"aaaa").unwrap();
        fs::write(source_dir.join("sub").join("b.bin"), b"bbbb").unwrap();
        let output_path = temp.path().join("SetDir.7z");

        let status = RomVaultApp::sam_process_7z_item(
            &source_dir,
            SamSourceKind::Directory,
            &output_path,
            crate::dialogs::SamOutputKind::SevenZipZstd,
            true,
            &ProcessControl::new(),
        )
        .unwrap();

        assert_eq!(status, "SEVENZIP_ZSTD_CREATED");
        assert!(output_path.exists());
        assert!(RomVaultApp::sam_verify_7z_output(&output_path).is_ok());

        let mut archive_file = File::open(&output_path).unwrap();
        let password = Password::empty();
        let archive = Archive::read(&mut archive_file, &password).unwrap();
        assert!(archive.is_solid);
        assert_eq!(archive.blocks[0].coders[0].encoder_method_id(), EncoderMethod::ID_ZSTD);
    }

    #[test]
    fn test_sam_process_7z_zstd_item_is_byte_stable_for_same_input() {
        let temp = tempdir().unwrap();
        let source_dir = temp.path().join("SetDir");
        fs::create_dir_all(source_dir.join("sub")).unwrap();
        fs::write(source_dir.join("a.bin"), b"aaaa").unwrap();
        fs::write(source_dir.join("sub").join("b.bin"), b"bbbb").unwrap();
        let first_output = temp.path().join("SetDir-a.7z");
        let second_output = temp.path().join("SetDir-b.7z");

        let first_status = RomVaultApp::sam_process_7z_item(
            &source_dir,
            SamSourceKind::Directory,
            &first_output,
            crate::dialogs::SamOutputKind::SevenZipZstd,
            true,
            &ProcessControl::new(),
        )
        .unwrap();
        let second_status = RomVaultApp::sam_process_7z_item(
            &source_dir,
            SamSourceKind::Directory,
            &second_output,
            crate::dialogs::SamOutputKind::SevenZipZstd,
            true,
            &ProcessControl::new(),
        )
        .unwrap();

        assert_eq!(first_status, "SEVENZIP_ZSTD_CREATED");
        assert_eq!(second_status, "SEVENZIP_ZSTD_CREATED");
        assert_eq!(fs::read(first_output).unwrap(), fs::read(second_output).unwrap());
    }
