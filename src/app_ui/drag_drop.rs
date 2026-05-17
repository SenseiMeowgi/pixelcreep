use slint::ComponentHandle;
use slint::winit_030::winit::event::WindowEvent;
use slint::winit_030::{EventResult, WinitWindowAccessor};

use crate::file_types::{is_supported_image, selection_from_path};
use crate::AppWindow;

use super::{selection, VIEW_IMPORT};

pub fn install(app: &AppWindow) {
    let app_weak = app.as_weak();

    app.window().on_winit_window_event({
        let app_weak = app_weak.clone();
        move |_, event| handle_window_event(&app_weak, event)
    });
}

fn handle_window_event(app_weak: &slint::Weak<AppWindow>, event: &WindowEvent) -> EventResult {
    match event {
        WindowEvent::HoveredFile(path) if is_supported_image(path) => {
            set_drag_active_if_import(app_weak.clone(), true);
            EventResult::Propagate
        }
        WindowEvent::HoveredFileCancelled => {
            set_drag_active_if_import(app_weak.clone(), false);
            EventResult::Propagate
        }
        WindowEvent::DroppedFile(path) => {
            let Some(image) = selection_from_path(path.clone()) else {
                return EventResult::Propagate;
            };

            let app_weak = app_weak.clone();
            if let Err(e) = slint::invoke_from_event_loop(move || {
                if let Some(app) = app_weak.upgrade() {
                    selection::apply(&app, image);
                }
            }) {
                eprintln!("drag_drop: invoke_from_event_loop error: {e}");
            }
            EventResult::Propagate
        }
        _ => EventResult::Propagate,
    }
}

fn set_drag_active_if_import(app_weak: slint::Weak<AppWindow>, active: bool) {
    if let Err(e) = slint::invoke_from_event_loop(move || {
        if let Some(app) = app_weak.upgrade() {
            if app.get_view() == VIEW_IMPORT {
                app.set_drag_active(active);
            }
        }
    }) {
        eprintln!("drag_drop: set_drag_active invoke_from_event_loop error: {e}");
    }
}
