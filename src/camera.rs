// src/camera.rs — pure Rust NDK Camera2 for Xilem/Masonry 0.4

use std::sync::{Arc, Mutex, OnceLock};
use std::sync::atomic::{AtomicU32, Ordering};

// ── Screen size ───────────────────────────────────────────────────────────────
static SCREEN_W: AtomicU32 = AtomicU32::new(1080);
static SCREEN_H: AtomicU32 = AtomicU32::new(1920);

pub fn get_screen_size() -> (u32, u32) {
    (SCREEN_W.load(Ordering::Relaxed), SCREEN_H.load(Ordering::Relaxed))
}

#[cfg(target_os = "android")]
pub fn store_screen_size_from_app(app: &android_activity::AndroidApp) {
    if let Some(win) = app.native_window() {
        let w = unsafe { ndk_sys::ANativeWindow_getWidth(win.ptr().as_ptr()) };
        let h = unsafe { ndk_sys::ANativeWindow_getHeight(win.ptr().as_ptr()) };
        if w > 0 && h > 0 {
            let (sw, sh) = if w < h { (w as u32, h as u32) } else { (h as u32, w as u32) };
            SCREEN_W.store(sw, Ordering::SeqCst);
            SCREEN_H.store(sh, Ordering::SeqCst);
            return;
        }
    }
    let _ = with_jni_raw(
        app.vm_as_ptr() as usize,
        app.activity_as_ptr() as usize,
        |env, activity| {
            let wm = env.call_method(activity, "getWindowManager",
                "()Landroid/view/WindowManager;", &[]).ok()?.l().ok()?;
            let display = env.call_method(&wm, "getDefaultDisplay",
                "()Landroid/view/Display;", &[]).ok()?.l().ok()?;
            let dm_class = env.find_class("android/util/DisplayMetrics").ok()?;
            let dm = env.new_object(dm_class, "()V", &[]).ok()?;
            env.call_method(&display, "getRealMetrics",
                "(Landroid/util/DisplayMetrics;)V",
                &[jni::objects::JValue::Object(&dm)]).ok()?;
            let w = env.get_field(&dm, "widthPixels",  "I").ok()?.i().ok()? as u32;
            let h = env.get_field(&dm, "heightPixels", "I").ok()?.i().ok()? as u32;
            if w > 0 && h > 0 {
                let (sw, sh) = if w < h { (w, h) } else { (h, w) };
                SCREEN_W.store(sw, Ordering::SeqCst);
                SCREEN_H.store(sh, Ordering::SeqCst);
            }
            Some(())
        },
    );
}

#[cfg(not(target_os = "android"))]
pub fn store_screen_size_from_app(_app: &()) {}

// ── Camera frame (pub so camera_widget can use it) ────────────────────────────
#[derive(Clone)]
pub struct CameraFrame {
    pub rgba:   Vec<u8>,
    pub width:  u32,
    pub height: u32,
}

// ── Shared frame buffer ───────────────────────────────────────────────────────
static FRAME_BUF: OnceLock<Arc<Mutex<Option<CameraFrame>>>> = OnceLock::new();

pub fn frame_buf() -> &'static Arc<Mutex<Option<CameraFrame>>> {
    FRAME_BUF.get_or_init(|| Arc::new(Mutex::new(None)))
}

// ── QR result ─────────────────────────────────────────────────────────────────
//
// Design: two-phase commit that survives event-loop crashes.
//
// PROBLEM: When the GPU device is lost (wgpu "Parent device is lost" panic),
// the winit event loop panics and restarts. AppState is recreated fresh, so
// any QR result stored only in AppState is permanently lost.
//
// SOLUTION:
//   Phase 1 — camera thread calls store_qr_result():
//     • Writes the string into the static QR_RESULT mutex
//     • Sets QR_READY = true
//     The result now lives in a 'static — it survives any panic/restart.
//
//   Phase 2 — app_logic calls peek_qr_result():
//     • Returns a CLONE of the string without removing it from the static.
//     • AppState stores the clone and sets qr_pending = true.
//     • If the event loop panics before set_screen(Success) completes,
//       QR_READY is still true on the next restart → AppState::new() sees it
//       and starts directly on the Success screen.
//
//   Commit — consume_qr_result() is called only AFTER set_screen(Success):
//     • Clears QR_READY and removes the string from the mutex.
//     • If this never runs (crash between peek and commit) the next
//       AppState::new() still finds QR_READY=true and goes to Success.

use std::sync::atomic::AtomicBool as QrAtomicBool;
static QR_READY:  QrAtomicBool                    = QrAtomicBool::new(false);
static QR_RESULT: OnceLock<Mutex<Option<String>>> = OnceLock::new();

fn qr_lock() -> &'static Mutex<Option<String>> {
    QR_RESULT.get_or_init(|| Mutex::new(None))
}

/// Called by the camera thread when a QR code is found.
pub fn store_qr_result(s: String) {
    if let Ok(mut g) = qr_lock().lock() { *g = Some(s); }
    // Set flag AFTER writing so peek never sees flag=true with empty string.
    QR_READY.store(true, Ordering::SeqCst);
    log::info!("[QR] stored — QR_READY=true");
}

/// Called by app_logic every frame. Returns Some(clone) without removing
/// the value, so the result survives a crash between this call and commit.
/// Fast path when QR_READY=false costs only one atomic load.
pub fn peek_qr_result() -> Option<String> {
    if !QR_READY.load(Ordering::SeqCst) { return None; }
    // Clone the value — do NOT take() it yet.
    let result = qr_lock().lock().ok()?.as_ref().map(|s| s.clone());
    if result.is_none() {
        // Flag was set but string is missing — defensive clear.
        QR_READY.store(false, Ordering::SeqCst);
        log::warn!("[QR] peek: QR_READY=true but string missing, clearing");
    }
    result
}

/// Called by app_logic AFTER set_screen(Success) confirmed the transition.
/// Removes the result from the static so a Back→re-scan works correctly.
pub fn consume_qr_result() {
    QR_READY.store(false, Ordering::SeqCst);
    if let Ok(mut g) = qr_lock().lock() { *g = None; }
    log::info!("[QR] consumed — QR_READY=false");
}

/// True if a QR result is waiting (used by AppState::new() on restart).
pub fn qr_result_ready() -> bool {
    QR_READY.load(Ordering::SeqCst)
}

/// Called when starting a new scan to clear any stale result.
pub fn clear_qr_result() {
    QR_READY.store(false, Ordering::SeqCst);
    if let Ok(mut g) = qr_lock().lock() { *g = None; }
}

/// Called once at app startup. Only clears if no result is pending —
/// preserves a QR result that survived a crash for AppState::new() recovery.
pub fn init_qr_channel() {
    if !QR_READY.load(Ordering::SeqCst) {
        clear_qr_result();
    }
}

// Keep poll_qr_result as an alias for peek (backward compat with desktop stub)
pub fn poll_qr_result() -> Option<String> { peek_qr_result() }

// ── UI wakeup ─────────────────────────────────────────────────────────────────
#[cfg(target_os = "android")]
static ANDROID_APP_FOR_WAKER: OnceLock<android_activity::AndroidApp> = OnceLock::new();

#[cfg(target_os = "android")]
pub fn init_wakeup_pipe(app: &android_activity::AndroidApp) {
    let _ = ANDROID_APP_FOR_WAKER.set(app.clone());
}

#[cfg(target_os = "android")]
pub fn wakeup_ui() {
    if let Some(app) = ANDROID_APP_FOR_WAKER.get() {
        app.create_waker().wake();
    }
}

#[cfg(not(target_os = "android"))]
pub fn init_wakeup_pipe(_app: &()) {}

#[cfg(not(target_os = "android"))]
pub fn wakeup_ui() {}

// ── JNI helper ────────────────────────────────────────────────────────────────
#[cfg(target_os = "android")]
fn with_jni_raw<T, F>(vm_ptr: usize, act_ptr: usize, f: F) -> Option<T>
where
    F: FnOnce(&mut jni::JNIEnv, &jni::objects::JObject) -> Option<T>,
{
    if vm_ptr == 0 { return None; }
    let vm  = unsafe { jni::JavaVM::from_raw(vm_ptr as *mut _) }.ok()?;
    let mut env = vm.attach_current_thread().ok()?;
    let activity = unsafe {
        jni::objects::JObject::from_raw(act_ptr as jni::sys::jobject)
    };
    // activity_as_ptr is owned by Android. Do not let JObject drop delete it.
    let out = f(&mut env, &activity);
    std::mem::forget(activity);
    out
}

// ═════════════════════════════════════════════════════════════════════════════
// ANDROID IMPLEMENTATION
// ═════════════════════════════════════════════════════════════════════════════
#[cfg(target_os = "android")]
mod android_impl {
    use super::{
        CameraFrame, frame_buf, wakeup_ui, with_jni_raw,
        store_qr_result, clear_qr_result, qr_result_ready,
    };
    use crate::{AppState, Screen};

    use std::sync::{
        atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering},
        Arc, Condvar, Mutex,
    };
    use android_activity::AndroidApp;
    use ndk_sys as ffi;

    const ACAMERA_LENS_FACING: u32                     = 0x0001_0006;
    const ACAMERA_LENS_FACING_BACK: u8                 = 1;
    const AIMAGE_FORMAT_YUV_420_888: i32               = 0x23;
    const ACAMERA_CONTROL_AF_MODE: u32                 = 0x0001_0002;
    const ACAMERA_CONTROL_AF_MODE_CONTINUOUS_VIDEO: u8 = 3;

    const W: i32 = 1280;
    const H: i32 = 720;
    const QR_EVERY: u64 = 4;

    static CAMERA_RUNNING:   AtomicBool  = AtomicBool::new(false);
    static STOP_REQUESTED:   AtomicBool  = AtomicBool::new(false);
    static DEV_DISCONNECTED: AtomicBool  = AtomicBool::new(false);
    static DEV_ERROR:        AtomicBool  = AtomicBool::new(false);
    static JAVA_VM_PTR:      AtomicUsize = AtomicUsize::new(0);
    static ACTIVITY_PTR:     AtomicUsize = AtomicUsize::new(0);
    static QR_FOUND_EXIT:    AtomicBool  = AtomicBool::new(false);
    static PERMISSION_PENDING: AtomicBool = AtomicBool::new(false);
    static PERMISSION_WATCHDOG_RUNNING: AtomicBool = AtomicBool::new(false);
    static PERMISSION_REQUESTED_AT_MS: AtomicU64 = AtomicU64::new(0);
    static SHOW_PERMISSION_SETTINGS_POPUP: AtomicBool = AtomicBool::new(false);

    // ── Frame-available condvar ───────────────────────────────────────────────
    struct FrameSignal { mutex: Mutex<bool>, cond: Condvar }

    static FRAME_SIGNAL: std::sync::OnceLock<Arc<FrameSignal>> =
        std::sync::OnceLock::new();

    fn frame_signal() -> &'static Arc<FrameSignal> {
        FRAME_SIGNAL.get_or_init(|| Arc::new(FrameSignal {
            mutex: Mutex::new(false),
            cond:  Condvar::new(),
        }))
    }

    fn signal_frame() {
        let s = frame_signal();
        if let Ok(mut g) = s.mutex.lock() { *g = true; s.cond.notify_one(); }
    }

    fn wait_for_frame(ms: u64) -> bool {
        let s = frame_signal();
        if let Ok(g) = s.mutex.lock() {
            match s.cond.wait_timeout(g, std::time::Duration::from_millis(ms)) {
                Ok((mut g2, _)) => { let r = *g2; *g2 = false; r }
                Err(_) => false,
            }
        } else { false }
    }

    unsafe extern "C" fn on_image_available(
        _: *mut std::ffi::c_void, _: *mut ffi::AImageReader,
    ) { signal_frame(); }

    unsafe extern "C" fn on_dev_disconnected(
        _: *mut std::ffi::c_void, _: *mut ffi::ACameraDevice,
    ) { DEV_DISCONNECTED.store(true, Ordering::SeqCst); signal_frame(); }

    unsafe extern "C" fn on_dev_error(
        _: *mut std::ffi::c_void, _: *mut ffi::ACameraDevice, _: i32,
    ) { DEV_ERROR.store(true, Ordering::SeqCst); signal_frame(); }

    unsafe extern "C" fn on_sess_ready(_: *mut std::ffi::c_void,
        _: *mut ffi::ACameraCaptureSession) {}
    unsafe extern "C" fn on_sess_active(_: *mut std::ffi::c_void,
        _: *mut ffi::ACameraCaptureSession) {
        log::info!("[CAM] session active — frames flowing");
    }
    unsafe extern "C" fn on_sess_closed(_: *mut std::ffi::c_void,
        _: *mut ffi::ACameraCaptureSession) {}

    pub fn init(app: AndroidApp) {
        JAVA_VM_PTR.store(app.vm_as_ptr() as usize, Ordering::SeqCst);
        ACTIVITY_PTR.store(app.activity_as_ptr() as usize, Ordering::SeqCst);
    }

    pub fn hide() {
        log::info!("[CAM] hide: signalling stop");
        STOP_REQUESTED.store(true, Ordering::SeqCst);
        signal_frame();
        if let Ok(mut fb) = frame_buf().lock() { *fb = None; }
        CAMERA_RUNNING.store(false, Ordering::SeqCst);
    }

    fn with_jni<T, F: FnOnce(&mut jni::JNIEnv, &jni::objects::JObject) -> Option<T>>(
        f: F,
    ) -> Option<T> {
        with_jni_raw(
            JAVA_VM_PTR.load(Ordering::SeqCst),
            ACTIVITY_PTR.load(Ordering::SeqCst),
            f,
        )
    }

    fn has_camera_permission() -> bool {
        with_jni(|env, act| {
            let p = env.new_string("android.permission.CAMERA").ok()?;
            let r = env.call_method(act, "checkSelfPermission",
                "(Ljava/lang/String;)I",
                &[jni::objects::JValue::Object(&p.into())]).ok()?.i().ok()?;
            Some(r == 0)
        }).unwrap_or(false)
    }

    fn has_window_focus() -> bool {
        with_jni(|env, act| {
            let focused = env.call_method(act, "hasWindowFocus", "()Z", &[])
                .ok()?
                .z()
                .ok()?;
            Some(focused)
        }).unwrap_or(true)
    }

    fn now_ms() -> u64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0)
    }

    fn maybe_flag_permission_denied() {
        if !PERMISSION_PENDING.load(Ordering::SeqCst) {
            return;
        }
        if has_camera_permission() {
            return;
        }

        let requested_at = PERMISSION_REQUESTED_AT_MS.load(Ordering::SeqCst);
        if requested_at == 0 {
            return;
        }

        // Wait a bit so we don't race the system permission prompt opening.
        if now_ms().saturating_sub(requested_at) < 800 {
            return;
        }

        // If the system prompt is still visible, activity usually loses focus.
        if !has_window_focus() {
            return;
        }

        if PERMISSION_PENDING
            .compare_exchange(true, false, Ordering::SeqCst, Ordering::SeqCst)
            .is_err()
        {
            return;
        }

        SHOW_PERMISSION_SETTINGS_POPUP.store(true, Ordering::SeqCst);
        log::info!("[CAM] permission denied -> show settings popup");
        wakeup_ui();
    }

    fn request_camera_permission() {
        with_jni(|env, act| {
           
            let ps = env.new_string("android.permission.CAMERA").ok()?;
            let po: jni::objects::JObject = ps.into();
            let arr = env.new_object_array(1, "java/lang/String", &po).ok()?;
            env.call_method(act, "requestPermissions",
                "([Ljava/lang/String;I)V",
                &[jni::objects::JValue::Object(&arr.into()),
                  jni::objects::JValue::Int(1001)]).ok()?;
            Some(())
        });
    }

    fn start_permission_watchdog() {
        if PERMISSION_WATCHDOG_RUNNING
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_err()
        {
            return;
        }

        std::thread::spawn(|| {
            // Keep UI waking while permission dialog is open so app_logic gets
            // chances to observe newly granted permission on all Android versions.
            for _ in 0..120 {
                if !PERMISSION_PENDING.load(Ordering::SeqCst) {
                    break;
                }
                wakeup_ui();
                if has_camera_permission() {
                    wakeup_ui();
                    break;
                }
                std::thread::sleep(std::time::Duration::from_millis(125));
            }
            PERMISSION_WATCHDOG_RUNNING.store(false, Ordering::SeqCst);
        });
    }

    pub fn poll_permission(state: &mut AppState) {
        if !matches!(state.screen, Screen::Info) { return; }
        if qr_result_ready() { return; }
        if !PERMISSION_PENDING.load(Ordering::SeqCst) { return; }
        if !has_camera_permission() {
            maybe_flag_permission_denied();
            return;
        }
        if CAMERA_RUNNING.load(Ordering::SeqCst) {
            PERMISSION_PENDING.store(false, Ordering::SeqCst);
            return;
        }
        if PERMISSION_PENDING
            .compare_exchange(true, false, Ordering::SeqCst, Ordering::SeqCst)
            .is_err()
        { return; }
        log::info!("[CAM] permission granted (polled) → starting scan");
        state.show_permission_error = false;
        if !qr_result_ready() {
            clear_qr_result();
        }
        state.set_screen(Screen::Scan);
        start_camera_thread();
        wakeup_ui();
    }

    pub fn on_resumed(state: &mut AppState) {
        if !matches!(state.screen, Screen::Info) { return; }
        if qr_result_ready() { return; }
        if !PERMISSION_PENDING.load(Ordering::SeqCst) { return; }
        if !has_camera_permission() {
            maybe_flag_permission_denied();
            return;
        }
        if CAMERA_RUNNING.load(Ordering::SeqCst) {
            PERMISSION_PENDING.store(false, Ordering::SeqCst);
            return;
        }
        if PERMISSION_PENDING
            .compare_exchange(true, false, Ordering::SeqCst, Ordering::SeqCst)
            .is_err()
        { return; }
        log::info!("[CAM] permission granted (resumed) → starting scan");
        state.show_permission_error = false;
        if !qr_result_ready() {
            clear_qr_result();
        }
        state.set_screen(Screen::Scan);
        std::thread::spawn(|| {
        std::thread::sleep(std::time::Duration::from_millis(300));
        start_camera_thread();
        wakeup_ui();
    });
    }

    pub fn scan_button(state: &mut AppState) {
        PERMISSION_PENDING.store(false, Ordering::SeqCst);
        if !has_camera_permission() {
            request_camera_permission();
            PERMISSION_REQUESTED_AT_MS.store(now_ms(), Ordering::SeqCst);
            PERMISSION_PENDING.store(true, Ordering::SeqCst);
            start_permission_watchdog();
            state.show_permission_error = false;
            return;
        }
        clear_qr_result();
        state.set_screen(Screen::Scan);
        start_camera_thread();
        wakeup_ui();
    }

    pub fn take_permission_settings_popup_request() -> bool {
        SHOW_PERMISSION_SETTINGS_POPUP.swap(false, Ordering::SeqCst)
    }

    pub fn open_app_settings() {
        let _ = with_jni(|env, act| {
            let action = env
                .new_string("android.settings.APPLICATION_DETAILS_SETTINGS")
                .ok()?;
            let action_obj: jni::objects::JObject = action.into();

            let intent = env
                .new_object(
                    "android/content/Intent",
                    "(Ljava/lang/String;)V",
                    &[jni::objects::JValue::Object(&action_obj)],
                )
                .ok()?;

            let pkg_obj = env
                .call_method(act, "getPackageName", "()Ljava/lang/String;", &[])
                .ok()?
                .l()
                .ok()?;
            let pkg_j = jni::objects::JString::from(pkg_obj);
            let pkg: String = env.get_string(&pkg_j).ok()?.into();

            let uri_s = env.new_string(format!("package:{pkg}")).ok()?;
            let uri_s_obj: jni::objects::JObject = uri_s.into();
            let uri = env
                .call_static_method(
                    "android/net/Uri",
                    "parse",
                    "(Ljava/lang/String;)Landroid/net/Uri;",
                    &[jni::objects::JValue::Object(&uri_s_obj)],
                )
                .ok()?
                .l()
                .ok()?;

            env.call_method(
                &intent,
                "setData",
                "(Landroid/net/Uri;)Landroid/content/Intent;",
                &[jni::objects::JValue::Object(&uri)],
            )
            .ok()?;

            env.call_method(
                act,
                "startActivity",
                "(Landroid/content/Intent;)V",
                &[jni::objects::JValue::Object(&intent)],
            )
            .ok()?;

            Some(())
        });
    }

    fn start_camera_thread() {
        STOP_REQUESTED.store(false, Ordering::SeqCst);
        DEV_DISCONNECTED.store(false, Ordering::SeqCst);
        DEV_ERROR.store(false, Ordering::SeqCst);
        QR_FOUND_EXIT.store(false, Ordering::SeqCst);

        if CAMERA_RUNNING
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_err()
        {
            log::warn!("[CAM] CAMERA_RUNNING still set — force clearing");
            CAMERA_RUNNING.store(false, Ordering::SeqCst);
            if CAMERA_RUNNING
                .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
                .is_err()
            {
                log::error!("[CAM] aborting spawn");
                return;
            }
        }

        log::info!("[CAM] spawning camera thread");
        std::thread::spawn(move || {
            log::info!("[CAM] thread started");
            if let Err(e) = run_camera() {
                log::error!("[CAM] error: {e}");
            }
            CAMERA_RUNNING.store(false, Ordering::SeqCst);
            // Keep frame_buf if QR was found — last frame stays until set_screen
            // clears it via hide(). On error/stop, clear it immediately.
            if !QR_FOUND_EXIT.load(Ordering::SeqCst) {
                if let Ok(mut fb) = frame_buf().lock() { *fb = None; }
            }
            wakeup_ui();
            log::info!("[CAM] thread exited");
        });
    }

    fn run_camera() -> Result<(), Box<dyn std::error::Error>> {
        let manager = unsafe { ffi::ACameraManager_create() };
        if manager.is_null() { return Err("ACameraManager_create null".into()); }
        scopeguard::defer! { unsafe { ffi::ACameraManager_delete(manager); } }

        let cam_id   = pick_back_camera(manager)?;
        let cam_id_c = std::ffi::CString::new(cam_id.as_str())?;
        log::info!("[CAM] opening camera {cam_id}");

        let mut dev_cbs = ffi::ACameraDevice_StateCallbacks {
            context:        std::ptr::null_mut(),
            onDisconnected: Some(on_dev_disconnected),
            onError:        Some(on_dev_error),
        };
        let mut device: *mut ffi::ACameraDevice = std::ptr::null_mut();
        let st = unsafe {
            ffi::ACameraManager_openCamera(
                manager, cam_id_c.as_ptr(), &mut dev_cbs, &mut device,
            )
        };
        if st != ffi::camera_status_t::ACAMERA_OK || device.is_null() {
            return Err(format!("openCamera: {st:?}").into());
        }
        scopeguard::defer! { unsafe { ffi::ACameraDevice_close(device); } }

        let mut reader: *mut ffi::AImageReader = std::ptr::null_mut();
        let st = unsafe {
            ffi::AImageReader_new(W, H, AIMAGE_FORMAT_YUV_420_888, 3, &mut reader)
        };
        if st != ffi::media_status_t::AMEDIA_OK || reader.is_null() {
            return Err(format!("AImageReader_new: {st:?}").into());
        }
        scopeguard::defer! { unsafe { ffi::AImageReader_delete(reader); } }

        let mut img_listener = ffi::AImageReader_ImageListener {
            context:          std::ptr::null_mut(),
            onImageAvailable: Some(on_image_available),
        };
        unsafe { ffi::AImageReader_setImageListener(reader, &mut img_listener); }

        let mut reader_anw: *mut ffi::ANativeWindow = std::ptr::null_mut();
        let st = unsafe { ffi::AImageReader_getWindow(reader, &mut reader_anw) };
        if st != ffi::media_status_t::AMEDIA_OK || reader_anw.is_null() {
            return Err("AImageReader_getWindow failed".into());
        }
        unsafe { ffi::ANativeWindow_acquire(reader_anw); }
        scopeguard::defer! { unsafe { ffi::ANativeWindow_release(reader_anw); } }

        let mut container: *mut ffi::ACaptureSessionOutputContainer = std::ptr::null_mut();
        unsafe { ffi::ACaptureSessionOutputContainer_create(&mut container); }
        scopeguard::defer! { unsafe { ffi::ACaptureSessionOutputContainer_free(container); } }

        let mut reader_out: *mut ffi::ACaptureSessionOutput = std::ptr::null_mut();
        unsafe {
            ffi::ACaptureSessionOutput_create(reader_anw, &mut reader_out);
            ffi::ACaptureSessionOutputContainer_add(container, reader_out);
        }
        scopeguard::defer! { unsafe { ffi::ACaptureSessionOutput_free(reader_out); } }

        let mut request: *mut ffi::ACaptureRequest = std::ptr::null_mut();
        unsafe {
            ffi::ACameraDevice_createCaptureRequest(
                device,
                ffi::ACameraDevice_request_template::TEMPLATE_PREVIEW,
                &mut request,
            );
        }
        if request.is_null() { return Err("createCaptureRequest null".into()); }
        scopeguard::defer! { unsafe { ffi::ACaptureRequest_free(request); } }

        unsafe {
            let af = ACAMERA_CONTROL_AF_MODE_CONTINUOUS_VIDEO;
            ffi::ACaptureRequest_setEntry_u8(request, ACAMERA_CONTROL_AF_MODE, 1, &af);
        }

        let mut target: *mut ffi::ACameraOutputTarget = std::ptr::null_mut();
        unsafe {
            ffi::ACameraOutputTarget_create(reader_anw, &mut target);
            ffi::ACaptureRequest_addTarget(request, target);
        }
        scopeguard::defer! { unsafe { ffi::ACameraOutputTarget_free(target); } }

        let mut sess_cbs = ffi::ACameraCaptureSession_stateCallbacks {
            context:  std::ptr::null_mut(),
            onClosed: Some(on_sess_closed),
            onReady:  Some(on_sess_ready),
            onActive: Some(on_sess_active),
        };
        let mut session: *mut ffi::ACameraCaptureSession = std::ptr::null_mut();
        let st = unsafe {
            ffi::ACameraDevice_createCaptureSession(
                device, container, &mut sess_cbs, &mut session,
            )
        };
        if st != ffi::camera_status_t::ACAMERA_OK || session.is_null() {
            return Err(format!("createCaptureSession: {st:?}").into());
        }
        scopeguard::defer! {
            unsafe {
                ffi::ACameraCaptureSession_stopRepeating(session);
                ffi::ACameraCaptureSession_close(session);
            }
        }

        let mut seq_id = 0i32;
        let reqs = [request];
        let st = unsafe {
            ffi::ACameraCaptureSession_setRepeatingRequest(
                session, std::ptr::null_mut(), 1,
                reqs.as_ptr() as *mut _, &mut seq_id,
            )
        };
        if st != ffi::camera_status_t::ACAMERA_OK {
            return Err(format!("setRepeatingRequest: {st:?}").into());
        }
        log::info!("[CAM] capture started seq={seq_id}");
        wakeup_ui();

        let mut total: u64 = 0;

        loop {
            if STOP_REQUESTED.load(Ordering::SeqCst)
                || DEV_DISCONNECTED.load(Ordering::SeqCst)
                || DEV_ERROR.load(Ordering::SeqCst)
            { break; }

            wait_for_frame(80);

            if STOP_REQUESTED.load(Ordering::SeqCst)
                || DEV_DISCONNECTED.load(Ordering::SeqCst)
                || DEV_ERROR.load(Ordering::SeqCst)
            { break; }

            let mut image: *mut ffi::AImage = std::ptr::null_mut();
            let st = unsafe { ffi::AImageReader_acquireLatestImage(reader, &mut image) };
            if st != ffi::media_status_t::AMEDIA_OK || image.is_null() { continue; }

            let mut img_w = 0i32;
            let mut img_h = 0i32;
            unsafe {
                ffi::AImage_getWidth(image,  &mut img_w);
                ffi::AImage_getHeight(image, &mut img_h);
            }
            if img_w <= 0 || img_h <= 0 {
                unsafe { ffi::AImage_delete(image); }
                continue;
            }

            total += 1;
            if total == 1 { log::info!("[CAM] first frame {}x{}", img_w, img_h); }

            // ── QR decode every QR_EVERY frames ──────────────────────────────
            if total % QR_EVERY == 0 {
                if let Some(qr) = decode_qr(image, img_w as u32, img_h as u32) {
                    log::info!("[CAM] QR found: {qr}");
                    unsafe { ffi::AImage_delete(image); }

                    QR_FOUND_EXIT.store(true, Ordering::SeqCst);
                    STOP_REQUESTED.store(true, Ordering::SeqCst);
                    PERMISSION_PENDING.store(false, Ordering::SeqCst);
                    store_qr_result(qr);
                    // Trigger multiple wakeups to survive devices where a
                    // single wake is occasionally dropped around camera teardown.
                    wakeup_ui();
                    std::thread::spawn(|| {
                        for _ in 0..20 {
                            wakeup_ui();
                            std::thread::sleep(std::time::Duration::from_millis(50));
                        }
                    });
                    return Ok(());
                }
            }

            // ── YUV → RGBA → frame buffer ─────────────────────────────────────
            if let Some(rgba) = yuv_to_rgba(image, img_w as u32, img_h as u32) {
                if let Ok(mut fb) = frame_buf().lock() {
                    *fb = Some(CameraFrame {
                        rgba, width: img_w as u32, height: img_h as u32,
                    });
                }
                wakeup_ui();
            }

            unsafe { ffi::AImage_delete(image); }
        }

        log::info!("[CAM] loop done after {total} frames");
        Ok(())
    }

    fn yuv_to_rgba(image: *mut ffi::AImage, width: u32, height: u32) -> Option<Vec<u8>> {
        let (w, h) = (width as usize, height as usize);

        macro_rules! plane {
            ($idx:expr) => {{
                let mut ptr: *mut u8 = std::ptr::null_mut();
                let mut len: i32     = 0;
                let mut rs: i32 = width as i32;
                let mut ps: i32 = 1;
                unsafe {
                    let st = ffi::AImage_getPlaneData(image, $idx, &mut ptr, &mut len);
                    if st != ffi::media_status_t::AMEDIA_OK { return None; }
                    ffi::AImage_getPlaneRowStride(image, $idx, &mut rs);
                    ffi::AImage_getPlanePixelStride(image, $idx, &mut ps);
                }
                if ptr.is_null() || len <= 0 { return None; }
                (unsafe { std::slice::from_raw_parts(ptr, len as usize) },
                 rs.max(1) as usize, ps.max(1) as usize)
            }};
        }

        let (y_d, y_rs, y_ps) = plane!(0);
        let (u_d, u_rs, u_ps) = plane!(1);
        let (v_d, v_rs, v_ps) = plane!(2);

        let mut out = vec![255u8; w * h * 4];
        for row in 0..h {
            let uv_row = row >> 1;
            for col in 0..w {
                let uv_col = col >> 1;
                let yi = row    * y_rs + col    * y_ps;
                let ui = uv_row * u_rs + uv_col * u_ps;
                let vi = uv_row * v_rs + uv_col * v_ps;
                if yi >= y_d.len() || ui >= u_d.len() || vi >= v_d.len() { continue; }
                let y = y_d[yi] as i32;
                let u = u_d[ui] as i32 - 128;
                let v = v_d[vi] as i32 - 128;
                let r = (y * 1000 + 1402 * v) / 1000;
                let g = (y * 1000 - 344  * u - 714 * v) / 1000;
                let b = (y * 1000 + 1772 * u) / 1000;
                let i = (row * w + col) * 4;
                out[i]     = r.clamp(0, 255) as u8;
                out[i + 1] = g.clamp(0, 255) as u8;
                out[i + 2] = b.clamp(0, 255) as u8;
            }
        }
        Some(out)
    }

    fn decode_qr(image: *mut ffi::AImage, width: u32, height: u32) -> Option<String> {
        let mut ptr: *mut u8 = std::ptr::null_mut();
        let mut len: i32     = 0;
        let st = unsafe { ffi::AImage_getPlaneData(image, 0, &mut ptr, &mut len) };
        if st != ffi::media_status_t::AMEDIA_OK || ptr.is_null() || len <= 0 { return None; }

        let mut row_stride: i32 = width as i32;
        unsafe { ffi::AImage_getPlaneRowStride(image, 0, &mut row_stride); }
        let rs = row_stride.max(1) as usize;
        let (w, h) = (width as usize, height as usize);
        let src = unsafe { std::slice::from_raw_parts(ptr, len as usize) };

        let y: Vec<u8> = if rs == w {
            if src.len() < w * h { return None; }
            src[..w * h].to_vec()
        } else {
            let needed = h.saturating_sub(1) * rs + w;
            if src.len() < needed { return None; }
            (0..h).flat_map(|r| src[r * rs..r * rs + w].iter().copied()).collect()
        };

        // Try 1: landscape 1280×720
        if let Some(gray) = image::GrayImage::from_raw(width, height, y.clone()) {
            let mut p = rqrr::PreparedImage::prepare(gray);
            for grid in p.detect_grids() {
                if let Ok((_, s)) = grid.decode() {
                    if !s.is_empty() { return Some(s); }
                }
            }
        }

        // Try 2: 90° CCW → portrait 720×1280
        let mut portrait = vec![0u8; w * h];
        for row in 0..h {
            for col in 0..w {
                portrait[col * h + (h - 1 - row)] = y[row * w + col];
            }
        }
        if let Some(gray) = image::GrayImage::from_raw(height as u32, width as u32, portrait) {
            let mut p = rqrr::PreparedImage::prepare(gray);
            for grid in p.detect_grids() {
                if let Ok((_, s)) = grid.decode() {
                    if !s.is_empty() { return Some(s); }
                }
            }
        }

        None
    }

    fn pick_back_camera(
        manager: *mut ffi::ACameraManager,
    ) -> Result<String, Box<dyn std::error::Error>> {
        let mut id_list: *mut ffi::ACameraIdList = std::ptr::null_mut();
        if unsafe { ffi::ACameraManager_getCameraIdList(manager, &mut id_list) }
            != ffi::camera_status_t::ACAMERA_OK || id_list.is_null()
        {
            return Err("getCameraIdList failed".into());
        }
        scopeguard::defer! { unsafe { ffi::ACameraManager_deleteCameraIdList(id_list); } }

        let num     = unsafe { (*id_list).numCameras } as usize;
        let ids_ptr = unsafe { (*id_list).cameraIds };
        let mut first: Option<String> = None;

        for i in 0..num {
            let id_ptr = unsafe { *ids_ptr.add(i) };
            if id_ptr.is_null() { continue; }
            let id = unsafe { std::ffi::CStr::from_ptr(id_ptr) }
                .to_string_lossy().into_owned();
            if first.is_none() { first = Some(id.clone()); }

            let id_c = std::ffi::CString::new(id.as_str())?;
            let mut meta: *mut ffi::ACameraMetadata = std::ptr::null_mut();
            if unsafe {
                ffi::ACameraManager_getCameraCharacteristics(
                    manager, id_c.as_ptr(), &mut meta,
                )
            } != ffi::camera_status_t::ACAMERA_OK || meta.is_null() { continue; }
            scopeguard::defer! { unsafe { ffi::ACameraMetadata_free(meta); } }

            let mut entry = ffi::ACameraMetadata_const_entry {
                tag: 0, type_: 0, count: 0,
                data: ffi::ACameraMetadata_const_entry__bindgen_ty_1 {
                    u8_: std::ptr::null(),
                },
            };
            if unsafe {
                ffi::ACameraMetadata_getConstEntry(meta, ACAMERA_LENS_FACING, &mut entry)
            } == ffi::camera_status_t::ACAMERA_OK
                && entry.count > 0
                && unsafe { *entry.data.u8_ } == ACAMERA_LENS_FACING_BACK
            {
                log::info!("[CAM] back camera: {id}");
                return Ok(id);
            }
        }
        log::warn!("[CAM] no back camera, using first");
        first.ok_or_else(|| "no cameras found".into())
    }
}

// ═════════════════════════════════════════════════════════════════════════════
// DESKTOP STUBS
// ═════════════════════════════════════════════════════════════════════════════
#[cfg(not(target_os = "android"))]
mod desktop_impl {
    use super::{store_qr_result, clear_qr_result};
    use crate::{AppState, Screen};

    pub fn init(_: ()) {}
    pub fn hide() {}
    pub fn poll_permission(_: &mut AppState) {}
    pub fn on_resumed(_: &mut AppState) {}
    pub fn take_permission_settings_popup_request() -> bool { false }
    pub fn open_app_settings() {}
    pub fn scan_button(state: &mut AppState) {
        clear_qr_result();
        state.set_screen(Screen::Scan);
        std::thread::spawn(|| {
            std::thread::sleep(std::time::Duration::from_secs(2));
            store_qr_result("DESKTOP_FAKE_QR_12345".to_string());
        });
    }
}

// ═════════════════════════════════════════════════════════════════════════════
// PUBLIC API — called from lib.rs and camera_widget.rs
// ═════════════════════════════════════════════════════════════════════════════
#[allow(unused_imports)]
use crate::AppState;

#[cfg(target_os = "android")]
pub fn init_android_app(app: android_activity::AndroidApp) {
    android_impl::init(app);
}

#[cfg(not(target_os = "android"))]
pub fn init_android_app(_app: ()) {
    // no-op on desktop
}

pub fn show_camera_overlay() {}   // no-op both platforms

pub fn hide_camera_overlay() {
    #[cfg(target_os = "android")]      { android_impl::hide(); }
    #[cfg(not(target_os = "android"))] { desktop_impl::hide(); }
}

pub fn poll_permission_granted(state: &mut AppState) {
    #[cfg(target_os = "android")]      { android_impl::poll_permission(state); }
    #[cfg(not(target_os = "android"))] { desktop_impl::poll_permission(state); }
}

pub fn on_android_resumed(state: &mut AppState) {
    #[cfg(target_os = "android")]      { android_impl::on_resumed(state); }
    #[cfg(not(target_os = "android"))] { desktop_impl::on_resumed(state); }
}

pub fn handle_scan_button(state: &mut AppState) {
    #[cfg(target_os = "android")]      { android_impl::scan_button(state); }
    #[cfg(not(target_os = "android"))] { desktop_impl::scan_button(state); }
}

pub fn take_permission_settings_popup_request() -> bool {
    #[cfg(target_os = "android")]      { android_impl::take_permission_settings_popup_request() }
    #[cfg(not(target_os = "android"))] { desktop_impl::take_permission_settings_popup_request() }
}

pub fn open_app_settings() {
    #[cfg(target_os = "android")]      { android_impl::open_app_settings(); }
    #[cfg(not(target_os = "android"))] { desktop_impl::open_app_settings(); }
}