use std::cell::RefCell;
use std::fs::File;
use std::io::{Read, Write};
use std::rc::Rc;

use sevenz_rust::{Archive, ArchiveEntry, BlockDecoder, Password};

use crate::zip_enums::ZipReturn;

pub(crate) struct SevenZipPendingWrite {
    pub(crate) header_index: usize,
    pub(crate) file: Rc<RefCell<File>>,
    pub(crate) mod_time: Option<i64>,
}

pub(crate) struct SharedFileWriter {
    pub(crate) file: Rc<RefCell<File>>,
}

impl Write for SharedFileWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.file.borrow_mut().write(buf)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.file.borrow_mut().flush()
    }
}

fn logical_name_eq(left: &str, right: &str) -> bool {
    #[cfg(windows)]
    {
        left.eq_ignore_ascii_case(right)
    }
    #[cfg(not(windows))]
    {
        left == right
    }
}

pub fn extract_entry_bytes(
    archive_path: &str,
    entry_name: &str,
) -> Result<Option<Vec<u8>>, ZipReturn> {
    let mut file = File::open(archive_path).map_err(|_| ZipReturn::ZipErrorGettingDataStream)?;
    let password = Password::empty();
    let archive =
        Archive::read(&mut file, &password).map_err(|_| ZipReturn::ZipErrorGettingDataStream)?;

    let mut found: Option<Vec<u8>> = None;
    for block_index in 0..archive.blocks.len() {
        let decoder = BlockDecoder::new(1, block_index, &archive, &password, &mut file);
        let mut each =
            |entry: &ArchiveEntry, reader: &mut dyn Read| -> Result<bool, sevenz_rust::Error> {
                if found.is_some() {
                    let _ = std::io::copy(reader, &mut std::io::sink());
                    return Ok(false);
                }
                if logical_name_eq(entry.name(), entry_name) {
                    let mut buffer = Vec::new();
                    reader.read_to_end(&mut buffer)?;
                    found = Some(buffer);
                    return Ok(false);
                }
                let _ = std::io::copy(reader, &mut std::io::sink());
                Ok(true)
            };
        let _ = decoder
            .for_each_entries(&mut each)
            .map_err(|_| ZipReturn::ZipErrorGettingDataStream)?;
        if found.is_some() {
            break;
        }
    }

    Ok(found)
}

pub fn extract_entry_to_writer(
    archive_path: &str,
    entry_name: &str,
    out: &mut dyn Write,
) -> Result<bool, ZipReturn> {
    let mut file = File::open(archive_path).map_err(|_| ZipReturn::ZipErrorGettingDataStream)?;
    let password = Password::empty();
    let archive =
        Archive::read(&mut file, &password).map_err(|_| ZipReturn::ZipErrorGettingDataStream)?;

    let mut found = false;
    for block_index in 0..archive.blocks.len() {
        let decoder = BlockDecoder::new(1, block_index, &archive, &password, &mut file);
        let mut each =
            |entry: &ArchiveEntry, reader: &mut dyn Read| -> Result<bool, sevenz_rust::Error> {
                if found {
                    let _ = std::io::copy(reader, &mut std::io::sink());
                    return Ok(false);
                }
                if logical_name_eq(entry.name(), entry_name) {
                    std::io::copy(reader, out)?;
                    found = true;
                    return Ok(false);
                }
                let _ = std::io::copy(reader, &mut std::io::sink());
                Ok(true)
            };
        let _ = decoder
            .for_each_entries(&mut each)
            .map_err(|_| ZipReturn::ZipErrorGettingDataStream)?;
        if found {
            break;
        }
    }

    Ok(found)
}

pub fn extract_entry_to_path(
    archive_path: &str,
    entry_name: &str,
    out_path: &std::path::Path,
) -> Result<bool, ZipReturn> {
    if let Some(parent) = out_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let file =
        std::fs::File::create(out_path).map_err(|_| ZipReturn::ZipErrorWritingToOutputStream)?;
    let mut writer = std::io::BufWriter::new(file);
    extract_entry_to_writer(archive_path, entry_name, &mut writer)
}

pub fn seven_zip_dictionary_size_from_uncompressed_size(uncompressed_size: u64) -> u32 {
    const DICT_SIZES: [u32; 22] = [
        0x10000, 0x18000, 0x20000, 0x30000, 0x40000, 0x60000, 0x80000, 0xC0000, 0x100000, 0x180000,
        0x200000, 0x300000, 0x400000, 0x600000, 0x800000, 0xC00000, 0x1000000, 0x1800000,
        0x2000000, 0x3000000, 0x4000000, 0x6000000,
    ];

    for v in DICT_SIZES {
        if v as u64 >= uncompressed_size {
            return v;
        }
    }
    DICT_SIZES[DICT_SIZES.len() - 1]
}
