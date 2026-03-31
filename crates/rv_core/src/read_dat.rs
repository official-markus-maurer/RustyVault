use std::fs;
use std::path::Path;
use std::rc::Rc;
use std::cell::RefCell;
use crate::rv_file::{FileStatus, RvFile};
use dat_reader::read_dat;
use dat_reader::enums::{FileType, HeaderFileType};
use crate::rv_dat::{RvDat, DatData};
use dat_reader::dat_store::{DatHeader, DatNode};
use crate::rv_game::RvGame;
use dat_reader::enums::DatStatus;
use rayon::prelude::*;

/// Central engine for reading DAT files and integrating them into the `DB` tree.
/// 
/// `DatUpdate` reads the physical `.dat` / `.xml` files residing in the `DatRoot` folder,
/// parses them using `dat_reader`, and translates the resulting `DatNode` hierarchies into
/// `RvFile` nodes within the `dir_root` DB tree.
/// 
/// Differences from C#:
/// - The C# implementation uses background workers for XML parsing but integrates into the UI thread synchronously.
/// - The Rust version implements highly scalable parallelization via `rayon`. It first reads and parses 
///   ALL `.dat` files simultaneously in parallel (since XML/CMP parsing is CPU bound and independent).
///   It then sequentially integrates the parsed ASTs into the `Rc<RefCell<RvFile>>` tree, dramatically
///   reducing the "Update DATs" time for large `DatRoot` setups.
pub struct DatUpdate;

impl DatUpdate {
    const PRESERVED_PHYSICAL_FLAGS: FileStatus = FileStatus::SIZE_FROM_HEADER
        .union(FileStatus::CRC_FROM_HEADER)
        .union(FileStatus::SHA1_FROM_HEADER)
        .union(FileStatus::MD5_FROM_HEADER)
        .union(FileStatus::ALT_SIZE_FROM_HEADER)
        .union(FileStatus::ALT_CRC_FROM_HEADER)
        .union(FileStatus::ALT_SHA1_FROM_HEADER)
        .union(FileStatus::ALT_MD5_FROM_HEADER)
        .union(FileStatus::HEADER_FILE_TYPE_FROM_HEADER);

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
            child_ref.name == name && child_ref.file_type == file_type
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
        if existing.chd_version.is_some() {
            new_rv.chd_version = existing.chd_version;
        }
        if existing.zip_struct != dat_reader::enums::ZipStructure::None {
            new_rv.zip_struct = existing.zip_struct;
        }
        new_rv.file_status.remove(Self::PRESERVED_PHYSICAL_FLAGS);
        new_rv.file_status.insert(existing.file_status & Self::PRESERVED_PHYSICAL_FLAGS);

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

    fn dat_path_matches_filter(dat_full_name: &str, dat_path: &str) -> bool {
        dat_path.len() <= dat_full_name.len() && &dat_full_name[0..dat_path.len()] == dat_path
    }

    /// Recursively scans `dat_dir_path`, parses all found DATs in parallel, and merges them into `root`.
    pub fn update_dat(root: Rc<RefCell<RvFile>>, dat_dir_path: &str) {
        println!("Scanning for DATs in {}...", dat_dir_path);
        
        let mut dats_found = Vec::new();
        Self::scan_dat_dir(dat_dir_path, &mut dats_found);

        println!("Found {} DAT files.", dats_found.len());

        let romvault_dir = {
            let root_ref = root.borrow();
            root_ref.children.iter().find(|c| c.borrow().name == "RustyVault").cloned()
        };

        if let Some(rv_dir) = romvault_dir {
            // Use Rayon to read and parse the DAT files in parallel!
            // Parsing the XML/CMP is entirely CPU bound and independent of the DB state.
            let parsed_results: Vec<(String, String, Result<dat_reader::dat_store::DatHeader, String>)> = dats_found
                .into_par_iter()
                .map(|(dat_path, virtual_dir)| {
                    if let Ok(buffer) = fs::read(&dat_path) {
                        let file_name = Path::new(&dat_path).file_name().unwrap_or_default().to_string_lossy().into_owned();
                        (dat_path.clone(), virtual_dir.clone(), read_dat(&buffer, &file_name))
                    } else {
                        (dat_path.clone(), virtual_dir.clone(), Err("Could not read file from disk".to_string()))
                    }
                })
                .collect();

            // After all DATs are parsed in parallel, we sequentially integrate them into the DB tree
            // since the tree itself is single-threaded (Rc/RefCell).
            for (dat_path, virtual_dir, parse_result) in parsed_results {
                println!("Integrating DAT: {}", dat_path);
                match parse_result {
                    Ok(dat_header) => {
                        println!("Successfully parsed DAT: {:?}", dat_header.name);
                        
                        // 1. Create a new RvDat entry
        // 2. Find or create the directory for this DAT in RustyVault
                        let mut current_parent = Rc::clone(&rv_dir);
                        
                        // First, traverse the physical directory path from DatRoot
                        if !virtual_dir.is_empty() {
                            // Split virtual_dir by both separators to be safe
                            let parts: Vec<&str> = virtual_dir.split(|c| c == '/' || c == '\\').filter(|s| !s.is_empty()).collect();
                            for part in parts {
                                let mut found = None;
                                {
                                    let mut cp_mut = current_parent.borrow_mut();
                                    cp_mut.cached_stats = None;
                                    for child in &cp_mut.children {
                                        if child.borrow().name == part {
                                            found = Some(Rc::clone(child));
                                            break;
                                        }
                                    }
                                    if found.is_none() {
                                        let mut new_dir = RvFile::new(FileType::Dir);
                                        new_dir.name = part.to_string();
                                        // Virtual directories for organizing DATs should not be removed if they are missing on disk
                                        new_dir.set_dat_got_status(dat_reader::enums::DatStatus::InDatCollect, dat_reader::enums::GotStatus::NotGot);
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

                        // Now handle the actual DAT name directory
                        let dir_name = dat_header.name.clone().unwrap_or_else(|| "Unknown_DAT".to_string());
                        
                        let mut rv_dir_mut = current_parent.borrow_mut();
                        
                        // Invalidate the cache of the parent
                        rv_dir_mut.cached_stats = None;
                        let mut target_dir = None;
                        
                        for child in &rv_dir_mut.children {
                            if child.borrow().name == dir_name {
                                target_dir = Some(Rc::clone(child));
                                break;
                            }
                        }
                        
                        let (target_dir, existing_children) = match target_dir {
                            Some(d) => {
                                let existing_children = {
                                    let mut existing = d.borrow_mut();
                                    existing.cached_stats = None;
                                    std::mem::take(&mut existing.children)
                                };
                                (d, existing_children)
                            },
                            None => {
                                let mut new_dir = RvFile::new(FileType::Dir);
                                new_dir.name = dir_name;
                                new_dir.set_dat_got_status(dat_reader::enums::DatStatus::InDatCollect, dat_reader::enums::GotStatus::NotGot);
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
                        
                        // 3. Attach the parsed dat data into the tree and map recursively
                        {
                            let mut td = target_dir.borrow_mut();
                            rv_dat_rc.borrow_mut().dat_index = 0;
                            td.dir_dats.clear();
                            td.dir_dats.push(Rc::clone(&rv_dat_rc));
                        }
                        
                        // Recursive mapping
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
                    },
                    Err(e) => {
                        println!("Error reading DAT {}: {}", dat_path, e);
                    }
                }
            }
        }
    }

    fn map_dat_node_to_rv_file(
        parent: Rc<RefCell<RvFile>>,
        dat_node: &DatNode,
        dat_rc: Rc<RefCell<RvDat>>,
        existing_children: &mut Vec<Rc<RefCell<RvFile>>>,
    ) {
        let mut file_type = dat_node.file_type;
        if file_type == dat_reader::enums::FileType::UnSet {
            if dat_node.is_dir() {
                // Default games/directories to Dir if unspecified
                file_type = dat_reader::enums::FileType::Dir;
            } else {
                // Default ROMs to File if unspecified
                file_type = dat_reader::enums::FileType::File;
            }
        }

        let existing_match =
            Self::take_matching_existing_child(existing_children, &dat_node.name, file_type);
        let mut new_rv = RvFile::new(file_type);
        new_rv.name = dat_node.name.clone();
        new_rv.set_dat_status(DatStatus::InDatCollect);
        new_rv.dat = Some(Rc::clone(&dat_rc));
        if let Some(existing) = &existing_match {
            Self::apply_existing_runtime_state(&mut new_rv, &existing.borrow());
        }

        if dat_node.is_dir() {
            let d_dir = dat_node.dir().unwrap();
            new_rv.set_zip_dat_struct(d_dir.dat_struct(), d_dir.dat_struct_fix());
            
            // Initially a DAT dir/game is completely "Missing"
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
                Self::map_dat_node_to_rv_file(Rc::clone(&new_rc), child, Rc::clone(&dat_rc), &mut old_children);
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

            // Initially a DAT file is completely "Missing" because we haven't scanned
            new_rv.set_dat_got_status(dat_reader::enums::DatStatus::InDatCollect, new_rv.got_status());
            new_rv.rep_status_reset();

            let new_rc = Rc::new(RefCell::new(new_rv));
            new_rc.borrow_mut().parent = Some(Rc::downgrade(&parent));
            parent.borrow_mut().child_add(new_rc);
        }
    }

    fn scan_dat_dir(path: &str, dats_found: &mut Vec<(String, String)>) {
        let root_path = Path::new(path);
        let base_len = if path == "DatRoot" { 8 } else { path.len() + 1 };
        Self::recursive_scan(root_path, root_path, base_len, dats_found);
    }

    fn recursive_scan(base_path: &Path, current_path: &Path, base_len: usize, dats_found: &mut Vec<(String, String)>) {
        if let Ok(entries) = fs::read_dir(current_path) {
            for entry in entries.filter_map(Result::ok) {
                let path = entry.path();
                if path.is_dir() {
                    Self::recursive_scan(base_path, &path, base_len, dats_found);
                } else if let Some(ext) = path.extension() {
                    let ext_str = ext.to_string_lossy().to_lowercase();
                    if ext_str == "dat" || ext_str == "xml" || ext_str == "datz" {
                        let full_path = path.to_string_lossy().into_owned();
                        
                        // Calculate the virtual directory path relative to DatRoot
                        // e.g. DatRoot\Archive\EmulatorArchived\foo.dat -> "Archive\EmulatorArchived"
                        let mut virtual_dir = String::new();
                        if full_path.len() > base_len {
                            let rel_path = &full_path[base_len..];
                            if let Some(parent) = Path::new(rel_path).parent() {
                                let parent_str = parent.to_string_lossy().into_owned();
                                if !parent_str.is_empty() {
                                    virtual_dir = parent_str;
                                }
                            }
                        }
                        
                        dats_found.push((full_path, virtual_dir));
                    }
                }
            }
        }
    }

    /// Cleans up orphaned DB nodes whose underlying physical DAT files have been deleted.
    pub fn check_all_dats(db_file: Rc<RefCell<RvFile>>, dat_path: &str) {
        let db_dir = db_file.borrow();
        if !db_dir.is_directory() {
            return;
        }

        for dat in &db_dir.dir_dats {
            let dat_full_name = dat.borrow().get_data(crate::rv_dat::DatData::DatRootFullName).unwrap_or_default();
            if Self::dat_path_matches_filter(&dat_full_name, dat_path) {
                dat.borrow_mut().time_stamp = i64::MAX;
            }
        }

        if let Some(dat) = &db_dir.dat {
            let dat_full_name = dat.borrow().get_data(crate::rv_dat::DatData::DatRootFullName).unwrap_or_default();
            if Self::dat_path_matches_filter(&dat_full_name, dat_path) {
                dat.borrow_mut().time_stamp = i64::MAX;
            }
        }

        let children = db_dir.children.clone();
        drop(db_dir);

        for child in children {
            Self::check_all_dats(child, dat_path);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_populate_rv_dat_from_header_sets_extended_metadata() {
        let dir = tempdir().unwrap();
        let dat_path = dir.path().join("sample.dat");
        fs::write(&dat_path, "test").unwrap();

        let mut header = DatHeader::default();
        header.id = Some("id-1".to_string());
        header.name = Some("SampleDat".to_string());
        header.root_dir = Some("Roms".to_string());
        header.description = Some("Desc".to_string());
        header.category = Some("Cat".to_string());
        header.version = Some("1.0".to_string());
        header.date = Some("2026-01-01".to_string());
        header.author = Some("Author".to_string());
        header.email = Some("a@example.com".to_string());
        header.homepage = Some("https://example.com".to_string());
        header.url = Some("https://example.com/dat".to_string());
        header.header = Some("nes".to_string());
        header.compression = Some("zip".to_string());
        header.merge_type = Some("split".to_string());
        header.dir = Some("full".to_string());

        let mut rv_dat = RvDat::new();
        DatUpdate::populate_rv_dat_from_header(&mut rv_dat, &header, &dat_path.to_string_lossy());

        assert_eq!(rv_dat.get_data(DatData::Id), Some("id-1".to_string()));
        assert_eq!(rv_dat.get_data(DatData::DatName), Some("SampleDat".to_string()));
        assert_eq!(rv_dat.get_data(DatData::DatRootFullName), Some(dat_path.to_string_lossy().to_string()));
        assert_eq!(rv_dat.get_data(DatData::RootDir), Some("Roms".to_string()));
        assert_eq!(rv_dat.get_data(DatData::Description), Some("Desc".to_string()));
        assert_eq!(rv_dat.get_data(DatData::Category), Some("Cat".to_string()));
        assert_eq!(rv_dat.get_data(DatData::Version), Some("1.0".to_string()));
        assert_eq!(rv_dat.get_data(DatData::Header), Some("nes".to_string()));
        assert_eq!(rv_dat.get_data(DatData::Compression), Some("zip".to_string()));
        assert_eq!(rv_dat.get_data(DatData::MergeType), Some("split".to_string()));
        assert_eq!(rv_dat.get_data(DatData::DirSetup), Some("full".to_string()));
        assert!(rv_dat.time_stamp > 0);
    }

    #[test]
    fn test_map_dat_node_to_rv_file_marks_dat_sourced_flags() {
        let parent = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
        let dat_rc = Rc::new(RefCell::new(RvDat::new()));

        let mut dat_node = DatNode::new_file("rom.nes".to_string(), FileType::File);
        dat_node.date_modified = Some(12345);
        let file = dat_node.file_mut().unwrap();
        file.size = Some(1024);
        file.crc = Some(vec![1, 2, 3, 4]);
        file.sha1 = Some(vec![5; 20]);
        file.md5 = Some(vec![6; 16]);
        file.header_file_type = dat_reader::enums::HeaderFileType::NES;

        DatUpdate::map_dat_node_to_rv_file(Rc::clone(&parent), &dat_node, Rc::clone(&dat_rc), &mut Vec::new());

        let child_rc = Rc::clone(&parent.borrow().children[0]);
        let child = child_rc.borrow();
        assert!(child.file_status_is(FileStatus::SIZE_FROM_DAT));
        assert!(child.file_status_is(FileStatus::CRC_FROM_DAT));
        assert!(child.file_status_is(FileStatus::SHA1_FROM_DAT));
        assert!(child.file_status_is(FileStatus::MD5_FROM_DAT));
        assert!(child.file_status_is(FileStatus::HEADER_FILE_TYPE_FROM_DAT));
        assert!(child.file_status_is(FileStatus::DATE_FROM_DAT));
        assert_eq!(child.file_mod_time_stamp, 12345);
    }

    #[test]
    fn test_map_dat_node_to_rv_file_preserves_existing_got_state_for_matching_node() {
        let parent = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
        let dat_rc = Rc::new(RefCell::new(RvDat::new()));

        let existing_child = Rc::new(RefCell::new(RvFile::new(FileType::File)));
        {
            let mut existing = existing_child.borrow_mut();
            existing.name = "rom.bin".to_string();
            existing.set_dat_got_status(DatStatus::InDatCollect, dat_reader::enums::GotStatus::Got);
            existing.tree_expanded = true;
            existing.tree_checked = crate::rv_file::TreeSelect::Locked;
        }

        let mut existing_children = vec![Rc::clone(&existing_child)];

        let mut dat_node = DatNode::new_file("rom.bin".to_string(), FileType::File);
        let file = dat_node.file_mut().unwrap();
        file.size = Some(4096);
        file.crc = Some(vec![1, 2, 3, 4]);

        DatUpdate::map_dat_node_to_rv_file(
            Rc::clone(&parent),
            &dat_node,
            Rc::clone(&dat_rc),
            &mut existing_children,
        );

        let mapped_child = {
            let parent_ref = parent.borrow();
            Rc::clone(&parent_ref.children[0])
        };
        let mapped = mapped_child.borrow();
        assert_eq!(mapped.got_status(), dat_reader::enums::GotStatus::Got);
        assert_eq!(mapped.rep_status(), crate::enums::RepStatus::Correct);
        assert!(mapped.tree_expanded);
        assert_eq!(mapped.tree_checked, crate::rv_file::TreeSelect::Locked);
        assert_eq!(mapped.size, Some(4096));
        assert_eq!(mapped.crc, Some(vec![1, 2, 3, 4]));
    }

    #[test]
    fn test_map_dat_node_to_rv_file_preserves_existing_physical_metadata_for_matching_node() {
        let parent = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
        let dat_rc = Rc::new(RefCell::new(RvDat::new()));

        let existing_child = Rc::new(RefCell::new(RvFile::new(FileType::File)));
        {
            let mut existing = existing_child.borrow_mut();
            existing.name = "rom.bin".to_string();
            existing.set_dat_got_status(DatStatus::InDatCollect, dat_reader::enums::GotStatus::Got);
            existing.size = Some(8192);
            existing.crc = Some(vec![9, 9, 9, 9]);
            existing.sha1 = Some(vec![8; 20]);
            existing.file_mod_time_stamp = 777;
            existing.header_file_type = HeaderFileType::NES;
            existing.file_status_set(FileStatus::HEADER_FILE_TYPE_FROM_HEADER);
            existing.local_header_offset = Some(55);
        }

        let mut existing_children = vec![Rc::clone(&existing_child)];

        let mut dat_node = DatNode::new_file("rom.bin".to_string(), FileType::File);
        dat_node.date_modified = Some(12345);
        let file = dat_node.file_mut().unwrap();
        file.size = Some(4096);
        file.crc = Some(vec![1, 2, 3, 4]);
        file.header_file_type = HeaderFileType::SNES | HeaderFileType::REQUIRED;

        DatUpdate::map_dat_node_to_rv_file(
            Rc::clone(&parent),
            &dat_node,
            Rc::clone(&dat_rc),
            &mut existing_children,
        );

        let mapped_child = {
            let parent_ref = parent.borrow();
            Rc::clone(&parent_ref.children[0])
        };
        let mapped = mapped_child.borrow();
        assert_eq!(mapped.size, Some(8192));
        assert_eq!(mapped.crc, Some(vec![9, 9, 9, 9]));
        assert_eq!(mapped.sha1, Some(vec![8; 20]));
        assert_eq!(mapped.file_mod_time_stamp, 777);
        assert_eq!(mapped.local_header_offset, Some(55));
        assert_eq!(mapped.header_file_type(), HeaderFileType::NES);
        assert!(mapped.header_file_type_required());
        assert!(mapped.file_status_is(FileStatus::HEADER_FILE_TYPE_FROM_HEADER));
    }

    #[test]
    fn test_map_dat_node_to_rv_file_preserves_existing_archive_state_for_matching_node() {
        let parent = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
        let dat_rc = Rc::new(RefCell::new(RvDat::new()));

        let existing_child = Rc::new(RefCell::new(RvFile::new(FileType::Zip)));
        {
            let mut existing = existing_child.borrow_mut();
            existing.name = "game.zip".to_string();
            existing.set_dat_got_status(DatStatus::InDatCollect, dat_reader::enums::GotStatus::Got);
            existing.zip_struct = dat_reader::enums::ZipStructure::ZipTrrnt;
            existing.file_mod_time_stamp = 123456;
        }

        let mut existing_children = vec![Rc::clone(&existing_child)];

        let dat_node = DatNode::new_dir("game.zip".to_string(), FileType::Zip);

        DatUpdate::map_dat_node_to_rv_file(
            Rc::clone(&parent),
            &dat_node,
            Rc::clone(&dat_rc),
            &mut existing_children,
        );

        let mapped_child = {
            let parent_ref = parent.borrow();
            Rc::clone(&parent_ref.children[0])
        };
        let mapped = mapped_child.borrow();
        assert_eq!(mapped.got_status(), dat_reader::enums::GotStatus::Got);
        assert_eq!(mapped.rep_status(), crate::enums::RepStatus::Correct);
        assert_eq!(mapped.zip_struct, dat_reader::enums::ZipStructure::ZipTrrnt);
        assert_eq!(mapped.file_mod_time_stamp, 123456);
    }

    #[test]
    fn test_map_dat_node_to_rv_file_preserves_unmatched_physical_child_as_not_in_dat() {
        let parent = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
        let dat_rc = Rc::new(RefCell::new(RvDat::new()));

        let existing_dir = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
        existing_dir.borrow_mut().name = "game".to_string();

        let orphan = Rc::new(RefCell::new(RvFile::new(FileType::File)));
        {
            let mut orphan_mut = orphan.borrow_mut();
            orphan_mut.name = "extra.bin".to_string();
            orphan_mut.set_dat_got_status(DatStatus::InDatCollect, dat_reader::enums::GotStatus::Got);
            orphan_mut.crc = Some(vec![1, 2, 3, 4]);
            orphan_mut.parent = Some(Rc::downgrade(&existing_dir));
        }
        existing_dir.borrow_mut().child_add(Rc::clone(&orphan));

        let mut existing_children = vec![Rc::clone(&existing_dir)];

        let dat_node = DatNode::new_dir("game".to_string(), FileType::Dir);

        DatUpdate::map_dat_node_to_rv_file(
            Rc::clone(&parent),
            &dat_node,
            Rc::clone(&dat_rc),
            &mut existing_children,
        );

        let mapped_dir = {
            let parent_ref = parent.borrow();
            Rc::clone(&parent_ref.children[0])
        };
        let dir_ref = mapped_dir.borrow();
        assert_eq!(dir_ref.children.len(), 1);
        let preserved = dir_ref.children[0].borrow();
        assert_eq!(preserved.name, "extra.bin");
        assert_eq!(preserved.dat_status(), DatStatus::NotInDat);
        assert_eq!(preserved.got_status(), dat_reader::enums::GotStatus::Got);
        assert_eq!(preserved.rep_status(), crate::enums::RepStatus::Unknown);
    }

    #[test]
    fn test_check_all_dats_marks_matching_dir_dats_using_real_paths() {
        let root = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));

        let dat_a = Rc::new(RefCell::new(RvDat::new()));
        dat_a.borrow_mut().set_data(DatData::DatRootFullName, Some("DatRoot\\Arcade\\a.dat".to_string()));

        let dat_b = Rc::new(RefCell::new(RvDat::new()));
        dat_b.borrow_mut().set_data(DatData::DatRootFullName, Some("DatRoot\\Console\\b.dat".to_string()));

        root.borrow_mut().dir_dats.push(Rc::clone(&dat_a));
        root.borrow_mut().dir_dats.push(Rc::clone(&dat_b));

        DatUpdate::check_all_dats(Rc::clone(&root), "DatRoot\\Arcade");

        assert_eq!(dat_a.borrow().time_stamp, i64::MAX);
        assert_ne!(dat_b.borrow().time_stamp, i64::MAX);
    }

    #[test]
    fn test_check_all_dats_marks_matching_directory_dat_using_real_path() {
        let root = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
        let child = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));

        let dat = Rc::new(RefCell::new(RvDat::new()));
        dat.borrow_mut().set_data(DatData::DatRootFullName, Some("DatRoot\\Console\\game.dat".to_string()));
        child.borrow_mut().dat = Some(Rc::clone(&dat));
        root.borrow_mut().child_add(Rc::clone(&child));

        DatUpdate::check_all_dats(Rc::clone(&root), "DatRoot\\Console");

        assert_eq!(dat.borrow().time_stamp, i64::MAX);
    }
}
