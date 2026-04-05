/// Asset loading macros and utilities.
///
/// Provides the `include_asset!` and `include_toolbar_image!` macros used throughout the UI.
/// Assets are resolved in this order:
/// - Environment overrides (`RUSTYROMS_ASSETS_DARK` / `RUSTYROMS_ASSETS_LIGHT`)
/// - Embedded bytes (compiled into the binary)
/// - Files on disk next to the executable or working directory (development convenience)
static DARK_MODE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(true);
pub(crate) static FALLBACK_PNG_1X1_TRANSPARENT: &[u8] = &[
    0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x48, 0x44, 0x52,
    0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x04, 0x00, 0x00, 0x00, 0xB5, 0x1C, 0x0C,
    0x02, 0x00, 0x00, 0x00, 0x0B, 0x49, 0x44, 0x41, 0x54, 0x78, 0xDA, 0x63, 0xFC, 0xFF, 0x1F, 0x00,
    0x03, 0x03, 0x01, 0xFF, 0xA5, 0xFC, 0x91, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4E, 0x44, 0xAE,
    0x42, 0x60, 0x82,
];

mod embedded_assets {
    include!(concat!(env!("OUT_DIR"), "/embedded_assets.rs"));
}

#[macro_export]
macro_rules! include_asset {
    ($file:literal) => {
        $crate::assets::themed_image_source($file, $crate::assets::FALLBACK_PNG_1X1_TRANSPARENT)
    };
}

pub fn set_dark_mode(is_dark: bool) {
    DARK_MODE.store(is_dark, std::sync::atomic::Ordering::Relaxed);
}

#[macro_export]
macro_rules! include_toolbar_image {
    ($file:literal) => {
        $crate::assets::themed_image_source($file, $crate::assets::FALLBACK_PNG_1X1_TRANSPARENT)
    };
}

fn is_dark_mode() -> bool {
    DARK_MODE.load(std::sync::atomic::Ordering::Relaxed)
}

fn try_read_asset_file(is_dark: bool, name: &str) -> Option<Vec<u8>> {
    let env_key = if is_dark {
        "RUSTYROMS_ASSETS_DARK"
    } else {
        "RUSTYROMS_ASSETS_LIGHT"
    };
    if let Ok(dir) = std::env::var(env_key) {
        let base = std::path::PathBuf::from(dir);
        let p = base.join(name);
        if let Ok(bytes) = std::fs::read(&p) {
            return Some(bytes);
        }
    }

    if let Some(bytes) = embedded_assets::embedded_asset_bytes(is_dark, name) {
        return Some(bytes.to_vec());
    }

    let mut roots: Vec<std::path::PathBuf> = Vec::new();
    if let Ok(exe) = std::env::current_exe() {
        if let Some(exe_dir) = exe.parent() {
            roots.push(
                exe_dir
                    .join("assets")
                    .join(if is_dark { "dark" } else { "light" }),
            );
            roots.push(exe_dir.join("assets").join(if is_dark {
                "images_dark"
            } else {
                "images_light"
            }));
        }
    }
    if let Ok(cwd) = std::env::current_dir() {
        roots.push(
            cwd.join("assets")
                .join(if is_dark { "dark" } else { "light" }),
        );
        roots.push(cwd.join("assets").join(if is_dark {
            "images_dark"
        } else {
            "images_light"
        }));
    }

    for root in roots {
        let p = root.join(name);
        if let Ok(bytes) = std::fs::read(&p) {
            return Some(bytes);
        }
    }

    None
}

pub fn themed_image_source(
    name: &'static str,
    fallback_bytes: &'static [u8],
) -> eframe::egui::ImageSource<'static> {
    use eframe::egui;
    use std::borrow::Cow;
    use std::collections::HashMap;
    use std::sync::{Mutex, OnceLock};

    static CACHE: OnceLock<Mutex<HashMap<String, egui::load::Bytes>>> = OnceLock::new();
    let cache = CACHE.get_or_init(|| Mutex::new(HashMap::new()));

    let is_dark = is_dark_mode();
    let theme_key = if is_dark { "dark" } else { "light" };
    let cache_key = format!("{}/{}", theme_key, name);

    if let Ok(cache) = cache.lock() {
        if let Some(bytes) = cache.get(&cache_key) {
            return egui::ImageSource::Bytes {
                uri: Cow::Owned(format!("bytes://{}", cache_key)),
                bytes: bytes.clone(),
            };
        }
    }

    let raw = try_read_asset_file(is_dark, name).unwrap_or_else(|| fallback_bytes.to_vec());
    let bytes: egui::load::Bytes = raw.into();

    if let Ok(mut cache) = cache.lock() {
        cache.insert(cache_key.clone(), bytes.clone());
    }

    egui::ImageSource::Bytes {
        uri: Cow::Owned(format!("bytes://{}", cache_key)),
        bytes,
    }
}

pub fn themed_image_source_optional(
    name: &'static str,
) -> Option<eframe::egui::ImageSource<'static>> {
    use eframe::egui;
    use std::borrow::Cow;
    use std::collections::HashMap;
    use std::sync::{Mutex, OnceLock};

    static CACHE: OnceLock<Mutex<HashMap<String, egui::load::Bytes>>> = OnceLock::new();
    let cache = CACHE.get_or_init(|| Mutex::new(HashMap::new()));

    let is_dark = is_dark_mode();
    let theme_key = if is_dark { "dark" } else { "light" };
    let cache_key = format!("{}/{}", theme_key, name);

    if let Ok(cache) = cache.lock() {
        if let Some(bytes) = cache.get(&cache_key) {
            return Some(egui::ImageSource::Bytes {
                uri: Cow::Owned(format!("bytes://{}", cache_key)),
                bytes: bytes.clone(),
            });
        }
    }

    let raw = try_read_asset_file(is_dark, name)?;
    let bytes: egui::load::Bytes = raw.into();

    if let Ok(mut cache) = cache.lock() {
        cache.insert(cache_key.clone(), bytes.clone());
    }

    Some(egui::ImageSource::Bytes {
        uri: Cow::Owned(format!("bytes://{}", cache_key)),
        bytes,
    })
}
