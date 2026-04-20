#[cfg(target_os = "android")]
use android_activity::AndroidApp;

use winit::error::EventLoopError;

// ✅ Everything comes from xilem directly — no masonry_winit imports needed
use xilem::{
    AnyWidgetView, EventLoopBuilder, WindowOptions, Xilem,
    view::{flex_col, label, text_button},
};

enum Screen {
    Info,
    Scan,
    Success,
}

struct AppState {
    screen: Screen,
}

impl AppState {
    fn new() -> Self {
        Self { screen: Screen::Info }
    }

    fn set_screen(&mut self, screen: Screen) {
        self.screen = screen;
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}

// ✅ Fix 1: Return Box<AnyWidgetView<AppState>> so all match arms share one type
fn app_logic(state: &mut AppState) -> Box<AnyWidgetView<AppState>> {
    match state.screen {
        Screen::Info    => Box::new(info_screen()),
        Screen::Scan    => Box::new(scan_screen()),
        Screen::Success => Box::new(success_screen()),
    }
}

fn info_screen() -> impl xilem::WidgetView<AppState> {
    flex_col((
        label("").text_size(36.0),
        label("📱").text_size(80.0),
        label("").text_size(18.0),
        label("Scanner Signature App")
            .text_size(30.0)
            .weight(xilem::FontWeight::BOLD),
        label("").text_size(10.0),
        label("Scan and verify QR codes for signatures and documents.")
            .text_size(17.0)
            .text_alignment(xilem::TextAlign::Center),
        label("").text_size(18.0),
        label("• Scan QR codes from paper or screen\n• Secure signature verification\n• Works offline and online\n• Simple, privacy-first design")
            .text_size(15.0)
            .text_alignment(xilem::TextAlign::Start),
        label("").text_size(24.0),
        text_button("Scan", |s: &mut AppState| {
            s.set_screen(Screen::Scan);
        }),
        label("").text_size(36.0),
    ))
}

fn scan_screen() -> impl xilem::WidgetView<AppState> {
    flex_col((
        label("").text_size(36.0),
        label("📸").text_size(80.0),
        label("").text_size(30.0),
        label("Scan Screen")
            .text_size(30.0)
            .weight(xilem::FontWeight::BOLD),
        label("").text_size(30.0),
        label("Point your camera at a QR code")
            .text_size(18.0)
            .text_alignment(xilem::TextAlign::Center),
        label("").text_size(40.0),
        text_button("✓ Success", |s: &mut AppState| {
            s.set_screen(Screen::Success);
        }),
        label("").text_size(10.0),
        text_button("← Back", |s: &mut AppState| {
            s.set_screen(Screen::Info);
        }),
        label("").text_size(36.0),
    ))
}

fn success_screen() -> impl xilem::WidgetView<AppState> {
    flex_col((
        label("").text_size(50.0),
        label("✅").text_size(80.0),
        label("").text_size(30.0),
        label("Success!")
            .text_size(40.0)
            .weight(xilem::FontWeight::BOLD),
        label("").text_size(30.0),
        label("QR code verified successfully")
            .text_size(18.0)
            .text_alignment(xilem::TextAlign::Center),
        label("").text_size(40.0),
        text_button("Back to Home", |s: &mut AppState| {
            s.set_screen(Screen::Info);
        }),
        label("").text_size(36.0),
    ))
}

// ✅ Fix 2: EventLoopBuilder comes from xilem, not winit directly
pub fn run(event_loop: EventLoopBuilder) -> Result<(), EventLoopError> {
    let app = Xilem::new_simple(
        AppState::new(),
        app_logic,
        WindowOptions::new("Scanner Signature App"),
    );
    app.run_in(event_loop)
}

#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
pub extern "C" fn android_main(app: AndroidApp) {
    use xilem::EventLoop;
    use winit::platform::android::EventLoopBuilderExtAndroid;

    android_logger::init_once(
        android_logger::Config::default().with_max_level(log::LevelFilter::Info),
    );
    log::info!("android_main started!");

    // ✅ Use xilem::EventLoop (which wraps MasonryUserEvent internally)
    let mut event_loop_builder = EventLoop::with_user_event();
    event_loop_builder.with_android_app(app);

    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        run(event_loop_builder)
    }));

    match result {
        Ok(Ok(())) => log::info!("Event loop exited normally"),
        Ok(Err(e)) => log::error!("Event loop returned error: {e}"),
        Err(_)     => log::error!("Event loop panicked"),
    }

    // Exit process so event loop can be recreated on next launch
    #[cfg(target_os = "android")]
    std::process::exit(0);
}

#[cfg(not(target_os = "android"))]
fn main() -> Result<(), EventLoopError> {
    use xilem::EventLoop;
    // ✅ Use xilem::EventLoop::with_user_event() — correct type for run_in()
    let event_loop = EventLoop::with_user_event();
    run(event_loop)
}