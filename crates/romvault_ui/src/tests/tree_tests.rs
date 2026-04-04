    use super::*;

    #[test]
    fn test_tree_color_from_stats_treats_all_merged_branch_as_grey() {
        let mut stats = rv_core::repair_status::RepairStatus::new();
        stats.total_roms = 2;
        stats.roms_not_collected = 1;
        stats.roms_unneeded = 1;

        assert_eq!(tree_color_from_stats(&stats), egui::Color32::from_rgb(150, 150, 150));
    }

    #[test]
    fn test_tree_icon_idx_from_stats_uses_merged_icon_for_all_merged_branch() {
        let mut stats = rv_core::repair_status::RepairStatus::new();
        stats.total_roms = 2;
        stats.roms_not_collected = 1;
        stats.roms_unneeded = 1;

        assert_eq!(tree_icon_idx_from_stats(&stats), 4);
    }

    #[test]
    fn test_tree_color_from_stats_treats_correct_mia_branch_as_green() {
        let mut stats = rv_core::repair_status::RepairStatus::new();
        stats.total_roms = 2;
        stats.roms_correct = 2;
        stats.roms_correct_mia = 2;

        assert_eq!(tree_color_from_stats(&stats), egui::Color32::from_rgb(0, 200, 0));
    }

    #[test]
    fn test_tree_icon_idx_from_stats_uses_green_icon_for_correct_mia_branch() {
        let mut stats = rv_core::repair_status::RepairStatus::new();
        stats.total_roms = 2;
        stats.roms_correct = 2;
        stats.roms_correct_mia = 2;

        assert_eq!(tree_icon_idx_from_stats(&stats), 3);
    }

    #[test]
    fn test_tree_color_from_stats_treats_missing_mia_branch_as_red() {
        let mut stats = rv_core::repair_status::RepairStatus::new();
        stats.total_roms = 2;
        stats.roms_missing = 2;
        stats.roms_missing_mia = 2;

        assert_eq!(tree_color_from_stats(&stats), egui::Color32::from_rgb(200, 0, 0));
    }

    #[test]
    fn test_tree_icon_idx_from_stats_uses_red_icon_for_missing_mia_branch() {
        let mut stats = rv_core::repair_status::RepairStatus::new();
        stats.total_roms = 2;
        stats.roms_missing = 2;
        stats.roms_missing_mia = 2;

        assert_eq!(tree_icon_idx_from_stats(&stats), 5);
    }

    #[test]
    fn test_tree_color_from_stats_treats_fixable_only_branch_as_cyan() {
        let mut stats = rv_core::repair_status::RepairStatus::new();
        stats.total_roms = 2;
        stats.roms_fixes = 2;

        assert_eq!(tree_color_from_stats(&stats), egui::Color32::from_rgb(0, 200, 200));
    }

    #[test]
    fn test_tree_icon_idx_from_stats_treats_fixable_only_branch_as_special_icon() {
        let mut stats = rv_core::repair_status::RepairStatus::new();
        stats.total_roms = 2;
        stats.roms_fixes = 2;

        assert_eq!(tree_icon_idx_from_stats(&stats), 4);
    }

    #[test]
    fn test_tree_color_from_rep_status_treats_corrupt_can_be_fixed_as_yellow() {
        assert_eq!(
            tree_color_from_rep_status(RepStatus::CorruptCanBeFixed, DatStatus::InDatCollect),
            egui::Color32::from_rgb(200, 200, 0)
        );
    }

    #[test]
    fn test_tree_color_from_rep_status_treats_move_to_corrupt_as_cyan() {
        assert_eq!(
            tree_color_from_rep_status(RepStatus::MoveToCorrupt, DatStatus::InDatCollect),
            egui::Color32::from_rgb(0, 200, 200)
        );
    }

    #[test]
    fn test_tree_color_from_rep_status_treats_needed_for_fix_and_rename_as_cyan() {
        assert_eq!(
            tree_color_from_rep_status(RepStatus::NeededForFix, DatStatus::InDatCollect),
            egui::Color32::from_rgb(0, 200, 200)
        );
        assert_eq!(
            tree_color_from_rep_status(RepStatus::Rename, DatStatus::InDatCollect),
            egui::Color32::from_rgb(0, 200, 200)
        );
    }

    #[test]
    fn test_tree_color_from_rep_status_treats_in_to_sort_variants_as_cyan() {
        assert_eq!(
            tree_color_from_rep_status(RepStatus::InToSort, DatStatus::InToSort),
            egui::Color32::from_rgb(0, 200, 200)
        );
        assert_eq!(
            tree_color_from_rep_status(RepStatus::DirInToSort, DatStatus::InToSort),
            egui::Color32::from_rgb(0, 200, 200)
        );
    }

    #[test]
    fn test_tree_color_from_rep_status_treats_delete_as_red() {
        assert_eq!(
            tree_color_from_rep_status(RepStatus::Delete, DatStatus::NotInDat),
            egui::Color32::from_rgb(200, 0, 0)
        );
    }

    #[test]
    fn test_tree_color_from_rep_status_treats_deleted_as_red() {
        assert_eq!(
            tree_color_from_rep_status(RepStatus::Deleted, DatStatus::NotInDat),
            egui::Color32::from_rgb(200, 0, 0)
        );
    }

    #[test]
    fn test_tree_color_from_rep_status_treats_missing_family_variants_as_red() {
        assert_eq!(
            tree_color_from_rep_status(RepStatus::Corrupt, DatStatus::InDatCollect),
            egui::Color32::from_rgb(200, 0, 0)
        );
        assert_eq!(
            tree_color_from_rep_status(RepStatus::DirCorrupt, DatStatus::InDatCollect),
            egui::Color32::from_rgb(200, 0, 0)
        );
        assert_eq!(
            tree_color_from_rep_status(RepStatus::Incomplete, DatStatus::InDatCollect),
            egui::Color32::from_rgb(200, 0, 0)
        );
    }

    #[test]
    fn test_tree_color_from_rep_status_treats_directory_status_families_consistently() {
        assert_eq!(
            tree_color_from_rep_status(RepStatus::DirCorrect, DatStatus::InDatCollect),
            egui::Color32::from_rgb(0, 200, 0)
        );
        assert_eq!(
            tree_color_from_rep_status(RepStatus::DirMissing, DatStatus::InDatCollect),
            egui::Color32::from_rgb(200, 0, 0)
        );
        assert_eq!(
            tree_color_from_rep_status(RepStatus::DirInToSort, DatStatus::InToSort),
            egui::Color32::from_rgb(0, 200, 200)
        );
        assert_eq!(
            tree_color_from_rep_status(RepStatus::DirUnknown, DatStatus::NotInDat),
            egui::Color32::from_rgb(150, 150, 150)
        );
    }

    #[test]
    fn test_tree_color_from_stats_treats_unknown_only_branch_as_grey() {
        let mut stats = rv_core::repair_status::RepairStatus::new();
        stats.total_roms = 2;
        stats.roms_unknown = 2;

        assert_eq!(tree_color_from_stats(&stats), egui::Color32::from_rgb(150, 150, 150));
    }

    #[test]
    fn test_tree_color_from_rep_status_treats_unknown_as_grey() {
        assert_eq!(
            tree_color_from_rep_status(RepStatus::Unknown, DatStatus::InDatCollect),
            egui::Color32::from_rgb(150, 150, 150)
        );
    }

    #[test]
    fn test_tree_color_from_rep_status_treats_unscanned_as_grey() {
        assert_eq!(
            tree_color_from_rep_status(RepStatus::UnScanned, DatStatus::InDatCollect),
            egui::Color32::from_rgb(150, 150, 150)
        );
    }

    #[test]
    fn test_tree_color_from_rep_status_treats_ignore_as_grey() {
        assert_eq!(
            tree_color_from_rep_status(RepStatus::Ignore, DatStatus::NotInDat),
            egui::Color32::from_rgb(150, 150, 150)
        );
    }

    #[test]
    fn test_tree_icon_idx_from_report_status_treats_unknown_as_grey_icon() {
        assert_eq!(tree_icon_idx_from_report_status(rv_core::enums::ReportStatus::Unknown), 4);
    }

    #[test]
    fn test_tree_icon_idx_from_report_status_treats_ignore_as_grey_icon() {
        assert_eq!(tree_icon_idx_from_report_status(rv_core::enums::ReportStatus::Ignore), 4);
    }

    #[test]
    fn test_tree_icon_idx_from_report_status_treats_merged_as_grey_icon() {
        assert_eq!(tree_icon_idx_from_report_status(rv_core::enums::ReportStatus::NotCollected), 4);
    }

    #[test]
    fn test_tree_icon_idx_from_report_status_treats_corrupt_as_red_icon() {
        assert_eq!(tree_icon_idx_from_report_status(rv_core::enums::ReportStatus::Corrupt), 1);
    }

    #[test]
    fn test_tree_icon_idx_from_report_status_treats_in_to_sort_as_special_icon() {
        assert_eq!(tree_icon_idx_from_report_status(rv_core::enums::ReportStatus::InToSort), 4);
    }

    #[test]
    fn test_tree_icon_idx_from_stats_treats_unknown_only_branch_as_grey_icon() {
        let mut stats = rv_core::repair_status::RepairStatus::new();
        stats.total_roms = 2;
        stats.roms_unknown = 2;

        assert_eq!(tree_icon_idx_from_stats(&stats), 4);
    }

    #[test]
    fn test_set_descendants_expanded_does_not_change_root_and_sets_all_directory_descendants() {
        let root = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
        let child0 = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
        let child1 = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
        let grandchild = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));

        child0.borrow_mut().tree_expanded = false;
        child1.borrow_mut().tree_expanded = true;
        grandchild.borrow_mut().tree_expanded = false;

        child0.borrow_mut().children.push(Rc::clone(&grandchild));
        root.borrow_mut().children.push(Rc::clone(&child0));
        root.borrow_mut().children.push(Rc::clone(&child1));

        RomVaultApp::set_descendants_expanded(&root, true);

        assert!(!root.borrow().tree_expanded);
        assert!(child0.borrow().tree_expanded);
        assert!(child1.borrow().tree_expanded);
        assert!(grandchild.borrow().tree_expanded);
    }

    #[test]
    fn test_expand_descendants_target_uses_first_directory_child_state() {
        let root = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
        let child0 = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
        let child1 = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));

        child0.borrow_mut().tree_expanded = false;
        child1.borrow_mut().tree_expanded = true;

        root.borrow_mut().children.push(Rc::clone(&child0));
        root.borrow_mut().children.push(Rc::clone(&child1));

        assert_eq!(RomVaultApp::expand_descendants_target(&root), Some(true));
    }

    #[test]
    fn test_set_tree_checked_locked_skips_to_sort_primary_and_cache() {
        let root = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
        let primary = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
        let cache = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
        let normal = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));

        primary
            .borrow_mut()
            .to_sort_status_set(rv_core::enums::ToSortDirType::TO_SORT_PRIMARY);
        cache
            .borrow_mut()
            .to_sort_status_set(rv_core::enums::ToSortDirType::TO_SORT_CACHE);

        primary.borrow_mut().tree_checked = TreeSelect::UnSelected;
        cache.borrow_mut().tree_checked = TreeSelect::UnSelected;
        normal.borrow_mut().tree_checked = TreeSelect::UnSelected;

        root.borrow_mut().children.push(Rc::clone(&primary));
        root.borrow_mut().children.push(Rc::clone(&cache));
        root.borrow_mut().children.push(Rc::clone(&normal));

        RomVaultApp::set_tree_checked_locked(&root, true);

        assert_eq!(primary.borrow().tree_checked, TreeSelect::UnSelected);
        assert_eq!(cache.borrow().tree_checked, TreeSelect::UnSelected);
        assert_eq!(normal.borrow().tree_checked, TreeSelect::Locked);
    }

    #[test]
    fn test_expand_selected_ancestors_sets_tree_expanded_on_parent_chain() {
        let root = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
        let child = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
        let grandchild = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));

        child.borrow_mut().parent = Some(Rc::downgrade(&root));
        grandchild.borrow_mut().parent = Some(Rc::downgrade(&child));

        root.borrow_mut().children.push(Rc::clone(&child));
        child.borrow_mut().children.push(Rc::clone(&grandchild));

        root.borrow_mut().tree_expanded = false;
        child.borrow_mut().tree_expanded = false;
        grandchild.borrow_mut().tree_expanded = false;

        let mut app = RomVaultApp::new();
        app.selected_node = Some(Rc::clone(&grandchild));
        app.expand_selected_ancestors();

        assert!(root.borrow().tree_expanded);
        assert!(child.borrow().tree_expanded);
        assert!(!grandchild.borrow().tree_expanded);
    }
