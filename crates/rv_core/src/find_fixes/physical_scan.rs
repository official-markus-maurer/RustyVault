impl FindFixes {
    fn scan_physical_node(
        candidate: Rc<RefCell<RvFile>>,
    ) -> Option<crate::scanned_file::ScannedFile> {
        let (candidate_type, candidate_got_status, candidate_name, candidate_has_hashes) = {
            let candidate_ref = candidate.borrow();
            (
                candidate_ref.file_type,
                candidate_ref.got_status(),
                candidate_ref.name_case().to_string(),
                Self::node_has_comparable_hashes(&candidate_ref),
            )
        };

        if candidate_got_status == GotStatus::Got && candidate_has_hashes {
            return None;
        }

        let parent = {
            let candidate_ref = candidate.borrow();
            candidate_ref.parent.as_ref().and_then(|w| w.upgrade())
        };

        match candidate_type {
            FileType::File | FileType::FileOnly => {
                let physical_path = Self::build_physical_path(Rc::clone(&candidate));
                if !physical_path.exists() {
                    return None;
                }
                crate::scanner::Scanner::scan_raw_file(&physical_path.to_string_lossy())
                    .ok()
            }
            FileType::FileZip | FileType::FileSevenZip => {
                let parent = parent?;
                let archive_type = parent.borrow().file_type;
                let archive_path = Self::build_physical_path(Rc::clone(&parent));
                if !archive_path.exists() {
                    return None;
                }
                let time_stamp = fs::metadata(&archive_path)
                    .ok()
                    .and_then(|meta| meta.modified().ok())
                    .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
                    .map(|dur| dur.as_secs() as i64)
                    .unwrap_or_default();
                crate::scanner::Scanner::scan_archive_file(
                    archive_type,
                    &archive_path.to_string_lossy(),
                    time_stamp,
                    true,
                )
                .ok()
                .and_then(|archive| {
                    archive.children.into_iter().find(|member| {
                        Self::physical_identity_eq(&member.name, &candidate_name)
                    })
                })
            }
            FileType::Zip | FileType::SevenZip => {
                let archive_path = Self::build_physical_path(Rc::clone(&candidate));
                if !archive_path.exists() {
                    return None;
                }
                let time_stamp = fs::metadata(&archive_path)
                    .ok()
                    .and_then(|meta| meta.modified().ok())
                    .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
                    .map(|dur| dur.as_secs() as i64)
                    .unwrap_or_default();
                crate::scanner::Scanner::scan_archive_file(
                    candidate_type,
                    &archive_path.to_string_lossy(),
                    time_stamp,
                    true,
                )
                .ok()
            }
            _ => None,
        }
    }

    fn read_physical_candidate_matches(
        current: Rc<RefCell<RvFile>>,
        file: &RvFile,
        candidate: Rc<RefCell<RvFile>>,
    ) -> bool {
        let current_scanned = Self::scan_physical_node(current);
        let candidate_scanned = Self::scan_physical_node(candidate);

        if let (Some(current_scanned), Some(candidate_scanned)) =
            (current_scanned.as_ref(), candidate_scanned.as_ref())
        {
            Self::scanned_files_match(current_scanned, candidate_scanned)
        } else if let Some(candidate_scanned) = candidate_scanned.as_ref() {
            Self::scanned_member_matches_target(file, candidate_scanned)
        } else {
            false
        }
    }

    fn has_redundant_physical_dat_match(
        current: Rc<RefCell<RvFile>>,
        file: &RvFile,
        candidate_files: &[Rc<RefCell<RvFile>>],
    ) -> bool {
        candidate_files.iter().any(|candidate| {
            if Rc::ptr_eq(candidate, &current) {
                return false;
            }
            if !matches!(
                candidate.borrow().dat_status(),
                DatStatus::InDatCollect
                    | DatStatus::InDatMIA
                    | DatStatus::InDatMerged
                    | DatStatus::InDatNoDump
            ) {
                return false;
            }
            Self::read_physical_candidate_matches(
                Rc::clone(&current),
                file,
                Rc::clone(candidate),
            )
        })
    }

    fn has_redundant_physical_collect_match(
        current: Rc<RefCell<RvFile>>,
        file: &RvFile,
        candidate_files: &[Rc<RefCell<RvFile>>],
    ) -> bool {
        candidate_files.iter().any(|candidate| {
            if Rc::ptr_eq(candidate, &current) {
                return false;
            }
            let candidate_ref = candidate.borrow();
            if candidate_ref.dat_status() != DatStatus::InDatCollect
                || candidate_ref.got_status() != GotStatus::Got
            {
                return false;
            }
            drop(candidate_ref);
            Self::read_physical_candidate_matches(
                Rc::clone(&current),
                file,
                Rc::clone(candidate),
            )
        })
    }

    fn has_redundant_collect_hash_match(
        current: Rc<RefCell<RvFile>>,
        file: &RvFile,
        files_got: &[Rc<RefCell<RvFile>>],
        crc_map: &HashMap<(u64, [u8; 4]), Vec<usize>>,
        sha1_map: &HashMap<(u64, [u8; 20]), Vec<usize>>,
        md5_map: &HashMap<(u64, [u8; 16]), Vec<usize>>,
    ) -> bool {
        let size = file.size.unwrap_or(0);
        let alt_size = file.alt_size.unwrap_or(size);
        let mut candidates = Vec::new();
        let mut seen_epoch = vec![0u32; files_got.len()];
        let mut epoch = 1u32;

        fn extend_unique_epoch(
            candidates: &mut Vec<usize>,
            idxs: &[usize],
            seen_epoch: &mut [u32],
            epoch: u32,
        ) {
            for &idx in idxs {
                if idx < seen_epoch.len() && seen_epoch[idx] != epoch {
                    seen_epoch[idx] = epoch;
                    candidates.push(idx);
                }
            }
        }

        let crc: Option<[u8; 4]> = file.crc.as_deref().and_then(|b| b.try_into().ok());
        let sha1: Option<[u8; 20]> = file.sha1.as_deref().and_then(|b| b.try_into().ok());
        let md5: Option<[u8; 16]> = file.md5.as_deref().and_then(|b| b.try_into().ok());

        if let Some(crc) = crc {
            if let Some(got_list) = crc_map.get(&(size, crc)) {
                extend_unique_epoch(&mut candidates, got_list, &mut seen_epoch, epoch);
            }
            if alt_size != size {
                if let Some(got_list) = crc_map.get(&(alt_size, crc)) {
                    extend_unique_epoch(&mut candidates, got_list, &mut seen_epoch, epoch);
                }
            }
        }
        epoch = epoch.wrapping_add(1);
        if epoch == 0 {
            seen_epoch.fill(0);
            epoch = 1;
        }
        if let Some(sha1) = sha1 {
            if let Some(got_list) = sha1_map.get(&(size, sha1)) {
                extend_unique_epoch(&mut candidates, got_list, &mut seen_epoch, epoch);
            }
            if alt_size != size {
                if let Some(got_list) = sha1_map.get(&(alt_size, sha1)) {
                    extend_unique_epoch(&mut candidates, got_list, &mut seen_epoch, epoch);
                }
            }
        }
        epoch = epoch.wrapping_add(1);
        if epoch == 0 {
            seen_epoch.fill(0);
            epoch = 1;
        }
        if let Some(md5) = md5 {
            if let Some(got_list) = md5_map.get(&(size, md5)) {
                extend_unique_epoch(&mut candidates, got_list, &mut seen_epoch, epoch);
            }
            if alt_size != size {
                if let Some(got_list) = md5_map.get(&(alt_size, md5)) {
                    extend_unique_epoch(&mut candidates, got_list, &mut seen_epoch, epoch);
                }
            }
        }

        candidates.into_iter().any(|idx| {
            let candidate = &files_got[idx];
            if Rc::ptr_eq(candidate, &current) {
                return false;
            }
            let candidate_ref = candidate.borrow();
            candidate_ref.dat_status() == DatStatus::InDatCollect
                && candidate_ref.got_status() == GotStatus::Got
        })
    }

    fn hydrate_physical_dat_files(candidate_files: &[Rc<RefCell<RvFile>>]) {
        for candidate in candidate_files {
            let needs_refresh = {
                let candidate_ref = candidate.borrow();
                matches!(
                    candidate_ref.dat_status(),
                    DatStatus::InDatCollect
                        | DatStatus::InDatMIA
                        | DatStatus::InDatMerged
                        | DatStatus::InDatNoDump
                ) && (candidate_ref.got_status() != GotStatus::Got
                    || !Self::node_has_comparable_hashes(&candidate_ref))
            };

            if !needs_refresh {
                continue;
            }

            let Some(scanned) = Self::scan_physical_node(Rc::clone(candidate)) else {
                continue;
            };

            let mut candidate_ref = candidate.borrow_mut();
            candidate_ref.got_status = scanned.got_status;
            candidate_ref.file_mod_time_stamp = scanned.file_mod_time_stamp;
            candidate_ref.size = scanned.size;
            candidate_ref.crc = scanned.crc;
            candidate_ref.sha1 = scanned.sha1;
            candidate_ref.md5 = scanned.md5;
            candidate_ref.alt_size = scanned.alt_size;
            candidate_ref.alt_crc = scanned.alt_crc;
            candidate_ref.alt_sha1 = scanned.alt_sha1;
            candidate_ref.alt_md5 = scanned.alt_md5;
            candidate_ref.header_file_type = scanned.header_file_type;
            candidate_ref.local_header_offset = scanned.local_header_offset;
            candidate_ref.cached_stats = None;
        }
    }
}
