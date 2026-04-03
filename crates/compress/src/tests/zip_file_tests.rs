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
            let options = FileOptions::<()>::default()
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
            let options = FileOptions::<()>::default().compression_method(CompressionMethod::Deflated);
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

    #[test]
    fn test_read_local_header_offsets_supports_zip64_extra_offset() {
        let path = unique_temp_zip("compress_zip64_offsets");

        let file_name = b"a";
        let extra_data: Vec<u8> = {
            let mut v = Vec::new();
            v.extend_from_slice(&0x0001u16.to_le_bytes());
            v.extend_from_slice(&8u16.to_le_bytes());
            v.extend_from_slice(&0x1_0000_0000u64.to_le_bytes());
            v
        };

        let central_size = 46 + file_name.len() + extra_data.len();
        let mut bytes = Vec::new();

        bytes.extend_from_slice(&[0x50, 0x4B, 0x01, 0x02]);
        bytes.extend_from_slice(&0u16.to_le_bytes());
        bytes.extend_from_slice(&0u16.to_le_bytes());
        bytes.extend_from_slice(&0u16.to_le_bytes());
        bytes.extend_from_slice(&0u16.to_le_bytes());
        bytes.extend_from_slice(&0u16.to_le_bytes());
        bytes.extend_from_slice(&0u16.to_le_bytes());
        bytes.extend_from_slice(&0u32.to_le_bytes());
        bytes.extend_from_slice(&0u32.to_le_bytes());
        bytes.extend_from_slice(&0u32.to_le_bytes());
        bytes.extend_from_slice(&(file_name.len() as u16).to_le_bytes());
        bytes.extend_from_slice(&(extra_data.len() as u16).to_le_bytes());
        bytes.extend_from_slice(&0u16.to_le_bytes());
        bytes.extend_from_slice(&0u16.to_le_bytes());
        bytes.extend_from_slice(&0u16.to_le_bytes());
        bytes.extend_from_slice(&0u32.to_le_bytes());
        bytes.extend_from_slice(&0xFFFF_FFFFu32.to_le_bytes());
        bytes.extend_from_slice(file_name);
        bytes.extend_from_slice(&extra_data);

        bytes.extend_from_slice(&[0x50, 0x4B, 0x05, 0x06]);
        bytes.extend_from_slice(&0u16.to_le_bytes());
        bytes.extend_from_slice(&0u16.to_le_bytes());
        bytes.extend_from_slice(&1u16.to_le_bytes());
        bytes.extend_from_slice(&1u16.to_le_bytes());
        bytes.extend_from_slice(&(central_size as u32).to_le_bytes());
        bytes.extend_from_slice(&0u32.to_le_bytes());
        bytes.extend_from_slice(&0u16.to_le_bytes());

        fs::write(&path, bytes).unwrap();

        let offsets = ZipFile::read_local_header_offsets(&path).unwrap();
        assert_eq!(offsets, vec![0x1_0000_0000u64]);
        let _ = fs::remove_file(path);
    }

    #[test]
    fn test_open_read_stream_from_local_header_pointer_supports_raw_and_nonraw_stored() {
        let path = unique_temp_zip("compress_zip_localptr_stream");
        {
            let file = File::create(&path).unwrap();
            let mut writer = ZipWriter::new(file);
            let options = FileOptions::<()>::default().compression_method(CompressionMethod::Stored);
            writer.start_file("a.bin", options).unwrap();
            writer.write_all(b"hello").unwrap();
            writer.finish().unwrap();
        }

        let mut zip_file = ZipFile::new();
        assert_eq!(zip_file.zip_file_open(&path, 0, true), ZipReturn::ZipGood);
        let local = zip_file.get_file_header(0).unwrap().local_head.unwrap();

        {
            let (mut stream, stream_size, compression) = zip_file
                .zip_file_open_read_stream_from_local_header_pointer(local, true)
                .unwrap();
            let mut data = Vec::new();
            stream.read_to_end(&mut data).unwrap();
            assert_eq!(compression, 0);
            assert_eq!(stream_size, 5);
            assert_eq!(data, b"hello");
        }

        {
            let (mut stream, stream_size, compression) = zip_file
                .zip_file_open_read_stream_from_local_header_pointer(local, false)
                .unwrap();
            let mut data = Vec::new();
            stream.read_to_end(&mut data).unwrap();
            assert_eq!(compression, 0);
            assert_eq!(stream_size, 5);
            assert_eq!(data, b"hello");
        }

        zip_file.zip_file_close();
        let _ = fs::remove_file(path);
    }

    #[test]
    fn test_open_read_stream_from_local_header_pointer_rejects_data_descriptor_flag() {
        let path = unique_temp_zip("compress_zip_localptr_dd");
        {
            let file = File::create(&path).unwrap();
            let mut writer = ZipWriter::new(file);
            let options = FileOptions::<()>::default().compression_method(CompressionMethod::Stored);
            writer.start_file("a.bin", options).unwrap();
            writer.write_all(b"hello").unwrap();
            writer.finish().unwrap();
        }

        let mut zip_file = ZipFile::new();
        assert_eq!(zip_file.zip_file_open(&path, 0, true), ZipReturn::ZipGood);
        let local = zip_file.get_file_header(0).unwrap().local_head.unwrap();
        zip_file.zip_file_close();

        let mut bytes = fs::read(&path).unwrap();
        let flag_pos = (local as usize) + 6;
        let flags = u16::from_le_bytes([bytes[flag_pos], bytes[flag_pos + 1]]);
        let new_flags = flags | 8;
        bytes[flag_pos..flag_pos + 2].copy_from_slice(&new_flags.to_le_bytes());
        fs::write(&path, bytes).unwrap();

        let mut zip_file = ZipFile::new();
        assert_eq!(zip_file.zip_file_open(&path, 0, true), ZipReturn::ZipGood);
        let err = zip_file
            .zip_file_open_read_stream_from_local_header_pointer(local, false)
            .err()
            .unwrap();
        assert_eq!(err, ZipReturn::ZipCannotFastOpen);
        zip_file.zip_file_close();

        let _ = fs::remove_file(path);
    }

    #[test]
    fn test_open_read_stream_from_local_header_pointer_supports_bzip2_and_zstd() {
        let path = unique_temp_zip("compress_zip_localptr_more");
        {
            let file = File::create(&path).unwrap();
            let mut writer = ZipWriter::new(file);

            let bz = FileOptions::<()>::default().compression_method(CompressionMethod::Bzip2);
            writer.start_file("a.bz2", bz).unwrap();
            writer.write_all(b"hello").unwrap();

            let zs = FileOptions::<()>::default().compression_method(CompressionMethod::Zstd);
            writer.start_file("b.zst", zs).unwrap();
            writer.write_all(b"world").unwrap();

            writer.finish().unwrap();
        }

        let mut zip_file = ZipFile::new();
        assert_eq!(zip_file.zip_file_open(&path, 0, true), ZipReturn::ZipGood);

        let local_a = zip_file.get_file_header(0).unwrap().local_head.unwrap();
        let local_b = zip_file.get_file_header(1).unwrap().local_head.unwrap();

        {
            let (mut stream, size, method) = zip_file
                .zip_file_open_read_stream_from_local_header_pointer(local_a, false)
                .unwrap();
            let mut out = Vec::new();
            stream.read_to_end(&mut out).unwrap();
            assert_eq!(method, 12);
            assert_eq!(size, 5);
            assert_eq!(out, b"hello");
        }

        {
            let (mut stream, size, method) = zip_file
                .zip_file_open_read_stream_from_local_header_pointer(local_b, false)
                .unwrap();
            let mut out = Vec::new();
            stream.read_to_end(&mut out).unwrap();
            assert_eq!(method, 93);
            assert_eq!(size, 5);
            assert_eq!(out, b"world");
        }

        zip_file.zip_file_close();
        let _ = fs::remove_file(path);
    }

    #[test]
    fn test_zip_fake_memory_stream_roundtrip() {
        let mut zip = ZipFile::new();
        zip.zip_create_fake();
        assert_eq!(zip.zip_file_fake_open_memory_stream(), ZipReturn::ZipGood);

        let local_header = zip
            .zip_file_add_fake(
                "a.bin",
                0,
                5,
                5,
                &[0x36, 0x10, 0xA6, 0x86],
                0,
                19961224233200,
            )
            .unwrap();
        assert!(local_header.starts_with(&[0x50, 0x4B, 0x03, 0x04]));

        let bytes = zip.zip_file_fake_close_memory_stream().unwrap();
        assert!(!bytes.is_empty());
    }
