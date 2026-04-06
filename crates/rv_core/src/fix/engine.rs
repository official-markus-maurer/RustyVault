include!("engine/helpers.rs");
include!("engine/delete_check.rs");
include!("engine/paths.rs");
include!("engine/source_files.rs");
include!("engine/torrentzip.rs");
include!("engine/apply.rs");

impl Fix {
    const TORRENTZIP_DOS_TIME: u16 = 48128;
    const TORRENTZIP_DOS_DATE: u16 = 8600;

    fn status_retains_shared_physical_path(rep_status: RepStatus) -> bool {
        !matches!(
            rep_status,
            RepStatus::Delete
                | RepStatus::UnNeeded
                | RepStatus::MoveToSort
                | RepStatus::MoveToCorrupt
                | RepStatus::Deleted
        )
    }

    fn dat_status_retains_shared_physical_path(dat_status: DatStatus) -> bool {
        !matches!(dat_status, DatStatus::NotInDat | DatStatus::InToSort)
    }

    fn has_retained_shared_physical_path(
        root: Rc<RefCell<RvFile>>,
        current: Rc<RefCell<RvFile>>,
        current_path: &Path,
    ) -> bool {
        let children = root.borrow().children.clone();
        for child in children {
            if Self::node_retains_shared_physical_path(
                Rc::clone(&child),
                Rc::clone(&current),
                current_path,
            ) {
                return true;
            }
        }
        false
    }

    fn node_retains_shared_physical_path(
        node: Rc<RefCell<RvFile>>,
        current: Rc<RefCell<RvFile>>,
        current_path: &Path,
    ) -> bool {
        if Rc::ptr_eq(&node, &current) {
            return false;
        }

        let (is_dir, got_status, dat_status, rep_status, children) = {
            let borrowed = node.borrow();
            (
                borrowed.is_directory(),
                borrowed.got_status(),
                borrowed.dat_status(),
                borrowed.rep_status(),
                borrowed.children.clone(),
            )
        };

        if !is_dir
            && got_status == GotStatus::Got
            && Self::dat_status_retains_shared_physical_path(dat_status)
            && Self::status_retains_shared_physical_path(rep_status)
        {
            let candidate_path = Self::build_physical_path(Rc::clone(&node), true);
            if Self::physical_path_eq_for_rename(&candidate_path, current_path) {
                return true;
            }
        }

        if is_dir {
            for child in children {
                if Self::node_retains_shared_physical_path(child, Rc::clone(&current), current_path) {
                    return true;
                }
            }
        }

        false
    }

    fn rename_directory_if_needed(dir: Rc<RefCell<RvFile>>) {
        if Self::has_case_only_sibling_dir(Rc::clone(&dir)) {
            return;
        }
        let current_path = Self::build_physical_path(Rc::clone(&dir), true);
        let target_path = Self::build_physical_path(Rc::clone(&dir), false);

        let rename_result = Self::rename_path_if_needed(&current_path, &target_path, "tmpdir");

        if rename_result.is_ok() {
            let mut dir_mut = dir.borrow_mut();
            dir_mut.file_name = dir_mut.name.clone();
        }
    }

    fn has_case_only_sibling_dir(dir: Rc<RefCell<RvFile>>) -> bool {
        #[cfg(not(windows))]
        {
            let _ = dir;
            false
        }
        #[cfg(windows)]
        {
            let (name, parent) = {
                let d = dir.borrow();
                (d.name.clone(), d.parent.as_ref().and_then(|p| p.upgrade()))
            };
            let Some(parent) = parent else {
                return false;
            };

            let siblings = parent.borrow().children.clone();
            for sib in siblings {
                if Rc::ptr_eq(&sib, &dir) {
                    continue;
                }
                let s = sib.borrow();
                if s.file_type != FileType::Dir {
                    continue;
                }
                if s.name == name {
                    continue;
                }
                if Self::logical_name_eq(&s.name, &name) {
                    return true;
                }
            }
            false
        }
    }

    fn get_archive_member_tosort_path(archive_path: &Path, child_name: &str, base_dir: &str) -> PathBuf {
        let mapped_base_dir = base_dir.replace('/', "\\");
        let mapped_base_path =
            crate::settings::find_dir_mapping(&mapped_base_dir).unwrap_or_else(|| mapped_base_dir.clone());
        if let Some((source_logical_key, source_root_path)) = crate::settings::find_mapping_for_physical_path(archive_path) {
            if let Some(relative_archive_path) = crate::settings::strip_physical_prefix(archive_path, &source_root_path) {
                let archive_name = relative_archive_path.file_name().unwrap_or_default();
                let mut target_path = PathBuf::from(&mapped_base_path);
                let mut relative_dirs: Vec<String> = relative_archive_path
                    .parent()
                    .map(|parent| {
                        parent
                            .components()
                            .filter_map(|component| match component {
                                std::path::Component::Normal(part) => Some(part.to_string_lossy().to_string()),
                                _ => None,
                            })
                            .collect()
                    })
                    .unwrap_or_default();

                if Self::logical_name_eq(&source_logical_key, "ToSort")
                    && Self::logical_name_eq(&mapped_base_dir, "ToSort\\Corrupt")
                    && relative_dirs.first().is_some_and(|s| Self::logical_name_eq(s, "Corrupt"))
                {
                    relative_dirs.remove(0);
                }

                for part in &relative_dirs {
                    target_path.push(part);
                }
                target_path.push(archive_name);

                for part in child_name.split(['/', '\\']).filter(|part| !part.is_empty()) {
                    target_path.push(part);
                }

                if let Some(parent) = target_path.parent() {
                    let _ = fs::create_dir_all(parent);
                }

                return target_path;
            }
        }

        let archive_parent = archive_path.parent().unwrap_or_else(|| Path::new(""));
        let archive_name = archive_path.file_name().unwrap_or_default();

        let mut root_base = PathBuf::new();
        let mut normal_components = Vec::new();

        for component in archive_parent.components() {
            match component {
                std::path::Component::Prefix(_) | std::path::Component::RootDir => {
                    root_base.push(component.as_os_str());
                }
                std::path::Component::Normal(part) => {
                    normal_components.push(part.to_string_lossy().to_string());
                }
                _ => {}
            }
        }

        if let Some(first_normal) = normal_components.first() {
            root_base.push(first_normal);
        }

        let mut relative_dirs = if normal_components.len() > 1 {
            normal_components[1..].to_vec()
        } else {
            Vec::new()
        };

        let normalized_base_dir = base_dir.replace('/', "\\");
        let base_parts: Vec<String> = normalized_base_dir
            .split('\\')
            .filter(|part| !part.is_empty())
            .map(|part| part.to_string())
            .collect();

        let shares_root = normal_components
            .first()
            .zip(base_parts.first())
            .is_some_and(|(path_root, base_root)| Self::logical_name_eq(path_root, base_root));

        if shares_root {
            let base_suffix: Vec<String> = base_parts.iter().skip(1).cloned().collect();
            let already_prefixed = relative_dirs.len() >= base_suffix.len()
                && base_suffix
                    .iter()
                    .zip(relative_dirs.iter())
                    .all(|(base_part, relative_part)| Self::logical_name_eq(base_part, relative_part));

            if !base_suffix.is_empty() && !already_prefixed {
                let mut prefixed = base_suffix;
                prefixed.extend(relative_dirs);
                relative_dirs = prefixed;
            }
        } else {
            let mut prefixed = base_parts;
            prefixed.extend(relative_dirs);
            relative_dirs = prefixed;
        }

        let mut target_path = root_base;
        for part in &relative_dirs {
            target_path.push(part);
        }
        target_path.push(archive_name);

        for part in child_name.split(['/', '\\']).filter(|part| !part.is_empty()) {
            target_path.push(part);
        }

        if let Some(parent) = target_path.parent() {
            let _ = fs::create_dir_all(parent);
        }

        target_path
    }

    fn find_source_file(
        file: &RvFile,
        crc_map: &HashMap<crate::hash_keys::CrcKey, Rc<RefCell<RvFile>>>,
        sha1_map: &HashMap<crate::hash_keys::Sha1Key, Rc<RefCell<RvFile>>>,
        md5_map: &HashMap<crate::hash_keys::Md5Key, Rc<RefCell<RvFile>>>,
    ) -> Option<Rc<RefCell<RvFile>>> {
        let size = file.size.unwrap_or(0);
        let alt_size = file.alt_size.unwrap_or(size);

        if let Some(key) = file.crc.as_deref().and_then(|b| crate::hash_keys::crc_key(size, b)) {
            if let Some(found) = crc_map.get(&key) {
                return Some(Rc::clone(found));
            }
        }
        if alt_size != size {
            if let Some(key) =
                file.crc.as_deref().and_then(|b| crate::hash_keys::crc_key(alt_size, b))
            {
                if let Some(found) = crc_map.get(&key) {
                    return Some(Rc::clone(found));
                }
            }
        }
        if let Some(key) = file
            .alt_crc
            .as_deref()
            .and_then(|b| crate::hash_keys::crc_key(alt_size, b))
        {
            if let Some(found) = crc_map.get(&key) {
                return Some(Rc::clone(found));
            }
        }

        if let Some(key) = file
            .sha1
            .as_deref()
            .and_then(|b| crate::hash_keys::sha1_key(size, b))
        {
            if let Some(found) = sha1_map.get(&key) {
                return Some(Rc::clone(found));
            }
        }
        if alt_size != size {
            if let Some(key) = file
                .sha1
                .as_deref()
                .and_then(|b| crate::hash_keys::sha1_key(alt_size, b))
            {
                if let Some(found) = sha1_map.get(&key) {
                    return Some(Rc::clone(found));
                }
            }
        }
        if let Some(key) = file
            .alt_sha1
            .as_deref()
            .and_then(|b| crate::hash_keys::sha1_key(alt_size, b))
        {
            if let Some(found) = sha1_map.get(&key) {
                return Some(Rc::clone(found));
            }
        }

        if let Some(key) = file
            .md5
            .as_deref()
            .and_then(|b| crate::hash_keys::md5_key(size, b))
        {
            if let Some(found) = md5_map.get(&key) {
                return Some(Rc::clone(found));
            }
        }
        if alt_size != size {
            if let Some(key) = file
                .md5
                .as_deref()
                .and_then(|b| crate::hash_keys::md5_key(alt_size, b))
            {
                if let Some(found) = md5_map.get(&key) {
                    return Some(Rc::clone(found));
                }
            }
        }
        if let Some(key) = file
            .alt_md5
            .as_deref()
            .and_then(|b| crate::hash_keys::md5_key(alt_size, b))
        {
            if let Some(found) = md5_map.get(&key) {
                return Some(Rc::clone(found));
            }
        }

        None
    }

    fn collect_archive_rebuild_entries(
        parent: Rc<RefCell<RvFile>>,
        target_prefix: &str,
        existing_prefix: &str,
        entries: &mut Vec<ArchiveRebuildEntry>,
        any_changes: &mut bool,
    ) {
        let children = parent.borrow().children.clone();
        for child in children {
            let (child_name, existing_child_name, is_directory) = {
                let child_ref = child.borrow();
                let child_name = if target_prefix.is_empty() {
                    child_ref.name.clone()
                } else {
                    format!("{}/{}", target_prefix, child_ref.name)
                };
                let existing_child_name = if existing_prefix.is_empty() {
                    child_ref.name_case().to_string()
                } else {
                    format!("{}/{}", existing_prefix, child_ref.name_case())
                };
                (child_name, existing_child_name, child_ref.is_directory())
            };

            if child_name != existing_child_name {
                *any_changes = true;
            }

            if is_directory {
                let has_children = !child.borrow().children.is_empty();
                if !has_children {
                    entries.push(ArchiveRebuildEntry {
                        node: Rc::clone(&child),
                        target_name: child_name.clone(),
                        existing_name: existing_child_name.clone(),
                        is_directory: true,
                    });
                }
                Self::collect_archive_rebuild_entries(
                    Rc::clone(&child),
                    &child_name,
                    &existing_child_name,
                    entries,
                    any_changes,
                );
            } else {
                entries.push(ArchiveRebuildEntry {
                    node: Rc::clone(&child),
                    target_name: child_name,
                    existing_name: existing_child_name,
                    is_directory: false,
                });
            }
        }
    }

    fn archive_child_matches_named(
        source_child: &RvFile,
        source_name: &str,
        target_child: &RvFile,
        target_name: &str,
    ) -> bool {
        if source_name != target_name {
            return false;
        }
        if source_child.size != target_child.size {
            return false;
        }
        if target_child.crc.is_some() && source_child.crc != target_child.crc {
            return false;
        }
        if target_child.sha1.is_some() && source_child.sha1 != target_child.sha1 {
            return false;
        }
        if target_child.md5.is_some() && source_child.md5 != target_child.md5 {
            return false;
        }

        true
    }

    fn collect_archive_match_entries(
        parent: Rc<RefCell<RvFile>>,
        prefix: &str,
        entries: &mut Vec<ArchiveMatchEntry>,
    ) {
        let children = parent.borrow().children.clone();
        for child in children {
            let (logical_name, is_directory) = {
                let child_ref = child.borrow();
                let logical_name = if prefix.is_empty() {
                    child_ref.name.clone()
                } else {
                    format!("{}/{}", prefix, child_ref.name)
                };
                (logical_name, child_ref.is_directory())
            };

            if is_directory {
                Self::collect_archive_match_entries(Rc::clone(&child), &logical_name, entries);
            } else {
                entries.push(ArchiveMatchEntry {
                    node: Rc::clone(&child),
                    logical_name,
                });
            }
        }
    }

    fn mark_tree_as_got(node: Rc<RefCell<RvFile>>) {
        let children = {
            let mut node_ref = node.borrow_mut();
            let dat_status = node_ref.dat_status();
            node_ref.set_got_status(dat_reader::enums::GotStatus::Got);
            node_ref.set_rep_status(match dat_status {
                dat_reader::enums::DatStatus::InDatMIA => RepStatus::CorrectMIA,
                dat_reader::enums::DatStatus::InToSort => RepStatus::InToSort,
                dat_reader::enums::DatStatus::NotInDat => RepStatus::Unknown,
                _ => RepStatus::Correct,
            });
            node_ref.cached_stats = None;
            node_ref.children.clone()
        };

        for child in children {
            Self::mark_tree_as_got(child);
        }
    }

    fn fix_archive_node(archive: Rc<RefCell<RvFile>>) {
        let (rep_status, current_path, target_path, is_read_only) = {
            let archive_ref = archive.borrow();
            let current_path = Self::build_physical_path(Rc::clone(&archive), true);
            let target_path = Self::build_physical_path(Rc::clone(&archive), false);
            (
                archive_ref.rep_status(),
                current_path,
                target_path,
                Self::is_fix_read_only(&archive_ref),
            )
        };

        if is_read_only {
            return;
        }

        match rep_status {
            RepStatus::Delete | RepStatus::UnNeeded => {
                let root = Self::find_tree_root(Rc::clone(&archive));
                if Self::has_retained_shared_physical_path(root, Rc::clone(&archive), &current_path) {
                    let mut archive_mut = archive.borrow_mut();
                    archive_mut.set_got_status(GotStatus::NotGot);
                    archive_mut.rep_status_reset();
                    return;
                }
                if Path::new(&current_path).exists() {
                    let _ = fs::remove_file(&current_path);
                }
                let mut archive_mut = archive.borrow_mut();
                archive_mut.set_got_status(GotStatus::NotGot);
                archive_mut.rep_status_reset();
            }
            RepStatus::MoveToSort => {
                let current_path_str = current_path.to_string_lossy();
                let tosort_path = Self::get_tosort_path(&current_path_str, "ToSort");
                let tosort_path = PathBuf::from(tosort_path);
                if current_path.exists() {
                    let _ = Self::rename_path_if_needed(&current_path, &tosort_path, "tmptosort");
                }
                let mut archive_mut = archive.borrow_mut();
                archive_mut.set_dat_got_status(dat_reader::enums::DatStatus::InToSort, GotStatus::Got);
                archive_mut.rep_status_reset();
            }
            RepStatus::MoveToCorrupt => {
                let current_path_str = current_path.to_string_lossy();
                let tosort_path = Self::get_tosort_path(&current_path_str, "ToSort/Corrupt");
                let tosort_path = PathBuf::from(tosort_path);
                if current_path.exists() {
                    let _ = Self::rename_path_if_needed(&current_path, &tosort_path, "tmptosort");
                }
                let mut archive_mut = archive.borrow_mut();
                archive_mut.set_got_status(GotStatus::NotGot);
                archive_mut.rep_status_reset();
            }
            RepStatus::Rename => {
                let _ = Self::rename_path_if_needed(&current_path, &target_path, "tmpfile");
                {
                    let mut archive_mut = archive.borrow_mut();
                    archive_mut.file_name = archive_mut.name.clone();
                    archive_mut.set_got_status(GotStatus::Got);
                    archive_mut.rep_status_reset();
                }
            }
            _ => {}
        }
    }

    fn torrentzip_flags(name: &str) -> u16 {
        0x0002 | if name.is_ascii() { 0 } else { 0x0800 }
    }

    #[cfg(test)]
    fn fix_a_zip(
        zip_file: Rc<RefCell<RvFile>>,
        queue: &mut Vec<Rc<RefCell<RvFile>>>,
        total_fixed: &mut i32,
        crc_map: &HashMap<crate::hash_keys::CrcKey, Rc<RefCell<RvFile>>>,
        sha1_map: &HashMap<crate::hash_keys::Sha1Key, Rc<RefCell<RvFile>>>,
        md5_map: &HashMap<crate::hash_keys::Md5Key, Rc<RefCell<RvFile>>>,
    ) {
        let mut retained_physical_counts = HashMap::new();
        Self::fix_a_zip_with_counts(
            zip_file,
            queue,
            total_fixed,
            crc_map,
            sha1_map,
            md5_map,
            &mut retained_physical_counts,
        );
    }

    fn fix_a_zip_with_counts(
        zip_file: Rc<RefCell<RvFile>>,
        queue: &mut Vec<Rc<RefCell<RvFile>>>,
        total_fixed: &mut i32,
        crc_map: &HashMap<crate::hash_keys::CrcKey, Rc<RefCell<RvFile>>>,
        sha1_map: &HashMap<crate::hash_keys::Sha1Key, Rc<RefCell<RvFile>>>,
        md5_map: &HashMap<crate::hash_keys::Md5Key, Rc<RefCell<RvFile>>>,
        retained_physical_counts: &mut HashMap<String, u32>,
    ) {
        if Self::try_zip_move(
            Rc::clone(&zip_file),
            queue,
            total_fixed,
            crc_map,
            sha1_map,
            md5_map,
        ) {
            return;
        }

        if zip_file.borrow().file_type == FileType::SevenZip
            && Self::rebuild_seven_zip_archive(
                Rc::clone(&zip_file),
                queue,
                total_fixed,
                crc_map,
                sha1_map,
                md5_map,
            )
        {
            return;
        }

        if Self::rebuild_zip_archive(Rc::clone(&zip_file), queue, total_fixed, crc_map, sha1_map, md5_map) {
            return;
        }

        let children = zip_file.borrow().children.clone();
        for child in children {
            Self::fix_a_file_with_counts(
                Rc::clone(&child),
                queue,
                total_fixed,
                crc_map,
                sha1_map,
                md5_map,
                retained_physical_counts,
            );
        }
    }

    #[cfg(test)]
    fn fix_a_file(
        file: Rc<RefCell<RvFile>>,
        queue: &mut Vec<Rc<RefCell<RvFile>>>,
        total_fixed: &mut i32,
        crc_map: &HashMap<crate::hash_keys::CrcKey, Rc<RefCell<RvFile>>>,
        sha1_map: &HashMap<crate::hash_keys::Sha1Key, Rc<RefCell<RvFile>>>,
        md5_map: &HashMap<crate::hash_keys::Md5Key, Rc<RefCell<RvFile>>>,
    ) {
        let mut retained_physical_counts = HashMap::new();
        Self::fix_a_file_with_counts(
            file,
            queue,
            total_fixed,
            crc_map,
            sha1_map,
            md5_map,
            &mut retained_physical_counts,
        );
    }

    fn fix_a_file_with_counts(
        file: Rc<RefCell<RvFile>>,
        queue: &mut Vec<Rc<RefCell<RvFile>>>,
        total_fixed: &mut i32,
        crc_map: &HashMap<crate::hash_keys::CrcKey, Rc<RefCell<RvFile>>>,
        sha1_map: &HashMap<crate::hash_keys::Sha1Key, Rc<RefCell<RvFile>>>,
        md5_map: &HashMap<crate::hash_keys::Md5Key, Rc<RefCell<RvFile>>>,
        retained_physical_counts: &mut HashMap<String, u32>,
    ) {
        let (rep_status, name, current_path, target_path, is_read_only) = {
            let file_ref = file.borrow();
            let current_path = Self::build_physical_path(Rc::clone(&file), true);
            let target_path = Self::build_physical_path(Rc::clone(&file), false);
            (
                file_ref.rep_status(),
                file_ref.name.clone(),
                current_path,
                target_path,
                Self::is_fix_read_only(&file_ref),
            )
        };

        if is_read_only {
            return;
        }

        match rep_status {
            RepStatus::Delete | RepStatus::UnNeeded => {
                debug!("Deleting file: {}", current_path.display());
                Self::report_action(format!("Delete: {}", current_path.display()));
                let shared_refs = Self::physical_path_ref_count(retained_physical_counts, &current_path);
                if shared_refs > 0 {
                    let mut file_mut = file.borrow_mut();
                    file_mut.set_got_status(GotStatus::NotGot);
                    file_mut.rep_status_reset();
                    return;
                }
                if rep_status == RepStatus::Delete
                    && current_path.exists()
                    && file.borrow().file_type == FileType::File
                    && file.borrow().parent.is_some()
                    && !Self::double_check_delete_should_skip(&file.borrow())
                {
                    let tree_root = Self::find_tree_root(Rc::clone(&file));
                    if Self::find_delete_check_candidate(tree_root, Rc::clone(&file)).is_none() {
                        tracing::warn!(
                            "DoubleCheckDelete: no retained candidate found; skipping delete for {}",
                            name
                        );
                        return;
                    }
                }
                if current_path.exists() {
                    if fs::remove_file(&current_path).is_err() {
                        if let Ok(canonical) = current_path.canonicalize() {
                            let _ = fs::remove_file(&canonical);
                        }
                    }

                    let mut current_dir = current_path.parent();
                    while let Some(parent) = current_dir {
                        if fs::remove_dir(parent).is_err() {
                            break;
                        }
                        current_dir = parent.parent();
                    }
                }
                let mut file_mut = file.borrow_mut();
                file_mut.set_got_status(GotStatus::NotGot);
                file_mut.rep_status_reset();
            }
            RepStatus::MoveToSort => {
                let current_path_str = current_path.to_string_lossy();
                let tosort_path = Self::get_tosort_path(&current_path_str, "ToSort");
                let tosort_path = PathBuf::from(tosort_path);
                debug!("Moving to ToSort: {} -> {}", current_path.display(), tosort_path.display());
                Self::report_action(format!("MoveToSort: {} -> {}", current_path.display(), tosort_path.display()));
                if current_path.exists() {
                    let shared_refs =
                        Self::physical_path_ref_count(retained_physical_counts, &current_path);
                    if shared_refs == 0 {
                        let _ = Self::rename_path_if_needed(&current_path, &tosort_path, "tmptosort");
                    }
                }
                Self::maybe_delete_old_cue_zip_in_tosort(&tosort_path);
                let mut file_mut = file.borrow_mut();
                file_mut.set_dat_got_status(dat_reader::enums::DatStatus::InToSort, GotStatus::Got);
                file_mut.rep_status_reset();
            }
            RepStatus::MoveToCorrupt => {
                let current_path_str = current_path.to_string_lossy();
                let tosort_path = Self::get_tosort_path(&current_path_str, "ToSort/Corrupt");
                let tosort_path = PathBuf::from(tosort_path);
                debug!(
                    "Moving corrupt file to ToSort/Corrupt: {} -> {}",
                    current_path.display(),
                    tosort_path.display()
                );
                Self::report_action(format!(
                    "MoveToCorrupt: {} -> {}",
                    current_path.display(),
                    tosort_path.display()
                ));
                if current_path.exists() {
                    let shared_refs =
                        Self::physical_path_ref_count(retained_physical_counts, &current_path);
                    if shared_refs == 0 {
                        let _ = Self::rename_path_if_needed(&current_path, &tosort_path, "tmptosort");
                    }
                }
                Self::maybe_delete_old_cue_zip_in_tosort(&tosort_path);
                let mut file_mut = file.borrow_mut();
                file_mut.set_got_status(GotStatus::NotGot);
                file_mut.rep_status_reset();
            }
            RepStatus::CanBeFixed | RepStatus::CanBeFixedMIA | RepStatus::CorruptCanBeFixed => {
                let source_file = {
                    let f = file.borrow();
                    Self::find_source_file(&f, crc_map, sha1_map, md5_map)
                };

                if let Some(src) = source_file {
                    let src_path = Self::build_physical_path(Rc::clone(&src), true);

                    if Self::physical_path_eq_for_rename(&src_path, &target_path) {
                        Self::report_action(format!(
                            "Fix rename (case): {} -> {}",
                            src_path.display(),
                            target_path.display()
                        ));
                        let _ = Self::rename_path_if_needed(&src_path, &target_path, "tmpfix");
                    } else {
                        debug!("Fixing file from source: {} -> {}", src_path.display(), target_path.display());
                        Self::report_action(format!(
                            "Fix copy: {} -> {}",
                            src_path.display(),
                            target_path.display()
                        ));

                        if let Some(parent) = target_path.parent() {
                            let _ = fs::create_dir_all(parent);
                        }

                        let _ = fs::remove_file(&target_path);
                        if !Self::copy_source_file_to_path(Rc::clone(&src), &target_path) {
                            tracing::warn!(
                                "Failed writing fixed file: {}",
                                target_path.display()
                            );
                        }
                        Self::increment_physical_path_ref(retained_physical_counts, &target_path);

                        let source_is_read_only = {
                            let src_ref = src.borrow();
                            Self::is_fix_read_only(&src_ref)
                        };

                        if !source_is_read_only {
                            Self::queue_source_cleanup(Rc::clone(&src), queue);
                        }
                    }

                    let mut file_mut = file.borrow_mut();
                    file_mut.set_got_status(GotStatus::Got);
                    file_mut.rep_status_reset();
                    *total_fixed += 1;
                } else {
                    trace!("Could not find source file for: {}", name);
                }
            }
            RepStatus::Rename => {
                debug!("Renaming file: {} -> {}", current_path.display(), target_path.display());
                Self::report_action(format!("Rename: {} -> {}", current_path.display(), target_path.display()));
                let _ = Self::rename_path_if_needed(&current_path, &target_path, "tmpfile");
                if !Self::physical_path_eq_for_rename(&current_path, &target_path) {
                    Self::decrement_physical_path_ref(retained_physical_counts, &current_path);
                    Self::increment_physical_path_ref(retained_physical_counts, &target_path);
                }
                {
                    let mut file_mut = file.borrow_mut();
                    file_mut.file_name = file_mut.name.clone();
                    file_mut.set_got_status(GotStatus::Got);
                    file_mut.rep_status_reset();
                }
            }
            _ => {}
        }
    }
}

