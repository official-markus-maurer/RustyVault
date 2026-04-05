#[derive(Clone, Copy)]
enum RomGridCopyColumn {
    Got,
    Rom,
    Size,
    Crc32,
    Sha1,
    Md5,
    AltSize,
    AltCrc32,
    AltSha1,
    AltMd5,
}

fn clip_hex(bytes: &Option<Vec<u8>>, max_len: usize) -> Option<String> {
    let b = bytes.as_ref()?;
    if b.is_empty() {
        return None;
    }
    let hex = hex::encode(b);
    Some(hex.chars().take(max_len).collect())
}

fn rom_clipboard_text(rom: &RvFile, col: RomGridCopyColumn) -> Option<String> {
    match col {
        RomGridCopyColumn::Rom => {
            if rom.name.is_empty() {
                None
            } else {
                Some(rom.name.clone())
            }
        }
        RomGridCopyColumn::Size => rom.size.map(|s| s.to_string()),
        RomGridCopyColumn::Crc32 => clip_hex(&rom.crc, 8),
        RomGridCopyColumn::Sha1 => clip_hex(&rom.sha1, 40),
        RomGridCopyColumn::Md5 => clip_hex(&rom.md5, 32),
        RomGridCopyColumn::AltSize => rom.alt_size.map(|s| s.to_string()),
        RomGridCopyColumn::AltCrc32 => clip_hex(&rom.alt_crc, 8),
        RomGridCopyColumn::AltSha1 => clip_hex(&rom.alt_sha1, 40),
        RomGridCopyColumn::AltMd5 => clip_hex(&rom.alt_md5, 32),
        RomGridCopyColumn::Got => {
            let name = rom.name.clone();
            let size = rom.size.map(|s| s.to_string()).unwrap_or_default();
            let crc = clip_hex(&rom.crc, 8).unwrap_or_default();
            let sha1 = clip_hex(&rom.sha1, 40).unwrap_or_default();
            let md5 = clip_hex(&rom.md5, 32).unwrap_or_default();

            if name.is_empty() && size.is_empty() && crc.is_empty() && sha1.is_empty() && md5.is_empty() {
                return None;
            }

            let mut out = String::new();
            out.push_str(&format!("Name : {name}\n"));
            out.push_str(&format!("Size : {size}\n"));
            out.push_str(&format!("CRC32: {crc}\n"));
            if !sha1.is_empty() {
                out.push_str(&format!("SHA1 : {sha1}\n"));
            }
            if !md5.is_empty() {
                out.push_str(&format!("MD5  : {md5}\n"));
            }
            Some(out)
        }
    }
}
