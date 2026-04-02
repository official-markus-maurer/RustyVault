use compress::i_compress::ICompress;
use compress::zip_enums::ZipReturn;
use crate::trrntzip_status::TrrntZipStatus;
use compress::structured_archive::ZipStructure;
use compress::zip_file::ZipFile;
use crate::process_control::ProcessControl;
use crate::zipped_file::ZippedFile;
use crate::torrent_zip_check::TorrentZipCheck;
use crate::torrent_zip_rebuild::TorrentZipRebuild;

/// High-level orchestration for the TorrentZip utility.
/// 
/// `TorrentZip` acts as the entry point for the CLI and UI tools to submit files
/// for verification and repacking.
/// 
/// Differences from C#:
/// - Maps 1:1 to the C# `TrrntZip.TorrentZip` entry class.
pub struct TorrentZip {
    pub force_rezip: bool,
    pub check_only: bool,
    pub out_zip_type: ZipStructure,
}

impl TorrentZip {
    pub fn new() -> Self {
        Self {
            force_rezip: false,
            check_only: false,
            out_zip_type: ZipStructure::ZipTrrnt,
        }
    }

    pub fn process(&self, filename: &str) -> TrrntZipStatus {
        self.process_with_control(filename, None)
    }

    pub fn process_with_control(&self, filename: &str, control: Option<&ProcessControl>) -> TrrntZipStatus {
        let mut zip_file = ZipFile::new();
        
        let open_status = zip_file.zip_file_open(filename, 0, true);
        if open_status != ZipReturn::ZipGood {
            return TrrntZipStatus::CORRUPT_ZIP;
        }

        let mut zipped_files = Vec::new();
        let count = zip_file.local_files_count();
        for i in 0..count {
            if let Some(header) = zip_file.get_file_header(i) {
                let mut zf = ZippedFile::new();
                zf.index = i as i32;
                zf.name = header.filename.clone();
                zf.size = header.uncompressed_size;
                zf.crc = header.crc.clone();
                zf.is_dir = header.is_directory;
                zipped_files.push(zf);
            }
        }

        let mut is_valid = match self.out_zip_type {
            ZipStructure::ZipTrrnt | ZipStructure::ZipZSTD => TorrentZipCheck::check_zip_files(&mut zipped_files),
            ZipStructure::SevenZipSLZMA | ZipStructure::SevenZipNLZMA | ZipStructure::SevenZipSZSTD | ZipStructure::SevenZipNZSTD => TorrentZipCheck::check_seven_zip_files(&mut zipped_files),
            _ => TorrentZipCheck::check_zip_files(&mut zipped_files),
        };
        
        if zip_file.zip_struct() != self.out_zip_type {
            is_valid |= TrrntZipStatus::BAD_EXTRA_DATA;
        }
        
        if is_valid == TrrntZipStatus::VALID_TRRNTZIP && !self.force_rezip {
            zip_file.zip_file_close();
            return TrrntZipStatus::VALID_TRRNTZIP;
        }
        
        if self.check_only {
            zip_file.zip_file_close();
            return is_valid;
        }

        println!("Rebuilding archive: {}", filename);
        let rebuild_status = TorrentZipRebuild::rezip_files_with_control(&zipped_files, &mut zip_file, self.out_zip_type, control);
        
        rebuild_status
    }
}
