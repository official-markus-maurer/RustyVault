use crate::rv_file::RvFile;
use std::cell::RefCell;
use std::rc::Rc;

/// Statistical accumulator for tracking the health of the ROM database.
///
/// `RepairStatus` is calculated dynamically by traversing the tree. It aggregates the
/// `RepStatus` of every individual file (e.g. `Correct`, `Missing`, `CanBeFixed`) to bubble
/// up folder-level and global statistics. This powers the main UI counters and progress bars.
///
/// Implementation notes:
/// - Uses per-node memoization (`RvFile.cached_stats`) to avoid recalculating unchanged subtrees.
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
    /// Number of files marked as corrupt-family issues
    pub roms_corrupt: i32,
    /// Number of files marked as `CanBeFixed`
    pub roms_fixes: i32,
    /// Number of files currently located in `ToSort` (`RepStatus::InToSort`)
    pub roms_in_to_sort: i32,
    /// Number of files marked as `NotCollected`
    pub roms_not_collected: i32,
    /// Number of files marked as `UnNeeded`
    pub roms_unneeded: i32,
    /// Number of files marked as `Unknown`
    pub roms_unknown: i32,
}

impl RepairStatus {
    pub fn dominant_rep_status(root: Rc<RefCell<RvFile>>) -> crate::enums::RepStatus {
        use crate::enums::RepStatus;
        use dat_reader::enums::GotStatus;

        const DISPLAY_ORDER: [RepStatus; 24] = [
            RepStatus::Error,
            RepStatus::UnSet,
            RepStatus::UnScanned,
            RepStatus::DirCorrupt,
            RepStatus::MoveToCorrupt,
            RepStatus::CorruptCanBeFixed,
            RepStatus::CanBeFixedMIA,
            RepStatus::CanBeFixed,
            RepStatus::MoveToSort,
            RepStatus::Delete,
            RepStatus::NeededForFix,
            RepStatus::Rename,
            RepStatus::Corrupt,
            RepStatus::Unknown,
            RepStatus::UnNeeded,
            RepStatus::Incomplete,
            RepStatus::Missing,
            RepStatus::MissingMIA,
            RepStatus::CorrectMIA,
            RepStatus::Correct,
            RepStatus::InToSort,
            RepStatus::NotCollected,
            RepStatus::Ignore,
            RepStatus::Deleted,
        ];

        if root.borrow().got_status() == GotStatus::FileLocked {
            return RepStatus::UnScanned;
        }

        let mut counts = vec![0u32; RepStatus::EndValue as usize];

        fn add_counts(node: Rc<RefCell<RvFile>>, counts: &mut [u32]) {
            let (rep_status, is_dir, children) = {
                let n = node.borrow();
                (n.rep_status(), n.is_directory(), n.children.clone())
            };
            let idx = rep_status as usize;
            if idx < counts.len() {
                counts[idx] += 1;
            }
            if is_dir {
                for child in children {
                    add_counts(child, counts);
                }
            }
        }

        let is_dir = root.borrow().is_directory();
        if is_dir {
            let children = root.borrow().children.clone();
            for child in children {
                add_counts(child, &mut counts);
            }
        } else {
            add_counts(Rc::clone(&root), &mut counts);
        }

        for status in DISPLAY_ORDER {
            let idx = status as usize;
            if idx < counts.len() && counts[idx] > 0 {
                return status;
            }
        }

        RepStatus::UnScanned
    }

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
            roms_corrupt: 0,
            roms_fixes: 0,
            roms_in_to_sort: 0,
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
        if n.dir_status.is_some() {
            n.dir_status = Some(crate::enums::ReportStatus::Unknown);
        }

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
        self.roms_correct
    }

    /// Returns the total number of missing or corrupt items.
    pub fn count_missing(&self) -> i32 {
        self.roms_missing + self.roms_fixes
    }

    /// Returns the total number of files that currently require fix work,
    /// including actionable cleanup states such as `UnNeeded`.
    pub fn count_fixes_needed(&self) -> i32 {
        self.roms_fixes + self.roms_unknown + self.roms_unneeded
    }

    fn synthesized_report_status(&self) -> crate::enums::ReportStatus {
        let merged_roms = self.roms_not_collected + self.roms_unneeded;
        let correct_roms = self.count_correct();
        let missing_roms = self.roms_missing;
        let plain_missing_roms = missing_roms - self.roms_corrupt;

        if self.total_roms == 0 || self.roms_unknown == self.total_roms {
            crate::enums::ReportStatus::Unknown
        } else if self.roms_corrupt == self.total_roms
            || (self.roms_corrupt > 0
                && plain_missing_roms == 0
                && correct_roms + merged_roms + self.roms_corrupt + self.roms_fixes
                    == self.total_roms)
        {
            crate::enums::ReportStatus::Corrupt
        } else if merged_roms == self.total_roms {
            if self.roms_unneeded > 0 && self.roms_not_collected == 0 {
                crate::enums::ReportStatus::UnNeeded
            } else {
                crate::enums::ReportStatus::NotCollected
            }
        } else if self.roms_in_to_sort == self.total_roms || self.roms_fixes == self.total_roms {
            crate::enums::ReportStatus::InToSort
        } else if correct_roms == self.total_roms {
            crate::enums::ReportStatus::Correct
        } else if missing_roms > 0 {
            crate::enums::ReportStatus::Missing
        } else if self.roms_fixes > 0 || self.roms_in_to_sort > 0 {
            crate::enums::ReportStatus::InToSort
        } else {
            crate::enums::ReportStatus::Unknown
        }
    }

    pub fn synthesized_dir_status(&self) -> crate::enums::ReportStatus {
        self.synthesized_report_status()
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
                let synthesized_status = if node.dir_status.is_some() {
                    Some(cached.synthesized_report_status())
                } else {
                    None
                };
                drop(node);
                if let Some(report_status) = synthesized_status {
                    root.borrow_mut().dir_status = Some(report_status);
                }
                let node = root.borrow();
                let cached = node.cached_stats.as_ref().unwrap();
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
                self.roms_corrupt += cached.roms_corrupt;
                self.roms_fixes += cached.roms_fixes;
                self.roms_in_to_sort += cached.roms_in_to_sort;
                self.roms_not_collected += cached.roms_not_collected;
                self.roms_unneeded += cached.roms_unneeded;
                self.roms_unknown += cached.roms_unknown;
                return;
            }

            is_dir = node.is_directory();
            is_file = node.is_file();
            is_game = node.game.is_some();
            rep_status = node.rep_status();
            children = if is_dir {
                node.children.clone()
            } else {
                Vec::new()
            };
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
                node_stats.roms_corrupt += child_status.roms_corrupt;
                node_stats.roms_fixes += child_status.roms_fixes;
                node_stats.roms_in_to_sort += child_status.roms_in_to_sort;
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
                }
                RepStatus::Missing
                | RepStatus::DirMissing
                | RepStatus::Corrupt
                | RepStatus::DirCorrupt
                | RepStatus::Incomplete => node_stats.games_missing += 1,
                RepStatus::MissingMIA => {
                    node_stats.games_missing += 1;
                    node_stats.games_missing_mia += 1;
                }
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
                }
                RepStatus::Missing | RepStatus::DirMissing => node_stats.roms_missing += 1,
                RepStatus::MissingMIA => {
                    node_stats.roms_missing += 1;
                    node_stats.roms_missing_mia += 1;
                }
                RepStatus::Corrupt | RepStatus::DirCorrupt | RepStatus::Incomplete => {
                    node_stats.roms_corrupt += 1;
                    node_stats.roms_missing += 1;
                }
                RepStatus::CanBeFixed
                | RepStatus::CanBeFixedMIA
                | RepStatus::CorruptCanBeFixed
                | RepStatus::MoveToSort
                | RepStatus::MoveToCorrupt
                | RepStatus::Delete
                | RepStatus::Deleted
                | RepStatus::NeededForFix
                | RepStatus::Rename
                | RepStatus::IncompleteRemove => node_stats.roms_fixes += 1,
                RepStatus::InToSort | RepStatus::DirInToSort => node_stats.roms_in_to_sort += 1,
                RepStatus::NotCollected => node_stats.roms_not_collected += 1,
                RepStatus::UnNeeded => node_stats.roms_unneeded += 1,
                RepStatus::Unknown | RepStatus::DirUnknown | RepStatus::UnScanned => {
                    node_stats.roms_unknown += 1
                }
                _ => {}
            }
        }

        // Cache the result for this node (even if it's a directory, so the UI can show its aggregated stats!)
        {
            let mut node = root.borrow_mut();
            node.cached_stats = Some(node_stats);
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
        self.roms_corrupt += node_stats.roms_corrupt;
        self.roms_fixes += node_stats.roms_fixes;
        self.roms_in_to_sort += node_stats.roms_in_to_sort;
        self.roms_not_collected += node_stats.roms_not_collected;
        self.roms_unneeded += node_stats.roms_unneeded;
        self.roms_unknown += node_stats.roms_unknown;
    }
}

impl Default for RepairStatus {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
#[path = "tests/repair_status_tests.rs"]
mod tests;
