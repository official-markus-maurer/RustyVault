use eframe::egui;
use crate::RomVaultApp;

/// Logic for rendering the layout splitters and status panes.
/// 
/// `panels.rs` manages the structural layout of the main window, separating the left tree
/// from the right grids, and drawing the bottom status logs and progress bars.
/// 
/// Differences from C#:
/// - C# uses `SplitContainer` components configured via the Visual Studio designer.
/// - The Rust version dynamically calculates `egui::SidePanel` and `egui::CentralPanel`
///   widths to achieve a responsive split layout.
pub fn draw_info_and_filters(app: &mut RomVaultApp, ui: &mut egui::Ui) {
    fn group_box<H: FnOnce(&mut egui::Ui), B: FnOnce(&mut egui::Ui)>(
        ui: &mut egui::Ui,
        size: egui::Vec2,
        header: H,
        body: B,
    ) {
        let (rect, _) = ui.allocate_exact_size(size, egui::Sense::hover());
        ui.painter().rect(
            rect,
            egui::Rounding::same(6.0),
            egui::Color32::from_rgb(30, 30, 33),
            egui::Stroke::new(1.0, egui::Color32::from_rgb(45, 45, 50)),
        );

        let inner_rect = rect.shrink(8.0);
        ui.allocate_ui_at_rect(inner_rect, |ui| {
            ui.set_width(inner_rect.width());
            header(ui);
            ui.separator();
            body(ui);
        });
    }

    let panel_height = if app.show_filter_panel { 180.0 } else { 180.0 };

    let available_width = ui.available_width();
    let filters_width = if app.show_filter_panel {
        (available_width * 0.32).clamp(240.0, 340.0)
    } else {
        32.0
    };
    let info_width = (available_width - filters_width - 8.0).max(260.0);

    let selected_name = app
        .selected_game
        .as_ref()
        .map(|g| g.borrow().name.clone())
        .unwrap_or_default();

    let (clone_of, rom_of, category, manufacturer, year, publisher, developer, description) =
        if let Some(game_node) = &app.selected_game {
            if let Some(ref g) = game_node.borrow().game {
                let gg = g.borrow();
                (
                    gg.get_data(rv_core::rv_game::GameData::CloneOf)
                        .unwrap_or_default(),
                    gg.get_data(rv_core::rv_game::GameData::RomOf)
                        .unwrap_or_default(),
                    gg.get_data(rv_core::rv_game::GameData::Category)
                        .unwrap_or_default(),
                    gg.get_data(rv_core::rv_game::GameData::Manufacturer)
                        .unwrap_or_default(),
                    gg.get_data(rv_core::rv_game::GameData::Year)
                        .unwrap_or_default(),
                    gg.get_data(rv_core::rv_game::GameData::Publisher)
                        .unwrap_or_default(),
                    gg.get_data(rv_core::rv_game::GameData::Developer)
                        .unwrap_or_default(),
                    gg.get_data(rv_core::rv_game::GameData::Description)
                        .unwrap_or_default(),
                )
            } else {
                (
                    String::new(),
                    String::new(),
                    String::new(),
                    String::new(),
                    String::new(),
                    String::new(),
                    String::new(),
                    String::new(),
                )
            }
        } else {
            (
                String::new(),
                String::new(),
                String::new(),
                String::new(),
                String::new(),
                String::new(),
                String::new(),
                String::new(),
            )
        };

    ui.horizontal(|ui| {
        ui.allocate_ui_with_layout(
            egui::vec2(info_width, panel_height),
            egui::Layout::top_down(egui::Align::LEFT),
            |ui| {
                group_box(
                    ui,
                    egui::vec2(info_width, panel_height),
                    |ui| {
                        ui.label(
                            egui::RichText::new("Game")
                                .strong()
                                .color(egui::Color32::LIGHT_GRAY),
                        );
                    },
                    |ui| {
                        if !selected_name.is_empty() {
                            ui.label(selected_name);
                            ui.add_space(4.0);
                        }

                        egui::Grid::new("game_info_grid")
                            .num_columns(4)
                            .spacing([10.0, 8.0])
                            .min_col_width(50.0)
                            .show(ui, |ui| {
                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |ui| {
                                        ui.label("Description:");
                                    },
                                );
                                ui.label(description);
                                ui.label("");
                                ui.label("");
                                ui.end_row();

                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |ui| {
                                        ui.label("Clone of:");
                                    },
                                );
                                ui.label(clone_of);
                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |ui| {
                                        ui.label("ROM of:");
                                    },
                                );
                                ui.label(rom_of);
                                ui.end_row();

                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |ui| {
                                        ui.label("Category:");
                                    },
                                );
                                ui.label(category);
                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |ui| {
                                        ui.label("Year:");
                                    },
                                );
                                ui.label(year);
                                ui.end_row();

                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |ui| {
                                        ui.label("Manufacturer:");
                                    },
                                );
                                ui.label(manufacturer);
                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |ui| {
                                        ui.label("Publisher:");
                                    },
                                );
                                ui.label(publisher);
                                ui.end_row();

                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |ui| {
                                        ui.label("Developer:");
                                    },
                                );
                                ui.label(developer);
                                ui.label("");
                                ui.label("");
                                ui.end_row();
                            });
                    },
                );
            },
        );

        ui.add_space(8.0);

        let mut toggle_filter = false;
        let mut clear_filter = false;

        ui.allocate_ui_with_layout(
            egui::vec2(filters_width, panel_height),
            egui::Layout::top_down(egui::Align::LEFT),
            |ui| {
                group_box(
                    ui,
                    egui::vec2(filters_width, panel_height),
                    |ui| {
                        if app.show_filter_panel {
                            ui.horizontal(|ui| {
                                ui.label(
                                    egui::RichText::new("Filters")
                                        .strong()
                                        .color(egui::Color32::LIGHT_GRAY),
                                );
                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |ui| {
                                        if ui.button("◀").clicked() {
                                            toggle_filter = true;
                                        }
                                        if ui.button("X").clicked() {
                                            clear_filter = true;
                                        }
                                    },
                                );
                            });
                        } else {
                            ui.vertical_centered(|ui| {
                                if ui.button("▶").clicked() {
                                    toggle_filter = true;
                                }
                            });
                        }
                    },
                    |ui| {
                        if app.show_filter_panel {
                            ui.add_sized(
                                [ui.available_width(), 20.0],
                                egui::TextEdit::singleline(&mut app.filter_text),
                            );
                            ui.add_space(10.0);

                            ui.columns(2, |columns| {
                                columns[0].checkbox(&mut app.show_complete, "Complete");
                                columns[0].add_space(6.0);
                                columns[0].checkbox(&mut app.show_partial, "Partial");
                                columns[0].add_space(6.0);
                                columns[0].checkbox(&mut app.show_empty, "Empty");
                                columns[1].checkbox(&mut app.show_fixes, "Fixes");
                                columns[1].add_space(6.0);
                                columns[1].checkbox(&mut app.show_mia, "MIA");
                                columns[1].add_space(6.0);
                                columns[1].checkbox(&mut app.show_merged, "Merged");
                            });
                        } else {
                            ui.vertical_centered(|ui| {
                                ui.add_space(2.0);
                            });
                        }
                    },
                );
            },
        );

        if toggle_filter {
            app.show_filter_panel = !app.show_filter_panel;
        }
        if clear_filter {
            app.filter_text.clear();
        }
    });
}
