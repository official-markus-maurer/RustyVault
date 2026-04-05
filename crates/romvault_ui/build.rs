use std::env;
use std::fs;
use std::path::{Path, PathBuf};

fn list_png_files(dir: &Path) -> Vec<String> {
    let mut out = Vec::new();
    let Ok(entries) = fs::read_dir(dir) else {
        return out;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        if path
            .extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| e.eq_ignore_ascii_case("png"))
        {
            if let Some(name) = path.file_name().and_then(|s| s.to_str()) {
                out.push(name.to_string());
            }
        }
    }
    out.sort();
    out
}

fn rel_asset_path(theme_dir: &str, file_name: &str) -> String {
    format!("/../../assets/{}/{}", theme_dir, file_name)
}

fn write_mapping(out: &mut String, is_dark: bool, theme_dir: &str, files: &[String]) {
    for name in files {
        let rel = rel_asset_path(theme_dir, name);
        let bool_key = if is_dark { "true" } else { "false" };
        out.push_str(&format!(
            "        ({bool_key}, \"{name}\") => Some(include_bytes!(concat!(env!(\"CARGO_MANIFEST_DIR\"), \"{rel}\")) as &[u8]),\n"
        ));
    }
}

fn main() {
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let project_root = manifest_dir.join("..").join("..");
    let dark_dir = project_root.join("assets").join("images_dark");
    let light_dir = project_root.join("assets").join("images_light");

    let dark_files = list_png_files(&dark_dir);
    let light_files = list_png_files(&light_dir);

    for name in &dark_files {
        println!(
            "cargo:rerun-if-changed={}",
            dark_dir.join(name).to_string_lossy()
        );
    }
    for name in &light_files {
        println!(
            "cargo:rerun-if-changed={}",
            light_dir.join(name).to_string_lossy()
        );
    }

    let mut code = String::new();
    code.push_str(
        "pub fn embedded_asset_bytes(is_dark: bool, name: &str) -> Option<&'static [u8]> {\n",
    );
    code.push_str("    match (is_dark, name) {\n");
    write_mapping(&mut code, true, "images_dark", &dark_files);
    write_mapping(&mut code, false, "images_light", &light_files);
    code.push_str("        _ => None,\n");
    code.push_str("    }\n");
    code.push_str("}\n");

    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    let dest_path = out_dir.join("embedded_assets.rs");
    fs::write(dest_path, code).unwrap();
}
