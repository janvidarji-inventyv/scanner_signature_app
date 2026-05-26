use std::sync::{Arc, Mutex};
use std::sync::atomic::{AtomicU64, Ordering};

use masonry::accesskit;
use masonry::core::{
    AccessCtx, AccessEvent, BoxConstraints, EventCtx, LayoutCtx, PaintCtx, PointerEvent,
    PropertiesMut, PropertiesRef, RegisterCtx, TextEvent, Update, UpdateCtx, Widget, WidgetId,
};
use masonry::vello::Scene;
use masonry::vello::peniko as vpeniko;

use kurbo::{Affine, BezPath, Cap, Join, Point, Rect, Size, Stroke};
use smallvec::SmallVec;

use xilem::{Pod, ViewCtx};
use xilem_core::{MessageContext, MessageResult, Mut, View, ViewMarker};

use crate::AppState;

static SIGNATURE_PAD_GEN: AtomicU64 = AtomicU64::new(0);

pub fn notify_signature_pad_changed() {
    SIGNATURE_PAD_GEN.fetch_add(1, Ordering::SeqCst);
}

#[derive(Clone, Debug, Default)]
pub struct SignaturePadState {
    pub strokes: Vec<Vec<Point>>,
    pub can_draw: bool,
    /// Size of the widget when strokes were recorded (set in layout when can_draw=true)
    pub canvas_size: (f64, f64),
}

pub struct SignaturePadWidget {
    pub model: Arc<Mutex<SignaturePadState>>,
    pointer_down: bool,
    last_seen_gen: u64,
}

impl SignaturePadWidget {
    pub fn new(model: Arc<Mutex<SignaturePadState>>) -> Self {
        Self {
            model,
            pointer_down: false,
            last_seen_gen: u64::MAX,
        }
    }

    fn push_point(&mut self, p: Point) {
        if let Ok(mut model) = self.model.lock() {
            if !model.can_draw {
                return;
            }
            if model.strokes.is_empty() {
                model.strokes.push(vec![p]);
                return;
            }
            if let Some(last) = model.strokes.last_mut() {
                last.push(p);
            }
        }
    }

    fn begin_stroke(&mut self, p: Point) {
        if let Ok(mut model) = self.model.lock() {
            if !model.can_draw {
                return;
            }
            model.strokes.push(vec![p]);
        }
    }

    fn can_draw(&self) -> bool {
        self.model.lock().map(|m| m.can_draw).unwrap_or(false)
    }
}

impl Widget for SignaturePadWidget {
    type Action = ();

    fn on_pointer_event(
        &mut self,
        ctx: &mut EventCtx<'_>,
        _props: &mut PropertiesMut<'_>,
        event: &PointerEvent,
    ) {
        match event {
            PointerEvent::Down(e) => {
                if !self.can_draw() {
                    return;
                }
                let p = ctx.local_position(e.state.position);
                self.begin_stroke(p);
                self.pointer_down = true;
                ctx.capture_pointer();
                ctx.request_render();
                ctx.set_handled();
            }
            PointerEvent::Move(e) => {
                if self.pointer_down {
                    let p = ctx.local_position(e.current.position);
                    self.push_point(p);
                    ctx.request_render();
                    ctx.set_handled();
                }
            }
            PointerEvent::Up(e) => {
                if self.pointer_down {
                    let p = ctx.local_position(e.state.position);
                    self.push_point(p);
                }
                self.pointer_down = false;
                ctx.release_pointer();
                ctx.request_render();
                ctx.set_handled();
            }
            PointerEvent::Cancel(_) => {
                self.pointer_down = false;
                ctx.release_pointer();
                ctx.request_render();
                ctx.set_handled();
            }
            _ => {}
        }
    }

    fn on_text_event(&mut self, _: &mut EventCtx<'_>, _: &mut PropertiesMut<'_>, _: &TextEvent) {}

    fn on_access_event(
        &mut self,
        _: &mut EventCtx<'_>,
        _: &mut PropertiesMut<'_>,
        _: &AccessEvent,
    ) {
    }

    fn register_children(&mut self, _: &mut RegisterCtx) {}

    fn update(
        &mut self,
        ctx: &mut UpdateCtx<'_>,
        _props: &mut PropertiesMut<'_>,
        event: &Update,
    ) {
        if let Update::WidgetAdded = event {
            ctx.request_render();
            ctx.request_anim_frame();
        }
    }

    fn on_anim_frame(
        &mut self,
        ctx: &mut UpdateCtx<'_>,
        _props: &mut PropertiesMut<'_>,
        _nanos: u64,
    ) {
        let gen = SIGNATURE_PAD_GEN.load(Ordering::SeqCst);
        if gen != self.last_seen_gen {
            self.last_seen_gen = gen;
            ctx.request_render();
        }
        ctx.request_anim_frame();
    }

    fn layout(
        &mut self,
        _ctx: &mut LayoutCtx<'_>,
        _props: &mut PropertiesMut<'_>,
        bc: &BoxConstraints,
    ) -> Size {
        let size = bc.max();
        // Record canvas size when in drawing mode so preview can scale correctly
        if let Ok(mut model) = self.model.lock() {
            if model.can_draw && size.width > 0.0 && size.height > 0.0 {
                model.canvas_size = (size.width, size.height);
            }
        }
        size
    }

    fn paint(&mut self, ctx: &mut PaintCtx<'_>, _props: &PropertiesRef<'_>, scene: &mut Scene) {
        let size = ctx.size();
        let w = size.width;
        let h = size.height;
        let rect = Rect::new(0.0, 0.0, w, h);

        scene.fill(
            vpeniko::Fill::NonZero,
            Affine::IDENTITY,
            vpeniko::Color::from_rgba8(255, 255, 255, 255),
            None,
            &rect,
        );

        let draw_stroke = Stroke {
            width: 3.0,
            join: Join::Round,
            start_cap: Cap::Round,
            end_cap: Cap::Round,
            ..Default::default()
        };

        if let Ok(model) = self.model.lock() {
            if model.can_draw {
                let border = Stroke {
                    width: 2.0,
                    join: Join::Round,
                    start_cap: Cap::Round,
                    end_cap: Cap::Round,
                    ..Default::default()
                };
                scene.stroke(
                    &border,
                    Affine::IDENTITY,
                    vpeniko::Color::from_rgba8(0, 0, 0, 255),
                    None,
                    &rect,
                );
            }

            // When in preview mode (can_draw=false), scale strokes to fit this widget
            let transform = if !model.can_draw
                && model.canvas_size.0 > 0.0
                && model.canvas_size.1 > 0.0
            {
                let sx = w / model.canvas_size.0;
                let sy = h / model.canvas_size.1;
                let scale = sx.min(sy);
                // Center the scaled content
                let tx = (w - model.canvas_size.0 * scale) * 0.5;
                let ty = (h - model.canvas_size.1 * scale) * 0.5;
                Affine::new([scale, 0.0, 0.0, scale, tx, ty])
            } else {
                Affine::IDENTITY
            };

            for stroke in &model.strokes {
                if stroke.len() < 2 {
                    continue;
                }
                let mut path = BezPath::new();
                path.move_to(stroke[0]);
                for p in &stroke[1..] {
                    path.line_to(*p);
                }
                scene.stroke(
                    &draw_stroke,
                    transform,
                    vpeniko::Color::from_rgba8(20, 20, 20, 255),
                    None,
                    &path,
                );
            }
        }
    }

    fn accessibility_role(&self) -> accesskit::Role {
        accesskit::Role::Canvas
    }

    fn accessibility(
        &mut self,
        _ctx: &mut AccessCtx<'_>,
        _props: &PropertiesRef<'_>,
        _node: &mut accesskit::Node,
    ) {
    }

    fn children_ids(&self) -> SmallVec<[WidgetId; 16]> {
        SmallVec::new()
    }
}

pub struct SignaturePadView {
    pub model: Arc<Mutex<SignaturePadState>>,
}

pub fn signature_pad_view(model: Arc<Mutex<SignaturePadState>>) -> SignaturePadView {
    SignaturePadView { model }
}

impl ViewMarker for SignaturePadView {}

impl View<AppState, (), ViewCtx> for SignaturePadView {
    type Element = Pod<SignaturePadWidget>;
    type ViewState = ();

    fn build(&self, ctx: &mut ViewCtx, _state: &mut AppState) -> (Self::Element, ()) {
        let pod = ctx.with_action_widget(|_| Pod::new(SignaturePadWidget::new(Arc::clone(&self.model))));
        (pod, ())
    }

    fn rebuild(
        &self,
        _prev: &Self,
        _vs: &mut (),
        _ctx: &mut ViewCtx,
        el: Mut<Self::Element>,
        _state: &mut AppState,
    ) {
        el.widget.model = Arc::clone(&self.model);
    }

    fn teardown(&self, _vs: &mut (), _ctx: &mut ViewCtx, _el: Mut<Self::Element>) {}

    fn message(
        &self,
        _vs: &mut (),
        _ctx: &mut MessageContext,
        _el: Mut<Self::Element>,
        _state: &mut AppState,
    ) -> MessageResult<()> {
        MessageResult::Nop
    }
}
