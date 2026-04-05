use eframe::egui;

use crate::RomVaultApp;

#[path = "dialogs_dir_mappings.rs"]
mod dialogs_dir_mappings;
#[path = "dialogs_dir_settings.rs"]
mod dialogs_dir_settings;
#[path = "dialogs_global_settings.rs"]
mod dialogs_global_settings;

#[path = "dialogs/about.rs"]
mod about;
#[path = "dialogs/color_key.rs"]
mod color_key;
#[path = "dialogs/rom_info.rs"]
mod rom_info;
#[path = "dialogs/sam.rs"]
mod sam;
#[path = "dialogs/sam_types.rs"]
mod sam_types;

#[cfg(test)]
pub(crate) use color_key::{color_key_entry_count, color_key_sections};
pub(crate) use sam_types::{SamInputKind, SamOutputKind, SAM_INPUT_OPTIONS, SAM_OUTPUT_OPTIONS};

pub fn draw_dialogs(app: &mut RomVaultApp, ctx: &egui::Context) {
    dialogs_dir_mappings::draw_dir_mappings(app, ctx);

    sam::draw_sam_dialog(app, ctx);
    color_key::draw_color_key_dialog(app, ctx);
    about::draw_about_dialog(app, ctx);
    rom_info::draw_rom_info_dialog(app, ctx);

    dialogs_dir_settings::draw_dir_settings(app, ctx);
    dialogs_global_settings::draw_global_settings(app, ctx);
}

#[cfg(test)]
#[path = "tests/dialogs_tests.rs"]
mod tests;
