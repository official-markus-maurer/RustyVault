/// Return codes and states for archive operations.
///
/// `ZipReturn` and `ZipOpenType` define the possible error states and I/O modes
/// when interacting with `ICompress` implementations.
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
    Zip64EndOfCentralDirectoryError,
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
