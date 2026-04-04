use std::cell::RefCell;
use std::collections::HashMap;
use std::fs::File;
use std::io::Write;
use std::rc::Rc;

use rv_core::enums::RepStatus;
use rv_core::rv_dat::{DatData, RvDat};
use rv_core::rv_file::RvFile;

#[derive(Clone)]
struct FixEntry {
    file_name: String,
    size: Option<u64>,
    crc: String,
    rep_status: RepStatus,
}

fn is_fixing(rep_status: RepStatus) -> bool {
    matches!(
        rep_status,
        RepStatus::CanBeFixed
            | RepStatus::CanBeFixedMIA
            | RepStatus::MoveToSort
            | RepStatus::Delete
            | RepStatus::NeededForFix
            | RepStatus::Rename
            | RepStatus::CorruptCanBeFixed
            | RepStatus::MoveToCorrupt
    )
}

fn dat_display_name(dat: &RvDat) -> String {
    let full = dat.get_data(DatData::DatRootFullName).unwrap_or_default();
    if let Some(idx) = full.find('\\') {
        full[idx + 1..].to_string()
    } else {
        full
    }
}

fn file_name_inside_game(node: Rc<RefCell<RvFile>>) -> String {
    let mut path_parts = Vec::new();
    let mut current = Some(node);

    while let Some(n) = current {
        let (name, parent, parent_has_game) = {
            let nb = n.borrow();
            let parent = nb.parent.as_ref().and_then(|p| p.upgrade());
            let parent_has_game = parent
                .as_ref()
                .map(|p| p.borrow().game.is_some())
                .unwrap_or(false);
            (nb.name.clone(), parent, parent_has_game)
        };

        if !name.is_empty() {
            path_parts.push(name);
        }

        if parent_has_game {
            break;
        }

        current = parent;
    }

    path_parts.reverse();
    path_parts.join("\\")
}

fn crc_hex(node: &RvFile) -> String {
    let hex = rv_core::arr_byte::to_hex_string(node.crc.as_deref());
    if hex.is_empty() {
        hex
    } else {
        hex.to_ascii_uppercase()
    }
}

fn collect_fix_entries_by_dat(root: Rc<RefCell<RvFile>>) -> Vec<(Rc<RefCell<RvDat>>, Vec<FixEntry>)> {
    let mut seen_dats: HashMap<*const RefCell<RvDat>, Rc<RefCell<RvDat>>> = HashMap::new();
    let mut groups: HashMap<*const RefCell<RvDat>, Vec<FixEntry>> = HashMap::new();

    let mut stack = vec![root];
    while let Some(node_rc) = stack.pop() {
        let (children, rep_status, size, dat_opt, is_file, crc, game) = {
            let n = node_rc.borrow();
            (
                n.children.clone(),
                n.rep_status(),
                n.size,
                n.dat.clone(),
                n.is_file(),
                crc_hex(&n),
                n.game.clone(),
            )
        };

        if let Some(dat_rc) = dat_opt {
            let dat_ptr = Rc::as_ptr(&dat_rc);
            seen_dats.entry(dat_ptr).or_insert_with(|| Rc::clone(&dat_rc));

            if is_file && game.is_none() && is_fixing(rep_status) {
                groups.entry(dat_ptr).or_default().push(FixEntry {
                    file_name: file_name_inside_game(Rc::clone(&node_rc)),
                    size,
                    crc,
                    rep_status,
                });
            }
        }

        for child in children {
            stack.push(child);
        }
    }

    let mut out: Vec<(Rc<RefCell<RvDat>>, Vec<FixEntry>)> = groups
        .into_iter()
        .filter_map(|(k, v)| seen_dats.get(&k).map(|d| (Rc::clone(d), v)))
        .collect();

    out.sort_by(|(a, _), (b, _)| {
        let da = dat_display_name(&a.borrow());
        let db = dat_display_name(&b.borrow());
        let la = da.to_ascii_lowercase();
        let lb = db.to_ascii_lowercase();
        la.cmp(&lb).then(da.cmp(&db))
    });
    for (_, entries) in out.iter_mut() {
        entries.sort_by(|a, b| {
            let la = a.file_name.to_ascii_lowercase();
            let lb = b.file_name.to_ascii_lowercase();
            la.cmp(&lb).then(a.file_name.cmp(&b.file_name))
        });
    }
    out
}

pub(crate) fn write_fix_report(path: &str, root: Rc<RefCell<RvFile>>) -> std::io::Result<()> {
    let mut file = File::create(path)?;

    writeln!(file, "Listing Fixes")?;
    writeln!(file, "-----------------------------------------")?;

    let groups = collect_fix_entries_by_dat(root);
    for (dat_rc, entries) in groups {
        if entries.is_empty() {
            continue;
        }

        let dat_name = dat_display_name(&dat_rc.borrow());
        writeln!(file, "{}", dat_name)?;

        let mut max_path = 0usize;
        let mut max_size = 0usize;
        let mut max_status = 0usize;
        for e in &entries {
            max_path = max_path.max(e.file_name.len());
            max_size = max_size.max(
                e.size
                    .map(|v| v.to_string().len())
                    .unwrap_or(0),
            );
            max_status = max_status.max(format!("{:?}", e.rep_status).len());
        }

        writeln!(
            file,
            "+{}+{}+----------+{}+",
            "-".repeat(max_path + 2),
            "-".repeat(max_size + 2),
            "-".repeat(max_status + 2)
        )?;

        for e in &entries {
            let size_str = e.size.map(|v| v.to_string()).unwrap_or_default();
            let status_str = format!("{:?}", e.rep_status);
            writeln!(
                file,
                "| {:<path_w$} | {:>size_w$} | {:<8} | {:<status_w$} |",
                e.file_name,
                size_str,
                e.crc,
                status_str,
                path_w = max_path,
                size_w = max_size,
                status_w = max_status
            )?;
        }

        writeln!(
            file,
            "+{}+{}+----------+{}+",
            "-".repeat(max_path + 2),
            "-".repeat(max_size + 2),
            "-".repeat(max_status + 2)
        )?;
        writeln!(file)?;
    }

    Ok(())
}

fn is_partial(rep_status: RepStatus) -> bool {
    matches!(
        rep_status,
        RepStatus::UnScanned
            | RepStatus::Missing
            | RepStatus::Corrupt
            | RepStatus::CanBeFixed
            | RepStatus::CanBeFixedMIA
            | RepStatus::CorruptCanBeFixed
    )
}

fn classify_dat(root: Rc<RefCell<RvFile>>, dat_rc: Rc<RefCell<RvDat>>) -> (i32, i32, i32, Vec<FixEntry>) {
    let dat_ptr = Rc::as_ptr(&dat_rc);
    let mut correct = 0i32;
    let mut missing = 0i32;
    let mut fixes_needed = 0i32;
    let mut partial_entries = Vec::new();

    let mut stack = vec![root];
    while let Some(node_rc) = stack.pop() {
        let (children, rep_status, size, dat_opt, is_file, crc, game) = {
            let n = node_rc.borrow();
            (
                n.children.clone(),
                n.rep_status(),
                n.size,
                n.dat.clone(),
                n.is_file(),
                crc_hex(&n),
                n.game.clone(),
            )
        };

        if let Some(node_dat) = dat_opt {
            if Rc::as_ptr(&node_dat) == dat_ptr && is_file && game.is_none() {
                match rep_status {
                    RepStatus::Correct | RepStatus::DirCorrect => correct += 1,
                    RepStatus::CorrectMIA => correct += 1,
                    RepStatus::Missing | RepStatus::MissingMIA | RepStatus::Corrupt | RepStatus::Incomplete => {
                        missing += 1
                    }
                    RepStatus::UnScanned | RepStatus::Unknown | RepStatus::DirUnknown => missing += 1,
                    RepStatus::CanBeFixed
                    | RepStatus::CanBeFixedMIA
                    | RepStatus::CorruptCanBeFixed
                    | RepStatus::InToSort
                    | RepStatus::DirInToSort
                    | RepStatus::MoveToSort
                    | RepStatus::MoveToCorrupt
                    | RepStatus::Delete
                    | RepStatus::Deleted
                    | RepStatus::NeededForFix
                    | RepStatus::Rename
                    | RepStatus::IncompleteRemove
                    | RepStatus::UnNeeded
                    | RepStatus::NotCollected => fixes_needed += 1,
                    _ => {}
                }

                if is_partial(rep_status) {
                    partial_entries.push(FixEntry {
                        file_name: file_name_inside_game(Rc::clone(&node_rc)),
                        size,
                        crc,
                        rep_status,
                    });
                }
            }
        }

        for child in children {
            stack.push(child);
        }
    }

    partial_entries.sort_by(|a, b| {
        let la = a.file_name.to_ascii_lowercase();
        let lb = b.file_name.to_ascii_lowercase();
        la.cmp(&lb).then(a.file_name.cmp(&b.file_name))
    });
    (correct, missing, fixes_needed, partial_entries)
}

pub(crate) fn write_full_report(path: &str, root: Rc<RefCell<RvFile>>) -> std::io::Result<()> {
    let mut file = File::create(path)?;

    let mut seen_dats: HashMap<*const RefCell<RvDat>, Rc<RefCell<RvDat>>> = HashMap::new();
    let mut stack = vec![Rc::clone(&root)];
    while let Some(node_rc) = stack.pop() {
        let (children, dat_opt) = {
            let n = node_rc.borrow();
            (n.children.clone(), n.dat.clone())
        };
        if let Some(dat_rc) = dat_opt {
            seen_dats.entry(Rc::as_ptr(&dat_rc)).or_insert_with(|| Rc::clone(&dat_rc));
        }
        for child in children {
            stack.push(child);
        }
    }

    let mut dats: Vec<Rc<RefCell<RvDat>>> = seen_dats.into_values().collect();
    dats.sort_by_key(|a| dat_display_name(&a.borrow()).to_ascii_lowercase());

    let mut complete = Vec::new();
    let mut empty = Vec::new();
    let mut partial = Vec::new();

    for dat_rc in dats {
        let (correct, missing, fixes_needed, partial_entries) = classify_dat(Rc::clone(&root), Rc::clone(&dat_rc));
        let dat_name = dat_display_name(&dat_rc.borrow());
        if correct > 0 && missing == 0 && fixes_needed == 0 {
            complete.push(dat_name);
        } else if correct == 0 && missing > 0 && fixes_needed == 0 {
            empty.push(dat_name);
        } else if (correct > 0 && missing > 0) || fixes_needed > 0 {
            partial.push((dat_name, partial_entries));
        }
    }

    writeln!(file, "Complete DAT Sets")?;
    writeln!(file, "-----------------------------------------")?;
    for name in complete {
        writeln!(file, "{}", name)?;
    }

    writeln!(file)?;
    writeln!(file)?;

    writeln!(file, "Empty DAT Sets")?;
    writeln!(file, "-----------------------------------------")?;
    for name in empty {
        writeln!(file, "{}", name)?;
    }

    writeln!(file)?;
    writeln!(file)?;

    writeln!(file, "Partial DAT Sets - (Listing Missing ROMs)")?;
    writeln!(file, "-----------------------------------------")?;
    for (name, entries) in partial {
        writeln!(file, "{}", name)?;

        let mut max_path = 0usize;
        let mut max_size = 0usize;
        let mut max_status = 0usize;
        for e in &entries {
            max_path = max_path.max(e.file_name.len());
            max_size = max_size.max(e.size.map(|v| v.to_string().len()).unwrap_or(0));
            max_status = max_status.max(format!("{:?}", e.rep_status).len());
        }

        writeln!(
            file,
            "+{}+{}+----------+{}+",
            "-".repeat(max_path + 2),
            "-".repeat(max_size + 2),
            "-".repeat(max_status + 2)
        )?;

        for e in &entries {
            let size_str = e.size.map(|v| v.to_string()).unwrap_or_default();
            let status_str = format!("{:?}", e.rep_status);
            writeln!(
                file,
                "| {:<path_w$} | {:>size_w$} | {:<8} | {:<status_w$} |",
                e.file_name,
                size_str,
                e.crc,
                status_str,
                path_w = max_path,
                size_w = max_size,
                status_w = max_status
            )?;
        }

        writeln!(
            file,
            "+{}+{}+----------+{}+",
            "-".repeat(max_path + 2),
            "-".repeat(max_size + 2),
            "-".repeat(max_status + 2)
        )?;
        writeln!(file)?;
    }

    Ok(())
}

#[cfg(test)]
#[path = "tests/reports_tests.rs"]
mod tests;
