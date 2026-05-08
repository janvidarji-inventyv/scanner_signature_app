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

use kurbo::{Affine, BezPath, Point, Rect, Size};
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
    pub frame_buf: Arc<Mutex<Option<CameraFrame>>>,
    pub active:    Arc<AtomicBool>,
}

impl CameraViewWidget {
    pub fn new(
        frame_buf: Arc<Mutex<Option<CameraFrame>>>,
        active:    Arc<AtomicBool>,
    ) -> Self {
        Self { frame_buf, active }
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
        // Always re-render and re-queue next frame.
        // This drives app_logic at vsync so poll_qr_result() is checked
        // within one frame (~16ms) of the QR being stored by the camera thread.
        // Masonry stops delivering frames automatically once this widget is
        // removed from the view tree (when Success screen replaces Scan screen).
        if camera::qr_result_ready() {
            log::info!("[UI] CameraViewWidget on_anim_frame: QR_READY -> submit action");
            ctx.submit_action::<CameraWidgetAction>(CameraWidgetAction::QrDetected);
            camera::wakeup_ui();
        }
        ctx.request_render();
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
            ctx.request_anim_frame();
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

            // 90° CCW rotation: NDK Camera2 delivers 1280×720 landscape,
            // sensor_orientation=90 → rotate CCW to fill portrait screen.
            //
            // Affine [a,b,c,d,e,f]:  maps (x,y) → (fh-y, x)
            let fw = frame.width  as f64;   // 1280
            let fh = frame.height as f64;   //  720
            let rot90ccw = Affine::new([0.0, 1.0, -1.0, 0.0, fh, 0.0]);

            let portrait_w = fh;   // 720
            let portrait_h = fw;   // 1280

            let s  = (ww / portrait_w).max(wh / portrait_h);
            let tx = (ww - s * portrait_w) / 2.0;
            let ty = (wh - s * portrait_h) / 2.0;
            let xform = Affine::translate((tx, ty)) * Affine::scale(s) * rot90ccw;

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
    for r in &[
        Rect::new(0.0,       0.0,       ww, by),
        Rect::new(0.0,       by + side, ww, wh),
        Rect::new(0.0,       by,        bx, by + side),
        Rect::new(bx + side, by,        ww, by + side),
    ] {
        scene.fill(vpeniko::Fill::NonZero, Affine::IDENTITY, dim, None, r);
    }
    draw_corner_brackets(scene, bx, by, side,
        vpeniko::Color::from_rgba8(0, 205, 215, 255));
}

// ── Corner brackets ───────────────────────────────────────────────────────────
fn draw_corner_brackets(
    scene: &mut Scene,
    bx: f64, by: f64, side: f64,
    teal: vpeniko::Color,
) {
    let arm    = 32.0_f64;
    let radius = 14.0_f64;
    let thick  =  4.5_f64;

    for &(cx, cy, hd, vd) in &[
        (bx,        by,         1.0_f64,  1.0_f64),
        (bx + side, by,        -1.0,       1.0),
        (bx,        by + side,  1.0,      -1.0),
        (bx + side, by + side, -1.0,      -1.0),
    ] {
        scene.fill(vpeniko::Fill::NonZero, Affine::IDENTITY, teal, None,
            &Rect::new(
                cx + hd * radius, cy - thick / 2.0,
                cx + hd * (radius + arm), cy + thick / 2.0,
            ).abs(),
        );
        scene.fill(vpeniko::Fill::NonZero, Affine::IDENTITY, teal, None,
            &Rect::new(
                cx - thick / 2.0, cy + vd * radius,
                cx + thick / 2.0, cy + vd * (radius + arm),
            ).abs(),
        );
        let arc_cx  = cx + hd * radius;
        let arc_cy  = cy + vd * radius;
        let a_start = match (hd as i32, vd as i32) {
            ( 1,  1) => std::f64::consts::PI,
            (-1,  1) => std::f64::consts::FRAC_PI_2 * 3.0,
            ( 1, -1) => std::f64::consts::FRAC_PI_2,
            _        => 0.0,
        };
        let ri = radius - thick / 2.0;
        let ro = radius + thick / 2.0;
        const STEPS: usize = 10;
        for i in 0..STEPS {
            let t  = std::f64::consts::FRAC_PI_2 / STEPS as f64;
            let a0 = a_start + t * i       as f64;
            let a1 = a_start + t * (i + 1) as f64;
            let mut path = BezPath::new();
            path.move_to(Point::new(arc_cx + ri * a0.cos(), arc_cy + ri * a0.sin()));
            path.line_to(Point::new(arc_cx + ro * a0.cos(), arc_cy + ro * a0.sin()));
            path.line_to(Point::new(arc_cx + ro * a1.cos(), arc_cy + ro * a1.sin()));
            path.line_to(Point::new(arc_cx + ri * a1.cos(), arc_cy + ri * a1.sin()));
            path.close_path();
            scene.fill(vpeniko::Fill::NonZero, Affine::IDENTITY, teal, None, &path);
        }
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
                                log::info!("[UI] CameraView.message commit request for Success");
                                state.qr_result  = Some(result);
                                state.qr_pending = false;
                                log::info!("[UI] DIRECT Scan->Success from CameraView.message");
                                state.set_screen(Screen::Success);
                                camera::consume_qr_result();
                                camera::wakeup_ui();
                                // Action(()) forces Xilem driver to rerun app_logic,
                                // which swaps the root view from Scan to Success.
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