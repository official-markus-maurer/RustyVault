use std::path::{Component, Path};

pub fn make_relative(from_directory: &str, to_path: &str) -> String {
    let from = Path::new(from_directory);
    let to = Path::new(to_path);

    let is_rooted = from.is_absolute() && to.is_absolute();
    if is_rooted && from.components().next() != to.components().next() {
        return to_path.to_string();
    }

    let from_parts: Vec<String> = from
        .components()
        .filter_map(|c| match c {
            Component::Normal(p) => Some(p.to_string_lossy().to_string()),
            _ => None,
        })
        .collect();
    let to_parts: Vec<String> = to
        .components()
        .filter_map(|c| match c {
            Component::Normal(p) => Some(p.to_string_lossy().to_string()),
            _ => None,
        })
        .collect();

    let length = std::cmp::min(from_parts.len(), to_parts.len());
    let mut last_common_root: isize = -1;
    for i in 0..length {
        #[cfg(windows)]
        let same = from_parts[i].eq_ignore_ascii_case(&to_parts[i]);
        #[cfg(not(windows))]
        let same = from_parts[i] == to_parts[i];
        if !same {
            break;
        }
        last_common_root = i as isize;
    }

    if last_common_root == -1 {
        return to_path.to_string();
    }

    let mut relative: Vec<String> = Vec::new();
    for part in from_parts.iter().skip(last_common_root as usize + 1) {
        if !part.is_empty() {
            relative.push("..".to_string());
        }
    }
    for part in to_parts.iter().skip(last_common_root as usize + 1) {
        relative.push(part.clone());
    }

    relative.join("\\")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn relative_path_basic() {
        assert_eq!(
            make_relative("C:\\a\\b\\c", "C:\\a\\b\\d\\e"),
            "..\\d\\e"
        );
    }
}
