use crate::rv_file::RvFile;
use std::rc::Rc;
use std::cell::RefCell;

#[derive(Clone, Copy)]
pub struct RepairStatus {
    pub total_games: i32,
    pub total_roms: i32,
    
    pub games_correct: i32,
    pub games_missing: i32,
    pub games_missing_mia: i32,
    pub games_fixes: i32,
    
    pub roms_correct: i32,
    pub roms_correct_mia: i32,
    pub roms_missing: i32,
    pub roms_missing_mia: i32,
    pub roms_fixes: i32,
    pub roms_unneeded: i32,
    pub roms_unknown: i32,
}

impl RepairStatus {
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

    pub fn count_correct(&self) -> i32 {
        self.roms_correct + self.roms_correct_mia
    }

    pub fn count_missing(&self) -> i32 {
        self.roms_missing + self.roms_missing_mia + self.roms_fixes + self.roms_unknown // Approximation
    }

    pub fn count_fixes_needed(&self) -> i32 {
        self.roms_fixes + self.roms_unneeded + self.roms_unknown
    }

    pub fn report_status(&mut self, root: Rc<RefCell<RvFile>>) {
        let is_dir;
        let rep_status;
        let children;
        
        {
            let node = root.borrow();
            is_dir = node.is_directory();
            rep_status = node.rep_status();
            children = if is_dir { node.children.clone() } else { Vec::new() };
        }
        
        // Very basic recursion simulating RomVaultCore/RepairStatus.cs
        if is_dir {
            for child in children {
                self.report_status(child);
            }
        } else {
            self.total_roms += 1;
            
            use crate::enums::RepStatus;
            match rep_status {
                RepStatus::Correct => self.roms_correct += 1,
                RepStatus::CorrectMIA => {
                    self.roms_correct += 1;
                    self.roms_correct_mia += 1;
                },
                RepStatus::Missing => self.roms_missing += 1,
                RepStatus::MissingMIA => {
                    self.roms_missing += 1;
                    self.roms_missing_mia += 1;
                },
                RepStatus::CanBeFixed | RepStatus::CanBeFixedMIA => self.roms_fixes += 1,
                RepStatus::UnNeeded => self.roms_unneeded += 1,
                RepStatus::Unknown | RepStatus::MoveToSort => self.roms_unknown += 1,
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
