use eframe::egui;
use std::cell::RefCell;
use std::rc::Rc;
use std::sync::mpsc::Receiver;

pub(crate) use rv_core::db::GLOBAL_DB;
use rv_core::find_fixes::FindFixes;
use rv_core::rv_file::{RvFile, TreeSelect};
use trrntzip::ProcessControl;

mod app_actions;
mod app_artwork;
mod app_sam;
mod app_update;
mod central_panel;
mod left_panel;
mod log_panel;
mod right_panel;
mod startup_ui;
mod status_bar;
mod top_menu;
#[cfg(test)]
pub(crate) use app_sam::SamSourceKind;

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

#[macro_use]
mod assets;
mod dialogs;
mod grids;
mod panels;
mod reports;
mod toolbar;
mod tree;
mod tree_presets;
mod utils;
pub(crate) use utils::format_number;

#[cfg(test)]
#[path = "tests/tree_presets_tests.rs"]
mod tree_presets_tests;

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
fn game_details_fields(
    game_name: &str,
    game: &rv_core::rv_game::RvGame,
) -> Vec<(&'static str, String)> {
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

    let mut count = if is_selected && is_actionable_fix_status(rep_status) {
        1
    } else {
        0
    };

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

            use egui::{FontFamily, FontId, TextStyle};
            style.text_styles.insert(
                TextStyle::Heading,
                FontId::new(20.0, FontFamily::Proportional),
            );
            style
                .text_styles
                .insert(TextStyle::Body, FontId::new(14.0, FontFamily::Proportional));
            style.text_styles.insert(
                TextStyle::Monospace,
                FontId::new(14.0, FontFamily::Monospace),
            );
            style.text_styles.insert(
                TextStyle::Button,
                FontId::new(14.0, FontFamily::Proportional),
            );

            cc.egui_ctx.set_style(style);

            Box::new(RomVaultApp::new())
        }),
    )
}

struct RomVaultApp {
    selected_node: Option<Rc<RefCell<RvFile>>>,
    selected_game: Option<Rc<RefCell<RvFile>>>,
    pending_tree_scroll_to_selected: bool,
    pub(crate) tree_rows_dirty: bool,
    pub(crate) tree_rows_cache: Vec<crate::tree::TreeRow>,
    pub(crate) tree_stats_queue: std::collections::VecDeque<Rc<RefCell<RvFile>>>,
    pub(crate) tree_stats_queued: std::collections::HashSet<usize>,
    db_cache_dirty: bool,
    db_cache_last_write: Option<std::time::Instant>,
    show_complete: bool,
    show_partial: bool,
    show_empty: bool,
    show_fixes: bool,
    show_mia: bool,
    show_merged: bool,
    filter_text: String,
    show_filter_panel: bool,
    task_logs: Vec<String>,
    task_running: bool,
    task_name: String,
    task_worker_rx: Option<Receiver<String>>,
    task_worker_handle: Option<std::thread::JoinHandle<()>>,
    task_selection_chain: Vec<String>,
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
    sam_worker_rx: Option<Receiver<crate::app_sam::SamWorkerEvent>>,
    show_color_key: bool,
    pub show_settings: bool,
    pub global_settings_tab: usize,
    show_about: bool,
    show_rom_info: bool,
    selected_rom_for_info: Option<Rc<RefCell<RvFile>>>,
    rom_info_lines: Vec<String>,
    active_game_info_tab: usize,
    loaded_info: Option<String>,
    loaded_info_type: String,
    loaded_logo: Option<Vec<u8>>,
    loaded_artwork: Option<Vec<u8>>,
    loaded_title: Option<Vec<u8>>,
    loaded_screen: Option<Vec<u8>>,
    last_selected_game_path: String,
    active_dat_rule: rv_core::settings::DatRule,
    global_settings: rv_core::settings::Settings,
    sort_col: Option<String>,
    sort_desc: bool,
    rom_grid_cache: Option<crate::grids::RomGridCache>,
    startup_active: bool,
    startup_status: String,
    startup_phase: u8,
    startup_done_at: Option<std::time::Instant>,
}

impl RomVaultApp {
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
            task_running: false,
            task_name: String::new(),
            task_worker_rx: None,
            task_worker_handle: None,
            task_selection_chain: Vec::new(),
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
}
