use eframe::egui;
use crate::{GLOBAL_DB, format_number, ui_missing_count, RomVaultApp};

pub fn draw_status_bar(app: &mut RomVaultApp, ctx: &egui::Context, fill: egui::Color32) {
    egui::TopBottomPanel::bottom("status_bar")
        .resizable(false)
        .min_height(24.0)
        .frame(egui::Frame::none().fill(fill).inner_margin(egui::Margin::symmetric(8.0, 4.0)))
        .show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label("RustyVault 3.6.1 (Rust Port)");
                ui.separator();

                let mut total_roms = 0;
                let mut total_missing = 0;

                GLOBAL_DB.with(|db_ref| {
                    if let Some(db) = db_ref.borrow().as_ref() {
                        if let Some(stats) = db.dir_root.borrow().cached_stats {
                            total_roms = stats.total_roms;
                            total_missing = ui_missing_count(&stats);
                        }
                    }
                });

                ui.label(format!("Total ROMs: {}", format_number(total_roms)));
                ui.separator();
                ui.label(format!("Missing ROMs: {}", format_number(total_missing)));

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if let Some(last_log) = app.task_logs.last() {
                        ui.label(last_log);
                    } else {
                        ui.label("Ready");
                    }
                });
            });
        });
}

