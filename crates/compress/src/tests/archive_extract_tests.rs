use crate::{ArchiveExtract, ICompress, ZipReturn, ZipFile};
use crc32fast::Hasher as Crc32Hasher;
use std::fs;
use std::io::Write;
use std::time::{SystemTime, UNIX_EPOCH};
use zip::write::FileOptions;

fn unique_temp(name: &str, ext: &str) -> String {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir()
        .join(format!("{}_{}.{}", name, unique, ext))
        .to_string_lossy()
        .to_string()
}

#[test]
fn archive_extract_zip_extracts_and_verifies_crc() {
    let zip_path = unique_temp("compress_extract", "zip");
    let out_dir = std::env::temp_dir()
        .join(format!("compress_extract_out_{}", SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos()))
        .to_string_lossy()
        .to_string();

    {
        let file = fs::File::create(&zip_path).unwrap();
        let mut writer = zip::ZipWriter::new(file);
        let opt = FileOptions::<()>::default().compression_method(zip::CompressionMethod::Stored);
        writer.start_file("a.txt", opt).unwrap();
        writer.write_all(b"hello").unwrap();
        writer.finish().unwrap();
    }

    let mut z = ZipFile::new();
    assert_eq!(z.zip_file_open(&zip_path, 0, true), ZipReturn::ZipGood);
    let mut crc = Crc32Hasher::new();
    crc.update(b"hello");
    let crc_be = crc.finalize().to_be_bytes().to_vec();
    assert_eq!(z.get_file_header(0).unwrap().crc.as_ref().unwrap(), &crc_be);
    z.zip_file_close();

    let mut ex = ArchiveExtract::new();
    assert!(ex.full_extract(&zip_path, &out_dir));
    let out_path = std::path::Path::new(&out_dir).join("a.txt");
    assert_eq!(fs::read(&out_path).unwrap(), b"hello");

    let _ = fs::remove_file(&zip_path);
    let _ = fs::remove_dir_all(&out_dir);
}
