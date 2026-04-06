pub(crate) struct TreeStatsActive {
    root_key: usize,
    stack: Vec<TreeStatsFrame>,
}

struct TreeStatsFrame {
    node: Rc<RefCell<RvFile>>,
    children: Vec<Rc<RefCell<RvFile>>>,
    child_index: usize,
    stats: rv_core::repair_status::RepairStatus,
    is_file: bool,
    is_game: bool,
    rep_status: rv_core::enums::RepStatus,
    has_dir_status: bool,
}

impl TreeStatsFrame {
    fn new(node: Rc<RefCell<RvFile>>) -> Self {
        let (children, is_file, is_game, rep_status, has_dir_status) = {
            let n = node.borrow();
            (
                if n.is_directory() {
                    n.children.clone()
                } else {
                    Vec::new()
                },
                n.is_file(),
                n.game.is_some(),
                n.rep_status(),
                n.dir_status.is_some(),
            )
        };
        Self {
            node,
            children,
            child_index: 0,
            stats: rv_core::repair_status::RepairStatus::new(),
            is_file,
            is_game,
            rep_status,
            has_dir_status,
        }
    }
}

impl TreeStatsActive {
    fn new(root: Rc<RefCell<RvFile>>, root_key: usize) -> Self {
        Self {
            root_key,
            stack: vec![TreeStatsFrame::new(root)],
        }
    }

    fn is_done(&self) -> bool {
        self.stack.is_empty()
    }
}

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
            if self.tree_stats_active.is_none() {
                let Some(node_rc) = self.tree_stats_queue.pop_front() else {
                    break;
                };
                let key = Rc::as_ptr(&node_rc) as usize;

                let should_compute = {
                    let node = node_rc.borrow();
                    node.is_directory() && node.cached_stats.is_none()
                };
                if !should_compute {
                    self.tree_stats_queued.remove(&key);
                    continue;
                }

                self.tree_stats_active = Some(TreeStatsActive::new(node_rc, key));
            }

            let Some(active) = self.tree_stats_active.as_mut() else {
                continue;
            };

            let mut steps = 0usize;
            while steps < 128 && start.elapsed() < std::time::Duration::from_millis(2) {
                let Some(frame) = active.stack.last_mut() else {
                    break;
                };

                let cached = { frame.node.borrow().cached_stats };
                if let Some(cached) = cached {
                    active.stack.pop();
                    if let Some(parent) = active.stack.last_mut() {
                        add_stats(&mut parent.stats, cached);
                    }
                    did_work = true;
                    steps += 1;
                    continue;
                }

                if frame.child_index < frame.children.len() {
                    let child_rc = Rc::clone(&frame.children[frame.child_index]);
                    if let Some(child_stats) = { child_rc.borrow().cached_stats } {
                        add_stats(&mut frame.stats, child_stats);
                        frame.child_index += 1;
                        did_work = true;
                        steps += 1;
                        continue;
                    }

                    let child_is_dir = { child_rc.borrow().is_directory() };
                    if child_is_dir {
                        frame.child_index += 1;
                        active.stack.push(TreeStatsFrame::new(child_rc));
                        steps += 1;
                        continue;
                    }

                    let child_stats = compute_leaf_stats(&child_rc.borrow());
                    {
                        let mut child = child_rc.borrow_mut();
                        if child.cached_stats.is_none() {
                            child.cached_stats = Some(child_stats);
                            child.ui_display_name.clear();
                        }
                    }
                    add_stats(&mut frame.stats, child_stats);
                    frame.child_index += 1;
                    did_work = true;
                    steps += 1;
                    continue;
                }

                apply_self_counts(
                    &mut frame.stats,
                    frame.is_game,
                    frame.is_file || (frame.is_game && frame.children.is_empty()),
                    frame.rep_status,
                );
                if frame.has_dir_status {
                    let status = frame.stats.synthesized_dir_status();
                    frame.node.borrow_mut().dir_status = Some(status);
                }
                {
                    let mut node = frame.node.borrow_mut();
                    if node.cached_stats.is_none() {
                        node.cached_stats = Some(frame.stats);
                        node.ui_display_name.clear();
                    }
                }

                let completed_stats = frame.stats;
                active.stack.pop();
                if let Some(parent) = active.stack.last_mut() {
                    add_stats(&mut parent.stats, completed_stats);
                }
                did_work = true;
                steps += 1;
            }

            if active.is_done() {
                self.tree_stats_queued.remove(&active.root_key);
                self.tree_stats_active = None;
            }
        }

        if did_work {
            ctx.request_repaint();
        }
    }
}

fn add_stats(a: &mut rv_core::repair_status::RepairStatus, b: rv_core::repair_status::RepairStatus) {
    a.total_games += b.total_games;
    a.total_roms += b.total_roms;
    a.games_correct += b.games_correct;
    a.games_missing += b.games_missing;
    a.games_missing_mia += b.games_missing_mia;
    a.games_fixes += b.games_fixes;
    a.roms_correct += b.roms_correct;
    a.roms_correct_mia += b.roms_correct_mia;
    a.roms_missing += b.roms_missing;
    a.roms_missing_mia += b.roms_missing_mia;
    a.roms_corrupt += b.roms_corrupt;
    a.roms_fixes += b.roms_fixes;
    a.roms_in_to_sort += b.roms_in_to_sort;
    a.roms_not_collected += b.roms_not_collected;
    a.roms_unneeded += b.roms_unneeded;
    a.roms_unknown += b.roms_unknown;
}

fn compute_leaf_stats(node: &RvFile) -> rv_core::repair_status::RepairStatus {
    let mut stats = rv_core::repair_status::RepairStatus::new();
    let is_game = node.game.is_some();
    let rep_status = node.rep_status();
    let count_as_file = node.is_file() || is_game;
    apply_self_counts(&mut stats, is_game, count_as_file, rep_status);
    stats
}

fn apply_self_counts(
    stats: &mut rv_core::repair_status::RepairStatus,
    is_game: bool,
    count_as_file: bool,
    rep_status: rv_core::enums::RepStatus,
) {
    use rv_core::enums::RepStatus;

    if is_game {
        stats.total_games += 1;
        match rep_status {
            RepStatus::Correct | RepStatus::DirCorrect => stats.games_correct += 1,
            RepStatus::CorrectMIA => {
                stats.games_correct += 1;
                stats.games_missing_mia += 1;
            }
            RepStatus::Missing
            | RepStatus::DirMissing
            | RepStatus::Corrupt
            | RepStatus::DirCorrupt
            | RepStatus::Incomplete => stats.games_missing += 1,
            RepStatus::MissingMIA => {
                stats.games_missing += 1;
                stats.games_missing_mia += 1;
            }
            RepStatus::CanBeFixed
            | RepStatus::CanBeFixedMIA
            | RepStatus::CorruptCanBeFixed
            | RepStatus::InToSort
            | RepStatus::DirInToSort
            | RepStatus::MoveToSort
            | RepStatus::MoveToCorrupt
            | RepStatus::Delete
            | RepStatus::Deleted
            | RepStatus::NeededForFix
            | RepStatus::Rename
            | RepStatus::IncompleteRemove => stats.games_fixes += 1,
            _ => {}
        }
    }

    if count_as_file {
        stats.total_roms += 1;
        match rep_status {
            RepStatus::Correct | RepStatus::DirCorrect => stats.roms_correct += 1,
            RepStatus::CorrectMIA => {
                stats.roms_correct += 1;
                stats.roms_correct_mia += 1;
            }
            RepStatus::Missing | RepStatus::DirMissing => stats.roms_missing += 1,
            RepStatus::MissingMIA => {
                stats.roms_missing += 1;
                stats.roms_missing_mia += 1;
            }
            RepStatus::Corrupt | RepStatus::DirCorrupt | RepStatus::Incomplete => {
                stats.roms_corrupt += 1;
                stats.roms_missing += 1;
            }
            RepStatus::CanBeFixed
            | RepStatus::CanBeFixedMIA
            | RepStatus::CorruptCanBeFixed
            | RepStatus::MoveToSort
            | RepStatus::MoveToCorrupt
            | RepStatus::Delete
            | RepStatus::Deleted
            | RepStatus::NeededForFix
            | RepStatus::Rename
            | RepStatus::IncompleteRemove => stats.roms_fixes += 1,
            RepStatus::InToSort | RepStatus::DirInToSort => stats.roms_in_to_sort += 1,
            RepStatus::NotCollected => stats.roms_not_collected += 1,
            RepStatus::UnNeeded => stats.roms_unneeded += 1,
            RepStatus::Unknown | RepStatus::DirUnknown | RepStatus::UnScanned => {
                stats.roms_unknown += 1
            }
            _ => {}
        }
    }
}
