    use super::*;

    #[test]
    fn test_game_summary_bucket_treats_not_collected_as_merged() {
        assert_eq!(game_summary_bucket(RepStatus::NotCollected), Some(RomStatusBucket::Merged));
        assert_eq!(game_summary_bucket(RepStatus::UnNeeded), Some(RomStatusBucket::Merged));
        assert_eq!(game_summary_bucket(RepStatus::Unknown), Some(RomStatusBucket::Unknown));
    }

    #[test]
    fn test_game_row_color_treats_not_collected_like_merged_statuses() {
        assert_eq!(game_row_color(RepStatus::NotCollected), egui::Color32::from_rgb(60, 60, 60));
        assert_eq!(game_row_color(RepStatus::UnNeeded), egui::Color32::from_rgb(60, 60, 60));
    }

    #[test]
    fn test_game_row_color_treats_corrupt_can_be_fixed_as_yellow() {
        assert_eq!(game_row_color(RepStatus::CorruptCanBeFixed), egui::Color32::from_rgb(80, 80, 40));
    }

    #[test]
    fn test_game_summary_bucket_treats_deleted_as_fixable() {
        assert_eq!(game_summary_bucket(RepStatus::Deleted), Some(RomStatusBucket::Fixes));
    }

    #[test]
    fn test_game_summary_bucket_treats_in_to_sort_as_fixable() {
        assert_eq!(game_summary_bucket(RepStatus::InToSort), Some(RomStatusBucket::Fixes));
    }

    #[test]
    fn test_game_row_color_treats_missing_family_variants_as_red() {
        assert_eq!(game_row_color(RepStatus::Corrupt), egui::Color32::from_rgb(80, 40, 40));
        assert_eq!(game_row_color(RepStatus::DirCorrupt), egui::Color32::from_rgb(80, 40, 40));
        assert_eq!(game_row_color(RepStatus::Incomplete), egui::Color32::from_rgb(80, 40, 40));
    }

    #[test]
    fn test_game_and_rom_row_colors_treat_directory_status_families_consistently() {
        assert_eq!(game_row_color(RepStatus::DirCorrect), egui::Color32::from_rgb(40, 80, 40));
        assert_eq!(game_row_color(RepStatus::DirMissing), egui::Color32::from_rgb(80, 40, 40));
        assert_eq!(game_row_color(RepStatus::DirInToSort), egui::Color32::from_rgb(40, 80, 80));
        assert_eq!(game_row_color(RepStatus::DirUnknown), egui::Color32::from_rgb(60, 60, 60));
        assert_eq!(rom_row_color(RepStatus::DirCorrect), egui::Color32::from_rgb(40, 80, 40));
        assert_eq!(rom_row_color(RepStatus::DirMissing), egui::Color32::from_rgb(80, 40, 40));
        assert_eq!(rom_row_color(RepStatus::DirInToSort), egui::Color32::from_rgb(40, 80, 80));
        assert_eq!(rom_row_color(RepStatus::DirUnknown), egui::Color32::from_rgb(60, 60, 60));
    }

    #[test]
    fn test_game_row_color_treats_needed_for_fix_and_rename_as_cyan() {
        assert_eq!(game_row_color(RepStatus::NeededForFix), egui::Color32::from_rgb(40, 80, 80));
        assert_eq!(game_row_color(RepStatus::Rename), egui::Color32::from_rgb(40, 80, 80));
    }

    #[test]
    fn test_game_and_rom_row_colors_treat_deleted_as_delete_red() {
        assert_eq!(game_row_color(RepStatus::Deleted), egui::Color32::from_rgb(120, 0, 0));
        assert_eq!(rom_row_color(RepStatus::Deleted), egui::Color32::from_rgb(120, 0, 0));
    }

    #[test]
    fn test_game_and_rom_row_colors_treat_unscanned_as_grey() {
        assert_eq!(game_row_color(RepStatus::UnScanned), egui::Color32::from_rgb(60, 60, 60));
        assert_eq!(rom_row_color(RepStatus::UnScanned), egui::Color32::from_rgb(60, 60, 60));
    }

    #[test]
    fn test_game_and_rom_row_colors_treat_ignore_as_grey() {
        assert_eq!(game_row_color(RepStatus::Ignore), egui::Color32::from_rgb(60, 60, 60));
        assert_eq!(rom_row_color(RepStatus::Ignore), egui::Color32::from_rgb(60, 60, 60));
    }

    #[test]
    fn test_game_summary_bucket_treats_directory_status_families_consistently() {
        assert_eq!(game_summary_bucket(RepStatus::DirCorrect), Some(RomStatusBucket::Correct));
        assert_eq!(game_summary_bucket(RepStatus::DirMissing), Some(RomStatusBucket::Missing));
        assert_eq!(game_summary_bucket(RepStatus::DirInToSort), Some(RomStatusBucket::Fixes));
        assert_eq!(game_summary_bucket(RepStatus::DirUnknown), Some(RomStatusBucket::Unknown));
    }

    #[test]
    fn test_game_summary_bucket_treats_unscanned_as_unknown() {
        assert_eq!(game_summary_bucket(RepStatus::UnScanned), Some(RomStatusBucket::Unknown));
    }

    #[test]
    fn test_rom_status_icon_idx_uses_dedicated_icons_for_specific_action_statuses() {
        assert_eq!(rom_status_icon_idx(RepStatus::InToSort), 10);
        assert_eq!(rom_status_icon_idx(RepStatus::NeededForFix), 11);
        assert_eq!(rom_status_icon_idx(RepStatus::Rename), 12);
        assert_eq!(rom_status_icon_idx(RepStatus::NotCollected), 14);
    }

    #[test]
    fn test_rom_status_icon_idx_uses_dedicated_icons_for_corrupt_incomplete_and_unscanned() {
        assert_eq!(rom_status_icon_idx(RepStatus::Corrupt), 17);
        assert_eq!(rom_status_icon_idx(RepStatus::Incomplete), 18);
        assert_eq!(rom_status_icon_idx(RepStatus::UnScanned), 19);
    }

    #[test]
    fn test_rom_status_icon_idx_uses_dedicated_ignore_icon() {
        assert_eq!(rom_status_icon_idx(RepStatus::Ignore), 20);
    }

    #[test]
    fn test_game_row_color_treats_in_to_sort_as_cyan() {
        assert_eq!(game_row_color(RepStatus::InToSort), egui::Color32::from_rgb(40, 80, 80));
    }

    #[test]
    fn test_rom_row_color_treats_missing_family_variants_as_red() {
        assert_eq!(rom_row_color(RepStatus::Corrupt), egui::Color32::from_rgb(80, 40, 40));
        assert_eq!(rom_row_color(RepStatus::DirCorrupt), egui::Color32::from_rgb(80, 40, 40));
        assert_eq!(rom_row_color(RepStatus::Incomplete), egui::Color32::from_rgb(80, 40, 40));
    }

    #[test]
    fn test_rom_row_color_treats_in_to_sort_as_cyan() {
        assert_eq!(rom_row_color(RepStatus::InToSort), egui::Color32::from_rgb(40, 80, 80));
    }

    #[test]
    fn test_rom_clipboard_text_truncates_hashes_like_reference() {
        let mut rom = RvFile::new(FileType::File);
        rom.name = "rom.bin".to_string();
        rom.size = Some(123);
        rom.crc = Some(vec![0x12, 0x34, 0x56, 0x78, 0x9A]);
        rom.sha1 = Some(vec![0xAA; 25]);
        rom.md5 = Some(vec![0xBB; 20]);

        let crc = rom_clipboard_text(&rom, RomGridCopyColumn::Crc32).unwrap();
        assert_eq!(crc.len(), 8);

        let sha1 = rom_clipboard_text(&rom, RomGridCopyColumn::Sha1).unwrap();
        assert_eq!(sha1.len(), 40);

        let md5 = rom_clipboard_text(&rom, RomGridCopyColumn::Md5).unwrap();
        assert_eq!(md5.len(), 32);

        let got = rom_clipboard_text(&rom, RomGridCopyColumn::Got).unwrap();
        assert!(got.contains("Name : rom.bin"));
        assert!(got.contains("Size : 123"));
        assert!(got.contains("CRC32: "));
    }

    #[test]
    fn test_game_clipboard_text_matches_reference_columns() {
        let mut node = RvFile::new(FileType::Dir);
        node.name = "GameA".to_string();
        node.file_mod_time_stamp = 19961224233200;

        let type_text = game_clipboard_text(&node, "Desc", GameGridCopyColumn::Type).unwrap();
        assert!(type_text.contains("GameA"));
        assert!(type_text.ends_with('\n'));

        let status_text = game_clipboard_text(&node, "MyDesc", GameGridCopyColumn::RomStatus).unwrap();
        assert!(status_text.contains("Name : GameA"));
        assert!(status_text.contains("Desc : MyDesc"));

        let desc_text = game_clipboard_text(&node, "MyDesc", GameGridCopyColumn::Description).unwrap();
        assert_eq!(desc_text, "MyDesc");
    }

    #[test]
    fn test_split_args_windows_style_respects_quotes() {
        let args = split_args_windows_style(r#"-a "hello world" -b \"x\""#);
        assert_eq!(args, vec!["-a", "hello world", "-b", "\"x\""]);
    }

    #[test]
    fn test_grid_visibility_flags_from_stats_keeps_fix_and_merged_separate_from_missing() {
        let mut stats = rv_core::repair_status::RepairStatus::new();
        stats.total_roms = 3;
        stats.roms_correct = 1;
        stats.roms_fixes = 1;
        stats.roms_not_collected = 1;

        let flags = grid_visibility_flags_from_stats(&stats);

        assert!(!flags.correct);
        assert!(!flags.missing);
        assert!(flags.fixes);
        assert!(!flags.merged);
        assert!(!flags.unknown);
    }

    #[test]
    fn test_grid_visibility_flags_from_stats_marks_all_merged_branch_as_merged() {
        let mut stats = rv_core::repair_status::RepairStatus::new();
        stats.total_roms = 2;
        stats.roms_not_collected = 1;
        stats.roms_unneeded = 1;

        let flags = grid_visibility_flags_from_stats(&stats);

        assert!(flags.merged);
        assert!(flags.fixes);
        assert!(!flags.correct);
        assert!(!flags.unknown);
    }

    #[test]
    fn test_grid_visibility_flags_from_stats_treats_fix_only_branch_as_special_action_state() {
        let mut stats = rv_core::repair_status::RepairStatus::new();
        stats.total_roms = 2;
        stats.roms_fixes = 2;

        let flags = grid_visibility_flags_from_stats(&stats);

        assert!(flags.fixes);
        assert!(flags.mia);
        assert!(!flags.correct);
        assert!(!flags.missing);
    }

    #[test]
    fn test_grid_visibility_flags_from_stats_treats_unknown_only_branch_as_unknown_not_fixable() {
        let mut stats = rv_core::repair_status::RepairStatus::new();
        stats.total_roms = 2;
        stats.roms_unknown = 2;

        let flags = grid_visibility_flags_from_stats(&stats);

        assert!(flags.unknown);
        assert!(!flags.fixes);
        assert!(!flags.correct);
        assert!(!flags.merged);
    }

    #[test]
    fn test_grid_visibility_flags_from_report_status_treats_corrupt_as_missing() {
        let flags = grid_visibility_flags_from_report_status(rv_core::enums::ReportStatus::Corrupt);

        assert!(flags.missing);
        assert!(!flags.correct);
        assert!(!flags.merged);
    }

    #[test]
    fn test_grid_visibility_flags_from_report_status_treats_in_to_sort_as_fixable() {
        let flags = grid_visibility_flags_from_report_status(rv_core::enums::ReportStatus::InToSort);

        assert!(flags.fixes);
        assert!(flags.mia);
        assert!(!flags.missing);
    }

    #[test]
    fn test_grid_visibility_flags_from_report_status_treats_unneeded_as_fixable_merged() {
        let flags = grid_visibility_flags_from_report_status(rv_core::enums::ReportStatus::UnNeeded);

        assert!(flags.fixes);
        assert!(flags.merged);
        assert!(!flags.missing);
    }

    #[test]
    fn test_grid_visibility_flags_from_report_status_treats_ignore_as_neutral_unknown() {
        let flags = grid_visibility_flags_from_report_status(rv_core::enums::ReportStatus::Ignore);

        assert!(flags.unknown);
        assert!(!flags.correct);
        assert!(!flags.missing);
        assert!(!flags.fixes);
        assert!(!flags.merged);
    }
