impl Fix {
    fn torrentzip_datetime() -> Option<ZipDateTime> {
        ZipDateTime::from_date_and_time(1996, 12, 24, 23, 32, 0).ok()
    }

    fn apply_torrentzip_metadata(zip_path: &str) -> bool {
        let Ok(mut zip_bytes) = fs::read(zip_path) else {
            return false;
        };

        let local_header_signature = [0x50, 0x4B, 0x03, 0x04];
        let central_header_signature = [0x50, 0x4B, 0x01, 0x02];
        let utf8_flag = 0x0800u16;

        let mut local_offset = 0usize;
        while local_offset + 30 <= zip_bytes.len()
            && zip_bytes[local_offset..local_offset + 4] == local_header_signature
        {
            let flags =
                u16::from_le_bytes([zip_bytes[local_offset + 6], zip_bytes[local_offset + 7]]);
            let normalized_flags = 0x0002 | (flags & utf8_flag);

            zip_bytes[local_offset + 4..local_offset + 6].copy_from_slice(&20u16.to_le_bytes());
            zip_bytes[local_offset + 6..local_offset + 8]
                .copy_from_slice(&normalized_flags.to_le_bytes());
            zip_bytes[local_offset + 8..local_offset + 10].copy_from_slice(&8u16.to_le_bytes());
            zip_bytes[local_offset + 10..local_offset + 12]
                .copy_from_slice(&Self::TORRENTZIP_DOS_TIME.to_le_bytes());
            zip_bytes[local_offset + 12..local_offset + 14]
                .copy_from_slice(&Self::TORRENTZIP_DOS_DATE.to_le_bytes());

            let compressed_size = u32::from_le_bytes([
                zip_bytes[local_offset + 18],
                zip_bytes[local_offset + 19],
                zip_bytes[local_offset + 20],
                zip_bytes[local_offset + 21],
            ]) as usize;
            let file_name_length = u16::from_le_bytes([
                zip_bytes[local_offset + 26],
                zip_bytes[local_offset + 27],
            ]) as usize;
            let extra_length = u16::from_le_bytes([
                zip_bytes[local_offset + 28],
                zip_bytes[local_offset + 29],
            ]) as usize;

            local_offset += 30 + file_name_length + extra_length + compressed_size;
        }

        let eocd_signature = [0x50, 0x4B, 0x05, 0x06];
        let Some(eocd_offset) = zip_bytes
            .windows(4)
            .rposition(|window| window == eocd_signature)
        else {
            return false;
        };

        if eocd_offset + 22 > zip_bytes.len() {
            return false;
        }

        let central_directory_size = u32::from_le_bytes([
            zip_bytes[eocd_offset + 12],
            zip_bytes[eocd_offset + 13],
            zip_bytes[eocd_offset + 14],
            zip_bytes[eocd_offset + 15],
        ]) as usize;
        let central_directory_offset = u32::from_le_bytes([
            zip_bytes[eocd_offset + 16],
            zip_bytes[eocd_offset + 17],
            zip_bytes[eocd_offset + 18],
            zip_bytes[eocd_offset + 19],
        ]) as usize;

        if central_directory_offset + central_directory_size > zip_bytes.len() {
            return false;
        }

        let central_directory_end = central_directory_offset + central_directory_size;
        let mut central_offset = central_directory_offset;
        while central_offset + 46 <= central_directory_end
            && zip_bytes[central_offset..central_offset + 4] == central_header_signature
        {
            let flags =
                u16::from_le_bytes([zip_bytes[central_offset + 8], zip_bytes[central_offset + 9]]);
            let normalized_flags = 0x0002 | (flags & utf8_flag);

            zip_bytes[central_offset + 4..central_offset + 6].copy_from_slice(&0u16.to_le_bytes());
            zip_bytes[central_offset + 6..central_offset + 8].copy_from_slice(&20u16.to_le_bytes());
            zip_bytes[central_offset + 8..central_offset + 10]
                .copy_from_slice(&normalized_flags.to_le_bytes());
            zip_bytes[central_offset + 10..central_offset + 12].copy_from_slice(&8u16.to_le_bytes());
            zip_bytes[central_offset + 12..central_offset + 14]
                .copy_from_slice(&Self::TORRENTZIP_DOS_TIME.to_le_bytes());
            zip_bytes[central_offset + 14..central_offset + 16]
                .copy_from_slice(&Self::TORRENTZIP_DOS_DATE.to_le_bytes());
            zip_bytes[central_offset + 34..central_offset + 36].copy_from_slice(&0u16.to_le_bytes());
            zip_bytes[central_offset + 36..central_offset + 38].copy_from_slice(&0u16.to_le_bytes());
            zip_bytes[central_offset + 38..central_offset + 40].copy_from_slice(&0u16.to_le_bytes());
            zip_bytes[central_offset + 40..central_offset + 42].copy_from_slice(&0u16.to_le_bytes());
            zip_bytes[central_offset + 42..central_offset + 46].copy_from_slice(&0u32.to_le_bytes());

            let file_name_length = u16::from_le_bytes([
                zip_bytes[central_offset + 28],
                zip_bytes[central_offset + 29],
            ]) as usize;
            let extra_length = u16::from_le_bytes([
                zip_bytes[central_offset + 30],
                zip_bytes[central_offset + 31],
            ]) as usize;
            let comment_length = u16::from_le_bytes([
                zip_bytes[central_offset + 32],
                zip_bytes[central_offset + 33],
            ]) as usize;

            central_offset += 46 + file_name_length + extra_length + comment_length;
        }

        let mut crc_hasher = crc32fast::Hasher::new();
        crc_hasher.update(
            &zip_bytes[central_directory_offset..central_directory_offset + central_directory_size],
        );
        let comment = format!("TORRENTZIPPED-{:08X}", crc_hasher.finalize());
        let comment_bytes = comment.into_bytes();

        zip_bytes[eocd_offset + 20..eocd_offset + 22]
            .copy_from_slice(&(comment_bytes.len() as u16).to_le_bytes());
        zip_bytes.truncate(eocd_offset + 22);
        zip_bytes.extend_from_slice(&comment_bytes);

        fs::write(zip_path, zip_bytes).is_ok()
    }
}
