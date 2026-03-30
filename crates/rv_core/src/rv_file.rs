use std::rc::{Rc, Weak};
use std::cell::RefCell;
use dat_reader::enums::{DatStatus, FileType, GotStatus, HeaderFileType, ZipStructure};
use crate::enums::{RepStatus, ReportStatus, ToSortDirType};
use crate::repair_status::RepairStatus;
use crate::rv_dat::RvDat;
use crate::rv_game::RvGame;

bitflags::bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize)]
    pub struct FileStatus: u32 {
        const NONE = 0;
        
        const SIZE_FROM_HEADER = 1 << 0;
        const CRC_FROM_HEADER = 1 << 1;
        const SHA1_FROM_HEADER = 1 << 2;
        const MD5_FROM_HEADER = 1 << 3;
        
        const ALT_SIZE_FROM_HEADER = 1 << 4;
        const ALT_CRC_FROM_HEADER = 1 << 5;
        const ALT_SHA1_FROM_HEADER = 1 << 6;
        const ALT_MD5_FROM_HEADER = 1 << 7;

        const SIZE_FROM_DAT = 1 << 8;
        const CRC_FROM_DAT = 1 << 9;
        const SHA1_FROM_DAT = 1 << 10;
        const MD5_FROM_DAT = 1 << 11;
        
        const ALT_SIZE_FROM_DAT = 1 << 12;
        const ALT_CRC_FROM_DAT = 1 << 13;
        const ALT_SHA1_FROM_DAT = 1 << 14;
        const ALT_MD5_FROM_DAT = 1 << 15;

        const DATE_FROM_DAT = 1 << 16;
        const HEADER_FILE_TYPE_FROM_DAT = 1 << 17;
        const HEADER_FILE_TYPE_FROM_HEADER = 1 << 18;

        const IS_ALT_FILE = 1 << 19;
    }
}

#[derive(serde::Serialize, serde::Deserialize)]
pub struct RvFile {
    pub name: String,
    pub file_name: String,
    
    #[serde(skip)]
    pub parent: Option<Weak<RefCell<RvFile>>>,
    #[serde(skip)]
    pub dat: Option<Rc<RefCell<RvDat>>>,
    
    // Serialized instead of `dat`. Used during cache load/save to avoid Rc duplication.
    pub dat_index_for_serde: Option<i32>,
    
    pub file_mod_time_stamp: i64,

    #[serde(skip)]
    pub search_found: bool,

    header_file_type: HeaderFileType,
    pub file_type: FileType,
    
    dat_status: DatStatus,
    got_status: GotStatus,
    rep_status: RepStatus,
    file_status: FileStatus,

    // RvDir specifics
    pub children: Vec<Rc<RefCell<RvFile>>>,
    #[serde(skip)]
    pub dir_status: Option<ReportStatus>,
    pub dir_dats: Vec<Rc<RefCell<RvDat>>>,

    zip_dat_struct: u8,
    pub zip_struct: ZipStructure,

    pub game: Option<Rc<RefCell<RvGame>>>,
    #[serde(skip)]
    pub ui_display_name: String,

    to_sort_type: ToSortDirType,

    // RvFile specifics
    pub size: Option<u64>,
    pub crc: Option<Vec<u8>>,
    pub sha1: Option<Vec<u8>>,
    pub md5: Option<Vec<u8>>,
    pub alt_size: Option<u64>,
    pub alt_crc: Option<Vec<u8>>,
    pub alt_sha1: Option<Vec<u8>>,
    pub alt_md5: Option<Vec<u8>>,

    pub merge: String,
    pub status: Option<String>,

    // UI specific traits ported from RvTreeRow.cs
    pub tree_expanded: bool,
    pub tree_checked: TreeSelect,

    #[serde(skip)]
    pub cached_stats: Option<RepairStatus>,
    
    // Temporarily holds the dat index during deserialization before relink_parents
    #[serde(skip)]
    pub tmp_dat_index: Option<i32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum TreeSelect {
    UnSelected,
    Selected,
    Locked,
}

impl RvFile {
    pub fn new(file_type: FileType) -> Self {
        Self {
            name: String::new(),
            file_name: String::new(),
            parent: None,
            dat: None,
            dat_index_for_serde: None,
            file_mod_time_stamp: i64::MIN,
            search_found: false,
            header_file_type: HeaderFileType::NOTHING,
            file_type,
            dat_status: DatStatus::NotInDat,
            got_status: GotStatus::NotGot,
            rep_status: RepStatus::UnSet,
            file_status: FileStatus::NONE,
            children: Vec::new(),
            dir_status: if file_type == FileType::Dir || file_type == FileType::Zip || file_type == FileType::SevenZip { Some(ReportStatus::Unknown) } else { None },
            dir_dats: Vec::new(),
            zip_dat_struct: 0,
            zip_struct: ZipStructure::None,
            game: None,
            ui_display_name: String::new(),
            to_sort_type: ToSortDirType::NONE,
            size: None,
            crc: None,
            sha1: None,
            md5: None,
            alt_size: None,
            alt_crc: None,
            alt_sha1: None,
            alt_md5: None,
            merge: String::new(),
            status: None,
            tree_expanded: false,
            tree_checked: TreeSelect::Selected,
            cached_stats: None,
            tmp_dat_index: None,
        }
    }

    pub fn is_directory(&self) -> bool {
        self.file_type == FileType::Dir || self.file_type == FileType::Zip || self.file_type == FileType::SevenZip
    }

    pub fn is_file(&self) -> bool {
        self.file_type == FileType::File || self.file_type == FileType::FileZip || self.file_type == FileType::FileSevenZip || self.file_type == FileType::FileOnly
    }

    pub fn child_add(&mut self, child: Rc<RefCell<RvFile>>) {
        self.children.push(child);
    }

    pub fn child_insert(&mut self, index: usize, child: Rc<RefCell<RvFile>>) {
        self.children.insert(index, child);
    }

    pub fn child_remove(&mut self, index: usize) {
        if index < self.children.len() {
            self.children.remove(index);
        }
    }

    pub fn dat_status(&self) -> DatStatus {
        self.dat_status
    }

    pub fn set_dat_status(&mut self, status: DatStatus) {
        self.dat_status = status;
    }

    pub fn got_status(&self) -> GotStatus {
        self.got_status
    }

    pub fn set_got_status(&mut self, status: GotStatus) {
        self.got_status = status;
    }

    pub fn rep_status(&self) -> RepStatus {
        self.rep_status
    }

    pub fn set_rep_status(&mut self, status: RepStatus) {
        self.rep_status = status;
    }

    pub fn rep_status_reset(&mut self) {
        // Rust port simplification of RomVaultCore/RvDB/rvFile.cs RepStatusReset
        self.search_found = false;
        
        let new_status = match (self.file_type, self.dat_status, self.got_status) {
            (FileType::File | FileType::Zip | FileType::SevenZip, dat_reader::enums::DatStatus::InDatCollect, dat_reader::enums::GotStatus::Got) => RepStatus::Correct,
            (FileType::File | FileType::Zip | FileType::SevenZip, dat_reader::enums::DatStatus::InDatCollect, dat_reader::enums::GotStatus::NotGot) => RepStatus::Missing,
            (FileType::File | FileType::Zip | FileType::SevenZip, dat_reader::enums::DatStatus::InDatCollect, dat_reader::enums::GotStatus::Corrupt) => RepStatus::Corrupt,
            (FileType::File | FileType::Zip | FileType::SevenZip, dat_reader::enums::DatStatus::InDatMIA, dat_reader::enums::GotStatus::NotGot) => RepStatus::MissingMIA,
            (FileType::File | FileType::Zip | FileType::SevenZip, dat_reader::enums::DatStatus::InDatMIA, dat_reader::enums::GotStatus::Got) => RepStatus::CorrectMIA,
            (FileType::File | FileType::Zip | FileType::SevenZip, dat_reader::enums::DatStatus::InToSort, dat_reader::enums::GotStatus::Got) => RepStatus::InToSort,
            (FileType::File | FileType::Zip | FileType::SevenZip, dat_reader::enums::DatStatus::InToSort, dat_reader::enums::GotStatus::NotGot) => RepStatus::Deleted,
            (FileType::File | FileType::Zip | FileType::SevenZip, dat_reader::enums::DatStatus::NotInDat, dat_reader::enums::GotStatus::Got) => RepStatus::Unknown,
            (FileType::File | FileType::Zip | FileType::SevenZip, dat_reader::enums::DatStatus::NotInDat, dat_reader::enums::GotStatus::NotGot) => RepStatus::Deleted,
            (FileType::Dir, dat_reader::enums::DatStatus::InDatCollect, dat_reader::enums::GotStatus::Got) => RepStatus::DirCorrect,
            (FileType::Dir, dat_reader::enums::DatStatus::InDatCollect, dat_reader::enums::GotStatus::NotGot) => RepStatus::DirMissing,
            (FileType::Dir, dat_reader::enums::DatStatus::NotInDat, dat_reader::enums::GotStatus::Got) => RepStatus::DirUnknown,
            (FileType::Dir, dat_reader::enums::DatStatus::InToSort, dat_reader::enums::GotStatus::Got) => RepStatus::DirInToSort,
            _ => RepStatus::UnScanned,
        };
        self.rep_status = new_status;
    }

    pub fn to_sort_status_set(&mut self, status: ToSortDirType) {
        self.to_sort_type |= status;
    }

    pub fn to_sort_status_is(&self, status: ToSortDirType) -> bool {
        self.to_sort_type.contains(status)
    }

    pub fn to_sort_status_clear(&mut self, status: ToSortDirType) {
        self.to_sort_type.remove(status);
    }

    pub fn header_file_type(&self) -> HeaderFileType {
        self.header_file_type & HeaderFileType::HEADER_MASK
    }

    pub fn header_file_type_required(&self) -> bool {
        self.header_file_type.contains(HeaderFileType::REQUIRED)
    }

    pub fn set_header_file_type(&mut self, val: HeaderFileType) {
        self.header_file_type = val;
    }

    pub fn zip_dat_struct(&self) -> ZipStructure {
        ZipStructure::from(self.zip_dat_struct & 0x7f)
    }

    pub fn zip_dat_struct_fix(&self) -> bool {
        (self.zip_dat_struct & 0x80) == 0x80
    }

    pub fn set_zip_dat_struct(&mut self, zip_structure: ZipStructure, fix: bool) {
        self.zip_dat_struct = zip_structure as u8;
        if fix {
            self.zip_dat_struct |= 0x80;
        }
    }

    pub fn new_zip_struct(&self) -> ZipStructure {
        if self.dat_status == DatStatus::NotInDat || self.dat_status == DatStatus::InToSort {
            self.zip_struct
        } else {
            self.zip_dat_struct()
        }
    }

    pub fn file_status_set(&mut self, flag: FileStatus) {
        self.file_status |= flag;
    }

    pub fn file_status_clear(&mut self, flag: FileStatus) {
        self.file_status.remove(flag);
    }

    pub fn file_status_is(&self, flag: FileStatus) -> bool {
        self.file_status.contains(flag)
    }

    pub fn set_dat_got_status(&mut self, dat: DatStatus, got: GotStatus) {
        self.dat_status = dat;
        self.got_status = got;
    }

    pub fn mark_as_missing(&mut self) {
        let mut i = 0;
        while i < self.children.len() {
            let child_rc = Rc::clone(&self.children[i]);
            let should_remove = {
                let mut child = child_rc.borrow_mut();
                child.file_remove()
            };

            if should_remove {
                self.children.remove(i);
            } else {
                let mut child = self.children[i].borrow_mut();
                if child.is_directory() {
                    child.mark_as_missing();
                }
                i += 1;
            }
        }
    }

    pub fn file_remove(&mut self) -> bool {
        // Equivalent to EFile.Delete in C#
        if self.dat_status == DatStatus::NotInDat {
            true
        } else {
            self.got_status = GotStatus::NotGot;
            self.file_status_clear(
                FileStatus::SIZE_FROM_HEADER | FileStatus::CRC_FROM_HEADER | 
                FileStatus::SHA1_FROM_HEADER | FileStatus::MD5_FROM_HEADER |
                FileStatus::ALT_SIZE_FROM_HEADER | FileStatus::ALT_CRC_FROM_HEADER | 
                FileStatus::ALT_SHA1_FROM_HEADER | FileStatus::ALT_MD5_FROM_HEADER |
                FileStatus::HEADER_FILE_TYPE_FROM_HEADER
            );
            if self.file_status_is(FileStatus::HEADER_FILE_TYPE_FROM_DAT) && !self.file_status_is(FileStatus::HEADER_FILE_TYPE_FROM_HEADER) {
                // Keep the header file type if it came from DAT
            } else {
                self.header_file_type = HeaderFileType::NOTHING;
            }

            if self.is_directory() {
                // Clear children not in dat
                let mut i = 0;
                while i < self.children.len() {
                    let child_rc = Rc::clone(&self.children[i]);
                    let should_remove = child_rc.borrow_mut().file_remove();
                    if should_remove {
                        self.children.remove(i);
                    } else {
                        i += 1;
                    }
                }
            }

            false // EFile.Keep
        }
    }

    pub fn name_case(&self) -> &str {
        if self.file_name.trim().is_empty() {
            &self.name
        } else {
            &self.file_name
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::rc::Rc;
    use std::cell::RefCell;

    #[test]
    fn test_rvfile_hierarchy() {
        let mut root = RvFile::new(FileType::Dir);
        root.name = "Root".to_string();

        let child1 = Rc::new(RefCell::new(RvFile::new(FileType::File)));
        child1.borrow_mut().name = "File1.zip".to_string();
        
        let child2 = Rc::new(RefCell::new(RvFile::new(FileType::File)));
        child2.borrow_mut().name = "File2.zip".to_string();

        root.child_add(Rc::clone(&child1));
        root.child_add(Rc::clone(&child2));

        assert_eq!(root.children.len(), 2);
        
        // Test parent linking
        assert!(Rc::ptr_eq(&child1.borrow().parent.as_ref().unwrap().upgrade().unwrap(), &child1.borrow().parent.as_ref().unwrap().upgrade().unwrap())); // They point to same node
        
        // Remove child
        root.child_remove(0);
        assert_eq!(root.children.len(), 1);
        assert_eq!(root.children[0].borrow().name, "File2.zip");
    }

    #[test]
    fn test_file_status_flags() {
        let mut file = RvFile::new(FileType::File);
        assert!(!file.file_status_is(FileStatus::SIZE_FROM_HEADER));
        
        file.file_status_set(FileStatus::SIZE_FROM_HEADER);
        assert!(file.file_status_is(FileStatus::SIZE_FROM_HEADER));
        
        file.file_status_clear(FileStatus::SIZE_FROM_HEADER);
        assert!(!file.file_status_is(FileStatus::SIZE_FROM_HEADER));
    }

    #[test]
    fn test_mark_as_missing() {
        let mut root = RvFile::new(FileType::Dir);
        root.dat_status = DatStatus::InDatCollect;
        
        let child1 = Rc::new(RefCell::new(RvFile::new(FileType::File)));
        child1.borrow_mut().dat_status = DatStatus::NotInDat; // Should be removed
        
        let child2 = Rc::new(RefCell::new(RvFile::new(FileType::File)));
        child2.borrow_mut().dat_status = DatStatus::InDatCollect; // Should be kept and marked NotGot

        root.child_add(Rc::clone(&child1));
        root.child_add(Rc::clone(&child2));

        root.mark_as_missing();

        assert_eq!(root.children.len(), 1);
        assert_eq!(root.children[0].borrow().got_status, GotStatus::NotGot);
    }
}
