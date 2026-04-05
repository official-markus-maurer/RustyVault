impl RomVaultApp {
    pub fn rebuild_tree_rows_cache(&mut self) {
        let mut rows: Vec<TreeRow> = Vec::new();

        GLOBAL_DB.with(|db_ref| {
            if let Some(db) = db_ref.borrow().as_ref() {
                let root = Rc::clone(&db.dir_root);
                let top_children = root.borrow().children.clone();

                let mut stack: Vec<(Rc<RefCell<RvFile>>, usize)> = Vec::new();
                for child in top_children.into_iter().rev() {
                    stack.push((child, 0));
                }

                while let Some((node_rc, depth)) = stack.pop() {
                    let (is_file, is_game, tree_expanded, dir_children) = {
                        let node = node_rc.borrow();
                        (
                            node.is_file(),
                            node.game.is_some(),
                            node.tree_expanded,
                            if node.tree_expanded {
                                node.children
                                    .iter()
                                    .filter(|c| {
                                        let cb = c.borrow();
                                        !cb.is_file() && cb.game.is_none()
                                    })
                                    .cloned()
                                    .collect::<Vec<_>>()
                            } else {
                                Vec::new()
                            },
                        )
                    };

                    if is_file || is_game {
                        continue;
                    }

                    rows.push(TreeRow {
                        node_rc: Rc::clone(&node_rc),
                        depth,
                    });

                    if tree_expanded {
                        for child in dir_children.into_iter().rev() {
                            stack.push((child, depth + 1));
                        }
                    }
                }
            }
        });

        self.tree_rows_cache = rows;
        self.tree_rows_dirty = false;
    }
}
