use super::*;
use crate::rv_game::RvGame;
use dat_reader::enums::{DatStatus, FileType, GotStatus};

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
