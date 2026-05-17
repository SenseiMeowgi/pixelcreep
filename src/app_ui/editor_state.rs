use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use crate::processing::DecodedImage;

pub struct EditorState {
    pub source: Arc<[u8]>,
    pub width: u32,
    pub height: u32,
    source_epoch: AtomicUsize,
    preview_epoch: AtomicUsize,
}

impl Default for EditorState {
    fn default() -> Self {
        Self {
            source: Arc::from([]),
            width: 0,
            height: 0,
            source_epoch: AtomicUsize::new(0),
            preview_epoch: AtomicUsize::new(0),
        }
    }
}

impl EditorState {
    pub fn is_loaded(&self) -> bool {
        !self.source.is_empty()
    }

    pub fn load(&mut self, decoded: DecodedImage) {
        self.width = decoded.width;
        self.height = decoded.height;
        self.source = Arc::from(decoded.rgba.into_boxed_slice());
        self.bump_source_epoch();
    }

    pub fn clear(&mut self) {
        self.source = Arc::from([]);
        self.width = 0;
        self.height = 0;
        self.bump_source_epoch();
    }

    pub fn source_epoch(&self) -> usize {
        self.source_epoch.load(Ordering::Acquire)
    }

    pub fn is_source_stale(&self, source_epoch: usize) -> bool {
        self.source_epoch.load(Ordering::Acquire) != source_epoch
    }

    pub fn preview_epoch(&self) -> usize {
        self.preview_epoch.load(Ordering::Acquire)
    }

    pub fn bump_preview_epoch(&self) -> usize {
        self.preview_epoch.fetch_add(1, Ordering::Release) + 1
    }

    pub fn is_preview_stale(&self, preview_epoch: usize) -> bool {
        self.preview_epoch.load(Ordering::Acquire) != preview_epoch
    }

    fn bump_source_epoch(&self) -> usize {
        self.source_epoch.fetch_add(1, Ordering::Release) + 1
    }
}
