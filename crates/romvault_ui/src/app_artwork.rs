use std::rc::Rc;

use crate::utils::{extract_image_from_zip, extract_text_from_zip, get_full_node_path};
use crate::RomVaultApp;

impl RomVaultApp {
    pub(crate) fn update_artwork(&mut self) {
        if let Some(game) = &self.selected_game {
            let game_name = &game.borrow().name;
            let game_base_name = std::path::Path::new(game_name)
                .file_stem()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();

            let full_path = get_full_node_path(Rc::clone(game));
            let full_path = rv_core::settings::find_dir_mapping(&full_path).unwrap_or(full_path);
            let dir_path = std::path::Path::new(&full_path)
                .parent()
                .unwrap_or_else(|| std::path::Path::new(""))
                .to_string_lossy()
                .to_string();

            if self.last_selected_game_path != full_path {
                self.last_selected_game_path = full_path.clone();

                let art_zip = format!("{}\\{}", dir_path, "artpreview.zip");
                self.loaded_artwork = extract_image_from_zip(&art_zip, &game_base_name);

                let logo_zip = format!("{}\\{}", dir_path, "marquees.zip");
                self.loaded_logo = extract_image_from_zip(&logo_zip, &game_base_name);

                let title_zip = format!("{}\\{}", dir_path, "cabinets.zip");
                self.loaded_title = extract_image_from_zip(&title_zip, &game_base_name);

                let screen_zip = format!("{}\\{}", dir_path, "snap.zip");
                self.loaded_screen = extract_image_from_zip(&screen_zip, &game_base_name);

                self.loaded_info = None;
                self.loaded_info_type = String::new();

                if let Some(nfo_text) = extract_text_from_zip(&full_path, ".nfo") {
                    self.loaded_info = Some(nfo_text);
                    self.loaded_info_type = "NFO".to_string();
                } else if let Some(diz_text) = extract_text_from_zip(&full_path, ".diz") {
                    self.loaded_info = Some(diz_text);
                    self.loaded_info_type = "DIZ".to_string();
                }
            }
        } else {
            self.loaded_artwork = None;
            self.loaded_logo = None;
            self.loaded_title = None;
            self.loaded_screen = None;
            self.loaded_info = None;
            self.loaded_info_type.clear();
            self.last_selected_game_path.clear();
        }
    }
}
