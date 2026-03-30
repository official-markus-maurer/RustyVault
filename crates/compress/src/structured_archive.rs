#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ZipStructure {
    None = 0,          // No structure
    ZipTrrnt = 1,      // Original Trrntzip
    ZipTDC = 2,        // Total DOS Collection, Date Time Deflate
    SevenZipTrrnt = 4, // this is the original t7z format
    ZipZSTD = 5,       // ZSTD Compression
    SevenZipSLZMA = 8, // Solid-LZMA this is rv7zip today
    SevenZipNLZMA = 9, // NonSolid-LZMA
    SevenZipSZSTD = 10, // Solid-zSTD
    SevenZipNZSTD = 11, // NonSolid-zSTD
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ZipDateType {
    Undefined,
    None,
    TrrntZip,
    DateTime,
}

pub fn get_compression_type(zip_struct: ZipStructure) -> u16 {
    match zip_struct {
        ZipStructure::None => 0,
        ZipStructure::ZipTrrnt | ZipStructure::ZipTDC => 8,
        ZipStructure::SevenZipTrrnt => u16::MAX,
        ZipStructure::ZipZSTD => 93,
        ZipStructure::SevenZipSLZMA | ZipStructure::SevenZipNLZMA => 14,
        ZipStructure::SevenZipSZSTD | ZipStructure::SevenZipNZSTD => 93,
    }
}

pub fn get_zip_comment_id(zip_struct: ZipStructure) -> &'static str {
    match zip_struct {
        ZipStructure::ZipTrrnt => "TORRENTZIPPED-",
        ZipStructure::ZipTDC => "TDC-",
        ZipStructure::ZipZSTD => "RVZSTD-",
        _ => "",
    }
}

pub fn get_zip_date_time_type(zip_struct: ZipStructure) -> ZipDateType {
    match zip_struct {
        ZipStructure::ZipTrrnt => ZipDateType::TrrntZip,
        ZipStructure::ZipTDC => ZipDateType::DateTime,
        ZipStructure::ZipZSTD => ZipDateType::None,
        _ => ZipDateType::Undefined,
    }
}
