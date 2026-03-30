use std::io::{Read, Write};
use crate::file_header::FileHeader;
use crate::structured_archive::ZipStructure;
use crate::zip_enums::{ZipOpenType, ZipReturn};

pub trait ICompress {
    fn local_files_count(&self) -> usize;
    
    fn get_file_header(&self, index: usize) -> Option<&FileHeader>;
    
    fn zip_open_type(&self) -> ZipOpenType;
    
    fn zip_file_open(&mut self, new_filename: &str, timestamp: i64, read_headers: bool) -> ZipReturn;
    
    fn zip_file_close(&mut self);
    
    fn zip_file_open_read_stream(&mut self, index: usize) -> Result<(Box<dyn Read>, u64), ZipReturn>;
    
    fn zip_file_close_read_stream(&mut self) -> ZipReturn;
    
    fn zip_struct(&self) -> ZipStructure;
    
    fn zip_filename(&self) -> &str;
    
    fn time_stamp(&self) -> i64;
    
    fn file_comment(&self) -> &str;
    
    fn zip_file_create(&mut self, new_filename: &str) -> ZipReturn;
    
    fn zip_file_open_write_stream(
        &mut self,
        raw: bool,
        filename: &str,
        uncompressed_size: u64,
        compression_method: u16,
        mod_time: Option<i64>,
    ) -> Result<Box<dyn Write>, ZipReturn>;
    
    fn zip_file_close_write_stream(&mut self, crc32: &[u8]) -> ZipReturn;
    
    fn zip_file_close_failed(&mut self);
}
