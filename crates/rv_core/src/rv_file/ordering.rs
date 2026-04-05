impl RvFile {
    fn ascii_lower(byte: u8) -> u8 {
        if byte.is_ascii_uppercase() {
            byte + 0x20
        } else {
            byte
        }
    }

    fn trrnt_zip_string_compare(a: &str, b: &str) -> std::cmp::Ordering {
        let bytes_a = a.as_bytes();
        let bytes_b = b.as_bytes();
        let len = std::cmp::min(bytes_a.len(), bytes_b.len());

        for i in 0..len {
            let ca = Self::ascii_lower(bytes_a[i]);
            let cb = Self::ascii_lower(bytes_b[i]);
            match ca.cmp(&cb) {
                std::cmp::Ordering::Equal => {}
                ord => return ord,
            }
        }

        bytes_a.len().cmp(&bytes_b.len())
    }

    fn trrnt_zip_string_compare_case(a: &str, b: &str) -> std::cmp::Ordering {
        let res = Self::trrnt_zip_string_compare(a, b);
        if res != std::cmp::Ordering::Equal {
            return res;
        }
        a.cmp(b)
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

    fn trrnt_7zip_string_compare(a: &str, b: &str) -> std::cmp::Ordering {
        let (path_a, name_a, ext_a) = Self::split_7zip_filename(a);
        let (path_b, name_b, ext_b) = Self::split_7zip_filename(b);

        match ext_a.cmp(ext_b) {
            std::cmp::Ordering::Equal => {}
            ord => return ord,
        }
        match name_a.cmp(name_b) {
            std::cmp::Ordering::Equal => {}
            ord => return ord,
        }
        path_a.cmp(path_b)
    }

    fn directory_name_compare(a: &str, b: &str) -> std::cmp::Ordering {
        a.to_ascii_lowercase().cmp(&b.to_ascii_lowercase())
    }

    fn compare_name_key(f1: FileType, name1: &str, f2: FileType, name2: &str) -> std::cmp::Ordering {
        if f1 == FileType::FileZip || f2 == FileType::FileZip {
            return Self::trrnt_zip_string_compare_case(name1, name2);
        }
        if f1 == FileType::FileSevenZip || f2 == FileType::FileSevenZip {
            return Self::trrnt_7zip_string_compare(name1, name2);
        }

        let res = Self::directory_name_compare(name1, name2);
        if res != std::cmp::Ordering::Equal {
            return res;
        }
        f1.cmp(&f2)
    }

    fn ordering_to_i32(ordering: std::cmp::Ordering) -> i32 {
        match ordering {
            std::cmp::Ordering::Less => -1,
            std::cmp::Ordering::Equal => 0,
            std::cmp::Ordering::Greater => 1,
        }
    }

    pub fn child_name_search(&self, file_type: FileType, name: &str) -> (i32, usize) {
        let mut bottom = 0usize;
        let mut top = self.children.len();
        let mut mid = 0usize;
        let mut res = -1i32;

        while bottom < top && res != 0 {
            mid = (bottom + top) / 2;
            let mid_key = {
                let mid_ref = self.children[mid].borrow();
                Self::compare_name_key(file_type, name, mid_ref.file_type, &mid_ref.name)
            };
            res = Self::ordering_to_i32(mid_key);
            if res < 0 {
                top = mid;
            } else if res > 0 {
                bottom = mid + 1;
            }
        }

        let mut index = mid;
        if res == 0 {
            while index > 0 {
                let prev_key = {
                    let prev_ref = self.children[index - 1].borrow();
                    Self::compare_name_key(file_type, name, prev_ref.file_type, &prev_ref.name)
                };
                if prev_key != std::cmp::Ordering::Equal {
                    break;
                }
                index -= 1;
            }
        } else if res > 0 {
            index += 1;
        }

        (res, index)
    }

    fn child_insert_index(&self, child: &RvFile) -> usize {
        let mut bottom = 0usize;
        let mut top = self.children.len();

        while bottom < top {
            let mid = (bottom + top) / 2;
            let mid_key = {
                let mid_ref = self.children[mid].borrow();
                Self::compare_name_key(child.file_type, &child.name, mid_ref.file_type, &mid_ref.name)
            };
            if mid_key == std::cmp::Ordering::Greater {
                bottom = mid + 1;
            } else {
                top = mid;
            }
        }

        bottom
    }
}
