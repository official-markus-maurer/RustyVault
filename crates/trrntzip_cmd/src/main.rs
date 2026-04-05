use std::env;
use std::fs::File;
use std::io::{self, Write};
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use compress::structured_archive::ZipStructure;
use rv_io::directory::Directory;
use rv_io::directory_info::DirectoryInfo;
use trrntzip::torrent_zip::TorrentZip;

/// CLI tool for verifying and rebuilding `.zip` files into `TorrentZip` format.
///
/// `trrntzip_cmd` normalizes archive file metadata (like timestamps
/// and file ordering) so their CRCs are deterministic across all computers.
fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        println!();
        println!("trrntzip: missing path");
        println!("Usage: trrntzip [OPTIONS] [PATH/ZIP FILES]");
        return;
    }

    let mut no_recursion = false;
    let mut gui_launch = false;
    let mut tz = TorrentZip::new();
    let mut log_file: Option<File> = None;

    let mut i = 1;
    while i < args.len() {
        let arg = &args[i];

        if let Some(option) = arg.strip_prefix('-') {
            match option {
                "?" => {
                    println!(
                        "TorrentZip.Net v{} - Powered by RustyVault",
                        env!("CARGO_PKG_VERSION")
                    );
                    println!();
                    println!("Usage: trrntzip [OPTIONS] [PATH/ZIP FILE]");
                    println!();
                    println!("Options:");
                    println!("-? : show this help");
                    println!("-o : Set Output Archive Structure");
                    println!("     ZT  = Zip-Trrnt");
                    println!("     ZZ  = Zip-ZSTD");
                    println!("     7SL = 7Zip-Solid-LZMA");
                    println!("     7NL = 7Zip-NonSolid-LZMA");
                    println!("     7SZ = 7Zip-Solid-ZSTD");
                    println!("     7NZ = 7Zip-NonSolid-ZSTD");
                    println!("-s : prevent sub-directory recursion");
                    println!("-f : force re-zip");
                    println!("-c : Check files only do not repair");
                    println!("-l : verbose logging");
                    println!("-v : show version");
                    println!("-g : pause when finished");
                    return;
                }
                "o" => {
                    if i + 1 < args.len() {
                        let next_arg = &args[i + 1];
                        match next_arg.as_str() {
                            "ZT" => tz.out_zip_type = ZipStructure::ZipTrrnt,
                            "ZZ" => tz.out_zip_type = ZipStructure::ZipZSTD,
                            "7SL" => tz.out_zip_type = ZipStructure::SevenZipSLZMA,
                            "7NL" => tz.out_zip_type = ZipStructure::SevenZipNLZMA,
                            "7SZ" => tz.out_zip_type = ZipStructure::SevenZipSZSTD,
                            "7NZ" => tz.out_zip_type = ZipStructure::SevenZipNZSTD,
                            _ => {
                                println!("Unknown Output Archive Structure : {}", next_arg);
                                println!("Valid Structures are : ZT, ZZ, 7SL, 7NL, 7SZ, 7NZ");
                                return;
                            }
                        }
                        i += 1;
                    }
                }
                "s" => no_recursion = true,
                "f" => tz.force_rezip = true,
                "c" => tz.check_only = true,
                "l" => {
                    let now = SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_secs();
                    let log_name = format!("outlog-{}.txt", now);
                    if let Ok(file) = File::create(&log_name) {
                        log_file = Some(file);
                    }
                }
                "v" => {
                    println!("TorrentZip v{}", env!("CARGO_PKG_VERSION"));
                    return;
                }
                "g" => gui_launch = true,
                _ => {}
            }
        }
        i += 1;
    }

    for arg in args.iter().skip(1) {
        if arg.starts_with('-') {
            continue;
        }

        let mut target = arg.clone();
        if let Some(stripped) = target.strip_prefix(".\\") {
            target = stripped.to_string();
        }

        if Directory::exists(&target) {
            process_dir(&target, &tz, no_recursion, &mut log_file);
            continue;
        }

        let path = Path::new(&target);
        let mut dir = path
            .parent()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_default();
        if dir.is_empty() {
            dir = env::current_dir()
                .ok()
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_default();
        }
        let pattern = path
            .file_name()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or(target.clone());
        let dir_info = DirectoryInfo::new(&dir);
        let files = dir_info.get_files(&pattern);
        for file in files {
            let ext = Path::new(&file.full_name)
                .extension()
                .unwrap_or_default()
                .to_string_lossy()
                .to_lowercase();
            if ext == "zip" || ext == "7z" {
                log_line(&mut log_file, &format!("Processing: {}", file.full_name));
                println!("Processing: {}", file.full_name);
                let status = tz.process(&file.full_name);
                log_line(&mut log_file, &format!("Result: {:?}", status));
                println!("Result: {:?}", status);
            }
        }
    }

    if let Some(f) = log_file.as_mut() {
        let _ = f.flush();
    }

    if gui_launch {
        println!("Complete. Press Enter to exit.");
        let mut input = String::new();
        let _ = io::stdin().read_line(&mut input);
    }
}

fn log_line(file: &mut Option<File>, line: &str) {
    if let Some(f) = file.as_mut() {
        let _ = writeln!(f, "{}", line);
    }
}

fn process_dir(dir_name: &str, tz: &TorrentZip, no_recursion: bool, log_file: &mut Option<File>) {
    log_line(log_file, &format!("Checking Dir : {}", dir_name));
    println!("Checking Dir : {}", dir_name);

    let dir_info = DirectoryInfo::new(dir_name);
    let files = dir_info.get_files("");

    for file in files {
        let ext = Path::new(&file.full_name)
            .extension()
            .unwrap_or_default()
            .to_string_lossy()
            .to_lowercase();
        if ext == "zip" || ext == "7z" {
            log_line(log_file, &format!("Processing: {}", file.full_name));
            println!("Processing: {}", file.full_name);
            let status = tz.process(&file.full_name);
            log_line(log_file, &format!("Result: {:?}", status));
            println!("Result: {:?}", status);
        }
    }

    if !no_recursion {
        let dirs = dir_info.get_directories();
        for dir in dirs {
            process_dir(&dir.full_name, tz, no_recursion, log_file);
        }
    }
}
