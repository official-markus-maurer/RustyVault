use eframe::egui;

use crate::RomVaultApp;

pub fn draw_dialogs(app: &mut RomVaultApp, ctx: &egui::Context) {
    if app.show_dir_mappings {
        let mut close_dir_mappings = false;
        egui::Window::new("Directory Mappings")
            .open(&mut app.show_dir_mappings)
            .show(ctx, |ui| {
                ui.heading("Directory Mappings");
                ui.separator();

                ui.label("Map physical directories to DAT roots.");
                ui.add_space(5.0);

                egui::Grid::new("dir_mappings_grid")
                    .num_columns(2)
                    .spacing([20.0, 10.0])
                    .striped(true)
                    .show(ui, |ui| {
                        ui.strong("Virtual Path");
                        ui.strong("Physical Path");
                        ui.end_row();

                        let mut idx_to_remove = None;
                        for (i, mapping) in app.global_settings.dir_mappings.iter_mut().enumerate() {
                            let mut key = mapping.dir_key.clone();
                            if ui.text_edit_singleline(&mut key).changed() {
                                mapping.dir_key = key;
                            }

                            let mut path = mapping.dir_path.clone();
                            if ui.text_edit_singleline(&mut path).changed() {
                                mapping.dir_path = path;
                            }

                            if ui.button("Remove").clicked() {
                                idx_to_remove = Some(i);
                            }
                            ui.end_row();
                        }

                        if let Some(idx) = idx_to_remove {
                            app.global_settings.dir_mappings.remove(idx);
                        }
                    });

                ui.separator();
                ui.horizontal(|ui| {
                    if ui.button("Add Mapping").clicked() {
                        app.global_settings.dir_mappings.push(rv_core::settings::DirMapping {
                            dir_key: "RustyVault\\NewMapping".to_string(),
                            dir_path: "C:\\Roms\\NewMapping".to_string(),
                        });
                        app.task_logs.push("Added new Directory Mapping".to_string());
                    }
                    if ui.button("Save Mappings").clicked() {
                        rv_core::settings::update_settings(app.global_settings.clone());
                        app.task_logs.push("Saved Directory Mappings".to_string());
                        close_dir_mappings = true;
                    }
                });

                ui.separator();
                if ui.button("Close").clicked() {
                    close_dir_mappings = true;
                }
            });
        if close_dir_mappings {
            app.show_dir_mappings = false;
        }
    }

    if app.show_sam_dialog {
        let mut close_sam = false;
        egui::Window::new("Structured Archive Maker (SAM-UI)")
            .open(&mut app.show_sam_dialog)
            .show(ctx, |ui| {
                ui.heading("Structured Archive Maker");
                ui.separator();
                ui.label("This utility allows you to convert directories and standard archives into TorrentZips or 7Zips.");
                ui.add_space(5.0);

                ui.label("Status: SAM functionality is integrated into the rv_core bindings.");
                ui.label("Drag and drop not fully supported in this egui context yet.");

                ui.add_space(5.0);
                ui.horizontal(|ui| {
                    ui.label("Input Format:");
                    let mut in_fmt = 0;
                    egui::ComboBox::from_id_source("sam_in")
                        .selected_text("Zip")
                        .show_ui(ui, |ui| {
                            ui.selectable_value(&mut in_fmt, 0, "Zip");
                        });
                    ui.label("Output Format:");
                    let mut out_fmt = 0;
                    egui::ComboBox::from_id_source("sam_out")
                        .selected_text("TorrentZip")
                        .show_ui(ui, |ui| {
                            ui.selectable_value(&mut out_fmt, 0, "TorrentZip");
                        });
                });

                ui.separator();
                if ui.button("Close").clicked() {
                    close_sam = true;
                }
            });
        if close_sam {
            app.show_sam_dialog = false;
        }
    }

    if app.show_color_key {
        let mut close_color_key = false;
        egui::Window::new("Color Key")
            .open(&mut app.show_color_key)
            .show(ctx, |ui| {
                ui.heading("Grid Status Colors");
                ui.separator();
                let keys = [
                    ("Correct / CorrectMIA", egui::Color32::from_rgb(214, 255, 214)),
                    ("Missing / MissingMIA", egui::Color32::from_rgb(255, 214, 214)),
                    ("CanBeFixed / CorruptCanBeFixed", egui::Color32::from_rgb(255, 255, 214)),
                    ("MoveToSort / MoveToCorrupt", egui::Color32::from_rgb(214, 255, 255)),
                    ("UnNeeded / Unknown", egui::Color32::from_rgb(214, 214, 214)),
                    ("Delete", egui::Color32::from_rgb(255, 0, 0)),
                ];
                for (label, color) in keys {
                    ui.horizontal(|ui| {
                        let (rect, _response) =
                            ui.allocate_exact_size(egui::vec2(20.0, 20.0), egui::Sense::hover());
                        ui.painter().rect_filled(rect, 0.0, color);
                        ui.label(label);
                    });
                }
                ui.add_space(10.0);
                if ui.button("Close").clicked() {
                    close_color_key = true;
                }
            });
        if close_color_key {
            app.show_color_key = false;
        }
    }

    if app.show_about {
        let mut close_about = false;
        egui::Window::new("About RustyVault")
            .open(&mut app.show_about)
            .show(ctx, |ui| {
                ui.vertical_centered(|ui| {
                    ui.heading("RustyVault");
                    ui.label("Version 3.6.1");
                    ui.add_space(10.0);
                    ui.label("A specialized ROM manager for organizing, verifying, and fixing ROM collections.");
                    ui.label("Forked/ported as RustyVault");
                    ui.add_space(10.0);
                    if ui.button("Close").clicked() {
                        close_about = true;
                    }
                });
            });
        if close_about {
            app.show_about = false;
        }
    }

    if app.show_rom_info {
        let mut close_rom_info = false;
        egui::Window::new("Rom Occurrence list")
            .open(&mut app.show_rom_info)
            .show(ctx, |ui| {
                if let Some(rom_rc) = &app.selected_rom_for_info {
                    let rom = rom_rc.borrow();
                    let file_path = rom.name.clone();
                    let got_status_str = format!("{:?}", rom.rep_status());

                    egui::ScrollArea::vertical().show(ui, |ui| {
                        ui.label(format!("{} | {}", got_status_str, file_path));
                    });
                }

                ui.add_space(10.0);
                if ui.button("Close").clicked() {
                    close_rom_info = true;
                }
            });
        if close_rom_info {
            app.show_rom_info = false;
            app.selected_rom_for_info = None;
        }
    }

    let mut close_dir_settings = false;
    if app.show_dir_settings {
        egui::Window::new("Directory Settings")
            .open(&mut app.show_dir_settings)
            .show(ctx, |ui| {
                ui.heading(format!("Settings for: {}", app.active_dat_rule.dir_key));
                ui.separator();

                ui.label("Archive Type:");
                egui::ComboBox::from_id_source("archive_type")
                    .selected_text(format!("{:?}", app.active_dat_rule.compression))
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut app.active_dat_rule.compression, dat_reader::enums::FileType::File, "Uncompressed");
                        ui.selectable_value(&mut app.active_dat_rule.compression, dat_reader::enums::FileType::Zip, "Zip");
                        ui.selectable_value(&mut app.active_dat_rule.compression, dat_reader::enums::FileType::SevenZip, "SevenZip");
                    });
                ui.checkbox(&mut app.active_dat_rule.compression_override_dat, "Override DAT Archive Type");

                ui.separator();

                ui.label("Merge Type:");
                egui::ComboBox::from_id_source("merge_type")
                    .selected_text(format!("{:?}", app.active_dat_rule.merge))
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut app.active_dat_rule.merge, rv_core::settings::MergeType::None, "Nothing");
                        ui.selectable_value(&mut app.active_dat_rule.merge, rv_core::settings::MergeType::Split, "Split");
                        ui.selectable_value(&mut app.active_dat_rule.merge, rv_core::settings::MergeType::Merge, "Merge");
                        ui.selectable_value(&mut app.active_dat_rule.merge, rv_core::settings::MergeType::NonMerged, "NonMerge");
                    });
                ui.checkbox(&mut app.active_dat_rule.merge_override_dat, "Override DAT Merge Type");

                ui.label("Filter Type:");
                egui::ComboBox::from_id_source("filter_type")
                    .selected_text(format!("{:?}", app.active_dat_rule.filter))
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut app.active_dat_rule.filter, rv_core::settings::FilterType::KeepAll, "Roms & CHDs");
                        ui.selectable_value(&mut app.active_dat_rule.filter, rv_core::settings::FilterType::RomsOnly, "Roms Only");
                        ui.selectable_value(&mut app.active_dat_rule.filter, rv_core::settings::FilterType::CHDsOnly, "CHDs Only");
                    });

                ui.separator();

                ui.label("Header Type:");
                egui::ComboBox::from_id_source("header_type")
                    .selected_text(format!("{:?}", app.active_dat_rule.header_type))
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut app.active_dat_rule.header_type, rv_core::settings::HeaderType::Optional, "Optional");
                        ui.selectable_value(&mut app.active_dat_rule.header_type, rv_core::settings::HeaderType::Headered, "Headered");
                        ui.selectable_value(&mut app.active_dat_rule.header_type, rv_core::settings::HeaderType::Headerless, "Headerless");
                    });

                ui.separator();

                ui.label("Directory Type:");
                egui::ComboBox::from_id_source("dir_type")
                    .selected_text(format!("{:?}", app.active_dat_rule.sub_dir_type))
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut app.active_dat_rule.sub_dir_type, rv_core::settings::RemoveSubType::KeepAllSubDirs, "Use subdirs for all sets");
                        ui.selectable_value(&mut app.active_dat_rule.sub_dir_type, rv_core::settings::RemoveSubType::RemoveAllSubDirs, "Do not use subdirs for sets");
                        ui.selectable_value(&mut app.active_dat_rule.sub_dir_type, rv_core::settings::RemoveSubType::RemoveSubIfNameMatches, "Use subdirs for rom name conflicts");
                        ui.selectable_value(&mut app.active_dat_rule.sub_dir_type, rv_core::settings::RemoveSubType::RemoveSubIfSingleGame, "Use subdirs for multi-rom sets");
                        ui.selectable_value(&mut app.active_dat_rule.sub_dir_type, rv_core::settings::RemoveSubType::RemoveSubIfSingleOrMatches, "Use subdirs for multi-rom sets or set/rom name mismatches");
                    });

                ui.separator();

                ui.checkbox(&mut app.active_dat_rule.multi_dat_dir_override, "Override multi DAT directory");
                ui.checkbox(&mut app.active_dat_rule.single_archive, "Merge into a single archive");
                ui.checkbox(&mut app.active_dat_rule.use_description_as_dir_name, "Use description for Directory Name");
                ui.checkbox(&mut app.active_dat_rule.use_id_for_name, "Use ID for Name");
                ui.checkbox(&mut app.active_dat_rule.complete_only, "Complete Only");
                ui.checkbox(&mut app.active_dat_rule.add_category_sub_dirs, "Add Category Sub Dirs");

                ui.separator();
                ui.label("Ignore Files (One per line):");
                let mut ignore_str = app.active_dat_rule.ignore_files.join("\n");
                if ui.text_edit_multiline(&mut ignore_str).changed() {
                    app.active_dat_rule.ignore_files = ignore_str
                        .lines()
                        .map(|s| s.to_string())
                        .filter(|s| !s.is_empty())
                        .collect();
                }

                ui.separator();
                if ui.button("Apply").clicked() {
                    rv_core::settings::set_rule(app.active_dat_rule.clone());
                    app.task_logs.push(format!(
                        "Applied Directory Settings for {}",
                        app.active_dat_rule.dir_key
                    ));
                    close_dir_settings = true;
                }
            });
    }
    if close_dir_settings {
        app.show_dir_settings = false;
    }

    let mut close_settings = false;
    if app.show_settings {
        egui::Window::new("Global Settings")
            .open(&mut app.show_settings)
            .show(ctx, |ui| {
                ui.heading("Global Settings");
                ui.separator();

                ui.horizontal(|ui| {
                    ui.label("DAT Root Directory:");
                    ui.text_edit_singleline(&mut app.global_settings.dat_root);
                });

                ui.label("Fix Level:");
                egui::ComboBox::from_id_source("fix_level")
                    .selected_text(format!("{:?}", app.global_settings.fix_level))
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

                ui.separator();

                ui.checkbox(&mut app.global_settings.detailed_fix_reporting, "Detailed Fix Reporting");
                ui.checkbox(&mut app.global_settings.double_check_delete, "Double Check Delete");
                ui.checkbox(&mut app.global_settings.debug_logs_enabled, "Enable Debug Logs");

                ui.horizontal(|ui| {
                    ui.checkbox(&mut app.global_settings.cache_save_timer_enabled, "Enable Cache Save Timer");
                    if app.global_settings.cache_save_timer_enabled {
                        ui.add(egui::DragValue::new(&mut app.global_settings.cache_save_time_period).speed(1));
                        ui.label("minutes");
                    }
                });

                ui.separator();

                ui.checkbox(&mut app.global_settings.mia_callback, "Send found MIA to RustyVault.com");
                ui.checkbox(&mut app.global_settings.mia_anon, "Send MIA Anonymously");
                ui.checkbox(&mut app.global_settings.do_not_report_feedback, "Do Not Report Feedback");
                ui.checkbox(&mut app.global_settings.delete_old_cue_files, "Delete Old Cue Files");
                ui.checkbox(&mut app.global_settings.darkness, "Use Dark Theme");
                ui.checkbox(&mut app.global_settings.check_chd_version, "Check CHD Version");

                ui.separator();
                ui.label("Directory Paths");
                ui.horizontal(|ui| {
                    ui.label("DatRoot:");
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

                ui.separator();
                if ui.button("Save Settings").clicked() {
                    rv_core::settings::update_settings(app.global_settings.clone());
                    app.task_logs.push("Saved Global Settings".to_string());
                    close_settings = true;
                }
            });
    }
    if close_settings {
        app.show_settings = false;
    }
}
