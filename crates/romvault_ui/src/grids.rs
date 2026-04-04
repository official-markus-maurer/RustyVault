use eframe::egui;
use std::cell::RefCell;
use std::rc::Rc;

use crate::utils::get_full_node_path;
use crate::RomVaultApp;
use dat_reader::enums::{DatStatus, FileType, GotStatus, ZipStructure};
use rv_core::db::GLOBAL_DB;
use rv_core::enums::RepStatus;
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

            if name.is_empty()
                && size.is_empty()
                && crc.is_empty()
                && sha1.is_empty()
                && md5.is_empty()
            {
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
        let ca = if ab[i].is_ascii_uppercase() {
            ab[i] + 0x20
        } else {
            ab[i]
        };
        let cb = if bb[i].is_ascii_uppercase() {
            bb[i] + 0x20
        } else {
            bb[i]
        };
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

fn emulator_info_for_game_dir(
    game_parent: Rc<RefCell<RvFile>>,
) -> Option<rv_core::settings::EmulatorInfo> {
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
        if candidate.sha1.as_ref() != Some(alt_sha1)
            && candidate.alt_sha1.as_ref() != Some(alt_sha1)
        {
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
            ZipStructure::ZipTrrnt
            | ZipStructure::ZipTDC
            | ZipStructure::ZipZSTD
            | ZipStructure::None => (ft, zs),
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

    let d = if rom
        .file_status
        .contains(rv_core::rv_file::FileStatus::HEADER_FILE_TYPE_FROM_DAT)
    {
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
        let req = if rom.header_file_type_required() {
            ",Required"
        } else {
            ""
        };
        out.push_str(&format!(
            " ({}{req} {d}{f})",
            header_file_type_label(header)
        ));
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

            *show_status =
                *show_status || child.status.as_ref().is_some_and(|s| !s.trim().is_empty());

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

fn grid_visibility_flags_from_stats(
    stats: &rv_core::repair_status::RepairStatus,
) -> GridVisibilityFlags {
    let total_roms = stats.total_roms;
    let merged_roms = stats.roms_not_collected + stats.roms_unneeded;
    let correct_roms = stats.count_correct();
    GridVisibilityFlags {
        correct: total_roms > 0 && correct_roms == total_roms,
        missing: stats.roms_missing > 0 || stats.roms_missing_mia > 0,
        fixes: stats.roms_fixes > 0 || stats.roms_unneeded > 0,
        mia: stats.roms_missing_mia > 0
            || stats.roms_correct_mia > 0
            || (total_roms > 0 && stats.roms_fixes == total_roms),
        merged: total_roms > 0 && merged_roms == total_roms,
        unknown: stats.roms_unknown > 0,
    }
}

fn grid_visibility_flags_from_report_status(
    report_status: rv_core::enums::ReportStatus,
) -> GridVisibilityFlags {
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
            RepStatus::Correct | RepStatus::CorrectMIA | RepStatus::DirCorrect => {
                egui::Color32::from_rgb(40, 80, 40)
            }
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
            RepStatus::Correct | RepStatus::CorrectMIA | RepStatus::DirCorrect => {
                egui::Color32::from_rgb(220, 245, 220)
            }
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
        RepStatus::Correct | RepStatus::CorrectMIA | RepStatus::DirCorrect => {
            Some(RomStatusBucket::Correct)
        }
        RepStatus::Missing
        | RepStatus::MissingMIA
        | RepStatus::DirMissing
        | RepStatus::DirCorrupt
        | RepStatus::Corrupt
        | RepStatus::Incomplete => Some(RomStatusBucket::Missing),
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
        RepStatus::Unknown | RepStatus::DirUnknown | RepStatus::UnScanned => {
            Some(RomStatusBucket::Unknown)
        }
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

include!("grids/app_impl.rs");

#[cfg(test)]
#[path = "tests/grids_tests.rs"]
mod tests;
