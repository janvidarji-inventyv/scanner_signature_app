// src/lib.rs
use winit::error::EventLoopError;
use winit::window::Icon;
use xilem::view::ZStackExt;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
#[cfg(target_os = "android")]
use std::sync::Once;

use xilem::{
    AnyWidgetView, EventLoopBuilder, WindowId, Xilem, window,
    view::{FlexSpacer, ObjectFit, button, flex_col, flex_row, image, label, sized_box, spinner, task, text_button, zstack},
};
use xilem::core::fork;
use xilem::core::one_of::Either;
use masonry::peniko::Color;
use masonry::properties::types::Length;
use xilem::style::Style;
use image::{ImageBuffer, Rgba};
use kurbo::Point;

mod image_assets;
mod camera;
mod camera_widget;
mod signature_pad_widget;

use camera_widget::camera_view;
use signature_pad_widget::{notify_signature_pad_changed, signature_pad_view, SignaturePadState};

#[cfg(target_os = "android")]
static TRACING_INIT: Once = Once::new();

#[cfg(target_os = "android")]
fn init_safe_android_tracing() {
    TRACING_INIT.call_once(|| {
        use tracing_subscriber::layer::SubscriberExt;
        use tracing_subscriber::util::SubscriberInitExt;
        use tracing_subscriber::EnvFilter;

        // Install our own subscriber first so Masonry/Xilem won't try their
        // default debug subscriber that writes to temp_dir and may panic on
        // Android devices where that path is not writable.
        let filter = EnvFilter::try_from_default_env()
            .unwrap_or_else(|_| EnvFilter::new("info"));

        let _ = tracing_subscriber::registry()
            .with(filter)
            .with(tracing_subscriber::fmt::layer().with_target(false).without_time())
            .try_init();
    });
}

#[derive(Clone, Debug)]
pub enum Screen {
    Launch,
    Info,
    Scan,
    Success,
    SignatureCapture,
    SignaturePad,
    SignaturePreview,
    SignatureSaved,
}

#[derive(Clone, Debug, PartialEq)]
pub enum ScanMode {
    Activation,
    SignatureCapture,
}

pub struct AppState {
    pub screen:                Screen,
    pub running:               bool,
    pub show_permission_error: bool,
    pub show_permission_popup: bool,
    pub is_scanning:           Arc<AtomicBool>,
    pub qr_result:             Option<String>,
    // ── FIX: explicit flag set by QR detection so app_logic always
    //    transitions even if the anim-loop wakeup arrives late ──────────
    pub qr_pending:            bool,
    pub scan_mode:             ScanMode,
    pub signature_pad_from_preview: bool,
    pub signature_pad:         Arc<Mutex<SignaturePadState>>,
    pub signature_save_message: Option<String>,
}

impl AppState {
    fn new() -> Self {
        // ── CRASH RECOVERY ────────────────────────────────────────────────────
        // If the GPU device was lost (wgpu panic) while a QR result was
        // already detected, QR_READY stays true in the static and the
        // string is still in QR_RESULT. On restart we go straight to the
        // Success screen instead of Info, so the user sees their result.
        if camera::qr_result_ready() {
            let qr = camera::peek_qr_result();
            log::info!("[UI] AppState::new — crash recovery, QR pending: {:?}",
                qr.as_deref().map(|s| &s[..s.len().min(40)]));
            // is_scanning stays false — camera is not running after a restart
            return Self {
                screen:                Screen::Success,
                running:               true,
                show_permission_error: false,
                show_permission_popup: false,
                is_scanning:           Arc::new(AtomicBool::new(false)),
                qr_result:             qr,
                qr_pending:            false, // already committed via peek path
                scan_mode:             ScanMode::Activation,
                signature_pad_from_preview: false,
                signature_pad:         Arc::new(Mutex::new(SignaturePadState { strokes: Vec::new(), can_draw: true, canvas_size: (0.0, 0.0) })),
                signature_save_message: None,
            };
        }
        let show_permission_popup = camera::take_permission_settings_popup_request();

        Self {
            screen:                Screen::Launch,
            running:               true,
            show_permission_error: false,
            show_permission_popup,
            is_scanning:           Arc::new(AtomicBool::new(false)),
            qr_result:             None,
            qr_pending:            false,
            scan_mode:             ScanMode::Activation,
            signature_pad_from_preview: false,
            signature_pad:         Arc::new(Mutex::new(SignaturePadState { strokes: Vec::new(), can_draw: true, canvas_size: (0.0, 0.0) })),
            signature_save_message: None,
        }
    }

    pub fn set_screen(&mut self, screen: Screen) {
        self.screen = screen.clone();
        self.show_permission_error = false;
        self.show_permission_popup = false;
        match screen {
            Screen::Scan => {
                self.is_scanning.store(true, Ordering::SeqCst);
                camera::show_camera_overlay();
            }
            _ => {
                self.is_scanning.store(false, Ordering::SeqCst);
                camera::hide_camera_overlay();
            }
        }
    }
}

impl Default for AppState {
    fn default() -> Self { Self::new() }
}

impl xilem::AppState for AppState {
    fn keep_running(&self) -> bool { self.running }
}

fn handle_system_back(state: &mut AppState) {
    if state.show_permission_popup {
        state.show_permission_popup = false;
        return;
    }

    match state.screen {
        Screen::Launch => {
            state.running = false;
        }
        Screen::Info => {
            state.running = false;
        }
        Screen::Scan => {
            state.qr_result = None;
            state.qr_pending = false;
            camera::clear_qr_result();
            let target = if state.scan_mode == ScanMode::Activation {
                Screen::Info
            } else {
                Screen::Success
            };
            state.set_screen(target);
        }
        Screen::Success => {
            state.set_screen(Screen::Info);
        }
        Screen::SignatureCapture => {
            state.set_screen(Screen::Success);
        }
        Screen::SignaturePad => {
            if let Ok(mut pad) = state.signature_pad.lock() {
                pad.can_draw = true;
            }
            camera::set_portrait_orientation();
            if state.signature_pad_from_preview {
                state.signature_pad_from_preview = false;
                state.set_screen(Screen::SignaturePreview);
            } else {
                state.set_screen(Screen::SignatureCapture);
            }
        }
        Screen::SignaturePreview => {
            if let Ok(mut pad) = state.signature_pad.lock() {
                pad.can_draw = true;
            }
            state.signature_pad_from_preview = true;
            camera::set_landscape_orientation();
            state.set_screen(Screen::SignaturePad);
        }
        Screen::SignatureSaved => {
            state.qr_result = None;
            state.qr_pending = false;
            state.scan_mode = ScanMode::Activation;
            camera::consume_qr_result();
            state.signature_save_message = None;
            if let Ok(mut pad) = state.signature_pad.lock() {
                pad.strokes.clear();
                pad.can_draw = true;
            }
            camera::set_portrait_orientation();
            state.set_screen(Screen::SignatureCapture);
        }
    }
}

fn app_logic(state: &mut AppState) -> Box<AnyWidgetView<AppState>> {
    // Single commit path for Scan -> Success. Keep QR in static until this
    // commit executes, then consume exactly once.
    if matches!(state.screen, Screen::Scan) {
        if state.qr_result.is_none() {
            if let Some(result) = camera::peek_qr_result() {
                state.qr_result = Some(result);
                state.qr_pending = true;
            }
        }

        if state.qr_result.is_some() || camera::qr_result_ready() {
            if state.qr_result.is_none() {
                state.qr_result = camera::peek_qr_result();
            }
            if state.qr_result.is_some() {
                let next_screen = match state.scan_mode {
                    ScanMode::Activation       => Screen::Success,
                    ScanMode::SignatureCapture => Screen::SignatureCapture,
                };
                log::info!("[UI] COMMIT Scan->{:?}", next_screen);
                state.qr_pending = false;
                state.set_screen(next_screen);
                camera::consume_qr_result();
                camera::wakeup_ui();
                return match state.screen {
                    Screen::Success          => Box::new(success_screen(state)),
                    Screen::SignatureCapture => Box::new(signature_capture_screen(state)),
                    _                        => Box::new(info_screen(state)),
                };
            }
        }
    }

    // Handle Android lifecycle callbacks (cheap no-ops when not needed)
    camera::on_android_resumed(state);
    camera::poll_permission_granted(state);
    if camera::take_permission_settings_popup_request() {
        state.show_permission_popup = true;
        state.show_permission_error = false;
    }

    // ── QR poll — peek without removing, runs every frame ────────────────────
    //
    // CRITICAL: we use peek_qr_result() (not take/poll) so that if the
    // event loop panics between here and the set_screen(Success) call below,
    // the QR result is still in the static on the next restart.
    // AppState::new() will see QR_READY=true and go straight to Success.
    //
    // consume_qr_result() is only called AFTER set_screen(Success) confirms
    // the transition completed successfully.
    if !state.qr_pending {
        if let Some(result) = camera::peek_qr_result() {
            log::info!("[UI] QR result peeked: {}...",
                &result[..result.len().min(40)]);
            if matches!(state.screen, Screen::Scan) {
                state.qr_result  = Some(result);
                state.qr_pending = true;
            }
        }
    }

    // ── Transition to Success ─────────────────────────────────────────────────
    //
    // Separated from the poll above to avoid re-entrancy:
    // set_screen → hide_camera_overlay() modifies camera globals.
    if state.qr_pending && state.qr_result.is_some() {
        let next_screen = match state.scan_mode {
            ScanMode::Activation        => Screen::Success,
            ScanMode::SignatureCapture  => Screen::SignatureCapture,
        };
        log::info!("[UI] transitioning to {:?} screen", next_screen);
        state.qr_pending = false;
        state.set_screen(next_screen);
        camera::consume_qr_result();
        camera::wakeup_ui();
    }

    match state.screen {
        Screen::Launch             => Box::new(launch_screen()),
        Screen::Info               => Box::new(info_screen(state)),
        Screen::Scan               => Box::new(scan_screen(state)),
        Screen::Success            => Box::new(success_screen(state)),
        Screen::SignatureCapture   => Box::new(signature_capture_screen(state)),
        Screen::SignaturePad       => Box::new(signature_pad_screen(state)),
        Screen::SignaturePreview   => Box::new(signature_preview_screen(state)),
        Screen::SignatureSaved     => Box::new(signature_saved_screen(state)),
    }
}

fn launch_screen() -> impl xilem::WidgetView<AppState> {
    let launch = image_assets::get_launch_image().clone();

    fork(
        sized_box(
            zstack((
                sized_box(image(launch).fit(ObjectFit::Cover))
                    .expand_width()
                    .expand_height(),
                flex_col((
                    FlexSpacer::Flex(1.0),
                    flex_row((
                        FlexSpacer::Flex(1.0),
                        sized_box(spinner())
                            .width(Length::px(56.0))
                            .height(Length::px(56.0)),
                        FlexSpacer::Flex(1.0),
                    )),
                    FlexSpacer::Flex(1.0),
                )),
            ))
        )
        .expand_width()
        .expand_height()
        .background(xilem::style::Background::Color(Color::WHITE)),
        task(
            |proxy| async move {
                std::thread::sleep(Duration::from_millis(5000));
                let _ = proxy.message(());
            },
            |state: &mut AppState, ()| {
                if matches!(state.screen, Screen::Launch) {
                    state.set_screen(Screen::Info);
                }
            },
        ),
    )
}

fn load_window_icon() -> Option<Icon> {
    let bytes = include_bytes!("assets/icon_sign_pad.webp");
    let img = image::load_from_memory(bytes).ok()?.into_rgba8();
    let (w, h) = (img.width(), img.height());
    Icon::from_rgba(img.into_raw(), w, h).ok()
}

fn draw_line_rgba(
    img: &mut ImageBuffer<Rgba<u8>, Vec<u8>>,
    x0: i32,
    y0: i32,
    x1: i32,
    y1: i32,
    color: Rgba<u8>,
) {
    let mut x = x0;
    let mut y = y0;
    let dx = (x1 - x0).abs();
    let dy = -(y1 - y0).abs();
    let sx = if x0 < x1 { 1 } else { -1 };
    let sy = if y0 < y1 { 1 } else { -1 };
    let mut err = dx + dy;

    loop {
        if x >= 0 && y >= 0 && (x as u32) < img.width() && (y as u32) < img.height() {
            img.put_pixel(x as u32, y as u32, color);
        }
        if x == x1 && y == y1 {
            break;
        }
        let e2 = 2 * err;
        if e2 >= dy {
            err += dy;
            x += sx;
        }
        if e2 <= dx {
            err += dx;
            y += sy;
        }
    }
}

fn save_signature_png(state: &AppState) -> Result<String, String> {
    let (strokes, canvas_size): (Vec<Vec<Point>>, (f64, f64)) = {
        let pad = state
            .signature_pad
            .lock()
            .map_err(|_| "failed to lock signature pad".to_string())?;
        (pad.strokes.clone(), pad.canvas_size)
    };

    if !strokes.iter().any(|s| !s.is_empty()) {
        return Err("no signature to save".to_string());
    }

    let width = canvas_size.0.max(1.0) as u32;
    let height = canvas_size.1.max(1.0) as u32;
    let mut img = ImageBuffer::from_pixel(width, height, Rgba([255, 255, 255, 255]));
    let ink = Rgba([20, 20, 20, 255]);

    for stroke in &strokes {
        if stroke.len() < 2 {
            continue;
        }
        for pair in stroke.windows(2) {
            let p0 = pair[0];
            let p1 = pair[1];
            draw_line_rgba(&mut img, p0.x as i32, p0.y as i32, p1.x as i32, p1.y as i32, ink);
        }
    }

    let base_dir = camera::app_internal_data_path()
        .ok_or_else(|| "app storage path unavailable".to_string())?;
    let sig_dir = base_dir.join("signatures");
    std::fs::create_dir_all(&sig_dir).map_err(|e| format!("create dir failed: {e}"))?;

    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| format!("time error: {e}"))?
        .as_secs();
    let file_path = sig_dir.join(format!("signature_{ts}.png"));
    img.save(&file_path)
        .map_err(|e| format!("save failed: {e}"))?;

    Ok(file_path.display().to_string())
}

// ── Scan screen ───────────────────────────────────────────────────────────────
//
// FIX: Removed the top_bar (Back button + "Scan QR Code" title) and
// hint_bar entirely — the camera view now fills the whole screen edge-to-edge.
// The camera widget itself draws the teal scan-box overlay.
fn scan_screen(state: &AppState) -> impl xilem::WidgetView<AppState> {
    // Full-screen camera view — no top bar, no hint bar.
    // sized_box + expand_width + expand_height make the widget fill the
    // entire window so the camera feed uses every pixel.
    sized_box(
        camera_view(
            Arc::clone(camera::frame_buf()),
            state.is_scanning.clone(),
        )
    )
    .expand_width()
    .expand_height()
}

// ── Info screen ───────────────────────────────────────────────────────────────
fn info_screen(state: &mut AppState) -> impl xilem::WidgetView<AppState> {
    let icon        = image_assets::get_icon().clone();
    let bullet_icon = image_assets::get_bullet_icon().clone();

    let gray        = Color::from_rgba8(116, 122, 123, 255);
    let teal        = Color::from_rgba8(0, 80, 116, 255);
    let black       = Color::from_rgba8(0, 0, 0, 255);
    let transparent = Color::from_rgba8(0, 0, 0, 0);

    let error_label = if state.show_permission_error {
        label("Camera permission required! Grant it and try again.")
            .text_size(14.0)
            .weight(xilem::FontWeight::BOLD)
            .text_alignment(xilem::TextAlign::Center)
            .color(Color::from_rgba8(255, 0, 0, 255))
    } else {
        label("").text_size(1.0).color(transparent)
    };

    // ── Header: icon centered, title centered ─────────────────────────────────
    let header = flex_col((
        label("").text_size(40.0),
        sized_box(image(icon))
            .width(Length::px(110.0))
            .height(Length::px(110.0)),
        label("Activate Your Device")
            .text_size(26.0)
            .weight(xilem::FontWeight::BOLD)
            .text_alignment(xilem::TextAlign::Center)
            .color(black),
        label("").text_size(2.0),
    ));

    // ── Desc: left-aligned gray text ──────────────────────────────────────────
    let desc = flex_col((
        label("Your MyTAXPrepOffice Signature Pad is not activated.")
            .text_size(15.0)
            .text_alignment(xilem::TextAlign::Left)
            .color(gray),
        label("Before you can capture signatures using this device,")
            .text_size(15.0)
            .text_alignment(xilem::TextAlign::Left)
            .color(gray),
        label("you will need to activate it through your")
            .text_size(15.0)
            .text_alignment(xilem::TextAlign::Left)
            .color(gray),
        label("MyTAXPrepOffice software.")
            .text_size(15.0)
            .text_alignment(xilem::TextAlign::Left)
            .color(gray),
        //label("").text_size(2.0),
        label("To activate your device, follow the steps below.")
            .text_size(15.0)
            .text_alignment(xilem::TextAlign::Left)
            .color(gray),
        //label("").text_size(2.0),
    )).padding(xilem::style::Padding::left(4.0));

    // ── Steps ─────────────────────────────────────────────────────────────────
    let step1 = flex_row((
        sized_box(image(bullet_icon.clone()))
            .width(Length::px(16.0))
            .height(Length::px(16.0)),
        flex_col((
            label("Login to MyTAXPrepOffice and select Signature")
                .text_size(15.0)
                .text_alignment(xilem::TextAlign::Left)
                .color(gray),
            label("Devices* under the Toolbox widget")
                .text_size(15.0)
                .text_alignment(xilem::TextAlign::Left)
                .color(gray),
        )),
    ));

    let step2 = flex_row((
        sized_box(image(bullet_icon.clone()))
            .width(Length::px(16.0))
            .height(Length::px(16.0)),
        flex_col((
            label("Click New under Signature Devices to display the")
                .text_size(15.0)
                .text_alignment(xilem::TextAlign::Left)
                .color(gray),
            label("Activation QR Code.")
                .text_size(15.0)
                .text_alignment(xilem::TextAlign::Left)
                .color(gray),
        )),
    ));

    let step3 = flex_row((
        sized_box(image(bullet_icon.clone()))
            .width(Length::px(16.0))
            .height(Length::px(16.0)),
        flex_col((
            label("Once the Activation QR Code is displayed, press")
                .text_size(15.0)
                .text_alignment(xilem::TextAlign::Left)
                .color(gray),
            label("the Activate Signature Pad button below to scan")
                .text_size(15.0)
                .text_alignment(xilem::TextAlign::Left)
                .color(gray),
            label("the Activation QR Code.")
                .text_size(15.0)
                .text_alignment(xilem::TextAlign::Left)
                .color(gray),
        )),
    ));

    let steps = flex_col((
        step1,
        label("").text_size(2.0),
        step2,
        label("").text_size(2.0),
        step3,
    )).padding(xilem::style::Padding::horizontal(16.0));

    // ── Footer note ───────────────────────────────────────────────────────────
    let footer = flex_col((
        label("*Please note, the Signature Devices option is only available to a user")
            .text_size(12.0)
            .text_alignment(xilem::TextAlign::Left)
            .color(gray),
        label("with Admin rights to MyTAXPrepOffice.")
            .text_size(12.0)
            .text_alignment(xilem::TextAlign::Left)
            .color(gray),
    ));

    // ── Button ────────────────────────────────────────────────────────────────
    let btn = text_button("ACTIVATE SIGNATURE PAD", |s: &mut AppState| {
        camera::handle_scan_button(s);
    })
    .background(xilem::style::Background::Color(teal))
    .corner_radius(26.0)
    .border_color(Color::TRANSPARENT);

    let bottom = flex_col((
        label("").text_size(14.0),
        error_label,
        sized_box(btn)
            .expand_width()
            .height(Length::px(52.0))
            .padding(xilem::style::Padding::right(25.0)),
        label("").text_size(34.0),
    ));

    let base = sized_box(
        flex_col((
            header,
            desc,
            steps,
            footer,
            FlexSpacer::Flex(1.0),
            bottom,
        ))
    )
    .expand_width()
    .expand_height()
    .background(xilem::style::Background::Color(Color::WHITE));

    if state.show_permission_popup {
        Either::A(
            sized_box(zstack((
                base,
                permission_required_popup(),
            )))
            .expand_width()
            .expand_height()
        )
    } else {
        Either::B(base)
    }
}

fn permission_required_popup() -> impl xilem::WidgetView<AppState> {
    let black = Color::from_rgba8(0, 0, 0, 255);
    let gray = Color::from_rgba8(116, 122, 123, 255);
    let teal = Color::from_rgba8(0, 80, 116, 255);
    let dim = Color::from_rgba8(0, 0, 0, 150);
    let light_btn = Color::from_rgba8(231, 235, 238, 255);

    let cancel_btn = button(
        label("Cancel")
            .text_size(16.0)
            .color(black),
        |s: &mut AppState| {
            s.show_permission_popup = false;
        }
    )
    .background(xilem::style::Background::Color(light_btn))
    .corner_radius(22.0)
    .border_color(Color::TRANSPARENT);

    let settings_btn = text_button("Go to Settings", |s: &mut AppState| {
        s.show_permission_popup = false;
        camera::open_app_settings();
    })
    .background(xilem::style::Background::Color(teal))
    .corner_radius(22.0)
    .border_color(Color::TRANSPARENT);

    let card = sized_box(
        flex_col((
            label("").text_size(2.0),
            label("Permission Required")
                .text_size(22.0)
                .weight(xilem::FontWeight::BOLD)
                .text_alignment(xilem::TextAlign::Center)
                .color(black),
            // Force wrapping by splitting into multiple short labels
            label("The camera permission is necessary")
                .text_size(14.0)
                .text_alignment(xilem::TextAlign::Center)
                .color(gray),
            label("for using this application.")
                .text_size(14.0)
                .text_alignment(xilem::TextAlign::Center)
                .color(gray),
            label("To proceed, please grant the permission")
                .text_size(14.0)
                .text_alignment(xilem::TextAlign::Center)
                .color(gray),
            label("in app settings.")
                .text_size(14.0)
                .text_alignment(xilem::TextAlign::Center)
                .color(gray),
            flex_row((
                sized_box(cancel_btn)
                    .width(Length::px(130.0))
                    .height(Length::px(44.0)),
                sized_box(settings_btn)
                    .width(Length::px(150.0))
                    .height(Length::px(44.0)),
            )),
            label("").text_size(6.0),
        ))
        .background(xilem::style::Background::Color(Color::WHITE))
        .corner_radius(16.0)
    )
    .width(Length::px(340.0));

    sized_box(zstack((
        sized_box(label(""))
            .expand_width()
            .expand_height()
            .background(xilem::style::Background::Color(dim)),
        card,
    )))
    .expand_width()
    .expand_height()
}

// ── Success screen ────────────────────────────────────────────────────────────
fn success_screen(_state: &AppState) -> impl xilem::WidgetView<AppState> {
    let gray        = Color::from_rgba8(116, 122, 123, 255);
    let black       = Color::from_rgba8(0, 0, 0, 255);
    let teal        = Color::from_rgba8(0, 80, 116, 255);

    let success_icon = image_assets::get_icon().clone();
    let bullet_icon  = image_assets::get_bullet_icon().clone();

    // ── Header: icon centered, title centered ─────────────────────────────────
    let header = flex_col((
        label("").text_size(40.0),
        sized_box(image(success_icon))
            .width(Length::px(110.0))
            .height(Length::px(110.0)),
        label("Capture Signature")
            .text_size(26.0)
            .weight(xilem::FontWeight::BOLD)
            .text_alignment(xilem::TextAlign::Center)
            .color(black),
        label("").text_size(2.0),
    ));

    // ── Intro: left-aligned ───────────────────────────────────────────────────
    let desc = flex_col((
        label("To capture a signature, follow the steps below:")
            .text_size(15.0)
            .text_alignment(xilem::TextAlign::Left)
            .color(gray),
    ));

    // ── Step 1 + sub-text ─────────────────────────────────────────────────────
    let step1 = flex_row((
        sized_box(image(bullet_icon.clone()))
            .width(Length::px(16.0))
            .height(Length::px(16.0)),
        label("Login to MyTAXPrepOffice.")
            .text_size(15.0)
            .text_alignment(xilem::TextAlign::Left)
            .color(gray),
    ));

    let step1_sub = flex_col((
        label("To capture an ERO / Preparer signature, open")
            .text_size(15.0)
            .text_alignment(xilem::TextAlign::Left)
            .color(gray),
        label("the Firm Setup from the Your Firm Widget, Then")
            .text_size(15.0)
            .text_alignment(xilem::TextAlign::Left)
            .color(gray),
        label("click on the pencil icon to edit the user and in that")
            .text_size(15.0)
            .text_alignment(xilem::TextAlign::Left)
            .color(gray),
        label("Scroll down to the bottom to locate the Signature")
            .text_size(15.0)
            .text_alignment(xilem::TextAlign::Left)
            .color(gray),
        label("section.")
            .text_size(15.0)
            .text_alignment(xilem::TextAlign::Left)
            .color(gray),
        label("").text_size(1.0),
        label("To capture a taxpayer signature, open the return,")
            .text_size(15.0)
            .text_alignment(xilem::TextAlign::Left)
            .color(gray),
        label("click the drop down arrow on the Signature button")
            .text_size(15.0)
            .text_alignment(xilem::TextAlign::Left)
            .color(gray),
        label("from the menu.")
            .text_size(15.0)
            .text_alignment(xilem::TextAlign::Left)
            .color(gray),
    )).padding(xilem::style::Padding::left(6.0));

    // ── Step 2 ────────────────────────────────────────────────────────────────
    let step2 = flex_row((
        sized_box(image(bullet_icon.clone()))
            .width(Length::px(16.0))
            .height(Length::px(16.0)),
        
        flex_col((
            label("In MyTAXPrepOffice, click Capture to display the")
                .text_size(15.0)
                .text_alignment(xilem::TextAlign::Left)
                .color(gray),
            label("Capture Signature QR Code.")
                .text_size(15.0)
                .text_alignment(xilem::TextAlign::Left)
                .color(gray),
        )),
    ));

    // ── Step 3 ────────────────────────────────────────────────────────────────
    let step3 = flex_row((
        sized_box(image(bullet_icon.clone()))
            .width(Length::px(16.0))
            .height(Length::px(16.0)),
        flex_col((
            label("Press the Capture Signature button below and")
                .text_size(15.0)
                .text_alignment(xilem::TextAlign::Left)
                .color(gray),
            label("scan the Capture Signature QR Code.")
                .text_size(15.0)
                .text_alignment(xilem::TextAlign::Left)
                .color(gray),
        )),
    ));

    // ── Step 4 ────────────────────────────────────────────────────────────────
    let step4 = flex_row((
        sized_box(image(bullet_icon.clone()))
            .width(Length::px(16.0))
            .height(Length::px(16.0)),
        label("Press Draw Signature, sign, then press Accept.")
            .text_size(15.0)
            .text_alignment(xilem::TextAlign::Left)
            .color(gray),
    ));

    // ── Step 5 ────────────────────────────────────────────────────────────────
    let step5 = flex_row((
        sized_box(image(bullet_icon.clone()))
            .width(Length::px(16.0))
            .height(Length::px(16.0)),
        label("Press Upload")
            .text_size(15.0)
            .text_alignment(xilem::TextAlign::Left)
            .color(gray),
    ));

    let steps = flex_col((
        step1,
        step1_sub,
       // label("").text_size(2.0),
        step2,
        //label("").text_size(2.0),
        step3,
        //label("").text_size(2.0),
        step4,
        //label("").text_size(2.0),
        step5,
    )).padding(xilem::style::Padding::horizontal(16.0));

    // ── Button ────────────────────────────────────────────────────────────────
    let btn = text_button("CAPTURE SIGNATURE", |s: &mut AppState| {
        s.qr_result = None;
        s.qr_pending = false;
        camera::clear_qr_result();
        s.scan_mode = ScanMode::SignatureCapture;
        camera::handle_scan_button(s);
    })
    .background(xilem::style::Background::Color(teal))
    .corner_radius(26.0)
    .border_color(Color::TRANSPARENT);

    let bottom = flex_col((
        label("").text_size(14.0),
        flex_row((
            FlexSpacer::Flex(1.0),
            sized_box(btn)
                .expand_width()
                .height(Length::px(52.0))
                .padding(xilem::style::Padding::right(25.0)),
            FlexSpacer::Flex(1.0),
        )),
        label("").text_size(34.0),
    ));

    sized_box(
        flex_col((
            header,
            desc,
            steps,
            FlexSpacer::Flex(1.0),
            bottom,
        ))
    )
    .expand_width()
    .expand_height()
    .background(xilem::style::Background::Color(Color::WHITE))
}

// ── Signature Capture Result screen ──────────────────────────────────────────
fn signature_capture_screen(state: &mut AppState) -> impl xilem::WidgetView<AppState> {
    let gray  = Color::from_rgba8(116, 122, 123, 255);
    let black = Color::from_rgba8(0, 0, 0, 255);
    let teal  = Color::from_rgba8(0, 80, 116, 255);

    let icon = image_assets::get_signature_icon().clone();

    let center_content = flex_row((
        FlexSpacer::Flex(1.0),
        flex_col((
            sized_box(image(icon))
                .width(Length::px(150.0))
                .height(Length::px(150.0)),
            label("Capture Signature For Taxpayer")
                .text_size(24.0)
                .text_alignment(xilem::TextAlign::Center)
                .color(gray),
            label("")
                .text_size(25.0),
            sized_box(label(" "))
                .width(Length::px(210.0))
                .height(Length::px(1.0))
                .background(xilem::style::Background::Color(black)),
        )),
        FlexSpacer::Flex(1.0),
    ));

    let btn = text_button("DRAW SIGNATURE", |s: &mut AppState| {
        s.signature_pad_from_preview = false;
        if let Ok(mut pad) = s.signature_pad.lock() {
            pad.strokes.clear();
            pad.can_draw = true;
        }
        camera::set_landscape_orientation();
        s.set_screen(Screen::SignaturePad);
    })
    .background(xilem::style::Background::Color(teal))
    .corner_radius(26.0)
    .border_color(Color::TRANSPARENT);

    let bottom = flex_col((
        label("").text_size(14.0),
        flex_row((
            FlexSpacer::Flex(1.0),
            sized_box(btn)
                .expand_width()
                .height(Length::px(52.0))
                .padding(xilem::style::Padding::right(25.0)),
            FlexSpacer::Flex(1.0),
        )),
        label("").text_size(34.0),
    ));

    sized_box(
        flex_col((
            FlexSpacer::Flex(1.5),
            center_content,
            FlexSpacer::Flex(1.0),
            bottom,
        ))
    )
    .expand_width()
    .expand_height()
    .background(xilem::style::Background::Color(Color::WHITE))
}

fn signature_pad_screen(state: &mut AppState) -> impl xilem::WidgetView<AppState> {
    let teal = Color::from_rgba8(0, 80, 116, 255);
    let black = Color::from_rgba8(0, 0, 0, 255);

    let pad = sized_box(signature_pad_view(Arc::clone(&state.signature_pad)))
        .expand_width()
        .expand_height();

    let clear_btn = text_button("CLEAR", |s: &mut AppState| {
        if let Ok(mut pad) = s.signature_pad.lock() {
            pad.strokes.clear();
            pad.can_draw = true;
        }
        notify_signature_pad_changed();
        camera::wakeup_ui();
    })
    .background(xilem::style::Background::Color(teal))
    .corner_radius(22.0)
    .border_color(Color::TRANSPARENT);

    let cancel_btn = text_button("CANCEL", |s: &mut AppState| {
        if let Ok(mut pad) = s.signature_pad.lock() {
            pad.strokes.clear();
            pad.can_draw = true;
        }
        camera::set_portrait_orientation();
        if s.signature_pad_from_preview {
            s.signature_pad_from_preview = false;
            s.set_screen(Screen::SignaturePreview);
        } else {
            s.set_screen(Screen::SignatureCapture);
        }
    })
    .background(xilem::style::Background::Color(teal))
    .corner_radius(22.0)
    .border_color(Color::TRANSPARENT);

    let accept_btn = text_button("ACCEPT", |s: &mut AppState| {
        let has_signature = if let Ok(pad) = s.signature_pad.lock() {
            pad.strokes.iter().any(|stroke| !stroke.is_empty())
        } else {
            false
        };

        if !has_signature {
            return;
        }

        if let Ok(mut pad) = s.signature_pad.lock() {
            pad.can_draw = false;
        }
        camera::set_portrait_orientation();
        s.set_screen(Screen::SignaturePreview);
    })
    .background(xilem::style::Background::Color(teal))
    .corner_radius(26.0)
    .border_color(Color::TRANSPARENT);

    sized_box(
        zstack((
            sized_box(pad)
                .expand_width()
                .expand_height()
                .background(xilem::style::Background::Color(Color::WHITE)),
            flex_col((
                label("").text_size(10.0),
                flex_row((
                    label("").text_size(8.0),
                    sized_box(cancel_btn)
                        .width(Length::px(110.0))
                        .height(Length::px(42.0)),
                    FlexSpacer::Flex(1.0),
                    sized_box(clear_btn)
                        .width(Length::px(110.0))
                        .height(Length::px(42.0)),
                    label("").text_size(8.0),
                )),
                FlexSpacer::Flex(1.0),
                flex_row((
                    FlexSpacer::Flex(1.0),
                    sized_box(accept_btn)
                        .width(Length::px(750.0))
                        .height(Length::px(52.0)),
                    FlexSpacer::Flex(1.0),
                )),
                label("").text_size(10.0),
            )),
        ))
    )
    .expand_width()
    .expand_height()
    .background(xilem::style::Background::Color(black))
    .padding(xilem::style::Padding::all(3.0))
}

fn signature_preview_screen(state: &mut AppState) -> impl xilem::WidgetView<AppState> {
    let gray  = Color::from_rgba8(116, 122, 123, 255);
    let teal = Color::from_rgba8(0, 80, 116, 255);
    let black = Color::from_rgba8(0, 0, 0, 255);
    let edit_icon = image_assets::get_edit_signature_icon().clone();

    let edit_btn = button(
        sized_box(image(edit_icon))
            .width(Length::px(65.0))
            .height(Length::px(65.0)),
        |s: &mut AppState| {
            s.signature_pad_from_preview = true;
            if let Ok(mut pad) = s.signature_pad.lock() {
                pad.can_draw = true;
            }
            camera::set_landscape_orientation();
            s.set_screen(Screen::SignaturePad);
        }
    )
    .background(xilem::style::Background::Color(Color::TRANSPARENT))
    .active_background_color(Color::TRANSPARENT)
    .corner_radius(0.0)
    .border_color(Color::TRANSPARENT)
    .border_width(0.0);

    let preview_canvas = sized_box(signature_pad_view(Arc::clone(&state.signature_pad)))
        .width(Length::px(268.0))
        .height(Length::px(120.0));

    let signature_box = sized_box(
        flex_col((
            flex_row((
                label("").text_size(12.0),
                label("Taxpayer:")
                    .text_size(18.0)
                    .text_alignment(xilem::TextAlign::Left)
                    .color(gray),
                FlexSpacer::Flex(1.0),
                sized_box(edit_btn)
                    .width(Length::px(65.0))
                    .height(Length::px(65.0)),
            )),
            label("").text_size(8.0),
            flex_row((
                label("").text_size(12.0),
                preview_canvas,
                label("").text_size(12.0),
            )),
            label("").text_size(12.0),
        ))
    )
    .width(Length::px(296.0))
    .height(Length::px(184.0))
    .background(xilem::style::Background::Color(Color::WHITE))
    .border_color(black)
    .border_width(1.0);

    let done_btn = text_button("SAVE", |s: &mut AppState| {
        if let Ok(mut pad) = s.signature_pad.lock() { pad.can_draw = true; }
        match save_signature_png(s) {
            Ok(path) => {
                s.signature_save_message = Some(format!("Signature saved successfully.\n{path}"));
            }
            Err(e) => {
                s.signature_save_message = Some(format!("Signature could not be saved: {e}"));
            }
        }
        camera::set_portrait_orientation();
        s.set_screen(Screen::SignatureSaved);
    })
    .background(xilem::style::Background::Color(teal))
    .corner_radius(24.0)
    .border_color(Color::TRANSPARENT);

    sized_box(
        flex_col((
            label("").text_size(44.0),
            flex_row((
                FlexSpacer::Flex(1.0),
                signature_box,
                FlexSpacer::Flex(1.0),
            )),
            FlexSpacer::Flex(1.0),
            flex_row((
                FlexSpacer::Flex(1.0),
                sized_box(done_btn)
                    .width(Length::px(320.0))
                    .height(Length::px(52.0)),
                FlexSpacer::Flex(1.0),
            )),
            label("").text_size(34.0),
        ))
    )
    .expand_width()
    .expand_height()
    .background(xilem::style::Background::Color(Color::WHITE))
}

fn signature_saved_screen(state: &mut AppState) -> impl xilem::WidgetView<AppState> {
    let teal = Color::from_rgba8(0, 80, 116, 255);
    let black = Color::from_rgba8(0, 0, 0, 255);

    let success_icon = image_assets::get_process_complete_icon().clone();

    let finish_btn = text_button("FINISH", |s: &mut AppState| {
        s.qr_result = None;
        s.qr_pending = false;
        s.scan_mode = ScanMode::Activation;
        camera::consume_qr_result();
        s.signature_save_message = None;
        if let Ok(mut pad) = s.signature_pad.lock() {
            pad.strokes.clear();
            pad.can_draw = true;
        }
        camera::set_portrait_orientation();
        s.set_screen(Screen::SignatureCapture);
    })
    .background(xilem::style::Background::Color(teal))
    .corner_radius(24.0)
    .border_color(Color::TRANSPARENT);

    sized_box(
        flex_col((
            FlexSpacer::Flex(1.3),
            flex_row((
                FlexSpacer::Flex(1.0),
                sized_box(image(success_icon))
                    .width(Length::px(128.0))
                    .height(Length::px(128.0)),
                FlexSpacer::Flex(1.0),
            )),
                flex_row((
                    sized_box(label("")).width(Length::px(24.0)),
                    label("All captured signatures have been")
                        .text_size(16.0)
                        .text_alignment(xilem::TextAlign::Left)
                        .color(black),
                )),
                flex_row((
                    sized_box(label("")).width(Length::px(24.0)),
                    label("successfully saved and are available for")
                        .text_size(16.0)
                        .text_alignment(xilem::TextAlign::Left)
                        .color(black),
                )),
                flex_row((
                    sized_box(label("")).width(Length::px(24.0)),
                    label("use in MyTAXPrepOffice. Press Finish to")
                        .text_size(16.0)
                        .text_alignment(xilem::TextAlign::Left)
                        .color(black),
                )),
                flex_row((
                    sized_box(label("")).width(Length::px(24.0)),
                    label("return to the captureSignatures screen.")
                        .text_size(16.0)
                        .text_alignment(xilem::TextAlign::Left)
                        .color(black),
                )),
            FlexSpacer::Flex(0.7),
            flex_row((
                FlexSpacer::Flex(1.0),
                sized_box(finish_btn)
                    .expand_width()
                    .height(Length::px(52.0))
                    .padding(xilem::style::Padding::right(25.0)),
                FlexSpacer::Flex(1.0),
            )),
            label("").text_size(34.0),
        ))
    )
    .expand_width()
    .expand_height()
    .background(xilem::style::Background::Color(Color::WHITE))
}

// ── Entry points ──────────────────────────────────────────────────────────────
pub fn run(event_loop: EventLoopBuilder) -> Result<(), EventLoopError> {
    let main_window_id = WindowId::next();
    let window_icon = load_window_icon();
    let app = Xilem::new(AppState::new(), move |state: &mut AppState| {
        std::iter::once(
            window(main_window_id, "Scanner Signature App", app_logic(state))
                .with_options(|o| {
                    o.on_close(handle_system_back)
                        .with_initial_window_icon(window_icon.clone())
                }),
        )
    });
    app.run_in(event_loop)
}

#[cfg(target_os = "android")]
fn wait_for_android_resume(app: &android_activity::AndroidApp) {
    use android_activity::MainEvent;
    let mut got_resume = false;
    loop {
        app.poll_events(Some(std::time::Duration::from_millis(100)), |event| {
            if let android_activity::PollEvent::Main(main_event) = event {
                match main_event {
                    MainEvent::Resume { .. } | MainEvent::GainedFocus => {
                        got_resume = true;
                    }
                    _ => {}
                }
            }
        });
        if got_resume && app.native_window().is_some() {
            std::thread::sleep(std::time::Duration::from_millis(200));
            break;
        }
    }
}

#[cfg(target_os = "android")]
#[no_mangle]
pub extern "C" fn android_main(app: android_activity::AndroidApp) {
    use std::ffi::CString;
    use ndk_sys::__android_log_print;
    use winit::platform::android::EventLoopBuilderExtAndroid;
    use xilem::EventLoop;

    const ANDROID_LOG_INFO: i32 = 4;
    let c_tag = CString::new("JNI").unwrap();
    let c_msg = CString::new("[JNI] android_main called").unwrap();
    unsafe { __android_log_print(ANDROID_LOG_INFO, c_tag.as_ptr(), c_msg.as_ptr()); }

    android_logger::init_once(
        android_logger::Config::default().with_max_level(log::LevelFilter::Debug),
    );

    // Masonry debug tracing writes to std::env::temp_dir(). On some Android 12
    // builds this defaults to a non-writable location, causing a panic before
    // first frame. Force temp vars to an app-internal writable directory.
    if let Some(mut tmp_dir) = app.internal_data_path() {
        tmp_dir.push("tmp");
        if let Err(e) = std::fs::create_dir_all(&tmp_dir) {
            log::warn!("[MAIN] failed to create tmp dir {}: {e}", tmp_dir.display());
        } else {
            std::env::set_var("TMPDIR", &tmp_dir);
            std::env::set_var("TMP", &tmp_dir);
            std::env::set_var("TEMP", &tmp_dir);
            log::info!("[MAIN] temp dir set to {}", tmp_dir.display());
        }
    } else {
        log::warn!("[MAIN] internal_data_path unavailable; temp dir unchanged");
    }

    init_safe_android_tracing();
    log::info!("\u{1F3AC} android_main started!");

    camera::store_screen_size_from_app(&app);
    camera::init_android_app(app.clone());
    camera::init_qr_channel(); // only clears if QR_READY is false — safe
    camera::init_wakeup_pipe(&app);

    let mut first_run = true;
    loop {
        let app_clone = app.clone();
        let mut event_loop_builder = EventLoop::with_user_event();
        event_loop_builder.with_android_app(app_clone);

        // On restart after a crash, do NOT call init_qr_channel() again —
        // that would wipe the QR result we're preserving for crash recovery.
        // AppState::new() handles recovery by checking qr_result_ready().
        if !first_run {
            log::info!("[MAIN] restarting event loop after crash/error");
        }
        first_run = false;

        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            run(event_loop_builder)
        }));

        match result {
            Ok(Ok(())) => {
                break;
            }
            Ok(Err(EventLoopError::RecreationAttempt)) => {
                camera::reset_runtime_state();
                wait_for_android_resume(&app);
            }
            Ok(Err(EventLoopError::ExitFailure(status))) => {
                log::info!("[MAIN] event loop exited with status {status}");
                camera::reset_runtime_state();
                break;
            }
            Ok(Err(EventLoopError::Os(e))) => {
                log::info!("[MAIN] event loop terminated (OS): {e}");
                camera::reset_runtime_state();
                break;
            }
            Ok(Err(EventLoopError::NotSupported(e))) => {
                log::info!("[MAIN] event loop terminated (NotSupported): {e}");
                camera::reset_runtime_state();
                break;
            }
            Err(_) => {
                log::error!("\u{274C} Panic in event loop");
                camera::reset_runtime_state();
                wait_for_android_resume(&app);
            }
        }
    }
}

#[cfg(not(target_os = "android"))]
fn main() -> Result<(), EventLoopError> {
    use xilem::EventLoop;
    env_logger::init();
    camera::init_qr_channel();
    let event_loop = EventLoop::with_user_event();
    run(event_loop)
}