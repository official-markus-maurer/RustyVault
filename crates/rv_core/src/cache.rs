use crate::enums::ToSortDirType;
use crate::rv_file::RvFile;
use bincode;
use crc32fast::Hasher;
use memmap2::MmapOptions;
use std::cell::RefCell;
use std::fs::{self, File};
use std::io::{BufReader, BufWriter, Read, Write};
use std::rc::{Rc, Weak};
use std::sync::{mpsc, OnceLock};
use std::thread;

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

struct CSharpCacheReader<'a> {
    bytes: &'a [u8],
    pos: usize,
}

impl<'a> CSharpCacheReader<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, pos: 0 }
    }

    fn remaining(&self) -> usize {
        self.bytes.len().saturating_sub(self.pos)
    }

    fn read_exact(&mut self, n: usize) -> Result<&'a [u8], String> {
        let end = self
            .pos
            .checked_add(n)
            .ok_or_else(|| "csharp cache read overflow".to_string())?;
        if end > self.bytes.len() {
            return Err("truncated csharp cache".to_string());
        }
        let s = &self.bytes[self.pos..end];
        self.pos = end;
        Ok(s)
    }

    fn read_u8(&mut self) -> Result<u8, String> {
        Ok(self.read_exact(1)?[0])
    }

    fn read_bool(&mut self) -> Result<bool, String> {
        Ok(self.read_u8()? != 0)
    }

    fn read_u32_le(&mut self) -> Result<u32, String> {
        let b = self.read_exact(4)?;
        Ok(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }

    fn read_i32_le(&mut self) -> Result<i32, String> {
        let b = self.read_exact(4)?;
        Ok(i32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }

    fn read_u64_le(&mut self) -> Result<u64, String> {
        let b = self.read_exact(8)?;
        Ok(u64::from_le_bytes([
            b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7],
        ]))
    }

    fn read_i64_le(&mut self) -> Result<i64, String> {
        let b = self.read_exact(8)?;
        Ok(i64::from_le_bytes([
            b[0], b[1], b[2], b[3], b[4], b[5], b[6], b[7],
        ]))
    }

    fn read_7bit_encoded_i32(&mut self) -> Result<u32, String> {
        let mut count: u32 = 0;
        let mut shift: u32 = 0;
        loop {
            if shift >= 35 {
                return Err("invalid 7-bit encoded int".to_string());
            }
            let b = self.read_u8()? as u32;
            count |= (b & 0x7f) << shift;
            if (b & 0x80) == 0 {
                return Ok(count);
            }
            shift += 7;
        }
    }

    fn read_dotnet_string(&mut self) -> Result<String, String> {
        let byte_len = self.read_7bit_encoded_i32()? as usize;
        let b = self.read_exact(byte_len)?;
        Ok(String::from_utf8_lossy(b).into_owned())
    }

    fn read_byte_array_u8len(&mut self) -> Result<Vec<u8>, String> {
        let len = self.read_u8()? as usize;
        Ok(self.read_exact(len)?.to_vec())
    }
}

struct CacheWriteJob {
    cache_path: std::path::PathBuf,
    backup_path: std::path::PathBuf,
    tmp_path: std::path::PathBuf,
    encoding: u8,
    flags: u8,
    payload: Vec<u8>,
    payload_crc32: u32,
    done: Option<mpsc::Sender<bool>>,
}

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
    const CACHE_FLAG_ZSTD: u8 = 1;
    const CACHE_ZSTD_LEVEL: i32 = 3;
    const CACHE_ZSTD_MIN_BYTES: usize = 1024 * 1024;
    const CSHARP_END_MARKER: u64 = 0x15a600dda7;

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

    fn writer_tx() -> &'static mpsc::Sender<CacheWriteJob> {
        static TX: OnceLock<mpsc::Sender<CacheWriteJob>> = OnceLock::new();
        TX.get_or_init(|| {
            let (tx, rx) = mpsc::channel::<CacheWriteJob>();
            thread::spawn(move || Self::writer_loop(rx));
            tx
        })
    }

    fn writer_loop(rx: mpsc::Receiver<CacheWriteJob>) {
        while let Ok(mut job) = rx.recv() {
            let mut pending_with_waiters = Vec::new();
            let mut coalesced: Option<CacheWriteJob> = None;
            while let Ok(next) = rx.try_recv() {
                if next.done.is_some() {
                    pending_with_waiters.push(next);
                } else {
                    coalesced = Some(next);
                }
            }

            let ok = Self::write_job(&job);
            if let Some(done) = job.done.take() {
                let _ = done.send(ok);
            }

            for mut w in pending_with_waiters {
                let ok = Self::write_job(&w);
                if let Some(done) = w.done.take() {
                    let _ = done.send(ok);
                }
            }

            if let Some(last) = coalesced {
                let _ = Self::write_job(&last);
            }
        }
    }

    fn write_job(job: &CacheWriteJob) -> bool {
        if job.tmp_path.exists() {
            let _ = fs::remove_file(&job.tmp_path);
        }
        if let Some(parent) = job.cache_path.parent() {
            if !parent.as_os_str().is_empty() {
                let _ = fs::create_dir_all(parent);
            }
        }

        let file = match File::create(&job.tmp_path) {
            Ok(f) => f,
            Err(_) => return false,
        };

        {
            let mut writer = BufWriter::with_capacity(16 * 1024 * 1024, file);
            if writer.write_all(Self::CACHE_MAGIC).is_err()
                || writer
                    .write_all(&Self::CACHE_VERSION.to_le_bytes())
                    .is_err()
                || writer.write_all(&[job.encoding]).is_err()
                || writer.write_all(&[job.flags]).is_err()
                || writer
                    .write_all(&(job.payload.len() as u64).to_le_bytes())
                    .is_err()
                || writer.write_all(&job.payload_crc32.to_le_bytes()).is_err()
                || writer.write_all(&job.payload).is_err()
                || writer.flush().is_err()
            {
                return false;
            }

            if writer.get_ref().sync_all().is_err() {
                return false;
            }
        }

        if Self::validate_cache_file(&job.tmp_path).is_err() {
            let _ = fs::remove_file(&job.tmp_path);
            return false;
        }

        if job.cache_path.exists() {
            if job.backup_path.exists() {
                let _ = fs::remove_file(&job.backup_path);
            }
            let _ = fs::rename(&job.cache_path, &job.backup_path);
        }

        if fs::rename(&job.tmp_path, &job.cache_path).is_err() {
            if fs::copy(&job.tmp_path, &job.cache_path).is_err() {
                let _ = fs::remove_file(&job.tmp_path);
                return false;
            }
            let _ = fs::remove_file(&job.tmp_path);
        }

        true
    }

    fn encode_root_payload(
        root: &Rc<RefCell<RvFile>>,
        config: bincode::config::Configuration,
    ) -> Result<(Vec<u8>, u8), String> {
        let payload =
            bincode::serde::encode_to_vec(root, config).map_err(|e| format!("{:?}", e))?;
        if payload.len() < Self::CACHE_ZSTD_MIN_BYTES {
            return Ok((payload, Self::CACHE_FLAGS_NONE));
        }
        let compressed =
            zstd::stream::encode_all(std::io::Cursor::new(&payload), Self::CACHE_ZSTD_LEVEL)
                .map_err(|e| format!("{:?}", e))?;
        if compressed.len() < payload.len() {
            Ok((compressed, Self::CACHE_FLAG_ZSTD))
        } else {
            Ok((payload, Self::CACHE_FLAGS_NONE))
        }
    }

    fn decode_root_from_payload(
        payload: &[u8],
        flags: u8,
        config: bincode::config::Configuration,
    ) -> Result<Rc<RefCell<RvFile>>, String> {
        if flags & Self::CACHE_FLAG_ZSTD != 0 {
            let decompressed = zstd::stream::decode_all(std::io::Cursor::new(payload))
                .map_err(|e| format!("{:?}", e))?;
            Self::decode_root_from_bytes(&decompressed, config)
        } else {
            Self::decode_root_from_bytes(payload, config)
        }
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
    pub fn write_cache(root: Rc<RefCell<RvFile>>) -> bool {
        if !root.borrow().cache_dirty {
            return false;
        }

        let (cache_path, backup_path, tmp_path) = Self::cache_paths();

        let config = match Self::CACHE_WRITE_ENCODING {
            Self::CACHE_ENCODING_VARINT => bincode::config::standard().with_variable_int_encoding(),
            Self::CACHE_ENCODING_FIXED => bincode::config::standard(),
            _ => bincode::config::standard(),
        };

        let (payload, flags) = match Self::encode_root_payload(&root, config) {
            Ok(v) => v,
            Err(_) => return false,
        };

        let mut hasher = Hasher::new();
        hasher.update(&payload);
        let payload_crc32 = hasher.finalize();

        let (done_tx, done_rx) = mpsc::channel();
        let job = CacheWriteJob {
            cache_path,
            backup_path,
            tmp_path,
            encoding: Self::CACHE_WRITE_ENCODING,
            flags,
            payload,
            payload_crc32,
            done: Some(done_tx),
        };

        if Self::writer_tx().send(job).is_err() {
            return false;
        }

        let ok = done_rx.recv().unwrap_or(false);
        if ok {
            root.borrow_mut().cache_dirty = false;
        }
        ok
    }

    pub fn enqueue_write_cache(root: Rc<RefCell<RvFile>>) -> bool {
        if !root.borrow().cache_dirty {
            return false;
        }

        let (cache_path, backup_path, tmp_path) = Self::cache_paths();
        let config = match Self::CACHE_WRITE_ENCODING {
            Self::CACHE_ENCODING_VARINT => bincode::config::standard().with_variable_int_encoding(),
            Self::CACHE_ENCODING_FIXED => bincode::config::standard(),
            _ => bincode::config::standard(),
        };

        let (payload, flags) = match Self::encode_root_payload(&root, config) {
            Ok(v) => v,
            Err(_) => return false,
        };

        let mut hasher = Hasher::new();
        hasher.update(&payload);
        let payload_crc32 = hasher.finalize();

        let job = CacheWriteJob {
            cache_path,
            backup_path,
            tmp_path,
            encoding: Self::CACHE_WRITE_ENCODING,
            flags,
            payload,
            payload_crc32,
            done: None,
        };

        if Self::writer_tx().send(job).is_err() {
            return false;
        }

        root.borrow_mut().cache_dirty = false;
        true
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

    fn parse_cache_v2_meta(bytes: &[u8]) -> Option<(u8, u64, u32)> {
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
        let flags = bytes[Self::CACHE_MAGIC.len() + 4 + 1];
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
        Some((flags, payload_len, payload_crc))
    }

    fn validate_cache_file(path: &std::path::Path) -> Result<(), String> {
        let file = File::open(path).map_err(|e| format!("{:?}", e))?;
        let mmap = unsafe { MmapOptions::new().map(&file) }.map_err(|e| format!("{:?}", e))?;
        let Some((offset, version, encoding)) = Self::parse_cache_header(&mmap) else {
            return Err("invalid cache header".to_string());
        };

        if version == Self::CACHE_VERSION {
            let Some((flags, payload_len, payload_crc32)) = Self::parse_cache_v2_meta(&mmap) else {
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
            let config_varint = bincode::config::standard().with_variable_int_encoding();
            let config_fixed = bincode::config::standard();
            let config = match encoding {
                Self::CACHE_ENCODING_VARINT => config_varint,
                Self::CACHE_ENCODING_FIXED => config_fixed,
                _ => return Err(format!("unsupported cache encoding {encoding}")),
            };
            let payload = &mmap[offset..end];
            let _ = Self::decode_root_from_payload(payload, flags, config)?;
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
                    if version == Self::CACHE_VERSION {
                        let Some((flags, payload_len, _)) = Self::parse_cache_v2_meta(&mmap) else {
                            return Err("invalid v2 cache meta".to_string());
                        };
                        let end = offset
                            .checked_add(payload_len as usize)
                            .ok_or_else(|| "invalid v2 cache length".to_string())?;
                        if end > mmap.len() {
                            return Err("truncated v2 cache payload".to_string());
                        }
                        let payload = &mmap[offset..end];
                        let r = Self::decode_root_from_payload(payload, flags, config)?;
                        println!("Deserialized cache via mmap in {:?}", start_time.elapsed());
                        let relink_start = std::time::Instant::now();
                        Self::relink_parents(Rc::clone(&r), None, None);
                        println!("Relinked parents in {:?}", relink_start.elapsed());
                        return Ok((r, version));
                    }

                    let payload = &mmap[offset..];
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
                if let Ok(r) = Self::decode_csharp_cache_from_bytes(&mmap) {
                    println!(
                        "Deserialized C# cache via mmap in {:?}",
                        start_time.elapsed()
                    );
                    let relink_start = std::time::Instant::now();
                    Self::relink_parents(Rc::clone(&r), None, None);
                    println!("Relinked parents in {:?}", relink_start.elapsed());
                    return Ok((r, 0));
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
                    let flags = extra[0];
                    let payload_len = u64::from_le_bytes(
                        extra[1..9]
                            .try_into()
                            .map_err(|_| "invalid v2 cache length".to_string())?,
                    );
                    let payload_crc32 = u32::from_le_bytes(
                        extra[9..13]
                            .try_into()
                            .map_err(|_| "invalid v2 cache checksum".to_string())?,
                    );
                    let mut payload = vec![0u8; payload_len as usize];
                    reader
                        .read_exact(&mut payload)
                        .map_err(|_| "truncated v2 cache payload".to_string())?;

                    let mut hasher = Hasher::new();
                    hasher.update(&payload);
                    let actual = hasher.finalize();
                    if actual != payload_crc32 {
                        return Err(format!(
                            "v2 cache checksum mismatch (expected {payload_crc32:08X}, got {actual:08X})"
                        ));
                    }

                    let r = Self::decode_root_from_payload(&payload, flags, config)?;
                    println!("Deserialized cache in {:?}", start_time.elapsed());
                    let relink_start = std::time::Instant::now();
                    Self::relink_parents(Rc::clone(&r), None, None);
                    println!("Relinked parents in {:?}", relink_start.elapsed());
                    return Ok((r, version));
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
            if let Ok(r) = Self::decode_csharp_cache_from_reader(&mut reader) {
                println!("Deserialized C# cache in {:?}", start_time.elapsed());
                let relink_start = std::time::Instant::now();
                Self::relink_parents(Rc::clone(&r), None, None);
                println!("Relinked parents in {:?}", relink_start.elapsed());
                return Ok((r, 0));
            }
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

    fn relink_parents(
        root: Rc<RefCell<RvFile>>,
        parent: Option<Weak<RefCell<RvFile>>>,
        parent_dir_dats: Option<Vec<Rc<RefCell<crate::rv_dat::RvDat>>>>,
    ) {
        let mut stack = vec![(Rc::clone(&root), parent, parent_dir_dats)];

        while let Some((node, p, p_dats)) = stack.pop() {
            let mut n = node.borrow_mut();
            n.parent = p.clone();
            n.cache_dirty = false;

            // Fixup: any item in ToSort should have a datStatus of InToSort
            if let Some(parent_weak) = &p {
                if let Some(parent_rc) = parent_weak.upgrade() {
                    if parent_rc.borrow().dat_status() == dat_reader::enums::DatStatus::InToSort {
                        n.dat_status = dat_reader::enums::DatStatus::InToSort;
                    }
                }
            }
            if n.to_sort_type.intersects(
                ToSortDirType::TO_SORT_PRIMARY
                    | ToSortDirType::TO_SORT_CACHE
                    | ToSortDirType::TO_SORT_FILE_ONLY,
            ) {
                n.dat_status = dat_reader::enums::DatStatus::InToSort;
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

    fn decode_csharp_cache_from_reader<R: std::io::Read>(
        reader: &mut R,
    ) -> Result<Rc<RefCell<RvFile>>, String> {
        let mut bytes = Vec::new();
        reader
            .read_to_end(&mut bytes)
            .map_err(|_| "failed to read csharp cache bytes".to_string())?;
        Self::decode_csharp_cache_from_bytes(&bytes)
    }

    fn decode_csharp_cache_from_bytes(bytes: &[u8]) -> Result<Rc<RefCell<RvFile>>, String> {
        if bytes.len() < 4 + 8 {
            return Err("truncated csharp cache".to_string());
        }
        let version = i32::from_le_bytes(bytes[0..4].try_into().unwrap());
        if version != 2 && version != 3 {
            return Err("not a csharp cache".to_string());
        }
        let marker = u64::from_le_bytes(bytes[bytes.len() - 8..].try_into().unwrap());
        if marker != Self::CSHARP_END_MARKER {
            return Err("csharp cache missing end marker".to_string());
        }

        let mut r = CSharpCacheReader::new(bytes);
        let version = r.read_i32_le()?;
        if version != 2 && version != 3 {
            return Err("unsupported csharp cache version".to_string());
        }

        let root = Self::read_csharp_rvfile(&mut r, version, true, false)?;
        if r.remaining() < 8 {
            return Err("csharp cache truncated after root".to_string());
        }
        let eof = r.read_u64_le()?;
        if eof != Self::CSHARP_END_MARKER {
            return Err("csharp cache invalid end marker".to_string());
        }

        Self::csharp_update_fix_tosort_status(Rc::clone(&root));
        Ok(root)
    }

    fn csharp_update_fix_tosort_status(root: Rc<RefCell<RvFile>>) {
        let children = root.borrow().children.clone();
        let want = ToSortDirType::TO_SORT_PRIMARY | ToSortDirType::TO_SORT_CACHE;
        if children.iter().any(|c| c.borrow().to_sort_status_is(want)) {
            return;
        }
        const OLD_PRIMARY: u32 = 1 << 30;
        const OLD_CACHE: u32 = 1 << 31;
        for child in children {
            let mut c = child.borrow_mut();
            let bits = c.file_status.bits();
            if bits & OLD_PRIMARY != 0 {
                c.to_sort_type.insert(ToSortDirType::TO_SORT_PRIMARY);
            }
            if bits & OLD_CACHE != 0 {
                c.to_sort_type.insert(ToSortDirType::TO_SORT_CACHE);
            }
            let cleared = bits & !(OLD_PRIMARY | OLD_CACHE);
            c.file_status = crate::rv_file::FileStatus::from_bits_retain(cleared);
        }
    }

    fn game_data_from_u8(id: u8) -> Option<crate::rv_game::GameData> {
        use crate::rv_game::GameData;
        Some(match id {
            11 => GameData::Id,
            1 => GameData::Description,
            2 => GameData::RomOf,
            3 => GameData::IsBios,
            4 => GameData::Sourcefile,
            5 => GameData::CloneOf,
            24 => GameData::CloneOfId,
            6 => GameData::SampleOf,
            7 => GameData::Board,
            8 => GameData::Year,
            9 => GameData::Manufacturer,
            10 => GameData::EmuArc,
            12 => GameData::Publisher,
            13 => GameData::Developer,
            14 => GameData::Genre,
            15 => GameData::SubGenre,
            16 => GameData::Ratings,
            17 => GameData::Score,
            18 => GameData::Players,
            19 => GameData::Enabled,
            20 => GameData::CRC,
            21 => GameData::RelatedTo,
            22 => GameData::Source,
            23 => GameData::Category,
            _ => return None,
        })
    }

    fn dat_data_from_u8(id: u8) -> Option<crate::rv_dat::DatData> {
        use crate::rv_dat::DatData;
        Some(match id {
            0 => DatData::Id,
            1 => DatData::DatName,
            2 => DatData::DatRootFullName,
            3 => DatData::RootDir,
            4 => DatData::Description,
            5 => DatData::Category,
            6 => DatData::Version,
            7 => DatData::Date,
            8 => DatData::Author,
            9 => DatData::Email,
            10 => DatData::HomePage,
            11 => DatData::Url,
            12 => DatData::FileType,
            13 => DatData::MergeType,
            14 => DatData::SuperDat,
            15 => DatData::DirSetup,
            16 => DatData::Header,
            17 => DatData::SubDirType,
            18 => DatData::Compression,
            _ => return None,
        })
    }

    fn read_csharp_rvfile(
        r: &mut CSharpCacheReader<'_>,
        version: i32,
        is_root: bool,
        force_in_to_sort: bool,
    ) -> Result<Rc<RefCell<RvFile>>, String> {
        use dat_reader::enums::{DatStatus, FileType, GotStatus, HeaderFileType, ZipStructure};

        let file_type = if is_root {
            FileType::Dir
        } else {
            let ft = r.read_u8()?;
            match ft {
                0 => FileType::UnSet,
                1 => FileType::Dir,
                2 => FileType::Zip,
                3 => FileType::SevenZip,
                4 => FileType::File,
                5 => FileType::FileZip,
                6 => FileType::FileSevenZip,
                100 => FileType::FileOnly,
                _ => FileType::UnSet,
            }
        };

        let flags = r.read_u32_le()?;

        let name = r.read_dotnet_string()?;
        let file_name = r.read_dotnet_string()?;
        let file_mod_time_stamp = r.read_i64_le()?;

        let has_dat = flags & (1 << 16) != 0;
        let dat_index = if has_dat {
            Some(r.read_i32_le()?)
        } else {
            None
        };

        let mut dat_status = match r.read_u8()? {
            0 => DatStatus::InDatCollect,
            1 => DatStatus::InDatMerged,
            2 => DatStatus::InDatNoDump,
            3 => DatStatus::NotInDat,
            4 => DatStatus::InToSort,
            5 => DatStatus::InDatMIA,
            _ => DatStatus::NotInDat,
        };
        if force_in_to_sort {
            dat_status = DatStatus::InToSort;
        }
        let got_status = match r.read_u8()? {
            0 => GotStatus::NotGot,
            1 => GotStatus::Got,
            2 => GotStatus::Corrupt,
            3 => GotStatus::FileLocked,
            _ => GotStatus::NotGot,
        };

        let is_compressed_dir = file_type == FileType::Zip || file_type == FileType::SevenZip;
        let mut zip_dat_struct: u8 = 0;
        if is_compressed_dir {
            if version >= 3 {
                zip_dat_struct = r.read_u8()?;
            } else if dat_status == DatStatus::InDatCollect {
                zip_dat_struct = match file_type {
                    FileType::SevenZip => ZipStructure::SevenZipSLZMA as u8,
                    _ => ZipStructure::ZipTrrnt as u8,
                };
            }
        }
        let mut zip_struct = ZipStructure::None;
        if is_compressed_dir {
            let zs = r.read_u8()?;
            zip_struct = match zs {
                0 => ZipStructure::None,
                1 => ZipStructure::ZipTrrnt,
                2 => ZipStructure::ZipTDC,
                4 => ZipStructure::SevenZipTrrnt,
                5 => ZipStructure::ZipZSTD,
                8 => ZipStructure::SevenZipSLZMA,
                9 => ZipStructure::SevenZipNLZMA,
                10 => ZipStructure::SevenZipSZSTD,
                11 => ZipStructure::SevenZipNZSTD,
                _ => ZipStructure::None,
            };
            if file_type == FileType::SevenZip && zip_struct == ZipStructure::ZipTrrnt {
                zip_struct = ZipStructure::SevenZipSLZMA;
            }
            if file_type == FileType::SevenZip
                && dat_status == DatStatus::InDatCollect
                && zip_dat_struct == ZipStructure::ZipTrrnt as u8
            {
                zip_dat_struct = ZipStructure::SevenZipSLZMA as u8;
            }
        }

        let has_tree = flags & (1 << 14) != 0;
        let has_game = flags & (1 << 15) != 0;
        let has_dir_dat = flags & (1 << 17) != 0;
        let has_children = flags & (1 << 18) != 0;

        let mut node = RvFile::new(file_type);
        node.name = name;
        node.file_name = file_name;
        node.file_mod_time_stamp = file_mod_time_stamp;
        node.dat_status = dat_status;
        node.got_status = got_status;
        node.zip_struct = zip_struct;
        node.zip_dat_struct = zip_dat_struct;
        if let Some(idx) = dat_index {
            node.dat_index_for_serde = Some(idx);
        }
        node.rep_status_reset();

        if has_tree {
            node.tree_expanded = r.read_bool()?;
            node.tree_checked = match r.read_u8()? {
                0 => crate::rv_file::TreeSelect::UnSelected,
                1 => crate::rv_file::TreeSelect::Selected,
                2 => crate::rv_file::TreeSelect::Locked,
                _ => crate::rv_file::TreeSelect::Selected,
            };
        }

        if has_game {
            let c = r.read_u8()? as usize;
            let mut game = crate::rv_game::RvGame::new();
            for _ in 0..c {
                let id = r.read_u8()?;
                let val = r.read_dotnet_string()?;
                if let Some(k) = Self::game_data_from_u8(id) {
                    game.add_data(k, &val);
                }
            }
            node.game = Some(Rc::new(RefCell::new(game)));
        }

        let mut dir_dats: Vec<Rc<RefCell<crate::rv_dat::RvDat>>> = Vec::new();
        if has_dir_dat {
            let count = r.read_i32_le()? as usize;
            for i in 0..count {
                let time_stamp = r.read_i64_le()?;
                let dat_flags = r.read_u8()?;
                let meta_count = r.read_u8()? as usize;
                let mut dat = crate::rv_dat::RvDat::new();
                dat.dat_index = i as i32;
                dat.time_stamp = time_stamp;
                dat.dat_flags = crate::rv_dat::DatFlags::from_bits_retain(dat_flags);
                for _ in 0..meta_count {
                    let id = r.read_u8()?;
                    let val = r.read_dotnet_string()?;
                    if let Some(k) = Self::dat_data_from_u8(id) {
                        dat.set_data(k, Some(val));
                    }
                }
                dir_dats.push(Rc::new(RefCell::new(dat)));
            }
            node.dir_dats = dir_dats.clone();
        }

        let mut children: Vec<Rc<RefCell<RvFile>>> = Vec::new();
        if has_children {
            let count = r.read_i32_le()? as usize;
            for i in 0..count {
                let child_force = force_in_to_sort || (is_root && i > 0);
                let child = Self::read_csharp_rvfile(r, version, false, child_force)?;
                children.push(child);
            }
        }
        node.children = children;

        let has_size = flags & (1 << 0) != 0;
        let has_crc = flags & (1 << 1) != 0;
        let has_sha1 = flags & (1 << 2) != 0;
        let has_md5 = flags & (1 << 3) != 0;
        let has_header_file_type = flags & (1 << 4) != 0;
        let has_alt_size = flags & (1 << 5) != 0;
        let has_alt_crc = flags & (1 << 6) != 0;
        let has_alt_sha1 = flags & (1 << 7) != 0;
        let has_alt_md5 = flags & (1 << 8) != 0;
        let has_merge = flags & (1 << 9) != 0;
        let has_status = flags & (1 << 10) != 0;
        let has_zip_file_index = flags & (1 << 11) != 0;
        let has_zip_file_header = flags & (1 << 12) != 0;
        let has_chd_version = flags & (1 << 13) != 0;
        let has_to_sort_status = flags & (1 << 19) != 0;

        if has_size {
            node.size = Some(r.read_u64_le()?);
        }
        if has_crc {
            node.crc = Some(r.read_byte_array_u8len()?);
        }
        if has_sha1 {
            node.sha1 = Some(r.read_byte_array_u8len()?);
        }
        if has_md5 {
            node.md5 = Some(r.read_byte_array_u8len()?);
        }
        if has_header_file_type {
            node.header_file_type = HeaderFileType::from_bits_retain(r.read_u8()?);
        }
        if has_alt_size {
            node.alt_size = Some(r.read_u64_le()?);
        }
        if has_alt_crc {
            node.alt_crc = Some(r.read_byte_array_u8len()?);
        }
        if has_alt_sha1 {
            node.alt_sha1 = Some(r.read_byte_array_u8len()?);
        }
        if has_alt_md5 {
            node.alt_md5 = Some(r.read_byte_array_u8len()?);
        }
        if has_merge {
            node.merge = r.read_dotnet_string()?;
        }
        if has_status {
            node.status = Some(r.read_dotnet_string()?);
        }
        if has_zip_file_index {
            let _ = r.read_i32_le()?;
        }
        if has_zip_file_header {
            node.local_header_offset = Some(r.read_u64_le()?);
        }
        if has_chd_version {
            node.chd_version = Some(r.read_i32_le()? as u32);
        }
        if has_to_sort_status {
            node.to_sort_type = ToSortDirType::from_bits_retain(r.read_u8()?);
        }
        let file_status_bits = r.read_u32_le()?;
        node.file_status = crate::rv_file::FileStatus::from_bits_retain(file_status_bits);

        Ok(Rc::new(RefCell::new(node)))
    }
}

#[cfg(test)]
#[path = "tests/cache_tests.rs"]
mod tests;
