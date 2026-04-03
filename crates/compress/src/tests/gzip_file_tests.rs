use crate::{GZipFile, ICompress, ZipReturn};
use crc32fast::Hasher as Crc32Hasher;
use std::fs;
use std::io::{Read, Write};
use std::time::{SystemTime, UNIX_EPOCH};

fn unique_temp_gz(name: &str) -> String {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir()
        .join(format!("{}_{}.gz", name, unique))
        .to_string_lossy()
        .to_string()
}

#[test]
fn gzip_write_then_read_headers_and_stream() {
    let path = unique_temp_gz("compress_gzip_roundtrip");
    let data = b"hello gzip";
    let mut crc = Crc32Hasher::new();
    crc.update(data);
    let crc_be = crc.finalize().to_be_bytes();

    let mut gz = GZipFile::new();
    assert_eq!(gz.zip_file_create(&path), ZipReturn::ZipGood);
    {
        let mut w = gz
            .zip_file_open_write_stream(false, "ignored", data.len() as u64, 8, None)
            .unwrap();
        w.write_all(data).unwrap();
    }
    assert_eq!(gz.zip_file_close_write_stream(&crc_be), ZipReturn::ZipGood);
    gz.zip_file_close();

    let mut gz = GZipFile::new();
    assert_eq!(gz.zip_file_open(&path, 0, true), ZipReturn::ZipGood);
    let hdr = gz.get_file_header(0).unwrap();
    assert_eq!(hdr.uncompressed_size, data.len() as u64);
    assert_eq!(hdr.crc.as_ref().unwrap(), &crc_be.to_vec());

    let (mut r, size) = gz.zip_file_open_read_stream(0).unwrap();
    assert_eq!(size, data.len() as u64);
    let mut out = Vec::new();
    r.read_to_end(&mut out).unwrap();
    assert_eq!(out, data);
    let _ = gz.zip_file_close_read_stream();
    gz.zip_file_close();

    let _ = fs::remove_file(&path);
}

