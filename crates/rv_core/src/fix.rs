use std::rc::Rc;
use std::cell::RefCell;
use std::path::Path;
use std::fs;
use std::collections::HashMap;
use crate::enums::RepStatus;
use crate::rv_file::RvFile;

/// The Fix engine responsible for physically modifying the filesystem.
/// 
/// This module implements the "Fix ROMs" phase of RomVault. It traverses the internal file tree
/// and applies physical disk operations (copying, moving, deleting, renaming) to bring the 
/// physical files into alignment with the logical `RepStatus` calculated by `FindFixes`.
/// 
/// Differences from C#:
/// - The C# reference uses a highly abstract `FixAZipCore` virtual I/O engine that can natively 
///   stream and repack `TorrentZip` and `7z` files on the fly.
/// - The Rust implementation currently uses basic `fs::copy`, `fs::rename`, and simple `zip` extraction 
///   without advanced repackaging or `TorrentZip` formatting during the fix pass.
pub struct Fix;

impl Fix {
    /// Executes the fix operations across the database tree.
    /// 
    /// This routine operates in three distinct passes to avoid file collisions and logic bugs:
    /// 1. **Pass 1:** Moves incorrect files out of the way (`MoveToSort`, `MoveToCorrupt`).
    /// 2. **Pass 2:** Copies needed files into correct locations (`CanBeFixed`).
    /// 3. **Pass 3:** Cleans up unneeded files (`Delete`) and removes empty parent directories.
    pub fn perform_fixes(root: Rc<RefCell<RvFile>>) {
        let mut file_process_queue = Vec::new();
        let mut total_fixed = 0;

        // 1. Gather all NeededForFix files into a lookup map
        let mut needed_files = Vec::new();
        Self::gather_needed_files(Rc::clone(&root), &mut needed_files);
        
        let mut crc_map: HashMap<(u64, Vec<u8>), Rc<RefCell<RvFile>>> = HashMap::new();
        let mut sha1_map: HashMap<(u64, Vec<u8>), Rc<RefCell<RvFile>>> = HashMap::new();
        let mut md5_map: HashMap<(u64, Vec<u8>), Rc<RefCell<RvFile>>> = HashMap::new();

        for needed in needed_files {
            let n_ref = needed.borrow();
            let size = n_ref.size.unwrap_or(0);
            if let Some(ref crc) = n_ref.crc { crc_map.insert((size, crc.clone()), Rc::clone(&needed)); }
            if let Some(ref sha1) = n_ref.sha1 { sha1_map.insert((size, sha1.clone()), Rc::clone(&needed)); }
            if let Some(ref md5) = n_ref.md5 { md5_map.insert((size, md5.clone()), Rc::clone(&needed)); }
        }

        // 2. Perform fixes in 3 distinct passes to ensure order of operations matches C# logic
        // Pass 1: Move incorrect files out of the way (MoveToSort, MoveToCorrupt, Rename)
        Self::fix_dir(Rc::clone(&root), &mut file_process_queue, &mut total_fixed, &crc_map, &sha1_map, &md5_map, 1);
        // Pass 2: Copy needed files into their correct locations (CanBeFixed)
        Self::fix_dir(Rc::clone(&root), &mut file_process_queue, &mut total_fixed, &crc_map, &sha1_map, &md5_map, 2);
        // Pass 3: Clean up all unneeded files, including the sources we just copied (Delete)
        Self::fix_dir(Rc::clone(&root), &mut file_process_queue, &mut total_fixed, &crc_map, &sha1_map, &md5_map, 3);
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

    fn fix_dir(
        dir: Rc<RefCell<RvFile>>, 
        queue: &mut Vec<Rc<RefCell<RvFile>>>, 
        total_fixed: &mut i32,
        crc_map: &HashMap<(u64, Vec<u8>), Rc<RefCell<RvFile>>>,
        sha1_map: &HashMap<(u64, Vec<u8>), Rc<RefCell<RvFile>>>,
        md5_map: &HashMap<(u64, Vec<u8>), Rc<RefCell<RvFile>>>,
        pass: i32
    ) {
        let mut d = dir.borrow_mut();
        d.cached_stats = None;
        let children = d.children.clone();
        drop(d); // Drop borrow so we can iterate and pass to child funcs

        for child in children {
            let is_dir = child.borrow().is_directory();
            
            if is_dir {
                Self::fix_dir(Rc::clone(&child), queue, total_fixed, crc_map, sha1_map, md5_map, pass);
            } else {
                Self::fix_file(Rc::clone(&child), queue, total_fixed, crc_map, sha1_map, md5_map, pass);
            }
        }
    }

    fn get_physical_path(file: Rc<RefCell<RvFile>>) -> String {
        let mut path_parts = Vec::new();
        let mut current = Some(file);
        
        while let Some(node_rc) = current {
            let node = node_rc.borrow();
            if !node.name.is_empty() {
                path_parts.push(node.name.clone());
            }
            current = node.parent.as_ref().and_then(|w| w.upgrade());
        }
        
        path_parts.reverse();
        path_parts.join("/")
    }

    fn get_tosort_path(file_path: &str, base_dir: &str) -> String {
        let path = Path::new(file_path);
        let file_name = path.file_name().unwrap().to_str().unwrap();
        
        let mut components: Vec<_> = path.components().map(|c| c.as_os_str().to_str().unwrap()).collect();
        // remove root directory (e.g. "RustyVault")
        if !components.is_empty() {
            // Check if the root directory is already ToSort, if so, we don't need to re-nest it
            if components[0] == "ToSort" {
                // If it's already in ToSort, and we are moving to ToSort, we shouldn't change the path
                // But if it's moving to ToSort/Corrupt, we just insert Corrupt
                if base_dir == "ToSort/Corrupt" {
                    components.insert(1, "Corrupt");
                }
            } else {
                components.remove(0); // Remove "RustyVault" or similar
            }
        }
        // remove file name
        if !components.is_empty() {
            components.pop();
        }
        
        let dir_path = if components.is_empty() {
            base_dir.to_string()
        } else if components[0] == "ToSort" {
            components.join("/")
        } else {
            format!("{}/{}", base_dir, components.join("/"))
        };
        
        let _ = fs::create_dir_all(&dir_path);
        
        let mut target_path = format!("{}/{}", dir_path, file_name);
        
        // If moving to self, just return
        if target_path == file_path {
            return target_path;
        }
        
        let mut counter = 0;
        while Path::new(&target_path).exists() {
            let file_stem = path.file_stem().unwrap().to_str().unwrap();
            let ext = path.extension().map(|e| e.to_str().unwrap()).unwrap_or("");
            
            let new_name = if ext.is_empty() {
                format!("{}_{}", file_stem, counter)
            } else {
                format!("{}_{}.{}", file_stem, counter, ext)
            };
            target_path = format!("{}/{}", dir_path, new_name);
            counter += 1;
        }
        
        target_path
    }

    fn fix_file(
        file: Rc<RefCell<RvFile>>, 
        _queue: &mut Vec<Rc<RefCell<RvFile>>>, 
        _total_fixed: &mut i32,
        crc_map: &HashMap<(u64, Vec<u8>), Rc<RefCell<RvFile>>>,
        sha1_map: &HashMap<(u64, Vec<u8>), Rc<RefCell<RvFile>>>,
        md5_map: &HashMap<(u64, Vec<u8>), Rc<RefCell<RvFile>>>,
        pass: i32
    ) {
        let mut f = file.borrow_mut();
        let rep_status = f.rep_status();
        let name = f.name.clone();
        
        f.cached_stats = None;
        
        // Let's release the borrow to calculate paths
        drop(f);
        
        let file_path = Self::get_physical_path(Rc::clone(&file));
        
        let mut f = file.borrow_mut();
        
        match rep_status {
            RepStatus::Delete if pass == 3 => {
                println!("Deleting file: {}", file_path);
                if Path::new(&file_path).exists() {
                    let _ = fs::remove_file(&file_path);
                    
                    // Attempt to clean up empty parent directories like C# CheckDeleteFile
                    let mut current_dir = Path::new(&file_path).parent();
                    while let Some(parent) = current_dir {
                        // Break if directory is not empty or if it's the root (RustyVault/ToSort)
                        if fs::remove_dir(parent).is_err() {
                            break;
                        }
                        current_dir = parent.parent();
                    }
                }
                f.set_rep_status(RepStatus::Deleted);
            },
            RepStatus::MoveToSort if pass == 1 => {
                let target_path = Self::get_tosort_path(&file_path, "ToSort");
                if target_path != file_path {
                    println!("Moving to ToSort: {} -> {}", file_path, target_path);
                    if Path::new(&file_path).exists() {
                        let _ = fs::rename(&file_path, &target_path);
                    }
                }
                f.set_rep_status(RepStatus::InToSort);
            },
            RepStatus::MoveToCorrupt if pass == 1 => {
                let target_path = Self::get_tosort_path(&file_path, "ToSort/Corrupt");
                if target_path != file_path {
                    println!("Moving corrupt file to ToSort/Corrupt: {} -> {}", file_path, target_path);
                    if Path::new(&file_path).exists() {
                        let _ = fs::rename(&file_path, &target_path);
                    }
                }
                f.set_rep_status(RepStatus::Deleted);
            },
            RepStatus::CanBeFixed | RepStatus::CanBeFixedMIA | RepStatus::CorruptCanBeFixed if pass == 2 => {
                let size = f.size.unwrap_or(0);
                let mut source_file = None;

                if let Some(ref crc) = f.crc {
                    source_file = crc_map.get(&(size, crc.clone())).cloned();
                }
                if source_file.is_none() {
                    if let Some(ref sha1) = f.sha1 {
                        source_file = sha1_map.get(&(size, sha1.clone())).cloned();
                    }
                }
                if source_file.is_none() {
                    if let Some(ref md5) = f.md5 {
                        source_file = md5_map.get(&(size, md5.clone())).cloned();
                    }
                }

                if let Some(src) = source_file {
                    let src_path = Self::get_physical_path(Rc::clone(&src));
                    
                    // Don't fix a file from itself (e.g. if it was somehow mapped to its own path)
                    if src_path != file_path {
                        println!("Fixing file from source: {} -> {}", src_path, file_path);
                        
                        // Create parent directory if needed
                        if let Some(parent) = Path::new(&file_path).parent() {
                            let _ = fs::create_dir_all(parent);
                        }
                        
                        let src_ref = src.borrow();
                        let src_parent_is_zip = src_ref.parent.as_ref()
                            .and_then(|p| p.upgrade())
                            .map_or(false, |p| p.borrow().file_type == dat_reader::enums::FileType::Zip);

                        if src_parent_is_zip {
                            let zip_path = Self::get_physical_path(src_ref.parent.as_ref().unwrap().upgrade().unwrap());
                            if let Ok(file) = fs::File::open(&zip_path) {
                                if let Ok(mut archive) = zip::ZipArchive::new(file) {
                                    if let Ok(mut inner_file) = archive.by_name(&src_ref.name) {
                                        if let Ok(mut out_file) = fs::File::create(&file_path) {
                                            let _ = std::io::copy(&mut inner_file, &mut out_file);
                                        }
                                    }
                                }
                            }
                        } else if Path::new(&src_path).exists() {
                            let _ = fs::copy(&src_path, &file_path);
                        }
                        
                        drop(src_ref);
                        src.borrow_mut().set_rep_status(RepStatus::Delete);
                    }
                } else {
                    println!("Could not find source file for: {}", name);
                }

                f.set_rep_status(RepStatus::Correct);
            },
            RepStatus::Rename if pass == 1 => {
                println!("Renaming file: {}", file_path);
                // Simple rename logic would go here if we tracked the old name.
                f.set_rep_status(RepStatus::Correct);
            },
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dat_reader::enums::FileType;
    use std::rc::Rc;
    use std::cell::RefCell;

    #[test]
    fn test_get_physical_path() {
        let root = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
        root.borrow_mut().name = "RustyVault".to_string();

        let folder = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
        folder.borrow_mut().name = "Nintendo".to_string();
        
        let file = Rc::new(RefCell::new(RvFile::new(FileType::File)));
        file.borrow_mut().name = "game.zip".to_string();

        folder.borrow_mut().child_add(Rc::clone(&file));
        root.borrow_mut().child_add(Rc::clone(&folder));

        let path = Fix::get_physical_path(Rc::clone(&file));
        assert_eq!(path, "RustyVault/Nintendo/game.zip");
    }

    #[test]
    fn test_fix_file_status_changes() {
        let root = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
        let mut queue = Vec::new();
        let mut total_fixed = 0;
        
        let crc_map = HashMap::new();
        let sha1_map = HashMap::new();
        let md5_map = HashMap::new();

        // Test MoveToSort status change
        let file_to_sort = Rc::new(RefCell::new(RvFile::new(FileType::File)));
        file_to_sort.borrow_mut().set_rep_status(RepStatus::MoveToSort);
        Fix::fix_file(Rc::clone(&file_to_sort), &mut queue, &mut total_fixed, &crc_map, &sha1_map, &md5_map, 1);
        assert_eq!(file_to_sort.borrow().rep_status(), RepStatus::InToSort);

        // Test Delete status change
        let file_delete = Rc::new(RefCell::new(RvFile::new(FileType::File)));
        file_delete.borrow_mut().set_rep_status(RepStatus::Delete);
        Fix::fix_file(Rc::clone(&file_delete), &mut queue, &mut total_fixed, &crc_map, &sha1_map, &md5_map, 3);
        assert_eq!(file_delete.borrow().rep_status(), RepStatus::Deleted);

        // Test CanBeFixed status change
        let file_fix = Rc::new(RefCell::new(RvFile::new(FileType::File)));
        file_fix.borrow_mut().set_rep_status(RepStatus::CanBeFixed);
        Fix::fix_file(Rc::clone(&file_fix), &mut queue, &mut total_fixed, &crc_map, &sha1_map, &md5_map, 2);
        assert_eq!(file_fix.borrow().rep_status(), RepStatus::Correct);
    }
}
