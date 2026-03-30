use crate::rv_file::RvFile;
use crate::scanned_file::ScannedFile;
use dat_reader::enums::FileType;
use crate::settings::EScanLevel;

pub struct FileCompare;

pub fn compare_db_to_file(db_file: &RvFile, file_c: &ScannedFile) -> i32 {
    let name_cmp = db_file.name.as_bytes().cmp(file_c.name.as_bytes());
    match name_cmp {
        std::cmp::Ordering::Less => -1,
        std::cmp::Ordering::Equal => 0,
        std::cmp::Ordering::Greater => 1,
    }
}

impl FileCompare {
    pub fn phase_1_test(db_file: &RvFile, test_file: &ScannedFile, e_scan_level: EScanLevel, index_case: i32) -> (bool, bool) {
        let mut matched_alt = false;

        // Name comparison
        let retv = if index_case == 0 {
            db_file.name.cmp(&test_file.name)
        } else {
            db_file.name.to_lowercase().cmp(&test_file.name.to_lowercase())
        };

        if retv != std::cmp::Ordering::Equal {
            return (false, matched_alt);
        }

        let db_file_type = db_file.file_type;
        let test_file_type = test_file.file_type;

        if db_file_type != test_file_type {
            return (false, matched_alt);
        }

        // Directories and Archives don't need deep hashing matches at this level
        if db_file_type == FileType::Dir || db_file_type == FileType::Zip || db_file_type == FileType::SevenZip {
            return (true, matched_alt);
        }

        // Header type check
        if db_file.header_file_type_required() {
            if db_file.header_file_type() != test_file.header_file_type {
                return (false, matched_alt);
            }
        }

        // If test file has CRC, we can do full hash matching
        if test_file.crc.is_some() {
            let matched = Self::compare_with_alt(db_file, test_file, &mut matched_alt);
            return (matched, matched_alt);
        }

        // If no hashes, and we are requiring Level 2 or 3 scanning but the db file isn't deep scanned
        // we can't match just on timestamp
        let is_deep_scanned = db_file.file_status_is(crate::rv_file::FileStatus::CRC_FROM_HEADER); // simplified check
        if e_scan_level != EScanLevel::Level1 && !is_deep_scanned {
            return (false, matched_alt);
        }

        // Timestamp match
        if db_file.file_mod_time_stamp != test_file.file_mod_time_stamp {
            return (false, matched_alt);
        }

        if db_file.size == test_file.size {
            return (true, matched_alt);
        }

        (false, matched_alt)
    }

    fn compare_with_alt(db_file: &RvFile, test_file: &ScannedFile, matched_alt: &mut bool) -> bool {
        // Standard compare
        let mut match_ok = true;
        if db_file.size.is_some() && db_file.size != test_file.size { match_ok = false; }
        if db_file.crc.is_some() && db_file.crc != test_file.crc { match_ok = false; }
        if db_file.sha1.is_some() && db_file.sha1 != test_file.sha1 { match_ok = false; }
        if db_file.md5.is_some() && db_file.md5 != test_file.md5 { match_ok = false; }

        if match_ok {
            *matched_alt = false;
            return true;
        }

        // Alt compare
        let mut alt_ok = true;
        if db_file.alt_size.is_some() && db_file.alt_size != test_file.size { alt_ok = false; }
        if db_file.alt_crc.is_some() && db_file.alt_crc != test_file.crc { alt_ok = false; }
        if db_file.alt_sha1.is_some() && db_file.alt_sha1 != test_file.sha1 { alt_ok = false; }
        if db_file.alt_md5.is_some() && db_file.alt_md5 != test_file.md5 { alt_ok = false; }

        if alt_ok && (db_file.alt_size.is_some() || db_file.alt_crc.is_some()) {
            *matched_alt = true;
            return true;
        }

        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compare_db_to_file() {
        let mut db_file = RvFile::new(FileType::File);
        db_file.name = "B_File.zip".to_string();

        let mut sc_file = ScannedFile::new(FileType::File);
        sc_file.name = "A_File.zip".to_string();

        assert_eq!(compare_db_to_file(&db_file, &sc_file), 1);
        
        sc_file.name = "C_File.zip".to_string();
        assert_eq!(compare_db_to_file(&db_file, &sc_file), -1);

        sc_file.name = "B_File.zip".to_string();
        assert_eq!(compare_db_to_file(&db_file, &sc_file), 0);
    }

    #[test]
    fn test_phase_1_test_hashes() {
        let mut db_file = RvFile::new(FileType::File);
        db_file.name = "rom.bin".to_string();
        db_file.size = Some(1024);
        db_file.crc = Some(vec![0xAA, 0xBB, 0xCC, 0xDD]);

        let mut sc_file = ScannedFile::new(FileType::File);
        sc_file.name = "rom.bin".to_string();
        sc_file.size = Some(1024);
        sc_file.crc = Some(vec![0xAA, 0xBB, 0xCC, 0xDD]);

        let (matched, alt) = FileCompare::phase_1_test(&db_file, &sc_file, EScanLevel::Level2, 0);
        assert!(matched);
        assert!(!alt);

        // Test Alt Match
        db_file.crc = Some(vec![0x11, 0x22, 0x33, 0x44]);
        db_file.alt_size = Some(1024);
        db_file.alt_crc = Some(vec![0xAA, 0xBB, 0xCC, 0xDD]);

        let (matched, alt) = FileCompare::phase_1_test(&db_file, &sc_file, EScanLevel::Level2, 0);
        assert!(matched);
        assert!(alt);

        // Test Mismatch
        sc_file.crc = Some(vec![0xFF, 0xFF, 0xFF, 0xFF]);
        let (matched, _) = FileCompare::phase_1_test(&db_file, &sc_file, EScanLevel::Level2, 0);
        assert!(!matched);
    }
}
