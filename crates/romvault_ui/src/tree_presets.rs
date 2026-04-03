use std::cell::RefCell;
use std::fs;
use std::rc::Rc;

use rv_core::rv_file::{RvFile, TreeSelect};

#[derive(Clone)]
pub(crate) struct PresetEntry {
    pub path: String,
    pub selected: TreeSelect,
    pub expanded: bool,
}

fn escape_xml(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            _ => out.push(ch),
        }
    }
    out
}

fn unescape_xml(s: &str) -> String {
    s.replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&amp;", "&")
}

fn extract_tag_value<'a>(xml: &'a str, tag: &str) -> Option<&'a str> {
    let open = format!("<{}>", tag);
    let close = format!("</{}>", tag);
    let start = xml.find(&open)? + open.len();
    let end = xml[start..].find(&close)? + start;
    Some(xml[start..end].trim())
}

fn parse_selected(s: &str) -> Option<TreeSelect> {
    match s.trim() {
        "UnSelected" => Some(TreeSelect::UnSelected),
        "Selected" => Some(TreeSelect::Selected),
        "Locked" => Some(TreeSelect::Locked),
        _ => None,
    }
}

fn selected_to_string(sel: TreeSelect) -> &'static str {
    match sel {
        TreeSelect::UnSelected => "UnSelected",
        TreeSelect::Selected => "Selected",
        TreeSelect::Locked => "Locked",
    }
}

pub(crate) fn read_preset_file(path: &str) -> Option<Vec<PresetEntry>> {
    let xml = fs::read_to_string(path).ok()?;
    let mut out = Vec::new();
    let mut cursor = 0usize;
    while let Some(start_idx) = xml[cursor..].find("<Entry>") {
        let start = cursor + start_idx + "<Entry>".len();
        let end_rel = xml[start..].find("</Entry>")?;
        let end = start + end_rel;
        let entry_xml = &xml[start..end];

        let path_val = extract_tag_value(entry_xml, "Path")?;
        let selected_val = extract_tag_value(entry_xml, "Selected")?;
        let expanded_val = extract_tag_value(entry_xml, "Expanded")?;

        let selected = parse_selected(selected_val)?;
        let expanded = expanded_val.eq_ignore_ascii_case("true");

        out.push(PresetEntry {
            path: unescape_xml(path_val),
            selected,
            expanded,
        });

        cursor = end + "</Entry>".len();
    }
    Some(out)
}

pub(crate) fn write_preset_file(path: &str, entries: &[PresetEntry]) -> std::io::Result<()> {
    let mut out = String::new();
    out.push_str("<?xml version=\"1.0\"?>\n");
    out.push_str("<ArrayOfEntry xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\" xmlns:xsd=\"http://www.w3.org/2001/XMLSchema\">\n");
    for entry in entries {
        out.push_str("  <Entry>\n");
        out.push_str("    <Path>");
        out.push_str(&escape_xml(&entry.path));
        out.push_str("</Path>\n");
        out.push_str("    <Selected>");
        out.push_str(selected_to_string(entry.selected));
        out.push_str("</Selected>\n");
        out.push_str("    <Expanded>");
        out.push_str(if entry.expanded { "true" } else { "false" });
        out.push_str("</Expanded>\n");
        out.push_str("  </Entry>\n");
    }
    out.push_str("</ArrayOfEntry>\n");
    fs::write(path, out)
}

pub(crate) fn collect_tree_state(root: Rc<RefCell<RvFile>>) -> Vec<PresetEntry> {
    fn walk(node: Rc<RefCell<RvFile>>, path: String, out: &mut Vec<PresetEntry>) {
        let n = node.borrow();
        if !n.is_directory() {
            return;
        }
        let node_path = if path.is_empty() {
            n.name.clone()
        } else {
            format!("{}\\{}", path, n.name)
        };
        out.push(PresetEntry {
            path: node_path.clone(),
            selected: n.tree_checked,
            expanded: n.tree_expanded,
        });
        let children = n.children.clone();
        drop(n);
        for child in children {
            walk(child, node_path.clone(), out);
        }
    }

    let mut out = Vec::new();
    let children = root.borrow().children.clone();
    for child in children {
        walk(child, String::new(), &mut out);
    }
    out
}

pub(crate) fn apply_tree_state(root: Rc<RefCell<RvFile>>, entries: &[PresetEntry]) {
    for entry in entries {
        let path_parts: Vec<&str> = entry.path.split('\\').filter(|p| !p.is_empty()).collect();
        let mut current = Rc::clone(&root);
        let mut found = true;

        for part in path_parts {
            let next = {
                let n = current.borrow();
                n.children
                    .iter()
                    .find(|c| c.borrow().name == part)
                    .map(Rc::clone)
            };
            if let Some(n) = next {
                current = n;
            } else {
                found = false;
                break;
            }
        }

        if found {
            let mut n = current.borrow_mut();
            n.tree_checked = entry.selected;
            n.tree_expanded = entry.expanded;
        }
    }
}
