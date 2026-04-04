use std::cell::RefCell;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom, Write};
use std::rc::Rc;

use crc32fast::Hasher as Crc32Hasher;

use crate::codepage_437;
use crate::structured_archive::{get_zip_comment_id, ZipStructure};
use crate::zip_enums::ZipReturn;
use crate::zip_extra_field;

use super::ZipFile;

pub(crate) trait ReadSeek: Read + Seek {}
impl<T: Read + Seek> ReadSeek for T {}

#[derive(Clone)]
pub(crate) struct CentralHeaderMeta {
    #[allow(dead_code)]
    pub(crate) flags: u16,
    pub(crate) compression_method: u16,
    pub(crate) compressed_size: u64,
    pub(crate) uncompressed_size: u64,
    pub(crate) crc32: u32,
    pub(crate) local_header_offset: u64,
    #[allow(dead_code)]
    pub(crate) header_last_modified: i64,
}

pub(crate) struct EocdLocator {
    pub(crate) local_files_count: u64,
    pub(crate) central_directory_size: u64,
    pub(crate) central_directory_offset: u64,
    pub(crate) central_directory_offset_correction: i64,
    pub(crate) comment_bytes: Vec<u8>,
    pub(crate) extra_data_found_on_end: bool,
}

pub(crate) enum ZipWriterFile {
    Memory(std::io::Cursor<Vec<u8>>),
}

impl Write for ZipWriterFile {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        match self {
            ZipWriterFile::Memory(c) => c.write(buf),
        }
    }

    fn flush(&mut self) -> std::io::Result<()> {
        match self {
            ZipWriterFile::Memory(c) => c.flush(),
        }
    }
}

impl Seek for ZipWriterFile {
    fn seek(&mut self, pos: SeekFrom) -> std::io::Result<u64> {
        match self {
            ZipWriterFile::Memory(c) => c.seek(pos),
        }
    }
}

pub(crate) struct PendingWrite {
    pub(crate) filename: String,
    pub(crate) compression_method: u16,
    pub(crate) mod_time: Option<i64>,
    pub(crate) uncompressed_size: u64,
    pub(crate) raw: bool,
    pub(crate) buffer: Rc<RefCell<Vec<u8>>>,
}

pub(crate) struct SharedBufferWriter {
    pub(crate) buffer: Rc<RefCell<Vec<u8>>>,
}

impl Write for SharedBufferWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.buffer.borrow_mut().extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

pub(crate) struct LocalFileHeaderInfo {
    pub(crate) flags: u16,
    pub(crate) compression_method: u16,
    pub(crate) compressed_size: u64,
    pub(crate) uncompressed_size: u64,
    pub(crate) data_offset: u64,
}

pub(crate) struct LocalHeaderFull {
    #[allow(dead_code)]
    pub(crate) flags: u16,
    pub(crate) compression_method: u16,
    pub(crate) crc32: u32,
    pub(crate) compressed_size: u64,
    pub(crate) uncompressed_size: u64,
    #[allow(dead_code)]
    pub(crate) header_last_modified: i64,
    #[allow(dead_code)]
    pub(crate) filename: String,
    #[allow(dead_code)]
    pub(crate) data_offset: u64,
}

pub(crate) struct ManualZipWriter {
    pub(crate) file: File,
    pub(crate) entries: Vec<ManualCentralEntry>,
    pub(crate) finalized: bool,
}

#[derive(Clone)]
pub(crate) struct ManualCentralEntry {
    pub(crate) filename: String,
    pub(crate) filename_bytes: Vec<u8>,
    pub(crate) flags: u16,
    pub(crate) compression_method: u16,
    pub(crate) dos_time: u16,
    pub(crate) dos_date: u16,
    pub(crate) crc32: u32,
    pub(crate) compressed_size: u64,
    pub(crate) uncompressed_size: u64,
    pub(crate) local_header_offset: u64,
    pub(crate) is_directory: bool,
}

impl ManualZipWriter {
    pub(crate) fn new(file: File) -> Self {
        Self {
            file,
            entries: Vec::new(),
            finalized: false,
        }
    }

    pub(crate) fn zip64_eocd_required(
        entries_len: usize,
        central_directory_offset: u64,
        central_directory_size: u64,
    ) -> bool {
        entries_len >= 0xFFFF
            || central_directory_offset >= 0xFFFF_FFFF
            || central_directory_size >= 0xFFFF_FFFF
    }

    pub(crate) fn write_local_entry(
        &mut self,
        filename: &str,
        compression_method: u16,
        mod_time: Option<i64>,
        crc32_be: &[u8],
        uncompressed_size: u64,
        mut compressed_data: Vec<u8>,
    ) -> Result<ManualCentralEntry, ZipReturn> {
        let (filename_bytes, flags) = Self::encode_filename_and_flags(filename);

        let dt = mod_time.and_then(ZipFile::zip_datetime_from_i64);
        let (dos_time, dos_date) = dt.map(|t| (t.timepart(), t.datepart())).unwrap_or((0, 0));

        let local_header_offset = self
            .file
            .stream_position()
            .map_err(|_| ZipReturn::ZipErrorOpeningFile)?;

        let crc32 = if crc32_be.len() == 4 {
            u32::from_be_bytes([crc32_be[0], crc32_be[1], crc32_be[2], crc32_be[3]])
        } else {
            0
        };

        if compressed_data.is_empty() && uncompressed_size == 0 && compression_method == 8 {
            compressed_data = vec![0x03, 0x00];
        }

        let compressed_size = compressed_data.len() as u64;
        let (extra, header_uncompressed_size, header_compressed_size, _header_local_offset) =
            zip_extra_field::write_zip64_extra(
                uncompressed_size,
                compressed_size,
                local_header_offset,
                false,
            );

        if filename_bytes.len() > u16::MAX as usize || extra.len() > u16::MAX as usize {
            return Err(ZipReturn::ZipFileNameToLong);
        }

        let is_directory = filename.ends_with('/');
        if is_directory && uncompressed_size != 0 {
            return Err(ZipReturn::ZipErrorWritingToOutputStream);
        }

        let version_needed = if compression_method == 93 {
            63u16
        } else if header_uncompressed_size == 0xFFFF_FFFF || header_compressed_size == 0xFFFF_FFFF {
            45u16
        } else {
            20u16
        };

        self.file
            .write_all(&0x04034B50u32.to_le_bytes())
            .map_err(|_| ZipReturn::ZipErrorWritingToOutputStream)?;
        self.file
            .write_all(&version_needed.to_le_bytes())
            .map_err(|_| ZipReturn::ZipErrorWritingToOutputStream)?;
        self.file
            .write_all(&flags.to_le_bytes())
            .map_err(|_| ZipReturn::ZipErrorWritingToOutputStream)?;
        self.file
            .write_all(&compression_method.to_le_bytes())
            .map_err(|_| ZipReturn::ZipErrorWritingToOutputStream)?;
        self.file
            .write_all(&dos_time.to_le_bytes())
            .map_err(|_| ZipReturn::ZipErrorWritingToOutputStream)?;
        self.file
            .write_all(&dos_date.to_le_bytes())
            .map_err(|_| ZipReturn::ZipErrorWritingToOutputStream)?;
        self.file
            .write_all(&crc32.to_le_bytes())
            .map_err(|_| ZipReturn::ZipErrorWritingToOutputStream)?;
        self.file
            .write_all(&header_compressed_size.to_le_bytes())
            .map_err(|_| ZipReturn::ZipErrorWritingToOutputStream)?;
        self.file
            .write_all(&header_uncompressed_size.to_le_bytes())
            .map_err(|_| ZipReturn::ZipErrorWritingToOutputStream)?;
        self.file
            .write_all(&(filename_bytes.len() as u16).to_le_bytes())
            .map_err(|_| ZipReturn::ZipErrorWritingToOutputStream)?;
        self.file
            .write_all(&(extra.len() as u16).to_le_bytes())
            .map_err(|_| ZipReturn::ZipErrorWritingToOutputStream)?;
        self.file
            .write_all(&filename_bytes)
            .map_err(|_| ZipReturn::ZipErrorWritingToOutputStream)?;
        if !extra.is_empty() {
            self.file
                .write_all(&extra)
                .map_err(|_| ZipReturn::ZipErrorWritingToOutputStream)?;
        }
        if !compressed_data.is_empty() {
            self.file
                .write_all(&compressed_data)
                .map_err(|_| ZipReturn::ZipErrorWritingToOutputStream)?;
        }

        Ok(ManualCentralEntry {
            filename: filename.to_string(),
            filename_bytes,
            flags,
            compression_method,
            dos_time,
            dos_date,
            crc32,
            compressed_size,
            uncompressed_size,
            local_header_offset,
            is_directory,
        })
    }

    pub(crate) fn finish(
        &mut self,
        zip_struct: ZipStructure,
        comment: &str,
    ) -> Result<(), ZipReturn> {
        if self.finalized {
            return Ok(());
        }

        let _ = self
            .file
            .seek(SeekFrom::End(0))
            .map_err(|_| ZipReturn::ZipErrorOpeningFile)?;

        let central_directory_offset = self
            .file
            .stream_position()
            .map_err(|_| ZipReturn::ZipErrorOpeningFile)?;

        let mut central = Vec::new();
        for entry in &self.entries {
            let (extra, header_uncompressed_size, header_compressed_size, header_local_offset) =
                zip_extra_field::write_zip64_extra(
                    entry.uncompressed_size,
                    entry.compressed_size,
                    entry.local_header_offset,
                    true,
                );

            if entry.filename_bytes.len() > u16::MAX as usize || extra.len() > u16::MAX as usize {
                return Err(ZipReturn::ZipFileNameToLong);
            }

            let version_needed = if entry.compression_method == 93 {
                63u16
            } else if header_uncompressed_size == 0xFFFF_FFFF
                || header_compressed_size == 0xFFFF_FFFF
                || header_local_offset == 0xFFFF_FFFF
            {
                45u16
            } else {
                20u16
            };

            central.extend_from_slice(&0x02014B50u32.to_le_bytes());
            central.extend_from_slice(&version_needed.to_le_bytes());
            central.extend_from_slice(&version_needed.to_le_bytes());
            central.extend_from_slice(&entry.flags.to_le_bytes());
            central.extend_from_slice(&entry.compression_method.to_le_bytes());
            central.extend_from_slice(&entry.dos_time.to_le_bytes());
            central.extend_from_slice(&entry.dos_date.to_le_bytes());
            central.extend_from_slice(&entry.crc32.to_le_bytes());
            central.extend_from_slice(&header_compressed_size.to_le_bytes());
            central.extend_from_slice(&header_uncompressed_size.to_le_bytes());
            central.extend_from_slice(&(entry.filename_bytes.len() as u16).to_le_bytes());
            central.extend_from_slice(&(extra.len() as u16).to_le_bytes());
            central.extend_from_slice(&0u16.to_le_bytes());
            central.extend_from_slice(&0u16.to_le_bytes());
            central.extend_from_slice(&0u16.to_le_bytes());
            central.extend_from_slice(&0u32.to_le_bytes());
            central.extend_from_slice(&header_local_offset.to_le_bytes());
            central.extend_from_slice(&entry.filename_bytes);
            if !extra.is_empty() {
                central.extend_from_slice(&extra);
            }
        }

        let central_directory_size = central.len() as u64;

        let comment_bytes = if zip_struct != ZipStructure::None {
            let mut crc = Crc32Hasher::new();
            crc.update(&central);
            let crc_hex = format!("{:08X}", crc.finalize());
            format!("{}{}", get_zip_comment_id(zip_struct), crc_hex).into_bytes()
        } else {
            comment.as_bytes().to_vec()
        };
        if comment_bytes.len() > u16::MAX as usize {
            return Err(ZipReturn::ZipFileNameToLong);
        }
        let comment_len = comment_bytes.len() as u16;

        self.file
            .write_all(&central)
            .map_err(|_| ZipReturn::ZipErrorWritingToOutputStream)?;

        let zip64_needed = Self::zip64_eocd_required(
            self.entries.len(),
            central_directory_offset,
            central_directory_size,
        );

        if zip64_needed {
            let zip64_eocd_offset = central_directory_offset + central_directory_size;
            self.file
                .write_all(&0x06064B50u32.to_le_bytes())
                .map_err(|_| ZipReturn::ZipErrorWritingToOutputStream)?;
            self.file
                .write_all(&44u64.to_le_bytes())
                .map_err(|_| ZipReturn::ZipErrorWritingToOutputStream)?;
            self.file
                .write_all(&45u16.to_le_bytes())
                .map_err(|_| ZipReturn::ZipErrorWritingToOutputStream)?;
            self.file
                .write_all(&45u16.to_le_bytes())
                .map_err(|_| ZipReturn::ZipErrorWritingToOutputStream)?;
            self.file
                .write_all(&0u32.to_le_bytes())
                .map_err(|_| ZipReturn::ZipErrorWritingToOutputStream)?;
            self.file
                .write_all(&0u32.to_le_bytes())
                .map_err(|_| ZipReturn::ZipErrorWritingToOutputStream)?;
            self.file
                .write_all(&(self.entries.len() as u64).to_le_bytes())
                .map_err(|_| ZipReturn::ZipErrorWritingToOutputStream)?;
            self.file
                .write_all(&(self.entries.len() as u64).to_le_bytes())
                .map_err(|_| ZipReturn::ZipErrorWritingToOutputStream)?;
            self.file
                .write_all(&central_directory_size.to_le_bytes())
                .map_err(|_| ZipReturn::ZipErrorWritingToOutputStream)?;
            self.file
                .write_all(&central_directory_offset.to_le_bytes())
                .map_err(|_| ZipReturn::ZipErrorWritingToOutputStream)?;

            self.file
                .write_all(&0x07064B50u32.to_le_bytes())
                .map_err(|_| ZipReturn::ZipErrorWritingToOutputStream)?;
            self.file
                .write_all(&0u32.to_le_bytes())
                .map_err(|_| ZipReturn::ZipErrorWritingToOutputStream)?;
            self.file
                .write_all(&zip64_eocd_offset.to_le_bytes())
                .map_err(|_| ZipReturn::ZipErrorWritingToOutputStream)?;
            self.file
                .write_all(&1u32.to_le_bytes())
                .map_err(|_| ZipReturn::ZipErrorWritingToOutputStream)?;
        }

        let entries_u16 = if zip64_needed {
            0xFFFF
        } else {
            self.entries.len() as u16
        };
        let cd_size_u32 = if zip64_needed {
            0xFFFF_FFFF
        } else {
            central_directory_size as u32
        };
        let cd_offset_u32 = if zip64_needed {
            0xFFFF_FFFF
        } else {
            central_directory_offset as u32
        };

        self.file
            .write_all(&0x06054B50u32.to_le_bytes())
            .map_err(|_| ZipReturn::ZipErrorWritingToOutputStream)?;
        self.file
            .write_all(&0u16.to_le_bytes())
            .map_err(|_| ZipReturn::ZipErrorWritingToOutputStream)?;
        self.file
            .write_all(&0u16.to_le_bytes())
            .map_err(|_| ZipReturn::ZipErrorWritingToOutputStream)?;
        self.file
            .write_all(&entries_u16.to_le_bytes())
            .map_err(|_| ZipReturn::ZipErrorWritingToOutputStream)?;
        self.file
            .write_all(&entries_u16.to_le_bytes())
            .map_err(|_| ZipReturn::ZipErrorWritingToOutputStream)?;
        self.file
            .write_all(&cd_size_u32.to_le_bytes())
            .map_err(|_| ZipReturn::ZipErrorWritingToOutputStream)?;
        self.file
            .write_all(&cd_offset_u32.to_le_bytes())
            .map_err(|_| ZipReturn::ZipErrorWritingToOutputStream)?;
        self.file
            .write_all(&comment_len.to_le_bytes())
            .map_err(|_| ZipReturn::ZipErrorWritingToOutputStream)?;
        if !comment_bytes.is_empty() {
            self.file
                .write_all(&comment_bytes)
                .map_err(|_| ZipReturn::ZipErrorWritingToOutputStream)?;
        }

        self.file
            .flush()
            .map_err(|_| ZipReturn::ZipErrorWritingToOutputStream)?;
        self.finalized = true;
        Ok(())
    }

    fn encode_filename_and_flags(name: &str) -> (Vec<u8>, u16) {
        let mut flags = 0x0002u16;
        let bytes = if let Some(cp) = codepage_437::encode(name) {
            cp
        } else {
            flags |= 0x0800;
            name.as_bytes().to_vec()
        };
        (bytes, flags)
    }
}
