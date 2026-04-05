pub fn apply_romvault7z_marker(path: &Path, zip_struct: ZipStructure) -> std::io::Result<()> {
    let variant = match zip_struct {
        ZipStructure::SevenZipSLZMA => b'1',
        ZipStructure::SevenZipNLZMA => b'2',
        ZipStructure::SevenZipSZSTD => b'3',
        ZipStructure::SevenZipNZSTD => b'4',
        _ => return Ok(()),
    };

    let mut input = File::open(path)?;
    let mut signature = [0u8; 32];
    input.read_exact(&mut signature)?;
    if signature[0..6] != [0x37, 0x7A, 0xBC, 0xAF, 0x27, 0x1C] {
        return Ok(());
    }

    let next_header_offset = u64::from_le_bytes(signature[12..20].try_into().unwrap());
    let next_header_size = u64::from_le_bytes(signature[20..28].try_into().unwrap());
    let next_header_crc = u32::from_le_bytes(signature[28..32].try_into().unwrap());

    let original_header_pos = 32u64.saturating_add(next_header_offset);
    let file_len = input.metadata()?.len();
    if original_header_pos > file_len {
        return Ok(());
    }

    let mut marker = [0u8; 32];
    marker[..11].copy_from_slice(b"RomVault7Z0");
    marker[11] = variant;

    let mut already_has_marker = false;
    if original_header_pos >= 32
        && input
            .seek(SeekFrom::Start(original_header_pos - 32))
            .is_ok()
    {
        let mut existing = [0u8; 32];
        if input.read_exact(&mut existing).is_ok() && existing[..11] == *b"RomVault7Z0" {
            already_has_marker = true;
        }
    }

    if already_has_marker {
        marker[12..16].copy_from_slice(&next_header_crc.to_le_bytes());
        marker[16..24].copy_from_slice(&original_header_pos.to_le_bytes());
        marker[24..32].copy_from_slice(&next_header_size.to_le_bytes());

        let mut io = File::options().write(true).open(path)?;
        io.seek(SeekFrom::Start(original_header_pos - 32))?;
        io.write_all(&marker)?;
        io.flush()?;
        return Ok(());
    }

    let new_next_header_offset = next_header_offset.saturating_add(32);
    let new_header_pos = 32u64.saturating_add(new_next_header_offset);

    marker[12..16].copy_from_slice(&next_header_crc.to_le_bytes());
    marker[16..24].copy_from_slice(&new_header_pos.to_le_bytes());
    marker[24..32].copy_from_slice(&next_header_size.to_le_bytes());

    signature[12..20].copy_from_slice(&new_next_header_offset.to_le_bytes());
    let mut crc = crc32fast::Hasher::new();
    crc.update(&signature[12..32]);
    signature[8..12].copy_from_slice(&crc.finalize().to_le_bytes());

    let tmp_path = path.with_extension(format!(
        "{}.rv7ztmp",
        path.extension().and_then(|e| e.to_str()).unwrap_or("")
    ));
    let mut output = File::create(&tmp_path)?;

    output.write_all(&signature)?;
    input.seek(SeekFrom::Start(32))?;
    let to_copy = original_header_pos.saturating_sub(32);
    std::io::copy(
        &mut std::io::Read::by_ref(&mut input).take(to_copy),
        &mut output,
    )?;
    output.write_all(&marker)?;
    input.seek(SeekFrom::Start(original_header_pos))?;
    std::io::copy(&mut input, &mut output)?;
    output.flush()?;
    drop(output);

    if std::fs::rename(&tmp_path, path).is_err() {
        std::fs::copy(&tmp_path, path)?;
        let _ = std::fs::remove_file(&tmp_path);
    }

    Ok(())
}
