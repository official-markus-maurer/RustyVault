/// Scan thoroughness levels
#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub enum EScanLevel {
    /// Basic scan
    Level1,
    /// Standard scan
    Level2,
    /// Deep scan
    Level3,
}

use std::path::Path;

/// Fix thoroughness levels
#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub enum EFixLevel {
    /// Basic fix
    Level1,
    /// Standard fix
    Level2,
    /// Deep fix
    Level3,
}

/// Archive merge formatting
#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub enum MergeType {
    /// Unspecified
    None,
    /// Split format
    Split,
    /// Merge format
    Merge,
    /// Non-merged format
    NonMerged,
    /// CHD merge format
    CHDsMerge,
}

/// Content filtering
#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub enum FilterType {
    /// Keep everything
    KeepAll,
    /// Roms only
    RomsOnly,
    /// CHDs only
    CHDsOnly,
}

/// Header retention policies
#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub enum HeaderType {
    /// Headers optional
    Optional,
    /// Require headers
    Headered,
    /// Strip headers
    Headerless,
}

/// Subdirectory cleaning rules
#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub enum RemoveSubType {
    /// Keep all
    KeepAllSubDirs,
    /// Remove all
    RemoveAllSubDirs,
    /// Remove if matches name
    RemoveSubIfNameMatches,
    /// Remove if single game
    RemoveSubIfSingleGame,
    /// Remove if single game or matches name
    RemoveSubIfSingleOrMatches,
}

/// Represents a mapping between a generic directory key and an absolute file path.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct DirMapping {
    /// The abstract directory key (e.g. `DatRoot`)
    pub dir_key: String,
    /// The absolute physical path on disk
    pub dir_path: String,
}

/// Configuration rules bound to a specific DAT directory.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct DatRule {
    /// The directory key this rule applies to
    pub dir_key: String,
    /// Optional physical path override
    pub dir_path: Option<String>,

    /// Target compression type (e.g. Zip, 7z)
    pub compression: dat_reader::enums::FileType,
    /// Override the DAT's requested compression
    #[serde(rename = "CompressionOverrideDAT")]
    pub compression_override_dat: bool,

    /// Target merge format
    pub merge: MergeType,
    /// Target content filter
    pub filter: FilterType,
    /// Target header policy
    pub header_type: HeaderType,
    /// Override the DAT's requested merge format
    #[serde(rename = "MergeOverrideDAT")]
    pub merge_override_dat: bool,

    /// Force a single combined archive
    pub single_archive: bool,
    /// Policy for cleaning up subdirectories
    pub sub_dir_type: RemoveSubType,
    /// Override the DAT's requested directory grouping
    #[serde(rename = "MultiDATDirOverride")]
    pub multi_dat_dir_override: bool,
    /// Use the description field for the folder name
    pub use_description_as_dir_name: bool,
    /// Use the internal ID for the folder name
    pub use_id_for_name: bool,

    /// Only keep complete sets
    pub complete_only: bool,
    /// Files to explicitly ignore during scanning
    #[serde(rename = "IgnoreFiles")]
    pub ignore_files: IgnoreFilesWrapper,
    /// Add category subdirectories
    pub add_category_sub_dirs: bool,
    /// Ordering for categories
    #[serde(rename = "CategoryOrder")]
    pub category_order: CategoryOrderWrapper,
}

/// XML wrapper for category ordering strings
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default)]
pub struct CategoryOrderWrapper {
    /// Array of category names
    #[serde(rename = "string", default)]
    pub items: Vec<String>,
}

impl Default for DatRule {
    fn default() -> Self {
        Self {
            dir_key: String::new(),
            dir_path: None,
            compression: dat_reader::enums::FileType::Zip,
            compression_override_dat: false,
            merge: MergeType::Split,
            filter: FilterType::KeepAll,
            header_type: HeaderType::Optional,
            merge_override_dat: false,
            single_archive: false,
            sub_dir_type: RemoveSubType::KeepAllSubDirs,
            multi_dat_dir_override: false,
            use_description_as_dir_name: false,
            use_id_for_name: false,
            complete_only: false,
            ignore_files: IgnoreFilesWrapper { items: Vec::new() },
            add_category_sub_dirs: false,
            category_order: CategoryOrderWrapper { items: Vec::new() },
        }
    }
}

/// Specific launch options for a tied emulator.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct EmulatorInfo {
    /// Directory in the DB tree
    pub tree_dir: Option<String>,
    /// Emulator executable
    pub exe_name: Option<String>,
    /// Command line arguments
    pub command_line: Option<String>,
    /// Working directory for launch
    pub working_directory: Option<String>,
    /// Additional search paths
    pub extra_path: Option<String>,
}

/// Wrapper for list of emulator info.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default)]
pub struct EmulatorInfoWrapper {
    /// Array of items
    #[serde(rename = "EmulatorInfo", default)]
    pub items: Vec<EmulatorInfo>,
}

/// Global configuration state for the RomVault core.
/// 
/// `Settings` mirrors the exact XML schema of the original C# `RomVault3cfg.xml`.
/// It dictates archive parsing, DAT directory mapping, compression configurations (ZSTD/7Z),
/// and global ignore lists.
/// 
/// Differences from C#:
/// - The Rust version uses `quick-xml` combined with custom Serde wrappers (e.g., `DatRulesWrapper`)
///   to precisely replicate the nested XML array structure that C#'s `XmlSerializer` generates by default.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct Settings {
    /// Only scan files
    pub files_only: bool,
    /// Root path for DATs
    pub dat_root: String,
    /// Filename of the primary cache
    pub cache_file: String,
    /// Target fix logic depth
    pub fix_level: EFixLevel,
    
    /// Array of directory-specific configurations
    #[serde(rename = "DatRules")]
    pub dat_rules: DatRulesWrapper,
    /// Array of physical folder mappings
    #[serde(rename = "DirMappings")]
    pub dir_mappings: DirMappingsWrapper,
    /// Array of global ignored files
    #[serde(rename = "IgnoreFiles")]
    pub ignore_files: IgnoreFilesWrapper,
    /// Emulator setup mapping
    #[serde(rename = "EInfo")]
    pub e_info: EmulatorInfoWrapper,
    
    /// Prompt user before deleting items
    pub double_check_delete: bool,
    /// Write debug logs to disk
    pub debug_logs_enabled: bool,
    /// Show granular reports in Fix UI
    pub detailed_fix_reporting: bool,
    
    /// UI Filter flag
    #[serde(rename = "chkBoxShowComplete")]
    pub chk_box_show_complete: bool,
    /// UI Filter flag
    #[serde(rename = "chkBoxShowPartial")]
    pub chk_box_show_partial: bool,
    /// UI Filter flag
    #[serde(rename = "chkBoxShowEmpty")]
    pub chk_box_show_empty: bool,
    /// UI Filter flag
    #[serde(rename = "chkBoxShowFixes")]
    pub chk_box_show_fixes: bool,
    /// UI Filter flag
    #[serde(rename = "chkBoxShowMIA")]
    pub chk_box_show_mia: bool,
    /// UI Filter flag
    #[serde(rename = "chkBoxShowMerged")]
    pub chk_box_show_merged: bool,
    
    /// Default export directory for Fix DATs
    pub fix_dat_out_path: Option<String>,
    
    /// Verify CHD headers
    #[serde(rename = "CheckCHDVersion")]
    pub check_chd_version: bool,
    /// Enable periodic autosave
    pub cache_save_timer_enabled: bool,
    /// Time period for autosave
    pub cache_save_time_period: i32,
    /// Missing-In-Action Callback feature
    #[serde(rename = "MIACallback")]
    pub mia_callback: bool,
    /// Missing-In-Action Anonymous feature
    #[serde(rename = "MIAAnon")]
    pub mia_anon: bool,
    /// Clean old cue files
    pub delete_old_cue_files: bool,
    /// ZSTD parallel compression workers
    #[serde(rename = "zstdCompCount")]
    pub zstd_comp_count: i32,
    /// 7Z Solid block configuration
    #[serde(rename = "sevenZDefaultStruct")]
    pub seven_z_default_struct: i32,
    /// UI Dark Mode toggle
    pub darkness: bool,
    /// Skip automatic reporting
    pub do_not_report_feedback: bool,
}

/// XML wrapper array for DatRules
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default)]
pub struct DatRulesWrapper {
    /// Internal array list
    #[serde(rename = "DatRule", default)]
    pub items: Vec<DatRule>,
}

/// XML wrapper array for DirMappings
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default)]
pub struct DirMappingsWrapper {
    /// Internal array list
    #[serde(rename = "DirMapping", default)]
    pub items: Vec<DirMapping>,
}

/// XML wrapper array for ignored string paths
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, Default)]
pub struct IgnoreFilesWrapper {
    /// Internal string list
    #[serde(rename = "string", default)]
    pub items: Vec<String>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            files_only: false,
            dat_root: String::new(),
            cache_file: String::new(),
            fix_level: EFixLevel::Level1,
            dat_rules: DatRulesWrapper { items: Vec::new() },
            dir_mappings: DirMappingsWrapper { items: vec![DirMapping { dir_key: "RustyVault".to_string(), dir_path: "RomRoot".to_string() }] },
            ignore_files: IgnoreFilesWrapper { items: Vec::new() },
            e_info: EmulatorInfoWrapper { items: Vec::new() },
            double_check_delete: true,
            debug_logs_enabled: false,
            detailed_fix_reporting: true,
            chk_box_show_complete: true,
            chk_box_show_partial: true,
            chk_box_show_empty: true,
            chk_box_show_fixes: true,
            chk_box_show_mia: true,
            chk_box_show_merged: true,
            fix_dat_out_path: None,
            check_chd_version: false,
            cache_save_timer_enabled: true,
            cache_save_time_period: 10,
            mia_callback: true,
            mia_anon: false,
            delete_old_cue_files: false,
            zstd_comp_count: 0,
            seven_z_default_struct: 3,
            darkness: false,
            do_not_report_feedback: false,
        }
    }
}

// Global settings instance placeholder
thread_local! {
    /// The global thread-local instance of the settings state.
    pub static GLOBAL_SETTINGS: std::cell::RefCell<Settings> = std::cell::RefCell::new(Settings::default());
}

/// Retrieves a clone of the globally active Settings.
pub fn get_settings() -> Settings {
    GLOBAL_SETTINGS.with(|s| s.borrow().clone())
}

/// Overwrites the globally active Settings with a new instance.
pub fn update_settings(new_settings: Settings) {
    GLOBAL_SETTINGS.with(|s| {
        *s.borrow_mut() = new_settings;
    });
}

/// Loads `RomVault3cfg.xml` from disk into the global `Settings` thread-local singleton.
pub fn load_settings_from_file() {
    let settings_path = Path::new("RomVault3cfg.xml");
    if settings_path.exists() {
        if let Ok(xml_str) = std::fs::read_to_string(settings_path) {
            if let Ok(mut settings) = quick_xml::de::from_str::<Settings>(&xml_str) {
                // Ensure dat_root is absolute
                if !settings.dat_root.is_empty() {
                    let dat_root_path = Path::new(&settings.dat_root);
                    if dat_root_path.is_relative() {
                        if let Ok(current_dir) = std::env::current_dir() {
                            settings.dat_root = current_dir.join(dat_root_path).to_string_lossy().into_owned();
                        }
                    }
                }
                
                GLOBAL_SETTINGS.with(|s| {
                    *s.borrow_mut() = settings;
                });
                return;
            }
        }
    }
    
    // If file doesn't exist or parsing failed, save default settings to file
    let new_settings = Settings::default();
    GLOBAL_SETTINGS.with(|s| {
        *s.borrow_mut() = new_settings.clone();
    });
    let _ = write_settings_to_file(&new_settings);
}

/// Writes a `Settings` instance to disk as `RomVault3cfg.xml`.
pub fn write_settings_to_file(settings: &Settings) -> Result<(), Box<dyn std::error::Error>> {
    let xml_str = quick_xml::se::to_string(settings)?;
    
    // Quick-xml doesn't add the XML declaration by default
    let full_xml = format!("<?xml version=\"1.0\" encoding=\"utf-8\"?>\n{}", xml_str);
    
    let path = "RomVault3cfg.xml";
    let temp_path = "RomVault3cfg.xml.temp";
    let backup_path = "RomVault3cfg.xmlbackup";
    
    std::fs::write(temp_path, full_xml)?;
    
    if Path::new(path).exists() {
        if Path::new(backup_path).exists() {
            let _ = std::fs::remove_file(backup_path);
        }
        let _ = std::fs::rename(path, backup_path);
    }
    
    std::fs::rename(temp_path, path)?;
    
    Ok(())
}

fn normalize_dir_key(dir_key: &str) -> String {
    dir_key.replace('/', "\\").trim_matches('\\').to_string()
}

    /// Looks up a specific DatRule by its `dir_key`.
    pub fn find_rule(dir_key: &str) -> DatRule {
        GLOBAL_SETTINGS.with(|s| {
            let settings = s.borrow();
            let normalized_dir_key = normalize_dir_key(dir_key);
            let mut current = normalized_dir_key.as_str();

            loop {
                if let Some(rule) = settings
                    .dat_rules
                    .items
                    .iter()
                    .find(|r| normalize_dir_key(&r.dir_key) == current)
                {
                    return rule.clone();
                }

                if let Some((parent, _)) = current.rsplit_once('\\') {
                    current = parent;
                    continue;
                }

                let mut rule = DatRule::default();
                rule.dir_key = normalized_dir_key;
                return rule;
            }
        })
    }

    /// Updates or inserts a specific DatRule by its `dir_key`.
    pub fn set_rule(rule: DatRule) {
        GLOBAL_SETTINGS.with(|s| {
            let mut settings = s.borrow_mut();
            let normalized_dir_key = normalize_dir_key(&rule.dir_key);
            let mut normalized_rule = rule;
            normalized_rule.dir_key = normalized_dir_key.clone();

            if let Some(pos) = settings
                .dat_rules
                .items
                .iter()
                .position(|r| normalize_dir_key(&r.dir_key) == normalized_dir_key)
            {
                settings.dat_rules.items[pos] = normalized_rule;
            } else {
                settings.dat_rules.items.push(normalized_rule);
            }
        });
    }

#[cfg(test)]
mod tests {
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
}
