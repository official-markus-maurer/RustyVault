    use super::*;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};
    use crc32fast::Hasher as Crc32Hasher;

    fn unique_temp_zip(name: &str) -> String {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir()
            .join(format!("{}_{}.zip", name, unique))
            .to_string_lossy()
            .to_string()
    }

    #[test]
    fn test_zip_file_write_stream_persists_written_data() {
        let path = unique_temp_zip("compress_zip_write");
        let mut zip_file = ZipFile::new();

        assert_eq!(zip_file.zip_file_create(&path), ZipReturn::ZipGood);
        let mut stream = zip_file
            .zip_file_open_write_stream(false, "hello.txt", 5, 8, Some(19961224233200))
            .unwrap();
        stream.write_all(b"hello").unwrap();
        drop(stream);
        assert_eq!(
            zip_file.zip_file_close_write_stream(&[0x36, 0x10, 0xA6, 0x86]),
            ZipReturn::ZipGood
        );
        zip_file.zip_file_close();

        let mut reopened = ZipFile::new();
        assert_eq!(reopened.zip_file_open(&path, 0, true), ZipReturn::ZipGood);
        assert_eq!(reopened.local_files_count(), 1);
        assert_eq!(reopened.get_file_header(0).unwrap().filename, "hello.txt");

        let (mut reader, size) = reopened.zip_file_open_read_stream(0).unwrap();
        let mut data = Vec::new();
        reader.read_to_end(&mut data).unwrap();
        assert_eq!(size, 5);
        assert_eq!(data, b"hello");

        reopened.zip_file_close();
        let _ = fs::remove_file(path);
    }

    #[test]
    fn test_zip_file_detects_valid_torrentzip_structure() {
        let path = unique_temp_zip("compress_zip_comment");
        {
            let file = File::create(&path).unwrap();
            let mut writer = ZipWriter::new(file);
            let dt = zip::DateTime::from_date_and_time(1996, 12, 24, 23, 32, 0).unwrap();
            let options = FileOptions::default()
                .compression_method(CompressionMethod::Deflated)
                .compression_level(Some(9))
                .last_modified_time(dt);
            writer.start_file("a.txt", options).unwrap();
            writer.write_all(b"a").unwrap();
            writer.set_comment("TORRENTZIPPED-00000000");
            writer.finish().unwrap();
        }

        {
            let mut bytes = fs::read(&path).unwrap();
            let eocd_offset = bytes
                .windows(4)
                .rposition(|window| window == [0x50, 0x4B, 0x05, 0x06])
                .unwrap();

            let central_directory_size = u32::from_le_bytes([
                bytes[eocd_offset + 12],
                bytes[eocd_offset + 13],
                bytes[eocd_offset + 14],
                bytes[eocd_offset + 15],
            ]) as usize;
            let central_directory_offset = u32::from_le_bytes([
                bytes[eocd_offset + 16],
                bytes[eocd_offset + 17],
                bytes[eocd_offset + 18],
                bytes[eocd_offset + 19],
            ]) as usize;
            let comment_length = u16::from_le_bytes([bytes[eocd_offset + 20], bytes[eocd_offset + 21]]) as usize;
            let comment_offset = eocd_offset + 22;

            let mut crc = Crc32Hasher::new();
            crc.update(&bytes[central_directory_offset..central_directory_offset + central_directory_size]);
            let cd_crc = format!("{:08X}", crc.finalize());

            let expected_prefix = b"TORRENTZIPPED-";
            assert_eq!(comment_length, expected_prefix.len() + 8);
            assert_eq!(&bytes[comment_offset..comment_offset + expected_prefix.len()], expected_prefix);

            bytes[comment_offset + expected_prefix.len()..comment_offset + expected_prefix.len() + 8]
                .copy_from_slice(cd_crc.as_bytes());

            fs::write(&path, bytes).unwrap();
        }

        let mut zip_file = ZipFile::new();
        assert_eq!(zip_file.zip_file_open(&path, 0, true), ZipReturn::ZipGood);
        assert_eq!(zip_file.zip_struct(), ZipStructure::ZipTrrnt);
        zip_file.zip_file_close();

        let _ = fs::remove_file(path);
    }

    #[test]
    fn test_zip_file_reads_local_header_offsets() {
        let path = unique_temp_zip("compress_zip_offsets");
        {
            let file = File::create(&path).unwrap();
            let mut writer = ZipWriter::new(file);
            let options = FileOptions::default().compression_method(CompressionMethod::Deflated);
            writer.start_file("a.txt", options).unwrap();
            writer.write_all(b"aaa").unwrap();
            writer.start_file("b.txt", options).unwrap();
            writer.write_all(b"bbbb").unwrap();
            writer.finish().unwrap();
        }

        let mut zip_file = ZipFile::new();
        assert_eq!(zip_file.zip_file_open(&path, 0, true), ZipReturn::ZipGood);
        let first = zip_file.get_file_header(0).unwrap().local_head.unwrap();
        let second = zip_file.get_file_header(1).unwrap().local_head.unwrap();
        assert_eq!(first, 0);
        assert!(second > first);
        zip_file.zip_file_close();

        let _ = fs::remove_file(path);
    }
