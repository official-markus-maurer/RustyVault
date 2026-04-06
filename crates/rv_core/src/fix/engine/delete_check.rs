impl Fix {
    fn identity_can_fix(deleting: &RvFile, candidate: &RvFile) -> bool {
        if candidate.got_status() != GotStatus::Got {
            return false;
        }
        if matches!(candidate.rep_status(), RepStatus::Delete | RepStatus::Deleted) {
            return false;
        }
        let expected_size = deleting.size.or(deleting.alt_size);
        if expected_size.is_some()
            && candidate.size != expected_size
            && candidate.alt_size != expected_size
        {
            return false;
        }

        let mut has_any = false;

        if let Some(crc) = deleting.crc.as_ref().or(deleting.alt_crc.as_ref()) {
            has_any = true;
            if candidate.crc.as_ref() != Some(crc) && candidate.alt_crc.as_ref() != Some(crc) {
                return false;
            }
        }
        if let Some(sha1) = deleting.sha1.as_ref().or(deleting.alt_sha1.as_ref()) {
            has_any = true;
            if candidate.sha1.as_ref() != Some(sha1) && candidate.alt_sha1.as_ref() != Some(sha1) {
                return false;
            }
        }
        if let Some(md5) = deleting.md5.as_ref().or(deleting.alt_md5.as_ref()) {
            has_any = true;
            if candidate.md5.as_ref() != Some(md5) && candidate.alt_md5.as_ref() != Some(md5) {
                return false;
            }
        }

        has_any
    }

    fn find_delete_check_candidate(
        root: Rc<RefCell<RvFile>>,
        deleting: Rc<RefCell<RvFile>>,
    ) -> Option<Rc<RefCell<RvFile>>> {
        let deleting_path = Self::build_physical_path(Rc::clone(&deleting), true);
        let deleting_ref = deleting.borrow();
        let deleting_size = deleting_ref.size;
        drop(deleting_ref);

        let mut stack = vec![root];
        while let Some(node) = stack.pop() {
            let (children, is_dir, got_status, rep_status, ts, ok_identity, parent) = {
                let b = node.borrow();
                (
                    b.children.clone(),
                    b.is_directory(),
                    b.got_status(),
                    b.rep_status(),
                    b.file_mod_time_stamp,
                    Self::identity_can_fix(&deleting.borrow(), &b),
                    b.parent.as_ref().and_then(|w| w.upgrade()),
                )
            };

            if is_dir {
                for c in children {
                    stack.push(c);
                }
                continue;
            }

            if got_status == GotStatus::Got
                && !matches!(rep_status, RepStatus::Delete | RepStatus::Deleted)
                && ok_identity
            {
                if let Some(parent_archive) = parent.as_ref() {
                    let ptype = parent_archive.borrow().file_type;
                    if matches!(
                        ptype,
                        dat_reader::enums::FileType::Zip | dat_reader::enums::FileType::SevenZip
                    ) {
                        let archive_path =
                            Self::build_physical_path(Rc::clone(parent_archive), true);
                        let archive_ts = parent_archive.borrow().file_mod_time_stamp;
                        if !archive_path.exists() {
                            continue;
                        }
                        if !Self::timestamp_matches(&archive_path, archive_ts) {
                            continue;
                        }
                        return Some(node);
                    }
                }

                let cand_path = Self::build_physical_path(Rc::clone(&node), true);
                if Self::physical_path_eq_for_rename(&cand_path, &deleting_path) {
                    continue;
                }
                if deleting_size.is_some() && !cand_path.exists() {
                    continue;
                }
                if !Self::timestamp_matches(&cand_path, ts) {
                    continue;
                }
                return Some(node);
            }
        }
        None
    }
}
