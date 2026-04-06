use crate::dat_store::{DatGame, DatHeader, DatNode, TRRNTZIP_DOS_DATETIME};
use crate::enums::FileType;
use crate::xml_writer::DatXmlWriter;

fn normalize_newlines(s: &str) -> String {
    s.replace("\r\n", "\n")
}

fn dos_datetime(year: i32, month: i32, day: i32, hour: i32, minute: i32, second: i32) -> i64 {
    let dos_date = ((year - 1980) << 9) | (month << 5) | day;
    let dos_time = (hour << 11) | (minute << 5) | (second / 2);
    ((dos_date as i64) << 16) | (dos_time as i64)
}

#[test]
fn test_write_dat_logiqx_matches_csharp_format() {
    let mut dat = DatHeader {
        name: Some("Test".to_string()),
        ..Default::default()
    };

    let mut game_dir = DatNode::new_dir("Game1".to_string(), FileType::Dir);
    {
        let d = game_dir.dir_mut().unwrap();
        let g = DatGame {
            id: Some("g1".to_string()),
            clone_of: Some("c1".to_string()),
            clone_of_id: Some("cid".to_string()),
            rom_of: Some("r1".to_string()),
            is_bios: Some("yes".to_string()),
            is_device: Some("yes".to_string()),
            runnable: Some("no".to_string()),
            category: vec!["cat".to_string()],
            description: Some("Desc".to_string()),
            year: Some("1980".to_string()),
            manufacturer: Some("Manu".to_string()),
            device_ref: vec!["dev1".to_string()],
            ..Default::default()
        };
        d.d_game = Some(Box::new(g));
    }

    let rom_dt = dos_datetime(2024, 1, 2, 3, 4, 6);

    let mut rom = DatNode::new_file("rom.bin".to_string(), FileType::File);
    rom.date_modified = Some(rom_dt);
    {
        let f = rom.file_mut().unwrap();
        f.size = Some(4);
        f.crc = Some(vec![0x12, 0x34, 0x56, 0x78]);
        f.sha1 = Some(vec![1; 20]);
        f.sha256 = Some(vec![2; 32]);
        f.md5 = Some(vec![3; 16]);
        f.status = Some("baddump".to_string());
        f.mia = Some("yes".to_string());
    }

    let mut disk = DatNode::new_file("disk.chd".to_string(), FileType::File);
    {
        let f = disk.file_mut().unwrap();
        f.is_disk = true;
        f.sha1 = Some(vec![4; 20]);
        f.merge = Some("merge.chd".to_string());
    }

    game_dir.dir_mut().unwrap().add_child(rom);
    game_dir.dir_mut().unwrap().add_child(disk);
    dat.base_dir.add_child(game_dir);

    let mut out = Vec::new();
    DatXmlWriter::write_dat(&mut out, &dat).unwrap();
    let got = normalize_newlines(&String::from_utf8(out).unwrap());

    let expected = normalize_newlines(
        "<?xml version=\"1.0\"?>\n\
<datafile>\n\
\t<header>\n\
\t\t<name>Test</name>\n\
\t\t<romvault/>\n\
\t</header>\n\
\t<game name=\"Game1\" id=\"g1\" cloneof=\"c1\" cloneofid=\"cid\" romof=\"r1\" isbios=\"yes\" isdevice=\"yes\" runnable=\"no\">\n\
\t\t<category>cat</category>\n\
\t\t<description>Desc</description>\n\
\t\t<year>1980</year>\n\
\t\t<manufacturer>Manu</manufacturer>\n\
\t\t<disk name=\"disk.chd\" sha1=\"0404040404040404040404040404040404040404\"/>\n\
\t\t<rom name=\"rom.bin\" size=\"4\" crc=\"12345678\" sha1=\"0101010101010101010101010101010101010101\" sha256=\"0202020202020202020202020202020202020202020202020202020202020202\" md5=\"03030303030303030303030303030303\" date=\"2024/01/02 03:04:06\" status=\"baddump\" mia=\"yes\"/>\n\
\t\t<device_ref name=\"dev1\"/>\n\
\t</game>\n\
</datafile>\n",
    );

    assert_eq!(got, expected);
}

#[test]
fn test_write_dat_newstyle_matches_csharp_format() {
    let mut dat = DatHeader {
        name: Some("Test".to_string()),
        ..Default::default()
    };

    let mut set_dir = DatNode::new_dir("Set1".to_string(), FileType::Zip);
    set_dir.date_modified = Some(7);
    {
        let d = set_dir.dir_mut().unwrap();
        let g = DatGame {
            id: Some("g1".to_string()),
            clone_of: Some("c1".to_string()),
            clone_of_id: Some("cid".to_string()),
            rom_of: Some("r1".to_string()),
            is_bios: Some("yes".to_string()),
            is_device: Some("yes".to_string()),
            runnable: Some("no".to_string()),
            category: vec!["cat".to_string()],
            description: Some("Desc".to_string()),
            year: Some("1980".to_string()),
            manufacturer: Some("Manu".to_string()),
            device_ref: vec!["dev1".to_string()],
            ..Default::default()
        };
        d.d_game = Some(Box::new(g));
    }

    let rom_dt = dos_datetime(2024, 1, 2, 3, 4, 6);

    let mut file = DatNode::new_file("rom.bin".to_string(), FileType::File);
    file.date_modified = Some(rom_dt);
    {
        let f = file.file_mut().unwrap();
        f.size = Some(4);
        f.crc = Some(vec![0x12, 0x34, 0x56, 0x78]);
        f.sha1 = Some(vec![1; 20]);
        f.sha256 = Some(vec![2; 32]);
        f.md5 = Some(vec![3; 16]);
        f.status = Some("baddump".to_string());
        f.mia = Some("yes".to_string());
    }

    let mut dir_marker = DatNode::new_file("empty/".to_string(), FileType::File);
    dir_marker.date_modified = Some(rom_dt);

    set_dir.dir_mut().unwrap().add_child(dir_marker);
    set_dir.dir_mut().unwrap().add_child(file);
    dat.base_dir.add_child(set_dir);

    let mut out = Vec::new();
    DatXmlWriter::write_dat_newstyle(&mut out, &dat).unwrap();
    let got = normalize_newlines(&String::from_utf8(out).unwrap());

    let expected = normalize_newlines(
        "<?xml version=\"1.0\"?>\n\
<RVDatFile>\n\
\t<header>\n\
\t\t<name>Test</name>\n\
\t\t<romvault/>\n\
\t</header>\n\
\t<set name=\"Set1\" type=\"trrntzip\" date=\"7\" id=\"g1\" cloneof=\"c1\" cloneofid=\"cid\" romof=\"r1\" isbios=\"yes\" isdevice=\"yes\" runnable=\"no\">\n\
\t\t<category>cat</category>\n\
\t\t<description>Desc</description>\n\
\t\t<year>1980</year>\n\
\t\t<manufacturer>Manu</manufacturer>\n\
\t\t<dir name=\"empty\" date=\"2024/01/02 03:04:06\"/>\n\
\t\t<file name=\"rom.bin\" size=\"4\" crc=\"12345678\" sha1=\"0101010101010101010101010101010101010101\" sha256=\"0202020202020202020202020202020202020202020202020202020202020202\" md5=\"03030303030303030303030303030303\" date=\"2024/01/02 03:04:06\" status=\"baddump\" mia=\"yes\"/>\n\
\t\t<device_ref name=\"dev1\"/>\n\
\t</set>\n\
</RVDatFile>\n",
    );

    assert_eq!(got, expected);
}

#[test]
fn test_write_mame_xml_matches_csharp_format() {
    let mut dat = DatHeader {
        mame_xml: true,
        name: Some("MAME 0.1".to_string()),
        ..Default::default()
    };

    let mut machine = DatNode::new_dir("machine1".to_string(), FileType::Dir);
    {
        let d = machine.dir_mut().unwrap();
        let g = DatGame {
            clone_of: Some("c1".to_string()),
            rom_of: Some("r1".to_string()),
            is_bios: Some("yes".to_string()),
            is_device: Some("yes".to_string()),
            runnable: Some("no".to_string()),
            description: Some(format!("Desc{}\u{7f}", "X")),
            ..Default::default()
        };
        d.d_game = Some(Box::new(g));
    }

    let mut rom = DatNode::new_file("rom.bin".to_string(), FileType::File);
    rom.date_modified = Some(TRRNTZIP_DOS_DATETIME);
    {
        let f = rom.file_mut().unwrap();
        f.merge = Some("merge.bin".to_string());
        f.size = Some(4);
        f.crc = Some(vec![0x12, 0x34, 0x56, 0x78]);
        f.sha1 = Some(vec![1; 20]);
        f.md5 = Some(vec![3; 16]);
        f.status = Some("good".to_string());
    }

    machine.dir_mut().unwrap().add_child(rom);
    dat.base_dir.add_child(machine);

    let mut out = Vec::new();
    DatXmlWriter::write_dat(&mut out, &dat).unwrap();
    let got = normalize_newlines(&String::from_utf8(out).unwrap());

    assert!(got.contains("<!DOCTYPE mame [\n"));
    assert!(got.contains("<mame build=\"MAME 0.1\">"));
    assert!(got.contains("\t<machine name=\"machine1\" cloneof=\"c1\" romof=\"r1\" isbios=\"yes\" isdevice=\"yes\" runnable=\"no\">"));
    assert!(got.contains("\t\t<description>DescX&#7f;</description>"));
    assert!(got.contains("\t\t<rom name=\"rom.bin\" merge=\"merge.bin\" size=\"4\" crc=\"12345678\" sha1=\"0101010101010101010101010101010101010101\" md5=\"03030303030303030303030303030303\"/>"));
}
