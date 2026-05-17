use std::path::Path;

use image::imageops::FilterType;

/// Longest edge for the editable preview buffer (keeps sliders responsive).
pub const MAX_PREVIEW_EDGE: u32 = 2048;

pub struct DecodedImage {
    pub rgba: Vec<u8>,
    pub width: u32,
    pub height: u32,
}

pub fn decode_for_edit(path: &Path) -> Option<DecodedImage> {
    let img = image::open(path).ok()?.into_rgba8();
    let (w, h) = img.dimensions();
    let img = if w.max(h) > MAX_PREVIEW_EDGE {
        let scale = MAX_PREVIEW_EDGE as f32 / w.max(h) as f32;
        let nw = ((w as f32 * scale).round() as u32).max(1);
        let nh = ((h as f32 * scale).round() as u32).max(1);
        image::imageops::resize(&img, nw, nh, FilterType::Triangle)
    } else {
        img
    };
    let (width, height) = img.dimensions();
    Some(DecodedImage {
        rgba: img.into_raw(),
        width,
        height,
    })
}
