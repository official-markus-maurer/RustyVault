use dat_reader::xml_reader::read_xml_dat;

#[test]
fn test_parse_xml_dat() {
    let xml = r#"<?xml version="1.0" encoding="UTF-8"?>
<datafile>
    <header>
        <name>Test Dat</name>
        <description>A Test DAT File</description>
        <version>1.0</version>
        <author>RustyRoms</author>
    </header>
    <game name="TestGame">
        <description>Test Game Description</description>
        <rom name="test.bin" size="1024" crc="12345678" md5="0123456789abcdef0123456789abcdef"/>
        <disk name="test_disk" sha1="0123456789abcdef0123456789abcdef01234567"/>
    </game>
</datafile>"#;

    let dat_header = read_xml_dat(xml, "test.dat").expect("Failed to parse XML");

    assert_eq!(dat_header.name.as_deref(), Some("Test Dat"));
    assert_eq!(dat_header.description.as_deref(), Some("A Test DAT File"));
    assert_eq!(dat_header.version.as_deref(), Some("1.0"));
    assert_eq!(dat_header.author.as_deref(), Some("RustyRoms"));

    let children = &dat_header.base_dir.children;
    assert_eq!(children.len(), 1);

    let game_node = &children[0];
    assert_eq!(game_node.name, "TestGame");
    assert!(game_node.is_dir());

    let game_dir = game_node.dir().unwrap();
    let d_game = game_dir.d_game.as_ref().unwrap();
    assert_eq!(d_game.description.as_deref(), Some("Test Game Description"));

    let roms = &game_dir.children;
    assert_eq!(roms.len(), 2);

    let rom = &roms[0];
    assert_eq!(rom.name, "test.bin");
    assert!(rom.is_file());
    let rom_file = rom.file().unwrap();
    assert_eq!(rom_file.size, Some(1024));
    assert_eq!(
        rom_file.crc.as_ref().unwrap(),
        &vec![0x12, 0x34, 0x56, 0x78]
    );

    let disk = &roms[1];
    assert_eq!(disk.name, "test_disk");
    assert!(disk.is_file());
    let disk_file = disk.file().unwrap();
    assert!(disk_file.is_disk);
    assert_eq!(
        disk_file.sha1.as_ref().unwrap(),
        &hex::decode("0123456789abcdef0123456789abcdef01234567").unwrap()
    );
}
