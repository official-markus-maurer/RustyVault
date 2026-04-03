    use super::*;
    use compress::i_compress::ICompress;
    use compress::zip_file::ZipFile;
    use tempfile::tempdir;
    use zip::write::FileOptions;
    use zip::{CompressionMethod, ZipArchive, ZipWriter};

    #[test]
    fn test_rezip_files_builds_deterministic_torrentzip() {
        let temp = tempdir().unwrap();
        let source_path = temp.path().join("sample.zip");

        {
            let file = fs::File::create(&source_path).unwrap();
            let mut writer = ZipWriter::new(file);
            let options = FileOptions::<()>::default()
                .compression_method(CompressionMethod::Deflated)
                .compression_level(Some(9));
            writer.start_file("b.bin", options).unwrap();
            writer.write_all(b"bbbb").unwrap();
            writer.start_file("a.bin", options).unwrap();
            writer.write_all(b"aaaa").unwrap();
            writer.finish().unwrap();
        }

        let mut zip_file = ZipFile::new();
        assert_eq!(
            zip_file.zip_file_open(&source_path.to_string_lossy(), 0, true),
            ZipReturn::ZipGood
        );

        let zipped_files = vec![
            ZippedFile {
                index: 1,
                name: "a.bin".to_string(),
                size: 4,
                crc: None,
                sha1: None,
                is_dir: false,
            },
            ZippedFile {
                index: 0,
                name: "b.bin".to_string(),
                size: 4,
                crc: None,
                sha1: None,
                is_dir: false,
            },
        ];

        let status = TorrentZipRebuild::rezip_files(&zipped_files, &mut zip_file, ZipStructure::ZipTrrnt);
        assert_eq!(status, TrrntZipStatus::VALID_TRRNTZIP);

        let rebuilt_bytes = fs::read(&source_path).unwrap();
        let rebuilt_a = TorrentZipRebuild::read_raw_zip_entry(&rebuilt_bytes, "a.bin").unwrap();
        assert_eq!(rebuilt_a.uncompressed_size, 4);
        let archive = ZipArchive::new(fs::File::open(&source_path).unwrap()).unwrap();
        let eocd_offset = rebuilt_bytes
            .windows(4)
            .rposition(|window| window == [0x50, 0x4B, 0x05, 0x06])
            .unwrap();
        let central_directory_size = u32::from_le_bytes([
            rebuilt_bytes[eocd_offset + 12],
            rebuilt_bytes[eocd_offset + 13],
            rebuilt_bytes[eocd_offset + 14],
            rebuilt_bytes[eocd_offset + 15],
        ]) as usize;
        let central_directory_offset = u32::from_le_bytes([
            rebuilt_bytes[eocd_offset + 16],
            rebuilt_bytes[eocd_offset + 17],
            rebuilt_bytes[eocd_offset + 18],
            rebuilt_bytes[eocd_offset + 19],
        ]) as usize;
        let mut crc = Crc32Hasher::new();
        crc.update(
            &rebuilt_bytes[central_directory_offset..central_directory_offset + central_directory_size],
        );
        let expected_comment = format!("TORRENTZIPPED-{:08X}", crc.finalize());
        assert_eq!(String::from_utf8_lossy(archive.comment()), expected_comment);

        let mut zip_file = ZipFile::new();
        assert_eq!(
            zip_file.zip_file_open(&source_path.to_string_lossy(), 0, true),
            ZipReturn::ZipGood
        );

        let zipped_files = vec![
            ZippedFile {
                index: 0,
                name: "a.bin".to_string(),
                size: 4,
                crc: None,
                sha1: None,
                is_dir: false,
            },
            ZippedFile {
                index: 1,
                name: "b.bin".to_string(),
                size: 4,
                crc: None,
                sha1: None,
                is_dir: false,
            },
        ];

        let status = TorrentZipRebuild::rezip_files(&zipped_files, &mut zip_file, ZipStructure::ZipTrrnt);
        assert_eq!(status, TrrntZipStatus::VALID_TRRNTZIP);

        let rebuilt_bytes_again = fs::read(&source_path).unwrap();
        assert_eq!(rebuilt_bytes, rebuilt_bytes_again);
    }

    #[test]
    fn test_rezip_files_with_hard_stop_removes_samtmp() {
        let temp = tempdir().unwrap();
        let source_path = temp.path().join("sample.zip");

        {
            let file = fs::File::create(&source_path).unwrap();
            let mut writer = ZipWriter::new(file);
            let options = FileOptions::<()>::default().compression_method(CompressionMethod::Stored);
            writer.start_file("a.bin", options).unwrap();
            writer.write_all(b"aaaa").unwrap();
            writer.finish().unwrap();
        }

        let mut zip_file = ZipFile::new();
        assert_eq!(
            zip_file.zip_file_open(&source_path.to_string_lossy(), 0, true),
            ZipReturn::ZipGood
        );

        let zipped_files = vec![ZippedFile {
            index: 0,
            name: "a.bin".to_string(),
            size: 4,
            crc: None,
            sha1: None,
            is_dir: false,
        }];

        let control = ProcessControl::new();
        control.request_hard_stop();
        let status = TorrentZipRebuild::rezip_files_with_control(
            &zipped_files,
            &mut zip_file,
            ZipStructure::ZipTrrnt,
            Some(&control),
        );

        assert_eq!(status, TrrntZipStatus::USER_ABORTED_HARD);
        assert!(!temp.path().join("__sample.zip.samtmp").exists());
        assert!(source_path.exists());
    }

    #[test]
    fn test_cleanup_samtmp_files_removes_nested_temp_files() {
        let temp = tempdir().unwrap();
        let nested = temp.path().join("nested");
        fs::create_dir_all(&nested).unwrap();
        fs::write(temp.path().join("__one.zip.samtmp"), b"a").unwrap();
        fs::write(nested.join("__two.zip.samtmp"), b"b").unwrap();
        fs::create_dir_all(nested.join("__three.zip.samtmp.dir")).unwrap();
        fs::write(nested.join("keep.zip"), b"c").unwrap();

        let removed = TorrentZipRebuild::cleanup_samtmp_files(temp.path(), true);

        assert_eq!(removed, 3);
        assert!(!temp.path().join("__one.zip.samtmp").exists());
        assert!(!nested.join("__two.zip.samtmp").exists());
        assert!(!nested.join("__three.zip.samtmp.dir").exists());
        assert!(nested.join("keep.zip").exists());
    }
