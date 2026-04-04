use eframe::egui;

use crate::RomVaultApp;

pub fn draw_dir_settings(app: &mut RomVaultApp, ctx: &egui::Context) {
    if !app.show_dir_settings {
        return;
    }

    let mut close_dir_settings = false;
    let default_height = if app.dir_settings_compact {
        360.0
    } else {
        650.0
    };
    egui::Window::new("Directory Settings")
        .open(&mut app.show_dir_settings)
        .default_width(450.0)
        .default_height(default_height)
        .show(ctx, |ui| {
            ui.heading(format!("Settings for: {}", app.active_dat_rule.dir_key));
            ui.horizontal(|ui| {
                ui.checkbox(&mut app.dir_settings_compact, "Compact");
            });
            ui.separator();

            if let Some(p) = app.active_dat_rule.dir_path.as_ref().filter(|p| !p.trim().is_empty()) {
                ui.label(format!("DirPath: {}", p));
                ui.separator();
            }

            fn normalize_rule_key(key: &str) -> String {
                key.replace('/', "\\").trim_matches('\\').to_string()
            }

            let active_key = normalize_rule_key(&app.active_dat_rule.dir_key);
            let mut switch_to_key: Option<String> = None;

            ui.label("Rules:");
            let rules = app.global_settings.dat_rules.items.clone();
            let table = egui_extras::TableBuilder::new(ui)
                .striped(true)
                .resizable(true)
                .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
                .column(egui_extras::Column::initial(190.0).resizable(true))
                .column(egui_extras::Column::initial(120.0).resizable(true))
                .column(egui_extras::Column::initial(90.0).resizable(true))
                .column(egui_extras::Column::initial(60.0).resizable(false))
                .min_scrolled_height(150.0);

            table
                .header(22.0, |mut header| {
                    header.col(|ui| {
                        ui.strong("Directory");
                    });
                    header.col(|ui| {
                        ui.strong("Compression");
                    });
                    header.col(|ui| {
                        ui.strong("Merge");
                    });
                    header.col(|ui| {
                        ui.strong("Single");
                    });
                })
                .body(|mut body| {
                    for r in &rules {
                        let rule_key = normalize_rule_key(&r.dir_key);
                        let is_current = {
                            #[cfg(windows)]
                            {
                                rule_key.eq_ignore_ascii_case(&active_key)
                            }
                            #[cfg(not(windows))]
                            {
                                rule_key == active_key
                            }
                        };
                        let is_child = {
                            if active_key.is_empty() || rule_key.len() <= active_key.len() {
                                false
                            } else {
                                let prefix = format!("{}\\", active_key);
                                #[cfg(windows)]
                                {
                                    rule_key
                                        .to_ascii_lowercase()
                                        .starts_with(&prefix.to_ascii_lowercase())
                                }
                                #[cfg(not(windows))]
                                {
                                    rule_key.starts_with(&prefix)
                                }
                            }
                        };
                        let is_tosort = r.dir_path.as_ref().is_some_and(|p| {
                            #[cfg(windows)]
                            {
                                p.trim().eq_ignore_ascii_case("ToSort")
                            }
                            #[cfg(not(windows))]
                            {
                                p.trim() == "ToSort"
                            }
                        });

                        let row_color = if is_tosort {
                            egui::Color32::from_rgb(255, 214, 255)
                        } else if is_current {
                            egui::Color32::from_rgb(214, 255, 214)
                        } else if is_child {
                            egui::Color32::from_rgb(255, 255, 214)
                        } else {
                            egui::Color32::TRANSPARENT
                        };

                        body.row(22.0, |mut row| {
                            row.col(|ui| {
                                if row_color != egui::Color32::TRANSPARENT {
                                    ui.painter().rect_filled(ui.max_rect(), 0.0, row_color);
                                }
                                let response = ui.selectable_label(is_current, &r.dir_key);
                                if response.clicked() {
                                    switch_to_key = Some(r.dir_key.clone());
                                }
                            });
                            row.col(|ui| {
                                if row_color != egui::Color32::TRANSPARENT {
                                    ui.painter().rect_filled(ui.max_rect(), 0.0, row_color);
                                }
                                ui.label(format!("{:?}", r.compression_sub));
                            });
                            row.col(|ui| {
                                if row_color != egui::Color32::TRANSPARENT {
                                    ui.painter().rect_filled(ui.max_rect(), 0.0, row_color);
                                }
                                ui.label(format!("{:?}", r.merge));
                            });
                            row.col(|ui| {
                                if row_color != egui::Color32::TRANSPARENT {
                                    ui.painter().rect_filled(ui.max_rect(), 0.0, row_color);
                                }
                                ui.label(if r.single_archive { "Yes" } else { "" });
                            });
                        });
                    }
                });

            if let Some(k) = switch_to_key {
                app.active_dat_rule = rv_core::settings::find_rule(&k);
            }

            ui.separator();
            if !app.dir_settings_compact {
                ui.horizontal(|ui| {
                    ui.selectable_value(&mut app.dir_settings_tab, 0, "Dir/Merge");
                    ui.selectable_value(&mut app.dir_settings_tab, 1, "Advanced");
                    ui.selectable_value(&mut app.dir_settings_tab, 2, "Exclude");
                });
                ui.separator();
            }

            if !app.dir_settings_compact && app.dir_settings_tab == 0 {
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
                        ui.selectable_value(
                            &mut app.active_dat_rule.compression,
                            dat_reader::enums::FileType::File,
                            "Uncompressed",
                        );
                        ui.selectable_value(
                            &mut app.active_dat_rule.compression,
                            dat_reader::enums::FileType::Zip,
                            "Zip",
                        );
                        ui.selectable_value(
                            &mut app.active_dat_rule.compression,
                            dat_reader::enums::FileType::SevenZip,
                            "SevenZip",
                        );
                        ui.selectable_value(
                            &mut app.active_dat_rule.compression,
                            dat_reader::enums::FileType::FileOnly,
                            "Mixed (Archive as File)",
                        );
                    });
                let is_file_only = app.active_dat_rule.compression == dat_reader::enums::FileType::FileOnly;
                if is_file_only {
                    app.active_dat_rule.compression_override_dat = false;
                }
                ui.add_enabled_ui(!is_file_only, |ui| {
                    ui.checkbox(
                        &mut app.active_dat_rule.compression_override_dat,
                        "Override DAT Archive Type",
                    );
                });

                let can_choose_sub = matches!(
                    app.active_dat_rule.compression,
                    dat_reader::enums::FileType::Zip | dat_reader::enums::FileType::SevenZip
                );
                ui.add_enabled_ui(can_choose_sub, |ui| {
                    ui.label("Archive Compression:");
                    let selected_text = match (app.active_dat_rule.compression, app.active_dat_rule.compression_sub) {
                        (dat_reader::enums::FileType::Zip, dat_reader::enums::ZipStructure::ZipZSTD) => "ZSTD",
                        (dat_reader::enums::FileType::Zip, _) => "Deflate - Trrntzip",
                        (dat_reader::enums::FileType::SevenZip, dat_reader::enums::ZipStructure::SevenZipNLZMA) => "LZMA Non-Solid",
                        (dat_reader::enums::FileType::SevenZip, dat_reader::enums::ZipStructure::SevenZipSZSTD) => "ZSTD Solid",
                        (dat_reader::enums::FileType::SevenZip, dat_reader::enums::ZipStructure::SevenZipNZSTD) => "ZSTD Non-Solid",
                        (dat_reader::enums::FileType::SevenZip, _) => "LZMA Solid - rv7z",
                        _ => "Default",
                    };
                    egui::ComboBox::from_id_source("compression_sub")
                        .selected_text(selected_text)
                        .show_ui(ui, |ui| match app.active_dat_rule.compression {
                            dat_reader::enums::FileType::Zip => {
                                ui.selectable_value(
                                    &mut app.active_dat_rule.compression_sub,
                                    dat_reader::enums::ZipStructure::ZipTrrnt,
                                    "Deflate - Trrntzip",
                                );
                                ui.selectable_value(
                                    &mut app.active_dat_rule.compression_sub,
                                    dat_reader::enums::ZipStructure::ZipZSTD,
                                    "ZSTD",
                                );
                            }
                            dat_reader::enums::FileType::SevenZip => {
                                ui.selectable_value(
                                    &mut app.active_dat_rule.compression_sub,
                                    dat_reader::enums::ZipStructure::SevenZipSLZMA,
                                    "LZMA Solid - rv7z",
                                );
                                ui.selectable_value(
                                    &mut app.active_dat_rule.compression_sub,
                                    dat_reader::enums::ZipStructure::SevenZipNLZMA,
                                    "LZMA Non-Solid",
                                );
                                ui.selectable_value(
                                    &mut app.active_dat_rule.compression_sub,
                                    dat_reader::enums::ZipStructure::SevenZipSZSTD,
                                    "ZSTD Solid",
                                );
                                ui.selectable_value(
                                    &mut app.active_dat_rule.compression_sub,
                                    dat_reader::enums::ZipStructure::SevenZipNZSTD,
                                    "ZSTD Non-Solid",
                                );
                            }
                            _ => {}
                        });

                    ui.checkbox(&mut app.active_dat_rule.convert_while_fixing, "Convert while fixing");
                });

                ui.separator();

                ui.label("Merge Type:");
                egui::ComboBox::from_id_source("merge_type")
                    .selected_text(format!("{:?}", app.active_dat_rule.merge))
                    .show_ui(ui, |ui| {
                        ui.selectable_value(
                            &mut app.active_dat_rule.merge,
                            rv_core::settings::MergeType::None,
                            "Nothing",
                        );
                        ui.selectable_value(
                            &mut app.active_dat_rule.merge,
                            rv_core::settings::MergeType::Split,
                            "Split",
                        );
                        ui.selectable_value(
                            &mut app.active_dat_rule.merge,
                            rv_core::settings::MergeType::Merge,
                            "Merge",
                        );
                        ui.selectable_value(
                            &mut app.active_dat_rule.merge,
                            rv_core::settings::MergeType::NonMerged,
                            "NonMerge",
                        );
                    });
                ui.checkbox(&mut app.active_dat_rule.merge_override_dat, "Override DAT Merge Type");

                ui.separator();

                ui.label("Header Type:");
                egui::ComboBox::from_id_source("header_type")
                    .selected_text(format!("{:?}", app.active_dat_rule.header_type))
                    .show_ui(ui, |ui| {
                        ui.selectable_value(
                            &mut app.active_dat_rule.header_type,
                            rv_core::settings::HeaderType::Optional,
                            "Optional",
                        );
                        ui.selectable_value(
                            &mut app.active_dat_rule.header_type,
                            rv_core::settings::HeaderType::Headered,
                            "Headered",
                        );
                        ui.selectable_value(
                            &mut app.active_dat_rule.header_type,
                            rv_core::settings::HeaderType::Headerless,
                            "Headerless",
                        );
                    });

                ui.separator();

                ui.label("Filter Type:");
                egui::ComboBox::from_id_source("filter_type")
                    .selected_text(format!("{:?}", app.active_dat_rule.filter))
                    .show_ui(ui, |ui| {
                        ui.selectable_value(
                            &mut app.active_dat_rule.filter,
                            rv_core::settings::FilterType::KeepAll,
                            "Roms & CHDs",
                        );
                        ui.selectable_value(
                            &mut app.active_dat_rule.filter,
                            rv_core::settings::FilterType::RomsOnly,
                            "Roms Only",
                        );
                        ui.selectable_value(
                            &mut app.active_dat_rule.filter,
                            rv_core::settings::FilterType::CHDsOnly,
                            "CHDs Only",
                        );
                    });

                ui.separator();

                ui.checkbox(&mut app.active_dat_rule.single_archive, "Merge into a single archive");
                ui.add_enabled_ui(app.active_dat_rule.single_archive, |ui| {
                    ui.label("Directory Type:");
                    egui::ComboBox::from_id_source("dir_type")
                        .selected_text(format!("{:?}", app.active_dat_rule.sub_dir_type))
                        .show_ui(ui, |ui| {
                            ui.selectable_value(
                                &mut app.active_dat_rule.sub_dir_type,
                                rv_core::settings::RemoveSubType::KeepAllSubDirs,
                                "Use subdirs for all sets",
                            );
                            ui.selectable_value(
                                &mut app.active_dat_rule.sub_dir_type,
                                rv_core::settings::RemoveSubType::RemoveAllSubDirs,
                                "Do not use subdirs for sets",
                            );
                            ui.selectable_value(
                                &mut app.active_dat_rule.sub_dir_type,
                                rv_core::settings::RemoveSubType::RemoveSubIfNameMatches,
                                "Use subdirs for rom name conflicts",
                            );
                            ui.selectable_value(
                                &mut app.active_dat_rule.sub_dir_type,
                                rv_core::settings::RemoveSubType::RemoveSubIfSingleGame,
                                "Use subdirs for multi-rom sets",
                            );
                            ui.selectable_value(
                                &mut app.active_dat_rule.sub_dir_type,
                                rv_core::settings::RemoveSubType::RemoveSubIfSingleOrMatches,
                                "Use subdirs for multi-rom sets or set/rom name mismatches",
                            );
                        });
                });
                ui.separator();
                ui.checkbox(
                    &mut app.active_dat_rule.use_description_as_dir_name,
                    "Use description for Directory Name",
                );
                ui.checkbox(
                    &mut app.active_dat_rule.multi_dat_dir_override,
                    "Override multi DAT directory",
                );
            } else if !app.dir_settings_compact && app.dir_settings_tab == 1 {
                ui.checkbox(&mut app.active_dat_rule.use_id_for_name, "Use ID for Name");
                ui.checkbox(&mut app.active_dat_rule.complete_only, "Complete Only");

                let prev_checked = app.active_dat_rule.add_category_sub_dirs;
                if ui
                    .checkbox(&mut app.active_dat_rule.add_category_sub_dirs, "Add Category Sub Dirs")
                    .changed()
                    && app.active_dat_rule.add_category_sub_dirs
                    && !prev_checked
                    && app.active_dat_rule.category_order.items.is_empty()
                {
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
            } else if !app.dir_settings_compact && app.dir_settings_tab == 2 {
                ui.label("Ignore Files (one per line):");
                ui.label("Tip: 'ignore:<pattern>' skips scanning. Without 'ignore:' it marks NotInDat files as Ignore. Supports '*' '?' and 'regex:<expr>'.");
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
            ui.horizontal(|ui| {
                if ui.button("Apply").clicked() {
                    rv_core::settings::set_rule(app.active_dat_rule.clone());
                    let settings = rv_core::settings::get_settings();
                    let _ = rv_core::settings::write_settings_to_file(&settings);
                    app.global_settings = rv_core::settings::get_settings();
                    app.active_dat_rule = rv_core::settings::find_rule(&app.active_dat_rule.dir_key);
                    app.task_logs.push(format!(
                        "Applied Directory Settings for {}",
                        app.active_dat_rule.dir_key
                    ));
                    close_dir_settings = true;
                }

                let can_delete = app.global_settings.dat_rules.items.iter().any(|r| {
                    let rule_key = normalize_rule_key(&r.dir_key);
                    #[cfg(windows)]
                    {
                        rule_key.eq_ignore_ascii_case(&active_key)
                    }
                    #[cfg(not(windows))]
                    {
                        rule_key == active_key
                    }
                });
                if ui
                    .add_enabled(app.dir_settings_compact && can_delete, egui::Button::new("Delete Rule"))
                    .clicked()
                {
                    rv_core::settings::delete_rule(&app.active_dat_rule.dir_key);
                    let settings = rv_core::settings::get_settings();
                    let _ = rv_core::settings::write_settings_to_file(&settings);
                    app.global_settings = settings;
                    app.active_dat_rule = rv_core::settings::find_rule(&app.active_dat_rule.dir_key);
                    app.task_logs.push(format!(
                        "Deleted Directory Rule for {}",
                        app.active_dat_rule.dir_key
                    ));
                    close_dir_settings = true;
                }
            });
        });

    if close_dir_settings {
        app.show_dir_settings = false;
    }
}
