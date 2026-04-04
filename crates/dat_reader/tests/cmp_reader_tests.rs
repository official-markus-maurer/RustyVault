use dat_reader::cmp_reader::read_cmp_dat;

#[test]
fn test_parse_cmp_dat() {
    let cmp = r#"clrmamepro (
        name "Test CMP Dat"
        description "A Test CMP DAT File"
        version "1.0"
        author "RustyRoms"
    )

    game (
        name "TestGame"
        description "Test Game Description"
        rom ( name test.bin size 1024 crc 12345678 md5 0123456789abcdef0123456789abcdef )
        disk ( name test_disk sha1 0123456789abcdef0123456789abcdef01234567 )
    )"#;

    let dat_header = read_cmp_dat(cmp, "test.dat").expect("Failed to parse CMP");

    assert_eq!(dat_header.name.as_deref(), Some("Test CMP Dat"));
    assert_eq!(
        dat_header.description.as_deref(),
        Some("A Test CMP DAT File")
    );
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
}
