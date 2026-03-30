use std::rc::Rc;
use std::cell::RefCell;
use std::io::Read as _;
use rv_core::rv_file::RvFile;

pub fn get_full_node_path(node: Rc<RefCell<RvFile>>) -> String {
    let mut path_parts = Vec::new();
    let mut current = Some(node);
    while let Some(n) = current {
        let name = n.borrow().name.clone();
        if !name.is_empty() {
            path_parts.push(name);
        }
        let parent = n.borrow().parent.as_ref().and_then(|w| w.upgrade());
        current = parent;
    }
    path_parts.reverse();
    path_parts.join("\\")
}

pub fn extract_text_from_zip(zip_path: &str, file_ext: &str) -> Option<String> {
    let file = std::fs::File::open(zip_path).ok()?;
    let mut archive = zip::ZipArchive::new(file).ok()?;
    for i in 0..archive.len() {
        if let Ok(mut file) = archive.by_index(i) {
            if let Some(name) = file.enclosed_name().map(|n| n.to_owned()) {
                if name.to_string_lossy().to_lowercase().ends_with(file_ext) {
                    let mut buffer = Vec::new();
                    if file.read_to_end(&mut buffer).is_ok() {
                        return Some(String::from_utf8_lossy(&buffer).into_owned());
                    }
                }
            }
        }
    }
    None
}

pub fn extract_image_from_zip(zip_path: &str, game_name: &str) -> Option<Vec<u8>> {
    let file = std::fs::File::open(zip_path).ok()?;
    let mut archive = zip::ZipArchive::new(file).ok()?;

    let target_png = format!("{}.png", game_name);
    let target_jpg = format!("{}.jpg", game_name);

    if let Ok(mut file) = archive.by_name(&target_png) {
        let mut buffer = Vec::new();
        if file.read_to_end(&mut buffer).is_ok() {
            return Some(buffer);
        }
    }
    if let Ok(mut file) = archive.by_name(&target_jpg) {
        let mut buffer = Vec::new();
        if file.read_to_end(&mut buffer).is_ok() {
            return Some(buffer);
        }
    }

    let target_png_lower = target_png.to_lowercase();
    let target_jpg_lower = target_jpg.to_lowercase();
    for i in 0..archive.len() {
        if let Ok(mut file) = archive.by_index(i) {
            if let Some(name) = file.enclosed_name().map(|n| n.to_owned()) {
                let n = name.to_string_lossy().to_lowercase();
                if n.ends_with(&target_png_lower) || n.ends_with(&target_jpg_lower) {
                    let mut buffer = Vec::new();
                    if file.read_to_end(&mut buffer).is_ok() {
                        return Some(buffer);
                    }
                }
            }
        }
    }
    None
}

#[allow(dead_code)]
pub fn format_number(n: i32) -> String {
    let s = n.to_string();
    let mut result = String::with_capacity(s.len() + s.len() / 3);
    let mut count = 0;
    for ch in s.chars().rev() {
        if count == 3 {
            result.push(',');
            count = 0;
        }
        result.push(ch);
        count += 1;
    }
    result.chars().rev().collect()
}
