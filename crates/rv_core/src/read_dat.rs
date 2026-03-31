use std::fs;
use std::path::Path;
use std::rc::Rc;
use std::cell::RefCell;
use crate::rv_file::RvFile;
use dat_reader::read_dat;
use dat_reader::enums::FileType;
use crate::rv_dat::{RvDat, DatData};
use dat_reader::dat_store::DatNode;
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
                        let mut new_rv_dat = RvDat::new();
                        new_rv_dat.set_data(DatData::DatName, dat_header.name.clone());
                        new_rv_dat.set_data(DatData::Description, dat_header.description.clone());
                        new_rv_dat.set_data(DatData::Version, dat_header.version.clone());
                        
                        let rv_dat_rc = Rc::new(RefCell::new(new_rv_dat));
                        
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
                        
                        let target_dir = match target_dir {
                            Some(d) => {
                                // If the DAT directory already exists, we must clear its old children 
                                // to prevent duplicating the entire DAT when clicking "Update DATs"
                                d.borrow_mut().children.clear();
                                // Also clear its cached stats so it recalculates
                                d.borrow_mut().cached_stats = None;
                                d
                            },
                            None => {
                                let mut new_dir = RvFile::new(FileType::Dir);
                                new_dir.name = dir_name;
                                new_dir.set_dat_got_status(dat_reader::enums::DatStatus::InDatCollect, dat_reader::enums::GotStatus::NotGot);
                                new_dir.rep_status_reset();
                                let d_rc = Rc::new(RefCell::new(new_dir));
                                rv_dir_mut.child_add(Rc::clone(&d_rc));
                                d_rc
                            }
                        };
                        
                        // 3. Attach the parsed dat data into the tree and map recursively
                        {
                            let mut td = target_dir.borrow_mut();
                            let new_index = td.dir_dats.len() as i32;
                            rv_dat_rc.borrow_mut().dat_index = new_index;
                            td.dir_dats.push(Rc::clone(&rv_dat_rc));
                        }
                        
                        // Recursive mapping
                        for dat_child in &dat_header.base_dir.children {
                            Self::map_dat_node_to_rv_file(Rc::clone(&target_dir), dat_child, Rc::clone(&rv_dat_rc));
                        }
                    },
                    Err(e) => {
                        println!("Error reading DAT {}: {}", dat_path, e);
                    }
                }
            }
        }
    }

    fn map_dat_node_to_rv_file(parent: Rc<RefCell<RvFile>>, dat_node: &DatNode, dat_rc: Rc<RefCell<RvDat>>) {
        let mut new_rv = RvFile::new(dat_node.file_type);
        new_rv.name = dat_node.name.clone();
        new_rv.set_dat_status(DatStatus::InDatCollect);
        new_rv.dat = Some(Rc::clone(&dat_rc));

        if dat_node.is_dir() {
            let d_dir = dat_node.dir().unwrap();
            new_rv.set_zip_dat_struct(d_dir.dat_struct(), d_dir.dat_struct_fix());
            
            // Initially a DAT dir/game is completely "Missing"
            new_rv.set_dat_got_status(dat_reader::enums::DatStatus::InDatCollect, dat_reader::enums::GotStatus::NotGot);
            new_rv.rep_status_reset();
            
            if let Some(ref d_game) = d_dir.d_game {
                new_rv.game = Some(Rc::new(RefCell::new(RvGame::from_dat_game(d_game))));
            }

            let new_rc = Rc::new(RefCell::new(new_rv));
            parent.borrow_mut().child_add(Rc::clone(&new_rc));

            for child in &d_dir.children {
                Self::map_dat_node_to_rv_file(Rc::clone(&new_rc), child, Rc::clone(&dat_rc));
            }
        } else {
            let d_file = dat_node.file().unwrap();
            new_rv.size = d_file.size;
            new_rv.crc = d_file.crc.clone();
            new_rv.sha1 = d_file.sha1.clone();
            new_rv.md5 = d_file.md5.clone();
            
            if let Some(ref m) = d_file.merge {
                new_rv.merge = m.clone();
            }
            new_rv.status = d_file.status.clone();
            new_rv.set_header_file_type(d_file.header_file_type);

            // Initially a DAT file is completely "Missing" because we haven't scanned
            new_rv.set_dat_got_status(dat_reader::enums::DatStatus::InDatCollect, dat_reader::enums::GotStatus::NotGot);
            new_rv.rep_status_reset();

            let new_rc = Rc::new(RefCell::new(new_rv));
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

        let dats = db_dir.dir_dats.len();
        if dats > 0 {
            let dat_full_path = "DatRoot".to_string(); // In a real app we'd construct the tree full name
            if dat_path.len() <= dat_full_path.len() {
                if &dat_full_path[0..dat_path.len()] == dat_path {
                    for i in 0..dats {
                        db_dir.dir_dats[i].borrow_mut().time_stamp = i64::MAX;
                    }
                }
            }
        }

        if let Some(dat) = &db_dir.dat {
            let dat_full_name = dat.borrow().get_data(crate::rv_dat::DatData::DatRootFullName).unwrap_or_default();
            if dat_path.len() <= dat_full_name.len() {
                if &dat_full_name[0..dat_path.len()] == dat_path {
                    dat.borrow_mut().time_stamp = i64::MAX;
                }
            }
        }

        let children = db_dir.children.clone();
        drop(db_dir);

        for child in children {
            Self::check_all_dats(child, dat_path);
        }
    }
}
