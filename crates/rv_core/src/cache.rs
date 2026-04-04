use std::fs::{self, File};
use std::io::{BufReader, BufWriter};
use std::rc::{Rc, Weak};
use std::cell::RefCell;
use crate::rv_file::RvFile;
use bincode;
use memmap2::MmapOptions;

/// Cache management system for serializing and deserializing the RomVault database.
/// 
/// The `Cache` struct is responsible for saving the entire state of the file tree
/// (`DB::dir_root`) to a local binary file, allowing instantaneous startup times
/// without having to re-parse all DATs and re-scan the entire physical disk.
/// 
/// Differences from C#:
/// - The C# reference uses a highly optimized, manually packed `BinaryWriter`/`BinaryReader`
///   stream implementation (`DB.Write` / `DB.Read`). It manually walks the tree and packs 
///   enums into bit-fields.
/// - The Rust implementation delegates serialization entirely to the `serde` framework
///   via the `bincode` format, which offers near-native performance automatically. 
/// - Rust utilizes `memmap2` for zero-copy memory-mapped file loading during cache reads,
///   massively accelerating deserialization of large trees compared to standard buffered I/O.
pub struct Cache;

impl Cache {
    const CACHE_FILE: &'static str = "RustyVault3_3.Cache";
    const BACKUP_FILE: &'static str = "RustyVault3_3.CacheBackup";
    const TMP_FILE: &'static str = "RustyVault3_3.Cache_tmp";

    pub fn cache_path() -> std::path::PathBuf {
        let (cache_path, _backup_path, _tmp_path) = Self::cache_paths();
        cache_path
    }

    fn cache_paths() -> (std::path::PathBuf, std::path::PathBuf, std::path::PathBuf) {
        let settings = crate::settings::get_settings();
        let raw = settings.cache_file.trim();

        let mut cache_path = if raw.is_empty() {
            std::path::PathBuf::from(Self::CACHE_FILE)
        } else if raw.ends_with(['\\', '/']) {
            std::path::PathBuf::from(raw).join(Self::CACHE_FILE)
        } else {
            std::path::PathBuf::from(raw)
        };

        if cache_path.file_name().is_none() {
            cache_path = cache_path.join(Self::CACHE_FILE);
        }

        if raw.is_empty() {
            return (
                std::path::PathBuf::from(Self::CACHE_FILE),
                std::path::PathBuf::from(Self::BACKUP_FILE),
                std::path::PathBuf::from(Self::TMP_FILE),
            );
        }

        let parent = cache_path.parent().unwrap_or_else(|| std::path::Path::new(""));
        let file_name = cache_path
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or(Self::CACHE_FILE);

        let backup = parent.join(format!("{file_name}Backup"));
        let tmp = parent.join(format!("{file_name}_tmp"));
        (cache_path, backup, tmp)
    }

    /// Reads the binary cache file from disk and deserializes it into the `dir_root` tree.
    /// 
    /// If memory mapping (`mmap`) is available, it performs a zero-copy read. Otherwise,
    /// it falls back to a standard buffered reader. After deserialization, it invokes
    /// `relink_parents` to reconstruct the `Weak` pointer tree hierarchy that `serde` skips.
    pub fn read_cache() -> Option<Rc<RefCell<RvFile>>> {
        let (cache_path, _backup_path, _tmp_path) = Self::cache_paths();
        let path = cache_path.as_path();
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

    /// Serializes the entire `RvFile` tree back to disk using `bincode` into the standard `RustyVault3_3.Cache` file format.
    pub fn write_cache(root: Rc<RefCell<RvFile>>) {
        Self::prepare_for_serialize(Rc::clone(&root));
        
        let (cache_path, backup_path, tmp_path) = Self::cache_paths();
        if tmp_path.exists() {
            let _ = fs::remove_file(&tmp_path);
        }

        if let Some(parent) = cache_path.parent() {
            if !parent.as_os_str().is_empty() {
                let _ = fs::create_dir_all(parent);
            }
        }

        let file = match File::create(&tmp_path) {
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

        if cache_path.exists() {
            if backup_path.exists() {
                let _ = fs::remove_file(&backup_path);
            }
            let _ = fs::rename(&cache_path, &backup_path);
        }

        let _ = fs::rename(&tmp_path, &cache_path);
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
            n.parent = p.clone();
            
            // Fixup: any item in ToSort should have a datStatus of InToSort
            if let Some(parent_weak) = &p {
                if let Some(parent_rc) = parent_weak.upgrade() {
                    if parent_rc.borrow().dat_status() == dat_reader::enums::DatStatus::InToSort {
                        n.set_dat_status(dat_reader::enums::DatStatus::InToSort);
                    }
                }
            }
            
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
#[path = "tests/cache_tests.rs"]
mod tests;
