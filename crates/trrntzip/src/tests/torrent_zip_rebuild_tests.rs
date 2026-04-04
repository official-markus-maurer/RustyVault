use super::*;
use compress::i_compress::ICompress;
use compress::seven_zip::SevenZipFile;
use compress::zip_file::ZipFile;
use sevenz_rust::compress_to_path as compress_to_7z_path;
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

    let status =
        TorrentZipRebuild::rezip_files(&zipped_files, &mut zip_file, ZipStructure::ZipTrrnt);
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

    let status =
        TorrentZipRebuild::rezip_files(&zipped_files, &mut zip_file, ZipStructure::ZipTrrnt);
    assert_eq!(status, TrrntZipStatus::VALID_TRRNTZIP);

    let rebuilt_bytes_again = fs::read(&source_path).unwrap();
    assert_eq!(rebuilt_bytes, rebuilt_bytes_again);
}

#[test]
fn test_rezip_files_torrentzip_raw_preserves_compressed_streams_from_valid_torrentzip_source() {
    fn deflate_raw_stored(bytes: &[u8]) -> Vec<u8> {
        let len = u16::try_from(bytes.len()).unwrap();
        let nlen = !len;
        let mut out = Vec::with_capacity(5 + bytes.len());
        out.push(0x01);
        out.extend_from_slice(&len.to_le_bytes());
        out.extend_from_slice(&nlen.to_le_bytes());
        out.extend_from_slice(bytes);
        out
    }

    let temp = tempdir().unwrap();
    let source_path = temp.path().join("source.zip");

    let filename = "hello.txt";
    let uncompressed = b"hello";
    let compressed = deflate_raw_stored(uncompressed);

    let mut crc_hasher = crc32fast::Hasher::new();
    crc_hasher.update(uncompressed);
    let crc_be = crc_hasher.finalize().to_be_bytes();

    {
        let mut zip_file = ZipFile::new();
        assert_eq!(
            zip_file.zip_file_create_with_structure(
                &source_path.to_string_lossy(),
                ZipStructure::ZipTrrnt
            ),
            ZipReturn::ZipGood
        );
        let mut stream = zip_file
            .zip_file_open_write_stream(true, filename, uncompressed.len() as u64, 8, None)
            .unwrap();
        stream.write_all(&compressed).unwrap();
        drop(stream);
        assert_eq!(
            zip_file.zip_file_close_write_stream(&crc_be),
            ZipReturn::ZipGood
        );
        zip_file.zip_file_close();
    }

    let before_bytes = fs::read(&source_path).unwrap();
    let before = TorrentZipRebuild::read_raw_zip_entry(&before_bytes, filename).unwrap();
    assert_eq!(before.compressed_data, compressed);

    let mut zip_file = ZipFile::new();
    assert_eq!(
        zip_file.zip_file_open(&source_path.to_string_lossy(), 0, true),
        ZipReturn::ZipGood
    );

    let zipped_files = vec![ZippedFile {
        index: 0,
        name: filename.to_string(),
        size: uncompressed.len() as u64,
        crc: None,
        sha1: None,
        is_dir: false,
    }];

    let status =
        TorrentZipRebuild::rezip_files(&zipped_files, &mut zip_file, ZipStructure::ZipTrrnt);
    assert_eq!(status, TrrntZipStatus::VALID_TRRNTZIP);

    let after_bytes = fs::read(&source_path).unwrap();
    let after = TorrentZipRebuild::read_raw_zip_entry(&after_bytes, filename).unwrap();
    assert_eq!(after.compressed_data, before.compressed_data);
}

#[test]
fn test_rezip_files_zipzstd_raw_preserves_compressed_streams_from_valid_zipzstd_source() {
    let temp = tempdir().unwrap();
    let source_path = temp.path().join("source_zstd.zip");

    let filename = "hello.txt";
    let uncompressed = b"hello";
    let compressed = vec![0xDE, 0xAD, 0xBE, 0xEF];

    let mut crc_hasher = crc32fast::Hasher::new();
    crc_hasher.update(uncompressed);
    let crc_be = crc_hasher.finalize().to_be_bytes();

    {
        let mut zip_file = ZipFile::new();
        assert_eq!(
            zip_file.zip_file_create_with_structure(
                &source_path.to_string_lossy(),
                ZipStructure::ZipZSTD
            ),
            ZipReturn::ZipGood
        );
        let mut stream = zip_file
            .zip_file_open_write_stream(true, filename, uncompressed.len() as u64, 93, None)
            .unwrap();
        stream.write_all(&compressed).unwrap();
        drop(stream);
        assert_eq!(
            zip_file.zip_file_close_write_stream(&crc_be),
            ZipReturn::ZipGood
        );
        zip_file.zip_file_close();
    }

    let mut source_zip = ZipFile::new();
    assert_eq!(
        source_zip.zip_file_open(&source_path.to_string_lossy(), 0, true),
        ZipReturn::ZipGood
    );
    assert_eq!(source_zip.zip_struct(), ZipStructure::ZipZSTD);
    let (mut raw_stream, _, method) = source_zip.zip_file_open_read_stream_ex(0, true).unwrap();
    assert_eq!(method, 93);
    let mut before = Vec::new();
    raw_stream.read_to_end(&mut before).unwrap();
    source_zip.zip_file_close_read_stream();
    source_zip.zip_file_close();
    assert_eq!(before, compressed);

    let mut for_rebuild = ZipFile::new();
    assert_eq!(
        for_rebuild.zip_file_open(&source_path.to_string_lossy(), 0, true),
        ZipReturn::ZipGood
    );
    let zipped_files = vec![ZippedFile {
        index: 0,
        name: filename.to_string(),
        size: uncompressed.len() as u64,
        crc: None,
        sha1: None,
        is_dir: false,
    }];

    let status =
        TorrentZipRebuild::rezip_files(&zipped_files, &mut for_rebuild, ZipStructure::ZipZSTD);
    assert_eq!(status, TrrntZipStatus::VALID_TRRNTZIP);

    let mut rebuilt = ZipFile::new();
    assert_eq!(
        rebuilt.zip_file_open(&source_path.to_string_lossy(), 0, true),
        ZipReturn::ZipGood
    );
    assert_eq!(rebuilt.zip_struct(), ZipStructure::ZipZSTD);
    let (mut raw_stream, _, method) = rebuilt.zip_file_open_read_stream_ex(0, true).unwrap();
    assert_eq!(method, 93);
    let mut after = Vec::new();
    raw_stream.read_to_end(&mut after).unwrap();
    rebuilt.zip_file_close_read_stream();
    rebuilt.zip_file_close();

    assert_eq!(after, before);
}

#[test]
fn test_rezip_files_ziptdc_raw_preserves_empty_file_deflate_stream() {
    let temp = tempdir().unwrap();
    let source_path = temp.path().join("source_tdc.zip");

    let filename = "empty.txt";
    let compressed = vec![0x01, 0x00, 0x00, 0xFF, 0xFF];
    let crc_be = [0u8, 0, 0, 0];

    {
        let mut zip_file = ZipFile::new();
        assert_eq!(
            zip_file.zip_file_create_with_structure(
                &source_path.to_string_lossy(),
                ZipStructure::ZipTDC
            ),
            ZipReturn::ZipGood
        );
        let mut stream = zip_file
            .zip_file_open_write_stream(true, filename, 0, 8, Some(20010101000000))
            .unwrap();
        stream.write_all(&compressed).unwrap();
        drop(stream);
        assert_eq!(
            zip_file.zip_file_close_write_stream(&crc_be),
            ZipReturn::ZipGood
        );
        zip_file.zip_file_close();
    }

    let mut before_zip = ZipFile::new();
    assert_eq!(
        before_zip.zip_file_open(&source_path.to_string_lossy(), 0, true),
        ZipReturn::ZipGood
    );
    assert_eq!(before_zip.zip_struct(), ZipStructure::ZipTDC);
    let (mut raw_stream, _, method) = before_zip.zip_file_open_read_stream_ex(0, true).unwrap();
    assert_eq!(method, 8);
    let mut before = Vec::new();
    raw_stream.read_to_end(&mut before).unwrap();
    before_zip.zip_file_close_read_stream();
    before_zip.zip_file_close();
    assert_eq!(before, compressed);

    let mut for_rebuild = ZipFile::new();
    assert_eq!(
        for_rebuild.zip_file_open(&source_path.to_string_lossy(), 0, true),
        ZipReturn::ZipGood
    );
    let zipped_files = vec![ZippedFile {
        index: 0,
        name: filename.to_string(),
        size: 0,
        crc: None,
        sha1: None,
        is_dir: false,
    }];

    let status =
        TorrentZipRebuild::rezip_files(&zipped_files, &mut for_rebuild, ZipStructure::ZipTDC);
    assert_eq!(status, TrrntZipStatus::VALID_TRRNTZIP);

    let mut rebuilt = ZipFile::new();
    assert_eq!(
        rebuilt.zip_file_open(&source_path.to_string_lossy(), 0, true),
        ZipReturn::ZipGood
    );
    assert_eq!(rebuilt.zip_struct(), ZipStructure::ZipTDC);
    let (mut raw_stream, _, method) = rebuilt.zip_file_open_read_stream_ex(0, true).unwrap();
    assert_eq!(method, 8);
    let mut after = Vec::new();
    raw_stream.read_to_end(&mut after).unwrap();
    rebuilt.zip_file_close_read_stream();
    rebuilt.zip_file_close();

    assert_eq!(after, before);
}

#[test]
fn test_rezip_files_ziptdc_preserves_dos_datetime_fields() {
    fn deflate_raw_stored(bytes: &[u8]) -> Vec<u8> {
        let len = u16::try_from(bytes.len()).unwrap();
        let nlen = !len;
        let mut out = Vec::with_capacity(5 + bytes.len());
        out.push(0x01);
        out.extend_from_slice(&len.to_le_bytes());
        out.extend_from_slice(&nlen.to_le_bytes());
        out.extend_from_slice(bytes);
        out
    }

    fn local_dos_time_date(zip_bytes: &[u8]) -> (u16, u16) {
        assert_eq!(&zip_bytes[0..4], &[0x50, 0x4B, 0x03, 0x04]);
        let time = u16::from_le_bytes([zip_bytes[10], zip_bytes[11]]);
        let date = u16::from_le_bytes([zip_bytes[12], zip_bytes[13]]);
        (time, date)
    }

    fn central_dos_time_date(zip_bytes: &[u8]) -> (u16, u16) {
        let off = zip_bytes
            .windows(4)
            .position(|w| w == [0x50, 0x4B, 0x01, 0x02])
            .unwrap();
        let time = u16::from_le_bytes([zip_bytes[off + 12], zip_bytes[off + 13]]);
        let date = u16::from_le_bytes([zip_bytes[off + 14], zip_bytes[off + 15]]);
        (time, date)
    }

    let temp = tempdir().unwrap();
    let source_path = temp.path().join("source_tdc_datetime.zip");

    let filename = "time.txt";
    let uncompressed = b"abc";
    let compressed = deflate_raw_stored(uncompressed);

    let mut crc_hasher = crc32fast::Hasher::new();
    crc_hasher.update(uncompressed);
    let crc_be = crc_hasher.finalize().to_be_bytes();

    let mod_time = Some(20010101000000);

    {
        let mut zip_file = ZipFile::new();
        assert_eq!(
            zip_file.zip_file_create_with_structure(
                &source_path.to_string_lossy(),
                ZipStructure::ZipTDC
            ),
            ZipReturn::ZipGood
        );
        let mut stream = zip_file
            .zip_file_open_write_stream(true, filename, uncompressed.len() as u64, 8, mod_time)
            .unwrap();
        stream.write_all(&compressed).unwrap();
        drop(stream);
        assert_eq!(
            zip_file.zip_file_close_write_stream(&crc_be),
            ZipReturn::ZipGood
        );
        zip_file.zip_file_close();
    }

    let before = fs::read(&source_path).unwrap();
    let (local_time_before, local_date_before) = local_dos_time_date(&before);
    let (central_time_before, central_date_before) = central_dos_time_date(&before);

    assert_eq!(local_time_before, 0);
    assert_eq!(local_date_before, 10785);
    assert_eq!(central_time_before, 0);
    assert_eq!(central_date_before, 10785);

    let mut for_rebuild = ZipFile::new();
    assert_eq!(
        for_rebuild.zip_file_open(&source_path.to_string_lossy(), 0, true),
        ZipReturn::ZipGood
    );

    let zipped_files = vec![ZippedFile {
        index: 0,
        name: filename.to_string(),
        size: uncompressed.len() as u64,
        crc: None,
        sha1: None,
        is_dir: false,
    }];

    let status =
        TorrentZipRebuild::rezip_files(&zipped_files, &mut for_rebuild, ZipStructure::ZipTDC);
    assert_eq!(status, TrrntZipStatus::VALID_TRRNTZIP);

    let after = fs::read(&source_path).unwrap();
    let (local_time_after, local_date_after) = local_dos_time_date(&after);
    let (central_time_after, central_date_after) = central_dos_time_date(&after);

    assert_eq!(
        (local_time_after, local_date_after),
        (local_time_before, local_date_before)
    );
    assert_eq!(
        (central_time_after, central_date_after),
        (central_time_before, central_date_before)
    );
}

#[test]
fn test_rezip_files_ziptdc_recompress_path_preserves_dos_datetime_fields() {
    fn local_dos_time_date(zip_bytes: &[u8]) -> (u16, u16) {
        assert_eq!(&zip_bytes[0..4], &[0x50, 0x4B, 0x03, 0x04]);
        let time = u16::from_le_bytes([zip_bytes[10], zip_bytes[11]]);
        let date = u16::from_le_bytes([zip_bytes[12], zip_bytes[13]]);
        (time, date)
    }

    fn central_dos_time_date(zip_bytes: &[u8]) -> (u16, u16) {
        let off = zip_bytes
            .windows(4)
            .position(|w| w == [0x50, 0x4B, 0x01, 0x02])
            .unwrap();
        let time = u16::from_le_bytes([zip_bytes[off + 12], zip_bytes[off + 13]]);
        let date = u16::from_le_bytes([zip_bytes[off + 14], zip_bytes[off + 15]]);
        (time, date)
    }

    let temp = tempdir().unwrap();
    let path = temp.path().join("source_tdc_recompress.zip");

    let filename = "time.txt";
    let contents = b"abcdef";
    let mod_time = Some(20010101000000);

    {
        let mut zip_file = ZipFile::new();
        assert_eq!(
            zip_file.zip_file_create(&path.to_string_lossy()),
            ZipReturn::ZipGood
        );
        let mut stream = zip_file
            .zip_file_open_write_stream(false, filename, contents.len() as u64, 8, mod_time)
            .unwrap();
        stream.write_all(contents).unwrap();
        drop(stream);
        let mut crc_hasher = crc32fast::Hasher::new();
        crc_hasher.update(contents);
        let crc_be = crc_hasher.finalize().to_be_bytes();
        assert_eq!(
            zip_file.zip_file_close_write_stream(&crc_be),
            ZipReturn::ZipGood
        );
        zip_file.zip_file_close();
    }

    let before = fs::read(&path).unwrap();
    let (local_time_before, local_date_before) = local_dos_time_date(&before);
    let (central_time_before, central_date_before) = central_dos_time_date(&before);

    assert_eq!(local_time_before, 0);
    assert_eq!(local_date_before, 10785);
    assert_eq!(central_time_before, 0);
    assert_eq!(central_date_before, 10785);

    let mut for_rebuild = ZipFile::new();
    assert_eq!(
        for_rebuild.zip_file_open(&path.to_string_lossy(), 0, true),
        ZipReturn::ZipGood
    );
    assert_eq!(for_rebuild.zip_struct(), ZipStructure::None);

    let zipped_files = vec![ZippedFile {
        index: 0,
        name: filename.to_string(),
        size: contents.len() as u64,
        crc: None,
        sha1: None,
        is_dir: false,
    }];

    let status =
        TorrentZipRebuild::rezip_files(&zipped_files, &mut for_rebuild, ZipStructure::ZipTDC);
    assert_eq!(status, TrrntZipStatus::VALID_TRRNTZIP);

    let after = fs::read(&path).unwrap();
    let (local_time_after, local_date_after) = local_dos_time_date(&after);
    let (central_time_after, central_date_after) = central_dos_time_date(&after);

    assert_eq!(
        (local_time_after, local_date_after),
        (local_time_before, local_date_before)
    );
    assert_eq!(
        (central_time_after, central_date_after),
        (central_time_before, central_date_before)
    );
}

#[test]
fn test_rezip_files_zipzstd_recompress_path_forces_dos_datetime_zero() {
    fn local_dos_time_date(zip_bytes: &[u8]) -> (u16, u16) {
        assert_eq!(&zip_bytes[0..4], &[0x50, 0x4B, 0x03, 0x04]);
        let time = u16::from_le_bytes([zip_bytes[10], zip_bytes[11]]);
        let date = u16::from_le_bytes([zip_bytes[12], zip_bytes[13]]);
        (time, date)
    }

    fn central_dos_time_date(zip_bytes: &[u8]) -> (u16, u16) {
        let off = zip_bytes
            .windows(4)
            .position(|w| w == [0x50, 0x4B, 0x01, 0x02])
            .unwrap();
        let time = u16::from_le_bytes([zip_bytes[off + 12], zip_bytes[off + 13]]);
        let date = u16::from_le_bytes([zip_bytes[off + 14], zip_bytes[off + 15]]);
        (time, date)
    }

    let temp = tempdir().unwrap();
    let path = temp.path().join("source_zstd_recompress.zip");

    let filename = "time.txt";
    let contents = b"abcdef";
    let mod_time = Some(20010101000000);

    {
        let mut zip_file = ZipFile::new();
        assert_eq!(
            zip_file.zip_file_create(&path.to_string_lossy()),
            ZipReturn::ZipGood
        );
        let mut stream = zip_file
            .zip_file_open_write_stream(false, filename, contents.len() as u64, 8, mod_time)
            .unwrap();
        stream.write_all(contents).unwrap();
        drop(stream);
        let mut crc_hasher = crc32fast::Hasher::new();
        crc_hasher.update(contents);
        let crc_be = crc_hasher.finalize().to_be_bytes();
        assert_eq!(
            zip_file.zip_file_close_write_stream(&crc_be),
            ZipReturn::ZipGood
        );
        zip_file.zip_file_close();
    }

    let mut for_rebuild = ZipFile::new();
    assert_eq!(
        for_rebuild.zip_file_open(&path.to_string_lossy(), 0, true),
        ZipReturn::ZipGood
    );
    assert_eq!(for_rebuild.zip_struct(), ZipStructure::None);

    let zipped_files = vec![ZippedFile {
        index: 0,
        name: filename.to_string(),
        size: contents.len() as u64,
        crc: None,
        sha1: None,
        is_dir: false,
    }];

    let status =
        TorrentZipRebuild::rezip_files(&zipped_files, &mut for_rebuild, ZipStructure::ZipZSTD);
    assert_eq!(status, TrrntZipStatus::VALID_TRRNTZIP);

    let after = fs::read(&path).unwrap();
    let (local_time_after, local_date_after) = local_dos_time_date(&after);
    let (central_time_after, central_date_after) = central_dos_time_date(&after);

    assert_eq!(local_time_after, 0);
    assert_eq!(local_date_after, 0);
    assert_eq!(central_time_after, 0);
    assert_eq!(central_date_after, 0);
}

#[test]
fn test_rezip_files_zipzstd_raw_preserves_empty_file_and_forces_dos_datetime_zero() {
    fn local_dos_time_date(zip_bytes: &[u8]) -> (u16, u16) {
        assert_eq!(&zip_bytes[0..4], &[0x50, 0x4B, 0x03, 0x04]);
        let time = u16::from_le_bytes([zip_bytes[10], zip_bytes[11]]);
        let date = u16::from_le_bytes([zip_bytes[12], zip_bytes[13]]);
        (time, date)
    }

    fn central_dos_time_date(zip_bytes: &[u8]) -> (u16, u16) {
        let off = zip_bytes
            .windows(4)
            .position(|w| w == [0x50, 0x4B, 0x01, 0x02])
            .unwrap();
        let time = u16::from_le_bytes([zip_bytes[off + 12], zip_bytes[off + 13]]);
        let date = u16::from_le_bytes([zip_bytes[off + 14], zip_bytes[off + 15]]);
        (time, date)
    }

    let temp = tempdir().unwrap();
    let source_path = temp.path().join("source_zstd_empty.zip");

    let filename = "empty.txt";
    let compressed = vec![0xCA, 0xFE, 0xBA, 0xBE];
    let crc_be = [0u8, 0, 0, 0];

    {
        let mut zip_file = ZipFile::new();
        assert_eq!(
            zip_file.zip_file_create_with_structure(
                &source_path.to_string_lossy(),
                ZipStructure::ZipZSTD
            ),
            ZipReturn::ZipGood
        );
        let mut stream = zip_file
            .zip_file_open_write_stream(true, filename, 0, 93, Some(20010101000000))
            .unwrap();
        stream.write_all(&compressed).unwrap();
        drop(stream);
        assert_eq!(
            zip_file.zip_file_close_write_stream(&crc_be),
            ZipReturn::ZipGood
        );
        zip_file.zip_file_close();
    }

    let mut before_zip = ZipFile::new();
    assert_eq!(
        before_zip.zip_file_open(&source_path.to_string_lossy(), 0, true),
        ZipReturn::ZipGood
    );
    assert_eq!(before_zip.zip_struct(), ZipStructure::ZipZSTD);
    let (mut raw_stream, _, method) = before_zip.zip_file_open_read_stream_ex(0, true).unwrap();
    assert_eq!(method, 93);
    let mut before = Vec::new();
    raw_stream.read_to_end(&mut before).unwrap();
    before_zip.zip_file_close_read_stream();
    before_zip.zip_file_close();
    assert_eq!(before, compressed);

    let mut for_rebuild = ZipFile::new();
    assert_eq!(
        for_rebuild.zip_file_open(&source_path.to_string_lossy(), 0, true),
        ZipReturn::ZipGood
    );
    let zipped_files = vec![ZippedFile {
        index: 0,
        name: filename.to_string(),
        size: 0,
        crc: None,
        sha1: None,
        is_dir: false,
    }];

    let status =
        TorrentZipRebuild::rezip_files(&zipped_files, &mut for_rebuild, ZipStructure::ZipZSTD);
    assert_eq!(status, TrrntZipStatus::VALID_TRRNTZIP);

    let after = fs::read(&source_path).unwrap();
    let (local_time_after, local_date_after) = local_dos_time_date(&after);
    let (central_time_after, central_date_after) = central_dos_time_date(&after);
    assert_eq!(local_time_after, 0);
    assert_eq!(local_date_after, 0);
    assert_eq!(central_time_after, 0);
    assert_eq!(central_date_after, 0);

    let mut rebuilt = ZipFile::new();
    assert_eq!(
        rebuilt.zip_file_open(&source_path.to_string_lossy(), 0, true),
        ZipReturn::ZipGood
    );
    assert_eq!(rebuilt.zip_struct(), ZipStructure::ZipZSTD);
    let (mut raw_stream, _, method) = rebuilt.zip_file_open_read_stream_ex(0, true).unwrap();
    assert_eq!(method, 93);
    let mut after_raw = Vec::new();
    raw_stream.read_to_end(&mut after_raw).unwrap();
    rebuilt.zip_file_close_read_stream();
    rebuilt.zip_file_close();

    assert_eq!(after_raw, before);
}

#[test]
fn test_build_torrentzip_archive_emits_zip64_extra_for_large_sizes_without_zip64_eocd() {
    let entries = vec![RawZipEntry {
        name: "big.bin".to_string(),
        compressed_data: vec![0x00],
        crc: 0,
        compressed_size: 1,
        uncompressed_size: 0x1_0000_0000,
        flags: TorrentZipRebuild::torrentzip_flags("big.bin"),
        compression_method: 8,
        external_attributes: 0,
    }];

    let bytes = TorrentZipRebuild::build_torrentzip_archive(&entries);

    assert_eq!(&bytes[0..4], &[0x50, 0x4B, 0x03, 0x04]);
    assert_eq!(u16::from_le_bytes([bytes[4], bytes[5]]), 45);
    assert_eq!(
        u32::from_le_bytes([bytes[22], bytes[23], bytes[24], bytes[25]]),
        0xFFFF_FFFF
    );

    let name_len = u16::from_le_bytes([bytes[26], bytes[27]]) as usize;
    let extra_len = u16::from_le_bytes([bytes[28], bytes[29]]) as usize;
    assert!(extra_len >= 4 + 8);
    let extra_start = 30 + name_len;
    assert_eq!(
        u16::from_le_bytes([bytes[extra_start], bytes[extra_start + 1]]),
        0x0001
    );

    assert!(bytes.windows(4).any(|w| w == [0x50, 0x4B, 0x01, 0x02]));
    assert!(!bytes.windows(4).any(|w| w == [0x50, 0x4B, 0x06, 0x06]));
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
fn test_apply_structured_zip_metadata_sets_zip64_version_needed_to_extract() {
    fn build_zip64_marker_bytes() -> Vec<u8> {
        let filename = b"a.bin";
        let local_zip64_extra = {
            let mut e = Vec::new();
            e.extend_from_slice(&0x0001u16.to_le_bytes());
            e.extend_from_slice(&16u16.to_le_bytes());
            e.extend_from_slice(&1u64.to_le_bytes());
            e.extend_from_slice(&2u64.to_le_bytes());
            e
        };
        let central_zip64_extra = {
            let mut e = Vec::new();
            e.extend_from_slice(&0x0001u16.to_le_bytes());
            e.extend_from_slice(&24u16.to_le_bytes());
            e.extend_from_slice(&1u64.to_le_bytes());
            e.extend_from_slice(&2u64.to_le_bytes());
            e.extend_from_slice(&0u64.to_le_bytes());
            e
        };

        let mut bytes = Vec::new();
        bytes.extend_from_slice(&0x04034B50u32.to_le_bytes());
        bytes.extend_from_slice(&45u16.to_le_bytes());
        bytes.extend_from_slice(&2u16.to_le_bytes());
        bytes.extend_from_slice(&8u16.to_le_bytes());
        bytes.extend_from_slice(&0u16.to_le_bytes());
        bytes.extend_from_slice(&0u16.to_le_bytes());
        bytes.extend_from_slice(&0u32.to_le_bytes());
        bytes.extend_from_slice(&0xFFFF_FFFFu32.to_le_bytes());
        bytes.extend_from_slice(&0xFFFF_FFFFu32.to_le_bytes());
        bytes.extend_from_slice(&(filename.len() as u16).to_le_bytes());
        bytes.extend_from_slice(&(local_zip64_extra.len() as u16).to_le_bytes());
        bytes.extend_from_slice(filename);
        bytes.extend_from_slice(&local_zip64_extra);
        bytes.extend_from_slice(&[0x03, 0x00]);

        let central_directory_offset = bytes.len();

        bytes.extend_from_slice(&0x02014B50u32.to_le_bytes());
        bytes.extend_from_slice(&0u16.to_le_bytes());
        bytes.extend_from_slice(&45u16.to_le_bytes());
        bytes.extend_from_slice(&2u16.to_le_bytes());
        bytes.extend_from_slice(&8u16.to_le_bytes());
        bytes.extend_from_slice(&0u16.to_le_bytes());
        bytes.extend_from_slice(&0u16.to_le_bytes());
        bytes.extend_from_slice(&0u32.to_le_bytes());
        bytes.extend_from_slice(&0xFFFF_FFFFu32.to_le_bytes());
        bytes.extend_from_slice(&0xFFFF_FFFFu32.to_le_bytes());
        bytes.extend_from_slice(&(filename.len() as u16).to_le_bytes());
        bytes.extend_from_slice(&(central_zip64_extra.len() as u16).to_le_bytes());
        bytes.extend_from_slice(&0u16.to_le_bytes());
        bytes.extend_from_slice(&0u16.to_le_bytes());
        bytes.extend_from_slice(&0u16.to_le_bytes());
        bytes.extend_from_slice(&0u32.to_le_bytes());
        bytes.extend_from_slice(&0xFFFF_FFFFu32.to_le_bytes());
        bytes.extend_from_slice(filename);
        bytes.extend_from_slice(&central_zip64_extra);

        let central_directory_size = bytes.len() - central_directory_offset;

        bytes.extend_from_slice(&0x06054B50u32.to_le_bytes());
        bytes.extend_from_slice(&0u16.to_le_bytes());
        bytes.extend_from_slice(&0u16.to_le_bytes());
        bytes.extend_from_slice(&1u16.to_le_bytes());
        bytes.extend_from_slice(&1u16.to_le_bytes());
        bytes.extend_from_slice(&(central_directory_size as u32).to_le_bytes());
        bytes.extend_from_slice(&(central_directory_offset as u32).to_le_bytes());
        bytes.extend_from_slice(&0u16.to_le_bytes());

        bytes
    }

    let temp = tempdir().unwrap();

    let path_tdc = temp.path().join("zip64_tdc.zip");
    fs::write(&path_tdc, build_zip64_marker_bytes()).unwrap();
    assert!(TorrentZipRebuild::apply_structured_zip_metadata(
        &path_tdc,
        ZipStructure::ZipTDC
    ));
    let patched = fs::read(&path_tdc).unwrap();
    assert_eq!(u16::from_le_bytes([patched[4], patched[5]]), 45);
    let central_offset = patched
        .windows(4)
        .position(|w| w == [0x50, 0x4B, 0x01, 0x02])
        .unwrap();
    assert_eq!(
        u16::from_le_bytes([patched[central_offset + 6], patched[central_offset + 7]]),
        45
    );

    let path_zstd = temp.path().join("zip64_zstd.zip");
    fs::write(&path_zstd, build_zip64_marker_bytes()).unwrap();
    assert!(TorrentZipRebuild::apply_structured_zip_metadata(
        &path_zstd,
        ZipStructure::ZipZSTD
    ));
    let patched = fs::read(&path_zstd).unwrap();
    assert_eq!(u16::from_le_bytes([patched[4], patched[5]]), 63);
    let central_offset = patched
        .windows(4)
        .position(|w| w == [0x50, 0x4B, 0x01, 0x02])
        .unwrap();
    assert_eq!(
        u16::from_le_bytes([patched[central_offset + 6], patched[central_offset + 7]]),
        63
    );
}

#[test]
fn test_apply_structured_zip_metadata_handles_zip64_eocd_when_required_by_sentinel() {
    fn build_zip64_sentinel_torrentzip() -> Vec<u8> {
        let filename = b"a.bin";
        let compressed = [0x03u8, 0x00u8];

        let mut bytes = Vec::new();

        bytes.extend_from_slice(&0x04034B50u32.to_le_bytes());
        bytes.extend_from_slice(&20u16.to_le_bytes());
        bytes.extend_from_slice(&2u16.to_le_bytes());
        bytes.extend_from_slice(&8u16.to_le_bytes());
        bytes.extend_from_slice(&TorrentZipRebuild::TORRENTZIP_DOS_TIME.to_le_bytes());
        bytes.extend_from_slice(&TorrentZipRebuild::TORRENTZIP_DOS_DATE.to_le_bytes());
        bytes.extend_from_slice(&0u32.to_le_bytes());
        bytes.extend_from_slice(&(compressed.len() as u32).to_le_bytes());
        bytes.extend_from_slice(&0u32.to_le_bytes());
        bytes.extend_from_slice(&(filename.len() as u16).to_le_bytes());
        bytes.extend_from_slice(&0u16.to_le_bytes());
        bytes.extend_from_slice(filename);
        bytes.extend_from_slice(&compressed);

        let central_directory_offset = bytes.len() as u64;

        bytes.extend_from_slice(&0x02014B50u32.to_le_bytes());
        bytes.extend_from_slice(&0u16.to_le_bytes());
        bytes.extend_from_slice(&20u16.to_le_bytes());
        bytes.extend_from_slice(&2u16.to_le_bytes());
        bytes.extend_from_slice(&8u16.to_le_bytes());
        bytes.extend_from_slice(&TorrentZipRebuild::TORRENTZIP_DOS_TIME.to_le_bytes());
        bytes.extend_from_slice(&TorrentZipRebuild::TORRENTZIP_DOS_DATE.to_le_bytes());
        bytes.extend_from_slice(&0u32.to_le_bytes());
        bytes.extend_from_slice(&(compressed.len() as u32).to_le_bytes());
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
        bytes.extend_from_slice(&0xFFFFu16.to_le_bytes());
        bytes.extend_from_slice(&0xFFFFu16.to_le_bytes());
        bytes.extend_from_slice(&0xFFFF_FFFFu32.to_le_bytes());
        bytes.extend_from_slice(&0xFFFF_FFFFu32.to_le_bytes());
        bytes.extend_from_slice(&0u16.to_le_bytes());

        bytes
    }

    let temp = tempdir().unwrap();
    let path = temp.path().join("zip64_sentinel.zip");
    fs::write(&path, build_zip64_sentinel_torrentzip()).unwrap();

    assert!(TorrentZipRebuild::apply_structured_zip_metadata(
        &path,
        ZipStructure::ZipTrrnt
    ));

    let mut zip_file = ZipFile::new();
    assert_eq!(
        zip_file.zip_file_open(&path.to_string_lossy(), 0, true),
        ZipReturn::ZipGood
    );
    assert_eq!(zip_file.zip_struct(), ZipStructure::ZipTrrnt);
    zip_file.zip_file_close();
}

#[test]
fn test_apply_structured_zip_metadata_handles_zip64_eocd_when_present_but_not_required() {
    fn build_optional_zip64_eocd_torrentzip() -> Vec<u8> {
        let filename = b"a.bin";
        let compressed = [0x03u8, 0x00u8];

        let mut bytes = Vec::new();

        bytes.extend_from_slice(&0x04034B50u32.to_le_bytes());
        bytes.extend_from_slice(&20u16.to_le_bytes());
        bytes.extend_from_slice(&2u16.to_le_bytes());
        bytes.extend_from_slice(&8u16.to_le_bytes());
        bytes.extend_from_slice(&TorrentZipRebuild::TORRENTZIP_DOS_TIME.to_le_bytes());
        bytes.extend_from_slice(&TorrentZipRebuild::TORRENTZIP_DOS_DATE.to_le_bytes());
        bytes.extend_from_slice(&0u32.to_le_bytes());
        bytes.extend_from_slice(&(compressed.len() as u32).to_le_bytes());
        bytes.extend_from_slice(&0u32.to_le_bytes());
        bytes.extend_from_slice(&(filename.len() as u16).to_le_bytes());
        bytes.extend_from_slice(&0u16.to_le_bytes());
        bytes.extend_from_slice(filename);
        bytes.extend_from_slice(&compressed);

        let central_directory_offset = bytes.len() as u64;

        bytes.extend_from_slice(&0x02014B50u32.to_le_bytes());
        bytes.extend_from_slice(&0u16.to_le_bytes());
        bytes.extend_from_slice(&20u16.to_le_bytes());
        bytes.extend_from_slice(&2u16.to_le_bytes());
        bytes.extend_from_slice(&8u16.to_le_bytes());
        bytes.extend_from_slice(&TorrentZipRebuild::TORRENTZIP_DOS_TIME.to_le_bytes());
        bytes.extend_from_slice(&TorrentZipRebuild::TORRENTZIP_DOS_DATE.to_le_bytes());
        bytes.extend_from_slice(&0u32.to_le_bytes());
        bytes.extend_from_slice(&(compressed.len() as u32).to_le_bytes());
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
        bytes.extend_from_slice(&0u16.to_le_bytes());

        bytes
    }

    let temp = tempdir().unwrap();
    let path = temp.path().join("zip64_present.zip");
    fs::write(&path, build_optional_zip64_eocd_torrentzip()).unwrap();

    assert!(TorrentZipRebuild::apply_structured_zip_metadata(
        &path,
        ZipStructure::ZipTrrnt
    ));

    let mut zip_file = ZipFile::new();
    assert_eq!(
        zip_file.zip_file_open(&path.to_string_lossy(), 0, true),
        ZipReturn::ZipGood
    );
    assert_eq!(zip_file.zip_struct(), ZipStructure::ZipTrrnt);
    zip_file.zip_file_close();
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

    let status =
        TorrentZipRebuild::rezip_files(&zipped_files, &mut zip_file, ZipStructure::ZipTrrnt);
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
        let options = FileOptions::<()>::default().compression_method(CompressionMethod::Stored);
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

    let status =
        TorrentZipRebuild::rezip_files(&zipped_files, &mut zip_file, ZipStructure::ZipTrrnt);
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

    let compression_method =
        u16::from_le_bytes([rebuilt_bytes[rel_offset + 8], rebuilt_bytes[rel_offset + 9]]);
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

    let status =
        TorrentZipRebuild::rezip_files(&zipped_files, &mut zip_file, ZipStructure::ZipTrrnt);
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
        writer
            .add_directory("dir/", FileOptions::<()>::default())
            .unwrap();
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

    let status =
        TorrentZipRebuild::rezip_files(&zipped_files, &mut zip_file, ZipStructure::ZipTrrnt);
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

    let status =
        TorrentZipRebuild::rezip_files(&zipped_files, &mut zip_file, ZipStructure::ZipTrrnt);
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
    let status =
        TorrentZipRebuild::rezip_files(&zipped_files, &mut seven, ZipStructure::SevenZipSLZMA);
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

    let status =
        TorrentZipRebuild::rezip_files(&zipped_files, &mut seven, ZipStructure::SevenZipSLZMA);
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
        writer
            .add_directory("dir/", FileOptions::<()>::default())
            .unwrap();
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
        ZippedFile {
            index: 1,
            name: "dir\\b.bin".to_string(),
            size: 4,
            crc: None,
            sha1: None,
            is_dir: false,
        },
        ZippedFile {
            index: 2,
            name: "a.bin".to_string(),
            size: 4,
            crc: None,
            sha1: None,
            is_dir: false,
        },
        ZippedFile {
            index: 0,
            name: "dir/".to_string(),
            size: 0,
            crc: None,
            sha1: None,
            is_dir: true,
        },
    ];

    let status =
        TorrentZipRebuild::rezip_files(&zipped_files, &mut zip_file, ZipStructure::ZipZSTD);
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
    let comment_len = u16::from_le_bytes([
        rebuilt_bytes[eocd_offset + 20],
        rebuilt_bytes[eocd_offset + 21],
    ]) as usize;
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
        ZippedFile {
            index: 0,
            name: "b.bin".to_string(),
            size: 4,
            crc: None,
            sha1: None,
            is_dir: false,
        },
        ZippedFile {
            index: 1,
            name: "a.bin".to_string(),
            size: 4,
            crc: None,
            sha1: None,
            is_dir: false,
        },
    ];

    let status = TorrentZipRebuild::rezip_files(&zipped_files, &mut zip_file, ZipStructure::ZipTDC);
    assert_eq!(status, TrrntZipStatus::VALID_TRRNTZIP);

    let mut archive = ZipArchive::new(fs::File::open(&source_path).unwrap()).unwrap();
    assert_eq!(archive.len(), 2);
    assert_eq!(archive.by_index(0).unwrap().name(), "a.bin");
    assert_eq!(archive.by_index(1).unwrap().name(), "b.bin");
}
