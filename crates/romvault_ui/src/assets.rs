/// Asset loading macros and utilities.
/// 
/// `assets.rs` provides macros like `include_asset!` and `include_toolbar_image!`
/// to embed static image resources directly into the Rust binary at compile time.
/// 
/// Differences from C#:
/// - C# uses `.resx` files and Visual Studio's built-in resource manager.
/// - Rust utilizes `include_bytes!` and an internal `egui` caching layer to embed
///   and serve raw PNG/SVG bytes efficiently to the immediate-mode UI renderer.
#[macro_export]
macro_rules! include_asset {
    ($file:literal) => {
        eframe::egui::ImageSource::Bytes {
            uri: std::borrow::Cow::Borrowed(concat!("bytes://", $file)),
            bytes: include_bytes!(concat!("../../../assets/images/", $file)).into(),
        }
    };
}

pub fn processed_image_source(
    name: &'static str,
    raw_bytes: &'static [u8],
) -> eframe::egui::ImageSource<'static> {
    use eframe::egui;
    use std::borrow::Cow;
    use std::collections::HashMap;
    use std::sync::{Mutex, OnceLock};

    static CACHE: OnceLock<Mutex<HashMap<&'static str, egui::load::Bytes>>> = OnceLock::new();
    let cache = CACHE.get_or_init(|| Mutex::new(HashMap::new()));

    if let Ok(cache) = cache.lock() {
        if let Some(bytes) = cache.get(name) {
            return egui::ImageSource::Bytes {
                uri: Cow::Owned(format!("bytes://processed/{}", name)),
                bytes: bytes.clone(),
            };
        }
    }

    let processed: Vec<u8> = raw_bytes.to_vec();

    let bytes: egui::load::Bytes = processed.into();
    if let Ok(mut cache) = cache.lock() {
        cache.insert(name, bytes.clone());
    }
    egui::ImageSource::Bytes {
        uri: Cow::Owned(format!("bytes://processed/{}", name)),
        bytes,
    }
}

#[macro_export]
macro_rules! include_toolbar_image {
    ($file:literal) => {
        $crate::assets::processed_image_source(
            $file,
            include_bytes!(concat!("../../../assets/images/", $file)),
        )
    };
}
