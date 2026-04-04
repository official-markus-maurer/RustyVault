use eframe::egui;

use crate::RomVaultApp;

pub fn draw_central_panel(app: &mut RomVaultApp, ctx: &egui::Context) {
    egui::CentralPanel::default()
        .frame(
            egui::Frame::none()
                .fill(ctx.style().visuals.panel_fill)
                .inner_margin(8.0),
        )
        .show(ctx, |ui| {
            egui::TopBottomPanel::top("info_and_filters_panel")
                .resizable(false)
                .exact_height(180.0)
                .frame(egui::Frame::none())
                .show_inside(ui, |ui| {
                    crate::panels::draw_info_and_filters(app, ui);
                });

            ui.add_space(8.0);

            egui::TopBottomPanel::top("game_grid_panel")
                .resizable(true)
                .min_height(200.0)
                .max_height(ui.available_height() * 0.6)
                .frame(egui::Frame::none())
                .show_inside(ui, |ui| {
                    app.draw_game_grid(ui);
                });

            ui.add_space(8.0);

            egui::CentralPanel::default()
                .frame(egui::Frame::none())
                .show_inside(ui, |ui| {
                    app.draw_rom_grid(ui);
                });
        });
}
