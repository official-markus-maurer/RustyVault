use std::rc::Rc;
use std::cell::RefCell;
use crate::rv_file::{FileStatus, RvFile};
use crate::scanned_file::ScannedFile;
use crate::compare::FileCompare;
use dat_reader::enums::FileType;

/// Synchronization engine between the physical filesystem and the internal database tree.
/// 
/// `FileScanning` compares the `ScannedFile` output produced by the `Scanner` against the
/// existing `RvFile` nodes in the `dir_root` tree. It updates the `GotStatus` of nodes
/// (e.g. `Got`, `NotGot`, `Corrupt`) based on whether the physical files still exist and match 
/// their expected cryptographic hashes.
/// 
/// Differences from C#:
/// - The C# `FileScanning` algorithm contains extensive `Phase2` deep-scan matching, CHD format 
///   validation, and highly complex status propagation rules.
/// - The Rust version implements a simplified 3-way merge algorithm (DB <-> FS). It correctly 
///   handles basic matching (`Phase 1`), marking files as `Got` or inserting `NotInDat` orphans, 
///   but skips some of the advanced header/CHD deep scan recoveries.
pub struct FileScanning;

impl FileScanning {
    const PHASE2_LOOKAHEAD: usize = 3;
    const PHYSICAL_STATUS_FLAGS: FileStatus = FileStatus::SIZE_FROM_HEADER
        .union(FileStatus::CRC_FROM_HEADER)
        .union(FileStatus::SHA1_FROM_HEADER)
        .union(FileStatus::MD5_FROM_HEADER)
        .union(FileStatus::ALT_SIZE_FROM_HEADER)
        .union(FileStatus::ALT_CRC_FROM_HEADER)
        .union(FileStatus::ALT_SHA1_FROM_HEADER)
        .union(FileStatus::ALT_MD5_FROM_HEADER)
        .union(FileStatus::HEADER_FILE_TYPE_FROM_HEADER);

    fn filesystem_index_case() -> i32 {
        if cfg!(windows) { 1 } else { 0 }
    }

    fn compare_names(left: &str, right: &str) -> std::cmp::Ordering {
        if Self::filesystem_index_case() == 0 {
            left.cmp(right)
        } else {
            left.to_ascii_lowercase().cmp(&right.to_ascii_lowercase())
        }
    }

    /// Recursively walks a physical directory tree alongside a DB directory tree,
    /// syncing the physical findings into the database.
    pub fn scan_dir(db_dir: Rc<RefCell<RvFile>>, file_dir: &mut ScannedFile) {
        Self::scan_dir_with_level(
            db_dir,
            file_dir,
            crate::settings::EScanLevel::Level2,
        );
    }

    pub fn scan_dir_with_level(
        db_dir: Rc<RefCell<RvFile>>,
        file_dir: &mut ScannedFile,
        scan_level: crate::settings::EScanLevel,
    ) {
        file_dir.sort();
        db_dir.borrow_mut().children.sort_by(|a, b| {
            Self::compare_names(&a.borrow().name, &b.borrow().name)
        });
        
        let mut db_index = 0;
        let mut file_index = 0;

        while db_index < db_dir.borrow().children.len() || file_index < file_dir.children.len() {
            let mut db_child = None;
            let res;

            let db_count = db_dir.borrow().children.len();
            let file_count = file_dir.children.len();

            if db_index < db_count && file_index < file_count {
                db_child = Some(Rc::clone(&db_dir.borrow().children[db_index]));
                
                let db_c = db_child.as_ref().unwrap();
                let file_c = &file_dir.children[file_index];
                res = crate::compare::compare_db_to_file(&db_c.borrow(), file_c);
            } else if file_index < file_count {
                res = 1;
            } else if db_index < db_count {
                db_child = Some(Rc::clone(&db_dir.borrow().children[db_index]));
                res = -1;
            } else {
                break;
            }

            if res != 0 && scan_level != crate::settings::EScanLevel::Level1 {
                if Self::try_realign_file_candidate_window(&db_dir, file_dir, db_index, file_index) {
                    continue;
                }
                if Self::try_realign_db_candidate_window(&db_dir, file_dir, db_index, file_index) {
                    continue;
                }

                if file_index < file_dir.children.len() {
                    if let Some(db_c) = &db_child {
                    let file_c = &mut file_dir.children[file_index];
                    let mut sc_clone = file_c.clone();
                    let (p2_match, p2_alt) =
                        FileCompare::phase_2_name_agnostic_test(&db_c.borrow(), &mut sc_clone);
                    if p2_match {
                        *file_c = sc_clone;
                        Self::match_found(Rc::clone(db_c), file_c, p2_alt);
                        db_index += 1;
                        file_index += 1;
                        continue;
                    }
                }
                }
            }

            match res {
                0 => {
                    let db_c = db_child.unwrap();
                    let file_c = &mut file_dir.children[file_index];

                    // simplified phase 1 comparison
                    let (mut matched, mut matched_alt) = FileCompare::phase_1_test(
                        &db_c.borrow(),
                        file_c,
                        scan_level,
                        Self::filesystem_index_case()
                    );

                    // If Phase 1 fails, fallback to Phase 2 (Deep scan fallback matching) 
                    if !matched && scan_level != crate::settings::EScanLevel::Level1 {
                        let mut sc_clone = file_c.clone();
                        let (p2_match, p2_alt) = FileCompare::phase_2_test(
                            &db_c.borrow(),
                            &mut sc_clone,
                            Self::filesystem_index_case(),
                        );
                        if p2_match {
                            matched = true;
                            matched_alt = p2_alt;
                            // Need to update original if matched
                            *file_c = sc_clone;
                        }
                    }

                    if matched {
                        Self::match_found(Rc::clone(&db_c), file_c, matched_alt);
                        if db_c.borrow().is_directory() {
                            Self::scan_dir_with_level(Rc::clone(&db_c), file_c, scan_level);
                        }
                        db_index += 1;
                    } else if Self::should_mark_corrupt(&db_c.borrow(), file_c) {
                        Self::corrupt_found(Rc::clone(&db_c), file_c);
                        db_index += 1;
                    } else {
                        Self::db_file_not_found(Rc::clone(&db_c), Rc::clone(&db_dir), &mut db_index);
                        Self::new_file_found(file_c, Rc::clone(&db_dir), db_index);
                        db_index += 1;
                    }

                    file_index += 1;
                },
                1 => {
                    let file_c = &file_dir.children[file_index];
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

    fn try_realign_file_candidate_window(
        db_dir: &Rc<RefCell<RvFile>>,
        file_dir: &mut ScannedFile,
        db_index: usize,
        file_index: usize,
    ) -> bool {
        if db_index >= db_dir.borrow().children.len() || file_index >= file_dir.children.len() {
            return false;
        }

        let db_child = Rc::clone(&db_dir.borrow().children[db_index]);
        let max_offset = std::cmp::min(
            Self::PHASE2_LOOKAHEAD,
            file_dir.children.len().saturating_sub(file_index + 1),
        );
        let mut current_candidate = file_dir.children[file_index].clone();
        let (current_matched, current_alt) =
            FileCompare::phase_2_name_agnostic_test(&db_child.borrow(), &mut current_candidate);
        let current_quality = if current_matched { Some(current_alt) } else { None };
        let mut best_match: Option<(usize, bool, ScannedFile)> = None;

        for offset in 1..=max_offset {
            let mut candidate = file_dir.children[file_index + offset].clone();
            let (matched, matched_alt) =
                FileCompare::phase_2_name_agnostic_test(&db_child.borrow(), &mut candidate);
            if matched {
                let is_better = match &best_match {
                    None => true,
                    Some((best_offset, best_alt, _)) => (!matched_alt && *best_alt)
                        || (matched_alt == *best_alt && offset < *best_offset),
                };
                if is_better {
                    best_match = Some((offset, matched_alt, candidate));
                }
            }
        }

        if let Some((offset, matched_alt, candidate)) = best_match {
            let should_realign = match current_quality {
                None => true,
                Some(current_alt) => current_alt && !matched_alt,
            };
            if !should_realign {
                return false;
            }
            file_dir.children[file_index + offset] = candidate;
            file_dir.children.swap(file_index, file_index + offset);
            return true;
        }

        false
    }

    fn try_realign_db_candidate_window(
        db_dir: &Rc<RefCell<RvFile>>,
        file_dir: &mut ScannedFile,
        db_index: usize,
        file_index: usize,
    ) -> bool {
        if db_index >= db_dir.borrow().children.len() || file_index >= file_dir.children.len() {
            return false;
        }

        let db_len = db_dir.borrow().children.len();
        let max_offset = std::cmp::min(Self::PHASE2_LOOKAHEAD, db_len.saturating_sub(db_index + 1));
        let candidate = file_dir.children[file_index].clone();
        let mut current_trial = candidate.clone();
        let current_match_quality = {
            let current_db_child = {
                let dir = db_dir.borrow();
                Rc::clone(&dir.children[db_index])
            };
            let (matched, matched_alt) =
                FileCompare::phase_2_name_agnostic_test(&current_db_child.borrow(), &mut current_trial);
            if matched { Some(matched_alt) } else { None }
        };
        let mut best_match: Option<(usize, bool, ScannedFile)> = None;

        for offset in 1..=max_offset {
            let next_db_child = {
                let dir = db_dir.borrow();
                Rc::clone(&dir.children[db_index + offset])
            };
            let mut trial = candidate.clone();
            let (matched, matched_alt) =
                FileCompare::phase_2_name_agnostic_test(&next_db_child.borrow(), &mut trial);
            if matched {
                let is_better = match &best_match {
                    None => true,
                    Some((best_offset, best_alt, _)) => (!matched_alt && *best_alt)
                        || (matched_alt == *best_alt && offset < *best_offset),
                };
                if is_better {
                    best_match = Some((offset, matched_alt, trial));
                }
            }
        }

        if let Some((offset, matched_alt, trial)) = best_match {
            let should_realign = match current_match_quality {
                None => true,
                Some(current_alt) => current_alt && !matched_alt,
            };
            if !should_realign {
                return false;
            }
            file_dir.children[file_index] = trial;
            db_dir.borrow_mut().children.swap(db_index, db_index + offset);
            return true;
        }

        false
    }

    fn match_found(db_child: Rc<RefCell<RvFile>>, file_child: &ScannedFile, _alt_match: bool) {
        let mut db_c = db_child.borrow_mut();
        
        // Invalidate stats cache since status is changing
        db_c.cached_stats = None;
        Self::apply_scanned_metadata(&mut db_c, file_child);
        
        match db_c.file_type {
            FileType::Zip | FileType::SevenZip => {
                let status = db_c.dat_status();
                db_c.set_dat_got_status(status, dat_reader::enums::GotStatus::Got);
            },
            FileType::Dir => {
                let status = db_c.dat_status();
                db_c.set_dat_got_status(status, dat_reader::enums::GotStatus::Got);
            },
            FileType::File => {
                let status = db_c.dat_status();
                db_c.set_dat_got_status(status, dat_reader::enums::GotStatus::Got);
            },
            _ => {}
        }
        db_c.rep_status_reset();
    }

    fn should_mark_corrupt(db_child: &RvFile, file_child: &ScannedFile) -> bool {
        db_child.dat_status() != dat_reader::enums::DatStatus::NotInDat
            && db_child.dat_status() != dat_reader::enums::DatStatus::InToSort
            && !db_child.is_directory()
            && !file_child.is_directory()
            && db_child.file_type == file_child.file_type
    }

    fn corrupt_found(db_child: Rc<RefCell<RvFile>>, file_child: &ScannedFile) {
        let mut db_c = db_child.borrow_mut();
        db_c.cached_stats = None;
        Self::apply_scanned_metadata(&mut db_c, file_child);
        let status = db_c.dat_status();
        db_c.set_dat_got_status(status, dat_reader::enums::GotStatus::Corrupt);
        db_c.rep_status_reset();
    }

    fn apply_scanned_metadata(db_child: &mut RvFile, scanned_file: &ScannedFile) {
        db_child.file_name = scanned_file.name.clone();
        db_child.file_mod_time_stamp = scanned_file.file_mod_time_stamp;
        db_child.local_header_offset = scanned_file.local_header_offset;
        db_child.header_file_type = scanned_file.header_file_type;
        db_child.zip_struct = scanned_file.zip_struct;
        db_child.file_status.remove(Self::PHYSICAL_STATUS_FLAGS);
        db_child.file_status.insert(scanned_file.status_flags);
        db_child.size = scanned_file.size;
        db_child.crc = scanned_file.crc.clone();
        db_child.sha1 = scanned_file.sha1.clone();
        db_child.md5 = scanned_file.md5.clone();
        db_child.alt_size = scanned_file.alt_size;
        db_child.alt_crc = scanned_file.alt_crc.clone();
        db_child.alt_sha1 = scanned_file.alt_sha1.clone();
        db_child.alt_md5 = scanned_file.alt_md5.clone();
        db_child.chd_version = scanned_file.chd_version;
    }

    fn new_file_found(file_child: &ScannedFile, db_dir: Rc<RefCell<RvFile>>, db_index: usize) {
        let parent_dat_status = db_dir.borrow().dat_status();
        let new_dat_status = if parent_dat_status == dat_reader::enums::DatStatus::InToSort {
            dat_reader::enums::DatStatus::InToSort
        } else {
            dat_reader::enums::DatStatus::NotInDat
        };

        let rc_child = Self::rv_file_from_scanned_file(file_child, new_dat_status, Rc::clone(&db_dir));
        
        let mut dir = db_dir.borrow_mut();
        dir.cached_stats = None; // Invalidate parent cache
        dir.child_insert(db_index, rc_child);
    }

    fn rv_file_from_scanned_file(
        scanned_file: &ScannedFile,
        dat_status: dat_reader::enums::DatStatus,
        parent: Rc<RefCell<RvFile>>,
    ) -> Rc<RefCell<RvFile>> {
        let mut new_child = RvFile::new(scanned_file.file_type);
        new_child.name = scanned_file.name.clone();
        new_child.file_name = scanned_file.name.clone();
        Self::apply_scanned_metadata(&mut new_child, scanned_file);
        new_child.set_dat_got_status(dat_status, dat_reader::enums::GotStatus::Got);
        new_child.parent = Some(Rc::downgrade(&parent));
        new_child.rep_status_reset();

        let rc_child = Rc::new(RefCell::new(new_child));

        if scanned_file.is_directory() {
            let child_dat_status = if dat_status == dat_reader::enums::DatStatus::InToSort {
                dat_reader::enums::DatStatus::InToSort
            } else {
                dat_reader::enums::DatStatus::NotInDat
            };

            for child in &scanned_file.children {
                let nested_child =
                    Self::rv_file_from_scanned_file(child, child_dat_status, Rc::clone(&rc_child));
                rc_child.borrow_mut().child_add(nested_child);
            }
        }

        rc_child
    }

    fn db_file_not_found(db_child: Rc<RefCell<RvFile>>, db_dir: Rc<RefCell<RvFile>>, db_index: &mut usize) {
        let should_remove = {
            let mut c = db_child.borrow_mut();
            c.cached_stats = None;
            
            // If it's a known Dat file/directory, we shouldn't fully remove it on missing scan
            // Just mark it as NotGot
            if c.dat_status() == dat_reader::enums::DatStatus::NotInDat || c.dat_status() == dat_reader::enums::DatStatus::InToSort {
                c.file_remove()
            } else {
                false
            }
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
                FileType::File => {
                    let status = c.dat_status();
                    c.set_dat_got_status(status, dat_reader::enums::GotStatus::NotGot);
                }
                _ => {}
            }
            c.rep_status_reset();
            *db_index += 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dat_reader::enums::GotStatus;

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

    #[test]
    fn test_new_directory_found_is_fully_integrated_recursively() {
        let db_dir = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
        db_dir.borrow_mut().name = "TestDir".to_string();

        let mut scanned_root = ScannedFile::new(FileType::Dir);
        scanned_root.name = "TestDir".to_string();

        let mut scanned_subdir = ScannedFile::new(FileType::Dir);
        scanned_subdir.name = "SubDir".to_string();

        let mut nested_file = ScannedFile::new(FileType::File);
        nested_file.name = "nested.rom".to_string();
        nested_file.size = Some(123);

        scanned_subdir.children.push(nested_file);
        scanned_root.children.push(scanned_subdir);

        FileScanning::scan_dir(Rc::clone(&db_dir), &mut scanned_root);

        let dir = db_dir.borrow();
        assert_eq!(dir.children.len(), 1);

        let subdir = dir.children[0].borrow();
        assert_eq!(subdir.name, "SubDir");
        assert_eq!(subdir.children.len(), 1);
        assert_eq!(subdir.children[0].borrow().name, "nested.rom");
        assert_eq!(subdir.children[0].borrow().size, Some(123));
        assert_eq!(subdir.children[0].borrow().got_status(), GotStatus::Got);
        assert_eq!(subdir.children[0].borrow().dat_status(), dat_reader::enums::DatStatus::NotInDat);
    }

    #[test]
    fn test_match_found_updates_scanned_metadata_on_existing_file() {
        let db_dir = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
        db_dir.borrow_mut().name = "Root".to_string();

        let mut existing_db_file = RvFile::new(FileType::File);
        existing_db_file.name = "match.bin".to_string();
        existing_db_file.size = Some(2048);
        existing_db_file.file_mod_time_stamp = 123456;
        existing_db_file.set_dat_got_status(dat_reader::enums::DatStatus::InDatCollect, GotStatus::NotGot);
        let existing_db_file = Rc::new(RefCell::new(existing_db_file));
        db_dir.borrow_mut().child_add(Rc::clone(&existing_db_file));

        let mut scanned_root = ScannedFile::new(FileType::Dir);
        scanned_root.name = "Root".to_string();

        let mut scanned = ScannedFile::new(FileType::File);
        scanned.name = "match.bin".to_string();
        scanned.file_mod_time_stamp = 123456;
        scanned.size = Some(2048);
        scanned.crc = Some(vec![1, 2, 3, 4]);
        scanned.sha1 = Some(vec![5; 20]);
        scanned.md5 = Some(vec![6; 16]);
        scanned.alt_size = Some(2000);
        scanned.alt_crc = Some(vec![7, 8, 9, 10]);
        scanned.alt_sha1 = Some(vec![11; 20]);
        scanned.alt_md5 = Some(vec![12; 16]);
        scanned.local_header_offset = Some(42);
        scanned.header_file_type = dat_reader::enums::HeaderFileType::ZIP;
        scanned_root.children.push(scanned);

        FileScanning::scan_dir(Rc::clone(&db_dir), &mut scanned_root);

        let matched = existing_db_file.borrow();
        assert_eq!(matched.got_status(), GotStatus::Got);
        assert_eq!(matched.file_mod_time_stamp, 123456);
        assert_eq!(matched.size, Some(2048));
        assert_eq!(matched.crc, Some(vec![1, 2, 3, 4]));
        assert_eq!(matched.sha1, Some(vec![5; 20]));
        assert_eq!(matched.md5, Some(vec![6; 16]));
        assert_eq!(matched.alt_size, Some(2000));
        assert_eq!(matched.alt_crc, Some(vec![7, 8, 9, 10]));
        assert_eq!(matched.alt_sha1, Some(vec![11; 20]));
        assert_eq!(matched.alt_md5, Some(vec![12; 16]));
        assert_eq!(matched.local_header_offset, Some(42));
        assert_eq!(matched.header_file_type, dat_reader::enums::HeaderFileType::ZIP);
    }

    #[test]
    fn test_match_found_updates_archive_zip_structure() {
        let db_dir = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
        db_dir.borrow_mut().name = "Root".to_string();

        let mut existing_archive = RvFile::new(FileType::Zip);
        existing_archive.name = "game.zip".to_string();
        existing_archive.zip_struct = dat_reader::enums::ZipStructure::None;
        existing_archive.set_dat_got_status(dat_reader::enums::DatStatus::InDatCollect, GotStatus::NotGot);
        let existing_archive = Rc::new(RefCell::new(existing_archive));
        db_dir.borrow_mut().child_add(Rc::clone(&existing_archive));

        let mut scanned_root = ScannedFile::new(FileType::Dir);
        scanned_root.name = "Root".to_string();

        let mut scanned_archive = ScannedFile::new(FileType::Zip);
        scanned_archive.name = "game.zip".to_string();
        scanned_archive.zip_struct = dat_reader::enums::ZipStructure::ZipTrrnt;
        scanned_root.children.push(scanned_archive);

        FileScanning::scan_dir(Rc::clone(&db_dir), &mut scanned_root);

        let matched = existing_archive.borrow();
        assert_eq!(matched.got_status(), GotStatus::Got);
        assert_eq!(matched.zip_struct, dat_reader::enums::ZipStructure::ZipTrrnt);
    }

    #[test]
    fn test_equal_name_hash_mismatch_marks_existing_file_corrupt_without_orphan() {
        let db_dir = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
        db_dir.borrow_mut().name = "Root".to_string();

        let mut existing_db_file = RvFile::new(FileType::File);
        existing_db_file.name = "bad.bin".to_string();
        existing_db_file.size = Some(10);
        existing_db_file.crc = Some(vec![0xAA, 0xBB, 0xCC, 0xDD]);
        existing_db_file.set_dat_got_status(dat_reader::enums::DatStatus::InDatCollect, GotStatus::NotGot);
        let existing_db_file = Rc::new(RefCell::new(existing_db_file));
        db_dir.borrow_mut().child_add(Rc::clone(&existing_db_file));

        let mut scanned_root = ScannedFile::new(FileType::Dir);
        scanned_root.name = "Root".to_string();

        let mut scanned = ScannedFile::new(FileType::File);
        scanned.name = "bad.bin".to_string();
        scanned.size = Some(10);
        scanned.crc = Some(vec![0x11, 0x22, 0x33, 0x44]);
        scanned.deep_scanned = true;
        scanned_root.children.push(scanned);

        FileScanning::scan_dir(Rc::clone(&db_dir), &mut scanned_root);

        let dir = db_dir.borrow();
        assert_eq!(dir.children.len(), 1);
        let matched = dir.children[0].borrow();
        assert_eq!(matched.name, "bad.bin");
        assert_eq!(matched.got_status(), GotStatus::Corrupt);
        assert_eq!(matched.dat_status(), dat_reader::enums::DatStatus::InDatCollect);
        assert_eq!(matched.crc, Some(vec![0x11, 0x22, 0x33, 0x44]));
    }

    #[test]
    fn test_renamed_file_is_recovered_by_phase2_without_creating_orphan() {
        let db_dir = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
        db_dir.borrow_mut().name = "Root".to_string();

        let mut existing_db_file = RvFile::new(FileType::File);
        existing_db_file.name = "expected.bin".to_string();
        existing_db_file.size = Some(4);
        existing_db_file.crc = Some(vec![0xAD, 0xF3, 0xF3, 0x63]);
        existing_db_file.set_dat_got_status(dat_reader::enums::DatStatus::InDatCollect, GotStatus::NotGot);
        let existing_db_file = Rc::new(RefCell::new(existing_db_file));
        db_dir.borrow_mut().child_add(Rc::clone(&existing_db_file));

        let mut scanned_root = ScannedFile::new(FileType::Dir);
        scanned_root.name = "Root".to_string();

        let mut scanned = ScannedFile::new(FileType::File);
        scanned.name = "renamed.bin".to_string();
        scanned.size = Some(4);
        scanned.crc = Some(vec![0xAD, 0xF3, 0xF3, 0x63]);
        scanned.deep_scanned = true;
        scanned_root.children.push(scanned);

        FileScanning::scan_dir(Rc::clone(&db_dir), &mut scanned_root);

        let dir = db_dir.borrow();
        assert_eq!(dir.children.len(), 1);
        let matched = dir.children[0].borrow();
        assert_eq!(matched.name, "expected.bin");
        assert_eq!(matched.got_status(), GotStatus::Got);
        assert_eq!(matched.dat_status(), dat_reader::enums::DatStatus::InDatCollect);
        assert_eq!(matched.crc, Some(vec![0xAD, 0xF3, 0xF3, 0x63]));
    }

    #[test]
    fn test_case_only_name_difference_matches_existing_file_on_windows_style_scan() {
        let db_dir = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
        db_dir.borrow_mut().name = "Root".to_string();

        let mut existing_db_file = RvFile::new(FileType::File);
        existing_db_file.name = "Game.zip".to_string();
        existing_db_file.size = Some(100);
        existing_db_file.file_mod_time_stamp = 123456;
        existing_db_file.set_dat_got_status(dat_reader::enums::DatStatus::InDatCollect, GotStatus::NotGot);
        let existing_db_file = Rc::new(RefCell::new(existing_db_file));
        db_dir.borrow_mut().child_add(Rc::clone(&existing_db_file));

        let mut scanned_root = ScannedFile::new(FileType::Dir);
        scanned_root.name = "Root".to_string();

        let mut scanned = ScannedFile::new(FileType::File);
        scanned.name = "game.zip".to_string();
        scanned.file_mod_time_stamp = 123456;
        scanned.size = Some(100);
        scanned_root.children.push(scanned);

        FileScanning::scan_dir(Rc::clone(&db_dir), &mut scanned_root);

        let dir = db_dir.borrow();
        assert_eq!(dir.children.len(), 1);
        let matched = dir.children[0].borrow();
        assert_eq!(matched.name, "Game.zip");
        assert_eq!(matched.got_status(), GotStatus::Got);
        assert_eq!(matched.dat_status(), dat_reader::enums::DatStatus::InDatCollect);
    }

    #[test]
    fn test_adjacent_file_candidate_is_realigned_before_orphaning() {
        let db_dir = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
        db_dir.borrow_mut().name = "Root".to_string();

        let mut expected = RvFile::new(FileType::File);
        expected.name = "target.bin".to_string();
        expected.size = Some(4);
        expected.crc = Some(vec![0xAD, 0xF3, 0xF3, 0x63]);
        expected.set_dat_got_status(dat_reader::enums::DatStatus::InDatCollect, GotStatus::NotGot);
        db_dir.borrow_mut().child_add(Rc::new(RefCell::new(expected)));

        let mut scanned_root = ScannedFile::new(FileType::Dir);
        scanned_root.name = "Root".to_string();

        let mut orphan = ScannedFile::new(FileType::File);
        orphan.name = "alpha.bin".to_string();
        orphan.size = Some(3);
        orphan.crc = Some(vec![1, 2, 3, 4]);
        orphan.deep_scanned = true;

        let mut renamed = ScannedFile::new(FileType::File);
        renamed.name = "zzz.bin".to_string();
        renamed.size = Some(4);
        renamed.crc = Some(vec![0xAD, 0xF3, 0xF3, 0x63]);
        renamed.deep_scanned = true;

        scanned_root.children.push(orphan);
        scanned_root.children.push(renamed);

        FileScanning::scan_dir(Rc::clone(&db_dir), &mut scanned_root);

        let dir = db_dir.borrow();
        assert_eq!(dir.children.len(), 2);
        let alpha = dir.children.iter().find(|child| child.borrow().name == "alpha.bin").unwrap();
        let target = dir.children.iter().find(|child| child.borrow().name == "target.bin").unwrap();
        assert_eq!(alpha.borrow().dat_status(), dat_reader::enums::DatStatus::NotInDat);
        assert_eq!(target.borrow().got_status(), GotStatus::Got);
    }

    #[test]
    fn test_adjacent_db_candidate_is_realigned_before_marking_missing() {
        let db_dir = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
        db_dir.borrow_mut().name = "Root".to_string();

        let mut missing = RvFile::new(FileType::File);
        missing.name = "alpha.bin".to_string();
        missing.size = Some(2);
        missing.crc = Some(vec![9, 9, 9, 9]);
        missing.set_dat_got_status(dat_reader::enums::DatStatus::InDatCollect, GotStatus::NotGot);
        db_dir.borrow_mut().child_add(Rc::new(RefCell::new(missing)));

        let mut expected = RvFile::new(FileType::File);
        expected.name = "target.bin".to_string();
        expected.size = Some(4);
        expected.crc = Some(vec![0xAD, 0xF3, 0xF3, 0x63]);
        expected.set_dat_got_status(dat_reader::enums::DatStatus::InDatCollect, GotStatus::NotGot);
        db_dir.borrow_mut().child_add(Rc::new(RefCell::new(expected)));

        let mut scanned_root = ScannedFile::new(FileType::Dir);
        scanned_root.name = "Root".to_string();

        let mut renamed = ScannedFile::new(FileType::File);
        renamed.name = "renamed.bin".to_string();
        renamed.size = Some(4);
        renamed.crc = Some(vec![0xAD, 0xF3, 0xF3, 0x63]);
        renamed.deep_scanned = true;
        scanned_root.children.push(renamed);

        FileScanning::scan_dir(Rc::clone(&db_dir), &mut scanned_root);

        let dir = db_dir.borrow();
        assert_eq!(dir.children.len(), 2);
        let alpha = dir.children.iter().find(|child| child.borrow().name == "alpha.bin").unwrap();
        let target = dir.children.iter().find(|child| child.borrow().name == "target.bin").unwrap();
        assert_eq!(alpha.borrow().got_status(), GotStatus::NotGot);
        assert_eq!(target.borrow().got_status(), GotStatus::Got);
    }

    #[test]
    fn test_two_step_file_candidate_window_is_realigned() {
        let db_dir = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
        db_dir.borrow_mut().name = "Root".to_string();

        let mut expected = RvFile::new(FileType::File);
        expected.name = "target.bin".to_string();
        expected.size = Some(4);
        expected.crc = Some(vec![0xAD, 0xF3, 0xF3, 0x63]);
        expected.set_dat_got_status(dat_reader::enums::DatStatus::InDatCollect, GotStatus::NotGot);
        db_dir.borrow_mut().child_add(Rc::new(RefCell::new(expected)));

        let mut scanned_root = ScannedFile::new(FileType::Dir);
        scanned_root.name = "Root".to_string();

        for name in ["alpha.bin", "beta.bin"] {
            let mut orphan = ScannedFile::new(FileType::File);
            orphan.name = name.to_string();
            orphan.size = Some(3);
            orphan.crc = Some(vec![1, 2, 3, 4]);
            orphan.deep_scanned = true;
            scanned_root.children.push(orphan);
        }

        let mut renamed = ScannedFile::new(FileType::File);
        renamed.name = "zzz.bin".to_string();
        renamed.size = Some(4);
        renamed.crc = Some(vec![0xAD, 0xF3, 0xF3, 0x63]);
        renamed.deep_scanned = true;
        scanned_root.children.push(renamed);

        FileScanning::scan_dir(Rc::clone(&db_dir), &mut scanned_root);

        let dir = db_dir.borrow();
        assert_eq!(dir.children.len(), 3);
        let target = dir.children.iter().find(|child| child.borrow().name == "target.bin").unwrap();
        assert_eq!(target.borrow().got_status(), GotStatus::Got);
    }

    #[test]
    fn test_two_step_db_candidate_window_is_realigned() {
        let db_dir = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
        db_dir.borrow_mut().name = "Root".to_string();

        for name in ["alpha.bin", "beta.bin"] {
            let mut missing = RvFile::new(FileType::File);
            missing.name = name.to_string();
            missing.size = Some(2);
            missing.crc = Some(vec![9, 9, 9, 9]);
            missing.set_dat_got_status(dat_reader::enums::DatStatus::InDatCollect, GotStatus::NotGot);
            db_dir.borrow_mut().child_add(Rc::new(RefCell::new(missing)));
        }

        let mut expected = RvFile::new(FileType::File);
        expected.name = "target.bin".to_string();
        expected.size = Some(4);
        expected.crc = Some(vec![0xAD, 0xF3, 0xF3, 0x63]);
        expected.set_dat_got_status(dat_reader::enums::DatStatus::InDatCollect, GotStatus::NotGot);
        db_dir.borrow_mut().child_add(Rc::new(RefCell::new(expected)));

        let mut scanned_root = ScannedFile::new(FileType::Dir);
        scanned_root.name = "Root".to_string();

        let mut renamed = ScannedFile::new(FileType::File);
        renamed.name = "renamed.bin".to_string();
        renamed.size = Some(4);
        renamed.crc = Some(vec![0xAD, 0xF3, 0xF3, 0x63]);
        renamed.deep_scanned = true;
        scanned_root.children.push(renamed);

        FileScanning::scan_dir(Rc::clone(&db_dir), &mut scanned_root);

        let dir = db_dir.borrow();
        assert_eq!(dir.children.len(), 3);
        let target = dir.children.iter().find(|child| child.borrow().name == "target.bin").unwrap();
        assert_eq!(target.borrow().got_status(), GotStatus::Got);
    }

    #[test]
    fn test_three_step_file_candidate_window_is_realigned() {
        let db_dir = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
        db_dir.borrow_mut().name = "Root".to_string();

        let mut expected = RvFile::new(FileType::File);
        expected.name = "target.bin".to_string();
        expected.size = Some(4);
        expected.crc = Some(vec![0xAD, 0xF3, 0xF3, 0x63]);
        expected.set_dat_got_status(dat_reader::enums::DatStatus::InDatCollect, GotStatus::NotGot);
        db_dir.borrow_mut().child_add(Rc::new(RefCell::new(expected)));

        let mut scanned_root = ScannedFile::new(FileType::Dir);
        scanned_root.name = "Root".to_string();

        for name in ["alpha.bin", "beta.bin", "gamma.bin"] {
            let mut orphan = ScannedFile::new(FileType::File);
            orphan.name = name.to_string();
            orphan.size = Some(3);
            orphan.crc = Some(vec![1, 2, 3, 4]);
            orphan.deep_scanned = true;
            scanned_root.children.push(orphan);
        }

        let mut renamed = ScannedFile::new(FileType::File);
        renamed.name = "zzz.bin".to_string();
        renamed.size = Some(4);
        renamed.crc = Some(vec![0xAD, 0xF3, 0xF3, 0x63]);
        renamed.deep_scanned = true;
        scanned_root.children.push(renamed);

        FileScanning::scan_dir(Rc::clone(&db_dir), &mut scanned_root);

        let dir = db_dir.borrow();
        assert_eq!(dir.children.len(), 4);
        let target = dir.children.iter().find(|child| child.borrow().name == "target.bin").unwrap();
        assert_eq!(target.borrow().got_status(), GotStatus::Got);
    }

    #[test]
    fn test_three_step_db_candidate_window_is_realigned() {
        let db_dir = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
        db_dir.borrow_mut().name = "Root".to_string();

        for name in ["alpha.bin", "beta.bin", "gamma.bin"] {
            let mut missing = RvFile::new(FileType::File);
            missing.name = name.to_string();
            missing.size = Some(2);
            missing.crc = Some(vec![9, 9, 9, 9]);
            missing.set_dat_got_status(dat_reader::enums::DatStatus::InDatCollect, GotStatus::NotGot);
            db_dir.borrow_mut().child_add(Rc::new(RefCell::new(missing)));
        }

        let mut expected = RvFile::new(FileType::File);
        expected.name = "target.bin".to_string();
        expected.size = Some(4);
        expected.crc = Some(vec![0xAD, 0xF3, 0xF3, 0x63]);
        expected.set_dat_got_status(dat_reader::enums::DatStatus::InDatCollect, GotStatus::NotGot);
        db_dir.borrow_mut().child_add(Rc::new(RefCell::new(expected)));

        let mut scanned_root = ScannedFile::new(FileType::Dir);
        scanned_root.name = "Root".to_string();

        let mut renamed = ScannedFile::new(FileType::File);
        renamed.name = "renamed.bin".to_string();
        renamed.size = Some(4);
        renamed.crc = Some(vec![0xAD, 0xF3, 0xF3, 0x63]);
        renamed.deep_scanned = true;
        scanned_root.children.push(renamed);

        FileScanning::scan_dir(Rc::clone(&db_dir), &mut scanned_root);

        let dir = db_dir.borrow();
        assert_eq!(dir.children.len(), 4);
        let target = dir.children.iter().find(|child| child.borrow().name == "target.bin").unwrap();
        assert_eq!(target.borrow().got_status(), GotStatus::Got);
    }

    #[test]
    fn test_file_candidate_window_prefers_primary_match_over_nearer_alt_match() {
        let db_dir = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
        db_dir.borrow_mut().name = "Root".to_string();

        let mut expected = RvFile::new(FileType::File);
        expected.name = "target.bin".to_string();
        expected.size = Some(4);
        expected.crc = Some(vec![0xAA, 0xBB, 0xCC, 0xDD]);
        expected.alt_size = Some(4);
        expected.alt_crc = Some(vec![0x11, 0x22, 0x33, 0x44]);
        expected.set_dat_got_status(dat_reader::enums::DatStatus::InDatCollect, GotStatus::NotGot);
        db_dir.borrow_mut().child_add(Rc::new(RefCell::new(expected)));

        let mut scanned_root = ScannedFile::new(FileType::Dir);
        scanned_root.name = "Root".to_string();

        let mut alt_match = ScannedFile::new(FileType::File);
        alt_match.name = "alt.bin".to_string();
        alt_match.size = Some(4);
        alt_match.crc = Some(vec![0x11, 0x22, 0x33, 0x44]);
        alt_match.deep_scanned = true;

        let mut primary_match = ScannedFile::new(FileType::File);
        primary_match.name = "primary.bin".to_string();
        primary_match.size = Some(4);
        primary_match.crc = Some(vec![0xAA, 0xBB, 0xCC, 0xDD]);
        primary_match.deep_scanned = true;

        scanned_root.children.push(alt_match);
        scanned_root.children.push(primary_match);

        FileScanning::scan_dir(Rc::clone(&db_dir), &mut scanned_root);

        let dir = db_dir.borrow();
        assert_eq!(dir.children.len(), 2);
        let target = dir.children.iter().find(|child| child.borrow().name == "target.bin").unwrap();
        assert_eq!(target.borrow().got_status(), GotStatus::Got);
        assert_eq!(target.borrow().crc, Some(vec![0xAA, 0xBB, 0xCC, 0xDD]));
    }

    #[test]
    fn test_db_candidate_window_prefers_primary_match_over_nearer_alt_match() {
        let db_dir = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
        db_dir.borrow_mut().name = "Root".to_string();

        let mut alt_expected = RvFile::new(FileType::File);
        alt_expected.name = "alt_target.bin".to_string();
        alt_expected.size = Some(5);
        alt_expected.alt_size = Some(4);
        alt_expected.alt_crc = Some(vec![0x11, 0x22, 0x33, 0x44]);
        alt_expected.set_dat_got_status(dat_reader::enums::DatStatus::InDatCollect, GotStatus::NotGot);
        db_dir.borrow_mut().child_add(Rc::new(RefCell::new(alt_expected)));

        let mut primary_expected = RvFile::new(FileType::File);
        primary_expected.name = "primary_target.bin".to_string();
        primary_expected.size = Some(4);
        primary_expected.crc = Some(vec![0xAA, 0xBB, 0xCC, 0xDD]);
        primary_expected.set_dat_got_status(dat_reader::enums::DatStatus::InDatCollect, GotStatus::NotGot);
        db_dir.borrow_mut().child_add(Rc::new(RefCell::new(primary_expected)));

        let mut scanned_root = ScannedFile::new(FileType::Dir);
        scanned_root.name = "Root".to_string();

        let mut scanned = ScannedFile::new(FileType::File);
        scanned.name = "renamed.bin".to_string();
        scanned.size = Some(4);
        scanned.crc = Some(vec![0xAA, 0xBB, 0xCC, 0xDD]);
        scanned.alt_size = Some(4);
        scanned.alt_crc = Some(vec![0x11, 0x22, 0x33, 0x44]);
        scanned.deep_scanned = true;
        scanned_root.children.push(scanned);

        FileScanning::scan_dir(Rc::clone(&db_dir), &mut scanned_root);

        let dir = db_dir.borrow();
        let primary = dir
            .children
            .iter()
            .find(|child| child.borrow().name == "primary_target.bin")
            .unwrap();
        assert_eq!(primary.borrow().got_status(), GotStatus::Got);
        assert_eq!(primary.borrow().crc, Some(vec![0xAA, 0xBB, 0xCC, 0xDD]));
    }

    #[test]
    fn test_apply_scanned_metadata_preserves_dat_flags_and_updates_physical_flags() {
        let db_dir = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
        db_dir.borrow_mut().name = "Root".to_string();

        let mut existing_db_file = RvFile::new(FileType::File);
        existing_db_file.name = "headered.nes".to_string();
        existing_db_file.file_status_set(FileStatus::CRC_FROM_DAT);
        existing_db_file.set_dat_got_status(dat_reader::enums::DatStatus::InDatCollect, GotStatus::NotGot);
        let existing_db_file = Rc::new(RefCell::new(existing_db_file));
        db_dir.borrow_mut().child_add(Rc::clone(&existing_db_file));

        let mut scanned_root = ScannedFile::new(FileType::Dir);
        scanned_root.name = "Root".to_string();

        let mut scanned = ScannedFile::new(FileType::File);
        scanned.name = "headered.nes".to_string();
        scanned.header_file_type = dat_reader::enums::HeaderFileType::NES;
        scanned.status_flags = FileStatus::HEADER_FILE_TYPE_FROM_HEADER | FileStatus::ALT_CRC_FROM_HEADER;
        scanned.crc = Some(vec![1, 2, 3, 4]);
        scanned.deep_scanned = true;
        scanned_root.children.push(scanned);

        FileScanning::scan_dir(Rc::clone(&db_dir), &mut scanned_root);

        let matched = existing_db_file.borrow();
        assert!(matched.file_status_is(FileStatus::CRC_FROM_DAT));
        assert!(matched.file_status_is(FileStatus::HEADER_FILE_TYPE_FROM_HEADER));
        assert!(matched.file_status_is(FileStatus::ALT_CRC_FROM_HEADER));
    }

    #[test]
    fn test_match_found_refreshes_file_name_to_current_scanned_name() {
        let db_dir = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
        db_dir.borrow_mut().name = "Root".to_string();

        let mut existing_db_file = RvFile::new(FileType::File);
        existing_db_file.name = "Game.zip".to_string();
        existing_db_file.file_name = "old_name.zip".to_string();
        existing_db_file.size = Some(100);
        existing_db_file.crc = Some(vec![0xAA, 0xBB, 0xCC, 0xDD]);
        existing_db_file.set_dat_got_status(dat_reader::enums::DatStatus::InDatCollect, GotStatus::NotGot);
        let existing_db_file = Rc::new(RefCell::new(existing_db_file));
        db_dir.borrow_mut().child_add(Rc::clone(&existing_db_file));

        let mut scanned_root = ScannedFile::new(FileType::Dir);
        scanned_root.name = "Root".to_string();

        let mut scanned = ScannedFile::new(FileType::File);
        scanned.name = "game.zip".to_string();
        scanned.size = Some(100);
        scanned.crc = Some(vec![0xAA, 0xBB, 0xCC, 0xDD]);
        scanned.deep_scanned = true;
        scanned_root.children.push(scanned);

        FileScanning::scan_dir(Rc::clone(&db_dir), &mut scanned_root);

        let matched = existing_db_file.borrow();
        assert_eq!(matched.file_name, "game.zip");
        assert_eq!(matched.got_status(), GotStatus::Got);
    }

    #[test]
    fn test_scan_dir_handles_windows_style_case_insensitive_sort_order() {
        let db_dir = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
        db_dir.borrow_mut().name = "Root".to_string();

        let mut upper = RvFile::new(FileType::File);
        upper.name = "B.bin".to_string();
        upper.size = Some(4);
        upper.crc = Some(vec![0, 0, 0, 2]);
        upper.set_dat_got_status(dat_reader::enums::DatStatus::InDatCollect, GotStatus::NotGot);
        db_dir.borrow_mut().child_add(Rc::new(RefCell::new(upper)));

        let mut lower = RvFile::new(FileType::File);
        lower.name = "a.bin".to_string();
        lower.size = Some(4);
        lower.crc = Some(vec![0, 0, 0, 1]);
        lower.set_dat_got_status(dat_reader::enums::DatStatus::InDatCollect, GotStatus::NotGot);
        db_dir.borrow_mut().child_add(Rc::new(RefCell::new(lower)));

        let mut scanned_root = ScannedFile::new(FileType::Dir);
        scanned_root.name = "Root".to_string();

        let mut scanned_a = ScannedFile::new(FileType::File);
        scanned_a.name = "a.bin".to_string();
        scanned_a.size = Some(4);
        scanned_a.crc = Some(vec![0, 0, 0, 1]);
        scanned_root.children.push(scanned_a);

        let mut scanned_b = ScannedFile::new(FileType::File);
        scanned_b.name = "B.bin".to_string();
        scanned_b.size = Some(4);
        scanned_b.crc = Some(vec![0, 0, 0, 2]);
        scanned_root.children.push(scanned_b);

        FileScanning::scan_dir(Rc::clone(&db_dir), &mut scanned_root);

        let dir = db_dir.borrow();
        for child in &dir.children {
            assert_eq!(child.borrow().got_status(), GotStatus::Got);
        }
    }
}
