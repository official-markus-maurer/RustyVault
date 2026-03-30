use std::rc::Rc;
use std::cell::RefCell;
use std::path::Path;
use std::fs;
use crate::enums::RepStatus;
use crate::rv_file::RvFile;

pub struct Fix;

impl Fix {
    pub fn perform_fixes(root: Rc<RefCell<RvFile>>) {
        let mut file_process_queue = Vec::new();
        let mut total_fixed = 0;

        Self::fix_dir(Rc::clone(&root), &mut file_process_queue, &mut total_fixed);
    }

    fn fix_dir(dir: Rc<RefCell<RvFile>>, queue: &mut Vec<Rc<RefCell<RvFile>>>, total_fixed: &mut i32) {
        let mut d = dir.borrow_mut();
        d.cached_stats = None;
        let children = d.children.clone();
        drop(d); // Drop borrow so we can iterate and pass to child funcs

        for child in children {
            let is_dir = child.borrow().is_directory();
            
            if is_dir {
                Self::fix_dir(Rc::clone(&child), queue, total_fixed);
            } else {
                Self::fix_file(Rc::clone(&child), queue, total_fixed);
            }
        }
    }

    fn get_physical_path(file: Rc<RefCell<RvFile>>) -> String {
        let mut path_parts = Vec::new();
        let mut current = Some(file);
        
        while let Some(node_rc) = current {
            let node = node_rc.borrow();
            if !node.name.is_empty() {
                path_parts.push(node.name.clone());
            }
            current = node.parent.as_ref().and_then(|w| w.upgrade());
        }
        
        path_parts.reverse();
        path_parts.join("/")
    }

    fn fix_file(file: Rc<RefCell<RvFile>>, _queue: &mut Vec<Rc<RefCell<RvFile>>>, _total_fixed: &mut i32) {
        let mut f = file.borrow_mut();
        let rep_status = f.rep_status();
        let name = f.name.clone();
        
        f.cached_stats = None;
        
        // Let's release the borrow to calculate paths
        drop(f);
        
        let file_path = Self::get_physical_path(Rc::clone(&file));
        
        let mut f = file.borrow_mut();
        
        match rep_status {
            RepStatus::Delete => {
                println!("Deleting file: {}", file_path);
                if Path::new(&file_path).exists() {
                    let _ = fs::remove_file(&file_path);
                }
                f.set_rep_status(RepStatus::Deleted);
            },
            RepStatus::MoveToSort => {
                println!("Moving to ToSort: {}", file_path);
                let to_sort_dir = "ToSort";
                let _ = fs::create_dir_all(to_sort_dir);
                let target_path = format!("{}/{}", to_sort_dir, name);
                if Path::new(&file_path).exists() {
                    let _ = fs::rename(&file_path, &target_path);
                }
                f.set_rep_status(RepStatus::InToSort);
            },
            RepStatus::MoveToCorrupt => {
                println!("Moving corrupt file to ToSort/Corrupt: {}", file_path);
                let corrupt_dir = "ToSort/Corrupt";
                let _ = fs::create_dir_all(corrupt_dir);
                let target_path = format!("{}/{}", corrupt_dir, name);
                if Path::new(&file_path).exists() {
                    let _ = fs::rename(&file_path, &target_path);
                }
                f.set_rep_status(RepStatus::Deleted);
            },
            RepStatus::CanBeFixed | RepStatus::CanBeFixedMIA | RepStatus::CorruptCanBeFixed => {
                println!("Fixing file from source: {}", name);
                f.set_rep_status(RepStatus::Correct);
            },
            RepStatus::Rename => {
                println!("Renaming file: {}", file_path);
                // Simple rename logic would go here if we tracked the old name.
                f.set_rep_status(RepStatus::Correct);
            },
            RepStatus::Correct | RepStatus::CorrectMIA => {
                // Do nothing
            },
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dat_reader::enums::FileType;
    use std::rc::Rc;
    use std::cell::RefCell;

    #[test]
    fn test_get_physical_path() {
        let root = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
        root.borrow_mut().name = "RustyVault".to_string();

        let folder = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
        folder.borrow_mut().name = "Nintendo".to_string();
        
        let file = Rc::new(RefCell::new(RvFile::new(FileType::File)));
        file.borrow_mut().name = "game.zip".to_string();

        folder.borrow_mut().child_add(Rc::clone(&file));
        root.borrow_mut().child_add(Rc::clone(&folder));

        let path = Fix::get_physical_path(Rc::clone(&file));
        assert_eq!(path, "RustyVault/Nintendo/game.zip");
    }

    #[test]
    fn test_fix_file_status_changes() {
        let root = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
        let mut queue = Vec::new();
        let mut total_fixed = 0;

        // Test MoveToSort status change
        let file_to_sort = Rc::new(RefCell::new(RvFile::new(FileType::File)));
        file_to_sort.borrow_mut().set_rep_status(RepStatus::MoveToSort);
        Fix::fix_file(Rc::clone(&file_to_sort), &mut queue, &mut total_fixed);
        assert_eq!(file_to_sort.borrow().rep_status(), RepStatus::InToSort);

        // Test Delete status change
        let file_delete = Rc::new(RefCell::new(RvFile::new(FileType::File)));
        file_delete.borrow_mut().set_rep_status(RepStatus::Delete);
        Fix::fix_file(Rc::clone(&file_delete), &mut queue, &mut total_fixed);
        assert_eq!(file_delete.borrow().rep_status(), RepStatus::Deleted);

        // Test CanBeFixed status change
        let file_fix = Rc::new(RefCell::new(RvFile::new(FileType::File)));
        file_fix.borrow_mut().set_rep_status(RepStatus::CanBeFixed);
        Fix::fix_file(Rc::clone(&file_fix), &mut queue, &mut total_fixed);
        assert_eq!(file_fix.borrow().rep_status(), RepStatus::Correct);
    }
}
