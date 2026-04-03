    use super::*;

    #[test]
    fn test_color_key_sections_use_icon_based_legend_with_many_entries() {
        assert!(color_key_entry_count() >= 50);
        assert!(color_key_sections().iter().any(|section| section.title == "Game List Grid - Resting"));
        assert!(color_key_sections().iter().any(|section| section.title == "ROM Details Grid - Fix Actions"));
        assert!(color_key_sections().iter().any(|section| section.title == "DAT Tree - Folders"));
    }

    #[test]
    fn test_color_key_sections_include_delete_and_tosort_icon_entries() {
        let all_entries: Vec<_> = color_key_sections()
            .iter()
            .flat_map(|section| section.entries.iter())
            .collect();

        assert!(all_entries.iter().any(|entry| entry.icon == "G_Delete.png" && entry.title == "Delete"));
        assert!(all_entries.iter().any(|entry| entry.icon == "R_InToSort_Delete.png" && entry.title == "ToSort Delete"));
        assert!(all_entries.iter().any(|entry| entry.icon == "DirectoryTree5.png" && entry.title == "Folder ToSort"));
    }

    #[test]
    fn test_sam_dialog_exposes_reference_style_input_and_output_options() {
        assert_eq!(SAM_INPUT_OPTIONS.len(), 4);
        assert_eq!(SAM_OUTPUT_OPTIONS.len(), 5);
        assert!(SAM_INPUT_OPTIONS.iter().any(|option| option.label() == "Directory"));
        assert!(SAM_INPUT_OPTIONS.iter().any(|option| option.label() == "Mixed"));
        assert!(SAM_OUTPUT_OPTIONS.iter().any(|option| option.label() == "TorrentZip"));
        assert!(SAM_OUTPUT_OPTIONS.iter().any(|option| option.label() == "7z Zstd"));
        assert!(crate::RomVaultApp::sam_output_kind_supported(SamOutputKind::SevenZipZstd));
        assert!(crate::RomVaultApp::sam_output_kind_support_message(SamOutputKind::SevenZipZstd).is_none());
    }

    #[test]
    fn test_sam_origin_output_mode_counts_as_valid_output_target() {
        let mut app = crate::RomVaultApp::new();
        assert!(!app.sam_has_usable_output_target());

        app.sam_use_origin_output = true;
        assert!(app.sam_has_usable_output_target());
    }
