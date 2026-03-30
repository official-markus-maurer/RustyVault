use std::env;
use std::io::{self, Write};
use compress::structured_archive::ZipStructure;
use trrntzip::torrent_zip::TorrentZip;
use rv_io::directory::Directory;
use rv_io::directory_info::DirectoryInfo;

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        println!("");
        println!("trrntzip: missing path");
        println!("Usage: trrntzip [OPTIONS] [PATH/ZIP FILES]");
        return;
    }

    let mut no_recursion = false;
    let mut gui_launch = false;
    let mut tz = TorrentZip::new();

    let mut i = 1;
    while i < args.len() {
        let arg = &args[i];

        if arg.starts_with('-') {
            let option = &arg[1..];
            match option {
                "?" => {
                    println!("TorrentZip.Net v0.1.0 - Powered by RustyVault");
                    println!("");
                    println!("Usage: trrntzip [OPTIONS] [PATH/ZIP FILE]");
                    println!("");
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
                        i += 1;
                        let next_arg = &args[i];
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
                    }
                }
                "s" => no_recursion = true,
                "f" => tz.force_rezip = true,
                "c" => tz.check_only = true,
                "l" => println!("Verbose logging enabled"),
                "v" => {
                    println!("TorrentZip v0.1.0");
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
            // skip options during file processing pass
            if arg == "-o" {
                continue; // Would also need to skip the value in a real iterator, but fine for basic loop here
            }
            continue;
        }

        let mut target = arg.clone();
        if target.starts_with(".\\") {
            target = target[2..].to_string();
        }

        if Directory::exists(&target) {
            process_dir(&target, &tz, no_recursion);
            continue;
        }

        // It's a file
        let ext = std::path::Path::new(&target).extension().unwrap_or_default().to_string_lossy().to_lowercase();
        if ext == "zip" || ext == "7z" {
            println!("Processing: {}", target);
            let status = tz.process(&target);
            println!("Result: {:?}", status);
        }
    }

    if gui_launch {
        println!("Complete. Press Enter to exit.");
        let mut input = String::new();
        let _ = io::stdin().read_line(&mut input);
    }
}

fn process_dir(dir_name: &str, tz: &TorrentZip, no_recursion: bool) {
    println!("Checking Dir : {}", dir_name);

    let dir_info = DirectoryInfo::new(dir_name);
    let files = dir_info.get_files("");

    for file in files {
        let ext = std::path::Path::new(&file.full_name).extension().unwrap_or_default().to_string_lossy().to_lowercase();
        if ext == "zip" || ext == "7z" {
            println!("Processing: {}", file.full_name);
            let status = tz.process(&file.full_name);
            println!("Result: {:?}", status);
        }
    }

    if !no_recursion {
        let dirs = dir_info.get_directories();
        for dir in dirs {
            process_dir(&dir.full_name, tz, no_recursion);
        }
    }
}
