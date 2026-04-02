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
