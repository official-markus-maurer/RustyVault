use std::rc::Rc;
use std::cell::RefCell;
use dat_reader::enums::DatStatus;
use crate::rv_file::RvFile;

/// Utility functions for querying the database tree.
/// 
/// `DbHelper` contains static helper functions for extracting specific subsets
/// of `RvFile` nodes from the `dir_root` tree, such as generating flat lists
/// of all currently selected directories.
/// 
/// Differences from C#:
/// - The C# `DB` class includes many of these helper functions directly within it.
/// - The Rust version splits them into a dedicated `DbHelper` module to keep `db.rs` clean
///   and prevent `RefCell` borrowing collisions during recursive queries.
pub struct DbHelper;

impl DbHelper {
    /// Recursively flattens a directory branch into a flat vector of directories
    /// that are actively marked as `InDatCollect` (part of the primary vault).
    pub fn get_selected_dir_list(lst_dir: &mut Vec<Rc<RefCell<RvFile>>>, this_dir: Rc<RefCell<RvFile>>) {
        let dir = this_dir.borrow();
        
        for child in &dir.children {
            let child_ref = child.borrow();
            
            if dir.dat_status() != DatStatus::InDatCollect {
                continue;
            }
            
            if !child_ref.is_directory() {
                continue;
            }
            
            // Assuming tree is selected if we reach here in this stripped down core
            lst_dir.push(Rc::clone(child));
            
            // Drop borrow to allow recursion
            drop(child_ref);
            Self::get_selected_dir_list(lst_dir, Rc::clone(child));
        }
    }
}
