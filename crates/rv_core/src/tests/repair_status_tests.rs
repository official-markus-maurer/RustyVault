use super::*;
use crate::rv_game::RvGame;
use dat_reader::enums::{DatStatus, FileType, GotStatus};

#[test]
fn test_rep_status_reset_matches_csharp_repair_status_matrix_primary() {
    fn expected_primary(
        file_type: FileType,
        dat_status: DatStatus,
        got_status: GotStatus,
    ) -> Option<crate::enums::RepStatus> {
        use crate::enums::RepStatus as RS;
        use dat_reader::enums::DatStatus as DS;
        use dat_reader::enums::FileType as FT;
        use dat_reader::enums::GotStatus as GS;

        match (file_type, dat_status, got_status) {
            (FT::Dir, DS::InDatCollect, GS::NotGot) => Some(RS::DirMissing),
            (FT::Dir, DS::InDatCollect, GS::Got) => Some(RS::DirCorrect),
            (FT::Dir, DS::InToSort, GS::NotGot) => Some(RS::Deleted),
            (FT::Dir, DS::InToSort, GS::Got) => Some(RS::DirInToSort),
            (FT::Dir, DS::NotInDat, GS::NotGot) => Some(RS::Deleted),
            (FT::Dir, DS::NotInDat, GS::Got) => Some(RS::DirUnknown),

            (FT::Zip, DS::InDatCollect, GS::NotGot) => Some(RS::DirMissing),
            (FT::Zip, DS::InDatCollect, GS::Got) => Some(RS::DirCorrect),
            (FT::Zip, DS::InDatCollect, GS::Corrupt) => Some(RS::DirCorrupt),
            (FT::Zip, DS::InDatCollect, GS::FileLocked) => Some(RS::UnScanned),
            (FT::Zip, DS::InToSort, GS::NotGot) => Some(RS::Deleted),
            (FT::Zip, DS::InToSort, GS::Got) => Some(RS::DirInToSort),
            (FT::Zip, DS::InToSort, GS::Corrupt) => Some(RS::DirCorrupt),
            (FT::Zip, DS::InToSort, GS::FileLocked) => Some(RS::UnScanned),
            (FT::Zip, DS::NotInDat, GS::NotGot) => Some(RS::Deleted),
            (FT::Zip, DS::NotInDat, GS::Got) => Some(RS::DirUnknown),
            (FT::Zip, DS::NotInDat, GS::Corrupt) => Some(RS::DirCorrupt),
            (FT::Zip, DS::NotInDat, GS::FileLocked) => Some(RS::UnScanned),

            (FT::SevenZip, DS::InDatCollect, GS::NotGot) => Some(RS::DirMissing),
            (FT::SevenZip, DS::InDatCollect, GS::Got) => Some(RS::DirCorrect),
            (FT::SevenZip, DS::InDatCollect, GS::Corrupt) => Some(RS::DirCorrupt),
            (FT::SevenZip, DS::InDatCollect, GS::FileLocked) => Some(RS::UnScanned),
            (FT::SevenZip, DS::InToSort, GS::NotGot) => Some(RS::Deleted),
            (FT::SevenZip, DS::InToSort, GS::Got) => Some(RS::DirInToSort),
            (FT::SevenZip, DS::InToSort, GS::Corrupt) => Some(RS::DirCorrupt),
            (FT::SevenZip, DS::InToSort, GS::FileLocked) => Some(RS::UnScanned),
            (FT::SevenZip, DS::NotInDat, GS::NotGot) => Some(RS::Deleted),
            (FT::SevenZip, DS::NotInDat, GS::Got) => Some(RS::DirUnknown),
            (FT::SevenZip, DS::NotInDat, GS::Corrupt) => Some(RS::DirCorrupt),
            (FT::SevenZip, DS::NotInDat, GS::FileLocked) => Some(RS::UnScanned),

            (FT::File, DS::InDatCollect, GS::NotGot) => Some(RS::Missing),
            (FT::File, DS::InDatCollect, GS::Got) => Some(RS::Correct),
            (FT::File, DS::InDatCollect, GS::Corrupt) => Some(RS::Corrupt),
            (FT::File, DS::InDatCollect, GS::FileLocked) => Some(RS::UnScanned),
            (FT::File, DS::InDatMerged, GS::NotGot) => Some(RS::NotCollected),
            (FT::File, DS::InDatMerged, GS::Got) => Some(RS::UnNeeded),
            (FT::File, DS::InDatMerged, GS::Corrupt) => Some(RS::Corrupt),
            (FT::File, DS::InDatMerged, GS::FileLocked) => Some(RS::UnScanned),
            (FT::File, DS::InDatMIA, GS::NotGot) => Some(RS::MissingMIA),
            (FT::File, DS::InDatMIA, GS::Got) => Some(RS::CorrectMIA),
            (FT::File, DS::InDatMIA, GS::Corrupt) => Some(RS::Corrupt),
            (FT::File, DS::InDatMIA, GS::FileLocked) => Some(RS::UnScanned),
            (FT::File, DS::InToSort, GS::NotGot) => Some(RS::Deleted),
            (FT::File, DS::InToSort, GS::Got) => Some(RS::InToSort),
            (FT::File, DS::InToSort, GS::Corrupt) => Some(RS::Corrupt),
            (FT::File, DS::InToSort, GS::FileLocked) => Some(RS::UnScanned),
            (FT::File, DS::NotInDat, GS::NotGot) => Some(RS::Deleted),
            (FT::File, DS::NotInDat, GS::Got) => Some(RS::Unknown),
            (FT::File, DS::NotInDat, GS::Corrupt) => Some(RS::Corrupt),
            (FT::File, DS::NotInDat, GS::FileLocked) => Some(RS::UnScanned),

            (FT::FileZip, DS::InDatCollect, GS::NotGot) => Some(RS::Missing),
            (FT::FileZip, DS::InDatCollect, GS::Got) => Some(RS::Correct),
            (FT::FileZip, DS::InDatCollect, GS::Corrupt) => Some(RS::Corrupt),
            (FT::FileZip, DS::InDatCollect, GS::FileLocked) => Some(RS::UnScanned),
            (FT::FileZip, DS::InDatMerged, GS::NotGot) => Some(RS::NotCollected),
            (FT::FileZip, DS::InDatMerged, GS::Got) => Some(RS::UnNeeded),
            (FT::FileZip, DS::InDatMerged, GS::Corrupt) => Some(RS::Corrupt),
            (FT::FileZip, DS::InDatMerged, GS::FileLocked) => Some(RS::UnScanned),
            (FT::FileZip, DS::InDatMIA, GS::NotGot) => Some(RS::MissingMIA),
            (FT::FileZip, DS::InDatMIA, GS::Got) => Some(RS::CorrectMIA),
            (FT::FileZip, DS::InDatMIA, GS::Corrupt) => Some(RS::Corrupt),
            (FT::FileZip, DS::InDatMIA, GS::FileLocked) => Some(RS::UnScanned),
            (FT::FileZip, DS::InToSort, GS::NotGot) => Some(RS::Deleted),
            (FT::FileZip, DS::InToSort, GS::Got) => Some(RS::InToSort),
            (FT::FileZip, DS::InToSort, GS::Corrupt) => Some(RS::Corrupt),
            (FT::FileZip, DS::InToSort, GS::FileLocked) => Some(RS::UnScanned),
            (FT::FileZip, DS::NotInDat, GS::NotGot) => Some(RS::Deleted),
            (FT::FileZip, DS::NotInDat, GS::Got) => Some(RS::Unknown),
            (FT::FileZip, DS::NotInDat, GS::Corrupt) => Some(RS::Corrupt),
            (FT::FileZip, DS::NotInDat, GS::FileLocked) => Some(RS::UnScanned),

            (FT::FileSevenZip, DS::InDatCollect, GS::NotGot) => Some(RS::Missing),
            (FT::FileSevenZip, DS::InDatCollect, GS::Got) => Some(RS::Correct),
            (FT::FileSevenZip, DS::InDatCollect, GS::Corrupt) => Some(RS::Corrupt),
            (FT::FileSevenZip, DS::InDatCollect, GS::FileLocked) => Some(RS::UnScanned),
            (FT::FileSevenZip, DS::InDatMerged, GS::NotGot) => Some(RS::NotCollected),
            (FT::FileSevenZip, DS::InDatMerged, GS::Got) => Some(RS::UnNeeded),
            (FT::FileSevenZip, DS::InDatMerged, GS::Corrupt) => Some(RS::Corrupt),
            (FT::FileSevenZip, DS::InDatMerged, GS::FileLocked) => Some(RS::UnScanned),
            (FT::FileSevenZip, DS::InDatMIA, GS::NotGot) => Some(RS::MissingMIA),
            (FT::FileSevenZip, DS::InDatMIA, GS::Got) => Some(RS::CorrectMIA),
            (FT::FileSevenZip, DS::InDatMIA, GS::Corrupt) => Some(RS::Corrupt),
            (FT::FileSevenZip, DS::InDatMIA, GS::FileLocked) => Some(RS::UnScanned),
            (FT::FileSevenZip, DS::InToSort, GS::NotGot) => Some(RS::Deleted),
            (FT::FileSevenZip, DS::InToSort, GS::Got) => Some(RS::InToSort),
            (FT::FileSevenZip, DS::InToSort, GS::Corrupt) => Some(RS::Corrupt),
            (FT::FileSevenZip, DS::InToSort, GS::FileLocked) => Some(RS::UnScanned),
            (FT::FileSevenZip, DS::NotInDat, GS::NotGot) => Some(RS::Deleted),
            (FT::FileSevenZip, DS::NotInDat, GS::Got) => Some(RS::Unknown),
            (FT::FileSevenZip, DS::NotInDat, GS::Corrupt) => Some(RS::Corrupt),
            (FT::FileSevenZip, DS::NotInDat, GS::FileLocked) => Some(RS::UnScanned),

            (FT::FileOnly, DS::InDatCollect, GS::NotGot) => Some(RS::Missing),
            (FT::FileOnly, DS::InDatCollect, GS::Got) => Some(RS::Correct),
            (FT::FileOnly, DS::InDatCollect, GS::Corrupt) => Some(RS::Corrupt),
            (FT::FileOnly, DS::InDatCollect, GS::FileLocked) => Some(RS::UnScanned),
            (FT::FileOnly, DS::InDatMerged, GS::NotGot) => Some(RS::NotCollected),
            (FT::FileOnly, DS::InDatMerged, GS::Got) => Some(RS::UnNeeded),
            (FT::FileOnly, DS::InDatMerged, GS::Corrupt) => Some(RS::Corrupt),
            (FT::FileOnly, DS::InDatMerged, GS::FileLocked) => Some(RS::UnScanned),
            (FT::FileOnly, DS::InDatMIA, GS::NotGot) => Some(RS::MissingMIA),
            (FT::FileOnly, DS::InDatMIA, GS::Got) => Some(RS::CorrectMIA),
            (FT::FileOnly, DS::InDatMIA, GS::Corrupt) => Some(RS::Corrupt),
            (FT::FileOnly, DS::InDatMIA, GS::FileLocked) => Some(RS::UnScanned),
            (FT::FileOnly, DS::InToSort, GS::NotGot) => Some(RS::Deleted),
            (FT::FileOnly, DS::InToSort, GS::Got) => Some(RS::InToSort),
            (FT::FileOnly, DS::InToSort, GS::Corrupt) => Some(RS::Corrupt),
            (FT::FileOnly, DS::InToSort, GS::FileLocked) => Some(RS::UnScanned),
            (FT::FileOnly, DS::NotInDat, GS::NotGot) => Some(RS::Deleted),
            (FT::FileOnly, DS::NotInDat, GS::Got) => Some(RS::Unknown),
            (FT::FileOnly, DS::NotInDat, GS::Corrupt) => Some(RS::Corrupt),
            (FT::FileOnly, DS::NotInDat, GS::FileLocked) => Some(RS::UnScanned),

            (
                FT::File | FT::FileZip | FT::FileSevenZip | FT::FileOnly,
                DS::InDatNoDump,
                GS::NotGot,
            ) => Some(RS::NotCollected),
            (
                FT::File | FT::FileZip | FT::FileSevenZip | FT::FileOnly,
                DS::InDatNoDump,
                GS::Got,
            ) => Some(RS::UnNeeded),
            (
                FT::File | FT::FileZip | FT::FileSevenZip | FT::FileOnly,
                DS::InDatNoDump,
                GS::Corrupt,
            ) => Some(RS::Corrupt),
            (
                FT::File | FT::FileZip | FT::FileSevenZip | FT::FileOnly,
                DS::InDatNoDump,
                GS::FileLocked,
            ) => Some(RS::UnScanned),

            _ => None,
        }
    }

    let file_types = [
        FileType::Dir,
        FileType::Zip,
        FileType::SevenZip,
        FileType::File,
        FileType::FileZip,
        FileType::FileSevenZip,
        FileType::FileOnly,
    ];
    let dat_statuses = [
        DatStatus::InDatCollect,
        DatStatus::InDatMerged,
        DatStatus::InDatNoDump,
        DatStatus::InDatMIA,
        DatStatus::InToSort,
        DatStatus::NotInDat,
    ];
    let got_statuses = [
        GotStatus::NotGot,
        GotStatus::Got,
        GotStatus::Corrupt,
        GotStatus::FileLocked,
    ];

    for file_type in file_types {
        for dat_status in dat_statuses {
            for got_status in got_statuses {
                let Some(expected) = expected_primary(file_type, dat_status, got_status) else {
                    continue;
                };

                let mut file = RvFile::new(file_type);
                file.set_dat_got_status(dat_status, got_status);
                file.rep_status_reset();
                assert_eq!(
                    file.rep_status(),
                    expected,
                    "mismatch for file_type={:?} dat_status={:?} got_status={:?}",
                    file_type,
                    dat_status,
                    got_status
                );
            }
        }
    }
}

#[test]
fn test_dominant_rep_status_uses_csharp_display_order_for_overlays() {
    use crate::enums::RepStatus;
    use dat_reader::enums::{DatStatus, FileType, GotStatus};

    let root = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));

    let missing = Rc::new(RefCell::new(RvFile::new(FileType::File)));
    missing
        .borrow_mut()
        .set_dat_got_status(DatStatus::InDatCollect, GotStatus::NotGot);
    missing.borrow_mut().rep_status_reset();

    let delete = Rc::new(RefCell::new(RvFile::new(FileType::File)));
    delete
        .borrow_mut()
        .set_dat_got_status(DatStatus::NotInDat, GotStatus::Got);
    delete.borrow_mut().set_rep_status(RepStatus::Delete);

    root.borrow_mut().child_add(Rc::clone(&missing));
    root.borrow_mut().child_add(Rc::clone(&delete));

    assert_eq!(
        crate::repair_status::RepairStatus::dominant_rep_status(Rc::clone(&root)),
        RepStatus::Delete
    );

    delete.borrow_mut().set_rep_status(RepStatus::NeededForFix);
    assert_eq!(
        crate::repair_status::RepairStatus::dominant_rep_status(Rc::clone(&root)),
        RepStatus::NeededForFix
    );

    delete.borrow_mut().set_rep_status(RepStatus::MoveToSort);
    assert_eq!(
        crate::repair_status::RepairStatus::dominant_rep_status(Rc::clone(&root)),
        RepStatus::MoveToSort
    );

    delete.borrow_mut().set_rep_status(RepStatus::Rename);
    assert_eq!(
        crate::repair_status::RepairStatus::dominant_rep_status(Rc::clone(&root)),
        RepStatus::Rename
    );
}

#[test]
fn test_dominant_rep_status_file_locked_wins() {
    use crate::enums::RepStatus;
    use dat_reader::enums::{DatStatus, FileType, GotStatus};

    let root = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
    root.borrow_mut()
        .set_dat_got_status(DatStatus::NotInDat, GotStatus::FileLocked);

    let child = Rc::new(RefCell::new(RvFile::new(FileType::File)));
    child
        .borrow_mut()
        .set_dat_got_status(DatStatus::NotInDat, GotStatus::Got);
    child.borrow_mut().set_rep_status(RepStatus::Delete);
    root.borrow_mut().child_add(Rc::clone(&child));

    assert_eq!(
        crate::repair_status::RepairStatus::dominant_rep_status(Rc::clone(&root)),
        RepStatus::UnScanned
    );
}

#[test]
fn test_repair_status_counting() {
    let root = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));

    // Add a Correct ROM
    let correct_rom = Rc::new(RefCell::new(RvFile::new(FileType::File)));
    correct_rom
        .borrow_mut()
        .set_dat_got_status(DatStatus::InDatCollect, GotStatus::Got);
    correct_rom.borrow_mut().rep_status_reset();

    // Add a Missing ROM
    let missing_rom = Rc::new(RefCell::new(RvFile::new(FileType::File)));
    missing_rom
        .borrow_mut()
        .set_dat_got_status(DatStatus::InDatCollect, GotStatus::NotGot);
    missing_rom.borrow_mut().rep_status_reset();

    // Add an Unknown ROM
    let unknown_rom = Rc::new(RefCell::new(RvFile::new(FileType::File)));
    unknown_rom
        .borrow_mut()
        .set_dat_got_status(DatStatus::NotInDat, GotStatus::Got);
    unknown_rom.borrow_mut().rep_status_reset();

    root.borrow_mut().child_add(correct_rom);
    root.borrow_mut().child_add(missing_rom);
    root.borrow_mut().child_add(unknown_rom);

    let mut status = RepairStatus::new();
    status.report_status(Rc::clone(&root));

    assert_eq!(status.total_roms, 3);
    assert_eq!(status.roms_correct, 1);
    assert_eq!(status.roms_missing, 1);
    assert_eq!(status.roms_unknown, 1);

    assert_eq!(status.count_correct(), 1);
    assert_eq!(status.count_missing(), 1);
    assert_eq!(status.count_fixes_needed(), 1);
}

#[test]
fn test_repair_status_fix_count_includes_unneeded_roms() {
    let root = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));

    let fixable_rom = Rc::new(RefCell::new(RvFile::new(FileType::File)));
    {
        let mut rom = fixable_rom.borrow_mut();
        rom.set_dat_got_status(DatStatus::InDatCollect, GotStatus::NotGot);
        rom.set_rep_status(crate::enums::RepStatus::CanBeFixed);
    }

    let merged_rom = Rc::new(RefCell::new(RvFile::new(FileType::File)));
    {
        let mut rom = merged_rom.borrow_mut();
        rom.set_dat_got_status(DatStatus::InDatMerged, GotStatus::Got);
        rom.set_rep_status(crate::enums::RepStatus::UnNeeded);
    }

    root.borrow_mut().child_add(fixable_rom);
    root.borrow_mut().child_add(merged_rom);

    let mut status = RepairStatus::new();
    status.report_status(Rc::clone(&root));

    assert_eq!(status.roms_fixes, 1);
    assert_eq!(status.roms_unneeded, 1);
    assert_eq!(status.count_fixes_needed(), 2);
}

#[test]
fn test_repair_status_tracks_not_collected_roms_separately() {
    let root = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));

    let merged_missing_rom = Rc::new(RefCell::new(RvFile::new(FileType::File)));
    {
        let mut rom = merged_missing_rom.borrow_mut();
        rom.set_dat_got_status(DatStatus::InDatMerged, GotStatus::NotGot);
        rom.rep_status_reset();
    }

    root.borrow_mut().child_add(merged_missing_rom);

    let mut status = RepairStatus::new();
    status.report_status(Rc::clone(&root));

    assert_eq!(status.total_roms, 1);
    assert_eq!(status.roms_not_collected, 1);
    assert_eq!(status.roms_unneeded, 0);
    assert_eq!(status.count_missing(), 0);
    assert_eq!(status.count_fixes_needed(), 0);
}

#[test]
fn test_repair_status_missing_count_excludes_unknown_roms() {
    let root = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));

    let missing_rom = Rc::new(RefCell::new(RvFile::new(FileType::File)));
    {
        let mut rom = missing_rom.borrow_mut();
        rom.set_dat_got_status(DatStatus::InDatCollect, GotStatus::NotGot);
        rom.rep_status_reset();
    }

    let unknown_rom = Rc::new(RefCell::new(RvFile::new(FileType::File)));
    {
        let mut rom = unknown_rom.borrow_mut();
        rom.set_dat_got_status(DatStatus::NotInDat, GotStatus::Got);
        rom.rep_status_reset();
    }

    root.borrow_mut().child_add(missing_rom);
    root.borrow_mut().child_add(unknown_rom);

    let mut status = RepairStatus::new();
    status.report_status(Rc::clone(&root));

    assert_eq!(status.roms_missing, 1);
    assert_eq!(status.roms_unknown, 1);
    assert_eq!(status.count_missing(), 1);
}

#[test]
fn test_repair_status_count_helpers_do_not_double_count_mia_variants() {
    let root = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));

    let correct_mia = Rc::new(RefCell::new(RvFile::new(FileType::File)));
    correct_mia
        .borrow_mut()
        .set_rep_status(crate::enums::RepStatus::CorrectMIA);

    let missing_mia = Rc::new(RefCell::new(RvFile::new(FileType::File)));
    missing_mia
        .borrow_mut()
        .set_rep_status(crate::enums::RepStatus::MissingMIA);

    root.borrow_mut().child_add(correct_mia);
    root.borrow_mut().child_add(missing_mia);

    let mut status = RepairStatus::new();
    status.report_status(Rc::clone(&root));

    assert_eq!(status.count_correct(), 1);
    assert_eq!(status.count_missing(), 1);
}

#[test]
fn test_repair_status_buckets_runtime_status_families_consistently() {
    let root = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));

    let missing_family = Rc::new(RefCell::new(RvFile::new(FileType::File)));
    missing_family
        .borrow_mut()
        .set_rep_status(crate::enums::RepStatus::Corrupt);

    let fix_family = Rc::new(RefCell::new(RvFile::new(FileType::File)));
    fix_family
        .borrow_mut()
        .set_rep_status(crate::enums::RepStatus::NeededForFix);

    let unknown_family = Rc::new(RefCell::new(RvFile::new(FileType::File)));
    unknown_family
        .borrow_mut()
        .set_rep_status(crate::enums::RepStatus::UnScanned);

    root.borrow_mut().child_add(missing_family);
    root.borrow_mut().child_add(fix_family);
    root.borrow_mut().child_add(unknown_family);

    let mut status = RepairStatus::new();
    status.report_status(Rc::clone(&root));

    assert_eq!(status.roms_missing, 1);
    assert_eq!(status.roms_fixes, 1);
    assert_eq!(status.roms_unknown, 1);
    assert_eq!(status.count_missing(), 2);
    assert_eq!(status.count_fixes_needed(), 2);
}

#[test]
fn test_repair_status_tracks_game_counters_for_game_nodes() {
    let root = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));

    let game = Rc::new(RefCell::new(RvFile::new(FileType::Zip)));
    {
        let mut node = game.borrow_mut();
        node.game = Some(Rc::new(RefCell::new(RvGame::from_description("Pac-Man"))));
        node.set_rep_status(crate::enums::RepStatus::CanBeFixed);
    }

    let rom = Rc::new(RefCell::new(RvFile::new(FileType::File)));
    rom.borrow_mut()
        .set_rep_status(crate::enums::RepStatus::Correct);
    game.borrow_mut().child_add(rom);
    root.borrow_mut().child_add(Rc::clone(&game));

    let mut status = RepairStatus::new();
    status.report_status(Rc::clone(&root));

    assert_eq!(status.total_games, 1);
    assert_eq!(status.games_fixes, 1);
    assert_eq!(status.games_correct, 0);
    assert_eq!(status.total_roms, 1);
    assert_eq!(status.roms_correct, 1);
}

#[test]
fn test_repair_status_uses_cached_game_counters() {
    let game = Rc::new(RefCell::new(RvFile::new(FileType::Zip)));
    {
        let mut node = game.borrow_mut();
        node.game = Some(Rc::new(RefCell::new(RvGame::from_description("Galaga"))));
        node.set_rep_status(crate::enums::RepStatus::MissingMIA);
    }

    let mut first_pass = RepairStatus::new();
    first_pass.report_status(Rc::clone(&game));

    let mut second_pass = RepairStatus::new();
    second_pass.report_status(Rc::clone(&game));

    assert_eq!(first_pass.total_games, 1);
    assert_eq!(first_pass.games_missing, 1);
    assert_eq!(first_pass.games_missing_mia, 1);
    assert_eq!(second_pass.total_games, 1);
    assert_eq!(second_pass.games_missing, 1);
    assert_eq!(second_pass.games_missing_mia, 1);
}

#[test]
fn test_repair_status_synthesizes_dir_status_for_fix_only_branch() {
    let root = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));

    let fixable_rom = Rc::new(RefCell::new(RvFile::new(FileType::File)));
    fixable_rom
        .borrow_mut()
        .set_rep_status(crate::enums::RepStatus::NeededForFix);
    root.borrow_mut().child_add(fixable_rom);

    let mut status = RepairStatus::new();
    status.report_status(Rc::clone(&root));

    assert_eq!(
        root.borrow().dir_status,
        Some(crate::enums::ReportStatus::InToSort)
    );
}

#[test]
fn test_repair_status_synthesizes_correctmia_only_branch_as_correct() {
    let root = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));

    let mia_rom = Rc::new(RefCell::new(RvFile::new(FileType::File)));
    mia_rom
        .borrow_mut()
        .set_rep_status(crate::enums::RepStatus::CorrectMIA);
    root.borrow_mut().child_add(mia_rom);

    let mut status = RepairStatus::new();
    status.report_status(Rc::clone(&root));

    assert_eq!(
        root.borrow().dir_status,
        Some(crate::enums::ReportStatus::Correct)
    );
}

#[test]
fn test_repair_status_synthesizes_dir_status_for_merged_branch() {
    let root = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));

    let merged_rom = Rc::new(RefCell::new(RvFile::new(FileType::File)));
    merged_rom
        .borrow_mut()
        .set_rep_status(crate::enums::RepStatus::UnNeeded);
    root.borrow_mut().child_add(merged_rom);

    let mut status = RepairStatus::new();
    status.report_status(Rc::clone(&root));

    assert_eq!(
        root.borrow().dir_status,
        Some(crate::enums::ReportStatus::UnNeeded)
    );
}

#[test]
fn test_repair_status_refreshes_dir_status_from_cached_stats() {
    let root = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
    {
        let mut node = root.borrow_mut();
        let mut cached = RepairStatus::new();
        cached.total_roms = 1;
        cached.roms_fixes = 1;
        node.cached_stats = Some(cached);
        node.dir_status = Some(crate::enums::ReportStatus::Unknown);
    }

    let mut status = RepairStatus::new();
    status.report_status(Rc::clone(&root));

    assert_eq!(status.total_roms, 1);
    assert_eq!(status.roms_fixes, 1);
    assert_eq!(
        root.borrow().dir_status,
        Some(crate::enums::ReportStatus::InToSort)
    );
}

#[test]
fn test_repair_status_synthesizes_mixed_correct_branch_as_unknown() {
    let root = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));

    let correct_rom = Rc::new(RefCell::new(RvFile::new(FileType::File)));
    correct_rom
        .borrow_mut()
        .set_rep_status(crate::enums::RepStatus::Correct);

    let unknown_rom = Rc::new(RefCell::new(RvFile::new(FileType::File)));
    unknown_rom
        .borrow_mut()
        .set_rep_status(crate::enums::RepStatus::Unknown);

    root.borrow_mut().child_add(correct_rom);
    root.borrow_mut().child_add(unknown_rom);

    let mut status = RepairStatus::new();
    status.report_status(Rc::clone(&root));

    assert_eq!(
        root.borrow().dir_status,
        Some(crate::enums::ReportStatus::Unknown)
    );
}

#[test]
fn test_repair_status_synthesizes_corrupt_only_branch_as_corrupt() {
    let root = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));

    let corrupt_rom = Rc::new(RefCell::new(RvFile::new(FileType::File)));
    corrupt_rom
        .borrow_mut()
        .set_rep_status(crate::enums::RepStatus::Corrupt);
    root.borrow_mut().child_add(corrupt_rom);

    let mut status = RepairStatus::new();
    status.report_status(Rc::clone(&root));

    assert_eq!(status.roms_corrupt, 1);
    assert_eq!(status.roms_missing, 1);
    assert_eq!(
        root.borrow().dir_status,
        Some(crate::enums::ReportStatus::Corrupt)
    );
}

#[test]
fn test_repair_status_synthesizes_mixed_correct_and_corrupt_branch_as_corrupt() {
    let root = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));

    let correct_rom = Rc::new(RefCell::new(RvFile::new(FileType::File)));
    correct_rom
        .borrow_mut()
        .set_rep_status(crate::enums::RepStatus::Correct);

    let corrupt_rom = Rc::new(RefCell::new(RvFile::new(FileType::File)));
    corrupt_rom
        .borrow_mut()
        .set_rep_status(crate::enums::RepStatus::Corrupt);

    root.borrow_mut().child_add(correct_rom);
    root.borrow_mut().child_add(corrupt_rom);

    let mut status = RepairStatus::new();
    status.report_status(Rc::clone(&root));

    assert_eq!(
        root.borrow().dir_status,
        Some(crate::enums::ReportStatus::Corrupt)
    );
}

#[test]
fn test_repair_status_synthesizes_merged_and_corrupt_branch_as_corrupt() {
    let root = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));

    let merged_rom = Rc::new(RefCell::new(RvFile::new(FileType::File)));
    merged_rom
        .borrow_mut()
        .set_rep_status(crate::enums::RepStatus::UnNeeded);

    let corrupt_rom = Rc::new(RefCell::new(RvFile::new(FileType::File)));
    corrupt_rom
        .borrow_mut()
        .set_rep_status(crate::enums::RepStatus::Corrupt);

    root.borrow_mut().child_add(merged_rom);
    root.borrow_mut().child_add(corrupt_rom);

    let mut status = RepairStatus::new();
    status.report_status(Rc::clone(&root));

    assert_eq!(
        root.borrow().dir_status,
        Some(crate::enums::ReportStatus::Corrupt)
    );
}

#[test]
fn test_repair_status_synthesizes_fix_and_corrupt_branch_as_corrupt() {
    let root = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));

    let fix_rom = Rc::new(RefCell::new(RvFile::new(FileType::File)));
    fix_rom
        .borrow_mut()
        .set_rep_status(crate::enums::RepStatus::NeededForFix);

    let corrupt_rom = Rc::new(RefCell::new(RvFile::new(FileType::File)));
    corrupt_rom
        .borrow_mut()
        .set_rep_status(crate::enums::RepStatus::Corrupt);

    root.borrow_mut().child_add(fix_rom);
    root.borrow_mut().child_add(corrupt_rom);

    let mut status = RepairStatus::new();
    status.report_status(Rc::clone(&root));

    assert_eq!(
        root.borrow().dir_status,
        Some(crate::enums::ReportStatus::Corrupt)
    );
}

#[test]
fn test_repair_status_refreshes_corrupt_dir_status_from_cached_stats() {
    let root = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
    {
        let mut node = root.borrow_mut();
        let mut cached = RepairStatus::new();
        cached.total_roms = 1;
        cached.roms_corrupt = 1;
        cached.roms_missing = 1;
        node.cached_stats = Some(cached);
        node.dir_status = Some(crate::enums::ReportStatus::Unknown);
    }

    let mut status = RepairStatus::new();
    status.report_status(Rc::clone(&root));

    assert_eq!(status.roms_corrupt, 1);
    assert_eq!(
        root.borrow().dir_status,
        Some(crate::enums::ReportStatus::Corrupt)
    );
}

#[test]
fn test_report_status_reset_invalidates_dir_status_recursively() {
    let root = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
    root.borrow_mut().dir_status = Some(crate::enums::ReportStatus::Correct);

    let child = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
    child.borrow_mut().dir_status = Some(crate::enums::ReportStatus::InToSort);
    root.borrow_mut().child_add(Rc::clone(&child));

    RepairStatus::report_status_reset(Rc::clone(&root));

    assert_eq!(
        root.borrow().dir_status,
        Some(crate::enums::ReportStatus::Unknown)
    );
    assert_eq!(
        child.borrow().dir_status,
        Some(crate::enums::ReportStatus::Unknown)
    );
}
