use eframe::egui;

use crate::RomVaultApp;

#[derive(Clone, Copy)]
pub(crate) struct ColorKeyEntry {
    pub(crate) icon: &'static str,
    pub(crate) title: &'static str,
    pub(crate) description: &'static str,
}

#[derive(Clone, Copy)]
pub(crate) struct ColorKeySection {
    pub(crate) title: &'static str,
    pub(crate) entries: &'static [ColorKeyEntry],
}

const GAME_LIST_RESTING: &[ColorKeyEntry] = &[
    ColorKeyEntry {
        icon: "G_Correct.png",
        title: "Correct",
        description: "The ROM is correct.",
    },
    ColorKeyEntry {
        icon: "G_CorrectMIA.png",
        title: "CorrectMIA",
        description: "The ROM was known to be MIA, but you found it.",
    },
    ColorKeyEntry {
        icon: "G_Missing.png",
        title: "Missing",
        description: "The ROM is missing.",
    },
    ColorKeyEntry {
        icon: "G_MissingMIA.png",
        title: "MissingMIA",
        description: "The ROM is known to be private or missing in action.",
    },
    ColorKeyEntry {
        icon: "G_InToSort.png",
        title: "InToSort",
        description: "The ROM is in a ToSort directory.",
    },
    ColorKeyEntry {
        icon: "G_UnNeeded.png",
        title: "UnNeeded",
        description:
            "The ROM is not needed here because it belongs in the parent or primary deduped set.",
    },
    ColorKeyEntry {
        icon: "G_NotCollected.png",
        title: "NotCollected",
        description:
            "The ROM is not collected here because it belongs in the parent or primary deduped set or is a bad dump.",
    },
    ColorKeyEntry {
        icon: "G_Ignore.png",
        title: "Ignore",
        description: "The file matches an ignore rule.",
    },
];

const GAME_LIST_FIXING: &[ColorKeyEntry] = &[
    ColorKeyEntry {
        icon: "G_CanBeFixed.png",
        title: "CanBeFixed",
        description:
            "The ROM is missing here, but it is available elsewhere and will be fixed.",
    },
    ColorKeyEntry {
        icon: "G_CanBeFixedMIA.png",
        title: "CanBeFixedMIA",
        description:
            "The MIA ROM is missing here, but it is available elsewhere and will be fixed.",
    },
    ColorKeyEntry {
        icon: "G_CorruptCanBeFixed.png",
        title: "CorruptCanBeFixed",
        description: "The ROM is corrupt, but another copy can repair it.",
    },
    ColorKeyEntry {
        icon: "G_MoveToCorrupt.png",
        title: "MoveToCorrupt",
        description: "The file is corrupt and will be moved to the ToSort Corrupt folder.",
    },
    ColorKeyEntry {
        icon: "G_MoveToSort.png",
        title: "MoveToSort",
        description:
            "The ROM is not needed here, but no copy is located elsewhere, so it will be moved to the primary ToSort.",
    },
    ColorKeyEntry {
        icon: "G_Rename.png",
        title: "Rename",
        description: "The ROM is needed here, but has the incorrect name and will be renamed.",
    },
    ColorKeyEntry {
        icon: "G_Delete.png",
        title: "Delete",
        description:
            "The ROM is not needed here, and a copy exists elsewhere, so it will be deleted.",
    },
];

const GAME_LIST_PROBLEM: &[ColorKeyEntry] = &[
    ColorKeyEntry {
        icon: "G_Unknown.png",
        title: "Unknown",
        description: "The file could not be scanned.",
    },
    ColorKeyEntry {
        icon: "G_Corrupt.png",
        title: "Corrupt",
        description: "The file is corrupt.",
    },
    ColorKeyEntry {
        icon: "G_DirCorrupt.png",
        title: "DirCorrupt",
        description: "The archive containing the file is corrupt.",
    },
    ColorKeyEntry {
        icon: "G_Incomplete.png",
        title: "Incomplete",
        description: "The ROM is needed here, but fixing would result in an incomplete set.",
    },
];

const SET_TYPES_HAVE: &[ColorKeyEntry] = &[
    ColorKeyEntry {
        icon: "Dir.png",
        title: "Uncompressed Set",
        description: "The set is uncompressed.",
    },
    ColorKeyEntry {
        icon: "Zip.png",
        title: "Zip",
        description: "The set is a regular ZIP archive.",
    },
    ColorKeyEntry {
        icon: "ZipTrrnt.png",
        title: "TorrentZip",
        description: "The set is torrentzipped.",
    },
    ColorKeyEntry {
        icon: "ZipTDC.png",
        title: "TDC Zip",
        description: "The set is zipped with deflate compression and matches a TDC DAT.",
    },
    ColorKeyEntry {
        icon: "ZipZstd.png",
        title: "Zstd Zip",
        description: "The set is zipped with zstd compression.",
    },
    ColorKeyEntry {
        icon: "SevenZip.png",
        title: "7z",
        description: "The set is a regular 7zip archive.",
    },
    ColorKeyEntry {
        icon: "SevenZipTrrnt.png",
        title: "Torrent7z",
        description: "The set is torrent7zipped.",
    },
    ColorKeyEntry {
        icon: "SevenZipNLZMA.png",
        title: "7z LZMA Non-Solid",
        description: "The set is a non-solid 7zip archive with LZMA compression.",
    },
    ColorKeyEntry {
        icon: "SevenZipSLZMA.png",
        title: "7z LZMA Solid",
        description: "The set is a solid 7zip archive with LZMA compression.",
    },
    ColorKeyEntry {
        icon: "SevenZipNZSTD.png",
        title: "7z Zstd Non-Solid",
        description: "The set is a non-solid 7zip archive with zstd compression.",
    },
    ColorKeyEntry {
        icon: "SevenZipSZSTD.png",
        title: "7z Zstd Solid",
        description: "The set is a solid 7zip archive with zstd compression.",
    },
];

const SET_TYPES_MISSING: &[ColorKeyEntry] = &[
    ColorKeyEntry {
        icon: "ZipMissing.png",
        title: "Zip Missing",
        description: "The ZIP set is missing.",
    },
    ColorKeyEntry {
        icon: "ZipTrrntMissing.png",
        title: "TorrentZip Missing",
        description: "The torrentzipped set is missing.",
    },
    ColorKeyEntry {
        icon: "ZipTDCMissing.png",
        title: "TDC Zip Missing",
        description: "The TDC zipped set is missing.",
    },
    ColorKeyEntry {
        icon: "ZipZstdMissing.png",
        title: "Zstd Zip Missing",
        description: "The zstd zipped set is missing.",
    },
    ColorKeyEntry {
        icon: "SevenZipMissing.png",
        title: "7z Missing",
        description: "The regular 7z set is missing.",
    },
    ColorKeyEntry {
        icon: "SevenZipSLZMAMissing.png",
        title: "7z Solid LZMA Missing",
        description: "The solid LZMA 7z set is missing.",
    },
    ColorKeyEntry {
        icon: "SevenZipSZSTDMissing.png",
        title: "7z Solid Zstd Missing",
        description: "The solid zstd 7z set is missing.",
    },
];

const SET_TYPES_CORRUPT: &[ColorKeyEntry] = &[
    ColorKeyEntry {
        icon: "ZipCorrupt.png",
        title: "Zip Corrupt",
        description: "The ZIP set is corrupt.",
    },
    ColorKeyEntry {
        icon: "ZipTrrntCorrupt.png",
        title: "TorrentZip Corrupt",
        description: "The torrentzipped set is corrupt.",
    },
    ColorKeyEntry {
        icon: "SevenZipCorrupt.png",
        title: "7z Corrupt",
        description: "The 7z set is corrupt.",
    },
    ColorKeyEntry {
        icon: "SevenZipSLZMACorrupt.png",
        title: "7z Solid LZMA Corrupt",
        description: "The solid LZMA 7z set is corrupt.",
    },
    ColorKeyEntry {
        icon: "SevenZipSZSTDCorrupt.png",
        title: "7z Solid Zstd Corrupt",
        description: "The solid zstd 7z set is corrupt.",
    },
];

const ROM_DETAILS_RESTING: &[ColorKeyEntry] = &[
    ColorKeyEntry {
        icon: "R_InDatCollect_Correct.png",
        title: "Correct",
        description: "The ROM is correct.",
    },
    ColorKeyEntry {
        icon: "R_InDatMIA_CorrectMIA.png",
        title: "CorrectMIA",
        description: "The ROM is known to be MIA but you have it.",
    },
    ColorKeyEntry {
        icon: "R_InDatCollect_Missing.png",
        title: "Missing",
        description: "The ROM is missing.",
    },
    ColorKeyEntry {
        icon: "R_InDatMIA_MissingMIA.png",
        title: "MissingMIA",
        description: "The ROM is known to be MIA.",
    },
    ColorKeyEntry {
        icon: "R_InDatMerged_UnNeeded.png",
        title: "UnNeeded",
        description:
            "The ROM is present but unneeded because it is wanted in a parent or primary deduped set.",
    },
    ColorKeyEntry {
        icon: "R_InDatMerged_NotCollected.png",
        title: "NotCollected",
        description:
            "The ROM is uncollected because it is wanted in a parent or primary deduped set.",
    },
    ColorKeyEntry {
        icon: "R_InToSort_InToSort.png",
        title: "InToSort",
        description: "The ROM is in a ToSort directory.",
    },
    ColorKeyEntry {
        icon: "R_NotInDat_Ignore.png",
        title: "Ignore",
        description: "The file matches an ignore rule.",
    },
    ColorKeyEntry {
        icon: "R_InDatCollect_UnScanned.png",
        title: "UnScanned",
        description: "The file is unscanned.",
    },
    ColorKeyEntry {
        icon: "R_InDatCollect_Corrupt.png",
        title: "Corrupt",
        description: "The file is corrupt.",
    },
];

const ROM_DETAILS_FIXING: &[ColorKeyEntry] = &[
    ColorKeyEntry {
        icon: "R_InDatCollect_CanBeFixed.png",
        title: "CanBeFixed",
        description: "The ROM can be fixed.",
    },
    ColorKeyEntry {
        icon: "R_InDatMIA_CanBeFixedMIA.png",
        title: "CanBeFixedMIA",
        description: "The MIA ROM can be fixed.",
    },
    ColorKeyEntry {
        icon: "R_InDatCollect_CorruptCanBeFixed.png",
        title: "CorruptCanBeFixed",
        description: "The ROM is corrupt, but can be fixed with another copy.",
    },
    ColorKeyEntry {
        icon: "R_NotInDat_Rename.png",
        title: "Rename",
        description: "The ROM will be renamed.",
    },
    ColorKeyEntry {
        icon: "R_InDatCollect_NeededForFix.png",
        title: "NeededForFix",
        description: "The ROM is needed to fix another file.",
    },
    ColorKeyEntry {
        icon: "R_NotInDat_MoveToSort.png",
        title: "MoveToSort",
        description: "The ROM will be moved to ToSort.",
    },
    ColorKeyEntry {
        icon: "R_NotInDat_Delete.png",
        title: "Delete",
        description: "The ROM will be deleted.",
    },
    ColorKeyEntry {
        icon: "R_InDatMerged_Delete.png",
        title: "Merged Delete",
        description: "The merged ROM will be deleted.",
    },
    ColorKeyEntry {
        icon: "R_InToSort_Delete.png",
        title: "ToSort Delete",
        description: "The ToSort ROM will be deleted.",
    },
    ColorKeyEntry {
        icon: "R_InDatCollect_MoveToCorrupt.png",
        title: "MoveToCorrupt",
        description: "The ROM will be moved to the corrupt folder.",
    },
];

const DAT_TREE_BRANCHES: &[ColorKeyEntry] = &[
    ColorKeyEntry {
        icon: "ExpandBoxPlus.png",
        title: "Collapsed Branch",
        description: "Click to expand.",
    },
    ColorKeyEntry {
        icon: "ExpandBoxMinus.png",
        title: "Expanded Branch",
        description: "Click to collapse.",
    },
    ColorKeyEntry {
        icon: "TickBoxUnTicked.png",
        title: "Unchecked",
        description: "The branch is excluded from scanning and fixing.",
    },
    ColorKeyEntry {
        icon: "TickBoxTicked.png",
        title: "Checked",
        description: "The branch is included in scanning and fixing.",
    },
    ColorKeyEntry {
        icon: "TickBoxLocked.png",
        title: "Locked",
        description: "The branch is included and will not be modified during fixing.",
    },
];

const DAT_TREE_FOLDERS: &[ColorKeyEntry] = &[
    ColorKeyEntry {
        icon: "DirectoryTree1.png",
        title: "Folder Missing",
        description: "All ROMs are missing.",
    },
    ColorKeyEntry {
        icon: "DirectoryTree2.png",
        title: "Folder Partial",
        description: "Some ROMs are missing.",
    },
    ColorKeyEntry {
        icon: "DirectoryTree3.png",
        title: "Folder Correct",
        description: "No ROMs are missing.",
    },
    ColorKeyEntry {
        icon: "DirectoryTree4.png",
        title: "Folder Merged/Unknown",
        description: "The branch is merged, ignored, or unknown.",
    },
    ColorKeyEntry {
        icon: "DirectoryTree5.png",
        title: "Folder ToSort",
        description: "The ToSort directory contains files present for fixes.",
    },
];

const DAT_TREE_DATS: &[ColorKeyEntry] = &[
    ColorKeyEntry {
        icon: "Tree3.png",
        title: "DAT Correct",
        description: "No ROMs are missing.",
    },
    ColorKeyEntry {
        icon: "Tree2.png",
        title: "DAT Partial",
        description: "Some ROMs are missing.",
    },
    ColorKeyEntry {
        icon: "Tree1.png",
        title: "DAT Missing",
        description: "All ROMs are missing.",
    },
    ColorKeyEntry {
        icon: "Tree4.png",
        title: "DAT Merged/Unknown",
        description: "The DAT is merged, ignored, or unknown.",
    },
    ColorKeyEntry {
        icon: "Tree5.png",
        title: "DAT ToSort",
        description: "The DAT branch contains only fixable ToSort work.",
    },
];

pub(crate) fn color_key_sections() -> &'static [ColorKeySection] {
    &[
        ColorKeySection {
            title: "Game List Grid - Resting",
            entries: GAME_LIST_RESTING,
        },
        ColorKeySection {
            title: "Game List Grid - Fixing",
            entries: GAME_LIST_FIXING,
        },
        ColorKeySection {
            title: "Game List Grid - Problem",
            entries: GAME_LIST_PROBLEM,
        },
        ColorKeySection {
            title: "Set Types - Have",
            entries: SET_TYPES_HAVE,
        },
        ColorKeySection {
            title: "Set Types - Missing",
            entries: SET_TYPES_MISSING,
        },
        ColorKeySection {
            title: "Set Types - Corrupt",
            entries: SET_TYPES_CORRUPT,
        },
        ColorKeySection {
            title: "ROM Details Grid - Resting Statuses",
            entries: ROM_DETAILS_RESTING,
        },
        ColorKeySection {
            title: "ROM Details Grid - Fix Actions",
            entries: ROM_DETAILS_FIXING,
        },
        ColorKeySection {
            title: "DAT Tree - Branches and Checkboxes",
            entries: DAT_TREE_BRANCHES,
        },
        ColorKeySection {
            title: "DAT Tree - Folders",
            entries: DAT_TREE_FOLDERS,
        },
        ColorKeySection {
            title: "DAT Tree - DATs",
            entries: DAT_TREE_DATS,
        },
    ]
}

pub(crate) fn color_key_entry_count() -> usize {
    color_key_sections()
        .iter()
        .map(|section| section.entries.len())
        .sum()
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
        "R_InDatCollect_CorruptCanBeFixed.png" => {
            include_asset!("R_InDatCollect_CorruptCanBeFixed.png")
        }
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

pub(super) fn draw_color_key_dialog(app: &mut RomVaultApp, ctx: &egui::Context) {
    if !app.show_color_key {
        return;
    }

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
            egui::ScrollArea::vertical()
                .auto_shrink([false, false])
                .show(ui, |ui| {
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
