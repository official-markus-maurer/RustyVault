impl RomVaultApp {
    pub fn draw_rom_grid(&mut self, ui: &mut egui::Ui) {
        let mut new_sort_col_rom = self.sort_col.clone();
        let mut new_sort_desc_rom = self.sort_desc;

        let empty_rows: &[RomGridRow] = &[];
        let (rom_rows, alt_found, show_status, show_file_mod_date, show_zip_index) = if let Some(
            selected_game,
        ) = &self.selected_game
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
                            "FileModDate" => a_ref
                                .file_mod_time_stamp
                                .cmp(&b_ref.file_mod_time_stamp),
                            "ZipIndex" => a_ref.local_header_offset.cmp(&b_ref.local_header_offset),
                            "InstanceCount" => std::cmp::Ordering::Equal,
                            _ => a.ui_name.cmp(&b.ui_name),
                        };
                        let cmp =
                            if cmp == std::cmp::Ordering::Equal && col.as_str() != "ROM (File)" {
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
                                sort_header_cell(
                                    ui,
                                    "Merge",
                                    &mut new_sort_col_rom,
                                    &mut new_sort_desc_rom,
                                )
                            });
                            header.col(|ui| {
                                sort_header_cell(
                                    ui,
                                    "Size",
                                    &mut new_sort_col_rom,
                                    &mut new_sort_desc_rom,
                                )
                            });
                            header.col(|ui| {
                                sort_header_cell(
                                    ui,
                                    "CRC32",
                                    &mut new_sort_col_rom,
                                    &mut new_sort_desc_rom,
                                )
                            });
                            header.col(|ui| {
                                sort_header_cell(
                                    ui,
                                    "SHA1",
                                    &mut new_sort_col_rom,
                                    &mut new_sort_desc_rom,
                                )
                            });
                            header.col(|ui| {
                                sort_header_cell(
                                    ui,
                                    "MD5",
                                    &mut new_sort_col_rom,
                                    &mut new_sort_desc_rom,
                                )
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
                                        if let Some(info) =
                                            rom_clipboard_text(&rom, RomGridCopyColumn::Got)
                                        {
                                            ui.output_mut(|o| o.copied_text = info);
                                            self.task_logs.push("Copied ROM info".to_string());
                                        }
                                    }
                                });
                                row.col(|ui| {
                                    ui.painter().rect_filled(ui.max_rect(), 0.0, row_color);
                                    let label_resp = ui.add(egui::SelectableLabel::new(
                                        false,
                                        &row_data.display_text,
                                    ));
                                    if label_resp.secondary_clicked() {
                                        if let Some(text) =
                                            rom_clipboard_text(&rom, RomGridCopyColumn::Rom)
                                        {
                                            ui.output_mut(|o| o.copied_text = text.clone());
                                            self.task_logs.push(format!("Copied: {}", text));
                                        }
                                    }
                                    label_resp.context_menu(|ui| {
                                        if ui.button("Copy ROM Name").clicked() {
                                            ui.output_mut(|o| o.copied_text = row_data.ui_name.clone());
                                            self.task_logs
                                                .push(format!("Copied: {}", row_data.ui_name));
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
                                    let resp =
                                        ui.add(egui::Label::new(text).sense(egui::Sense::click()));
                                    if resp.secondary_clicked() {
                                        if let Some(copy) =
                                            rom_clipboard_text(&rom, RomGridCopyColumn::Size)
                                        {
                                            ui.output_mut(|o| o.copied_text = copy.clone());
                                            self.task_logs.push(format!("Copied: {}", copy));
                                        }
                                    }
                                });
                                row.col(|ui| {
                                    ui.painter().rect_filled(ui.max_rect(), 0.0, row_color);
                                    let text = format_cell_with_source_flags(
                                        rom.crc.as_ref().map(hex::encode).unwrap_or_default(),
                                        &rom,
                                        rv_core::rv_file::FileStatus::CRC_FROM_DAT,
                                        rv_core::rv_file::FileStatus::CRC_FROM_HEADER,
                                    );
                                    let resp =
                                        ui.add(egui::Label::new(text).sense(egui::Sense::click()));
                                    if resp.secondary_clicked() {
                                        if let Some(copy) =
                                            rom_clipboard_text(&rom, RomGridCopyColumn::Crc32)
                                        {
                                            ui.output_mut(|o| o.copied_text = copy.clone());
                                            self.task_logs.push(format!("Copied: {}", copy));
                                        }
                                    }
                                });
                                row.col(|ui| {
                                    ui.painter().rect_filled(ui.max_rect(), 0.0, row_color);
                                    let text = format_cell_with_source_flags(
                                        rom.sha1.as_ref().map(hex::encode).unwrap_or_default(),
                                        &rom,
                                        rv_core::rv_file::FileStatus::SHA1_FROM_DAT,
                                        rv_core::rv_file::FileStatus::SHA1_FROM_HEADER,
                                    );
                                    let resp =
                                        ui.add(egui::Label::new(text).sense(egui::Sense::click()));
                                    if resp.secondary_clicked() {
                                        if let Some(copy) =
                                            rom_clipboard_text(&rom, RomGridCopyColumn::Sha1)
                                        {
                                            ui.output_mut(|o| o.copied_text = copy.clone());
                                            self.task_logs.push(format!("Copied: {}", copy));
                                        }
                                    }
                                });
                                row.col(|ui| {
                                    ui.painter().rect_filled(ui.max_rect(), 0.0, row_color);
                                    let text = format_cell_with_source_flags(
                                        rom.md5.as_ref().map(hex::encode).unwrap_or_default(),
                                        &rom,
                                        rv_core::rv_file::FileStatus::MD5_FROM_DAT,
                                        rv_core::rv_file::FileStatus::MD5_FROM_HEADER,
                                    );
                                    let resp =
                                        ui.add(egui::Label::new(text).sense(egui::Sense::click()));
                                    if resp.secondary_clicked() {
                                        if let Some(copy) =
                                            rom_clipboard_text(&rom, RomGridCopyColumn::Md5)
                                        {
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
                                        let resp = ui.add(
                                            egui::Label::new(text).sense(egui::Sense::click()),
                                        );
                                        if resp.secondary_clicked() {
                                            if let Some(copy) = rom_clipboard_text(
                                                &rom,
                                                RomGridCopyColumn::AltSize,
                                            ) {
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
                                        let resp = ui.add(
                                            egui::Label::new(text).sense(egui::Sense::click()),
                                        );
                                        if resp.secondary_clicked() {
                                            if let Some(copy) = rom_clipboard_text(
                                                &rom,
                                                RomGridCopyColumn::AltCrc32,
                                            ) {
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
                                        let resp = ui.add(
                                            egui::Label::new(text).sense(egui::Sense::click()),
                                        );
                                        if resp.secondary_clicked() {
                                            if let Some(copy) = rom_clipboard_text(
                                                &rom,
                                                RomGridCopyColumn::AltSha1,
                                            ) {
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
                                        let resp = ui.add(
                                            egui::Label::new(text).sense(egui::Sense::click()),
                                        );
                                        if resp.secondary_clicked() {
                                            if let Some(copy) = rom_clipboard_text(
                                                &rom,
                                                RomGridCopyColumn::AltMd5,
                                            ) {
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
                                        ui.label(
                                            row_data
                                                .zip_index
                                                .map(|v| v.to_string())
                                                .unwrap_or_default(),
                                        );
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
                                        self.rom_info_lines =
                                            collect_rom_occurrence_lines(Rc::clone(&rom_rc));
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
