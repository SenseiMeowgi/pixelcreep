use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::file_types::is_supported_image;

pub const RECENT_IMAGE_LIMIT: usize = 5;

#[derive(Debug, Default, Deserialize, Serialize)]
pub struct AppConfig {
    #[serde(default)]
    pub recent_images: Vec<PathBuf>,
}

impl AppConfig {
    pub fn clean_recent_images(&mut self) -> bool {
        let before = self.recent_images.clone();
        self.recent_images = clean_recent_images(&self.recent_images, |path| {
            path.exists() && is_supported_image(path)
        });
        self.recent_images != before
    }

    pub fn record_recent_image(&mut self, path: &Path) {
        self.recent_images = record_recent_image(&self.recent_images, path);
    }
}

pub fn load() -> AppConfig {
    let Some(path) = config_path() else {
        return AppConfig::default();
    };

    fs::read_to_string(path)
        .ok()
        .map(|content| parse_config(&content))
        .unwrap_or_default()
}

pub fn save(config: &AppConfig) -> io::Result<()> {
    let Some(path) = config_path() else {
        return Ok(());
    };

    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }

    let content = serde_json::to_string_pretty(config)?;
    fs::write(path, content)
}

pub fn config_path() -> Option<PathBuf> {
    Some(config_dir()?.join("config.json"))
}

fn clean_recent_images<F>(paths: &[PathBuf], is_valid: F) -> Vec<PathBuf>
where
    F: Fn(&Path) -> bool,
{
    let mut cleaned = Vec::new();
    for path in paths {
        if cleaned.len() == RECENT_IMAGE_LIMIT {
            break;
        }
        if !is_valid(path) || cleaned.iter().any(|existing| existing == path) {
            continue;
        }
        cleaned.push(path.clone());
    }
    cleaned
}

fn record_recent_image(paths: &[PathBuf], path: &Path) -> Vec<PathBuf> {
    let mut recent = Vec::with_capacity(RECENT_IMAGE_LIMIT);
    recent.push(path.to_path_buf());

    for existing in paths {
        if recent.len() == RECENT_IMAGE_LIMIT {
            break;
        }
        if existing != path {
            recent.push(existing.clone());
        }
    }

    recent
}

fn parse_config(content: &str) -> AppConfig {
    serde_json::from_str(content).unwrap_or_default()
}

#[cfg(target_os = "linux")]
fn config_dir() -> Option<PathBuf> {
    Some(home_dir()?.join(".config").join("pixelcreep"))
}

#[cfg(target_os = "macos")]
fn config_dir() -> Option<PathBuf> {
    Some(
        home_dir()?
            .join("Library")
            .join("Application Support")
            .join("pixelcreep"),
    )
}

#[cfg(windows)]
fn config_dir() -> Option<PathBuf> {
    std::env::var_os("APPDATA")
        .map(PathBuf::from)
        .map(|path| path.join("pixelcreep"))
}

#[cfg(not(any(target_os = "linux", target_os = "macos", windows)))]
fn config_dir() -> Option<PathBuf> {
    Some(home_dir()?.join(".config").join("pixelcreep"))
}

#[cfg(not(windows))]
fn home_dir() -> Option<PathBuf> {
    std::env::var_os("HOME").map(PathBuf::from)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn path(value: &str) -> PathBuf {
        PathBuf::from(value)
    }

    #[test]
    fn record_recent_image_dedupes_and_moves_to_top() {
        let paths = vec![path("/a.png"), path("/b.png"), path("/c.png")];

        assert_eq!(
            record_recent_image(&paths, Path::new("/b.png")),
            vec![path("/b.png"), path("/a.png"), path("/c.png")]
        );
    }

    #[test]
    fn record_recent_image_caps_to_limit() {
        let paths = vec![
            path("/1.png"),
            path("/2.png"),
            path("/3.png"),
            path("/4.png"),
            path("/5.png"),
        ];

        assert_eq!(
            record_recent_image(&paths, Path::new("/6.png")),
            vec![
                path("/6.png"),
                path("/1.png"),
                path("/2.png"),
                path("/3.png"),
                path("/4.png"),
            ]
        );
    }

    #[test]
    fn clean_recent_images_filters_invalid_and_duplicates() {
        let paths = vec![
            path("/missing.png"),
            path("/a.png"),
            path("/a.png"),
            path("/b.txt"),
            path("/c.png"),
        ];

        assert_eq!(
            clean_recent_images(&paths, |path| path.extension().is_some_and(|ext| ext == "png")
                && !path.to_string_lossy().contains("missing")),
            vec![path("/a.png"), path("/c.png")]
        );
    }

    #[test]
    fn parse_config_tolerates_malformed_json() {
        assert!(parse_config("{not json").recent_images.is_empty());
    }
}
