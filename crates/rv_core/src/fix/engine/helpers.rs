impl Fix {
    fn default_seven_zip_struct_from_settings() -> ZipStructure {
        match crate::settings::get_settings().seven_z_default_struct {
            0 => ZipStructure::SevenZipSLZMA,
            1 => ZipStructure::SevenZipNLZMA,
            2 => ZipStructure::SevenZipSZSTD,
            _ => ZipStructure::SevenZipNZSTD,
        }
    }

    fn effective_desired_zip_struct(file_type: FileType, desired: ZipStructure) -> ZipStructure {
        match file_type {
            FileType::SevenZip | FileType::FileSevenZip => match desired {
                ZipStructure::SevenZipSLZMA
                | ZipStructure::SevenZipNLZMA
                | ZipStructure::SevenZipSZSTD
                | ZipStructure::SevenZipNZSTD
                | ZipStructure::SevenZipTrrnt => desired,
                _ => Self::default_seven_zip_struct_from_settings(),
            },
            FileType::Zip | FileType::FileZip => match desired {
                ZipStructure::ZipTrrnt | ZipStructure::ZipZSTD | ZipStructure::ZipTDC => desired,
                _ => ZipStructure::ZipTrrnt,
            },
            _ => desired,
        }
    }

    fn double_check_delete_should_skip(file: &RvFile) -> bool {
        if !crate::settings::get_settings().double_check_delete {
            return true;
        }
        if matches!(file.dat_status(), DatStatus::InToSort)
            && file.name.to_ascii_lowercase().ends_with(".cue")
        {
            return true;
        }
        if file.size == Some(0) {
            return true;
        }
        false
    }

    fn timestamp_matches(path: &PathBuf, expected_secs: i64) -> bool {
        if expected_secs == i64::MIN {
            return true;
        }
        let Ok(meta) = fs::metadata(path) else {
            return false;
        };
        let Ok(modified) = meta.modified() else {
            return false;
        };
        let Ok(dur) = modified.duration_since(std::time::UNIX_EPOCH) else {
            return false;
        };
        dur.as_secs() as i64 == expected_secs
    }
}
