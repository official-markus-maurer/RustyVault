use dat_reader::enums::ZipStructure;
use trrntzip::torrent_zip_check::TorrentZipCheck;
use trrntzip::zipped_file::ZippedFile;

impl super::Fix {
    pub(super) fn sort_archive_rebuild_entries(
        entries: &mut [super::ArchiveRebuildEntry],
        desired_zip_struct: ZipStructure,
    ) {
        entries.sort_by(|a, b| {
            let a_ref = a.node.borrow();
            let b_ref = b.node.borrow();
            let zf_a = ZippedFile {
                index: 0,
                name: a.target_name.clone(),
                size: a_ref.size.unwrap_or(0),
                crc: a_ref.crc.clone(),
                sha1: a_ref.sha1.clone(),
                is_dir: a.is_directory,
            };
            let zf_b = ZippedFile {
                index: 0,
                name: b.target_name.clone(),
                size: b_ref.size.unwrap_or(0),
                crc: b_ref.crc.clone(),
                sha1: b_ref.sha1.clone(),
                is_dir: b.is_directory,
            };

            let cmp = match desired_zip_struct {
                ZipStructure::SevenZipTrrnt
                | ZipStructure::SevenZipSLZMA
                | ZipStructure::SevenZipNLZMA
                | ZipStructure::SevenZipSZSTD
                | ZipStructure::SevenZipNZSTD => TorrentZipCheck::trrnt_7zip_string_compare(&zf_a, &zf_b),
                _ => TorrentZipCheck::trrnt_zip_string_compare(&zf_a, &zf_b),
            };

            cmp.cmp(&0)
        });
    }
}

