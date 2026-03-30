use dat_reader::enums::HeaderFileType;
use std::io::{self, Read};

struct Data {
    offset: usize,
    value: Vec<u8>,
}

struct Detector {
    ftype: HeaderFileType,
    header_length: usize,
    file_offset: usize,
    header_id: String,
    data: Data,
}

pub struct FileHeaders;

impl FileHeaders {
    fn get_detectors() -> Vec<Detector> {
        vec![
            Detector {
                ftype: HeaderFileType::ZIP,
                header_length: 0,
                file_offset: 0,
                header_id: "ZIP".to_string(),
                data: Data { offset: 0, value: vec![0x50, 0x4B, 0x03, 0x04] },
            },
            Detector {
                ftype: HeaderFileType::GZ,
                header_length: 0,
                file_offset: 0,
                header_id: "GZ".to_string(),
                data: Data { offset: 0, value: vec![0x1F, 0x8B] },
            },
            Detector {
                ftype: HeaderFileType::SEVEN_ZIP,
                header_length: 0,
                file_offset: 0,
                header_id: "7z".to_string(),
                data: Data { offset: 0, value: vec![0x37, 0x7A, 0xBC, 0xAF, 0x27, 0x1C] },
            },
            Detector {
                ftype: HeaderFileType::RAR,
                header_length: 0,
                file_offset: 0,
                header_id: "RAR".to_string(),
                data: Data { offset: 0, value: vec![0x52, 0x61, 0x72, 0x21, 0x1A, 0x07, 0x00] },
            },
            Detector {
                ftype: HeaderFileType::CHD,
                header_length: 0,
                file_offset: 0,
                header_id: "CHD".to_string(),
                data: Data { offset: 0, value: vec![0x4D, 0x43, 0x6F, 0x6D, 0x70, 0x72, 0x48, 0x44] },
            },
            Detector {
                ftype: HeaderFileType::A7800,
                header_length: 128,
                file_offset: 128,
                header_id: "A78".to_string(),
                data: Data { offset: 1, value: vec![0x41, 0x54, 0x41, 0x52, 0x49, 0x37, 0x38, 0x30, 0x30] },
            },
            Detector {
                ftype: HeaderFileType::LYNX,
                header_length: 64,
                file_offset: 64,
                header_id: "LYNX".to_string(),
                data: Data { offset: 0, value: vec![0x4C, 0x59, 0x4E, 0x58] },
            },
            Detector {
                ftype: HeaderFileType::NES,
                header_length: 16,
                file_offset: 16,
                header_id: "NES".to_string(),
                data: Data { offset: 0, value: vec![0x4E, 0x45, 0x53, 0x1A] },
            },
            Detector {
                ftype: HeaderFileType::FDS,
                header_length: 16,
                file_offset: 16,
                header_id: "FDS".to_string(),
                data: Data { offset: 0, value: vec![0x46, 0x44, 0x53, 0x1A] },
            },
            Detector {
                ftype: HeaderFileType::PCE,
                header_length: 512,
                file_offset: 512,
                header_id: "PCE".to_string(),
                data: Data { offset: 0, value: vec![0x40, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xAA, 0xBB, 0x02] },
            },
            Detector {
                ftype: HeaderFileType::PSID,
                header_length: 124,
                file_offset: 124,
                header_id: "PSID".to_string(),
                data: Data { offset: 0, value: vec![0x50, 0x53, 0x49, 0x44] },
            },
            Detector {
                ftype: HeaderFileType::SNES,
                header_length: 512,
                file_offset: 512,
                header_id: "SMC".to_string(),
                data: Data { offset: 0, value: vec![0xAA, 0xBB, 0x04, 0x00, 0x00, 0x00, 0x00, 0x00] },
            },
            Detector {
                ftype: HeaderFileType::SPC,
                header_length: 256,
                file_offset: 256,
                header_id: "SPC".to_string(),
                data: Data { offset: 0, value: vec![0x53, 0x4E, 0x45, 0x53, 0x2D, 0x53, 0x50, 0x43, 0x37, 0x30, 0x30, 0x20, 0x53, 0x6F, 0x75, 0x6E, 0x64, 0x20, 0x46, 0x69, 0x6C, 0x65] },
            },
        ]
    }

    pub fn get_file_type_from_stream(stream: &mut dyn Read) -> io::Result<(HeaderFileType, usize)> {
        let mut buffer = [0u8; 512];
        let bytes_read = stream.read(&mut buffer)?;
        
        Ok(Self::get_file_type_from_buffer(&buffer[..bytes_read]))
    }

    pub fn get_file_type_from_buffer(buffer: &[u8]) -> (HeaderFileType, usize) {
        for detector in Self::get_detectors() {
            if buffer.len() < detector.data.value.len() + detector.data.offset {
                continue;
            }

            if Self::byte_comp(buffer, &detector.data) {
                return (detector.ftype, detector.file_offset);
            }
        }

        (HeaderFileType::NOTHING, 0)
    }

    fn byte_comp(buffer: &[u8], d: &Data) -> bool {
        if buffer.len() < d.value.len() + d.offset {
            return false;
        }

        for i in 0..d.value.len() {
            if buffer[i + d.offset] != d.value[i] {
                return false;
            }
        }

        true
    }
}
