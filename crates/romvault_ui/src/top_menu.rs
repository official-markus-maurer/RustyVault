use eframe::egui;

use crate::RomVaultApp;

pub fn draw_top_menu(app: &mut RomVaultApp, ctx: &egui::Context) {
    egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
        egui::menu::bar(ui, |ui| {
            let is_idle = app.is_idle();
            ui.menu_button("File", |ui| {
                if ui
                    .add_enabled(is_idle, egui::Button::new("Add ToSort"))
                    .clicked()
                {
                    app.prompt_add_tosort();
                    ui.close_menu();
                }
                if ui.button("Exit").clicked() {
                    std::process::exit(0);
                }
            });
            ui.menu_button("Update DATs", |ui| {
                if ui
                    .add_enabled(is_idle, egui::Button::new("Update New DATs"))
                    .clicked()
                {
                    let is_shift = ui.input(|i| i.modifiers.shift);
                    app.update_dats(is_shift);
                    ui.close_menu();
                }
                if ui
                    .add_enabled(is_idle, egui::Button::new("Refresh All DATs"))
                    .clicked()
                {
                    app.update_dats(true);
                    ui.close_menu();
                }
            });
            ui.menu_button("Scan ROMs", |ui| {
                if ui
                    .add_enabled(is_idle, egui::Button::new("Scan Quick (Headers Only)"))
                    .clicked()
                {
                    app.launch_scan_roms_task(
                        "Scan ROMs (Quick)",
                        "Scanning selected ROM roots (Headers Only)...",
                        rv_core::settings::EScanLevel::Level1,
                    );
                    ui.close_menu();
                }
                if ui.add_enabled(is_idle, egui::Button::new("Scan")).clicked() {
                    app.launch_scan_roms_task(
                        "Scan ROMs",
                        "Scanning selected ROM roots...",
                        rv_core::settings::EScanLevel::Level2,
                    );
                    ui.close_menu();
                }
                if ui
                    .add_enabled(is_idle, egui::Button::new("Scan Full (Complete Re-Scan)"))
                    .clicked()
                {
                    app.launch_scan_roms_task(
                        "Scan ROMs (Full)",
                        "Scanning selected ROM roots (Full Rescan)...",
                        rv_core::settings::EScanLevel::Level3,
                    );
                    ui.close_menu();
                }
            });
            ui.menu_button("Find Fixes", |ui| {
                if ui
                    .add_enabled(is_idle, egui::Button::new("Find Fixes"))
                    .clicked()
                {
                    app.launch_task("Find Fixes", |tx| {
                        let _ = tx.send("Running FindFixes...".to_string());
                        crate::GLOBAL_DB.with(|db_ref| {
                            if let Some(db) = db_ref.borrow().as_ref() {
                                rv_core::find_fixes::FindFixes::scan_files(std::rc::Rc::clone(
                                    &db.dir_root,
                                ));
                                db.dir_root.borrow_mut().cached_stats = None;
                            }
                        });
                    });
                    ui.close_menu();
                }
            });
            ui.menu_button("Fix ROMs", |ui| {
                if ui
                    .add_enabled(is_idle, egui::Button::new("Fix ROMs"))
                    .clicked()
                {
                    app.launch_fix_roms_task();
                    ui.close_menu();
                }
                if ui
                    .add_enabled(is_idle, egui::Button::new("Scan / Find Fix / Fix"))
                    .clicked()
                {
                    app.launch_scan_find_fix_fix_task();
                    ui.close_menu();
                }
            });
            ui.menu_button("Reports", |ui| {
                if ui
                    .add_enabled(is_idle, egui::Button::new("Fix Dat Report"))
                    .clicked()
                {
                    app.prompt_fixdat_report(true);
                    ui.close_menu();
                }
                if ui
                    .add_enabled(is_idle, egui::Button::new("Full Report"))
                    .clicked()
                {
                    app.prompt_full_report();
                    ui.close_menu();
                }
                if ui
                    .add_enabled(is_idle, egui::Button::new("Fix Report"))
                    .clicked()
                {
                    app.prompt_fix_report();
                    ui.close_menu();
                }
                if ui
                    .add_enabled(is_idle, egui::Button::new("Full DAT Export"))
                    .clicked()
                {
                    app.prompt_fixdat_report(false);
                    ui.close_menu();
                }
            });
            ui.menu_button("Settings", |ui| {
                if ui
                    .add_enabled(is_idle, egui::Button::new("RustyVault Settings"))
                    .clicked()
                {
                    app.global_settings = rv_core::settings::get_settings();
                    app.show_settings = true;
                    ui.close_menu();
                }
                if ui
                    .add_enabled(is_idle, egui::Button::new("Directory Settings"))
                    .clicked()
                {
                    let dir_key = if let Some(node) = app.selected_node.as_ref() {
                        let n = node.borrow();
                        let logical = if n.is_directory() {
                            n.get_logical_name()
                        } else {
                            n.get_parent()
                                .map(|p| p.borrow().get_logical_name())
                                .unwrap_or_else(|| "RustyVault".to_string())
                        };
                        if logical.is_empty() {
                            "RustyVault".to_string()
                        } else {
                            logical
                        }
                    } else {
                        "RustyVault".to_string()
                    };

                    app.active_dat_rule = rv_core::settings::find_rule(&dir_key);
                    app.show_dir_settings = true;
                    ui.close_menu();
                }
                if ui
                    .add_enabled(is_idle, egui::Button::new("Directory Mappings"))
                    .clicked()
                {
                    app.open_dir_mappings();
                    ui.close_menu();
                }
                ui.separator();
                if ui.button("Toggle Dark Mode").clicked() {
                    app.task_logs.push("Toggled Dark Mode".to_string());
                    let mut ctx_style = (*ui.ctx().style()).clone();
                    if ctx_style.visuals.dark_mode {
                        ctx_style.visuals = egui::Visuals::light();
                        app.global_settings.darkness = false;
                    } else {
                        ctx_style.visuals = egui::Visuals::dark();
                        app.global_settings.darkness = true;
                    }
                    ui.ctx().set_style(ctx_style);
                    rv_core::settings::update_settings(app.global_settings.clone());
                    let _ = rv_core::settings::write_settings_to_file(&app.global_settings);
                    ui.close_menu();
                }
            });
            ui.menu_button("Add ToSort", |ui| {
                if ui
                    .add_enabled(is_idle, egui::Button::new("Add ToSort"))
                    .clicked()
                {
                    app.prompt_add_tosort();
                    ui.close_menu();
                }
            });
            ui.menu_button("Help", |ui| {
                if ui.button("Structured Archive Maker").clicked() {
                    app.show_sam_dialog = true;
                    ui.close_menu();
                }
                if ui.button("Color Key").clicked() {
                    app.show_color_key = true;
                    ui.close_menu();
                }
                if cfg!(debug_assertions) && ui.button("Garbage Collect").clicked() {
                    app.garbage_collect(ctx);
                    ui.close_menu();
                }
                if ui.button("Whats New").clicked() {
                    app.task_logs.push("Opening Whats New Wiki...".to_string());
                    let _ = std::process::Command::new("cmd")
                        .args([
                            "/C",
                            "start",
                            "https://wiki.romvault.com/doku.php?id=whats_new",
                        ])
                        .spawn();
                    ui.close_menu();
                }
                if ui.button("Visit Help Wiki").clicked() {
                    app.task_logs.push("Opening Help Wiki...".to_string());
                    let _ = std::process::Command::new("cmd")
                        .args(["/C", "start", "https://wiki.romvault.com/doku.php?id=help"])
                        .spawn();
                    ui.close_menu();
                }
                if ui.button("About RustyVault").clicked() {
                    app.show_about = true;
                    ui.close_menu();
                }
            });

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.small_button("Patreon").clicked() {
                    let _ = std::process::Command::new("cmd")
                        .args(["/C", "start", "", "https://www.patreon.com/romvault"])
                        .spawn();
                }
                if ui.small_button("PayPal").clicked() {
                    let _ = std::process::Command::new("cmd")
                        .args(["/C", "start", "", "http://paypal.me/romvault"])
                        .spawn();
                }
            });
        });
    });
}
