    use super::*;
    use crate::settings::{get_settings, update_settings, Settings};
    use dat_reader::dat_store::{DatDir, DatNode};
    use tempfile::tempdir;

    #[test]
    fn test_recursive_dat_tree_finding_dat() {
        let mut dat = RvDat::new();
        dat.set_data(DatData::DatName, Some("TestDat".to_string()));
        let rv_dat = Rc::new(RefCell::new(dat));

        let t_dir = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
        
        let missing_file = Rc::new(RefCell::new(RvFile::new(FileType::File)));
        {
            let mut mf = missing_file.borrow_mut();
            mf.name = "missing.rom".to_string();
            mf.dat = Some(Rc::clone(&rv_dat));
            mf.set_dat_got_status(DatStatus::InDatCollect, GotStatus::NotGot);
            mf.set_rep_status(RepStatus::Missing);
        }

        let fixable_file = Rc::new(RefCell::new(RvFile::new(FileType::File)));
        {
            let mut ff = fixable_file.borrow_mut();
            ff.name = "fixable.rom".to_string();
            ff.dat = Some(Rc::clone(&rv_dat));
            ff.set_dat_got_status(DatStatus::InDatCollect, GotStatus::NotGot);
            ff.set_rep_status(RepStatus::CanBeFixed);
        }

        t_dir.borrow_mut().child_add(Rc::clone(&missing_file));
        t_dir.borrow_mut().child_add(Rc::clone(&fixable_file));

        let out_dir = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));

        // Test red_only = true (only strictly missing, not fixable)
        let found = FixDatReport::recursive_dat_tree_finding_dat(Rc::clone(&rv_dat), Rc::clone(&t_dir), Rc::clone(&out_dir), true);
        
        assert_eq!(found, 1);
        assert_eq!(out_dir.borrow().children.len(), 1);
        assert_eq!(out_dir.borrow().children[0].borrow().name, "missing.rom");

        // Test red_only = false (all missing/fixable)
        let out_dir_all = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
        let found_all = FixDatReport::recursive_dat_tree_finding_dat(Rc::clone(&rv_dat), Rc::clone(&t_dir), Rc::clone(&out_dir_all), false);
        
        assert_eq!(found_all, 2);
        assert_eq!(out_dir_all.borrow().children.len(), 2);
    }

    #[test]
    fn test_recursive_dat_tree_finding_dat_red_only_excludes_action_family_statuses() {
        let mut dat = RvDat::new();
        dat.set_data(DatData::DatName, Some("TestDat".to_string()));
        let rv_dat = Rc::new(RefCell::new(dat));

        let t_dir = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));

        let action_file = Rc::new(RefCell::new(RvFile::new(FileType::File)));
        {
            let mut file = action_file.borrow_mut();
            file.name = "rename.rom".to_string();
            file.dat = Some(Rc::clone(&rv_dat));
            file.set_dat_got_status(DatStatus::InDatCollect, GotStatus::NotGot);
            file.set_rep_status(RepStatus::Rename);
        }

        t_dir.borrow_mut().child_add(Rc::clone(&action_file));

        let out_dir = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
        let found = FixDatReport::recursive_dat_tree_finding_dat(Rc::clone(&rv_dat), Rc::clone(&t_dir), Rc::clone(&out_dir), true);

        assert_eq!(found, 0);
        assert!(out_dir.borrow().children.is_empty());
    }

    #[test]
    fn test_recursive_dat_tree_descends_into_child_directories_with_bound_dats() {
        let tmp = tempdir().unwrap();

        let mut dat = RvDat::new();
        dat.set_data(DatData::DatName, Some("ChildDat".to_string()));
        dat.set_data(DatData::DatRootFullName, Some("C:\\DatRoot\\ChildDat.dat".to_string()));
        let rv_dat = Rc::new(RefCell::new(dat));

        let root = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
        root.borrow_mut().name = "Root".to_string();

        let child_dir = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
        {
            let mut dir = child_dir.borrow_mut();
            dir.name = "Child".to_string();
            dir.dat = Some(Rc::clone(&rv_dat));
        }

        let missing_file = Rc::new(RefCell::new(RvFile::new(FileType::File)));
        {
            let mut file = missing_file.borrow_mut();
            file.name = "missing.rom".to_string();
            file.dat = Some(Rc::clone(&rv_dat));
            file.set_dat_got_status(DatStatus::InDatCollect, GotStatus::NotGot);
            file.set_rep_status(RepStatus::Missing);
        }

        child_dir.borrow_mut().child_add(Rc::clone(&missing_file));
        root.borrow_mut().child_add(Rc::clone(&child_dir));

        FixDatReport::recursive_dat_tree(tmp.path().to_string_lossy().as_ref(), Rc::clone(&root), false);

        let entries = std::fs::read_dir(tmp.path()).unwrap().collect::<Result<Vec<_>, _>>().unwrap();
        assert_eq!(entries.len(), 1);
        assert!(entries[0].path().file_name().unwrap().to_string_lossy().contains("ChildDat"));
    }

    #[test]
    fn test_recursive_dat_tree_finding_dat_includes_indatmerged_and_indatnodump_entries() {
        let mut dat = RvDat::new();
        dat.set_data(DatData::DatName, Some("TestDat".to_string()));
        let rv_dat = Rc::new(RefCell::new(dat));

        let t_dir = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));

        let merged_file = Rc::new(RefCell::new(RvFile::new(FileType::File)));
        {
            let mut mf = merged_file.borrow_mut();
            mf.name = "merged.rom".to_string();
            mf.dat = Some(Rc::clone(&rv_dat));
            mf.set_dat_got_status(DatStatus::InDatMerged, GotStatus::NotGot);
            mf.set_rep_status(RepStatus::Missing);
        }

        let nodump_file = Rc::new(RefCell::new(RvFile::new(FileType::File)));
        {
            let mut nf = nodump_file.borrow_mut();
            nf.name = "nodump.rom".to_string();
            nf.dat = Some(Rc::clone(&rv_dat));
            nf.set_dat_got_status(DatStatus::InDatNoDump, GotStatus::Corrupt);
            nf.set_rep_status(RepStatus::Corrupt);
        }

        t_dir.borrow_mut().child_add(Rc::clone(&merged_file));
        t_dir.borrow_mut().child_add(Rc::clone(&nodump_file));

        let out_dir = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
        let found = FixDatReport::recursive_dat_tree_finding_dat(
            Rc::clone(&rv_dat),
            Rc::clone(&t_dir),
            Rc::clone(&out_dir),
            false,
        );

        assert_eq!(found, 2);
        let names: Vec<String> = out_dir
            .borrow()
            .children
            .iter()
            .map(|child| child.borrow().name.clone())
            .collect();
        assert!(names.contains(&"merged.rom".to_string()));
        assert!(names.contains(&"nodump.rom".to_string()));
    }

    #[test]
    fn test_fix_single_level_dat_reuses_case_only_matching_parent_directory() {
        let t_dir = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));

        let existing_parent = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
        {
            let mut parent = existing_parent.borrow_mut();
            parent.name = "MYDAT".to_string();
            parent.game = Some(Rc::new(RefCell::new(RvGame::from_description("MYDAT"))));
        }
        t_dir.borrow_mut().child_add(Rc::clone(&existing_parent));

        let loose_file = Rc::new(RefCell::new(RvFile::new(FileType::File)));
        loose_file.borrow_mut().name = "mydat.rom".to_string();
        t_dir.borrow_mut().child_add(Rc::clone(&loose_file));

        FixDatReport::fix_single_level_dat(Rc::clone(&t_dir));

        let t_dir_ref = t_dir.borrow();
        assert_eq!(t_dir_ref.children.len(), 1);
        let parent = t_dir_ref.children[0].borrow();
        assert_eq!(parent.name, "MYDAT");
        assert_eq!(parent.children.len(), 1);
        assert_eq!(parent.children[0].borrow().name, "mydat.rom");
    }

    #[test]
    fn test_remove_unneeded_directories_drops_empty_branches() {
        let mut root = DatDir::new(FileType::Dir);
        let mut keep_dir = DatNode::new_dir("Keep".to_string(), FileType::Dir);
        if let Some(d) = keep_dir.dir_mut() {
            d.add_child(DatNode::new_file("present.bin".to_string(), FileType::File));
        }

        let empty_dir = DatNode::new_dir("Empty".to_string(), FileType::Dir);

        root.add_child(keep_dir);
        root.add_child(empty_dir);

        FixDatReport::remove_unneeded_directories(&mut root);

        assert_eq!(root.children.len(), 1);
        assert_eq!(root.children[0].name, "Keep");
    }

    #[test]
    fn test_archive_directory_flatten_and_remove_unneeded_directories_keep_flattened_entries() {
        let mut root = DatDir::new(FileType::Dir);
        let mut game = DatNode::new_dir("game".to_string(), FileType::Dir);
        if let Some(game_dir) = game.dir_mut() {
            game_dir.d_game = Some(Box::default());

            let empty_dir = DatNode::new_dir("empty".to_string(), FileType::Dir);
            game_dir.add_child(empty_dir);

            let mut nested_dir = DatNode::new_dir("sub".to_string(), FileType::Dir);
            if let Some(nested_child) = nested_dir.dir_mut() {
                nested_child.add_child(DatNode::new_file("missing.bin".to_string(), FileType::File));
            }
            game_dir.add_child(nested_dir);
        }
        root.add_child(game);

        FixDatReport::archive_directory_flatten(&mut root);
        FixDatReport::remove_unneeded_directories(&mut root);

        let game_dir = root.children[0].dir().unwrap();
        let child_names: Vec<_> = game_dir.children.iter().map(|child| child.name.clone()).collect();
        assert_eq!(child_names, vec!["empty/".to_string(), "sub/".to_string(), "sub/missing.bin".to_string()]);
    }

    #[test]
    fn test_dat_relative_parent_for_output_uses_configured_dat_root() {
        let original_settings = get_settings();
        let temp = tempdir().unwrap();
        let settings = Settings {
            dat_root: temp.path().join("CustomDatRoot").to_string_lossy().into_owned(),
            ..Default::default()
        };
        update_settings(settings);

        let dat_path = temp
            .path()
            .join("CustomDatRoot")
            .join("Arcade")
            .join("MAME")
            .join("mame.dat");

        let relative = FixDatReport::dat_relative_parent_for_output(&dat_path.to_string_lossy());
        update_settings(original_settings);

        assert_eq!(relative, "Arcade_MAME");
    }

    #[test]
    fn test_dat_relative_parent_for_output_falls_back_when_not_under_dat_root() {
        let original_settings = get_settings();
        update_settings(Settings::default());

        let relative = FixDatReport::dat_relative_parent_for_output(r"C:\Elsewhere\mame.dat");
        update_settings(original_settings);

        assert_eq!(relative, "Unknown");
    }

    #[test]
    fn test_dat_relative_parent_for_output_matches_dat_root_case_insensitively_on_windows() {
        let original_settings = get_settings();
        let temp = tempdir().unwrap();
        let settings = Settings {
            dat_root: temp.path().join("CustomDatRoot").to_string_lossy().into_owned(),
            ..Default::default()
        };
        update_settings(settings);

        let dat_path = temp
            .path()
            .join("customdatroot")
            .join("Arcade")
            .join("MAME")
            .join("mame.dat")
            .to_string_lossy()
            .into_owned();

        let relative = FixDatReport::dat_relative_parent_for_output(&dat_path);
        update_settings(original_settings);

        assert_eq!(relative, "Arcade_MAME");
    }
