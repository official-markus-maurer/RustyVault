use crate::rv_file::RvFile;
use std::rc::Rc;
use std::cell::RefCell;

/// Statistical accumulator for tracking the health of the ROM database.
/// 
/// `RepairStatus` is calculated dynamically by traversing the tree. It aggregates the 
/// `RepStatus` of every individual file (e.g. `Correct`, `Missing`, `CanBeFixed`) to bubble 
/// up folder-level and global statistics. This powers the main UI counters and progress bars.
/// 
/// Differences from C#:
/// - The logic is nearly identical to C#'s `ReportStatus` recursive aggregation.
/// - Rust utilizes an internal caching layer (`cached_stats` on `RvFile`) to dramatically speed up 
///   subsequent tree traversals by memoizing the `RepairStatus` of unchanged branches.
#[derive(Clone, Copy)]
pub struct RepairStatus {
    /// Total number of games
    pub total_games: i32,
    /// Total number of ROMs
    pub total_roms: i32,
    
    /// Number of games marked as `Correct`
    pub games_correct: i32,
    /// Number of games marked as `Missing`
    pub games_missing: i32,
    /// Number of games marked as `Missing MIA`
    pub games_missing_mia: i32,
    /// Number of games marked as `CanBeFixed`
    pub games_fixes: i32,
    
    /// Number of files marked as `Correct`
    pub roms_correct: i32,
    /// Number of files marked as `CorrectMIA`
    pub roms_correct_mia: i32,
    /// Number of files marked as `Missing`
    pub roms_missing: i32,
    /// Number of files marked as `MIA`
    pub roms_missing_mia: i32,
    /// Number of files marked as `CanBeFixed`
    pub roms_fixes: i32,
    /// Number of files marked as `NotCollected`
    pub roms_not_collected: i32,
    /// Number of files marked as `UnNeeded`
    pub roms_unneeded: i32,
    /// Number of files marked as `Unknown`
    pub roms_unknown: i32,
}

impl RepairStatus {
    /// Initializes an empty `RepairStatus` struct with all counters set to zero.
    pub fn new() -> Self {
        Self {
            total_games: 0,
            total_roms: 0,
            games_correct: 0,
            games_missing: 0,
            games_missing_mia: 0,
            games_fixes: 0,
            roms_correct: 0,
            roms_correct_mia: 0,
            roms_missing: 0,
            roms_missing_mia: 0,
            roms_fixes: 0,
            roms_not_collected: 0,
            roms_unneeded: 0,
            roms_unknown: 0,
        }
    }

    /// Recursively clears the `cached_stats` of every node in the provided tree branch.
    /// This forces a full recalculation of statistics on the next tree traversal.
    pub fn report_status_reset(root: Rc<RefCell<RvFile>>) {
        let mut n = root.borrow_mut();
        n.rep_status_reset();
        n.cached_stats = None;

        if !n.is_directory() {
            return;
        }

        let children = n.children.clone();
        drop(n);

        for child in children {
            Self::report_status_reset(child);
        }
    }

    /// Returns the total number of correct items.
    pub fn count_correct(&self) -> i32 {
        self.roms_correct + self.roms_correct_mia
    }

    /// Returns the total number of missing or corrupt items.
    pub fn count_missing(&self) -> i32 {
        self.roms_missing + self.roms_missing_mia + self.roms_fixes
    }

    /// Returns the total number of files that have been marked as fixable.
    pub fn count_fixes_needed(&self) -> i32 {
        self.roms_fixes + self.roms_unknown
    }

    fn synthesized_report_status(&self) -> crate::enums::ReportStatus {
        let merged_roms = self.roms_not_collected + self.roms_unneeded;
        let correct_roms = self.count_correct();
        let missing_roms = self.roms_missing + self.roms_missing_mia;

        if self.total_roms == 0 {
            crate::enums::ReportStatus::Unknown
        } else if self.roms_unknown == self.total_roms {
            crate::enums::ReportStatus::Unknown
        } else if merged_roms == self.total_roms {
            if self.roms_unneeded > 0 && self.roms_not_collected == 0 {
                crate::enums::ReportStatus::UnNeeded
            } else {
                crate::enums::ReportStatus::NotCollected
            }
        } else if self.roms_fixes == self.total_roms {
            crate::enums::ReportStatus::InToSort
        } else if correct_roms == self.total_roms {
            crate::enums::ReportStatus::Correct
        } else if missing_roms > 0 {
            crate::enums::ReportStatus::Missing
        } else if self.roms_fixes > 0 {
            crate::enums::ReportStatus::InToSort
        } else if correct_roms > 0 {
            crate::enums::ReportStatus::Correct
        } else {
            crate::enums::ReportStatus::Unknown
        }
    }

    /// Recursively traverses a tree branch and calculates its exact `RepairStatus` by 
    /// aggregating the status of all children. Automatically utilizes and updates `cached_stats`.
    pub fn report_status(&mut self, root: Rc<RefCell<RvFile>>) {
        let is_dir;
        let is_file;
        let is_game;
        let rep_status;
        let children;
        
        {
            let node = root.borrow();
            // If we already have cached stats, just use them and return immediately!
            // BUT WE MUST ALSO ADD THEM TO `self` SO THE PARENT GETS THEM!
            if let Some(cached) = &node.cached_stats {
                self.total_games += cached.total_games;
                self.total_roms += cached.total_roms;
                self.games_correct += cached.games_correct;
                self.games_missing += cached.games_missing;
                self.games_missing_mia += cached.games_missing_mia;
                self.games_fixes += cached.games_fixes;
                self.roms_correct += cached.roms_correct;
                self.roms_correct_mia += cached.roms_correct_mia;
                self.roms_missing += cached.roms_missing;
                self.roms_missing_mia += cached.roms_missing_mia;
                self.roms_fixes += cached.roms_fixes;
                self.roms_not_collected += cached.roms_not_collected;
                self.roms_unneeded += cached.roms_unneeded;
                self.roms_unknown += cached.roms_unknown;
                return;
            }

            is_dir = node.is_directory();
            is_file = node.is_file();
            is_game = node.game.is_some();
            rep_status = node.rep_status();
            children = if is_dir { node.children.clone() } else { Vec::new() };
        }
        
        // We calculate stats for this node specifically
        let mut node_stats = RepairStatus::new();
        
        if is_dir {
            for child in &children {
                let mut child_status = RepairStatus::new();
                child_status.report_status(Rc::clone(child));
                
                // Add the child's *aggregate* stats to our node's running total.
                // Since child_status.report_status automatically adds to its `self`,
                // child_status ALREADY contains the full aggregate of that branch!
                node_stats.total_games += child_status.total_games;
                node_stats.total_roms += child_status.total_roms;
                node_stats.games_correct += child_status.games_correct;
                node_stats.games_missing += child_status.games_missing;
                node_stats.games_missing_mia += child_status.games_missing_mia;
                node_stats.games_fixes += child_status.games_fixes;
                node_stats.roms_correct += child_status.roms_correct;
                node_stats.roms_correct_mia += child_status.roms_correct_mia;
                node_stats.roms_missing += child_status.roms_missing;
                node_stats.roms_missing_mia += child_status.roms_missing_mia;
                node_stats.roms_fixes += child_status.roms_fixes;
                node_stats.roms_not_collected += child_status.roms_not_collected;
                node_stats.roms_unneeded += child_status.roms_unneeded;
                node_stats.roms_unknown += child_status.roms_unknown;
            }
        } 
        
        // Count it as a file if it is explicitly a file, OR if it's a game container (like a ZIP/7z)
        // BUT don't double count! If the game has children (ROMs), we counted the ROMs!
        // In RomVault, Dir nodes that act as games but have no children are counted as files.
        let count_as_file = is_file || (is_game && children.is_empty());

        use crate::enums::RepStatus;

        if is_game {
            node_stats.total_games += 1;
            match rep_status {
                RepStatus::Correct | RepStatus::DirCorrect => node_stats.games_correct += 1,
                RepStatus::CorrectMIA => {
                    node_stats.games_correct += 1;
                    node_stats.games_missing_mia += 1;
                },
                RepStatus::Missing | RepStatus::DirMissing | RepStatus::Corrupt | RepStatus::DirCorrupt | RepStatus::Incomplete => {
                    node_stats.games_missing += 1
                }
                RepStatus::MissingMIA => {
                    node_stats.games_missing += 1;
                    node_stats.games_missing_mia += 1;
                },
                RepStatus::CanBeFixed
                | RepStatus::CanBeFixedMIA
                | RepStatus::CorruptCanBeFixed
                | RepStatus::InToSort
                | RepStatus::DirInToSort
                | RepStatus::MoveToSort
                | RepStatus::MoveToCorrupt
                | RepStatus::Delete
                | RepStatus::Deleted
                | RepStatus::NeededForFix
                | RepStatus::Rename
                | RepStatus::IncompleteRemove => node_stats.games_fixes += 1,
                _ => {}
            }
        }

        if count_as_file {
            node_stats.total_roms += 1;

            match rep_status {
                RepStatus::Correct | RepStatus::DirCorrect => node_stats.roms_correct += 1,
                RepStatus::CorrectMIA => {
                    node_stats.roms_correct += 1;
                    node_stats.roms_correct_mia += 1;
                },
                RepStatus::Missing | RepStatus::DirMissing | RepStatus::Corrupt | RepStatus::DirCorrupt | RepStatus::Incomplete => {
                    node_stats.roms_missing += 1
                }
                RepStatus::MissingMIA => {
                    node_stats.roms_missing += 1;
                    node_stats.roms_missing_mia += 1;
                },
                RepStatus::CanBeFixed
                | RepStatus::CanBeFixedMIA
                | RepStatus::CorruptCanBeFixed
                | RepStatus::InToSort
                | RepStatus::DirInToSort
                | RepStatus::MoveToSort
                | RepStatus::MoveToCorrupt
                | RepStatus::Delete
                | RepStatus::Deleted
                | RepStatus::NeededForFix
                | RepStatus::Rename
                | RepStatus::IncompleteRemove => node_stats.roms_fixes += 1,
                RepStatus::NotCollected => node_stats.roms_not_collected += 1,
                RepStatus::UnNeeded => node_stats.roms_unneeded += 1,
                RepStatus::Unknown | RepStatus::DirUnknown | RepStatus::UnScanned => node_stats.roms_unknown += 1,
                _ => {}
            }
        }

        // Cache the result for this node (even if it's a directory, so the UI can show its aggregated stats!)
        {
            let mut node = root.borrow_mut();
            node.cached_stats = Some(node_stats.clone());
            if node.dir_status.is_some() {
                node.dir_status = Some(node_stats.synthesized_report_status());
            }
        }

        // Add this node's stats to the parent's running total
        self.total_games += node_stats.total_games;
        self.total_roms += node_stats.total_roms;
        self.games_correct += node_stats.games_correct;
        self.games_missing += node_stats.games_missing;
        self.games_missing_mia += node_stats.games_missing_mia;
        self.games_fixes += node_stats.games_fixes;
        self.roms_correct += node_stats.roms_correct;
        self.roms_correct_mia += node_stats.roms_correct_mia;
        self.roms_missing += node_stats.roms_missing;
        self.roms_missing_mia += node_stats.roms_missing_mia;
        self.roms_fixes += node_stats.roms_fixes;
        self.roms_not_collected += node_stats.roms_not_collected;
        self.roms_unneeded += node_stats.roms_unneeded;
        self.roms_unknown += node_stats.roms_unknown;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dat_reader::enums::{FileType, GotStatus, DatStatus};
    use crate::rv_game::RvGame;

    #[test]
    fn test_repair_status_counting() {
        let root = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
        
        // Add a Correct ROM
        let correct_rom = Rc::new(RefCell::new(RvFile::new(FileType::File)));
        correct_rom.borrow_mut().set_dat_got_status(DatStatus::InDatCollect, GotStatus::Got);
        correct_rom.borrow_mut().rep_status_reset();
        
        // Add a Missing ROM
        let missing_rom = Rc::new(RefCell::new(RvFile::new(FileType::File)));
        missing_rom.borrow_mut().set_dat_got_status(DatStatus::InDatCollect, GotStatus::NotGot);
        missing_rom.borrow_mut().rep_status_reset();
        
        // Add an Unknown ROM
        let unknown_rom = Rc::new(RefCell::new(RvFile::new(FileType::File)));
        unknown_rom.borrow_mut().set_dat_got_status(DatStatus::NotInDat, GotStatus::Got);
        unknown_rom.borrow_mut().rep_status_reset();

        root.borrow_mut().child_add(correct_rom);
        root.borrow_mut().child_add(missing_rom);
        root.borrow_mut().child_add(unknown_rom);

        let mut status = RepairStatus::new();
        status.report_status(Rc::clone(&root));

        assert_eq!(status.total_roms, 3);
        assert_eq!(status.roms_correct, 1);
        assert_eq!(status.roms_missing, 1);
        assert_eq!(status.roms_unknown, 1);

        assert_eq!(status.count_correct(), 1);
        assert_eq!(status.count_missing(), 1);
        assert_eq!(status.count_fixes_needed(), 1);
    }

    #[test]
    fn test_repair_status_fix_count_excludes_unneeded_roms() {
        let root = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));

        let fixable_rom = Rc::new(RefCell::new(RvFile::new(FileType::File)));
        {
            let mut rom = fixable_rom.borrow_mut();
            rom.set_dat_got_status(DatStatus::InDatCollect, GotStatus::NotGot);
            rom.set_rep_status(crate::enums::RepStatus::CanBeFixed);
        }

        let merged_rom = Rc::new(RefCell::new(RvFile::new(FileType::File)));
        {
            let mut rom = merged_rom.borrow_mut();
            rom.set_dat_got_status(DatStatus::InDatMerged, GotStatus::Got);
            rom.set_rep_status(crate::enums::RepStatus::UnNeeded);
        }

        root.borrow_mut().child_add(fixable_rom);
        root.borrow_mut().child_add(merged_rom);

        let mut status = RepairStatus::new();
        status.report_status(Rc::clone(&root));

        assert_eq!(status.roms_fixes, 1);
        assert_eq!(status.roms_unneeded, 1);
        assert_eq!(status.count_fixes_needed(), 1);
    }

    #[test]
    fn test_repair_status_tracks_not_collected_roms_separately() {
        let root = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));

        let merged_missing_rom = Rc::new(RefCell::new(RvFile::new(FileType::File)));
        {
            let mut rom = merged_missing_rom.borrow_mut();
            rom.set_dat_got_status(DatStatus::InDatMerged, GotStatus::NotGot);
            rom.rep_status_reset();
        }

        root.borrow_mut().child_add(merged_missing_rom);

        let mut status = RepairStatus::new();
        status.report_status(Rc::clone(&root));

        assert_eq!(status.total_roms, 1);
        assert_eq!(status.roms_not_collected, 1);
        assert_eq!(status.roms_unneeded, 0);
        assert_eq!(status.count_missing(), 0);
        assert_eq!(status.count_fixes_needed(), 0);
    }

    #[test]
    fn test_repair_status_missing_count_excludes_unknown_roms() {
        let root = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));

        let missing_rom = Rc::new(RefCell::new(RvFile::new(FileType::File)));
        {
            let mut rom = missing_rom.borrow_mut();
            rom.set_dat_got_status(DatStatus::InDatCollect, GotStatus::NotGot);
            rom.rep_status_reset();
        }

        let unknown_rom = Rc::new(RefCell::new(RvFile::new(FileType::File)));
        {
            let mut rom = unknown_rom.borrow_mut();
            rom.set_dat_got_status(DatStatus::NotInDat, GotStatus::Got);
            rom.rep_status_reset();
        }

        root.borrow_mut().child_add(missing_rom);
        root.borrow_mut().child_add(unknown_rom);

        let mut status = RepairStatus::new();
        status.report_status(Rc::clone(&root));

        assert_eq!(status.roms_missing, 1);
        assert_eq!(status.roms_unknown, 1);
        assert_eq!(status.count_missing(), 1);
    }

    #[test]
    fn test_repair_status_buckets_runtime_status_families_consistently() {
        let root = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));

        let missing_family = Rc::new(RefCell::new(RvFile::new(FileType::File)));
        missing_family.borrow_mut().set_rep_status(crate::enums::RepStatus::Corrupt);

        let fix_family = Rc::new(RefCell::new(RvFile::new(FileType::File)));
        fix_family.borrow_mut().set_rep_status(crate::enums::RepStatus::NeededForFix);

        let unknown_family = Rc::new(RefCell::new(RvFile::new(FileType::File)));
        unknown_family.borrow_mut().set_rep_status(crate::enums::RepStatus::UnScanned);

        root.borrow_mut().child_add(missing_family);
        root.borrow_mut().child_add(fix_family);
        root.borrow_mut().child_add(unknown_family);

        let mut status = RepairStatus::new();
        status.report_status(Rc::clone(&root));

        assert_eq!(status.roms_missing, 1);
        assert_eq!(status.roms_fixes, 1);
        assert_eq!(status.roms_unknown, 1);
        assert_eq!(status.count_missing(), 2);
        assert_eq!(status.count_fixes_needed(), 2);
    }

    #[test]
    fn test_repair_status_tracks_game_counters_for_game_nodes() {
        let root = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));

        let game = Rc::new(RefCell::new(RvFile::new(FileType::Zip)));
        {
            let mut node = game.borrow_mut();
            node.game = Some(Rc::new(RefCell::new(RvGame::from_description("Pac-Man"))));
            node.set_rep_status(crate::enums::RepStatus::CanBeFixed);
        }

        let rom = Rc::new(RefCell::new(RvFile::new(FileType::File)));
        rom.borrow_mut().set_rep_status(crate::enums::RepStatus::Correct);
        game.borrow_mut().child_add(rom);
        root.borrow_mut().child_add(Rc::clone(&game));

        let mut status = RepairStatus::new();
        status.report_status(Rc::clone(&root));

        assert_eq!(status.total_games, 1);
        assert_eq!(status.games_fixes, 1);
        assert_eq!(status.games_correct, 0);
        assert_eq!(status.total_roms, 1);
        assert_eq!(status.roms_correct, 1);
    }

    #[test]
    fn test_repair_status_uses_cached_game_counters() {
        let game = Rc::new(RefCell::new(RvFile::new(FileType::Zip)));
        {
            let mut node = game.borrow_mut();
            node.game = Some(Rc::new(RefCell::new(RvGame::from_description("Galaga"))));
            node.set_rep_status(crate::enums::RepStatus::MissingMIA);
        }

        let mut first_pass = RepairStatus::new();
        first_pass.report_status(Rc::clone(&game));

        let mut second_pass = RepairStatus::new();
        second_pass.report_status(Rc::clone(&game));

        assert_eq!(first_pass.total_games, 1);
        assert_eq!(first_pass.games_missing, 1);
        assert_eq!(first_pass.games_missing_mia, 1);
        assert_eq!(second_pass.total_games, 1);
        assert_eq!(second_pass.games_missing, 1);
        assert_eq!(second_pass.games_missing_mia, 1);
    }

    #[test]
    fn test_repair_status_synthesizes_dir_status_for_fix_only_branch() {
        let root = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));

        let fixable_rom = Rc::new(RefCell::new(RvFile::new(FileType::File)));
        fixable_rom.borrow_mut().set_rep_status(crate::enums::RepStatus::NeededForFix);
        root.borrow_mut().child_add(fixable_rom);

        let mut status = RepairStatus::new();
        status.report_status(Rc::clone(&root));

        assert_eq!(root.borrow().dir_status, Some(crate::enums::ReportStatus::InToSort));
    }

    #[test]
    fn test_repair_status_synthesizes_dir_status_for_merged_branch() {
        let root = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));

        let merged_rom = Rc::new(RefCell::new(RvFile::new(FileType::File)));
        merged_rom.borrow_mut().set_rep_status(crate::enums::RepStatus::UnNeeded);
        root.borrow_mut().child_add(merged_rom);

        let mut status = RepairStatus::new();
        status.report_status(Rc::clone(&root));

        assert_eq!(root.borrow().dir_status, Some(crate::enums::ReportStatus::UnNeeded));
    }
}
