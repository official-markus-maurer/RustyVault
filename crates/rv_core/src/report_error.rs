use std::fs::{self, File, OpenOptions};
use std::path::PathBuf;

pub fn update_log_path() -> PathBuf {
    let dir = std::env::current_dir().unwrap_or_default().join("Logs");
    let _ = fs::create_dir_all(&dir);
    let now = chrono::Local::now().format("%Y-%m-%d %H-%M-%S").to_string();
    dir.join(format!("{now} UpdateLog.txt"))
}

pub fn open_update_log_file() -> Option<File> {
    OpenOptions::new()
        .create(true)
        .append(true)
        .open(update_log_path())
        .ok()
}
