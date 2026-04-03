#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ZipExtraFieldInfo {
    pub is_zip64: bool,
    pub uncompressed_size: Option<u64>,
    pub compressed_size: Option<u64>,
    pub local_header_offset: Option<u64>,
    pub modified_time_ticks: Option<i64>,
    pub accessed_time_ticks: Option<i64>,
    pub created_time_ticks: Option<i64>,
    pub extra_data_found: bool,
}

pub fn parse_extra_fields(
    extra: &[u8],
    central_dir: bool,
    header_uncompressed_size: u32,
    header_compressed_size: u32,
    header_local_header_offset: u32,
) -> ZipExtraFieldInfo {
    let mut out = ZipExtraFieldInfo {
        is_zip64: false,
        ..Default::default()
    };

    let mut block_pos = 0usize;
    while block_pos + 4 <= extra.len() {
        let type_id = u16::from_le_bytes([extra[block_pos], extra[block_pos + 1]]);
        block_pos += 2;
        let block_len = u16::from_le_bytes([extra[block_pos], extra[block_pos + 1]]) as usize;
        block_pos += 2;
        if block_pos + block_len > extra.len() {
            break;
        }

        let mut pos = block_pos;
        match type_id {
            0x0001 => {
                out.is_zip64 = true;
                if header_uncompressed_size == 0xFFFF_FFFF && pos + 8 <= block_pos + block_len {
                    out.uncompressed_size =
                        Some(u64::from_le_bytes(extra[pos..pos + 8].try_into().unwrap()));
                    pos += 8;
                }
                if header_compressed_size == 0xFFFF_FFFF && pos + 8 <= block_pos + block_len {
                    out.compressed_size =
                        Some(u64::from_le_bytes(extra[pos..pos + 8].try_into().unwrap()));
                    pos += 8;
                }
                if central_dir
                    && header_local_header_offset == 0xFFFF_FFFF
                    && pos + 8 <= block_pos + block_len
                {
                    out.local_header_offset =
                        Some(u64::from_le_bytes(extra[pos..pos + 8].try_into().unwrap()));
                }
            }
            0x000a => {
                out.extra_data_found = true;
                if pos + 4 > block_pos + block_len {
                    block_pos += block_len;
                    continue;
                }
                pos += 4;
                if pos + 4 > block_pos + block_len {
                    block_pos += block_len;
                    continue;
                }
                let tag1 = i16::from_le_bytes([extra[pos], extra[pos + 1]]) as i32;
                pos += 2;
                let size1 = i16::from_le_bytes([extra[pos], extra[pos + 1]]) as i32;
                pos += 2;
                if tag1 == 0x0001 && size1 == 24 && pos + 24 <= block_pos + block_len {
                    let m = i64::from_le_bytes(extra[pos..pos + 8].try_into().unwrap());
                    pos += 8;
                    let a = i64::from_le_bytes(extra[pos..pos + 8].try_into().unwrap());
                    pos += 8;
                    let c = i64::from_le_bytes(extra[pos..pos + 8].try_into().unwrap());
                    out.modified_time_ticks = Some(utc_ticks_from_ntfs_datetime(m));
                    out.accessed_time_ticks = Some(utc_ticks_from_ntfs_datetime(a));
                    out.created_time_ticks = Some(utc_ticks_from_ntfs_datetime(c));
                }
            }
            0x5455 => {
                out.extra_data_found = true;
                if pos >= block_pos + block_len {
                    block_pos += block_len;
                    continue;
                }
                let flags = extra[pos];
                pos += 1;
                if (flags & 0x01) != 0 && pos + 4 <= block_pos + block_len {
                    let secs = i32::from_le_bytes(extra[pos..pos + 4].try_into().unwrap());
                    pos += 4;
                    out.modified_time_ticks = Some(utc_ticks_from_unix_seconds(secs));
                }
                if !central_dir {
                    if (flags & 0x02) != 0 && pos + 4 <= block_pos + block_len {
                        let secs = i32::from_le_bytes(extra[pos..pos + 4].try_into().unwrap());
                        pos += 4;
                        out.accessed_time_ticks = Some(utc_ticks_from_unix_seconds(secs));
                    }
                    if (flags & 0x04) != 0 && pos + 4 <= block_pos + block_len {
                        let secs = i32::from_le_bytes(extra[pos..pos + 4].try_into().unwrap());
                        out.created_time_ticks = Some(utc_ticks_from_unix_seconds(secs));
                    }
                }
            }
            0x0007 | 0x4453 | 0x4b46 | 0x5855 | 0x7875 => {
                out.extra_data_found = true;
            }
            _ => {}
        }

        block_pos += block_len;
    }

    out
}

pub fn write_zip64_extra(
    uncompressed_size: u64,
    compressed_size: u64,
    relative_offset_of_local_header: u64,
    central_dir: bool,
) -> (Vec<u8>, u32, u32, u32) {
    let mut e_zip64 = Vec::<u8>::new();

    let (header_uncompressed_size, header_compressed_size, header_relative_offset) = if !central_dir {
        if uncompressed_size >= 0xFFFF_FFFF || compressed_size >= 0xFFFF_FFFF {
            e_zip64.extend_from_slice(&uncompressed_size.to_le_bytes());
            e_zip64.extend_from_slice(&compressed_size.to_le_bytes());
            (0xFFFF_FFFF, 0xFFFF_FFFF, 0)
        } else {
            (uncompressed_size as u32, compressed_size as u32, 0)
        }
    } else {
        let header_uncompressed_size = if uncompressed_size >= 0xFFFF_FFFF {
            e_zip64.extend_from_slice(&uncompressed_size.to_le_bytes());
            0xFFFF_FFFF
        } else {
            uncompressed_size as u32
        };

        let header_compressed_size = if compressed_size >= 0xFFFF_FFFF {
            e_zip64.extend_from_slice(&compressed_size.to_le_bytes());
            0xFFFF_FFFF
        } else {
            compressed_size as u32
        };

        let header_relative_offset = if relative_offset_of_local_header >= 0xFFFF_FFFF {
            e_zip64.extend_from_slice(&relative_offset_of_local_header.to_le_bytes());
            0xFFFF_FFFF
        } else {
            relative_offset_of_local_header as u32
        };

        (header_uncompressed_size, header_compressed_size, header_relative_offset)
    };

    if e_zip64.is_empty() {
        return (Vec::new(), header_uncompressed_size, header_compressed_size, header_relative_offset);
    }

    let mut out = Vec::with_capacity(4 + e_zip64.len());
    out.extend_from_slice(&0x0001u16.to_le_bytes());
    out.extend_from_slice(&(e_zip64.len() as u16).to_le_bytes());
    out.extend_from_slice(&e_zip64);

    (out, header_uncompressed_size, header_compressed_size, header_relative_offset)
}

fn utc_ticks_from_ntfs_datetime(ntfs_ticks: i64) -> i64 {
    const FILE_TIME_TO_UTC_TIME: i64 = 504_911_232_000_000_000;
    ntfs_ticks.saturating_add(FILE_TIME_TO_UTC_TIME)
}

fn utc_ticks_from_unix_seconds(secs: i32) -> i64 {
    const EPOCH_TIME_TO_UTC_TIME: i64 = 621_355_968_000_000_000;
    const TICKS_PER_SECOND: i64 = 10_000_000;
    EPOCH_TIME_TO_UTC_TIME.saturating_add((secs as i64).saturating_mul(TICKS_PER_SECOND))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_ut_timestamp_central_dir() {
        let secs: i32 = 1;
        let mut extra = Vec::new();
        extra.extend_from_slice(&0x5455u16.to_le_bytes());
        extra.extend_from_slice(&5u16.to_le_bytes());
        extra.push(0x01);
        extra.extend_from_slice(&secs.to_le_bytes());

        let info = parse_extra_fields(&extra, true, 0, 0, 0);
        assert_eq!(info.modified_time_ticks, Some(621_355_968_010_000_000));
        assert_eq!(info.accessed_time_ticks, None);
        assert_eq!(info.created_time_ticks, None);
    }

    #[test]
    fn parses_zip64_offset_only() {
        let local_offset: u64 = 0x1_0000_0000;
        let mut extra = Vec::new();
        extra.extend_from_slice(&0x0001u16.to_le_bytes());
        extra.extend_from_slice(&8u16.to_le_bytes());
        extra.extend_from_slice(&local_offset.to_le_bytes());

        let info = parse_extra_fields(&extra, true, 0, 0, 0xFFFF_FFFF);
        assert!(info.is_zip64);
        assert_eq!(info.local_header_offset, Some(local_offset));
        assert_eq!(info.uncompressed_size, None);
        assert_eq!(info.compressed_size, None);
    }
}

