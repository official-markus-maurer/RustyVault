    use super::*;
    use compress::i_compress::ICompress;
    use compress::zip_file::ZipFile;
    use compress::seven_zip::SevenZipFile;
    use tempfile::tempdir;
    use zip::write::FileOptions;
    use zip::{CompressionMethod, ZipArchive, ZipWriter};
    use sevenz_rust::compress_to_path as compress_to_7z_path;

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

    #[test]
    fn test_rezip_files_uses_cp437_when_possible() {
        let temp = tempdir().unwrap();
        let source_path = temp.path().join("sample.zip");
        let name = "é.bin";

        {
            let file = fs::File::create(&source_path).unwrap();
            let mut writer = ZipWriter::new(file);
            let options = FileOptions::<()>::default()
                .compression_method(CompressionMethod::Deflated)
                .compression_level(Some(9));
            writer.start_file(name, options).unwrap();
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
            name: name.to_string(),
            size: 4,
            crc: None,
            sha1: None,
            is_dir: false,
        }];

        let status = TorrentZipRebuild::rezip_files(&zipped_files, &mut zip_file, ZipStructure::ZipTrrnt);
        assert_eq!(status, TrrntZipStatus::VALID_TRRNTZIP);

        let rebuilt_bytes = fs::read(&source_path).unwrap();
        let eocd_offset = rebuilt_bytes
            .windows(4)
            .rposition(|window| window == [0x50, 0x4B, 0x05, 0x06])
            .unwrap();
        let central_directory_offset = u32::from_le_bytes([
            rebuilt_bytes[eocd_offset + 16],
            rebuilt_bytes[eocd_offset + 17],
            rebuilt_bytes[eocd_offset + 18],
            rebuilt_bytes[eocd_offset + 19],
        ]) as usize;

        let sig = u32::from_le_bytes([
            rebuilt_bytes[central_directory_offset],
            rebuilt_bytes[central_directory_offset + 1],
            rebuilt_bytes[central_directory_offset + 2],
            rebuilt_bytes[central_directory_offset + 3],
        ]);
        assert_eq!(sig, 0x02014B50);

        let flags = u16::from_le_bytes([
            rebuilt_bytes[central_directory_offset + 8],
            rebuilt_bytes[central_directory_offset + 9],
        ]);
        assert_eq!(flags & 0x0800, 0);

        let file_name_length = u16::from_le_bytes([
            rebuilt_bytes[central_directory_offset + 28],
            rebuilt_bytes[central_directory_offset + 29],
        ]) as usize;
        let name_start = central_directory_offset + 46;
        let name_end = name_start + file_name_length;
        let name_bytes = &rebuilt_bytes[name_start..name_end];
        let expected = compress::codepage_437::encode(name).unwrap();
        assert_eq!(name_bytes, expected.as_slice());
    }

    #[test]
    fn test_rezip_files_deflates_empty_directory_entries() {
        let temp = tempdir().unwrap();
        let source_path = temp.path().join("sample.zip");
        let dir_name = "empty/";

        {
            let file = fs::File::create(&source_path).unwrap();
            let mut writer = ZipWriter::new(file);
            let options = FileOptions::<()>::default()
                .compression_method(CompressionMethod::Stored);
            writer.add_directory(dir_name, options).unwrap();
            writer.finish().unwrap();
        }

        let mut zip_file = ZipFile::new();
        assert_eq!(
            zip_file.zip_file_open(&source_path.to_string_lossy(), 0, true),
            ZipReturn::ZipGood
        );

        let zipped_files = vec![ZippedFile {
            index: 0,
            name: dir_name.to_string(),
            size: 0,
            crc: None,
            sha1: None,
            is_dir: true,
        }];

        let status = TorrentZipRebuild::rezip_files(&zipped_files, &mut zip_file, ZipStructure::ZipTrrnt);
        assert_eq!(status, TrrntZipStatus::VALID_TRRNTZIP);

        let rebuilt_bytes = fs::read(&source_path).unwrap();
        let eocd_offset = rebuilt_bytes
            .windows(4)
            .rposition(|window| window == [0x50, 0x4B, 0x05, 0x06])
            .unwrap();
        let central_directory_offset = u32::from_le_bytes([
            rebuilt_bytes[eocd_offset + 16],
            rebuilt_bytes[eocd_offset + 17],
            rebuilt_bytes[eocd_offset + 18],
            rebuilt_bytes[eocd_offset + 19],
        ]) as usize;

        let rel_offset = u32::from_le_bytes([
            rebuilt_bytes[central_directory_offset + 42],
            rebuilt_bytes[central_directory_offset + 43],
            rebuilt_bytes[central_directory_offset + 44],
            rebuilt_bytes[central_directory_offset + 45],
        ]) as usize;
        let local_sig = u32::from_le_bytes([
            rebuilt_bytes[rel_offset],
            rebuilt_bytes[rel_offset + 1],
            rebuilt_bytes[rel_offset + 2],
            rebuilt_bytes[rel_offset + 3],
        ]);
        assert_eq!(local_sig, 0x04034B50);

        let compression_method = u16::from_le_bytes([
            rebuilt_bytes[rel_offset + 8],
            rebuilt_bytes[rel_offset + 9],
        ]);
        assert_eq!(compression_method, 8);

        let compressed_size = u32::from_le_bytes([
            rebuilt_bytes[rel_offset + 18],
            rebuilt_bytes[rel_offset + 19],
            rebuilt_bytes[rel_offset + 20],
            rebuilt_bytes[rel_offset + 21],
        ]);
        assert!(compressed_size > 0);
    }

    #[test]
    fn test_rezip_files_normalizes_backslashes_to_forward_slashes() {
        let temp = tempdir().unwrap();
        let source_path = temp.path().join("sample.zip");

        {
            let file = fs::File::create(&source_path).unwrap();
            let mut writer = ZipWriter::new(file);
            let options = FileOptions::<()>::default()
                .compression_method(CompressionMethod::Deflated)
                .compression_level(Some(9));
            writer.start_file("dir\\a.bin", options).unwrap();
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
            name: "dir\\a.bin".to_string(),
            size: 4,
            crc: None,
            sha1: None,
            is_dir: false,
        }];

        let status = TorrentZipRebuild::rezip_files(&zipped_files, &mut zip_file, ZipStructure::ZipTrrnt);
        assert_eq!(status, TrrntZipStatus::VALID_TRRNTZIP);

        let mut archive = ZipArchive::new(fs::File::open(&source_path).unwrap()).unwrap();
        assert_eq!(archive.len(), 1);
        assert_eq!(archive.by_index(0).unwrap().name(), "dir/a.bin");
    }

    #[test]
    fn test_rezip_files_removes_unneeded_directory_entries() {
        let temp = tempdir().unwrap();
        let source_path = temp.path().join("sample.zip");

        {
            let file = fs::File::create(&source_path).unwrap();
            let mut writer = ZipWriter::new(file);
            let options = FileOptions::<()>::default()
                .compression_method(CompressionMethod::Deflated)
                .compression_level(Some(9));
            writer.add_directory("dir/", FileOptions::<()>::default()).unwrap();
            writer.start_file("dir/a.bin", options).unwrap();
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
                index: 0,
                name: "dir/".to_string(),
                size: 0,
                crc: None,
                sha1: None,
                is_dir: true,
            },
            ZippedFile {
                index: 1,
                name: "dir/a.bin".to_string(),
                size: 4,
                crc: None,
                sha1: None,
                is_dir: false,
            },
        ];

        let status = TorrentZipRebuild::rezip_files(&zipped_files, &mut zip_file, ZipStructure::ZipTrrnt);
        assert_eq!(status, TrrntZipStatus::VALID_TRRNTZIP);

        let mut archive = ZipArchive::new(fs::File::open(&source_path).unwrap()).unwrap();
        assert_eq!(archive.len(), 1);
        assert_eq!(archive.by_index(0).unwrap().name(), "dir/a.bin");
    }

    #[test]
    fn test_rezip_files_rejects_duplicate_names() {
        let temp = tempdir().unwrap();
        let source_path = temp.path().join("sample.zip");

        {
            let file = fs::File::create(&source_path).unwrap();
            let mut writer = ZipWriter::new(file);
            let options = FileOptions::<()>::default()
                .compression_method(CompressionMethod::Deflated)
                .compression_level(Some(9));
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
                index: 0,
                name: "a.bin".to_string(),
                size: 4,
                crc: None,
                sha1: None,
                is_dir: false,
            },
            ZippedFile {
                index: 0,
                name: "a.bin".to_string(),
                size: 4,
                crc: None,
                sha1: None,
                is_dir: false,
            },
        ];

        let status = TorrentZipRebuild::rezip_files(&zipped_files, &mut zip_file, ZipStructure::ZipTrrnt);
        assert_eq!(status, TrrntZipStatus::REPEAT_FILES_FOUND);
        assert!(source_path.exists());
    }

    fn trrnt7z_sort_key(name: &str) -> (String, String, String) {
        let dir_index = name.rfind('/');
        let (path, base) = if let Some(i) = dir_index {
            (&name[..i], &name[i + 1..])
        } else {
            ("", name)
        };
        let ext_index = base.rfind('.');
        let (stem, ext) = if let Some(i) = ext_index {
            (&base[..i], &base[i + 1..])
        } else {
            (base, "")
        };
        (ext.to_string(), stem.to_string(), path.to_string())
    }

    #[test]
    fn test_rezip_files_sevenzip_normalizes_and_sorts() {
        let temp = tempdir().unwrap();
        let src_dir = temp.path().join("src7z");
        fs::create_dir_all(src_dir.join("dir")).unwrap();
        fs::write(src_dir.join("b.txt"), b"bbbb").unwrap();
        fs::write(src_dir.join("a.bin"), b"aaaa").unwrap();
        fs::write(src_dir.join("dir").join("c.dat"), b"cccc").unwrap();

        let source_path = temp.path().join("sample.7z");
        compress_to_7z_path(&src_dir, &source_path).unwrap();

        let mut seven = SevenZipFile::new();
        assert_eq!(
            seven.zip_file_open(&source_path.to_string_lossy(), 0, true),
            ZipReturn::ZipGood
        );

        let mut zipped_files = Vec::new();
        for i in 0..seven.local_files_count() {
            let h = seven.get_file_header(i).unwrap();
            let mut name = h.filename.clone();
            if name.contains('/') {
                name = name.replace('/', "\\");
            }
            zipped_files.push(ZippedFile {
                index: i as i32,
                name,
                size: h.uncompressed_size,
                crc: h.crc.clone(),
                sha1: None,
                is_dir: h.is_directory,
            });
        }
        seven.zip_file_close();
        zipped_files.reverse();

        let mut seven = SevenZipFile::new();
        assert_eq!(
            seven.zip_file_open(&source_path.to_string_lossy(), 0, true),
            ZipReturn::ZipGood
        );
        let status = TorrentZipRebuild::rezip_files(&zipped_files, &mut seven, ZipStructure::SevenZipSLZMA);
        assert_eq!(status, TrrntZipStatus::VALID_TRRNTZIP);

        let mut rebuilt = SevenZipFile::new();
        assert_eq!(
            rebuilt.zip_file_open(&source_path.to_string_lossy(), 0, true),
            ZipReturn::ZipGood
        );
        let mut names = Vec::new();
        for i in 0..rebuilt.local_files_count() {
            let h = rebuilt.get_file_header(i).unwrap();
            names.push(h.filename.clone());
        }
        rebuilt.zip_file_close();

        assert!(names.iter().all(|n| !n.contains('\\')));

        let mut expected = names.clone();
        expected.sort_by_key(|a| trrnt7z_sort_key(a));
        assert_eq!(names, expected);
    }

    #[test]
    fn test_rezip_files_sevenzip_rejects_duplicate_names() {
        let temp = tempdir().unwrap();
        let src_dir = temp.path().join("src7z_dupe");
        fs::create_dir_all(&src_dir).unwrap();
        fs::write(src_dir.join("a.bin"), b"aaaa").unwrap();

        let source_path = temp.path().join("sample.7z");
        compress_to_7z_path(&src_dir, &source_path).unwrap();

        let mut seven = SevenZipFile::new();
        assert_eq!(
            seven.zip_file_open(&source_path.to_string_lossy(), 0, true),
            ZipReturn::ZipGood
        );

        let mut picked = None;
        for i in 0..seven.local_files_count() {
            let h = seven.get_file_header(i).unwrap();
            if !h.filename.is_empty() && !h.is_directory {
                picked = Some((i, h.filename.clone(), h.uncompressed_size));
                break;
            }
        }
        let (picked_index, picked_name, picked_size) = picked.unwrap();
        let zipped_files = vec![
            ZippedFile {
                index: picked_index as i32,
                name: picked_name.replace('/', "\\"),
                size: picked_size,
                crc: None,
                sha1: None,
                is_dir: false,
            },
            ZippedFile {
                index: picked_index as i32,
                name: picked_name.replace('/', "\\"),
                size: picked_size,
                crc: None,
                sha1: None,
                is_dir: false,
            },
        ];

        let status = TorrentZipRebuild::rezip_files(&zipped_files, &mut seven, ZipStructure::SevenZipSLZMA);
        assert_eq!(status, TrrntZipStatus::REPEAT_FILES_FOUND);
        assert!(source_path.exists());
    }

    #[test]
    fn test_rezip_files_zipzstd_normalizes_sorts_and_prunes_dirs() {
        let temp = tempdir().unwrap();
        let source_path = temp.path().join("sample.zip");

        {
            let file = fs::File::create(&source_path).unwrap();
            let mut writer = ZipWriter::new(file);
            writer.add_directory("dir/", FileOptions::<()>::default()).unwrap();
            let options = FileOptions::<()>::default()
                .compression_method(CompressionMethod::Deflated)
                .compression_level(Some(9));
            writer.start_file("dir\\b.bin", options).unwrap();
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
            ZippedFile { index: 1, name: "dir\\b.bin".to_string(), size: 4, crc: None, sha1: None, is_dir: false },
            ZippedFile { index: 2, name: "a.bin".to_string(), size: 4, crc: None, sha1: None, is_dir: false },
            ZippedFile { index: 0, name: "dir/".to_string(), size: 0, crc: None, sha1: None, is_dir: true },
        ];

        let status = TorrentZipRebuild::rezip_files(&zipped_files, &mut zip_file, ZipStructure::ZipZSTD);
        assert_eq!(status, TrrntZipStatus::VALID_TRRNTZIP);

        let mut archive = ZipArchive::new(fs::File::open(&source_path).unwrap()).unwrap();
        assert_eq!(archive.len(), 2);
        assert_eq!(archive.by_index(0).unwrap().name(), "a.bin");
        assert_eq!(archive.by_index(1).unwrap().name(), "dir/b.bin");

        let rebuilt_bytes = fs::read(&source_path).unwrap();
        let eocd_offset = rebuilt_bytes
            .windows(4)
            .rposition(|window| window == [0x50, 0x4B, 0x05, 0x06])
            .unwrap();
        let comment_len = u16::from_le_bytes([rebuilt_bytes[eocd_offset + 20], rebuilt_bytes[eocd_offset + 21]]) as usize;
        let comment = &rebuilt_bytes[eocd_offset + 22..eocd_offset + 22 + comment_len];
        assert!(String::from_utf8_lossy(comment).starts_with("RVZSTD-"));
    }

    #[test]
    fn test_rezip_files_ziptdc_is_sorted() {
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
            ZippedFile { index: 0, name: "b.bin".to_string(), size: 4, crc: None, sha1: None, is_dir: false },
            ZippedFile { index: 1, name: "a.bin".to_string(), size: 4, crc: None, sha1: None, is_dir: false },
        ];

        let status = TorrentZipRebuild::rezip_files(&zipped_files, &mut zip_file, ZipStructure::ZipTDC);
        assert_eq!(status, TrrntZipStatus::VALID_TRRNTZIP);

        let mut archive = ZipArchive::new(fs::File::open(&source_path).unwrap()).unwrap();
        assert_eq!(archive.len(), 2);
        assert_eq!(archive.by_index(0).unwrap().name(), "a.bin");
        assert_eq!(archive.by_index(1).unwrap().name(), "b.bin");
    }
