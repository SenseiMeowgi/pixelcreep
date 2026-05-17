use std::cell::RefCell;
use std::sync::Arc;

use slint::Image;

use crate::file_types::ImageSelection;
use crate::AppWindow;

use super::pixel_sort::{self, PixelSortCtx};
use super::recent;
use super::{VIEW_IMPORT, VIEW_PIXEL_SORT};

thread_local! {
    static PIXEL_SORT_CTX: RefCell<Option<Arc<PixelSortCtx>>> = const { RefCell::new(None) };
}

pub fn init(ctx: Arc<PixelSortCtx>) {
    PIXEL_SORT_CTX.with(|slot| *slot.borrow_mut() = Some(ctx));
}

pub fn apply(app: &AppWindow, selection: ImageSelection) {
    app.set_selected_path(selection.path.display().to_string().into());
    app.set_selected_label(selection.label.into());
    app.set_drag_active(false);

    let loaded = PIXEL_SORT_CTX.with(|slot| {
        slot.borrow()
            .as_ref()
            .map(|ctx| pixel_sort::load_image(app, ctx, &selection.path))
            .unwrap_or(false)
    });

    if loaded {
        recent::record_loaded(&selection.path);
        app.set_view(VIEW_PIXEL_SORT);
    } else {
        app.set_source_image(Image::default());
        app.set_image_width(0);
        app.set_image_height(0);
        PIXEL_SORT_CTX.with(|slot| {
            if let Some(ctx) = slot.borrow().as_ref() {
                pixel_sort::clear_image(ctx);
            }
        });
        app.set_view(VIEW_IMPORT);
    }
}
