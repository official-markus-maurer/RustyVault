    use super::*;
    use std::fs::File;
    use std::io::Write;
    use tempfile::tempdir;
    use zip::write::SimpleFileOptions;
    use zip::{CompressionMethod, ZipWriter};

    #[test]
    fn test_scan_raw_file() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("test_rom.bin");
        let mut file = File::create(&file_path).unwrap();
        file.write_all(b"dummy rom data").unwrap();

        let scanned = Scanner::scan_raw_file(file_path.to_str().unwrap()).expect("Failed to scan file");
        
        assert_eq!(scanned.name, "test_rom.bin");
        assert_eq!(scanned.size, Some(14));
        assert!(scanned.crc.is_some());
        assert!(scanned.sha1.is_some());
        assert!(scanned.md5.is_some());
        assert_eq!(scanned.got_status, GotStatus::Got);
    }

    #[test]
    fn test_scan_raw_file_populates_headerless_alt_hashes() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("headered.nes");
        let mut file = File::create(&file_path).unwrap();
        let mut bytes = vec![0x4E, 0x45, 0x53, 0x1A];
        bytes.extend_from_slice(&[0; 12]);
        bytes.extend_from_slice(b"DATA");
        file.write_all(&bytes).unwrap();

        let scanned = Scanner::scan_raw_file(file_path.to_str().unwrap()).expect("Failed to scan file");
        let mut crc = Crc32Hasher::new();
        crc.update(b"DATA");

        assert_eq!(scanned.header_file_type, HeaderFileType::NES);
        assert_eq!(scanned.alt_size, Some(4));
        assert_eq!(scanned.alt_crc, Some(crc.finalize().to_be_bytes().to_vec()));
        assert!(scanned.alt_sha1.is_some());
        assert!(scanned.alt_md5.is_some());
        assert!(scanned.status_flags.contains(FileStatus::HEADER_FILE_TYPE_FROM_HEADER));
        assert!(scanned.status_flags.contains(FileStatus::ALT_CRC_FROM_HEADER));
    }
    #[test]
    fn test_scan_directory_parallel() {
        let dir = tempdir().unwrap();
        
        // Create a few files to be scanned in parallel
        for i in 0..5 {
            let file_path = dir.path().join(format!("test_rom_{}.bin", i));
            std::fs::write(&file_path, format!("dummy rom data {}", i).as_bytes()).unwrap();
        }
        
        // Add a nested directory
        let sub_dir = dir.path().join("subdir");
        std::fs::create_dir(&sub_dir).unwrap();
        std::fs::write(sub_dir.join("nested.bin"), b"nested data").unwrap();

        let scanned_children = Scanner::scan_directory(dir.path().to_str().unwrap());
        
        // Should find 5 files + 1 directory
        assert_eq!(scanned_children.len(), 6);
        
        // Find the subdirectory and verify it was scanned recursively
        let subdir_node = scanned_children.iter().find(|c| c.file_type == FileType::Dir).unwrap();
        assert_eq!(subdir_node.name, "subdir");
        assert_eq!(subdir_node.children.len(), 1);
        assert_eq!(subdir_node.children[0].name, "nested.bin");
        assert_eq!(subdir_node.children[0].size, Some(11));
    }

    #[test]
    fn test_scan_directory_level2_hashes_loose_files() {
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("hashed.bin");
        std::fs::write(&file_path, b"hash me").unwrap();

        let scanned_children = Scanner::scan_directory_with_level(
            dir.path().to_str().unwrap(),
            crate::settings::EScanLevel::Level2,
        );

        let scanned_file = scanned_children.iter().find(|c| c.name == "hashed.bin").unwrap();
        assert!(scanned_file.crc.is_some());
        assert!(scanned_file.sha1.is_some());
        assert!(scanned_file.md5.is_some());
        assert!(scanned_file.deep_scanned);
    }

    #[test]
    fn test_scan_archive_file_populates_headerless_alt_hashes() {
        let dir = tempdir().unwrap();
        let archive_path = dir.path().join("headered.zip");
        {
            let file = File::create(&archive_path).unwrap();
            let mut writer = ZipWriter::new(file);
            let options = SimpleFileOptions::default()
                .compression_method(CompressionMethod::Deflated)
                .compression_level(Some(9));
            let mut bytes = vec![0x4E, 0x45, 0x53, 0x1A];
            bytes.extend_from_slice(&[0; 12]);
            bytes.extend_from_slice(b"DATA");
            writer.start_file("rom.nes", options).unwrap();
            writer.write_all(&bytes).unwrap();
            writer.finish().unwrap();
        }

        let scanned_archive = Scanner::scan_archive_file(
            FileType::Zip,
            archive_path.to_str().unwrap(),
            0,
            true,
        )
        .expect("Failed to scan archive");

        let scanned_file = scanned_archive
            .children
            .iter()
            .find(|entry| entry.name == "rom.nes")
            .unwrap();
        let mut crc = Crc32Hasher::new();
        crc.update(b"DATA");

        assert_eq!(scanned_file.header_file_type, HeaderFileType::NES);
        assert_eq!(scanned_file.alt_size, Some(4));
        assert_eq!(scanned_file.alt_crc, Some(crc.finalize().to_be_bytes().to_vec()));
        assert!(scanned_file.alt_sha1.is_some());
        assert!(scanned_file.alt_md5.is_some());
        assert!(scanned_file.status_flags.contains(FileStatus::SIZE_FROM_HEADER));
        assert!(scanned_file.status_flags.contains(FileStatus::CRC_FROM_HEADER));
        assert!(scanned_file.status_flags.contains(FileStatus::HEADER_FILE_TYPE_FROM_HEADER));
        assert!(scanned_file.status_flags.contains(FileStatus::ALT_CRC_FROM_HEADER));
    }
