use std::fs::{self, File};
use std::io::{BufReader, BufWriter};
use std::path::Path;
use std::rc::{Rc, Weak};
use std::cell::RefCell;
use crate::rv_file::RvFile;
use bincode;
use memmap2::MmapOptions;

pub struct Cache;

impl Cache {
    const CACHE_FILE: &'static str = "RustyVault3_3.Cache";
    const BACKUP_FILE: &'static str = "RustyVault3_3.CacheBackup";
    const TMP_FILE: &'static str = "RustyVault3_3.Cache_tmp";

    pub fn read_cache() -> Option<Rc<RefCell<RvFile>>> {
        let path = Path::new(Self::CACHE_FILE);
        if !path.exists() {
            return None;
        }

        let file = match File::open(path) {
            Ok(f) => f,
            Err(_) => return None,
        };

        // Configure bincode
        let config = bincode::config::standard().with_variable_int_encoding();

        // Try a zero-copy memory-mapped read first for maximum throughput
        let start_time = std::time::Instant::now();
        let root: Rc<RefCell<RvFile>> = if let Ok(mmap) = unsafe { MmapOptions::new().map(&file) } {
            match bincode::serde::decode_from_slice(&mmap, config) {
                Ok((r, _bytes_read)) => {
                    println!("Deserialized cache via mmap in {:?}", start_time.elapsed());
                    r
                }
                Err(e) => {
                    println!("mmap decode failed ({:?}); falling back to buffered read", e);
                    let mut reader = BufReader::with_capacity(16 * 1024 * 1024, file);
                    match bincode::serde::decode_from_std_read(&mut reader, config) {
                        Ok(r) => {
                            println!("Deserialized cache in {:?}", start_time.elapsed());
                            r
                        }
                        Err(e) => {
                            println!("Error reading cache: {:?}", e);
                            return None;
                        }
                    }
                }
            }
        } else {
            let mut reader = BufReader::with_capacity(16 * 1024 * 1024, file);
            match bincode::serde::decode_from_std_read(&mut reader, config) {
                Ok(r) => {
                    println!("Deserialized cache in {:?}", start_time.elapsed());
                    r
                }
                Err(e) => {
                    println!("Error reading cache: {:?}", e);
                    return None;
                }
            }
        };

        let relink_start = std::time::Instant::now();
        // Post-deserialization: re-link the 'parent' Weak pointers and resolve dat references
        Self::relink_parents(Rc::clone(&root), None, None);
        println!("Relinked parents in {:?}", relink_start.elapsed());

        Some(root)
    }

    pub fn write_cache(root: Rc<RefCell<RvFile>>) {
        Self::prepare_for_serialize(Rc::clone(&root));
        
        if Path::new(Self::TMP_FILE).exists() {
            let _ = fs::remove_file(Self::TMP_FILE);
        }

        let file = match File::create(Self::TMP_FILE) {
            Ok(f) => f,
            Err(e) => {
                println!("Error creating temp cache file: {:?}", e);
                return;
            }
        };

        let mut writer = BufWriter::with_capacity(16 * 1024 * 1024, file);

        let config = bincode::config::standard().with_variable_int_encoding();

        if let Err(e) = bincode::serde::encode_into_std_write(&root, &mut writer, config) {
            println!("Error writing cache: {:?}", e);
            return;
        }

        if Path::new(Self::CACHE_FILE).exists() {
            if Path::new(Self::BACKUP_FILE).exists() {
                let _ = fs::remove_file(Self::BACKUP_FILE);
            }
            let _ = fs::rename(Self::CACHE_FILE, Self::BACKUP_FILE);
        }

        let _ = fs::rename(Self::TMP_FILE, Self::CACHE_FILE);
    }

    fn prepare_for_serialize(root: Rc<RefCell<RvFile>>) {
        let mut stack = vec![Rc::clone(&root)];
        while let Some(node) = stack.pop() {
            let mut n = node.borrow_mut();
            n.dat_index_for_serde = n.dat.as_ref().map(|d| d.borrow().dat_index);
            for child in &n.children {
                stack.push(Rc::clone(child));
            }
        }
    }

    fn relink_parents(root: Rc<RefCell<RvFile>>, parent: Option<Weak<RefCell<RvFile>>>, parent_dir_dats: Option<Vec<Rc<RefCell<crate::rv_dat::RvDat>>>>) {
        let mut stack = vec![(Rc::clone(&root), parent, parent_dir_dats)];
        
        while let Some((node, p, p_dats)) = stack.pop() {
            let mut n = node.borrow_mut();
            n.parent = p;
            
            if (n.file_type == dat_reader::enums::FileType::Dir || 
                n.file_type == dat_reader::enums::FileType::Zip || 
                n.file_type == dat_reader::enums::FileType::SevenZip) && 
                n.dir_status.is_none() {
                n.dir_status = Some(crate::enums::ReportStatus::Unknown);
            }
            
            // Resolve dat index
            if let Some(idx) = n.dat_index_for_serde {
                if let Some(ref dats) = p_dats {
                    if idx >= 0 && (idx as usize) < dats.len() {
                        n.dat = Some(Rc::clone(&dats[idx as usize]));
                    }
                }
            }
            
            let weak_node = Rc::downgrade(&node);
            let current_dats = if !n.dir_dats.is_empty() {
                Some(n.dir_dats.clone())
            } else {
                p_dats.clone()
            };
            
            for child in n.children.iter().rev() {
                stack.push((Rc::clone(child), Some(weak_node.clone()), current_dats.clone()));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use dat_reader::enums::FileType;

    #[test]
    fn test_cache_serialization_and_relinking() {
        let root = Rc::new(RefCell::new(RvFile::new(FileType::Dir)));
        root.borrow_mut().name = "Root".to_string();

        let child = Rc::new(RefCell::new(RvFile::new(FileType::File)));
        child.borrow_mut().name = "File1.zip".to_string();
        
        let dat = Rc::new(RefCell::new(crate::rv_dat::RvDat::new()));
        dat.borrow_mut().dat_index = 0;
        root.borrow_mut().dir_dats.push(Rc::clone(&dat));
        
        child.borrow_mut().dat = Some(Rc::clone(&dat));

        root.borrow_mut().child_add(Rc::clone(&child));

        // Prepare for serialization
        Cache::prepare_for_serialize(Rc::clone(&root));
        assert_eq!(child.borrow().dat_index_for_serde, Some(0));

        // Unlink explicitly to simulate raw deserialized state
        child.borrow_mut().parent = None;
        child.borrow_mut().dat = None;

        // Relink
        Cache::relink_parents(Rc::clone(&root), None, None);

        // Verify parent link restored
        assert!(child.borrow().parent.is_some());
        let p = child.borrow().parent.as_ref().unwrap().upgrade().unwrap();
        assert_eq!(p.borrow().name, "Root");

        // Verify Dat reference restored
        assert!(child.borrow().dat.is_some());
        assert_eq!(child.borrow().dat.as_ref().unwrap().borrow().dat_index, 0);
    }
}
