impl FindFixes {
    fn is_tree_selected(node: &RvFile) -> bool {
        matches!(node.tree_checked, TreeSelect::Selected | TreeSelect::Locked)
    }

    fn source_is_consumable(node: &RvFile) -> bool {
        !matches!(node.tree_checked, TreeSelect::Locked)
            && !matches!(node.dat_status(), DatStatus::InDatCollect | DatStatus::InDatMIA)
    }

    fn got_source_priority(node: &RvFile) -> (u8, u8, u8) {
        let location_priority = match node.dat_status() {
            DatStatus::InDatCollect => 0,
            DatStatus::InDatMIA => 1,
            DatStatus::InDatMerged | DatStatus::InDatNoDump => 2,
            DatStatus::InToSort => 3,
            DatStatus::NotInDat => 4,
        };
        let corruption_priority = match node.got_status() {
            GotStatus::Got => 0,
            GotStatus::Corrupt => 1,
            _ => 2,
        };
        let consumable_priority = if Self::source_is_consumable(node) { 0 } else { 1 };
        (location_priority, corruption_priority, consumable_priority)
    }

    fn preferred_got_idx(
        got_list: &[usize],
        files_got: &[Rc<RefCell<RvFile>>],
        used_got_indices: &HashSet<usize>,
        got_identity_keys: &[String],
        got_identity_retains: &[bool],
        got_retained_counts: &std::collections::HashMap<String, u32>,
        missing: &RvFile,
    ) -> Option<usize> {
        got_list
            .iter()
            .copied()
            .filter(|idx| !used_got_indices.contains(idx))
            .filter(|idx| {
                let got = files_got[*idx].borrow();
                Self::missing_can_be_fixed_by_got(missing, &got)
            })
            .min_by_key(|idx| {
                let got = files_got[*idx].borrow();
                let (location_priority, corruption_priority, consumable_priority) =
                    Self::got_source_priority(&got);
                let shared_backing_priority = if Self::has_other_retained_shared_physical_path(
                    *idx,
                    got_identity_keys,
                    got_identity_retains,
                    got_retained_counts,
                ) {
                    1
                } else {
                    0
                };
                (
                    location_priority,
                    shared_backing_priority,
                    corruption_priority,
                    consumable_priority,
                )
            })
    }
}
