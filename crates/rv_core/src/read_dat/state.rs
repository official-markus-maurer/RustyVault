impl DatUpdate {
    fn populate_rv_dat_from_header(rv_dat: &mut RvDat, dat_header: &DatHeader, dat_path: &str) {
        rv_dat.game_meta_data.clear();
        rv_dat.set_data(DatData::Id, dat_header.id.clone());
        rv_dat.set_data(DatData::DatName, dat_header.name.clone());
        rv_dat.set_data(DatData::DatRootFullName, Some(dat_path.to_string()));
        rv_dat.set_data(DatData::RootDir, dat_header.root_dir.clone());
        rv_dat.set_data(DatData::Description, dat_header.description.clone());
        rv_dat.set_data(DatData::Category, dat_header.category.clone());
        rv_dat.set_data(DatData::Version, dat_header.version.clone());
        rv_dat.set_data(DatData::Date, dat_header.date.clone());
        rv_dat.set_data(DatData::Author, dat_header.author.clone());
        rv_dat.set_data(DatData::Email, dat_header.email.clone());
        rv_dat.set_data(DatData::HomePage, dat_header.homepage.clone());
        rv_dat.set_data(DatData::Url, dat_header.url.clone());
        rv_dat.set_data(DatData::Header, dat_header.header.clone());
        rv_dat.set_data(DatData::Compression, dat_header.compression.clone());
        rv_dat.set_data(DatData::MergeType, dat_header.merge_type.clone());
        rv_dat.set_data(DatData::DirSetup, dat_header.dir.clone());
        rv_dat.time_stamp = fs::metadata(dat_path)
            .ok()
            .and_then(|m| m.modified().ok())
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_secs() as i64)
            .unwrap_or_default();
    }

    fn take_matching_existing_child(
        existing_children: &mut Vec<Rc<RefCell<RvFile>>>,
        name: &str,
        file_type: FileType,
    ) -> Option<Rc<RefCell<RvFile>>> {
        let match_index = existing_children.iter().position(|child| {
            let child_ref = child.borrow();
            Self::normalized_path_eq(&child_ref.name, name) && child_ref.file_type == file_type
        })?;
        Some(existing_children.remove(match_index))
    }

    fn apply_existing_runtime_state(new_rv: &mut RvFile, existing: &RvFile) {
        new_rv.set_got_status(existing.got_status());
        new_rv.tree_checked = existing.tree_checked;
        new_rv.tree_expanded = existing.tree_expanded;
    }

    fn preserve_existing_physical_state(new_rv: &mut RvFile, existing: &RvFile) {
        if existing.got_status() == dat_reader::enums::GotStatus::NotGot {
            return;
        }

        if existing.file_mod_time_stamp != i64::MIN {
            new_rv.file_mod_time_stamp = existing.file_mod_time_stamp;
        }
        if existing.local_header_offset.is_some() {
            new_rv.local_header_offset = existing.local_header_offset;
        }
        if existing.size.is_some() {
            new_rv.size = existing.size;
        }
        if existing.crc.is_some() {
            new_rv.crc = existing.crc.clone();
        }
        if existing.sha1.is_some() {
            new_rv.sha1 = existing.sha1.clone();
        }
        if existing.md5.is_some() {
            new_rv.md5 = existing.md5.clone();
        }
        if existing.sha256.is_some() {
            new_rv.sha256 = existing.sha256.clone();
        }
        if existing.alt_size.is_some() {
            new_rv.alt_size = existing.alt_size;
        }
        if existing.alt_crc.is_some() {
            new_rv.alt_crc = existing.alt_crc.clone();
        }
        if existing.alt_sha1.is_some() {
            new_rv.alt_sha1 = existing.alt_sha1.clone();
        }
        if existing.alt_md5.is_some() {
            new_rv.alt_md5 = existing.alt_md5.clone();
        }
        if existing.alt_sha256.is_some() {
            new_rv.alt_sha256 = existing.alt_sha256.clone();
        }
        if existing.chd_version.is_some() {
            new_rv.chd_version = existing.chd_version;
        }
        if existing.zip_struct != dat_reader::enums::ZipStructure::None {
            new_rv.zip_struct = existing.zip_struct;
        }
        new_rv.file_status.remove(Self::PRESERVED_PHYSICAL_FLAGS);
        new_rv
            .file_status
            .insert(existing.file_status & Self::PRESERVED_PHYSICAL_FLAGS);

        if existing.file_status_is(FileStatus::HEADER_FILE_TYPE_FROM_HEADER) {
            let required = new_rv.header_file_type & HeaderFileType::REQUIRED;
            new_rv.header_file_type = existing.header_file_type() | required;
        }
    }

    fn preserve_unmatched_existing_subtree(node_rc: Rc<RefCell<RvFile>>) -> Option<Rc<RefCell<RvFile>>> {
        let children = {
            let mut node = node_rc.borrow_mut();
            std::mem::take(&mut node.children)
        };

        let mut kept_children = Vec::new();
        for child in children {
            if let Some(kept_child) = Self::preserve_unmatched_existing_subtree(child) {
                kept_children.push(kept_child);
            }
        }

        let should_keep = {
            let node = node_rc.borrow();
            node.got_status() != dat_reader::enums::GotStatus::NotGot || !kept_children.is_empty()
        };

        if !should_keep {
            return None;
        }

        {
            let mut node = node_rc.borrow_mut();
            node.children = kept_children;
            node.dat = None;
            node.dir_dats.clear();
            if node.dat_status() != DatStatus::NotInDat {
                node.set_dat_status(DatStatus::NotInDat);
            }
            node.cached_stats = None;
            node.rep_status_reset();
        }

        Some(node_rc)
    }
}
