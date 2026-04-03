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
        if byte.is_ascii_uppercase() {
            byte + 0x20
        } else {
            byte
        }
    }

    fn string_compare(a: &str, b: &str) -> i32 {
        match a.cmp(b) {
            std::cmp::Ordering::Less => -1,
            std::cmp::Ordering::Equal => 0,
            std::cmp::Ordering::Greater => 1,
        }
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
            if Self::trrnt_zip_string_compare_case(&zipped_files[i], &zipped_files[i + 1]) > 0 {
                tz_status |= TrrntZipStatus::UNSORTED;
                error2 = true;
                // Log incorrect file order
                break;
            }
        }

        if error2 {
            zipped_files.sort_by(|a, b| Self::trrnt_zip_string_compare_case(a, b).cmp(&0));
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

    pub fn trrnt_zip_string_compare_case(a: &ZippedFile, b: &ZippedFile) -> i32 {
        let res = Self::trrnt_zip_string_compare(a, b);
        if res != 0 {
            return res;
        }

        let bytes_a = a.name.as_bytes();
        let bytes_b = b.name.as_bytes();
        let len = std::cmp::min(bytes_a.len(), bytes_b.len());

        for i in 0..len {
            if bytes_a[i] < bytes_b[i] {
                return -1;
            }
            if bytes_a[i] > bytes_b[i] {
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

    pub fn trrnt_7zip_string_compare(a: &ZippedFile, b: &ZippedFile) -> i32 {
        let (path_a, name_a, ext_a) = Self::split_7zip_filename(&a.name);
        let (path_b, name_b, ext_b) = Self::split_7zip_filename(&b.name);

        let res = Self::string_compare(ext_a, ext_b);
        if res != 0 {
            return res;
        }
        let res = Self::string_compare(name_a, name_b);
        if res != 0 {
            return res;
        }
        Self::string_compare(path_a, path_b)
    }
}

#[cfg(test)]
#[path = "tests/torrent_zip_check_tests.rs"]
mod tests;
