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
    use image::ImageEncoder;
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

    let processed: Vec<u8> = match image::load_from_memory(raw_bytes) {
        Ok(img) => {
            let mut rgba = img.to_rgba8();
            let (w, h) = rgba.dimensions();

            // Exact color-key: remove only pixels that exactly match the top-left color.
            // This keeps edges and text pixels untouched to avoid "scuffed" outlines.
            let bg = rgba.get_pixel(0, 0).0;
            for p in rgba.pixels_mut() {
                if p.0[0] == bg[0] && p.0[1] == bg[1] && p.0[2] == bg[2] && p.0[3] == bg[3] {
                    p.0[3] = 0;
                }
            }

            let mut out = Vec::new();
            if image::codecs::png::PngEncoder::new(&mut out)
                .write_image(rgba.as_raw(), w, h, image::ExtendedColorType::Rgba8)
                .is_ok()
            {
                out
            } else {
                raw_bytes.to_vec()
            }
        }
        Err(_) => raw_bytes.to_vec(),
    };

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
