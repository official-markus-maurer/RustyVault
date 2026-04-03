use std::cell::RefCell;
use std::rc::Rc;

use dat_reader::enums::FileType;

use crate::rv_dat::DatData;
use crate::rv_file::RvFile;

fn node_logical_path(node: &RvFile) -> String {
    let mut parts = vec![node.name.clone()];
    let mut parent = node.get_parent();
    while let Some(p) = parent {
        let b = p.borrow();
        if !b.name.is_empty() {
            parts.push(b.name.clone());
        }
        parent = b.get_parent();
    }
    parts.reverse();
    parts.join("\\")
}

pub fn is_file_only(in_file: Rc<RefCell<RvFile>>) -> bool {
    if crate::settings::get_settings().files_only {
        return true;
    }

    let logical_path = {
        let f = in_file.borrow();
        node_logical_path(&f)
    };

    let dat_rule = crate::settings::find_rule(&logical_path);
    if dat_rule.compression == FileType::FileOnly {
        return true;
    }

    let mut dat_header_type: Option<String> = None;
    let mut current: Option<Rc<RefCell<RvFile>>> = Some(in_file);
    while let Some(rc) = current {
        let b = rc.borrow();
        if let Some(ref dat) = b.dat {
            dat_header_type = dat.borrow().get_data(DatData::Compression);
            break;
        }
        for d in &b.dir_dats {
            if d
                .borrow()
                .get_data(DatData::Compression)
                .is_some_and(|v| v.eq_ignore_ascii_case("fileonly"))
            {
                dat_header_type = Some("fileonly".to_string());
                break;
            }
        }
        if dat_header_type.is_some() {
            break;
        }
        current = b.get_parent();
    }

    if let Some(v) = dat_header_type {
        return v.eq_ignore_ascii_case("fileonly");
    }

    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use dat_reader::enums::{DatStatus, GotStatus};

    #[test]
    fn global_files_only_overrides() {
        crate::settings::GLOBAL_SETTINGS.with(|s| s.borrow_mut().files_only = true);
        let f = Rc::new(RefCell::new(RvFile::new(FileType::File)));
        assert!(is_file_only(Rc::clone(&f)));
        crate::settings::GLOBAL_SETTINGS.with(|s| s.borrow_mut().files_only = false);
    }

    #[test]
    fn default_is_not_file_only() {
        let mut f = RvFile::new(FileType::File);
        f.set_dat_got_status(DatStatus::NotInDat, GotStatus::NotGot);
        let rc = Rc::new(RefCell::new(f));
        assert!(!is_file_only(rc));
    }
}

