use eframe::egui;
use std::rc::Rc;

use crate::RomVaultApp;

pub fn draw_left_panel(
    app: &mut RomVaultApp,
    ctx: &egui::Context,
    dark_mode: bool,
    info_frame_fill: egui::Color32,
    info_frame_stroke: egui::Stroke,
) {
    egui::SidePanel::left("left_panel")
        .resizable(true)
        .default_width(400.0)
        .frame(egui::Frame::none().inner_margin(8.0).fill(ctx.style().visuals.panel_fill))
        .show(ctx, |ui| {
            egui::Frame::none()
                .fill(info_frame_fill)
                .rounding(6.0)
                .stroke(info_frame_stroke)
                .inner_margin(8.0)
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    let text = if dark_mode {
                        egui::RichText::new("Dat Info")
                            .strong()
                            .color(egui::Color32::LIGHT_GRAY)
                    } else {
                        egui::RichText::new("Dat Info").strong()
                    };
                    ui.label(text);
                    ui.separator();

                    egui::Grid::new("dat_info_grid")
                        .num_columns(4)
                        .spacing([10.0, 4.0])
                        .min_col_width(50.0)
                        .show(ui, |ui| {
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                ui.label("Name:");
                            });
                            if let Some(node) = &app.selected_node {
                                if let Some(dat) = &node.borrow().dat {
                                    ui.label(
                                        dat.borrow()
                                            .get_data(rv_core::rv_dat::DatData::DatName)
                                            .unwrap_or_default(),
                                    );
                                } else {
                                    ui.label("");
                                }
                            } else {
                                ui.label("");
                            }

                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                ui.label("Version:");
                            });
                            if let Some(node) = &app.selected_node {
                                if let Some(dat) = &node.borrow().dat {
                                    ui.label(
                                        dat.borrow()
                                            .get_data(rv_core::rv_dat::DatData::Version)
                                            .unwrap_or_default(),
                                    );
                                } else {
                                    ui.label("");
                                }
                            } else {
                                ui.label("");
                            }
                            ui.end_row();

                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                ui.label("Description:");
                            });
                            if let Some(node) = &app.selected_node {
                                if let Some(dat) = &node.borrow().dat {
                                    ui.label(
                                        dat.borrow()
                                            .get_data(rv_core::rv_dat::DatData::Description)
                                            .unwrap_or_default(),
                                    );
                                } else {
                                    ui.label("");
                                }
                            } else {
                                ui.label("");
                            }

                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                ui.label("Date:");
                            });
                            if let Some(node) = &app.selected_node {
                                if let Some(dat) = &node.borrow().dat {
                                    ui.label(
                                        dat.borrow()
                                            .get_data(rv_core::rv_dat::DatData::Date)
                                            .unwrap_or_default(),
                                    );
                                } else {
                                    ui.label("");
                                }
                            } else {
                                ui.label("");
                            }
                            ui.end_row();

                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                ui.label("Category:");
                            });
                            if let Some(node) = &app.selected_node {
                                if let Some(dat) = &node.borrow().dat {
                                    ui.label(
                                        dat.borrow()
                                            .get_data(rv_core::rv_dat::DatData::Category)
                                            .unwrap_or_default(),
                                    );
                                } else {
                                    ui.label("");
                                }
                            } else {
                                ui.label("");
                            }
                            ui.label("");
                            ui.label("");
                            ui.end_row();

                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                ui.label("Author:");
                            });
                            if let Some(node) = &app.selected_node {
                                if let Some(dat) = &node.borrow().dat {
                                    ui.label(
                                        dat.borrow()
                                            .get_data(rv_core::rv_dat::DatData::Author)
                                            .unwrap_or_default(),
                                    );
                                } else {
                                    ui.label("");
                                }
                            } else {
                                ui.label("");
                            }
                            ui.label("");
                            ui.label("");
                            ui.end_row();

                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                ui.label("ROM Path:");
                            });
                            if let Some(node) = &app.selected_node {
                                if let Some(dat) = &node.borrow().dat {
                                    ui.label(
                                        dat.borrow()
                                            .get_data(rv_core::rv_dat::DatData::RootDir)
                                            .unwrap_or_default(),
                                    );
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
                .fill(info_frame_fill)
                .rounding(6.0)
                .stroke(info_frame_stroke)
                .inner_margin(8.0)
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    let text = if dark_mode {
                        egui::RichText::new("Tree Status")
                            .strong()
                            .color(egui::Color32::LIGHT_GRAY)
                    } else {
                        egui::RichText::new("Tree Status").strong()
                    };
                    ui.label(text);
                    ui.separator();
                    let mut got = 0;
                    let mut missing = 0;
                    let mut fixable = 0;
                    let mut unknown = 0;

                    if let Some(node_rc) = &app.selected_node {
                        let node = node_rc.borrow();
                        if let Some(stats) = &node.cached_stats {
                            got = stats.count_correct();
                            missing = crate::ui_missing_count(stats);
                            fixable = crate::ui_fixable_count(stats);
                            unknown = stats.roms_unknown;
                        } else {
                            drop(node);
                            let mut stats = rv_core::repair_status::RepairStatus::new();
                            stats.report_status(Rc::clone(node_rc));
                            let mut node_mut = node_rc.borrow_mut();
                            node_mut.cached_stats = Some(stats);

                            got = stats.count_correct();
                            missing = crate::ui_missing_count(&stats);
                            fixable = crate::ui_fixable_count(&stats);
                            unknown = stats.roms_unknown;
                        }
                    }

                    egui::Grid::new("tree_status_grid").num_columns(4).show(ui, |ui| {
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.label("ROMs Got:");
                        });
                        ui.label(crate::format_number(got));
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.label("ROMs Missing:");
                        });
                        ui.label(crate::format_number(missing));
                        ui.end_row();

                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.label("ROMs Fixable:");
                        });
                        ui.label(crate::format_number(fixable));
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.label("ROMs Unknown:");
                        });
                        ui.label(crate::format_number(unknown));
                        ui.end_row();
                    });
                });

            ui.add_space(2.0);
            ui.add_space(5.0);

            app.process_tree_stats_queue(ctx);
            if app.tree_rows_dirty {
                app.rebuild_tree_rows_cache();
            }

            let row_height = 18.0;
            let row_count = app.tree_rows_cache.len();
            egui::ScrollArea::vertical().show_rows(ui, row_height, row_count, |ui, range| {
                for idx in range {
                    let row = app.tree_rows_cache[idx].clone();
                    app.draw_tree_row(ui, &row);
                }
            });
        });
}
