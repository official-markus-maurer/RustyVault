use std::path::Path as StdPath;

pub struct Path;

impl Path {
    pub fn get_file_name(path: &str) -> String {
        StdPath::new(path)
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned()
    }

    pub fn combine(path1: &str, path2: &str) -> String {
        StdPath::new(path1)
            .join(path2)
            .to_string_lossy()
            .into_owned()
    }
}
