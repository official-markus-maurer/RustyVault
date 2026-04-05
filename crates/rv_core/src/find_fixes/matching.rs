impl FindFixes {
    fn extend_unique_got_candidates(
        candidates: &mut Vec<usize>,
        got_list: &[usize],
        seen: &mut HashSet<usize>,
    ) {
        for idx in got_list {
            if seen.insert(*idx) {
                candidates.push(*idx);
            }
        }
    }

    fn has_redundant_romroot_match(
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
        let mut seen = HashSet::new();

        if let Some(crc) = file
            .crc
            .as_deref()
            .and_then(|b| <[u8; 4]>::try_from(b).ok())
        {
            if let Some(got_list) = crc_map.get(&(size, crc)) {
                Self::extend_unique_got_candidates(&mut candidates, got_list, &mut seen);
            }
            if alt_size != size {
                if let Some(got_list) = crc_map.get(&(alt_size, crc)) {
                    Self::extend_unique_got_candidates(&mut candidates, got_list, &mut seen);
                }
            }
        }
        if let Some(alt_crc) = file
            .alt_crc
            .as_deref()
            .and_then(|b| <[u8; 4]>::try_from(b).ok())
        {
            if let Some(got_list) = crc_map.get(&(alt_size, alt_crc)) {
                Self::extend_unique_got_candidates(&mut candidates, got_list, &mut seen);
            }
        }
        if let Some(sha1) = file
            .sha1
            .as_deref()
            .and_then(|b| <[u8; 20]>::try_from(b).ok())
        {
            if let Some(got_list) = sha1_map.get(&(size, sha1)) {
                Self::extend_unique_got_candidates(&mut candidates, got_list, &mut seen);
            }
            if alt_size != size {
                if let Some(got_list) = sha1_map.get(&(alt_size, sha1)) {
                    Self::extend_unique_got_candidates(&mut candidates, got_list, &mut seen);
                }
            }
        }
        if let Some(alt_sha1) = file
            .alt_sha1
            .as_deref()
            .and_then(|b| <[u8; 20]>::try_from(b).ok())
        {
            if let Some(got_list) = sha1_map.get(&(alt_size, alt_sha1)) {
                Self::extend_unique_got_candidates(&mut candidates, got_list, &mut seen);
            }
        }
        if let Some(md5) = file
            .md5
            .as_deref()
            .and_then(|b| <[u8; 16]>::try_from(b).ok())
        {
            if let Some(got_list) = md5_map.get(&(size, md5)) {
                Self::extend_unique_got_candidates(&mut candidates, got_list, &mut seen);
            }
            if alt_size != size {
                if let Some(got_list) = md5_map.get(&(alt_size, md5)) {
                    Self::extend_unique_got_candidates(&mut candidates, got_list, &mut seen);
                }
            }
        }
        if let Some(alt_md5) = file
            .alt_md5
            .as_deref()
            .and_then(|b| <[u8; 16]>::try_from(b).ok())
        {
            if let Some(got_list) = md5_map.get(&(alt_size, alt_md5)) {
                Self::extend_unique_got_candidates(&mut candidates, got_list, &mut seen);
            }
        }

        candidates.into_iter().any(|idx| {
            if Rc::ptr_eq(&files_got[idx], &current) {
                return false;
            }
            let candidate = files_got[idx].borrow();
            candidate.got_status() == GotStatus::Got
                && matches!(
                    candidate.dat_status(),
                    DatStatus::InDatCollect
                        | DatStatus::InDatMIA
                        | DatStatus::InDatMerged
                        | DatStatus::InDatNoDump
                )
        })
    }

    fn has_pending_fix_target_match(
        file: &RvFile,
        files_missing: &[Rc<RefCell<RvFile>>],
        crc_map: &HashMap<(u64, [u8; 4]), Vec<usize>>,
        sha1_map: &HashMap<(u64, [u8; 20]), Vec<usize>>,
        md5_map: &HashMap<(u64, [u8; 16]), Vec<usize>>,
    ) -> bool {
        let size = file.size.unwrap_or(0);
        let alt_size = file.alt_size.unwrap_or(size);
        let mut candidates = Vec::new();
        let mut seen = HashSet::new();

        if let Some(crc) = file
            .crc
            .as_deref()
            .and_then(|b| <[u8; 4]>::try_from(b).ok())
        {
            if let Some(missing_list) = crc_map.get(&(size, crc)) {
                Self::extend_unique_got_candidates(&mut candidates, missing_list, &mut seen);
            }
            if alt_size != size {
                if let Some(missing_list) = crc_map.get(&(alt_size, crc)) {
                    Self::extend_unique_got_candidates(&mut candidates, missing_list, &mut seen);
                }
            }
        }
        if let Some(alt_crc) = file
            .alt_crc
            .as_deref()
            .and_then(|b| <[u8; 4]>::try_from(b).ok())
        {
            if let Some(missing_list) = crc_map.get(&(alt_size, alt_crc)) {
                Self::extend_unique_got_candidates(&mut candidates, missing_list, &mut seen);
            }
        }
        if let Some(sha1) = file
            .sha1
            .as_deref()
            .and_then(|b| <[u8; 20]>::try_from(b).ok())
        {
            if let Some(missing_list) = sha1_map.get(&(size, sha1)) {
                Self::extend_unique_got_candidates(&mut candidates, missing_list, &mut seen);
            }
            if alt_size != size {
                if let Some(missing_list) = sha1_map.get(&(alt_size, sha1)) {
                    Self::extend_unique_got_candidates(&mut candidates, missing_list, &mut seen);
                }
            }
        }
        if let Some(alt_sha1) = file
            .alt_sha1
            .as_deref()
            .and_then(|b| <[u8; 20]>::try_from(b).ok())
        {
            if let Some(missing_list) = sha1_map.get(&(alt_size, alt_sha1)) {
                Self::extend_unique_got_candidates(&mut candidates, missing_list, &mut seen);
            }
        }
        if let Some(md5) = file
            .md5
            .as_deref()
            .and_then(|b| <[u8; 16]>::try_from(b).ok())
        {
            if let Some(missing_list) = md5_map.get(&(size, md5)) {
                Self::extend_unique_got_candidates(&mut candidates, missing_list, &mut seen);
            }
            if alt_size != size {
                if let Some(missing_list) = md5_map.get(&(alt_size, md5)) {
                    Self::extend_unique_got_candidates(&mut candidates, missing_list, &mut seen);
                }
            }
        }
        if let Some(alt_md5) = file
            .alt_md5
            .as_deref()
            .and_then(|b| <[u8; 16]>::try_from(b).ok())
        {
            if let Some(missing_list) = md5_map.get(&(alt_size, alt_md5)) {
                Self::extend_unique_got_candidates(&mut candidates, missing_list, &mut seen);
            }
        }

        candidates.into_iter().any(|idx| {
            let candidate = files_missing[idx].borrow();
            matches!(candidate.rep_status(), RepStatus::CanBeFixed | RepStatus::CanBeFixedMIA)
                && matches!(candidate.dat_status(), DatStatus::InDatCollect | DatStatus::InDatMIA)
        })
    }

    #[allow(clippy::too_many_arguments)]
    fn hashes_match_file(
        file: &RvFile,
        size: u64,
        alt_size: Option<u64>,
        crc: &Option<Vec<u8>>,
        alt_crc: &Option<Vec<u8>>,
        sha1: &Option<Vec<u8>>,
        alt_sha1: &Option<Vec<u8>>,
        md5: &Option<Vec<u8>>,
        alt_md5: &Option<Vec<u8>>,
    ) -> bool {
        let file_size = file.size.unwrap_or(0);
        let file_alt_size = file.alt_size.unwrap_or(file_size);
        let candidate_alt_size = alt_size.unwrap_or(size);

        let crc_match = file
            .crc
            .as_ref()
            .zip(crc.as_ref())
            .is_some_and(|(left, right)| file_size == size && left == right)
            || file
                .crc
                .as_ref()
                .zip(alt_crc.as_ref())
                .is_some_and(|(left, right)| file_size == candidate_alt_size && left == right)
            || file
                .alt_crc
                .as_ref()
                .zip(crc.as_ref())
                .is_some_and(|(left, right)| file_alt_size == size && left == right)
            || file
                .alt_crc
                .as_ref()
                .zip(alt_crc.as_ref())
                .is_some_and(|(left, right)| file_alt_size == candidate_alt_size && left == right);

        let sha1_match = file
            .sha1
            .as_ref()
            .zip(sha1.as_ref())
            .is_some_and(|(left, right)| file_size == size && left == right)
            || file
                .sha1
                .as_ref()
                .zip(alt_sha1.as_ref())
                .is_some_and(|(left, right)| file_size == candidate_alt_size && left == right)
            || file
                .alt_sha1
                .as_ref()
                .zip(sha1.as_ref())
                .is_some_and(|(left, right)| file_alt_size == size && left == right)
            || file
                .alt_sha1
                .as_ref()
                .zip(alt_sha1.as_ref())
                .is_some_and(|(left, right)| file_alt_size == candidate_alt_size && left == right);

        let md5_match = file
            .md5
            .as_ref()
            .zip(md5.as_ref())
            .is_some_and(|(left, right)| file_size == size && left == right)
            || file
                .md5
                .as_ref()
                .zip(alt_md5.as_ref())
                .is_some_and(|(left, right)| file_size == candidate_alt_size && left == right)
            || file
                .alt_md5
                .as_ref()
                .zip(md5.as_ref())
                .is_some_and(|(left, right)| file_alt_size == size && left == right)
            || file
                .alt_md5
                .as_ref()
                .zip(alt_md5.as_ref())
                .is_some_and(|(left, right)| file_alt_size == candidate_alt_size && left == right);

        crc_match || sha1_match || md5_match
    }

    fn scanned_member_matches_target(file: &RvFile, scanned: &crate::scanned_file::ScannedFile) -> bool {
        Self::hashes_match_file(
            file,
            scanned.size.unwrap_or(0),
            scanned.alt_size,
            &scanned.crc,
            &scanned.alt_crc,
            &scanned.sha1,
            &scanned.alt_sha1,
            &scanned.md5,
            &scanned.alt_md5,
        )
    }

    fn scanned_files_match(
        left: &crate::scanned_file::ScannedFile,
        right: &crate::scanned_file::ScannedFile,
    ) -> bool {
        let left_size = left.size.unwrap_or(0);
        let left_alt_size = left.alt_size.unwrap_or(left_size);
        let right_size = right.size.unwrap_or(0);
        let right_alt_size = right.alt_size.unwrap_or(right_size);

        let crc_match = left
            .crc
            .as_ref()
            .zip(right.crc.as_ref())
            .is_some_and(|(a, b)| left_size == right_size && a == b)
            || left
                .crc
                .as_ref()
                .zip(right.alt_crc.as_ref())
                .is_some_and(|(a, b)| left_size == right_alt_size && a == b)
            || left
                .alt_crc
                .as_ref()
                .zip(right.crc.as_ref())
                .is_some_and(|(a, b)| left_alt_size == right_size && a == b)
            || left
                .alt_crc
                .as_ref()
                .zip(right.alt_crc.as_ref())
                .is_some_and(|(a, b)| left_alt_size == right_alt_size && a == b);

        let sha1_match = left
            .sha1
            .as_ref()
            .zip(right.sha1.as_ref())
            .is_some_and(|(a, b)| left_size == right_size && a == b)
            || left
                .sha1
                .as_ref()
                .zip(right.alt_sha1.as_ref())
                .is_some_and(|(a, b)| left_size == right_alt_size && a == b)
            || left
                .alt_sha1
                .as_ref()
                .zip(right.sha1.as_ref())
                .is_some_and(|(a, b)| left_alt_size == right_size && a == b)
            || left
                .alt_sha1
                .as_ref()
                .zip(right.alt_sha1.as_ref())
                .is_some_and(|(a, b)| left_alt_size == right_alt_size && a == b);

        let md5_match = left
            .md5
            .as_ref()
            .zip(right.md5.as_ref())
            .is_some_and(|(a, b)| left_size == right_size && a == b)
            || left
                .md5
                .as_ref()
                .zip(right.alt_md5.as_ref())
                .is_some_and(|(a, b)| left_size == right_alt_size && a == b)
            || left
                .alt_md5
                .as_ref()
                .zip(right.md5.as_ref())
                .is_some_and(|(a, b)| left_alt_size == right_size && a == b)
            || left
                .alt_md5
                .as_ref()
                .zip(right.alt_md5.as_ref())
                .is_some_and(|(a, b)| left_alt_size == right_alt_size && a == b);

        crc_match || sha1_match || md5_match
    }

    fn node_has_comparable_hashes(node: &RvFile) -> bool {
        node.crc.is_some()
            || node.alt_crc.is_some()
            || node.sha1.is_some()
            || node.alt_sha1.is_some()
            || node.md5.is_some()
            || node.alt_md5.is_some()
    }
}
