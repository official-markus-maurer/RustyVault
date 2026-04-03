use eframe::egui;

use crate::RomVaultApp;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum SamInputKind {
    Directory,
    Zip,
    SevenZip,
    Mixed,
}

impl SamInputKind {
    fn label(self) -> &'static str {
        match self {
            SamInputKind::Directory => "Directory",
            SamInputKind::Zip => "Zip",
            SamInputKind::SevenZip => "7z",
            SamInputKind::Mixed => "Mixed",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum SamOutputKind {
    TorrentZip,
    Zip,
    ZipZstd,
    SevenZipLzma,
    SevenZipZstd,
}

impl SamOutputKind {
    fn label(self) -> &'static str {
        match self {
            SamOutputKind::TorrentZip => "TorrentZip",
            SamOutputKind::Zip => "Zip",
            SamOutputKind::ZipZstd => "Zip Zstd",
            SamOutputKind::SevenZipLzma => "7z LZMA",
            SamOutputKind::SevenZipZstd => "7z Zstd",
        }
    }
}

pub(crate) const SAM_INPUT_OPTIONS: [SamInputKind; 4] = [
    SamInputKind::Directory,
    SamInputKind::Zip,
    SamInputKind::SevenZip,
    SamInputKind::Mixed,
];

pub(crate) const SAM_OUTPUT_OPTIONS: [SamOutputKind; 5] = [
    SamOutputKind::TorrentZip,
    SamOutputKind::Zip,
    SamOutputKind::ZipZstd,
    SamOutputKind::SevenZipLzma,
    SamOutputKind::SevenZipZstd,
];

#[derive(Clone, Copy)]
struct ColorKeyEntry {
    icon: &'static str,
    title: &'static str,
    description: &'static str,
}

#[derive(Clone, Copy)]
struct ColorKeySection {
    title: &'static str,
    entries: &'static [ColorKeyEntry],
}

const GAME_LIST_RESTING: &[ColorKeyEntry] = &[
    ColorKeyEntry { icon: "G_Correct.png", title: "Correct", description: "The ROM is correct." },
    ColorKeyEntry { icon: "G_CorrectMIA.png", title: "CorrectMIA", description: "The ROM was known to be MIA, but you found it." },
    ColorKeyEntry { icon: "G_Missing.png", title: "Missing", description: "The ROM is missing." },
    ColorKeyEntry { icon: "G_MissingMIA.png", title: "MissingMIA", description: "The ROM is known to be private or missing in action." },
    ColorKeyEntry { icon: "G_InToSort.png", title: "InToSort", description: "The ROM is in a ToSort directory." },
    ColorKeyEntry { icon: "G_UnNeeded.png", title: "UnNeeded", description: "The ROM is not needed here because it belongs in the parent or primary deduped set." },
    ColorKeyEntry { icon: "G_NotCollected.png", title: "NotCollected", description: "The ROM is not collected here because it belongs in the parent or primary deduped set or is a bad dump." },
    ColorKeyEntry { icon: "G_Ignore.png", title: "Ignore", description: "The file matches an ignore rule." },
];

const GAME_LIST_FIXING: &[ColorKeyEntry] = &[
    ColorKeyEntry { icon: "G_CanBeFixed.png", title: "CanBeFixed", description: "The ROM is missing here, but it is available elsewhere and will be fixed." },
    ColorKeyEntry { icon: "G_CanBeFixedMIA.png", title: "CanBeFixedMIA", description: "The MIA ROM is missing here, but it is available elsewhere and will be fixed." },
    ColorKeyEntry { icon: "G_CorruptCanBeFixed.png", title: "CorruptCanBeFixed", description: "The ROM is corrupt, but another copy can repair it." },
    ColorKeyEntry { icon: "G_MoveToCorrupt.png", title: "MoveToCorrupt", description: "The file is corrupt and will be moved to the ToSort Corrupt folder." },
    ColorKeyEntry { icon: "G_MoveToSort.png", title: "MoveToSort", description: "The ROM is not needed here, but no copy is located elsewhere, so it will be moved to the primary ToSort." },
    ColorKeyEntry { icon: "G_Rename.png", title: "Rename", description: "The ROM is needed here, but has the incorrect name and will be renamed." },
    ColorKeyEntry { icon: "G_Delete.png", title: "Delete", description: "The ROM is not needed here, and a copy exists elsewhere, so it will be deleted." },
];

const GAME_LIST_PROBLEM: &[ColorKeyEntry] = &[
    ColorKeyEntry { icon: "G_Unknown.png", title: "Unknown", description: "The file could not be scanned." },
    ColorKeyEntry { icon: "G_Corrupt.png", title: "Corrupt", description: "The file is corrupt." },
    ColorKeyEntry { icon: "G_DirCorrupt.png", title: "DirCorrupt", description: "The archive containing the file is corrupt." },
    ColorKeyEntry { icon: "G_Incomplete.png", title: "Incomplete", description: "The ROM is needed here, but fixing would result in an incomplete set." },
];

const SET_TYPES_HAVE: &[ColorKeyEntry] = &[
    ColorKeyEntry { icon: "Dir.png", title: "Uncompressed Set", description: "The set is uncompressed." },
    ColorKeyEntry { icon: "Zip.png", title: "Zip", description: "The set is a regular ZIP archive." },
    ColorKeyEntry { icon: "ZipTrrnt.png", title: "TorrentZip", description: "The set is torrentzipped." },
    ColorKeyEntry { icon: "ZipTDC.png", title: "TDC Zip", description: "The set is zipped with deflate compression and matches a TDC DAT." },
    ColorKeyEntry { icon: "ZipZstd.png", title: "Zstd Zip", description: "The set is zipped with zstd compression." },
    ColorKeyEntry { icon: "SevenZip.png", title: "7z", description: "The set is a regular 7zip archive." },
    ColorKeyEntry { icon: "SevenZipTrrnt.png", title: "Torrent7z", description: "The set is torrent7zipped." },
    ColorKeyEntry { icon: "SevenZipNLZMA.png", title: "7z LZMA Non-Solid", description: "The set is a non-solid 7zip archive with LZMA compression." },
    ColorKeyEntry { icon: "SevenZipSLZMA.png", title: "7z LZMA Solid", description: "The set is a solid 7zip archive with LZMA compression." },
    ColorKeyEntry { icon: "SevenZipNZSTD.png", title: "7z Zstd Non-Solid", description: "The set is a non-solid 7zip archive with zstd compression." },
    ColorKeyEntry { icon: "SevenZipSZSTD.png", title: "7z Zstd Solid", description: "The set is a solid 7zip archive with zstd compression." },
];

const SET_TYPES_MISSING: &[ColorKeyEntry] = &[
    ColorKeyEntry { icon: "ZipMissing.png", title: "Zip Missing", description: "The ZIP set is missing." },
    ColorKeyEntry { icon: "ZipTrrntMissing.png", title: "TorrentZip Missing", description: "The torrentzipped set is missing." },
    ColorKeyEntry { icon: "ZipTDCMissing.png", title: "TDC Zip Missing", description: "The TDC zipped set is missing." },
    ColorKeyEntry { icon: "ZipZstdMissing.png", title: "Zstd Zip Missing", description: "The zstd zipped set is missing." },
    ColorKeyEntry { icon: "SevenZipMissing.png", title: "7z Missing", description: "The regular 7z set is missing." },
    ColorKeyEntry { icon: "SevenZipSLZMAMissing.png", title: "7z Solid LZMA Missing", description: "The solid LZMA 7z set is missing." },
    ColorKeyEntry { icon: "SevenZipSZSTDMissing.png", title: "7z Solid Zstd Missing", description: "The solid zstd 7z set is missing." },
];

const SET_TYPES_CORRUPT: &[ColorKeyEntry] = &[
    ColorKeyEntry { icon: "ZipCorrupt.png", title: "Zip Corrupt", description: "The ZIP set is corrupt." },
    ColorKeyEntry { icon: "ZipTrrntCorrupt.png", title: "TorrentZip Corrupt", description: "The torrentzipped set is corrupt." },
    ColorKeyEntry { icon: "SevenZipCorrupt.png", title: "7z Corrupt", description: "The 7z set is corrupt." },
    ColorKeyEntry { icon: "SevenZipSLZMACorrupt.png", title: "7z Solid LZMA Corrupt", description: "The solid LZMA 7z set is corrupt." },
    ColorKeyEntry { icon: "SevenZipSZSTDCorrupt.png", title: "7z Solid Zstd Corrupt", description: "The solid zstd 7z set is corrupt." },
];

const ROM_DETAILS_RESTING: &[ColorKeyEntry] = &[
    ColorKeyEntry { icon: "R_InDatCollect_Correct.png", title: "Correct", description: "The ROM is correct." },
    ColorKeyEntry { icon: "R_InDatMIA_CorrectMIA.png", title: "CorrectMIA", description: "The ROM is known to be MIA but you have it." },
    ColorKeyEntry { icon: "R_InDatCollect_Missing.png", title: "Missing", description: "The ROM is missing." },
    ColorKeyEntry { icon: "R_InDatMIA_MissingMIA.png", title: "MissingMIA", description: "The ROM is known to be MIA." },
    ColorKeyEntry { icon: "R_InDatMerged_UnNeeded.png", title: "UnNeeded", description: "The ROM is present but unneeded because it is wanted in a parent or primary deduped set." },
    ColorKeyEntry { icon: "R_InDatMerged_NotCollected.png", title: "NotCollected", description: "The ROM is uncollected because it is wanted in a parent or primary deduped set." },
    ColorKeyEntry { icon: "R_InToSort_InToSort.png", title: "InToSort", description: "The ROM is in a ToSort directory." },
    ColorKeyEntry { icon: "R_NotInDat_Ignore.png", title: "Ignore", description: "The file matches an ignore rule." },
    ColorKeyEntry { icon: "R_InDatCollect_UnScanned.png", title: "UnScanned", description: "The file is unscanned." },
    ColorKeyEntry { icon: "R_InDatCollect_Corrupt.png", title: "Corrupt", description: "The file is corrupt." },
];

const ROM_DETAILS_FIXING: &[ColorKeyEntry] = &[
    ColorKeyEntry { icon: "R_InDatCollect_CanBeFixed.png", title: "CanBeFixed", description: "The ROM is missing. It can be fixed." },
    ColorKeyEntry { icon: "R_InDatMIA_CanBeFixedMIA.png", title: "CanBeFixedMIA", description: "The ROM is missing and known to be MIA. It can be fixed." },
    ColorKeyEntry { icon: "R_InDatCollect_CorruptCanBeFixed.png", title: "CorruptCanBeFixed", description: "The ROM is corrupt. It can be fixed by another copy of the ROM." },
    ColorKeyEntry { icon: "R_NotInDat_Rename.png", title: "Rename", description: "The ROM is not needed here. It can be renamed to fix another ROM in the set." },
    ColorKeyEntry { icon: "R_InDatCollect_NeededForFix.png", title: "NeededForFix", description: "The ROM is not needed here. It can be used for a fix elsewhere." },
    ColorKeyEntry { icon: "R_NotInDat_MoveToSort.png", title: "MoveToSort", description: "The ROM is not needed here. It will be moved to the primary ToSort." },
    ColorKeyEntry { icon: "R_NotInDat_Delete.png", title: "Delete", description: "The ROM is not needed here. It will be deleted because a copy exists elsewhere." },
    ColorKeyEntry { icon: "R_InDatMerged_NotCollected.png", title: "Merged NotCollected", description: "The ROM is not needed here. It can be moved to the parent or primary deduped set." },
    ColorKeyEntry { icon: "R_InDatMerged_Delete.png", title: "Merged Delete", description: "The ROM is not needed here. It will be deleted because it is deduped or merged and a copy exists elsewhere." },
    ColorKeyEntry { icon: "R_InToSort_Delete.png", title: "ToSort Delete", description: "The ROM is no longer needed in ToSort. It will be deleted because a collected copy exists elsewhere." },
    ColorKeyEntry { icon: "R_InDatCollect_MoveToCorrupt.png", title: "MoveToCorrupt", description: "The file is corrupt. It will be moved to the primary ToSort Corrupt folder." },
];

const DAT_TREE_BRANCHES: &[ColorKeyEntry] = &[
    ColorKeyEntry { icon: "ExpandBoxPlus.png", title: "Collapsed Branch", description: "The branch is collapsed." },
    ColorKeyEntry { icon: "ExpandBoxMinus.png", title: "Expanded Branch", description: "The branch is expanded." },
    ColorKeyEntry { icon: "TickBoxUnTicked.png", title: "Deselected", description: "The branch is deselected. ROMs will not be scanned and cannot be used for fixes." },
    ColorKeyEntry { icon: "TickBoxTicked.png", title: "Selected", description: "The branch is selected. ROMs will be scanned and can be used for fixes." },
    ColorKeyEntry { icon: "TickBoxLocked.png", title: "Locked", description: "The branch is locked as read-only. ROMs will be scanned and can be used for fixes, however they will not be altered." },
];

const DAT_TREE_FOLDERS: &[ColorKeyEntry] = &[
    ColorKeyEntry { icon: "DirectoryTree3.png", title: "Folder Correct", description: "No ROMs are missing from any DATs in the branch." },
    ColorKeyEntry { icon: "DirectoryTree2.png", title: "Folder Partial", description: "Some ROMs are missing from DATs in the branch." },
    ColorKeyEntry { icon: "DirectoryTree1.png", title: "Folder Missing", description: "All ROMs are missing from DATs in the branch." },
    ColorKeyEntry { icon: "DirectoryTree4.png", title: "Folder Merged/Unknown", description: "The branch is merged, ignored, or unknown." },
    ColorKeyEntry { icon: "DirectoryTree5.png", title: "Folder ToSort", description: "The ToSort directory contains files present for fixes." },
];

const DAT_TREE_DATS: &[ColorKeyEntry] = &[
    ColorKeyEntry { icon: "Tree3.png", title: "DAT Correct", description: "No ROMs are missing." },
    ColorKeyEntry { icon: "Tree2.png", title: "DAT Partial", description: "Some ROMs are missing." },
    ColorKeyEntry { icon: "Tree1.png", title: "DAT Missing", description: "All ROMs are missing." },
    ColorKeyEntry { icon: "Tree4.png", title: "DAT Merged/Unknown", description: "The DAT is merged, ignored, or unknown." },
    ColorKeyEntry { icon: "Tree5.png", title: "DAT ToSort", description: "The DAT branch contains only fixable ToSort work." },
];

fn color_key_sections() -> &'static [ColorKeySection] {
    &[
        ColorKeySection { title: "Game List Grid - Resting", entries: GAME_LIST_RESTING },
        ColorKeySection { title: "Game List Grid - Fixing", entries: GAME_LIST_FIXING },
        ColorKeySection { title: "Game List Grid - Problem", entries: GAME_LIST_PROBLEM },
        ColorKeySection { title: "Set Types - Have", entries: SET_TYPES_HAVE },
        ColorKeySection { title: "Set Types - Missing", entries: SET_TYPES_MISSING },
        ColorKeySection { title: "Set Types - Corrupt", entries: SET_TYPES_CORRUPT },
        ColorKeySection { title: "ROM Details Grid - Resting Statuses", entries: ROM_DETAILS_RESTING },
        ColorKeySection { title: "ROM Details Grid - Fix Actions", entries: ROM_DETAILS_FIXING },
        ColorKeySection { title: "DAT Tree - Branches and Checkboxes", entries: DAT_TREE_BRANCHES },
        ColorKeySection { title: "DAT Tree - Folders", entries: DAT_TREE_FOLDERS },
        ColorKeySection { title: "DAT Tree - DATs", entries: DAT_TREE_DATS },
    ]
}

fn color_key_entry_count() -> usize {
    color_key_sections().iter().map(|section| section.entries.len()).sum()
}

fn color_key_icon_source(icon: &'static str) -> egui::ImageSource<'static> {
    match icon {
        "G_Correct.png" => include_asset!("G_Correct.png"),
        "G_CorrectMIA.png" => include_asset!("G_CorrectMIA.png"),
        "G_Missing.png" => include_asset!("G_Missing.png"),
        "G_MissingMIA.png" => include_asset!("G_MissingMIA.png"),
        "G_InToSort.png" => include_asset!("G_InToSort.png"),
        "G_UnNeeded.png" => include_asset!("G_UnNeeded.png"),
        "G_NotCollected.png" => include_asset!("G_NotCollected.png"),
        "G_Ignore.png" => include_asset!("G_Ignore.png"),
        "G_CanBeFixed.png" => include_asset!("G_CanBeFixed.png"),
        "G_CanBeFixedMIA.png" => include_asset!("G_CanBeFixedMIA.png"),
        "G_CorruptCanBeFixed.png" => include_asset!("G_CorruptCanBeFixed.png"),
        "G_MoveToCorrupt.png" => include_asset!("G_MoveToCorrupt.png"),
        "G_MoveToSort.png" => include_asset!("G_MoveToSort.png"),
        "G_Rename.png" => include_asset!("G_Rename.png"),
        "G_Delete.png" => include_asset!("G_Delete.png"),
        "G_Unknown.png" => include_asset!("G_Unknown.png"),
        "G_Corrupt.png" => include_asset!("G_Corrupt.png"),
        "G_DirCorrupt.png" => include_asset!("G_DirCorrupt.png"),
        "G_Incomplete.png" => include_asset!("G_Incomplete.png"),
        "Dir.png" => include_asset!("Dir.png"),
        "Zip.png" => include_asset!("Zip.png"),
        "ZipTrrnt.png" => include_asset!("ZipTrrnt.png"),
        "ZipTDC.png" => include_asset!("ZipTDC.png"),
        "ZipZstd.png" => include_asset!("ZipZstd.png"),
        "SevenZip.png" => include_asset!("SevenZip.png"),
        "SevenZipTrrnt.png" => include_asset!("SevenZipTrrnt.png"),
        "SevenZipNLZMA.png" => include_asset!("SevenZipNLZMA.png"),
        "SevenZipSLZMA.png" => include_asset!("SevenZipSLZMA.png"),
        "SevenZipNZSTD.png" => include_asset!("SevenZipNZSTD.png"),
        "SevenZipSZSTD.png" => include_asset!("SevenZipSZSTD.png"),
        "ZipMissing.png" => include_asset!("ZipMissing.png"),
        "ZipTrrntMissing.png" => include_asset!("ZipTrrntMissing.png"),
        "ZipTDCMissing.png" => include_asset!("ZipTDCMissing.png"),
        "ZipZstdMissing.png" => include_asset!("ZipZstdMissing.png"),
        "SevenZipMissing.png" => include_asset!("SevenZipMissing.png"),
        "SevenZipSLZMAMissing.png" => include_asset!("SevenZipSLZMAMissing.png"),
        "SevenZipSZSTDMissing.png" => include_asset!("SevenZipSZSTDMissing.png"),
        "ZipCorrupt.png" => include_asset!("ZipCorrupt.png"),
        "ZipTrrntCorrupt.png" => include_asset!("ZipTrrntCorrupt.png"),
        "SevenZipCorrupt.png" => include_asset!("SevenZipCorrupt.png"),
        "SevenZipSLZMACorrupt.png" => include_asset!("SevenZipSLZMACorrupt.png"),
        "SevenZipSZSTDCorrupt.png" => include_asset!("SevenZipSZSTDCorrupt.png"),
        "R_InDatCollect_Correct.png" => include_asset!("R_InDatCollect_Correct.png"),
        "R_InDatMIA_CorrectMIA.png" => include_asset!("R_InDatMIA_CorrectMIA.png"),
        "R_InDatCollect_Missing.png" => include_asset!("R_InDatCollect_Missing.png"),
        "R_InDatMIA_MissingMIA.png" => include_asset!("R_InDatMIA_MissingMIA.png"),
        "R_InDatMerged_UnNeeded.png" => include_asset!("R_InDatMerged_UnNeeded.png"),
        "R_InDatMerged_NotCollected.png" => include_asset!("R_InDatMerged_NotCollected.png"),
        "R_InToSort_InToSort.png" => include_asset!("R_InToSort_InToSort.png"),
        "R_NotInDat_Ignore.png" => include_asset!("R_NotInDat_Ignore.png"),
        "R_InDatCollect_UnScanned.png" => include_asset!("R_InDatCollect_UnScanned.png"),
        "R_InDatCollect_Corrupt.png" => include_asset!("R_InDatCollect_Corrupt.png"),
        "R_InDatCollect_CanBeFixed.png" => include_asset!("R_InDatCollect_CanBeFixed.png"),
        "R_InDatMIA_CanBeFixedMIA.png" => include_asset!("R_InDatMIA_CanBeFixedMIA.png"),
        "R_InDatCollect_CorruptCanBeFixed.png" => include_asset!("R_InDatCollect_CorruptCanBeFixed.png"),
        "R_NotInDat_Rename.png" => include_asset!("R_NotInDat_Rename.png"),
        "R_InDatCollect_NeededForFix.png" => include_asset!("R_InDatCollect_NeededForFix.png"),
        "R_NotInDat_MoveToSort.png" => include_asset!("R_NotInDat_MoveToSort.png"),
        "R_NotInDat_Delete.png" => include_asset!("R_NotInDat_Delete.png"),
        "R_InDatMerged_Delete.png" => include_asset!("R_InDatMerged_Delete.png"),
        "R_InToSort_Delete.png" => include_asset!("R_InToSort_Delete.png"),
        "R_InDatCollect_MoveToCorrupt.png" => include_asset!("R_InDatCollect_MoveToCorrupt.png"),
        "ExpandBoxPlus.png" => include_asset!("ExpandBoxPlus.png"),
        "ExpandBoxMinus.png" => include_asset!("ExpandBoxMinus.png"),
        "TickBoxUnTicked.png" => include_asset!("TickBoxUnTicked.png"),
        "TickBoxTicked.png" => include_asset!("TickBoxTicked.png"),
        "TickBoxLocked.png" => include_asset!("TickBoxLocked.png"),
        "DirectoryTree1.png" => include_asset!("DirectoryTree1.png"),
        "DirectoryTree2.png" => include_asset!("DirectoryTree2.png"),
        "DirectoryTree3.png" => include_asset!("DirectoryTree3.png"),
        "DirectoryTree4.png" => include_asset!("DirectoryTree4.png"),
        "DirectoryTree5.png" => include_asset!("DirectoryTree5.png"),
        "Tree1.png" => include_asset!("Tree1.png"),
        "Tree2.png" => include_asset!("Tree2.png"),
        "Tree3.png" => include_asset!("Tree3.png"),
        "Tree4.png" => include_asset!("Tree4.png"),
        "Tree5.png" => include_asset!("Tree5.png"),
        _ => unreachable!(),
    }
}

fn render_color_key_entry(ui: &mut egui::Ui, entry: ColorKeyEntry) {
    ui.horizontal(|ui| {
        ui.add(
            egui::Image::new(color_key_icon_source(entry.icon))
                .fit_to_exact_size(egui::vec2(20.0, 20.0))
                .texture_options(egui::TextureOptions::NEAREST),
        );
        ui.vertical(|ui| {
            ui.strong(entry.title);
            ui.label(entry.description);
        });
    });
}

#[cfg(test)]
#[path = "tests/dialogs_tests.rs"]
mod tests;

/// Logic for drawing all popup dialog windows in the application.
/// 
/// `dialogs.rs` handles rendering the Global Settings, Directory Settings, Directory Mappings,
/// Add ToSort, and About popups.
/// 
/// Differences from C#:
/// - The C# version utilizes individual `.Designer.cs` WinForms definitions for every single popup 
///   dialog (e.g. `FrmSettings`, `FrmDirectorySettings`, `FrmRegistration`).
/// - The Rust version groups all of these popups into a single `draw_dialogs` function, toggling
///   their visibility via boolean state flags stored in the main `RomVaultApp` struct.
pub fn draw_dialogs(app: &mut RomVaultApp, ctx: &egui::Context) {
    if app.show_dir_mappings {
        let mut close_dir_mappings = false;
        egui::Window::new("Directory Mappings")
            .open(&mut app.show_dir_mappings)
            .resizable(true)
            .default_width(600.0)
            .show(ctx, |ui| {
                ui.heading("Directory Mappings");
                ui.separator();

                // Faithful C# RomVault layout: Table with "Directory" and "Mapping" columns
                let table = egui_extras::TableBuilder::new(ui)
                    .striped(true)
                    .resizable(true)
                    .cell_layout(egui::Layout::left_to_right(egui::Align::Center))
                    .column(egui_extras::Column::initial(200.0).at_least(100.0)) // Directory
                    .column(egui_extras::Column::remainder()) // Mapping
                    .min_scrolled_height(200.0);

                table.header(24.0, |mut header| {
                    header.col(|ui| { ui.strong("Directory"); });
                    header.col(|ui| { ui.strong("Mapping"); });
                })
                .body(|mut body| {
                    let mappings_len = app.working_dir_mappings.len();
                    for i in 0..mappings_len {
                        let is_selected = app.selected_dir_mapping_idx == Some(i);
                        
                        body.row(26.0, |mut row| {
                            // Directory column
                            row.col(|ui| {
                                if is_selected {
                                    ui.painter().rect_filled(ui.max_rect(), 0.0, ui.visuals().selection.bg_fill.linear_multiply(0.3));
                                }
                                let mut key = app.working_dir_mappings[i].dir_key.clone();
                                let response = ui.add(egui::TextEdit::singleline(&mut key).desired_width(ui.available_width()).frame(false));
                                if response.changed() {
                                    app.working_dir_mappings[i].dir_key = key;
                                }
                                if response.gained_focus() {
                                    app.selected_dir_mapping_idx = Some(i);
                                }
                            });
                            // Mapping column
                            row.col(|ui| {
                                if is_selected {
                                    ui.painter().rect_filled(ui.max_rect(), 0.0, ui.visuals().selection.bg_fill.linear_multiply(0.3));
                                }
                                let mut path = app.working_dir_mappings[i].dir_path.clone();
                                let response = ui.add(egui::TextEdit::singleline(&mut path).desired_width(ui.available_width()).frame(false));
                                if response.changed() {
                                    app.working_dir_mappings[i].dir_path = path;
                                }
                                if response.gained_focus() {
                                    app.selected_dir_mapping_idx = Some(i);
                                }
                            });
                        });
                    }
                });

                ui.add_space(10.0);

                ui.horizontal(|ui| {
                    if ui.button("Add").clicked() {
                        app.working_dir_mappings.push(rv_core::settings::DirMapping {
                            dir_key: "NewDirectory".to_string(),
                            dir_path: "NewMapping".to_string(),
                        });
                        app.selected_dir_mapping_idx = Some(app.working_dir_mappings.len() - 1);
                    }
                    if ui.button("Remove").clicked() {
                        if let Some(idx) = app.selected_dir_mapping_idx {
                            if idx < app.working_dir_mappings.len() {
                                app.working_dir_mappings.remove(idx);
                                app.selected_dir_mapping_idx = None;
                            }
                        }
                    }
                });

                ui.separator();
                ui.horizontal(|ui| {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button("Cancel").clicked() {
                            close_dir_mappings = true;
                        }
                        if ui.button("OK").clicked() {
                            app.global_settings.dir_mappings.items = app.working_dir_mappings.clone();
                            rv_core::settings::update_settings(app.global_settings.clone());
                            app.task_logs.push("Saved Directory Mappings".to_string());
                            close_dir_mappings = true;
                        }
                    });
                });
            });

        if close_dir_mappings {
            app.show_dir_mappings = false;
        }
    }

    if app.show_sam_dialog {
        let mut close_sam = false;
        let mut sam_window_open = app.show_sam_dialog;
        egui::Window::new("Structured Archive Maker")
            .open(&mut sam_window_open)
            .resizable(true)
            .default_width(860.0)
            .default_height(560.0)
            .show(ctx, |ui| {
                ui.heading("Structured Archive Maker");
                ui.separator();
                ui.label("Convert directories and archives into normalized output formats using a workflow closer to the classic RomVault SAM tool.");
                ui.add_space(8.0);

                ui.columns(2, |columns| {
                    columns[0].group(|ui| {
                        ui.heading("Source Files");
                        ui.separator();
                        ui.label("Files queued for conversion");
                        egui::ScrollArea::vertical()
                            .id_source("sam_source_files")
                            .max_height(220.0)
                            .show(ui, |ui| {
                                if app.sam_source_items.is_empty() {
                                    ui.label("No source files added.");
                                } else {
                                    for (idx, item) in app.sam_source_items.iter().enumerate() {
                                        let selected = app.sam_selected_source_idx == Some(idx);
                                        if ui.selectable_label(selected, item).clicked() {
                                            app.sam_selected_source_idx = Some(idx);
                                        }
                                    }
                                }
                            });

                        ui.add_space(6.0);
                        ui.label("Add source path");
                        ui.text_edit_singleline(&mut app.sam_pending_source_path);
                        ui.horizontal(|ui| {
                            let pending = app.sam_pending_source_path.trim();
                            if ui.add_enabled(!pending.is_empty(), egui::Button::new("Add")).clicked() {
                                if !app.sam_source_items.iter().any(|item| item.eq_ignore_ascii_case(pending)) {
                                    app.sam_source_items.push(pending.to_string());
                                    app.sam_selected_source_idx = Some(app.sam_source_items.len() - 1);
                                }
                                app.sam_pending_source_path.clear();
                            }
                            if ui.add_enabled(app.sam_selected_source_idx.is_some(), egui::Button::new("Remove")).clicked() {
                                if let Some(idx) = app.sam_selected_source_idx.take() {
                                    if idx < app.sam_source_items.len() {
                                        app.sam_source_items.remove(idx);
                                    }
                                }
                            }
                            if ui.add_enabled(!app.sam_source_items.is_empty(), egui::Button::new("Clear")).clicked() {
                                app.sam_source_items.clear();
                                app.sam_selected_source_idx = None;
                            }
                        });
                    });

                    columns[1].group(|ui| {
                        ui.heading("Options");
                        ui.separator();

                        ui.label("Input Type");
                        egui::ComboBox::from_id_source("sam_input_kind")
                            .selected_text(app.sam_input_kind.label())
                            .show_ui(ui, |ui| {
                                for option in SAM_INPUT_OPTIONS {
                                    ui.selectable_value(&mut app.sam_input_kind, option, option.label());
                                }
                            });

                        ui.label("Output Type");
                        egui::ComboBox::from_id_source("sam_output_kind")
                            .selected_text(app.sam_output_kind.label())
                            .show_ui(ui, |ui| {
                                for option in SAM_OUTPUT_OPTIONS {
                                    let supported = crate::RomVaultApp::sam_output_kind_supported(option);
                                    let label = if supported {
                                        option.label().to_string()
                                    } else {
                                        format!("{} (Unavailable)", option.label())
                                    };
                                    ui.add_enabled_ui(supported, |ui| {
                                        ui.selectable_value(&mut app.sam_output_kind, option, label);
                                    });
                                }
                            });

                        ui.add_space(8.0);
                        if ui
                            .checkbox(&mut app.sam_use_origin_output, "Use source location for output")
                            .changed()
                            && app.sam_use_origin_output
                        {
                            app.sam_output_directory.clear();
                        }
                        ui.label(if app.sam_use_origin_output {
                            "Output Directory (Disabled: origin output enabled)"
                        } else {
                            "Output Directory"
                        });
                        if app.sam_use_origin_output {
                            ui.add_enabled(
                                false,
                                egui::TextEdit::singleline(&mut app.sam_output_directory)
                                    .hint_text("Disabled while using source location output"),
                            );
                        } else {
                            ui.text_edit_singleline(&mut app.sam_output_directory);
                        }

                        ui.add_space(8.0);
                        ui.checkbox(&mut app.sam_recurse_subdirs, "Recurse subdirectories");
                        ui.checkbox(&mut app.sam_rebuild_existing, "Rebuild archives even if output exists");
                        ui.checkbox(&mut app.sam_remove_source, "Remove source files after successful conversion");
                        ui.checkbox(&mut app.sam_verify_output, "Verify output after conversion");

                        ui.add_space(8.0);
                        ui.heading("Status");
                        ui.separator();
                        ui.label(format!("Queued items: {}", app.sam_source_items.len()));
                        ui.label(format!("Completed items: {}/{}", app.sam_completed_items, app.sam_total_items));
                        ui.label(format!("Input profile: {}", app.sam_input_kind.label()));
                        ui.label(format!("Output profile: {}", app.sam_output_kind.label()));
                        ui.label(format!("Run state: {}", app.sam_status_text));
                        if let Some(current_item) = app.sam_current_item.as_ref() {
                            ui.label(format!("Current item: {}", current_item));
                        }
                        ui.label(if app.sam_use_origin_output {
                            "Output uses each source item's origin location"
                        } else if app.sam_output_directory.trim().is_empty() {
                            "Output directory not set"
                        } else {
                            "Output directory ready"
                        });
                        if let Some(message) = crate::RomVaultApp::sam_output_kind_support_message(app.sam_output_kind) {
                            ui.label(message);
                        }
                        ui.add_space(6.0);
                        ui.label("Soft stop finishes the current conversion and then stops. Hard stop aborts immediately and cleans up any remaining .samtmp files.");
                    });
                });

                ui.separator();
                ui.horizontal(|ui| {
                    let can_start = !app.sam_running
                        && !app.sam_source_items.is_empty()
                        && app.sam_has_usable_output_target()
                        && crate::RomVaultApp::sam_output_kind_supported(app.sam_output_kind);
                    if ui.add_enabled(can_start, egui::Button::new("Start")).clicked() {
                        app.start_sam_job();
                    }
                    if ui
                        .add_enabled(app.sam_running && !app.sam_soft_stop_requested, egui::Button::new("Soft Stop"))
                        .clicked()
                    {
                        app.request_sam_soft_stop();
                    }
                    if ui
                        .add_enabled(app.sam_running && !app.sam_hard_stop_requested, egui::Button::new("Hard Stop"))
                        .clicked()
                    {
                        app.request_sam_hard_stop();
                    }
                    if ui.button("Close").clicked() {
                        close_sam = true;
                    }
                });
            });
        app.show_sam_dialog = sam_window_open;
        if close_sam {
            app.show_sam_dialog = false;
        }
    }

    if app.show_color_key {
        let mut close_color_key = false;
        egui::Window::new("Color and Icon Key")
            .open(&mut app.show_color_key)
            .resizable(true)
            .default_width(780.0)
            .default_height(720.0)
            .show(ctx, |ui| {
                ui.heading("Color and Icon Key");
                ui.label("RomVault uses icons to show ROM, set, and tree states. This legend mirrors the more extensive reference application layout.");
                ui.label(format!("{} legend entries", color_key_entry_count()));
                ui.separator();
                egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                    for section in color_key_sections() {
                        ui.heading(section.title);
                        ui.separator();
                        for entry in section.entries {
                            render_color_key_entry(ui, *entry);
                            ui.add_space(4.0);
                        }
                        ui.add_space(10.0);
                    }
                });
                ui.add_space(10.0);
                if ui.button("Close").clicked() {
                    close_color_key = true;
                }
            });
        if close_color_key {
            app.show_color_key = false;
        }
    }

    if app.show_about {
        let mut close_about = false;
        egui::Window::new("About RustyVault")
            .open(&mut app.show_about)
            .show(ctx, |ui| {
                let startup_path = std::env::current_exe()
                    .ok()
                    .and_then(|p| p.parent().map(|p| p.to_string_lossy().to_string()))
                    .unwrap_or_else(|| std::env::current_dir().unwrap_or_default().to_string_lossy().to_string());

                ui.vertical_centered(|ui| {
                    ui.heading("RustyVault");
                    ui.label(format!("Version 3.6.1 : {}", startup_path));
                    ui.add_space(10.0);
                    ui.label("ROMVault3 is written by Gordon J.");
                    ui.label("Forked/ported as RustyVault");
                    ui.add_space(10.0);

                    ui.horizontal(|ui| {
                        if ui.button("Website").clicked() {
                            let _ = std::process::Command::new("cmd")
                                .args(["/C", "start", "", "http://www.romvault.com/"])
                                .spawn();
                        }
                        if ui.button("PayPal").clicked() {
                            let _ = std::process::Command::new("cmd")
                                .args(["/C", "start", "", "http://paypal.me/romvault"])
                                .spawn();
                        }
                        if ui.button("Patreon").clicked() {
                            let _ = std::process::Command::new("cmd")
                                .args(["/C", "start", "", "https://www.patreon.com/romvault"])
                                .spawn();
                        }
                    });

                    ui.add_space(10.0);
                    if ui.button("Close").clicked() {
                        close_about = true;
                    }
                });
            });
        if close_about {
            app.show_about = false;
        }
    }

    if app.show_rom_info {
        let mut close_rom_info = false;
        egui::Window::new("Rom Occurrence list")
            .open(&mut app.show_rom_info)
            .show(ctx, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    for line in &app.rom_info_lines {
                        ui.label(line);
                    }
                });

                ui.add_space(10.0);
                if ui.button("Close").clicked() {
                    close_rom_info = true;
                }
            });
        if close_rom_info {
            app.show_rom_info = false;
            app.selected_rom_for_info = None;
            app.rom_info_lines.clear();
        }
    }

    let mut close_dir_settings = false;
    if app.show_dir_settings {
        egui::Window::new("Directory Settings")
            .open(&mut app.show_dir_settings)
            .default_width(450.0)
            .show(ctx, |ui| {
                ui.heading(format!("Settings for: {}", app.active_dat_rule.dir_key));
                ui.separator();

                ui.horizontal(|ui| {
                    ui.selectable_value(&mut app.dir_settings_tab, 0, "Dir/Merge");
                    ui.selectable_value(&mut app.dir_settings_tab, 1, "Advanced");
                    ui.selectable_value(&mut app.dir_settings_tab, 2, "Exclude");
                });
                ui.separator();

                if app.dir_settings_tab == 0 {
                    ui.label("Archive Type:");
                    egui::ComboBox::from_id_source("archive_type")
                        .selected_text(match app.active_dat_rule.compression {
                            dat_reader::enums::FileType::File => "Uncompressed",
                            dat_reader::enums::FileType::Zip => "Zip",
                            dat_reader::enums::FileType::SevenZip => "SevenZip",
                            dat_reader::enums::FileType::FileOnly => "Mixed (Archive as File)",
                            _ => "Unknown",
                        })
                        .show_ui(ui, |ui| {
                            ui.selectable_value(&mut app.active_dat_rule.compression, dat_reader::enums::FileType::File, "Uncompressed");
                            ui.selectable_value(&mut app.active_dat_rule.compression, dat_reader::enums::FileType::Zip, "Zip");
                            ui.selectable_value(&mut app.active_dat_rule.compression, dat_reader::enums::FileType::SevenZip, "SevenZip");
                            ui.selectable_value(&mut app.active_dat_rule.compression, dat_reader::enums::FileType::FileOnly, "Mixed (Archive as File)");
                        });
                    ui.checkbox(&mut app.active_dat_rule.compression_override_dat, "Override DAT Archive Type");

                    ui.separator();

                    ui.label("Merge Type:");
                    egui::ComboBox::from_id_source("merge_type")
                        .selected_text(format!("{:?}", app.active_dat_rule.merge))
                        .show_ui(ui, |ui| {
                            ui.selectable_value(&mut app.active_dat_rule.merge, rv_core::settings::MergeType::None, "Nothing");
                            ui.selectable_value(&mut app.active_dat_rule.merge, rv_core::settings::MergeType::Split, "Split");
                            ui.selectable_value(&mut app.active_dat_rule.merge, rv_core::settings::MergeType::Merge, "Merge");
                            ui.selectable_value(&mut app.active_dat_rule.merge, rv_core::settings::MergeType::NonMerged, "NonMerge");
                        });
                    ui.checkbox(&mut app.active_dat_rule.merge_override_dat, "Override DAT Merge Type");

                    ui.separator();

                    ui.label("Header Type:");
                    egui::ComboBox::from_id_source("header_type")
                        .selected_text(format!("{:?}", app.active_dat_rule.header_type))
                        .show_ui(ui, |ui| {
                            ui.selectable_value(&mut app.active_dat_rule.header_type, rv_core::settings::HeaderType::Optional, "Optional");
                            ui.selectable_value(&mut app.active_dat_rule.header_type, rv_core::settings::HeaderType::Headered, "Headered");
                            ui.selectable_value(&mut app.active_dat_rule.header_type, rv_core::settings::HeaderType::Headerless, "Headerless");
                        });

                    ui.separator();

                    ui.label("Filter Type:");
                    egui::ComboBox::from_id_source("filter_type")
                        .selected_text(format!("{:?}", app.active_dat_rule.filter))
                        .show_ui(ui, |ui| {
                            ui.selectable_value(&mut app.active_dat_rule.filter, rv_core::settings::FilterType::KeepAll, "Roms & CHDs");
                            ui.selectable_value(&mut app.active_dat_rule.filter, rv_core::settings::FilterType::RomsOnly, "Roms Only");
                            ui.selectable_value(&mut app.active_dat_rule.filter, rv_core::settings::FilterType::CHDsOnly, "CHDs Only");
                        });

                    ui.separator();

                    ui.label("Directory Type:");
                    egui::ComboBox::from_id_source("dir_type")
                        .selected_text(format!("{:?}", app.active_dat_rule.sub_dir_type))
                        .show_ui(ui, |ui| {
                            ui.selectable_value(&mut app.active_dat_rule.sub_dir_type, rv_core::settings::RemoveSubType::KeepAllSubDirs, "Use subdirs for all sets");
                            ui.selectable_value(&mut app.active_dat_rule.sub_dir_type, rv_core::settings::RemoveSubType::RemoveAllSubDirs, "Do not use subdirs for sets");
                            ui.selectable_value(&mut app.active_dat_rule.sub_dir_type, rv_core::settings::RemoveSubType::RemoveSubIfNameMatches, "Use subdirs for rom name conflicts");
                            ui.selectable_value(&mut app.active_dat_rule.sub_dir_type, rv_core::settings::RemoveSubType::RemoveSubIfSingleGame, "Use subdirs for multi-rom sets");
                            ui.selectable_value(&mut app.active_dat_rule.sub_dir_type, rv_core::settings::RemoveSubType::RemoveSubIfSingleOrMatches, "Use subdirs for multi-rom sets or set/rom name mismatches");
                        });

                    ui.separator();

                    ui.checkbox(&mut app.active_dat_rule.single_archive, "Merge into a single archive");
                    ui.checkbox(&mut app.active_dat_rule.use_description_as_dir_name, "Use description for Directory Name");
                    ui.checkbox(&mut app.active_dat_rule.multi_dat_dir_override, "Override multi DAT directory");
                } else if app.dir_settings_tab == 1 {
                    ui.checkbox(&mut app.active_dat_rule.use_id_for_name, "Use ID for Name");
                    ui.checkbox(&mut app.active_dat_rule.complete_only, "Complete Only");
                    
                    let prev_checked = app.active_dat_rule.add_category_sub_dirs;
                    if ui.checkbox(&mut app.active_dat_rule.add_category_sub_dirs, "Add Category Sub Dirs").changed()
                        && app.active_dat_rule.add_category_sub_dirs
                        && !prev_checked
                        && app.active_dat_rule.category_order.items.is_empty()
                    {
                        app.active_dat_rule.category_order.items = vec![
                            "Preproduction".to_string(),
                            "Educational".to_string(),
                            "Guides".to_string(),
                            "Manuals".to_string(),
                            "Magazines".to_string(),
                            "Documents".to_string(),
                            "Audio".to_string(),
                            "Video".to_string(),
                            "Multimedia".to_string(),
                            "Coverdiscs".to_string(),
                            "Covermount".to_string(),
                            "Bonus Discs".to_string(),
                            "Bonus".to_string(),
                            "Add-Ons".to_string(),
                            "Source Code".to_string(),
                            "Updates".to_string(),
                            "Applications".to_string(),
                            "Demos".to_string(),
                            "Games".to_string(),
                            "Miscellaneous".to_string(),
                        ];
                    }
                    ui.separator();
                    ui.add_enabled_ui(app.active_dat_rule.add_category_sub_dirs, |ui| {
                        ui.label("Category Order (one per line):");
                        let mut cat_str = app.active_dat_rule.category_order.items.join("\n");
                        if ui.text_edit_multiline(&mut cat_str).changed() {
                            app.active_dat_rule.category_order.items = cat_str
                                .lines()
                                .map(|s: &str| s.to_string())
                                .filter(|s: &String| !s.is_empty())
                                .collect();
                        }
                    });

                } else if app.dir_settings_tab == 2 {
                    ui.label("Ignore Files (one per line):");
                    let mut ignore_str = app.active_dat_rule.ignore_files.items.join("\n");
                    if ui.text_edit_multiline(&mut ignore_str).changed() {
                        app.active_dat_rule.ignore_files.items = ignore_str
                            .lines()
                            .map(|s: &str| s.to_string())
                            .filter(|s: &String| !s.is_empty())
                            .collect();
                    }
                }

                ui.separator();
                if ui.button("Apply").clicked() {
                    rv_core::settings::set_rule(app.active_dat_rule.clone());
                    app.task_logs.push(format!(
                        "Applied Directory Settings for {}",
                        app.active_dat_rule.dir_key
                    ));
                    close_dir_settings = true;
                }
            });
    }
    if close_dir_settings {
        app.show_dir_settings = false;
    }

    let mut close_settings = false;
    if app.show_settings {
        egui::Window::new("Global Settings")
            .open(&mut app.show_settings)
            .default_width(450.0)
            .show(ctx, |ui| {
                ui.heading("RomVault Settings");
                ui.separator();

                ui.horizontal(|ui| {
                    ui.selectable_value(&mut app.global_settings_tab, 0, "Core Settings");
                    ui.selectable_value(&mut app.global_settings_tab, 1, "DATVault / Compression");
                    ui.selectable_value(&mut app.global_settings_tab, 2, "Logging / UI");
                });
                ui.separator();

                if app.global_settings_tab == 0 {
                    ui.horizontal(|ui| {
                        ui.label("DAT Root Directory:");
                        ui.text_edit_singleline(&mut app.global_settings.dat_root);
                    });

                    ui.horizontal(|ui| {
                        ui.label("Cache File:");
                        ui.text_edit_singleline(&mut app.global_settings.cache_file);
                    });

                    ui.horizontal(|ui| {
                        ui.label("Fix DAT Output Path:");
                        let mut fix_path = app.global_settings.fix_dat_out_path.clone().unwrap_or_default();
                        if ui.text_edit_singleline(&mut fix_path).changed() {
                            app.global_settings.fix_dat_out_path =
                                if fix_path.is_empty() { None } else { Some(fix_path) };
                        }
                    });

                    ui.add_space(5.0);
                    ui.label("Fix Level:");
                    egui::ComboBox::from_id_source("fix_level")
                        .selected_text(match app.global_settings.fix_level {
                            rv_core::settings::EFixLevel::Level1 => "Level 1 - Fast copy Match on CRC",
                            rv_core::settings::EFixLevel::Level2 => "Level 2 - Fast copy if SHA1 scanned",
                            rv_core::settings::EFixLevel::Level3 => "Level 3 - Uncompress/Hash/Compress",
                        })
                        .show_ui(ui, |ui| {
                            ui.selectable_value(
                                &mut app.global_settings.fix_level,
                                rv_core::settings::EFixLevel::Level1,
                                "Level 1 - Fast copy Match on CRC",
                            );
                            ui.selectable_value(
                                &mut app.global_settings.fix_level,
                                rv_core::settings::EFixLevel::Level2,
                                "Level 2 - Fast copy if SHA1 scanned",
                            );
                            ui.selectable_value(
                                &mut app.global_settings.fix_level,
                                rv_core::settings::EFixLevel::Level3,
                                "Level 3 - Uncompress/Hash/Compress",
                            );
                        });

                    ui.add_space(5.0);
                    ui.label("Filenames not to remove (one per line):");
                    let mut ignore_str = app.global_settings.ignore_files.items.join("\n");
                    if ui.text_edit_multiline(&mut ignore_str).changed() {
                        app.global_settings.ignore_files.items = ignore_str
                            .lines()
                            .map(|s: &str| s.to_string())
                            .filter(|s: &String| !s.is_empty())
                            .collect();
                    }

                    ui.add_space(5.0);
                    ui.checkbox(&mut app.global_settings.double_check_delete, "Double check file exists elsewhere before deleting");

                    ui.horizontal(|ui| {
                        ui.checkbox(&mut app.global_settings.cache_save_timer_enabled, "Save Cache on timer every");
                        if app.global_settings.cache_save_timer_enabled {
                            ui.add(egui::DragValue::new(&mut app.global_settings.cache_save_time_period).speed(1).clamp_range(5..=60));
                            ui.label("Minutes");
                        }
                    });

                } else if app.global_settings_tab == 1 {
                    ui.label("DATVault:");
                    ui.checkbox(&mut app.global_settings.mia_callback, "Send Found MIA notifications");
                    if app.global_settings.mia_callback {
                        ui.horizontal(|ui| {
                            ui.add_space(20.0);
                            ui.checkbox(&mut app.global_settings.mia_anon, "Send anonymously");
                        });
                    }
                    
                    ui.checkbox(&mut app.global_settings.delete_old_cue_files, "Delete previous Cue file zips in ToSort");

                    ui.add_space(10.0);
                    ui.label("Compression:");
                    ui.horizontal(|ui| {
                        ui.label("Max ZSTD workers:");
                        egui::ComboBox::from_id_source("zstd_workers")
                            .selected_text(if app.global_settings.zstd_comp_count == 0 { "Auto".to_string() } else { app.global_settings.zstd_comp_count.to_string() })
                            .show_ui(ui, |ui| {
                                ui.selectable_value(&mut app.global_settings.zstd_comp_count, 0, "Auto");
                                for i in 1..=64 {
                                    ui.selectable_value(&mut app.global_settings.zstd_comp_count, i, i.to_string());
                                }
                            });
                    });

                    ui.horizontal(|ui| {
                        ui.label("Default 7Z type:");
                        egui::ComboBox::from_id_source("7z_struct")
                            .selected_text(match app.global_settings.seven_z_default_struct {
                                0 => "LZMA Solid - rv7z",
                                1 => "LZMA Non-Solid",
                                2 => "ZSTD Solid",
                                3 => "ZSTD Non-Solid",
                                _ => "Unknown",
                            })
                            .show_ui(ui, |ui| {
                                ui.selectable_value(&mut app.global_settings.seven_z_default_struct, 0, "LZMA Solid - rv7z");
                                ui.selectable_value(&mut app.global_settings.seven_z_default_struct, 1, "LZMA Non-Solid");
                                ui.selectable_value(&mut app.global_settings.seven_z_default_struct, 2, "ZSTD Solid");
                                ui.selectable_value(&mut app.global_settings.seven_z_default_struct, 3, "ZSTD Non-Solid");
                            });
                    });

                } else if app.global_settings_tab == 2 {
                    ui.checkbox(&mut app.global_settings.detailed_fix_reporting, "Show detailed actions in Fixing Status window");
                    ui.checkbox(&mut app.global_settings.debug_logs_enabled, "Enable Debug logging");
                    ui.checkbox(&mut app.global_settings.do_not_report_feedback, "Do not report feedback");
                    ui.checkbox(&mut app.global_settings.darkness, "Dark Mode (Restart required.)");
                    ui.checkbox(&mut app.global_settings.check_chd_version, "Check CHD Version");
                }

                ui.separator();
                ui.horizontal(|ui| {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button("Cancel").clicked() {
                            close_settings = true;
                        }
                        if ui.button("OK").clicked() {
                            rv_core::settings::update_settings(app.global_settings.clone());
                            app.task_logs.push("Saved Global Settings".to_string());
                            close_settings = true;
                        }
                    });
                });
            });
    }
    if close_settings {
        app.show_settings = false;
    }
}
