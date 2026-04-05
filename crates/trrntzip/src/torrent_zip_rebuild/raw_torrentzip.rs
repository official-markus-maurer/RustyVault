impl TorrentZipRebuild {
    #[allow(dead_code)]
    fn read_raw_zip_entry(zip_bytes: &[u8], entry_name: &str) -> Option<RawZipEntry> {
        let eocd_offset = zip_bytes
            .windows(4)
            .rposition(|window| window == [0x50, 0x4B, 0x05, 0x06])?;

        if eocd_offset + 22 > zip_bytes.len() {
            return None;
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
            return None;
        }

        let mut central_offset = central_directory_offset;
        while central_offset + 46 <= central_directory_offset + central_directory_size {
            if zip_bytes[central_offset..central_offset + 4] != [0x50, 0x4B, 0x01, 0x02] {
                return None;
            }

            let flags =
                u16::from_le_bytes([zip_bytes[central_offset + 8], zip_bytes[central_offset + 9]]);
            let compression_method = u16::from_le_bytes([
                zip_bytes[central_offset + 10],
                zip_bytes[central_offset + 11],
            ]);
            let crc = u32::from_le_bytes([
                zip_bytes[central_offset + 16],
                zip_bytes[central_offset + 17],
                zip_bytes[central_offset + 18],
                zip_bytes[central_offset + 19],
            ]);
            let compressed_size = u32::from_le_bytes([
                zip_bytes[central_offset + 20],
                zip_bytes[central_offset + 21],
                zip_bytes[central_offset + 22],
                zip_bytes[central_offset + 23],
            ]);
            let uncompressed_size = u32::from_le_bytes([
                zip_bytes[central_offset + 24],
                zip_bytes[central_offset + 25],
                zip_bytes[central_offset + 26],
                zip_bytes[central_offset + 27],
            ]);
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
            let relative_offset = u32::from_le_bytes([
                zip_bytes[central_offset + 42],
                zip_bytes[central_offset + 43],
                zip_bytes[central_offset + 44],
                zip_bytes[central_offset + 45],
            ]);

            let name_start = central_offset + 46;
            let name_end = name_start + file_name_length;
            if name_end > zip_bytes.len() {
                return None;
            }

            let name_bytes = &zip_bytes[name_start..name_end];
            let current_name = if (flags & 0x0800) != 0 {
                std::str::from_utf8(name_bytes).ok()?.to_string()
            } else {
                codepage_437::decode(name_bytes)
            };
            if current_name == entry_name {
                if compression_method != 8 {
                    return None;
                }
                if (flags & 8) == 8 {
                    return None;
                }

                let extra_end = name_end + extra_length;
                if extra_end > zip_bytes.len() {
                    return None;
                }
                let extra_bytes = &zip_bytes[name_end..extra_end];
                let extra_info = zip_extra_field::parse_extra_fields(
                    extra_bytes,
                    true,
                    uncompressed_size,
                    compressed_size,
                    relative_offset,
                );

                let compressed_size_u64 = if compressed_size == 0xFFFF_FFFF {
                    extra_info.compressed_size?
                } else {
                    compressed_size as u64
                };
                let uncompressed_size_u64 = if uncompressed_size == 0xFFFF_FFFF {
                    extra_info.uncompressed_size?
                } else {
                    uncompressed_size as u64
                };
                let relative_offset_u64 = if relative_offset == 0xFFFF_FFFF {
                    extra_info.local_header_offset?
                } else {
                    relative_offset as u64
                };

                let relative_offset_usize = usize::try_from(relative_offset_u64).ok()?;
                if relative_offset_usize + 30 > zip_bytes.len()
                    || zip_bytes[relative_offset_usize..relative_offset_usize + 4]
                        != [0x50, 0x4B, 0x03, 0x04]
                {
                    return None;
                }

                let local_name_length = u16::from_le_bytes([
                    zip_bytes[relative_offset_usize + 26],
                    zip_bytes[relative_offset_usize + 27],
                ]) as usize;
                let local_extra_length = u16::from_le_bytes([
                    zip_bytes[relative_offset_usize + 28],
                    zip_bytes[relative_offset_usize + 29],
                ]) as usize;
                let data_offset =
                    relative_offset_usize + 30 + local_name_length + local_extra_length;
                let data_end = data_offset + usize::try_from(compressed_size_u64).ok()?;

                if data_end > zip_bytes.len() {
                    return None;
                }

                return Some(RawZipEntry {
                    name: entry_name.to_string(),
                    compressed_data: zip_bytes[data_offset..data_end].to_vec(),
                    crc,
                    compressed_size: compressed_size_u64,
                    uncompressed_size: uncompressed_size_u64,
                    flags: 0x0002 | (flags & 0x0800),
                    compression_method: 8,
                    external_attributes: 0,
                });
            }

            central_offset += 46 + file_name_length + extra_length + comment_length;
        }

        None
    }

    fn build_torrentzip_archive(entries: &[RawZipEntry]) -> Vec<u8> {
        let mut archive_bytes = Vec::new();
        let mut central_directory = Vec::new();

        for entry in entries {
            let name_bytes = if (entry.flags & 0x0800) != 0 {
                entry.name.as_bytes().to_vec()
            } else {
                codepage_437::encode(&entry.name).unwrap_or_else(|| entry.name.as_bytes().to_vec())
            };

            let local_offset = archive_bytes.len() as u64;
            let needs_zip64_offset = local_offset > 0xFFFF_FFFF;
            let needs_zip64_comp = entry.compressed_size > 0xFFFF_FFFF;
            let needs_zip64_uncomp = entry.uncompressed_size > 0xFFFF_FFFF;
            let is_zip64 = needs_zip64_offset || needs_zip64_comp || needs_zip64_uncomp;

            let local_version_needed: u16 = if is_zip64 { 45 } else { 20 };
            let local_comp_u32 = if needs_zip64_comp {
                0xFFFF_FFFF
            } else {
                entry.compressed_size as u32
            };
            let local_uncomp_u32 = if needs_zip64_uncomp {
                0xFFFF_FFFF
            } else {
                entry.uncompressed_size as u32
            };

            let local_extra = if needs_zip64_comp || needs_zip64_uncomp {
                let mut payload = Vec::new();
                if needs_zip64_uncomp {
                    payload.extend_from_slice(&entry.uncompressed_size.to_le_bytes());
                }
                if needs_zip64_comp {
                    payload.extend_from_slice(&entry.compressed_size.to_le_bytes());
                }
                let mut e = Vec::new();
                e.extend_from_slice(&0x0001u16.to_le_bytes());
                e.extend_from_slice(&(payload.len() as u16).to_le_bytes());
                e.extend_from_slice(&payload);
                e
            } else {
                Vec::new()
            };

            archive_bytes.extend_from_slice(&0x04034B50u32.to_le_bytes());
            archive_bytes.extend_from_slice(&local_version_needed.to_le_bytes());
            archive_bytes.extend_from_slice(&entry.flags.to_le_bytes());
            archive_bytes.extend_from_slice(&entry.compression_method.to_le_bytes());
            archive_bytes.extend_from_slice(&Self::TORRENTZIP_DOS_TIME.to_le_bytes());
            archive_bytes.extend_from_slice(&Self::TORRENTZIP_DOS_DATE.to_le_bytes());
            archive_bytes.extend_from_slice(&entry.crc.to_le_bytes());
            archive_bytes.extend_from_slice(&local_comp_u32.to_le_bytes());
            archive_bytes.extend_from_slice(&local_uncomp_u32.to_le_bytes());
            archive_bytes.extend_from_slice(&(name_bytes.len() as u16).to_le_bytes());
            archive_bytes.extend_from_slice(&(local_extra.len() as u16).to_le_bytes());
            archive_bytes.extend_from_slice(&name_bytes);
            archive_bytes.extend_from_slice(&local_extra);
            archive_bytes.extend_from_slice(&entry.compressed_data);

            let central_version_needed: u16 = if is_zip64 { 45 } else { 20 };
            let central_comp_u32 = local_comp_u32;
            let central_uncomp_u32 = local_uncomp_u32;
            let central_offset_u32 = if needs_zip64_offset {
                0xFFFF_FFFF
            } else {
                local_offset as u32
            };
            let central_extra = if is_zip64 {
                let mut payload = Vec::new();
                if needs_zip64_uncomp {
                    payload.extend_from_slice(&entry.uncompressed_size.to_le_bytes());
                }
                if needs_zip64_comp {
                    payload.extend_from_slice(&entry.compressed_size.to_le_bytes());
                }
                if needs_zip64_offset {
                    payload.extend_from_slice(&local_offset.to_le_bytes());
                }
                let mut e = Vec::new();
                e.extend_from_slice(&0x0001u16.to_le_bytes());
                e.extend_from_slice(&(payload.len() as u16).to_le_bytes());
                e.extend_from_slice(&payload);
                e
            } else {
                Vec::new()
            };

            central_directory.extend_from_slice(&0x02014B50u32.to_le_bytes());
            central_directory.extend_from_slice(&0u16.to_le_bytes());
            central_directory.extend_from_slice(&central_version_needed.to_le_bytes());
            central_directory.extend_from_slice(&entry.flags.to_le_bytes());
            central_directory.extend_from_slice(&entry.compression_method.to_le_bytes());
            central_directory.extend_from_slice(&Self::TORRENTZIP_DOS_TIME.to_le_bytes());
            central_directory.extend_from_slice(&Self::TORRENTZIP_DOS_DATE.to_le_bytes());
            central_directory.extend_from_slice(&entry.crc.to_le_bytes());
            central_directory.extend_from_slice(&central_comp_u32.to_le_bytes());
            central_directory.extend_from_slice(&central_uncomp_u32.to_le_bytes());
            central_directory.extend_from_slice(&(name_bytes.len() as u16).to_le_bytes());
            central_directory.extend_from_slice(&(central_extra.len() as u16).to_le_bytes());
            central_directory.extend_from_slice(&0u16.to_le_bytes());
            central_directory.extend_from_slice(&0u16.to_le_bytes());
            central_directory.extend_from_slice(&0u16.to_le_bytes());
            central_directory.extend_from_slice(&entry.external_attributes.to_le_bytes());
            central_directory.extend_from_slice(&central_offset_u32.to_le_bytes());
            central_directory.extend_from_slice(&name_bytes);
            central_directory.extend_from_slice(&central_extra);
        }

        let mut comment_crc = Crc32Hasher::new();
        comment_crc.update(&central_directory);
        let comment = format!("TORRENTZIPPED-{:08X}", comment_crc.finalize());

        let central_directory_offset = archive_bytes.len() as u64;
        let central_directory_size = central_directory.len() as u64;
        archive_bytes.extend_from_slice(&central_directory);

        let zip64_required = entries.len() >= 0xFFFF
            || central_directory_size >= 0xFFFF_FFFF
            || central_directory_offset >= 0xFFFF_FFFF;

        if zip64_required {
            let zip64_eocd_offset = archive_bytes.len() as u64;
            archive_bytes.extend_from_slice(&0x06064B50u32.to_le_bytes());
            archive_bytes.extend_from_slice(&44u64.to_le_bytes());
            archive_bytes.extend_from_slice(&45u16.to_le_bytes());
            archive_bytes.extend_from_slice(&45u16.to_le_bytes());
            archive_bytes.extend_from_slice(&0u32.to_le_bytes());
            archive_bytes.extend_from_slice(&0u32.to_le_bytes());
            archive_bytes.extend_from_slice(&(entries.len() as u64).to_le_bytes());
            archive_bytes.extend_from_slice(&(entries.len() as u64).to_le_bytes());
            archive_bytes.extend_from_slice(&central_directory_size.to_le_bytes());
            archive_bytes.extend_from_slice(&central_directory_offset.to_le_bytes());

            archive_bytes.extend_from_slice(&0x07064B50u32.to_le_bytes());
            archive_bytes.extend_from_slice(&0u32.to_le_bytes());
            archive_bytes.extend_from_slice(&zip64_eocd_offset.to_le_bytes());
            archive_bytes.extend_from_slice(&1u32.to_le_bytes());
        }

        let entries_u16 = if entries.len() >= 0xFFFF {
            0xFFFF
        } else {
            entries.len() as u16
        };
        let cd_size_u32 = if central_directory_size >= 0xFFFF_FFFF {
            0xFFFF_FFFF
        } else {
            central_directory_size as u32
        };
        let cd_offset_u32 = if central_directory_offset >= 0xFFFF_FFFF {
            0xFFFF_FFFF
        } else {
            central_directory_offset as u32
        };

        archive_bytes.extend_from_slice(&0x06054B50u32.to_le_bytes());
        archive_bytes.extend_from_slice(&0u16.to_le_bytes());
        archive_bytes.extend_from_slice(&0u16.to_le_bytes());
        archive_bytes.extend_from_slice(&entries_u16.to_le_bytes());
        archive_bytes.extend_from_slice(&entries_u16.to_le_bytes());
        archive_bytes.extend_from_slice(&cd_size_u32.to_le_bytes());
        archive_bytes.extend_from_slice(&cd_offset_u32.to_le_bytes());
        archive_bytes.extend_from_slice(&(comment.len() as u16).to_le_bytes());
        archive_bytes.extend_from_slice(comment.as_bytes());
        archive_bytes
    }
}
