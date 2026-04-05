impl RomVaultApp {
    pub fn enqueue_tree_stats(&mut self, node_rc: Rc<RefCell<RvFile>>) {
        let should_enqueue = {
            let node = node_rc.borrow();
            node.is_directory() && node.cached_stats.is_none()
        };
        if !should_enqueue {
            return;
        }

        let key = Rc::as_ptr(&node_rc) as usize;
        if self.tree_stats_queued.insert(key) {
            self.tree_stats_queue.push_back(node_rc);
        }
    }

    pub fn enqueue_tree_stats_priority(&mut self, node_rc: Rc<RefCell<RvFile>>) {
        let should_enqueue = {
            let node = node_rc.borrow();
            node.is_directory() && node.cached_stats.is_none()
        };
        if !should_enqueue {
            return;
        }

        let key = Rc::as_ptr(&node_rc) as usize;
        if self.tree_stats_queued.insert(key) {
            self.tree_stats_queue.push_front(node_rc);
        }
    }

    pub fn process_tree_stats_queue(&mut self, ctx: &egui::Context) {
        let start = std::time::Instant::now();
        let mut did_work = false;
        while start.elapsed() < std::time::Duration::from_millis(2) {
            let Some(node_rc) = self.tree_stats_queue.pop_front() else {
                break;
            };
            let key = Rc::as_ptr(&node_rc) as usize;
            self.tree_stats_queued.remove(&key);

            let should_compute = {
                let node = node_rc.borrow();
                node.is_directory() && node.cached_stats.is_none()
            };
            if !should_compute {
                continue;
            }

            let mut stats = rv_core::repair_status::RepairStatus::new();
            stats.report_status(Rc::clone(&node_rc));

            {
                let mut node = node_rc.borrow_mut();
                if node.cached_stats.is_none() {
                    node.cached_stats = Some(stats);
                    node.ui_display_name.clear();
                    did_work = true;
                }
            }
        }

        if did_work {
            ctx.request_repaint();
        }
    }
}
