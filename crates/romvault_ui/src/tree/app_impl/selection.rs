impl RomVaultApp {
    pub fn expand_selected_ancestors(&mut self) {
        let Some(selected) = &self.selected_node else {
            return;
        };

        let mut current = selected.borrow().parent.as_ref().and_then(|p| p.upgrade());
        while let Some(node_rc) = current {
            self.enqueue_tree_stats_priority(Rc::clone(&node_rc));
            let next = {
                let mut n = node_rc.borrow_mut();
                if !n.tree_expanded {
                    n.tree_expanded = true;
                    self.tree_rows_dirty = true;
                }
                n.parent.as_ref().and_then(|p| p.upgrade())
            };
            current = next;
        }
    }

    pub fn select_node(&mut self, node_rc: Rc<RefCell<RvFile>>) {
        let is_game_like = {
            let node = node_rc.borrow();
            node.game.is_some()
                || (node.is_directory()
                    && node.dat_status() != DatStatus::NotInDat
                    && node.children.iter().any(|c| c.borrow().is_file()))
        };

        self.selected_node = Some(Rc::clone(&node_rc));
        if is_game_like {
            self.selected_game = Some(node_rc);
        } else {
            self.selected_game = None;
        }
        self.pending_tree_scroll_to_selected = true;
        if let Some(selected) = self.selected_node.as_ref() {
            self.enqueue_tree_stats_priority(Rc::clone(selected));
        }
        self.expand_selected_ancestors();
    }
}
