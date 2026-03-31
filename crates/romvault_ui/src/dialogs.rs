use eframe::egui;

use crate::RomVaultApp;

/// Logic for drawing all popup dialog windows in the application.
/// 
/// `dialogs.rs` handles rendering the Global Settings, Directory Settings, Directory Mappings,
/// Add ToSort, and About popups.
/// 
/// Differences from C#:
/// - The C# version utilizes individual `.Designer.cs` WinForms definitions for every single popup 
///   dialog (e.g. `FrmSettings`, `FrmDirectorySettings`, `FrmRegistration`).
/// - The Rust version groups all of these popups into a single `draw_dialogs` function, toggling
///   their visibility via boolean state flags stored in the main `RomVaultApp` struct.
pub fn draw_dialogs(app: &mut RomVaultApp, ctx: &egui::Context) {
    if app.show_dir_mappings {
        let mut close_dir_mappings = false;
        egui::Window::new("Directory Mappings")
            .open(&mut app.show_dir_mappings)
            .resizable(true)
            .default_width(600.0)
            .show(ctx, |ui| {
                ui.heading("Directory Mappings");
                ui.separator();

                // Faithful C# RomVault layout: Table with "Directory" and "Mapping" columns
                let table = egui_extras::TableBuilder::new(ui)
                    .striped(true)
                    .resizable(true)
                    .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
                    .column(egui_extras::Column::initial(200.0).at_least(100.0)) // Directory
                    .column(egui_extras::Column::remainder()) // Mapping
                    .min_scrolled_height(200.0);

                table.header(24.0, |mut header| {
                    header.col(|ui| { ui.strong("Directory"); });
                    header.col(|ui| { ui.strong("Mapping"); });
                })
                .body(|mut body| {
                    let mappings_len = app.working_dir_mappings.len();
                    for i in 0..mappings_len {
                        let is_selected = app.selected_dir_mapping_idx == Some(i);
                        
                        body.row(26.0, |mut row| {
                            // Directory column
                            row.col(|ui| {
                                if is_selected {
                                    ui.painter().rect_filled(ui.max_rect(), 0.0, ui.visuals().selection.bg_fill.linear_multiply(0.3));
                                }
                                let mut key = app.working_dir_mappings[i].dir_key.clone();
                                let response = ui.add(egui::TextEdit::singleline(&mut key).desired_width(ui.available_width()).frame(false));
                                if response.changed() {
                                    app.working_dir_mappings[i].dir_key = key;
                                }
                                if response.gained_focus() {
                                    app.selected_dir_mapping_idx = Some(i);
                                }
                            });
                            // Mapping column
                            row.col(|ui| {
                                if is_selected {
                                    ui.painter().rect_filled(ui.max_rect(), 0.0, ui.visuals().selection.bg_fill.linear_multiply(0.3));
                                }
                                let mut path = app.working_dir_mappings[i].dir_path.clone();
                                let response = ui.add(egui::TextEdit::singleline(&mut path).desired_width(ui.available_width()).frame(false));
                                if response.changed() {
                                    app.working_dir_mappings[i].dir_path = path;
                                }
                                if response.gained_focus() {
                                    app.selected_dir_mapping_idx = Some(i);
                                }
                            });
                        });
                    }
                });

                ui.add_space(10.0);

                ui.horizontal(|ui| {
                    if ui.button("Add").clicked() {
                        app.working_dir_mappings.push(rv_core::settings::DirMapping {
                            dir_key: "NewDirectory".to_string(),
                            dir_path: "NewMapping".to_string(),
                        });
                        app.selected_dir_mapping_idx = Some(app.working_dir_mappings.len() - 1);
                    }
                    if ui.button("Remove").clicked() {
                        if let Some(idx) = app.selected_dir_mapping_idx {
                            if idx < app.working_dir_mappings.len() {
                                app.working_dir_mappings.remove(idx);
                                app.selected_dir_mapping_idx = None;
                            }
                        }
                    }
                });

                ui.separator();
                ui.horizontal(|ui| {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button("Cancel").clicked() {
                            close_dir_mappings = true;
                        }
                        if ui.button("OK").clicked() {
                            app.global_settings.dir_mappings.items = app.working_dir_mappings.clone();
                            rv_core::settings::update_settings(app.global_settings.clone());
                            app.task_logs.push("Saved Directory Mappings".to_string());
                            close_dir_mappings = true;
                        }
                    });
                });
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
            .default_width(450.0)
            .show(ctx, |ui| {
                ui.heading(format!("Settings for: {}", app.active_dat_rule.dir_key));
                ui.separator();

                ui.horizontal(|ui| {
                    ui.selectable_value(&mut app.dir_settings_tab, 0, "Dir/Merge");
                    ui.selectable_value(&mut app.dir_settings_tab, 1, "Advanced");
                    ui.selectable_value(&mut app.dir_settings_tab, 2, "Exclude");
                });
                ui.separator();

                if app.dir_settings_tab == 0 {
                    ui.label("Archive Type:");
                    egui::ComboBox::from_id_source("archive_type")
                        .selected_text(match app.active_dat_rule.compression {
                            dat_reader::enums::FileType::File => "Uncompressed",
                            dat_reader::enums::FileType::Zip => "Zip",
                            dat_reader::enums::FileType::SevenZip => "SevenZip",
                            dat_reader::enums::FileType::FileOnly => "Mixed (Archive as File)",
                            _ => "Unknown",
                        })
                        .show_ui(ui, |ui| {
                            ui.selectable_value(&mut app.active_dat_rule.compression, dat_reader::enums::FileType::File, "Uncompressed");
                            ui.selectable_value(&mut app.active_dat_rule.compression, dat_reader::enums::FileType::Zip, "Zip");
                            ui.selectable_value(&mut app.active_dat_rule.compression, dat_reader::enums::FileType::SevenZip, "SevenZip");
                            ui.selectable_value(&mut app.active_dat_rule.compression, dat_reader::enums::FileType::FileOnly, "Mixed (Archive as File)");
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

                    ui.label("Filter Type:");
                    egui::ComboBox::from_id_source("filter_type")
                        .selected_text(format!("{:?}", app.active_dat_rule.filter))
                        .show_ui(ui, |ui| {
                            ui.selectable_value(&mut app.active_dat_rule.filter, rv_core::settings::FilterType::KeepAll, "Roms & CHDs");
                            ui.selectable_value(&mut app.active_dat_rule.filter, rv_core::settings::FilterType::RomsOnly, "Roms Only");
                            ui.selectable_value(&mut app.active_dat_rule.filter, rv_core::settings::FilterType::CHDsOnly, "CHDs Only");
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

                    ui.checkbox(&mut app.active_dat_rule.single_archive, "Merge into a single archive");
                    ui.checkbox(&mut app.active_dat_rule.use_description_as_dir_name, "Use description for Directory Name");
                    ui.checkbox(&mut app.active_dat_rule.multi_dat_dir_override, "Override multi DAT directory");
                } else if app.dir_settings_tab == 1 {
                    ui.checkbox(&mut app.active_dat_rule.use_id_for_name, "Use ID for Name");
                    ui.checkbox(&mut app.active_dat_rule.complete_only, "Complete Only");
                    
                    let prev_checked = app.active_dat_rule.add_category_sub_dirs;
                    if ui.checkbox(&mut app.active_dat_rule.add_category_sub_dirs, "Add Category Sub Dirs").changed() {
                        if app.active_dat_rule.add_category_sub_dirs && !prev_checked {
                            if app.active_dat_rule.category_order.items.is_empty() {
                                app.active_dat_rule.category_order.items = vec![
                                    "Preproduction".to_string(),
                                    "Educational".to_string(),
                                    "Guides".to_string(),
                                    "Manuals".to_string(),
                                    "Magazines".to_string(),
                                    "Documents".to_string(),
                                    "Audio".to_string(),
                                    "Video".to_string(),
                                    "Multimedia".to_string(),
                                    "Coverdiscs".to_string(),
                                    "Covermount".to_string(),
                                    "Bonus Discs".to_string(),
                                    "Bonus".to_string(),
                                    "Add-Ons".to_string(),
                                    "Source Code".to_string(),
                                    "Updates".to_string(),
                                    "Applications".to_string(),
                                    "Demos".to_string(),
                                    "Games".to_string(),
                                    "Miscellaneous".to_string(),
                                ];
                            }
                        }
                    }

                    ui.separator();
                    ui.add_enabled_ui(app.active_dat_rule.add_category_sub_dirs, |ui| {
                        ui.label("Category Order (one per line):");
                        let mut cat_str = app.active_dat_rule.category_order.items.join("\n");
                        if ui.text_edit_multiline(&mut cat_str).changed() {
                            app.active_dat_rule.category_order.items = cat_str
                                .lines()
                                .map(|s: &str| s.to_string())
                                .filter(|s: &String| !s.is_empty())
                                .collect();
                        }
                    });

                } else if app.dir_settings_tab == 2 {
                    ui.label("Ignore Files (one per line):");
                    let mut ignore_str = app.active_dat_rule.ignore_files.items.join("\n");
                    if ui.text_edit_multiline(&mut ignore_str).changed() {
                        app.active_dat_rule.ignore_files.items = ignore_str
                            .lines()
                            .map(|s: &str| s.to_string())
                            .filter(|s: &String| !s.is_empty())
                            .collect();
                    }
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
            .default_width(450.0)
            .show(ctx, |ui| {
                ui.heading("RomVault Settings");
                ui.separator();

                ui.horizontal(|ui| {
                    ui.selectable_value(&mut app.global_settings_tab, 0, "Core Settings");
                    ui.selectable_value(&mut app.global_settings_tab, 1, "DATVault / Compression");
                    ui.selectable_value(&mut app.global_settings_tab, 2, "Logging / UI");
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
                    ui.label("Filenames not to remove (one per line):");
                    let mut ignore_str = app.global_settings.ignore_files.items.join("\n");
                    if ui.text_edit_multiline(&mut ignore_str).changed() {
                        app.global_settings.ignore_files.items = ignore_str
                            .lines()
                            .map(|s: &str| s.to_string())
                            .filter(|s: &String| !s.is_empty())
                            .collect();
                    }

                    ui.add_space(5.0);
                    ui.checkbox(&mut app.global_settings.double_check_delete, "Double check file exists elsewhere before deleting");

                    ui.horizontal(|ui| {
                        ui.checkbox(&mut app.global_settings.cache_save_timer_enabled, "Save Cache on timer every");
                        if app.global_settings.cache_save_timer_enabled {
                            ui.add(egui::DragValue::new(&mut app.global_settings.cache_save_time_period).speed(1).clamp_range(5..=60));
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
                    
                    ui.checkbox(&mut app.global_settings.delete_old_cue_files, "Delete previous Cue file zips in ToSort");

                    ui.add_space(10.0);
                    ui.label("Compression:");
                    ui.horizontal(|ui| {
                        ui.label("Max ZSTD workers:");
                        egui::ComboBox::from_id_source("zstd_workers")
                            .selected_text(if app.global_settings.zstd_comp_count == 0 { "Auto".to_string() } else { app.global_settings.zstd_comp_count.to_string() })
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
                    ui.checkbox(&mut app.global_settings.detailed_fix_reporting, "Show detailed actions in Fixing Status window");
                    ui.checkbox(&mut app.global_settings.debug_logs_enabled, "Enable Debug logging");
                    ui.checkbox(&mut app.global_settings.do_not_report_feedback, "Do not report feedback");
                    ui.checkbox(&mut app.global_settings.darkness, "Dark Mode (Restart required.)");
                    ui.checkbox(&mut app.global_settings.check_chd_version, "Check CHD Version");
                }

                ui.separator();
                ui.horizontal(|ui| {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button("Cancel").clicked() {
                            close_settings = true;
                        }
                        if ui.button("OK").clicked() {
                            rv_core::settings::update_settings(app.global_settings.clone());
                            app.task_logs.push("Saved Global Settings".to_string());
                            close_settings = true;
                        }
                    });
                });
            });
    }
    if close_settings {
        app.show_settings = false;
    }
}
