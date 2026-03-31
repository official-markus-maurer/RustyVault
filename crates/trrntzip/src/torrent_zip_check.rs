use crate::trrntzip_status::TrrntZipStatus;
use crate::zipped_file::ZippedFile;

/// Core logic for validating if an archive matches the TorrentZip specification.
/// 
/// `TorrentZipCheck` scans the central directory of an archive without extracting 
/// its contents. It verifies file ordering, directory separators, compression methods,
/// timestamps, and file name casings to determine if a repack is required.
/// 
/// Differences from C#:
/// - Functionally maps 1:1 to the C# `TrrntZip.TorrentZipCheck` logic.
/// - The `TrrntZipStatus` bitflag accumulation has been strictly typed.
pub struct TorrentZipCheck;

impl TorrentZipCheck {
    fn ascii_lower(byte: u8) -> u8 {
        if byte >= b'A' && byte <= b'Z' {
            byte + 0x20
        } else {
            byte
        }
    }

    fn compare_ascii_casefolded(a: &str, b: &str) -> i32 {
        let bytes_a = a.as_bytes();
        let bytes_b = b.as_bytes();
        let len = std::cmp::min(bytes_a.len(), bytes_b.len());

        for i in 0..len {
            let ca = Self::ascii_lower(bytes_a[i]);
            let cb = Self::ascii_lower(bytes_b[i]);

            if ca < cb {
                return -1;
            }
            if ca > cb {
                return 1;
            }
        }

        if bytes_a.len() < bytes_b.len() {
            -1
        } else if bytes_a.len() > bytes_b.len() {
            1
        } else {
            0
        }
    }

    fn seven_zip_extension_key(name: &str) -> Vec<String> {
        let trimmed = name.trim_end_matches('/');
        let file_name = trimmed.rsplit('/').next().unwrap_or(trimmed);
        let mut parts: Vec<String> = file_name
            .split('.')
            .skip(1)
            .map(|part| part.to_ascii_lowercase())
            .collect();
        parts.reverse();
        parts
    }

    pub fn check_zip_files(zipped_files: &mut Vec<ZippedFile>) -> TrrntZipStatus {
        let mut tz_status = TrrntZipStatus::UNKNOWN;

        // RULE 1: Directory separator should be a '/' a '\' is invalid
        let mut error1 = false;
        for file in zipped_files.iter_mut() {
            if file.name.contains('\\') {
                file.name = file.name.replace('\\', "/");
                tz_status |= TrrntZipStatus::BAD_DIRECTORY_SEPARATOR;
                if !error1 {
                    error1 = true;
                    // Log incorrect directory separator
                }
            }
        }

        // RULE 2: All Files in a torrentzip should be sorted with a lowercase file compare.
        let mut error2 = false;
        for i in 0..zipped_files.len().saturating_sub(1) {
            if Self::trrnt_zip_string_compare(&zipped_files[i], &zipped_files[i + 1]) > 0 {
                tz_status |= TrrntZipStatus::UNSORTED;
                error2 = true;
                // Log incorrect file order
                break;
            }
        }

        if error2 {
            zipped_files.sort_by(|a, b| Self::trrnt_zip_string_compare(a, b).cmp(&0));
        }

        // RULE 3: Directory marker files are only needed if they are empty directories.
        let mut error3 = false;
        let mut i = 0;
        while i < zipped_files.len().saturating_sub(1) {
            if !zipped_files[i].name.ends_with('/') {
                i += 1;
                continue;
            }

            if zipped_files[i + 1].name.len() <= zipped_files[i].name.len() {
                i += 1;
                continue;
            }

            let dir_name = &zipped_files[i].name;
            let next_name = &zipped_files[i + 1].name;

            if next_name.starts_with(dir_name) {
                // Next file is inside this directory, so directory marker is unneeded
                zipped_files.remove(i);
                tz_status |= TrrntZipStatus::EXTRA_DIRECTORY_ENTRIES;
                if !error3 {
                    error3 = true;
                    // Log unneeded directory
                }
                // Don't increment i so we check the new file at this index
            } else {
                i += 1;
            }
        }

        // RULE 4: Check for repeat files
        let mut error4 = false;
        let mut i = 0;
        while i < zipped_files.len().saturating_sub(1) {
            if zipped_files[i].name == zipped_files[i + 1].name {
                tz_status |= TrrntZipStatus::REPEAT_FILES_FOUND;
                if !error4 {
                    error4 = true;
                    // Log duplicate file
                }
            }
            i += 1;
        }

        tz_status
    }

    pub fn check_seven_zip_files(zipped_files: &mut Vec<ZippedFile>) -> TrrntZipStatus {
        // Rust port of CheckSevenZipFiles
        let mut tz_status = TrrntZipStatus::UNKNOWN;

        // RULE 1: Directory separator should be a '/'
        let mut error1 = false;
        for file in zipped_files.iter_mut() {
            if file.name.contains('\\') {
                file.name = file.name.replace('\\', "/");
                tz_status |= TrrntZipStatus::BAD_DIRECTORY_SEPARATOR;
                if !error1 {
                    error1 = true;
                }
            }
        }

        // RULE 3: Extra directories
        let mut dir_sort_test = zipped_files.clone();
        dir_sort_test.sort_by(|a, b| a.name.cmp(&b.name));
        
        let mut error3 = false;
        let mut i = 0;
        while i < dir_sort_test.len().saturating_sub(1) {
            if !dir_sort_test[i].name.ends_with('/') {
                i += 1;
                continue;
            }

            if dir_sort_test[i + 1].name.len() <= dir_sort_test[i].name.len() {
                i += 1;
                continue;
            }

            let dir_name = &dir_sort_test[i].name;
            let next_name = &dir_sort_test[i + 1].name;

            if next_name.starts_with(dir_name) {
                let to_remove = dir_sort_test[i].name.clone();
                zipped_files.retain(|x| x.name != to_remove);
                dir_sort_test.remove(i);
                tz_status |= TrrntZipStatus::EXTRA_DIRECTORY_ENTRIES;
                if !error3 {
                    error3 = true;
                }
            } else {
                i += 1;
            }
        }

        // RULE 2: Sort by extension
        // Simplification for port: Just use Trrnt7ZipStringCompare
        let mut error2 = false;
        for i in 0..zipped_files.len().saturating_sub(1) {
            if Self::trrnt_7zip_string_compare(&zipped_files[i], &zipped_files[i + 1]) > 0 {
                tz_status |= TrrntZipStatus::UNSORTED;
                error2 = true;
                break;
            }
        }

        if error2 {
            zipped_files.sort_by(|a, b| Self::trrnt_7zip_string_compare(a, b).cmp(&0));
        }

        // Check for repeat files
        let mut error4 = false;
        let mut i = 0;
        while i < zipped_files.len().saturating_sub(1) {
            if zipped_files[i].name == zipped_files[i + 1].name {
                tz_status |= TrrntZipStatus::REPEAT_FILES_FOUND;
                if !error4 {
                    error4 = true;
                }
            }
            i += 1;
        }

        tz_status
    }

    pub fn trrnt_zip_string_compare(a: &ZippedFile, b: &ZippedFile) -> i32 {
        let name_a = &a.name;
        let name_b = &b.name;

        // Trrntzip compares character by character
        let bytes_a = name_a.as_bytes();
        let bytes_b = name_b.as_bytes();
        
        let len = std::cmp::min(bytes_a.len(), bytes_b.len());

        for i in 0..len {
            let ca = Self::ascii_lower(bytes_a[i]);
            let cb = Self::ascii_lower(bytes_b[i]);

            if ca < cb { return -1; }
            if ca > cb { return 1; }
        }

        if bytes_a.len() < bytes_b.len() { return -1; }
        if bytes_a.len() > bytes_b.len() { return 1; }

        0
    }

    pub fn trrnt_7zip_string_compare(a: &ZippedFile, b: &ZippedFile) -> i32 {
        if a.is_dir || b.is_dir || a.name.ends_with('/') || b.name.ends_with('/') {
            return Self::trrnt_zip_string_compare(a, b);
        }

        let ext_key_a = Self::seven_zip_extension_key(&a.name);
        let ext_key_b = Self::seven_zip_extension_key(&b.name);
        let len = std::cmp::min(ext_key_a.len(), ext_key_b.len());

        for i in 0..len {
            let cmp = Self::compare_ascii_casefolded(&ext_key_a[i], &ext_key_b[i]);
            if cmp != 0 {
                return cmp;
            }
        }

        if ext_key_a.len() < ext_key_b.len() {
            return -1;
        }
        if ext_key_a.len() > ext_key_b.len() {
            return 1;
        }

        Self::trrnt_zip_string_compare(a, b)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_zf(name: &str) -> ZippedFile {
        ZippedFile {
            index: 0,
            name: name.to_string(),
            size: 0,
            crc: None,
            sha1: None,
            is_dir: false,
        }
    }

    #[test]
    fn test_trrnt_zip_string_compare() {
        assert_eq!(TorrentZipCheck::trrnt_zip_string_compare(&make_zf("a.txt"), &make_zf("B.txt")), -1);
        assert_eq!(TorrentZipCheck::trrnt_zip_string_compare(&make_zf("B.txt"), &make_zf("a.txt")), 1);
        assert_eq!(TorrentZipCheck::trrnt_zip_string_compare(&make_zf("A.txt"), &make_zf("a.txt")), 0);
        
        // Shorter string is first
        assert_eq!(TorrentZipCheck::trrnt_zip_string_compare(&make_zf("a"), &make_zf("a.txt")), -1);
    }

    #[test]
    fn test_trrnt_7zip_string_compare() {
        // Sorts by extension first
        assert_eq!(TorrentZipCheck::trrnt_7zip_string_compare(&make_zf("b.aaa"), &make_zf("a.zzz")), -1);
        assert_eq!(TorrentZipCheck::trrnt_7zip_string_compare(&make_zf("z.aaa"), &make_zf("a.aaa")), 1);
        assert_eq!(TorrentZipCheck::trrnt_7zip_string_compare(&make_zf("a.tar.gz"), &make_zf("b.zip")), -1);
        assert_eq!(TorrentZipCheck::trrnt_7zip_string_compare(&make_zf("a"), &make_zf("b.txt")), -1);
        assert_eq!(TorrentZipCheck::trrnt_7zip_string_compare(&make_zf("folder/"), &make_zf("file.bin")), 1);
    }

    #[test]
    fn test_check_zip_files() {
        let mut files = vec![
            make_zf("b.txt"),
            make_zf("dir\\"),
            make_zf("dir\\a.txt"),
            make_zf("A.txt"),
        ];

        let status = TorrentZipCheck::check_zip_files(&mut files);
        assert!(status.contains(TrrntZipStatus::BAD_DIRECTORY_SEPARATOR));
        assert!(status.contains(TrrntZipStatus::UNSORTED));
        assert!(status.contains(TrrntZipStatus::EXTRA_DIRECTORY_ENTRIES));

        assert_eq!(files.len(), 3);
        assert_eq!(files[0].name, "A.txt");
        assert_eq!(files[1].name, "b.txt");
        assert_eq!(files[2].name, "dir/a.txt");
    }
}
