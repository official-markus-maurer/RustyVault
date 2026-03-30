#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EScanLevel {
    Level1,
    Level2,
    Level3,
}

use std::fs::File;
use std::path::Path;

#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub enum EFixLevel {
    Level1,
    Level2,
    Level3,
}

#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub enum MergeType {
    None,
    Split,
    Merge,
    NonMerged,
    CHDsMerge,
}

#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub enum FilterType {
    KeepAll,
    RomsOnly,
    CHDsOnly,
}

#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub enum HeaderType {
    Optional,
    Headered,
    Headerless,
}

#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub enum RemoveSubType {
    KeepAllSubDirs,
    RemoveAllSubDirs,
    RemoveSubIfNameMatches,
    RemoveSubIfSingleGame,
    RemoveSubIfSingleOrMatches,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DirMapping {
    pub dir_key: String,
    pub dir_path: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DatRule {
    pub dir_key: String,
    pub dir_path: Option<String>,

    pub compression: dat_reader::enums::FileType,
    pub compression_override_dat: bool,

    pub merge: MergeType,
    pub filter: FilterType,
    pub header_type: HeaderType,
    pub merge_override_dat: bool,

    pub single_archive: bool,
    pub sub_dir_type: RemoveSubType,
    pub multi_dat_dir_override: bool,
    pub use_description_as_dir_name: bool,
    pub use_id_for_name: bool,

    pub complete_only: bool,
    pub ignore_files: Vec<String>,
    pub add_category_sub_dirs: bool,
    pub category_order: Vec<String>,
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
            ignore_files: Vec::new(),
            add_category_sub_dirs: false,
            category_order: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Settings {
    pub files_only: bool,
    pub dat_root: String,
    pub cache_file: String,
    pub fix_level: EFixLevel,
    
    pub dat_rules: Vec<DatRule>,
    pub dir_mappings: Vec<DirMapping>,
    pub ignore_files: Vec<String>,
    
    pub double_check_delete: bool,
    pub debug_logs_enabled: bool,
    pub detailed_fix_reporting: bool,
    
    pub chk_box_show_complete: bool,
    pub chk_box_show_partial: bool,
    pub chk_box_show_empty: bool,
    pub chk_box_show_fixes: bool,
    pub chk_box_show_mia: bool,
    pub chk_box_show_merged: bool,
    
    pub fix_dat_out_path: Option<String>,
    
    pub check_chd_version: bool,
    pub cache_save_timer_enabled: bool,
    pub cache_save_time_period: i32,
    pub mia_callback: bool,
    pub mia_anon: bool,
    pub delete_old_cue_files: bool,
    pub zstd_comp_count: i32,
    pub seven_z_default_struct: i32,
    pub darkness: bool,
    pub do_not_report_feedback: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            files_only: false,
            dat_root: String::new(),
            cache_file: String::new(),
            fix_level: EFixLevel::Level1,
            dat_rules: Vec::new(),
            dir_mappings: vec![DirMapping { dir_key: "RustyVault".to_string(), dir_path: "RomRoot".to_string() }],
            ignore_files: Vec::new(),
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
    pub static GLOBAL_SETTINGS: std::cell::RefCell<Settings> = std::cell::RefCell::new(Settings::default());
}

pub fn get_settings() -> Settings {
    GLOBAL_SETTINGS.with(|s| s.borrow().clone())
}

pub fn update_settings(new_settings: Settings) {
    GLOBAL_SETTINGS.with(|s| {
        *s.borrow_mut() = new_settings.clone();
    });
    let _ = write_settings_to_file(&new_settings);
}

pub fn load_settings_from_file() {
    let path = Path::new("RustyVault3cfg.json");
    if path.exists() {
        if let Ok(file) = File::open(path) {
            let config = bincode::config::standard();
            if let Ok(settings) = bincode::serde::decode_from_std_read(&mut std::io::BufReader::new(file), config) {
                update_settings(settings);
            }
        }
    }
}

pub fn write_settings_to_file(settings: &Settings) -> Result<(), Box<dyn std::error::Error>> {
    let file = File::create("RustyVault3cfg.json")?;
    let config = bincode::config::standard();
    bincode::serde::encode_into_std_write(settings, &mut std::io::BufWriter::new(file), config)?;
    Ok(())
}

    pub fn find_rule(dir_key: &str) -> DatRule {
        GLOBAL_SETTINGS.with(|s| {
            let settings = s.borrow();
            // In a real port, this would walk up the directory tree looking for the closest rule
            // For now, return the exact match or default
            settings.dat_rules.iter()
                .find(|r| r.dir_key == dir_key)
                .cloned()
                .unwrap_or_else(|| {
                    let mut rule = DatRule::default();
                    rule.dir_key = dir_key.to_string();
                    rule
                })
        })
    }

    pub fn set_rule(rule: DatRule) {
        GLOBAL_SETTINGS.with(|s| {
            let mut settings = s.borrow_mut();
            if let Some(pos) = settings.dat_rules.iter().position(|r| r.dir_key == rule.dir_key) {
                settings.dat_rules[pos] = rule;
            } else {
                settings.dat_rules.push(rule);
            }
        });
    }
