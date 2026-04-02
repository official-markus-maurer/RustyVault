    use super::*;

    #[test]
    fn test_sort_uses_windows_style_case_insensitive_ordering() {
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
