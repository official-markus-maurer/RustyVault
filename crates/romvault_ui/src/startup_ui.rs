use eframe::egui;

use crate::RomVaultApp;

pub fn draw_startup(app: &mut RomVaultApp, ctx: &egui::Context) -> bool {
    if !app.startup_active {
        return false;
    }

    let screen = ctx.screen_rect();
    egui::Window::new("Starting RustyVault")
        .collapsible(false)
        .resizable(false)
        .title_bar(true)
        .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
        .fixed_pos(egui::pos2(screen.center().x - 180.0, screen.center().y - 100.0))
        .show(ctx, |ui| {
            ui.vertical_centered(|ui| {
                ui.add(egui::Spinner::new().size(24.0));
                ui.add_space(8.0);
                ui.label(egui::RichText::new("Loading cache and preparing UI...").strong());
                ui.add_space(6.0);
                egui::ScrollArea::vertical().max_height(120.0).show(ui, |ui| {
                    ui.label(&app.startup_status);
                });
            });
        });

    if app.startup_phase == 0 {
        let cache_path = rv_core::Cache::cache_path();
        let has_cache = cache_path.exists();
        app.startup_status = if has_cache {
            format!("Loading cache: {}", cache_path.display())
        } else {
            "No cache found, creating default tree...".to_string()
        };
        app.startup_phase = 1;
        ctx.request_repaint();
        return true;
    }

    if app.startup_phase == 1 {
        let start = std::time::Instant::now();
        rv_core::db::init_db();
        let elapsed = start.elapsed();
        app.startup_status = format!("Database ready in {:?}", elapsed);
        app.startup_phase = 2;
        app.startup_done_at = Some(std::time::Instant::now());
        ctx.request_repaint();
        return true;
    }

    if app.startup_phase == 2 {
        if let Some(done_at) = app.startup_done_at {
            if done_at.elapsed().as_millis() >= 350 {
                app.startup_active = false;
            } else {
                ctx.request_repaint();
                return true;
            }
        } else {
            app.startup_active = false;
        }
    }

    false
}

