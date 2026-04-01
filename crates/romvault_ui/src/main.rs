use eframe::egui;
use std::rc::Rc;
use std::cell::RefCell;
use rv_core::read_dat::DatUpdate;
use rv_core::scanner::Scanner;
use rv_core::find_fixes::FindFixes;
use rv_core::fix::Fix;
use rv_core::fix_dat_report::FixDatReport;
use rv_core::file_scanning::FileScanning;
use std::sync::mpsc::{channel, Sender};
use dat_reader::enums::FileType;
use rv_core::db::{init_db, GLOBAL_DB};
use rv_core::rv_file::{RvFile, TreeSelect};
use dat_reader::enums::DatStatus;

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
    stats.roms_missing + stats.roms_missing_mia + stats.roms_fixes
}

fn ui_fixable_count(stats: &rv_core::repair_status::RepairStatus) -> i32 {
    stats.roms_fixes + stats.roms_unknown
}

#[cfg(test)]
mod tests {
    use super::*;
    use rv_core::rv_file::RvFile;
    use dat_reader::enums::FileType;
    use std::rc::Rc;
    use std::cell::RefCell;
    use crate::utils::get_full_node_path;

    #[test]
    fn test_get_full_node_path() {
        let root = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
        root.borrow_mut().name = "RustyVault".to_string();

        let sub_dir = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
        sub_dir.borrow_mut().name = "MAME".to_string();
        sub_dir.borrow_mut().parent = Some(Rc::downgrade(&root));
        
        let game = Rc::new(RefCell::new(RvFile::new(FileType::Zip)));
        game.borrow_mut().name = "pacman.zip".to_string();
        game.borrow_mut().parent = Some(Rc::downgrade(&sub_dir));

        sub_dir.borrow_mut().child_add(Rc::clone(&game));
        root.borrow_mut().child_add(Rc::clone(&sub_dir));

        let path = get_full_node_path(Rc::clone(&game));
        assert_eq!(path, "RustyVault\\MAME\\pacman.zip");
    }

    #[test]
    fn test_branch_has_selected_nodes_finds_selected_descendant() {
        let root = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
        root.borrow_mut().tree_checked = TreeSelect::UnSelected;

        let child_dir = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
        child_dir.borrow_mut().tree_checked = TreeSelect::UnSelected;

        let selected_file = Rc::new(RefCell::new(RvFile::new(FileType::File)));
        selected_file.borrow_mut().tree_checked = TreeSelect::Selected;

        child_dir.borrow_mut().child_add(Rc::clone(&selected_file));
        root.borrow_mut().child_add(Rc::clone(&child_dir));

        assert!(RomVaultApp::branch_has_selected_nodes(&root.borrow()));
    }

    #[test]
    fn test_ui_missing_count_excludes_unknown_and_not_collected() {
        let mut stats = rv_core::repair_status::RepairStatus::new();
        stats.roms_missing = 1;
        stats.roms_missing_mia = 1;
        stats.roms_fixes = 1;
        stats.roms_unknown = 2;
        stats.roms_not_collected = 3;

        assert_eq!(ui_missing_count(&stats), 3);
    }

    #[test]
    fn test_ui_fixable_count_excludes_not_collected_and_unneeded() {
        let mut stats = rv_core::repair_status::RepairStatus::new();
        stats.roms_fixes = 1;
        stats.roms_unknown = 2;
        stats.roms_not_collected = 3;
        stats.roms_unneeded = 4;

        assert_eq!(ui_fixable_count(&stats), 3);
    }
}

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
            let _ = tx.send("Performing physical fixes...".to_string());
            GLOBAL_DB.with(|db_ref| {
                if let Some(db) = db_ref.borrow().as_ref() {
                    Fix::perform_fixes(Rc::clone(&db.dir_root));
                }
            });

            let _ = tx.send("Rescanning to sync DB with disk...".to_string());
            Self::scan_selected_roots(&tx, rv_core::settings::EScanLevel::Level2);

            let _ = tx.send("Finding Fixes...".to_string());
            GLOBAL_DB.with(|db_ref| {
                if let Some(db) = db_ref.borrow().as_ref() {
                    FindFixes::scan_files(Rc::clone(&db.dir_root));
                    rv_core::repair_status::RepairStatus::report_status_reset(Rc::clone(&db.dir_root));
                }
            });
        });
    }

    fn launch_scan_find_fix_fix_task(&mut self) {
        self.launch_task("Scan / Find Fix / Fix", |tx| {
            let _ = tx.send("Full automated fix routine started...".to_string());

            let _ = tx.send("Scanning...".to_string());
            Self::scan_selected_roots(&tx, rv_core::settings::EScanLevel::Level2);

            let _ = tx.send("Finding Fixes...".to_string());
            GLOBAL_DB.with(|db_ref| {
                if let Some(db) = db_ref.borrow().as_ref() {
                    FindFixes::scan_files(Rc::clone(&db.dir_root));
                    rv_core::repair_status::RepairStatus::report_status_reset(Rc::clone(&db.dir_root));
                }
            });

            let _ = tx.send("Fixing...".to_string());
            GLOBAL_DB.with(|db_ref| {
                if let Some(db) = db_ref.borrow().as_ref() {
                    Fix::perform_fixes(Rc::clone(&db.dir_root));
                }
            });

            let _ = tx.send("Rescanning to sync DB with disk...".to_string());
            Self::scan_selected_roots(&tx, rv_core::settings::EScanLevel::Level2);

            let _ = tx.send("Finalizing Fixes...".to_string());
            GLOBAL_DB.with(|db_ref| {
                if let Some(db) = db_ref.borrow().as_ref() {
                    FindFixes::scan_files(Rc::clone(&db.dir_root));
                    rv_core::repair_status::RepairStatus::report_status_reset(Rc::clone(&db.dir_root));
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
                            got = stats.roms_correct + stats.roms_correct_mia;
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
                            
                            got = stats.roms_correct + stats.roms_correct_mia;
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
