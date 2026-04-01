use eframe::egui;
use std::cell::RefCell;
use std::rc::Rc;

use crate::RomVaultApp;
use dat_reader::enums::{DatStatus, FileType};
use rv_core::db::GLOBAL_DB;
use rv_core::enums::RepStatus;
use rv_core::file_scanning::FileScanning;
use rv_core::fix_dat_report::FixDatReport;
use rv_core::read_dat::DatUpdate;
use rv_core::rv_file::{RvFile, TreeSelect};
use rv_core::scanner::Scanner;

fn merged_roms(stats: &rv_core::repair_status::RepairStatus) -> i32 {
    stats.roms_not_collected + stats.roms_unneeded
}

fn correct_roms(stats: &rv_core::repair_status::RepairStatus) -> i32 {
    stats.count_correct()
}

fn missing_roms(stats: &rv_core::repair_status::RepairStatus) -> i32 {
    stats.roms_missing + stats.roms_missing_mia
}

fn unknown_roms(stats: &rv_core::repair_status::RepairStatus) -> i32 {
    stats.roms_unknown
}

fn tree_color_from_rep_status(rep_status: RepStatus, dat_status: DatStatus) -> egui::Color32 {
    match rep_status {
        RepStatus::Correct | RepStatus::CorrectMIA => egui::Color32::from_rgb(0, 200, 0),
        RepStatus::Missing | RepStatus::MissingMIA | RepStatus::Corrupt | RepStatus::DirCorrupt | RepStatus::Incomplete => {
            egui::Color32::from_rgb(200, 0, 0)
        }
        RepStatus::CanBeFixed | RepStatus::CanBeFixedMIA | RepStatus::CorruptCanBeFixed => {
            egui::Color32::from_rgb(200, 200, 0)
        }
        RepStatus::MoveToSort | RepStatus::MoveToCorrupt | RepStatus::NeededForFix | RepStatus::Rename => {
            egui::Color32::from_rgb(0, 200, 200)
        }
        RepStatus::Delete => egui::Color32::from_rgb(200, 0, 0),
        RepStatus::NotCollected | RepStatus::UnNeeded | RepStatus::Unknown => egui::Color32::from_rgb(150, 150, 150),
        _ => {
            if dat_status == DatStatus::NotInDat {
                egui::Color32::from_rgb(150, 150, 150)
            } else {
                egui::Color32::WHITE
            }
        }
    }
}

fn tree_color_from_stats(stats: &rv_core::repair_status::RepairStatus) -> egui::Color32 {
    if stats.total_roms == 0 && stats.roms_unknown == 0 {
        egui::Color32::from_rgb(150, 150, 150)
    } else if unknown_roms(stats) == stats.total_roms && stats.total_roms > 0 {
        egui::Color32::from_rgb(150, 150, 150)
    } else if merged_roms(stats) == stats.total_roms && stats.total_roms > 0 {
        egui::Color32::from_rgb(150, 150, 150)
    } else if correct_roms(stats) == stats.total_roms && stats.total_roms > 0 {
        egui::Color32::from_rgb(0, 200, 0)
    } else if missing_roms(stats) == stats.total_roms && stats.total_roms > 0 {
        egui::Color32::from_rgb(200, 0, 0)
    } else if correct_roms(stats) > 0 {
        egui::Color32::from_rgb(200, 200, 0)
    } else if stats.roms_fixes > 0 {
        egui::Color32::from_rgb(200, 200, 0)
    } else {
        egui::Color32::WHITE
    }
}

fn tree_icon_idx_from_stats(stats: &rv_core::repair_status::RepairStatus) -> i32 {
    if stats.total_roms == 0 {
        2
    } else if unknown_roms(stats) == stats.total_roms {
        4
    } else if merged_roms(stats) == stats.total_roms {
        4
    } else if correct_roms(stats) == stats.total_roms {
        3
    } else if missing_roms(stats) == stats.total_roms {
        1
    } else {
        2
    }
}

fn tree_icon_idx_from_report_status(report_status: rv_core::enums::ReportStatus) -> i32 {
    if !report_status.has_correct() && report_status.has_missing(false) {
        1
    } else if report_status.has_unknown() || report_status.has_all_merged() {
        4
    } else if !report_status.has_missing(false) && report_status.has_mia() {
        5
    } else if !report_status.has_missing(false) {
        3
    } else {
        2
    }
}

/// Contains the logic for rendering the recursive left-hand directory tree.
/// 
/// `tree.rs` implements `RomVaultApp::draw_tree_node`, which is called every frame to
/// visually construct the hierarchical representation of the `dir_root` DB tree.
/// 
/// Differences from C#:
/// - C# uses `WinForms.TreeView` and dynamically loads children via `OnBeforeExpand` events.
/// - The Rust version uses `egui::CollapsingHeader` and traverses the actual `RvFile` pointers
///   in memory every frame. It relies entirely on `node.cached_stats` (computed by `RepairStatus`) 
///   to instantaneously color-code folders without deep recursion on the UI thread.
impl RomVaultApp {
    /// Recursively draws a single `RvFile` node and its children in the UI tree.
    pub fn draw_tree_node(&mut self, ui: &mut egui::Ui, node_rc: Rc<RefCell<RvFile>>, parent_path: String) {
        let is_file;
        let is_directory;
        let is_game;
        let color;
        let icon_idx;
        let img_src;
        let tree_checked;
        let tree_expanded;
        let node_name;
        let cached_stats;
        let is_in_to_sort;
        let to_sort_is_primary;
        let to_sort_is_cache;
        let node_path;
        let mut ui_display_name;

        {
            let mut node = node_rc.borrow_mut();
            is_file = node.is_file();
            is_directory = node.is_directory();
            is_game = node.game.is_some();
            node_name = node.name.clone();
            ui_display_name = node.ui_display_name.clone();

            node_path = if parent_path.is_empty() {
                node_name.clone()
            } else {
                format!("{}\\{}", parent_path, node_name)
            };

            is_in_to_sort = node.dat_status() == DatStatus::InToSort;
            to_sort_is_primary = node.to_sort_status_is(rv_core::enums::ToSortDirType::TO_SORT_PRIMARY);
            to_sort_is_cache = node.to_sort_status_is(rv_core::enums::ToSortDirType::TO_SORT_CACHE);

            if is_directory && node.cached_stats.is_none() {
                drop(node);
                let mut stats = rv_core::repair_status::RepairStatus::new();
                stats.report_status(Rc::clone(&node_rc));

                node = node_rc.borrow_mut();
                node.cached_stats = Some(stats.clone());
                node.ui_display_name.clear();
                ui_display_name.clear();

                cached_stats = Some(stats);
            } else {
                cached_stats = node.cached_stats.clone();
            }

            color = if let Some(stats) = &cached_stats {
                tree_color_from_stats(stats)
            } else {
                tree_color_from_rep_status(node.rep_status(), node.dat_status())
            };

            icon_idx = if let Some(stats) = &cached_stats {
                tree_icon_idx_from_stats(stats)
            } else if let Some(ds) = &node.dir_status {
                tree_icon_idx_from_report_status(*ds)
            } else {
                2
            };

            img_src = if node.dat.is_none() && node.dir_dats.is_empty() {
                match icon_idx {
                    1 => include_asset!("DirectoryTree1.png"),
                    2 => include_asset!("DirectoryTree2.png"),
                    3 => include_asset!("DirectoryTree3.png"),
                    4 => include_asset!("DirectoryTree4.png"),
                    5 => include_asset!("DirectoryTree5.png"),
                    _ => include_asset!("DirectoryTree3.png"),
                }
            } else {
                match icon_idx {
                    1 => include_asset!("Tree1.png"),
                    2 => include_asset!("Tree2.png"),
                    3 => include_asset!("Tree3.png"),
                    4 => include_asset!("Tree4.png"),
                    5 => include_asset!("Tree5.png"),
                    _ => include_asset!("Tree3.png"),
                }
            };

            tree_checked = node.tree_checked.clone();
            tree_expanded = node.tree_expanded;

            if is_directory && ui_display_name.is_empty() {
                let icon = match node.file_type {
                    FileType::Dir => "📁",
                    FileType::Zip | FileType::SevenZip => "🗄",
                    _ => "📄",
                };
                let mut name = format!("{} {}", icon, node.name);

                if is_in_to_sort {
                    if to_sort_is_primary && to_sort_is_cache {
                        name = format!("{} (Primary)", name);
                    } else if to_sort_is_primary {
                        name = format!("{} (Primary)", name);
                    } else if to_sort_is_cache {
                        name = format!("{} (Cache)", name);
                    } else if node.to_sort_status_is(rv_core::enums::ToSortDirType::TO_SORT_FILE_ONLY) {
                        name = format!("{} (File Only)", name);
                    }

                    if let Some(stats) = cached_stats {
                        name = format!("{} (Files: {})", name, crate::format_number(stats.total_roms));
                    } else {
                        name = format!("{} (Files: 0)", name);
                    }
                } else {
                    if node.dat.is_none() && node.dir_dats.len() == 1 {
                        let desc = node.dir_dats[0]
                            .borrow()
                            .get_data(rv_core::rv_dat::DatData::Description)
                            .unwrap_or_default();
                        if !desc.is_empty() {
                            name = format!("{}: {}", name, desc);
                        }
                    } else if let Some(dat) = &node.dat {
                        if dat
                            .borrow()
                            .dat_flags
                            .contains(rv_core::rv_dat::DatFlags::AUTO_ADDED_DIRECTORY)
                        {
                            name = format!("{}: ", name);
                        }
                    }

                    if let Some(stats) = cached_stats {
                        let mut parts = Vec::new();
                        // Always show Have and Missing if there are any stats at all, to match C# reference
                        if stats.total_roms > 0 {
                            parts.push(format!("Have: {}", crate::format_number(stats.roms_correct)));
                            if stats.roms_correct_mia > 0 {
                                parts.push(format!("Found MIA: {}", crate::format_number(stats.roms_correct_mia)));
                            }
                            parts.push(format!("Missing: {}", crate::format_number(stats.roms_missing)));
                            if stats.roms_missing_mia > 0 {
                                parts.push(format!("MIA: {}", crate::format_number(stats.roms_missing_mia)));
                            }
                            if stats.roms_fixes > 0 {
                                parts.push(format!("Fixes: {}", crate::format_number(stats.roms_fixes)));
                            }
                            if stats.roms_not_collected > 0 {
                                parts.push(format!("NotCollected: {}", crate::format_number(stats.roms_not_collected)));
                            }
                            if stats.roms_unknown > 0 {
                                parts.push(format!("Unknown: {}", crate::format_number(stats.roms_unknown)));
                            }
                            if stats.roms_unneeded > 0 {
                                parts.push(format!("UnNeeded: {}", crate::format_number(stats.roms_unneeded)));
                            }
                        }

                        if !parts.is_empty() {
                            name = format!("{} ( {} )", name, parts.join(" \\ "));
                        } else {
                            name = format!("{} ( Have: 0 \\ Missing: 0 )", name);
                        }
                    }
                }

                node.ui_display_name = name.clone();
                ui_display_name = name;
            } else if !is_directory && ui_display_name.is_empty() {
                let name = node.name.clone();
                node.ui_display_name = name.clone();
                ui_display_name = name;
            }
        }

        if is_file || is_game {
            return;
        }

        let has_expandable_children = if is_directory {
            // A node is expandable if it has at least one child that is NOT a file and NOT a game.
            // i.e. it contains another directory or a DAT folder
            node_rc.borrow().children.iter().any(|c| {
                let cb = c.borrow();
                !cb.is_file() && cb.game.is_none()
            })
        } else {
            false
        };

        let _node_id = ui.make_persistent_id(&node_path);
        let mut toggle_expanded = false;
        let mut clicked_label = false;
        let row_height = 18.0;
        let current_y = ui.cursor().min.y;
        let is_visible = ui.clip_rect().intersects(egui::Rect::from_min_size(
            egui::pos2(ui.cursor().min.x, current_y),
            egui::vec2(ui.available_width(), row_height),
        ));

        let row_rect = ui.allocate_exact_size(egui::vec2(ui.available_width(), row_height), egui::Sense::click());
        let np_clone = node_path.clone();

        if is_visible {
            let mut ui_builder = ui.child_ui(row_rect.0, *ui.layout());

            ui_builder.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 2.0;

                if has_expandable_children {
                    let expand_icon = if tree_expanded {
                        include_asset!("ExpandBoxMinus.png")
                    } else {
                        include_asset!("ExpandBoxPlus.png")
                    };
                    let expand_resp =
                        ui.add_sized([9.0, 9.0], egui::ImageButton::new(expand_icon).frame(false));
                    if expand_resp.clicked() {
                        toggle_expanded = true;
                    }
                } else {
                    ui.add_space(9.0);
                }

                let checkbox_img = match tree_checked {
                    TreeSelect::Selected => include_asset!("TickBoxTicked.png"),
                    TreeSelect::UnSelected => include_asset!("TickBoxUnTicked.png"),
                    TreeSelect::Locked => include_asset!("TickBoxLocked.png"),
                };
                let checkbox_resp =
                    ui.add_sized([13.0, 13.0], egui::ImageButton::new(checkbox_img).frame(false));
                if checkbox_resp.clicked() {
                    let is_shift = ui.input(|i| i.modifiers.shift);
                    let new_state = match tree_checked {
                        TreeSelect::Selected => TreeSelect::UnSelected,
                        _ => TreeSelect::Selected,
                    };

                    let mut stack = vec![Rc::clone(&node_rc)];
                    while let Some(current) = stack.pop() {
                        let mut n = current.borrow_mut();
                        n.tree_checked = new_state.clone();
                        let children = n.children.clone();
                        drop(n);
                        if !is_shift {
                            for child in children {
                                stack.push(Rc::clone(&child));
                            }
                        }
                    }
                } else if checkbox_resp.secondary_clicked() {
                    let is_shift = ui.input(|i| i.modifiers.shift);
                    let mut stack = vec![Rc::clone(&node_rc)];
                    while let Some(current) = stack.pop() {
                        let mut n = current.borrow_mut();
                        if n.to_sort_status_is(rv_core::enums::ToSortDirType::TO_SORT_PRIMARY) {
                            continue;
                        }
                        n.tree_checked = TreeSelect::Locked;
                        let children = n.children.clone();
                        drop(n);
                        if !is_shift {
                            for child in children {
                                stack.push(Rc::clone(&child));
                            }
                        }
                    }
                }

                ui.add_sized([16.0, row_height], egui::Image::new(img_src).max_width(16.0));

                let is_selected = self
                    .selected_node
                    .as_ref()
                    .map_or(false, |n| Rc::ptr_eq(n, &node_rc));

                let clean_name = ui_display_name
                    .trim_start_matches(|c: char| !c.is_alphanumeric() && c != '(' && c != '[')
                    .trim();
                let label_text = egui::RichText::new(clean_name).color(color);
                let label_resp = if is_selected {
                    let bg_color = ui.visuals().selection.bg_fill;
                    ui.painter().rect_filled(ui.cursor(), 0.0, bg_color);
                    ui.add(egui::Label::new(label_text).sense(egui::Sense::click()))
                } else {
                    ui.add(egui::Label::new(label_text).sense(egui::Sense::click()))
                };

                if label_resp.clicked() {
                    clicked_label = true;
                }

                enum TreeAction {
                    ScanQuick,
                    ScanNormal,
                    ScanFull,
                    UpdateDats,
                }
                let mut pending_action = None;

                label_resp.context_menu(|ui| {
                    if ui.button("Scan Quick (Headers Only)").clicked() {
                        pending_action = Some(TreeAction::ScanQuick);
                        ui.close_menu();
                    }
                    if ui.button("Scan").clicked() {
                        pending_action = Some(TreeAction::ScanNormal);
                        ui.close_menu();
                    }
                    if ui.button("Scan Full (Complete Re-Scan)").clicked() {
                        pending_action = Some(TreeAction::ScanFull);
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui.button("Directory Settings").clicked() {
                        self.active_dat_rule = rv_core::settings::find_rule(&node_name);
                        self.show_dir_settings = true;
                        ui.close_menu();
                    }
                    if ui.button("Directory Mappings").clicked() {
                        self.open_dir_mappings();
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui.button("Open Directory").clicked() {
                        self.task_logs.push(format!("Opening Directory: {}", np_clone));
                        let _ = std::process::Command::new("explorer").arg(&np_clone).spawn();
                        ui.close_menu();
                    }

                    if is_in_to_sort {
                        if ui.button("Open ToSort Directory").clicked() {
                            self.task_logs.push(format!("Opening ToSort Directory: {}", np_clone));
                            let _ = std::process::Command::new("explorer").arg(&np_clone).spawn();
                            ui.close_menu();
                        }
                        ui.separator();
                        if ui.button("Move Up").clicked() {
                            self.task_logs.push(format!("Move ToSort Up: {}", node_rc.borrow().name));
                            GLOBAL_DB.with(|db_ref| {
                                if let Some(db) = db_ref.borrow().as_ref() {
                                    let mut dir_root = db.dir_root.borrow_mut();
                                    let mut idx = None;
                                    for (i, child) in dir_root.children.iter().enumerate() {
                                        if Rc::ptr_eq(child, &node_rc) {
                                            idx = Some(i);
                                            break;
                                        }
                                    }
                                    if let Some(i) = idx {
                                        // Ensure we don't swap it above the first ToSort folder (idx 1 usually, as RustyVault is 0)
                                        // or above RustyVault itself
                                        if i > 1 {
                                            dir_root.children.swap(i, i - 1);
                                        }
                                    }
                                    drop(dir_root);
                                    db.write_cache();
                                }
                            });
                            ui.close_menu();
                        }
                        if ui.button("Move Down").clicked() {
                            self.task_logs.push(format!("Move ToSort Down: {}", node_rc.borrow().name));
                            GLOBAL_DB.with(|db_ref| {
                                if let Some(db) = db_ref.borrow().as_ref() {
                                    let mut dir_root = db.dir_root.borrow_mut();
                                    let mut idx = None;
                                    for (i, child) in dir_root.children.iter().enumerate() {
                                        if Rc::ptr_eq(child, &node_rc) {
                                            idx = Some(i);
                                            break;
                                        }
                                    }
                                    if let Some(i) = idx {
                                        if i < dir_root.children.len() - 1 {
                                            dir_root.children.swap(i, i + 1);
                                        }
                                    }
                                    drop(dir_root);
                                    db.write_cache();
                                }
                            });
                            ui.close_menu();
                        }
                        if ui.button("Set To Primary ToSort").clicked() {
                            self.task_logs.push(format!("Set To Primary ToSort: {}", node_rc.borrow().name));
                            node_rc
                                .borrow_mut()
                                .to_sort_status_set(rv_core::enums::ToSortDirType::TO_SORT_PRIMARY);
                            GLOBAL_DB.with(|db_ref| {
                                if let Some(db) = db_ref.borrow().as_ref() {
                                    db.write_cache();
                                }
                            });
                            ui.close_menu();
                        }
                        if ui.button("Set To Cache ToSort").clicked() {
                            self.task_logs.push(format!("Set To Cache ToSort: {}", node_rc.borrow().name));
                            node_rc
                                .borrow_mut()
                                .to_sort_status_set(rv_core::enums::ToSortDirType::TO_SORT_CACHE);
                            GLOBAL_DB.with(|db_ref| {
                                if let Some(db) = db_ref.borrow().as_ref() {
                                    db.write_cache();
                                }
                            });
                            ui.close_menu();
                        }
                        if ui.button("Set To File Only ToSort").clicked() {
                            self.task_logs.push(format!("Set To File Only ToSort: {}", node_rc.borrow().name));
                            node_rc
                                .borrow_mut()
                                .to_sort_status_set(rv_core::enums::ToSortDirType::TO_SORT_FILE_ONLY);
                            GLOBAL_DB.with(|db_ref| {
                                if let Some(db) = db_ref.borrow().as_ref() {
                                    db.write_cache();
                                }
                            });
                            ui.close_menu();
                        }
                        if ui.button("Clear File Only ToSort").clicked() {
                            self.task_logs.push(format!("Clear File Only ToSort: {}", node_rc.borrow().name));
                            node_rc
                                .borrow_mut()
                                .to_sort_status_clear(rv_core::enums::ToSortDirType::TO_SORT_FILE_ONLY);
                            GLOBAL_DB.with(|db_ref| {
                                if let Some(db) = db_ref.borrow().as_ref() {
                                    db.write_cache();
                                }
                            });
                            ui.close_menu();
                        }
                        ui.separator();
                        if ui.button("Remove").clicked() {
                            self.task_logs.push(format!("Remove ToSort Directory: {}", node_rc.borrow().name));
                            GLOBAL_DB.with(|db_ref| {
                                if let Some(db) = db_ref.borrow().as_ref() {
                                    let mut dir_root = db.dir_root.borrow_mut();
                                    let mut idx_to_remove = None;
                                    for (i, child) in dir_root.children.iter().enumerate() {
                                        if Rc::ptr_eq(child, &node_rc) {
                                            idx_to_remove = Some(i);
                                            break;
                                        }
                                    }
                                    if let Some(idx) = idx_to_remove {
                                        dir_root.child_remove(idx);
                                    }
                                    drop(dir_root);
                                    db.write_cache();
                                }
                            });
                            ui.close_menu();
                        }
                    } else {
                        if ui.button("Set Dir Dat Settings").clicked() {
                            self.active_dat_rule = rv_core::settings::find_rule(&node_name);
                            self.show_dir_settings = true;
                            ui.close_menu();
                        }
                        if ui.button("Set Dir Mappings").clicked() {
                            self.open_dir_mappings();
                            ui.close_menu();
                        }
                        if ui.button("Update DATs").clicked() {
                            pending_action = Some(TreeAction::UpdateDats);
                            ui.close_menu();
                        }
                        ui.separator();
                        if ui.button("Open Directory").clicked() {
                            self.task_logs.push(format!("Opening Directory: {}", node_path));
                            let _ = std::process::Command::new("explorer").arg(&node_path).spawn();
                            ui.close_menu();
                        }
                        ui.separator();
                        if ui.button("Save fix DATs").clicked() {
                            let node_clone = Rc::clone(&node_rc);
                            self.launch_task("Save fix DATs", move |tx| {
                                let _ = tx.send("Generating FixDATs to Desktop...".to_string());
                                GLOBAL_DB.with(|_db_ref| {
                                    let desktop_path = std::path::PathBuf::from(
                                        std::env::var("USERPROFILE").unwrap_or_default(),
                                    )
                                    .join("Desktop");
                                    FixDatReport::recursive_dat_tree(&desktop_path.to_string_lossy(), node_clone, true);
                                });
                            });
                            ui.close_menu();
                        }
                        if ui.button("Save full DAT").clicked() {
                            let node_clone = Rc::clone(&node_rc);
                            self.launch_task("Save full DAT", move |tx| {
                                let _ = tx.send("Generating Full DAT to Desktop...".to_string());
                                GLOBAL_DB.with(|_db_ref| {
                                    let desktop_path = std::path::PathBuf::from(
                                        std::env::var("USERPROFILE").unwrap_or_default(),
                                    )
                                    .join("Desktop");
                                    FixDatReport::recursive_dat_tree(&desktop_path.to_string_lossy(), node_clone, false);
                                });
                            });
                            ui.close_menu();
                        }
                        if ui.button("Make DAT").clicked() {
                            self.task_logs.push(
                                "Make DAT functionality requires ExternalDatConverterTo implementation".to_string(),
                            );
                            ui.close_menu();
                        }
                    }
                });

                if let Some(action) = pending_action {
                    match action {
                        TreeAction::ScanQuick => {
                            let np = np_clone.clone();
                            let target_rc = Rc::clone(&node_rc);
                            self.launch_task("Scan ROMs (Quick)", move |tx| {
                                let _ = tx.send(format!("Scanning {} (Headers Only)...", np));
                                let files = Scanner::scan_directory_with_level(&np, rv_core::settings::EScanLevel::Level1);
                                let mut root_scan = rv_core::scanned_file::ScannedFile::new(FileType::Dir);
                                root_scan.children = files;
                                let _ = tx.send("Integrating files into DB...".to_string());
                                FileScanning::scan_dir_with_level(target_rc, &mut root_scan, rv_core::settings::EScanLevel::Level1);
                            });
                        }
                        TreeAction::ScanNormal => {
                            let np = np_clone.clone();
                            let target_rc = Rc::clone(&node_rc);
                            self.launch_task("Scan ROMs", move |tx| {
                                let _ = tx.send(format!("Scanning {}...", np));
                                let files = Scanner::scan_directory_with_level(&np, rv_core::settings::EScanLevel::Level2);
                                let mut root_scan = rv_core::scanned_file::ScannedFile::new(FileType::Dir);
                                root_scan.children = files;
                                let _ = tx.send("Integrating files into DB...".to_string());
                                FileScanning::scan_dir_with_level(target_rc, &mut root_scan, rv_core::settings::EScanLevel::Level2);
                            });
                        }
                        TreeAction::ScanFull => {
                            let np = np_clone.clone();
                            let target_rc = Rc::clone(&node_rc);
                            self.launch_task("Scan ROMs (Full)", move |tx| {
                                let _ = tx.send(format!("Scanning {} (Full Re-Scan)...", np));
                                let files = Scanner::scan_directory_with_level(&np, rv_core::settings::EScanLevel::Level3);
                                let mut root_scan = rv_core::scanned_file::ScannedFile::new(FileType::Dir);
                                root_scan.children = files;
                                let _ = tx.send("Integrating files into DB...".to_string());
                                FileScanning::scan_dir_with_level(target_rc, &mut root_scan, rv_core::settings::EScanLevel::Level3);
                            });
                        }
                        TreeAction::UpdateDats => {
                            let np = np_clone.clone();
                            let target_rc = Rc::clone(&node_rc);
                            self.launch_task("Update DATs", move |tx| {
                                let _ = tx.send(format!("Updating DATs for {}...", np));
                                DatUpdate::update_dat(target_rc, &np);
                            });
                        }
                    }
                }

                if clicked_label {
                    self.selected_node = Some(Rc::clone(&node_rc));
                }

                if toggle_expanded {
                    let mut n = node_rc.borrow_mut();
                    n.tree_expanded = !n.tree_expanded;
                }
            });
        } else if toggle_expanded {
            let mut n = node_rc.borrow_mut();
            n.tree_expanded = !n.tree_expanded;
        }

        if tree_expanded && has_expandable_children {
            let start_y = current_y + row_height;

            ui.horizontal(|ui| {
                ui.add_space(18.0);
                ui.vertical(|ui| {
                    let children = node_rc.borrow().children.clone();
                    for child in children {
                        self.draw_tree_node(ui, child, np_clone.clone());
                    }
                });
            });

            let end_y = ui.cursor().min.y;
            let line_x = row_rect.0.min.x + 9.0;
            let line_rect =
                egui::Rect::from_min_max(egui::pos2(line_x, start_y), egui::pos2(line_x + 1.0, end_y));
            if ui.clip_rect().intersects(line_rect) {
                let stroke = egui::Stroke::new(1.0, egui::Color32::from_gray(100));
                ui.painter()
                    .line_segment([egui::pos2(line_x, start_y), egui::pos2(line_x, end_y)], stroke);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tree_color_from_stats_treats_all_merged_branch_as_grey() {
        let mut stats = rv_core::repair_status::RepairStatus::new();
        stats.total_roms = 2;
        stats.roms_not_collected = 1;
        stats.roms_unneeded = 1;

        assert_eq!(tree_color_from_stats(&stats), egui::Color32::from_rgb(150, 150, 150));
    }

    #[test]
    fn test_tree_icon_idx_from_stats_uses_merged_icon_for_all_merged_branch() {
        let mut stats = rv_core::repair_status::RepairStatus::new();
        stats.total_roms = 2;
        stats.roms_not_collected = 1;
        stats.roms_unneeded = 1;

        assert_eq!(tree_icon_idx_from_stats(&stats), 4);
    }

    #[test]
    fn test_tree_color_from_stats_treats_correct_mia_branch_as_green() {
        let mut stats = rv_core::repair_status::RepairStatus::new();
        stats.total_roms = 2;
        stats.roms_correct_mia = 2;

        assert_eq!(tree_color_from_stats(&stats), egui::Color32::from_rgb(0, 200, 0));
    }

    #[test]
    fn test_tree_icon_idx_from_stats_uses_green_icon_for_correct_mia_branch() {
        let mut stats = rv_core::repair_status::RepairStatus::new();
        stats.total_roms = 2;
        stats.roms_correct_mia = 2;

        assert_eq!(tree_icon_idx_from_stats(&stats), 3);
    }

    #[test]
    fn test_tree_color_from_stats_treats_missing_mia_branch_as_red() {
        let mut stats = rv_core::repair_status::RepairStatus::new();
        stats.total_roms = 2;
        stats.roms_missing_mia = 2;

        assert_eq!(tree_color_from_stats(&stats), egui::Color32::from_rgb(200, 0, 0));
    }

    #[test]
    fn test_tree_icon_idx_from_stats_uses_red_icon_for_missing_mia_branch() {
        let mut stats = rv_core::repair_status::RepairStatus::new();
        stats.total_roms = 2;
        stats.roms_missing_mia = 2;

        assert_eq!(tree_icon_idx_from_stats(&stats), 1);
    }

    #[test]
    fn test_tree_color_from_stats_treats_fixable_only_branch_as_yellow() {
        let mut stats = rv_core::repair_status::RepairStatus::new();
        stats.total_roms = 2;
        stats.roms_fixes = 2;

        assert_eq!(tree_color_from_stats(&stats), egui::Color32::from_rgb(200, 200, 0));
    }

    #[test]
    fn test_tree_color_from_rep_status_treats_corrupt_can_be_fixed_as_yellow() {
        assert_eq!(
            tree_color_from_rep_status(RepStatus::CorruptCanBeFixed, DatStatus::InDatCollect),
            egui::Color32::from_rgb(200, 200, 0)
        );
    }

    #[test]
    fn test_tree_color_from_rep_status_treats_move_to_corrupt_as_cyan() {
        assert_eq!(
            tree_color_from_rep_status(RepStatus::MoveToCorrupt, DatStatus::InDatCollect),
            egui::Color32::from_rgb(0, 200, 200)
        );
    }

    #[test]
    fn test_tree_color_from_rep_status_treats_needed_for_fix_and_rename_as_cyan() {
        assert_eq!(
            tree_color_from_rep_status(RepStatus::NeededForFix, DatStatus::InDatCollect),
            egui::Color32::from_rgb(0, 200, 200)
        );
        assert_eq!(
            tree_color_from_rep_status(RepStatus::Rename, DatStatus::InDatCollect),
            egui::Color32::from_rgb(0, 200, 200)
        );
    }

    #[test]
    fn test_tree_color_from_rep_status_treats_delete_as_red() {
        assert_eq!(
            tree_color_from_rep_status(RepStatus::Delete, DatStatus::NotInDat),
            egui::Color32::from_rgb(200, 0, 0)
        );
    }

    #[test]
    fn test_tree_color_from_rep_status_treats_missing_family_variants_as_red() {
        assert_eq!(
            tree_color_from_rep_status(RepStatus::Corrupt, DatStatus::InDatCollect),
            egui::Color32::from_rgb(200, 0, 0)
        );
        assert_eq!(
            tree_color_from_rep_status(RepStatus::DirCorrupt, DatStatus::InDatCollect),
            egui::Color32::from_rgb(200, 0, 0)
        );
        assert_eq!(
            tree_color_from_rep_status(RepStatus::Incomplete, DatStatus::InDatCollect),
            egui::Color32::from_rgb(200, 0, 0)
        );
    }

    #[test]
    fn test_tree_color_from_stats_treats_unknown_only_branch_as_grey() {
        let mut stats = rv_core::repair_status::RepairStatus::new();
        stats.total_roms = 2;
        stats.roms_unknown = 2;

        assert_eq!(tree_color_from_stats(&stats), egui::Color32::from_rgb(150, 150, 150));
    }

    #[test]
    fn test_tree_color_from_rep_status_treats_unknown_as_grey() {
        assert_eq!(
            tree_color_from_rep_status(RepStatus::Unknown, DatStatus::InDatCollect),
            egui::Color32::from_rgb(150, 150, 150)
        );
    }

    #[test]
    fn test_tree_icon_idx_from_report_status_treats_unknown_as_grey_icon() {
        assert_eq!(tree_icon_idx_from_report_status(rv_core::enums::ReportStatus::Unknown), 4);
    }

    #[test]
    fn test_tree_icon_idx_from_report_status_treats_merged_as_grey_icon() {
        assert_eq!(tree_icon_idx_from_report_status(rv_core::enums::ReportStatus::NotCollected), 4);
    }

    #[test]
    fn test_tree_icon_idx_from_stats_treats_unknown_only_branch_as_grey_icon() {
        let mut stats = rv_core::repair_status::RepairStatus::new();
        stats.total_roms = 2;
        stats.roms_unknown = 2;

        assert_eq!(tree_icon_idx_from_stats(&stats), 4);
    }
}

