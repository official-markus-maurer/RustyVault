    use super::*;

    #[test]
    fn test_read_mess_xml_dat() {
        let xml = r#"<?xml version="1.0"?>
        <softwarelist name="testlist" description="A Test List">
            <software name="mygame" cloneof="parentgame">
                <description>My Game</description>
                <year>1999</year>
                <publisher>Tester</publisher>
                <part name="cart" interface="cartridge">
                    <dataarea name="rom" size="2048">
                        <rom name="rom1.bin" size="1024" crc="12345678" sha1="abcdef"/>
                        <rom size="1024" loadflag="continue"/>
                    </dataarea>
                    <diskarea name="disk">
                        <disk name="mydisk" sha1="987654"/>
                    </diskarea>
                </part>
            </software>
        </softwarelist>"#;

        let header = read_mess_xml_dat(xml, "test.dat").unwrap();
        assert_eq!(header.name, Some("testlist".to_string()));
        assert_eq!(header.description, Some("A Test List".to_string()));

        let games = &header.base_dir.children;
        assert_eq!(games.len(), 1);

        let game_node = &games[0];
        assert_eq!(game_node.name, "mygame");

        let game_dir = game_node.dir().unwrap();
        let game_meta = game_dir.d_game.as_ref().unwrap();
        assert_eq!(game_meta.clone_of, Some("parentgame".to_string()));
        assert_eq!(game_meta.description, Some("My Game".to_string()));
        assert_eq!(game_meta.year, Some("1999".to_string()));
        assert_eq!(game_meta.manufacturer, Some("Tester".to_string()));

        let files = &game_dir.children;
        assert_eq!(files.len(), 2);

        // Rom Check with continuation size
        let rom = files[0].file().unwrap();
        assert_eq!(files[0].name, "rom1.bin");
        assert_eq!(rom.size, Some(2048)); // 1024 + 1024 from continue flag
        assert_eq!(rom.crc, Some(hex::decode("12345678").unwrap()));

        // Disk Check
        let disk = files[1].file().unwrap();
        assert_eq!(files[1].name, "mydisk");
        assert_eq!(
            disk.sha1,
            Some(hex::decode("0000000000000000000000000000000000987654").unwrap())
        );
    }
