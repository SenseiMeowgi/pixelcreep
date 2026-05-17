use std::path::Path;
use std::sync::{Arc, Condvar, Mutex};

use slint::{ComponentHandle, Image, SharedPixelBuffer};

use crate::processing::{
    apply_compose_rgba8, asdf_sort, decode_for_edit, portal_sort, AdjustParams,
    AsdfSortMode as ProcessingAsdfSortMode, AsdfSortParams, DecodedImage, PortalDirection,
    PortalSortParams, TransformParams,
};
use crate::{AppWindow, AsdfSortUiParams, PortalSortUiParams, SortDirection, SortMethod};

use super::editor_state::EditorState;

pub struct PixelSortCtx {
    pub editor: Arc<Mutex<EditorState>>,
    inbox: Arc<RenderInbox>,
}

impl PixelSortCtx {
    pub fn new() -> Self {
        Self {
            editor: Arc::new(Mutex::new(EditorState::default())),
            inbox: Arc::new(RenderInbox::default()),
        }
    }
}

#[derive(Default)]
struct RenderInbox {
    state: Mutex<InboxState>,
    cv: Condvar,
}

#[derive(Default)]
struct InboxState {
    preview: Option<PreviewRequest>,
    sort: Option<SortRequest>,
}

#[derive(Clone, Copy)]
struct PreviewRequest {
    source_epoch: usize,
    preview_epoch: usize,
    adjust: AdjustParams,
    transform: TransformParams,
    doc_mode: i32,
}

#[derive(Clone, Copy)]
struct SortRequest {
    adjust: AdjustParams,
    transform: TransformParams,
    operation: SortOperation,
    doc_mode: i32,
}

#[derive(Clone, Copy)]
enum SortOperation {
    Threshold(AsdfSortParams),
    Portal(PortalSortParams),
}

impl RenderInbox {
    fn post_preview(&self, req: PreviewRequest) {
        let mut s = self.state.lock().unwrap();
        s.preview = Some(req);
        drop(s);
        self.cv.notify_one();
    }

    fn post_sort(&self, req: SortRequest) {
        let mut s = self.state.lock().unwrap();
        s.sort = Some(req);
        s.preview = None;
        drop(s);
        self.cv.notify_one();
    }
}

enum WorkItem {
    Preview(PreviewRequest),
    Sort(SortRequest),
}

pub fn install(app: &AppWindow, ctx: Arc<PixelSortCtx>) {
    let app_weak = app.as_weak();
    let editor = ctx.editor.clone();
    let inbox = ctx.inbox.clone();

    spawn_render_worker(app_weak.clone(), editor.clone(), inbox.clone());

    app.on_adj_changed({
        let app_weak = app_weak.clone();
        let editor = editor.clone();
        let inbox = inbox.clone();
        move || post_preview(&app_weak, &editor, &inbox, PreviewFreshness::RequireLatest)
    });

    app.on_drag_preview_changed({
        let app_weak = app_weak.clone();
        let editor = editor.clone();
        let inbox = inbox.clone();
        move || {
            post_preview(
                &app_weak,
                &editor,
                &inbox,
                PreviewFreshness::AllowSuperseded,
            )
        }
    });

    app.on_run_sort({
        let app_weak = app_weak.clone();
        let editor = editor.clone();
        let inbox = inbox.clone();
        move || post_sort(&app_weak, &editor, &inbox)
    });

    app.on_reset_adjustments({
        let app_weak = app_weak.clone();
        let editor = editor.clone();
        let inbox = inbox.clone();
        move || {
            let Some(app) = app_weak.upgrade() else {
                return;
            };
            reset_adjustments(&app);
            post_preview(
                &app.as_weak(),
                &editor,
                &inbox,
                PreviewFreshness::RequireLatest,
            );
        }
    });

    app.on_reset_sort_params({
        let app_weak = app_weak.clone();
        move || {
            let Some(app) = app_weak.upgrade() else {
                return;
            };
            reset_current_sort_settings(&app);
        }
    });
}

pub fn load_image(app: &AppWindow, ctx: &PixelSortCtx, path: &Path) -> bool {
    let Some(decoded) = decode_for_edit(path) else {
        return false;
    };

    reset_adjustments(app);
    select_default_sort_settings(app);
    app.set_image_width(decoded.width as i32);
    app.set_image_height(decoded.height as i32);
    ctx.editor.lock().unwrap().load(decoded);
    post_preview(
        &app.as_weak(),
        &ctx.editor,
        &ctx.inbox,
        PreviewFreshness::RequireLatest,
    );
    true
}

pub fn clear_image(ctx: &PixelSortCtx) {
    ctx.editor.lock().unwrap().clear();
}

fn reset_adjustments(app: &AppWindow) {
    app.set_adj_brightness(0.);
    app.set_adj_contrast(0.);
    app.set_adj_saturation(0.);
    app.set_adj_vibrance(0.);
    app.set_adj_hue(0.);
    app.set_adj_rotation(0.);
    app.set_adj_zoom(100.);
    app.set_adj_pan_x(0.);
    app.set_adj_pan_y(0.);
    // Keep the live-drag delta consistent with the new pan: zeroing
    // last-rendered-pan alongside pan-x/y means translate = 0, no visible jump.
    app.set_last_rendered_pan_x(0.);
    app.set_last_rendered_pan_y(0.);
    app.set_doc_mode(0);
}

fn select_default_sort_settings(app: &AppWindow) {
    app.set_ps_method(SortMethod::Portal);
    reset_threshold_sort_settings(app);
    reset_portal_sort_settings(app);
}

fn reset_current_sort_settings(app: &AppWindow) {
    match app.get_ps_method() {
        SortMethod::Threshold => reset_threshold_sort_settings(app),
        SortMethod::Portal => reset_portal_sort_settings(app),
        _ => {}
    }
}

fn reset_threshold_sort_settings(app: &AppWindow) {
    app.set_threshold_params(default_threshold_params());
}

fn reset_portal_sort_settings(app: &AppWindow) {
    app.set_portal_params(default_portal_params());
}

fn default_threshold_params() -> AsdfSortUiParams {
    AsdfSortUiParams {
        mode: crate::AsdfSortMode::White,
        loops: 1.,
        white_value: -12_345_678.,
        black_value: -3_456_789.,
        bright_value: 127.,
        dark_value: 223.,
    }
}

fn default_portal_params() -> PortalSortUiParams {
    PortalSortUiParams {
        max_iterations: 2000.,
        dist: 200.,
        margin: 50.,
        mark_seeds: false,
        direction: SortDirection::Right,
    }
}

fn threshold_params_from_app(app: &AppWindow) -> AsdfSortParams {
    let params = app.get_threshold_params();
    AsdfSortParams {
        mode: match params.mode {
            crate::AsdfSortMode::White => ProcessingAsdfSortMode::White,
            crate::AsdfSortMode::Black => ProcessingAsdfSortMode::Black,
            crate::AsdfSortMode::Bright => ProcessingAsdfSortMode::Bright,
            crate::AsdfSortMode::Dark => ProcessingAsdfSortMode::Dark,
        },
        loops: params.loops.round().clamp(1., 20.) as u32,
        white_value: params.white_value.round().clamp(-16_581_375., 0.) as i32,
        black_value: params.black_value.round().clamp(-16_581_375., 0.) as i32,
        bright_value: params.bright_value.round().clamp(0., 255.) as u8,
        dark_value: params.dark_value.round().clamp(0., 255.) as u8,
    }
}

/// Returns the document output dimensions based on the selected mode and source dims.
/// mode 0 = original, 1 = rotated 90° (swap w/h), 2 = 1:1 square (min dim).
fn doc_dims(mode: i32, src_w: u32, src_h: u32) -> (u32, u32) {
    match mode {
        1 => (src_h, src_w),
        2 => {
            let side = src_w.min(src_h);
            (side, side)
        }
        _ => (src_w, src_h),
    }
}

fn adjust_from_app(app: &AppWindow) -> AdjustParams {
    AdjustParams {
        brightness: app.get_adj_brightness() / 100.0,
        contrast: app.get_adj_contrast() / 100.0,
        saturation: app.get_adj_saturation() / 100.0,
        hue: app.get_adj_hue(),
        vibrance: app.get_adj_vibrance() / 100.0,
    }
}

fn transform_from_app(app: &AppWindow) -> TransformParams {
    TransformParams {
        rotation_deg: app.get_adj_rotation(),
        zoom: (app.get_adj_zoom() / 100.0).max(0.01),
        pan_x_norm: app.get_adj_pan_x(),
        pan_y_norm: app.get_adj_pan_y(),
    }
}

fn portal_params_from_app(app: &AppWindow) -> PortalSortParams {
    let params = app.get_portal_params();
    PortalSortParams {
        max_iterations: params.max_iterations.round().max(1.) as u32,
        dist: params.dist.round().max(1.) as u32,
        margin: params.margin.round().clamp(1., 255.) as u8,
        mark_seeds: params.mark_seeds,
        direction: match params.direction {
            crate::SortDirection::Left => PortalDirection::Left,
            crate::SortDirection::Right => PortalDirection::Right,
            crate::SortDirection::Up => PortalDirection::Up,
            crate::SortDirection::Down => PortalDirection::Down,
        },
    }
}

#[derive(Clone, Copy)]
enum PreviewFreshness {
    RequireLatest,
    AllowSuperseded,
}

fn post_preview(
    app_weak: &slint::Weak<AppWindow>,
    editor: &Arc<Mutex<EditorState>>,
    inbox: &Arc<RenderInbox>,
    freshness: PreviewFreshness,
) {
    let Some(app) = app_weak.upgrade() else {
        return;
    };
    let (source_epoch, preview_epoch) = {
        let editor = editor.lock().unwrap();
        if !editor.is_loaded() {
            return;
        }
        let preview_epoch = match freshness {
            PreviewFreshness::RequireLatest => editor.bump_preview_epoch(),
            PreviewFreshness::AllowSuperseded => editor.preview_epoch(),
        };
        (editor.source_epoch(), preview_epoch)
    };
    inbox.post_preview(PreviewRequest {
        source_epoch,
        preview_epoch,
        adjust: adjust_from_app(&app),
        transform: transform_from_app(&app),
        doc_mode: app.get_doc_mode(),
    });
}

fn post_sort(
    app_weak: &slint::Weak<AppWindow>,
    editor: &Arc<Mutex<EditorState>>,
    inbox: &Arc<RenderInbox>,
) {
    let Some(app) = app_weak.upgrade() else {
        return;
    };
    if !editor.lock().unwrap().is_loaded() {
        return;
    }
    let operation = match app.get_ps_method() {
        SortMethod::Threshold => SortOperation::Threshold(threshold_params_from_app(&app)),
        SortMethod::Portal => SortOperation::Portal(portal_params_from_app(&app)),
        _ => return,
    };
    app.set_busy(true);
    inbox.post_sort(SortRequest {
        adjust: adjust_from_app(&app),
        transform: transform_from_app(&app),
        operation,
        doc_mode: app.get_doc_mode(),
    });
}

fn spawn_render_worker(
    app_weak: slint::Weak<AppWindow>,
    editor: Arc<Mutex<EditorState>>,
    inbox: Arc<RenderInbox>,
) {
    std::thread::Builder::new()
        .name("pixelcreep-render".into())
        .spawn(move || worker_loop(app_weak, editor, inbox))
        .expect("spawn render worker");
}

fn worker_loop(
    app_weak: slint::Weak<AppWindow>,
    editor: Arc<Mutex<EditorState>>,
    inbox: Arc<RenderInbox>,
) {
    // Reused output buffer; resized on dimension changes only.
    let mut working: Vec<u8> = Vec::new();

    loop {
        let item = {
            let mut s = inbox.state.lock().unwrap();
            loop {
                if let Some(req) = s.sort.take() {
                    s.preview = None;
                    break WorkItem::Sort(req);
                }
                if let Some(req) = s.preview.take() {
                    break WorkItem::Preview(req);
                }
                s = inbox.cv.wait(s).unwrap();
            }
        };

        // Snapshot source + dims briefly.
        let snapshot = {
            let st = editor.lock().unwrap();
            if !st.is_loaded() {
                continue;
            }
            (Arc::clone(&st.source), st.width, st.height)
        };
        let (source, src_w, src_h) = snapshot;

        match item {
            WorkItem::Preview(req) => {
                if editor.lock().unwrap().is_source_stale(req.source_epoch) {
                    continue;
                }
                if editor.lock().unwrap().is_preview_stale(req.preview_epoch) {
                    continue;
                }
                let (dst_w, dst_h) = doc_dims(req.doc_mode, src_w, src_h);
                let pixel_bytes = (dst_w as usize) * (dst_h as usize) * 4;
                if working.len() != pixel_bytes {
                    working.clear();
                    working.resize(pixel_bytes, 0);
                }
                apply_compose_rgba8(
                    &source,
                    src_w,
                    src_h,
                    &mut working,
                    dst_w,
                    dst_h,
                    req.adjust,
                    req.transform,
                );
                publish_preview(
                    &app_weak,
                    &editor,
                    &working,
                    dst_w,
                    dst_h,
                    req.source_epoch,
                    req.preview_epoch,
                    req.transform.pan_x_norm,
                    req.transform.pan_y_norm,
                );
            }
            WorkItem::Sort(req) => {
                let (dst_w, dst_h) = doc_dims(req.doc_mode, src_w, src_h);
                let pixel_bytes = (dst_w as usize) * (dst_h as usize) * 4;
                if working.len() != pixel_bytes {
                    working.clear();
                    working.resize(pixel_bytes, 0);
                }
                apply_compose_rgba8(
                    &source,
                    src_w,
                    src_h,
                    &mut working,
                    dst_w,
                    dst_h,
                    req.adjust,
                    req.transform,
                );
                match req.operation {
                    SortOperation::Threshold(params) => {
                        asdf_sort(&mut working, dst_w, dst_h, params);
                    }
                    SortOperation::Portal(params) => {
                        portal_sort(&mut working, dst_w, dst_h, params);
                    }
                }
                publish_sort_result(&app_weak, &editor, &mut working, dst_w, dst_h);
            }
        }
    }
}

fn publish_preview(
    app_weak: &slint::Weak<AppWindow>,
    editor: &Arc<Mutex<EditorState>>,
    pixels: &[u8],
    width: u32,
    height: u32,
    source_epoch: usize,
    preview_epoch: usize,
    baked_pan_x: f32,
    baked_pan_y: f32,
) {
    let buffer = SharedPixelBuffer::<slint::Rgba8Pixel>::clone_from_slice(pixels, width, height);
    let app_weak = app_weak.clone();
    let editor = editor.clone();
    if let Err(e) = slint::invoke_from_event_loop(move || {
        let Some(app) = app_weak.upgrade() else {
            return;
        };
        if editor.lock().unwrap().is_source_stale(source_epoch) {
            return;
        }
        if editor.lock().unwrap().is_preview_stale(preview_epoch) {
            return;
        }
        app.set_source_image(Image::from_rgba8(buffer));
        app.set_last_rendered_pan_x(baked_pan_x);
        app.set_last_rendered_pan_y(baked_pan_y);
    }) {
        eprintln!("publish_preview: invoke_from_event_loop error: {e}");
    }
}

fn publish_sort_result(
    app_weak: &slint::Weak<AppWindow>,
    editor: &Arc<Mutex<EditorState>>,
    pixels: &mut Vec<u8>,
    width: u32,
    height: u32,
) {
    let result = std::mem::take(pixels);
    let app_weak = app_weak.clone();
    let editor = editor.clone();
    if let Err(e) = slint::invoke_from_event_loop(move || {
        let Some(app) = app_weak.upgrade() else {
            return;
        };
        app.set_busy(false);
        editor.lock().unwrap().load(DecodedImage {
            rgba: result,
            width,
            height,
        });
        app.set_image_width(width as i32);
        app.set_image_height(height as i32);
        reset_adjustments(&app);
        push_image_to_ui(&app, &editor);
    }) {
        eprintln!("publish_sort_result: invoke_from_event_loop error: {e}");
    }
}

fn push_image_to_ui(app: &AppWindow, editor: &Arc<Mutex<EditorState>>) {
    let (source, width, height) = {
        let s = editor.lock().unwrap();
        if !s.is_loaded() {
            return;
        }
        (Arc::clone(&s.source), s.width, s.height)
    };
    let buffer = SharedPixelBuffer::<slint::Rgba8Pixel>::clone_from_slice(&source, width, height);
    app.set_source_image(Image::from_rgba8(buffer));
}
