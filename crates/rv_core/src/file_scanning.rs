use std::rc::Rc;
use std::cell::RefCell;
use crate::rv_file::RvFile;
use crate::scanned_file::ScannedFile;
use crate::compare::FileCompare;
use dat_reader::enums::{FileType, GotStatus};

pub struct FileScanning;

impl FileScanning {
    pub fn scan_dir(db_dir: Rc<RefCell<RvFile>>, file_dir: &mut ScannedFile) {
        file_dir.sort();
        
        let mut db_index = 0;
        let mut file_index = 0;

        while db_index < db_dir.borrow().children.len() || file_index < file_dir.children.len() {
            let mut db_child = None;
            let mut file_child = None;
            let res;

            let db_count = db_dir.borrow().children.len();
            let file_count = file_dir.children.len();

            if db_index < db_count && file_index < file_count {
                db_child = Some(Rc::clone(&db_dir.borrow().children[db_index]));
                file_child = Some(&mut file_dir.children[file_index]);
                
                let db_c = db_child.as_ref().unwrap();
                let file_c = file_child.as_ref().unwrap();
                res = crate::compare::compare_db_to_file(&db_c.borrow(), file_c);
            } else if file_index < file_count {
                file_child = Some(&mut file_dir.children[file_index]);
                res = 1;
            } else if db_index < db_count {
                db_child = Some(Rc::clone(&db_dir.borrow().children[db_index]));
                res = -1;
            } else {
                break;
            }

            match res {
                0 => {
                    let db_c = db_child.unwrap();
                    let file_c = file_child.unwrap();

                    // simplified phase 1 comparison
                    let (matched, matched_alt) = FileCompare::phase_1_test(
                        &db_c.borrow(),
                        file_c,
                        crate::settings::EScanLevel::Level1,
                        0
                    );

                    if matched {
                        Self::match_found(Rc::clone(&db_c), file_c, matched_alt);
                        if db_c.borrow().is_directory() {
                            Self::scan_dir(Rc::clone(&db_c), file_c);
                        }
                    } else {
                        // Normally we would branch out into Phase2Test (deep scan matching)
                        // For simplicity in this core port, we just treat it as not found and new file
                        Self::db_file_not_found(Rc::clone(&db_c), Rc::clone(&db_dir), &mut db_index);
                        Self::new_file_found(file_c, Rc::clone(&db_dir), db_index);
                        db_index += 1;
                    }

                    db_index += 1;
                    file_index += 1;
                },
                1 => {
                    let file_c = file_child.unwrap();
                    Self::new_file_found(file_c, Rc::clone(&db_dir), db_index);
                    db_index += 1;
                    file_index += 1;
                },
                -1 => {
                    let db_c = db_child.unwrap();
                    Self::db_file_not_found(Rc::clone(&db_c), Rc::clone(&db_dir), &mut db_index);
                },
                _ => {}
            }
        }
    }

    fn match_found(db_child: Rc<RefCell<RvFile>>, file_child: &ScannedFile, _alt_match: bool) {
        let mut db_c = db_child.borrow_mut();
        
        // Invalidate stats cache since status is changing
        db_c.cached_stats = None;
        
        match db_c.file_type {
            FileType::Zip | FileType::SevenZip => {
                let status = db_c.dat_status();
                db_c.set_dat_got_status(status, GotStatus::Got);
            },
            FileType::Dir => {
                let status = db_c.dat_status();
                db_c.set_dat_got_status(status, GotStatus::Got);
            },
            FileType::File => {
                let status = db_c.dat_status();
                db_c.set_dat_got_status(status, GotStatus::Got);
                db_c.size = file_child.size;
                db_c.crc = file_child.crc.clone();
                db_c.sha1 = file_child.sha1.clone();
                db_c.md5 = file_child.md5.clone();
            },
            _ => {}
        }
    }

    fn new_file_found(file_child: &ScannedFile, db_dir: Rc<RefCell<RvFile>>, db_index: usize) {
        let mut new_child = RvFile::new(file_child.file_type);
        new_child.name = file_child.name.clone();
        new_child.size = file_child.size;
        new_child.crc = file_child.crc.clone();
        new_child.sha1 = file_child.sha1.clone();
        new_child.md5 = file_child.md5.clone();
        new_child.set_dat_got_status(dat_reader::enums::DatStatus::NotInDat, GotStatus::Got);
        
        let rc_child = Rc::new(RefCell::new(new_child));
        
        let mut dir = db_dir.borrow_mut();
        dir.cached_stats = None; // Invalidate parent cache
        dir.child_insert(db_index, rc_child);
    }

    fn db_file_not_found(db_child: Rc<RefCell<RvFile>>, db_dir: Rc<RefCell<RvFile>>, db_index: &mut usize) {
        let should_remove = {
            let mut c = db_child.borrow_mut();
            c.cached_stats = None;
            c.file_remove()
        };

        let mut dir = db_dir.borrow_mut();
        dir.cached_stats = None; // Invalidate parent cache
        
        if should_remove {
            dir.child_remove(*db_index);
        } else {
            let mut c = db_child.borrow_mut();
            match c.file_type {
                FileType::Zip | FileType::SevenZip | FileType::Dir => {
                    c.mark_as_missing();
                }
                _ => {}
            }
            *db_index += 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_file_scanning_integration() {
        let db_dir = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
        db_dir.borrow_mut().name = "TestDir".to_string();

        let mut existing_db_file = RvFile::new(FileType::File);
        existing_db_file.name = "exist.zip".to_string();
        existing_db_file.size = Some(100);
        existing_db_file.set_dat_got_status(dat_reader::enums::DatStatus::InDatCollect, GotStatus::NotGot);
        db_dir.borrow_mut().child_add(Rc::new(RefCell::new(existing_db_file)));

        let mut scanned_root = ScannedFile::new(FileType::Dir);
        scanned_root.name = "TestDir".to_string();

        let mut scan1 = ScannedFile::new(FileType::File);
        scan1.name = "exist.zip".to_string();
        scan1.size = Some(100);
        
        let mut scan2 = ScannedFile::new(FileType::File);
        scan2.name = "new_file.zip".to_string();
        scan2.size = Some(200);

        scanned_root.children.push(scan1);
        scanned_root.children.push(scan2);

        FileScanning::scan_dir(Rc::clone(&db_dir), &mut scanned_root);

        let dir = db_dir.borrow();
        assert_eq!(dir.children.len(), 2);
        
        // "exist.zip" should be matched and marked Got
        let c1 = dir.children[0].borrow();
        assert_eq!(c1.name, "exist.zip");
        assert_eq!(c1.got_status(), GotStatus::Got);
        assert_eq!(c1.dat_status(), dat_reader::enums::DatStatus::InDatCollect);

        // "new_file.zip" should be integrated as NotInDat but Got
        let c2 = dir.children[1].borrow();
        assert_eq!(c2.name, "new_file.zip");
        assert_eq!(c2.got_status(), GotStatus::Got);
        assert_eq!(c2.dat_status(), dat_reader::enums::DatStatus::NotInDat);
    }
}