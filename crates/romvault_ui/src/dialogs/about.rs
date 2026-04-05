use eframe::egui;

use crate::RomVaultApp;

pub(super) fn draw_about_dialog(app: &mut RomVaultApp, ctx: &egui::Context) {
    if !app.show_about {
        return;
    }

    let mut close_about = false;
    egui::Window::new("About RustyVault")
        .open(&mut app.show_about)
        .show(ctx, |ui| {
            let startup_path = std::env::current_exe()
                .ok()
                .and_then(|p| p.parent().map(|p| p.to_string_lossy().to_string()))
                .unwrap_or_else(|| {
                    std::env::current_dir()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .to_string()
                });

            ui.vertical_centered(|ui| {
                ui.heading("RustyVault");
                ui.label(format!("Version 3.6.1 : {}", startup_path));
                ui.add_space(10.0);
                ui.label("ROMVault3 is written by Gordon J.");
                ui.label("Forked/ported as RustyVault");
                ui.add_space(10.0);

                ui.horizontal(|ui| {
                    if ui.button("Website").clicked() {
                        let _ = std::process::Command::new("cmd")
                            .args(["/C", "start", "", "http://www.romvault.com/"])
                            .spawn();
                    }
                    if ui.button("PayPal").clicked() {
                        let _ = std::process::Command::new("cmd")
                            .args(["/C", "start", "", "http://paypal.me/romvault"])
                            .spawn();
                    }
                    if ui.button("Patreon").clicked() {
                        let _ = std::process::Command::new("cmd")
                            .args(["/C", "start", "", "https://www.patreon.com/romvault"])
                            .spawn();
                    }
                });

                ui.add_space(10.0);
                if ui.button("Close").clicked() {
                    close_about = true;
                }
            });
        });
    if close_about {
        app.show_about = false;
    }
}
