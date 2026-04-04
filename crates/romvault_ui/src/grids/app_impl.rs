impl RomVaultApp {
    pub fn draw_game_grid(&mut self, ui: &mut egui::Ui) {
        let selection_color = ui.style().visuals.selection.bg_fill;

        enum GridAction {
            ScanQuick(Rc<RefCell<RvFile>>),
            ScanNormal(Rc<RefCell<RvFile>>),
            ScanFull(Rc<RefCell<RvFile>>),
            NavigateUp,
            NavigateDown(Rc<RefCell<RvFile>>),
            LaunchEmulator(Rc<RefCell<RvFile>>),
            OpenWebPage(Rc<RefCell<RvFile>>),
        }
        let mut pending_action = None;

        let mut new_sort_col = self.sort_col.clone();
        let mut new_sort_desc = self.sort_desc;

        let filter_lc = self.filter_text.to_lowercase();
        let mut visible_children: Vec<Rc<RefCell<RvFile>>> = Vec::new();
        let mut show_description = false;
        let mut wide_type_column = false;
        if let Some(selected) = &self.selected_node {
            let node = selected.borrow();
            for child_rc in node
                .children
                .iter()
                .filter(|c| !c.borrow().is_file() || c.borrow().game.is_some())
            {
                let child = child_rc.borrow();
                if child.is_file() && child.game.is_none() {
                    continue;
                }

                let mut should_show = false;
                let visibility_flags = if let Some(stats) = &child.cached_stats {
                    Some(grid_visibility_flags_from_stats(stats))
                } else {
                    child.dir_status.map(grid_visibility_flags_from_report_status)
                };
                if let Some(flags) = visibility_flags {
                    let g_correct = flags.correct;
                    let g_missing = flags.missing;
                    let g_fixes = flags.fixes;
                    let g_mia = flags.mia;
                    let g_merged = flags.merged;
                    let g_unknown = flags.unknown;

                    should_show = should_show || (self.show_complete && g_correct && !g_missing && !g_fixes);
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

                if !self.filter_text.is_empty() && !child.name.to_lowercase().contains(&filter_lc) {
                    should_show = false;
                }
                if !should_show {
                    continue;
                }

                if !show_description {
                    if let Some(ref g) = child.game {
                        let desc = g
                            .borrow()
                            .get_data(rv_core::rv_game::GameData::Description)
                            .unwrap_or_default();
                        if !desc.trim().is_empty() && desc != "¤" {
                            show_description = true;
                        }
                    }
                }

                if !wide_type_column {
                    let expected = if child.dat_status() != DatStatus::NotInDat
                        && child.dat_status() != DatStatus::InToSort
                    {
                        Some(game_type_icon_key(child.file_type, child.zip_dat_struct()))
                    } else {
                        None
                    };
                    let have = if child.got_status() != GotStatus::NotGot {
                        Some(game_type_icon_key(child.file_type, child.zip_struct))
                    } else {
                        None
                    };
                    if let (Some(e), Some(h)) = (expected, have) {
                        if e != h {
                            wide_type_column = true;
                        }
                    }
                }

                visible_children.push(Rc::clone(child_rc));
            }
        }

        if let Some(col) = &self.sort_col {
            let desc = self.sort_desc;
            visible_children.sort_by(|a, b| {
                let a = a.borrow();
                let b = b.borrow();
                let cmp = match col.as_str() {
                    "Game (Directory / Zip)" => trrntzip_name_cmp(&a.name, &b.name),
                    "Description" => {
                        let da = game_display_description(&a);
                        let db = game_display_description(&b);
                        da.cmp(&db).then(trrntzip_name_cmp(&a.name, &b.name))
                    }
                    "Type" => a
                        .file_type
                        .cmp(&b.file_type)
                        .then(b.zip_struct.cmp(&a.zip_struct))
                        .then(a.rep_status().cmp(&b.rep_status()))
                        .then(trrntzip_name_cmp(&a.name, &b.name)),
                    "Modified" => a
                        .file_mod_time_stamp
                        .cmp(&b.file_mod_time_stamp)
                        .then(trrntzip_name_cmp(&a.name, &b.name)),
                    _ => trrntzip_name_cmp(&a.name, &b.name),
                };
                if desc { cmp.reverse() } else { cmp }
            });
        }

        let dark_mode = ui.visuals().dark_mode;
        let grid_fill = if dark_mode {
            egui::Color32::from_rgb(20, 20, 22)
        } else {
            ui.visuals().panel_fill
        };
        let grid_stroke = if dark_mode {
            egui::Stroke::new(1.0, egui::Color32::from_rgb(40, 40, 45))
        } else {
            egui::Stroke::new(1.0, egui::Color32::from_rgb(220, 220, 226))
        };

        egui::Frame::none()
            .fill(grid_fill)
            .stroke(grid_stroke)
            .rounding(6.0)
            .inner_margin(4.0)
            .show(ui, |ui| {
                egui::ScrollArea::horizontal().show(ui, |ui| {
                    let type_width = if wide_type_column { 90.0 } else { 44.0 };
                    let mut table = egui_extras::TableBuilder::new(ui)
                        .striped(true)
                        .resizable(true)
                        .vscroll(true)
                        .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
                        .column(egui_extras::Column::initial(type_width).at_least(type_width))
                        .column(egui_extras::Column::initial(350.0).at_least(40.0));

                    if show_description {
                        table = table.column(egui_extras::Column::initial(350.0).at_least(40.0));
                    }

                    table = table
                        .column(egui_extras::Column::initial(150.0).at_least(40.0))
                        .column(egui_extras::Column::remainder());

                    table
                        .header(20.0, |mut header| {
                            header.col(|ui| {
                                sort_header_cell(ui, "Type", &mut new_sort_col, &mut new_sort_desc)
                            });
                            header.col(|ui| {
                                sort_header_cell(
                                    ui,
                                    "Game (Directory / Zip)",
                                    &mut new_sort_col,
                                    &mut new_sort_desc,
                                );
                            });
                            if show_description {
                                header.col(|ui| {
                                    sort_header_cell(
                                        ui,
                                        "Description",
                                        &mut new_sort_col,
                                        &mut new_sort_desc,
                                    )
                                });
                            }
                            header.col(|ui| {
                                sort_header_cell(ui, "Modified", &mut new_sort_col, &mut new_sort_desc)
                            });
                            header.col(|ui| {
                                sort_header_cell(ui, "ROM Status", &mut new_sort_col, &mut new_sort_desc)
                            });
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
                                            if label_resp.hovered()
                                                && ui.input(|i| {
                                                    i.pointer.button_double_clicked(
                                                        egui::PointerButton::Secondary,
                                                    )
                                                })
                                            {
                                                pending_action = Some(GridAction::NavigateUp);
                                            }
                                        });
                                        if show_description {
                                            row.col(|ui| {
                                                ui.label("");
                                            });
                                        }
                                        row.col(|ui| {
                                            ui.label("");
                                        });
                                        row.col(|ui| {
                                            ui.label("");
                                        });
                                    });
                                }

                                let row_count = visible_children.len();
                                body.rows(20.0, row_count, |mut row| {
                                    let child_rc = &visible_children[row.index()];
                                    let child = child_rc.borrow();

                                    let mut row_color = game_row_color_for_mode(child.rep_status(), dark_mode);

                                    let is_selected = self
                                        .selected_game
                                        .as_ref()
                                        .is_some_and(|s| Rc::ptr_eq(s, child_rc));
                                    if is_selected {
                                        row_color = selection_color;
                                    }

                                    row.col(|ui| {
                                        ui.painter().rect_filled(ui.max_rect(), 0.0, row_color);
                                        let expected_key = if child.dat_status() != DatStatus::NotInDat
                                            && child.dat_status() != DatStatus::InToSort
                                        {
                                            Some(game_type_icon_key(
                                                child.file_type,
                                                child.zip_dat_struct(),
                                            ))
                                        } else {
                                            None
                                        };
                                        let have_key = if child.got_status() != GotStatus::NotGot {
                                            Some(game_type_icon_key(child.file_type, child.zip_struct))
                                        } else {
                                            None
                                        };
                                        let mismatch = expected_key
                                            .is_some_and(|e| have_key.is_some_and(|h| e != h));

                                        let expected_img = expected_key
                                            .map(|(ft, zs)| game_grid_icon_source(game_type_icon_missing(ft, zs)));
                                        let have_img = have_key.map(|(ft, zs)| {
                                            game_grid_icon_source(
                                                if child.got_status() == GotStatus::Corrupt {
                                                    game_type_icon_corrupt(ft, zs)
                                                } else {
                                                    game_type_icon_normal(ft, zs)
                                                },
                                            )
                                        });
                                        let convert_img = game_grid_icon_source(if child.zip_dat_struct_fix() {
                                            "ZipConvert.png"
                                        } else {
                                            "ZipConvert1.png"
                                        });

                                        if mismatch {
                                            ui.horizontal(|ui| {
                                                if let Some(h) = have_img {
                                                    ui.add(
                                                        egui::Image::new(h)
                                                            .texture_options(egui::TextureOptions::NEAREST)
                                                            .max_width(16.0),
                                                    );
                                                }
                                                ui.add(
                                                    egui::Image::new(convert_img)
                                                        .texture_options(egui::TextureOptions::NEAREST)
                                                        .max_width(16.0),
                                                );
                                                if let Some(e) = expected_img {
                                                    ui.add(
                                                        egui::Image::new(e)
                                                            .texture_options(egui::TextureOptions::NEAREST)
                                                            .max_width(16.0),
                                                    );
                                                }
                                            });
                                        } else if let Some(h) = have_img {
                                            ui.add(
                                                egui::Image::new(h)
                                                    .texture_options(egui::TextureOptions::NEAREST)
                                                    .max_width(16.0),
                                            );
                                        } else if let Some(e) = expected_img {
                                            ui.add(
                                                egui::Image::new(e)
                                                    .texture_options(egui::TextureOptions::NEAREST)
                                                    .max_width(16.0),
                                            );
                                        } else {
                                            ui.add(
                                                egui::Image::new(game_grid_icon_source("default2.png"))
                                                    .texture_options(egui::TextureOptions::NEAREST)
                                                    .max_width(16.0),
                                            );
                                        }
                                    });
                                    row.col(|ui| {
                                        ui.painter().rect_filled(ui.max_rect(), 0.0, row_color);
                                        let label_text = if child.file_name.is_empty() {
                                            child.name.clone()
                                        } else {
                                            format!("{} (Found: {})", child.name, child.file_name)
                                        };
                                        let label_resp =
                                            ui.add(egui::SelectableLabel::new(is_selected, label_text));
                                        if ui.input(|i| i.modifiers.shift) {
                                            label_resp.context_menu(|ui| {
                                                let mut has_open_target = false;

                                                if child.file_type == FileType::Dir && !self.sam_running {
                                                    if ui.button("Scan").clicked() {
                                                        pending_action =
                                                            Some(GridAction::ScanNormal(Rc::clone(child_rc)));
                                                        ui.close_menu();
                                                    }
                                                    if ui.button("Scan Quick (Headers Only)").clicked() {
                                                        pending_action =
                                                            Some(GridAction::ScanQuick(Rc::clone(child_rc)));
                                                        ui.close_menu();
                                                    }
                                                    if ui.button("Scan Full (Complete Re-Scan)").clicked() {
                                                        pending_action =
                                                            Some(GridAction::ScanFull(Rc::clone(child_rc)));
                                                        ui.close_menu();
                                                    }
                                                    ui.separator();
                                                }

                                                let full_path = get_full_node_path(Rc::clone(child_rc));
                                                let full_path = rv_core::settings::find_dir_mapping(&full_path)
                                                    .unwrap_or(full_path);
                                                if child.file_type == FileType::Dir {
                                                    if std::path::Path::new(&full_path).is_dir() {
                                                        has_open_target = true;
                                                        if ui.button("Open Dir").clicked() {
                                                            self.task_logs.push(format!(
                                                                "Opening Dir: {}",
                                                                full_path
                                                            ));
                                                            let _ = std::process::Command::new("cmd")
                                                                .args(["/C", "start", "", &full_path])
                                                                .spawn();
                                                            ui.close_menu();
                                                        }
                                                    }
                                                } else if matches!(
                                                    child.file_type,
                                                    FileType::Zip | FileType::SevenZip
                                                ) && std::path::Path::new(&full_path).is_file()
                                                {
                                                    has_open_target = true;
                                                    let label = if child.file_type == FileType::Zip {
                                                        "Open Zip"
                                                    } else {
                                                        "Open 7Zip"
                                                    };
                                                    if ui.button(label).clicked() {
                                                        self.task_logs.push(format!("Opening: {}", full_path));
                                                        let _ = std::process::Command::new("cmd")
                                                            .args(["/C", "start", "", &full_path])
                                                            .spawn();
                                                        ui.close_menu();
                                                    }
                                                }

                                                let parent_path = std::path::Path::new(&full_path)
                                                    .parent()
                                                    .unwrap_or_else(|| std::path::Path::new(""))
                                                    .to_string_lossy()
                                                    .to_string();
                                                if std::path::Path::new(&parent_path).is_dir() {
                                                    has_open_target = true;
                                                    if ui.button("Open Parent").clicked() {
                                                        self.task_logs.push(format!(
                                                            "Opening Parent: {}",
                                                            parent_path
                                                        ));
                                                        let _ = std::process::Command::new("cmd")
                                                            .args(["/C", "start", "", &parent_path])
                                                            .spawn();
                                                        ui.close_menu();
                                                    }
                                                }

                                                if has_open_target {
                                                    if let Some(parent_rc) =
                                                        child_rc.borrow().parent.as_ref().and_then(|p| p.upgrade())
                                                    {
                                                        if emulator_info_for_game_dir(parent_rc).is_some()
                                                            && ui.button("Launch emulator").clicked()
                                                        {
                                                            pending_action = Some(GridAction::LaunchEmulator(
                                                                Rc::clone(child_rc),
                                                            ));
                                                            ui.close_menu();
                                                        }
                                                    }
                                                }

                                                let home_page = child
                                                    .dat
                                                    .as_ref()
                                                    .and_then(|d| d.borrow().get_data(DatData::HomePage))
                                                    .unwrap_or_default();
                                                let has_no_intro = home_page == "No-Intro"
                                                    && child
                                                        .dat
                                                        .as_ref()
                                                        .and_then(|d| d.borrow().get_data(DatData::Id))
                                                        .map(|s| !s.trim().is_empty())
                                                        .unwrap_or(false)
                                                    && child
                                                        .game
                                                        .as_ref()
                                                        .and_then(|g| {
                                                            g.borrow().get_data(rv_core::rv_game::GameData::Id)
                                                        })
                                                        .map(|s| !s.trim().is_empty())
                                                        .unwrap_or(false);
                                                let has_redump = home_page == "redump.org"
                                                    && child
                                                        .game
                                                        .as_ref()
                                                        .and_then(|g| {
                                                            g.borrow().get_data(rv_core::rv_game::GameData::Id)
                                                        })
                                                        .map(|s| !s.trim().is_empty())
                                                        .unwrap_or(false);
                                                if (has_no_intro || has_redump)
                                                    && ui.button("Open Web Page").clicked()
                                                {
                                                    pending_action =
                                                        Some(GridAction::OpenWebPage(Rc::clone(child_rc)));
                                                    ui.close_menu();
                                                }
                                            });
                                        }

                                        if label_resp.double_clicked() {
                                            if child.game.is_none() && child.file_type == FileType::Dir {
                                                pending_action = Some(GridAction::NavigateDown(Rc::clone(child_rc)));
                                            } else {
                                                pending_action =
                                                    Some(GridAction::LaunchEmulator(Rc::clone(child_rc)));
                                            }
                                        } else if label_resp.clicked() {
                                            self.selected_game = Some(Rc::clone(child_rc));
                                        }

                                        if label_resp.hovered()
                                            && ui.input(|i| {
                                                i.pointer
                                                    .button_double_clicked(egui::PointerButton::Secondary)
                                            })
                                        {
                                            pending_action = Some(GridAction::NavigateUp);
                                        }
                                    });
                                    if show_description {
                                        row.col(|ui| {
                                            ui.painter().rect_filled(ui.max_rect(), 0.0, row_color);
                                            ui.label(game_display_description(&child));
                                        });
                                    }
                                    row.col(|ui| {
                                        ui.painter().rect_filled(ui.max_rect(), 0.0, row_color);
                                        let time_str = compress::compress_utils::zip_date_time_to_string(Some(
                                            child.file_mod_time_stamp,
                                        ));
                                        ui.label(format_cell_with_source_flags(
                                            time_str,
                                            &child,
                                            rv_core::rv_file::FileStatus::DATE_FROM_DAT,
                                            rv_core::rv_file::FileStatus::NONE,
                                        ));
                                    });
                                    row.col(|ui| {
                                        ui.painter().rect_filled(ui.max_rect(), 0.0, row_color);
                                        ui.horizontal(|ui| {
                                            let (correct, missing, fixes, merged, unknown) =
                                                if let Some(stats) = &child.cached_stats {
                                                    (
                                                        stats.count_correct() as usize,
                                                        (stats.roms_missing + stats.roms_missing_mia) as usize,
                                                        stats.roms_fixes as usize,
                                                        (stats.roms_not_collected + stats.roms_unneeded) as usize,
                                                        stats.roms_unknown as usize,
                                                    )
                                                } else {
                                                    let mut correct = 0;
                                                    let mut missing = 0;
                                                    let mut fixes = 0;
                                                    let mut merged = 0;
                                                    let mut unknown = 0;

                                                    for rom in &child.children {
                                                        match game_summary_bucket(rom.borrow().rep_status()) {
                                                            Some(RomStatusBucket::Correct) => correct += 1,
                                                            Some(RomStatusBucket::Missing) => missing += 1,
                                                            Some(RomStatusBucket::Fixes) => fixes += 1,
                                                            Some(RomStatusBucket::Merged) => merged += 1,
                                                            Some(RomStatusBucket::Unknown) => unknown += 1,
                                                            None => {}
                                                        }
                                                    }

                                                    (correct, missing, fixes, merged, unknown)
                                                };

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
                                            if merged > 0 {
                                                ui.add(
                                                    egui::Image::new(include_asset!("G_UnNeeded.png"))
                                                        .texture_options(egui::TextureOptions::NEAREST)
                                                        .max_width(16.0),
                                                );
                                                ui.label(merged.to_string());
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
                        });
                });
            });

        if let Some(action) = pending_action {
            match action {
                GridAction::ScanQuick(target_rc) => {
                    let name = target_rc.borrow().name.clone();
                    let logical = get_full_node_path(Rc::clone(&target_rc));
                    let np = rv_core::settings::find_dir_mapping(&logical).unwrap_or(logical.clone());
                    let rule = rv_core::settings::find_rule(&logical);
                    self.launch_task("Scan ROMs (Quick)", move |tx| {
                        let _ = tx.send(format!("Scanning {} (Headers Only)...", name));
                        let files = Scanner::scan_directory_with_level_and_ignore(
                            &np,
                            rv_core::settings::EScanLevel::Level1,
                            &rule.ignore_files.items,
                        );
                        let mut root_scan = rv_core::scanned_file::ScannedFile::new(FileType::Dir);
                        root_scan.children = files;
                        let _ = tx.send("Integrating files into DB...".to_string());
                        FileScanning::scan_dir_with_level(
                            target_rc,
                            &mut root_scan,
                            rv_core::settings::EScanLevel::Level1,
                        );
                    });
                }
                GridAction::ScanNormal(target_rc) => {
                    let name = target_rc.borrow().name.clone();
                    let logical = get_full_node_path(Rc::clone(&target_rc));
                    let np = rv_core::settings::find_dir_mapping(&logical).unwrap_or(logical.clone());
                    let rule = rv_core::settings::find_rule(&logical);
                    self.launch_task("Scan ROMs", move |tx| {
                        let _ = tx.send(format!("Scanning {}...", name));
                        let files = Scanner::scan_directory_with_level_and_ignore(
                            &np,
                            rv_core::settings::EScanLevel::Level2,
                            &rule.ignore_files.items,
                        );
                        let mut root_scan = rv_core::scanned_file::ScannedFile::new(FileType::Dir);
                        root_scan.children = files;
                        let _ = tx.send("Integrating files into DB...".to_string());
                        FileScanning::scan_dir_with_level(
                            target_rc,
                            &mut root_scan,
                            rv_core::settings::EScanLevel::Level2,
                        );
                    });
                }
                GridAction::ScanFull(target_rc) => {
                    let name = target_rc.borrow().name.clone();
                    let logical = get_full_node_path(Rc::clone(&target_rc));
                    let np = rv_core::settings::find_dir_mapping(&logical).unwrap_or(logical.clone());
                    let rule = rv_core::settings::find_rule(&logical);
                    self.launch_task("Scan ROMs (Full)", move |tx| {
                        let _ = tx.send(format!("Scanning {} (Full Re-Scan)...", name));
                        let files = Scanner::scan_directory_with_level_and_ignore(
                            &np,
                            rv_core::settings::EScanLevel::Level3,
                            &rule.ignore_files.items,
                        );
                        let mut root_scan = rv_core::scanned_file::ScannedFile::new(FileType::Dir);
                        root_scan.children = files;
                        let _ = tx.send("Integrating files into DB...".to_string());
                        FileScanning::scan_dir_with_level(
                            target_rc,
                            &mut root_scan,
                            rv_core::settings::EScanLevel::Level3,
                        );
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
                        self.select_node(ns);
                    }
                }
                GridAction::NavigateDown(target_rc) => {
                    self.select_node(target_rc);
                }
                GridAction::LaunchEmulator(target_rc) => {
                    let game = target_rc.borrow();
                    if launch_emulator_for_game(&game) {
                        self.task_logs.push(format!("Launch emulator: {}", game.name));
                    } else {
                        self.task_logs.push("Launch emulator failed.".to_string());
                    }
                }
                GridAction::OpenWebPage(target_rc) => {
                    let game = target_rc.borrow();
                    if !open_web_page_for_game(&game) {
                        self.task_logs
                            .push("No Web Page mapping available for this game.".to_string());
                    }
                }
            }
        }

        self.sort_col = new_sort_col;
        self.sort_desc = new_sort_desc;
    }

    pub fn draw_rom_grid(&mut self, ui: &mut egui::Ui) {
        let mut new_sort_col_rom = self.sort_col.clone();
        let mut new_sort_desc_rom = self.sort_desc;

        let empty_rows: &[RomGridRow] = &[];
        let (rom_rows, alt_found, show_status, show_file_mod_date, show_zip_index) = if let Some(
            selected_game,
        ) = &self.selected_game
        {
            let game_ptr = Rc::as_ptr(selected_game) as usize;
            let game_child_count = selected_game.borrow().children.len();
            let mut needs_rebuild = match self.rom_grid_cache.as_ref() {
                Some(c) => {
                    c.game_ptr != game_ptr
                        || c.game_child_count != game_child_count
                        || c.show_merged != self.show_merged
                }
                None => true,
            };
            if let Some(c) = self.rom_grid_cache.as_ref() {
                if self.db_cache_dirty && !c.built_while_db_dirty {
                    needs_rebuild = true;
                }
            }

            if needs_rebuild {
                let mut rows: Vec<RomGridRow> = Vec::new();
                let mut alt_found = false;
                let mut show_status = false;
                let mut show_file_mod_date = false;
                let mut show_zip_index = false;
                collect_rom_grid_rows(
                    selected_game,
                    "",
                    self.show_merged,
                    &mut rows,
                    &mut alt_found,
                    &mut show_status,
                    &mut show_file_mod_date,
                    &mut show_zip_index,
                );
                if show_zip_index {
                    compute_zip_indices(&mut rows);
                }
                self.rom_grid_cache = Some(RomGridCache {
                    game_ptr,
                    game_child_count,
                    show_merged: self.show_merged,
                    built_while_db_dirty: self.db_cache_dirty,
                    alt_found,
                    show_status,
                    show_file_mod_date,
                    show_zip_index,
                    rows,
                    last_sort_col: None,
                    last_sort_desc: false,
                });
            }

            let cache = self.rom_grid_cache.as_mut().unwrap();
            if cache.last_sort_col != self.sort_col || cache.last_sort_desc != self.sort_desc {
                if let Some(col) = &self.sort_col {
                    let desc = self.sort_desc;
                    cache.rows.sort_by(|a, b| {
                        let a_ref = a.rom_rc.borrow();
                        let b_ref = b.rom_rc.borrow();
                        let cmp = match col.as_str() {
                            "Got" => a_ref
                                .got_status()
                                .cmp(&b_ref.got_status())
                                .then(a_ref.rep_status().cmp(&b_ref.rep_status()))
                                .then(a.ui_name.cmp(&b.ui_name)),
                            "ROM (File)" => a.ui_name.cmp(&b.ui_name),
                            "Merge" => a_ref.merge.cmp(&b_ref.merge),
                            "Size" => a_ref.size.cmp(&b_ref.size),
                            "CRC32" => a_ref.crc.cmp(&b_ref.crc),
                            "SHA1" => a_ref.sha1.cmp(&b_ref.sha1),
                            "MD5" => a_ref.md5.cmp(&b_ref.md5),
                            "AltSize" => a_ref.alt_size.cmp(&b_ref.alt_size),
                            "AltCRC32" => a_ref.alt_crc.cmp(&b_ref.alt_crc),
                            "AltSHA1" => a_ref.alt_sha1.cmp(&b_ref.alt_sha1),
                            "AltMD5" => a_ref.alt_md5.cmp(&b_ref.alt_md5),
                            "Status" => a_ref.status.cmp(&b_ref.status),
                            "FileModDate" => {
                                a_ref.file_mod_time_stamp.cmp(&b_ref.file_mod_time_stamp)
                            }
                            "ZipIndex" => a_ref.local_header_offset.cmp(&b_ref.local_header_offset),
                            "InstanceCount" => std::cmp::Ordering::Equal,
                            _ => a.ui_name.cmp(&b.ui_name),
                        };
                        let cmp = if cmp == std::cmp::Ordering::Equal && col.as_str() != "ROM (File)" {
                            a.ui_name.cmp(&b.ui_name)
                        } else {
                            cmp
                        };
                        if desc { cmp.reverse() } else { cmp }
                    });
                }
                cache.last_sort_col = self.sort_col.clone();
                cache.last_sort_desc = self.sort_desc;
            }

            (
                &cache.rows[..],
                cache.alt_found,
                cache.show_status,
                cache.show_file_mod_date,
                cache.show_zip_index,
            )
        } else {
            self.rom_grid_cache = None;
            (empty_rows, false, false, false, false)
        };

        let dark_mode = ui.visuals().dark_mode;
        let grid_fill = if dark_mode {
            egui::Color32::from_rgb(20, 20, 22)
        } else {
            ui.visuals().panel_fill
        };
        let grid_stroke = if dark_mode {
            egui::Stroke::new(1.0, egui::Color32::from_rgb(40, 40, 45))
        } else {
            egui::Stroke::new(1.0, egui::Color32::from_rgb(220, 220, 226))
        };

        egui::Frame::none()
            .fill(grid_fill)
            .stroke(grid_stroke)
            .rounding(6.0)
            .inner_margin(4.0)
            .show(ui, |ui| {
                egui::ScrollArea::horizontal().show(ui, |ui| {
                    let mut table = egui_extras::TableBuilder::new(ui)
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
                        .column(egui_extras::Column::initial(200.0).at_least(40.0));

                    if alt_found {
                        table = table
                            .column(egui_extras::Column::initial(100.0).at_least(40.0))
                            .column(egui_extras::Column::initial(150.0).at_least(40.0))
                            .column(egui_extras::Column::initial(200.0).at_least(40.0))
                            .column(egui_extras::Column::initial(200.0).at_least(40.0));
                    }

                    if show_status {
                        table = table.column(egui_extras::Column::initial(100.0).at_least(40.0));
                    }

                    if show_file_mod_date {
                        table = table.column(egui_extras::Column::initial(150.0).at_least(40.0));
                    }

                    if show_zip_index {
                        table = table.column(egui_extras::Column::initial(100.0).at_least(40.0));
                    }

                    table
                        .column(egui_extras::Column::remainder())
                        .header(20.0, |mut header| {
                            header.col(|ui| {
                                sort_header_cell(
                                    ui,
                                    "Got",
                                    &mut new_sort_col_rom,
                                    &mut new_sort_desc_rom,
                                );
                            });
                            header.col(|ui| {
                                sort_header_cell(
                                    ui,
                                    "ROM (File)",
                                    &mut new_sort_col_rom,
                                    &mut new_sort_desc_rom,
                                );
                            });
                            header.col(|ui| {
                                sort_header_cell(
                                    ui,
                                    "Merge",
                                    &mut new_sort_col_rom,
                                    &mut new_sort_desc_rom,
                                )
                            });
                            header.col(|ui| {
                                sort_header_cell(
                                    ui,
                                    "Size",
                                    &mut new_sort_col_rom,
                                    &mut new_sort_desc_rom,
                                )
                            });
                            header.col(|ui| {
                                sort_header_cell(
                                    ui,
                                    "CRC32",
                                    &mut new_sort_col_rom,
                                    &mut new_sort_desc_rom,
                                )
                            });
                            header.col(|ui| {
                                sort_header_cell(
                                    ui,
                                    "SHA1",
                                    &mut new_sort_col_rom,
                                    &mut new_sort_desc_rom,
                                )
                            });
                            header.col(|ui| {
                                sort_header_cell(
                                    ui,
                                    "MD5",
                                    &mut new_sort_col_rom,
                                    &mut new_sort_desc_rom,
                                )
                            });
                            if alt_found {
                                header.col(|ui| {
                                    sort_header_cell(
                                        ui,
                                        "AltSize",
                                        &mut new_sort_col_rom,
                                        &mut new_sort_desc_rom,
                                    )
                                });
                                header.col(|ui| {
                                    sort_header_cell(
                                        ui,
                                        "AltCRC32",
                                        &mut new_sort_col_rom,
                                        &mut new_sort_desc_rom,
                                    )
                                });
                                header.col(|ui| {
                                    sort_header_cell(
                                        ui,
                                        "AltSHA1",
                                        &mut new_sort_col_rom,
                                        &mut new_sort_desc_rom,
                                    )
                                });
                                header.col(|ui| {
                                    sort_header_cell(
                                        ui,
                                        "AltMD5",
                                        &mut new_sort_col_rom,
                                        &mut new_sort_desc_rom,
                                    )
                                });
                            }
                            if show_status {
                                header.col(|ui| {
                                    sort_header_cell(
                                        ui,
                                        "Status",
                                        &mut new_sort_col_rom,
                                        &mut new_sort_desc_rom,
                                    )
                                });
                            }
                            if show_file_mod_date {
                                header.col(|ui| {
                                    sort_header_cell(
                                        ui,
                                        "FileModDate",
                                        &mut new_sort_col_rom,
                                        &mut new_sort_desc_rom,
                                    )
                                });
                            }
                            if show_zip_index {
                                header.col(|ui| {
                                    sort_header_cell(
                                        ui,
                                        "ZipIndex",
                                        &mut new_sort_col_rom,
                                        &mut new_sort_desc_rom,
                                    )
                                });
                            }
                            header.col(|ui| {
                                sort_header_cell(
                                    ui,
                                    "InstanceCount",
                                    &mut new_sort_col_rom,
                                    &mut new_sort_desc_rom,
                                )
                            });
                        })
                        .body(|body| {
                            let row_count = rom_rows.len();
                            body.rows(20.0, row_count, |mut row| {
                                let row_data = &rom_rows[row.index()];
                                let rom_rc = Rc::clone(&row_data.rom_rc);
                                let rom = rom_rc.borrow();
                                let row_color = rom_row_color_for_mode(rom.rep_status(), dark_mode);

                                let status_icon = match rom_status_icon_idx(rom.rep_status()) {
                                    0 => include_asset!("G_Correct.png"),
                                    1 => include_asset!("G_CorrectMIA.png"),
                                    2 => include_asset!("G_Missing.png"),
                                    3 => include_asset!("G_DirCorrupt.png"),
                                    4 => include_asset!("G_MissingMIA.png"),
                                    5 => include_asset!("G_CanBeFixed.png"),
                                    6 => include_asset!("G_CanBeFixedMIA.png"),
                                    7 => include_asset!("G_CorruptCanBeFixed.png"),
                                    8 => include_asset!("G_MoveToSort.png"),
                                    9 => include_asset!("G_MoveToCorrupt.png"),
                                    10 => include_asset!("G_InToSort.png"),
                                    11 => include_asset!("G_NeededForFix.png"),
                                    12 => include_asset!("G_Rename.png"),
                                    13 => include_asset!("G_Delete.png"),
                                    14 => include_asset!("G_NotCollected.png"),
                                    15 => include_asset!("G_UnNeeded.png"),
                                    17 => include_asset!("G_Corrupt.png"),
                                    18 => include_asset!("G_Incomplete.png"),
                                    19 => include_asset!("G_UnScanned.png"),
                                    20 => include_asset!("G_Ignore.png"),
                                    _ => include_asset!("G_Unknown.png"),
                                };
                                row.col(|ui| {
                                    ui.painter().rect_filled(ui.max_rect(), 0.0, row_color);
                                    let resp = ui.add(
                                        egui::Image::new(status_icon)
                                            .texture_options(egui::TextureOptions::NEAREST)
                                            .max_width(16.0),
                                    );
                                    if resp.secondary_clicked() {
                                        if let Some(info) = rom_clipboard_text(&rom, RomGridCopyColumn::Got) {
                                            ui.output_mut(|o| o.copied_text = info);
                                            self.task_logs.push("Copied ROM info".to_string());
                                        }
                                    }
                                });
                                row.col(|ui| {
                                    ui.painter().rect_filled(ui.max_rect(), 0.0, row_color);
                                    let label_resp =
                                        ui.add(egui::SelectableLabel::new(false, &row_data.display_text));
                                    if label_resp.secondary_clicked() {
                                        if let Some(text) = rom_clipboard_text(&rom, RomGridCopyColumn::Rom) {
                                            ui.output_mut(|o| o.copied_text = text.clone());
                                            self.task_logs.push(format!("Copied: {}", text));
                                        }
                                    }
                                    label_resp.context_menu(|ui| {
                                        if ui.button("Copy ROM Name").clicked() {
                                            ui.output_mut(|o| o.copied_text = row_data.ui_name.clone());
                                            self.task_logs.push(format!("Copied: {}", row_data.ui_name));
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
                                    let text = format_cell_with_source_flags(
                                        rom.size.map(|s| s.to_string()).unwrap_or_default(),
                                        &rom,
                                        rv_core::rv_file::FileStatus::SIZE_FROM_DAT,
                                        rv_core::rv_file::FileStatus::SIZE_FROM_HEADER,
                                    );
                                    let resp = ui.add(egui::Label::new(text).sense(egui::Sense::click()));
                                    if resp.secondary_clicked() {
                                        if let Some(copy) = rom_clipboard_text(&rom, RomGridCopyColumn::Size) {
                                            ui.output_mut(|o| o.copied_text = copy.clone());
                                            self.task_logs.push(format!("Copied: {}", copy));
                                        }
                                    }
                                });
                                row.col(|ui| {
                                    ui.painter().rect_filled(ui.max_rect(), 0.0, row_color);
                                    let text = format_cell_with_source_flags(
                                        rom.crc.as_ref().map(hex::encode).unwrap_or_default(),
                                        &rom,
                                        rv_core::rv_file::FileStatus::CRC_FROM_DAT,
                                        rv_core::rv_file::FileStatus::CRC_FROM_HEADER,
                                    );
                                    let resp = ui.add(egui::Label::new(text).sense(egui::Sense::click()));
                                    if resp.secondary_clicked() {
                                        if let Some(copy) = rom_clipboard_text(&rom, RomGridCopyColumn::Crc32) {
                                            ui.output_mut(|o| o.copied_text = copy.clone());
                                            self.task_logs.push(format!("Copied: {}", copy));
                                        }
                                    }
                                });
                                row.col(|ui| {
                                    ui.painter().rect_filled(ui.max_rect(), 0.0, row_color);
                                    let text = format_cell_with_source_flags(
                                        rom.sha1.as_ref().map(hex::encode).unwrap_or_default(),
                                        &rom,
                                        rv_core::rv_file::FileStatus::SHA1_FROM_DAT,
                                        rv_core::rv_file::FileStatus::SHA1_FROM_HEADER,
                                    );
                                    let resp = ui.add(egui::Label::new(text).sense(egui::Sense::click()));
                                    if resp.secondary_clicked() {
                                        if let Some(copy) = rom_clipboard_text(&rom, RomGridCopyColumn::Sha1) {
                                            ui.output_mut(|o| o.copied_text = copy.clone());
                                            self.task_logs.push(format!("Copied: {}", copy));
                                        }
                                    }
                                });
                                row.col(|ui| {
                                    ui.painter().rect_filled(ui.max_rect(), 0.0, row_color);
                                    let text = format_cell_with_source_flags(
                                        rom.md5.as_ref().map(hex::encode).unwrap_or_default(),
                                        &rom,
                                        rv_core::rv_file::FileStatus::MD5_FROM_DAT,
                                        rv_core::rv_file::FileStatus::MD5_FROM_HEADER,
                                    );
                                    let resp = ui.add(egui::Label::new(text).sense(egui::Sense::click()));
                                    if resp.secondary_clicked() {
                                        if let Some(copy) = rom_clipboard_text(&rom, RomGridCopyColumn::Md5) {
                                            ui.output_mut(|o| o.copied_text = copy.clone());
                                            self.task_logs.push(format!("Copied: {}", copy));
                                        }
                                    }
                                });
                                if alt_found {
                                    row.col(|ui| {
                                        ui.painter().rect_filled(ui.max_rect(), 0.0, row_color);
                                        let text = format_cell_with_source_flags(
                                            rom.alt_size.map(|s| s.to_string()).unwrap_or_default(),
                                            &rom,
                                            rv_core::rv_file::FileStatus::ALT_SIZE_FROM_DAT,
                                            rv_core::rv_file::FileStatus::ALT_SIZE_FROM_HEADER,
                                        );
                                        let resp = ui.add(egui::Label::new(text).sense(egui::Sense::click()));
                                        if resp.secondary_clicked() {
                                            if let Some(copy) = rom_clipboard_text(&rom, RomGridCopyColumn::AltSize) {
                                                ui.output_mut(|o| o.copied_text = copy.clone());
                                                self.task_logs.push(format!("Copied: {}", copy));
                                            }
                                        }
                                    });
                                    row.col(|ui| {
                                        ui.painter().rect_filled(ui.max_rect(), 0.0, row_color);
                                        let text = format_cell_with_source_flags(
                                            rom.alt_crc.as_ref().map(hex::encode).unwrap_or_default(),
                                            &rom,
                                            rv_core::rv_file::FileStatus::ALT_CRC_FROM_DAT,
                                            rv_core::rv_file::FileStatus::ALT_CRC_FROM_HEADER,
                                        );
                                        let resp = ui.add(egui::Label::new(text).sense(egui::Sense::click()));
                                        if resp.secondary_clicked() {
                                            if let Some(copy) = rom_clipboard_text(&rom, RomGridCopyColumn::AltCrc32)
                                            {
                                                ui.output_mut(|o| o.copied_text = copy.clone());
                                                self.task_logs.push(format!("Copied: {}", copy));
                                            }
                                        }
                                    });
                                    row.col(|ui| {
                                        ui.painter().rect_filled(ui.max_rect(), 0.0, row_color);
                                        let text = format_cell_with_source_flags(
                                            rom.alt_sha1.as_ref().map(hex::encode).unwrap_or_default(),
                                            &rom,
                                            rv_core::rv_file::FileStatus::ALT_SHA1_FROM_DAT,
                                            rv_core::rv_file::FileStatus::ALT_SHA1_FROM_HEADER,
                                        );
                                        let resp = ui.add(egui::Label::new(text).sense(egui::Sense::click()));
                                        if resp.secondary_clicked() {
                                            if let Some(copy) = rom_clipboard_text(&rom, RomGridCopyColumn::AltSha1)
                                            {
                                                ui.output_mut(|o| o.copied_text = copy.clone());
                                                self.task_logs.push(format!("Copied: {}", copy));
                                            }
                                        }
                                    });
                                    row.col(|ui| {
                                        ui.painter().rect_filled(ui.max_rect(), 0.0, row_color);
                                        let text = format_cell_with_source_flags(
                                            rom.alt_md5.as_ref().map(hex::encode).unwrap_or_default(),
                                            &rom,
                                            rv_core::rv_file::FileStatus::ALT_MD5_FROM_DAT,
                                            rv_core::rv_file::FileStatus::ALT_MD5_FROM_HEADER,
                                        );
                                        let resp = ui.add(egui::Label::new(text).sense(egui::Sense::click()));
                                        if resp.secondary_clicked() {
                                            if let Some(copy) = rom_clipboard_text(&rom, RomGridCopyColumn::AltMd5)
                                            {
                                                ui.output_mut(|o| o.copied_text = copy.clone());
                                                self.task_logs.push(format!("Copied: {}", copy));
                                            }
                                        }
                                    });
                                }
                                if show_status {
                                    row.col(|ui| {
                                        ui.painter().rect_filled(ui.max_rect(), 0.0, row_color);
                                        ui.label(rom.status.as_deref().unwrap_or(""));
                                    });
                                }
                                if show_file_mod_date {
                                    row.col(|ui| {
                                        ui.painter().rect_filled(ui.max_rect(), 0.0, row_color);
                                        let text = format_file_mod_date_cell(&rom);
                                        ui.label(format_cell_with_source_flags(
                                            text,
                                            &rom,
                                            rv_core::rv_file::FileStatus::DATE_FROM_DAT,
                                            rv_core::rv_file::FileStatus::NONE,
                                        ));
                                    });
                                }
                                if show_zip_index {
                                    row.col(|ui| {
                                        ui.painter().rect_filled(ui.max_rect(), 0.0, row_color);
                                        ui.label(row_data.zip_index.map(|v| v.to_string()).unwrap_or_default());
                                    });
                                }
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
                                        self.rom_info_lines = collect_rom_occurrence_lines(Rc::clone(&rom_rc));
                                        self.show_rom_info = true;
                                    }
                                });
                            });
                        });
                });
            });

        self.sort_col = new_sort_col_rom;
        self.sort_desc = new_sort_desc_rom;
    }
}

