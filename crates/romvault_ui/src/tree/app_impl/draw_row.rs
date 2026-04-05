impl RomVaultApp {
    pub fn draw_tree_row(&mut self, ui: &mut egui::Ui, row: &TreeRow) {
        let node_rc = Rc::clone(&row.node_rc);
        let depth = row.depth;

        let (
            is_file,
            is_directory,
            is_game,
            tree_checked,
            tree_expanded,
            has_multiple_dats,
            has_child_dirs,
            is_in_to_sort,
        ) = {
            let node = node_rc.borrow();
            (
                node.is_file(),
                node.is_directory(),
                node.game.is_some(),
                node.tree_checked,
                node.tree_expanded,
                node.is_directory() && node.dat.is_none() && node.dir_dats.len() > 1,
                node.children.iter().any(|c| {
                    let cb = c.borrow();
                    !cb.is_file() && cb.game.is_none()
                }),
                node.dat_status() == DatStatus::InToSort,
            )
        };
        if is_file || is_game {
            return;
        }
        let has_expandable_children = has_child_dirs || has_multiple_dats;

        let row_height = 18.0;
        let row_rect = ui.allocate_exact_size(
            egui::vec2(ui.available_width(), row_height),
            egui::Sense::click(),
        );

        let is_selected_for_scroll = self
            .selected_node
            .as_ref()
            .is_some_and(|n| Rc::ptr_eq(n, &node_rc));
        if is_selected_for_scroll && self.pending_tree_scroll_to_selected {
            ui.scroll_to_rect(row_rect.0, Some(egui::Align::Center));
            self.pending_tree_scroll_to_selected = false;
        }

        let color;
        let icon_idx;
        let img_src;
        let cached_stats;
        let mut ui_display_name;

        let should_enqueue_stats = {
            let node = node_rc.borrow();
            is_directory && node.cached_stats.is_none()
        };
        if should_enqueue_stats {
            if is_selected_for_scroll {
                self.enqueue_tree_stats_priority(Rc::clone(&node_rc));
            } else {
                self.enqueue_tree_stats(Rc::clone(&node_rc));
            }
        }

        {
            let mut node = node_rc.borrow_mut();

            cached_stats = node.cached_stats;
            ui_display_name = node.ui_display_name.clone();

            color = if let Some(stats) = &cached_stats {
                tree_color_from_stats(stats)
            } else {
                tree_color_from_rep_status(node.rep_status(), node.dat_status())
            };

            icon_idx = if let Some(stats) = &cached_stats {
                tree_icon_idx_from_stats(stats)
            } else if let Some(ds) = &node.dir_status {
                tree_icon_idx_from_report_status(*ds)
            } else {
                2
            };

            img_src = if node.dat.is_none() && node.dir_dats.is_empty() {
                match icon_idx {
                    1 => include_asset!("DirectoryTree1.png"),
                    2 => include_asset!("DirectoryTree2.png"),
                    3 => include_asset!("DirectoryTree3.png"),
                    4 => include_asset!("DirectoryTree4.png"),
                    5 => include_asset!("DirectoryTree5.png"),
                    _ => include_asset!("DirectoryTree3.png"),
                }
            } else {
                match icon_idx {
                    1 => include_asset!("Tree1.png"),
                    2 => include_asset!("Tree2.png"),
                    3 => include_asset!("Tree3.png"),
                    4 => include_asset!("Tree4.png"),
                    5 => include_asset!("Tree5.png"),
                    _ => include_asset!("Tree3.png"),
                }
            };

            if is_directory && ui_display_name.is_empty() {
                let icon = match node.file_type {
                    FileType::Dir => "📁",
                    FileType::Zip | FileType::SevenZip => "🗄",
                    _ => "📄",
                };
                let mut name = format!("{} {}", icon, node.name);

                if is_in_to_sort {
                    let to_sort_is_primary =
                        node.to_sort_status_is(rv_core::enums::ToSortDirType::TO_SORT_PRIMARY);
                    let to_sort_is_cache =
                        node.to_sort_status_is(rv_core::enums::ToSortDirType::TO_SORT_CACHE);

                    if to_sort_is_primary && to_sort_is_cache {
                        name = format!("{} (Primary, Cache)", name);
                    } else if to_sort_is_primary {
                        name = format!("{} (Primary)", name);
                    } else if to_sort_is_cache {
                        name = format!("{} (Cache)", name);
                    } else if node.to_sort_status_is(rv_core::enums::ToSortDirType::TO_SORT_FILE_ONLY)
                    {
                        name = format!("{} (File Only)", name);
                    }

                    if let Some(stats) = cached_stats {
                        name = format!(
                            "{} (Files: {})",
                            name,
                            crate::format_number(stats.total_roms)
                        );
                    } else {
                        name = format!("{} (Files: 0)", name);
                    }
                } else if let Some(stats) = cached_stats {
                    if node.dat.is_none() && node.dir_dats.len() == 1 {
                        let desc = node.dir_dats[0]
                            .borrow()
                            .get_data(rv_core::rv_dat::DatData::Description)
                            .unwrap_or_default();
                        if !desc.is_empty() {
                            name = format!("{}: {}", name, desc);
                        }
                    } else if let Some(dat) = &node.dat {
                        if dat
                            .borrow()
                            .dat_flags
                            .contains(rv_core::rv_dat::DatFlags::AUTO_ADDED_DIRECTORY)
                        {
                            name = format!("{}: ", name);
                        }
                    }

                    if !is_in_to_sort {
                        let mut parts = Vec::new();
                        if stats.total_roms > 0 {
                            parts.push(format!(
                                "Have: {}",
                                crate::format_number(correct_plain(&stats))
                            ));
                            if stats.roms_correct_mia > 0 {
                                parts.push(format!(
                                    "Found MIA: {}",
                                    crate::format_number(stats.roms_correct_mia)
                                ));
                            }
                            parts.push(format!(
                                "Missing: {}",
                                crate::format_number(missing_plain(&stats))
                            ));
                            if stats.roms_missing_mia > 0 {
                                parts.push(format!(
                                    "MIA: {}",
                                    crate::format_number(stats.roms_missing_mia)
                                ));
                            }
                            if stats.roms_fixes > 0 {
                                parts.push(format!("Fixes: {}", crate::format_number(stats.roms_fixes)));
                            }
                            if stats.roms_not_collected > 0 {
                                parts.push(format!(
                                    "NotCollected: {}",
                                    crate::format_number(stats.roms_not_collected)
                                ));
                            }
                            if stats.roms_unknown > 0 {
                                parts.push(format!("Unknown: {}", crate::format_number(stats.roms_unknown)));
                            }
                            if stats.roms_unneeded > 0 {
                                parts.push(format!(
                                    "UnNeeded: {}",
                                    crate::format_number(stats.roms_unneeded)
                                ));
                            }
                        }

                        if !parts.is_empty() {
                            name = format!("{} ( {} )", name, parts.join(" \\ "));
                        } else {
                            name = format!("{} ( Have: 0 \\ Missing: 0 )", name);
                        }
                    }
                }

                node.ui_display_name = name.clone();
                ui_display_name = name;
            }
        }

        let mut toggle_expanded = false;
        let mut expand_descendants = None;
        let mut clicked_label = false;

        let mut ui_builder = ui.child_ui(row_rect.0, *ui.layout());
        let is_selected = self
            .selected_node
            .as_ref()
            .is_some_and(|n| Rc::ptr_eq(n, &node_rc));
        if is_selected {
            let bg_color = ui_builder.visuals().selection.bg_fill;
            ui_builder.painter().rect_filled(row_rect.0, 0.0, bg_color);
        }

        ui_builder.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 2.0;
            ui.add_space(18.0 * depth as f32);

            if has_expandable_children {
                let expand_resp = if let Some(img) =
                    crate::assets::themed_image_source_optional(if tree_expanded {
                        "ExpandBoxMinus.png"
                    } else {
                        "ExpandBoxPlus.png"
                    })
                {
                    ui.add_sized([9.0, 9.0], egui::ImageButton::new(img).frame(false))
                } else {
                    ui.add_sized(
                        [9.0, 9.0],
                        egui::Button::new(if tree_expanded { "▾" } else { "▸" }).frame(false),
                    )
                };
                if expand_resp.clicked() {
                    toggle_expanded = true;
                } else if expand_resp.secondary_clicked() {
                    expand_descendants = Self::expand_descendants_target(&node_rc);
                }
            } else {
                ui.add_space(9.0);
            }

            let checkbox_img = match tree_checked {
                TreeSelect::Selected => include_asset!("TickBoxTicked.png"),
                TreeSelect::UnSelected => include_asset!("TickBoxUnTicked.png"),
                TreeSelect::Locked => include_asset!("TickBoxLocked.png"),
            };
            let checkbox_resp = ui
                .add_enabled_ui(!self.ui_working(), |ui| {
                    ui.add_sized(
                        [13.0, 13.0],
                        egui::ImageButton::new(checkbox_img).frame(false),
                    )
                })
                .inner;
            if checkbox_resp.clicked() {
                let is_shift = ui.input(|i| i.modifiers.shift);
                let new_state = match tree_checked {
                    TreeSelect::Selected => TreeSelect::UnSelected,
                    _ => TreeSelect::Selected,
                };

                let mut stack = vec![Rc::clone(&node_rc)];
                while let Some(current) = stack.pop() {
                    let mut n = current.borrow_mut();
                    n.tree_checked = new_state;
                    let children = n.children.clone();
                    drop(n);
                    if !is_shift {
                        for child in children {
                            stack.push(Rc::clone(&child));
                        }
                    }
                }
                if !self.ui_working() {
                    self.db_cache_dirty = true;
                }
            } else if checkbox_resp.secondary_clicked() {
                let is_shift = ui.input(|i| i.modifiers.shift);
                Self::set_tree_checked_locked(&node_rc, !is_shift);
                if !self.ui_working() {
                    self.db_cache_dirty = true;
                }
            }

            ui.add_sized(
                [16.0, row_height],
                egui::Image::new(img_src).max_width(16.0),
            );

            let clean_name = ui_display_name
                .trim_start_matches(|c: char| !c.is_alphanumeric() && c != '(' && c != '[')
                .trim();
            let label_color = if is_selected {
                ui.visuals().selection.stroke.color
            } else {
                color
            };
            let label_text = egui::RichText::new(clean_name).color(label_color);
            let label_resp = ui.add(egui::Label::new(label_text).sense(egui::Sense::click()));

            if label_resp.clicked() {
                clicked_label = true;
            }
            if label_resp.secondary_clicked() {
                self.select_node(Rc::clone(&node_rc));
            }

            enum TreeAction {
                Quick,
                Normal,
                Full,
            }
            let mut pending_action = None;
            let mut pending_action_logical: Option<String> = None;

            label_resp.context_menu(|ui| {
                if ui.button("Scan").clicked() {
                    pending_action = Some(TreeAction::Normal);
                    pending_action_logical = Some(node_rc.borrow().get_logical_name());
                    ui.close_menu();
                }
                if ui.button("Scan Quick (Headers Only)").clicked() {
                    pending_action = Some(TreeAction::Quick);
                    pending_action_logical = Some(node_rc.borrow().get_logical_name());
                    ui.close_menu();
                }
                if ui.button("Scan Full (Complete Re-Scan)").clicked() {
                    pending_action = Some(TreeAction::Full);
                    pending_action_logical = Some(node_rc.borrow().get_logical_name());
                    ui.close_menu();
                }

                if is_in_to_sort {
                    let np_clone = node_rc.borrow().get_logical_name();
                    let open_tosort =
                        rv_core::settings::find_dir_mapping(&np_clone).unwrap_or_else(|| np_clone.clone());
                    let can_open_tosort = std::path::Path::new(&open_tosort).is_dir();
                    if ui
                        .add_enabled(can_open_tosort, egui::Button::new("Open ToSort Directory"))
                        .clicked()
                    {
                        self.task_logs
                            .push(format!("Opening ToSort Directory: {}", open_tosort));
                        let _ = std::process::Command::new("cmd")
                            .args(["/C", "start", "", &open_tosort])
                            .spawn();
                        ui.close_menu();
                    }
                    ui.separator();

                    let mut can_move_up = false;
                    let mut can_move_down = false;
                    GLOBAL_DB.with(|db_ref| {
                        if let Some(db) = db_ref.borrow().as_ref() {
                            let dir_root = db.dir_root.borrow();
                            let mut idx = None;
                            for (i, child) in dir_root.children.iter().enumerate() {
                                if Rc::ptr_eq(child, &node_rc) {
                                    idx = Some(i);
                                    break;
                                }
                            }
                            if let Some(i) = idx {
                                can_move_up = i >= 2;
                                if !dir_root.children.is_empty() {
                                    can_move_down = i <= dir_root.children.len().saturating_sub(2);
                                }
                            }
                        }
                    });
                    let (show_set_file_only, show_clear_file_only) = {
                        let n = node_rc.borrow();
                        let is_primary =
                            n.to_sort_status_is(rv_core::enums::ToSortDirType::TO_SORT_PRIMARY);
                        let is_cache =
                            n.to_sort_status_is(rv_core::enums::ToSortDirType::TO_SORT_CACHE);
                        let is_file_only =
                            n.to_sort_status_is(rv_core::enums::ToSortDirType::TO_SORT_FILE_ONLY);
                        (!(is_file_only || is_primary || is_cache), is_file_only)
                    };

                    if ui
                        .add_enabled(!self.ui_working() && can_move_up, egui::Button::new("Move Up"))
                        .clicked()
                    {
                        self.task_logs
                            .push(format!("Move ToSort Up: {}", node_rc.borrow().name));
                        GLOBAL_DB.with(|db_ref| {
                            if let Some(db) = db_ref.borrow().as_ref() {
                                let mut dir_root = db.dir_root.borrow_mut();
                                let mut idx = None;
                                for (i, child) in dir_root.children.iter().enumerate() {
                                    if Rc::ptr_eq(child, &node_rc) {
                                        idx = Some(i);
                                        break;
                                    }
                                }
                                if let Some(i) = idx {
                                    if i > 1 {
                                        dir_root.children.swap(i, i - 1);
                                    }
                                }
                            }
                        });
                        if !self.ui_working() {
                            self.db_cache_dirty = true;
                        }
                        self.tree_rows_dirty = true;
                        ui.close_menu();
                    }
                    if ui
                        .add_enabled(
                            !self.ui_working() && can_move_down,
                            egui::Button::new("Move Down"),
                        )
                        .clicked()
                    {
                        self.task_logs
                            .push(format!("Move ToSort Down: {}", node_rc.borrow().name));
                        GLOBAL_DB.with(|db_ref| {
                            if let Some(db) = db_ref.borrow().as_ref() {
                                let mut dir_root = db.dir_root.borrow_mut();
                                let mut idx = None;
                                for (i, child) in dir_root.children.iter().enumerate() {
                                    if Rc::ptr_eq(child, &node_rc) {
                                        idx = Some(i);
                                        break;
                                    }
                                }
                                if let Some(i) = idx {
                                    if i < dir_root.children.len().saturating_sub(1) {
                                        dir_root.children.swap(i, i + 1);
                                    }
                                }
                            }
                        });
                        if !self.ui_working() {
                            self.db_cache_dirty = true;
                        }
                        self.tree_rows_dirty = true;
                        ui.close_menu();
                    }
                    if ui
                        .add_enabled(!self.ui_working(), egui::Button::new("Set To Primary ToSort"))
                        .clicked()
                    {
                        self.task_logs
                            .push(format!("Set To Primary ToSort: {}", node_rc.borrow().name));
                        GLOBAL_DB.with(|db_ref| {
                            if let Some(db) = db_ref.borrow().as_ref() {
                                let mut clicked = node_rc.borrow_mut();
                                if clicked.tree_checked == TreeSelect::Locked {
                                    clicked.tree_checked = TreeSelect::Selected;
                                }
                                clicked.ui_display_name.clear();
                                drop(clicked);

                                let root = db.dir_root.borrow();
                                let mut old_primary: Option<Rc<RefCell<RvFile>>> = None;
                                for child in root.children.iter().skip(1) {
                                    if child
                                        .borrow()
                                        .to_sort_status_is(rv_core::enums::ToSortDirType::TO_SORT_PRIMARY)
                                    {
                                        old_primary = Some(Rc::clone(child));
                                        break;
                                    }
                                }
                                drop(root);

                                let was_cache = old_primary
                                    .as_ref()
                                    .map(|n| {
                                        n.borrow()
                                            .to_sort_status_is(rv_core::enums::ToSortDirType::TO_SORT_CACHE)
                                    })
                                    .unwrap_or(false);
                                if let Some(op) = old_primary {
                                    let mut opm = op.borrow_mut();
                                    opm.to_sort_status_clear(
                                        rv_core::enums::ToSortDirType::TO_SORT_PRIMARY
                                            | rv_core::enums::ToSortDirType::TO_SORT_CACHE,
                                    );
                                    opm.ui_display_name.clear();
                                }

                                let mut clicked = node_rc.borrow_mut();
                                clicked.to_sort_status_set(rv_core::enums::ToSortDirType::TO_SORT_PRIMARY);
                                if was_cache {
                                    clicked.to_sort_status_set(rv_core::enums::ToSortDirType::TO_SORT_CACHE);
                                }
                            }
                        });
                        if !self.ui_working() {
                            self.db_cache_dirty = true;
                        }
                        self.tree_rows_dirty = true;
                        ui.close_menu();
                    }
                    if ui
                        .add_enabled(!self.ui_working(), egui::Button::new("Set To Cache ToSort"))
                        .clicked()
                    {
                        self.task_logs
                            .push(format!("Set To Cache ToSort: {}", node_rc.borrow().name));
                        GLOBAL_DB.with(|db_ref| {
                            if let Some(db) = db_ref.borrow().as_ref() {
                                let mut clicked = node_rc.borrow_mut();
                                if clicked.tree_checked == TreeSelect::Locked {
                                    clicked.tree_checked = TreeSelect::Selected;
                                }
                                clicked.ui_display_name.clear();
                                drop(clicked);

                                let root = db.dir_root.borrow();
                                let mut old_cache: Option<Rc<RefCell<RvFile>>> = None;
                                for child in root.children.iter().skip(1) {
                                    if child
                                        .borrow()
                                        .to_sort_status_is(rv_core::enums::ToSortDirType::TO_SORT_CACHE)
                                    {
                                        old_cache = Some(Rc::clone(child));
                                        break;
                                    }
                                }
                                drop(root);

                                if let Some(oc) = old_cache {
                                    let mut ocm = oc.borrow_mut();
                                    ocm.to_sort_status_clear(rv_core::enums::ToSortDirType::TO_SORT_CACHE);
                                    ocm.ui_display_name.clear();
                                }
                                node_rc.borrow_mut()
                                    .to_sort_status_set(rv_core::enums::ToSortDirType::TO_SORT_CACHE);
                            }
                        });
                        if !self.ui_working() {
                            self.db_cache_dirty = true;
                        }
                        self.tree_rows_dirty = true;
                        ui.close_menu();
                    }
                    if show_set_file_only
                        && ui
                            .add_enabled(!self.ui_working(), egui::Button::new("Set To File Only ToSort"))
                            .clicked()
                    {
                        self.task_logs.push(format!(
                            "Set To File Only ToSort: {}",
                            node_rc.borrow().name
                        ));
                        if node_rc.borrow().tree_checked == TreeSelect::Locked {
                            node_rc.borrow_mut().tree_checked = TreeSelect::Selected;
                        }
                        node_rc.borrow_mut().ui_display_name.clear();
                        node_rc.borrow_mut()
                            .to_sort_status_set(rv_core::enums::ToSortDirType::TO_SORT_FILE_ONLY);
                        if !self.ui_working() {
                            self.db_cache_dirty = true;
                        }
                        self.tree_rows_dirty = true;
                        ui.close_menu();
                    }
                    if show_clear_file_only
                        && ui
                            .add_enabled(!self.ui_working(), egui::Button::new("Clear File Only ToSort"))
                            .clicked()
                    {
                        self.task_logs
                            .push(format!("Clear File Only ToSort: {}", node_rc.borrow().name));
                        node_rc.borrow_mut().ui_display_name.clear();
                        node_rc.borrow_mut()
                            .to_sort_status_clear(rv_core::enums::ToSortDirType::TO_SORT_FILE_ONLY);
                        if !self.ui_working() {
                            self.db_cache_dirty = true;
                        }
                        self.tree_rows_dirty = true;
                        ui.close_menu();
                    }
                    ui.separator();
                    if ui
                        .add_enabled(!self.ui_working(), egui::Button::new("Remove"))
                        .clicked()
                    {
                        self.task_logs.push(format!(
                            "Remove ToSort Directory: {}",
                            node_rc.borrow().name
                        ));
                        let mut select_after_remove: Option<Rc<RefCell<RvFile>>> = None;
                        GLOBAL_DB.with(|db_ref| {
                            if let Some(db) = db_ref.borrow().as_ref() {
                                let mut dir_root = db.dir_root.borrow_mut();
                                let mut idx_to_remove = None;
                                for (i, child) in dir_root.children.iter().enumerate() {
                                    if Rc::ptr_eq(child, &node_rc) {
                                        idx_to_remove = Some(i);
                                        break;
                                    }
                                }
                                if let Some(idx) = idx_to_remove {
                                    if idx > 0 && idx - 1 < dir_root.children.len() {
                                        select_after_remove =
                                            Some(Rc::clone(&dir_root.children[idx - 1]));
                                    }
                                    dir_root.child_remove(idx);
                                }
                                drop(dir_root);

                                rv_core::repair_status::RepairStatus::report_status_reset(Rc::clone(&db.dir_root));
                            }
                        });
                        if let Some(selected) = &self.selected_node {
                            if Rc::ptr_eq(selected, &node_rc) {
                                self.selected_node = None;
                            }
                        }
                        if let Some(new_sel) = select_after_remove {
                            self.select_node(new_sel);
                        }
                        if !self.ui_working() {
                            self.db_cache_dirty = true;
                        }
                        self.tree_rows_dirty = true;
                        ui.close_menu();
                    }
                    ui.separator();
                }

                if ui.button("Set Dir Dat Settings").clicked() {
                    let node_path = node_rc.borrow().get_logical_name();
                    self.active_dat_rule = rv_core::settings::find_rule(&node_path);
                    self.dir_settings_tab = 0;
                    self.dir_settings_compact = false;
                    self.show_dir_settings = true;
                    ui.close_menu();
                }
                if ui.button("Set Dir Mappings").clicked() {
                    self.open_dir_mappings();
                    ui.close_menu();
                }
                ui.separator();
                let node_path = node_rc.borrow().get_logical_name();
                let open_path =
                    rv_core::settings::find_dir_mapping(&node_path).unwrap_or_else(|| node_path.clone());
                let can_open_dir = std::path::Path::new(&open_path).exists();
                if ui
                    .add_enabled(can_open_dir, egui::Button::new("Open Directory"))
                    .clicked()
                {
                    self.task_logs.push(format!("Opening Directory: {}", open_path));
                    let _ = std::process::Command::new("cmd")
                        .args(["/C", "start", "", &open_path])
                        .spawn();
                    ui.close_menu();
                }
                ui.separator();
                if ui
                    .add_enabled(!self.ui_working(), egui::Button::new("Save fix DATs"))
                    .clicked()
                {
                    self.prompt_fixdat_report_for_node(true, Rc::clone(&node_rc));
                    ui.close_menu();
                }
                if ui
                    .add_enabled(!self.ui_working(), egui::Button::new("Save full DAT"))
                    .clicked()
                {
                    self.prompt_make_dat(Rc::clone(&node_rc));
                    ui.close_menu();
                }
            });

            if let Some(action) = pending_action {
                let logical =
                    pending_action_logical.unwrap_or_else(|| node_rc.borrow().get_logical_name());
                match action {
                    TreeAction::Quick => {
                        let target_key =
                            crate::normalize_full_name_key(&node_rc.borrow().get_full_name());
                        let np =
                            rv_core::settings::find_dir_mapping(&logical).unwrap_or(logical.clone());
                        self.launch_task("Scan ROMs (Quick)", move |tx| {
                            let _ = tx.send(format!("Scanning {} (Headers Only)...", logical));
                            let rule = rv_core::settings::find_rule(&logical);
                            let files = Scanner::scan_directory_with_level_and_ignore(
                                &np,
                                rv_core::settings::EScanLevel::Level1,
                                &rule.ignore_files.items,
                            );
                            let mut root_scan = rv_core::scanned_file::ScannedFile::new(FileType::Dir);
                            root_scan.children = files;
                            let _ = tx.send("Integrating files into DB...".to_string());
                            crate::GLOBAL_DB.with(|db_ref| {
                                if let Some(db) = db_ref.borrow().as_ref() {
                                    if let Some(target_rc) =
                                        crate::find_node_by_full_name_key(&db.dir_root, &target_key)
                                    {
                                        FileScanning::scan_dir_with_level(
                                            target_rc,
                                            &mut root_scan,
                                            rv_core::settings::EScanLevel::Level1,
                                        );
                                    } else {
                                        let _ = tx.send("Scan target no longer exists in DB.".to_string());
                                    }
                                }
                            });
                        });
                    }
                    TreeAction::Normal => {
                        let target_key =
                            crate::normalize_full_name_key(&node_rc.borrow().get_full_name());
                        let np =
                            rv_core::settings::find_dir_mapping(&logical).unwrap_or(logical.clone());
                        self.launch_task("Scan ROMs", move |tx| {
                            let _ = tx.send(format!("Scanning {}...", logical));
                            let rule = rv_core::settings::find_rule(&logical);
                            let files = Scanner::scan_directory_with_level_and_ignore(
                                &np,
                                rv_core::settings::EScanLevel::Level2,
                                &rule.ignore_files.items,
                            );
                            let mut root_scan = rv_core::scanned_file::ScannedFile::new(FileType::Dir);
                            root_scan.children = files;
                            let _ = tx.send("Integrating files into DB...".to_string());
                            crate::GLOBAL_DB.with(|db_ref| {
                                if let Some(db) = db_ref.borrow().as_ref() {
                                    if let Some(target_rc) =
                                        crate::find_node_by_full_name_key(&db.dir_root, &target_key)
                                    {
                                        FileScanning::scan_dir_with_level(
                                            target_rc,
                                            &mut root_scan,
                                            rv_core::settings::EScanLevel::Level2,
                                        );
                                    } else {
                                        let _ = tx.send("Scan target no longer exists in DB.".to_string());
                                    }
                                }
                            });
                        });
                    }
                    TreeAction::Full => {
                        let target_key =
                            crate::normalize_full_name_key(&node_rc.borrow().get_full_name());
                        let np =
                            rv_core::settings::find_dir_mapping(&logical).unwrap_or(logical.clone());
                        self.launch_task("Scan ROMs (Full)", move |tx| {
                            let _ = tx.send(format!("Scanning {} (Full Re-Scan)...", logical));
                            let rule = rv_core::settings::find_rule(&logical);
                            let files = Scanner::scan_directory_with_level_and_ignore(
                                &np,
                                rv_core::settings::EScanLevel::Level3,
                                &rule.ignore_files.items,
                            );
                            let mut root_scan = rv_core::scanned_file::ScannedFile::new(FileType::Dir);
                            root_scan.children = files;
                            let _ = tx.send("Integrating files into DB...".to_string());
                            crate::GLOBAL_DB.with(|db_ref| {
                                if let Some(db) = db_ref.borrow().as_ref() {
                                    if let Some(target_rc) =
                                        crate::find_node_by_full_name_key(&db.dir_root, &target_key)
                                    {
                                        FileScanning::scan_dir_with_level(
                                            target_rc,
                                            &mut root_scan,
                                            rv_core::settings::EScanLevel::Level3,
                                        );
                                    } else {
                                        let _ = tx.send("Scan target no longer exists in DB.".to_string());
                                    }
                                }
                            });
                        });
                    }
                }
            }

            if clicked_label {
                self.select_node(Rc::clone(&node_rc));
            }
        });

        if let Some(expanded) = expand_descendants {
            Self::set_descendants_expanded(&node_rc, expanded);
            self.tree_rows_dirty = true;
        }

        if toggle_expanded {
            let mut collapse_selected_to_self = false;
            if tree_expanded {
                if let Some(selected) = &self.selected_node {
                    collapse_selected_to_self = Self::is_ancestor_or_self(&node_rc, selected);
                }
            }
            let mut n = node_rc.borrow_mut();
            n.tree_expanded = !n.tree_expanded;
            self.tree_rows_dirty = true;
            drop(n);
            if collapse_selected_to_self {
                self.select_node(Rc::clone(&node_rc));
            }
        }

        if has_multiple_dats {
            let _ = has_multiple_dats;
        }
    }
}
