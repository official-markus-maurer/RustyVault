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
    ) -> Option<usize> {
        got_list
            .iter()
            .copied()
            .filter(|idx| !used_got_indices.contains(idx))
            .min_by_key(|idx| {
                let got = files_got[*idx].borrow();
                let (location_priority, corruption_priority, consumable_priority) =
                    Self::got_source_priority(&got);
                let shared_backing_priority =
                    if Self::has_retained_shared_physical_path(*idx, files_got) {
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
