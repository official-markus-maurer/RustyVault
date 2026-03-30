use bitflags::bitflags;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize)]
pub enum DatStatus {
    InDatCollect,
    InDatMerged,
    InDatNoDump,
    NotInDat, // Any item not in a dat and not in ToSort should have this status
    InToSort, // All items in any ToSort directory should have this status
    InDatMIA,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize)]
pub enum FileType {
    UnSet = 0,
    Dir = 1,
    Zip = 2,
    SevenZip = 3,
    File = 4,
    FileZip = 5,
    FileSevenZip = 6,

    FileOnly = 100,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize)]
pub enum GotStatus {
    NotGot,
    Got,
    Corrupt,
    FileLocked,
}

bitflags! {
    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize)]
    pub struct HeaderFileType: u8 {
        const NOTHING = 0;
        const ZIP = 1;
        const GZ = 2;
        const SEVEN_ZIP = 3;
        const RAR = 4;

        const CHD = 5;

        const A7800 = 6;
        const LYNX = 7;
        const NES = 8;
        const FDS = 9;
        const PCE = 10;
        const PSID = 11;
        const SNES = 12;
        const SPC = 13;

        const HEADER_MASK = 0x1f;
        const REQUIRED = 0x80;
    }
}

impl Default for HeaderFileType {
    fn default() -> Self {
        HeaderFileType::NOTHING
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize)]
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

impl From<u8> for ZipStructure {
    fn from(value: u8) -> Self {
        match value {
            1 => ZipStructure::ZipTrrnt,
            2 => ZipStructure::ZipTDC,
            4 => ZipStructure::SevenZipTrrnt,
            5 => ZipStructure::ZipZSTD,
            8 => ZipStructure::SevenZipSLZMA,
            9 => ZipStructure::SevenZipNLZMA,
            10 => ZipStructure::SevenZipSZSTD,
            11 => ZipStructure::SevenZipNZSTD,
            _ => ZipStructure::None,
        }
    }
}
