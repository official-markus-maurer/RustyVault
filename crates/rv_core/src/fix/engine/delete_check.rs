impl Fix {
    fn identity_can_fix(deleting: &RvFile, candidate: &RvFile) -> bool {
        if candidate.got_status() != GotStatus::Got {
            return false;
        }
        if matches!(candidate.rep_status(), RepStatus::Delete | RepStatus::Deleted) {
            return false;
        }
        if deleting.size.is_some() && deleting.size != candidate.size {
            return false;
        }

        let mut has_any = false;

        if let Some(ref crc) = deleting.crc {
            has_any = true;
            if candidate.crc.as_ref() != Some(crc) && candidate.alt_crc.as_ref() != Some(crc) {
                return false;
            }
        }
        if let Some(ref sha1) = deleting.sha1 {
            has_any = true;
            if candidate.sha1.as_ref() != Some(sha1) && candidate.alt_sha1.as_ref() != Some(sha1) {
                return false;
            }
        }
        if let Some(ref md5) = deleting.md5 {
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
        let deleting_ts = deleting_ref.file_mod_time_stamp;
        let deleting_size = deleting_ref.size;
        drop(deleting_ref);

        let mut stack = vec![root];
        while let Some(node) = stack.pop() {
            let (children, is_dir, got_status, rep_status, ts, ok_identity) = {
                let b = node.borrow();
                (
                    b.children.clone(),
                    b.is_directory(),
                    b.got_status(),
                    b.rep_status(),
                    b.file_mod_time_stamp,
                    Self::identity_can_fix(&deleting.borrow(), &b),
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
                if deleting_ts != i64::MIN && !Self::timestamp_matches(&deleting_path, deleting_ts) {
                    continue;
                }
                return Some(node);
            }
        }
        None
    }
}
