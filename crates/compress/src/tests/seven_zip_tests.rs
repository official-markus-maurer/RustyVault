use super::*;
use std::fs::{self, OpenOptions};
use std::io::{Read, Write};
use std::time::{SystemTime, UNIX_EPOCH};

fn unique_temp_7z(name: &str) -> String {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir()
        .join(format!("{}_{}.7z", name, unique))
        .to_string_lossy()
        .to_string()
}

#[test]
fn detects_torrent7z_trailer() {
    let base_path = unique_temp_7z("compress_torrent7z");
    let stage_dir = std::env::temp_dir().join(format!(
        "compress_torrent7z_stage_{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let _ = fs::remove_dir_all(&stage_dir);
    fs::create_dir_all(&stage_dir).unwrap();
    fs::write(stage_dir.join("a.txt"), b"hello").unwrap();

    sevenz_rust::compress_to_path(&stage_dir, &base_path).unwrap();

    const CRC_SZ: usize = 128;
    const T7Z_SIG_SIZE: usize = 34;
    const T7Z_FOOTER_SIZE: usize = T7Z_SIG_SIZE + 4;
    const BUFFER_SIZE: usize = 256 + 8 + T7Z_FOOTER_SIZE;

    let mut original = Vec::new();
    fs::File::open(&base_path).unwrap().read_to_end(&mut original).unwrap();
    let footer_offset = original.len() as u64;

    let sig_header: Vec<u8> =
        b"\xA9\x9F\xD1\x57\x08\xA9\xD7\xEA\x29\x64\xB2\x36\x1B\x83\x52\x33\x01torrent7z_0.9beta"
            .to_vec();
    let mut footer = vec![0u8; T7Z_FOOTER_SIZE];
    footer[4..4 + sig_header.len()].copy_from_slice(&sig_header);

    let mut buffer = vec![0u8; BUFFER_SIZE];
    let first_len = original.len().min(CRC_SZ);
    buffer[..first_len].copy_from_slice(&original[..first_len]);

    let start_last = footer_offset.saturating_sub(CRC_SZ as u64) as usize;
    let last_block = &original[start_last..];
    let last_len = last_block.len().min(CRC_SZ);
    buffer[CRC_SZ..CRC_SZ + last_len].copy_from_slice(&last_block[..last_len]);

    buffer[256..264].copy_from_slice(&footer_offset.to_le_bytes());
    buffer[264..264 + T7Z_FOOTER_SIZE].copy_from_slice(&footer);
    buffer[264..268].fill(0xFF);

    let mut crc = crc32fast::Hasher::new();
    crc.update(&buffer);
    let calc_crc = crc.finalize();
    footer[0..4].copy_from_slice(&calc_crc.to_le_bytes());

    let mut out = OpenOptions::new().append(true).open(&base_path).unwrap();
    out.write_all(&footer).unwrap();
    out.flush().unwrap();
    drop(out);

    let mut sz = SevenZipFile::new();
    assert_eq!(sz.zip_file_open(&base_path, 0, true), ZipReturn::ZipGood);
    assert_eq!(sz.zip_struct(), ZipStructure::SevenZipTrrnt);
    sz.zip_file_close();

    let _ = fs::remove_file(&base_path);
    let _ = fs::remove_dir_all(&stage_dir);
}
