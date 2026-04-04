use super::*;

#[test]
fn test_read_rom_center_dat() {
    let dat = r#"[CREDITS]
Author=Tester
Version=1.00
[DAT]
split=true
merge=split
[GAMES]
¬parentgame¬desc1¬mygame¬MyGame¬rom1.bin¬12345678¬1024¬romofgame¬merge.bin¬
[DISKS]
¬parentgame¬desc1¬mygame¬MyGame¬mydisk¬9876543210abcdef¬size¬romof¬mergedisk¬"#;

    let header = read_rom_center_dat(dat, "test.dat").unwrap();
    assert_eq!(header.author, Some("Tester".to_string()));
    assert_eq!(header.split, Some("true".to_string()));

    let games = &header.base_dir.children;
    assert_eq!(games.len(), 1);

    let game_node = &games[0];
    assert_eq!(game_node.name, "mygame");

    let game_dir = game_node.dir().unwrap();
    let game_meta = game_dir.d_game.as_ref().unwrap();
    assert_eq!(game_meta.clone_of, Some("parentgame".to_string()));
    assert_eq!(game_meta.description, Some("MyGame".to_string()));

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
        Some(hex::decode("0000000000000000000000009876543210abcdef").unwrap())
    );
}
