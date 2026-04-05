impl Fix {
    fn logical_name_eq(left: &str, right: &str) -> bool {
        #[cfg(windows)]
        {
            left.eq_ignore_ascii_case(right)
        }
        #[cfg(not(windows))]
        {
            left == right
        }
    }

    fn physical_path_eq_for_rename(left: &Path, right: &Path) -> bool {
        #[cfg(windows)]
        {
            left.to_string_lossy().eq_ignore_ascii_case(&right.to_string_lossy())
        }
        #[cfg(not(windows))]
        {
            left == right
        }
    }

    fn rename_path_if_needed(
        current_path: &Path,
        target_path: &Path,
        temp_suffix: &str,
    ) -> std::io::Result<()> {
        if current_path == target_path || !current_path.exists() {
            return Ok(());
        }

        if Self::physical_path_eq_for_rename(current_path, target_path) {
            let mut temp_path = current_path.to_path_buf();
            let temp_name = format!(
                "{}.{}-{}",
                target_path
                    .file_name()
                    .and_then(|name| name.to_str())
                    .unwrap_or("tmp"),
                temp_suffix,
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .map(|d| d.as_nanos())
                    .unwrap_or_default()
            );
            temp_path.set_file_name(temp_name);
            fs::rename(current_path, &temp_path).and_then(|_| fs::rename(&temp_path, target_path))
        } else {
            fs::rename(current_path, target_path)
        }
    }

    fn get_tosort_path(file_path: &str, base_dir: &str) -> String {
        let path = Path::new(file_path);
        let file_name = path.file_name().and_then(|name| name.to_str()).unwrap_or_default();

        let mapped_base_dir = base_dir.replace('/', "\\");
        let mapped_base_path =
            crate::settings::find_dir_mapping(&mapped_base_dir).unwrap_or_else(|| mapped_base_dir.clone());
        if let Some((source_logical_key, source_root_path)) =
            crate::settings::find_mapping_for_physical_path(path)
        {
            if let Some(relative_path) = crate::settings::strip_physical_prefix(path, &source_root_path)
            {
                let mut relative_dirs: Vec<String> = relative_path
                    .parent()
                    .map(|parent| {
                        parent
                            .components()
                            .filter_map(|component| match component {
                                std::path::Component::Normal(part) => {
                                    Some(part.to_string_lossy().to_string())
                                }
                                _ => None,
                            })
                            .collect()
                    })
                    .unwrap_or_default();

                if Self::logical_name_eq(&source_logical_key, "ToSort")
                    && Self::logical_name_eq(&mapped_base_dir, "ToSort\\Corrupt")
                    && relative_dirs.first().is_some_and(|s| Self::logical_name_eq(s, "Corrupt"))
                {
                    relative_dirs.remove(0);
                }

                let mut dir_path = PathBuf::from(&mapped_base_path);
                for part in &relative_dirs {
                    dir_path.push(part);
                }
                let _ = fs::create_dir_all(&dir_path);

                let mut target_path_buf = dir_path.join(file_name);
                if target_path_buf == path {
                    return target_path_buf.to_string_lossy().replace('\\', "/");
                }

                let mut target_path = target_path_buf.to_string_lossy().replace('\\', "/");
                let mut counter = 0;
                while Path::new(&target_path).exists() {
                    let file_stem = path.file_stem().unwrap().to_str().unwrap();
                    let ext = path.extension().map(|e| e.to_str().unwrap()).unwrap_or("");

                    let new_name = if ext.is_empty() {
                        format!("{}_{}", file_stem, counter)
                    } else {
                        format!("{}_{}.{}", file_stem, counter, ext)
                    };
                    target_path_buf = dir_path.join(new_name);
                    target_path = target_path_buf.to_string_lossy().replace('\\', "/");
                    counter += 1;
                }

                return target_path;
            }
        }

        let mut root_base = PathBuf::new();
        let mut normal_components = Vec::new();

        for component in path.components() {
            match component {
                std::path::Component::Prefix(_) | std::path::Component::RootDir => {
                    root_base.push(component.as_os_str());
                }
                std::path::Component::Normal(part) => {
                    normal_components.push(part.to_string_lossy().to_string());
                }
                _ => {}
            }
        }

        if let Some(first_normal) = normal_components.first() {
            root_base.push(first_normal);
        }

        let mut relative_dirs = if normal_components.len() > 1 {
            normal_components[1..normal_components.len().saturating_sub(1)].to_vec()
        } else {
            Vec::new()
        };

        let normalized_base_dir = base_dir.replace('/', "\\");
        let base_parts: Vec<String> = normalized_base_dir
            .split('\\')
            .filter(|part| !part.is_empty())
            .map(|part| part.to_string())
            .collect();

        let shares_root = normal_components
            .first()
            .zip(base_parts.first())
            .is_some_and(|(path_root, base_root)| Self::logical_name_eq(path_root, base_root));

        if shares_root {
            let base_suffix: Vec<String> = base_parts.iter().skip(1).cloned().collect();
            let already_prefixed = relative_dirs.len() >= base_suffix.len()
                && base_suffix
                    .iter()
                    .zip(relative_dirs.iter())
                    .all(|(base_part, relative_part)| Self::logical_name_eq(base_part, relative_part));

            if !base_suffix.is_empty() && !already_prefixed {
                let mut prefixed = base_suffix;
                prefixed.extend(relative_dirs);
                relative_dirs = prefixed;
            }
        } else {
            let mut prefixed = base_parts.clone();
            prefixed.extend(relative_dirs);
            relative_dirs = prefixed;
        }

        let mut dir_path = root_base;
        for part in &relative_dirs {
            dir_path.push(part);
        }

        let _ = fs::create_dir_all(&dir_path);

        let mut target_path_buf = dir_path.join(file_name);
        if target_path_buf == path {
            return target_path_buf.to_string_lossy().replace('\\', "/");
        }

        let mut target_path = target_path_buf.to_string_lossy().replace('\\', "/");
        let mut counter = 0;
        while Path::new(&target_path).exists() {
            let file_stem = path.file_stem().unwrap().to_str().unwrap();
            let ext = path.extension().map(|e| e.to_str().unwrap()).unwrap_or("");

            let new_name = if ext.is_empty() {
                format!("{}_{}", file_stem, counter)
            } else {
                format!("{}_{}.{}", file_stem, counter, ext)
            };
            target_path_buf = dir_path.join(new_name);
            target_path = target_path_buf.to_string_lossy().replace('\\', "/");
            counter += 1;
        }

        target_path
    }
}
