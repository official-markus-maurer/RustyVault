impl Fix {
    fn is_fix_selected(node: &RvFile) -> bool {
        matches!(node.tree_checked, TreeSelect::Selected | TreeSelect::Locked)
    }

    fn is_fix_read_only(node: &RvFile) -> bool {
        matches!(node.tree_checked, TreeSelect::Locked)
    }

    fn has_selected_descendant(node: Rc<RefCell<RvFile>>) -> bool {
        let children = node.borrow().children.clone();
        for child in children {
            if Self::is_fix_selected(&child.borrow()) || Self::has_selected_descendant(Rc::clone(&child)) {
                return true;
            }
        }
        false
    }

    pub fn perform_fixes(root: Rc<RefCell<RvFile>>) {
        info!("Starting Fix execution pass...");
        Self::report_action("Fix pass started");
        let mut file_process_queue = Vec::new();
        let mut file_process_queue_head = 0usize;
        let mut total_fixed = 0;
        let settings = crate::settings::get_settings();
        let mut cache_timer = if settings.cache_save_timer_enabled {
            Some(std::time::Instant::now())
        } else {
            None
        };

        let mut needed_files = Vec::new();
        Self::gather_needed_files(Rc::clone(&root), &mut needed_files);

        use crate::hash_keys::{crc_key, md5_key, sha1_key, CrcKey, Md5Key, Sha1Key};

        let mut crc_map: HashMap<CrcKey, Rc<RefCell<RvFile>>> =
            HashMap::with_capacity(needed_files.len() * 2);
        let mut sha1_map: HashMap<Sha1Key, Rc<RefCell<RvFile>>> =
            HashMap::with_capacity(needed_files.len() * 2);
        let mut md5_map: HashMap<Md5Key, Rc<RefCell<RvFile>>> =
            HashMap::with_capacity(needed_files.len() * 2);

        for needed in needed_files {
            let n_ref = needed.borrow();
            let size = n_ref.size.unwrap_or(0);
            let alt_size = n_ref.alt_size.unwrap_or(size);
            if let Some(crc) = n_ref.crc.as_deref().and_then(|b| crc_key(size, b)) {
                crc_map.insert(crc, Rc::clone(&needed));
            }
            if let Some(crc) = n_ref
                .alt_crc
                .as_deref()
                .and_then(|b| crc_key(alt_size, b))
            {
                crc_map.insert(crc, Rc::clone(&needed));
            }
            if let Some(sha1) = n_ref.sha1.as_deref().and_then(|b| sha1_key(size, b)) {
                sha1_map.insert(sha1, Rc::clone(&needed));
            }
            if let Some(sha1) = n_ref
                .alt_sha1
                .as_deref()
                .and_then(|b| sha1_key(alt_size, b))
            {
                sha1_map.insert(sha1, Rc::clone(&needed));
            }
            if let Some(md5) = n_ref.md5.as_deref().and_then(|b| md5_key(size, b)) {
                md5_map.insert(md5, Rc::clone(&needed));
            }
            if let Some(md5) = n_ref
                .alt_md5
                .as_deref()
                .and_then(|b| md5_key(alt_size, b))
            {
                md5_map.insert(md5, Rc::clone(&needed));
            }
        }

        let children = root.borrow().children.clone();
        for child in children {
            Self::fix_base(
                Rc::clone(&child),
                false,
                &mut file_process_queue,
                &mut total_fixed,
                &crc_map,
                &sha1_map,
                &md5_map,
            );
            while file_process_queue_head < file_process_queue.len() {
                let queued_file = Rc::clone(&file_process_queue[file_process_queue_head]);
                file_process_queue_head += 1;
                Self::fix_base(
                    queued_file,
                    true,
                    &mut file_process_queue,
                    &mut total_fixed,
                    &crc_map,
                    &sha1_map,
                    &md5_map,
                );
                if let Some(last) = cache_timer {
                    if last.elapsed().as_secs_f64() / 60.0 > settings.cache_save_time_period as f64 {
                        // TODO(perf): cache writes serialize the full DB; use incremental writes or a background writer.
                        crate::cache::Cache::write_cache(Rc::clone(&root));
                        cache_timer = Some(std::time::Instant::now());
                    } else {
                        cache_timer = Some(last);
                    }
                }
            }
            file_process_queue.clear();
            file_process_queue_head = 0;
            if let Some(last) = cache_timer {
                if last.elapsed().as_secs_f64() / 60.0 > settings.cache_save_time_period as f64 {
                    crate::cache::Cache::write_cache(Rc::clone(&root));
                    cache_timer = Some(std::time::Instant::now());
                } else {
                    cache_timer = Some(last);
                }
            }
        }

        info!("Fix execution complete. Total fixed: {}", total_fixed);
        Self::report_action(format!("Fix pass complete. Total fixed: {}", total_fixed));
    }

    fn fix_dir(
        dir: Rc<RefCell<RvFile>>,
        queue: &mut Vec<Rc<RefCell<RvFile>>>,
        total_fixed: &mut i32,
        crc_map: &HashMap<crate::hash_keys::CrcKey, Rc<RefCell<RvFile>>>,
        sha1_map: &HashMap<crate::hash_keys::Sha1Key, Rc<RefCell<RvFile>>>,
        md5_map: &HashMap<crate::hash_keys::Md5Key, Rc<RefCell<RvFile>>>,
    ) {
        let children = dir.borrow().children.clone();

        for child in children {
            Self::fix_base(
                Rc::clone(&child),
                false,
                queue,
                total_fixed,
                crc_map,
                sha1_map,
                md5_map,
            );

            let mut head = 0usize;
            while head < queue.len() {
                let queued_file = Rc::clone(&queue[head]);
                head += 1;
                Self::fix_base(queued_file, true, queue, total_fixed, crc_map, sha1_map, md5_map);
            }
            queue.clear();
        }
    }

    fn fix_base(
        child: Rc<RefCell<RvFile>>,
        force_selected: bool,
        queue: &mut Vec<Rc<RefCell<RvFile>>>,
        total_fixed: &mut i32,
        crc_map: &HashMap<crate::hash_keys::CrcKey, Rc<RefCell<RvFile>>>,
        sha1_map: &HashMap<crate::hash_keys::Sha1Key, Rc<RefCell<RvFile>>>,
        md5_map: &HashMap<crate::hash_keys::Md5Key, Rc<RefCell<RvFile>>>,
    ) {
        if child.borrow().rep_status() == RepStatus::Deleted {
            return;
        }

        let (file_type, rep_status, is_selected) = {
            let child_ref = child.borrow();
            (
                child_ref.file_type,
                child_ref.rep_status(),
                force_selected || Self::is_fix_selected(&child_ref),
            )
        };

        match file_type {
            FileType::Zip | FileType::SevenZip => {
                if matches!(
                    rep_status,
                    RepStatus::Delete
                        | RepStatus::UnNeeded
                        | RepStatus::MoveToSort
                        | RepStatus::MoveToCorrupt
                        | RepStatus::Rename
                ) {
                    Self::fix_archive_node(Rc::clone(&child));
                    return;
                }
                if !is_selected && !Self::has_selected_descendant(Rc::clone(&child)) {
                    return;
                }
                Self::fix_a_zip(Rc::clone(&child), queue, total_fixed, crc_map, sha1_map, md5_map);
            }
            FileType::Dir => {
                if is_selected {
                    if rep_status == RepStatus::DirMissing {
                        Self::ensure_physical_directory_exists(Rc::clone(&child));
                    }
                    let has_name = !child.borrow().name.is_empty();
                    if has_name {
                        Self::rename_directory_if_needed(Rc::clone(&child));
                    }
                }
                Self::fix_dir(Rc::clone(&child), queue, total_fixed, crc_map, sha1_map, md5_map);
            }
            FileType::File | FileType::FileOnly | FileType::FileSevenZip | FileType::FileZip => {
                if !is_selected {
                    return;
                }
                Self::fix_a_file(Rc::clone(&child), queue, total_fixed, crc_map, sha1_map, md5_map);
            }
            _ => {}
        }
    }

    fn gather_needed_files(dir: Rc<RefCell<RvFile>>, needed: &mut Vec<Rc<RefCell<RvFile>>>) {
        let d = dir.borrow();
        for child in &d.children {
            if child.borrow().is_directory() {
                Self::gather_needed_files(Rc::clone(child), needed);
            } else if child.borrow().rep_status() == RepStatus::NeededForFix {
                needed.push(Rc::clone(child));
            }
        }
    }

    fn get_physical_path(file: Rc<RefCell<RvFile>>) -> String {
        Self::build_physical_path(file, false)
            .to_string_lossy()
            .replace('\\', "/")
    }

    fn get_existing_physical_path(file: Rc<RefCell<RvFile>>) -> String {
        Self::build_physical_path(file, true)
            .to_string_lossy()
            .replace('\\', "/")
    }

    fn build_physical_path(file: Rc<RefCell<RvFile>>, use_existing_names: bool) -> PathBuf {
        let mut path_parts = Vec::new();
        let mut current = Some(file);

        while let Some(node_rc) = current {
            let node = node_rc.borrow();
            let component = if use_existing_names { node.name_case() } else { &node.name };
            if !component.is_empty() {
                path_parts.push(component.to_string());
            }
            current = node.parent.as_ref().and_then(|w| w.upgrade());
        }

        path_parts.reverse();
        let logical_path = path_parts.join("\\");
        if Path::new(&logical_path).is_absolute() {
            return PathBuf::from(logical_path);
        }
        if let Some(mapped_path) = crate::settings::find_dir_mapping(&logical_path) {
            return PathBuf::from(mapped_path);
        }

        let mut path = PathBuf::new();
        for part in path_parts {
            if path.as_os_str().is_empty() {
                path = PathBuf::from(part);
            } else {
                path.push(part);
            }
        }
        path
    }

    fn ensure_physical_directory_exists(dir: Rc<RefCell<RvFile>>) {
        let target_path = Self::build_physical_path(Rc::clone(&dir), false);
        if target_path.as_os_str().is_empty() {
            return;
        }

        if target_path.exists() {
            if target_path.is_dir() {
                let mut dir_mut = dir.borrow_mut();
                dir_mut.file_name = dir_mut.name.clone();
                if dir_mut.got_status() != GotStatus::Got {
                    dir_mut.set_got_status(GotStatus::Got);
                }
                dir_mut.rep_status_reset();
            }
            return;
        }

        if let Err(e) = fs::create_dir_all(&target_path) {
            Self::report_action(format!(
                "Failed to create directory '{}': {}",
                target_path.to_string_lossy(),
                e
            ));
            return;
        }

        let mut dir_mut = dir.borrow_mut();
        dir_mut.file_name = dir_mut.name.clone();
        dir_mut.set_got_status(GotStatus::Got);
        dir_mut.rep_status_reset();
    }

    fn find_tree_root(node: Rc<RefCell<RvFile>>) -> Rc<RefCell<RvFile>> {
        let mut current = node;
        loop {
            let parent = {
                let borrowed = current.borrow();
                borrowed.parent.as_ref().and_then(|w| w.upgrade())
            };
            if let Some(parent) = parent {
                current = parent;
            } else {
                return current;
            }
        }
    }
}
