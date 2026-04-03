    use super::*;

    #[test]
    fn test_sort_dir_uses_case_insensitive_then_case_sensitive_ordering() {
        let mut dir = ScannedFile::new(FileType::Dir);
        let mut upper = ScannedFile::new(FileType::File);
        upper.name = "B.bin".to_string();
        let mut lower = ScannedFile::new(FileType::File);
        lower.name = "a.bin".to_string();
        dir.children.push(upper);
        dir.children.push(lower);

        dir.sort();

        assert_eq!(dir.children[0].name, "a.bin");
        assert_eq!(dir.children[1].name, "B.bin");
    }

    #[test]
    fn test_sort_dir_tiebreaks_case_sensitively() {
        let mut dir = ScannedFile::new(FileType::Dir);
        let mut lower = ScannedFile::new(FileType::File);
        lower.name = "a.bin".to_string();
        let mut upper = ScannedFile::new(FileType::File);
        upper.name = "A.bin".to_string();
        dir.children.push(lower);
        dir.children.push(upper);

        dir.sort();

        assert_eq!(dir.children[0].name, "A.bin");
        assert_eq!(dir.children[1].name, "a.bin");
    }

    #[test]
    fn test_sort_zip_uses_trrntzip_compare_case() {
        let mut zip = ScannedFile::new(FileType::Zip);
        let mut lower = ScannedFile::new(FileType::FileZip);
        lower.name = "a.bin".to_string();
        let mut upper = ScannedFile::new(FileType::FileZip);
        upper.name = "A.bin".to_string();
        zip.children.push(lower);
        zip.children.push(upper);

        zip.sort();

        assert_eq!(zip.children[0].name, "A.bin");
        assert_eq!(zip.children[1].name, "a.bin");
    }

    #[test]
    fn test_sort_sevenzip_uses_extension_then_name_then_path() {
        let mut seven = ScannedFile::new(FileType::SevenZip);
        let mut a = ScannedFile::new(FileType::FileSevenZip);
        a.name = "b.zzz".to_string();
        let mut b = ScannedFile::new(FileType::FileSevenZip);
        b.name = "a.aaa".to_string();
        seven.children.push(a);
        seven.children.push(b);

        seven.sort();

        assert_eq!(seven.children[0].name, "a.aaa");
        assert_eq!(seven.children[1].name, "b.zzz");
    }
