use eframe::egui;

use crate::RomVaultApp;

pub fn draw_right_panel(app: &mut RomVaultApp, ctx: &egui::Context) {
    let has_info = app.loaded_info.is_some();
    let has_artwork = app.loaded_logo.is_some() || app.loaded_artwork.is_some();
    let has_screens = app.loaded_title.is_some() || app.loaded_screen.is_some();
    let show_right_panel = has_info || has_artwork || has_screens;

    if !show_right_panel {
        app.active_game_info_tab = 0;
        return;
    }

    let fallback_tab = if has_info {
        0
    } else if has_artwork {
        1
    } else {
        2
    };
    match app.active_game_info_tab {
        0 if !has_info => app.active_game_info_tab = fallback_tab,
        1 if !has_artwork => app.active_game_info_tab = fallback_tab,
        2 if !has_screens => app.active_game_info_tab = fallback_tab,
        3.. => app.active_game_info_tab = fallback_tab,
        _ => {}
    }

    egui::SidePanel::right("tab_emu_arc_panel")
        .resizable(true)
        .default_width(220.0)
        .frame(egui::Frame::none().inner_margin(8.0).fill(ctx.style().visuals.panel_fill))
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                if has_info {
                    ui.selectable_value(
                        &mut app.active_game_info_tab,
                        0,
                        if app.loaded_info_type.is_empty() {
                            "Info"
                        } else {
                            &app.loaded_info_type
                        },
                    );
                }
                if has_artwork {
                    ui.selectable_value(&mut app.active_game_info_tab, 1, "Artwork");
                }
                if has_screens {
                    ui.selectable_value(&mut app.active_game_info_tab, 2, "Screens");
                }
            });
            ui.separator();

            if app.active_game_info_tab == 0 && has_info {
                egui::ScrollArea::both().show(ui, |ui| {
                    if let Some(info_text) = &app.loaded_info {
                        ui.label(egui::RichText::new(info_text).font(egui::FontId::monospace(12.0)));
                    }
                });
            } else if app.active_game_info_tab == 1 && has_artwork {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    ui.vertical(|ui| {
                        if app.loaded_logo.is_some() {
                            ui.label("Logo:");
                            ui.group(|ui| {
                                ui.set_min_height(100.0);
                                ui.centered_and_justified(|ui| {
                                    if let Some(bytes) = &app.loaded_logo {
                                        ui.add(
                                            egui::Image::from_bytes("bytes://logo", bytes.clone())
                                                .max_width(ui.available_width()),
                                        );
                                    }
                                });
                            });
                            ui.add_space(10.0);
                        }

                        if app.loaded_artwork.is_some() {
                            ui.label("Artwork:");
                            ui.group(|ui| {
                                ui.set_min_height(200.0);
                                ui.centered_and_justified(|ui| {
                                    if let Some(bytes) = &app.loaded_artwork {
                                        ui.add(
                                            egui::Image::from_bytes("bytes://artwork", bytes.clone())
                                                .max_width(ui.available_width()),
                                        );
                                    }
                                });
                            });
                        }
                    });
                });
            } else if app.active_game_info_tab == 2 && has_screens {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    ui.vertical(|ui| {
                        if app.loaded_title.is_some() {
                            ui.label("Title Screen:");
                            ui.group(|ui| {
                                ui.set_min_height(150.0);
                                ui.centered_and_justified(|ui| {
                                    if let Some(bytes) = &app.loaded_title {
                                        ui.add(
                                            egui::Image::from_bytes("bytes://title", bytes.clone())
                                                .max_width(ui.available_width()),
                                        );
                                    }
                                });
                            });
                            ui.add_space(10.0);
                        }

                        if app.loaded_screen.is_some() {
                            ui.label("Screenshot:");
                            ui.group(|ui| {
                                ui.set_min_height(150.0);
                                ui.centered_and_justified(|ui| {
                                    if let Some(bytes) = &app.loaded_screen {
                                        ui.add(
                                            egui::Image::from_bytes("bytes://screen", bytes.clone())
                                                .max_width(ui.available_width()),
                                        );
                                    }
                                });
                            });
                        }
                    });
                });
            }
        });
}
