impl Fix {
    fn read_zip_entry_bytes(zip_path: &str, entry_name: &str) -> Option<Vec<u8>> {
        let file = File::open(zip_path).ok()?;
        let mut archive = ZipArchive::new(file).ok()?;
        let mut exact_match = None;
        let mut logical_match = None;

        for index in 0..archive.len() {
            let Ok(entry) = archive.by_index(index) else {
                continue;
            };
            if entry.name() == entry_name {
                exact_match = Some(index);
                break;
            }
            if logical_match.is_none() && Self::logical_name_eq(entry.name(), entry_name) {
                logical_match = Some(index);
            }
        }

        let mut entry = archive.by_index(exact_match.or(logical_match)?).ok()?;
        let mut buffer = Vec::new();
        entry.read_to_end(&mut buffer).ok()?;
        Some(buffer)
    }

    fn read_raw_zip_entry(zip_path: &str, entry_name: &str) -> Option<StoredZipEntry> {
        let zip_bytes = fs::read(zip_path).ok()?;
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
            ]) as usize;

            let name_start = central_offset + 46;
            let name_end = name_start + file_name_length;
            if name_end > zip_bytes.len() {
                return None;
            }

            let current_name = String::from_utf8_lossy(&zip_bytes[name_start..name_end]);
            if Self::logical_name_eq(&current_name, entry_name) {
                if compression_method != 8 {
                    return None;
                }

                if relative_offset + 30 > zip_bytes.len()
                    || zip_bytes[relative_offset..relative_offset + 4] != [0x50, 0x4B, 0x03, 0x04]
                {
                    return None;
                }

                let local_name_length = u16::from_le_bytes([
                    zip_bytes[relative_offset + 26],
                    zip_bytes[relative_offset + 27],
                ]) as usize;
                let local_extra_length = u16::from_le_bytes([
                    zip_bytes[relative_offset + 28],
                    zip_bytes[relative_offset + 29],
                ]) as usize;
                let data_offset = relative_offset + 30 + local_name_length + local_extra_length;
                let data_end = data_offset + compressed_size as usize;

                if data_end > zip_bytes.len() {
                    return None;
                }

                return Some(StoredZipEntry {
                    compressed_data: zip_bytes[data_offset..data_end].to_vec(),
                    crc,
                    compressed_size,
                    uncompressed_size,
                });
            }

            central_offset += 46 + file_name_length + extra_length + comment_length;
        }

        None
    }

    fn find_containing_archive(
        file: Rc<RefCell<RvFile>>,
    ) -> Option<(Rc<RefCell<RvFile>>, String, FileType)> {
        let mut path_parts = Vec::new();
        let mut current = Some(file);

        while let Some(node_rc) = current {
            let node = node_rc.borrow();
            let component = node.name_case().to_string();
            let parent = node.parent.as_ref().and_then(|p| p.upgrade());
            drop(node);

            let parent_rc = parent?;

            let parent_type = parent_rc.borrow().file_type;
            if matches!(parent_type, FileType::Zip | FileType::SevenZip) {
                if !component.is_empty() {
                    path_parts.push(component);
                }
                path_parts.reverse();
                return Some((parent_rc, path_parts.join("/"), parent_type));
            }

            if !component.is_empty() {
                path_parts.push(component);
            }
            current = Some(parent_rc);
        }

        None
    }

    fn read_seven_zip_entry_bytes(archive_path: &str, entry_name: &str) -> Option<Vec<u8>> {
        compress::seven_zip::extract_entry_bytes(archive_path, entry_name)
            .ok()
            .flatten()
    }

    fn read_source_file_bytes(source_file: Rc<RefCell<RvFile>>) -> Option<Vec<u8>> {
        let source_path = Self::get_existing_physical_path(Rc::clone(&source_file));

        if let Some((parent_archive, source_name, parent_type)) =
            Self::find_containing_archive(Rc::clone(&source_file))
        {
            let archive_path = Self::get_existing_physical_path(parent_archive);
            return match parent_type {
                FileType::Zip => Self::read_zip_entry_bytes(&archive_path, &source_name),
                FileType::SevenZip => Self::read_seven_zip_entry_bytes(&archive_path, &source_name),
                _ => fs::read(&source_path).ok(),
            };
        }

        fs::read(&source_path).ok()
    }

    fn queue_source_cleanup(
        source_file: Rc<RefCell<RvFile>>,
        queue: &mut Vec<Rc<RefCell<RvFile>>>,
    ) {
        let parent_archive =
            Self::find_containing_archive(Rc::clone(&source_file)).map(|(archive, _, _)| archive);

        if let Some(parent_archive) = parent_archive {
            source_file.borrow_mut().set_rep_status(RepStatus::Delete);
            if !queue.iter().any(|queued| Rc::ptr_eq(queued, &parent_archive)) {
                queue.push(parent_archive);
            }
        } else {
            source_file.borrow_mut().set_rep_status(RepStatus::Delete);
            if !queue.iter().any(|queued| Rc::ptr_eq(queued, &source_file)) {
                queue.push(source_file);
            }
        }
    }

    fn source_uses_same_archive_path(
        source_file: Rc<RefCell<RvFile>>,
        target_archive_path: &Path,
    ) -> bool {
        Self::find_containing_archive(source_file)
            .map(|(archive, _, _)| {
                let source_archive_path = Self::get_existing_physical_path(archive);
                Self::physical_path_eq_for_rename(Path::new(&source_archive_path), target_archive_path)
            })
            .unwrap_or(false)
    }
}
