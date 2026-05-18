// src/camera_widget.rs

use std::sync::{Arc, Mutex};
use std::sync::atomic::AtomicBool;

use masonry::core::{
    AccessCtx, AccessEvent, BoxConstraints, EventCtx, LayoutCtx, PaintCtx,
    PointerEvent, PropertiesMut, PropertiesRef, RegisterCtx, TextEvent,
    Update, UpdateCtx, Widget, WidgetId,
};
use masonry::accesskit;
use masonry::vello::peniko as vpeniko;

// ── CORRECT peniko 0.5 imports ────────────────────────────────────────────────
//
// peniko 0.5 (as pulled in by masonry 0.4 / vello 0.6):
//
//  • ImageData   — the pixel-data struct (renamed from `Image` in 0.4)
//  • ImageBrush  — wrapper that implements Into<ImageBrushRef> required by
//                  Scene::draw_image(image: impl Into<ImageBrushRef>, …)
//  • ImageFormat — pixel format enum (Rgba8, etc.)
//  • ImageAlphaType — only two variants exist in 0.5:
//        Alpha              = straight / un-premultiplied  ← use this for raw camera
//        PremultipliedAlpha  does NOT exist in peniko 0.5
//  • Blob        — reference-counted byte buffer, unchanged
//
// CORRECT call sequence for draw_image:
//   let img_data = ImageData { data: blob, format, alpha_type, width, height };
//   let brush    = ImageBrush::from(img_data);   // ImageBrush owns the data
//   scene.draw_image(&brush, transform);          // &ImageBrush → ImageBrushRef ✓
//
// DO NOT pass &ImageData — From<&ImageData> is not implemented in 0.5.
// DO NOT use PremultipliedAlpha — that variant does not exist in 0.5.
// ─────────────────────────────────────────────────────────────────────────────
use masonry::peniko::{Blob, ImageAlphaType, ImageBrush, ImageData, ImageFormat};
use masonry::vello::Scene;

use kurbo::{Affine, BezPath, Cap, Join, Point, Rect, Size, Stroke};
use smallvec::SmallVec;

use xilem::{Pod, ViewCtx};
use xilem_core::{MessageContext, MessageResult, Mut, View, ViewMarker};

use crate::camera::{self, CameraFrame};
use crate::{AppState, Screen};

#[derive(Clone, Debug)]
pub enum CameraWidgetAction {
    QrDetected,
}

// ─────────────────────────────────────────────────────────────────────────────
pub struct CameraViewWidget {
    pub frame_buf:    Arc<Mutex<Option<CameraFrame>>>,
    pub active:       Arc<AtomicBool>,
    /// Generation of the last camera frame we rendered. We only call
    /// request_render() when this changes, avoiding 60fps GPU texture
    /// re-uploads when the camera has not produced a new frame yet.
    last_frame_gen: u64,
}

impl CameraViewWidget {
    pub fn new(
        frame_buf: Arc<Mutex<Option<CameraFrame>>>,
        active:    Arc<AtomicBool>,
    ) -> Self {
        Self { frame_buf, active, last_frame_gen: u64::MAX }
    }
}

impl Widget for CameraViewWidget {
    type Action = CameraWidgetAction;

    fn on_pointer_event(&mut self, _: &mut EventCtx<'_>, _: &mut PropertiesMut<'_>, _: &PointerEvent) {}
    fn on_text_event   (&mut self, _: &mut EventCtx<'_>, _: &mut PropertiesMut<'_>, _: &TextEvent)   {}
    fn on_access_event (&mut self, _: &mut EventCtx<'_>, _: &mut PropertiesMut<'_>, _: &AccessEvent) {}
    fn register_children(&mut self, _: &mut RegisterCtx) {}

    fn on_anim_frame(
        &mut self,
        ctx:    &mut UpdateCtx<'_>,
        _props: &mut PropertiesMut<'_>,
        _nanos: u64,
    ) {
        if !self.active.load(std::sync::atomic::Ordering::SeqCst) {
            return;
        }
        if camera::qr_result_ready() {
            log::info!("[UI] CameraViewWidget on_anim_frame: QR_READY -> submit action");
            ctx.submit_action::<CameraWidgetAction>(CameraWidgetAction::QrDetected);
            camera::wakeup_ui();
        }
        // Only request a render (and GPU texture upload) when the camera
        // thread has produced a new frame. This prevents 60fps redundant draws.
        let current_gen = camera::frame_gen();
        if current_gen != self.last_frame_gen {
            self.last_frame_gen = current_gen;
            ctx.request_render();
        }
        ctx.request_anim_frame();
    }

    fn update(
        &mut self,
        ctx:    &mut UpdateCtx<'_>,
        _props: &mut PropertiesMut<'_>,
        event:  &Update,
    ) {
        if let Update::WidgetAdded = event {
            ctx.request_render();
            if self.active.load(std::sync::atomic::Ordering::SeqCst) {
                ctx.request_anim_frame();
            }
        }
    }

    fn layout(
        &mut self,
        _ctx:   &mut LayoutCtx<'_>,
        _props: &mut PropertiesMut<'_>,
        bc:     &BoxConstraints,
    ) -> Size {
        bc.max()
    }

    fn paint(&mut self, ctx: &mut PaintCtx<'_>, _props: &PropertiesRef<'_>, scene: &mut Scene) {
        let size = ctx.size();
        let ww   = size.width;
        let wh   = size.height;
        let clip = Rect::new(0.0, 0.0, ww, wh);

        let frame_opt: Option<CameraFrame> = self
            .frame_buf
            .lock()
            .ok()
            .and_then(|g: std::sync::MutexGuard<'_, Option<CameraFrame>>| g.clone());

        if let Some(frame) = frame_opt {
            // Black base
            scene.fill(
                vpeniko::Fill::NonZero,
                Affine::IDENTITY,
                vpeniko::Color::from_rgba8(0, 0, 0, 255),
                None,
                &clip,
            );

            // ── CORRECT peniko 0.5 image + draw ──────────────────────────────
            let blob = Blob::new(Arc::new(frame.rgba));
            let img_data = ImageData {
                data:       blob,
                format:     ImageFormat::Rgba8,
                alpha_type: ImageAlphaType::Alpha,   // straight alpha; PremultipliedAlpha doesn't exist in 0.5
                width:      frame.width,
                height:     frame.height,
            };
            let brush = ImageBrush::from(img_data);  // must wrap before passing to draw_image

            // Keep preview stable without locking Activity orientation:
            // - portrait widget: rotate camera frame 90 CCW
            // - landscape widget: keep native landscape orientation
            let fw = frame.width as f64;
            let fh = frame.height as f64;
            let is_portrait_widget = wh >= ww;
            let xform = if is_portrait_widget {
                // Affine [a,b,c,d,e,f]: maps (x,y) -> (fh-y, x)
                let rot90ccw = Affine::new([0.0, 1.0, -1.0, 0.0, fh, 0.0]);
                let out_w = fh;
                let out_h = fw;
                let s = (ww / out_w).max(wh / out_h);
                let tx = (ww - s * out_w) / 2.0;
                let ty = (wh - s * out_h) / 2.0;
                Affine::translate((tx, ty)) * Affine::scale(s) * rot90ccw
            } else {
                let out_w = fw;
                let out_h = fh;
                let s = (ww / out_w).max(wh / out_h);
                let tx = (ww - s * out_w) / 2.0;
                let ty = (wh - s * out_h) / 2.0;
                Affine::translate((tx, ty)) * Affine::scale(s)
            };

            scene.push_clip_layer(Affine::IDENTITY, &clip);
            scene.draw_image(&brush, xform);   // &ImageBrush → Into<ImageBrushRef> ✓
            scene.pop_layer();

            draw_scan_overlay(scene, ww, wh);
        } else {
            draw_warming_up(scene, ww, wh);
        }
    }

    fn accessibility_role(&self) -> accesskit::Role { accesskit::Role::Image }

    fn accessibility(
        &mut self,
        _ctx:   &mut AccessCtx<'_>,
        _props: &PropertiesRef<'_>,
        _node:  &mut accesskit::Node,
    ) {}

    fn children_ids(&self) -> SmallVec<[WidgetId; 16]> { SmallVec::new() }
}

// ── Scan box geometry ─────────────────────────────────────────────────────────
fn scan_box(ww: f64, wh: f64) -> (f64, f64, f64) {
    let side = ww.min(wh) * 0.76;
    let bx   = (ww - side) / 2.0;
    let by   = (wh - side) / 2.0;
    (bx, by, side)
}

// ── Warming-up placeholder ────────────────────────────────────────────────────
fn draw_warming_up(scene: &mut Scene, ww: f64, wh: f64) {
    scene.fill(
        vpeniko::Fill::NonZero, Affine::IDENTITY,
        vpeniko::Color::from_rgba8(0, 0, 0, 200), None,
        &Rect::new(0.0, 0.0, ww, wh),
    );
    let (bx, by, side) = scan_box(ww, wh);
    draw_corner_brackets(scene, bx, by, side,
        vpeniko::Color::from_rgba8(0, 205, 215, 255));
}

// ── Live overlay ──────────────────────────────────────────────────────────────
fn draw_scan_overlay(scene: &mut Scene, ww: f64, wh: f64) {
    let (bx, by, side) = scan_box(ww, wh);
    let dim = vpeniko::Color::from_rgba8(0, 0, 0, 130);
    // Outer fullscreen rect minus an inner rounded-rect hole.
    let mut mask = BezPath::new();
    mask.move_to(Point::new(0.0, 0.0));
    mask.line_to(Point::new(ww, 0.0));
    mask.line_to(Point::new(ww, wh));
    mask.line_to(Point::new(0.0, wh));
    mask.close_path();
    append_rounded_rect_path(&mut mask, bx, by, side, side, 18.0);
    scene.fill(vpeniko::Fill::EvenOdd, Affine::IDENTITY, dim, None, &mask);

    draw_corner_brackets(scene, bx, by, side,
        vpeniko::Color::from_rgba8(0, 205, 215, 255));
}

fn append_rounded_rect_path(
    path: &mut BezPath,
    x: f64,
    y: f64,
    w: f64,
    h: f64,
    mut r: f64,
) {
    r = r.min(w * 0.5).min(h * 0.5).max(0.0);
    let x0 = x;
    let y0 = y;
    let x1 = x + w;
    let y1 = y + h;

    path.move_to(Point::new(x0 + r, y0));
    path.line_to(Point::new(x1 - r, y0));
    path.quad_to(Point::new(x1, y0), Point::new(x1, y0 + r));
    path.line_to(Point::new(x1, y1 - r));
    path.quad_to(Point::new(x1, y1), Point::new(x1 - r, y1));
    path.line_to(Point::new(x0 + r, y1));
    path.quad_to(Point::new(x0, y1), Point::new(x0, y1 - r));
    path.line_to(Point::new(x0, y0 + r));
    path.quad_to(Point::new(x0, y0), Point::new(x0 + r, y0));
    path.close_path();
}

// ── Corner brackets ───────────────────────────────────────────────────────────
fn draw_corner_brackets(
    scene: &mut Scene,
    bx: f64, by: f64, side: f64,
    teal: vpeniko::Color,
) {
    let arm    = 36.0_f64;
    let radius = 16.0_f64;
    let thick  = 5.5_f64;
    let style  = Stroke {
        width: thick,
        join: Join::Round,
        start_cap: Cap::Round,
        end_cap: Cap::Round,
        ..Default::default()
    };

    for &(cx, cy, hd, vd) in &[
        (bx,        by,         1.0_f64,  1.0_f64),
        (bx + side, by,        -1.0,       1.0),
        (bx,        by + side,  1.0,      -1.0),
        (bx + side, by + side, -1.0,      -1.0),
    ] {
        let mut path = BezPath::new();
        // Start from horizontal outer end, curve around corner, end at vertical outer end.
        path.move_to(Point::new(cx + hd * (radius + arm), cy));
        path.line_to(Point::new(cx + hd * radius, cy));
        path.quad_to(
            Point::new(cx, cy),
            Point::new(cx, cy + vd * radius),
        );
        path.line_to(Point::new(cx, cy + vd * (radius + arm)));
        scene.stroke(&style, Affine::IDENTITY, teal, None, &path);
    }
}

// ═════════════════════════════════════════════════════════════════════════════
// Xilem View wrapper
// ═════════════════════════════════════════════════════════════════════════════
pub struct CameraView {
    pub frame_buf: Arc<Mutex<Option<CameraFrame>>>,
    pub active:    Arc<AtomicBool>,
}

pub fn camera_view(
    frame_buf: Arc<Mutex<Option<CameraFrame>>>,
    active:    Arc<AtomicBool>,
) -> CameraView {
    CameraView { frame_buf, active }
}

impl ViewMarker for CameraView {}

impl View<AppState, (), ViewCtx> for CameraView {
    type Element   = Pod<CameraViewWidget>;
    type ViewState = ();

    fn build(&self, ctx: &mut ViewCtx, _state: &mut AppState) -> (Self::Element, ()) {
        let pod = ctx.with_action_widget(|_| {
            Pod::new(CameraViewWidget::new(
                Arc::clone(&self.frame_buf),
                Arc::clone(&self.active),
            ))
        });
        (pod, ())
    }

    fn rebuild(
        &self,
        _prev:  &Self,
        _vs:    &mut (),
        _ctx:   &mut ViewCtx,
        el:     Mut<Self::Element>,
        _state: &mut AppState,
    ) {
        el.widget.frame_buf = Arc::clone(&self.frame_buf);
        el.widget.active    = Arc::clone(&self.active);
    }

    fn teardown(&self, _vs: &mut (), _ctx: &mut ViewCtx, _el: Mut<Self::Element>) {}

    fn message(
        &self,
        _vs:    &mut (),
        ctx:    &mut MessageContext,
        _el:    Mut<Self::Element>,
        state:  &mut AppState,
    ) -> MessageResult<()> {
        if !ctx.remaining_path().is_empty() {
            return MessageResult::Stale;
        }

        match ctx.take_message::<CameraWidgetAction>() {
            Some(action) => {
                log::info!("[UI] CameraView.message received CameraWidgetAction");
                match *action {
                    CameraWidgetAction::QrDetected => {
                        if matches!(state.screen, Screen::Scan) {
                            if let Some(result) = camera::peek_qr_result() {
                                log::info!("[UI] CameraView.message queueing Success transition");
                                state.qr_result  = Some(result);
                                state.qr_pending = true;
                                // Let app_logic own the actual Scan->Success commit.
                                return MessageResult::Action(());
                            }
                        }
                        MessageResult::Nop
                    }
                }
            }
            None => MessageResult::Stale,
        }
    }
}