use eframe::egui;
use std::cell::RefCell;
use std::rc::Rc;

use crate::RomVaultApp;
use dat_reader::enums::{DatStatus, FileType};
use rv_core::db::GLOBAL_DB;
use rv_core::enums::RepStatus;
use rv_core::file_scanning::FileScanning;
use rv_core::rv_file::{RvFile, TreeSelect};
use rv_core::scanner::Scanner;

#[derive(Clone)]
pub struct TreeRow {
    pub node_rc: Rc<RefCell<RvFile>>,
    pub depth: usize,
}

fn merged_roms(stats: &rv_core::repair_status::RepairStatus) -> i32 {
    stats.roms_not_collected + stats.roms_unneeded
}

fn correct_roms(stats: &rv_core::repair_status::RepairStatus) -> i32 {
    stats.count_correct()
}

fn missing_roms(stats: &rv_core::repair_status::RepairStatus) -> i32 {
    stats.roms_missing
}

fn unknown_roms(stats: &rv_core::repair_status::RepairStatus) -> i32 {
    stats.roms_unknown
}

fn correct_plain(stats: &rv_core::repair_status::RepairStatus) -> i32 {
    stats.roms_correct - stats.roms_correct_mia
}

fn missing_plain(stats: &rv_core::repair_status::RepairStatus) -> i32 {
    stats.roms_missing - stats.roms_missing_mia
}

fn tree_color_from_rep_status(rep_status: RepStatus, dat_status: DatStatus) -> egui::Color32 {
    match rep_status {
        RepStatus::Correct | RepStatus::CorrectMIA | RepStatus::DirCorrect => {
            egui::Color32::from_rgb(0, 200, 0)
        }
        RepStatus::Missing
        | RepStatus::MissingMIA
        | RepStatus::DirMissing
        | RepStatus::Corrupt
        | RepStatus::DirCorrupt
        | RepStatus::Incomplete => egui::Color32::from_rgb(200, 0, 0),
        RepStatus::CanBeFixed | RepStatus::CanBeFixedMIA | RepStatus::CorruptCanBeFixed => {
            egui::Color32::from_rgb(200, 200, 0)
        }
        RepStatus::MoveToSort
        | RepStatus::MoveToCorrupt
        | RepStatus::NeededForFix
        | RepStatus::Rename
        | RepStatus::InToSort
        | RepStatus::DirInToSort => egui::Color32::from_rgb(0, 200, 200),
        RepStatus::Delete | RepStatus::Deleted => egui::Color32::from_rgb(200, 0, 0),
        RepStatus::NotCollected
        | RepStatus::UnNeeded
        | RepStatus::Unknown
        | RepStatus::DirUnknown
        | RepStatus::UnScanned
        | RepStatus::Ignore => egui::Color32::from_rgb(150, 150, 150),
        _ => {
            if dat_status == DatStatus::NotInDat {
                egui::Color32::from_rgb(150, 150, 150)
            } else {
                egui::Color32::WHITE
            }
        }
    }
}

fn tree_color_from_stats(stats: &rv_core::repair_status::RepairStatus) -> egui::Color32 {
    if (stats.total_roms == 0 && stats.roms_unknown == 0)
        || (stats.total_roms > 0
            && (unknown_roms(stats) == stats.total_roms || merged_roms(stats) == stats.total_roms))
    {
        egui::Color32::from_rgb(150, 150, 150)
    } else if (stats.roms_fixes == stats.total_roms || stats.roms_in_to_sort == stats.total_roms)
        && stats.total_roms > 0
    {
        egui::Color32::from_rgb(0, 200, 200)
    } else if correct_roms(stats) == stats.total_roms && stats.total_roms > 0 {
        egui::Color32::from_rgb(0, 200, 0)
    } else if missing_roms(stats) == stats.total_roms && stats.total_roms > 0 {
        egui::Color32::from_rgb(200, 0, 0)
    } else if correct_roms(stats) > 0 || stats.roms_fixes > 0 || stats.roms_in_to_sort > 0 {
        egui::Color32::from_rgb(200, 200, 0)
    } else {
        egui::Color32::WHITE
    }
}

fn tree_icon_idx_from_stats(stats: &rv_core::repair_status::RepairStatus) -> i32 {
    if stats.total_roms == 0 {
        2
    } else if unknown_roms(stats) == stats.total_roms
        || merged_roms(stats) == stats.total_roms
        || stats.roms_fixes > 0
        || stats.roms_in_to_sort > 0
    {
        4
    } else if correct_plain(stats) == 0 && missing_plain(stats) > 0 {
        1
    } else if missing_plain(stats) == 0 && stats.roms_missing_mia > 0 {
        5
    } else if missing_plain(stats) == 0 {
        3
    } else {
        2
    }
}

fn tree_icon_idx_from_report_status(report_status: rv_core::enums::ReportStatus) -> i32 {
    if report_status == rv_core::enums::ReportStatus::InToSort {
        4
    } else if !report_status.has_correct() && report_status.has_missing(false) {
        1
    } else if report_status.has_unknown() || report_status.has_all_merged() {
        4
    } else if !report_status.has_missing(false) {
        3
    } else {
        2
    }
}

include!("tree/app_impl.rs");

#[cfg(test)]
#[path = "tests/tree_tests.rs"]
mod tests;
