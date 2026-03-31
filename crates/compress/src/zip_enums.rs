/// Return codes and states for archive operations.
/// 
/// `ZipReturn` and `ZipOpenType` define the possible error states and I/O modes
/// when interacting with `ICompress` implementations.
/// 
/// Differences from C#:
/// - Identical 1:1 mapping to the C# `Compress` enums.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ZipReturn {
    ZipGood,
    ZipFileLocked,
    ZipFileCountError,
    ZipSignatureError,
    ZipExtraDataOnEndOfZip,
    ZipUnsupportedCompression,
    ZipLocalFileHeaderError,
    ZipCentralDirError,
    ZipEndOfCentralDirectoryError,
    Zip64EndOfCentralDirError,
    Zip64EndOfCentralDirectoryLocatorError,
    ZipReadingFromOutputFile,
    ZipWritingToInputFile,
    ZipErrorGettingDataStream,
    ZipCRCDecodeError,
    ZipDecodeError,
    ZipFileNameToLong,
    ZipFileAlreadyOpen,
    ZipCannotFastOpen,
    ZipErrorOpeningFile,
    ZipErrorFileNotFound,
    ZipErrorReadingFile,
    ZipErrorTimeStamp,
    ZipErrorRollBackFile,
    ZipTryingToAccessADirectory,
    ZipErrorWritingToOutputStream,
    ZipTrrntzipIncorrectCompressionUsed,
    ZipTrrntzipIncorrectFileOrder,
    ZipTrrntzipIncorrectDirectoryAddedToZip,
    ZipTrrntZipIncorrectDataStream,
    ZipUntested,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ZipOpenType {
    Closed,
    OpenRead,
    OpenWrite,
    OpenFakeWrite,
}
