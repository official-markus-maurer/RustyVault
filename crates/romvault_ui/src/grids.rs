use eframe::egui;
use std::cell::RefCell;
use std::rc::Rc;

use crate::RomVaultApp;
use crate::utils::get_full_node_path;
use dat_reader::enums::{DatStatus, FileType, GotStatus, ZipStructure};
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

fn trrntzip_name_cmp(a: &str, b: &str) -> std::cmp::Ordering {
    let ab = a.as_bytes();
    let bb = b.as_bytes();
    let len = std::cmp::min(ab.len(), bb.len());
    for i in 0..len {
        let ca = if ab[i].is_ascii_uppercase() { ab[i] + 0x20 } else { ab[i] };
        let cb = if bb[i].is_ascii_uppercase() { bb[i] + 0x20 } else { bb[i] };
        if ca < cb {
            return std::cmp::Ordering::Less;
        }
        if ca > cb {
            return std::cmp::Ordering::Greater;
        }
    }
    match ab.len().cmp(&bb.len()) {
        std::cmp::Ordering::Equal => {
            for i in 0..len {
                if ab[i] < bb[i] {
                    return std::cmp::Ordering::Less;
                }
                if ab[i] > bb[i] {
                    return std::cmp::Ordering::Greater;
                }
            }
            std::cmp::Ordering::Equal
        }
        other => other,
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

            if n.is_file() && n.game.is_none() && file_group_match(&needle, &n) {
                out.push(format!("{:?} | {}", n.got_status(), n.get_full_name()));
            }
        }
    });

    out.sort();
    out
}

fn sort_header_cell(
    ui: &mut egui::Ui,
    title: &str,
    new_sort_col: &mut Option<String>,
    new_sort_desc: &mut bool,
) {
    if ui
        .selectable_label(new_sort_col.as_deref() == Some(title), title)
        .clicked()
    {
        if new_sort_col.as_deref() == Some(title) {
            *new_sort_desc = !*new_sort_desc;
        } else {
            *new_sort_col = Some(title.to_string());
            *new_sort_desc = false;
        }
    }
}

fn game_type_icon_normal(ft: FileType, zs: ZipStructure) -> &'static str {
    match ft {
        FileType::Dir => "Dir.png",
        FileType::Zip => match zs {
            ZipStructure::ZipTrrnt => "ZipTrrnt.png",
            ZipStructure::ZipTDC => "ZipTDC.png",
            ZipStructure::ZipZSTD => "ZipZstd.png",
            _ => "Zip.png",
        },
        FileType::SevenZip => match zs {
            ZipStructure::SevenZipTrrnt => "SevenZipTrrnt.png",
            ZipStructure::SevenZipSLZMA => "SevenZipSLZMA.png",
            ZipStructure::SevenZipNLZMA => "SevenZipNLZMA.png",
            ZipStructure::SevenZipSZSTD => "SevenZipSZSTD.png",
            ZipStructure::SevenZipNZSTD => "SevenZipNZSTD.png",
            _ => "SevenZip.png",
        },
        _ => "default2.png",
    }
}

fn game_type_icon_missing(ft: FileType, zs: ZipStructure) -> &'static str {
    match ft {
        FileType::Dir => "DirMissing.png",
        FileType::Zip => match zs {
            ZipStructure::ZipTrrnt => "ZipTrrntMissing.png",
            ZipStructure::ZipTDC => "ZipTDCMissing.png",
            ZipStructure::ZipZSTD => "ZipZstdMissing.png",
            _ => "ZipMissing.png",
        },
        FileType::SevenZip => match zs {
            ZipStructure::SevenZipSLZMA => "SevenZipSLZMAMissing.png",
            ZipStructure::SevenZipNLZMA => "SevenZipNLZMAMissing.png",
            ZipStructure::SevenZipSZSTD => "SevenZipSZSTDMissing.png",
            ZipStructure::SevenZipNZSTD => "SevenZipNZSTDMissing.png",
            _ => "SevenZipMissing.png",
        },
        _ => "default2.png",
    }
}

fn game_type_icon_corrupt(ft: FileType, zs: ZipStructure) -> &'static str {
    match ft {
        FileType::Zip => match zs {
            ZipStructure::ZipTrrnt => "ZipTrrntCorrupt.png",
            ZipStructure::ZipTDC => "ZipTDCCorrupt.png",
            ZipStructure::ZipZSTD => "ZipZstdCorrupt.png",
            _ => "ZipCorrupt.png",
        },
        FileType::SevenZip => match zs {
            ZipStructure::SevenZipSLZMA => "SevenZipSLZMACorrupt.png",
            ZipStructure::SevenZipNLZMA => "SevenZipNLZMACorrupt.png",
            ZipStructure::SevenZipSZSTD => "SevenZipSZSTDCorrupt.png",
            ZipStructure::SevenZipNZSTD => "SevenZipNZSTDCorrupt.png",
            _ => "SevenZipCorrupt.png",
        },
        _ => game_type_icon_normal(ft, zs),
    }
}

fn game_type_icon_key(ft: FileType, zs: ZipStructure) -> (FileType, ZipStructure) {
    match ft {
        FileType::Zip => match zs {
            ZipStructure::ZipTrrnt | ZipStructure::ZipTDC | ZipStructure::ZipZSTD | ZipStructure::None => (ft, zs),
            _ => (ft, ZipStructure::None),
        },
        FileType::SevenZip => match zs {
            ZipStructure::SevenZipTrrnt
            | ZipStructure::SevenZipSLZMA
            | ZipStructure::SevenZipNLZMA
            | ZipStructure::SevenZipSZSTD
            | ZipStructure::SevenZipNZSTD
            | ZipStructure::None => (ft, zs),
            _ => (ft, ZipStructure::None),
        },
        _ => (ft, ZipStructure::None),
    }
}

fn game_grid_icon_source(icon: &'static str) -> egui::ImageSource<'static> {
    match icon {
        "Dir.png" => include_asset!("Dir.png"),
        "DirMissing.png" => include_asset!("DirMissing.png"),
        "Zip.png" => include_asset!("Zip.png"),
        "ZipMissing.png" => include_asset!("ZipMissing.png"),
        "ZipCorrupt.png" => include_asset!("ZipCorrupt.png"),
        "ZipTrrnt.png" => include_asset!("ZipTrrnt.png"),
        "ZipTrrntMissing.png" => include_asset!("ZipTrrntMissing.png"),
        "ZipTrrntCorrupt.png" => include_asset!("ZipTrrntCorrupt.png"),
        "ZipTDC.png" => include_asset!("ZipTDC.png"),
        "ZipTDCMissing.png" => include_asset!("ZipTDCMissing.png"),
        "ZipTDCCorrupt.png" => include_asset!("ZipTDCCorrupt.png"),
        "ZipZstd.png" => include_asset!("ZipZstd.png"),
        "ZipZstdMissing.png" => include_asset!("ZipZstdMissing.png"),
        "ZipZstdCorrupt.png" => include_asset!("ZipZstdCorrupt.png"),
        "SevenZip.png" => include_asset!("SevenZip.png"),
        "SevenZipMissing.png" => include_asset!("SevenZipMissing.png"),
        "SevenZipCorrupt.png" => include_asset!("SevenZipCorrupt.png"),
        "SevenZipTrrnt.png" => include_asset!("SevenZipTrrnt.png"),
        "SevenZipSLZMA.png" => include_asset!("SevenZipSLZMA.png"),
        "SevenZipSLZMAMissing.png" => include_asset!("SevenZipSLZMAMissing.png"),
        "SevenZipSLZMACorrupt.png" => include_asset!("SevenZipSLZMACorrupt.png"),
        "SevenZipNLZMA.png" => include_asset!("SevenZipNLZMA.png"),
        "SevenZipNLZMAMissing.png" => include_asset!("SevenZipNLZMAMissing.png"),
        "SevenZipNLZMACorrupt.png" => include_asset!("SevenZipNLZMACorrupt.png"),
        "SevenZipSZSTD.png" => include_asset!("SevenZipSZSTD.png"),
        "SevenZipSZSTDMissing.png" => include_asset!("SevenZipSZSTDMissing.png"),
        "SevenZipSZSTDCorrupt.png" => include_asset!("SevenZipSZSTDCorrupt.png"),
        "SevenZipNZSTD.png" => include_asset!("SevenZipNZSTD.png"),
        "SevenZipNZSTDMissing.png" => include_asset!("SevenZipNZSTDMissing.png"),
        "SevenZipNZSTDCorrupt.png" => include_asset!("SevenZipNZSTDCorrupt.png"),
        "ZipConvert.png" => include_asset!("ZipConvert.png"),
        "ZipConvert1.png" => include_asset!("ZipConvert1.png"),
        "default2.png" => include_asset!("default2.png"),
        _ => include_asset!("default2.png"),
    }
}

fn header_file_type_label(t: dat_reader::enums::HeaderFileType) -> &'static str {
    use dat_reader::enums::HeaderFileType as H;

    match t & H::HEADER_MASK {
        H::NOTHING => "Nothing",
        H::ZIP => "ZIP",
        H::GZ => "GZ",
        H::SEVEN_ZIP => "7Z",
        H::RAR => "RAR",
        H::CHD => "CHD",
        H::A7800 => "A7800",
        H::LYNX => "LYNX",
        H::NES => "NES",
        H::FDS => "FDS",
        H::PCE => "PCE",
        H::PSID => "PSID",
        H::SNES => "SNES",
        H::SPC => "SPC",
        _ => "Unknown",
    }
}

fn format_cell_with_source_flags(
    txt: String,
    rom: &RvFile,
    dat_flag: rv_core::rv_file::FileStatus,
    header_flag: rv_core::rv_file::FileStatus,
) -> String {
    let mut flags = String::new();
    if !dat_flag.is_empty() && rom.file_status.contains(dat_flag) {
        flags.push('D');
    }
    if !header_flag.is_empty() && rom.file_status.contains(header_flag) {
        flags.push('F');
    }
    if flags.is_empty() {
        txt
    } else {
        format!("{txt} ({flags})")
    }
}

fn rom_ui_display_name(prefix: &str, rom_name: &str) -> String {
    if prefix.is_empty() {
        rom_name.to_string()
    } else {
        format!("{prefix}{rom_name}")
    }
}

fn rom_display_name(rom: &RvFile, ui_display_name: &str) -> String {
    let mut out = ui_display_name.to_string();

    if !rom.file_name.is_empty() {
        out.push_str(&format!(" (Found: {})", rom.file_name));
    }

    if let Some(v) = rom.chd_version {
        out.push_str(&format!(" (V{v})"));
    }

    let d = if rom.file_status.contains(rv_core::rv_file::FileStatus::HEADER_FILE_TYPE_FROM_DAT) {
        "D"
    } else {
        ""
    };
    let f = if rom
        .file_status
        .contains(rv_core::rv_file::FileStatus::HEADER_FILE_TYPE_FROM_HEADER)
    {
        "F"
    } else {
        ""
    };
    let header = rom.header_file_type();
    if header != dat_reader::enums::HeaderFileType::NOTHING || !d.is_empty() || !f.is_empty() {
        let req = if rom.header_file_type_required() { ",Required" } else { "" };
        out.push_str(&format!(" ({}{req} {d}{f})", header_file_type_label(header)));
    }

    out
}

#[derive(Clone)]
pub(crate) struct RomGridRow {
    rom_rc: Rc<RefCell<RvFile>>,
    ui_name: String,
    display_text: String,
    local_header_offset: Option<u64>,
    zip_index: Option<usize>,
}

#[derive(Clone)]
pub(crate) struct RomGridCache {
    pub(crate) game_ptr: usize,
    pub(crate) game_child_count: usize,
    pub(crate) show_merged: bool,
    pub(crate) built_while_db_dirty: bool,
    pub(crate) alt_found: bool,
    pub(crate) show_status: bool,
    pub(crate) show_file_mod_date: bool,
    pub(crate) show_zip_index: bool,
    pub(crate) rows: Vec<RomGridRow>,
    pub(crate) last_sort_col: Option<String>,
    pub(crate) last_sort_desc: bool,
}

#[allow(clippy::too_many_arguments)]
fn collect_rom_grid_rows(
    node_rc: &Rc<RefCell<RvFile>>,
    prefix: &str,
    show_merged: bool,
    out: &mut Vec<RomGridRow>,
    alt_found: &mut bool,
    show_status: &mut bool,
    show_file_mod_date: &mut bool,
    show_zip_index: &mut bool,
) {
    const TORRENTZIP_DOS_TIME: u16 = 48128;
    const TORRENTZIP_DOS_DATE: u16 = 8600;
    const TORRENTZIP_DOS_PACKED: i64 =
        ((TORRENTZIP_DOS_DATE as i64) << 16) | (TORRENTZIP_DOS_TIME as i64);

    let children = node_rc.borrow().children.clone();
    for child_rc in children {
        let child = child_rc.borrow();
        if child.is_file() {
            if !show_merged
                && child.dat_status() == DatStatus::InDatMerged
                && child.rep_status() == RepStatus::NotCollected
            {
                continue;
            }

            let ui_name = rom_ui_display_name(prefix, &child.name);
            let display_text = rom_display_name(&child, &ui_name);

            *alt_found = *alt_found
                || child.alt_size.is_some()
                || child.alt_crc.is_some()
                || child.alt_sha1.is_some()
                || child.alt_md5.is_some();

            *show_status = *show_status
                || child
                    .status
                    .as_ref()
                    .is_some_and(|s| !s.trim().is_empty());

            *show_file_mod_date = *show_file_mod_date
                || (child.file_mod_time_stamp != 0
                    && child.file_mod_time_stamp != i64::MIN
                    && child.file_mod_time_stamp != TORRENTZIP_DOS_PACKED);

            *show_zip_index = *show_zip_index || child.local_header_offset.is_some();

            drop(child);
            out.push(RomGridRow {
                rom_rc: Rc::clone(&child_rc),
                ui_name,
                display_text,
                local_header_offset: child_rc.borrow().local_header_offset,
                zip_index: None,
            });
        } else if child.file_type == FileType::Dir {
            let next_prefix = format!("{}{}/", prefix, child.name);
            drop(child);
            collect_rom_grid_rows(
                &child_rc,
                &next_prefix,
                show_merged,
                out,
                alt_found,
                show_status,
                show_file_mod_date,
                show_zip_index,
            );
        }
    }
}

fn compute_zip_indices(rom_rows: &mut [RomGridRow]) {
    let mut offsets: Vec<(u64, usize)> = rom_rows
        .iter()
        .enumerate()
        .filter_map(|(i, r)| r.local_header_offset.map(|o| (o, i)))
        .collect();
    offsets.sort_by_key(|(o, _)| *o);
    for (idx, (_, row_i)) in offsets.into_iter().enumerate() {
        rom_rows[row_i].zip_index = Some(idx);
    }
}

fn format_file_mod_date_cell(rom: &RvFile) -> String {
    const TORRENTZIP_DOS_TIME: u16 = 48128;
    const TORRENTZIP_DOS_DATE: u16 = 8600;
    const TORRENTZIP_DOS_PACKED: i64 =
        ((TORRENTZIP_DOS_DATE as i64) << 16) | (TORRENTZIP_DOS_TIME as i64);

    if rom.file_mod_time_stamp == 0 || rom.file_mod_time_stamp == i64::MIN {
        return String::new();
    }
    if rom.file_mod_time_stamp == TORRENTZIP_DOS_PACKED {
        return "Trrntziped".to_string();
    }
    compress::compress_utils::zip_date_time_to_string(Some(rom.file_mod_time_stamp))
}

fn game_display_description(game_node: &RvFile) -> String {
    let mut desc = if let Some(ref g) = game_node.game {
        g.borrow()
            .get_data(rv_core::rv_game::GameData::Description)
            .unwrap_or_default()
    } else {
        String::new()
    };

    if desc == "¤" {
        let fallback = std::path::Path::new(&game_node.name)
            .file_stem()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        if !fallback.is_empty() {
            desc = fallback;
        }
    }

    desc
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

#[cfg(test)]
fn game_row_color(rep_status: RepStatus) -> egui::Color32 {
    game_row_color_for_mode(rep_status, true)
}

#[cfg(test)]
fn rom_row_color(rep_status: RepStatus) -> egui::Color32 {
    game_row_color_for_mode(rep_status, true)
}

fn game_row_color_for_mode(rep_status: RepStatus, dark_mode: bool) -> egui::Color32 {
    if dark_mode {
        match rep_status {
            RepStatus::Correct | RepStatus::CorrectMIA | RepStatus::DirCorrect => egui::Color32::from_rgb(40, 80, 40),
            RepStatus::Missing
            | RepStatus::MissingMIA
            | RepStatus::DirMissing
            | RepStatus::DirCorrupt
            | RepStatus::Corrupt
            | RepStatus::Incomplete => egui::Color32::from_rgb(80, 40, 40),
            RepStatus::CanBeFixed | RepStatus::CanBeFixedMIA | RepStatus::CorruptCanBeFixed => {
                egui::Color32::from_rgb(80, 80, 40)
            }
            RepStatus::MoveToSort
            | RepStatus::MoveToCorrupt
            | RepStatus::NeededForFix
            | RepStatus::Rename
            | RepStatus::InToSort
            | RepStatus::DirInToSort => egui::Color32::from_rgb(40, 80, 80),
            RepStatus::NotCollected
            | RepStatus::UnNeeded
            | RepStatus::Unknown
            | RepStatus::DirUnknown
            | RepStatus::UnScanned
            | RepStatus::Ignore => egui::Color32::from_rgb(60, 60, 60),
            RepStatus::Delete | RepStatus::Deleted => egui::Color32::from_rgb(120, 0, 0),
            _ => egui::Color32::TRANSPARENT,
        }
    } else {
        match rep_status {
            RepStatus::Correct | RepStatus::CorrectMIA | RepStatus::DirCorrect => egui::Color32::from_rgb(220, 245, 220),
            RepStatus::Missing
            | RepStatus::MissingMIA
            | RepStatus::DirMissing
            | RepStatus::DirCorrupt
            | RepStatus::Corrupt
            | RepStatus::Incomplete => egui::Color32::from_rgb(255, 225, 225),
            RepStatus::CanBeFixed | RepStatus::CanBeFixedMIA | RepStatus::CorruptCanBeFixed => {
                egui::Color32::from_rgb(255, 245, 210)
            }
            RepStatus::MoveToSort
            | RepStatus::MoveToCorrupt
            | RepStatus::NeededForFix
            | RepStatus::Rename
            | RepStatus::InToSort
            | RepStatus::DirInToSort => egui::Color32::from_rgb(220, 245, 245),
            RepStatus::NotCollected
            | RepStatus::UnNeeded
            | RepStatus::Unknown
            | RepStatus::DirUnknown
            | RepStatus::UnScanned
            | RepStatus::Ignore => egui::Color32::from_rgb(238, 238, 242),
            RepStatus::Delete | RepStatus::Deleted => egui::Color32::from_rgb(255, 200, 200),
            _ => egui::Color32::TRANSPARENT,
        }
    }
}

fn rom_row_color_for_mode(rep_status: RepStatus, dark_mode: bool) -> egui::Color32 {
    game_row_color_for_mode(rep_status, dark_mode)
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

        let filter_lc = self.filter_text.to_lowercase();
        let mut visible_children: Vec<Rc<RefCell<RvFile>>> = Vec::new();
        let mut show_description = false;
        let mut wide_type_column = false;
        if let Some(selected) = &self.selected_node {
            let node = selected.borrow();
            for child_rc in node
                .children
                .iter()
                .filter(|c| !c.borrow().is_file() || c.borrow().game.is_some())
            {
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

                    should_show = should_show || (self.show_complete && g_correct && !g_missing && !g_fixes);
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

                if !self.filter_text.is_empty()
                    && !child.name.to_lowercase().contains(&filter_lc)
                {
                    should_show = false;
                }
                if !should_show {
                    continue;
                }

                if !show_description {
                    if let Some(ref g) = child.game {
                        let desc = g
                            .borrow()
                            .get_data(rv_core::rv_game::GameData::Description)
                            .unwrap_or_default();
                        if !desc.trim().is_empty() && desc != "¤" {
                            show_description = true;
                        }
                    }
                }

                if !wide_type_column {
                    let expected = if child.dat_status() != DatStatus::NotInDat
                        && child.dat_status() != DatStatus::InToSort
                    {
                        Some(game_type_icon_key(child.file_type, child.zip_dat_struct()))
                    } else {
                        None
                    };
                    let have = if child.got_status() != GotStatus::NotGot {
                        Some(game_type_icon_key(child.file_type, child.zip_struct))
                    } else {
                        None
                    };
                    if let (Some(e), Some(h)) = (expected, have) {
                        if e != h {
                            wide_type_column = true;
                        }
                    }
                }

                visible_children.push(Rc::clone(child_rc));
            }
        }

        if let Some(col) = &self.sort_col {
            let desc = self.sort_desc;
            visible_children.sort_by(|a, b| {
                let a = a.borrow();
                let b = b.borrow();
                let cmp = match col.as_str() {
                    "Game (Directory / Zip)" => trrntzip_name_cmp(&a.name, &b.name),
                    "Description" => {
                        let da = game_display_description(&a);
                        let db = game_display_description(&b);
                        da.cmp(&db).then(trrntzip_name_cmp(&a.name, &b.name))
                    }
                    "Type" => a
                        .file_type
                        .cmp(&b.file_type)
                        .then(b.zip_struct.cmp(&a.zip_struct))
                        .then(a.rep_status().cmp(&b.rep_status()))
                        .then(trrntzip_name_cmp(&a.name, &b.name)),
                    "Modified" => a
                        .file_mod_time_stamp
                        .cmp(&b.file_mod_time_stamp)
                        .then(trrntzip_name_cmp(&a.name, &b.name)),
                    _ => trrntzip_name_cmp(&a.name, &b.name),
                };
                if desc { cmp.reverse() } else { cmp }
            });
        }

        let dark_mode = ui.visuals().dark_mode;
        let grid_fill = if dark_mode {
            egui::Color32::from_rgb(20, 20, 22)
        } else {
            ui.visuals().panel_fill
        };
        let grid_stroke = if dark_mode {
            egui::Stroke::new(1.0, egui::Color32::from_rgb(40, 40, 45))
        } else {
            egui::Stroke::new(1.0, egui::Color32::from_rgb(220, 220, 226))
        };

        egui::Frame::none()
            .fill(grid_fill)
            .stroke(grid_stroke)
            .rounding(6.0)
            .inner_margin(4.0)
            .show(ui, |ui| {
                egui::ScrollArea::horizontal().show(ui, |ui| {
                    let type_width = if wide_type_column { 90.0 } else { 44.0 };
                    let mut table = egui_extras::TableBuilder::new(ui)
                        .striped(true)
                        .resizable(true)
                        .vscroll(true)
                        .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
                        .column(egui_extras::Column::initial(type_width).at_least(type_width))
                        .column(egui_extras::Column::initial(350.0).at_least(40.0));

                    if show_description {
                        table = table.column(egui_extras::Column::initial(350.0).at_least(40.0));
                    }

                    table = table
                        .column(egui_extras::Column::initial(150.0).at_least(40.0))
                        .column(egui_extras::Column::remainder());

                    table
                        .header(20.0, |mut header| {
                            header.col(|ui| {
                                sort_header_cell(ui, "Type", &mut new_sort_col, &mut new_sort_desc)
                            });
                            header.col(|ui| {
                                sort_header_cell(
                                    ui,
                                    "Game (Directory / Zip)",
                                    &mut new_sort_col,
                                    &mut new_sort_desc,
                                );
                            });
                            if show_description {
                                header.col(|ui| {
                                    sort_header_cell(
                                        ui,
                                        "Description",
                                        &mut new_sort_col,
                                        &mut new_sort_desc,
                                    )
                                });
                            }
                            header.col(|ui| {
                                sort_header_cell(ui, "Modified", &mut new_sort_col, &mut new_sort_desc)
                            });
                            header.col(|ui| {
                                sort_header_cell(ui, "ROM Status", &mut new_sort_col, &mut new_sort_desc)
                            });
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
                                if show_description {
                                    row.col(|ui| {
                                        ui.label("");
                                    });
                                }
                                row.col(|ui| {
                                    ui.label("");
                                });
                                row.col(|ui| {
                                    ui.label("");
                                });
                            });
                        }

                        let row_count = visible_children.len();
                        body.rows(20.0, row_count, |mut row| {
                            let child_rc = &visible_children[row.index()];
                            let child = child_rc.borrow();

                            let mut row_color = game_row_color_for_mode(child.rep_status(), dark_mode);

                            let is_selected = self
                                .selected_game
                                .as_ref()
                                .is_some_and(|s| Rc::ptr_eq(s, child_rc));
                            if is_selected {
                                row_color = selection_color;
                            }

                            row.col(|ui| {
                                ui.painter().rect_filled(ui.max_rect(), 0.0, row_color);
                                let expected_key = if child.dat_status() != DatStatus::NotInDat
                                    && child.dat_status() != DatStatus::InToSort
                                {
                                    Some(game_type_icon_key(child.file_type, child.zip_dat_struct()))
                                } else {
                                    None
                                };
                                let have_key = if child.got_status() != GotStatus::NotGot {
                                    Some(game_type_icon_key(child.file_type, child.zip_struct))
                                } else {
                                    None
                                };
                                let mismatch =
                                    expected_key.is_some_and(|e| have_key.is_some_and(|h| e != h));

                                let expected_img = expected_key
                                    .map(|(ft, zs)| game_grid_icon_source(game_type_icon_missing(ft, zs)));
                                let have_img = have_key.map(|(ft, zs)| {
                                    game_grid_icon_source(if child.got_status() == GotStatus::Corrupt {
                                        game_type_icon_corrupt(ft, zs)
                                    } else {
                                        game_type_icon_normal(ft, zs)
                                    })
                                });
                                let convert_img = game_grid_icon_source(if child.zip_dat_struct_fix() {
                                    "ZipConvert.png"
                                } else {
                                    "ZipConvert1.png"
                                });

                                if mismatch {
                                    ui.horizontal(|ui| {
                                        if let Some(h) = have_img {
                                            ui.add(
                                                egui::Image::new(h)
                                                    .texture_options(egui::TextureOptions::NEAREST)
                                                    .max_width(16.0),
                                            );
                                        }
                                        ui.add(
                                            egui::Image::new(convert_img)
                                                .texture_options(egui::TextureOptions::NEAREST)
                                                .max_width(16.0),
                                        );
                                        if let Some(e) = expected_img {
                                            ui.add(
                                                egui::Image::new(e)
                                                    .texture_options(egui::TextureOptions::NEAREST)
                                                    .max_width(16.0),
                                            );
                                        }
                                    });
                                } else if let Some(h) = have_img {
                                    ui.add(
                                        egui::Image::new(h)
                                            .texture_options(egui::TextureOptions::NEAREST)
                                            .max_width(16.0),
                                    );
                                } else if let Some(e) = expected_img {
                                    ui.add(
                                        egui::Image::new(e)
                                            .texture_options(egui::TextureOptions::NEAREST)
                                            .max_width(16.0),
                                    );
                                } else {
                                    ui.add(
                                        egui::Image::new(game_grid_icon_source("default2.png"))
                                            .texture_options(egui::TextureOptions::NEAREST)
                                            .max_width(16.0),
                                    );
                                }
                            });
                            row.col(|ui| {
                                ui.painter().rect_filled(ui.max_rect(), 0.0, row_color);
                                let label_text = if child.file_name.is_empty() {
                                    child.name.clone()
                                } else {
                                    format!("{} (Found: {})", child.name, child.file_name)
                                };
                                let label_resp = ui.add(egui::SelectableLabel::new(is_selected, label_text));
                                if ui.input(|i| i.modifiers.shift) {
                                    label_resp.context_menu(|ui| {
                                        let mut has_open_target = false;

                                        if child.file_type == FileType::Dir && !self.sam_running {
                                            if ui.button("Scan").clicked() {
                                                pending_action = Some(GridAction::ScanNormal(Rc::clone(child_rc)));
                                                ui.close_menu();
                                            }
                                            if ui.button("Scan Quick (Headers Only)").clicked() {
                                                pending_action = Some(GridAction::ScanQuick(Rc::clone(child_rc)));
                                                ui.close_menu();
                                            }
                                            if ui.button("Scan Full (Complete Re-Scan)").clicked() {
                                                pending_action = Some(GridAction::ScanFull(Rc::clone(child_rc)));
                                                ui.close_menu();
                                            }
                                            ui.separator();
                                        }

                                        let full_path = get_full_node_path(Rc::clone(child_rc));
                                        let full_path =
                                            rv_core::settings::find_dir_mapping(&full_path).unwrap_or(full_path);
                                        if child.file_type == FileType::Dir {
                                            if std::path::Path::new(&full_path).is_dir() {
                                                has_open_target = true;
                                                if ui.button("Open Dir").clicked() {
                                                    self.task_logs.push(format!("Opening Dir: {}", full_path));
                                                    let _ = std::process::Command::new("cmd")
                                                        .args(["/C", "start", "", &full_path])
                                                        .spawn();
                                                    ui.close_menu();
                                                }
                                            }
                                        } else if matches!(child.file_type, FileType::Zip | FileType::SevenZip)
                                            && std::path::Path::new(&full_path).is_file()
                                        {
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

                                        let parent_path = std::path::Path::new(&full_path)
                                            .parent()
                                            .unwrap_or_else(|| std::path::Path::new(""))
                                            .to_string_lossy()
                                            .to_string();
                                        if std::path::Path::new(&parent_path).is_dir() {
                                            has_open_target = true;
                                            if ui.button("Open Parent").clicked() {
                                                self.task_logs.push(format!("Opening Parent: {}", parent_path));
                                                let _ = std::process::Command::new("cmd")
                                                    .args(["/C", "start", "", &parent_path])
                                                    .spawn();
                                                ui.close_menu();
                                            }
                                        }

                                        if has_open_target {
                                            if let Some(parent_rc) =
                                                child_rc.borrow().parent.as_ref().and_then(|p| p.upgrade())
                                            {
                                                if emulator_info_for_game_dir(parent_rc).is_some()
                                                    && ui.button("Launch emulator").clicked()
                                                {
                                                    pending_action =
                                                        Some(GridAction::LaunchEmulator(Rc::clone(child_rc)));
                                                    ui.close_menu();
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
                                                .and_then(|g| {
                                                    g.borrow().get_data(rv_core::rv_game::GameData::Id)
                                                })
                                                .map(|s| !s.trim().is_empty())
                                                .unwrap_or(false);
                                        let has_redump = home_page == "redump.org"
                                            && child
                                                .game
                                                .as_ref()
                                                .and_then(|g| {
                                                    g.borrow().get_data(rv_core::rv_game::GameData::Id)
                                                })
                                                .map(|s| !s.trim().is_empty())
                                                .unwrap_or(false);
                                        if (has_no_intro || has_redump) && ui.button("Open Web Page").clicked() {
                                            pending_action = Some(GridAction::OpenWebPage(Rc::clone(child_rc)));
                                            ui.close_menu();
                                        }
                                    });
                                }

                                if label_resp.double_clicked() {
                                    if child.game.is_none() && child.file_type == FileType::Dir {
                                        pending_action = Some(GridAction::NavigateDown(Rc::clone(child_rc)));
                                    } else {
                                        pending_action = Some(GridAction::LaunchEmulator(Rc::clone(child_rc)));
                                    }
                                } else if label_resp.clicked() {
                                    self.selected_game = Some(Rc::clone(child_rc));
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
                            if show_description {
                                row.col(|ui| {
                                    ui.painter().rect_filled(ui.max_rect(), 0.0, row_color);
                                    ui.label(game_display_description(&child));
                                });
                            }
                            row.col(|ui| {
                                ui.painter().rect_filled(ui.max_rect(), 0.0, row_color);
                                let time_str = compress::compress_utils::zip_date_time_to_string(Some(
                                    child.file_mod_time_stamp,
                                ));
                                ui.label(format_cell_with_source_flags(
                                    time_str,
                                    &child,
                                    rv_core::rv_file::FileStatus::DATE_FROM_DAT,
                                    rv_core::rv_file::FileStatus::NONE,
                                ));
                            });
                            row.col(|ui| {
                                ui.painter().rect_filled(ui.max_rect(), 0.0, row_color);
                                ui.horizontal(|ui| {
                                    let (correct, missing, fixes, merged, unknown) =
                                        if let Some(stats) = &child.cached_stats {
                                            (
                                                stats.count_correct() as usize,
                                                (stats.roms_missing + stats.roms_missing_mia) as usize,
                                                stats.roms_fixes as usize,
                                                (stats.roms_not_collected + stats.roms_unneeded) as usize,
                                                stats.roms_unknown as usize,
                                            )
                                        } else {
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

                                            (correct, missing, fixes, merged, unknown)
                                        };

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
                            });
                        });
                    }
                        });
                });
            });

        if let Some(action) = pending_action {
            match action {
                GridAction::ScanQuick(target_rc) => {
                    let name = target_rc.borrow().name.clone();
                    let logical = get_full_node_path(Rc::clone(&target_rc));
                    let np = rv_core::settings::find_dir_mapping(&logical).unwrap_or(logical.clone());
                    let rule = rv_core::settings::find_rule(&logical);
                    self.launch_task("Scan ROMs (Quick)", move |tx| {
                        let _ = tx.send(format!("Scanning {} (Headers Only)...", name));
                        let files = Scanner::scan_directory_with_level_and_ignore(&np, rv_core::settings::EScanLevel::Level1, &rule.ignore_files.items);
                        let mut root_scan = rv_core::scanned_file::ScannedFile::new(FileType::Dir);
                        root_scan.children = files;
                        let _ = tx.send("Integrating files into DB...".to_string());
                        FileScanning::scan_dir_with_level(target_rc, &mut root_scan, rv_core::settings::EScanLevel::Level1);
                    });
                }
                GridAction::ScanNormal(target_rc) => {
                    let name = target_rc.borrow().name.clone();
                    let logical = get_full_node_path(Rc::clone(&target_rc));
                    let np = rv_core::settings::find_dir_mapping(&logical).unwrap_or(logical.clone());
                    let rule = rv_core::settings::find_rule(&logical);
                    self.launch_task("Scan ROMs", move |tx| {
                        let _ = tx.send(format!("Scanning {}...", name));
                        let files = Scanner::scan_directory_with_level_and_ignore(&np, rv_core::settings::EScanLevel::Level2, &rule.ignore_files.items);
                        let mut root_scan = rv_core::scanned_file::ScannedFile::new(FileType::Dir);
                        root_scan.children = files;
                        let _ = tx.send("Integrating files into DB...".to_string());
                        FileScanning::scan_dir_with_level(target_rc, &mut root_scan, rv_core::settings::EScanLevel::Level2);
                    });
                }
                GridAction::ScanFull(target_rc) => {
                    let name = target_rc.borrow().name.clone();
                    let logical = get_full_node_path(Rc::clone(&target_rc));
                    let np = rv_core::settings::find_dir_mapping(&logical).unwrap_or(logical.clone());
                    let rule = rv_core::settings::find_rule(&logical);
                    self.launch_task("Scan ROMs (Full)", move |tx| {
                        let _ = tx.send(format!("Scanning {} (Full Re-Scan)...", name));
                        let files = Scanner::scan_directory_with_level_and_ignore(&np, rv_core::settings::EScanLevel::Level3, &rule.ignore_files.items);
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

        let empty_rows: &[RomGridRow] = &[];
        let (rom_rows, alt_found, show_status, show_file_mod_date, show_zip_index) = if let Some(selected_game) =
            &self.selected_game
        {
            let game_ptr = Rc::as_ptr(selected_game) as usize;
            let game_child_count = selected_game.borrow().children.len();
            let mut needs_rebuild = match self.rom_grid_cache.as_ref() {
                Some(c) => {
                    c.game_ptr != game_ptr
                        || c.game_child_count != game_child_count
                        || c.show_merged != self.show_merged
                }
                None => true,
            };
            if let Some(c) = self.rom_grid_cache.as_ref() {
                if self.db_cache_dirty && !c.built_while_db_dirty {
                    needs_rebuild = true;
                }
            }

            if needs_rebuild {
                let mut rows: Vec<RomGridRow> = Vec::new();
                let mut alt_found = false;
                let mut show_status = false;
                let mut show_file_mod_date = false;
                let mut show_zip_index = false;
                collect_rom_grid_rows(
                    selected_game,
                    "",
                    self.show_merged,
                    &mut rows,
                    &mut alt_found,
                    &mut show_status,
                    &mut show_file_mod_date,
                    &mut show_zip_index,
                );
                if show_zip_index {
                    compute_zip_indices(&mut rows);
                }
                self.rom_grid_cache = Some(RomGridCache {
                    game_ptr,
                    game_child_count,
                    show_merged: self.show_merged,
                    built_while_db_dirty: self.db_cache_dirty,
                    alt_found,
                    show_status,
                    show_file_mod_date,
                    show_zip_index,
                    rows,
                    last_sort_col: None,
                    last_sort_desc: false,
                });
            }

            let cache = self.rom_grid_cache.as_mut().unwrap();
            if cache.last_sort_col != self.sort_col || cache.last_sort_desc != self.sort_desc {
                if let Some(col) = &self.sort_col {
                    let desc = self.sort_desc;
                    cache.rows.sort_by(|a, b| {
                        let a_ref = a.rom_rc.borrow();
                        let b_ref = b.rom_rc.borrow();
                        let cmp = match col.as_str() {
                            "Got" => a_ref
                                .got_status()
                                .cmp(&b_ref.got_status())
                                .then(a_ref.rep_status().cmp(&b_ref.rep_status()))
                                .then(a.ui_name.cmp(&b.ui_name)),
                            "ROM (File)" => a.ui_name.cmp(&b.ui_name),
                            "Merge" => a_ref.merge.cmp(&b_ref.merge),
                            "Size" => a_ref.size.cmp(&b_ref.size),
                            "CRC32" => a_ref.crc.cmp(&b_ref.crc),
                            "SHA1" => a_ref.sha1.cmp(&b_ref.sha1),
                            "MD5" => a_ref.md5.cmp(&b_ref.md5),
                            "AltSize" => a_ref.alt_size.cmp(&b_ref.alt_size),
                            "AltCRC32" => a_ref.alt_crc.cmp(&b_ref.alt_crc),
                            "AltSHA1" => a_ref.alt_sha1.cmp(&b_ref.alt_sha1),
                            "AltMD5" => a_ref.alt_md5.cmp(&b_ref.alt_md5),
                            "Status" => a_ref.status.cmp(&b_ref.status),
                            "FileModDate" => a_ref.file_mod_time_stamp.cmp(&b_ref.file_mod_time_stamp),
                            "ZipIndex" => a_ref.local_header_offset.cmp(&b_ref.local_header_offset),
                            "InstanceCount" => std::cmp::Ordering::Equal,
                            _ => a.ui_name.cmp(&b.ui_name),
                        };
                        let cmp = if cmp == std::cmp::Ordering::Equal && col.as_str() != "ROM (File)" {
                            a.ui_name.cmp(&b.ui_name)
                        } else {
                            cmp
                        };
                        if desc { cmp.reverse() } else { cmp }
                    });
                }
                cache.last_sort_col = self.sort_col.clone();
                cache.last_sort_desc = self.sort_desc;
            }

            (
                &cache.rows[..],
                cache.alt_found,
                cache.show_status,
                cache.show_file_mod_date,
                cache.show_zip_index,
            )
        } else {
            self.rom_grid_cache = None;
            (empty_rows, false, false, false, false)
        };

        let dark_mode = ui.visuals().dark_mode;
        let grid_fill = if dark_mode {
            egui::Color32::from_rgb(20, 20, 22)
        } else {
            ui.visuals().panel_fill
        };
        let grid_stroke = if dark_mode {
            egui::Stroke::new(1.0, egui::Color32::from_rgb(40, 40, 45))
        } else {
            egui::Stroke::new(1.0, egui::Color32::from_rgb(220, 220, 226))
        };

        egui::Frame::none()
            .fill(grid_fill)
            .stroke(grid_stroke)
            .rounding(6.0)
            .inner_margin(4.0)
            .show(ui, |ui| {
                egui::ScrollArea::horizontal().show(ui, |ui| {
                    let mut table = egui_extras::TableBuilder::new(ui)
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
                        .column(egui_extras::Column::initial(200.0).at_least(40.0));

                    if alt_found {
                        table = table
                            .column(egui_extras::Column::initial(100.0).at_least(40.0))
                            .column(egui_extras::Column::initial(150.0).at_least(40.0))
                            .column(egui_extras::Column::initial(200.0).at_least(40.0))
                            .column(egui_extras::Column::initial(200.0).at_least(40.0));
                    }

                    if show_status {
                        table = table.column(egui_extras::Column::initial(100.0).at_least(40.0));
                    }

                    if show_file_mod_date {
                        table = table.column(egui_extras::Column::initial(150.0).at_least(40.0));
                    }

                    if show_zip_index {
                        table = table.column(egui_extras::Column::initial(100.0).at_least(40.0));
                    }

                    table
                        .column(egui_extras::Column::remainder())
                        .header(20.0, |mut header| {
                            header.col(|ui| {
                                sort_header_cell(
                                    ui,
                                    "Got",
                                    &mut new_sort_col_rom,
                                    &mut new_sort_desc_rom,
                                );
                            });
                            header.col(|ui| {
                                sort_header_cell(
                                    ui,
                                    "ROM (File)",
                                    &mut new_sort_col_rom,
                                    &mut new_sort_desc_rom,
                                );
                            });
                            header.col(|ui| {
                                sort_header_cell(ui, "Merge", &mut new_sort_col_rom, &mut new_sort_desc_rom)
                            });
                            header.col(|ui| {
                                sort_header_cell(ui, "Size", &mut new_sort_col_rom, &mut new_sort_desc_rom)
                            });
                            header.col(|ui| {
                                sort_header_cell(ui, "CRC32", &mut new_sort_col_rom, &mut new_sort_desc_rom)
                            });
                            header.col(|ui| {
                                sort_header_cell(ui, "SHA1", &mut new_sort_col_rom, &mut new_sort_desc_rom)
                            });
                            header.col(|ui| {
                                sort_header_cell(ui, "MD5", &mut new_sort_col_rom, &mut new_sort_desc_rom)
                            });
                            if alt_found {
                                header.col(|ui| {
                                    sort_header_cell(
                                        ui,
                                        "AltSize",
                                        &mut new_sort_col_rom,
                                        &mut new_sort_desc_rom,
                                    )
                                });
                                header.col(|ui| {
                                    sort_header_cell(
                                        ui,
                                        "AltCRC32",
                                        &mut new_sort_col_rom,
                                        &mut new_sort_desc_rom,
                                    )
                                });
                                header.col(|ui| {
                                    sort_header_cell(
                                        ui,
                                        "AltSHA1",
                                        &mut new_sort_col_rom,
                                        &mut new_sort_desc_rom,
                                    )
                                });
                                header.col(|ui| {
                                    sort_header_cell(
                                        ui,
                                        "AltMD5",
                                        &mut new_sort_col_rom,
                                        &mut new_sort_desc_rom,
                                    )
                                });
                            }
                            if show_status {
                                header.col(|ui| {
                                    sort_header_cell(
                                        ui,
                                        "Status",
                                        &mut new_sort_col_rom,
                                        &mut new_sort_desc_rom,
                                    )
                                });
                            }
                            if show_file_mod_date {
                                header.col(|ui| {
                                    sort_header_cell(
                                        ui,
                                        "FileModDate",
                                        &mut new_sort_col_rom,
                                        &mut new_sort_desc_rom,
                                    )
                                });
                            }
                            if show_zip_index {
                                header.col(|ui| {
                                    sort_header_cell(
                                        ui,
                                        "ZipIndex",
                                        &mut new_sort_col_rom,
                                        &mut new_sort_desc_rom,
                                    )
                                });
                            }
                            header.col(|ui| {
                                sort_header_cell(
                                    ui,
                                    "InstanceCount",
                                    &mut new_sort_col_rom,
                                    &mut new_sort_desc_rom,
                                )
                            });
                        })
                        .body(|body| {
                            let row_count = rom_rows.len();
                            body.rows(20.0, row_count, |mut row| {
                                let row_data = &rom_rows[row.index()];
                                let rom_rc = Rc::clone(&row_data.rom_rc);
                                let rom = rom_rc.borrow();
                                let row_color = rom_row_color_for_mode(rom.rep_status(), dark_mode);

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
                                    let label_resp =
                                        ui.add(egui::SelectableLabel::new(false, &row_data.display_text));
                                    if label_resp.secondary_clicked() {
                                        if let Some(text) = rom_clipboard_text(&rom, RomGridCopyColumn::Rom) {
                                            ui.output_mut(|o| o.copied_text = text.clone());
                                            self.task_logs.push(format!("Copied: {}", text));
                                        }
                                    }
                                    label_resp.context_menu(|ui| {
                                        if ui.button("Copy ROM Name").clicked() {
                                            ui.output_mut(|o| o.copied_text = row_data.ui_name.clone());
                                            self.task_logs.push(format!("Copied: {}", row_data.ui_name));
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
                                    let text = format_cell_with_source_flags(
                                        rom.size.map(|s| s.to_string()).unwrap_or_default(),
                                        &rom,
                                        rv_core::rv_file::FileStatus::SIZE_FROM_DAT,
                                        rv_core::rv_file::FileStatus::SIZE_FROM_HEADER,
                                    );
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
                                    let text = format_cell_with_source_flags(
                                        rom
                                        .crc
                                        .as_ref()
                                        .map(hex::encode)
                                        .unwrap_or_default(),
                                        &rom,
                                        rv_core::rv_file::FileStatus::CRC_FROM_DAT,
                                        rv_core::rv_file::FileStatus::CRC_FROM_HEADER,
                                    );
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
                                    let text = format_cell_with_source_flags(
                                        rom
                                        .sha1
                                        .as_ref()
                                        .map(hex::encode)
                                        .unwrap_or_default(),
                                        &rom,
                                        rv_core::rv_file::FileStatus::SHA1_FROM_DAT,
                                        rv_core::rv_file::FileStatus::SHA1_FROM_HEADER,
                                    );
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
                                    let text = format_cell_with_source_flags(
                                        rom
                                        .md5
                                        .as_ref()
                                        .map(hex::encode)
                                        .unwrap_or_default(),
                                        &rom,
                                        rv_core::rv_file::FileStatus::MD5_FROM_DAT,
                                        rv_core::rv_file::FileStatus::MD5_FROM_HEADER,
                                    );
                                    let resp = ui.add(egui::Label::new(text).sense(egui::Sense::click()));
                                    if resp.secondary_clicked() {
                                        if let Some(copy) = rom_clipboard_text(&rom, RomGridCopyColumn::Md5) {
                                            ui.output_mut(|o| o.copied_text = copy.clone());
                                            self.task_logs.push(format!("Copied: {}", copy));
                                        }
                                    }
                                });
                                if alt_found {
                                    row.col(|ui| {
                                        ui.painter().rect_filled(ui.max_rect(), 0.0, row_color);
                                        let text = format_cell_with_source_flags(
                                            rom.alt_size.map(|s| s.to_string()).unwrap_or_default(),
                                            &rom,
                                            rv_core::rv_file::FileStatus::ALT_SIZE_FROM_DAT,
                                            rv_core::rv_file::FileStatus::ALT_SIZE_FROM_HEADER,
                                        );
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
                                        let text = format_cell_with_source_flags(
                                            rom.alt_crc.as_ref().map(hex::encode).unwrap_or_default(),
                                            &rom,
                                            rv_core::rv_file::FileStatus::ALT_CRC_FROM_DAT,
                                            rv_core::rv_file::FileStatus::ALT_CRC_FROM_HEADER,
                                        );
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
                                        let text = format_cell_with_source_flags(
                                            rom.alt_sha1.as_ref().map(hex::encode).unwrap_or_default(),
                                            &rom,
                                            rv_core::rv_file::FileStatus::ALT_SHA1_FROM_DAT,
                                            rv_core::rv_file::FileStatus::ALT_SHA1_FROM_HEADER,
                                        );
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
                                        let text = format_cell_with_source_flags(
                                            rom.alt_md5.as_ref().map(hex::encode).unwrap_or_default(),
                                            &rom,
                                            rv_core::rv_file::FileStatus::ALT_MD5_FROM_DAT,
                                            rv_core::rv_file::FileStatus::ALT_MD5_FROM_HEADER,
                                        );
                                        let resp = ui.add(egui::Label::new(text).sense(egui::Sense::click()));
                                        if resp.secondary_clicked() {
                                            if let Some(copy) = rom_clipboard_text(&rom, RomGridCopyColumn::AltMd5) {
                                                ui.output_mut(|o| o.copied_text = copy.clone());
                                                self.task_logs.push(format!("Copied: {}", copy));
                                            }
                                        }
                                    });
                                }
                                if show_status {
                                    row.col(|ui| {
                                        ui.painter().rect_filled(ui.max_rect(), 0.0, row_color);
                                        ui.label(rom.status.as_deref().unwrap_or(""));
                                    });
                                }
                                if show_file_mod_date {
                                    row.col(|ui| {
                                        ui.painter().rect_filled(ui.max_rect(), 0.0, row_color);
                                        let text = format_file_mod_date_cell(&rom);
                                        ui.label(format_cell_with_source_flags(
                                            text,
                                            &rom,
                                            rv_core::rv_file::FileStatus::DATE_FROM_DAT,
                                            rv_core::rv_file::FileStatus::NONE,
                                        ));
                                    });
                                }
                                if show_zip_index {
                                    row.col(|ui| {
                                        ui.painter().rect_filled(ui.max_rect(), 0.0, row_color);
                                        ui.label(row_data.zip_index.map(|v| v.to_string()).unwrap_or_default());
                                    });
                                }
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
                        });
                });
            });

        self.sort_col = new_sort_col_rom;
        self.sort_desc = new_sort_desc_rom;
    }
}

#[cfg(test)]
#[path = "tests/grids_tests.rs"]
mod tests;
