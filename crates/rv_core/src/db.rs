use std::fs;
use std::path::Path;
use std::rc::Rc;
use std::cell::RefCell;
use dat_reader::enums::{DatStatus, FileType};
use crate::rv_file::RvFile;
use crate::enums::ToSortDirType;

use crate::cache::Cache;

/// Represents the global database state for the RomVault core.
/// 
/// In the C# reference, this is managed as a static `DB` class containing the `dirTree`.
/// In Rust, this is managed as a `thread_local!` instance of `DB` holding the root node of the file tree.
/// The `dir_root` is a hierarchical tree of `RvFile` nodes representing physical and virtual (DAT) directories.
pub struct DB {
    /// The root node of the internal file tree.
    pub dir_root: Rc<RefCell<RvFile>>,
}

impl DB {
    fn ensure_root_dir(logical_name: &str, fallback_path: &str) {
        let resolved_path = crate::settings::find_dir_mapping(logical_name)
            .unwrap_or_else(|| fallback_path.to_string());
        if !Path::new(&resolved_path).exists() {
            let _ = fs::create_dir_all(&resolved_path);
        }
    }

    /// Initializes a new database instance.
    /// 
    /// If a valid cache file exists (`RomVault.db`), it loads the tree from disk.
    /// Otherwise, it initializes a fresh tree with default `RustyVault` and `ToSort` directories.
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
            let rv_rc = Rc::new(RefCell::new(rv));
            rv_rc.borrow_mut().parent = Some(Rc::downgrade(&root));
            root_mut.child_add(Rc::clone(&rv_rc));
            
            let mut ts = RvFile::new(FileType::Dir);
            ts.name = "ToSort".to_string();
            ts.set_dat_status(DatStatus::InToSort);
            ts.to_sort_status_set(ToSortDirType::TO_SORT_PRIMARY | ToSortDirType::TO_SORT_CACHE);
            let ts_rc = Rc::new(RefCell::new(ts));
            ts_rc.borrow_mut().parent = Some(Rc::downgrade(&root));
            root_mut.child_add(Rc::clone(&ts_rc));
        }

        Self::check_create_root_dirs();

        Self {
            dir_root: root,
        }
    }

    /// Checks for and creates essential physical root directories (`DatRoot`, `RustyVault`, `ToSort`).
    /// 
    /// This mimics the C# initialization behavior where physical paths are generated
    /// based on the global configuration upon starting up the DB.
    pub fn check_create_root_dirs() {
        // Create DatRoot
        let dat_root = crate::settings::get_settings().dat_root;
        let dat_root_path = if dat_root.is_empty() { "DatRoot" } else { &dat_root };
        if !Path::new(dat_root_path).exists() {
            let _ = fs::create_dir_all(dat_root_path);
        }

        Self::ensure_root_dir("RustyVault", "RustyVault");
        Self::ensure_root_dir("ToSort", "ToSort");
    }

    /// Retrieves the designated cache directory for sorting operations.
    /// Used heavily by extraction/fixing routines to identify temporary workspaces.
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

    /// Retrieves the primary `ToSort` directory.
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

    /// Retrieves the file-only `ToSort` directory variant, or falls back to primary.
    pub fn get_to_sort_file_only(&self) -> Rc<RefCell<RvFile>> {
        let root = self.dir_root.borrow();
        for child in root.children.iter().skip(1) {
            if child.borrow().to_sort_status_is(ToSortDirType::TO_SORT_FILE_ONLY) {
                return Rc::clone(child);
            }
        }

        self.get_to_sort_primary()
    }

    /// Serializes the entire `dir_root` tree to disk via `cache::Cache::write_cache`.
    pub fn write_cache(&self) {
        Cache::write_cache(Rc::clone(&self.dir_root));
    }
}

impl Default for DB {
    fn default() -> Self {
        Self::new()
    }
}

thread_local! {
    /// Global, thread-local database instance. 
    /// Mimics the C# static `DB` class structure while abiding by Rust's safety guarantees.
    pub static GLOBAL_DB: RefCell<Option<DB>> = const { RefCell::new(None) };
}

/// Initializes the global DB if not already initialized.
pub fn init_db() {
    GLOBAL_DB.with(|db| {
        *db.borrow_mut() = Some(DB::new());
    });
}

#[cfg(test)]
#[path = "tests/db_tests.rs"]
mod tests;
