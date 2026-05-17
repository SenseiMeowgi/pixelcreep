use std::path::{Path, PathBuf};

pub const IMAGE_EXTENSIONS: &[&str] = &[
    "avif", "bmp", "gif", "heic", "heif", "ico", "jpeg", "jpg", "png", "tif", "tiff", "webp",
];

pub struct ImageSelection {
    pub path: PathBuf,
    pub label: String,
}

pub fn is_supported_image(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .is_some_and(|ext| IMAGE_EXTENSIONS.contains(&ext.to_ascii_lowercase().as_str()))
}

pub fn selection_from_path(path: PathBuf) -> Option<ImageSelection> {
    if !is_supported_image(&path) {
        return None;
    }

    let label = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("Image")
        .to_owned();

    Some(ImageSelection { path, label })
}
