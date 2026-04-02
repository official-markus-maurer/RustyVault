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
use rv_core::fix_dat_report::FixDatReport;
use rv_core::file_scanning::FileScanning;
use dat_reader::enums::FileType;
use rv_core::db::{init_db, GLOBAL_DB};
use rv_core::rv_file::{RvFile, TreeSelect};
use dat_reader::enums::DatStatus;
use sevenz_rust::{SevenZArchiveEntry, SevenZWriter};
use trrntzip::{ProcessControl, StopMode, TorrentZip, TorrentZipRebuild, TrrntZipStatus};
use zip::read::ZipArchive;
use zip::write::FileOptions;
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
mod utils;
mod toolbar;
mod dialogs;
mod tree;
mod grids;
use crate::utils::{get_full_node_path, extract_text_from_zip, extract_image_from_zip};

fn ui_missing_count(stats: &rv_core::repair_status::RepairStatus) -> i32 {
    stats.count_missing()
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
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1200.0, 800.0])
            .with_min_inner_size([800.0, 600.0]),
        ..Default::default()
    };

    eframe::run_native(
        "RustyRoms UI",
        options,
        Box::new(|cc| {
            egui_extras::install_image_loaders(&cc.egui_ctx);
            
            // Customize visual style for a more modern look
            let mut style = (*cc.egui_ctx.style()).clone();
            style.spacing.item_spacing = egui::vec2(8.0, 8.0);
            style.spacing.button_padding = egui::vec2(6.0, 4.0);
            style.visuals = egui::Visuals::dark();
            style.visuals.window_rounding = egui::Rounding::same(8.0);
            style.visuals.menu_rounding = egui::Rounding::same(4.0);
            style.visuals.panel_fill = egui::Color32::from_rgb(25, 25, 27);
            style.visuals.widgets.noninteractive.bg_fill = egui::Color32::from_rgb(35, 35, 38);
            style.visuals.widgets.inactive.bg_fill = egui::Color32::from_rgb(45, 45, 48);
            style.visuals.widgets.hovered.bg_fill = egui::Color32::from_rgb(55, 55, 60);
            style.visuals.widgets.active.bg_fill = egui::Color32::from_rgb(65, 65, 70);
            
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
    pub show_dir_mappings: bool,
    working_dir_mappings: Vec<rv_core::settings::DirMapping>,
    selected_dir_mapping_idx: Option<usize>,
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
    fn new() -> Self {
        rv_core::settings::load_settings_from_file();

        Self {
            selected_node: None,
            selected_game: None,
            show_complete: true,
            show_partial: true,
            show_empty: true,
            show_fixes: true,
            show_mia: true,
            show_merged: true,
            filter_text: String::new(),
            show_filter_panel: true,
            task_logs: Vec::new(),

            show_dir_settings: false,
            dir_settings_tab: 0,
            show_dir_mappings: false,
            working_dir_mappings: Vec::new(),
            selected_dir_mapping_idx: None,
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
            active_game_info_tab: 0,
            loaded_info: None,
            loaded_info_type: String::new(),
            loaded_logo: None,
            loaded_artwork: None,
            loaded_title: None,
            loaded_screen: None,
            last_selected_game_path: String::new(),
            active_dat_rule: rv_core::settings::DatRule::default(),
            global_settings: rv_core::settings::get_settings(),
            sort_col: Some("ROM (File)".to_string()),
            sort_desc: false,
            startup_active: true,
            startup_status: "Starting...".to_string(),
            startup_phase: 0,
            startup_done_at: None,
        }
    }

    pub fn open_dir_mappings(&mut self) {
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
        
        // Due to Rc<RefCell> in rv_core, we cannot pass DB across threads.
        // For now, we execute synchronously on the main thread.
        self.task_logs.push(format!("Starting {}...", task_name));
        
        f(tx.clone());
        
        while let Ok(msg) = rx.try_recv() {
            self.task_logs.push(msg);
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
            crate::dialogs::SamOutputKind::SevenZipLzma => Some("7z"),
            crate::dialogs::SamOutputKind::SevenZipZstd => None,
        }
    }

    pub(crate) fn sam_output_kind_supported(output_kind: crate::dialogs::SamOutputKind) -> bool {
        !matches!(output_kind, crate::dialogs::SamOutputKind::SevenZipZstd)
    }

    pub(crate) fn sam_output_kind_support_message(output_kind: crate::dialogs::SamOutputKind) -> Option<&'static str> {
        match output_kind {
            crate::dialogs::SamOutputKind::SevenZipZstd => Some("7z Zstd is visible for reference parity, but the current archive backend cannot write stop-safe 7z Zstd output yet."),
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
        children.sort_by_key(|path| path.to_string_lossy().to_lowercase());

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
            if source_path.is_dir() {
                source_path.parent().map(Path::to_path_buf)
            } else {
                source_path.parent().map(Path::to_path_buf)
            }
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
        children.sort_by_key(|path| path.to_string_lossy().to_lowercase());

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
        let options = FileOptions::default().compression_method(compression);
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
                    return Err(sevenz_rust::Error::io(std::io::Error::new(
                        std::io::ErrorKind::Interrupted,
                        "USER_ABORTED_HARD",
                    )));
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
                            return Err(sevenz_rust::Error::io(std::io::Error::new(
                                std::io::ErrorKind::Interrupted,
                                "USER_ABORTED_HARD",
                            )));
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
        sevenz_rust::Archive::read(&mut file, 0, sevenz_rust::Password::empty().as_slice())
            .map_err(|err| err.to_string())?;
        Ok(())
    }

    fn sam_process_7z_item(
        source_path: &Path,
        source_kind: SamSourceKind,
        output_path: &Path,
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

            let mut writer = SevenZWriter::create(&temp_archive).map_err(|err| err.to_string())?;
            for (relative_path, is_dir) in entries {
                if control.is_hard_stop_requested() {
                    return Err("USER_ABORTED_HARD".to_string());
                }

                let disk_path = source_dir.join(&relative_path);
                let entry_name = relative_path.to_string_lossy().replace('\\', "/");
                let entry = SevenZArchiveEntry::from_path(&disk_path, entry_name);
                if is_dir {
                    writer.push_archive_entry::<&[u8]>(entry, None).map_err(|err| err.to_string())?;
                } else {
                    let file = File::open(&disk_path).map_err(|err| err.to_string())?;
                    let reader = SamInterruptReader {
                        inner: file,
                        control: control.clone(),
                    };
                    writer.push_archive_entry(entry, Some(reader)).map_err(|err| {
                        if control.is_hard_stop_requested() {
                            "USER_ABORTED_HARD".to_string()
                        } else {
                            err.to_string()
                        }
                    })?;
                }
            }
            writer.finish().map_err(|err| err.to_string())?;

            if verify_output {
                Self::sam_verify_7z_output(&temp_archive)?;
            }

            if output_path.exists() {
                let _ = fs::remove_file(output_path);
            }
            fs::rename(&temp_archive, output_path).map_err(|err| err.to_string())?;
            Ok("SEVENZIP_LZMA_CREATED".to_string())
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
                crate::dialogs::SamOutputKind::SevenZipLzma => Self::sam_process_7z_item(
                    source_path,
                    source_kind,
                    &output_path,
                    request.verify_output,
                    &control,
                ),
                crate::dialogs::SamOutputKind::SevenZipZstd => {
                    Err("7z output is not yet available through the stop-safe SAM backend.".to_string())
                }
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
                    let files = Scanner::scan_directory_with_level(&name, scan_level);
                    let mut root_scan = rv_core::scanned_file::ScannedFile::new(FileType::Dir);
                    root_scan.name = name.clone();
                    root_scan.children = files;
                    let _ = tx.send(format!("Integrating {} files into DB...", name));
                    FileScanning::scan_dir_with_level(Rc::clone(&child), &mut root_scan, scan_level);
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
                        Fix::perform_fixes(Rc::clone(&db.dir_root));
                    }
                });

                let _ = tx.send(format!("Rescanning to sync DB with disk (pass {pass}/4)..."));
                Self::scan_selected_roots(&tx, rv_core::settings::EScanLevel::Level2);
            }

            let _ = tx.send("Refreshing final repair state...".to_string());
            GLOBAL_DB.with(|db_ref| {
                if let Some(db) = db_ref.borrow().as_ref() {
                    recompute_fix_plan(Rc::clone(&db.dir_root));
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
                        Fix::perform_fixes(Rc::clone(&db.dir_root));
                    }
                });

                let _ = tx.send(format!("Rescanning to sync DB with disk (pass {pass}/4)..."));
                Self::scan_selected_roots(&tx, rv_core::settings::EScanLevel::Level2);
            }

            let _ = tx.send("Refreshing final repair state...".to_string());
            GLOBAL_DB.with(|db_ref| {
                if let Some(db) = db_ref.borrow().as_ref() {
                    recompute_fix_plan(Rc::clone(&db.dir_root));
                }
            });
        });
    }
    // Tree Preset Loading/Saving (simulating DatTreeStatusStore)
    fn load_tree_preset(&mut self, preset_index: i32) {
        let filename = format!("treeDefault{}.json", preset_index);
        if let Ok(data) = std::fs::read_to_string(&filename) {
            if let Ok(preset_data) = serde_json::from_str::<serde_json::Value>(&data) {
                if let Some(entries) = preset_data.as_array() {
                    rv_core::GLOBAL_DB.with(|db_ref| {
                        if let Some(db) = db_ref.borrow().as_ref() {
                            for entry in entries {
                                if let (Some(path), Some(selected), Some(expanded)) = (
                                    entry.get("Path").and_then(|v| v.as_str()),
                                    entry.get("Selected").and_then(|v| v.as_u64()),
                                    entry.get("Expanded").and_then(|v| v.as_bool())
                                ) {
                                    // Walk the tree to find the matching path
                                    let path_parts: Vec<&str> = path.split('\\').collect();
                                    let mut current = Rc::clone(&db.dir_root);
                                    let mut found = true;
                                    
                                    for part in path_parts {
                                        if part.is_empty() { continue; }
                                        let mut next = None;
                                        let n = current.borrow();
                                        for child in &n.children {
                                            if child.borrow().name == part {
                                                next = Some(Rc::clone(child));
                                                break;
                                            }
                                        }
                                        drop(n);
                                        
                                        if let Some(n) = next {
                                            current = n;
                                        } else {
                                            found = false;
                                            break;
                                        }
                                    }
                                    
                                    if found {
                                        let mut n = current.borrow_mut();
                                        n.tree_checked = match selected {
                                            0 => TreeSelect::UnSelected,
                                            1 => TreeSelect::Selected,
                                            2 => TreeSelect::Locked,
                                            _ => TreeSelect::Selected,
                                        };
                                        n.tree_expanded = expanded;
                                    }
                                }
                            }
                            db.write_cache();
                        }
                    });
                }
                self.task_logs.push(format!("Loaded Tree Preset {}", preset_index));
            }
        } else {
            self.task_logs.push(format!("Preset {} not found", preset_index));
        }
    }

    fn get_tree_state(node: Rc<RefCell<RvFile>>, path: String, entries: &mut Vec<serde_json::Value>) {
        let n = node.borrow();
        if !n.is_directory() { return; }
        
        let node_path = if path.is_empty() { n.name.clone() } else { format!("{}\\{}", path, n.name) };
        
        let sel_val = match n.tree_checked {
            TreeSelect::UnSelected => 0,
            TreeSelect::Selected => 1,
            TreeSelect::Locked => 2,
        };
        
        entries.push(serde_json::json!({
            "Path": node_path,
            "Selected": sel_val,
            "Expanded": n.tree_expanded
        }));
        
        let children = n.children.clone();
        drop(n);
        
        for child in children {
            Self::get_tree_state(child, node_path.clone(), entries);
        }
    }

    fn save_tree_preset(&mut self, preset_index: i32) {
        let mut entries = Vec::new();
        rv_core::GLOBAL_DB.with(|db_ref| {
            if let Some(db) = db_ref.borrow().as_ref() {
                Self::get_tree_state(Rc::clone(&db.dir_root), String::new(), &mut entries);
            }
        });
        
        let filename = format!("treeDefault{}.json", preset_index);
        let _ = std::fs::write(&filename, serde_json::to_string_pretty(&entries).unwrap());
        self.task_logs.push(format!("Saved Tree Preset {}", preset_index));
    }
}

impl eframe::App for RomVaultApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Handle startup splash progress
        if self.startup_active {
            let screen = ctx.screen_rect();
            egui::Window::new("Starting RustyVault")
                .collapsible(false)
                .resizable(false)
                .title_bar(true)
                .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
                .fixed_pos(egui::pos2(screen.center().x - 180.0, screen.center().y - 100.0))
                .show(ctx, |ui| {
                    ui.vertical_centered(|ui| {
                        ui.add(egui::Spinner::new().size(24.0));
                        ui.add_space(8.0);
                        ui.label(egui::RichText::new("Loading cache and preparing UI...").strong());
                        ui.add_space(6.0);
                        egui::ScrollArea::vertical().max_height(120.0).show(ui, |ui| {
                            ui.label(&self.startup_status);
                        });
                    });
                });

            if self.startup_phase == 0 {
                let has_cache = std::path::Path::new("RustyVault3_3.Cache").exists();
                self.startup_status = if has_cache {
                    "Loading cache: RustyVault3_3.Cache".to_string()
                } else {
                    "No cache found, creating default tree...".to_string()
                };
                self.startup_phase = 1;
                ctx.request_repaint();
                return;
            }

            if self.startup_phase == 1 {
                let start = std::time::Instant::now();
                init_db();
                let elapsed = start.elapsed();
                self.startup_status = format!("Database ready in {:?}", elapsed);
                self.startup_phase = 2;
                self.startup_done_at = Some(std::time::Instant::now());
                ctx.request_repaint();
                return;
            }

            if self.startup_phase == 2 {
                if let Some(done_at) = self.startup_done_at {
                    if done_at.elapsed().as_millis() >= 350 {
                        self.startup_active = false;
                    } else {
                        ctx.request_repaint();
                        return;
                    }
                } else {
                    self.startup_active = false;
                }
            }
        }
        self.poll_sam_worker();
        if self.sam_running {
            ctx.request_repaint_after(std::time::Duration::from_millis(100));
        }
        // Update artwork cache if selection changed
        self.update_artwork();
        
        // Global Keyboard Shortcuts
        if ctx.input(|i| i.key_pressed(egui::Key::F5) && i.modifiers.shift) {
            self.launch_task("Update All DATs", move |tx| {
                GLOBAL_DB.with(|db_ref| {
                    if let Some(db) = db_ref.borrow().as_ref() {
                        let _ = tx.send("Scanning DatRoot...".to_string());
                        DatUpdate::check_all_dats(Rc::clone(&db.dir_root), "DatRoot");
                        DatUpdate::update_dat(Rc::clone(&db.dir_root), "DatRoot");
                    }
                });
            });
        } else if ctx.input(|i| i.key_pressed(egui::Key::F5)) {
            self.launch_task("Update DATs", move |tx| {
                GLOBAL_DB.with(|db_ref| {
                    if let Some(db) = db_ref.borrow().as_ref() {
                        let _ = tx.send("Scanning DatRoot...".to_string());
                        DatUpdate::update_dat(Rc::clone(&db.dir_root), "DatRoot");
                    }
                });
            });
        }

        if ctx.input(|i| i.key_pressed(egui::Key::F6) && i.modifiers.shift) {
            self.launch_scan_roms_task(
                "Scan ROMs (Quick)",
                "Scanning selected ROM roots (Headers Only)...",
                rv_core::settings::EScanLevel::Level1,
            );
        } else if ctx.input(|i| i.key_pressed(egui::Key::F6) && i.modifiers.ctrl) {
            self.launch_scan_roms_task(
                "Scan ROMs (Full)",
                "Scanning selected ROM roots (Full Rescan)...",
                rv_core::settings::EScanLevel::Level3,
            );
        } else if ctx.input(|i| i.key_pressed(egui::Key::F6)) {
            self.launch_scan_roms_task(
                "Scan ROMs",
                "Scanning selected ROM roots...",
                rv_core::settings::EScanLevel::Level2,
            );
        }

        if ctx.input(|i| i.key_pressed(egui::Key::F7)) {
            self.launch_task("Find Fixes", move |tx| {
                let _ = tx.send("Running FindFixes...".to_string());
                GLOBAL_DB.with(|db_ref| {
                    if let Some(db) = db_ref.borrow().as_ref() {
                        FindFixes::scan_files(Rc::clone(&db.dir_root));
                    }
                });
            });
        }

        if ctx.input(|i| i.key_pressed(egui::Key::F8)) {
            self.launch_fix_roms_task();
        }

        if ctx.input(|i| i.key_pressed(egui::Key::F9) && i.modifiers.shift) {
            self.launch_task("Generate Reports (Full)", |tx| {
                let _ = tx.send("Generating Full DATs (All ROMs) to Desktop...".to_string());
                GLOBAL_DB.with(|db_ref| {
                    if let Some(db) = db_ref.borrow().as_ref() {
                        let desktop_path = std::path::PathBuf::from(std::env::var("USERPROFILE").unwrap_or_default()).join("Desktop");
                        FixDatReport::recursive_dat_tree(&desktop_path.to_string_lossy(), Rc::clone(&db.dir_root), false);
                    }
                });
            });
        } else if ctx.input(|i| i.key_pressed(egui::Key::F9) && i.modifiers.ctrl) {
            self.launch_task("Generate Reports (Fix)", |tx| {
                let _ = tx.send("Generating Fix Report...".to_string());
            });
        } else if ctx.input(|i| i.key_pressed(egui::Key::F9)) {
            self.launch_task("Generate Reports (Missing)", |tx| {
                let _ = tx.send("Generating FixDATs (Missing ROMs) to Desktop...".to_string());
                GLOBAL_DB.with(|db_ref| {
                    if let Some(db) = db_ref.borrow().as_ref() {
                        let desktop_path = std::path::PathBuf::from(std::env::var("USERPROFILE").unwrap_or_default()).join("Desktop");
                        FixDatReport::recursive_dat_tree(&desktop_path.to_string_lossy(), Rc::clone(&db.dir_root), true);
                    }
                });
            });
        }

        if ctx.input(|i| i.key_pressed(egui::Key::F10) && i.modifiers.shift) {
            self.active_dat_rule = rv_core::settings::find_rule("RustyVault");
            self.show_dir_settings = true;
        } else if ctx.input(|i| i.key_pressed(egui::Key::F10) && i.modifiers.ctrl) {
            self.open_dir_mappings();
        } else if ctx.input(|i| i.key_pressed(egui::Key::F10)) {
            self.global_settings = rv_core::settings::get_settings();
            self.show_settings = true;
        }

        if ctx.input(|i| i.key_pressed(egui::Key::F12)) {
            self.show_about = true;
        }

        egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
            egui::menu::bar(ui, |ui| {
                ui.menu_button("File", |ui| {
                    if ui.button("Add ToSort").clicked() {
                        if let Some(folder) = rfd::FileDialog::new()
                            .set_title("Select new ToSort Folder")
                            .pick_folder()
                        {
                            let path = folder.to_string_lossy().to_string();
                            self.task_logs.push(format!("Add ToSort folder requested: {}", path));
                            rv_core::db::GLOBAL_DB.with(|db_ref| {
                                if let Some(db) = db_ref.borrow().as_ref() {
                                    let ts = std::rc::Rc::new(std::cell::RefCell::new(
                                        rv_core::rv_file::RvFile::new(dat_reader::enums::FileType::Dir)
                                    ));
                                    {
                                        let mut t = ts.borrow_mut();
                                        t.name = path;
                                        t.set_dat_status(dat_reader::enums::DatStatus::InToSort);
                                    }
                                    db.dir_root.borrow_mut().child_add(ts);
                                    rv_core::repair_status::RepairStatus::report_status_reset(std::rc::Rc::clone(&db.dir_root));
                                    db.write_cache();
                                }
                            });
                        }
                        ui.close_menu();
                    }
                    if ui.button("Exit").clicked() {
                        std::process::exit(0);
                    }
                });
                ui.menu_button("Update DATs", |ui| {
                    if ui.button("Update New DATs").clicked() {
                        let is_shift = ui.input(|i| i.modifiers.shift);
                        self.launch_task("Update DATs", move |tx| {
                            GLOBAL_DB.with(|db_ref| {
                                if let Some(db) = db_ref.borrow().as_ref() {
                                    if is_shift {
                                        let _ = tx.send("Shift pressed: Full DAT Rescan...".to_string());
                                        DatUpdate::check_all_dats(Rc::clone(&db.dir_root), "DatRoot");
                                    }
                                    let _ = tx.send("Scanning DatRoot...".to_string());
                                    DatUpdate::update_dat(Rc::clone(&db.dir_root), "DatRoot");
                                    db.dir_root.borrow_mut().cached_stats = None;
                                }
                            });
                        });
                        ui.close_menu();
                    }
                    if ui.button("Refresh All DATs").clicked() {
                        self.launch_task("Update All DATs", move |tx| {
                            GLOBAL_DB.with(|db_ref| {
                                if let Some(db) = db_ref.borrow().as_ref() {
                                    let _ = tx.send("Scanning DatRoot...".to_string());
                                    DatUpdate::check_all_dats(Rc::clone(&db.dir_root), "DatRoot");
                                    DatUpdate::update_dat(Rc::clone(&db.dir_root), "DatRoot");
                                    db.dir_root.borrow_mut().cached_stats = None;
                                }
                            });
                        });
                        ui.close_menu();
                    }
                });
                ui.menu_button("Scan ROMs", |ui| {
                    if ui.button("Scan Quick (Headers Only)").clicked() {
                        self.launch_scan_roms_task(
                            "Scan ROMs (Quick)",
                            "Scanning selected ROM roots (Headers Only)...",
                            rv_core::settings::EScanLevel::Level1,
                        );
                        ui.close_menu();
                    }
                    if ui.button("Scan").clicked() {
                        self.launch_scan_roms_task(
                            "Scan ROMs",
                            "Scanning selected ROM roots...",
                            rv_core::settings::EScanLevel::Level2,
                        );
                        ui.close_menu();
                    }
                    if ui.button("Scan Full (Complete Re-Scan)").clicked() {
                        self.launch_scan_roms_task(
                            "Scan ROMs (Full)",
                            "Scanning selected ROM roots (Full Rescan)...",
                            rv_core::settings::EScanLevel::Level3,
                        );
                        ui.close_menu();
                    }
                });
                ui.menu_button("Find Fixes", |ui| {
                    if ui.button("Find Fixes").clicked() {
                        self.launch_task("Find Fixes", |tx| {
                            let _ = tx.send("Running FindFixes...".to_string());
                            GLOBAL_DB.with(|db_ref| {
                                if let Some(db) = db_ref.borrow().as_ref() {
                                    FindFixes::scan_files(Rc::clone(&db.dir_root));
                                    db.dir_root.borrow_mut().cached_stats = None;
                                }
                            });
                        });
                        ui.close_menu();
                    }
                });
                ui.menu_button("Fix ROMs", |ui| {
                    if ui.button("Fix ROMs").clicked() {
                        self.launch_fix_roms_task();
                        ui.close_menu();
                    }
                    if ui.button("Scan / Find Fix / Fix").clicked() {
                        self.launch_scan_find_fix_fix_task();
                        ui.close_menu();
                    }
                });
                ui.menu_button("Reports", |ui| {
                    if ui.button("Fix Dat Report").clicked() {
                        self.launch_task("Generate Reports (Missing)", |tx| {
                            let _ = tx.send("Generating FixDATs (Missing ROMs) to Desktop...".to_string());
                            GLOBAL_DB.with(|db_ref| {
                                if let Some(db) = db_ref.borrow().as_ref() {
                                    let desktop_path = std::path::PathBuf::from(std::env::var("USERPROFILE").unwrap_or_default()).join("Desktop");
                                    FixDatReport::recursive_dat_tree(&desktop_path.to_string_lossy(), Rc::clone(&db.dir_root), true);
                                }
                            });
                        });
                        ui.close_menu();
                    }
                    if ui.button("Full Report").clicked() {
                        self.launch_task("Generate Reports (Full)", |tx| {
                            let _ = tx.send("Generating Full DATs (All ROMs) to Desktop...".to_string());
                            GLOBAL_DB.with(|db_ref| {
                                if let Some(db) = db_ref.borrow().as_ref() {
                                    let desktop_path = std::path::PathBuf::from(std::env::var("USERPROFILE").unwrap_or_default()).join("Desktop");
                                    FixDatReport::recursive_dat_tree(&desktop_path.to_string_lossy(), Rc::clone(&db.dir_root), false);
                                }
                            });
                        });
                        ui.close_menu();
                    }
                });
                ui.menu_button("Settings", |ui| {
                    if ui.button("RustyVault Settings").clicked() {
                        self.global_settings = rv_core::settings::get_settings();
                        self.show_settings = true;
                        ui.close_menu();
                    }
                    if ui.button("Directory Settings").clicked() {
                        self.active_dat_rule = rv_core::settings::find_rule("RustyVault");
                        self.show_dir_settings = true;
                        ui.close_menu();
                    }
                    if ui.button("Directory Mappings").clicked() {
                        self.open_dir_mappings();
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui.button("Toggle Dark Mode").clicked() {
                        self.task_logs.push("Toggled Dark Mode (Requires restart to fully apply in C#, but handled dynamically here)".to_string());
                        let mut ctx_style = (*ui.ctx().style()).clone();
                        if ctx_style.visuals.dark_mode {
                            ctx_style.visuals = egui::Visuals::light();
                        } else {
                            ctx_style.visuals = egui::Visuals::dark();
                        }
                        ui.ctx().set_style(ctx_style);
                        ui.close_menu();
                    }
                });
                ui.menu_button("Add ToSort", |ui| {
                    if ui.button("Add ToSort").clicked() {
                        if let Some(folder) = rfd::FileDialog::new()
                            .set_title("Select new ToSort Folder")
                            .pick_folder()
                        {
                            let path = folder.to_string_lossy().to_string();
                            self.task_logs.push(format!("Add ToSort folder requested: {}", path));
                            GLOBAL_DB.with(|db_ref| {
                                if let Some(db) = db_ref.borrow().as_ref() {
                                    let ts = Rc::new(RefCell::new(
                                        RvFile::new(FileType::Dir)
                                    ));
                                    {
                                        let mut t = ts.borrow_mut();
                                        t.name = path;
                                        t.set_dat_status(DatStatus::InToSort);
                                    }
                                    db.dir_root.borrow_mut().child_add(ts);
                                    rv_core::repair_status::RepairStatus::report_status_reset(Rc::clone(&db.dir_root));
                                    db.write_cache();
                                }
                            });
                        }
                        ui.close_menu();
                    }
                });
                ui.menu_button("Help", |ui| {
                    if ui.button("Structured Archive Maker").clicked() {
                        self.show_sam_dialog = true;
                        ui.close_menu();
                    }
                    if ui.button("Color Key").clicked() {
                        self.show_color_key = true;
                        ui.close_menu();
                    }
                    if ui.button("Whats New").clicked() {
                        self.task_logs.push("Opening Whats New Wiki...".to_string());
                        let _ = std::process::Command::new("cmd")
                            .args(["/C", "start", "https://wiki.romvault.com/doku.php?id=whats_new"])
                            .spawn();
                        ui.close_menu();
                    }
                    if ui.button("Visit Help Wiki").clicked() {
                        self.task_logs.push("Opening Help Wiki...".to_string());
                        let _ = std::process::Command::new("cmd")
                            .args(["/C", "start", "https://wiki.romvault.com/doku.php?id=help"])
                            .spawn();
                        ui.close_menu();
                    }
                    if ui.button("Discord").clicked() {
                        self.task_logs.push("Opening Discord...".to_string());
                        let _ = std::process::Command::new("cmd")
                            .args(["/C", "start", "https://discord.gg/123456"]) // Placeholder discord link
                            .spawn();
                        ui.close_menu();
                    }
                    if ui.button("About RustyVault").clicked() {
                        self.show_about = true;
                        ui.close_menu();
                    }
                });
            });
        });

        dialogs::draw_dialogs(self, ctx);

        toolbar::draw_left_toolbar(self, ctx);

        egui::TopBottomPanel::bottom("status_bar")
            .resizable(false)
            .min_height(24.0)
            .frame(egui::Frame::none()
                .fill(egui::Color32::from_rgb(20, 20, 22))
                .inner_margin(egui::Margin::symmetric(8.0, 4.0))
            )
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label("RustyVault 3.6.1 (Rust Port)");
                    ui.separator();
                    
                    // Simple global stats
                    let mut total_roms = 0;
                    let mut total_missing = 0;
                    
                    GLOBAL_DB.with(|db_ref| {
                        if let Some(db) = db_ref.borrow().as_ref() {
                            if let Some(stats) = db.dir_root.borrow().cached_stats {
                                total_roms = stats.total_roms;
                                total_missing = ui_missing_count(&stats);
                            }
                        }
                    });
                    
                    ui.label(format!("Total ROMs: {}", format_number(total_roms)));
                    ui.separator();
                    ui.label(format!("Missing ROMs: {}", format_number(total_missing)));
                    
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        // We use task_logs as a proxy for the last action/status
                        if let Some(last_log) = self.task_logs.last() {
                            ui.label(last_log);
                        } else {
                            ui.label("Ready");
                        }
                    });
                });
            });

        // Bottom panel for Task Logging
        egui::TopBottomPanel::bottom("log_panel")
            .resizable(true)
            .min_height(100.0)
            .frame(egui::Frame::none().inner_margin(8.0).fill(egui::Color32::from_rgb(25, 25, 27)))
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.heading("Task Log");
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button("Clear").clicked() {
                            self.task_logs.clear();
                        }
                    });
                });
                ui.separator();
                egui::ScrollArea::vertical().stick_to_bottom(true).show(ui, |ui| {
                    for log in &self.task_logs {
                        ui.label(log);
                    }
                });
            });

        // Left panel for File Tree
        egui::SidePanel::left("left_panel")
            .resizable(true)
            .default_width(400.0)
            .frame(egui::Frame::none().inner_margin(8.0).fill(ctx.style().visuals.panel_fill))
            .show(ctx, |ui| {
                egui::Frame::none()
                    .fill(egui::Color32::from_rgb(30, 30, 33))
                    .rounding(6.0)
                    .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(45, 45, 50)))
                    .inner_margin(8.0)
                    .show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        ui.label(egui::RichText::new("Dat Info").strong().color(egui::Color32::LIGHT_GRAY));
                        ui.separator();
                    
                    egui::Grid::new("dat_info_grid")
                        .num_columns(4)
                        .spacing([10.0, 4.0])
                        .min_col_width(50.0)
                        .show(ui, |ui| {
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| { ui.label("Name:"); });
                            if let Some(node) = &self.selected_node {
                                if let Some(dat) = &node.borrow().dat {
                                    ui.label(dat.borrow().get_data(rv_core::rv_dat::DatData::DatName).unwrap_or_default());
                                } else {
                                    ui.label("");
                                }
                            } else {
                                ui.label("");
                            }
                            
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| { ui.label("Version:"); });
                            if let Some(node) = &self.selected_node {
                                if let Some(dat) = &node.borrow().dat {
                                    ui.label(dat.borrow().get_data(rv_core::rv_dat::DatData::Version).unwrap_or_default());
                                } else {
                                    ui.label("");
                                }
                            } else {
                                ui.label("");
                            }
                            ui.end_row();
                            
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| { ui.label("Description:"); });
                            if let Some(node) = &self.selected_node {
                                if let Some(dat) = &node.borrow().dat {
                                    ui.label(dat.borrow().get_data(rv_core::rv_dat::DatData::Description).unwrap_or_default());
                                } else {
                                    ui.label("");
                                }
                            } else {
                                ui.label("");
                            }
                            
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| { ui.label("Date:"); });
                            if let Some(node) = &self.selected_node {
                                if let Some(dat) = &node.borrow().dat {
                                    ui.label(dat.borrow().get_data(rv_core::rv_dat::DatData::Date).unwrap_or_default());
                                } else {
                                    ui.label("");
                                }
                            } else {
                                ui.label("");
                            }
                            ui.end_row();
                            
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| { ui.label("Category:"); });
                            if let Some(node) = &self.selected_node {
                                if let Some(dat) = &node.borrow().dat {
                                    ui.label(dat.borrow().get_data(rv_core::rv_dat::DatData::Category).unwrap_or_default());
                                } else {
                                    ui.label("");
                                }
                            } else {
                                ui.label("");
                            }
                            ui.label("");
                            ui.label("");
                            ui.end_row();
                            
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| { ui.label("Author:"); });
                            if let Some(node) = &self.selected_node {
                                if let Some(dat) = &node.borrow().dat {
                                    ui.label(dat.borrow().get_data(rv_core::rv_dat::DatData::Author).unwrap_or_default());
                                } else {
                                    ui.label("");
                                }
                            } else {
                                ui.label("");
                            }
                            ui.label("");
                            ui.label("");
                            ui.end_row();
                            
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| { ui.label("ROM Path:"); });
                            if let Some(node) = &self.selected_node {
                                if let Some(dat) = &node.borrow().dat {
                                    ui.label(dat.borrow().get_data(rv_core::rv_dat::DatData::RootDir).unwrap_or_default());
                                } else {
                                    ui.label("");
                                }
                            } else {
                                ui.label("");
                            }
                            ui.label("");
                            ui.label("");
                            ui.end_row();
                        });
                });
                
                ui.add_space(8.0);
                egui::Frame::none()
                    .fill(egui::Color32::from_rgb(30, 30, 33))
                    .rounding(6.0)
                    .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(45, 45, 50)))
                    .inner_margin(8.0)
                    .show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        ui.label(egui::RichText::new("Tree Status").strong().color(egui::Color32::LIGHT_GRAY));
                        ui.separator();
                    let mut got = 0;
                    let mut missing = 0;
                    let mut fixable = 0;
                    let mut unknown = 0;

                    if let Some(node_rc) = &self.selected_node {
                        let node = node_rc.borrow();
                        if let Some(stats) = &node.cached_stats {
                            got = stats.count_correct();
                            missing = ui_missing_count(stats);
                            fixable = ui_fixable_count(stats);
                            unknown = stats.roms_unknown;
                        } else {
                            // Only calculate once if not cached
                            drop(node);
                            let mut stats = rv_core::repair_status::RepairStatus::new();
                            stats.report_status(Rc::clone(node_rc));
                            let mut node_mut = node_rc.borrow_mut();
                            node_mut.cached_stats = Some(stats.clone());
                            
                            got = stats.count_correct();
                            missing = ui_missing_count(&stats);
                            fixable = ui_fixable_count(&stats);
                            unknown = stats.roms_unknown;
                        }
                    }
                    
                    egui::Grid::new("tree_status_grid").num_columns(4).show(ui, |ui| {
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| { ui.label("ROMs Got:"); });
                        ui.label(format_number(got));
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| { ui.label("ROMs Missing:"); });
                        ui.label(format_number(missing));
                        ui.end_row();
                        
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| { ui.label("ROMs Fixable:"); });
                        ui.label(format_number(fixable));
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| { ui.label("ROMs Unknown:"); });
                        ui.label(format_number(unknown));
                        ui.end_row();
                    });
                });
                
                ui.add_space(2.0);
                ui.add_space(5.0);

                // Directory Tree without group box wrapper to match C# visual style
                egui::ScrollArea::both().show(ui, |ui| {
                    GLOBAL_DB.with(|db_ref| {
                        if let Some(db) = db_ref.borrow().as_ref() {
                            let root = Rc::clone(&db.dir_root);
                            let children = root.borrow().children.clone();
                            for child in children {
                                self.draw_tree_node(ui, child, "".to_string());
                            }
                        }
                    });
                });
            });

        // Right panel (Info/Artwork/Screens) should only appear when there is actually something to show
        let has_info = self.loaded_info.is_some();
        let has_artwork = self.loaded_logo.is_some() || self.loaded_artwork.is_some();
        let has_screens = self.loaded_title.is_some() || self.loaded_screen.is_some();
        let show_right_panel = has_info || has_artwork || has_screens;

        if show_right_panel {
            let fallback_tab = if has_info { 0 } else if has_artwork { 1 } else { 2 };
            match self.active_game_info_tab {
                0 if !has_info => self.active_game_info_tab = fallback_tab,
                1 if !has_artwork => self.active_game_info_tab = fallback_tab,
                2 if !has_screens => self.active_game_info_tab = fallback_tab,
                _ => {}
            }

            egui::SidePanel::right("tab_emu_arc_panel")
                .resizable(true)
                .default_width(220.0)
                .frame(egui::Frame::none().inner_margin(8.0).fill(ctx.style().visuals.panel_fill))
                .show(ctx, |ui| {
                    ui.horizontal(|ui| {
                        if has_info {
                            ui.selectable_value(
                                &mut self.active_game_info_tab,
                                0,
                                if self.loaded_info_type.is_empty() { "Info" } else { &self.loaded_info_type },
                            );
                        }
                        if has_artwork {
                            ui.selectable_value(&mut self.active_game_info_tab, 1, "Artwork");
                        }
                        if has_screens {
                            ui.selectable_value(&mut self.active_game_info_tab, 2, "Screens");
                        }
                    });
                    ui.separator();

                    if self.active_game_info_tab == 0 && has_info {
                        egui::ScrollArea::both().show(ui, |ui| {
                            if let Some(info_text) = &self.loaded_info {
                                ui.label(
                                    egui::RichText::new(info_text)
                                        .font(egui::FontId::monospace(12.0)),
                                );
                            }
                        });
                    } else if self.active_game_info_tab == 1 && has_artwork {
                        egui::ScrollArea::vertical().show(ui, |ui| {
                            ui.vertical(|ui| {
                                if self.loaded_logo.is_some() {
                                    ui.label("Logo:");
                                    ui.group(|ui| {
                                        ui.set_min_height(100.0);
                                        ui.centered_and_justified(|ui| {
                                            if let Some(bytes) = &self.loaded_logo {
                                                ui.add(
                                                    egui::Image::from_bytes("bytes://logo", bytes.clone())
                                                        .max_width(ui.available_width()),
                                                );
                                            }
                                        });
                                    });
                                    ui.add_space(10.0);
                                }

                                if self.loaded_artwork.is_some() {
                                    ui.label("Artwork:");
                                    ui.group(|ui| {
                                        ui.set_min_height(200.0);
                                        ui.centered_and_justified(|ui| {
                                            if let Some(bytes) = &self.loaded_artwork {
                                                ui.add(
                                                    egui::Image::from_bytes("bytes://artwork", bytes.clone())
                                                        .max_width(ui.available_width()),
                                                );
                                            }
                                        });
                                    });
                                }
                            });
                        });
                    } else if self.active_game_info_tab == 2 && has_screens {
                        egui::ScrollArea::vertical().show(ui, |ui| {
                            ui.vertical(|ui| {
                                if self.loaded_title.is_some() {
                                    ui.label("Title Screen:");
                                    ui.group(|ui| {
                                        ui.set_min_height(150.0);
                                        ui.centered_and_justified(|ui| {
                                            if let Some(bytes) = &self.loaded_title {
                                                ui.add(
                                                    egui::Image::from_bytes("bytes://title", bytes.clone())
                                                        .max_width(ui.available_width()),
                                                );
                                            }
                                        });
                                    });
                                    ui.add_space(10.0);
                                }

                                if self.loaded_screen.is_some() {
                                    ui.label("Screenshot:");
                                    ui.group(|ui| {
                                        ui.set_min_height(150.0);
                                        ui.centered_and_justified(|ui| {
                                            if let Some(bytes) = &self.loaded_screen {
                                                ui.add(
                                                    egui::Image::from_bytes("bytes://screen", bytes.clone())
                                                        .max_width(ui.available_width()),
                                                );
                                            }
                                        });
                                    });
                                }
                            });
                        });
                    }
                });
        } else {
            self.active_game_info_tab = 0;
        }

        // Central panel for Game/ROM grids
        egui::CentralPanel::default()
            .frame(egui::Frame::none().fill(egui::Color32::from_rgb(20, 20, 22)).inner_margin(8.0))
            .show(ctx, |ui| {
                egui::TopBottomPanel::top("info_and_filters_panel")
                .resizable(false)
                .exact_height(180.0)
                .frame(egui::Frame::none())
                .show_inside(ui, |ui| {
                    panels::draw_info_and_filters(self, ui);
                });

            ui.add_space(8.0);

            egui::TopBottomPanel::top("game_grid_panel")
                .resizable(true)
                .min_height(200.0)
                .max_height(ui.available_height() * 0.6)
                .frame(egui::Frame::none())
                .show_inside(ui, |ui| {
                    self.draw_game_grid(ui);
                });

            ui.add_space(8.0);

            egui::CentralPanel::default()
                .frame(egui::Frame::none())
                .show_inside(ui, |ui| {
                    self.draw_rom_grid(ui);
                });
        });
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
