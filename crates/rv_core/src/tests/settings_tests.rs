    use super::*;

    fn with_settings_test_state(test: impl FnOnce()) {
        let original = get_settings();
        update_settings(Settings::default());
        test();
        update_settings(original);
    }

    #[test]
    fn test_find_rule_returns_exact_match() {
        with_settings_test_state(|| {
            let mut rule = DatRule::default();
            rule.dir_key = "DatRoot\\Arcade\\MAME".to_string();
            rule.single_archive = true;
            set_rule(rule);

            let found = find_rule("DatRoot\\Arcade\\MAME");
            assert!(found.single_archive);
            assert_eq!(found.dir_key, "DatRoot\\Arcade\\MAME");
        });
    }

    #[test]
    fn test_find_rule_walks_up_to_closest_parent_rule() {
        with_settings_test_state(|| {
            let mut parent_rule = DatRule::default();
            parent_rule.dir_key = "DatRoot\\Arcade".to_string();
            parent_rule.use_description_as_dir_name = true;
            set_rule(parent_rule);

            let found = find_rule("DatRoot\\Arcade\\MAME\\Clones");
            assert!(found.use_description_as_dir_name);
            assert_eq!(found.dir_key, "DatRoot\\Arcade");
        });
    }

    #[test]
    fn test_find_rule_normalizes_path_separators_before_lookup() {
        with_settings_test_state(|| {
            let mut rule = DatRule::default();
            rule.dir_key = "DatRoot\\Console".to_string();
            rule.use_id_for_name = true;
            set_rule(rule);

            let found = find_rule("DatRoot/Console/GameBoy");
            assert!(found.use_id_for_name);
            assert_eq!(found.dir_key, "DatRoot\\Console");
        });
    }

    #[test]
    fn test_find_rule_returns_default_for_missing_path() {
        with_settings_test_state(|| {
            let found = find_rule("DatRoot\\Unknown");
            assert_eq!(found.dir_key, "DatRoot\\Unknown");
            assert_eq!(found.compression, dat_reader::enums::FileType::Zip);
            assert!(!found.single_archive);
        });
    }

    #[test]
    fn test_set_rule_normalizes_dir_key_and_replaces_equivalent_path() {
        with_settings_test_state(|| {
            let mut first_rule = DatRule::default();
            first_rule.dir_key = "DatRoot/Console".to_string();
            first_rule.single_archive = true;
            set_rule(first_rule);

            let mut replacement_rule = DatRule::default();
            replacement_rule.dir_key = "DatRoot\\Console".to_string();
            replacement_rule.use_id_for_name = true;
            set_rule(replacement_rule);

            let settings = get_settings();
            assert_eq!(settings.dat_rules.items.len(), 1);
            assert_eq!(settings.dat_rules.items[0].dir_key, "DatRoot\\Console");
            assert!(settings.dat_rules.items[0].use_id_for_name);
            assert!(!settings.dat_rules.items[0].single_archive);
        });
    }

    #[test]
    fn test_find_rule_trims_trailing_separators() {
        with_settings_test_state(|| {
            let mut rule = DatRule::default();
            rule.dir_key = "DatRoot\\Arcade".to_string();
            rule.single_archive = true;
            set_rule(rule);

            let found = find_rule("\\DatRoot\\Arcade\\");
            assert!(found.single_archive);
            assert_eq!(found.dir_key, "DatRoot\\Arcade");
        });
    }

    #[test]
    fn test_find_rule_is_case_insensitive_on_windows_style_keys() {
        with_settings_test_state(|| {
            let mut rule = DatRule::default();
            rule.dir_key = "DatRoot\\Arcade".to_string();
            rule.single_archive = true;
            set_rule(rule);

            let found = find_rule("datroot\\arcade\\mame");
            assert!(found.single_archive);
            assert_eq!(found.dir_key, "DatRoot\\Arcade");
        });
    }

    #[test]
    fn test_find_dir_mapping_returns_exact_match() {
        with_settings_test_state(|| {
            set_dir_mapping(DirMapping {
                dir_key: "DatRoot\\Arcade".to_string(),
                dir_path: r"C:\Roms\Arcade".to_string(),
            });

            let found = find_dir_mapping("DatRoot\\Arcade").unwrap();
            assert_eq!(std::path::PathBuf::from(found), std::path::PathBuf::from(r"C:\Roms\Arcade"));
        });
    }

    #[test]
    fn test_find_dir_mapping_walks_up_to_parent_and_appends_suffix() {
        with_settings_test_state(|| {
            set_dir_mapping(DirMapping {
                dir_key: "DatRoot".to_string(),
                dir_path: r"C:\Roms".to_string(),
            });

            let found = find_dir_mapping("DatRoot\\Arcade\\MAME").unwrap();
            assert_eq!(std::path::PathBuf::from(found), std::path::PathBuf::from(r"C:\Roms\Arcade\MAME"));
        });
    }

    #[test]
    fn test_find_dir_mapping_normalizes_separators_and_trims_edges() {
        with_settings_test_state(|| {
            set_dir_mapping(DirMapping {
                dir_key: "DatRoot\\Console".to_string(),
                dir_path: r"D:\Sets\Console".to_string(),
            });

            let found = find_dir_mapping("\\DatRoot/Console/GameBoy\\").unwrap();
            assert_eq!(std::path::PathBuf::from(found), std::path::PathBuf::from(r"D:\Sets\Console\GameBoy"));
        });
    }

    #[test]
    fn test_set_dir_mapping_replaces_equivalent_normalized_key() {
        with_settings_test_state(|| {
            set_dir_mapping(DirMapping {
                dir_key: "DatRoot/Console".to_string(),
                dir_path: r"C:\Old".to_string(),
            });
            set_dir_mapping(DirMapping {
                dir_key: "DatRoot\\Console".to_string(),
                dir_path: r"C:\New".to_string(),
            });

            let settings = get_settings();
            let matching: Vec<_> = settings
                .dir_mappings
                .items
                .iter()
                .filter(|m| normalize_dir_key(&m.dir_key) == "DatRoot\\Console")
                .collect();
            assert_eq!(matching.len(), 1);
            assert_eq!(matching[0].dir_key, "DatRoot\\Console");
            assert_eq!(std::path::PathBuf::from(&matching[0].dir_path), std::path::PathBuf::from(r"C:\New"));
        });
    }

    #[test]
    fn test_set_dir_mapping_replaces_equivalent_key_with_different_case() {
        with_settings_test_state(|| {
            set_dir_mapping(DirMapping {
                dir_key: "DatRoot\\Console".to_string(),
                dir_path: r"C:\Old".to_string(),
            });
            set_dir_mapping(DirMapping {
                dir_key: "datroot\\console".to_string(),
                dir_path: r"C:\New".to_string(),
            });

            let settings = get_settings();
            let matching: Vec<_> = settings
                .dir_mappings
                .items
                .iter()
                .filter(|m| logical_dir_key_eq(&normalize_dir_key(&m.dir_key), "DatRoot\\Console"))
                .collect();
            assert_eq!(matching.len(), 1);
            assert_eq!(std::path::PathBuf::from(&matching[0].dir_path), std::path::PathBuf::from(r"C:\New"));
        });
    }

    #[test]
    fn test_find_mapping_for_physical_path_prefers_longest_matching_prefix() {
        with_settings_test_state(|| {
            set_dir_mapping(DirMapping {
                dir_key: "RustyVault".to_string(),
                dir_path: r"C:\Root".to_string(),
            });
            set_dir_mapping(DirMapping {
                dir_key: "RustyVault\\Nintendo".to_string(),
                dir_path: r"C:\Root\Nintendo".to_string(),
            });

            let (dir_key, mapping_path) =
                find_mapping_for_physical_path(std::path::Path::new(r"C:\Root\Nintendo\game.zip")).unwrap();
            assert_eq!(dir_key, "RustyVault\\Nintendo");
            assert_eq!(mapping_path, std::path::PathBuf::from(r"C:\Root\Nintendo"));
        });
    }

    #[test]
    fn test_find_mapping_for_physical_path_is_case_insensitive_on_windows_paths() {
        with_settings_test_state(|| {
            set_dir_mapping(DirMapping {
                dir_key: "RustyVault\\Nintendo".to_string(),
                dir_path: r"C:\Root\Nintendo".to_string(),
            });

            let (dir_key, mapping_path) =
                find_mapping_for_physical_path(std::path::Path::new(r"c:\root\nintendo\GAME.zip")).unwrap();
            assert_eq!(dir_key, "RustyVault\\Nintendo");
            assert_eq!(mapping_path, std::path::PathBuf::from(r"C:\Root\Nintendo"));
        });
    }

    #[test]
    fn test_strip_physical_prefix_is_case_insensitive_on_windows_paths() {
        let relative =
            strip_physical_prefix(std::path::Path::new(r"c:\root\nintendo\GAME.zip"), std::path::Path::new(r"C:\Root"))
                .unwrap();
        assert_eq!(relative, std::path::PathBuf::from(r"nintendo\GAME.zip"));
    }
