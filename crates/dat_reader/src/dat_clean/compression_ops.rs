impl DatClean {
    pub fn set_ext(t_dat: &mut DatDir, header_file_type: HeaderFileType) {
        let mut children = Vec::new();
        children.append(&mut t_dat.children);
        for mut node in children {
            match node.node {
                crate::dat_store::DatBase::File(_) => {
                    let is_disk = node.file().is_some_and(|f| f.is_disk);
                    if is_disk {
                        node.name.push_str(".chd");
                    }
                    if let Some(f) = node.file_mut() {
                        if is_disk {
                            f.header_file_type = HeaderFileType::CHD;
                            if let Some(merge) = f.merge.as_mut() {
                                merge.push_str(".chd");
                            }
                        } else {
                            f.header_file_type = header_file_type;
                        }
                    }
                    t_dat.add_child(node);
                }
                crate::dat_store::DatBase::Dir(_) => {
                    let ext = Self::get_ext(node.file_type);
                    node.name.push_str(ext);
                    if let Some(d) = node.dir_mut() {
                        Self::set_ext(d, header_file_type);
                    }
                    t_dat.add_child(node);
                }
            }
        }
    }

    fn get_ext(dft: FileType) -> &'static str {
        match dft {
            FileType::Zip => ".zip",
            FileType::SevenZip => ".7z",
            _ => "",
        }
    }

    pub fn set_compression_type(
        in_dat: &mut DatNode,
        file_type: FileType,
        zs: ZipStructure,
        fix: bool,
    ) {
        if in_dat.file().is_some() {
            in_dat.file_type = Self::file_type_from_dir(file_type);
            return;
        }
        let had_game = in_dat.dir().is_some_and(|d| d.d_game.is_some());
        let existing_type = in_dat.file_type;

        let mut effective_file_type = if !had_game || file_type == FileType::Dir {
            FileType::Dir
        } else {
            file_type
        };
        let mut effective_zs = zs;

        if had_game && file_type != FileType::Dir && existing_type != FileType::UnSet {
            match existing_type {
                FileType::Dir => {
                    effective_file_type = FileType::Dir;
                    effective_zs = ZipStructure::None;
                }
                FileType::Zip => {
                    effective_file_type = FileType::Zip;
                    effective_zs = ZipStructure::ZipTrrnt;
                }
                FileType::SevenZip => {
                    effective_file_type = FileType::SevenZip;
                    effective_zs = ZipStructure::SevenZipNZSTD;
                }
                _ => {}
            }
        }

        in_dat.file_type = effective_file_type;

        if let Some(d_dir) = in_dat.dir_mut() {
            if had_game && file_type != FileType::Dir && effective_file_type != FileType::Dir {
                let checked =
                    if Self::is_trrntzip_date_times(d_dir, effective_zs, effective_file_type) {
                        ZipStructure::ZipTrrnt
                    } else {
                        effective_zs
                    };
                d_dir.set_dat_struct(checked, fix);
            }

            let children = std::mem::take(&mut d_dir.children);
            for mut child in children {
                Self::set_compression_type(&mut child, file_type, zs, fix);
                d_dir.add_child(child);
            }
        }
    }

    fn file_type_from_dir(file_type: FileType) -> FileType {
        match file_type {
            FileType::UnSet => FileType::UnSet,
            FileType::Dir => FileType::File,
            FileType::Zip => FileType::FileZip,
            FileType::SevenZip => FileType::FileSevenZip,
            _ => FileType::File,
        }
    }

    fn is_trrntzip_date_times(d_dir: &DatDir, zs: ZipStructure, file_type: FileType) -> bool {
        if file_type != FileType::Zip || zs != ZipStructure::ZipTDC {
            return false;
        }
        for child in &d_dir.children {
            if child.file().is_some() {
                if child.date_modified != Some(crate::dat_store::TRRNTZIP_DOS_DATETIME) {
                    return false;
                }
            } else {
                return false;
            }
        }
        true
    }
}
