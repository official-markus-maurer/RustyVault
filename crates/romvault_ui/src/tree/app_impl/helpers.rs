impl RomVaultApp {
    fn ui_working(&self) -> bool {
        self.sam_running
    }

    fn expand_descendants_target(node_rc: &Rc<RefCell<RvFile>>) -> Option<bool> {
        let children = node_rc.borrow().children.clone();
        for child in children {
            let cb = child.borrow();
            if cb.is_directory() && cb.game.is_none() {
                return Some(!cb.tree_expanded);
            }
        }
        None
    }

    fn set_descendants_expanded(node_rc: &Rc<RefCell<RvFile>>, expanded: bool) {
        let children = node_rc.borrow().children.clone();
        let mut stack: Vec<Rc<RefCell<RvFile>>> = children
            .into_iter()
            .filter(|c| {
                let cb = c.borrow();
                cb.is_directory() && cb.game.is_none()
            })
            .collect();

        while let Some(current) = stack.pop() {
            let grandchildren = {
                let mut n = current.borrow_mut();
                n.tree_expanded = expanded;
                n.children.clone()
            };
            for gc in grandchildren {
                let gcb = gc.borrow();
                if gcb.is_directory() && gcb.game.is_none() {
                    drop(gcb);
                    stack.push(gc);
                }
            }
        }
    }

    fn set_tree_checked_locked(node_rc: &Rc<RefCell<RvFile>>, recurse: bool) {
        let mut stack = vec![Rc::clone(node_rc)];
        while let Some(current) = stack.pop() {
            let children = {
                let mut n = current.borrow_mut();
                if n.to_sort_status_is(rv_core::enums::ToSortDirType::TO_SORT_PRIMARY)
                    || n.to_sort_status_is(rv_core::enums::ToSortDirType::TO_SORT_CACHE)
                {
                    Vec::new()
                } else {
                    n.tree_checked = TreeSelect::Locked;
                    n.children.clone()
                }
            };
            if recurse {
                for child in children {
                    stack.push(child);
                }
            }
        }
    }

    fn is_ancestor_or_self(ancestor: &Rc<RefCell<RvFile>>, node: &Rc<RefCell<RvFile>>) -> bool {
        let mut current = Some(Rc::clone(node));
        while let Some(rc) = current {
            if Rc::ptr_eq(&rc, ancestor) {
                return true;
            }
            current = rc.borrow().parent.as_ref().and_then(|p| p.upgrade());
        }
        false
    }
}
