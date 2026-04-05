#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum SamInputKind {
    Directory,
    Zip,
    SevenZip,
    Mixed,
}

impl SamInputKind {
    pub(crate) fn label(self) -> &'static str {
        match self {
            SamInputKind::Directory => "Directory",
            SamInputKind::Zip => "Zip",
            SamInputKind::SevenZip => "7z",
            SamInputKind::Mixed => "Mixed",
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum SamOutputKind {
    TorrentZip,
    Zip,
    ZipZstd,
    SevenZipLzma,
    SevenZipZstd,
}

impl SamOutputKind {
    pub(crate) fn label(self) -> &'static str {
        match self {
            SamOutputKind::TorrentZip => "TorrentZip",
            SamOutputKind::Zip => "Zip",
            SamOutputKind::ZipZstd => "Zip Zstd",
            SamOutputKind::SevenZipLzma => "7z LZMA",
            SamOutputKind::SevenZipZstd => "7z Zstd",
        }
    }
}

pub(crate) const SAM_INPUT_OPTIONS: [SamInputKind; 4] = [
    SamInputKind::Directory,
    SamInputKind::Zip,
    SamInputKind::SevenZip,
    SamInputKind::Mixed,
];

pub(crate) const SAM_OUTPUT_OPTIONS: [SamOutputKind; 5] = [
    SamOutputKind::TorrentZip,
    SamOutputKind::Zip,
    SamOutputKind::ZipZstd,
    SamOutputKind::SevenZipLzma,
    SamOutputKind::SevenZipZstd,
];
