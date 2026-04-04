use eframe::egui;

use crate::RomVaultApp;

pub fn draw_global_settings(app: &mut RomVaultApp, ctx: &egui::Context) {
    if !app.show_settings {
        return;
    }

    let mut close_settings = false;
    egui::Window::new("Global Settings")
        .open(&mut app.show_settings)
        .default_width(450.0)
        .show(ctx, |ui| {
            ui.heading("RomVault Settings");
            ui.separator();

            ui.horizontal(|ui| {
                ui.selectable_value(&mut app.global_settings_tab, 0, "Core Settings");
                ui.selectable_value(&mut app.global_settings_tab, 1, "DATVault / Compression");
                ui.selectable_value(&mut app.global_settings_tab, 2, "Logging / UI");
                ui.selectable_value(&mut app.global_settings_tab, 3, "Emulators");
            });
            ui.separator();

            if app.global_settings_tab == 0 {
                ui.horizontal(|ui| {
                    ui.label("DAT Root Directory:");
                    ui.text_edit_singleline(&mut app.global_settings.dat_root);
                });

                ui.horizontal(|ui| {
                    ui.label("Cache File:");
                    ui.text_edit_singleline(&mut app.global_settings.cache_file);
                });

                ui.horizontal(|ui| {
                    ui.label("Fix DAT Output Path:");
                    let mut fix_path = app.global_settings.fix_dat_out_path.clone().unwrap_or_default();
                    if ui.text_edit_singleline(&mut fix_path).changed() {
                        app.global_settings.fix_dat_out_path =
                            if fix_path.is_empty() { None } else { Some(fix_path) };
                    }
                });

                ui.add_space(5.0);
                ui.label("Fix Level:");
                egui::ComboBox::from_id_source("fix_level")
                    .selected_text(match app.global_settings.fix_level {
                        rv_core::settings::EFixLevel::Level1 => "Level 1 - Fast copy Match on CRC",
                        rv_core::settings::EFixLevel::Level2 => "Level 2 - Fast copy if SHA1 scanned",
                        rv_core::settings::EFixLevel::Level3 => "Level 3 - Uncompress/Hash/Compress",
                    })
                    .show_ui(ui, |ui| {
                        ui.selectable_value(
                            &mut app.global_settings.fix_level,
                            rv_core::settings::EFixLevel::Level1,
                            "Level 1 - Fast copy Match on CRC",
                        );
                        ui.selectable_value(
                            &mut app.global_settings.fix_level,
                            rv_core::settings::EFixLevel::Level2,
                            "Level 2 - Fast copy if SHA1 scanned",
                        );
                        ui.selectable_value(
                            &mut app.global_settings.fix_level,
                            rv_core::settings::EFixLevel::Level3,
                            "Level 3 - Uncompress/Hash/Compress",
                        );
                    });

                ui.add_space(5.0);
                ui.label("Ignore Files (one per line):");
                ui.label("Tip: 'ignore:<pattern>' skips scanning. Without 'ignore:' it marks NotInDat files as Ignore. Supports '*' '?' and 'regex:<expr>'.");
                let mut ignore_str = app.global_settings.ignore_files.items.join("\n");
                if ui.text_edit_multiline(&mut ignore_str).changed() {
                    app.global_settings.ignore_files.items = ignore_str
                        .lines()
                        .map(|s: &str| s.to_string())
                        .filter(|s: &String| !s.is_empty())
                        .collect();
                }

                ui.add_space(5.0);
                ui.checkbox(
                    &mut app.global_settings.double_check_delete,
                    "Double check file exists elsewhere before deleting",
                );

                ui.horizontal(|ui| {
                    ui.checkbox(&mut app.global_settings.cache_save_timer_enabled, "Save Cache on timer every");
                    if app.global_settings.cache_save_timer_enabled {
                        ui.add(
                            egui::DragValue::new(&mut app.global_settings.cache_save_time_period)
                                .speed(1)
                                .clamp_range(5..=60),
                        );
                        ui.label("Minutes");
                    }
                });
            } else if app.global_settings_tab == 1 {
                ui.label("DATVault:");
                ui.checkbox(&mut app.global_settings.mia_callback, "Send Found MIA notifications");
                if app.global_settings.mia_callback {
                    ui.horizontal(|ui| {
                        ui.add_space(20.0);
                        ui.checkbox(&mut app.global_settings.mia_anon, "Send anonymously");
                    });
                }

                ui.checkbox(
                    &mut app.global_settings.delete_old_cue_files,
                    "Delete previous Cue file zips in ToSort",
                );

                ui.add_space(10.0);
                ui.label("Compression:");
                ui.horizontal(|ui| {
                    ui.label("Max ZSTD workers:");
                    egui::ComboBox::from_id_source("zstd_workers")
                        .selected_text(if app.global_settings.zstd_comp_count == 0 {
                            "Auto".to_string()
                        } else {
                            app.global_settings.zstd_comp_count.to_string()
                        })
                        .show_ui(ui, |ui| {
                            ui.selectable_value(&mut app.global_settings.zstd_comp_count, 0, "Auto");
                            for i in 1..=64 {
                                ui.selectable_value(&mut app.global_settings.zstd_comp_count, i, i.to_string());
                            }
                        });
                });

                ui.horizontal(|ui| {
                    ui.label("Default 7Z type:");
                    egui::ComboBox::from_id_source("7z_struct")
                        .selected_text(match app.global_settings.seven_z_default_struct {
                            0 => "LZMA Solid - rv7z",
                            1 => "LZMA Non-Solid",
                            2 => "ZSTD Solid",
                            3 => "ZSTD Non-Solid",
                            _ => "Unknown",
                        })
                        .show_ui(ui, |ui| {
                            ui.selectable_value(&mut app.global_settings.seven_z_default_struct, 0, "LZMA Solid - rv7z");
                            ui.selectable_value(&mut app.global_settings.seven_z_default_struct, 1, "LZMA Non-Solid");
                            ui.selectable_value(&mut app.global_settings.seven_z_default_struct, 2, "ZSTD Solid");
                            ui.selectable_value(&mut app.global_settings.seven_z_default_struct, 3, "ZSTD Non-Solid");
                        });
                });
            } else if app.global_settings_tab == 2 {
                ui.checkbox(
                    &mut app.global_settings.detailed_fix_reporting,
                    "Show detailed actions in Fixing Status window",
                );
                ui.checkbox(&mut app.global_settings.debug_logs_enabled, "Enable Debug logging");
                ui.checkbox(
                    &mut app.global_settings.do_not_report_feedback,
                    "Do not report feedback",
                );
                ui.checkbox(&mut app.global_settings.darkness, "Dark Mode (Restart required.)");
                ui.checkbox(&mut app.global_settings.check_chd_version, "Check CHD Version");
            } else if app.global_settings_tab == 3 {
                ui.label("Emulator Mappings:");
                ui.add_space(4.0);

                let table = egui_extras::TableBuilder::new(ui)
                    .striped(true)
                    .resizable(true)
                    .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
                    .column(egui_extras::Column::initial(140.0).resizable(true))
                    .column(egui_extras::Column::initial(160.0).resizable(true))
                    .column(egui_extras::Column::remainder())
                    .column(egui_extras::Column::initial(160.0).resizable(true))
                    .column(egui_extras::Column::initial(160.0).resizable(true))
                    .min_scrolled_height(220.0);

                table
                    .header(24.0, |mut header| {
                        header.col(|ui| {
                            ui.strong("Tree Dir");
                        });
                        header.col(|ui| {
                            ui.strong("Exe");
                        });
                        header.col(|ui| {
                            ui.strong("Command Line");
                        });
                        header.col(|ui| {
                            ui.strong("Working Dir");
                        });
                        header.col(|ui| {
                            ui.strong("Extra PATH");
                        });
                    })
                    .body(|mut body| {
                        let len = app.global_settings.e_info.items.len();
                        for i in 0..len {
                            body.row(24.0, |mut row| {
                                let is_selected = app.selected_emulator_idx == Some(i);

                                row.col(|ui| {
                                    if is_selected {
                                        ui.painter().rect_filled(
                                            ui.max_rect(),
                                            0.0,
                                            ui.visuals().selection.bg_fill.linear_multiply(0.3),
                                        );
                                    }
                                    let mut v = app.global_settings.e_info.items[i]
                                        .tree_dir
                                        .clone()
                                        .unwrap_or_default();
                                    let resp = ui.add(
                                        egui::TextEdit::singleline(&mut v)
                                            .desired_width(ui.available_width())
                                            .frame(false),
                                    );
                                    if resp.changed() {
                                        let s = v.trim().replace('/', "\\");
                                        app.global_settings.e_info.items[i].tree_dir =
                                            if s.is_empty() { None } else { Some(s) };
                                    }
                                    if resp.gained_focus() {
                                        app.selected_emulator_idx = Some(i);
                                    }
                                });

                                row.col(|ui| {
                                    if is_selected {
                                        ui.painter().rect_filled(
                                            ui.max_rect(),
                                            0.0,
                                            ui.visuals().selection.bg_fill.linear_multiply(0.3),
                                        );
                                    }
                                    let mut v = app.global_settings.e_info.items[i]
                                        .exe_name
                                        .clone()
                                        .unwrap_or_default();
                                    let resp = ui.add(
                                        egui::TextEdit::singleline(&mut v)
                                            .desired_width(ui.available_width())
                                            .frame(false),
                                    );
                                    if resp.changed() {
                                        let s = v.trim().to_string();
                                        app.global_settings.e_info.items[i].exe_name =
                                            if s.is_empty() { None } else { Some(s) };
                                    }
                                    if resp.gained_focus() {
                                        app.selected_emulator_idx = Some(i);
                                    }
                                });

                                row.col(|ui| {
                                    if is_selected {
                                        ui.painter().rect_filled(
                                            ui.max_rect(),
                                            0.0,
                                            ui.visuals().selection.bg_fill.linear_multiply(0.3),
                                        );
                                    }
                                    let mut v = app.global_settings.e_info.items[i]
                                        .command_line
                                        .clone()
                                        .unwrap_or_default();
                                    let resp = ui.add(
                                        egui::TextEdit::singleline(&mut v)
                                            .desired_width(ui.available_width())
                                            .frame(false),
                                    );
                                    if resp.changed() {
                                        let s = v.trim().to_string();
                                        app.global_settings.e_info.items[i].command_line =
                                            if s.is_empty() { None } else { Some(s) };
                                    }
                                    if resp.gained_focus() {
                                        app.selected_emulator_idx = Some(i);
                                    }
                                });

                                row.col(|ui| {
                                    if is_selected {
                                        ui.painter().rect_filled(
                                            ui.max_rect(),
                                            0.0,
                                            ui.visuals().selection.bg_fill.linear_multiply(0.3),
                                        );
                                    }
                                    let mut v = app.global_settings.e_info.items[i]
                                        .working_directory
                                        .clone()
                                        .unwrap_or_default();
                                    let resp = ui.add(
                                        egui::TextEdit::singleline(&mut v)
                                            .desired_width(ui.available_width())
                                            .frame(false),
                                    );
                                    if resp.changed() {
                                        let s = v.trim().to_string();
                                        app.global_settings.e_info.items[i].working_directory =
                                            if s.is_empty() { None } else { Some(s) };
                                    }
                                    if resp.gained_focus() {
                                        app.selected_emulator_idx = Some(i);
                                    }
                                });

                                row.col(|ui| {
                                    if is_selected {
                                        ui.painter().rect_filled(
                                            ui.max_rect(),
                                            0.0,
                                            ui.visuals().selection.bg_fill.linear_multiply(0.3),
                                        );
                                    }
                                    let mut v = app.global_settings.e_info.items[i]
                                        .extra_path
                                        .clone()
                                        .unwrap_or_default();
                                    let resp = ui.add(
                                        egui::TextEdit::singleline(&mut v)
                                            .desired_width(ui.available_width())
                                            .frame(false),
                                    );
                                    if resp.changed() {
                                        let s = v.trim().to_string();
                                        app.global_settings.e_info.items[i].extra_path =
                                            if s.is_empty() { None } else { Some(s) };
                                    }
                                    if resp.gained_focus() {
                                        app.selected_emulator_idx = Some(i);
                                    }
                                });
                            });
                        }
                    });

                ui.add_space(6.0);
                ui.horizontal(|ui| {
                    if ui.button("Add").clicked() {
                        app.global_settings.e_info.items.push(rv_core::settings::EmulatorInfo {
                            tree_dir: None,
                            exe_name: None,
                            command_line: None,
                            working_directory: None,
                            extra_path: None,
                        });
                        app.selected_emulator_idx = Some(app.global_settings.e_info.items.len() - 1);
                    }

                    let can_remove = app
                        .selected_emulator_idx
                        .is_some_and(|idx| idx < app.global_settings.e_info.items.len());
                    if ui.add_enabled(can_remove, egui::Button::new("Remove")).clicked() {
                        if let Some(idx) = app.selected_emulator_idx {
                            if idx < app.global_settings.e_info.items.len() {
                                app.global_settings.e_info.items.remove(idx);
                            }
                        }
                        app.selected_emulator_idx = None;
                    }
                });
            }

            ui.separator();
            ui.horizontal(|ui| {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("Cancel").clicked() {
                        close_settings = true;
                    }
                    if ui.button("OK").clicked() {
                        app.global_settings.e_info.items.retain(|ei| {
                            ei.tree_dir.as_ref().is_some_and(|s| !s.trim().is_empty())
                                || ei.exe_name.as_ref().is_some_and(|s| !s.trim().is_empty())
                                || ei.command_line.as_ref().is_some_and(|s| !s.trim().is_empty())
                        });
                        for ei in &mut app.global_settings.e_info.items {
                            if let Some(s) = ei.tree_dir.as_ref() {
                                let v = s.trim().replace('/', "\\").trim_matches('\\').to_string();
                                ei.tree_dir = if v.is_empty() { None } else { Some(v) };
                            }
                            if let Some(s) = ei.exe_name.as_ref() {
                                let v = s.trim().to_string();
                                ei.exe_name = if v.is_empty() { None } else { Some(v) };
                            }
                            if let Some(s) = ei.command_line.as_ref() {
                                let v = s.trim().to_string();
                                ei.command_line = if v.is_empty() { None } else { Some(v) };
                            }
                            if let Some(s) = ei.working_directory.as_ref() {
                                let v = s.trim().to_string();
                                ei.working_directory = if v.is_empty() { None } else { Some(v) };
                            }
                            if let Some(s) = ei.extra_path.as_ref() {
                                let v = s.trim().to_string();
                                ei.extra_path = if v.is_empty() { None } else { Some(v) };
                            }
                        }
                        rv_core::settings::update_settings(app.global_settings.clone());
                        let _ = rv_core::settings::write_settings_to_file(&app.global_settings);
                        app.global_settings = rv_core::settings::get_settings();
                        rv_core::db::DB::check_create_root_dirs();
                        let mut ctx_style = (*ctx.style()).clone();
                        ctx_style.visuals = if app.global_settings.darkness {
                            egui::Visuals::dark()
                        } else {
                            egui::Visuals::light()
                        };
                        ctx.set_style(ctx_style);
                        app.task_logs.push("Saved Global Settings".to_string());
                        close_settings = true;
                    }
                });
            });
        });

    if close_settings {
        app.show_settings = false;
    }
}
