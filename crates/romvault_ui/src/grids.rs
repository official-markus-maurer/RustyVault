use eframe::egui;
use std::cell::RefCell;
use std::rc::Rc;

use crate::RomVaultApp;
use crate::utils::get_full_node_path;
use dat_reader::enums::FileType;
use rv_core::enums::RepStatus;
use rv_core::db::GLOBAL_DB;
use rv_core::file_scanning::FileScanning;
use rv_core::rv_dat::DatData;
use rv_core::rv_file::RvFile;
use rv_core::scanner::Scanner;

#[derive(Clone, Copy)]
enum RomGridCopyColumn {
    Got,
    Rom,
    Size,
    Crc32,
    Sha1,
    Md5,
    AltSize,
    AltCrc32,
    AltSha1,
    AltMd5,
}

fn clip_hex(bytes: &Option<Vec<u8>>, max_len: usize) -> Option<String> {
    let b = bytes.as_ref()?;
    if b.is_empty() {
        return None;
    }
    let hex = hex::encode(b);
    Some(hex.chars().take(max_len).collect())
}

fn rom_clipboard_text(rom: &RvFile, col: RomGridCopyColumn) -> Option<String> {
    match col {
        RomGridCopyColumn::Rom => {
            if rom.name.is_empty() {
                None
            } else {
                Some(rom.name.clone())
            }
        }
        RomGridCopyColumn::Size => rom.size.map(|s| s.to_string()),
        RomGridCopyColumn::Crc32 => clip_hex(&rom.crc, 8),
        RomGridCopyColumn::Sha1 => clip_hex(&rom.sha1, 40),
        RomGridCopyColumn::Md5 => clip_hex(&rom.md5, 32),
        RomGridCopyColumn::AltSize => rom.alt_size.map(|s| s.to_string()),
        RomGridCopyColumn::AltCrc32 => clip_hex(&rom.alt_crc, 8),
        RomGridCopyColumn::AltSha1 => clip_hex(&rom.alt_sha1, 40),
        RomGridCopyColumn::AltMd5 => clip_hex(&rom.alt_md5, 32),
        RomGridCopyColumn::Got => {
            let name = rom.name.clone();
            let size = rom.size.map(|s| s.to_string()).unwrap_or_default();
            let crc = clip_hex(&rom.crc, 8).unwrap_or_default();
            let sha1 = clip_hex(&rom.sha1, 40).unwrap_or_default();
            let md5 = clip_hex(&rom.md5, 32).unwrap_or_default();

            if name.is_empty() && size.is_empty() && crc.is_empty() && sha1.is_empty() && md5.is_empty() {
                return None;
            }

            let mut out = String::new();
            out.push_str(&format!("Name : {name}\n"));
            out.push_str(&format!("Size : {size}\n"));
            out.push_str(&format!("CRC32: {crc}\n"));
            if !sha1.is_empty() {
                out.push_str(&format!("SHA1 : {sha1}\n"));
            }
            if !md5.is_empty() {
                out.push_str(&format!("MD5  : {md5}\n"));
            }
            Some(out)
        }
    }
}

#[derive(Clone, Copy)]
enum GameGridCopyColumn {
    Type,
    Game,
    Description,
    Modified,
    RomStatus,
}

fn game_clipboard_text(game_node: &RvFile, description: &str, col: GameGridCopyColumn) -> Option<String> {
    match col {
        GameGridCopyColumn::Type => {
            let full_path = game_node.get_full_name();
            let dat_dir = game_node
                .dat
                .as_ref()
                .and_then(|d| d.borrow().get_data(DatData::DatRootFullName))
                .unwrap_or_default();
            Some(format!("{}\n{}\n{}\n", game_node.name, full_path, dat_dir))
        }
        GameGridCopyColumn::Modified | GameGridCopyColumn::RomStatus => {
            Some(format!("Name : {}\nDesc : {}\n", game_node.name, description))
        }
        GameGridCopyColumn::Game => {
            if game_node.name.is_empty() {
                None
            } else {
                Some(game_node.name.clone())
            }
        }
        GameGridCopyColumn::Description => {
            if description.is_empty() {
                None
            } else {
                Some(description.to_string())
            }
        }
    }
}

fn split_args_windows_style(input: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut in_quotes = false;
    let mut chars = input.chars().peekable();
    while let Some(ch) = chars.next() {
        match ch {
            '"' => {
                in_quotes = !in_quotes;
            }
            '\\' => {
                if chars.peek() == Some(&'"') {
                    chars.next();
                    cur.push('"');
                } else {
                    cur.push('\\');
                }
            }
            c if c.is_whitespace() && !in_quotes => {
                if !cur.is_empty() {
                    out.push(std::mem::take(&mut cur));
                }
            }
            _ => cur.push(ch),
        }
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

fn emulator_info_for_game_dir(game_parent: Rc<RefCell<RvFile>>) -> Option<rv_core::settings::EmulatorInfo> {
    let rel = get_full_node_path(Rc::clone(&game_parent));
    let rel = rel
        .split_once('\\')
        .map(|(_, rest)| rest.to_string())
        .unwrap_or(rel);

    let settings = rv_core::settings::get_settings();
    for ei in settings.e_info.items {
        let tree_dir = ei.tree_dir.clone().unwrap_or_default();
        if tree_dir.is_empty() {
            continue;
        }
        if !tree_dir.eq_ignore_ascii_case(&rel) {
            continue;
        }

        let command_line = ei.command_line.clone().unwrap_or_default();
        if command_line.trim().is_empty() {
            continue;
        }
        let exe_name = ei.exe_name.clone().unwrap_or_default();
        if exe_name.trim().is_empty() {
            continue;
        }
        if !std::path::Path::new(&exe_name).exists() {
            continue;
        }

        return Some(ei);
    }
    None
}

fn launch_emulator_for_game(game_node: &RvFile) -> bool {
    let parent_rc = match game_node.parent.as_ref().and_then(|p| p.upgrade()) {
        Some(p) => p,
        None => return false,
    };
    let Some(ei) = emulator_info_for_game_dir(Rc::clone(&parent_rc)) else {
        return false;
    };

    let exe_name = ei.exe_name.unwrap_or_default();
    if exe_name.trim().is_empty() {
        return false;
    }
    if !std::path::Path::new(&exe_name).exists() {
        return false;
    }

    let game_full_name = game_node.get_full_name();
    let game_directory = std::path::Path::new(&game_full_name)
        .parent()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_default();
    let game_name = std::path::Path::new(&game_node.name)
        .file_stem()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();

    let mut args = ei.command_line.unwrap_or_default();
    args = args.replace("{gamename}", &game_name);
    args = args.replace("{gamefilename}", &game_node.name);
    args = args.replace("{gamedirectory}", &game_directory);

    let working_dir = ei
        .working_directory
        .clone()
        .filter(|w| !w.trim().is_empty())
        .unwrap_or_else(|| {
            std::path::Path::new(&exe_name)
                .parent()
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_default()
        });

    let mut cmd = std::process::Command::new(&exe_name);
    if !working_dir.is_empty() {
        cmd.current_dir(&working_dir);
    }

    if let Some(extra) = ei.extra_path.as_ref().filter(|p| !p.trim().is_empty()) {
        let existing = std::env::var("PATH").unwrap_or_default();
        cmd.env("PATH", format!("{};{}", extra, existing));
    }

    for a in split_args_windows_style(&args) {
        cmd.arg(a);
    }

    cmd.spawn().is_ok()
}

fn open_web_page_for_game(game_node: &RvFile) -> bool {
    let Some(game) = &game_node.game else {
        return false;
    };
    let game_id = game.borrow().get_data(rv_core::rv_game::GameData::Id);
    let Some(game_id) = game_id.filter(|s| !s.trim().is_empty()) else {
        return false;
    };

    let home_page = game_node
        .dat
        .as_ref()
        .and_then(|d| d.borrow().get_data(DatData::HomePage))
        .unwrap_or_default();

    if home_page == "No-Intro" {
        let dat_id = game_node
            .dat
            .as_ref()
            .and_then(|d| d.borrow().get_data(DatData::Id))
            .unwrap_or_default();
        if dat_id.trim().is_empty() {
            return false;
        }
        let url = format!(
            "https://datomatic.no-intro.org/index.php?page=show_record&s={}&n={}",
            dat_id, game_id
        );
        return std::process::Command::new("cmd")
            .args(["/C", "start", &url])
            .spawn()
            .is_ok();
    }

    if home_page == "redump.org" {
        let url = format!("http://redump.org/disc/{}/", game_id);
        return std::process::Command::new("cmd")
            .args(["/C", "start", &url])
            .spawn()
            .is_ok();
    }

    false
}

fn file_group_match(needle: &RvFile, candidate: &RvFile) -> bool {
    if needle.size.is_some() && candidate.size.is_some() && needle.size != candidate.size {
        return false;
    }

    let mut has_any = false;

    if let Some(ref crc) = needle.crc {
        has_any = true;
        if candidate.crc.as_ref() != Some(crc) && candidate.alt_crc.as_ref() != Some(crc) {
            return false;
        }
    }
    if let Some(ref alt_crc) = needle.alt_crc {
        has_any = true;
        if candidate.crc.as_ref() != Some(alt_crc) && candidate.alt_crc.as_ref() != Some(alt_crc) {
            return false;
        }
    }
    if let Some(ref sha1) = needle.sha1 {
        has_any = true;
        if candidate.sha1.as_ref() != Some(sha1) && candidate.alt_sha1.as_ref() != Some(sha1) {
            return false;
        }
    }
    if let Some(ref alt_sha1) = needle.alt_sha1 {
        has_any = true;
        if candidate.sha1.as_ref() != Some(alt_sha1) && candidate.alt_sha1.as_ref() != Some(alt_sha1) {
            return false;
        }
    }
    if let Some(ref md5) = needle.md5 {
        has_any = true;
        if candidate.md5.as_ref() != Some(md5) && candidate.alt_md5.as_ref() != Some(md5) {
            return false;
        }
    }
    if let Some(ref alt_md5) = needle.alt_md5 {
        has_any = true;
        if candidate.md5.as_ref() != Some(alt_md5) && candidate.alt_md5.as_ref() != Some(alt_md5) {
            return false;
        }
    }

    has_any
}

fn collect_rom_occurrence_lines(needle_rc: Rc<RefCell<RvFile>>) -> Vec<String> {
    let needle = needle_rc.borrow();
    let mut out = Vec::new();

    GLOBAL_DB.with(|db_ref| {
        let binding = db_ref.borrow();
        let Some(db) = binding.as_ref() else {
            return;
        };
        let root = Rc::clone(&db.dir_root);
        drop(binding);

        let mut stack = vec![root];
        while let Some(node_rc) = stack.pop() {
            let n = node_rc.borrow();
            let children = n.children.clone();
            for child in children {
                stack.push(child);
            }

            if n.is_file() && n.game.is_none() {
                if file_group_match(&needle, &n) {
                    out.push(format!("{:?} | {}", n.got_status(), n.get_full_name()));
                }
            }
        }
    });

    out.sort();
    out
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RomStatusBucket {
    Correct,
    Missing,
    Fixes,
    Merged,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct GridVisibilityFlags {
    correct: bool,
    missing: bool,
    fixes: bool,
    mia: bool,
    merged: bool,
    unknown: bool,
}

fn grid_visibility_flags_from_stats(stats: &rv_core::repair_status::RepairStatus) -> GridVisibilityFlags {
    let total_roms = stats.total_roms;
    let merged_roms = stats.roms_not_collected + stats.roms_unneeded;
    let correct_roms = stats.count_correct();
    GridVisibilityFlags {
        correct: total_roms > 0 && correct_roms == total_roms,
        missing: stats.roms_missing > 0 || stats.roms_missing_mia > 0,
        fixes: stats.roms_fixes > 0 || stats.roms_unneeded > 0,
        mia: stats.roms_missing_mia > 0 || stats.roms_correct_mia > 0 || (total_roms > 0 && stats.roms_fixes == total_roms),
        merged: total_roms > 0 && merged_roms == total_roms,
        unknown: stats.roms_unknown > 0,
    }
}

fn grid_visibility_flags_from_report_status(report_status: rv_core::enums::ReportStatus) -> GridVisibilityFlags {
    GridVisibilityFlags {
        correct: report_status.has_correct(),
        missing: report_status.has_missing(false),
        fixes: report_status.has_fixes_needed(),
        mia: report_status.has_mia(),
        merged: report_status.has_all_merged(),
        unknown: report_status.has_unknown(),
    }
}

fn game_row_color(rep_status: RepStatus) -> egui::Color32 {
    match rep_status {
        RepStatus::Correct | RepStatus::CorrectMIA | RepStatus::DirCorrect => egui::Color32::from_rgb(40, 80, 40),
        RepStatus::Missing | RepStatus::MissingMIA | RepStatus::DirMissing | RepStatus::DirCorrupt | RepStatus::Corrupt | RepStatus::Incomplete => {
            egui::Color32::from_rgb(80, 40, 40)
        }
        RepStatus::CanBeFixed | RepStatus::CanBeFixedMIA | RepStatus::CorruptCanBeFixed => {
            egui::Color32::from_rgb(80, 80, 40)
        }
        RepStatus::MoveToSort | RepStatus::MoveToCorrupt | RepStatus::NeededForFix | RepStatus::Rename | RepStatus::InToSort | RepStatus::DirInToSort => {
            egui::Color32::from_rgb(40, 80, 80)
        }
        RepStatus::NotCollected | RepStatus::UnNeeded | RepStatus::Unknown | RepStatus::DirUnknown | RepStatus::UnScanned | RepStatus::Ignore => {
            egui::Color32::from_rgb(60, 60, 60)
        }
        RepStatus::Delete | RepStatus::Deleted => egui::Color32::from_rgb(120, 0, 0),
        _ => egui::Color32::TRANSPARENT,
    }
}

fn rom_row_color(rep_status: RepStatus) -> egui::Color32 {
    match rep_status {
        RepStatus::Correct | RepStatus::CorrectMIA | RepStatus::DirCorrect => egui::Color32::from_rgb(40, 80, 40),
        RepStatus::Missing | RepStatus::MissingMIA | RepStatus::DirMissing | RepStatus::DirCorrupt | RepStatus::Corrupt | RepStatus::Incomplete => {
            egui::Color32::from_rgb(80, 40, 40)
        }
        RepStatus::CanBeFixed | RepStatus::CanBeFixedMIA | RepStatus::CorruptCanBeFixed => {
            egui::Color32::from_rgb(80, 80, 40)
        }
        RepStatus::MoveToSort | RepStatus::MoveToCorrupt | RepStatus::NeededForFix | RepStatus::Rename | RepStatus::InToSort | RepStatus::DirInToSort => {
            egui::Color32::from_rgb(40, 80, 80)
        }
        RepStatus::NotCollected | RepStatus::UnNeeded | RepStatus::Unknown | RepStatus::DirUnknown | RepStatus::UnScanned | RepStatus::Ignore => {
            egui::Color32::from_rgb(60, 60, 60)
        }
        RepStatus::Delete | RepStatus::Deleted => egui::Color32::from_rgb(120, 0, 0),
        _ => egui::Color32::TRANSPARENT,
    }
}

fn game_summary_bucket(rep_status: RepStatus) -> Option<RomStatusBucket> {
    match rep_status {
        RepStatus::Correct | RepStatus::CorrectMIA | RepStatus::DirCorrect => Some(RomStatusBucket::Correct),
        RepStatus::Missing | RepStatus::MissingMIA | RepStatus::DirMissing | RepStatus::DirCorrupt | RepStatus::Corrupt | RepStatus::Incomplete => {
            Some(RomStatusBucket::Missing)
        }
        RepStatus::CanBeFixed
        | RepStatus::CanBeFixedMIA
        | RepStatus::CorruptCanBeFixed
        | RepStatus::DirInToSort
        | RepStatus::InToSort
        | RepStatus::MoveToSort
        | RepStatus::Delete
        | RepStatus::Deleted
        | RepStatus::NeededForFix
        | RepStatus::Rename
        | RepStatus::MoveToCorrupt => Some(RomStatusBucket::Fixes),
        RepStatus::NotCollected | RepStatus::UnNeeded => Some(RomStatusBucket::Merged),
        RepStatus::Unknown | RepStatus::DirUnknown | RepStatus::UnScanned => Some(RomStatusBucket::Unknown),
        _ => None,
    }
}

fn rom_status_icon_idx(rep_status: RepStatus) -> i32 {
    match rep_status {
        RepStatus::Correct | RepStatus::DirCorrect => 0,
        RepStatus::CorrectMIA => 1,
        RepStatus::Missing | RepStatus::DirMissing => 2,
        RepStatus::DirCorrupt => 3,
        RepStatus::MissingMIA => 4,
        RepStatus::CanBeFixed => 5,
        RepStatus::CanBeFixedMIA => 6,
        RepStatus::CorruptCanBeFixed => 7,
        RepStatus::MoveToSort => 8,
        RepStatus::MoveToCorrupt => 9,
        RepStatus::InToSort | RepStatus::DirInToSort => 10,
        RepStatus::NeededForFix => 11,
        RepStatus::Rename => 12,
        RepStatus::Delete | RepStatus::Deleted => 13,
        RepStatus::NotCollected => 14,
        RepStatus::UnNeeded => 15,
        RepStatus::Unknown | RepStatus::DirUnknown => 16,
        RepStatus::Corrupt => 17,
        RepStatus::Incomplete => 18,
        RepStatus::UnScanned => 19,
        RepStatus::Ignore => 20,
        _ => 16,
    }
}

/// Logic for rendering the DataGridView component.
/// 
/// `grids.rs` contains the logic for rendering the right-hand panel of the main UI,
/// which displays the children of the currently selected tree node in a tabular format.
/// 
/// Differences from C#:
/// - C# utilizes the stateful `WinForms.DataGridView` control.
/// - The Rust version manually draws an `egui::Grid`, dynamically fetching the currently 
///   selected node from the `RomVaultApp` state and rendering its children every frame.
impl RomVaultApp {
    pub fn draw_game_grid(&mut self, ui: &mut egui::Ui) {
        let selection_color = ui.style().visuals.selection.bg_fill;

        enum GridAction {
            ScanQuick(Rc<RefCell<RvFile>>),
            ScanNormal(Rc<RefCell<RvFile>>),
            ScanFull(Rc<RefCell<RvFile>>),
            NavigateUp,
            NavigateDown(Rc<RefCell<RvFile>>),
            LaunchEmulator(Rc<RefCell<RvFile>>),
            OpenWebPage(Rc<RefCell<RvFile>>),
        }
        let mut pending_action = None;

        let mut new_sort_col = self.sort_col.clone();
        let mut new_sort_desc = self.sort_desc;

        egui::ScrollArea::horizontal().show(ui, |ui| {
            egui_extras::TableBuilder::new(ui)
                .striped(true)
                .resizable(true)
                .vscroll(true)
                .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
                .column(egui_extras::Column::initial(40.0).at_least(40.0))
                .column(egui_extras::Column::initial(350.0).at_least(40.0))
                .column(egui_extras::Column::initial(350.0).at_least(40.0))
                .column(egui_extras::Column::initial(150.0).at_least(40.0))
                .column(egui_extras::Column::remainder())
                .header(20.0, |mut header| {
                    let mut make_header = |ui: &mut egui::Ui, title: &str| {
                        if ui
                            .selectable_label(new_sort_col.as_deref() == Some(title), title)
                            .clicked()
                        {
                            if new_sort_col.as_deref() == Some(title) {
                                new_sort_desc = !new_sort_desc;
                            } else {
                                new_sort_col = Some(title.to_string());
                                new_sort_desc = false;
                            }
                        }
                    };
                    header.col(|ui| make_header(ui, "Type"));
                    header.col(|ui| make_header(ui, "Game (Directory / Zip)"));
                    header.col(|ui| make_header(ui, "Description"));
                    header.col(|ui| make_header(ui, "Modified"));
                    header.col(|ui| make_header(ui, "ROM Status"));
                })
                .body(|mut body| {
                    if let Some(selected) = &self.selected_node {
                        let node = selected.borrow();

                        if node.parent.is_some() {
                            body.row(20.0, |mut row| {
                                row.col(|ui| {
                                    ui.add(
                                        egui::Image::new(include_asset!("Dir.png"))
                                            .texture_options(egui::TextureOptions::NEAREST)
                                            .max_width(16.0),
                                    );
                                });
                                row.col(|ui| {
                                    let label_resp = ui.add(egui::SelectableLabel::new(false, ".."));
                                    if label_resp.double_clicked() {
                                        pending_action = Some(GridAction::NavigateUp);
                                    }
                                    if label_resp.hovered()
                                        && ui.input(|i| {
                                            i.pointer
                                                .button_double_clicked(egui::PointerButton::Secondary)
                                        })
                                    {
                                        pending_action = Some(GridAction::NavigateUp);
                                    }
                                });
                                row.col(|ui| {
                                    ui.label("");
                                });
                                row.col(|ui| {
                                    ui.label("");
                                });
                                row.col(|ui| {
                                    ui.label("");
                                });
                            });
                        }

                        let mut sorted_children: Vec<Rc<RefCell<RvFile>>> = node
                            .children
                            .iter()
                            .filter(|c| !c.borrow().is_file() || c.borrow().game.is_some())
                            .cloned()
                            .collect();

                        if let Some(col) = &self.sort_col {
                            let desc = self.sort_desc;
                            sorted_children.sort_by(|a, b| {
                                let a = a.borrow();
                                let b = b.borrow();
                                let cmp = match col.as_str() {
                                    "Game (Directory / Zip)" => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
                                    "Description" => {
                                        let da = a
                                            .game
                                            .as_ref()
                                            .and_then(|g| {
                                                g.borrow().get_data(rv_core::rv_game::GameData::Description)
                                            })
                                            .unwrap_or_default();
                                        let db = b
                                            .game
                                            .as_ref()
                                            .and_then(|g| {
                                                g.borrow().get_data(rv_core::rv_game::GameData::Description)
                                            })
                                            .unwrap_or_default();
                                        da.cmp(&db)
                                    }
                                    "Type" => a.file_type.cmp(&b.file_type),
                                    "Modified" => a.file_mod_time_stamp.cmp(&b.file_mod_time_stamp),
                                    _ => a.name.cmp(&b.name),
                                };
                                if desc { cmp.reverse() } else { cmp }
                            });
                        }

                        for child_rc in sorted_children {
                            let child = child_rc.borrow();

                            if child.is_file() && child.game.is_none() {
                                continue;
                            }

                            let mut should_show = false;

                            let visibility_flags = if let Some(stats) = &child.cached_stats {
                                Some(grid_visibility_flags_from_stats(stats))
                            } else {
                                child.dir_status.map(grid_visibility_flags_from_report_status)
                            };

                            if let Some(flags) = visibility_flags {
                                let g_correct = flags.correct;
                                let g_missing = flags.missing;
                                let g_fixes = flags.fixes;
                                let g_mia = flags.mia;
                                let g_merged = flags.merged;
                                let g_unknown = flags.unknown;

                                should_show =
                                    should_show || (self.show_complete && g_correct && !g_missing && !g_fixes);
                                should_show = should_show || (self.show_partial && g_correct && g_missing);
                                should_show = should_show || (self.show_empty && !g_correct && g_missing);
                                should_show = should_show || (self.show_fixes && g_fixes);
                                should_show = should_show || (self.show_mia && g_mia);
                                should_show = should_show || (self.show_merged && g_merged);
                                should_show = should_show || g_unknown;

                                if !g_correct && !g_missing && !g_unknown && !g_fixes && !g_mia && !g_merged {
                                    should_show = true;
                                }
                            } else {
                                should_show = true;
                            }

                            if !self.filter_text.is_empty() {
                                if !child
                                    .name
                                    .to_lowercase()
                                    .contains(&self.filter_text.to_lowercase())
                                {
                                    should_show = false;
                                }
                            }

                            if !should_show {
                                continue;
                            }

                            let description = if let Some(ref g) = child.game {
                                g.borrow()
                                    .get_data(rv_core::rv_game::GameData::Description)
                                    .unwrap_or_default()
                            } else {
                                "".to_string()
                            };

                            let mut row_color = game_row_color(child.rep_status());

                            let is_selected =
                                self.selected_game.as_ref().map_or(false, |s| Rc::ptr_eq(s, &child_rc));
                            if is_selected {
                                row_color = selection_color;
                            }

                            let file_icon = match child.file_type {
                                FileType::Dir => include_asset!("Dir.png"),
                                FileType::Zip => include_asset!("Zip.png"),
                                FileType::SevenZip => include_asset!("SevenZip.png"),
                                _ => include_asset!("default2.png"),
                            };

                            body.row(20.0, |mut row| {
                                row.col(|ui| {
                                    ui.painter().rect_filled(ui.max_rect(), 0.0, row_color);
                                    let resp = ui.interact(
                                        ui.max_rect(),
                                        ui.make_persistent_id((child.name.clone(), "game_type_cell")),
                                        egui::Sense::click(),
                                    );
                                    ui.add(
                                        egui::Image::new(file_icon)
                                            .texture_options(egui::TextureOptions::NEAREST)
                                            .max_width(16.0),
                                    );
                                    if resp.secondary_clicked() && !ui.input(|i| i.modifiers.shift) {
                                        if let Some(text) = game_clipboard_text(&child, &description, GameGridCopyColumn::Type) {
                                            ui.output_mut(|o| o.copied_text = text);
                                            self.task_logs.push("Copied game info".to_string());
                                        }
                                    }
                                });
                                row.col(|ui| {
                                    ui.painter().rect_filled(ui.max_rect(), 0.0, row_color);
                                    let label_resp =
                                        ui.add(egui::SelectableLabel::new(is_selected, &child.name));

                                    if label_resp.secondary_clicked() && !ui.input(|i| i.modifiers.shift) {
                                        if let Some(text) = game_clipboard_text(&child, &description, GameGridCopyColumn::Game) {
                                            ui.output_mut(|o| o.copied_text = text.clone());
                                            self.task_logs.push(format!("Copied: {}", text));
                                        }
                                    }
                                    if ui.input(|i| i.modifiers.shift) {
                                        label_resp.context_menu(|ui| {
                                            let mut has_open_target = false;

                                            if child.file_type == FileType::Dir && !self.sam_running {
                                                if ui.button("Scan").clicked() {
                                                    pending_action =
                                                        Some(GridAction::ScanNormal(Rc::clone(&child_rc)));
                                                    ui.close_menu();
                                                }
                                                if ui.button("Scan Quick (Headers Only)").clicked() {
                                                    pending_action =
                                                        Some(GridAction::ScanQuick(Rc::clone(&child_rc)));
                                                    ui.close_menu();
                                                }
                                                if ui.button("Scan Full (Complete Re-Scan)").clicked() {
                                                    pending_action =
                                                        Some(GridAction::ScanFull(Rc::clone(&child_rc)));
                                                    ui.close_menu();
                                                }
                                                ui.separator();
                                            }

                                            let full_path = get_full_node_path(Rc::clone(&child_rc));
                                            if child.file_type == FileType::Dir {
                                                if std::path::Path::new(&full_path).is_dir() {
                                                    has_open_target = true;
                                                    if ui.button("Open Dir").clicked() {
                                                        self.task_logs.push(format!("Opening Dir: {}", full_path));
                                                        let _ = std::process::Command::new("explorer")
                                                            .arg(&full_path)
                                                            .spawn();
                                                        ui.close_menu();
                                                    }
                                                }
                                            } else if matches!(child.file_type, FileType::Zip | FileType::SevenZip) {
                                                if std::path::Path::new(&full_path).is_file() {
                                                    has_open_target = true;
                                                    let label = if child.file_type == FileType::Zip {
                                                        "Open Zip"
                                                    } else {
                                                        "Open 7Zip"
                                                    };
                                                    if ui.button(label).clicked() {
                                                        self.task_logs.push(format!("Opening: {}", full_path));
                                                        let _ = std::process::Command::new("cmd")
                                                            .args(["/C", "start", "", &full_path])
                                                            .spawn();
                                                        ui.close_menu();
                                                    }
                                                }
                                            }

                                            let parent_path = std::path::Path::new(&full_path)
                                                .parent()
                                                .unwrap_or_else(|| std::path::Path::new(""))
                                                .to_string_lossy()
                                                .to_string();
                                            if std::path::Path::new(&parent_path).is_dir() {
                                                has_open_target = true;
                                                if ui.button("Open Parent").clicked() {
                                                    self.task_logs.push(format!("Opening Parent: {}", parent_path));
                                                    let _ = std::process::Command::new("explorer")
                                                        .arg(&parent_path)
                                                        .spawn();
                                                    ui.close_menu();
                                                }
                                            }

                                            if has_open_target {
                                                if let Some(parent_rc) = child_rc.borrow().parent.as_ref().and_then(|p| p.upgrade()) {
                                                    if emulator_info_for_game_dir(parent_rc).is_some() {
                                                        if ui.button("Launch emulator").clicked() {
                                                            pending_action = Some(GridAction::LaunchEmulator(Rc::clone(&child_rc)));
                                                            ui.close_menu();
                                                        }
                                                    }
                                                }
                                            }

                                            let home_page = child
                                                .dat
                                                .as_ref()
                                                .and_then(|d| d.borrow().get_data(DatData::HomePage))
                                                .unwrap_or_default();
                                            let has_no_intro = home_page == "No-Intro"
                                                && child
                                                    .dat
                                                    .as_ref()
                                                    .and_then(|d| d.borrow().get_data(DatData::Id))
                                                    .map(|s| !s.trim().is_empty())
                                                    .unwrap_or(false)
                                                && child
                                                    .game
                                                    .as_ref()
                                                    .and_then(|g| g.borrow().get_data(rv_core::rv_game::GameData::Id))
                                                    .map(|s| !s.trim().is_empty())
                                                    .unwrap_or(false);
                                            let has_redump = home_page == "redump.org"
                                                && child
                                                    .game
                                                    .as_ref()
                                                    .and_then(|g| g.borrow().get_data(rv_core::rv_game::GameData::Id))
                                                    .map(|s| !s.trim().is_empty())
                                                    .unwrap_or(false);
                                            if has_no_intro || has_redump {
                                                if ui.button("Open Web Page").clicked() {
                                                    pending_action = Some(GridAction::OpenWebPage(Rc::clone(&child_rc)));
                                                    ui.close_menu();
                                                }
                                            }

                                            if ui.button("Copy Info").clicked() {
                                                let info = format!("Name: {}\nDesc: {}", child.name, description);
                                                ui.output_mut(|o| o.copied_text = info);
                                                self.task_logs.push("Copied Game Info".to_string());
                                                ui.close_menu();
                                            }
                                        });
                                    }

                                    if label_resp.double_clicked() {
                                        if child.game.is_none() && child.file_type == FileType::Dir {
                                            pending_action =
                                                Some(GridAction::NavigateDown(Rc::clone(&child_rc)));
                                        } else {
                                            pending_action = Some(GridAction::LaunchEmulator(Rc::clone(&child_rc)));
                                        }
                                    } else if label_resp.clicked() {
                                        self.selected_game = Some(Rc::clone(&child_rc));
                                    }

                                    if label_resp.hovered()
                                        && ui.input(|i| {
                                            i.pointer
                                                .button_double_clicked(egui::PointerButton::Secondary)
                                        })
                                    {
                                        pending_action = Some(GridAction::NavigateUp);
                                    }
                                });
                                row.col(|ui| {
                                    ui.painter().rect_filled(ui.max_rect(), 0.0, row_color);
                                    let resp = ui.interact(
                                        ui.max_rect(),
                                        ui.make_persistent_id((child.name.clone(), "game_desc_cell")),
                                        egui::Sense::click(),
                                    );
                                    ui.label(description.clone());
                                    if resp.secondary_clicked() && !ui.input(|i| i.modifiers.shift) {
                                        if let Some(text) = game_clipboard_text(&child, &description, GameGridCopyColumn::Description) {
                                            ui.output_mut(|o| o.copied_text = text.clone());
                                            self.task_logs.push(format!("Copied: {}", text));
                                        }
                                    }
                                });
                                row.col(|ui| {
                                    ui.painter().rect_filled(ui.max_rect(), 0.0, row_color);
                                    let resp = ui.interact(
                                        ui.max_rect(),
                                        ui.make_persistent_id((child.name.clone(), "game_modified_cell")),
                                        egui::Sense::click(),
                                    );
                                    let time_str = compress::compress_utils::zip_date_time_to_string(Some(child.file_mod_time_stamp));
                                    ui.label(time_str);
                                    if resp.secondary_clicked() && !ui.input(|i| i.modifiers.shift) {
                                        if let Some(text) = game_clipboard_text(&child, &description, GameGridCopyColumn::Modified) {
                                            ui.output_mut(|o| o.copied_text = text);
                                            self.task_logs.push("Copied game info".to_string());
                                        }
                                    }
                                });
                                row.col(|ui| {
                                    ui.painter().rect_filled(ui.max_rect(), 0.0, row_color);
                                    let resp = ui.interact(
                                        ui.max_rect(),
                                        ui.make_persistent_id((child.name.clone(), "game_rom_status_cell")),
                                        egui::Sense::click(),
                                    );
                                    ui.horizontal(|ui| {
                                        let mut correct = 0;
                                        let mut missing = 0;
                                        let mut fixes = 0;
                                        let mut merged = 0;
                                        let mut unknown = 0;

                                        for rom in &child.children {
                                            match game_summary_bucket(rom.borrow().rep_status()) {
                                                Some(RomStatusBucket::Correct) => correct += 1,
                                                Some(RomStatusBucket::Missing) => missing += 1,
                                                Some(RomStatusBucket::Fixes) => fixes += 1,
                                                Some(RomStatusBucket::Merged) => merged += 1,
                                                Some(RomStatusBucket::Unknown) => unknown += 1,
                                                None => {}
                                            }
                                        }

                                        if correct > 0 {
                                            ui.add(
                                                egui::Image::new(include_asset!("G_Correct.png"))
                                                    .texture_options(egui::TextureOptions::NEAREST)
                                                    .max_width(16.0),
                                            );
                                            ui.label(correct.to_string());
                                        }
                                        if missing > 0 {
                                            ui.add(
                                                egui::Image::new(include_asset!("G_Missing.png"))
                                                    .texture_options(egui::TextureOptions::NEAREST)
                                                    .max_width(16.0),
                                            );
                                            ui.label(missing.to_string());
                                        }
                                        if fixes > 0 {
                                            ui.add(
                                                egui::Image::new(include_asset!("G_CanBeFixed.png"))
                                                    .texture_options(egui::TextureOptions::NEAREST)
                                                    .max_width(16.0),
                                            );
                                            ui.label(fixes.to_string());
                                        }
                                        if merged > 0 {
                                            ui.add(
                                                egui::Image::new(include_asset!("G_UnNeeded.png"))
                                                    .texture_options(egui::TextureOptions::NEAREST)
                                                    .max_width(16.0),
                                            );
                                            ui.label(merged.to_string());
                                        }
                                        if unknown > 0 {
                                            ui.add(
                                                egui::Image::new(include_asset!("G_Unknown.png"))
                                                    .texture_options(egui::TextureOptions::NEAREST)
                                                    .max_width(16.0),
                                            );
                                            ui.label(unknown.to_string());
                                        }
                                    });
                                    if resp.secondary_clicked() && !ui.input(|i| i.modifiers.shift) {
                                        if let Some(text) = game_clipboard_text(&child, &description, GameGridCopyColumn::RomStatus) {
                                            ui.output_mut(|o| o.copied_text = text);
                                            self.task_logs.push("Copied game info".to_string());
                                        }
                                    }
                                });
                            });
                        }
                    }
                });
        });

        if let Some(action) = pending_action {
            match action {
                GridAction::ScanQuick(target_rc) => {
                    let name = target_rc.borrow().name.clone();
                    let np = get_full_node_path(Rc::clone(&target_rc));
                    self.launch_task("Scan ROMs (Quick)", move |tx| {
                        let _ = tx.send(format!("Scanning {} (Headers Only)...", name));
                        let files = Scanner::scan_directory_with_level(&np, rv_core::settings::EScanLevel::Level1);
                        let mut root_scan = rv_core::scanned_file::ScannedFile::new(FileType::Dir);
                        root_scan.children = files;
                        let _ = tx.send("Integrating files into DB...".to_string());
                        FileScanning::scan_dir_with_level(target_rc, &mut root_scan, rv_core::settings::EScanLevel::Level1);
                    });
                }
                GridAction::ScanNormal(target_rc) => {
                    let name = target_rc.borrow().name.clone();
                    let np = get_full_node_path(Rc::clone(&target_rc));
                    self.launch_task("Scan ROMs", move |tx| {
                        let _ = tx.send(format!("Scanning {}...", name));
                        let files = Scanner::scan_directory_with_level(&np, rv_core::settings::EScanLevel::Level2);
                        let mut root_scan = rv_core::scanned_file::ScannedFile::new(FileType::Dir);
                        root_scan.children = files;
                        let _ = tx.send("Integrating files into DB...".to_string());
                        FileScanning::scan_dir_with_level(target_rc, &mut root_scan, rv_core::settings::EScanLevel::Level2);
                    });
                }
                GridAction::ScanFull(target_rc) => {
                    let name = target_rc.borrow().name.clone();
                    let np = get_full_node_path(Rc::clone(&target_rc));
                    self.launch_task("Scan ROMs (Full)", move |tx| {
                        let _ = tx.send(format!("Scanning {} (Full Re-Scan)...", name));
                        let files = Scanner::scan_directory_with_level(&np, rv_core::settings::EScanLevel::Level3);
                        let mut root_scan = rv_core::scanned_file::ScannedFile::new(FileType::Dir);
                        root_scan.children = files;
                        let _ = tx.send("Integrating files into DB...".to_string());
                        FileScanning::scan_dir_with_level(target_rc, &mut root_scan, rv_core::settings::EScanLevel::Level3);
                    });
                }
                GridAction::NavigateUp => {
                    let mut new_selected = None;
                    if let Some(selected) = &self.selected_node {
                        if let Some(parent) = &selected.borrow().parent {
                            if let Some(parent_rc) = parent.upgrade() {
                                new_selected = Some(parent_rc);
                            }
                        }
                    }
                    if let Some(ns) = new_selected {
                        self.select_node(ns);
                    }
                }
                GridAction::NavigateDown(target_rc) => {
                    self.select_node(target_rc);
                }
                GridAction::LaunchEmulator(target_rc) => {
                    let game = target_rc.borrow();
                    if launch_emulator_for_game(&game) {
                        self.task_logs.push(format!("Launch emulator: {}", game.name));
                    } else {
                        self.task_logs.push("Launch emulator failed.".to_string());
                    }
                }
                GridAction::OpenWebPage(target_rc) => {
                    let game = target_rc.borrow();
                    if !open_web_page_for_game(&game) {
                        self.task_logs.push("No Web Page mapping available for this game.".to_string());
                    }
                }
            }
        }

        self.sort_col = new_sort_col;
        self.sort_desc = new_sort_desc;
    }

    pub fn draw_rom_grid(&mut self, ui: &mut egui::Ui) {
        let mut new_sort_col_rom = self.sort_col.clone();
        let mut new_sort_desc_rom = self.sort_desc;

        egui::ScrollArea::horizontal().show(ui, |ui| {
            egui_extras::TableBuilder::new(ui)
                .striped(true)
                .resizable(true)
                .vscroll(true)
                .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
                .column(egui_extras::Column::initial(40.0).at_least(40.0))
                .column(egui_extras::Column::initial(350.0).at_least(40.0))
                .column(egui_extras::Column::initial(100.0).at_least(40.0))
                .column(egui_extras::Column::initial(100.0).at_least(40.0))
                .column(egui_extras::Column::initial(150.0).at_least(40.0))
                .column(egui_extras::Column::initial(200.0).at_least(40.0))
                .column(egui_extras::Column::initial(200.0).at_least(40.0))
                .column(egui_extras::Column::initial(100.0).at_least(40.0))
                .column(egui_extras::Column::initial(150.0).at_least(40.0))
                .column(egui_extras::Column::initial(200.0).at_least(40.0))
                .column(egui_extras::Column::initial(200.0).at_least(40.0))
                .column(egui_extras::Column::initial(100.0).at_least(40.0))
                .column(egui_extras::Column::initial(150.0).at_least(40.0))
                .column(egui_extras::Column::initial(100.0).at_least(40.0))
                .column(egui_extras::Column::remainder())
                .header(20.0, |mut header| {
                    let mut make_header = |ui: &mut egui::Ui, title: &str| {
                        if ui
                            .selectable_label(new_sort_col_rom.as_deref() == Some(title), title)
                            .clicked()
                        {
                            if new_sort_col_rom.as_deref() == Some(title) {
                                new_sort_desc_rom = !new_sort_desc_rom;
                            } else {
                                new_sort_col_rom = Some(title.to_string());
                                new_sort_desc_rom = false;
                            }
                        }
                    };
                    header.col(|ui| {
                        ui.strong("Got");
                    });
                    header.col(|ui| make_header(ui, "ROM (File)"));
                    header.col(|ui| make_header(ui, "Merge"));
                    header.col(|ui| make_header(ui, "Size"));
                    header.col(|ui| make_header(ui, "CRC32"));
                    header.col(|ui| make_header(ui, "SHA1"));
                    header.col(|ui| make_header(ui, "MD5"));
                    header.col(|ui| make_header(ui, "AltSize"));
                    header.col(|ui| make_header(ui, "AltCRC32"));
                    header.col(|ui| make_header(ui, "AltSHA1"));
                    header.col(|ui| make_header(ui, "AltMD5"));
                    header.col(|ui| make_header(ui, "Status"));
                    header.col(|ui| make_header(ui, "FileModDate"));
                    header.col(|ui| make_header(ui, "ZipIndex"));
                    header.col(|ui| make_header(ui, "InstanceCount"));
                })
                .body(|mut body| {
                    if let Some(selected_game) = &self.selected_game {
                        let game = selected_game.borrow();

                        let mut sorted_roms: Vec<Rc<RefCell<RvFile>>> = game
                            .children
                            .iter()
                            .filter(|c| c.borrow().is_file())
                            .cloned()
                            .collect();

                        if let Some(col) = &self.sort_col {
                            let desc = self.sort_desc;
                            sorted_roms.sort_by(|a, b| {
                                let a = a.borrow();
                                let b = b.borrow();
                                let cmp = match col.as_str() {
                                    "ROM (File)" => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
                                    "Size" => a.size.cmp(&b.size),
                                    "Merge" => a.merge.cmp(&b.merge),
                                    "CRC32" => a.crc.cmp(&b.crc),
                                    "SHA1" => a.sha1.cmp(&b.sha1),
                                    "MD5" => a.md5.cmp(&b.md5),
                                    "AltSize" => a.alt_size.cmp(&b.alt_size),
                                    "AltCRC32" => a.alt_crc.cmp(&b.alt_crc),
                                    "AltSHA1" => a.alt_sha1.cmp(&b.alt_sha1),
                                    "AltMD5" => a.alt_md5.cmp(&b.alt_md5),
                                    "Status" => a.status.cmp(&b.status),
                                    "FileModDate" => a.file_mod_time_stamp.cmp(&b.file_mod_time_stamp),
                                    _ => a.name.cmp(&b.name),
                                };
                                if desc { cmp.reverse() } else { cmp }
                            });
                        }

                        for rom_rc in sorted_roms {
                            let rom = rom_rc.borrow();
                            let row_color = rom_row_color(rom.rep_status());

                            let status_icon = match rom_status_icon_idx(rom.rep_status()) {
                                0 => include_asset!("G_Correct.png"),
                                1 => include_asset!("G_CorrectMIA.png"),
                                2 => include_asset!("G_Missing.png"),
                                3 => include_asset!("G_DirCorrupt.png"),
                                4 => include_asset!("G_MissingMIA.png"),
                                5 => include_asset!("G_CanBeFixed.png"),
                                6 => include_asset!("G_CanBeFixedMIA.png"),
                                7 => include_asset!("G_CorruptCanBeFixed.png"),
                                8 => include_asset!("G_MoveToSort.png"),
                                9 => include_asset!("G_MoveToCorrupt.png"),
                                10 => include_asset!("G_InToSort.png"),
                                11 => include_asset!("G_NeededForFix.png"),
                                12 => include_asset!("G_Rename.png"),
                                13 => include_asset!("G_Delete.png"),
                                14 => include_asset!("G_NotCollected.png"),
                                15 => include_asset!("G_UnNeeded.png"),
                                17 => include_asset!("G_Corrupt.png"),
                                18 => include_asset!("G_Incomplete.png"),
                                19 => include_asset!("G_UnScanned.png"),
                                20 => include_asset!("G_Ignore.png"),
                                _ => include_asset!("G_Unknown.png"),
                            };

                            body.row(20.0, |mut row| {
                                row.col(|ui| {
                                    ui.painter().rect_filled(ui.max_rect(), 0.0, row_color);
                                    let resp = ui.add(
                                        egui::Image::new(status_icon)
                                            .texture_options(egui::TextureOptions::NEAREST)
                                            .max_width(16.0),
                                    );
                                    if resp.secondary_clicked() {
                                        if let Some(info) = rom_clipboard_text(&rom, RomGridCopyColumn::Got) {
                                            ui.output_mut(|o| o.copied_text = info);
                                            self.task_logs.push("Copied ROM info".to_string());
                                        }
                                    }
                                });
                                row.col(|ui| {
                                    ui.painter().rect_filled(ui.max_rect(), 0.0, row_color);
                                    let label_resp = ui.add(egui::SelectableLabel::new(false, &rom.name));
                                    if label_resp.secondary_clicked() {
                                        if let Some(text) = rom_clipboard_text(&rom, RomGridCopyColumn::Rom) {
                                            ui.output_mut(|o| o.copied_text = text.clone());
                                            self.task_logs.push(format!("Copied: {}", text));
                                        }
                                    }
                                    label_resp.context_menu(|ui| {
                                        if ui.button("Copy ROM Name").clicked() {
                                            ui.output_mut(|o| o.copied_text = rom.name.clone());
                                            self.task_logs.push(format!("Copied: {}", rom.name));
                                            ui.close_menu();
                                        }
                                    });
                                });
                                row.col(|ui| {
                                    ui.painter().rect_filled(ui.max_rect(), 0.0, row_color);
                                    ui.label(&rom.merge);
                                });
                                row.col(|ui| {
                                    ui.painter().rect_filled(ui.max_rect(), 0.0, row_color);
                                    let text = rom.size.map(|s| s.to_string()).unwrap_or_else(|| "-".to_string());
                                    let resp = ui.add(egui::Label::new(text).sense(egui::Sense::click()));
                                    if resp.secondary_clicked() {
                                        if let Some(copy) = rom_clipboard_text(&rom, RomGridCopyColumn::Size) {
                                            ui.output_mut(|o| o.copied_text = copy.clone());
                                            self.task_logs.push(format!("Copied: {}", copy));
                                        }
                                    }
                                });
                                row.col(|ui| {
                                    ui.painter().rect_filled(ui.max_rect(), 0.0, row_color);
                                    let text = rom
                                        .crc
                                        .as_ref()
                                        .map(|b| hex::encode(b))
                                        .unwrap_or_else(|| "-".to_string());
                                    let resp = ui.add(egui::Label::new(text).sense(egui::Sense::click()));
                                    if resp.secondary_clicked() {
                                        if let Some(copy) = rom_clipboard_text(&rom, RomGridCopyColumn::Crc32) {
                                            ui.output_mut(|o| o.copied_text = copy.clone());
                                            self.task_logs.push(format!("Copied: {}", copy));
                                        }
                                    }
                                });
                                row.col(|ui| {
                                    ui.painter().rect_filled(ui.max_rect(), 0.0, row_color);
                                    let text = rom
                                        .sha1
                                        .as_ref()
                                        .map(|b| hex::encode(b))
                                        .unwrap_or_else(|| "-".to_string());
                                    let resp = ui.add(egui::Label::new(text).sense(egui::Sense::click()));
                                    if resp.secondary_clicked() {
                                        if let Some(copy) = rom_clipboard_text(&rom, RomGridCopyColumn::Sha1) {
                                            ui.output_mut(|o| o.copied_text = copy.clone());
                                            self.task_logs.push(format!("Copied: {}", copy));
                                        }
                                    }
                                });
                                row.col(|ui| {
                                    ui.painter().rect_filled(ui.max_rect(), 0.0, row_color);
                                    let text = rom
                                        .md5
                                        .as_ref()
                                        .map(|b| hex::encode(b))
                                        .unwrap_or_else(|| "-".to_string());
                                    let resp = ui.add(egui::Label::new(text).sense(egui::Sense::click()));
                                    if resp.secondary_clicked() {
                                        if let Some(copy) = rom_clipboard_text(&rom, RomGridCopyColumn::Md5) {
                                            ui.output_mut(|o| o.copied_text = copy.clone());
                                            self.task_logs.push(format!("Copied: {}", copy));
                                        }
                                    }
                                });
                                row.col(|ui| {
                                    ui.painter().rect_filled(ui.max_rect(), 0.0, row_color);
                                    let text = rom.alt_size.map_or("".to_string(), |s| s.to_string());
                                    let resp = ui.add(egui::Label::new(text).sense(egui::Sense::click()));
                                    if resp.secondary_clicked() {
                                        if let Some(copy) = rom_clipboard_text(&rom, RomGridCopyColumn::AltSize) {
                                            ui.output_mut(|o| o.copied_text = copy.clone());
                                            self.task_logs.push(format!("Copied: {}", copy));
                                        }
                                    }
                                });
                                row.col(|ui| {
                                    ui.painter().rect_filled(ui.max_rect(), 0.0, row_color);
                                    let text = rom.alt_crc.as_ref().map_or("".to_string(), |h| hex::encode(h));
                                    let resp = ui.add(egui::Label::new(text).sense(egui::Sense::click()));
                                    if resp.secondary_clicked() {
                                        if let Some(copy) = rom_clipboard_text(&rom, RomGridCopyColumn::AltCrc32) {
                                            ui.output_mut(|o| o.copied_text = copy.clone());
                                            self.task_logs.push(format!("Copied: {}", copy));
                                        }
                                    }
                                });
                                row.col(|ui| {
                                    ui.painter().rect_filled(ui.max_rect(), 0.0, row_color);
                                    let text = rom.alt_sha1.as_ref().map_or("".to_string(), |h| hex::encode(h));
                                    let resp = ui.add(egui::Label::new(text).sense(egui::Sense::click()));
                                    if resp.secondary_clicked() {
                                        if let Some(copy) = rom_clipboard_text(&rom, RomGridCopyColumn::AltSha1) {
                                            ui.output_mut(|o| o.copied_text = copy.clone());
                                            self.task_logs.push(format!("Copied: {}", copy));
                                        }
                                    }
                                });
                                row.col(|ui| {
                                    ui.painter().rect_filled(ui.max_rect(), 0.0, row_color);
                                    let text = rom.alt_md5.as_ref().map_or("".to_string(), |h| hex::encode(h));
                                    let resp = ui.add(egui::Label::new(text).sense(egui::Sense::click()));
                                    if resp.secondary_clicked() {
                                        if let Some(copy) = rom_clipboard_text(&rom, RomGridCopyColumn::AltMd5) {
                                            ui.output_mut(|o| o.copied_text = copy.clone());
                                            self.task_logs.push(format!("Copied: {}", copy));
                                        }
                                    }
                                });
                                row.col(|ui| {
                                    ui.painter().rect_filled(ui.max_rect(), 0.0, row_color);
                                    ui.label(rom.status.as_deref().unwrap_or(""));
                                });
                                row.col(|ui| {
                                    ui.painter().rect_filled(ui.max_rect(), 0.0, row_color);
                                    ui.label(if rom.file_mod_time_stamp > 0 {
                                        rom.file_mod_time_stamp.to_string()
                                    } else {
                                        "".to_string()
                                    });
                                });
                                row.col(|ui| {
                                    ui.painter().rect_filled(ui.max_rect(), 0.0, row_color);
                                    ui.label("");
                                });
                                row.col(|ui| {
                                    ui.painter().rect_filled(ui.max_rect(), 0.0, row_color);
                                    let instance_count = if matches!(
                                        rom.rep_status(),
                                        RepStatus::Correct
                                            | RepStatus::CorrectMIA
                                            | RepStatus::CanBeFixed
                                            | RepStatus::CanBeFixedMIA
                                    ) {
                                        "1"
                                    } else {
                                        "0"
                                    };
                                    if ui.link(instance_count).clicked() {
                                        self.selected_rom_for_info = Some(Rc::clone(&rom_rc));
                                        self.rom_info_lines = collect_rom_occurrence_lines(Rc::clone(&rom_rc));
                                        self.show_rom_info = true;
                                    }
                                });
                            });
                        }
                    }
                });
        });

        self.sort_col = new_sort_col_rom;
        self.sort_desc = new_sort_desc_rom;
    }
}

#[cfg(test)]
#[path = "tests/grids_tests.rs"]
mod tests;
