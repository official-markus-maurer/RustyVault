use eframe::egui;

use crate::RomVaultApp;

pub fn draw_log_panel(app: &mut RomVaultApp, ctx: &egui::Context, fill: egui::Color32) {
    egui::TopBottomPanel::bottom("log_panel")
        .resizable(true)
        .min_height(100.0)
        .frame(egui::Frame::none().inner_margin(8.0).fill(fill))
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.heading("Task Log");
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button("Clear").clicked() {
                        app.task_logs.clear();
                    }
                });
            });
            ui.separator();
            egui::ScrollArea::vertical().stick_to_bottom(true).show(ui, |ui| {
                for log in &app.task_logs {
                    ui.label(log);
                }
            });
        });
}

