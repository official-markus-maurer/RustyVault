impl DatClean {
    pub fn make_dat_single_level(
        dat_header: &mut crate::dat_store::DatHeader,
        use_description: bool,
        mut sub_dir_type: RemoveSubType,
        is_files: bool,
        add_category: bool,
        cat_order: &[String],
    ) {
        let original = std::mem::take(&mut dat_header.base_dir.children);
        dat_header.dir = Some("noautodir".to_string());

        let mut root_dir_name = String::new();
        if root_dir_name.is_empty()
            && use_description
            && dat_header
                .description
                .as_deref()
                .is_some_and(|d| !d.trim().is_empty())
        {
            root_dir_name = dat_header.description.clone().unwrap_or_default();
        }
        if root_dir_name.is_empty() {
            root_dir_name = dat_header.name.clone().unwrap_or_default();
        }

        if sub_dir_type == RemoveSubType::RemoveAllIfNoConflicts {
            let mut seen = std::collections::HashSet::<String>::new();
            let mut found_repeat = false;
            for set in &original {
                let Some(dir_set) = set.dir() else { continue };
                for rom in &dir_set.children {
                    let key = rom.name.to_ascii_lowercase();
                    if !seen.insert(key) {
                        found_repeat = true;
                        break;
                    }
                }
                if found_repeat {
                    sub_dir_type = RemoveSubType::KeepAllSubDirs;
                    break;
                }
            }
        }

        dat_header.base_dir.children.clear();

        let d_game = crate::dat_store::DatGame {
            description: dat_header.description.clone(),
            ..Default::default()
        };

        if is_files {
            Self::make_single_level_into_dir(
                &mut dat_header.base_dir,
                original,
                sub_dir_type,
                add_category,
                cat_order,
                true,
            );
            return;
        }

        let mut out_node = DatNode::new_dir(root_dir_name.clone(), FileType::UnSet);
        if let Some(d) = out_node.dir_mut() {
            d.d_game = Some(Box::new(d_game));
        }
        dat_header.base_dir.add_child(out_node);
        let out_index = dat_header.base_dir.children.len() - 1;
        let out_dir = dat_header.base_dir.children[out_index].dir_mut().unwrap();
        Self::make_single_level_into_dir(
            out_dir,
            original,
            sub_dir_type,
            add_category,
            cat_order,
            false,
        );
    }

    fn make_single_level_into_dir(
        out_dir: &mut DatDir,
        original: Vec<DatNode>,
        sub_dir_type: RemoveSubType,
        add_category: bool,
        cat_order: &[String],
        is_files: bool,
    ) {
        for mut set in original {
            let set_name = set.name.clone();
            let set_game = set.dir().and_then(|d| d.d_game.clone());
            let set_category = if add_category {
                set.dir().and_then(|d| Self::find_category(d, cat_order))
            } else {
                None
            };

            let Some(dir_set) = set.dir_mut() else {
                continue;
            };
            let set_children = std::mem::take(&mut dir_set.children);
            let set_len = set_children.len();

            for mut rom in set_children {
                if sub_dir_type == RemoveSubType::KeepAllSubDirs {
                    Self::add_back_dir(is_files, out_dir, &set_name, set_game.as_deref(), rom);
                    continue;
                }
                if sub_dir_type == RemoveSubType::RemoveSubIfSingleFiles && set_len != 1 {
                    Self::add_back_dir(is_files, out_dir, &set_name, set_game.as_deref(), rom);
                    continue;
                }
                if sub_dir_type == RemoveSubType::RemoveSubIfNameMatches {
                    if set_len != 1 {
                        Self::add_back_dir(is_files, out_dir, &set_name, set_game.as_deref(), rom);
                        continue;
                    }

                    let mut test_rom_name = std::path::Path::new(&rom.name)
                        .file_stem()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .to_string();
                    if let Some(ref cat) = set_category {
                        if !cat.is_empty() {
                            test_rom_name = format!("{}/{}", cat, test_rom_name);
                        }
                    }
                    if test_rom_name != set_name {
                        Self::add_back_dir(is_files, out_dir, &set_name, set_game.as_deref(), rom);
                        continue;
                    }
                }

                if let Some(ref cat) = set_category {
                    if !cat.is_empty() {
                        rom.name = format!("{}/{}", cat, rom.name);
                    }
                }

                out_dir.add_child(rom);
            }
        }
    }

    fn add_back_dir(
        is_files: bool,
        out_dir: &mut DatDir,
        set_name: &str,
        set_game: Option<&crate::dat_store::DatGame>,
        rom: DatNode,
    ) {
        if is_files {
            let existing_idx = out_dir
                .children
                .iter()
                .position(|n| n.is_dir() && n.name == set_name);
            let idx = if let Some(i) = existing_idx {
                i
            } else {
                let mut new_dir = DatNode::new_dir(set_name.to_string(), FileType::UnSet);
                if let Some(d) = new_dir.dir_mut() {
                    d.d_game = set_game.map(|g| Box::new(g.clone()));
                }
                out_dir.children.push(new_dir);
                out_dir.children.len() - 1
            };
            if let Some(d) = out_dir.children[idx].dir_mut() {
                d.add_child(rom);
            }
            return;
        }

        let mut rom = rom;
        rom.name = format!("{}/{}", set_name, rom.name);
        out_dir.add_child(rom);
    }
}
