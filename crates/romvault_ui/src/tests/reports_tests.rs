use std::cell::RefCell;
use std::rc::Rc;

use rv_core::enums::RepStatus;
use rv_core::rv_dat::{DatData, RvDat};
use rv_core::rv_file::RvFile;
use rv_core::rv_game::RvGame;
use dat_reader::enums::FileType;

use crate::reports::write_fix_report;
use crate::reports::write_full_report;

#[test]
fn test_write_fix_report_includes_dat_name_and_fix_rows() {
    let root = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));

    let mut dat = RvDat::new();
    dat.set_data(DatData::DatRootFullName, Some("DatRoot\\TestDat".to_string()));
    let dat_rc = Rc::new(RefCell::new(dat));

    let game_rc = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
    game_rc.borrow_mut().name = "GameA".to_string();
    game_rc.borrow_mut().dat = Some(Rc::clone(&dat_rc));
    game_rc.borrow_mut().game = Some(Rc::new(RefCell::new(RvGame::new())));
    game_rc.borrow_mut().parent = Some(Rc::downgrade(&root));

    let rom_rc = Rc::new(RefCell::new(RvFile::new(FileType::File)));
    {
        let mut rom = rom_rc.borrow_mut();
        rom.name = "rom.bin".to_string();
        rom.dat = Some(Rc::clone(&dat_rc));
        rom.set_rep_status(RepStatus::CanBeFixed);
        rom.size = Some(123);
        rom.crc = Some(vec![0x12, 0x34, 0x56, 0x78]);
        rom.parent = Some(Rc::downgrade(&game_rc));
    }

    game_rc.borrow_mut().children.push(Rc::clone(&rom_rc));
    root.borrow_mut().children.push(Rc::clone(&game_rc));

    let dir = tempfile::tempdir().unwrap();
    let out_path = dir.path().join("fix_report.txt");
    write_fix_report(&out_path.to_string_lossy(), Rc::clone(&root)).unwrap();

    let text = std::fs::read_to_string(out_path).unwrap();
    assert!(text.contains("Listing Fixes"));
    assert!(text.contains("TestDat"));
    assert!(text.contains("rom.bin"));
    assert!(text.contains("123"));
    assert!(text.contains("12345678"));
    assert!(text.contains("CanBeFixed"));
}

#[test]
fn test_write_full_report_emits_expected_sections() {
    let root = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));

    let mut dat = RvDat::new();
    dat.set_data(DatData::DatRootFullName, Some("DatRoot\\TestDat".to_string()));
    let dat_rc = Rc::new(RefCell::new(dat));

    let game_rc = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
    game_rc.borrow_mut().name = "GameA".to_string();
    game_rc.borrow_mut().dat = Some(Rc::clone(&dat_rc));
    game_rc.borrow_mut().game = Some(Rc::new(RefCell::new(RvGame::new())));
    game_rc.borrow_mut().parent = Some(Rc::downgrade(&root));

    let rom_rc = Rc::new(RefCell::new(RvFile::new(FileType::File)));
    {
        let mut rom = rom_rc.borrow_mut();
        rom.name = "rom.bin".to_string();
        rom.dat = Some(Rc::clone(&dat_rc));
        rom.set_rep_status(RepStatus::Missing);
        rom.size = Some(123);
        rom.crc = Some(vec![0x12, 0x34, 0x56, 0x78]);
        rom.parent = Some(Rc::downgrade(&game_rc));
    }

    let rom_ok_rc = Rc::new(RefCell::new(RvFile::new(FileType::File)));
    {
        let mut rom = rom_ok_rc.borrow_mut();
        rom.name = "ok.bin".to_string();
        rom.dat = Some(Rc::clone(&dat_rc));
        rom.set_rep_status(RepStatus::Correct);
        rom.size = Some(1);
        rom.crc = Some(vec![0, 0, 0, 0]);
        rom.parent = Some(Rc::downgrade(&game_rc));
    }

    game_rc.borrow_mut().children.push(Rc::clone(&rom_rc));
    game_rc.borrow_mut().children.push(Rc::clone(&rom_ok_rc));
    root.borrow_mut().children.push(Rc::clone(&game_rc));

    let dir = tempfile::tempdir().unwrap();
    let out_path = dir.path().join("full_report.txt");
    write_full_report(&out_path.to_string_lossy(), Rc::clone(&root)).unwrap();

    let text = std::fs::read_to_string(out_path).unwrap();
    assert!(text.contains("Complete DAT Sets"));
    assert!(text.contains("Empty DAT Sets"));
    assert!(text.contains("Partial DAT Sets - (Listing Missing ROMs)"));
    assert!(text.contains("TestDat"));
    assert!(text.contains("rom.bin"));
    assert!(text.contains("12345678"));
}
