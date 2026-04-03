use std::cell::RefCell;
use std::rc::Rc;

use crate::enums::RepStatus;
use crate::rv_file::{RvFile, TreeSelect};

fn node_logical_path(node: &RvFile) -> String {
    let mut parts = vec![node.name.clone()];
    let mut parent = node.get_parent();
    while let Some(p) = parent {
        let b = p.borrow();
        if !b.name.is_empty() {
            parts.push(b.name.clone());
        }
        parent = b.get_parent();
    }
    parts.reverse();
    parts.join("\\")
}

fn is_tree_selected(node: &RvFile) -> bool {
    matches!(node.tree_checked, TreeSelect::Selected | TreeSelect::Locked)
}

fn status_check_file(rep: RepStatus, found_missing: &mut bool, found_got_or_fixable: &mut bool) {
    match rep {
        RepStatus::Missing | RepStatus::MissingMIA => *found_missing = true,
        RepStatus::Correct | RepStatus::CorrectMIA | RepStatus::CanBeFixed | RepStatus::CanBeFixedMIA => {
            *found_got_or_fixable = true
        }
        RepStatus::MoveToSort | RepStatus::NotCollected => {}
        _ => {}
    }
}

fn status_check_dir(node: Rc<RefCell<RvFile>>, found_missing: &mut bool, found_got_or_fixable: &mut bool) {
    let children = node.borrow().children.clone();
    for child in children {
        let child_is_dir = child.borrow().is_directory();
        if child_is_dir {
            status_check_dir(child, found_missing, found_got_or_fixable);
        } else {
            let rep = child.borrow().rep_status();
            status_check_file(rep, found_missing, found_got_or_fixable);
        }
    }
}

fn status_set_file(node: &mut RvFile) {
    match node.rep_status() {
        RepStatus::CanBeFixed | RepStatus::CanBeFixedMIA => node.set_rep_status(RepStatus::Incomplete),
        RepStatus::Correct | RepStatus::CorrectMIA => node.set_rep_status(RepStatus::IncompleteRemove),
        _ => {}
    }
    node.cached_stats = None;
}

fn status_set_dir(node: Rc<RefCell<RvFile>>) {
    let children = node.borrow().children.clone();
    for child in children {
        if child.borrow().is_directory() {
            status_set_dir(child);
        } else {
            status_set_file(&mut child.borrow_mut());
        }
    }
}

fn remove_partial_sets(game_dir: Rc<RefCell<RvFile>>) {
    if game_dir.borrow().game.is_none() {
        return;
    }
    let mut found_missing = false;
    let mut found_got_or_fixable = false;
    status_check_dir(Rc::clone(&game_dir), &mut found_missing, &mut found_got_or_fixable);
    if !found_missing || !found_got_or_fixable {
        return;
    }
    status_set_dir(game_dir);
}

fn check_remove_partial(base: Rc<RefCell<RvFile>>, selected: bool) {
    let next_selected = selected && is_tree_selected(&base.borrow());
    let dat_rule_complete_only = if next_selected {
        let logical = node_logical_path(&base.borrow());
        crate::settings::find_rule(&format!("{}\\", logical)).complete_only
    } else {
        false
    };

    let children = base.borrow().children.clone();
    for child in children {
        if child.borrow().game.is_some() {
            if next_selected && dat_rule_complete_only {
                remove_partial_sets(child);
            }
        } else {
            check_remove_partial(child, next_selected);
        }
    }
}

pub fn apply_complete_only(root: Rc<RefCell<RvFile>>) {
    let child0 = root.borrow().children.first().cloned();
    let start = child0.unwrap_or_else(|| Rc::clone(&root));
    check_remove_partial(start, true);
}

