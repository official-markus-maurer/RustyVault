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
    const PHYSICAL_STATUS_FLAGS: FileStatus = FileStatus::SIZE_FROM_HEADER
        .union(FileStatus::CRC_FROM_HEADER)
        .union(FileStatus::SHA1_FROM_HEADER)
        .union(FileStatus::MD5_FROM_HEADER)
        .union(FileStatus::ALT_SIZE_FROM_HEADER)
        .union(FileStatus::ALT_CRC_FROM_HEADER)
        .union(FileStatus::ALT_SHA1_FROM_HEADER)
        .union(FileStatus::ALT_MD5_FROM_HEADER)
        .union(FileStatus::HEADER_FILE_TYPE_FROM_HEADER);

    fn compare_names_for_group(left: &str, right: &str) -> std::cmp::Ordering {
        if cfg!(windows) {
            left.to_ascii_lowercase().cmp(&right.to_ascii_lowercase())
        } else {
            left.cmp(right)
        }
    }

    fn ascii_lower(byte: u8) -> u8 {
        if byte.is_ascii_uppercase() { byte + 0x20 } else { byte }
    }

    fn compare_dir_names_case(left: &str, right: &str) -> std::cmp::Ordering {
        let res = left.to_ascii_lowercase().cmp(&right.to_ascii_lowercase());
        if res != std::cmp::Ordering::Equal {
            return res;
        }
        left.cmp(right)
    }

    fn compare_trrnt_zip_names_case(left: &str, right: &str) -> std::cmp::Ordering {
        let bytes_a = left.as_bytes();
        let bytes_b = right.as_bytes();
        let len = std::cmp::min(bytes_a.len(), bytes_b.len());
        for i in 0..len {
            let ca = Self::ascii_lower(bytes_a[i]);
            let cb = Self::ascii_lower(bytes_b[i]);
            if ca < cb {
                return std::cmp::Ordering::Less;
            }
            if ca > cb {
                return std::cmp::Ordering::Greater;
            }
        }
        let res = bytes_a.len().cmp(&bytes_b.len());
        if res != std::cmp::Ordering::Equal {
            return res;
        }
        left.cmp(right)
    }

    fn split_7zip_filename(filename: &str) -> (&str, &str, &str) {
        let dir_index = filename.rfind('/');
        let (path, name) = if let Some(i) = dir_index {
            (&filename[..i], &filename[i + 1..])
        } else {
            ("", filename)
        };

        let ext_index = name.rfind('.');
        if let Some(i) = ext_index {
            (path, &name[..i], &name[i + 1..])
        } else {
            (path, name, "")
        }
    }

    fn compare_trrnt_7zip_names(left: &str, right: &str) -> std::cmp::Ordering {
        let (path_a, name_a, ext_a) = Self::split_7zip_filename(left);
        let (path_b, name_b, ext_b) = Self::split_7zip_filename(right);

        let res = ext_a.cmp(ext_b);
        if res != std::cmp::Ordering::Equal {
            return res;
        }
        let res = name_a.cmp(name_b);
        if res != std::cmp::Ordering::Equal {
            return res;
        }
        path_a.cmp(path_b)
    }

    fn compare_db_child_names(container_type: FileType, left: &str, right: &str) -> std::cmp::Ordering {
        match container_type {
            FileType::Zip => Self::compare_trrnt_zip_names_case(left, right),
            FileType::SevenZip => Self::compare_trrnt_7zip_names(left, right),
            FileType::Dir => Self::compare_dir_names_case(left, right),
            _ => Self::compare_dir_names_case(left, right),
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
        let container_type = db_dir.borrow().file_type;
        {
            let mut dir_mut = db_dir.borrow_mut();
            for child in dir_mut.children.iter_mut() {
                child.borrow_mut().search_found = false;
            }
            dir_mut.children.sort_by(|a, b| {
                let a_b = a.borrow();
                let b_b = b.borrow();
                Self::compare_db_child_names(container_type, &a_b.name, &b_b.name)
            });
        }
        for child in file_dir.children.iter_mut() {
            child.search_found = false;
        }
        
        let mut db_index = 0;
        let mut file_index = 0;

        while db_index < db_dir.borrow().children.len() || file_index < file_dir.children.len() {
            let (db_count, file_count) = {
                let dir = db_dir.borrow();
                (dir.children.len(), file_dir.children.len())
            };

            let mut db_child: Option<Rc<RefCell<RvFile>>> = None;
            let res: i32;

            if db_index < db_count && file_index < file_count {
                let db_c = Rc::clone(&db_dir.borrow().children[db_index]);
                let file_c = &file_dir.children[file_index];
                res = match Self::compare_names_for_group(&db_c.borrow().name, &file_c.name) {
                    std::cmp::Ordering::Less => -1,
                    std::cmp::Ordering::Equal => 0,
                    std::cmp::Ordering::Greater => 1,
                };
                db_child = Some(db_c);
            } else if file_index < file_count {
                res = 1;
            } else if db_index < db_count {
                res = -1;
                db_child = Some(Rc::clone(&db_dir.borrow().children[db_index]));
            } else {
                break;
            }

            match res {
                0 => {
                    let db_first = db_child.unwrap();

                    let mut dbs: Vec<Rc<RefCell<RvFile>>> = Vec::new();
                    let mut dbs_count = 1usize;
                    dbs.push(Rc::clone(&db_first));

                    while db_index + dbs_count < db_dir.borrow().children.len()
                        && Self::compare_names_for_group(
                            &db_first.borrow().name,
                            &db_dir.borrow().children[db_index + dbs_count].borrow().name,
                        ) == std::cmp::Ordering::Equal
                    {
                        dbs.push(Rc::clone(&db_dir.borrow().children[db_index + dbs_count]));
                        dbs_count += 1;
                    }

                    let file_first_index = file_index;
                    let mut files_count = 1usize;
                    while file_first_index + files_count < file_dir.children.len()
                        && Self::compare_names_for_group(
                            &file_dir.children[file_first_index].name,
                            &file_dir.children[file_first_index + files_count].name,
                        ) == std::cmp::Ordering::Equal
                    {
                        files_count += 1;
                    }

                    for db in dbs.iter() {
                        db.borrow_mut().search_found = false;
                    }
                    for i in 0..files_count {
                        file_dir.children[file_first_index + i].search_found = false;
                    }

                    let case_test = files_count > 1;
                    let mut recurse_pairs: Vec<(Rc<RefCell<RvFile>>, usize, bool)> = Vec::new();

                    for index_case in if case_test { 0 } else { 1 }..2 {
                        for fi in 0..files_count {
                            let file_pos = file_first_index + fi;
                            if file_dir.children[file_pos].search_found {
                                continue;
                            }

                            for db_rc in dbs.iter().take(dbs_count) {
                                if db_rc.borrow().search_found {
                                    continue;
                                }

                                let (matched, matched_alt) = {
                                    let db_b = db_rc.borrow();
                                    FileCompare::phase_1_test(&db_b, &file_dir.children[file_pos], scan_level, index_case)
                                };

                                if !matched {
                                    continue;
                                }

                                Self::match_found(Rc::clone(db_rc), &file_dir.children[file_pos], matched_alt);
                                db_rc.borrow_mut().search_found = true;
                                file_dir.children[file_pos].search_found = true;

                                let needs_recurse = {
                                    let db_b = db_rc.borrow();
                                    match db_b.file_type {
                                        FileType::Dir => true,
                                        FileType::Zip | FileType::SevenZip => {
                                            Self::should_scan_archive_contents(&db_b, &file_dir.children[file_pos], scan_level)
                                        }
                                        _ => false,
                                    }
                                };
                                if needs_recurse {
                                    recurse_pairs.push((Rc::clone(db_rc), file_pos, matched_alt));
                                }
                                break;
                            }

                            if file_dir.children[file_pos].search_found {
                                continue;
                            }

                            for db_rc in dbs.iter().take(dbs_count) {
                                if db_rc.borrow().search_found {
                                    continue;
                                }

                                let (matched, matched_alt) = {
                                    let file_ref = &mut file_dir.children[file_pos];
                                    let db_b = db_rc.borrow();
                                    FileCompare::phase_2_test(&db_b, file_ref, index_case)
                                };

                                if !matched {
                                    continue;
                                }

                                Self::match_found(Rc::clone(db_rc), &file_dir.children[file_pos], matched_alt);
                                db_rc.borrow_mut().search_found = true;
                                file_dir.children[file_pos].search_found = true;

                                let needs_recurse = {
                                    let db_b = db_rc.borrow();
                                    match db_b.file_type {
                                        FileType::Dir => true,
                                        FileType::Zip | FileType::SevenZip => {
                                            Self::should_scan_archive_contents(&db_b, &file_dir.children[file_pos], scan_level)
                                        }
                                        _ => false,
                                    }
                                };
                                if needs_recurse {
                                    recurse_pairs.push((Rc::clone(db_rc), file_pos, matched_alt));
                                }
                                break;
                            }
                        }
                    }

                    for db_rc in dbs.iter().take(dbs_count) {
                        if db_rc.borrow().search_found {
                            continue;
                        }
                        for fi in 0..files_count {
                            let file_pos = file_first_index + fi;
                            if file_dir.children[file_pos].search_found {
                                continue;
                            }
                            let should_corrupt = {
                                let db_b = db_rc.borrow();
                                Self::should_mark_corrupt(&db_b, &file_dir.children[file_pos])
                            };
                            if should_corrupt {
                                Self::corrupt_found(Rc::clone(db_rc), &file_dir.children[file_pos]);
                                db_rc.borrow_mut().search_found = true;
                                file_dir.children[file_pos].search_found = true;
                                break;
                            }
                        }
                    }

                    for (db_c, file_pos, alt_match) in recurse_pairs {
                        let file_child = &mut file_dir.children[file_pos];
                        let db_type = db_c.borrow().file_type;
                        match db_type {
                            FileType::Dir => {
                                Self::scan_dir_with_level(Rc::clone(&db_c), file_child, scan_level);
                            }
                            FileType::Zip | FileType::SevenZip => {
                                if Self::should_scan_archive_contents(&db_c.borrow(), file_child, scan_level) {
                                    if db_c.borrow().file_mod_time_stamp != file_child.file_mod_time_stamp {
                                        db_c.borrow_mut().mark_as_missing();
                                        Self::match_found(Rc::clone(&db_c), file_child, alt_match);
                                    }
                                    Self::scan_dir_with_level(Rc::clone(&db_c), file_child, scan_level);
                                }
                            }
                            _ => {}
                        }
                    }

                    for db_rc in dbs.iter().take(dbs_count) {
                        if db_rc.borrow().search_found {
                            db_index += 1;
                            continue;
                        }
                        Self::db_file_not_found(Rc::clone(db_rc), Rc::clone(&db_dir), &mut db_index);
                    }

                    for fi in 0..files_count {
                        let file_pos = file_first_index + fi;
                        if file_dir.children[file_pos].search_found {
                            continue;
                        }
                        let file_clone = file_dir.children[file_pos].clone();
                        Self::new_file_found(&file_clone, Rc::clone(&db_dir), db_index, scan_level);
                        db_index += 1;
                    }

                    file_index += files_count;
                }
                1 => {
                    let file_c = file_dir.children[file_index].clone();
                    Self::new_file_found(&file_c, Rc::clone(&db_dir), db_index, scan_level);
                    db_index += 1;
                    file_index += 1;
                }
                -1 => {
                    let db_c = db_child.unwrap();
                    Self::db_file_not_found(Rc::clone(&db_c), Rc::clone(&db_dir), &mut db_index);
                }
                _ => {}
            }
        }
    }

    fn archive_is_deep_scanned(db_child: &RvFile) -> bool {
        db_child.children.iter().all(|c| {
            let c = c.borrow();
            c.is_directory()
                || c.crc.is_some()
                || c.sha1.is_some()
                || c.md5.is_some()
                || c.alt_crc.is_some()
                || c.alt_sha1.is_some()
                || c.alt_md5.is_some()
        })
    }

    fn should_scan_archive_contents(
        db_child: &RvFile,
        file_child: &ScannedFile,
        scan_level: crate::settings::EScanLevel,
    ) -> bool {
        if db_child.file_mod_time_stamp != file_child.file_mod_time_stamp {
            return true;
        }
        match scan_level {
            crate::settings::EScanLevel::Level3 => true,
            crate::settings::EScanLevel::Level2 => !Self::archive_is_deep_scanned(db_child),
            crate::settings::EScanLevel::Level1 => false,
        }
    }

    fn match_found(db_child: Rc<RefCell<RvFile>>, file_child: &ScannedFile, alt_match: bool) {
        let mut db_c = db_child.borrow_mut();
        
        // Invalidate stats cache since status is changing
        db_c.cached_stats = None;
        Self::apply_scanned_metadata(&mut db_c, file_child);
        if alt_match {
            db_c.file_status_set(FileStatus::IS_ALT_FILE);
        } else {
            db_c.file_status_clear(FileStatus::IS_ALT_FILE);
        }
        
        match db_c.file_type {
            FileType::Zip | FileType::SevenZip => {
                let status = db_c.dat_status();
                db_c.set_dat_got_status(status, dat_reader::enums::GotStatus::Got);
            },
            FileType::Dir => {
                let status = db_c.dat_status();
                db_c.set_dat_got_status(status, dat_reader::enums::GotStatus::Got);
            },
            FileType::File | FileType::FileZip | FileType::FileSevenZip | FileType::FileOnly => {
                let status = db_c.dat_status();
                db_c.set_dat_got_status(status, dat_reader::enums::GotStatus::Got);
            },
            _ => {}
        }
        db_c.rep_status_reset();
    }

    fn should_mark_corrupt(db_child: &RvFile, file_child: &ScannedFile) -> bool {
        let compatible_leaf_types =
            matches!(db_child.file_type, FileType::File | FileType::FileOnly | FileType::FileZip | FileType::FileSevenZip)
                && matches!(file_child.file_type, FileType::File | FileType::FileOnly | FileType::FileZip | FileType::FileSevenZip);
        db_child.dat_status() != dat_reader::enums::DatStatus::NotInDat
            && db_child.dat_status() != dat_reader::enums::DatStatus::InToSort
            && !db_child.is_directory()
            && !file_child.is_directory()
            && (db_child.file_type == file_child.file_type || compatible_leaf_types)
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
        db_child.deep_scanned = scanned_file.deep_scanned;
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
        db_child.sha256 = scanned_file.sha256.clone();
        db_child.alt_size = scanned_file.alt_size;
        db_child.alt_crc = scanned_file.alt_crc.clone();
        db_child.alt_sha1 = scanned_file.alt_sha1.clone();
        db_child.alt_md5 = scanned_file.alt_md5.clone();
        db_child.alt_sha256 = scanned_file.alt_sha256.clone();
        db_child.chd_version = scanned_file.chd_version;
    }

    fn new_file_found(
        file_child: &ScannedFile,
        db_dir: Rc<RefCell<RvFile>>,
        db_index: usize,
        scan_level: crate::settings::EScanLevel,
    ) {
        let parent_dat_status = db_dir.borrow().dat_status();
        let new_dat_status = if parent_dat_status == dat_reader::enums::DatStatus::InToSort {
            dat_reader::enums::DatStatus::InToSort
        } else {
            dat_reader::enums::DatStatus::NotInDat
        };

        let mut scanned = file_child.clone();
        if matches!(scanned.file_type, FileType::File | FileType::FileOnly | FileType::FileZip | FileType::FileSevenZip)
            && scan_level != crate::settings::EScanLevel::Level1
            && !scanned.deep_scanned
            && scanned.got_status != dat_reader::enums::GotStatus::FileLocked
        {
            let parent_path = db_dir.borrow().get_full_name();
            let full_path = std::path::PathBuf::from(parent_path)
                .join(&scanned.name)
                .to_string_lossy()
                .to_string();
            if let Ok(sf) = crate::scanner::Scanner::scan_raw_file(&full_path) {
                scanned = sf;
            }
        }

        let rc_child = Self::rv_file_from_scanned_file(&scanned, new_dat_status, Rc::clone(&db_dir));
        
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
                FileType::File | FileType::FileZip | FileType::FileSevenZip | FileType::FileOnly => {
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
#[path = "tests/file_scanning_tests.rs"]
mod tests;
