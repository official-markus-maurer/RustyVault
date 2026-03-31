use eframe::egui;
use std::cell::RefCell;
use std::rc::Rc;

use crate::RomVaultApp;
use crate::utils::get_full_node_path;
use dat_reader::enums::FileType;
use rv_core::enums::RepStatus;
use rv_core::file_scanning::FileScanning;
use rv_core::rv_file::RvFile;
use rv_core::scanner::Scanner;

/// Logic for rendering the DataGridView component.
/// 
/// `grids.rs` contains the logic for rendering the right-hand panel of the main UI,
/// which displays the children of the currently selected tree node in a tabular format.
/// 
/// Differences from C#:
/// - C# utilizes the stateful `WinForms.DataGridView` control.
/// - The Rust version manually draws an `egui::Grid`, dynamically fetching the currently 
///   selected node from the `RomVaultApp` state and rendering its children every frame.
impl RomVaultApp {
    pub fn draw_game_grid(&mut self, ui: &mut egui::Ui) {
        let selection_color = ui.style().visuals.selection.bg_fill;

        enum GridAction {
            ScanQuick(Rc<RefCell<RvFile>>),
            ScanNormal(Rc<RefCell<RvFile>>),
            ScanFull(Rc<RefCell<RvFile>>),
            NavigateUp,
            NavigateDown(Rc<RefCell<RvFile>>),
            FixRom(Rc<RefCell<RvFile>>),
        }
        let mut pending_action = None;

        let mut new_sort_col = self.sort_col.clone();
        let mut new_sort_desc = self.sort_desc;

        egui::ScrollArea::horizontal().show(ui, |ui| {
            egui_extras::TableBuilder::new(ui)
                .striped(true)
                .resizable(true)
                .vscroll(true)
                .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
                .column(egui_extras::Column::initial(40.0).at_least(40.0))
                .column(egui_extras::Column::initial(350.0).at_least(40.0))
                .column(egui_extras::Column::initial(350.0).at_least(40.0))
                .column(egui_extras::Column::initial(150.0).at_least(40.0))
                .column(egui_extras::Column::remainder())
                .header(20.0, |mut header| {
                    let mut make_header = |ui: &mut egui::Ui, title: &str| {
                        if ui
                            .selectable_label(new_sort_col.as_deref() == Some(title), title)
                            .clicked()
                        {
                            if new_sort_col.as_deref() == Some(title) {
                                new_sort_desc = !new_sort_desc;
                            } else {
                                new_sort_col = Some(title.to_string());
                                new_sort_desc = false;
                            }
                        }
                    };
                    header.col(|ui| make_header(ui, "Type"));
                    header.col(|ui| make_header(ui, "Game (Directory / Zip)"));
                    header.col(|ui| make_header(ui, "Description"));
                    header.col(|ui| make_header(ui, "Modified"));
                    header.col(|ui| make_header(ui, "ROM Status"));
                })
                .body(|mut body| {
                    if let Some(selected) = &self.selected_node {
                        let node = selected.borrow();

                        if node.parent.is_some() {
                            body.row(20.0, |mut row| {
                                row.col(|ui| {
                                    ui.add(
                                        egui::Image::new(include_asset!("Dir.png"))
                                            .texture_options(egui::TextureOptions::NEAREST)
                                            .max_width(16.0),
                                    );
                                });
                                row.col(|ui| {
                                    let label_resp = ui.add(egui::SelectableLabel::new(false, ".."));
                                    if label_resp.double_clicked() {
                                        pending_action = Some(GridAction::NavigateUp);
                                    }
                                });
                                row.col(|ui| {
                                    ui.label("");
                                });
                                row.col(|ui| {
                                    ui.label("");
                                });
                                row.col(|ui| {
                                    ui.label("");
                                });
                            });
                        }

                        let mut sorted_children: Vec<Rc<RefCell<RvFile>>> = node
                            .children
                            .iter()
                            .filter(|c| !c.borrow().is_file() || c.borrow().game.is_some())
                            .cloned()
                            .collect();

                        if let Some(col) = &self.sort_col {
                            let desc = self.sort_desc;
                            sorted_children.sort_by(|a, b| {
                                let a = a.borrow();
                                let b = b.borrow();
                                let cmp = match col.as_str() {
                                    "Game (Directory / Zip)" => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
                                    "Description" => {
                                        let da = a
                                            .game
                                            .as_ref()
                                            .and_then(|g| {
                                                g.borrow().get_data(rv_core::rv_game::GameData::Description)
                                            })
                                            .unwrap_or_default();
                                        let db = b
                                            .game
                                            .as_ref()
                                            .and_then(|g| {
                                                g.borrow().get_data(rv_core::rv_game::GameData::Description)
                                            })
                                            .unwrap_or_default();
                                        da.cmp(&db)
                                    }
                                    "Type" => a.file_type.cmp(&b.file_type),
                                    "Modified" => a.file_mod_time_stamp.cmp(&b.file_mod_time_stamp),
                                    _ => a.name.cmp(&b.name),
                                };
                                if desc { cmp.reverse() } else { cmp }
                            });
                        }

                        for child_rc in sorted_children {
                            let child = child_rc.borrow();

                            if child.is_file() && child.game.is_none() {
                                continue;
                            }

                            let mut should_show = false;

                            if let Some(dir_status) = child.dir_status {
                                let g_correct = dir_status.has_correct();
                                let g_missing = dir_status.has_missing(false);
                                let g_fixes = dir_status.has_fixes_needed();
                                let g_mia = dir_status.has_mia();
                                let g_merged = dir_status.has_all_merged();
                                let g_unknown = dir_status.has_unknown();

                                should_show =
                                    should_show || (self.show_complete && g_correct && !g_missing && !g_fixes);
                                should_show = should_show || (self.show_partial && g_correct && g_missing);
                                should_show = should_show || (self.show_empty && !g_correct && g_missing);
                                should_show = should_show || (self.show_fixes && g_fixes);
                                should_show = should_show || (self.show_mia && g_mia);
                                should_show = should_show || (self.show_merged && g_merged);
                                should_show = should_show || g_unknown;

                                if !g_correct && !g_missing && !g_unknown && !g_fixes && !g_mia && !g_merged {
                                    should_show = true;
                                }
                            } else {
                                should_show = true;
                            }

                            if !self.filter_text.is_empty() {
                                if !child
                                    .name
                                    .to_lowercase()
                                    .contains(&self.filter_text.to_lowercase())
                                {
                                    should_show = false;
                                }
                            }

                            if !should_show {
                                continue;
                            }

                            let description = if let Some(ref g) = child.game {
                                g.borrow()
                                    .get_data(rv_core::rv_game::GameData::Description)
                                    .unwrap_or_default()
                            } else {
                                "".to_string()
                            };

                            let mut row_color = match child.rep_status() {
                                RepStatus::Correct | RepStatus::CorrectMIA => egui::Color32::from_rgb(40, 80, 40),
                                RepStatus::Missing | RepStatus::MissingMIA => egui::Color32::from_rgb(80, 40, 40),
                                RepStatus::CanBeFixed | RepStatus::CanBeFixedMIA => egui::Color32::from_rgb(80, 80, 40),
                                RepStatus::MoveToSort | RepStatus::MoveToCorrupt => egui::Color32::from_rgb(40, 80, 80),
                                RepStatus::UnNeeded | RepStatus::Unknown => egui::Color32::from_rgb(60, 60, 60),
                                RepStatus::Delete => egui::Color32::from_rgb(120, 0, 0),
                                _ => egui::Color32::TRANSPARENT,
                            };

                            let is_selected =
                                self.selected_game.as_ref().map_or(false, |s| Rc::ptr_eq(s, &child_rc));
                            if is_selected {
                                row_color = selection_color;
                            }

                            let file_icon = match child.file_type {
                                FileType::Dir => include_asset!("Dir.png"),
                                FileType::Zip => include_asset!("Zip.png"),
                                FileType::SevenZip => include_asset!("SevenZip.png"),
                                _ => include_asset!("default2.png"),
                            };

                            body.row(20.0, |mut row| {
                                row.col(|ui| {
                                    ui.painter().rect_filled(ui.max_rect(), 0.0, row_color);
                                    ui.add(
                                        egui::Image::new(file_icon)
                                            .texture_options(egui::TextureOptions::NEAREST)
                                            .max_width(16.0),
                                    );
                                });
                                row.col(|ui| {
                                    ui.painter().rect_filled(ui.max_rect(), 0.0, row_color);
                                    let label_resp =
                                        ui.add(egui::SelectableLabel::new(is_selected, &child.name));

                                    label_resp.context_menu(|ui| {
                                        if ui.button("Scan Quick (Headers Only)").clicked() {
                                            pending_action =
                                                Some(GridAction::ScanQuick(Rc::clone(&child_rc)));
                                            ui.close_menu();
                                        }
                                        if ui.button("Scan").clicked() {
                                            pending_action =
                                                Some(GridAction::ScanNormal(Rc::clone(&child_rc)));
                                            ui.close_menu();
                                        }
                                        if ui.button("Scan Full (Complete Re-Scan)").clicked() {
                                            pending_action =
                                                Some(GridAction::ScanFull(Rc::clone(&child_rc)));
                                            ui.close_menu();
                                        }
                                        ui.separator();
                                        if ui.button("Open Dir").clicked() {
                                            let full_path = get_full_node_path(Rc::clone(&child_rc));
                                            self.task_logs.push(format!("Opening Dir: {}", full_path));
                                            let _ = std::process::Command::new("explorer")
                                                .arg(&full_path)
                                                .spawn();
                                            ui.close_menu();
                                        }
                                        if ui.button("Open Parent").clicked() {
                                            let full_path = get_full_node_path(Rc::clone(&child_rc));
                                            let parent_path = std::path::Path::new(&full_path)
                                                .parent()
                                                .unwrap_or_else(|| std::path::Path::new(""))
                                                .to_string_lossy()
                                                .to_string();
                                            self.task_logs.push(format!("Opening Parent: {}", parent_path));
                                            let _ = std::process::Command::new("explorer")
                                                .arg(&parent_path)
                                                .spawn();
                                            ui.close_menu();
                                        }
                                        ui.separator();
                                        if ui.button("Fix ROM").clicked() {
                                            pending_action =
                                                Some(GridAction::FixRom(Rc::clone(&child_rc)));
                                            ui.close_menu();
                                        }
                                        if ui.button("Launch emulator").clicked() {
                                            self.task_logs.push(format!("Launch emulator: {}", child.name));
                                            ui.close_menu();
                                        }
                                        if ui.button("Open Web Page").clicked() {
                                            let mut opened = false;
                                            if let Some(game) = &child.game {
                                                let game_id =
                                                    game.borrow().get_data(rv_core::rv_game::GameData::Id);
                                                if let Some(id) = game_id {
                                                    let url = format!("http://redump.org/disc/{}/", id);
                                                    self.task_logs.push(format!("Opening Web Page: {}", url));
                                                    let _ = std::process::Command::new("cmd")
                                                        .args(["/C", "start", &url])
                                                        .spawn();
                                                    opened = true;
                                                }
                                            }
                                            if !opened {
                                                self.task_logs.push(
                                                    "No Web Page mapping available for this game.".to_string(),
                                                );
                                            }
                                            ui.close_menu();
                                        }
                                        ui.separator();
                                        if ui.button("Copy Info").clicked() {
                                            let info = format!("Name: {}\nDesc: {}", child.name, description);
                                            ui.output_mut(|o| o.copied_text = info);
                                            self.task_logs.push("Copied Game Info".to_string());
                                            ui.close_menu();
                                        }
                                    });

                                    if label_resp.double_clicked() {
                                        if child.game.is_none() && child.file_type == FileType::Dir {
                                            pending_action =
                                                Some(GridAction::NavigateDown(Rc::clone(&child_rc)));
                                        } else {
                                            self.task_logs.push(format!("Launch emulator: {}", child.name));
                                        }
                                    } else if label_resp.clicked() {
                                        self.selected_game = Some(Rc::clone(&child_rc));
                                    }
                                });
                                row.col(|ui| {
                                    ui.painter().rect_filled(ui.max_rect(), 0.0, row_color);
                                    ui.label(description);
                                });
                                row.col(|ui| {
                                    ui.painter().rect_filled(ui.max_rect(), 0.0, row_color);
                                    ui.label("");
                                });
                                row.col(|ui| {
                                    ui.painter().rect_filled(ui.max_rect(), 0.0, row_color);
                                    ui.horizontal(|ui| {
                                        let mut correct = 0;
                                        let mut missing = 0;
                                        let mut fixes = 0;
                                        let mut unknown = 0;

                                        for rom in &child.children {
                                            match rom.borrow().rep_status() {
                                                RepStatus::Correct | RepStatus::CorrectMIA => correct += 1,
                                                RepStatus::Missing
                                                | RepStatus::MissingMIA
                                                | RepStatus::DirCorrupt
                                                | RepStatus::Corrupt
                                                | RepStatus::Incomplete => missing += 1,
                                                RepStatus::CanBeFixed
                                                | RepStatus::CanBeFixedMIA
                                                | RepStatus::CorruptCanBeFixed
                                                | RepStatus::MoveToSort
                                                | RepStatus::Delete
                                                | RepStatus::NeededForFix
                                                | RepStatus::Rename
                                                | RepStatus::MoveToCorrupt => fixes += 1,
                                                RepStatus::Unknown | RepStatus::UnNeeded => unknown += 1,
                                                _ => {}
                                            }
                                        }

                                        if correct > 0 {
                                            ui.add(
                                                egui::Image::new(include_asset!("G_Correct.png"))
                                                    .texture_options(egui::TextureOptions::NEAREST)
                                                    .max_width(16.0),
                                            );
                                            ui.label(correct.to_string());
                                        }
                                        if missing > 0 {
                                            ui.add(
                                                egui::Image::new(include_asset!("G_Missing.png"))
                                                    .texture_options(egui::TextureOptions::NEAREST)
                                                    .max_width(16.0),
                                            );
                                            ui.label(missing.to_string());
                                        }
                                        if fixes > 0 {
                                            ui.add(
                                                egui::Image::new(include_asset!("G_CanBeFixed.png"))
                                                    .texture_options(egui::TextureOptions::NEAREST)
                                                    .max_width(16.0),
                                            );
                                            ui.label(fixes.to_string());
                                        }
                                        if unknown > 0 {
                                            ui.add(
                                                egui::Image::new(include_asset!("G_Unknown.png"))
                                                    .texture_options(egui::TextureOptions::NEAREST)
                                                    .max_width(16.0),
                                            );
                                            ui.label(unknown.to_string());
                                        }
                                    });
                                });
                            });
                        }
                    }
                });
        });

        if let Some(action) = pending_action {
            match action {
                GridAction::ScanQuick(target_rc) => {
                    let name = target_rc.borrow().name.clone();
                    let np = get_full_node_path(Rc::clone(&target_rc));
                    self.launch_task("Scan ROMs (Quick)", move |tx| {
                        let _ = tx.send(format!("Scanning {} (Headers Only)...", name));
                        let files = Scanner::scan_directory(&np);
                        let mut root_scan = rv_core::scanned_file::ScannedFile::new(FileType::Dir);
                        root_scan.children = files;
                        let _ = tx.send("Integrating files into DB...".to_string());
                        FileScanning::scan_dir(target_rc, &mut root_scan);
                    });
                }
                GridAction::ScanNormal(target_rc) => {
                    let name = target_rc.borrow().name.clone();
                    let np = get_full_node_path(Rc::clone(&target_rc));
                    self.launch_task("Scan ROMs", move |tx| {
                        let _ = tx.send(format!("Scanning {}...", name));
                        let files = Scanner::scan_directory(&np);
                        let mut root_scan = rv_core::scanned_file::ScannedFile::new(FileType::Dir);
                        root_scan.children = files;
                        let _ = tx.send("Integrating files into DB...".to_string());
                        FileScanning::scan_dir(target_rc, &mut root_scan);
                    });
                }
                GridAction::ScanFull(target_rc) => {
                    let name = target_rc.borrow().name.clone();
                    let np = get_full_node_path(Rc::clone(&target_rc));
                    self.launch_task("Scan ROMs (Full)", move |tx| {
                        let _ = tx.send(format!("Scanning {} (Full Re-Scan)...", name));
                        let files = Scanner::scan_directory(&np);
                        let mut root_scan = rv_core::scanned_file::ScannedFile::new(FileType::Dir);
                        root_scan.children = files;
                        let _ = tx.send("Integrating files into DB...".to_string());
                        FileScanning::scan_dir(target_rc, &mut root_scan);
                    });
                }
                GridAction::NavigateUp => {
                    let mut new_selected = None;
                    if let Some(selected) = &self.selected_node {
                        if let Some(parent) = &selected.borrow().parent {
                            if let Some(parent_rc) = parent.upgrade() {
                                new_selected = Some(parent_rc);
                            }
                        }
                    }
                    if let Some(ns) = new_selected {
                        self.selected_node = Some(ns);
                    }
                }
                GridAction::NavigateDown(target_rc) => {
                    self.selected_node = Some(target_rc);
                }
                GridAction::FixRom(target_rc) => {
                    let name = target_rc.borrow().name.clone();
                    self.task_logs.push(format!("Fix ROM: {}", name));
                    self.launch_task("Fix Individual ROM", move |tx| {
                        let _ = tx.send(format!("Attempting to fix {}", name));
                    });
                }
            }
        }

        self.sort_col = new_sort_col;
        self.sort_desc = new_sort_desc;
    }

    pub fn draw_rom_grid(&mut self, ui: &mut egui::Ui) {
        let mut new_sort_col_rom = self.sort_col.clone();
        let mut new_sort_desc_rom = self.sort_desc;

        egui::ScrollArea::horizontal().show(ui, |ui| {
            egui_extras::TableBuilder::new(ui)
                .striped(true)
                .resizable(true)
                .vscroll(true)
                .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
                .column(egui_extras::Column::initial(40.0).at_least(40.0))
                .column(egui_extras::Column::initial(350.0).at_least(40.0))
                .column(egui_extras::Column::initial(100.0).at_least(40.0))
                .column(egui_extras::Column::initial(100.0).at_least(40.0))
                .column(egui_extras::Column::initial(150.0).at_least(40.0))
                .column(egui_extras::Column::initial(200.0).at_least(40.0))
                .column(egui_extras::Column::initial(200.0).at_least(40.0))
                .column(egui_extras::Column::initial(100.0).at_least(40.0))
                .column(egui_extras::Column::initial(150.0).at_least(40.0))
                .column(egui_extras::Column::initial(200.0).at_least(40.0))
                .column(egui_extras::Column::initial(200.0).at_least(40.0))
                .column(egui_extras::Column::initial(100.0).at_least(40.0))
                .column(egui_extras::Column::initial(150.0).at_least(40.0))
                .column(egui_extras::Column::initial(100.0).at_least(40.0))
                .column(egui_extras::Column::remainder())
                .header(20.0, |mut header| {
                    let mut make_header = |ui: &mut egui::Ui, title: &str| {
                        if ui
                            .selectable_label(new_sort_col_rom.as_deref() == Some(title), title)
                            .clicked()
                        {
                            if new_sort_col_rom.as_deref() == Some(title) {
                                new_sort_desc_rom = !new_sort_desc_rom;
                            } else {
                                new_sort_col_rom = Some(title.to_string());
                                new_sort_desc_rom = false;
                            }
                        }
                    };
                    header.col(|ui| {
                        ui.strong("Got");
                    });
                    header.col(|ui| make_header(ui, "ROM (File)"));
                    header.col(|ui| make_header(ui, "Merge"));
                    header.col(|ui| make_header(ui, "Size"));
                    header.col(|ui| make_header(ui, "CRC32"));
                    header.col(|ui| make_header(ui, "SHA1"));
                    header.col(|ui| make_header(ui, "MD5"));
                    header.col(|ui| make_header(ui, "AltSize"));
                    header.col(|ui| make_header(ui, "AltCRC32"));
                    header.col(|ui| make_header(ui, "AltSHA1"));
                    header.col(|ui| make_header(ui, "AltMD5"));
                    header.col(|ui| make_header(ui, "Status"));
                    header.col(|ui| make_header(ui, "FileModDate"));
                    header.col(|ui| make_header(ui, "ZipIndex"));
                    header.col(|ui| make_header(ui, "InstanceCount"));
                })
                .body(|mut body| {
                    if let Some(selected_game) = &self.selected_game {
                        let game = selected_game.borrow();

                        let mut sorted_roms: Vec<Rc<RefCell<RvFile>>> = game
                            .children
                            .iter()
                            .filter(|c| c.borrow().is_file())
                            .cloned()
                            .collect();

                        if let Some(col) = &self.sort_col {
                            let desc = self.sort_desc;
                            sorted_roms.sort_by(|a, b| {
                                let a = a.borrow();
                                let b = b.borrow();
                                let cmp = match col.as_str() {
                                    "ROM (File)" => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
                                    "Size" => a.size.cmp(&b.size),
                                    "Merge" => a.merge.cmp(&b.merge),
                                    "CRC32" => a.crc.cmp(&b.crc),
                                    "SHA1" => a.sha1.cmp(&b.sha1),
                                    "MD5" => a.md5.cmp(&b.md5),
                                    "AltSize" => a.alt_size.cmp(&b.alt_size),
                                    "AltCRC32" => a.alt_crc.cmp(&b.alt_crc),
                                    "AltSHA1" => a.alt_sha1.cmp(&b.alt_sha1),
                                    "AltMD5" => a.alt_md5.cmp(&b.alt_md5),
                                    "Status" => a.status.cmp(&b.status),
                                    "FileModDate" => a.file_mod_time_stamp.cmp(&b.file_mod_time_stamp),
                                    _ => a.name.cmp(&b.name),
                                };
                                if desc { cmp.reverse() } else { cmp }
                            });
                        }

                        for rom_rc in sorted_roms {
                            let rom = rom_rc.borrow();
                            let row_color = match rom.rep_status() {
                                RepStatus::Correct | RepStatus::CorrectMIA => egui::Color32::from_rgb(40, 80, 40),
                                RepStatus::Missing | RepStatus::MissingMIA => egui::Color32::from_rgb(80, 40, 40),
                                RepStatus::CanBeFixed | RepStatus::CanBeFixedMIA | RepStatus::CorruptCanBeFixed => {
                                    egui::Color32::from_rgb(80, 80, 40)
                                }
                                RepStatus::MoveToSort | RepStatus::MoveToCorrupt => egui::Color32::from_rgb(40, 80, 80),
                                RepStatus::UnNeeded | RepStatus::Unknown => egui::Color32::from_rgb(60, 60, 60),
                                RepStatus::Delete => egui::Color32::from_rgb(120, 0, 0),
                                _ => egui::Color32::TRANSPARENT,
                            };

                            let status_icon = match rom.rep_status() {
                                RepStatus::Correct => include_asset!("G_Correct.png"),
                                RepStatus::CorrectMIA => include_asset!("G_CorrectMIA.png"),
                                RepStatus::Missing => include_asset!("G_Missing.png"),
                                RepStatus::MissingMIA => include_asset!("G_MissingMIA.png"),
                                RepStatus::CanBeFixed => include_asset!("G_CanBeFixed.png"),
                                RepStatus::CanBeFixedMIA => include_asset!("G_CanBeFixedMIA.png"),
                                RepStatus::CorruptCanBeFixed => include_asset!("G_CorruptCanBeFixed.png"),
                                RepStatus::MoveToSort => include_asset!("G_MoveToSort.png"),
                                RepStatus::MoveToCorrupt => include_asset!("G_MoveToCorrupt.png"),
                                RepStatus::Delete => include_asset!("G_Delete.png"),
                                RepStatus::UnNeeded => include_asset!("G_UnNeeded.png"),
                                RepStatus::Unknown => include_asset!("G_Unknown.png"),
                                _ => include_asset!("G_Unknown.png"),
                            };

                            body.row(20.0, |mut row| {
                                row.col(|ui| {
                                    ui.painter().rect_filled(ui.max_rect(), 0.0, row_color);
                                    ui.add(
                                        egui::Image::new(status_icon)
                                            .texture_options(egui::TextureOptions::NEAREST)
                                            .max_width(16.0),
                                    );
                                });
                                row.col(|ui| {
                                    ui.painter().rect_filled(ui.max_rect(), 0.0, row_color);
                                    let label_resp = ui.add(egui::SelectableLabel::new(false, &rom.name));
                                    label_resp.context_menu(|ui| {
                                        if ui.button("Copy ROM Name").clicked() {
                                            ui.output_mut(|o| o.copied_text = rom.name.clone());
                                            self.task_logs.push(format!("Copied: {}", rom.name));
                                            ui.close_menu();
                                        }
                                    });
                                });
                                row.col(|ui| {
                                    ui.painter().rect_filled(ui.max_rect(), 0.0, row_color);
                                    ui.label(&rom.merge);
                                });
                                row.col(|ui| {
                                    ui.painter().rect_filled(ui.max_rect(), 0.0, row_color);
                                    if let Some(s) = rom.size {
                                        ui.label(s.to_string());
                                    } else {
                                        ui.label("-");
                                    }
                                });
                                row.col(|ui| {
                                    ui.painter().rect_filled(ui.max_rect(), 0.0, row_color);
                                    ui.label(rom.crc.as_ref().map(|b| hex::encode(b)).unwrap_or_else(|| "-".to_string()));
                                });
                                row.col(|ui| {
                                    ui.painter().rect_filled(ui.max_rect(), 0.0, row_color);
                                    ui.label(rom.sha1.as_ref().map(|b| hex::encode(b)).unwrap_or_else(|| "-".to_string()));
                                });
                                row.col(|ui| {
                                    ui.painter().rect_filled(ui.max_rect(), 0.0, row_color);
                                    ui.label(rom.md5.as_ref().map(|b| hex::encode(b)).unwrap_or_else(|| "-".to_string()));
                                });
                                row.col(|ui| {
                                    ui.painter().rect_filled(ui.max_rect(), 0.0, row_color);
                                    ui.label(rom.alt_size.map_or("".to_string(), |s| s.to_string()));
                                });
                                row.col(|ui| {
                                    ui.painter().rect_filled(ui.max_rect(), 0.0, row_color);
                                    ui.label(rom.alt_crc.as_ref().map_or("".to_string(), |h| hex::encode(h)));
                                });
                                row.col(|ui| {
                                    ui.painter().rect_filled(ui.max_rect(), 0.0, row_color);
                                    ui.label(rom.alt_sha1.as_ref().map_or("".to_string(), |h| hex::encode(h)));
                                });
                                row.col(|ui| {
                                    ui.painter().rect_filled(ui.max_rect(), 0.0, row_color);
                                    ui.label(rom.alt_md5.as_ref().map_or("".to_string(), |h| hex::encode(h)));
                                });
                                row.col(|ui| {
                                    ui.painter().rect_filled(ui.max_rect(), 0.0, row_color);
                                    ui.label(rom.status.as_deref().unwrap_or(""));
                                });
                                row.col(|ui| {
                                    ui.painter().rect_filled(ui.max_rect(), 0.0, row_color);
                                    ui.label(if rom.file_mod_time_stamp > 0 {
                                        rom.file_mod_time_stamp.to_string()
                                    } else {
                                        "".to_string()
                                    });
                                });
                                row.col(|ui| {
                                    ui.painter().rect_filled(ui.max_rect(), 0.0, row_color);
                                    ui.label("");
                                });
                                row.col(|ui| {
                                    ui.painter().rect_filled(ui.max_rect(), 0.0, row_color);
                                    let instance_count = if matches!(
                                        rom.rep_status(),
                                        RepStatus::Correct
                                            | RepStatus::CorrectMIA
                                            | RepStatus::CanBeFixed
                                            | RepStatus::CanBeFixedMIA
                                    ) {
                                        "1"
                                    } else {
                                        "0"
                                    };
                                    if ui.link(instance_count).clicked() {
                                        self.selected_rom_for_info = Some(Rc::clone(&rom_rc));
                                        self.show_rom_info = true;
                                    }
                                });
                            });
                        }
                    }
                });
        });

        self.sort_col = new_sort_col_rom;
        self.sort_desc = new_sort_desc_rom;
    }
}

