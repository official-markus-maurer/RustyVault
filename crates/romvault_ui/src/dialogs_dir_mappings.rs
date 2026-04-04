use eframe::egui;

use crate::RomVaultApp;

pub fn draw_dir_mappings(app: &mut RomVaultApp, ctx: &egui::Context) {
    if !app.show_dir_mappings {
        return;
    }

    let mut close_dir_mappings = false;
    egui::Window::new("Directory Mappings")
        .open(&mut app.show_dir_mappings)
        .resizable(true)
        .default_width(600.0)
        .show(ctx, |ui| {
            ui.heading("Directory Mappings");
            ui.separator();

            fn normalize_dir_key_key(dir_key: &str) -> String {
                dir_key.replace('/', "\\").trim_matches('\\').to_string()
            }

            fn is_base_mapping_key(dir_key: &str) -> bool {
                let key = normalize_dir_key_key(dir_key);
                #[cfg(windows)]
                {
                    key.eq_ignore_ascii_case("RustyVault") || key.eq_ignore_ascii_case("ToSort")
                }
                #[cfg(not(windows))]
                {
                    key == "RustyVault" || key == "ToSort"
                }
            }

            let selected_key = app
                .selected_dir_mapping_idx
                .and_then(|idx| app.working_dir_mappings.get(idx))
                .map(|m| normalize_dir_key_key(&m.dir_key));

            let has_invalid_paths = app.working_dir_mappings.iter().any(|m| {
                let p = m.dir_path.trim();
                !p.is_empty() && !std::path::Path::new(p).exists()
            });
            if has_invalid_paths {
                ui.colored_label(
                    egui::Color32::from_rgb(200, 0, 0),
                    "One or more mapping paths do not exist. Fix them before saving.",
                );
                ui.add_space(6.0);
            }

            let table = egui_extras::TableBuilder::new(ui)
                .striped(true)
                .resizable(true)
                .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
                .column(egui_extras::Column::initial(200.0).at_least(100.0))
                .column(egui_extras::Column::remainder())
                .min_scrolled_height(200.0);

            table
                .header(24.0, |mut header| {
                    header.col(|ui| {
                        ui.strong("Directory");
                    });
                    header.col(|ui| {
                        ui.strong("Mapping");
                    });
                })
                .body(|mut body| {
                    let mappings_len = app.working_dir_mappings.len();
                    for i in 0..mappings_len {
                        let is_selected = app.selected_dir_mapping_idx == Some(i);
                        let dir_key = app.working_dir_mappings[i].dir_key.clone();
                        let mapping_path = app.working_dir_mappings[i].dir_path.clone();
                        let normalized_key = normalize_dir_key_key(&dir_key);
                        let is_tosort = {
                            #[cfg(windows)]
                            {
                                normalized_key.eq_ignore_ascii_case("ToSort")
                            }
                            #[cfg(not(windows))]
                            {
                                normalized_key == "ToSort"
                            }
                        };
                        let is_current = selected_key.as_ref().is_some_and(|k| {
                            #[cfg(windows)]
                            {
                                k.eq_ignore_ascii_case(&normalized_key)
                            }
                            #[cfg(not(windows))]
                            {
                                k == &normalized_key
                            }
                        });
                        let is_child = selected_key.as_ref().is_some_and(|k| {
                            if k.is_empty() || normalized_key.len() <= k.len() {
                                return false;
                            }
                            let prefix = format!("{}\\", k);
                            #[cfg(windows)]
                            {
                                normalized_key
                                    .to_ascii_lowercase()
                                    .starts_with(&prefix.to_ascii_lowercase())
                            }
                            #[cfg(not(windows))]
                            {
                                normalized_key.starts_with(&prefix)
                            }
                        });
                        let path_invalid = {
                            let p = mapping_path.trim();
                            !p.is_empty() && !std::path::Path::new(p).exists()
                        };

                        let row_color = if path_invalid {
                            egui::Color32::from_rgb(255, 214, 214)
                        } else if is_tosort {
                            egui::Color32::from_rgb(255, 214, 255)
                        } else if is_current {
                            egui::Color32::from_rgb(214, 255, 214)
                        } else if is_child {
                            egui::Color32::from_rgb(255, 255, 214)
                        } else {
                            egui::Color32::TRANSPARENT
                        };

                        body.row(26.0, |mut row| {
                            row.col(|ui| {
                                if row_color != egui::Color32::TRANSPARENT {
                                    ui.painter().rect_filled(ui.max_rect(), 0.0, row_color);
                                } else if is_selected {
                                    ui.painter().rect_filled(
                                        ui.max_rect(),
                                        0.0,
                                        ui.visuals().selection.bg_fill.linear_multiply(0.3),
                                    );
                                }
                                let mut key = app.working_dir_mappings[i].dir_key.clone();
                                let response = ui.add(
                                    egui::TextEdit::singleline(&mut key)
                                        .desired_width(ui.available_width())
                                        .frame(false),
                                );
                                if response.changed() {
                                    app.working_dir_mappings[i].dir_key = key;
                                }
                                if response.gained_focus() {
                                    app.selected_dir_mapping_idx = Some(i);
                                }
                            });
                            row.col(|ui| {
                                if row_color != egui::Color32::TRANSPARENT {
                                    ui.painter().rect_filled(ui.max_rect(), 0.0, row_color);
                                } else if is_selected {
                                    ui.painter().rect_filled(
                                        ui.max_rect(),
                                        0.0,
                                        ui.visuals().selection.bg_fill.linear_multiply(0.3),
                                    );
                                }
                                let mut path = app.working_dir_mappings[i].dir_path.clone();
                                let response = ui.add(
                                    egui::TextEdit::singleline(&mut path)
                                        .desired_width(ui.available_width())
                                        .frame(false),
                                );
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
                    app.working_dir_mappings
                        .push(rv_core::settings::DirMapping {
                            dir_key: "NewDirectory".to_string(),
                            dir_path: "NewMapping".to_string(),
                        });
                    app.selected_dir_mapping_idx = Some(app.working_dir_mappings.len() - 1);
                }
                let can_remove = app.selected_dir_mapping_idx.is_some_and(|idx| {
                    app.working_dir_mappings
                        .get(idx)
                        .is_some_and(|m| !is_base_mapping_key(&m.dir_key))
                });
                if ui
                    .add_enabled(can_remove, egui::Button::new("Remove"))
                    .clicked()
                {
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
                        let mut map: std::collections::BTreeMap<
                            String,
                            rv_core::settings::DirMapping,
                        > = std::collections::BTreeMap::new();

                        for m in app
                            .working_dir_mappings
                            .iter()
                            .filter(|m| !m.dir_key.trim().is_empty())
                        {
                            let mut key = normalize_dir_key_key(&m.dir_key);
                            #[cfg(windows)]
                            {
                                if key.eq_ignore_ascii_case("RustyVault") {
                                    key = "RustyVault".to_string();
                                } else if key.eq_ignore_ascii_case("ToSort") {
                                    key = "ToSort".to_string();
                                }
                            }
                            let mut path = m.dir_path.trim().to_string();
                            if key == "RustyVault" && path.is_empty() {
                                path = "RomRoot".to_string();
                            }
                            if key == "ToSort" && path.is_empty() {
                                path = "ToSort".to_string();
                            }

                            #[cfg(windows)]
                            let map_key = key.to_ascii_lowercase();
                            #[cfg(not(windows))]
                            let map_key = key.clone();

                            map.insert(
                                map_key,
                                rv_core::settings::DirMapping {
                                    dir_key: key,
                                    dir_path: path,
                                },
                            );
                        }

                        #[cfg(windows)]
                        let rustyvault_key = "rustyvault";
                        #[cfg(not(windows))]
                        let rustyvault_key = "RustyVault";
                        if !map.contains_key(rustyvault_key) {
                            #[cfg(windows)]
                            let map_key = "rustyvault".to_string();
                            #[cfg(not(windows))]
                            let map_key = "RustyVault".to_string();
                            map.insert(
                                map_key,
                                rv_core::settings::DirMapping {
                                    dir_key: "RustyVault".to_string(),
                                    dir_path: "RomRoot".to_string(),
                                },
                            );
                        }

                        #[cfg(windows)]
                        let tosort_key = "tosort";
                        #[cfg(not(windows))]
                        let tosort_key = "ToSort";
                        if !map.contains_key(tosort_key) {
                            #[cfg(windows)]
                            let map_key = "tosort".to_string();
                            #[cfg(not(windows))]
                            let map_key = "ToSort".to_string();
                            map.insert(
                                map_key,
                                rv_core::settings::DirMapping {
                                    dir_key: "ToSort".to_string(),
                                    dir_path: "ToSort".to_string(),
                                },
                            );
                        }

                        let mappings: Vec<rv_core::settings::DirMapping> =
                            map.into_values().collect();

                        for m in &mappings {
                            if m.dir_path.is_empty() {
                                continue;
                            }
                            if !std::path::Path::new(&m.dir_path).exists() {
                                app.task_logs.push(format!(
                                    "Directory mapping path does not exist: {} => {}",
                                    m.dir_key, m.dir_path
                                ));
                                return;
                            }
                        }

                        app.global_settings.dir_mappings.items = mappings;
                        rv_core::settings::update_settings(app.global_settings.clone());
                        let _ = rv_core::settings::write_settings_to_file(&app.global_settings);
                        rv_core::db::DB::check_create_root_dirs();
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
