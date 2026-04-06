use super::*;
use crc32fast::Hasher as Crc32Hasher;
use std::fs;
use std::time::{SystemTime, UNIX_EPOCH};
use zip::write::FileOptions;
use zip::CompressionMethod;

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
        let comment_length =
            u16::from_le_bytes([bytes[eocd_offset + 20], bytes[eocd_offset + 21]]) as usize;
        let comment_offset = eocd_offset + 22;

        let mut crc = Crc32Hasher::new();
        crc.update(
            &bytes[central_directory_offset..central_directory_offset + central_directory_size],
        );
        let cd_crc = format!("{:08X}", crc.finalize());

        let expected_prefix = b"TORRENTZIPPED-";
        assert_eq!(comment_length, expected_prefix.len() + 8);
        assert_eq!(
            &bytes[comment_offset..comment_offset + expected_prefix.len()],
            expected_prefix
        );

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
fn test_zip_file_structure_validation_rejects_utf8_flag_for_cp437_name() {
    let path = unique_temp_zip("compress_zip_utf8_flag");
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
    let comment_offset = eocd_offset + 22;

    let central_offset = central_directory_offset;
    assert_eq!(
        &bytes[central_offset..central_offset + 4],
        &[0x50, 0x4B, 0x01, 0x02]
    );
    let local_offset = u32::from_le_bytes([
        bytes[central_offset + 42],
        bytes[central_offset + 43],
        bytes[central_offset + 44],
        bytes[central_offset + 45],
    ]) as usize;
    assert_eq!(
        &bytes[local_offset..local_offset + 4],
        &[0x50, 0x4B, 0x03, 0x04]
    );

    let utf8_bit = 0x0800u16;
    let new_flags = utf8_bit.to_le_bytes();
    bytes[central_offset + 8] = new_flags[0];
    bytes[central_offset + 9] = new_flags[1];
    bytes[local_offset + 6] = new_flags[0];
    bytes[local_offset + 7] = new_flags[1];

    let mut crc = Crc32Hasher::new();
    crc.update(&bytes[central_directory_offset..central_directory_offset + central_directory_size]);
    let cd_crc = format!("{:08X}", crc.finalize());
    bytes[comment_offset + "TORRENTZIPPED-".len()..comment_offset + "TORRENTZIPPED-".len() + 8]
        .copy_from_slice(cd_crc.as_bytes());

    fs::write(&path, bytes).unwrap();

    let mut zip_file = ZipFile::new();
    assert_eq!(zip_file.zip_file_open(&path, 0, true), ZipReturn::ZipGood);
    assert_eq!(zip_file.zip_struct(), ZipStructure::None);
    zip_file.zip_file_close();

    let _ = fs::remove_file(path);
}

#[test]
fn test_zip_file_create_with_structure_enforces_compression_method() {
    let path = unique_temp_zip("compress_zip_struct_method");
    let mut zip_file = ZipFile::new();
    assert_eq!(
        zip_file.zip_file_create_with_structure(&path, ZipStructure::ZipTrrnt),
        ZipReturn::ZipGood
    );

    let err = match zip_file.zip_file_open_write_stream(false, "a.txt", 1, 0, None) {
        Ok(_) => panic!("expected error"),
        Err(e) => e,
    };
    assert_eq!(err, ZipReturn::ZipTrrntzipIncorrectCompressionUsed);

    zip_file.zip_file_close();
    let _ = fs::remove_file(path);
}

#[test]
fn test_zip_file_create_with_structure_enforces_file_order() {
    let path = unique_temp_zip("compress_zip_struct_order");
    let mut zip_file = ZipFile::new();
    assert_eq!(
        zip_file.zip_file_create_with_structure(&path, ZipStructure::ZipTrrnt),
        ZipReturn::ZipGood
    );

    let mut stream = zip_file
        .zip_file_open_write_stream(false, "b.txt", 1, 8, None)
        .unwrap();
    stream.write_all(b"b").unwrap();
    drop(stream);
    assert_eq!(
        zip_file.zip_file_close_write_stream(&[]),
        ZipReturn::ZipGood
    );

    let err = match zip_file.zip_file_open_write_stream(false, "a.txt", 1, 8, None) {
        Ok(_) => panic!("expected error"),
        Err(e) => e,
    };
    assert_eq!(err, ZipReturn::ZipTrrntzipIncorrectFileOrder);

    zip_file.zip_file_close();
    let _ = fs::remove_file(path);
}

#[test]
fn test_zip_file_create_with_structure_allows_case_distinct_directory_prefix() {
    let path = unique_temp_zip("compress_zip_struct_dir_prefix_case");
    let mut zip_file = ZipFile::new();
    assert_eq!(
        zip_file.zip_file_create_with_structure(&path, ZipStructure::ZipTrrnt),
        ZipReturn::ZipGood
    );

    let dir_name = "A/";
    let dir_stream = zip_file
        .zip_file_open_write_stream(false, dir_name, 0, 8, None)
        .unwrap();
    drop(dir_stream);
    assert_eq!(
        zip_file.zip_file_close_write_stream(&[]),
        ZipReturn::ZipGood
    );

    let file_name = "a/file.txt";
    let mut stream = zip_file
        .zip_file_open_write_stream(false, file_name, 1, 8, None)
        .unwrap();
    stream.write_all(b"x").unwrap();
    drop(stream);
    assert_eq!(
        zip_file.zip_file_close_write_stream(&[]),
        ZipReturn::ZipGood
    );

    zip_file.zip_file_close();

    let mut reopened = ZipFile::new();
    assert_eq!(reopened.zip_file_open(&path, 0, true), ZipReturn::ZipGood);
    assert_eq!(reopened.zip_struct(), ZipStructure::ZipTrrnt);
    reopened.zip_file_close();

    let _ = fs::remove_file(path);
}

#[test]
fn test_zip_file_create_with_structure_writes_correct_comment_crc() {
    let path = unique_temp_zip("compress_zip_struct_comment");
    let mut zip_file = ZipFile::new();
    assert_eq!(
        zip_file.zip_file_create_with_structure(&path, ZipStructure::ZipTrrnt),
        ZipReturn::ZipGood
    );

    let mut stream = zip_file
        .zip_file_open_write_stream(false, "a.txt", 1, 8, None)
        .unwrap();
    stream.write_all(b"a").unwrap();
    drop(stream);
    assert_eq!(
        zip_file.zip_file_close_write_stream(&[]),
        ZipReturn::ZipGood
    );
    zip_file.zip_file_close();

    let bytes = fs::read(&path).unwrap();
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
    let comment_length =
        u16::from_le_bytes([bytes[eocd_offset + 20], bytes[eocd_offset + 21]]) as usize;
    let comment_offset = eocd_offset + 22;

    let mut crc = Crc32Hasher::new();
    crc.update(&bytes[central_directory_offset..central_directory_offset + central_directory_size]);
    let expected_comment = format!("TORRENTZIPPED-{:08X}", crc.finalize());

    assert_eq!(comment_length, expected_comment.len());
    assert_eq!(
        &bytes[comment_offset..comment_offset + comment_length],
        expected_comment.as_bytes()
    );

    let mut reopened = ZipFile::new();
    assert_eq!(reopened.zip_file_open(&path, 0, true), ZipReturn::ZipGood);
    assert_eq!(reopened.zip_struct(), ZipStructure::ZipTrrnt);
    reopened.zip_file_close();

    let _ = fs::remove_file(path);
}

#[test]
fn test_zip_file_detects_torrentzip_with_zip64_extra_fields() {
    let path = unique_temp_zip("compress_zip_struct_zip64_extra");

    let filename = b"a.txt";
    let flags: u16 = 2;
    let method: u16 = 8;
    let dos_time: u16 = ZipFile::TORRENTZIP_DOS_TIME;
    let dos_date: u16 = ZipFile::TORRENTZIP_DOS_DATE;

    let zip64_extra = {
        let mut e = Vec::new();
        e.extend_from_slice(&0x0001u16.to_le_bytes());
        e.extend_from_slice(&16u16.to_le_bytes());
        e.extend_from_slice(&0u64.to_le_bytes());
        e.extend_from_slice(&0u64.to_le_bytes());
        e
    };

    let mut bytes = Vec::new();

    // Local header
    bytes.extend_from_slice(&0x04034B50u32.to_le_bytes());
    bytes.extend_from_slice(&45u16.to_le_bytes()); // version needed (zip64)
    bytes.extend_from_slice(&flags.to_le_bytes());
    bytes.extend_from_slice(&method.to_le_bytes());
    bytes.extend_from_slice(&dos_time.to_le_bytes());
    bytes.extend_from_slice(&dos_date.to_le_bytes());
    bytes.extend_from_slice(&0u32.to_le_bytes()); // crc32
    bytes.extend_from_slice(&0xFFFF_FFFFu32.to_le_bytes()); // comp size
    bytes.extend_from_slice(&0xFFFF_FFFFu32.to_le_bytes()); // uncomp size
    bytes.extend_from_slice(&(filename.len() as u16).to_le_bytes());
    bytes.extend_from_slice(&(zip64_extra.len() as u16).to_le_bytes());
    bytes.extend_from_slice(filename);
    bytes.extend_from_slice(&zip64_extra);

    let central_directory_offset = bytes.len();

    // Central directory header
    bytes.extend_from_slice(&0x02014B50u32.to_le_bytes());
    bytes.extend_from_slice(&0u16.to_le_bytes()); // version made by
    bytes.extend_from_slice(&45u16.to_le_bytes()); // version needed
    bytes.extend_from_slice(&flags.to_le_bytes());
    bytes.extend_from_slice(&method.to_le_bytes());
    bytes.extend_from_slice(&dos_time.to_le_bytes());
    bytes.extend_from_slice(&dos_date.to_le_bytes());
    bytes.extend_from_slice(&0u32.to_le_bytes()); // crc32
    bytes.extend_from_slice(&0xFFFF_FFFFu32.to_le_bytes()); // comp size
    bytes.extend_from_slice(&0xFFFF_FFFFu32.to_le_bytes()); // uncomp size
    bytes.extend_from_slice(&(filename.len() as u16).to_le_bytes());
    bytes.extend_from_slice(&(zip64_extra.len() as u16).to_le_bytes());
    bytes.extend_from_slice(&0u16.to_le_bytes()); // comment len
    bytes.extend_from_slice(&0u16.to_le_bytes()); // disk start
    bytes.extend_from_slice(&0u16.to_le_bytes()); // int attrs
    bytes.extend_from_slice(&0u32.to_le_bytes()); // ext attrs
    bytes.extend_from_slice(&0u32.to_le_bytes()); // local header offset
    bytes.extend_from_slice(filename);
    bytes.extend_from_slice(&zip64_extra);

    let central_directory_size = bytes.len() - central_directory_offset;

    let mut crc = Crc32Hasher::new();
    crc.update(&bytes[central_directory_offset..central_directory_offset + central_directory_size]);
    let comment = format!("TORRENTZIPPED-{:08X}", crc.finalize());

    // EOCD
    bytes.extend_from_slice(&0x06054B50u32.to_le_bytes());
    bytes.extend_from_slice(&0u16.to_le_bytes());
    bytes.extend_from_slice(&0u16.to_le_bytes());
    bytes.extend_from_slice(&1u16.to_le_bytes());
    bytes.extend_from_slice(&1u16.to_le_bytes());
    bytes.extend_from_slice(&(central_directory_size as u32).to_le_bytes());
    bytes.extend_from_slice(&(central_directory_offset as u32).to_le_bytes());
    bytes.extend_from_slice(&(comment.len() as u16).to_le_bytes());
    bytes.extend_from_slice(comment.as_bytes());

    fs::write(&path, &bytes).unwrap();

    let mut zip_file = ZipFile::new();
    assert_eq!(zip_file.zip_file_open(&path, 0, true), ZipReturn::ZipGood);
    assert_eq!(zip_file.zip_struct(), ZipStructure::ZipTrrnt);
    zip_file.zip_file_close();

    let _ = fs::remove_file(path);
}

#[test]
fn test_zip_file_detects_torrentzip_with_zip64_eocd_boundary_values() {
    let path = unique_temp_zip("compress_zip_struct_zip64_eocd_boundary");

    let filename = b"a.txt";
    let flags: u16 = 2;
    let method: u16 = 8;
    let dos_time: u16 = ZipFile::TORRENTZIP_DOS_TIME;
    let dos_date: u16 = ZipFile::TORRENTZIP_DOS_DATE;

    let mut bytes = Vec::new();

    // Local header + data ("a" deflated)
    bytes.extend_from_slice(&0x04034B50u32.to_le_bytes());
    bytes.extend_from_slice(&20u16.to_le_bytes());
    bytes.extend_from_slice(&flags.to_le_bytes());
    bytes.extend_from_slice(&method.to_le_bytes());
    bytes.extend_from_slice(&dos_time.to_le_bytes());
    bytes.extend_from_slice(&dos_date.to_le_bytes());
    bytes.extend_from_slice(&0u32.to_le_bytes()); // crc32
    bytes.extend_from_slice(&2u32.to_le_bytes()); // comp size
    bytes.extend_from_slice(&0u32.to_le_bytes()); // uncomp size
    bytes.extend_from_slice(&(filename.len() as u16).to_le_bytes());
    bytes.extend_from_slice(&0u16.to_le_bytes());
    bytes.extend_from_slice(filename);
    bytes.extend_from_slice(&[0x03, 0x00]); // empty deflate stream for size=0? minimal; keep sizes consistent with 2 bytes

    let central_directory_offset = bytes.len() as u64;

    // Central directory header
    bytes.extend_from_slice(&0x02014B50u32.to_le_bytes());
    bytes.extend_from_slice(&0u16.to_le_bytes()); // version made by
    bytes.extend_from_slice(&20u16.to_le_bytes()); // version needed
    bytes.extend_from_slice(&flags.to_le_bytes());
    bytes.extend_from_slice(&method.to_le_bytes());
    bytes.extend_from_slice(&dos_time.to_le_bytes());
    bytes.extend_from_slice(&dos_date.to_le_bytes());
    bytes.extend_from_slice(&0u32.to_le_bytes()); // crc32
    bytes.extend_from_slice(&2u32.to_le_bytes()); // comp size
    bytes.extend_from_slice(&0u32.to_le_bytes()); // uncomp size
    bytes.extend_from_slice(&(filename.len() as u16).to_le_bytes());
    bytes.extend_from_slice(&0u16.to_le_bytes()); // extra len
    bytes.extend_from_slice(&0u16.to_le_bytes()); // comment len
    bytes.extend_from_slice(&0u16.to_le_bytes()); // disk start
    bytes.extend_from_slice(&0u16.to_le_bytes()); // int attrs
    bytes.extend_from_slice(&0u32.to_le_bytes()); // ext attrs
    bytes.extend_from_slice(&0u32.to_le_bytes()); // local header offset
    bytes.extend_from_slice(filename);

    let central_directory_size = (bytes.len() as u64) - central_directory_offset;

    let mut crc = Crc32Hasher::new();
    crc.update(
        &bytes[central_directory_offset as usize
            ..(central_directory_offset + central_directory_size) as usize],
    );
    let comment = format!("TORRENTZIPPED-{:08X}", crc.finalize());

    // Zip64 EOCD + locator even though archive is small (boundary behavior)
    let zip64_eocd_offset = bytes.len() as u64;
    bytes.extend_from_slice(&0x06064B50u32.to_le_bytes());
    bytes.extend_from_slice(&44u64.to_le_bytes());
    bytes.extend_from_slice(&45u16.to_le_bytes());
    bytes.extend_from_slice(&45u16.to_le_bytes());
    bytes.extend_from_slice(&0u32.to_le_bytes());
    bytes.extend_from_slice(&0u32.to_le_bytes());
    bytes.extend_from_slice(&1u64.to_le_bytes());
    bytes.extend_from_slice(&1u64.to_le_bytes());
    bytes.extend_from_slice(&central_directory_size.to_le_bytes());
    bytes.extend_from_slice(&central_directory_offset.to_le_bytes());

    bytes.extend_from_slice(&0x07064B50u32.to_le_bytes());
    bytes.extend_from_slice(&0u32.to_le_bytes());
    bytes.extend_from_slice(&zip64_eocd_offset.to_le_bytes());
    bytes.extend_from_slice(&1u32.to_le_bytes());

    // EOCD with sentinel values forcing zip64Required
    bytes.extend_from_slice(&0x06054B50u32.to_le_bytes());
    bytes.extend_from_slice(&0u16.to_le_bytes());
    bytes.extend_from_slice(&0u16.to_le_bytes());
    bytes.extend_from_slice(&0xFFFFu16.to_le_bytes());
    bytes.extend_from_slice(&0xFFFFu16.to_le_bytes());
    bytes.extend_from_slice(&0xFFFF_FFFFu32.to_le_bytes());
    bytes.extend_from_slice(&0xFFFF_FFFFu32.to_le_bytes());
    bytes.extend_from_slice(&(comment.len() as u16).to_le_bytes());
    bytes.extend_from_slice(comment.as_bytes());

    fs::write(&path, &bytes).unwrap();

    let mut zip_file = ZipFile::new();
    assert_eq!(zip_file.zip_file_open(&path, 0, true), ZipReturn::ZipGood);
    assert_eq!(zip_file.zip_struct(), ZipStructure::ZipTrrnt);
    zip_file.zip_file_close();

    let _ = fs::remove_file(path);
}

#[test]
fn test_zip_file_detects_torrentzip_with_zip64_eocd_present_but_not_required() {
    let path = unique_temp_zip("compress_zip_struct_zip64_present");

    let filename = b"a.txt";
    let flags: u16 = 2;
    let method: u16 = 8;
    let dos_time: u16 = ZipFile::TORRENTZIP_DOS_TIME;
    let dos_date: u16 = ZipFile::TORRENTZIP_DOS_DATE;

    let mut bytes = Vec::new();

    bytes.extend_from_slice(&0x04034B50u32.to_le_bytes());
    bytes.extend_from_slice(&20u16.to_le_bytes());
    bytes.extend_from_slice(&flags.to_le_bytes());
    bytes.extend_from_slice(&method.to_le_bytes());
    bytes.extend_from_slice(&dos_time.to_le_bytes());
    bytes.extend_from_slice(&dos_date.to_le_bytes());
    bytes.extend_from_slice(&0u32.to_le_bytes());
    bytes.extend_from_slice(&2u32.to_le_bytes());
    bytes.extend_from_slice(&0u32.to_le_bytes());
    bytes.extend_from_slice(&(filename.len() as u16).to_le_bytes());
    bytes.extend_from_slice(&0u16.to_le_bytes());
    bytes.extend_from_slice(filename);
    bytes.extend_from_slice(&[0x03, 0x00]);

    let central_directory_offset = bytes.len() as u64;

    bytes.extend_from_slice(&0x02014B50u32.to_le_bytes());
    bytes.extend_from_slice(&0u16.to_le_bytes());
    bytes.extend_from_slice(&20u16.to_le_bytes());
    bytes.extend_from_slice(&flags.to_le_bytes());
    bytes.extend_from_slice(&method.to_le_bytes());
    bytes.extend_from_slice(&dos_time.to_le_bytes());
    bytes.extend_from_slice(&dos_date.to_le_bytes());
    bytes.extend_from_slice(&0u32.to_le_bytes());
    bytes.extend_from_slice(&2u32.to_le_bytes());
    bytes.extend_from_slice(&0u32.to_le_bytes());
    bytes.extend_from_slice(&(filename.len() as u16).to_le_bytes());
    bytes.extend_from_slice(&0u16.to_le_bytes());
    bytes.extend_from_slice(&0u16.to_le_bytes());
    bytes.extend_from_slice(&0u16.to_le_bytes());
    bytes.extend_from_slice(&0u16.to_le_bytes());
    bytes.extend_from_slice(&0u32.to_le_bytes());
    bytes.extend_from_slice(&0u32.to_le_bytes());
    bytes.extend_from_slice(filename);

    let central_directory_size = (bytes.len() as u64) - central_directory_offset;

    let mut crc = Crc32Hasher::new();
    crc.update(
        &bytes[central_directory_offset as usize
            ..(central_directory_offset + central_directory_size) as usize],
    );
    let comment = format!("TORRENTZIPPED-{:08X}", crc.finalize());

    let zip64_eocd_offset = bytes.len() as u64;
    bytes.extend_from_slice(&0x06064B50u32.to_le_bytes());
    bytes.extend_from_slice(&44u64.to_le_bytes());
    bytes.extend_from_slice(&45u16.to_le_bytes());
    bytes.extend_from_slice(&45u16.to_le_bytes());
    bytes.extend_from_slice(&0u32.to_le_bytes());
    bytes.extend_from_slice(&0u32.to_le_bytes());
    bytes.extend_from_slice(&1u64.to_le_bytes());
    bytes.extend_from_slice(&1u64.to_le_bytes());
    bytes.extend_from_slice(&central_directory_size.to_le_bytes());
    bytes.extend_from_slice(&central_directory_offset.to_le_bytes());

    bytes.extend_from_slice(&0x07064B50u32.to_le_bytes());
    bytes.extend_from_slice(&0u32.to_le_bytes());
    bytes.extend_from_slice(&zip64_eocd_offset.to_le_bytes());
    bytes.extend_from_slice(&1u32.to_le_bytes());

    bytes.extend_from_slice(&0x06054B50u32.to_le_bytes());
    bytes.extend_from_slice(&0u16.to_le_bytes());
    bytes.extend_from_slice(&0u16.to_le_bytes());
    bytes.extend_from_slice(&1u16.to_le_bytes());
    bytes.extend_from_slice(&1u16.to_le_bytes());
    bytes.extend_from_slice(&(central_directory_size as u32).to_le_bytes());
    bytes.extend_from_slice(&(central_directory_offset as u32).to_le_bytes());
    bytes.extend_from_slice(&(comment.len() as u16).to_le_bytes());
    bytes.extend_from_slice(comment.as_bytes());

    fs::write(&path, &bytes).unwrap();

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

#[test]
fn test_zip_file_add_fake_matches_reference_header_fields_for_ascii_and_utf8_and_zstd() {
    let mut zip = ZipFile::new();
    zip.zip_create_fake();
    assert_eq!(zip.zip_file_fake_open_memory_stream(), ZipReturn::ZipGood);

    let ascii = zip
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
    assert_eq!(&ascii[0..4], &[0x50, 0x4B, 0x03, 0x04]);
    assert_eq!(u16::from_le_bytes([ascii[4], ascii[5]]), 20);
    assert_eq!(u16::from_le_bytes([ascii[6], ascii[7]]), 2);
    assert_eq!(u16::from_le_bytes([ascii[8], ascii[9]]), 0);

    let utf8_name = "pi_你.bin";
    let utf8 = zip
        .zip_file_add_fake(
            utf8_name,
            0,
            5,
            5,
            &[0x36, 0x10, 0xA6, 0x86],
            0,
            19961224233200,
        )
        .unwrap();
    assert_eq!(u16::from_le_bytes([utf8[4], utf8[5]]), 20);
    assert_eq!(u16::from_le_bytes([utf8[6], utf8[7]]) & (1 << 11), 1 << 11);
    let utf8_name_len = u16::from_le_bytes([utf8[26], utf8[27]]) as usize;
    assert_eq!(&utf8[30..30 + utf8_name_len], utf8_name.as_bytes());

    let zstd = zip
        .zip_file_add_fake(
            "zstd.bin",
            0,
            5,
            5,
            &[0x36, 0x10, 0xA6, 0x86],
            93,
            19961224233200,
        )
        .unwrap();
    assert_eq!(u16::from_le_bytes([zstd[4], zstd[5]]), 63);
    assert_eq!(u16::from_le_bytes([zstd[8], zstd[9]]), 93);

    let bytes = zip.zip_file_fake_close_memory_stream().unwrap();
    assert!(!bytes.is_empty());
}

#[test]
fn test_zip_member_listing_preserves_archive_order() {
    let path = unique_temp_zip("compress_zip_order");
    {
        let file = File::create(&path).unwrap();
        let mut writer = ZipWriter::new(file);
        let opt = FileOptions::<()>::default().compression_method(CompressionMethod::Stored);
        writer.start_file("b.txt", opt).unwrap();
        writer.write_all(b"b").unwrap();
        writer.start_file("a.txt", opt).unwrap();
        writer.write_all(b"a").unwrap();
        writer.finish().unwrap();
    }

    let mut zip_file = ZipFile::new();
    assert_eq!(zip_file.zip_file_open(&path, 0, true), ZipReturn::ZipGood);
    assert_eq!(zip_file.local_files_count(), 2);
    assert_eq!(zip_file.get_file_header(0).unwrap().filename, "b.txt");
    assert_eq!(zip_file.get_file_header(1).unwrap().filename, "a.txt");
    zip_file.zip_file_close();
    let _ = fs::remove_file(path);
}

#[test]
fn test_zip_file_raw_write_stream_preserves_compressed_bytes() {
    let path = unique_temp_zip("compress_zip_raw_write");
    let filename = "hello.txt";
    let uncompressed = b"hello";
    let compressed = deflate_raw_best(uncompressed).unwrap();

    let mut hasher = Crc32Hasher::new();
    hasher.update(uncompressed);
    let crc_be = hasher.finalize().to_be_bytes();

    let mut zip_file = ZipFile::new();
    assert_eq!(zip_file.zip_file_create(&path), ZipReturn::ZipGood);
    let mut stream = zip_file
        .zip_file_open_write_stream(
            true,
            filename,
            uncompressed.len() as u64,
            8,
            Some(19961224233200),
        )
        .unwrap();
    stream.write_all(&compressed).unwrap();
    drop(stream);
    assert_eq!(
        zip_file.zip_file_close_write_stream(&crc_be),
        ZipReturn::ZipGood
    );
    zip_file.zip_file_close();

    let bytes = fs::read(&path).unwrap();
    assert!(bytes.starts_with(&[0x50, 0x4B, 0x03, 0x04]));
    let name_len = u16::from_le_bytes([bytes[26], bytes[27]]) as usize;
    let extra_len = u16::from_le_bytes([bytes[28], bytes[29]]) as usize;
    let data_offset = 30 + name_len + extra_len;
    let comp_size = u32::from_le_bytes([bytes[18], bytes[19], bytes[20], bytes[21]]) as usize;
    let stored = &bytes[data_offset..data_offset + comp_size];
    assert_eq!(stored, compressed.as_slice());

    let mut reopened = ZipFile::new();
    assert_eq!(reopened.zip_file_open(&path, 0, true), ZipReturn::ZipGood);
    let (mut reader, size) = reopened.zip_file_open_read_stream(0).unwrap();
    let mut out = Vec::new();
    reader.read_to_end(&mut out).unwrap();
    assert_eq!(size, 5);
    assert_eq!(out, uncompressed);
    reopened.zip_file_close();

    let _ = fs::remove_file(path);
}

#[test]
fn test_zip_file_nonraw_deflate_empty_file_writes_torrentzip_zero_length_marker() {
    let path = unique_temp_zip("compress_zip_deflate_empty_marker");
    let mut zip_file = ZipFile::new();
    assert_eq!(zip_file.zip_file_create(&path), ZipReturn::ZipGood);

    let stream = zip_file
        .zip_file_open_write_stream(false, "empty.bin", 0, 8, Some(19961224233200))
        .unwrap();
    drop(stream);
    assert_eq!(
        zip_file.zip_file_close_write_stream(&[0, 0, 0, 0]),
        ZipReturn::ZipGood
    );
    zip_file.zip_file_close();

    let bytes = fs::read(&path).unwrap();
    assert!(bytes.starts_with(&[0x50, 0x4B, 0x03, 0x04]));
    let name_len = u16::from_le_bytes([bytes[26], bytes[27]]) as usize;
    let extra_len = u16::from_le_bytes([bytes[28], bytes[29]]) as usize;
    let data_offset = 30 + name_len + extra_len;
    let comp_size = u32::from_le_bytes([bytes[18], bytes[19], bytes[20], bytes[21]]) as usize;
    let data = &bytes[data_offset..data_offset + comp_size];
    assert_eq!(data, &[0x03, 0x00]);

    let _ = fs::remove_file(path);
}

#[test]
fn test_zip_file_nonraw_deflate_empty_directory_writes_torrentzip_zero_length_marker() {
    let path = unique_temp_zip("compress_zip_deflate_empty_dir_marker");
    let mut zip_file = ZipFile::new();
    assert_eq!(zip_file.zip_file_create(&path), ZipReturn::ZipGood);

    let stream = zip_file
        .zip_file_open_write_stream(false, "dir/", 0, 8, Some(19961224233200))
        .unwrap();
    drop(stream);
    assert_eq!(
        zip_file.zip_file_close_write_stream(&[0, 0, 0, 0]),
        ZipReturn::ZipGood
    );
    zip_file.zip_file_close();

    let bytes = fs::read(&path).unwrap();
    assert!(bytes.starts_with(&[0x50, 0x4B, 0x03, 0x04]));
    let name_len = u16::from_le_bytes([bytes[26], bytes[27]]) as usize;
    let extra_len = u16::from_le_bytes([bytes[28], bytes[29]]) as usize;
    let data_offset = 30 + name_len + extra_len;
    let comp_size = u32::from_le_bytes([bytes[18], bytes[19], bytes[20], bytes[21]]) as usize;
    let data = &bytes[data_offset..data_offset + comp_size];
    assert_eq!(data, &[0x03, 0x00]);

    let _ = fs::remove_file(path);
}

#[test]
fn test_zip_file_raw_write_stream_requires_crc() {
    let path = unique_temp_zip("compress_zip_raw_requires_crc");
    let mut zip_file = ZipFile::new();
    assert_eq!(zip_file.zip_file_create(&path), ZipReturn::ZipGood);

    let mut stream = zip_file
        .zip_file_open_write_stream(true, "a.bin", 1, 8, Some(19961224233200))
        .unwrap();
    stream.write_all(&[0x03, 0x00]).unwrap();
    drop(stream);

    assert_eq!(
        zip_file.zip_file_close_write_stream(&[]),
        ZipReturn::ZipErrorWritingToOutputStream
    );
    zip_file.zip_file_close_failed();

    let _ = fs::remove_file(path);
}

#[test]
fn test_zip_file_raw_write_stream_requires_nonzero_compressed_bytes_for_nonzero_uncompressed() {
    let path = unique_temp_zip("compress_zip_raw_requires_data");
    let mut zip_file = ZipFile::new();
    assert_eq!(zip_file.zip_file_create(&path), ZipReturn::ZipGood);

    let stream = zip_file
        .zip_file_open_write_stream(true, "a.bin", 1, 8, Some(19961224233200))
        .unwrap();
    drop(stream);

    assert_eq!(
        zip_file.zip_file_close_write_stream(&[0, 0, 0, 0]),
        ZipReturn::ZipErrorWritingToOutputStream
    );
    zip_file.zip_file_close_failed();

    let _ = fs::remove_file(path);
}

#[test]
fn test_zip_file_roll_back_removes_last_written_entry() {
    let path = unique_temp_zip("compress_zip_rollback");
    let mut zip_file = ZipFile::new();
    assert_eq!(zip_file.zip_file_create(&path), ZipReturn::ZipGood);

    let mut stream = zip_file
        .zip_file_open_write_stream(false, "a.bin", 1, 0, Some(0))
        .unwrap();
    stream.write_all(&[0xAA]).unwrap();
    drop(stream);
    assert_eq!(
        zip_file.zip_file_close_write_stream(&[0, 0, 0, 0]),
        ZipReturn::ZipGood
    );

    let mut stream = zip_file
        .zip_file_open_write_stream(false, "b.bin", 1, 0, Some(0))
        .unwrap();
    stream.write_all(&[0xBB]).unwrap();
    drop(stream);
    assert_eq!(
        zip_file.zip_file_close_write_stream(&[0, 0, 0, 0]),
        ZipReturn::ZipGood
    );

    assert_eq!(zip_file.zip_file_roll_back(), ZipReturn::ZipGood);

    let mut stream = zip_file
        .zip_file_open_write_stream(false, "c.bin", 1, 0, Some(0))
        .unwrap();
    stream.write_all(&[0xCC]).unwrap();
    drop(stream);
    assert_eq!(
        zip_file.zip_file_close_write_stream(&[0, 0, 0, 0]),
        ZipReturn::ZipGood
    );

    zip_file.zip_file_close();

    let mut reopen = ZipFile::new();
    assert_eq!(reopen.zip_file_open(&path, 0, true), ZipReturn::ZipGood);
    assert_eq!(reopen.local_files_count(), 2);
    assert_eq!(reopen.get_file_header(0).unwrap().filename, "a.bin");
    assert_eq!(reopen.get_file_header(1).unwrap().filename, "c.bin");
    reopen.zip_file_close();

    let _ = fs::remove_file(path);
}

#[test]
fn test_manual_zip_writer_zip64_eocd_thresholds_match_reference() {
    assert!(!ManualZipWriter::zip64_eocd_required(0xFFFE, 0, 0));
    assert!(ManualZipWriter::zip64_eocd_required(0xFFFF, 0, 0));

    assert!(!ManualZipWriter::zip64_eocd_required(0, 0xFFFF_FFFE, 0));
    assert!(ManualZipWriter::zip64_eocd_required(0, 0xFFFF_FFFF, 0));

    assert!(!ManualZipWriter::zip64_eocd_required(0, 0, 0xFFFF_FFFE));
    assert!(ManualZipWriter::zip64_eocd_required(0, 0, 0xFFFF_FFFF));

    let mut zip_file = ZipFile::new();
    let path = unique_temp_zip("compress_zip64_threshold");
    assert_eq!(zip_file.zip_file_create(&path), ZipReturn::ZipGood);
    let mut stream = zip_file
        .zip_file_open_write_stream(false, "a.txt", 1, 0, Some(0))
        .unwrap();
    stream.write_all(b"a").unwrap();
    drop(stream);
    assert_eq!(
        zip_file.zip_file_close_write_stream(&[0, 0, 0, 0]),
        ZipReturn::ZipGood
    );
    zip_file.zip_file_close();

    let bytes = fs::read(&path).unwrap();
    let eocd_offset = bytes
        .windows(4)
        .rposition(|window| window == [0x50, 0x4B, 0x05, 0x06])
        .unwrap();
    assert_eq!(
        u16::from_le_bytes([bytes[eocd_offset + 8], bytes[eocd_offset + 9]]),
        1
    );

    let _ = fs::remove_file(path);
}
