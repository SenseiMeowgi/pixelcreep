use std::path::PathBuf;

use slint::ComponentHandle;

use crate::file_types::{selection_from_path, IMAGE_EXTENSIONS};
use crate::AppWindow;

use super::selection;

pub fn install(app: &AppWindow) {
    app.on_pick_image({
        let app_weak = app.as_weak();
        move || spawn_image_picker(app_weak.clone())
    });
}

fn open_image_dialog() -> Option<PathBuf> {
    rfd::FileDialog::new()
        .set_title("Open image")
        .add_filter("Images", IMAGE_EXTENSIONS)
        .pick_file()
}

fn spawn_image_picker(app_weak: slint::Weak<AppWindow>) {
    std::thread::spawn(move || {
        let Some(path) = open_image_dialog() else {
            return;
        };

        if let Err(e) = slint::invoke_from_event_loop(move || {
            let Some(app) = app_weak.upgrade() else {
                return;
            };
            let Some(image) = selection_from_path(path) else {
                return;
            };

            selection::apply(&app, image);
        }) {
            eprintln!("image_picker: invoke_from_event_loop error: {e}");
        }
    });
}
