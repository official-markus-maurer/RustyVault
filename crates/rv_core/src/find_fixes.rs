use std::rc::Rc;
use std::cell::RefCell;
use std::collections::HashMap;
use dat_reader::enums::{DatStatus, GotStatus};
use crate::enums::RepStatus;
use crate::rv_file::RvFile;

/// The logical matching engine for resolving missing ROMs.
/// 
/// `FindFixes` is responsible for calculating the logical repair state (`RepStatus`) of the 
/// database. It identifies missing files in the primary `RustyVault` and attempts to map them 
/// to available files sitting in `ToSort` using exact CRC/SHA1/MD5 hash matching.
/// 
/// Differences from C#:
/// - The C# reference uses complex multithreading/Task dispatching for hashing and checking rules.
/// - The Rust version currently implements a linear traversal and simple in-memory `HashMap` lookup
///   to perform hash matching between `Got` files and `Missing` files.
/// - CHDs and advanced rules (like size-only matching) are currently simplified compared to C#'s `FindFixes.cs`.
pub struct FindFixes;

impl FindFixes {
    /// Recursively scans the tree to pair `Missing` files with unassigned `Got` files.
    pub fn scan_files(root: Rc<RefCell<RvFile>>) {
        // Step 1: Reset Status
        Self::reset_status(Rc::clone(&root));

        // Step 2: Get Selected Files
        let mut files_got = Vec::new();
        let mut files_missing = Vec::new();
        Self::get_selected_files(Rc::clone(&root), true, &mut files_got, &mut files_missing);

        // Step 3: Group Got Files by Hashes to create O(1) lookup indexes
        let mut crc_map: HashMap<(u64, Vec<u8>), Vec<Rc<RefCell<RvFile>>>> = HashMap::new();
        let mut sha1_map: HashMap<(u64, Vec<u8>), Vec<Rc<RefCell<RvFile>>>> = HashMap::new();
        let mut md5_map: HashMap<(u64, Vec<u8>), Vec<Rc<RefCell<RvFile>>>> = HashMap::new();

        for got in &files_got {
            let got_ref = got.borrow();
            let size = got_ref.size.unwrap_or(0);
            
            if let Some(ref crc) = got_ref.crc {
                crc_map.entry((size, crc.clone())).or_default().push(Rc::clone(got));
            }
            if let Some(ref sha1) = got_ref.sha1 {
                sha1_map.entry((size, sha1.clone())).or_default().push(Rc::clone(got));
            }
            if let Some(ref md5) = got_ref.md5 {
                md5_map.entry((size, md5.clone())).or_default().push(Rc::clone(got));
            }
        }

        // Step 4: Match Missing files against Got indexes
        for missing in &files_missing {
            let mut missing_ref = missing.borrow_mut();
            let size = missing_ref.size.unwrap_or(0);

            let mut found_got = None;

            // Try to find a match by CRC first
            if let Some(ref crc) = missing_ref.crc {
                if let Some(got_list) = crc_map.get(&(size, crc.clone())) {
                    if let Some(first_got) = got_list.first() {
                        found_got = Some(Rc::clone(first_got));
                    }
                }
            }

            // Fallback to SHA1 if no CRC match or missing CRC
            if found_got.is_none() {
                if let Some(ref sha1) = missing_ref.sha1 {
                    if let Some(got_list) = sha1_map.get(&(size, sha1.clone())) {
                        if let Some(first_got) = got_list.first() {
                            found_got = Some(Rc::clone(first_got));
                        }
                    }
                }
            }

            // Fallback to MD5 if still no match
            if found_got.is_none() {
                if let Some(ref md5) = missing_ref.md5 {
                    if let Some(got_list) = md5_map.get(&(size, md5.clone())) {
                        if let Some(first_got) = got_list.first() {
                            found_got = Some(Rc::clone(first_got));
                        }
                    }
                }
            }

            // If we found a matching file, flag it as fixable
            let is_mia = missing_ref.dat_status() == DatStatus::InDatMIA;
            if let Some(got) = found_got {
                let is_corrupt = got.borrow().got_status() == GotStatus::Corrupt;
                
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
                if got_mut.rep_status() == RepStatus::UnScanned {
                    got_mut.set_rep_status(RepStatus::NeededForFix);
                }
                
                missing_ref.cached_stats = None;
                got_mut.cached_stats = None;
            } else {
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
        selected: bool,
        got_files: &mut Vec<Rc<RefCell<RvFile>>>,
        missing_files: &mut Vec<Rc<RefCell<RvFile>>>,
    ) {
        let n = node.borrow();
        
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
            // In full implementation, we check if tree node is selected
            Self::get_selected_files(child, selected, got_files, missing_files);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dat_reader::enums::FileType;

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
}
