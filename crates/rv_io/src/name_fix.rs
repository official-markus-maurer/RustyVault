use std::path::PathBuf;

pub struct NameFix;

impl NameFix {
    pub fn add_long_path_prefix(path: &str) -> String {
        if cfg!(unix) {
            return path.to_string();
        }
        if path.is_empty() || path.starts_with(r"\\?\") {
            return path.to_string();
        }
        if let Some(stripped) = path.strip_prefix(r"\\") {
            return format!(r"\\?\UNC\{}", stripped);
        }

        let mut ret = path.to_string();
        if ret.len() > 2 && !ret[1..2].eq(":") {
            if let Ok(cwd) = std::env::current_dir() {
                let joined: PathBuf = cwd.join(&ret);
                ret = joined.to_string_lossy().into_owned();
            }
        }
        ret = Self::clean_dots(&ret);
        format!(r"\\?\{}", ret)
    }

    pub fn remove_long_path_prefix(path: &str) -> String {
        if cfg!(unix) {
            return path.to_string();
        }
        if let Some(stripped) = path.strip_prefix(r"\\?\UNC\") {
            return format!(r"\\{}", stripped);
        }
        if let Some(stripped) = path.strip_prefix(r"\\?\") {
            return stripped.to_string();
        }
        path.to_string()
    }

    fn clean_dots(path: &str) -> String {
        let mut ret = path.to_string();
        loop {
            let Some(index) = ret.find(r"\..\") else {
                break;
            };
            let path1 = &ret[..index];
            let path2 = &ret[index + 4..];
            let Some(path1_back) = path1.rfind('\\') else {
                ret = path2.to_string();
                continue;
            };
            let mut new_path = String::new();
            new_path.push_str(&path1[..path1_back + 1]);
            new_path.push_str(path2);
            ret = new_path;
        }
        ret
    }
}
