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
