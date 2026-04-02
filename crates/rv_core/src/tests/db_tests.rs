    use super::*;
    use crate::settings::{get_settings, set_dir_mapping, update_settings, DirMapping, Settings};
    use tempfile::tempdir;

    fn with_db_test_state(test: impl FnOnce(&std::path::Path)) {
        let original_settings = get_settings();
        let temp = tempdir().unwrap();

        let mut settings = Settings::default();
        settings.dat_root = temp.path().join("DatRoot").to_string_lossy().into_owned();
        update_settings(settings);
        test(temp.path());
        update_settings(original_settings);
    }

    #[test]
    fn test_check_create_root_dirs_uses_mapped_rustyvault_path() {
        with_db_test_state(|temp_path| {
            set_dir_mapping(DirMapping {
                dir_key: "RustyVault".to_string(),
                dir_path: temp_path.join("RomRoot").to_string_lossy().into_owned(),
            });

            DB::check_create_root_dirs();

            assert!(temp_path.join("RomRoot").exists());
            assert!(temp_path.join("DatRoot").exists());
        });
    }

    #[test]
    fn test_check_create_root_dirs_uses_custom_tosort_mapping() {
        with_db_test_state(|temp_path| {
            set_dir_mapping(DirMapping {
                dir_key: "ToSort".to_string(),
                dir_path: temp_path.join("SortedOutput").to_string_lossy().into_owned(),
            });

            DB::check_create_root_dirs();

            assert!(temp_path.join("SortedOutput").exists());
        });
    }
