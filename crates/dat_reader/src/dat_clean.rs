use crate::dat_store::{DatDir, DatNode};
use crate::enums::{DatStatus, FileType, HeaderFileType, ZipStructure};
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RemoveSubType {
    KeepAllSubDirs,
    RemoveAllSubDirs,
    RemoveAllIfNoConflicts,
    RemoveSubIfSingleFiles,
    RemoveSubIfNameMatches,
}

pub struct DatClean;

#[derive(Debug, Clone)]
struct ChdMove {
    target_dir_path: Vec<usize>,
    zip_dir_path: Vec<usize>,
    file_index: usize,
}

impl DatClean {
    pub fn directory_flatten(d_dir: &mut DatDir) {
        let mut list = Vec::new();
        Self::directory_flat(d_dir, &mut list, "");
        d_dir.children.clear();
        d_dir.children.extend(list);
    }

    fn directory_flat(d_dir: &mut DatDir, new_dir: &mut Vec<DatNode>, sub_dir: &str) {
        let mut children = Vec::new();
        children.append(&mut d_dir.children);
        for mut node in children {
            let is_game = node.dir().is_some_and(|d| d.d_game.is_some());
            if is_game {
                node.name = if sub_dir.is_empty() {
                    node.name
                } else {
                    format!("{}\\{}", sub_dir, node.name)
                };
                new_dir.push(node);
            } else {
                let next_name = if sub_dir.is_empty() {
                    node.name.clone()
                } else {
                    format!("{}\\{}", sub_dir, node.name)
                };
                if let Some(child_dir) = node.dir_mut() {
                    Self::directory_flat(child_dir, new_dir, &next_name);
                }
            }
        }
    }

    pub fn archive_directory_flatten(d_dir: &mut DatDir) {
        if d_dir.d_game.is_some() {
            let mut list = Vec::new();
            Self::archive_flat(d_dir, &mut list, "");
            d_dir.children.clear();
            d_dir.children.extend(list);
            return;
        }
        for node in &mut d_dir.children {
            if let Some(dat_dir) = node.dir_mut() {
                Self::archive_directory_flatten(dat_dir);
            }
        }
    }

    fn archive_flat(d_dir: &DatDir, new_dir: &mut Vec<DatNode>, sub_dir: &str) {
        for node in &d_dir.children {
            let this_name = if sub_dir.is_empty() {
                node.name.to_string()
            } else {
                format!("{}/{}", sub_dir, node.name)
            };
            if node.file().is_some() {
                let mut new_node = node.clone();
                new_node.name = this_name;
                new_dir.push(new_node);
                continue;
            }
            let Some(child_dir) = node.dir() else {
                continue;
            };
            let mut dir_marker = DatNode::new_file(format!("{}/", this_name), FileType::UnSet);
            if let Some(f) = dir_marker.file_mut() {
                f.size = Some(0);
                f.crc = Some(vec![0, 0, 0, 0]);
            }
            new_dir.push(dir_marker);
            Self::archive_flat(child_dir, new_dir, &this_name);
        }
    }

    pub fn directory_sort(d_dir: &mut DatDir) {
        d_dir
            .children
            .sort_by(|a, b| Self::alphanum_cmp(&a.name, &b.name).cmp(&0));
        for node in &mut d_dir.children {
            if let Some(dat_dir) = node.dir_mut() {
                Self::directory_sort(dat_dir);
            }
        }
    }

    pub fn directory_expand(d_dir: &mut DatDir) {
        loop {
            let found = d_dir
                .children
                .iter()
                .any(|n| Self::check_dir(n.file_type) && n.name.contains('/'));
            if !found {
                break;
            }

            let mut old = Vec::new();
            old.append(&mut d_dir.children);
            for mut node in old {
                if Self::check_dir(node.file_type) && node.name.contains('/') {
                    let split = node.name.find('/').unwrap_or(0);
                    let part0 = node.name[..split].to_string();
                    let part1 = node.name[split + 1..].to_string();
                    node.name = part1;

                    let dir_index = d_dir
                        .children
                        .iter()
                        .position(|c| c.is_dir() && c.name == part0);
                    let dir_node = if let Some(i) = dir_index {
                        &mut d_dir.children[i]
                    } else {
                        d_dir
                            .children
                            .push(DatNode::new_dir(part0.clone(), FileType::Dir));
                        d_dir.children.last_mut().unwrap()
                    };
                    if !node.name.is_empty() {
                        dir_node.dir_mut().unwrap().add_child(node);
                    }
                } else {
                    d_dir.children.push(node);
                }
            }
        }

        for node in &mut d_dir.children {
            if let Some(dat_dir) = node.dir_mut() {
                Self::directory_expand(dat_dir);
            }
        }
    }

    pub fn remove_device_ref(d_dir: &mut DatDir) {
        for node in &mut d_dir.children {
            if let Some(ddir) = node.dir_mut() {
                if let Some(game) = ddir.d_game.as_mut() {
                    game.device_ref.clear();
                }
                Self::remove_device_ref(ddir);
            }
        }
    }

    pub fn check_deduped(d_dir: &mut DatDir) {
        for node in &mut d_dir.children {
            if let Some(file) = node.file_mut() {
                if file
                    .status
                    .as_deref()
                    .is_some_and(|s| s.eq_ignore_ascii_case("deduped"))
                {
                    node.dat_status = DatStatus::InDatMerged;
                }
            }
            if let Some(ddir) = node.dir_mut() {
                Self::check_deduped(ddir);
            }
        }
    }

    pub fn add_category(t_dat: &mut DatDir, cat_order: &[String]) {
        for node in &mut t_dat.children {
            let Some(m_game) = node.dir_mut() else {
                continue;
            };
            if m_game.d_game.is_none() {
                Self::add_category(m_game, cat_order);
                continue;
            }
            if let Some(cat) = Self::find_category(m_game, cat_order) {
                node.name = format!("{}/{}", cat, node.name);
            }
        }
    }

    pub fn find_category(m_game: &DatDir, cat_order: &[String]) -> Option<String> {
        let game = m_game.d_game.as_ref()?;
        if game.category.is_empty() {
            return None;
        }
        if game.category.len() == 1 {
            let c = game.category[0].trim();
            if !c.is_empty() {
                return Some(c.to_string());
            }
            return None;
        }

        let mut best = None::<usize>;
        for cat in &game.category {
            let c = cat.trim();
            if c.is_empty() {
                continue;
            }
            for (i, order) in cat_order.iter().enumerate() {
                if order.eq_ignore_ascii_case(c) {
                    if best.is_none_or(|b| i < b) {
                        best = Some(i);
                    }
                    break;
                }
            }
        }
        best.map(|i| cat_order[i].clone())
    }

    pub fn set_ext(t_dat: &mut DatDir, header_file_type: HeaderFileType) {
        let mut children = Vec::new();
        children.append(&mut t_dat.children);
        for mut node in children {
            match node.node {
                crate::dat_store::DatBase::File(_) => {
                    let is_disk = node.file().is_some_and(|f| f.is_disk);
                    if is_disk {
                        node.name.push_str(".chd");
                    }
                    if let Some(f) = node.file_mut() {
                        if is_disk {
                            f.header_file_type = HeaderFileType::CHD;
                            if let Some(merge) = f.merge.as_mut() {
                                merge.push_str(".chd");
                            }
                        } else {
                            f.header_file_type = header_file_type;
                        }
                    }
                    t_dat.add_child(node);
                }
                crate::dat_store::DatBase::Dir(_) => {
                    let ext = Self::get_ext(node.file_type);
                    node.name.push_str(ext);
                    if let Some(d) = node.dir_mut() {
                        Self::set_ext(d, header_file_type);
                    }
                    t_dat.add_child(node);
                }
            }
        }
    }

    fn get_ext(dft: FileType) -> &'static str {
        match dft {
            FileType::Zip => ".zip",
            FileType::SevenZip => ".7z",
            _ => "",
        }
    }

    pub fn set_compression_type(
        in_dat: &mut DatNode,
        file_type: FileType,
        zs: ZipStructure,
        fix: bool,
    ) {
        if in_dat.file().is_some() {
            in_dat.file_type = Self::file_type_from_dir(file_type);
            return;
        }
        let had_game = in_dat.dir().is_some_and(|d| d.d_game.is_some());
        let existing_type = in_dat.file_type;

        let mut effective_file_type = if !had_game || file_type == FileType::Dir {
            FileType::Dir
        } else {
            file_type
        };
        let mut effective_zs = zs;

        if had_game && file_type != FileType::Dir && existing_type != FileType::UnSet {
            match existing_type {
                FileType::Dir => {
                    effective_file_type = FileType::Dir;
                    effective_zs = ZipStructure::None;
                }
                FileType::Zip => {
                    effective_file_type = FileType::Zip;
                    effective_zs = ZipStructure::ZipTrrnt;
                }
                FileType::SevenZip => {
                    effective_file_type = FileType::SevenZip;
                    effective_zs = ZipStructure::SevenZipNZSTD;
                }
                _ => {}
            }
        }

        in_dat.file_type = effective_file_type;

        if let Some(d_dir) = in_dat.dir_mut() {
            if had_game && file_type != FileType::Dir && effective_file_type != FileType::Dir {
                let checked =
                    if Self::is_trrntzip_date_times(d_dir, effective_zs, effective_file_type) {
                        ZipStructure::ZipTrrnt
                    } else {
                        effective_zs
                    };
                d_dir.set_dat_struct(checked, fix);
            }

            let children = std::mem::take(&mut d_dir.children);
            for mut child in children {
                Self::set_compression_type(&mut child, file_type, zs, fix);
                d_dir.add_child(child);
            }
        }
    }

    fn file_type_from_dir(file_type: FileType) -> FileType {
        match file_type {
            FileType::UnSet => FileType::UnSet,
            FileType::Dir => FileType::File,
            FileType::Zip => FileType::FileZip,
            FileType::SevenZip => FileType::FileSevenZip,
            _ => FileType::File,
        }
    }

    fn is_trrntzip_date_times(d_dir: &DatDir, zs: ZipStructure, file_type: FileType) -> bool {
        if file_type != FileType::Zip || zs != ZipStructure::ZipTDC {
            return false;
        }
        for child in &d_dir.children {
            if child.file().is_some() {
                if child.date_modified != Some(crate::dat_store::TRRNTZIP_DOS_DATETIME) {
                    return false;
                }
            } else {
                return false;
            }
        }
        true
    }

    pub fn set_status(t_dat: &mut DatDir) {
        for node in &mut t_dat.children {
            if let Some(m_dir) = node.dir_mut() {
                Self::set_status(m_dir);
                continue;
            }
            if let Some(m_file) = node.file_mut() {
                let _ = m_file;
                Self::rom_check_collect(node);
            }
        }
    }

    fn rom_check_collect(node: &mut DatNode) {
        if node.dat_status == DatStatus::InDatMerged {
            return;
        }
        let Some(t_rom) = node.file_mut() else { return };

        if let Some(merge) = t_rom.merge.as_mut() {
            if !merge.is_empty() {
                *merge = format!("(No-Merge) {}", merge);
            }
        }

        if t_rom.status.as_deref() == Some("nodump") {
            node.dat_status = DatStatus::InDatNoDump;
            return;
        }
        if t_rom
            .mia
            .as_deref()
            .is_some_and(|m| m.eq_ignore_ascii_case("yes"))
            && t_rom.size.unwrap_or(0) != 0
        {
            node.dat_status = DatStatus::InDatMIA;
            return;
        }

        let crc_is_zero = t_rom
            .crc
            .as_ref()
            .is_some_and(|c| c.len() == 4 && c.iter().all(|b| *b == 0));
        if crc_is_zero && t_rom.size.unwrap_or(0) == 0 {
            node.dat_status = DatStatus::InDatCollect;
            return;
        }

        node.dat_status = DatStatus::InDatCollect;
    }

    pub fn clear_description(d_dir: &mut DatDir) {
        for node in &mut d_dir.children {
            let stem = std::path::Path::new(&node.name)
                .file_stem()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();
            let Some(ddir) = node.dir_mut() else { continue };
            if let Some(game) = ddir.d_game.as_mut() {
                if let Some(desc) = game.description.as_mut() {
                    if stem == *desc {
                        *desc = "¤".to_string();
                    }
                }
                continue;
            }
            Self::clear_description(ddir);
        }
    }

    pub fn dat_set_add_id_numbers(t_dat: &mut DatDir, id: &str) {
        let mut current_id = id.to_string();
        for node in &mut t_dat.children {
            if let Some(m_game) = node.dir() {
                if let Some(game) = m_game.d_game.as_ref() {
                    if let Some(id2) = game.id.as_ref() {
                        current_id = id2.clone();
                    }
                }
            }
            node.name = format!("{} - {}", current_id, node.name);
            if let Some(m_game) = node.dir_mut() {
                Self::dat_set_add_id_numbers(m_game, &current_id);
            }
        }
    }

    pub fn dat_set_match_ids(t_dat: &mut DatDir) {
        let mut lookup = HashMap::<String, String>::new();
        for node in &t_dat.children {
            let Some(m_game) = node.dir() else { continue };
            let Some(game) = m_game.d_game.as_ref() else {
                continue;
            };
            if let Some(id) = game.id.as_ref() {
                if !id.is_empty() {
                    lookup
                        .entry(id.clone())
                        .or_insert_with(|| node.name.clone());
                }
            }
        }
        if lookup.is_empty() {
            return;
        }
        for node in &mut t_dat.children {
            let Some(m_game) = node.dir_mut() else {
                continue;
            };
            let Some(game) = m_game.d_game.as_mut() else {
                continue;
            };
            let Some(clone_id) = game.clone_of_id.as_ref() else {
                continue;
            };
            if let Some(name) = lookup.get(clone_id) {
                game.clone_of = Some(name.clone());
            }
        }
    }

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
        let children = std::mem::take(&mut t_dat.children);
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

    pub fn remove_date_time(d_dir: &mut DatDir) {
        for node in &mut d_dir.children {
            if node.file().is_some() {
                node.date_modified = None;
            }
            if let Some(ddir) = node.dir_mut() {
                Self::remove_date_time(ddir);
            }
        }
    }

    pub fn remove_md5(d_dir: &mut DatDir) {
        for node in &mut d_dir.children {
            if let Some(df) = node.file_mut() {
                df.md5 = None;
            }
            if let Some(ddir) = node.dir_mut() {
                Self::remove_md5(ddir);
            }
        }
    }

    pub fn remove_sha256(d_dir: &mut DatDir) {
        for node in &mut d_dir.children {
            if let Some(df) = node.file_mut() {
                df.sha256 = None;
            }
            if let Some(ddir) = node.dir_mut() {
                Self::remove_sha256(ddir);
            }
        }
    }

    pub fn clean_filenames(d_dir: &mut DatDir) {
        for node in &mut d_dir.children {
            Self::clean_filename(node);
            if let Some(ddir) = node.dir_mut() {
                Self::clean_filenames(ddir);
            }
        }
    }

    pub fn clean_file_names_full(in_dat: &mut DatNode) {
        let Some(d_dir) = in_dat.dir_mut() else {
            return;
        };
        let mut children = Vec::new();
        children.append(&mut d_dir.children);

        for mut child in children {
            let original = child.name.clone();
            match child.file_type {
                FileType::UnSet | FileType::File | FileType::Dir => {
                    child.name = child.name.trim_start_matches(' ').to_string();
                    child.name = child.name.trim_end_matches(['.', ' ']).to_string();
                }
                FileType::Zip | FileType::SevenZip => {
                    child.name = child.name.trim_start_matches(' ').to_string();
                    child.name = child.name.trim_end_matches(' ').to_string();
                }
                _ => {}
            }
            if child.name.trim().is_empty() {
                child.name = "_".to_string();
            }

            if child.is_dir() && child.file_type == FileType::Dir {
                Self::clean_file_names_full(&mut child);
            }

            let _ = original;
            d_dir.add_child(child);
        }
    }

    pub fn fix_dupes(d_dir: &mut DatDir) {
        d_dir.children.sort_by(|a, b| {
            let t = a.file_type.cmp(&b.file_type);
            if t != std::cmp::Ordering::Equal {
                return t;
            }
            let la = a.name.to_ascii_lowercase();
            let lb = b.name.to_ascii_lowercase();
            la.cmp(&lb).then(a.name.cmp(&b.name))
        });

        let mut last_name = String::new();
        let mut last_type = FileType::UnSet;
        let mut match_count = 0usize;

        for node in &mut d_dir.children {
            let this_name = node.name.clone();
            let ft = node.file_type;

            if ft == last_type && last_name.eq_ignore_ascii_case(&this_name) {
                match ft {
                    FileType::Dir | FileType::Zip | FileType::SevenZip => {
                        node.name = format!("{}_{}", this_name, match_count);
                    }
                    _ => {
                        let ext = std::path::Path::new(&this_name)
                            .extension()
                            .map(|e| format!(".{}", e.to_string_lossy()))
                            .unwrap_or_default();
                        let stem = if ext.is_empty() {
                            this_name.clone()
                        } else {
                            this_name[..this_name.len().saturating_sub(ext.len())].to_string()
                        };
                        node.name = format!("{}_{}{}", stem, match_count, ext);
                    }
                }
                match_count += 1;
            } else {
                match_count = 0;
                last_name = this_name;
                last_type = ft;
            }

            if let Some(ddir) = node.dir_mut() {
                Self::fix_dupes(ddir);
            }
        }
    }

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
        mut rom: DatNode,
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

        rom.name = format!("{}/{}", set_name, rom.name);
        out_dir.add_child(rom);
    }

    pub fn remove_dupes(t_dat: &mut DatDir, test_name: bool, test_with_merge_name: bool) {
        let len = t_dat.children.len();
        for i in 0..len {
            if let Some(m_game) = t_dat.children[i].dir_mut() {
                if m_game.d_game.is_none() {
                    Self::remove_dupes(m_game, test_name, test_with_merge_name);
                    continue;
                }

                let mut found = true;
                while found {
                    found = false;
                    let mut r = 0usize;
                    while r < m_game.children.len() {
                        let mut t = r + 1;
                        while t < m_game.children.len() {
                            let (df0, df1) =
                                match (m_game.children[r].file(), m_game.children[t].file()) {
                                    (Some(a), Some(b)) => (a, b),
                                    _ => {
                                        t += 1;
                                        continue;
                                    }
                                };
                            if test_name && m_game.children[r].name != m_game.children[t].name {
                                t += 1;
                                continue;
                            }
                            let has_crc = df0.crc.is_some() && df1.crc.is_some();
                            if has_crc && df0.crc != df1.crc {
                                t += 1;
                                continue;
                            }
                            let has_sha1 = df0.sha1.is_some() && df1.sha1.is_some();
                            if has_sha1 && df0.sha1 != df1.sha1 {
                                t += 1;
                                continue;
                            }
                            let has_sha256 = df0.sha256.is_some() && df1.sha256.is_some();
                            if has_sha256 && df0.sha256 != df1.sha256 {
                                t += 1;
                                continue;
                            }
                            let has_md5 = df0.md5.is_some() && df1.md5.is_some();
                            if has_md5 && df0.md5 != df1.md5 {
                                t += 1;
                                continue;
                            }
                            if !has_crc && !has_sha1 && !has_md5 {
                                t += 1;
                                continue;
                            }

                            found = true;

                            let name0 = m_game.children[r].name.clone();
                            let name1 = m_game.children[t].name.clone();

                            let ns0 = name0.contains('/');
                            let ns1 = name1.contains('/');

                            let remove_index = if ns0 && !ns1 {
                                r
                            } else if !ns0 && ns1 {
                                t
                            } else if ns0 && ns1 {
                                let s0 = name0.split('/').next().unwrap_or("").to_string();
                                let s1 = name1.split('/').next().unwrap_or("").to_string();
                                if s0 != s1 {
                                    t
                                } else {
                                    let res = Self::alphanum_cmp(&name0, &name1);
                                    if res >= 0 {
                                        r
                                    } else {
                                        t
                                    }
                                }
                            } else {
                                let merge1 = df1.merge.clone().unwrap_or_default();
                                if name0 == name1 || (test_with_merge_name && name0 == merge1) {
                                    t
                                } else {
                                    found = false;
                                    t += 1;
                                    continue;
                                }
                            };

                            m_game.children.remove(remove_index);
                            r = m_game.children.len();
                            t = m_game.children.len();
                        }
                        r += 1;
                    }
                }
            }
        }
    }

    pub fn remove_empty_sets(in_dat: &mut DatNode) -> bool {
        if in_dat.file().is_some() {
            return true;
        }
        let Some(d_dir) = in_dat.dir_mut() else {
            return false;
        };
        if d_dir.children.is_empty() {
            return false;
        }
        let children = std::mem::take(&mut d_dir.children);
        let mut found = false;
        for mut child in children {
            if Self::remove_empty_sets(&mut child) {
                found = true;
                d_dir.add_child(child);
            }
        }
        found
    }

    pub fn remove_not_collected(in_dat: &mut DatNode) -> bool {
        if in_dat.file().is_some() {
            return matches!(
                in_dat.dat_status,
                DatStatus::InDatCollect | DatStatus::InDatNoDump
            );
        }
        let Some(d_dir) = in_dat.dir_mut() else {
            return false;
        };
        if d_dir.children.is_empty() {
            return false;
        }
        let children = std::mem::take(&mut d_dir.children);
        let mut found = false;
        for mut child in children {
            if Self::remove_not_collected(&mut child) {
                found = true;
                d_dir.add_child(child);
            }
        }
        found
    }

    pub fn remove_no_dumps(t_dat: &mut DatDir) {
        for node in &mut t_dat.children {
            let Some(m_game) = node.dir_mut() else {
                continue;
            };
            if m_game.d_game.is_none() {
                Self::remove_no_dumps(m_game);
                continue;
            }
            let children = std::mem::take(&mut m_game.children);
            for child in children {
                let remove = child
                    .file()
                    .and_then(|f| f.status.as_deref())
                    .is_some_and(|s| s == "nodump");
                if !remove {
                    m_game.add_child(child);
                }
            }
        }
    }

    pub fn remove_chd(t_dat: &mut DatDir) {
        for node in &mut t_dat.children {
            let Some(m_game) = node.dir_mut() else {
                continue;
            };
            if m_game.d_game.is_none() {
                Self::remove_chd(m_game);
                continue;
            }
            let children = std::mem::take(&mut m_game.children);
            for child in children {
                let remove = child.file().is_some_and(|f| f.is_disk);
                if !remove {
                    m_game.add_child(child);
                }
            }
        }
    }

    pub fn remove_non_chd(t_dat: &mut DatDir) {
        for node in &mut t_dat.children {
            let Some(m_game) = node.dir_mut() else {
                continue;
            };
            if m_game.d_game.is_none() {
                Self::remove_non_chd(m_game);
                continue;
            }
            let children = std::mem::take(&mut m_game.children);
            for child in children {
                let remove = child.file().is_some_and(|f| !f.is_disk);
                if !remove {
                    m_game.add_child(child);
                }
            }
        }
    }

    pub fn remove_all_date_time(t_dat: &mut DatDir) {
        for node in &mut t_dat.children {
            node.date_modified = None;
            if let Some(m_game) = node.dir_mut() {
                if m_game.dat_struct() == ZipStructure::ZipTDC {
                    continue;
                }
                Self::remove_all_date_time(m_game);
            }
        }
    }

    pub fn remove_unneeded_directories(t_dat: &mut DatDir) {
        for node in &mut t_dat.children {
            let file_type = node.file_type;
            let is_container = node.dir().is_some_and(|d| d.d_game.is_none());
            let dat_struct = node
                .dir()
                .map(|d| d.dat_struct())
                .unwrap_or(ZipStructure::None);

            let Some(m_game) = node.dir_mut() else {
                continue;
            };
            if file_type == FileType::Dir || (file_type == FileType::UnSet && is_container) {
                Self::remove_unneeded_directories(m_game);
                continue;
            }
            if dat_struct == ZipStructure::ZipTDC {
                continue;
            }
            Self::remove_unneeded_directories_from_zip(m_game);
        }
    }

    pub fn remove_unneeded_directories_from_zip(m_game: &mut DatDir) {
        let mut i = 0usize;
        while i < m_game.children.len() {
            let is_dir_marker = m_game.children[i]
                .file()
                .is_some_and(|f| f.size.unwrap_or(0) == 0)
                && m_game.children[i].name.ends_with('/')
                && !m_game.children[i].name.is_empty();
            if !is_dir_marker {
                i += 1;
                continue;
            }

            let dir_name = m_game.children[i].name.clone();
            let mut found = false;
            for j in 0..m_game.children.len() {
                if i == j {
                    continue;
                }
                let comp_name = &m_game.children[j].name;
                if comp_name.len() <= dir_name.len() {
                    continue;
                }
                if comp_name.starts_with(&dir_name) {
                    found = true;
                    break;
                }
            }
            if found {
                m_game.children.remove(i);
                continue;
            }
            i += 1;
        }
    }

    pub fn remove_files_not_in_games(t_dat: &mut DatDir) {
        let children = std::mem::take(&mut t_dat.children);
        for mut child in children {
            if child.file().is_some() {
                continue;
            }
            if let Some(dat_dir) = child.dir_mut() {
                if dat_dir.d_game.is_none() {
                    Self::remove_files_not_in_games(dat_dir);
                }
            }
            t_dat.add_child(child);
        }
    }

    pub fn remove_empty_directories(t_dat: &mut DatDir) {
        let children = std::mem::take(&mut t_dat.children);
        for mut child in children {
            let Some(dat_dir) = child.dir_mut() else {
                t_dat.add_child(child);
                continue;
            };
            if dat_dir.d_game.is_none() {
                Self::remove_empty_directories(dat_dir);
                t_dat.add_child(child);
                continue;
            }
            if dat_dir.children.is_empty() {
                continue;
            }
            t_dat.add_child(child);
        }
    }

    fn clean_filename(node: &mut DatNode) {
        if node.name.is_empty() {
            return;
        }
        let mut ret = node.name.replace('\\', "/").replace("./", "/");
        ret = ret.replace("./", "/");
        let mut chars: Vec<char> = ret.chars().collect();
        for c in &mut chars {
            let v = *c as u32;
            if matches!(*c, ':' | '*' | '?' | '<' | '>' | '|' | '"') || v < 32 {
                *c = '-';
            }
        }
        node.name = chars.into_iter().collect();
    }

    fn check_dir(file_type: FileType) -> bool {
        !matches!(file_type, FileType::FileZip | FileType::FileSevenZip)
    }

    pub fn alphanum_cmp(s1: &str, s2: &str) -> i32 {
        if s1.is_empty() || s2.is_empty() {
            return 0;
        }

        let ns1 = s1.contains('\\');
        let ns2 = s2.contains('\\');
        if ns1 && !ns2 {
            return -1;
        }
        if ns2 && !ns1 {
            return 1;
        }

        let mut a = s1;
        let mut b = s2;
        if ns1 && ns2 {
            let p1 = a.find('\\').unwrap();
            let p2 = b.find('\\').unwrap();
            let mut ts1 = &a[..p1];
            let mut ts2 = &b[..p2];
            if ts1 == ts2 {
                ts1 = &a[p1 + 1..];
                ts2 = &b[p2 + 1..];
            }
            a = ts1;
            b = ts2;
        }

        let bytes_a = a.as_bytes();
        let bytes_b = b.as_bytes();
        let mut i = 0usize;
        let mut j = 0usize;
        while i < bytes_a.len() && j < bytes_b.len() {
            let is_digit_a = bytes_a[i].is_ascii_digit();
            let is_digit_b = bytes_b[j].is_ascii_digit();

            let start_i = i;
            while i < bytes_a.len() && bytes_a[i].is_ascii_digit() == is_digit_a {
                i += 1;
            }
            let start_j = j;
            while j < bytes_b.len() && bytes_b[j].is_ascii_digit() == is_digit_b {
                j += 1;
            }

            let chunk_a = &a[start_i..i];
            let chunk_b = &b[start_j..j];

            let result = if is_digit_a && is_digit_b {
                let na = chunk_a.parse::<u64>().unwrap_or(0);
                let nb = chunk_b.parse::<u64>().unwrap_or(0);
                let cmp = na.cmp(&nb);
                if cmp == std::cmp::Ordering::Equal && chunk_a.len() != chunk_b.len() {
                    (chunk_a.len() as i32) - (chunk_b.len() as i32)
                } else {
                    match cmp {
                        std::cmp::Ordering::Less => -1,
                        std::cmp::Ordering::Equal => 0,
                        std::cmp::Ordering::Greater => 1,
                    }
                }
            } else {
                let ca = chunk_a.to_ascii_lowercase();
                let cb = chunk_b.to_ascii_lowercase();
                match ca.cmp(&cb) {
                    std::cmp::Ordering::Less => -1,
                    std::cmp::Ordering::Equal => 0,
                    std::cmp::Ordering::Greater => 1,
                }
            };

            if result != 0 {
                return result;
            }
        }
        (b.len() as i32) - (a.len() as i32)
    }
}
