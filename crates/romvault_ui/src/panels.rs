use eframe::egui;
use crate::RomVaultApp;

pub fn draw_info_and_filters(app: &mut RomVaultApp, ui: &mut egui::Ui) {
    let panel_height = if app.show_filter_panel { 170.0 } else { 120.0 };
    egui::Frame::none()
        .fill(egui::Color32::from_rgb(30, 30, 33))
        .rounding(6.0)
        .stroke(egui::Stroke::new(1.0, egui::Color32::from_rgb(45, 45, 50)))
        .inner_margin(10.0)
        .show(ui, |ui| {
            ui.set_min_height(panel_height);

            let selected_name = app
                .selected_game
                .as_ref()
                .map(|g| g.borrow().name.clone())
                .unwrap_or_default();
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("Game").strong().color(egui::Color32::LIGHT_GRAY));
                if !selected_name.is_empty() {
                    ui.separator();
                    ui.add(
                        egui::Label::new(
                            egui::RichText::new(selected_name)
                                .color(egui::Color32::from_rgb(210, 210, 210)),
                        )
                        .wrap(true),
                    );
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let toggle_label = if app.show_filter_panel { "◀" } else { "▶" };
                    if ui.button(toggle_label).clicked() {
                        app.show_filter_panel = !app.show_filter_panel;
                    }
                });
            });

            ui.separator();

            let available_width = ui.available_width();
            let filters_width = if app.show_filter_panel {
                (available_width * 0.35).clamp(220.0, 320.0)
            } else {
                30.0
            };
            let info_width = (available_width - filters_width - 12.0).max(260.0);

            ui.horizontal(|ui| {
                ui.allocate_ui_with_layout(
                    egui::vec2(info_width, panel_height),
                    egui::Layout::top_down(egui::Align::LEFT),
                    |ui| {
                        let (clone_of, rom_of, category, manufacturer, year, publisher, developer, description) =
                            if let Some(game_node) = &app.selected_game {
                                if let Some(ref g) = game_node.borrow().game {
                                    let gg = g.borrow();
                                    (
                                        gg.get_data(rv_core::rv_game::GameData::CloneOf).unwrap_or_default(),
                                        gg.get_data(rv_core::rv_game::GameData::RomOf).unwrap_or_default(),
                                        gg.get_data(rv_core::rv_game::GameData::Category).unwrap_or_default(),
                                        gg.get_data(rv_core::rv_game::GameData::Manufacturer).unwrap_or_default(),
                                        gg.get_data(rv_core::rv_game::GameData::Year).unwrap_or_default(),
                                        gg.get_data(rv_core::rv_game::GameData::Publisher).unwrap_or_default(),
                                        gg.get_data(rv_core::rv_game::GameData::Developer).unwrap_or_default(),
                                        gg.get_data(rv_core::rv_game::GameData::Description).unwrap_or_default(),
                                    )
                                } else {
                                    (String::new(), String::new(), String::new(), String::new(), String::new(), String::new(), String::new(), String::new())
                                }
                            } else {
                                (String::new(), String::new(), String::new(), String::new(), String::new(), String::new(), String::new(), String::new())
                            };

                        egui::Grid::new("game_info_kv")
                            .num_columns(2)
                            .spacing([12.0, 6.0])
                            .min_col_width(110.0)
                            .show(ui, |ui| {
                                let label_color = egui::Color32::from_rgb(150, 150, 150);

                                ui.label(egui::RichText::new("Description").color(label_color));
                                ui.add(egui::Label::new(description).wrap(true));
                                ui.end_row();

                                ui.label(egui::RichText::new("Clone of").color(label_color));
                                ui.add(egui::Label::new(clone_of).wrap(true));
                                ui.end_row();

                                ui.label(egui::RichText::new("Rom of").color(label_color));
                                ui.add(egui::Label::new(rom_of).wrap(true));
                                ui.end_row();

                                ui.label(egui::RichText::new("Category").color(label_color));
                                ui.add(egui::Label::new(category).wrap(true));
                                ui.end_row();

                                ui.label(egui::RichText::new("Year").color(label_color));
                                ui.add(egui::Label::new(year).wrap(true));
                                ui.end_row();

                                ui.label(egui::RichText::new("Manufacturer").color(label_color));
                                ui.add(egui::Label::new(manufacturer).wrap(true));
                                ui.end_row();

                                ui.label(egui::RichText::new("Publisher").color(label_color));
                                ui.add(egui::Label::new(publisher).wrap(true));
                                ui.end_row();

                                ui.label(egui::RichText::new("Developer").color(label_color));
                                ui.add(egui::Label::new(developer).wrap(true));
                                ui.end_row();
                            });
                    },
                );

                ui.add_space(6.0);
                ui.add(egui::Separator::default().vertical());
                ui.add_space(6.0);

                ui.allocate_ui_with_layout(
                    egui::vec2(filters_width, panel_height),
                    egui::Layout::top_down(egui::Align::LEFT),
                    |ui| {
                        if app.show_filter_panel {
                            ui.horizontal(|ui| {
                                ui.label(egui::RichText::new("Filters").strong().color(egui::Color32::LIGHT_GRAY));
                                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                                    if ui.button("X").on_hover_text("Clear Filter Text").clicked() {
                                        app.filter_text.clear();
                                    }
                                });
                            });
                            ui.add_sized([ui.available_width(), 20.0], egui::TextEdit::singleline(&mut app.filter_text));
                            ui.add_space(6.0);

                            ui.columns(2, |columns| {
                                columns[0].checkbox(&mut app.show_complete, "Complete");
                                columns[0].checkbox(&mut app.show_partial, "Partial");
                                columns[0].checkbox(&mut app.show_empty, "Empty");
                                columns[1].checkbox(&mut app.show_fixes, "Fixes");
                                columns[1].checkbox(&mut app.show_mia, "MIA");
                                columns[1].checkbox(&mut app.show_merged, "Merged");
                            });
                        } else {
                            ui.vertical_centered(|ui| {
                                if ui.button("▶").on_hover_text("Show Filters").clicked() {
                                    app.show_filter_panel = true;
                                }
                            });
                        }
                    },
                );
            });
        });
}
