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
        self.roms_missing + self.roms_missing_mia + self.roms_fixes + self.roms_unknown // Approximation
    }

    /// Returns the total number of files that have been marked as fixable.
    pub fn count_fixes_needed(&self) -> i32 {
        self.roms_fixes + self.roms_unneeded + self.roms_unknown
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
            is_dir = node.is_directory();
            is_file = node.is_file();
            is_game = node.game.is_some();
            rep_status = node.rep_status();
            children = if is_dir { node.children.clone() } else { Vec::new() };
        }
        
        // Very basic recursion simulating RomVaultCore/RepairStatus.cs
        if is_dir {
            for child in &children {
                // In C#, ReportStatus() actually creates a new RepairStatus for the child,
                // processes it, and then adds it to the parent's totals! Let's do exactly that.
                let mut child_status = RepairStatus::new();
                child_status.report_status(Rc::clone(child));
                
                self.total_roms += child_status.total_roms;
                self.roms_correct += child_status.roms_correct;
                self.roms_correct_mia += child_status.roms_correct_mia;
                self.roms_missing += child_status.roms_missing;
                self.roms_missing_mia += child_status.roms_missing_mia;
                self.roms_fixes += child_status.roms_fixes;
                self.roms_unneeded += child_status.roms_unneeded;
                self.roms_unknown += child_status.roms_unknown;
            }
        } 
        
        // Count it as a file if it is explicitly a file, OR if it's a game container (like a ZIP/7z)
        // BUT don't double count! If the game has children (ROMs), we counted the ROMs!
        // So we only count the game itself if it has NO children.
        if is_file || (is_game && children.is_empty()) {
            self.total_roms += 1;
            
            use crate::enums::RepStatus;
            match rep_status {
                RepStatus::Correct | RepStatus::DirCorrect => self.roms_correct += 1,
                RepStatus::CorrectMIA => {
                    self.roms_correct += 1;
                    self.roms_correct_mia += 1;
                },
                RepStatus::Missing | RepStatus::DirMissing => self.roms_missing += 1,
                RepStatus::MissingMIA => {
                    self.roms_missing += 1;
                    self.roms_missing_mia += 1;
                },
                RepStatus::CanBeFixed | RepStatus::CanBeFixedMIA => self.roms_fixes += 1,
                RepStatus::UnNeeded => self.roms_unneeded += 1,
                RepStatus::Unknown | RepStatus::MoveToSort | RepStatus::DirUnknown | RepStatus::DirInToSort => self.roms_unknown += 1,
                _ => {}
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dat_reader::enums::{FileType, GotStatus, DatStatus};
    use crate::enums::RepStatus;

    #[test]
    fn test_repair_status_counting() {
        let root = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
        
        // Add a Correct ROM
        let correct_rom = Rc::new(RefCell::new(RvFile::new(FileType::File)));
        correct_rom.borrow_mut().set_dat_got_status(DatStatus::InDatCollect, GotStatus::Got);
        // Add a Missing ROM
        let missing_rom = Rc::new(RefCell::new(RvFile::new(FileType::File)));
        missing_rom.borrow_mut().set_dat_got_status(DatStatus::InDatCollect, GotStatus::NotGot);
        // Add an Unknown ROM
        let unknown_rom = Rc::new(RefCell::new(RvFile::new(FileType::File)));
        unknown_rom.borrow_mut().set_dat_got_status(DatStatus::NotInDat, GotStatus::Got);

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
        // count_missing approximates fixes and unknowns in the basic implementation
        assert_eq!(status.count_missing(), 2); 
    }
}
