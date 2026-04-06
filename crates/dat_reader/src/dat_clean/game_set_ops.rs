impl DatClean {
    pub fn set_files_as_games(t_dat: &mut DatDir) {
        let mut new_dirs_at_level: HashMap<String, usize> = HashMap::new();
        let mut i = 0usize;
        while i < t_dat.children.len() {
            if t_dat.children[i].is_dir() {
                let ft = t_dat.children[i].file_type;
                if let Some(d) = t_dat.children[i].dir_mut() {
                    if ft == FileType::Dir && d.d_game.is_none() {
                        Self::set_files_as_games(d);
                    }
                }
                i += 1;
                continue;
            }

            let name = t_dat.children[i].name.clone();
            let game_name = std::path::Path::new(&name)
                .file_stem()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();
            let key = game_name.to_ascii_lowercase();

            if let Some(existing_idx) = new_dirs_at_level.get(&key).copied() {
                let file_node = t_dat.children.remove(i);
                if let Some(d) = t_dat.children[existing_idx].dir_mut() {
                    d.add_child(file_node);
                }
                continue;
            }

            let file_node = t_dat.children.remove(i);
            let mut new_game = DatNode::new_dir(game_name.clone(), FileType::UnSet);
            if let Some(d) = new_game.dir_mut() {
                let g = crate::dat_store::DatGame {
                    description: Some(game_name.clone()),
                    ..Default::default()
                };
                d.d_game = Some(Box::new(g));
                d.add_child(file_node);
            }
            t_dat.children.push(new_game);
            let new_index = t_dat.children.len() - 1;
            new_dirs_at_level.insert(key, new_index);
        }
    }

    pub fn set_archives_as_games(t_dat: &mut DatDir) {
        for node in &mut t_dat.children {
            let ft = node.file_type;
            let name = node.name.clone();
            let Some(d) = node.dir_mut() else { continue };
            match ft {
                FileType::Dir => Self::set_archives_as_games(d),
                FileType::Zip | FileType::SevenZip => {
                    let g = crate::dat_store::DatGame {
                        description: Some(name),
                        ..Default::default()
                    };
                    d.d_game = Some(Box::new(g));
                }
                _ => {}
            }
        }
    }

    pub fn set_first_level_dirs_as_games(t_dat: &mut DatDir) {
        for node in &mut t_dat.children {
            let name = node.name.clone();
            let Some(d) = node.dir_mut() else { continue };
            let g = crate::dat_store::DatGame {
                description: Some(name),
                ..Default::default()
            };
            d.d_game = Some(Box::new(g));
        }
    }

    pub fn has_rom_of(t_dat: &DatDir) -> bool {
        for node in &t_dat.children {
            let Some(m_game) = node.dir() else { continue };
            if let Some(game) = m_game.d_game.as_ref() {
                if game.rom_of.as_ref().is_some_and(|s| !s.trim().is_empty())
                    || game.clone_of.as_ref().is_some_and(|s| !s.trim().is_empty())
                    || game
                        .clone_of_id
                        .as_ref()
                        .is_some_and(|s| !s.trim().is_empty())
                {
                    return true;
                }
                continue;
            }
            if Self::has_rom_of(m_game) {
                return true;
            }
        }
        false
    }

    pub fn find_parent_sets_for_game(
        search_name: &str,
        search_file_type: FileType,
        search_game: &crate::dat_store::DatGame,
        parent_dir: &DatDir,
        include_bios: bool,
    ) -> Vec<usize> {
        let mut parent_name = search_game.rom_of.clone().unwrap_or_default();
        if parent_name.is_empty() || parent_name == search_name {
            parent_name = search_game.clone_of.clone().unwrap_or_default();
        }
        if parent_name.is_empty() || parent_name == search_name {
            return Vec::new();
        }

        let mut out = Vec::new();
        let mut seen = std::collections::HashSet::<usize>::new();
        let mut current = parent_name;

        loop {
            let mut found = None;
            for (i, node) in parent_dir.children.iter().enumerate() {
                if node.file_type != search_file_type || node.name != current || !node.is_dir() {
                    continue;
                }
                let Some(d) = node.dir() else { continue };
                if d.d_game.is_none() {
                    continue;
                }
                found = Some(i);
                break;
            }

            let Some(i) = found else { break };
            if !seen.insert(i) {
                break;
            }

            let parent_game_dir = parent_dir.children[i].dir().unwrap();
            let parent_game = parent_game_dir.d_game.as_ref().unwrap();
            if !include_bios && parent_game.is_bios.as_deref() == Some("yes") {
                break;
            }

            out.push(i);

            let next = parent_game
                .rom_of
                .clone()
                .unwrap_or_else(|| parent_game.clone_of.clone().unwrap_or_default());
            if next.is_empty() || next == current {
                break;
            }
            current = next;
        }

        out
    }

    fn bytes_equal(a: &Option<Vec<u8>>, b: &Option<Vec<u8>>) -> bool {
        match (a.as_ref(), b.as_ref()) {
            (Some(x), Some(y)) => x == y,
            _ => true,
        }
    }

    fn file_equivalent(a: &crate::dat_store::DatFile, b: &crate::dat_store::DatFile) -> bool {
        if a.size.is_some() && b.size.is_some() && a.size != b.size {
            return false;
        }
        if !Self::bytes_equal(&a.crc, &b.crc) {
            return false;
        }
        if !Self::bytes_equal(&a.sha1, &b.sha1) {
            return false;
        }
        if !Self::bytes_equal(&a.sha256, &b.sha256) {
            return false;
        }
        if !Self::bytes_equal(&a.md5, &b.md5) {
            return false;
        }
        if a.is_disk != b.is_disk {
            return false;
        }
        true
    }

    fn find_rom_in_parent(
        dr0: &crate::dat_store::DatFile,
        lst_parent_game_indices: &[usize],
        parent_dir: &DatDir,
    ) -> bool {
        for idx in lst_parent_game_indices {
            let Some(pdir) = parent_dir.children.get(*idx).and_then(|n| n.dir()) else {
                continue;
            };
            for node in &pdir.children {
                let Some(dr1) = node.file() else { continue };
                if Self::file_equivalent(dr0, dr1) {
                    return true;
                }
            }
        }
        false
    }

    pub fn dat_set_make_split_set(t_dat: &mut DatDir) {
        let snapshot = t_dat.clone();
        for node in &mut t_dat.children {
            let game_name = node.name.clone();
            let game_file_type = node.file_type;
            let game_meta = node.dir().and_then(|d| d.d_game.as_deref()).cloned();
            let Some(m_game) = node.dir_mut() else {
                continue;
            };
            if m_game.d_game.is_none() {
                Self::dat_set_make_split_set(m_game);
                continue;
            }
            let Some(game_meta) = game_meta.as_ref() else {
                continue;
            };
            let parent_indices = Self::find_parent_sets_for_game(
                &game_name,
                game_file_type,
                game_meta,
                &snapshot,
                true,
            );
            if parent_indices.is_empty() {
                continue;
            }
            for child in &mut m_game.children {
                let Some(f) = child.file_mut() else { continue };
                if f.status.as_deref() == Some("nodump") {
                    continue;
                }
                if Self::find_rom_in_parent(f, &parent_indices, &snapshot) {
                    if f.merge.as_ref().is_none_or(|m| m.is_empty()) {
                        f.merge = Some("(Auto Merged)".to_string());
                    }
                    child.dat_status = DatStatus::InDatMerged;
                }
            }
        }
    }

    pub fn dat_set_make_merge_set(t_dat: &mut DatDir, merge_with_game_name: bool) {
        let len = t_dat.children.len();
        let mut moves: Vec<(usize, Vec<DatNode>)> = Vec::new();

        for i in 0..len {
            if !t_dat.children[i].is_dir() {
                continue;
            }

            let is_game = t_dat.children[i].dir().is_some_and(|d| d.d_game.is_some());
            if !is_game {
                if let Some(d) = t_dat.children[i].dir_mut() {
                    Self::dat_set_make_merge_set(d, merge_with_game_name);
                }
                continue;
            }

            let game_name = t_dat.children[i].name.clone();
            let game_file_type = t_dat.children[i].file_type;
            let game_meta = match t_dat.children[i]
                .dir()
                .and_then(|d| d.d_game.as_deref())
                .cloned()
            {
                Some(v) => v,
                None => continue,
            };

            let parent_indices = Self::find_parent_sets_for_game(
                &game_name,
                game_file_type,
                &game_meta,
                t_dat,
                true,
            );
            if parent_indices.is_empty() {
                continue;
            }

            let mut p_games = Vec::new();
            let mut p_bios = Vec::new();
            for idx in &parent_indices {
                let Some(p) = t_dat.children.get(*idx).and_then(|n| n.dir()) else {
                    continue;
                };
                let is_bios = p
                    .d_game
                    .as_ref()
                    .and_then(|g| g.is_bios.as_ref())
                    .is_some_and(|v| v.eq_ignore_ascii_case("yes"));
                if is_bios {
                    p_bios.push(*idx);
                } else {
                    p_games.push(*idx);
                }
            }

            let children_snapshot = t_dat.children[i]
                .dir()
                .map(|d| d.children.clone())
                .unwrap_or_default();

            let mut keep = Vec::new();
            for child in children_snapshot {
                let Some(f) = child.file() else { continue };
                if f.status.as_deref() == Some("nodump") {
                    keep.push(child);
                    continue;
                }
                if !Self::find_rom_in_parent(f, &p_bios, t_dat) {
                    keep.push(child);
                }
            }

            if p_games.is_empty() {
                if let Some(d) = t_dat.children[i].dir_mut() {
                    d.children = keep;
                }
                continue;
            }

            if let Some(d) = t_dat.children[i].dir_mut() {
                d.children.clear();
            }

            let top_parent_idx = *p_games.last().unwrap();
            let mut to_move = Vec::new();
            for mut child in keep {
                if merge_with_game_name {
                    if let Some(f) = child.file() {
                        if !f.is_disk {
                            child.name = format!("{}/{}", game_name, child.name);
                        }
                    }
                }
                to_move.push(child);
            }
            moves.push((top_parent_idx, to_move));
        }

        for (parent_idx, files) in moves {
            if let Some(parent_game) = t_dat.children.get_mut(parent_idx).and_then(|n| n.dir_mut())
            {
                for file in files {
                    parent_game.add_child(file);
                }
            }
        }
    }

    pub fn dat_set_make_non_merge_set(t_dat: &mut DatDir) {
        let snapshot = t_dat.clone();
        let len = t_dat.children.len();

        for i in 0..len {
            if !t_dat.children[i].is_dir() {
                continue;
            }

            let is_game = t_dat.children[i].dir().is_some_and(|d| d.d_game.is_some());
            if !is_game {
                if let Some(d) = t_dat.children[i].dir_mut() {
                    Self::dat_set_make_non_merge_set(d);
                }
                continue;
            }

            let device_refs = snapshot.children[i]
                .dir()
                .and_then(|d| d.d_game.as_ref())
                .map(|g| g.device_ref.clone())
                .unwrap_or_default();
            if device_refs.is_empty() {
                continue;
            }

            let mut devices = Vec::<usize>::new();
            let mut seen = std::collections::HashSet::<usize>::new();
            for dev in device_refs {
                Self::add_device(&dev, &snapshot, &mut devices, &mut seen);
            }

            let existing_children = t_dat.children[i]
                .dir()
                .map(|d| d.children.clone())
                .unwrap_or_default();

            let mut additions = Vec::new();
            for dev_idx in devices {
                let Some(dev_dir) = snapshot.children.get(dev_idx).and_then(|n| n.dir()) else {
                    continue;
                };
                for dev_file in &dev_dir.children {
                    let Some(df0) = dev_file.file() else { continue };
                    let mut found = false;
                    for mg_child in &existing_children {
                        let Some(df1) = mg_child.file() else { continue };
                        if df0.sha1.is_some()
                            && df1.sha1.is_some()
                            && df0.sha1 == df1.sha1
                            && dev_file.name == mg_child.name
                        {
                            found = true;
                            break;
                        }
                    }
                    if !found {
                        additions.push(dev_file.clone());
                    }
                }
            }

            if let Some(m_game) = t_dat.children[i].dir_mut() {
                for a in additions {
                    m_game.add_child(a);
                }
            }
        }
    }

    fn add_device(
        device: &str,
        t_dat: &DatDir,
        devices: &mut Vec<usize>,
        seen: &mut std::collections::HashSet<usize>,
    ) {
        let mut index = None;
        for (i, node) in t_dat.children.iter().enumerate() {
            if node.is_dir() && node.name == device {
                index = Some(i);
                break;
            }
        }
        let Some(i) = index else { return };
        if !seen.insert(i) {
            return;
        }
        devices.push(i);
        let Some(dev_dir) = t_dat.children[i].dir() else {
            return;
        };
        let Some(game) = dev_dir.d_game.as_ref() else {
            return;
        };
        for child in &game.device_ref {
            Self::add_device(child, t_dat, devices, seen);
        }
    }

    pub fn remove_devices(t_dat: &mut DatDir) {
        let children = t_dat.take_children();
        for mut child in children {
            if let Some(m_game) = child.dir_mut() {
                if m_game.d_game.is_none() {
                    Self::remove_devices(m_game);
                    t_dat.add_child(child);
                } else {
                    if let Some(g) = m_game.d_game.as_ref() {
                        if g.is_device.as_deref() == Some("yes")
                            && g.runnable.as_deref() == Some("no")
                        {
                            continue;
                        }
                    }
                    t_dat.add_child(child);
                }
            }
        }
    }

    pub fn move_up_chds(in_dat: &mut DatNode) {
        let Some(root) = in_dat.dir_mut() else { return };
        let mut moves = Vec::new();
        let mut path = Vec::new();
        Self::collect_chd_moves(root, &mut path, &mut moves);
        moves.sort_by(|a, b| {
            let dl = b.zip_dir_path.len().cmp(&a.zip_dir_path.len());
            if dl != std::cmp::Ordering::Equal {
                return dl;
            }
            b.file_index.cmp(&a.file_index)
        });
        for m in moves {
            Self::apply_chd_move(root, &m);
        }
    }

    fn collect_chd_moves(dir: &DatDir, path: &mut Vec<usize>, moves: &mut Vec<ChdMove>) {
        for (i, node) in dir.children.iter().enumerate() {
            if let Some(child_dir) = node.dir() {
                path.push(i);
                Self::collect_chd_moves(child_dir, path, moves);
                path.pop();
            }
        }

        for (j, node) in dir.children.iter().enumerate() {
            if let Some(f) = node.file() {
                if node.file_type != FileType::File && f.is_disk {
                    let zip_dir_path = path.clone();
                    let target_dir_path = if zip_dir_path.len() >= 2 {
                        zip_dir_path[..zip_dir_path.len() - 2].to_vec()
                    } else {
                        Vec::new()
                    };
                    moves.push(ChdMove {
                        target_dir_path,
                        zip_dir_path,
                        file_index: j,
                    });
                }
            }
        }
    }

    fn get_dir_mut<'a>(root: &'a mut DatDir, path: &[usize]) -> Option<&'a mut DatDir> {
        let mut cur = root;
        for &idx in path {
            let node = cur.children.get_mut(idx)?;
            cur = node.dir_mut()?;
        }
        Some(cur)
    }

    fn apply_chd_move(root: &mut DatDir, m: &ChdMove) {
        let (zip_name, zip_game, file_node) = {
            let Some(zip_parent_path) = m.zip_dir_path.split_last().map(|(_, p)| p) else {
                return;
            };
            let Some(zip_parent) = Self::get_dir_mut(root, zip_parent_path) else {
                return;
            };
            let zip_index = *m.zip_dir_path.last().unwrap_or(&usize::MAX);
            let Some(zip_node) = zip_parent.children.get_mut(zip_index) else {
                return;
            };
            let zip_name = zip_node.name.clone();
            let zip_game = zip_node.dir().and_then(|d| d.d_game.clone());
            let Some(zip_dir) = zip_node.dir_mut() else {
                return;
            };
            if m.file_index >= zip_dir.children.len() {
                return;
            }
            let mut file_node = zip_dir.children.remove(m.file_index);
            file_node.file_type = FileType::File;
            (zip_name, zip_game, file_node)
        };

        let Some(target_dir) = Self::get_dir_mut(root, &m.target_dir_path) else {
            return;
        };
        let existing_idx = target_dir
            .children
            .iter()
            .position(|n| n.is_dir() && n.file_type == FileType::Dir && n.name == zip_name);

        let idx = if let Some(i) = existing_idx {
            i
        } else {
            let mut tmp = DatNode::new_dir(zip_name.clone(), FileType::Dir);
            if let Some(d) = tmp.dir_mut() {
                d.d_game = zip_game;
            }
            target_dir.children.push(tmp);
            target_dir.children.len() - 1
        };

        if let Some(d) = target_dir.children[idx].dir_mut() {
            d.add_child(file_node);
        }
    }
}
