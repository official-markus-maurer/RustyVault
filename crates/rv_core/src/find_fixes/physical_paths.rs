impl FindFixes {
    fn build_physical_path(file: Rc<RefCell<RvFile>>) -> PathBuf {
        let mut path_parts = Vec::new();
        let mut current = Some(file);

        while let Some(node_rc) = current {
            let node = node_rc.borrow();
            let component = node.name_case().to_string();
            if !component.is_empty() {
                path_parts.push(component);
            }
            current = node.parent.as_ref().and_then(|w| w.upgrade());
        }

        path_parts.reverse();
        let logical_path = path_parts.join("\\");
        let has_nested_absolute_component = path_parts
            .iter()
            .skip(1)
            .any(|part| Path::new(part).is_absolute());
        if Path::new(&logical_path).is_absolute() && !has_nested_absolute_component {
            return PathBuf::from(logical_path);
        }
        if let Some(mapped_path) = crate::settings::find_dir_mapping(&logical_path) {
            return PathBuf::from(mapped_path);
        }

        let mut path = PathBuf::new();
        for part in path_parts {
            if Path::new(&part).is_absolute() || path.as_os_str().is_empty() {
                path = PathBuf::from(part);
            } else {
                path.push(part);
            }
        }
        path
    }

    fn build_physical_identity(file: Rc<RefCell<RvFile>>) -> String {
        let mut member_parts = Vec::new();
        let mut current = Some(Rc::clone(&file));

        while let Some(node_rc) = current {
            let (name, parent) = {
                let node = node_rc.borrow();
                (
                    node.name_case().to_string(),
                    node.parent.as_ref().and_then(|w| w.upgrade()),
                )
            };

            let Some(parent_rc) = parent else {
                break;
            };

            let parent_file_type = parent_rc.borrow().file_type;
            if matches!(parent_file_type, FileType::Zip | FileType::SevenZip) {
                if !name.is_empty() {
                    member_parts.push(name);
                }
                member_parts.reverse();
                let archive_path = Self::build_physical_path(parent_rc);
                return format!("{}::{}", archive_path.to_string_lossy(), member_parts.join("/"));
            }

            if !name.is_empty() {
                member_parts.push(name);
            }
            current = Some(parent_rc);
        }

        Self::build_physical_path(file).to_string_lossy().to_string()
    }

    fn physical_identity_eq(left: &str, right: &str) -> bool {
        #[cfg(windows)]
        {
            fn normalize(identity: &str) -> String {
                let (path_part, member_part) =
                    identity.split_once("::").unwrap_or((identity, ""));
                let mut normalized_path = path_part.replace('/', "\\");
                while normalized_path.len() > 3 && normalized_path.ends_with('\\') {
                    normalized_path.pop();
                }
                if member_part.is_empty() {
                    normalized_path
                } else {
                    format!(
                        "{}::{}",
                        normalized_path,
                        member_part.replace('\\', "/").trim_matches('/'),
                    )
                }
            }

            normalize(left).eq_ignore_ascii_case(&normalize(right))
        }
        #[cfg(not(windows))]
        {
            left == right
        }
    }
}
