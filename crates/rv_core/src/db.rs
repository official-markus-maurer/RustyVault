use std::fs;
use std::path::Path;
use std::rc::Rc;
use std::cell::RefCell;
use dat_reader::enums::{DatStatus, FileType};
use crate::rv_file::RvFile;
use crate::enums::ToSortDirType;

use crate::cache::Cache;

pub struct DB {
    pub dir_root: Rc<RefCell<RvFile>>,
}

impl DB {
    pub fn new() -> Self {
        if let Some(root) = Cache::read_cache() {
            Self::check_create_root_dirs();
            return Self { dir_root: root };
        }

        let root = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
        {
            let mut root_mut = root.borrow_mut();
            root_mut.set_dat_status(DatStatus::InDatCollect);

            let mut rv = RvFile::new(FileType::Dir);
        rv.name = "RustyVault".to_string();
            rv.set_dat_status(DatStatus::InDatCollect);
            root_mut.child_add(Rc::new(RefCell::new(rv)));

            let mut ts = RvFile::new(FileType::Dir);
            ts.name = "ToSort".to_string();
            ts.set_dat_status(DatStatus::InToSort);
            ts.to_sort_status_set(ToSortDirType::TO_SORT_PRIMARY | ToSortDirType::TO_SORT_CACHE);
            root_mut.child_add(Rc::new(RefCell::new(ts)));
        }

        Self::check_create_root_dirs();

        Self {
            dir_root: root,
        }
    }

    fn check_create_root_dirs() {
        // Create DatRoot
        let dat_root = crate::settings::get_settings().dat_root;
        let dat_root_path = if dat_root.is_empty() { "DatRoot" } else { &dat_root };
        if !Path::new(dat_root_path).exists() {
            let _ = fs::create_dir_all(dat_root_path);
        }

        // Create RustyVault
        if !Path::new("RustyVault").exists() {
            let _ = fs::create_dir_all("RustyVault");
        }

        // Create ToSort
        if !Path::new("ToSort").exists() {
            let _ = fs::create_dir_all("ToSort");
        }
        
        // Create ToSort Cache Directory (mimicking the ToSortPrimary logic)
        // Usually it's just the ToSort dir itself for Primary/Cache, but let's ensure it exists
        if !Path::new("ToSort").exists() {
            let _ = fs::create_dir_all("ToSort");
        }
    }

    pub fn get_to_sort_cache(&self) -> Rc<RefCell<RvFile>> {
        let root = self.dir_root.borrow();
        for child in &root.children {
            if child.borrow().to_sort_status_is(ToSortDirType::TO_SORT_CACHE) {
                return Rc::clone(child);
            }
        }
        
        // Fallback to first child which is typically RustyVault or ToSort
        if root.children.len() > 1 {
            Rc::clone(&root.children[1])
        } else {
            Rc::clone(&self.dir_root)
        }
    }

    pub fn get_to_sort_primary(&self) -> Rc<RefCell<RvFile>> {
        let root = self.dir_root.borrow();
        for child in root.children.iter().skip(1) {
            if child.borrow().to_sort_status_is(ToSortDirType::TO_SORT_PRIMARY) {
                return Rc::clone(child);
            }
        }

        if root.children.len() > 1 {
            Rc::clone(&root.children[1])
        } else {
            Rc::clone(&self.dir_root)
        }
    }

    pub fn get_to_sort_file_only(&self) -> Rc<RefCell<RvFile>> {
        let root = self.dir_root.borrow();
        for child in root.children.iter().skip(1) {
            if child.borrow().to_sort_status_is(ToSortDirType::TO_SORT_FILE_ONLY) {
                return Rc::clone(child);
            }
        }

        self.get_to_sort_primary()
    }

    pub fn write_cache(&self) {
        Cache::write_cache(Rc::clone(&self.dir_root));
    }
}

// Global DB instance placeholder
thread_local! {
    pub static GLOBAL_DB: RefCell<Option<DB>> = RefCell::new(None);
}

pub fn init_db() {
    GLOBAL_DB.with(|db| {
        *db.borrow_mut() = Some(DB::new());
    });
}
