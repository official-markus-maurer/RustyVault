use std::rc::Rc;
use std::cell::RefCell;
use dat_reader::enums::DatStatus;
use crate::rv_file::RvFile;

pub struct DbHelper;

impl DbHelper {
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
