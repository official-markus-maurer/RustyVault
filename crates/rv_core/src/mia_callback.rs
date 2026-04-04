use std::cell::RefCell;
use std::collections::HashSet;
use std::fs::OpenOptions;
use std::io::Write;
use std::rc::Rc;

use dat_reader::enums::{DatStatus, FileType, GotStatus};

thread_local! {
    static REPORTED_KEYS: RefCell<HashSet<String>> = RefCell::new(HashSet::new());
}

fn collect_found_mia_files(node: Rc<RefCell<crate::rv_file::RvFile>>, out: &mut Vec<Rc<RefCell<crate::rv_file::RvFile>>>) {
    let (children, file_type, dat_status, got_status) = {
        let n = node.borrow();
        (n.children.clone(), n.file_type, n.dat_status(), n.got_status())
    };

    if matches!(file_type, FileType::File | FileType::FileZip | FileType::FileSevenZip | FileType::FileOnly | FileType::Zip | FileType::SevenZip)
        && dat_status == DatStatus::InDatMIA
        && got_status == GotStatus::Got
    {
        out.push(Rc::clone(&node));
    }

    for c in children {
        collect_found_mia_files(c, out);
    }
}

pub fn report_found_mia(root: Rc<RefCell<crate::rv_file::RvFile>>) {
    let settings = crate::settings::get_settings();
    if settings.do_not_report_feedback || !settings.mia_callback {
        return;
    }

    let mut found = Vec::new();
    collect_found_mia_files(Rc::clone(&root), &mut found);
    if found.is_empty() {
        return;
    }

    let log_dir = std::env::current_dir().unwrap_or_default().join("Logs");
    let _ = std::fs::create_dir_all(&log_dir);
    let log_path = log_dir.join("MIA_Found.txt");

    let now = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
    let mut file = match OpenOptions::new().create(true).append(true).open(&log_path) {
        Ok(f) => f,
        Err(_) => return,
    };

    for f_rc in found {
        let (full_name, name, size, crc, sha1, md5) = {
            let f = f_rc.borrow();
            (
                f.get_full_name(),
                f.name.clone(),
                f.size,
                f.crc.clone(),
                f.sha1.clone(),
                f.md5.clone(),
            )
        };

        let key = format!("{}|{:?}|{:?}", full_name, size, crc);
        let should_write = REPORTED_KEYS.with(|set| {
            let mut set = set.borrow_mut();
            if set.contains(&key) {
                false
            } else {
                set.insert(key);
                true
            }
        });
        if !should_write {
            continue;
        }

        if settings.mia_anon {
            let _ = writeln!(file, "[{now}] Found MIA: {}", name);
            crate::task_reporter::task_log(format!("[MIA] Found: {}", name));
        } else {
            let crc_hex = crc.as_ref().map(|v| crate::to_hex_string(Some(v.as_slice()))).unwrap_or_default();
            let sha1_hex = sha1
                .as_ref()
                .map(|v| crate::to_hex_string(Some(v.as_slice())))
                .unwrap_or_default();
            let md5_hex = md5.as_ref().map(|v| crate::to_hex_string(Some(v.as_slice()))).unwrap_or_default();
            let _ = writeln!(
                file,
                "[{now}] Found MIA: {} | size={:?} crc={} sha1={} md5={}",
                full_name,
                size,
                crc_hex,
                sha1_hex,
                md5_hex
            );
            crate::task_reporter::task_log(format!("[MIA] Found: {}", full_name));
        }
    }
}
