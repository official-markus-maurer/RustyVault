use eframe::egui;
use std::cell::RefCell;
use std::collections::HashSet;
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::mpsc::{Receiver, Sender, TryRecvError, channel};
use std::thread;
use compress::structured_archive::ZipStructure;
use rv_core::read_dat::DatUpdate;
use rv_core::scanner::Scanner;
use rv_core::find_fixes::FindFixes;
use rv_core::fix::Fix;
use rv_core::file_scanning::FileScanning;
use dat_reader::enums::FileType;
use rv_core::db::GLOBAL_DB;
use rv_core::rv_file::{RvFile, TreeSelect};
use sevenz_rust::encoder_options::ZstandardOptions;
use sevenz_rust::{ArchiveEntry, ArchiveWriter, EncoderConfiguration, EncoderMethod, Password, SourceReader};
use trrntzip::{ProcessControl, StopMode, TorrentZip, TorrentZipRebuild, TrrntZipStatus};
use zip::read::ZipArchive;
use zip::write::FileOptions;
mod status_bar;
mod startup_ui;
mod top_menu;
mod log_panel;
mod left_panel;
mod right_panel;
mod central_panel;
#[cfg(test)]
fn trurip_meta_fields(game: &rv_core::rv_game::RvGame) -> Vec<(&'static str, String)> {
    use rv_core::rv_game::GameData;

    let mut out = Vec::new();
    let get = |k| game.get_data(k).unwrap_or_default();

    let publisher = get(GameData::Publisher);
    if !publisher.trim().is_empty() {
        out.push(("Publisher", publisher));
    }
    let developer = get(GameData::Developer);
    if !developer.trim().is_empty() {
        out.push(("Developer", developer));
    }
    let title_id = get(GameData::Id);
    if !title_id.trim().is_empty() {
        out.push(("Title Id", title_id));
    }
    let source = get(GameData::Source);
    if !source.trim().is_empty() {
        out.push(("Source", source));
    }
    let clone_of = get(GameData::CloneOf);
    if !clone_of.trim().is_empty() {
        out.push(("Clone of", clone_of));
    }
    let related_to = get(GameData::RelatedTo);
    if !related_to.trim().is_empty() {
        out.push(("Related to", related_to));
    }
    let year = get(GameData::Year);
    if !year.trim().is_empty() {
        out.push(("Year", year));
    }
    let players = get(GameData::Players);
    if !players.trim().is_empty() {
        out.push(("Players", players));
    }
    let genre = get(GameData::Genre);
    if !genre.trim().is_empty() {
        out.push(("Genre", genre));
    }
    let sub_genre = get(GameData::SubGenre);
    if !sub_genre.trim().is_empty() {
        out.push(("SubGenre", sub_genre));
    }
    let ratings = get(GameData::Ratings);
    if !ratings.trim().is_empty() {
        out.push(("Ratings", ratings));
    }
    let score = get(GameData::Score);
    if !score.trim().is_empty() {
        out.push(("Score", score));
    }

    out
}
use zip::{CompressionMethod, ZipWriter};

/// Main GUI entry point using `egui` and `eframe`.
/// 
/// `romvault_ui` completely replaces the C# WinForms implementation. It provides a modern, 
/// dark-mode native desktop application that leverages immediate-mode rendering.
/// 
/// Differences from C#:
/// - C# uses stateful `WinForms` components (TreeView, DataGridView, etc.) which bind directly 
///   to the database objects.
/// - The Rust `egui` implementation is "immediate mode", meaning the entire UI tree is rebuilt 
///   and drawn 60 times a second. It traverses the `RvFile` tree every frame to render the left pane,
///   relying heavily on the `cached_stats` inside `RepairStatus` to maintain high frame rates.
#[macro_use]
mod assets;
mod panels;
mod reports;
mod tree_presets;
mod utils;
mod toolbar;
mod dialogs;
mod tree;
mod grids;
#[cfg(test)]
#[path = "tests/tree_presets_tests.rs"]
mod tree_presets_tests;
use crate::utils::{get_full_node_path, extract_text_from_zip, extract_image_from_zip};

fn ui_missing_count(stats: &rv_core::repair_status::RepairStatus) -> i32 {
    stats.count_missing()
}

#[cfg(test)]
fn game_desc_value(game_name: &str, game: &rv_core::rv_game::RvGame) -> String {
    use rv_core::rv_game::GameData;

    let mut desc = game.get_data(GameData::Description).unwrap_or_default();
    if desc == "¤" {
        let fallback = std::path::Path::new(game_name)
            .file_stem()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        if !fallback.is_empty() {
            desc = fallback;
        }
    }
    desc
}

#[cfg(test)]
fn game_details_fields(game_name: &str, game: &rv_core::rv_game::RvGame) -> Vec<(&'static str, String)> {
    use rv_core::rv_game::GameData;

    let mut out = Vec::new();
    let get = |k| game.get_data(k).unwrap_or_default();

    let id = get(GameData::Id);
    let name_value = if id.trim().is_empty() {
        game_name.to_string()
    } else {
        format!("{game_name} (ID:{id})")
    };
    if !name_value.trim().is_empty() {
        out.push(("Name", name_value));
    }

    let desc = game_desc_value(game_name, game);
    if !desc.trim().is_empty() {
        out.push(("Description", desc));
    }

    let emu_arc = get(GameData::EmuArc);
    if emu_arc != "yes" {
        let manufacturer = get(GameData::Manufacturer);
        if !manufacturer.trim().is_empty() {
            out.push(("Manufacturer", manufacturer));
        }
        let clone_of = get(GameData::CloneOf);
        if !clone_of.trim().is_empty() {
            out.push(("CloneOf", clone_of));
        }
        let rom_of = get(GameData::RomOf);
        if !rom_of.trim().is_empty() {
            out.push(("RomOf", rom_of));
        }
        let year = get(GameData::Year);
        if !year.trim().is_empty() {
            out.push(("Year", year));
        }
        let category = get(GameData::Category);
        if !category.trim().is_empty() {
            out.push(("Category", category));
        }
    }

    out
}

fn normalize_full_name_key(name: &str) -> String {
    name.replace('\\', "/").to_ascii_lowercase()
}

fn full_name_chain_from_node(node_rc: &Rc<RefCell<RvFile>>) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = Some(Rc::clone(node_rc));
    while let Some(rc) = cur {
        out.push(normalize_full_name_key(&rc.borrow().get_full_name()));
        cur = rc.borrow().parent.as_ref().and_then(|p| p.upgrade());
    }
    out
}

fn find_node_by_full_name_key(
    root: &Rc<RefCell<RvFile>>,
    target_key: &str,
) -> Option<Rc<RefCell<RvFile>>> {
    let mut stack = vec![Rc::clone(root)];
    while let Some(node_rc) = stack.pop() {
        let node = node_rc.borrow();
        if normalize_full_name_key(&node.get_full_name()) == target_key {
            return Some(Rc::clone(&node_rc));
        }
        for child in node.children.iter() {
            stack.push(Rc::clone(child));
        }
    }
    None
}

fn ui_fixable_count(stats: &rv_core::repair_status::RepairStatus) -> i32 {
    stats.count_fixes_needed()
}

fn recompute_fix_plan(root: Rc<RefCell<RvFile>>) {
    FindFixes::scan_files(Rc::clone(&root));
    rv_core::repair_status::RepairStatus::report_status_reset(root);
}

fn is_actionable_fix_status(status: rv_core::enums::RepStatus) -> bool {
    matches!(
        status,
        rv_core::enums::RepStatus::CanBeFixed
            | rv_core::enums::RepStatus::CanBeFixedMIA
            | rv_core::enums::RepStatus::CorruptCanBeFixed
            | rv_core::enums::RepStatus::UnNeeded
            | rv_core::enums::RepStatus::MoveToSort
            | rv_core::enums::RepStatus::MoveToCorrupt
            | rv_core::enums::RepStatus::Rename
            | rv_core::enums::RepStatus::Delete
            | rv_core::enums::RepStatus::IncompleteRemove
    )
}

fn count_selected_actionable_fixes(node: Rc<RefCell<RvFile>>) -> i32 {
    let (is_selected, rep_status, is_dir, children) = {
        let n = node.borrow();
        (
            matches!(n.tree_checked, TreeSelect::Selected | TreeSelect::Locked),
            n.rep_status(),
            n.is_directory(),
            n.children.clone(),
        )
    };

    let mut count = if is_selected && is_actionable_fix_status(rep_status) { 1 } else { 0 };

    if is_dir {
        for child in children {
            count += count_selected_actionable_fixes(child);
        }
    }

    count
}

fn current_fixable_count(root: Rc<RefCell<RvFile>>) -> i32 {
    count_selected_actionable_fixes(root)
}

#[cfg(test)]
#[path = "tests/main_tests.rs"]
mod tests;

fn main() -> eframe::Result<()> {
    rv_core::settings::load_settings_from_file();
    let initial_settings = rv_core::settings::get_settings();
    let initial_darkness = initial_settings.darkness;

    if initial_settings.debug_logs_enabled {
        let file = rv_core::open_update_log_file();
        if let Some(file) = file {
            let _ = tracing_subscriber::fmt()
                .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
                .with_writer(move || file.try_clone().unwrap())
                .try_init();
        } else {
            let _ = tracing_subscriber::fmt()
                .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
                .try_init();
        }
    } else {
        let _ = tracing_subscriber::fmt()
            .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
            .try_init();
    }

    let startup_path = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|p| p.to_string_lossy().to_string()))
        .unwrap_or_else(|| {
            std::env::current_dir()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string()
        });
    let window_title = format!("RomVault (3.6.1) {}", startup_path);

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title(window_title.clone())
            .with_inner_size([1200.0, 800.0])
            .with_min_inner_size([800.0, 600.0]),
        ..Default::default()
    };

    eframe::run_native(
        &window_title,
        options,
        Box::new(move |cc| {
            egui_extras::install_image_loaders(&cc.egui_ctx);
            
            // Customize visual style for a more modern look
            let mut style = (*cc.egui_ctx.style()).clone();
            style.spacing.item_spacing = egui::vec2(8.0, 8.0);
            style.spacing.button_padding = egui::vec2(6.0, 4.0);
            style.visuals = if initial_darkness {
                egui::Visuals::dark()
            } else {
                egui::Visuals::light()
            };
            style.visuals.window_rounding = egui::Rounding::same(8.0);
            style.visuals.menu_rounding = egui::Rounding::same(4.0);
            if initial_darkness {
                style.visuals.panel_fill = egui::Color32::from_rgb(25, 25, 27);
                style.visuals.widgets.noninteractive.bg_fill = egui::Color32::from_rgb(35, 35, 38);
                style.visuals.widgets.inactive.bg_fill = egui::Color32::from_rgb(45, 45, 48);
                style.visuals.widgets.hovered.bg_fill = egui::Color32::from_rgb(55, 55, 60);
                style.visuals.widgets.active.bg_fill = egui::Color32::from_rgb(65, 65, 70);
            }
            
            // Tweak text styles for better readability
            use egui::{FontFamily, FontId, TextStyle};
            style.text_styles.insert(TextStyle::Heading, FontId::new(20.0, FontFamily::Proportional));
            style.text_styles.insert(TextStyle::Body, FontId::new(14.0, FontFamily::Proportional));
            style.text_styles.insert(TextStyle::Monospace, FontId::new(14.0, FontFamily::Monospace));
            style.text_styles.insert(TextStyle::Button, FontId::new(14.0, FontFamily::Proportional));
            
            cc.egui_ctx.set_style(style);
            
            Box::new(RomVaultApp::new())
        }),
    )
}

struct RomVaultApp {
    // Current selected node in the tree for the right pane views
    selected_node: Option<Rc<RefCell<RvFile>>>,
    selected_game: Option<Rc<RefCell<RvFile>>>,
    pending_tree_scroll_to_selected: bool,
    pub(crate) tree_rows_dirty: bool,
    pub(crate) tree_rows_cache: Vec<crate::tree::TreeRow>,
    pub(crate) tree_stats_queue: std::collections::VecDeque<Rc<RefCell<RvFile>>>,
    pub(crate) tree_stats_queued: std::collections::HashSet<usize>,
    db_cache_dirty: bool,
    db_cache_last_write: Option<std::time::Instant>,

    // Filter states
    show_complete: bool,
    show_partial: bool,
    show_empty: bool,
    show_fixes: bool,
    show_mia: bool,
    show_merged: bool,
    filter_text: String,
    show_filter_panel: bool,

    // Task Logging
    task_logs: Vec<String>,

    // Dialog state
    pub show_dir_settings: bool,
    pub dir_settings_tab: usize,
    pub dir_settings_compact: bool,
    pub show_dir_mappings: bool,
    working_dir_mappings: Vec<rv_core::settings::DirMapping>,
    selected_dir_mapping_idx: Option<usize>,
    selected_emulator_idx: Option<usize>,
    show_sam_dialog: bool,
    sam_source_items: Vec<String>,
    sam_selected_source_idx: Option<usize>,
    sam_pending_source_path: String,
    sam_output_directory: String,
    sam_use_origin_output: bool,
    sam_input_kind: crate::dialogs::SamInputKind,
    sam_output_kind: crate::dialogs::SamOutputKind,
    sam_recurse_subdirs: bool,
    sam_rebuild_existing: bool,
    sam_remove_source: bool,
    sam_verify_output: bool,
    sam_running: bool,
    sam_soft_stop_requested: bool,
    sam_hard_stop_requested: bool,
    sam_status_text: String,
    sam_current_item: Option<String>,
    sam_completed_items: usize,
    sam_total_items: usize,
    sam_stop_control: Option<ProcessControl>,
    sam_worker_rx: Option<Receiver<SamWorkerEvent>>,
    show_color_key: bool,
    pub show_settings: bool,
    pub global_settings_tab: usize,
    show_about: bool,
    show_rom_info: bool,
    selected_rom_for_info: Option<Rc<RefCell<RvFile>>>,
    rom_info_lines: Vec<String>,

    // Active Game Info Tab
    active_game_info_tab: usize, // 0 = Info, 1 = Artwork, 2 = Screens
    
    // Info/NFO caching
    loaded_info: Option<String>,
    loaded_info_type: String, // "NFO", "DIZ"

    // Artwork caching
    loaded_logo: Option<Vec<u8>>,
    loaded_artwork: Option<Vec<u8>>,
    loaded_title: Option<Vec<u8>>,
    loaded_screen: Option<Vec<u8>>,
    last_selected_game_path: String,

    // Editing Directory Settings
    active_dat_rule: rv_core::settings::DatRule,
    
    // Editing Global Settings
    global_settings: rv_core::settings::Settings,

    // Sorting
    sort_col: Option<String>,
    sort_desc: bool,
    rom_grid_cache: Option<crate::grids::RomGridCache>,

    // Startup splash
    startup_active: bool,
    startup_status: String,
    startup_phase: u8,
    startup_done_at: Option<std::time::Instant>,
}

#[derive(Clone)]
struct SamJobRequest {
    sources: Vec<String>,
    output_directory: String,
    use_origin_output: bool,
    input_kind: crate::dialogs::SamInputKind,
    output_kind: crate::dialogs::SamOutputKind,
    recurse_subdirs: bool,
    rebuild_existing: bool,
    remove_source: bool,
    verify_output: bool,
}

enum SamWorkerEvent {
    Started { total_items: usize },
    ItemStarted { item: String, index: usize, total: usize },
    Log(String),
    ItemFinished { item: String, status: String },
    Finished { status: String },
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum SamSourceKind {
    Directory,
    Zip,
    SevenZip,
}

struct SamInterruptReader<R> {
    inner: R,
    control: ProcessControl,
}

impl<R: Read> Read for SamInterruptReader<R> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        if self.control.is_hard_stop_requested() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::Interrupted,
                "USER_ABORTED_HARD",
            ));
        }
        self.inner.read(buf)
    }
}

impl RomVaultApp {
    const SAM_7Z_ZSTD_LEVEL: u32 = 19;

    fn new() -> Self {
        rv_core::settings::load_settings_from_file();
        let initial_settings = rv_core::settings::get_settings();

        Self {
            selected_node: None,
            selected_game: None,
            pending_tree_scroll_to_selected: false,
            tree_rows_dirty: true,
            tree_rows_cache: Vec::new(),
            tree_stats_queue: std::collections::VecDeque::new(),
            tree_stats_queued: std::collections::HashSet::new(),
            db_cache_dirty: false,
            db_cache_last_write: None,
            show_complete: initial_settings.chk_box_show_complete,
            show_partial: initial_settings.chk_box_show_partial,
            show_empty: initial_settings.chk_box_show_empty,
            show_fixes: initial_settings.chk_box_show_fixes,
            show_mia: initial_settings.chk_box_show_mia,
            show_merged: initial_settings.chk_box_show_merged,
            filter_text: String::new(),
            show_filter_panel: true,
            task_logs: Vec::new(),

            show_dir_settings: false,
            dir_settings_tab: 0,
            dir_settings_compact: false,
            show_dir_mappings: false,
            working_dir_mappings: Vec::new(),
            selected_dir_mapping_idx: None,
            selected_emulator_idx: None,
            show_sam_dialog: false,
            sam_source_items: Vec::new(),
            sam_selected_source_idx: None,
            sam_pending_source_path: String::new(),
            sam_output_directory: String::new(),
            sam_use_origin_output: false,
            sam_input_kind: crate::dialogs::SamInputKind::Directory,
            sam_output_kind: crate::dialogs::SamOutputKind::TorrentZip,
            sam_recurse_subdirs: true,
            sam_rebuild_existing: false,
            sam_remove_source: false,
            sam_verify_output: true,
            sam_running: false,
            sam_soft_stop_requested: false,
            sam_hard_stop_requested: false,
            sam_status_text: "Idle".to_string(),
            sam_current_item: None,
            sam_completed_items: 0,
            sam_total_items: 0,
            sam_stop_control: None,
            sam_worker_rx: None,
            show_color_key: false,
            show_settings: false,
            global_settings_tab: 0,
            show_about: false,
            show_rom_info: false,
            selected_rom_for_info: None,
            rom_info_lines: Vec::new(),
            active_game_info_tab: 0,
            loaded_info: None,
            loaded_info_type: String::new(),
            loaded_logo: None,
            loaded_artwork: None,
            loaded_title: None,
            loaded_screen: None,
            last_selected_game_path: String::new(),
            active_dat_rule: rv_core::settings::DatRule::default(),
            global_settings: initial_settings,
            sort_col: Some("ROM (File)".to_string()),
            sort_desc: false,
            rom_grid_cache: None,
            startup_active: true,
            startup_status: "Starting...".to_string(),
            startup_phase: 0,
            startup_done_at: None,
        }
    }

    pub(crate) fn persist_filter_settings(&mut self) {
        let mut settings = rv_core::settings::get_settings();
        settings.chk_box_show_complete = self.show_complete;
        settings.chk_box_show_partial = self.show_partial;
        settings.chk_box_show_empty = self.show_empty;
        settings.chk_box_show_fixes = self.show_fixes;
        settings.chk_box_show_mia = self.show_mia;
        settings.chk_box_show_merged = self.show_merged;

        rv_core::settings::update_settings(settings.clone());
        let _ = rv_core::settings::write_settings_to_file(&settings);
        self.global_settings = settings;
    }

    fn prompt_add_tosort(&mut self) {
        if self.sam_running {
            return;
        }

        let Some(folder) = rfd::FileDialog::new()
            .set_title("Select new ToSort Folder")
            .pick_folder() else {
            return;
        };

        let path = folder.to_string_lossy().to_string();
        self.task_logs.push(format!("Add ToSort folder requested: {}", path));
        rv_core::db::GLOBAL_DB.with(|db_ref| {
            if let Some(db) = db_ref.borrow().as_ref() {
                let ts = std::rc::Rc::new(std::cell::RefCell::new(
                    rv_core::rv_file::RvFile::new(dat_reader::enums::FileType::Dir),
                ));
                {
                    let mut t = ts.borrow_mut();
                    t.name = path;
                    t.set_dat_status(dat_reader::enums::DatStatus::InToSort);
                }
                db.dir_root.borrow_mut().child_add(ts);
                rv_core::repair_status::RepairStatus::report_status_reset(std::rc::Rc::clone(&db.dir_root));
            }
        });
        self.db_cache_dirty = true;
    }

    fn prompt_fix_report(&mut self) {
        if self.sam_running {
            return;
        }

        let Some(path) = rfd::FileDialog::new()
            .set_title("Generate Fix Report")
            .set_file_name("RVFixReport.txt")
            .add_filter("Rom Vault Fixing Report", &["txt"])
            .save_file() else {
            return;
        };

        let path_str = path.to_string_lossy().to_string();
        self.launch_task("Generate Reports (Fix)", move |tx| {
            let _ = tx.send(format!("Generating Fix Report to {path_str}..."));
            GLOBAL_DB.with(|db_ref| {
                if let Some(db) = db_ref.borrow().as_ref() {
                    if let Err(e) = crate::reports::write_fix_report(&path_str, Rc::clone(&db.dir_root)) {
                        let _ = tx.send(format!("Failed to write Fix Report: {e}"));
                    }
                }
            });
        });
    }

    fn prompt_full_report(&mut self) {
        if self.sam_running {
            return;
        }

        let Some(path) = rfd::FileDialog::new()
            .set_title("Generate Full Report")
            .set_file_name("RVFullReport.txt")
            .add_filter("Rom Vault Report", &["txt"])
            .save_file() else {
            return;
        };

        let path_str = path.to_string_lossy().to_string();
        self.launch_task("Generate Reports (Full)", move |tx| {
            let _ = tx.send(format!("Generating Full Report to {path_str}..."));
            GLOBAL_DB.with(|db_ref| {
                if let Some(db) = db_ref.borrow().as_ref() {
                    if let Err(e) = crate::reports::write_full_report(&path_str, Rc::clone(&db.dir_root)) {
                        let _ = tx.send(format!("Failed to write Full Report: {e}"));
                    }
                }
            });
        });
    }

    pub(crate) fn prompt_fixdat_report(&mut self, red_only: bool) {
        if self.sam_running {
            return;
        }

        let settings = rv_core::settings::get_settings();
        let mut dlg = rfd::FileDialog::new();
        dlg = if red_only {
            dlg.set_title("Select FixDAT output folder (Missing/MIA only)")
        } else {
            dlg.set_title("Select FixDAT output folder (Missing/MIA + Fixable)")
        };
        if let Some(default_dir) = settings.fix_dat_out_path.as_ref().filter(|p| !p.trim().is_empty()) {
            dlg = dlg.set_directory(default_dir);
        }

        let Some(folder) = dlg.pick_folder() else {
            return;
        };

        let out_dir = folder.to_string_lossy().to_string();
        let mut new_settings = settings.clone();
        if new_settings.fix_dat_out_path.as_deref() != Some(&out_dir) {
            new_settings.fix_dat_out_path = Some(out_dir.clone());
            rv_core::settings::update_settings(new_settings.clone());
            let _ = rv_core::settings::write_settings_to_file(&new_settings);
            self.global_settings = new_settings;
        }

        self.launch_task(
            if red_only {
                "Generate FixDATs (Missing)"
            } else {
                "Generate FixDATs (All)"
            },
            move |tx| {
                let _ = tx.send(format!("Generating FixDATs to {out_dir}..."));
                GLOBAL_DB.with(|db_ref| {
                    if let Some(db) = db_ref.borrow().as_ref() {
                        rv_core::fix_dat_report::FixDatReport::recursive_dat_tree(
                            &out_dir,
                            Rc::clone(&db.dir_root),
                            red_only,
                        );
                    }
                });
            },
        );
    }

    pub(crate) fn prompt_fixdat_report_for_node(&mut self, red_only: bool, base_dir: Rc<RefCell<RvFile>>) {
        if self.sam_running {
            return;
        }

        let settings = rv_core::settings::get_settings();
        let mut dlg = rfd::FileDialog::new();
        dlg = if red_only {
            dlg.set_title("Select FixDAT output folder (Missing/MIA only)")
        } else {
            dlg.set_title("Select FixDAT output folder (Missing/MIA + Fixable)")
        };
        if let Some(default_dir) = settings.fix_dat_out_path.as_ref().filter(|p| !p.trim().is_empty()) {
            dlg = dlg.set_directory(default_dir);
        }

        let Some(folder) = dlg.pick_folder() else {
            return;
        };

        let out_dir = folder.to_string_lossy().to_string();
        let mut new_settings = settings.clone();
        if new_settings.fix_dat_out_path.as_deref() != Some(&out_dir) {
            new_settings.fix_dat_out_path = Some(out_dir.clone());
            rv_core::settings::update_settings(new_settings.clone());
            let _ = rv_core::settings::write_settings_to_file(&new_settings);
            self.global_settings = new_settings;
        }

        self.launch_task(
            if red_only {
                "Generate FixDATs (Missing)"
            } else {
                "Generate FixDATs (All)"
            },
            move |tx| {
                let _ = tx.send(format!("Generating FixDATs to {out_dir}..."));
                rv_core::fix_dat_report::FixDatReport::recursive_dat_tree(&out_dir, base_dir, red_only);
            },
        );
    }

    pub(crate) fn prompt_make_dat(&mut self, node_rc: Rc<RefCell<RvFile>>) {
        if self.sam_running {
            return;
        }

        let default_name = {
            let n = node_rc.borrow();
            if n.name.is_empty() {
                "export.dat".to_string()
            } else if n.name.to_ascii_lowercase().ends_with(".dat") {
                n.name.clone()
            } else {
                format!("{}.dat", n.name)
            }
        };

        let Some(path) = rfd::FileDialog::new()
            .set_title("Save a Dat File")
            .set_file_name(&default_name)
            .add_filter("DAT file", &["dat"])
            .save_file() else {
            return;
        };

        let path_str = path.to_string_lossy().to_string();
        self.launch_task("Make DAT", move |tx| {
            let _ = tx.send(format!("Writing DAT to {path_str}..."));
            let converter = rv_core::external_dat_converter_to::ExternalDatConverterTo {
                filter_merged: true,
                ..rv_core::external_dat_converter_to::ExternalDatConverterTo::new()
            };
            let Some(dh) = converter.convert_to_external_dat(Rc::clone(&node_rc)) else {
                let _ = tx.send("Make DAT failed: not a directory node".to_string());
                return;
            };

            match std::fs::File::create(&path_str) {
                Ok(mut f) => {
                    if let Err(e) = dat_reader::xml_writer::DatXmlWriter::write_dat(&mut f, &dh) {
                        let _ = tx.send(format!("Make DAT failed: {e}"));
                    }
                }
                Err(e) => {
                    let _ = tx.send(format!("Make DAT failed: {e}"));
                }
            }
        });
    }

    pub(crate) fn update_dats(&mut self, check_all: bool) {
        if self.sam_running {
            return;
        }

        let dat_root = rv_core::settings::get_settings().dat_root;
        let dat_root_path = if dat_root.is_empty() { "DatRoot".to_string() } else { dat_root };

        self.launch_task(
            if check_all { "Update All DATs" } else { "Update DATs" },
            move |tx| {
                GLOBAL_DB.with(|db_ref| {
                    if let Some(db) = db_ref.borrow().as_ref() {
                        let _ = tx.send(format!("Scanning {}...", dat_root_path));
                        if check_all {
                            let _ = tx.send("Full DAT rescan...".to_string());
                            DatUpdate::check_all_dats(Rc::clone(&db.dir_root), &dat_root_path);
                        }
                        DatUpdate::update_dat(Rc::clone(&db.dir_root), &dat_root_path);
                        rv_core::repair_status::RepairStatus::report_status_reset(Rc::clone(&db.dir_root));
                        db.dir_root.borrow_mut().cached_stats = None;
                    }
                });
            },
        );
    }

    fn flush_db_cache_if_needed(&mut self) {
        if self.sam_running {
            return;
        }
        if !self.db_cache_dirty {
            return;
        }

        let now = std::time::Instant::now();
        let should_write = self
            .db_cache_last_write
            .map(|last| now.duration_since(last) >= std::time::Duration::from_millis(500))
            .unwrap_or(true);
        if !should_write {
            return;
        }

        GLOBAL_DB.with(|db_ref| {
            if let Some(db) = db_ref.borrow().as_ref() {
                db.write_cache();
            }
        });
        self.db_cache_dirty = false;
        self.db_cache_last_write = Some(now);
    }

    fn garbage_collect(&mut self, ctx: &egui::Context) {
        self.loaded_artwork = None;
        self.loaded_logo = None;
        self.loaded_title = None;
        self.loaded_screen = None;
        self.loaded_info = None;
        self.loaded_info_type.clear();

        self.show_rom_info = false;
        self.selected_rom_for_info = None;
        self.rom_info_lines.clear();

        ctx.memory_mut(|mem| *mem = Default::default());
    }

    pub fn open_dir_mappings(&mut self) {
        self.global_settings = rv_core::settings::get_settings();
        self.working_dir_mappings = self.global_settings.dir_mappings.items.clone();
        self.selected_dir_mapping_idx = None;
        self.show_dir_mappings = true;
    }

    fn update_artwork(&mut self) {
        if let Some(game) = &self.selected_game {
            let game_name = &game.borrow().name;
            // The path might include .zip extension, so strip it for matching inside the artwork zip
            let game_base_name = std::path::Path::new(game_name)
                .file_stem()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();

            let full_path = get_full_node_path(Rc::clone(game));
            let full_path = rv_core::settings::find_dir_mapping(&full_path).unwrap_or(full_path);
            // e.g. "RomVault\MAME\pacman.zip" -> "RomVault\MAME"
            let dir_path = std::path::Path::new(&full_path).parent().unwrap_or_else(|| std::path::Path::new("")).to_string_lossy().to_string();

            if self.last_selected_game_path != full_path {
                self.last_selected_game_path = full_path.clone();
                
                // Load Art
                let art_zip = format!("{}\\{}", dir_path, "artpreview.zip");
                self.loaded_artwork = extract_image_from_zip(&art_zip, &game_base_name);
                
                // Load Logo
                let logo_zip = format!("{}\\{}", dir_path, "marquees.zip");
                self.loaded_logo = extract_image_from_zip(&logo_zip, &game_base_name);
                
                // Load Title
                let title_zip = format!("{}\\{}", dir_path, "cabinets.zip"); // C# reference says cabinets.zip for title
                self.loaded_title = extract_image_from_zip(&title_zip, &game_base_name);
                
                // Load Screen
                let screen_zip = format!("{}\\{}", dir_path, "snap.zip");
                self.loaded_screen = extract_image_from_zip(&screen_zip, &game_base_name);

                // Load Info/NFO
                // C# Reference tries to load NFO/DIZ from the game zip itself or external info
                // We'll try reading from the game zip directly if it exists
                self.loaded_info = None;
                self.loaded_info_type = String::new();
                
                if let Some(nfo_text) = extract_text_from_zip(&full_path, ".nfo") {
                    self.loaded_info = Some(nfo_text);
                    self.loaded_info_type = "NFO".to_string();
                } else if let Some(diz_text) = extract_text_from_zip(&full_path, ".diz") {
                    self.loaded_info = Some(diz_text);
                    self.loaded_info_type = "DIZ".to_string();
                }
            }
        } else {
            self.loaded_artwork = None;
            self.loaded_logo = None;
            self.loaded_title = None;
            self.loaded_screen = None;
            self.loaded_info = None;
            self.loaded_info_type.clear();
            self.last_selected_game_path.clear();
        }
    }

    fn launch_task<F>(&mut self, task_name: &str, f: F)
    where
        F: FnOnce(Sender<String>) + 'static,
    {
        let (tx, rx) = channel();
        let selection_chain = self
            .selected_node
            .as_ref()
            .map(full_name_chain_from_node)
            .unwrap_or_default();
        
        // Due to Rc<RefCell> in rv_core, we cannot pass DB across threads.
        // For now, we execute synchronously on the main thread.
        self.task_logs.push(format!("Starting {}...", task_name));
        
        f(tx.clone());
        
        while let Ok(msg) = rx.try_recv() {
            self.task_logs.push(msg);
        }

        if !selection_chain.is_empty() {
            let mut found_any = false;
            GLOBAL_DB.with(|db_ref| {
                let binding = db_ref.borrow();
                let Some(db) = binding.as_ref() else { return };
                for key in &selection_chain {
                    if let Some(found) = find_node_by_full_name_key(&db.dir_root, key) {
                        found_any = true;
                        self.select_node(found);
                        break;
                    }
                }
            });
            if !found_any {
                self.selected_node = None;
            }
        }
        
        self.task_logs.push("Saving DB Cache...".to_string());
        rv_core::GLOBAL_DB.with(|db_ref| {
            if let Some(db) = db_ref.borrow().as_ref() {
                db.write_cache();
            }
        });
        
        self.task_logs.push("Task completed.".to_string());
    }

    fn sam_output_extension(output_kind: crate::dialogs::SamOutputKind) -> Option<&'static str> {
        match output_kind {
            crate::dialogs::SamOutputKind::TorrentZip
            | crate::dialogs::SamOutputKind::Zip
            | crate::dialogs::SamOutputKind::ZipZstd => Some("zip"),
            crate::dialogs::SamOutputKind::SevenZipLzma
            | crate::dialogs::SamOutputKind::SevenZipZstd => Some("7z"),
        }
    }

    pub(crate) fn sam_output_kind_supported(output_kind: crate::dialogs::SamOutputKind) -> bool {
        let _ = output_kind;
        true
    }

    pub(crate) fn sam_output_kind_support_message(output_kind: crate::dialogs::SamOutputKind) -> Option<&'static str> {
        let _ = output_kind;
        None
    }

    fn sam_7z_content_methods(
        output_kind: crate::dialogs::SamOutputKind,
    ) -> Option<Vec<EncoderConfiguration>> {
        match output_kind {
            crate::dialogs::SamOutputKind::SevenZipLzma => {
                Some(vec![EncoderConfiguration::new(EncoderMethod::LZMA)])
            }
            crate::dialogs::SamOutputKind::SevenZipZstd => {
                Some(vec![EncoderConfiguration::from(ZstandardOptions::from_level(
                    Self::SAM_7Z_ZSTD_LEVEL,
                ))])
            }
            _ => None,
        }
    }

    fn sam_collect_stage_entries(
        base_dir: &Path,
        current_dir: &Path,
        entries: &mut Vec<(PathBuf, bool)>,
    ) -> Result<(), String> {
        let mut children: Vec<_> = fs::read_dir(current_dir)
            .map_err(|err| err.to_string())?
            .flatten()
            .map(|entry| entry.path())
            .collect();
        children.sort_by(|a, b| {
            let sa = a.to_string_lossy();
            let sb = b.to_string_lossy();
            let la = sa.to_ascii_lowercase();
            let lb = sb.to_ascii_lowercase();
            la.cmp(&lb).then(sa.cmp(&sb))
        });

        if current_dir != base_dir {
            entries.push((
                current_dir
                    .strip_prefix(base_dir)
                    .map_err(|err| err.to_string())?
                    .to_path_buf(),
                true,
            ));
        }

        for child in children {
            if child.is_dir() {
                Self::sam_collect_stage_entries(base_dir, &child, entries)?;
            } else {
                entries.push((
                    child.strip_prefix(base_dir).map_err(|err| err.to_string())?.to_path_buf(),
                    false,
                ));
            }
        }

        Ok(())
    }

    fn sam_source_kind(path: &Path) -> Option<SamSourceKind> {
        if path.is_dir() {
            Some(SamSourceKind::Directory)
        } else if path
            .extension()
            .and_then(|ext| ext.to_str())
            .is_some_and(|ext| ext.eq_ignore_ascii_case("zip"))
        {
            Some(SamSourceKind::Zip)
        } else if path
            .extension()
            .and_then(|ext| ext.to_str())
            .is_some_and(|ext| ext.eq_ignore_ascii_case("7z"))
        {
            Some(SamSourceKind::SevenZip)
        } else {
            None
        }
    }

    fn sam_input_allows_source(input_kind: crate::dialogs::SamInputKind, source_kind: SamSourceKind) -> bool {
        match input_kind {
            crate::dialogs::SamInputKind::Directory => source_kind == SamSourceKind::Directory,
            crate::dialogs::SamInputKind::Zip => source_kind == SamSourceKind::Zip,
            crate::dialogs::SamInputKind::SevenZip => source_kind == SamSourceKind::SevenZip,
            crate::dialogs::SamInputKind::Mixed => true,
        }
    }

    fn collect_sam_work_items(
        source: &Path,
        recurse: bool,
        input_kind: crate::dialogs::SamInputKind,
        items: &mut Vec<PathBuf>,
        seen: &mut HashSet<PathBuf>,
    ) {
        if let Some(source_kind) = Self::sam_source_kind(source) {
            if Self::sam_input_allows_source(input_kind, source_kind) {
                let canonical = source.canonicalize().unwrap_or_else(|_| source.to_path_buf());
                if seen.insert(canonical) {
                    items.push(source.to_path_buf());
                }
            }
        }

        if !source.is_dir() || !recurse {
            return;
        }

        let Ok(entries) = fs::read_dir(source) else {
            return;
        };

        for entry in entries.flatten() {
            Self::collect_sam_work_items(&entry.path(), true, input_kind, items, seen);
        }
    }

    fn sam_output_path(
        output_root: &Path,
        source_path: &Path,
        output_kind: crate::dialogs::SamOutputKind,
    ) -> Option<PathBuf> {
        let extension = Self::sam_output_extension(output_kind)?;
        let stem = if source_path.is_dir() {
            source_path.file_name()?.to_string_lossy().to_string()
        } else {
            source_path.file_stem()?.to_string_lossy().to_string()
        };
        Some(output_root.join(format!("{}.{}", stem, extension)))
    }

    fn sam_output_root_for_source(
        source_path: &Path,
        output_directory: &str,
        use_origin_output: bool,
    ) -> Option<PathBuf> {
        if use_origin_output {
            source_path.parent().map(Path::to_path_buf)
        } else if output_directory.trim().is_empty() {
            None
        } else {
            Some(PathBuf::from(output_directory))
        }
    }

    pub(crate) fn sam_has_usable_output_target(&self) -> bool {
        self.sam_use_origin_output || !self.sam_output_directory.trim().is_empty()
    }

    fn sam_archive_temp_path(output_path: &Path) -> PathBuf {
        let file_name = output_path.file_name().unwrap_or_default().to_string_lossy();
        output_path
            .parent()
            .unwrap_or(Path::new(""))
            .join(format!("__{}.samtmp", file_name))
    }

    fn sam_stage_dir(output_path: &Path) -> PathBuf {
        let file_name = output_path.file_name().unwrap_or_default().to_string_lossy();
        output_path
            .parent()
            .unwrap_or(Path::new(""))
            .join(format!("__{}.samtmp.dir", file_name))
    }

    fn sam_normalize_archive_entry_name(relative_path: &Path) -> String {
        relative_path.to_string_lossy().replace('\\', "/")
    }

    fn sam_deterministic_7z_entry(relative_path: &Path, is_dir: bool) -> ArchiveEntry {
        let entry_name = Self::sam_normalize_archive_entry_name(relative_path);
        if is_dir {
            ArchiveEntry::new_directory(&entry_name)
        } else {
            ArchiveEntry::new_file(&entry_name)
        }
    }

    fn sam_hard_stop_requested(control: &ProcessControl) -> bool {
        control.is_hard_stop_requested()
    }

    fn sam_copy_stream<R: Read, W: Write>(
        reader: &mut R,
        writer: &mut W,
        control: &ProcessControl,
    ) -> Result<(), String> {
        let mut buffer = [0u8; 64 * 1024];
        loop {
            if Self::sam_hard_stop_requested(control) {
                return Err("USER_ABORTED_HARD".to_string());
            }
            let read = reader.read(&mut buffer).map_err(|err| err.to_string())?;
            if read == 0 {
                return Ok(());
            }
            writer.write_all(&buffer[..read]).map_err(|err| err.to_string())?;
        }
    }

    fn sam_collect_directory_entries(
        base_dir: &Path,
        current_dir: &Path,
        entries: &mut Vec<(PathBuf, bool)>,
    ) -> Result<(), String> {
        let mut children: Vec<_> = fs::read_dir(current_dir)
            .map_err(|err| err.to_string())?
            .flatten()
            .map(|entry| entry.path())
            .collect();
        children.sort_by(|a, b| {
            let sa = a.to_string_lossy();
            let sb = b.to_string_lossy();
            let la = sa.to_ascii_lowercase();
            let lb = sb.to_ascii_lowercase();
            la.cmp(&lb).then(sa.cmp(&sb))
        });

        if current_dir != base_dir {
            entries.push((
                current_dir
                    .strip_prefix(base_dir)
                    .map_err(|err| err.to_string())?
                    .to_path_buf(),
                true,
            ));
        }

        for child in children {
            if child.is_dir() {
                Self::sam_collect_directory_entries(base_dir, &child, entries)?;
            } else {
                entries.push((
                    child.strip_prefix(base_dir).map_err(|err| err.to_string())?.to_path_buf(),
                    false,
                ));
            }
        }

        Ok(())
    }

    fn sam_write_zip_from_directory(
        source_dir: &Path,
        output_path: &Path,
        compression: CompressionMethod,
        control: &ProcessControl,
    ) -> Result<(), String> {
        let file = File::create(output_path).map_err(|err| err.to_string())?;
        let mut writer = ZipWriter::new(file);
        let options = FileOptions::<()>::default().compression_method(compression);
        let mut entries = Vec::new();
        Self::sam_collect_directory_entries(source_dir, source_dir, &mut entries)?;

        for (relative_path, is_dir) in entries {
            if Self::sam_hard_stop_requested(control) {
                return Err("USER_ABORTED_HARD".to_string());
            }
            let name = relative_path.to_string_lossy().replace('\\', "/");
            if is_dir {
                writer.add_directory(format!("{}/", name.trim_end_matches('/')), options).map_err(|err| err.to_string())?;
            } else {
                writer.start_file(&name, options).map_err(|err| err.to_string())?;
                let mut file = File::open(source_dir.join(&relative_path)).map_err(|err| err.to_string())?;
                Self::sam_copy_stream(&mut file, &mut writer, control)?;
            }
        }

        writer.finish().map_err(|err| err.to_string())?;
        Ok(())
    }

    fn sam_extract_zip_to_directory(
        source_path: &Path,
        stage_dir: &Path,
        control: &ProcessControl,
    ) -> Result<(), String> {
        let file = File::open(source_path).map_err(|err| err.to_string())?;
        let mut archive = ZipArchive::new(file).map_err(|err| err.to_string())?;

        for idx in 0..archive.len() {
            if Self::sam_hard_stop_requested(control) {
                return Err("USER_ABORTED_HARD".to_string());
            }
            let mut entry = archive.by_index(idx).map_err(|err| err.to_string())?;
            let out_path = stage_dir.join(entry.mangled_name());
            if entry.is_dir() {
                fs::create_dir_all(&out_path).map_err(|err| err.to_string())?;
            } else {
                if let Some(parent) = out_path.parent() {
                    fs::create_dir_all(parent).map_err(|err| err.to_string())?;
                }
                let mut output = File::create(&out_path).map_err(|err| err.to_string())?;
                Self::sam_copy_stream(&mut entry, &mut output, control)?;
            }
        }

        Ok(())
    }

    fn sam_extract_7z_to_directory(
        source_path: &Path,
        stage_dir: &Path,
        control: &ProcessControl,
    ) -> Result<(), String> {
        sevenz_rust::decompress_file_with_extract_fn(
            source_path,
            stage_dir,
            |entry, reader, dest| {
                if control.is_hard_stop_requested() {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::Interrupted,
                        "USER_ABORTED_HARD",
                    )
                    .into());
                }
                let out_path = dest.to_path_buf();
                if entry.name().ends_with('/') {
                    fs::create_dir_all(&out_path)?;
                } else {
                    if let Some(parent) = out_path.parent() {
                        fs::create_dir_all(parent)?;
                    }
                    let mut output = File::create(&out_path)?;
                    let mut buffer = [0u8; 64 * 1024];
                    loop {
                        if control.is_hard_stop_requested() {
                            return Err(std::io::Error::new(
                                std::io::ErrorKind::Interrupted,
                                "USER_ABORTED_HARD",
                            )
                            .into());
                        }
                        let read = reader.read(&mut buffer)?;
                        if read == 0 {
                            break;
                        }
                        output.write_all(&buffer[..read])?;
                    }
                }
                Ok(true)
            },
        )
        .map_err(|err| {
            if control.is_hard_stop_requested() {
                "USER_ABORTED_HARD".to_string()
            } else {
                err.to_string()
            }
        })?;
        Ok(())
    }

    fn sam_prepare_source_directory(
        source_path: &Path,
        source_kind: SamSourceKind,
        stage_dir: &Path,
        control: &ProcessControl,
    ) -> Result<Option<PathBuf>, String> {
        match source_kind {
            SamSourceKind::Directory => Ok(None),
            SamSourceKind::Zip => {
                fs::create_dir_all(stage_dir).map_err(|err| err.to_string())?;
                Self::sam_extract_zip_to_directory(source_path, stage_dir, control)?;
                Ok(Some(stage_dir.to_path_buf()))
            }
            SamSourceKind::SevenZip => {
                fs::create_dir_all(stage_dir).map_err(|err| err.to_string())?;
                Self::sam_extract_7z_to_directory(source_path, stage_dir, control)?;
                Ok(Some(stage_dir.to_path_buf()))
            }
        }
    }

    fn sam_verify_zip_output(output_path: &Path) -> Result<(), String> {
        let file = File::open(output_path).map_err(|err| err.to_string())?;
        let archive = ZipArchive::new(file).map_err(|err| err.to_string())?;
        let _ = archive.len();
        Ok(())
    }

    fn sam_verify_7z_output(output_path: &Path) -> Result<(), String> {
        let mut file = File::open(output_path).map_err(|err| err.to_string())?;
        let password = Password::empty();
        sevenz_rust::Archive::read(&mut file, &password)
            .map_err(|err| err.to_string())?;
        Ok(())
    }

    fn sam_process_7z_item(
        source_path: &Path,
        source_kind: SamSourceKind,
        output_path: &Path,
        output_kind: crate::dialogs::SamOutputKind,
        verify_output: bool,
        control: &ProcessControl,
    ) -> Result<String, String> {
        let temp_archive = Self::sam_archive_temp_path(output_path);
        let stage_dir = Self::sam_stage_dir(output_path);
        let _ = fs::remove_file(&temp_archive);
        let _ = fs::remove_dir_all(&stage_dir);

        let result = (|| -> Result<String, String> {
            let prepared_dir = Self::sam_prepare_source_directory(source_path, source_kind, &stage_dir, control)?;
            let source_dir = prepared_dir.as_deref().unwrap_or(source_path);
            let mut entries = Vec::new();
            Self::sam_collect_stage_entries(source_dir, source_dir, &mut entries)?;

            let Some(content_methods) = Self::sam_7z_content_methods(output_kind) else {
                return Err("The selected 7z output type is not available.".to_string());
            };
            let mut writer: ArchiveWriter<File> =
                ArchiveWriter::create(&temp_archive).map_err(|err| err.to_string())?;
            writer.set_content_methods(content_methods);
            let mut solid_entries = Vec::new();
            let mut solid_readers: Vec<SourceReader<SamInterruptReader<File>>> = Vec::new();
            for (relative_path, is_dir) in entries {
                if control.is_hard_stop_requested() {
                    return Err("USER_ABORTED_HARD".to_string());
                }

                let disk_path = source_dir.join(&relative_path);
                let entry = Self::sam_deterministic_7z_entry(&relative_path, is_dir);
                if is_dir {
                    writer
                        .push_archive_entry::<&[u8]>(entry, None)
                        .map_err(|err: sevenz_rust::Error| err.to_string())?;
                } else {
                    let file = File::open(&disk_path).map_err(|err| err.to_string())?;
                    let reader = SourceReader::new(SamInterruptReader {
                        inner: file,
                        control: control.clone(),
                    });
                    solid_entries.push(entry);
                    solid_readers.push(reader);
                }
            }
            if !solid_entries.is_empty() {
                writer.push_archive_entries(solid_entries, solid_readers).map_err(|err: sevenz_rust::Error| {
                    if control.is_hard_stop_requested() {
                        "USER_ABORTED_HARD".to_string()
                    } else {
                        err.to_string()
                    }
                })?;
            }
            writer.finish().map_err(|err| err.to_string())?;

            if verify_output {
                Self::sam_verify_7z_output(&temp_archive)?;
            }

            if output_path.exists() {
                let _ = fs::remove_file(output_path);
            }
            fs::rename(&temp_archive, output_path).map_err(|err| err.to_string())?;
            Ok(match output_kind {
                crate::dialogs::SamOutputKind::SevenZipLzma => "SEVENZIP_LZMA_CREATED".to_string(),
                crate::dialogs::SamOutputKind::SevenZipZstd => "SEVENZIP_ZSTD_CREATED".to_string(),
                _ => unreachable!(),
            })
        })();

        if result.is_err() {
            let _ = fs::remove_file(&temp_archive);
        }
        let _ = fs::remove_dir_all(&stage_dir);
        result
    }

    fn sam_process_zip_family_item(
        source_path: &Path,
        source_kind: SamSourceKind,
        output_path: &Path,
        output_kind: crate::dialogs::SamOutputKind,
        verify_output: bool,
        control: &ProcessControl,
    ) -> Result<String, String> {
        let temp_archive = Self::sam_archive_temp_path(output_path);
        let stage_dir = Self::sam_stage_dir(output_path);
        let _ = fs::remove_file(&temp_archive);
        let _ = fs::remove_dir_all(&stage_dir);

        let result = (|| -> Result<String, String> {
            let prepared_dir = Self::sam_prepare_source_directory(source_path, source_kind, &stage_dir, control)?;
            let source_dir = prepared_dir.as_deref().unwrap_or(source_path);
            let compression = match output_kind {
                crate::dialogs::SamOutputKind::ZipZstd => CompressionMethod::Zstd,
                _ => CompressionMethod::Deflated,
            };

            Self::sam_write_zip_from_directory(source_dir, &temp_archive, compression, control)?;

            if output_kind == crate::dialogs::SamOutputKind::TorrentZip {
                let mut sam = TorrentZip::new();
                sam.force_rezip = true;
                sam.check_only = false;
                sam.out_zip_type = ZipStructure::ZipTrrnt;
                let status = sam.process_with_control(&temp_archive.to_string_lossy(), Some(control));
                if status == TrrntZipStatus::USER_ABORTED_HARD {
                    return Err("USER_ABORTED_HARD".to_string());
                }
                if status != TrrntZipStatus::VALID_TRRNTZIP {
                    return Err(format!("{:?}", status));
                }
                if verify_output {
                    let verify_status = sam.process(&temp_archive.to_string_lossy());
                    if verify_status != TrrntZipStatus::VALID_TRRNTZIP {
                        return Err(format!("SAM verification reported {:?} for {}", verify_status, temp_archive.to_string_lossy()));
                    }
                }
            } else if verify_output {
                Self::sam_verify_zip_output(&temp_archive)?;
            }

            if output_path.exists() {
                let _ = fs::remove_file(output_path);
            }
            fs::rename(&temp_archive, output_path).map_err(|err| err.to_string())?;
            Ok(match output_kind {
                crate::dialogs::SamOutputKind::TorrentZip => "VALID_TRRNTZIP".to_string(),
                crate::dialogs::SamOutputKind::Zip => "ZIP_CREATED".to_string(),
                crate::dialogs::SamOutputKind::ZipZstd => "ZIP_ZSTD_CREATED".to_string(),
                _ => unreachable!(),
            })
        })();

        if result.is_err() {
            let _ = fs::remove_file(&temp_archive);
        }
        if result.is_err() || output_path.exists() {
            let _ = fs::remove_dir_all(&stage_dir);
        }

        result
    }

    fn cleanup_samtmp_for_request(request: &SamJobRequest) -> usize {
        let mut visited = HashSet::new();
        let mut removed = 0;

        if !request.use_origin_output && !request.output_directory.trim().is_empty() {
            let output_dir = PathBuf::from(&request.output_directory);
            if visited.insert(output_dir.clone()) {
                removed += TorrentZipRebuild::cleanup_samtmp_files(&output_dir, true);
            }
        }

        for source in &request.sources {
            let path = PathBuf::from(source);
            let cleanup_root = if request.use_origin_output {
                path.parent().map(Path::to_path_buf).unwrap_or(path.clone())
            } else if path.is_dir() {
                path.clone()
            } else {
                path.parent().map(Path::to_path_buf).unwrap_or(path.clone())
            };
            if visited.insert(cleanup_root.clone()) {
                removed += TorrentZipRebuild::cleanup_samtmp_files(&cleanup_root, true);
            }
        }

        removed
    }

    fn run_sam_job(request: SamJobRequest, control: ProcessControl, tx: Sender<SamWorkerEvent>) {
        let mut work_items = Vec::new();
        let mut seen = HashSet::new();
        for source in &request.sources {
            Self::collect_sam_work_items(
                Path::new(source),
                request.recurse_subdirs,
                request.input_kind,
                &mut work_items,
                &mut seen,
            );
        }

        let _ = tx.send(SamWorkerEvent::Started { total_items: work_items.len() });

        if !Self::sam_output_kind_supported(request.output_kind) {
            let _ = tx.send(SamWorkerEvent::Finished {
                status: Self::sam_output_kind_support_message(request.output_kind)
                    .unwrap_or("The selected SAM output type is not available.")
                    .to_string(),
            });
            return;
        }

        for (idx, source_path) in work_items.iter().enumerate() {
            if control.stop_mode() != StopMode::Running {
                break;
            }

            let item_label = source_path.to_string_lossy().to_string();
            let _ = tx.send(SamWorkerEvent::ItemStarted {
                item: item_label.clone(),
                index: idx + 1,
                total: work_items.len(),
            });

            let Some(source_kind) = Self::sam_source_kind(source_path) else {
                let _ = tx.send(SamWorkerEvent::Log(format!("SAM skipped unsupported source {}", item_label)));
                continue;
            };

            let Some(output_root) = Self::sam_output_root_for_source(
                source_path,
                &request.output_directory,
                request.use_origin_output,
            ) else {
                let _ = tx.send(SamWorkerEvent::Log(format!(
                    "SAM skipped {} because no usable output location could be resolved.",
                    item_label
                )));
                continue;
            };
            let _ = fs::create_dir_all(&output_root);

            let Some(output_path) = Self::sam_output_path(&output_root, source_path, request.output_kind) else {
                let _ = tx.send(SamWorkerEvent::Log(format!(
                    "SAM skipped {} because the selected output type is not available.",
                    item_label
                )));
                continue;
            };

            if output_path.exists() && !request.rebuild_existing {
                let _ = tx.send(SamWorkerEvent::Log(format!(
                    "SAM skipped {} because {} already exists.",
                    item_label,
                    output_path.to_string_lossy()
                )));
                continue;
            }

            let result = match request.output_kind {
                crate::dialogs::SamOutputKind::TorrentZip
                | crate::dialogs::SamOutputKind::Zip
                | crate::dialogs::SamOutputKind::ZipZstd => Self::sam_process_zip_family_item(
                    source_path,
                    source_kind,
                    &output_path,
                    request.output_kind,
                    request.verify_output,
                    &control,
                ),
                crate::dialogs::SamOutputKind::SevenZipLzma
                | crate::dialogs::SamOutputKind::SevenZipZstd => Self::sam_process_7z_item(
                    source_path,
                    source_kind,
                    &output_path,
                    request.output_kind,
                    request.verify_output,
                    &control,
                ),
            };

            match result {
                Ok(status) => {
                    if request.remove_source {
                        if source_kind == SamSourceKind::Directory {
                            let _ = fs::remove_dir_all(source_path);
                        } else if source_path != &output_path {
                            let _ = fs::remove_file(source_path);
                        }
                    }
                    let _ = tx.send(SamWorkerEvent::ItemFinished { item: item_label, status });
                }
                Err(status) => {
                    let _ = tx.send(SamWorkerEvent::ItemFinished {
                        item: item_label.clone(),
                        status: status.clone(),
                    });
                    if status == "USER_ABORTED_HARD" {
                        let removed = Self::cleanup_samtmp_for_request(&request);
                        let _ = tx.send(SamWorkerEvent::Log(format!(
                            "SAM hard stop removed {} .samtmp file(s).",
                            removed
                        )));
                        break;
                    }
                    let _ = tx.send(SamWorkerEvent::Log(format!("SAM {}", status)));
                }
            }
        }

        let finish_status = match control.stop_mode() {
            StopMode::HardStop => {
                let removed = Self::cleanup_samtmp_for_request(&request);
                format!("SAM hard stopped. Removed {} .samtmp file(s).", removed)
            }
            StopMode::SoftStop => "SAM soft stopped after the current conversion.".to_string(),
            StopMode::Running => "SAM completed.".to_string(),
        };
        let _ = tx.send(SamWorkerEvent::Finished { status: finish_status });
    }

    fn start_sam_job(&mut self) {
        if self.sam_running {
            return;
        }
        if !self.sam_has_usable_output_target() {
            self.task_logs.push("SAM requires either an output directory or origin-location output mode.".to_string());
            return;
        }

        let request = SamJobRequest {
            sources: self.sam_source_items.clone(),
            output_directory: self.sam_output_directory.clone(),
            use_origin_output: self.sam_use_origin_output,
            input_kind: self.sam_input_kind,
            output_kind: self.sam_output_kind,
            recurse_subdirs: self.sam_recurse_subdirs,
            rebuild_existing: self.sam_rebuild_existing,
            remove_source: self.sam_remove_source,
            verify_output: self.sam_verify_output,
        };
        let control = ProcessControl::new();
        let worker_control = control.clone();
        let (tx, rx) = channel();

        self.sam_running = true;
        self.sam_soft_stop_requested = false;
        self.sam_hard_stop_requested = false;
        self.sam_status_text = "Running".to_string();
        self.sam_current_item = None;
        self.sam_completed_items = 0;
        self.sam_total_items = 0;
        self.sam_stop_control = Some(control);
        self.sam_worker_rx = Some(rx);
        self.task_logs.push(format!(
            "Starting SAM with {} queued source path(s).",
            request.sources.len()
        ));

        thread::spawn(move || Self::run_sam_job(request, worker_control, tx));
    }

    fn request_sam_soft_stop(&mut self) {
        if let Some(control) = self.sam_stop_control.as_ref() {
            control.request_soft_stop();
            self.sam_soft_stop_requested = true;
            self.sam_status_text = "Soft stop requested".to_string();
            self.task_logs.push("SAM soft stop requested.".to_string());
        }
    }

    fn request_sam_hard_stop(&mut self) {
        if let Some(control) = self.sam_stop_control.as_ref() {
            control.request_hard_stop();
            self.sam_hard_stop_requested = true;
            self.sam_status_text = "Hard stop requested".to_string();
            self.task_logs.push("SAM hard stop requested.".to_string());
        }
    }

    fn poll_sam_worker(&mut self) {
        let mut finished = false;

        if let Some(rx) = self.sam_worker_rx.as_ref() {
            loop {
                match rx.try_recv() {
                    Ok(SamWorkerEvent::Started { total_items }) => {
                        self.sam_total_items = total_items;
                        self.sam_status_text = format!("Running {} item(s)", total_items);
                    }
                    Ok(SamWorkerEvent::ItemStarted { item, index, total }) => {
                        self.sam_current_item = Some(item.clone());
                        self.sam_status_text = format!("Processing {}/{}", index, total);
                        self.task_logs.push(format!("SAM processing {}", item));
                    }
                    Ok(SamWorkerEvent::Log(message)) => {
                        self.task_logs.push(message);
                    }
                    Ok(SamWorkerEvent::ItemFinished { item, status }) => {
                        self.sam_completed_items += 1;
                        self.task_logs.push(format!("SAM finished {} with {}", item, status));
                    }
                    Ok(SamWorkerEvent::Finished { status }) => {
                        self.sam_status_text = status.clone();
                        self.task_logs.push(status);
                        finished = true;
                    }
                    Err(TryRecvError::Empty) => break,
                    Err(TryRecvError::Disconnected) => {
                        finished = true;
                        break;
                    }
                }
            }
        }

        if finished {
            self.sam_running = false;
            self.sam_current_item = None;
            self.sam_stop_control = None;
            self.sam_worker_rx = None;
            self.sam_soft_stop_requested = false;
            self.sam_hard_stop_requested = false;
        }
    }

    fn scan_selected_roots(tx: &Sender<String>, scan_level: rv_core::settings::EScanLevel) {
        GLOBAL_DB.with(|db_ref| {
            if let Some(db) = db_ref.borrow().as_ref() {
                let settings = rv_core::settings::get_settings();
                let mut cache_timer = if settings.cache_save_timer_enabled {
                    Some(std::time::Instant::now())
                } else {
                    None
                };

                let root_children = db.dir_root.borrow().children.clone();

                for child in root_children {
                    let (name, is_selected) = {
                        let node = child.borrow();
                        (node.name.clone(), Self::branch_has_selected_nodes(&node))
                    };

                    if !is_selected {
                        continue;
                    }

                    let _ = tx.send(format!("Scanning {}...", name));
                    let physical_path = rv_core::settings::find_dir_mapping(&name).unwrap_or(name.clone());
                    let rule = rv_core::settings::find_rule(&name);
                    let files = Scanner::scan_directory_with_level_and_ignore(
                        &physical_path,
                        scan_level,
                        &rule.ignore_files.items,
                    );
                    let mut root_scan = rv_core::scanned_file::ScannedFile::new(FileType::Dir);
                    root_scan.name = name.clone();
                    root_scan.children = files;
                    let _ = tx.send(format!("Integrating {} files into DB...", name));
                    FileScanning::scan_dir_with_level(Rc::clone(&child), &mut root_scan, scan_level);

                    if let Some(last) = cache_timer {
                        if last.elapsed().as_secs_f64() / 60.0
                            > settings.cache_save_time_period as f64
                        {
                            let _ = tx.send("Saving Cache".to_string());
                            db.write_cache();
                            let _ = tx.send("Saving Cache Complete".to_string());
                            cache_timer = Some(std::time::Instant::now());
                        } else {
                            cache_timer = Some(last);
                        }
                    }
                }

                rv_core::repair_status::RepairStatus::report_status_reset(Rc::clone(&db.dir_root));
            }
        });
    }

    fn branch_has_selected_nodes(node: &RvFile) -> bool {
        if matches!(node.tree_checked, TreeSelect::Selected | TreeSelect::Locked) {
            return true;
        }

        for child in &node.children {
            if Self::branch_has_selected_nodes(&child.borrow()) {
                return true;
            }
        }

        false
    }

    fn launch_scan_roms_task(
        &mut self,
        task_name: &'static str,
        status_message: &'static str,
        scan_level: rv_core::settings::EScanLevel,
    ) {
        self.launch_task(task_name, move |tx| {
            let _ = tx.send(status_message.to_string());
            Self::scan_selected_roots(&tx, scan_level);
        });
    }

    fn launch_fix_roms_task(&mut self) {
        self.launch_task("Fix ROMs", |tx| {
            let _ = tx.send("Rescanning to refresh fix plan...".to_string());
            Self::scan_selected_roots(&tx, rv_core::settings::EScanLevel::Level2);

            for pass in 1..=4 {
                let _ = tx.send(format!("Finding Fixes (pass {pass}/4)..."));
                let pending = GLOBAL_DB.with(|db_ref| {
                    if let Some(db) = db_ref.borrow().as_ref() {
                        recompute_fix_plan(Rc::clone(&db.dir_root));
                        current_fixable_count(Rc::clone(&db.dir_root))
                    } else {
                        0
                    }
                });

                if pending == 0 {
                    break;
                }

                let _ = tx.send(format!("Performing physical fixes (pass {pass}/4)..."));
                GLOBAL_DB.with(|db_ref| {
                    if let Some(db) = db_ref.borrow().as_ref() {
                        rv_core::task_reporter::set_task_reporter(tx.clone());
                        Fix::perform_fixes(Rc::clone(&db.dir_root));
                        rv_core::task_reporter::clear_task_reporter();
                    }
                });

                let _ = tx.send(format!("Rescanning to sync DB with disk (pass {pass}/4)..."));
                Self::scan_selected_roots(&tx, rv_core::settings::EScanLevel::Level2);
            }

            let _ = tx.send("Refreshing final repair state...".to_string());
            GLOBAL_DB.with(|db_ref| {
                if let Some(db) = db_ref.borrow().as_ref() {
                    recompute_fix_plan(Rc::clone(&db.dir_root));
                    rv_core::task_reporter::set_task_reporter(tx.clone());
                    rv_core::report_found_mia(Rc::clone(&db.dir_root));
                    rv_core::task_reporter::clear_task_reporter();
                }
            });
        });
    }

    fn launch_scan_find_fix_fix_task(&mut self) {
        self.launch_task("Scan / Find Fix / Fix", |tx| {
            let _ = tx.send("Full automated fix routine started...".to_string());

            let _ = tx.send("Scanning...".to_string());
            Self::scan_selected_roots(&tx, rv_core::settings::EScanLevel::Level2);

            for pass in 1..=4 {
                let _ = tx.send(format!("Finding Fixes (pass {pass}/4)..."));
                let pending = GLOBAL_DB.with(|db_ref| {
                    if let Some(db) = db_ref.borrow().as_ref() {
                        recompute_fix_plan(Rc::clone(&db.dir_root));
                        current_fixable_count(Rc::clone(&db.dir_root))
                    } else {
                        0
                    }
                });

                if pending == 0 {
                    break;
                }

                let _ = tx.send(format!("Fixing (pass {pass}/4)..."));
                GLOBAL_DB.with(|db_ref| {
                    if let Some(db) = db_ref.borrow().as_ref() {
                        rv_core::task_reporter::set_task_reporter(tx.clone());
                        Fix::perform_fixes(Rc::clone(&db.dir_root));
                        rv_core::task_reporter::clear_task_reporter();
                    }
                });

                let _ = tx.send(format!("Rescanning to sync DB with disk (pass {pass}/4)..."));
                Self::scan_selected_roots(&tx, rv_core::settings::EScanLevel::Level2);
            }

            let _ = tx.send("Refreshing final repair state...".to_string());
            GLOBAL_DB.with(|db_ref| {
                if let Some(db) = db_ref.borrow().as_ref() {
                    recompute_fix_plan(Rc::clone(&db.dir_root));
                    rv_core::task_reporter::set_task_reporter(tx.clone());
                    rv_core::report_found_mia(Rc::clone(&db.dir_root));
                    rv_core::task_reporter::clear_task_reporter();
                }
            });
        });
    }
    fn load_tree_preset(&mut self, preset_index: i32) {
        if self.sam_running {
            return;
        }

        let filename = format!("treeDefault{}.xml", preset_index);
        let Some(entries) = crate::tree_presets::read_preset_file(&filename) else {
            self.task_logs.push(format!("Preset {} not found", preset_index));
            return;
        };

        rv_core::GLOBAL_DB.with(|db_ref| {
            if let Some(db) = db_ref.borrow().as_ref() {
                crate::tree_presets::apply_tree_state(Rc::clone(&db.dir_root), &entries);
            }
        });
        self.db_cache_dirty = true;
        self.task_logs.push(format!("Loaded Tree Preset {}", preset_index));
    }

    fn save_tree_preset(&mut self, preset_index: i32) {
        if self.sam_running {
            return;
        }

        let mut entries = Vec::new();
        rv_core::GLOBAL_DB.with(|db_ref| {
            if let Some(db) = db_ref.borrow().as_ref() {
                entries = crate::tree_presets::collect_tree_state(Rc::clone(&db.dir_root));
            }
        });

        let filename = format!("treeDefault{}.xml", preset_index);
        let _ = crate::tree_presets::write_preset_file(&filename, &entries);
        self.task_logs.push(format!("Saved Tree Preset {}", preset_index));
    }
}

impl eframe::App for RomVaultApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if crate::startup_ui::draw_startup(self, ctx) {
            return;
        }
        self.poll_sam_worker();
        if self.sam_running {
            ctx.request_repaint_after(std::time::Duration::from_millis(100));
        }
        // Update artwork cache if selection changed
        self.update_artwork();
        crate::top_menu::draw_top_menu(self, ctx);

        dialogs::draw_dialogs(self, ctx);

        toolbar::draw_left_toolbar(self, ctx);

        let dark_mode = ctx.style().visuals.dark_mode;
        let status_bar_fill = if dark_mode {
            egui::Color32::from_rgb(20, 20, 22)
        } else {
            ctx.style().visuals.faint_bg_color
        };
        let log_panel_fill = if dark_mode {
            egui::Color32::from_rgb(25, 25, 27)
        } else {
            ctx.style().visuals.panel_fill
        };
        let info_frame_fill = if dark_mode {
            egui::Color32::from_rgb(30, 30, 33)
        } else {
            egui::Color32::from_rgb(248, 248, 250)
        };
        let info_frame_stroke = if dark_mode {
            egui::Stroke::new(1.0, egui::Color32::from_rgb(45, 45, 50))
        } else {
            egui::Stroke::new(1.0, egui::Color32::from_rgb(214, 214, 220))
        };

        crate::status_bar::draw_status_bar(self, ctx, status_bar_fill);

        crate::log_panel::draw_log_panel(self, ctx, log_panel_fill);

        crate::left_panel::draw_left_panel(
            self,
            ctx,
            dark_mode,
            info_frame_fill,
            info_frame_stroke,
        );

        crate::right_panel::draw_right_panel(self, ctx);

        crate::central_panel::draw_central_panel(self, ctx);

        self.flush_db_cache_if_needed();
    }
}

fn format_number(n: i32) -> String {
    let s = n.to_string();
    let mut result = String::new();
    let mut count = 0;
    for c in s.chars().rev() {
        if count == 3 {
            result.push('.');
            count = 0;
        }
        result.push(c);
        count += 1;
    }
    result.chars().rev().collect()
}
