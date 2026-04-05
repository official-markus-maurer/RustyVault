impl FindFixes {
    fn reset_status(node: Rc<RefCell<RvFile>>) {
        crate::repair_status::RepairStatus::report_status_reset(node);
    }

    fn get_selected_files(
        node: Rc<RefCell<RvFile>>,
        got_files: &mut Vec<Rc<RefCell<RvFile>>>,
        missing_files: &mut Vec<Rc<RefCell<RvFile>>>,
    ) {
        let n = node.borrow();
        let selected = Self::is_tree_selected(&n);

        if !n.is_directory() {
            match n.got_status() {
                GotStatus::Got | GotStatus::Corrupt => {
                    if selected || n.dat_status() == DatStatus::InToSort {
                        got_files.push(Rc::clone(&node));
                    }
                }
                GotStatus::NotGot => {
                    if selected && !matches!(n.dat_status(), DatStatus::NotInDat | DatStatus::InToSort) {
                        missing_files.push(Rc::clone(&node));
                    }
                }
                _ => {}
            }
        }

        if !n.is_directory() {
            return;
        }

        let children = n.children.clone();
        drop(n);

        for child in children {
            Self::get_selected_files(child, got_files, missing_files);
        }
    }

    fn get_all_got_files(node: Rc<RefCell<RvFile>>, got_files: &mut Vec<Rc<RefCell<RvFile>>>) {
        let n = node.borrow();

        if !n.is_directory() && matches!(n.got_status(), GotStatus::Got | GotStatus::Corrupt) {
            got_files.push(Rc::clone(&node));
        }

        if !n.is_directory() {
            return;
        }

        let children = n.children.clone();
        drop(n);

        for child in children {
            Self::get_all_got_files(child, got_files);
        }
    }

    fn get_all_dat_files(node: Rc<RefCell<RvFile>>, dat_files: &mut Vec<Rc<RefCell<RvFile>>>) {
        let n = node.borrow();

        if !n.is_directory()
            && matches!(
                n.dat_status(),
                DatStatus::InDatCollect
                    | DatStatus::InDatMIA
                    | DatStatus::InDatMerged
                    | DatStatus::InDatNoDump
            )
        {
            dat_files.push(Rc::clone(&node));
        }

        if !n.is_directory() {
            return;
        }

        let children = n.children.clone();
        drop(n);

        for child in children {
            Self::get_all_dat_files(child, dat_files);
        }
    }
}
