use crate::enums::ToSortDirType;
use crate::rv_file::RvFile;
use bincode;
use crc32fast::Hasher;
use memmap2::MmapOptions;
use std::cell::RefCell;
use std::fs::{self, File};
use std::io::{BufReader, BufWriter, Read, Seek, SeekFrom, Write};
use std::rc::{Rc, Weak};

/// Cache management system for serializing and deserializing the RomVault database.
///
/// The `Cache` struct is responsible for saving the entire state of the file tree
/// (`DB::dir_root`) to a local binary file, allowing instantaneous startup times
/// without having to re-parse all DATs and re-scan the entire physical disk.
///
/// Implementation notes:
/// - Serialization uses `bincode` (serde-based) for compact, fast binary encoding.
/// - Reads prefer memory-mapped I/O (`memmap2`) when possible to reduce copies.
/// - A small header (magic/version/encoding) is written to detect incompatible caches early.
pub struct Cache;

impl Cache {
    const CACHE_FILE: &'static str = "RustyVault3_3.Cache";
    const BACKUP_FILE: &'static str = "RustyVault3_3.CacheBackup";
    const TMP_FILE: &'static str = "RustyVault3_3.Cache_tmp";
    const CACHE_MAGIC: &'static [u8; 8] = b"RVDBIN\0\0";
    const CACHE_VERSION: u32 = 2;
    const CACHE_ENCODING_VARINT: u8 = 1;
    const CACHE_ENCODING_FIXED: u8 = 2;
    const CACHE_WRITE_ENCODING: u8 = Self::CACHE_ENCODING_VARINT;
    const CACHE_FLAGS_NONE: u8 = 0;

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
        // TODO(perf): cache read/write is full-tree serialization. Consider incremental updates or chunked storage to avoid
        // rewriting the entire DB on every change.
        // TODO(threading): move cache writes off the UI thread with a single background writer and atomic swap.
        // TODO(perf): consider compressing large portions of the serialized tree and/or using zstd frames per chunk.
        let (cache_path, backup_path, tmp_path) = Self::cache_paths();
        let candidates = [cache_path.as_path(), backup_path.as_path()];

        for path in candidates {
            if !path.exists() {
                continue;
            }
            match Self::read_cache_from_path(path) {
                Ok((r, version)) => {
                    if version != Self::CACHE_VERSION {
                        Self::write_cache(Rc::clone(&r));
                    }
                    return Some(r);
                }
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

        // TODO(perf): keep a dirty flag and avoid full cache writes when nothing changed.
        // TODO(perf): avoid writing cache twice per UI task (pre + post). Prefer a single atomic write when the task completes.
        // TODO(threading): move cache writes to a single background writer thread so tasks can enqueue "write requests"
        // without blocking scan/fix work.
        // TODO(perf): store cache as chunked data (e.g. per top-level root) to avoid serializing the entire tree on small changes.
        // TODO(perf): consider interning repeated strings (names/paths/status strings) to reduce serialized size and encode time.
        // TODO(perf): consider fixed-int encoding for bincode again, but only behind a bumped cache version + migration and/or
        // a strict write+read-back validation gate (already present below).
        // TODO(perf): avoid `prepare_for_serialize` walking the full tree on every write; maintain `dat_index_for_serde`
        // incrementally as nodes are mutated.
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

            let config = match Self::CACHE_WRITE_ENCODING {
                Self::CACHE_ENCODING_VARINT => {
                    bincode::config::standard().with_variable_int_encoding()
                }
                Self::CACHE_ENCODING_FIXED => bincode::config::standard(),
                _ => bincode::config::standard(),
            };

            let payload_len_offset = Self::CACHE_MAGIC.len() + 4 + 1 + 1;
            let payload_crc_offset = payload_len_offset + 8;

            if writer.write_all(Self::CACHE_MAGIC).is_err()
                || writer
                    .write_all(&Self::CACHE_VERSION.to_le_bytes())
                    .is_err()
                || writer.write_all(&[Self::CACHE_WRITE_ENCODING]).is_err()
                || writer.write_all(&[Self::CACHE_FLAGS_NONE]).is_err()
                || writer.write_all(&0u64.to_le_bytes()).is_err()
                || writer.write_all(&0u32.to_le_bytes()).is_err()
            {
                println!("Error writing cache: could not write header");
                return;
            }

            struct CountingCrcWriter<W: Write> {
                inner: W,
                hasher: Hasher,
                bytes_written: u64,
            }

            impl<W: Write> CountingCrcWriter<W> {
                fn new(inner: W) -> Self {
                    Self {
                        inner,
                        hasher: Hasher::new(),
                        bytes_written: 0,
                    }
                }
            }

            impl<W: Write> Write for CountingCrcWriter<W> {
                fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
                    let n = self.inner.write(buf)?;
                    if n > 0 {
                        self.hasher.update(&buf[..n]);
                        self.bytes_written = self.bytes_written.saturating_add(n as u64);
                    }
                    Ok(n)
                }

                fn flush(&mut self) -> std::io::Result<()> {
                    self.inner.flush()
                }
            }

            let mut payload_writer = CountingCrcWriter::new(writer);
            if let Err(e) =
                bincode::serde::encode_into_std_write(&root, &mut payload_writer, config)
            {
                println!("Error writing cache: {:?}", e);
                return;
            }

            if payload_writer.flush().is_err() {
                println!("Error writing cache: flush failed");
                return;
            }

            let payload_len = payload_writer.bytes_written;
            let payload_crc32 = payload_writer.hasher.finalize();

            let mut file = match payload_writer.inner.into_inner() {
                Ok(f) => f,
                Err(e) => {
                    println!("Error writing cache: finalize writer failed: {:?}", e);
                    return;
                }
            };

            if file
                .seek(SeekFrom::Start(payload_len_offset as u64))
                .is_err()
                || file.write_all(&payload_len.to_le_bytes()).is_err()
                || file
                    .seek(SeekFrom::Start(payload_crc_offset as u64))
                    .is_err()
                || file.write_all(&payload_crc32.to_le_bytes()).is_err()
            {
                println!("Error writing cache: could not finalize header");
                return;
            }

            if file.sync_all().is_err() {
                println!("Error writing cache: sync failed");
                return;
            }
        }

        if let Err(e) = Self::validate_cache_file(&tmp_path) {
            println!("Error writing cache: validation failed: {e}");
            let _ = fs::remove_file(&tmp_path);
            return;
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
        let base_len = Self::CACHE_MAGIC.len() + 4 + 1;
        if bytes.len() < base_len {
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
        if version == Self::CACHE_VERSION {
            let header_len = base_len + 1 + 8 + 4;
            if bytes.len() < header_len {
                return None;
            }
            Some((header_len, version, encoding))
        } else {
            Some((base_len, version, encoding))
        }
    }

    fn parse_cache_v2_meta(bytes: &[u8]) -> Option<(u64, u32)> {
        let header_len = Self::CACHE_MAGIC.len() + 4 + 1 + 1 + 8 + 4;
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
        if version != Self::CACHE_VERSION {
            return None;
        }
        let payload_len_offset = Self::CACHE_MAGIC.len() + 4 + 1 + 1;
        let payload_crc_offset = payload_len_offset + 8;
        let payload_len = u64::from_le_bytes(
            bytes[payload_len_offset..payload_len_offset + 8]
                .try_into()
                .ok()?,
        );
        let payload_crc = u32::from_le_bytes(
            bytes[payload_crc_offset..payload_crc_offset + 4]
                .try_into()
                .ok()?,
        );
        Some((payload_len, payload_crc))
    }

    fn validate_cache_file(path: &std::path::Path) -> Result<(), String> {
        let file = File::open(path).map_err(|e| format!("{:?}", e))?;
        let mmap = unsafe { MmapOptions::new().map(&file) }.map_err(|e| format!("{:?}", e))?;
        let Some((offset, version, encoding)) = Self::parse_cache_header(&mmap) else {
            return Err("invalid cache header".to_string());
        };

        if version == Self::CACHE_VERSION {
            let Some((payload_len, payload_crc32)) = Self::parse_cache_v2_meta(&mmap) else {
                return Err("invalid v2 cache meta".to_string());
            };
            let end = offset
                .checked_add(payload_len as usize)
                .ok_or_else(|| "invalid v2 cache length".to_string())?;
            if end > mmap.len() {
                return Err("truncated v2 cache payload".to_string());
            }
            let mut hasher = Hasher::new();
            hasher.update(&mmap[offset..end]);
            let actual = hasher.finalize();
            if actual != payload_crc32 {
                return Err(format!(
                    "v2 cache checksum mismatch (expected {payload_crc32:08X}, got {actual:08X})"
                ));
            }
            return Ok(());
        }

        let config_varint = bincode::config::standard().with_variable_int_encoding();
        let config_fixed = bincode::config::standard();
        let config = match encoding {
            Self::CACHE_ENCODING_VARINT => config_varint,
            Self::CACHE_ENCODING_FIXED => config_fixed,
            _ => return Err(format!("unsupported cache encoding {encoding}")),
        };

        let _ = Self::decode_root_from_bytes(&mmap[offset..], config)?;
        Ok(())
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

    fn read_cache_from_path(path: &std::path::Path) -> Result<(Rc<RefCell<RvFile>>, u32), String> {
        let config_varint = bincode::config::standard().with_variable_int_encoding();
        let config_fixed = bincode::config::standard();

        let start_time = std::time::Instant::now();

        let mut mmap_error: Option<String> = None;
        if let Ok(file) = File::open(path) {
            if let Ok(mmap) = unsafe { MmapOptions::new().map(&file) } {
                if let Some((offset, version, encoding)) = Self::parse_cache_header(&mmap) {
                    if version > Self::CACHE_VERSION {
                        return Err(format!("unsupported cache version {version}"));
                    }
                    let config = match encoding {
                        Self::CACHE_ENCODING_VARINT => config_varint,
                        Self::CACHE_ENCODING_FIXED => config_fixed,
                        _ => return Err(format!("unsupported cache encoding {encoding}")),
                    };
                    let payload = if version == Self::CACHE_VERSION {
                        if let Some((payload_len, _)) = Self::parse_cache_v2_meta(&mmap) {
                            let end = offset
                                .checked_add(payload_len as usize)
                                .ok_or_else(|| "invalid v2 cache length".to_string())?;
                            if end > mmap.len() {
                                return Err("truncated v2 cache payload".to_string());
                            }
                            &mmap[offset..end]
                        } else {
                            &mmap[offset..]
                        }
                    } else {
                        &mmap[offset..]
                    };
                    let r = Self::decode_root_from_bytes(payload, config)?;
                    println!("Deserialized cache via mmap in {:?}", start_time.elapsed());
                    let relink_start = std::time::Instant::now();
                    Self::relink_parents(Rc::clone(&r), None, None);
                    println!("Relinked parents in {:?}", relink_start.elapsed());
                    return Ok((r, version));
                }

                match Self::decode_root_from_bytes(&mmap, config_varint) {
                    Ok(r) => {
                        println!("Deserialized cache via mmap in {:?}", start_time.elapsed());
                        let relink_start = std::time::Instant::now();
                        Self::relink_parents(Rc::clone(&r), None, None);
                        println!("Relinked parents in {:?}", relink_start.elapsed());
                        return Ok((r, 0));
                    }
                    Err(e) => mmap_error = Some(e),
                }
                match Self::decode_root_from_bytes(&mmap, config_fixed) {
                    Ok(r) => {
                        println!("Deserialized cache via mmap in {:?}", start_time.elapsed());
                        let relink_start = std::time::Instant::now();
                        Self::relink_parents(Rc::clone(&r), None, None);
                        println!("Relinked parents in {:?}", relink_start.elapsed());
                        return Ok((r, 0));
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
            let mut base = [0u8; 13];
            if reader.read_exact(&mut base).is_ok()
                && &base[..Self::CACHE_MAGIC.len()] == Self::CACHE_MAGIC
            {
                let version = u32::from_le_bytes(
                    base[Self::CACHE_MAGIC.len()..Self::CACHE_MAGIC.len() + 4]
                        .try_into()
                        .unwrap(),
                );
                let encoding = base[Self::CACHE_MAGIC.len() + 4];
                if version > Self::CACHE_VERSION {
                    return Err(format!("unsupported cache version {version}"));
                }
                let config = match encoding {
                    Self::CACHE_ENCODING_VARINT => config_varint,
                    Self::CACHE_ENCODING_FIXED => config_fixed,
                    _ => return Err(format!("unsupported cache encoding {encoding}")),
                };
                if version == Self::CACHE_VERSION {
                    let mut extra = [0u8; 13];
                    if reader.read_exact(&mut extra).is_err() {
                        return Err("truncated v2 cache header".to_string());
                    }
                }
                let r = Self::decode_root_from_reader(&mut reader, config)?;
                println!("Deserialized cache in {:?}", start_time.elapsed());
                let relink_start = std::time::Instant::now();
                Self::relink_parents(Rc::clone(&r), None, None);
                println!("Relinked parents in {:?}", relink_start.elapsed());
                return Ok((r, version));
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
                    return Ok((r, 0));
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
                    return Ok((r, 0));
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
            if n.to_sort_type.intersects(
                ToSortDirType::TO_SORT_PRIMARY
                    | ToSortDirType::TO_SORT_CACHE
                    | ToSortDirType::TO_SORT_FILE_ONLY,
            ) {
                n.set_dat_status(dat_reader::enums::DatStatus::InToSort);
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
