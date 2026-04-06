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

include!("directory_ops.rs");
include!("metadata_ops.rs");
include!("compression_ops.rs");
include!("game_set_ops.rs");
include!("filename_ops.rs");
include!("single_level_ops.rs");

impl DatClean {
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
        let children = d_dir.take_children();
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
        let children = d_dir.take_children();
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
            let children = m_game.take_children();
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
            let children = m_game.take_children();
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
            let children = m_game.take_children();
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
        let children = t_dat.take_children();
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
        let children = t_dat.take_children();
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
