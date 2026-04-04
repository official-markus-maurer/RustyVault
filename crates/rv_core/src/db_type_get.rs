use dat_reader::enums::FileType;

pub fn dir_from_file(file_type: FileType) -> FileType {
    match file_type {
        FileType::File => FileType::Dir,
        FileType::FileZip => FileType::Zip,
        FileType::FileSevenZip => FileType::SevenZip,
        _ => FileType::Zip,
    }
}

pub fn file_from_dir(file_type: FileType) -> FileType {
    match file_type {
        FileType::Dir => FileType::File,
        FileType::Zip => FileType::FileZip,
        FileType::SevenZip => FileType::FileSevenZip,
        _ => FileType::Zip,
    }
}

pub fn is_compressed_dir(file_type: FileType) -> bool {
    file_type == FileType::Zip || file_type == FileType::SevenZip
}

pub fn from_extension(ext: &str) -> FileType {
    match ext.to_ascii_lowercase().as_str() {
        ".7z" => FileType::SevenZip,
        ".zip" => FileType::Zip,
        _ => FileType::File,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_extensions() {
        assert_eq!(from_extension(".zip"), FileType::Zip);
        assert_eq!(from_extension(".7Z"), FileType::SevenZip);
        assert_eq!(from_extension(".bin"), FileType::File);
    }
}
