use crate::rv_file::RvFile;
use bincode;
use memmap2::MmapOptions;
use std::cell::RefCell;
use std::fs::{self, File};
use std::io::{BufReader, BufWriter, Read, Write};
use std::rc::{Rc, Weak};

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
    const CACHE_MAGIC: &'static [u8; 8] = b"RVDBIN\0\0";
    const CACHE_VERSION: u32 = 1;
    const CACHE_ENCODING_VARINT: u8 = 1;
    const CACHE_ENCODING_FIXED: u8 = 2;

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

        let parent = cache_path
            .parent()
            .unwrap_or_else(|| std::path::Path::new(""));
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
        let (cache_path, backup_path, tmp_path) = Self::cache_paths();
        let candidates = [cache_path.as_path(), backup_path.as_path()];

        for path in candidates {
            if !path.exists() {
                continue;
            }
            match Self::read_cache_from_path(path) {
                Ok(r) => return Some(r),
                Err(e) => {
                    if e.contains("AnyNotSupported") {
                        Self::quarantine_cache_files(&cache_path, &backup_path, &tmp_path);
                        return None;
                    }
                }
            }
        }

        None
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

        {
            let file = match File::create(&tmp_path) {
                Ok(f) => f,
                Err(e) => {
                    println!("Error creating temp cache file: {:?}", e);
                    return;
                }
            };

            let mut writer = BufWriter::with_capacity(16 * 1024 * 1024, file);

            let config = bincode::config::standard().with_variable_int_encoding();

            if writer.write_all(Self::CACHE_MAGIC).is_err()
                || writer
                    .write_all(&Self::CACHE_VERSION.to_le_bytes())
                    .is_err()
                || writer.write_all(&[Self::CACHE_ENCODING_VARINT]).is_err()
            {
                println!("Error writing cache: could not write header");
                return;
            }

            if let Err(e) = bincode::serde::encode_into_std_write(&root, &mut writer, config) {
                println!("Error writing cache: {:?}", e);
                return;
            }

            if writer.flush().is_err() {
                println!("Error writing cache: flush failed");
                return;
            }
            if writer.get_ref().sync_all().is_err() {
                println!("Error writing cache: sync failed");
                return;
            }
        }

        if cache_path.exists() {
            if backup_path.exists() {
                let _ = fs::remove_file(&backup_path);
            }
            let _ = fs::rename(&cache_path, &backup_path);
        }

        if fs::rename(&tmp_path, &cache_path).is_err() {
            if fs::copy(&tmp_path, &cache_path).is_err() {
                let _ = fs::remove_file(&tmp_path);
                return;
            }
            let _ = fs::remove_file(&tmp_path);
        }
    }

    fn quarantine_cache_files(
        cache_path: &std::path::Path,
        backup_path: &std::path::Path,
        tmp_path: &std::path::Path,
    ) {
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .ok()
            .map(|d| d.as_secs())
            .unwrap_or(0);

        for path in [cache_path, backup_path, tmp_path] {
            if !path.exists() {
                continue;
            }
            let parent = path.parent().unwrap_or_else(|| std::path::Path::new(""));
            let name = path.file_name().and_then(|s| s.to_str()).unwrap_or("cache");
            let quarantined = parent.join(format!("{name}.bad.{unique}"));
            let _ = fs::rename(path, quarantined);
        }
    }

    fn parse_cache_header(bytes: &[u8]) -> Option<(usize, u32, u8)> {
        let header_len = Self::CACHE_MAGIC.len() + 4 + 1;
        if bytes.len() < header_len {
            return None;
        }
        if &bytes[..Self::CACHE_MAGIC.len()] != Self::CACHE_MAGIC {
            return None;
        }
        let version = u32::from_le_bytes(
            bytes[Self::CACHE_MAGIC.len()..Self::CACHE_MAGIC.len() + 4]
                .try_into()
                .ok()?,
        );
        let encoding = bytes[Self::CACHE_MAGIC.len() + 4];
        Some((header_len, version, encoding))
    }

    fn decode_root_from_bytes(
        bytes: &[u8],
        config: bincode::config::Configuration,
    ) -> Result<Rc<RefCell<RvFile>>, String> {
        bincode::serde::decode_from_slice(bytes, config)
            .map(|(r, _)| r)
            .map_err(|e| format!("{:?}", e))
    }

    fn decode_root_from_reader<R: std::io::Read>(
        reader: &mut R,
        config: bincode::config::Configuration,
    ) -> Result<Rc<RefCell<RvFile>>, String> {
        bincode::serde::decode_from_std_read(reader, config).map_err(|e| format!("{:?}", e))
    }

    fn read_cache_from_path(path: &std::path::Path) -> Result<Rc<RefCell<RvFile>>, String> {
        let config_varint = bincode::config::standard().with_variable_int_encoding();
        let config_fixed = bincode::config::standard();

        let start_time = std::time::Instant::now();

        let mut mmap_error: Option<String> = None;
        if let Ok(file) = File::open(path) {
            if let Ok(mmap) = unsafe { MmapOptions::new().map(&file) } {
                if let Some((offset, version, encoding)) = Self::parse_cache_header(&mmap) {
                    if version != Self::CACHE_VERSION {
                        return Err(format!("unsupported cache version {version}"));
                    }
                    let config = match encoding {
                        Self::CACHE_ENCODING_VARINT => config_varint,
                        Self::CACHE_ENCODING_FIXED => config_fixed,
                        _ => return Err(format!("unsupported cache encoding {encoding}")),
                    };
                    let r = Self::decode_root_from_bytes(&mmap[offset..], config)?;
                    println!("Deserialized cache via mmap in {:?}", start_time.elapsed());
                    let relink_start = std::time::Instant::now();
                    Self::relink_parents(Rc::clone(&r), None, None);
                    println!("Relinked parents in {:?}", relink_start.elapsed());
                    return Ok(r);
                }

                match Self::decode_root_from_bytes(&mmap, config_varint) {
                    Ok(r) => {
                        println!("Deserialized cache via mmap in {:?}", start_time.elapsed());
                        let relink_start = std::time::Instant::now();
                        Self::relink_parents(Rc::clone(&r), None, None);
                        println!("Relinked parents in {:?}", relink_start.elapsed());
                        return Ok(r);
                    }
                    Err(e) => mmap_error = Some(e),
                }
                match Self::decode_root_from_bytes(&mmap, config_fixed) {
                    Ok(r) => {
                        println!("Deserialized cache via mmap in {:?}", start_time.elapsed());
                        let relink_start = std::time::Instant::now();
                        Self::relink_parents(Rc::clone(&r), None, None);
                        println!("Relinked parents in {:?}", relink_start.elapsed());
                        return Ok(r);
                    }
                    Err(e) => {
                        mmap_error = Some(match mmap_error.take() {
                            Some(prev) => format!("{prev}; {e}"),
                            None => e,
                        });
                    }
                }
            }
        }

        let mut buffered_error: Option<String> = None;
        if let Ok(file) = File::open(path) {
            let mut reader = BufReader::with_capacity(16 * 1024 * 1024, file);
            let mut header = [0u8; 13];
            if reader.read_exact(&mut header).is_ok()
                && &header[..Self::CACHE_MAGIC.len()] == Self::CACHE_MAGIC
            {
                let version = u32::from_le_bytes(
                    header[Self::CACHE_MAGIC.len()..Self::CACHE_MAGIC.len() + 4]
                        .try_into()
                        .unwrap(),
                );
                let encoding = header[Self::CACHE_MAGIC.len() + 4];
                if version != Self::CACHE_VERSION {
                    return Err(format!("unsupported cache version {version}"));
                }
                let config = match encoding {
                    Self::CACHE_ENCODING_VARINT => config_varint,
                    Self::CACHE_ENCODING_FIXED => config_fixed,
                    _ => return Err(format!("unsupported cache encoding {encoding}")),
                };
                let r = Self::decode_root_from_reader(&mut reader, config)?;
                println!("Deserialized cache in {:?}", start_time.elapsed());
                let relink_start = std::time::Instant::now();
                Self::relink_parents(Rc::clone(&r), None, None);
                println!("Relinked parents in {:?}", relink_start.elapsed());
                return Ok(r);
            }
        }

        if let Ok(file) = File::open(path) {
            let mut reader = BufReader::with_capacity(16 * 1024 * 1024, file);
            match Self::decode_root_from_reader(&mut reader, config_varint) {
                Ok(r) => {
                    println!("Deserialized cache in {:?}", start_time.elapsed());
                    let relink_start = std::time::Instant::now();
                    Self::relink_parents(Rc::clone(&r), None, None);
                    println!("Relinked parents in {:?}", relink_start.elapsed());
                    return Ok(r);
                }
                Err(e) => buffered_error = Some(e),
            }
        }

        if let Ok(file) = File::open(path) {
            let mut reader = BufReader::with_capacity(16 * 1024 * 1024, file);
            match Self::decode_root_from_reader(&mut reader, config_fixed) {
                Ok(r) => {
                    println!("Deserialized cache in {:?}", start_time.elapsed());
                    let relink_start = std::time::Instant::now();
                    Self::relink_parents(Rc::clone(&r), None, None);
                    println!("Relinked parents in {:?}", relink_start.elapsed());
                    return Ok(r);
                }
                Err(e) => {
                    buffered_error = Some(match buffered_error.take() {
                        Some(prev) => format!("{prev}; {e}"),
                        None => e,
                    });
                }
            }
        }

        if let Some(e) = buffered_error.or(mmap_error) {
            return Err(e);
        }

        Err("cache read failed".to_string())
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

    fn relink_parents(
        root: Rc<RefCell<RvFile>>,
        parent: Option<Weak<RefCell<RvFile>>>,
        parent_dir_dats: Option<Vec<Rc<RefCell<crate::rv_dat::RvDat>>>>,
    ) {
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

            if (n.file_type == dat_reader::enums::FileType::Dir
                || n.file_type == dat_reader::enums::FileType::Zip
                || n.file_type == dat_reader::enums::FileType::SevenZip)
                && n.dir_status.is_none()
            {
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
                stack.push((
                    Rc::clone(child),
                    Some(weak_node.clone()),
                    current_dats.clone(),
                ));
            }
        }
    }
}

#[cfg(test)]
#[path = "tests/cache_tests.rs"]
mod tests;
