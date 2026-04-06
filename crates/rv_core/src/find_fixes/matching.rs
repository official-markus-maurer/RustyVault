impl FindFixes {
    const ZERO_MD5: [u8; 16] = [
        0xD4, 0x1D, 0x8C, 0xD9, 0x8F, 0x00, 0xB2, 0x04, 0xE9, 0x80, 0x09, 0x98, 0xEC, 0xF8,
        0x42, 0x7E,
    ];
    const ZERO_SHA1: [u8; 20] = [
        0xDA, 0x39, 0xA3, 0xEE, 0x5E, 0x6B, 0x4B, 0x0D, 0x32, 0x55, 0xBF, 0xEF, 0x95, 0x60,
        0x18, 0x90, 0xAF, 0xD8, 0x07, 0x09,
    ];
    const ZERO_CRC: [u8; 4] = [0x00, 0x00, 0x00, 0x00];

    fn is_zero_length_file(file: &RvFile) -> bool {
        let mut found_one = false;
        if let Some(md5) = file.md5.as_deref() {
            if md5 != Self::ZERO_MD5 {
                return false;
            }
            found_one = true;
        }
        if let Some(sha1) = file.sha1.as_deref() {
            if sha1 != Self::ZERO_SHA1 {
                return false;
            }
            found_one = true;
        }
        if let Some(crc) = file.crc.as_deref() {
            if crc != Self::ZERO_CRC {
                return false;
            }
            found_one = true;
        }
        if let Some(size) = file.size {
            if size != 0 {
                return false;
            }
            found_one = true;
        }
        if found_one {
            return true;
        }
        file.name.ends_with('/')
    }

    fn sha1_compatible(left: &RvFile, right: &RvFile) -> Option<bool> {
        let lefts = [left.sha1.as_ref(), left.alt_sha1.as_ref()];
        let rights = [right.sha1.as_ref(), right.alt_sha1.as_ref()];
        let has_any = lefts.iter().any(|v| v.is_some()) && rights.iter().any(|v| v.is_some());
        if !has_any {
            return None;
        }
        Some(
            lefts.iter().flatten().any(|l| rights.iter().flatten().any(|r| *l == *r)),
        )
    }

    fn md5_compatible(left: &RvFile, right: &RvFile) -> Option<bool> {
        let lefts = [left.md5.as_ref(), left.alt_md5.as_ref()];
        let rights = [right.md5.as_ref(), right.alt_md5.as_ref()];
        let has_any = lefts.iter().any(|v| v.is_some()) && rights.iter().any(|v| v.is_some());
        if !has_any {
            return None;
        }
        Some(
            lefts.iter().flatten().any(|l| rights.iter().flatten().any(|r| *l == *r)),
        )
    }

    fn got_and_matching_are_full_matches(got: &RvFile, matching: &RvFile) -> bool {
        if Self::is_zero_length_file(got) && Self::is_zero_length_file(matching) {
            return true;
        }
        if got.is_deep_scanned() && matching.is_deep_scanned() {
            if let Some(ok) = Self::sha1_compatible(got, matching) {
                if !ok {
                    return false;
                }
            }
            if let Some(ok) = Self::md5_compatible(got, matching) {
                if !ok {
                    return false;
                }
            }
        }
        true
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
        let alt_crc: Option<[u8; 4]> = file.alt_crc.as_deref().and_then(|b| b.try_into().ok());
        let sha1: Option<[u8; 20]> = file.sha1.as_deref().and_then(|b| b.try_into().ok());
        let alt_sha1: Option<[u8; 20]> = file.alt_sha1.as_deref().and_then(|b| b.try_into().ok());
        let md5: Option<[u8; 16]> = file.md5.as_deref().and_then(|b| b.try_into().ok());
        let alt_md5: Option<[u8; 16]> = file.alt_md5.as_deref().and_then(|b| b.try_into().ok());

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
        if let Some(alt_crc) = alt_crc {
            if let Some(got_list) = crc_map.get(&(alt_size, alt_crc)) {
                extend_unique_epoch(&mut candidates, got_list, &mut seen_epoch, epoch);
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
        if let Some(alt_sha1) = alt_sha1 {
            if let Some(got_list) = sha1_map.get(&(alt_size, alt_sha1)) {
                extend_unique_epoch(&mut candidates, got_list, &mut seen_epoch, epoch);
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
        if let Some(alt_md5) = alt_md5 {
            if let Some(got_list) = md5_map.get(&(alt_size, alt_md5)) {
                extend_unique_epoch(&mut candidates, got_list, &mut seen_epoch, epoch);
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
                && Self::got_and_matching_are_full_matches(file, &candidate)
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
        let mut seen_epoch = vec![0u32; files_missing.len()];
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
        let alt_crc: Option<[u8; 4]> = file.alt_crc.as_deref().and_then(|b| b.try_into().ok());
        let sha1: Option<[u8; 20]> = file.sha1.as_deref().and_then(|b| b.try_into().ok());
        let alt_sha1: Option<[u8; 20]> = file.alt_sha1.as_deref().and_then(|b| b.try_into().ok());
        let md5: Option<[u8; 16]> = file.md5.as_deref().and_then(|b| b.try_into().ok());
        let alt_md5: Option<[u8; 16]> = file.alt_md5.as_deref().and_then(|b| b.try_into().ok());

        if let Some(crc) = crc {
            if let Some(missing_list) = crc_map.get(&(size, crc)) {
                extend_unique_epoch(&mut candidates, missing_list, &mut seen_epoch, epoch);
            }
            if alt_size != size {
                if let Some(missing_list) = crc_map.get(&(alt_size, crc)) {
                    extend_unique_epoch(&mut candidates, missing_list, &mut seen_epoch, epoch);
                }
            }
        }
        if let Some(alt_crc) = alt_crc {
            if let Some(missing_list) = crc_map.get(&(alt_size, alt_crc)) {
                extend_unique_epoch(&mut candidates, missing_list, &mut seen_epoch, epoch);
            }
        }
        epoch = epoch.wrapping_add(1);
        if epoch == 0 {
            seen_epoch.fill(0);
            epoch = 1;
        }
        if let Some(sha1) = sha1 {
            if let Some(missing_list) = sha1_map.get(&(size, sha1)) {
                extend_unique_epoch(&mut candidates, missing_list, &mut seen_epoch, epoch);
            }
            if alt_size != size {
                if let Some(missing_list) = sha1_map.get(&(alt_size, sha1)) {
                    extend_unique_epoch(&mut candidates, missing_list, &mut seen_epoch, epoch);
                }
            }
        }
        if let Some(alt_sha1) = alt_sha1 {
            if let Some(missing_list) = sha1_map.get(&(alt_size, alt_sha1)) {
                extend_unique_epoch(&mut candidates, missing_list, &mut seen_epoch, epoch);
            }
        }
        epoch = epoch.wrapping_add(1);
        if epoch == 0 {
            seen_epoch.fill(0);
            epoch = 1;
        }
        if let Some(md5) = md5 {
            if let Some(missing_list) = md5_map.get(&(size, md5)) {
                extend_unique_epoch(&mut candidates, missing_list, &mut seen_epoch, epoch);
            }
            if alt_size != size {
                if let Some(missing_list) = md5_map.get(&(alt_size, md5)) {
                    extend_unique_epoch(&mut candidates, missing_list, &mut seen_epoch, epoch);
                }
            }
        }
        if let Some(alt_md5) = alt_md5 {
            if let Some(missing_list) = md5_map.get(&(alt_size, alt_md5)) {
                extend_unique_epoch(&mut candidates, missing_list, &mut seen_epoch, epoch);
            }
        }

        candidates.into_iter().any(|idx| {
            let candidate = files_missing[idx].borrow();
            matches!(candidate.rep_status(), RepStatus::CanBeFixed | RepStatus::CanBeFixedMIA)
                && matches!(candidate.dat_status(), DatStatus::InDatCollect | DatStatus::InDatMIA)
                && Self::got_and_matching_are_full_matches(file, &candidate)
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
        candidate_deep_scanned: bool,
    ) -> bool {
        let file_size = file.size.unwrap_or(0);
        let file_alt_size = file.alt_size.unwrap_or(file_size);
        let candidate_alt_size = alt_size.unwrap_or(size);

        let file_has_sha1 = file.sha1.is_some() || file.alt_sha1.is_some();
        let file_has_md5 = file.md5.is_some() || file.alt_md5.is_some();
        let candidate_has_sha1 = sha1.is_some() || alt_sha1.is_some();
        let candidate_has_md5 = md5.is_some() || alt_md5.is_some();

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

        if candidate_deep_scanned && file.is_deep_scanned() && file_has_sha1 && candidate_has_sha1 && !sha1_match {
            return false;
        }
        if candidate_deep_scanned && file.is_deep_scanned() && file_has_md5 && candidate_has_md5 && !md5_match {
            return false;
        }

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
            scanned.deep_scanned,
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

        let left_has_sha1 = left.sha1.is_some() || left.alt_sha1.is_some();
        let left_has_md5 = left.md5.is_some() || left.alt_md5.is_some();
        let right_has_sha1 = right.sha1.is_some() || right.alt_sha1.is_some();
        let right_has_md5 = right.md5.is_some() || right.alt_md5.is_some();

        if left.deep_scanned && right.deep_scanned && left_has_sha1 && right_has_sha1 && !sha1_match {
            return false;
        }
        if left.deep_scanned && right.deep_scanned && left_has_md5 && right_has_md5 && !md5_match {
            return false;
        }

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

    fn missing_can_be_fixed_by_got(missing: &RvFile, got: &RvFile) -> bool {
        use crate::rv_file::FileStatus;
        use dat_reader::enums::HeaderFileType;

        let missing_zero_len = missing.size == Some(0)
            && missing.crc.is_none()
            && missing.sha1.is_none()
            && missing.md5.is_none()
            && missing.alt_crc.is_none()
            && missing.alt_sha1.is_none()
            && missing.alt_md5.is_none();
        if missing_zero_len {
            return got.size == Some(0);
        }

        if missing.header_file_type() != HeaderFileType::NOTHING
            && got.header_file_type() != HeaderFileType::NOTHING
            && missing.header_file_type() != got.header_file_type()
        {
            return false;
        }

        if missing.header_file_type_required()
            && (got.header_file_type() == HeaderFileType::NOTHING
                || !got.file_status_is(FileStatus::HEADER_FILE_TYPE_FROM_HEADER))
        {
            return false;
        }

        let got_has_verified_hashes = got.is_deep_scanned();

        if got_has_verified_hashes {
            if missing.file_status_is(FileStatus::SHA1_FROM_DAT) {
                if let (Some(missing_sha1), Some(got_sha1)) =
                    (missing.sha1.as_ref(), got.sha1.as_ref())
                {
                    if missing_sha1 != got_sha1
                        && got.alt_sha1.as_ref() != Some(missing_sha1)
                    {
                        return false;
                    }
                }
            }
            if missing.file_status_is(FileStatus::ALT_SHA1_FROM_DAT) {
                if let (Some(missing_sha1), Some(got_sha1)) =
                    (missing.alt_sha1.as_ref(), got.sha1.as_ref())
                {
                    if missing_sha1 != got_sha1
                        && got.alt_sha1.as_ref() != Some(missing_sha1)
                    {
                        return false;
                    }
                }
            }

            if missing.file_status_is(FileStatus::MD5_FROM_DAT) {
                if let (Some(missing_md5), Some(got_md5)) = (missing.md5.as_ref(), got.md5.as_ref())
                {
                    if missing_md5 != got_md5
                        && got.alt_md5.as_ref() != Some(missing_md5)
                    {
                        return false;
                    }
                }
            }
            if missing.file_status_is(FileStatus::ALT_MD5_FROM_DAT) {
                if let (Some(missing_md5), Some(got_md5)) =
                    (missing.alt_md5.as_ref(), got.md5.as_ref())
                {
                    if missing_md5 != got_md5
                        && got.alt_md5.as_ref() != Some(missing_md5)
                    {
                        return false;
                    }
                }
            }
        }

        true
    }
}
