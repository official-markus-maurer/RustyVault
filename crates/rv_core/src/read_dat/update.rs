impl DatUpdate {
    fn header_file_type_from_dat_header(header: Option<&str>) -> dat_reader::enums::HeaderFileType {
        use dat_reader::enums::HeaderFileType;

        let Some(header) = header.map(|s| s.trim()).filter(|s| !s.is_empty()) else {
            return HeaderFileType::NOTHING;
        };
        let header = header.to_ascii_lowercase();

        match header.as_str() {
            "a7800.xml" | "no-intro_a7800.xml" => HeaderFileType::A7800,
            "lynx.xml" | "no-intro_lnx.xml" => HeaderFileType::LYNX,
            "fds.xml" | "no-intro_fds.xml" => HeaderFileType::FDS,
            "no-intro_nes.xml" | "nongoodnes.xml" | "nes.xml" | "nes0.xml" => HeaderFileType::NES,
            "pce.xml" => HeaderFileType::PCE,
            "psid.xml" => HeaderFileType::PSID,
            "snes.xml" => HeaderFileType::SNES,
            "spc.xml" => HeaderFileType::SPC,
            _ => HeaderFileType::NOTHING,
        }
    }

    fn apply_header_rules(dat_header: &mut dat_reader::dat_store::DatHeader, rule: &crate::settings::DatRule) {
        use crate::settings::HeaderType;
        use dat_reader::dat_clean::DatClean;
        use dat_reader::enums::HeaderFileType;

        let mut header_file_type = Self::header_file_type_from_dat_header(dat_header.header.as_deref());
        if header_file_type != HeaderFileType::NOTHING {
            match rule.header_type {
                HeaderType::Optional => {}
                HeaderType::Headerless => header_file_type = HeaderFileType::NOTHING,
                HeaderType::Headered => header_file_type |= HeaderFileType::REQUIRED,
            }
        }

        DatClean::set_ext(&mut dat_header.base_dir, header_file_type);
    }

    fn apply_dat_rules(dat_header: &mut dat_reader::dat_store::DatHeader, rule: &crate::settings::DatRule) {
        if rule.compression_override_dat {
            dat_header.compression = Some(match rule.compression {
                dat_reader::enums::FileType::Zip => "zip".to_string(),
                dat_reader::enums::FileType::SevenZip => "7z".to_string(),
                _ => dat_header.compression.clone().unwrap_or_default(),
            });
        }

        if rule.merge_override_dat {
            dat_header.merge_type = Some(match rule.merge {
                crate::settings::MergeType::Split => "split".to_string(),
                crate::settings::MergeType::Merge => "merged".to_string(),
                crate::settings::MergeType::NonMerged => "nonmerged".to_string(),
                crate::settings::MergeType::CHDsMerge => "chdsmerged".to_string(),
                crate::settings::MergeType::None => dat_header.merge_type.clone().unwrap_or_default(),
            });
        }
    }

    fn dat_directory_name(dat_header: &dat_reader::dat_store::DatHeader, rule: &crate::settings::DatRule) -> String {
        if rule.use_description_as_dir_name {
            if let Some(desc) = dat_header.description.as_deref().map(|s| s.trim()).filter(|s| !s.is_empty()) {
                return desc.to_string();
            }
        }
        if rule.use_id_for_name {
            if let Some(id) = dat_header.id.as_deref().map(|s| s.trim()).filter(|s| !s.is_empty()) {
                return id.to_string();
            }
        }
        dat_header
            .name
            .clone()
            .unwrap_or_else(|| "Unknown_DAT".to_string())
    }

    /// Recursively scans `dat_dir_path`, parses all found DATs in parallel, and merges them into `root`.
    pub fn update_dat(root: Rc<RefCell<RvFile>>, dat_dir_path: &str) {
        println!("Scanning for DATs in {}...", dat_dir_path);

        let mut dats_found = Vec::new();
        Self::scan_dat_dir(dat_dir_path, &mut dats_found);

        println!("Found {} DAT files.", dats_found.len());

        let romvault_dir = {
            let root_ref = root.borrow();
            root_ref
                .children
                .iter()
                .find(|c| Self::normalized_path_eq(&c.borrow().name, "RustyVault"))
                .cloned()
        };

        if let Some(rv_dir) = romvault_dir {
            let parsed_results: Vec<(String, String, Result<dat_reader::dat_store::DatHeader, String>)> =
                dats_found
                    .into_par_iter()
                    .map(|(dat_path, virtual_dir)| {
                        if let Ok(buffer) = fs::read(&dat_path) {
                            let file_name = Path::new(&dat_path)
                                .file_name()
                                .unwrap_or_default()
                                .to_string_lossy()
                                .into_owned();
                            (
                                dat_path.clone(),
                                virtual_dir.clone(),
                                read_dat(&buffer, &file_name),
                            )
                        } else {
                            (
                                dat_path.clone(),
                                virtual_dir.clone(),
                                Err("Could not read file from disk".to_string()),
                            )
                        }
                    })
                    .collect();

            for (dat_path, virtual_dir, parse_result) in parsed_results {
                println!("Integrating DAT: {}", dat_path);
                match parse_result {
                    Ok(mut dat_header) => {
                        println!("Successfully parsed DAT: {:?}", dat_header.name);

                        let mut current_parent = Rc::clone(&rv_dir);

                        if !virtual_dir.is_empty() {
                            let parts: Vec<&str> = virtual_dir
                                .split(['/', '\\'])
                                .filter(|s| !s.is_empty())
                                .collect();
                            for part in parts {
                                let mut found = None;
                                {
                                    let mut cp_mut = current_parent.borrow_mut();
                                    cp_mut.cached_stats = None;
                                    let (res, index) = cp_mut.child_name_search(FileType::Dir, part);
                                    if res == 0 && index < cp_mut.children.len() {
                                        found = Some(Rc::clone(&cp_mut.children[index]));
                                    }
                                    if found.is_none() {
                                        let mut new_dir = RvFile::new(FileType::Dir);
                                        new_dir.name = part.to_string();
                                        new_dir.set_dat_got_status(
                                            dat_reader::enums::DatStatus::InDatCollect,
                                            dat_reader::enums::GotStatus::NotGot,
                                        );
                                        new_dir.rep_status_reset();
                                        let d_rc = Rc::new(RefCell::new(new_dir));
                                        d_rc.borrow_mut().parent = Some(Rc::downgrade(&current_parent));
                                        cp_mut.child_add(Rc::clone(&d_rc));
                                        found = Some(d_rc);
                                    }
                                }
                                current_parent = found.unwrap();
                            }
                        }

                        let default_dir_name = dat_header
                            .name
                            .clone()
                            .unwrap_or_else(|| "Unknown_DAT".to_string());

                        let dat_dir_key = {
                            let base = current_parent.borrow().get_logical_name();
                            if base.is_empty() {
                                default_dir_name.clone()
                            } else {
                                format!("{}\\{}", base, default_dir_name)
                            }
                        };
                        let rule = crate::settings::find_rule(&dat_dir_key);
                        Self::apply_header_rules(&mut dat_header, &rule);
                        Self::apply_dat_rules(&mut dat_header, &rule);

                        if rule.add_category_sub_dirs {
                            if let Some(cat) = dat_header.category.as_deref().map(|s| s.trim()).filter(|s| !s.is_empty()) {
                                let mut found = None;
                                {
                                    let mut cp_mut = current_parent.borrow_mut();
                                    cp_mut.cached_stats = None;
                                    let (res, index) = cp_mut.child_name_search(FileType::Dir, cat);
                                    if res == 0 && index < cp_mut.children.len() {
                                        found = Some(Rc::clone(&cp_mut.children[index]));
                                    }
                                    if found.is_none() {
                                        let mut new_dir = RvFile::new(FileType::Dir);
                                        new_dir.name = cat.to_string();
                                        new_dir.set_dat_got_status(
                                            dat_reader::enums::DatStatus::InDatCollect,
                                            dat_reader::enums::GotStatus::NotGot,
                                        );
                                        new_dir.rep_status_reset();
                                        let d_rc = Rc::new(RefCell::new(new_dir));
                                        d_rc.borrow_mut().parent = Some(Rc::downgrade(&current_parent));
                                        cp_mut.child_add(Rc::clone(&d_rc));
                                        found = Some(d_rc);
                                    }
                                }
                                current_parent = found.unwrap();
                            }
                        }

                        let dir_name = Self::dat_directory_name(&dat_header, &rule);

                        let mut rv_dir_mut = current_parent.borrow_mut();

                        rv_dir_mut.cached_stats = None;
                        let mut target_dir = None;

                        let (res, index) = rv_dir_mut.child_name_search(FileType::Dir, &dir_name);
                        if res == 0 && index < rv_dir_mut.children.len() {
                            target_dir = Some(Rc::clone(&rv_dir_mut.children[index]));
                        }

                        let (target_dir, existing_children) = match target_dir {
                            Some(d) => {
                                let existing_children = {
                                    let mut existing = d.borrow_mut();
                                    existing.cached_stats = None;
                                    std::mem::take(&mut existing.children)
                                };
                                (d, existing_children)
                            }
                            None => {
                                let mut new_dir = RvFile::new(FileType::Dir);
                                new_dir.name = dir_name;
                                new_dir.set_dat_got_status(
                                    dat_reader::enums::DatStatus::InDatCollect,
                                    dat_reader::enums::GotStatus::NotGot,
                                );
                                new_dir.rep_status_reset();
                                let d_rc = Rc::new(RefCell::new(new_dir));
                                d_rc.borrow_mut().parent = Some(Rc::downgrade(&current_parent));
                                rv_dir_mut.child_add(Rc::clone(&d_rc));
                                (d_rc, Vec::new())
                            }
                        };

                        let rv_dat_rc = {
                            let existing = target_dir.borrow().dir_dats.first().cloned();
                            let dat_rc = existing.unwrap_or_else(|| Rc::new(RefCell::new(RvDat::new())));
                            {
                                let mut dat_mut = dat_rc.borrow_mut();
                                Self::populate_rv_dat_from_header(&mut dat_mut, &dat_header, &dat_path);
                            }
                            dat_rc
                        };

                        {
                            let mut td = target_dir.borrow_mut();
                            rv_dat_rc.borrow_mut().dat_index = 0;
                            td.dir_dats.clear();
                            td.dir_dats.push(Rc::clone(&rv_dat_rc));
                        }

                        let mut existing_children = existing_children;
                        for dat_child in &dat_header.base_dir.children {
                            Self::map_dat_node_to_rv_file(
                                Rc::clone(&target_dir),
                                dat_child,
                                Rc::clone(&rv_dat_rc),
                                &mut existing_children,
                            );
                        }
                        for leftover in existing_children {
                            if let Some(preserved) = Self::preserve_unmatched_existing_subtree(leftover) {
                                preserved.borrow_mut().parent = Some(Rc::downgrade(&target_dir));
                                target_dir.borrow_mut().child_add(preserved);
                            }
                        }
                    }
                    Err(e) => {
                        println!("Error reading DAT {}: {}", dat_path, e);
                    }
                }
            }
        }
    }
}
