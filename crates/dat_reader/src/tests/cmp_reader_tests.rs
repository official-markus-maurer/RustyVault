    use super::*;

    #[test]
    fn test_cmp_dat_loader() {
        let cmp_data = r#"
clrmamepro (
    name "Test CMP Dat"
    description "A test DAT"
    version "1.0"
)

game (
    name mygame
    description "My Game"
    rom ( name rom1.bin size 1024 crc 12345678 sha1 abcdef )
    disk ( name mydisk sha1 987654 )
)
        "#;

        let header = read_cmp_dat(cmp_data, "test.dat").unwrap();
        assert_eq!(header.name, Some("Test CMP Dat".to_string()));
        assert_eq!(header.version, Some("1.0".to_string()));

        let games = &header.base_dir.children;
        assert_eq!(games.len(), 1);

        let game_node = &games[0];
        assert_eq!(game_node.name, "mygame");

        let game_dir = game_node.dir().unwrap();
        let game_meta = game_dir.d_game.as_ref().unwrap();
        assert_eq!(game_meta.description, Some("My Game".to_string()));

        let files = &game_dir.children;
        assert_eq!(files.len(), 2);

        // Rom Check
        let rom = files[0].file().unwrap();
        assert_eq!(files[0].name, "rom1.bin");
        assert_eq!(rom.size, Some(1024));
        assert_eq!(rom.crc, Some(hex::decode("12345678").unwrap()));

        // Disk Check
        let disk = files[1].file().unwrap();
        assert_eq!(files[1].name, "mydisk");
        assert_eq!(
            disk.sha1,
            Some(hex::decode("0000000000000000000000000000000000987654").unwrap())
        );
    }
