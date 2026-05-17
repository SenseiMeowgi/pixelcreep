mod drag_drop;
mod image_picker;
mod pixel_sort;
mod recent;
mod selection;
mod status;
mod editor_state;

use std::sync::Arc;

use crate::AppWindow;

pub const VIEW_IMPORT: i32 = 0;
pub const VIEW_PIXEL_SORT: i32 = 1;

pub fn install(app: &AppWindow) {
    let pixel_sort_ctx = Arc::new(pixel_sort::PixelSortCtx::new());
    selection::init(pixel_sort_ctx.clone());
    status::install(app);
    recent::install(app);
    image_picker::install(app);
    drag_drop::install(app);
    pixel_sort::install(app, pixel_sort_ctx);
}
