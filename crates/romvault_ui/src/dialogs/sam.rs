use eframe::egui;

use crate::RomVaultApp;

use super::{SAM_INPUT_OPTIONS, SAM_OUTPUT_OPTIONS};

pub(super) fn draw_sam_dialog(app: &mut RomVaultApp, ctx: &egui::Context) {
    if !app.show_sam_dialog {
        return;
    }

    let mut close_sam = false;
    let mut sam_window_open = app.show_sam_dialog;
    egui::Window::new("Structured Archive Maker")
        .open(&mut sam_window_open)
        .resizable(true)
        .default_width(860.0)
        .default_height(560.0)
        .show(ctx, |ui| {
            ui.heading("Structured Archive Maker");
            ui.separator();
            ui.label("Convert directories and archives into normalized output formats using a workflow closer to the classic RomVault SAM tool.");
            ui.add_space(8.0);

            ui.columns(2, |columns| {
                columns[0].group(|ui| {
                    ui.heading("Source Files");
                    ui.separator();
                    ui.label("Files queued for conversion");
                    egui::ScrollArea::vertical()
                        .id_source("sam_source_files")
                        .max_height(220.0)
                        .show(ui, |ui| {
                            if app.sam_source_items.is_empty() {
                                ui.label("No source files added.");
                            } else {
                                for (idx, item) in app.sam_source_items.iter().enumerate() {
                                    let selected = app.sam_selected_source_idx == Some(idx);
                                    if ui.selectable_label(selected, item).clicked() {
                                        app.sam_selected_source_idx = Some(idx);
                                    }
                                }
                            }
                        });

                    ui.add_space(6.0);
                    ui.label("Add source path");
                    ui.text_edit_singleline(&mut app.sam_pending_source_path);
                    ui.horizontal(|ui| {
                        let pending = app.sam_pending_source_path.trim();
                        if ui
                            .add_enabled(!pending.is_empty(), egui::Button::new("Add"))
                            .clicked()
                        {
                            if !app
                                .sam_source_items
                                .iter()
                                .any(|item| item.eq_ignore_ascii_case(pending))
                            {
                                app.sam_source_items.push(pending.to_string());
                                app.sam_selected_source_idx = Some(app.sam_source_items.len() - 1);
                            }
                            app.sam_pending_source_path.clear();
                        }
                        if ui
                            .add_enabled(
                                app.sam_selected_source_idx.is_some(),
                                egui::Button::new("Remove"),
                            )
                            .clicked()
                        {
                            if let Some(idx) = app.sam_selected_source_idx.take() {
                                if idx < app.sam_source_items.len() {
                                    app.sam_source_items.remove(idx);
                                }
                            }
                        }
                        if ui
                            .add_enabled(!app.sam_source_items.is_empty(), egui::Button::new("Clear"))
                            .clicked()
                        {
                            app.sam_source_items.clear();
                            app.sam_selected_source_idx = None;
                        }
                    });
                });

                columns[1].group(|ui| {
                    ui.heading("Options");
                    ui.separator();

                    ui.label("Input Type");
                    egui::ComboBox::from_id_source("sam_input_kind")
                        .selected_text(app.sam_input_kind.label())
                        .show_ui(ui, |ui| {
                            for option in SAM_INPUT_OPTIONS {
                                ui.selectable_value(
                                    &mut app.sam_input_kind,
                                    option,
                                    option.label(),
                                );
                            }
                        });

                    ui.label("Output Type");
                    egui::ComboBox::from_id_source("sam_output_kind")
                        .selected_text(app.sam_output_kind.label())
                        .show_ui(ui, |ui| {
                            for option in SAM_OUTPUT_OPTIONS {
                                let supported =
                                    crate::RomVaultApp::sam_output_kind_supported(option);
                                let label = if supported {
                                    option.label().to_string()
                                } else {
                                    format!("{} (Unavailable)", option.label())
                                };
                                ui.add_enabled_ui(supported, |ui| {
                                    ui.selectable_value(&mut app.sam_output_kind, option, label);
                                });
                            }
                        });

                    ui.add_space(8.0);
                    if ui
                        .checkbox(
                            &mut app.sam_use_origin_output,
                            "Use source location for output",
                        )
                        .changed()
                        && app.sam_use_origin_output
                    {
                        app.sam_output_directory.clear();
                    }
                    ui.label(if app.sam_use_origin_output {
                        "Output Directory (Disabled: origin output enabled)"
                    } else {
                        "Output Directory"
                    });
                    if app.sam_use_origin_output {
                        ui.add_enabled(
                            false,
                            egui::TextEdit::singleline(&mut app.sam_output_directory)
                                .hint_text("Disabled while using source location output"),
                        );
                    } else {
                        ui.text_edit_singleline(&mut app.sam_output_directory);
                    }

                    ui.add_space(8.0);
                    ui.checkbox(&mut app.sam_recurse_subdirs, "Recurse subdirectories");
                    ui.checkbox(
                        &mut app.sam_rebuild_existing,
                        "Rebuild archives even if output exists",
                    );
                    ui.checkbox(
                        &mut app.sam_remove_source,
                        "Remove source files after successful conversion",
                    );
                    ui.checkbox(&mut app.sam_verify_output, "Verify output after conversion");

                    ui.add_space(8.0);
                    ui.heading("Status");
                    ui.separator();
                    ui.label(format!("Queued items: {}", app.sam_source_items.len()));
                    ui.label(format!(
                        "Completed items: {}/{}",
                        app.sam_completed_items, app.sam_total_items
                    ));
                    ui.label(format!("Input profile: {}", app.sam_input_kind.label()));
                    ui.label(format!("Output profile: {}", app.sam_output_kind.label()));
                    ui.label(format!("Run state: {}", app.sam_status_text));
                    if let Some(current_item) = app.sam_current_item.as_ref() {
                        ui.label(format!("Current item: {}", current_item));
                    }
                    ui.label(if app.sam_use_origin_output {
                        "Output uses each source item's origin location"
                    } else if app.sam_output_directory.trim().is_empty() {
                        "Output directory not set"
                    } else {
                        "Output directory ready"
                    });
                    if let Some(message) =
                        crate::RomVaultApp::sam_output_kind_support_message(app.sam_output_kind)
                    {
                        ui.label(message);
                    }
                    ui.add_space(6.0);
                    ui.label("Soft stop finishes the current conversion and then stops. Hard stop aborts immediately and cleans up any remaining .samtmp files.");
                });
            });

            ui.separator();
            ui.horizontal(|ui| {
                let can_start = app.is_idle()
                    && !app.sam_source_items.is_empty()
                    && app.sam_has_usable_output_target()
                    && crate::RomVaultApp::sam_output_kind_supported(app.sam_output_kind);
                if ui.add_enabled(can_start, egui::Button::new("Start")).clicked() {
                    app.start_sam_job();
                }
                if ui
                    .add_enabled(
                        app.sam_running && !app.sam_soft_stop_requested,
                        egui::Button::new("Soft Stop"),
                    )
                    .clicked()
                {
                    app.request_sam_soft_stop();
                }
                if ui
                    .add_enabled(
                        app.sam_running && !app.sam_hard_stop_requested,
                        egui::Button::new("Hard Stop"),
                    )
                    .clicked()
                {
                    app.request_sam_hard_stop();
                }
                if ui.button("Close").clicked() {
                    close_sam = true;
                }
            });
        });
    app.show_sam_dialog = sam_window_open;
    if close_sam {
        app.show_sam_dialog = false;
    }
}
