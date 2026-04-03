use crate::dat_store::{DatGame, DatHeader, DatNode};
use crate::enums::{FileType, ZipStructure};
use crate::json_writer::DatJsonWriter;

#[test]
fn test_json_writer_emits_expected_shape_for_torrentzip_game() {
    let mut header = DatHeader {
        name: Some("TestDat".to_string()),
        ..Default::default()
    };

    let mut game = DatNode::new_dir("Game1".to_string(), FileType::Zip);
    {
        let dir = game.dir_mut().unwrap();
        dir.set_dat_struct(ZipStructure::ZipTrrnt, false);
        dir.d_game = Some(Box::new(DatGame {
            description: Some("Desc".to_string()),
            year: Some("1999".to_string()),
            manufacturer: Some("Acme".to_string()),
            ..Default::default()
        }));

        let mut file = DatNode::new_file("a.bin".to_string(), FileType::File);
        {
            let f = file.file_mut().unwrap();
            f.size = Some(4);
            f.crc = Some(vec![0x12, 0x34, 0x56, 0x78]);
            f.sha1 = Some(vec![0x00, 0x11]);
        }
        dir.children.push(file);
    }
    header.base_dir.children.push(game);

    let mut out = Vec::new();
    DatJsonWriter::write_dat(&mut out, &header, false).unwrap();
    let v: serde_json::Value = serde_json::from_slice(&out).unwrap();

    assert_eq!(v["Header"]["name"], "TestDat");
    assert_eq!(v["root"].as_array().unwrap().len(), 1);
    assert_eq!(v["root"][0]["name"], "Game1");
    assert_eq!(v["root"][0]["type"], "trrntzip");
    assert_eq!(v["root"][0]["objects"].as_array().unwrap().len(), 1);
    assert_eq!(v["root"][0]["objects"][0]["name"], "a.bin");
    assert_eq!(v["root"][0]["objects"][0]["type"], "filetrrntzip");
    assert_eq!(v["root"][0]["objects"][0]["crc"], "12345678");
    assert_eq!(v["root"][0]["objects"][0]["sha1"], "0011");
}
