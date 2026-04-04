use eframe::egui;

use crate::RomVaultApp;

impl eframe::App for RomVaultApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        crate::assets::set_dark_mode(ctx.style().visuals.dark_mode);
        if crate::startup_ui::draw_startup(self, ctx) {
            return;
        }
        self.poll_sam_worker();
        if self.sam_running {
            ctx.request_repaint_after(std::time::Duration::from_millis(100));
        }
        self.update_artwork();
        crate::top_menu::draw_top_menu(self, ctx);

        crate::dialogs::draw_dialogs(self, ctx);

        crate::toolbar::draw_left_toolbar(self, ctx);

        let dark_mode = ctx.style().visuals.dark_mode;
        let status_bar_fill = if dark_mode {
            egui::Color32::from_rgb(20, 20, 22)
        } else {
            ctx.style().visuals.faint_bg_color
        };
        let log_panel_fill = if dark_mode {
            egui::Color32::from_rgb(25, 25, 27)
        } else {
            ctx.style().visuals.panel_fill
        };
        let info_frame_fill = if dark_mode {
            egui::Color32::from_rgb(30, 30, 33)
        } else {
            egui::Color32::from_rgb(248, 248, 250)
        };
        let info_frame_stroke = if dark_mode {
            egui::Stroke::new(1.0, egui::Color32::from_rgb(45, 45, 50))
        } else {
            egui::Stroke::new(1.0, egui::Color32::from_rgb(214, 214, 220))
        };

        crate::status_bar::draw_status_bar(self, ctx, status_bar_fill);

        crate::log_panel::draw_log_panel(self, ctx, log_panel_fill);

        crate::left_panel::draw_left_panel(
            self,
            ctx,
            dark_mode,
            info_frame_fill,
            info_frame_stroke,
        );

        crate::right_panel::draw_right_panel(self, ctx);

        crate::central_panel::draw_central_panel(self, ctx);

        self.flush_db_cache_if_needed();
    }
}
