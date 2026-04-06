impl FindFixes {
    fn cleanup_status_retains_shared_path(rep_status: RepStatus) -> bool {
        !matches!(
            rep_status,
            RepStatus::Delete
                | RepStatus::UnNeeded
                | RepStatus::MoveToSort
                | RepStatus::MoveToCorrupt
                | RepStatus::Deleted
        )
    }

    fn dat_status_retains_shared_path(dat_status: DatStatus) -> bool {
        !matches!(dat_status, DatStatus::NotInDat | DatStatus::InToSort)
    }

    fn physical_identity_key(identity: &str) -> String {
        #[cfg(windows)]
        {
            let (path_part, member_part) = identity.split_once("::").unwrap_or((identity, ""));
            let mut normalized_path = path_part.replace('/', "\\");
            while normalized_path.len() > 3 && normalized_path.ends_with('\\') {
                normalized_path.pop();
            }
            let normalized = if member_part.is_empty() {
                normalized_path
            } else {
                format!(
                    "{}::{}",
                    normalized_path,
                    member_part.replace('\\', "/").trim_matches('/'),
                )
            };
            normalized.to_ascii_lowercase()
        }
        #[cfg(not(windows))]
        {
            identity.to_string()
        }
    }

    fn build_retained_physical_identity_index(
        files_got: &[Rc<RefCell<RvFile>>],
    ) -> (Vec<String>, Vec<bool>, HashMap<String, u32>) {
        let mut keys = Vec::with_capacity(files_got.len());
        let mut retains = Vec::with_capacity(files_got.len());
        let mut counts: HashMap<String, u32> = HashMap::new();

        for got in files_got {
            let (got_status, dat_status, rep_status) = {
                let g = got.borrow();
                (g.got_status(), g.dat_status(), g.rep_status())
            };
            let retains_shared = got_status == GotStatus::Got
                && Self::dat_status_retains_shared_path(dat_status)
                && Self::cleanup_status_retains_shared_path(rep_status);
            let identity = Self::build_physical_identity(Rc::clone(got));
            let key = Self::physical_identity_key(&identity);

            if retains_shared {
                *counts.entry(key.clone()).or_insert(0) += 1;
            }
            keys.push(key);
            retains.push(retains_shared);
        }

        (keys, retains, counts)
    }

    fn has_other_retained_shared_physical_path(
        current_idx: usize,
        identity_keys: &[String],
        identity_retains: &[bool],
        retained_counts: &HashMap<String, u32>,
    ) -> bool {
        let Some(key) = identity_keys.get(current_idx) else {
            return false;
        };
        let count = retained_counts.get(key).copied().unwrap_or(0);
        if identity_retains.get(current_idx).copied().unwrap_or(false) {
            count > 1
        } else {
            count > 0
        }
    }

    fn merged_cleanup_status_with_shared(
        current_idx: usize,
        identity_keys: &[String],
        identity_retains: &[bool],
        retained_counts: &HashMap<String, u32>,
    ) -> RepStatus {
        if Self::has_other_retained_shared_physical_path(
            current_idx,
            identity_keys,
            identity_retains,
            retained_counts,
        ) {
            RepStatus::NotCollected
        } else {
            RepStatus::UnNeeded
        }
    }
}
