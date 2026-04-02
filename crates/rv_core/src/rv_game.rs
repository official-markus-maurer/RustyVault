use dat_reader::dat_store::DatGame;

/// Identifiers for standard DAT Game fields
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize)]
pub enum GameData {
    /// Internal DB ID
    Id = 11,
    /// Game description
    Description = 1,
    /// Primary parent ROM
    RomOf = 2,
    /// Is this a BIOS set
    IsBios = 3,
    /// Source file name
    Sourcefile = 4,
    /// Clone parent
    CloneOf = 5,
    /// Internal clone ID
    CloneOfId = 24,
    /// Sample parent
    SampleOf = 6,
    /// Arcade board type
    Board = 7,
    /// Release year
    Year = 8,
    /// Game manufacturer
    Manufacturer = 9,

    /// EmuArc metadata
    EmuArc = 10,
    /// Game publisher
    Publisher = 12,
    /// Game developer
    Developer = 13,
    /// Primary genre
    Genre = 14,
    /// Sub-genre
    SubGenre = 15,
    /// Content rating
    Ratings = 16,
    /// Review score
    Score = 17,
    /// Number of players
    Players = 18,
    /// Is this game enabled
    Enabled = 19,
    /// Primary CRC
    CRC = 20,
    /// Related game ID
    RelatedTo = 21,
    /// Primary source
    Source = 22,

    /// Categorization
    Category = 23,
}

/// A key-value pair storing metadata for a Game
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct GameMetaData {
    /// The metadata key
    pub id: GameData,
    /// The metadata value
    pub value: String,
}

/// Logical representation of a Game set within a DAT.
/// 
/// `RvGame` maps the parsed `<game>` or `<machine>` XML nodes from a DAT file into 
/// the internal database. It holds metadata such as the game's Description, CloneOf 
/// relationships, Manufacturer, and Year.
/// 
/// Differences from C#:
/// - Similar to `RvDat`, the C# version utilizes an array-based string packing mechanism.
/// - The Rust version dynamically pushes to a `Vec<GameMetaData>` to drastically reduce
///   memory and serialization footprint, since most sets only use `Description`.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RvGame {
    /// List of key-value metadata pairs
    pub game_meta_data: Vec<GameMetaData>,
}

impl RvGame {
    /// Initializes an empty `RvGame` structure
    pub fn new() -> Self {
        Self {
            game_meta_data: Vec::new(),
        }
    }

    /// Initializes an `RvGame` directly from a description string
    pub fn from_description(description: &str) -> Self {
        let mut game = Self::new();
        game.add_data(GameData::Description, description);
        game
    }

    /// Converts a parsed `DatGame` AST node into an internal `RvGame` object
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

    /// Adds or updates a metadata string by its `GameData` key
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

    /// Retrieves a metadata string by its `GameData` key
    pub fn get_data(&self, id: GameData) -> Option<String> {
        self.game_meta_data.iter().find(|m| m.id == id).map(|m| m.value.clone())
    }
}

#[cfg(test)]
#[path = "tests/rv_game_tests.rs"]
mod tests;
