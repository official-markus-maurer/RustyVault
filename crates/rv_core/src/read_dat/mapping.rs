impl DatUpdate {
    fn map_dat_node_to_rv_file(
        parent: Rc<RefCell<RvFile>>,
        dat_node: &DatNode,
        dat_rc: Rc<RefCell<RvDat>>,
        existing_children: &mut Vec<Rc<RefCell<RvFile>>>,
    ) {
        let mut file_type = dat_node.file_type;
        if file_type == dat_reader::enums::FileType::UnSet {
            if dat_node.is_dir() {
                file_type = dat_reader::enums::FileType::Dir;
            } else {
                file_type = dat_reader::enums::FileType::File;
            }
        }

        let existing_match =
            Self::take_matching_existing_child(existing_children, &dat_node.name, file_type);
        let mut new_rv = RvFile::new(file_type);
        new_rv.name = dat_node.name.clone();
        new_rv.set_dat_status(DatStatus::InDatCollect);
        new_rv.set_dat_ref(Some(Rc::clone(&dat_rc)));
        if let Some(existing) = &existing_match {
            Self::apply_existing_runtime_state(&mut new_rv, &existing.borrow());
        }

        if dat_node.is_dir() {
            let d_dir = dat_node.dir().unwrap();
            let logical_key = {
                let parent_name = parent.borrow().get_logical_name();
                if parent_name.is_empty() {
                    dat_node.name.clone()
                } else {
                    format!("{}\\{}", parent_name, dat_node.name)
                }
            };
            let rule = crate::settings::find_rule(&logical_key);
            let mut desired = d_dir.dat_struct();
            if rule.compression_override_dat {
                desired = match rule.compression {
                    dat_reader::enums::FileType::Zip => match rule.compression_sub {
                        dat_reader::enums::ZipStructure::ZipTrrnt
                        | dat_reader::enums::ZipStructure::ZipZSTD
                        | dat_reader::enums::ZipStructure::ZipTDC => rule.compression_sub,
                        _ => dat_reader::enums::ZipStructure::ZipTrrnt,
                    },
                    dat_reader::enums::FileType::SevenZip => match rule.compression_sub {
                        dat_reader::enums::ZipStructure::SevenZipSLZMA
                        | dat_reader::enums::ZipStructure::SevenZipNLZMA
                        | dat_reader::enums::ZipStructure::SevenZipSZSTD
                        | dat_reader::enums::ZipStructure::SevenZipNZSTD => rule.compression_sub,
                        _ => dat_reader::enums::ZipStructure::SevenZipSLZMA,
                    },
                    _ => dat_reader::enums::ZipStructure::None,
                };
            }
            let mut fix = d_dir.dat_struct_fix();
            if !rule.convert_while_fixing {
                fix = false;
            }
            new_rv.set_zip_dat_struct(desired, fix);

            new_rv.set_dat_got_status(dat_reader::enums::DatStatus::InDatCollect, new_rv.got_status());
            if let Some(existing) = &existing_match {
                Self::preserve_existing_physical_state(&mut new_rv, &existing.borrow());
            }
            new_rv.rep_status_reset();

            if let Some(ref d_game) = d_dir.d_game {
                new_rv.game = Some(Rc::new(RefCell::new(RvGame::from_dat_game(d_game))));
            }

            let new_rc = Rc::new(RefCell::new(new_rv));
            new_rc.borrow_mut().parent = Some(Rc::downgrade(&parent));
            parent.borrow_mut().child_add(Rc::clone(&new_rc));

            let mut old_children = if let Some(existing) = existing_match {
                std::mem::take(&mut existing.borrow_mut().children)
            } else {
                Vec::new()
            };
            for child in &d_dir.children {
                Self::map_dat_node_to_rv_file(
                    Rc::clone(&new_rc),
                    child,
                    Rc::clone(&dat_rc),
                    &mut old_children,
                );
            }
            for leftover in old_children {
                if let Some(preserved) = Self::preserve_unmatched_existing_subtree(leftover) {
                    preserved.borrow_mut().parent = Some(Rc::downgrade(&new_rc));
                    new_rc.borrow_mut().child_add(preserved);
                }
            }
        } else {
            let d_file = dat_node.file().unwrap();
            new_rv.size = d_file.size;
            new_rv.crc = d_file.crc.clone();
            new_rv.sha1 = d_file.sha1.clone();
            new_rv.md5 = d_file.md5.clone();
            new_rv.sha256 = d_file.sha256.clone();
            if new_rv.size.is_some() {
                new_rv.file_status_set(FileStatus::SIZE_FROM_DAT);
            }
            if new_rv.crc.is_some() {
                new_rv.file_status_set(FileStatus::CRC_FROM_DAT);
            }
            if new_rv.sha1.is_some() {
                new_rv.file_status_set(FileStatus::SHA1_FROM_DAT);
            }
            if new_rv.md5.is_some() {
                new_rv.file_status_set(FileStatus::MD5_FROM_DAT);
            }
            if new_rv.sha256.is_some() {
                new_rv.file_status_set(FileStatus::SHA256_FROM_DAT);
            }

            if let Some(ref m) = d_file.merge {
                new_rv.merge = m.clone();
            }
            new_rv.status = d_file.status.clone();
            new_rv.set_header_file_type(d_file.header_file_type);
            if d_file.header_file_type != dat_reader::enums::HeaderFileType::NOTHING {
                new_rv.file_status_set(FileStatus::HEADER_FILE_TYPE_FROM_DAT);
            }
            if let Some(date_modified) = dat_node.date_modified {
                new_rv.file_mod_time_stamp = date_modified;
                new_rv.file_status_set(FileStatus::DATE_FROM_DAT);
            }
            if let Some(existing) = &existing_match {
                Self::preserve_existing_physical_state(&mut new_rv, &existing.borrow());
            }

            new_rv.set_dat_got_status(dat_reader::enums::DatStatus::InDatCollect, new_rv.got_status());
            new_rv.rep_status_reset();

            let new_rc = Rc::new(RefCell::new(new_rv));
            new_rc.borrow_mut().parent = Some(Rc::downgrade(&parent));
            parent.borrow_mut().child_add(new_rc);
        }
    }
}
