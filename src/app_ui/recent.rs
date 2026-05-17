use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use slint::{ComponentHandle, ModelRc, SharedString, VecModel};

use crate::config::{self, AppConfig};
use crate::file_types::selection_from_path;
use crate::{AppWindow, RecentImage};

use super::selection;

struct RecentState {
    config: AppConfig,
    model: Rc<VecModel<RecentImage>>,
}

thread_local! {
    static RECENT_STATE: RefCell<Option<RecentState>> = const { RefCell::new(None) };
}

pub fn install(app: &AppWindow) {
    let mut config = config::load();
    if config.clean_recent_images() {
        let _ = config::save(&config);
    }

    let model = Rc::new(VecModel::from(recent_rows(&config)));
    app.set_recent_images(ModelRc::from(model.clone()));

    RECENT_STATE.with(|slot| {
        *slot.borrow_mut() = Some(RecentState { config, model });
    });

    app.on_open_recent_image({
        let app_weak = app.as_weak();
        move |path| {
            let Some(app) = app_weak.upgrade() else {
                return;
            };
            open_recent_image(&app, PathBuf::from(path.as_str()));
        }
    });
}

pub fn record_loaded(path: &Path) {
    RECENT_STATE.with(|slot| {
        let mut state = slot.borrow_mut();
        let Some(state) = state.as_mut() else {
            return;
        };

        state.config.record_recent_image(path);
        let _ = config::save(&state.config);
        refresh_model(state);
    });
}

fn open_recent_image(app: &AppWindow, path: PathBuf) {
    let Some(image) = selection_from_path(path) else {
        clean_and_refresh();
        return;
    };

    if !image.path.exists() {
        clean_and_refresh();
        return;
    }

    selection::apply(app, image);
}

fn clean_and_refresh() {
    RECENT_STATE.with(|slot| {
        let mut state = slot.borrow_mut();
        let Some(state) = state.as_mut() else {
            return;
        };

        if state.config.clean_recent_images() {
            let _ = config::save(&state.config);
            refresh_model(state);
        }
    });
}

fn refresh_model(state: &RecentState) {
    state.model.set_vec(recent_rows(&state.config));
}

fn recent_rows(config: &AppConfig) -> Vec<RecentImage> {
    config
        .recent_images
        .iter()
        .map(|path| RecentImage {
            label: label_for_path(path).into(),
            path: path.display().to_string().into(),
        })
        .collect()
}

fn label_for_path(path: &Path) -> SharedString {
    path.file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("Image")
        .into()
}
