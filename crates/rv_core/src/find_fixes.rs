use std::rc::Rc;
use std::cell::RefCell;
use std::collections::HashMap;
use tracing::{info, trace};
use dat_reader::enums::{DatStatus, GotStatus};
use crate::enums::RepStatus;
use crate::rv_file::{RvFile, TreeSelect};

/// The logical matching engine for resolving missing ROMs.
/// 
/// `FindFixes` is responsible for calculating the logical repair state (`RepStatus`) of the 
/// database. It identifies missing files in the primary `RustyRoms` and attempts to map them 
/// to available files sitting in `ToSort` using exact CRC/SHA1/MD5 hash matching.
/// 
/// Differences from C#:
/// - The C# reference uses standard Threads to parallelize the creation of `FileGroup` lookup indexes
///   (`FastArraySort.SortWithFilter`).
/// - The Rust version leverages `rayon` to safely build parallel lookup `HashMap` indexes across 
///   available CPU cores, providing equivalent or faster multi-threaded performance while maintaining
///   memory safety without manual thread joining.
pub struct FindFixes;

impl FindFixes {
    fn is_tree_selected(node: &RvFile) -> bool {
        matches!(node.tree_checked, TreeSelect::Selected | TreeSelect::Locked)
    }

    /// Recursively scans the tree to pair `Missing` files with unassigned `Got` files.
    pub fn scan_files(root: Rc<RefCell<RvFile>>) {
        info!("Starting FindFixes pass...");
        // Step 1: Reset tree statuses
        Self::reset_status(Rc::clone(&root));

        // Step 2: Get Selected Files
        let mut files_got = Vec::new();
        let mut files_missing = Vec::new();
        Self::get_selected_files(Rc::clone(&root), &mut files_got, &mut files_missing);

        info!("FindFixes: Collected {} Got files and {} Missing files.", files_got.len(), files_missing.len());

        // Convert the RC pointers to a form that can be shared across threads
        // We will build the maps using parallel iteration over the files_got list
        let mut hash_data = Vec::with_capacity(files_got.len());
        for (idx, got) in files_got.iter().enumerate() {
            let got_ref = got.borrow();
            hash_data.push((
                idx,
                got_ref.size.unwrap_or(0),
                got_ref.crc.clone(),
                got_ref.sha1.clone(),
                got_ref.md5.clone()
            ));
        }

        // Now we can use rayon to build the three hash maps in parallel!
        let (crc_map, (sha1_map, md5_map)) = rayon::join(
            || {
                let mut map: HashMap<(u64, Vec<u8>), Vec<usize>> = HashMap::new();
                for (idx, size, crc, _, _) in &hash_data {
                    if let Some(c) = crc {
                        map.entry((*size, c.clone())).or_default().push(*idx);
                    }
                }
                map
            },
            || {
                rayon::join(
                    || {
                        let mut map: HashMap<(u64, Vec<u8>), Vec<usize>> = HashMap::new();
                        for (idx, size, _, sha1, _) in &hash_data {
                            if let Some(s) = sha1 {
                                map.entry((*size, s.clone())).or_default().push(*idx);
                            }
                        }
                        map
                    },
                    || {
                        let mut map: HashMap<(u64, Vec<u8>), Vec<usize>> = HashMap::new();
                        for (idx, size, _, _, md5) in &hash_data {
                            if let Some(m) = md5 {
                                map.entry((*size, m.clone())).or_default().push(*idx);
                            }
                        }
                        map
                    }
                )
            }
        );

        // Step 4: Match Missing files against Got indexes
        for missing in &files_missing {
            let mut missing_ref = missing.borrow_mut();
            let size = missing_ref.size.unwrap_or(0);

            let mut found_got_idx = None;

            // Try to find a match by CRC first
            if let Some(ref crc) = missing_ref.crc {
                if let Some(got_list) = crc_map.get(&(size, crc.clone())) {
                    if let Some(first_got) = got_list.first() {
                        found_got_idx = Some(*first_got);
                    }
                }
            }

            // Fallback to SHA1 if no CRC match or missing CRC
            if found_got_idx.is_none() {
                if let Some(ref sha1) = missing_ref.sha1 {
                    if let Some(got_list) = sha1_map.get(&(size, sha1.clone())) {
                        if let Some(first_got) = got_list.first() {
                            found_got_idx = Some(*first_got);
                        }
                    }
                }
            }

            // Fallback to MD5 if still no match
            if found_got_idx.is_none() {
                if let Some(ref md5) = missing_ref.md5 {
                    if let Some(got_list) = md5_map.get(&(size, md5.clone())) {
                        if let Some(first_got) = got_list.first() {
                            found_got_idx = Some(*first_got);
                        }
                    }
                }
            }

            // If we found a matching file, flag it as fixable
            let is_mia = missing_ref.dat_status() == DatStatus::InDatMIA;
            if let Some(got_idx) = found_got_idx {
                let got = &files_got[got_idx];
                let is_corrupt = got.borrow().got_status() == GotStatus::Corrupt;
                
                trace!("Found fix match: missing '{}' mapped to got file.", missing_ref.name);
                
                missing_ref.set_rep_status(
                    if is_corrupt {
                        RepStatus::CorruptCanBeFixed
                    } else if is_mia {
                        RepStatus::CanBeFixedMIA
                    } else {
                        RepStatus::CanBeFixed
                    }
                );

                // Mark the got file so it knows it is needed
                let mut got_mut = got.borrow_mut();
                let current_rep = got_mut.rep_status();
                if current_rep == RepStatus::UnScanned || current_rep == RepStatus::InToSort || current_rep == RepStatus::Unknown || current_rep == RepStatus::Deleted {
                    got_mut.set_rep_status(RepStatus::NeededForFix);
                }
                
                missing_ref.cached_stats = None;
                got_mut.cached_stats = None;
            } else {
                trace!("No fix found for missing file: {}", missing_ref.name);
                let is_mia = missing_ref.dat_status() == DatStatus::InDatMIA;
                missing_ref.set_rep_status(
                    if is_mia {
                        RepStatus::MissingMIA
                    } else {
                        RepStatus::Missing
                    }
                );
                missing_ref.cached_stats = None;
            }
        }
        
        // Step 5: Handle corrupt files that aren't needed
        for got in &files_got {
            let mut got_ref = got.borrow_mut();
            if got_ref.got_status() == GotStatus::Corrupt {
                if got_ref.rep_status() == RepStatus::NeededForFix {
                    // It's a corrupt file but it matches a needed hash (maybe header issue)
                    // Let's leave it as NeededForFix or CorruptCanBeFixed
                } else if got_ref.dat_status() == DatStatus::InDatCollect {
                    got_ref.set_rep_status(RepStatus::MoveToCorrupt);
                    got_ref.cached_stats = None;
                } else {
                    got_ref.set_rep_status(RepStatus::Delete);
                    got_ref.cached_stats = None;
                }
            }
        }

        // Step 6: Mark remaining unused got files
        for got in &files_got {
            let mut got_ref = got.borrow_mut();
            
            if got_ref.rep_status() == RepStatus::NeededForFix || got_ref.rep_status() == RepStatus::Correct || got_ref.rep_status() == RepStatus::Delete || got_ref.rep_status() == RepStatus::MoveToCorrupt {
                continue;
            }
            
            // If the file is exactly where it needs to be and matches a Dat, it's correct
            if got_ref.dat_status() == DatStatus::InDatCollect {
                got_ref.set_rep_status(RepStatus::Correct);
                got_ref.cached_stats = None;
            } else if got_ref.dat_status() == DatStatus::InDatMIA {
                got_ref.set_rep_status(RepStatus::CorrectMIA);
                got_ref.cached_stats = None;
            } else if got_ref.dat_status() == DatStatus::InToSort {
                // Keep in tosort unless needed, if it's unused in ToSort, mark it as UnNeeded so it gets skipped
                // The C# RomVault just leaves them in ToSort unless they are fixed or deleted by double-check
                got_ref.set_rep_status(RepStatus::UnScanned);
                got_ref.cached_stats = None;
            } else if got_ref.dat_status() == DatStatus::NotInDat {
                got_ref.set_rep_status(RepStatus::MoveToSort);
                got_ref.cached_stats = None;
            }
        }
    }

    fn reset_status(node: Rc<RefCell<RvFile>>) {
        crate::repair_status::RepairStatus::report_status_reset(node);
    }

    fn get_selected_files(
        node: Rc<RefCell<RvFile>>,
        got_files: &mut Vec<Rc<RefCell<RvFile>>>,
        missing_files: &mut Vec<Rc<RefCell<RvFile>>>,
    ) {
        let n = node.borrow();
        let selected = Self::is_tree_selected(&n);
        
        if selected {
            if !n.is_directory() {
                match n.got_status() {
                    GotStatus::Got | GotStatus::Corrupt => {
                        got_files.push(Rc::clone(&node));
                    }
                    GotStatus::NotGot => {
                        missing_files.push(Rc::clone(&node));
                    }
                    _ => {}
                }
            }
        }

        if !n.is_directory() {
            return;
        }

        let children = n.children.clone();
        drop(n); // Drop borrow for recursion

        for child in children {
            Self::get_selected_files(child, got_files, missing_files);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dat_reader::enums::FileType;

    #[test]
    fn test_find_fixes_exact_crc_match() {
        let root = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
        
        // Mock a ToSort directory with a Got file
        let to_sort = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
        to_sort.borrow_mut().set_dat_status(DatStatus::InToSort);
        
        let mut got_file = RvFile::new(FileType::File);
        got_file.name = "got_file.bin".to_string();
        got_file.size = Some(1024);
        got_file.crc = Some(vec![0xAA, 0xBB, 0xCC, 0xDD]);
        got_file.set_dat_status(DatStatus::InToSort);
        got_file.set_got_status(GotStatus::Got);
        let got_rc = Rc::new(RefCell::new(got_file));
        to_sort.borrow_mut().child_add(Rc::clone(&got_rc));
        
        // Mock a DatRoot directory with a Missing file
        let dat_root = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
        dat_root.borrow_mut().set_dat_status(DatStatus::InDatCollect);
        
        let mut missing_file = RvFile::new(FileType::File);
        missing_file.name = "missing_file.bin".to_string();
        missing_file.size = Some(1024);
        missing_file.crc = Some(vec![0xAA, 0xBB, 0xCC, 0xDD]); // Exact CRC Match
        missing_file.set_dat_status(DatStatus::InDatCollect);
        missing_file.set_got_status(GotStatus::NotGot);
        let missing_rc = Rc::new(RefCell::new(missing_file));
        dat_root.borrow_mut().child_add(Rc::clone(&missing_rc));

        root.borrow_mut().child_add(to_sort);
        root.borrow_mut().child_add(dat_root);

        FindFixes::scan_files(Rc::clone(&root));

        // Missing file should now be flagged as CanBeFixed
        assert_eq!(missing_rc.borrow().rep_status(), RepStatus::CanBeFixed);
        // Got file should be flagged as NeededForFix
        assert_eq!(got_rc.borrow().rep_status(), RepStatus::NeededForFix);
    }

    #[test]
    fn test_find_fixes_matching() {
        let root = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
        
        // Setup a missing ROM
        let missing = Rc::new(RefCell::new(RvFile::new(FileType::File)));
        {
            let mut m = missing.borrow_mut();
            m.name = "game.rom".to_string();
            m.size = Some(1024);
            m.crc = Some(vec![0x12, 0x34, 0x56, 0x78]);
            m.set_dat_got_status(DatStatus::InDatCollect, GotStatus::NotGot);
        }
        
        // Setup an unknown ROM that matches the missing ROM
        let unknown = Rc::new(RefCell::new(RvFile::new(FileType::File)));
        {
            let mut u = unknown.borrow_mut();
            u.name = "random.bin".to_string();
            u.size = Some(1024);
            u.crc = Some(vec![0x12, 0x34, 0x56, 0x78]);
            u.set_dat_got_status(DatStatus::NotInDat, GotStatus::Got);
        }

        root.borrow_mut().child_add(Rc::clone(&missing));
        root.borrow_mut().child_add(Rc::clone(&unknown));

        FindFixes::scan_files(Rc::clone(&root));

        // Missing should now be marked CanBeFixed
        assert_eq!(missing.borrow().rep_status(), RepStatus::CanBeFixed);
        
        // Unknown should be marked NeededForFix
        assert_eq!(unknown.borrow().rep_status(), RepStatus::NeededForFix);
    }

    #[test]
    fn test_find_fixes_unneeded() {
        let root = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
        
        // Setup an unknown ROM that DOES NOT match anything
        let unknown = Rc::new(RefCell::new(RvFile::new(FileType::File)));
        {
            let mut u = unknown.borrow_mut();
            u.name = "junk.txt".to_string();
            u.size = Some(123);
            u.crc = Some(vec![0xFF, 0xFF]);
            u.set_dat_got_status(DatStatus::NotInDat, GotStatus::Got);
        }

        root.borrow_mut().child_add(Rc::clone(&unknown));

        FindFixes::scan_files(Rc::clone(&root));

        // Unknown should be marked MoveToSort since it's not needed
        assert_eq!(unknown.borrow().rep_status(), RepStatus::MoveToSort);
    }

    #[test]
    fn test_find_fixes_fallback_sha1_match() {
        let root = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
        
        // Setup a missing ROM with NO CRC, only SHA1
        let missing = Rc::new(RefCell::new(RvFile::new(FileType::File)));
        {
            let mut m = missing.borrow_mut();
            m.name = "game.rom".to_string();
            m.size = Some(1024);
            m.crc = None; // No CRC
            m.sha1 = Some(vec![0xAA, 0xBB, 0xCC, 0xDD]);
            m.set_dat_got_status(DatStatus::InDatCollect, GotStatus::NotGot);
        }
        
        // Setup an unknown ROM with NO CRC, only SHA1
        let unknown = Rc::new(RefCell::new(RvFile::new(FileType::File)));
        {
            let mut u = unknown.borrow_mut();
            u.name = "random.bin".to_string();
            u.size = Some(1024);
            u.crc = None; // No CRC
            u.sha1 = Some(vec![0xAA, 0xBB, 0xCC, 0xDD]);
            u.set_dat_got_status(DatStatus::NotInDat, GotStatus::Got);
        }

        root.borrow_mut().child_add(Rc::clone(&missing));
        root.borrow_mut().child_add(Rc::clone(&unknown));

        FindFixes::scan_files(Rc::clone(&root));

        // Missing should now be marked CanBeFixed via SHA1 fallback
        assert_eq!(missing.borrow().rep_status(), RepStatus::CanBeFixed);
        
        // Unknown should be marked NeededForFix
        assert_eq!(unknown.borrow().rep_status(), RepStatus::NeededForFix);
    }

    #[test]
    fn test_find_fixes_fallback_md5_match() {
        let root = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
        
        // Setup a missing ROM with NO CRC/SHA1, only MD5
        let missing = Rc::new(RefCell::new(RvFile::new(FileType::File)));
        {
            let mut m = missing.borrow_mut();
            m.name = "game.rom".to_string();
            m.size = Some(1024);
            m.crc = None;
            m.sha1 = None;
            m.md5 = Some(vec![0x11, 0x22, 0x33, 0x44]);
            m.set_dat_got_status(DatStatus::InDatCollect, GotStatus::NotGot);
        }
        
        // Setup an unknown ROM with NO CRC/SHA1, only MD5
        let unknown = Rc::new(RefCell::new(RvFile::new(FileType::File)));
        {
            let mut u = unknown.borrow_mut();
            u.name = "random.bin".to_string();
            u.size = Some(1024);
            u.crc = None;
            u.sha1 = None;
            u.md5 = Some(vec![0x11, 0x22, 0x33, 0x44]);
            u.set_dat_got_status(DatStatus::NotInDat, GotStatus::Got);
        }

        root.borrow_mut().child_add(Rc::clone(&missing));
        root.borrow_mut().child_add(Rc::clone(&unknown));

        FindFixes::scan_files(Rc::clone(&root));

        // Missing should now be marked CanBeFixed via MD5 fallback
        assert_eq!(missing.borrow().rep_status(), RepStatus::CanBeFixed);
        
        // Unknown should be marked NeededForFix
        assert_eq!(unknown.borrow().rep_status(), RepStatus::NeededForFix);
    }

    #[test]
    fn test_find_fixes_ignores_unselected_source_file() {
        let root = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));

        let source_dir = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
        source_dir.borrow_mut().tree_checked = TreeSelect::Selected;

        let source_file = Rc::new(RefCell::new(RvFile::new(FileType::File)));
        {
            let mut f = source_file.borrow_mut();
            f.name = "source.bin".to_string();
            f.size = Some(1024);
            f.crc = Some(vec![0x10, 0x20, 0x30, 0x40]);
            f.set_dat_got_status(DatStatus::NotInDat, GotStatus::Got);
            f.tree_checked = TreeSelect::UnSelected;
        }
        source_dir.borrow_mut().child_add(Rc::clone(&source_file));

        let target_dir = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
        target_dir.borrow_mut().tree_checked = TreeSelect::Selected;

        let missing_file = Rc::new(RefCell::new(RvFile::new(FileType::File)));
        {
            let mut f = missing_file.borrow_mut();
            f.name = "missing.bin".to_string();
            f.size = Some(1024);
            f.crc = Some(vec![0x10, 0x20, 0x30, 0x40]);
            f.set_dat_got_status(DatStatus::InDatCollect, GotStatus::NotGot);
            f.tree_checked = TreeSelect::Selected;
        }
        target_dir.borrow_mut().child_add(Rc::clone(&missing_file));

        root.borrow_mut().child_add(source_dir);
        root.borrow_mut().child_add(target_dir);

        FindFixes::scan_files(Rc::clone(&root));

        assert_eq!(missing_file.borrow().rep_status(), RepStatus::Missing);
        assert_eq!(source_file.borrow().rep_status(), RepStatus::Unknown);
    }

    #[test]
    fn test_find_fixes_allows_locked_source_branch() {
        let root = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));

        let source_dir = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
        source_dir.borrow_mut().tree_checked = TreeSelect::Locked;

        let source_file = Rc::new(RefCell::new(RvFile::new(FileType::File)));
        {
            let mut f = source_file.borrow_mut();
            f.name = "source.bin".to_string();
            f.size = Some(1024);
            f.crc = Some(vec![0x10, 0x20, 0x30, 0x40]);
            f.set_dat_got_status(DatStatus::NotInDat, GotStatus::Got);
            f.tree_checked = TreeSelect::Selected;
        }
        source_dir.borrow_mut().child_add(Rc::clone(&source_file));

        let target_dir = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
        target_dir.borrow_mut().tree_checked = TreeSelect::Selected;

        let missing_file = Rc::new(RefCell::new(RvFile::new(FileType::File)));
        {
            let mut f = missing_file.borrow_mut();
            f.name = "missing.bin".to_string();
            f.size = Some(1024);
            f.crc = Some(vec![0x10, 0x20, 0x30, 0x40]);
            f.set_dat_got_status(DatStatus::InDatCollect, GotStatus::NotGot);
            f.tree_checked = TreeSelect::Selected;
        }
        target_dir.borrow_mut().child_add(Rc::clone(&missing_file));

        root.borrow_mut().child_add(source_dir);
        root.borrow_mut().child_add(target_dir);

        FindFixes::scan_files(Rc::clone(&root));

        assert_eq!(missing_file.borrow().rep_status(), RepStatus::CanBeFixed);
        assert_eq!(source_file.borrow().rep_status(), RepStatus::NeededForFix);
    }
}
