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

    fn has_retained_shared_physical_path(
        current_idx: usize,
        files_got: &[Rc<RefCell<RvFile>>],
    ) -> bool {
        let current_path = Self::build_physical_identity(Rc::clone(&files_got[current_idx]));
        files_got.iter().enumerate().any(|(idx, candidate)| {
            if idx == current_idx {
                return false;
            }

            let candidate_ref = candidate.borrow();
            let candidate_path = Self::build_physical_identity(Rc::clone(candidate));
            candidate_ref.got_status() == GotStatus::Got
                && Self::dat_status_retains_shared_path(candidate_ref.dat_status())
                && Self::cleanup_status_retains_shared_path(candidate_ref.rep_status())
                && Self::physical_identity_eq(&candidate_path, &current_path)
        })
    }

    fn merged_cleanup_status(current_idx: usize, files_got: &[Rc<RefCell<RvFile>>]) -> RepStatus {
        if Self::has_retained_shared_physical_path(current_idx, files_got) {
            RepStatus::NotCollected
        } else {
            RepStatus::UnNeeded
        }
    }
}
