// src/lib.rs
use winit::error::EventLoopError;
use xilem::view::ZStackExt;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
#[cfg(target_os = "android")]
use std::sync::Once;

use xilem::{
    AnyWidgetView, EventLoopBuilder, WindowOptions, Xilem,
    view::{FlexSpacer, flex_col, flex_row, image, label, sized_box, text_button, zstack},
};
use xilem::core::one_of::Either;
use masonry::peniko::Color;
use masonry::properties::types::Length;
use xilem::style::Style;

mod image_assets;
mod camera;
mod camera_widget;

use camera_widget::camera_view;

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
    Info,
    Scan,
    Success,
}

pub struct AppState {
    pub screen:                Screen,
    pub show_permission_error: bool,
    pub show_permission_popup: bool,
    pub is_scanning:           Arc<AtomicBool>,
    pub qr_result:             Option<String>,
    // ── FIX: explicit flag set by QR detection so app_logic always
    //    transitions even if the anim-loop wakeup arrives late ──────────
    pub qr_pending:            bool,
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
                show_permission_error: false,
                show_permission_popup: false,
                is_scanning:           Arc::new(AtomicBool::new(false)),
                qr_result:             qr,
                qr_pending:            false, // already committed via peek path
            };
        }
        Self {
            screen:                Screen::Info,
            show_permission_error: false,
            show_permission_popup: false,
            is_scanning:           Arc::new(AtomicBool::new(false)),
            qr_result:             None,
            qr_pending:            false,
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
                log::info!("[UI] COMMIT Scan->Success");
                state.qr_pending = false;
                state.set_screen(Screen::Success);
                camera::consume_qr_result();
                camera::wakeup_ui();
                return Box::new(success_screen(state));
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
        log::info!("[UI] transitioning to Success screen");
        state.qr_pending = false;
        state.set_screen(Screen::Success);
        // NOW it is safe to consume: set_screen(Success) completed,
        // so even if wgpu panics immediately after this point, AppState::new()
        // on restart will see screen=Success and qr_result already in state.
        // (Actually on restart AppState is recreated — but qr_result_ready()
        // will still be true until consume runs, so new() recovers correctly.)
        camera::consume_qr_result();
        camera::wakeup_ui();
    }

    match state.screen {
        Screen::Info    => Box::new(info_screen(state)),
        Screen::Scan    => Box::new(scan_screen(state)),
        Screen::Success => Box::new(success_screen(state)),
    }
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
    )).padding(xilem::style::Padding::left(16.0));

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
            .width(Length::px(320.0))
            .height(Length::px(52.0)),
        label("").text_size(16.0),
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
        Either::A(zstack((
            base,
            permission_required_popup(),
        )))
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

    let cancel_btn = text_button("Cancel", |s: &mut AppState| {
        s.show_permission_popup = false;
    })
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
            label("Permission Required")
                .text_size(22.0)
                .weight(xilem::FontWeight::BOLD)
                .text_alignment(xilem::TextAlign::Center)
                .color(black),
            label("").text_size(10.0),
            label("The camera permission is necessary for using this application.")
                .text_size(14.0)
                .text_alignment(xilem::TextAlign::Center)
                .color(gray),
            label("To proceed, please grant the permission in app settings.")
                .text_size(14.0)
                .text_alignment(xilem::TextAlign::Center)
                .color(gray),
            label("").text_size(18.0),
            flex_row((
                sized_box(cancel_btn)
                    .width(Length::px(130.0))
                    .height(Length::px(44.0)),
                label("  ").text_size(10.0),
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

    zstack((
        sized_box(label(""))
            .expand_width()
            .expand_height()
            .background(xilem::style::Background::Color(dim)),
        card,
    ))
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
    )).padding(xilem::style::Padding::left(16.0));

    // ── Button ────────────────────────────────────────────────────────────────
    let btn = text_button("CAPTURE SIGNATURE", |s: &mut AppState| {
        camera::consume_qr_result();
        s.set_screen(Screen::Info);
        s.qr_result = None;
        s.qr_pending = false;
    })
    .background(xilem::style::Background::Color(teal))
    .corner_radius(26.0)
    .border_color(Color::TRANSPARENT);

    let bottom = flex_col((
        label("").text_size(14.0),
        flex_row((
            FlexSpacer::Flex(1.0),
            sized_box(btn)
                .width(Length::px(320.0))
                .height(Length::px(52.0)),
            FlexSpacer::Flex(1.0),
        )),
        label("").text_size(16.0),
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

// ── Entry points ──────────────────────────────────────────────────────────────
pub fn run(event_loop: EventLoopBuilder) -> Result<(), EventLoopError> {
    let app = Xilem::new_simple(
        AppState::new(),
        app_logic,
        WindowOptions::new("Scanner Signature App"),
    );
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
            Ok(Ok(())) => { break; }
            Ok(Err(e)) => {
                let msg = e.to_string();
                if msg.contains("can't be recreated") || msg.contains("EventLoop") {
                    wait_for_android_resume(&app);
                } else {
                    log::error!("\u{274C} EventLoopError: {e}");
                    std::thread::sleep(std::time::Duration::from_millis(300));
                }
            }
            Err(_) => {
                log::error!("\u{274C} Panic in event loop");
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