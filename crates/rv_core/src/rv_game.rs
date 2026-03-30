use dat_reader::dat_store::DatGame;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum GameData {
    Id = 11,
    Description = 1,
    RomOf = 2,
    IsBios = 3,
    Sourcefile = 4,
    CloneOf = 5,
    CloneOfId = 24,
    SampleOf = 6,
    Board = 7,
    Year = 8,
    Manufacturer = 9,

    EmuArc = 10,
    Publisher = 12,
    Developer = 13,
    Genre = 14,
    SubGenre = 15,
    Ratings = 16,
    Score = 17,
    Players = 18,
    Enabled = 19,
    CRC = 20,
    RelatedTo = 21,
    Source = 22,

    Category = 23,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct GameMetaData {
    pub id: GameData,
    pub value: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RvGame {
    pub game_meta_data: Vec<GameMetaData>,
}

impl RvGame {
    pub fn new() -> Self {
        Self {
            game_meta_data: Vec::new(),
        }
    }

    pub fn from_description(description: &str) -> Self {
        let mut game = Self::new();
        game.add_data(GameData::Description, description);
        game
    }

    pub fn from_dat_game(d_game: &DatGame) -> Self {
        let mut game = Self::new();
        game.check_attribute(&d_game.id, GameData::Id);
        game.check_attribute(&d_game.description, GameData::Description);
        
        let category = if d_game.category.is_empty() {
            None
        } else {
            Some(d_game.category.join(" | "))
        };
        game.check_attribute(&category, GameData::Category);
        
        game.check_attribute(&d_game.rom_of, GameData::RomOf);
        game.check_attribute(&d_game.is_bios, GameData::IsBios);
        game.check_attribute(&d_game.source_file, GameData::Sourcefile);
        game.check_attribute(&d_game.clone_of, GameData::CloneOf);
        game.check_attribute(&d_game.clone_of_id, GameData::CloneOfId);
        game.check_attribute(&d_game.sample_of, GameData::SampleOf);
        game.check_attribute(&d_game.board, GameData::Board);
        game.check_attribute(&d_game.year, GameData::Year);
        game.check_attribute(&d_game.manufacturer, GameData::Manufacturer);

        if d_game.is_emu_arc {
            game.add_data(GameData::EmuArc, "yes");
            game.check_attribute(&d_game.publisher, GameData::Publisher);
            game.check_attribute(&d_game.developer, GameData::Developer);
            game.check_attribute(&d_game.genre, GameData::Genre);
            game.check_attribute(&d_game.sub_genre, GameData::SubGenre);
            game.check_attribute(&d_game.ratings, GameData::Ratings);
            game.check_attribute(&d_game.score, GameData::Score);
            game.check_attribute(&d_game.players, GameData::Players);
            game.check_attribute(&d_game.enabled, GameData::Enabled);
            game.check_attribute(&d_game.crc, GameData::CRC);
            game.check_attribute(&d_game.related_to, GameData::RelatedTo);
            game.check_attribute(&d_game.source, GameData::Source);
        }
        
        game
    }

    fn check_attribute(&mut self, source: &Option<String>, g_param: GameData) {
        if let Some(s) = source {
            if !s.trim().is_empty() {
                self.add_data(g_param, s);
            }
        }
    }

    pub fn add_data(&mut self, id: GameData, value: &str) {
        if let Some(meta) = self.game_meta_data.iter_mut().find(|m| m.id == id) {
            meta.value = value.to_string();
        } else {
            self.game_meta_data.push(GameMetaData {
                id,
                value: value.to_string(),
            });
        }
    }

    pub fn get_data(&self, id: GameData) -> Option<String> {
        self.game_meta_data.iter().find(|m| m.id == id).map(|m| m.value.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rvgame_data_mapping() {
        let mut game = RvGame {
            game_meta_data: Vec::new(),
        };

        // Test adding new data
        game.add_data(GameData::Id, "game_id_1");
        assert_eq!(game.get_data(GameData::Id), Some("game_id_1".to_string()));
        
        // Test updating existing data
        game.add_data(GameData::Id, "game_id_2");
        assert_eq!(game.get_data(GameData::Id), Some("game_id_2".to_string()));
        
        // Test non-existent data
        assert_eq!(game.get_data(GameData::Description), None);

        // Test check_attribute
        game.check_attribute(&Some("   ".to_string()), GameData::Description);
        assert_eq!(game.get_data(GameData::Description), None); // Should ignore whitespace

        game.check_attribute(&Some("My Description".to_string()), GameData::Description);
        assert_eq!(game.get_data(GameData::Description), Some("My Description".to_string()));
    }
}
