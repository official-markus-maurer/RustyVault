use eframe::egui;

use crate::RomVaultApp;

pub(super) fn draw_rom_info_dialog(app: &mut RomVaultApp, ctx: &egui::Context) {
    if !app.show_rom_info {
        return;
    }

    let mut close_rom_info = false;
    egui::Window::new("Rom Occurrence list")
        .open(&mut app.show_rom_info)
        .show(ctx, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                for line in &app.rom_info_lines {
                    ui.label(line);
                }
            });

            ui.add_space(10.0);
            if ui.button("Close").clicked() {
                close_rom_info = true;
            }
        });
    if close_rom_info {
        app.show_rom_info = false;
        app.selected_rom_for_info = None;
        app.rom_info_lines.clear();
    }
}
