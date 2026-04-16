use jni::JNIEnv;
use jni::objects::{JClass, JString};
use jni::sys::jstring;
use std::sync::{Arc, Mutex};
use serde_json::json;
use crate::state::{AppState, AppScreen, ScanData};
use uuid::Uuid;

lazy_static::lazy_static! {
    static ref APP_STATE: Arc<Mutex<AppState>> = Arc::new(Mutex::new(AppState::default()));
}

/// Initialize the app and return status
#[no_mangle]
pub extern "C" fn Java_com_inventyv_scannersigantureapp_NativeBridge_initializeApp(
    mut env: JNIEnv,
    _class: JClass,
) -> jstring {
    let state = APP_STATE.lock().unwrap();
    let response = json!({
        "status": "initialized",
        "version": &state.app_version,
        "screen": format!("{:?}", state.current_screen),
        "timestamp": chrono::Local::now().to_rfc3339()
    });
    
    env.new_string(response.to_string())
        .expect("Couldn't create java string!")
        .into_raw()
}

/// Get current app state as JSON
#[no_mangle]
pub extern "C" fn Java_com_inventyv_scannersigantureapp_NativeBridge_getAppState(
    mut env: JNIEnv,
    _class: JClass,
) -> jstring {
    let state = APP_STATE.lock().unwrap();
    let state_json = serde_json::to_string(&*state).unwrap_or_else(|_| "{}".to_string());
    
    env.new_string(state_json)
        .expect("Couldn't create java string!")
        .into_raw()
}

/// Process scan result from Android camera
#[no_mangle]
pub extern "C" fn Java_com_inventyv_scannersigantureapp_NativeBridge_processScanResult(
    mut env: JNIEnv,
    _class: JClass,
    scan_result: JString,
) {
    let result_str: String = env
        .get_string(&scan_result)
        .expect("Couldn't get java string!")
        .to_string_lossy()
        .into_owned();

    let mut state = APP_STATE.lock().unwrap();
    state.scan_data = Some(ScanData {
        scan_id: Uuid::new_v4().to_string(),
        scan_result: result_str,
        scan_type: "barcode".to_string(),
        timestamp: chrono::Local::now().to_rfc3339(),
        confidence: 0.95,
    });
    state.current_screen = AppScreen::SignatureInfo;
}

/// Add a signature point with coordinates and pressure
#[no_mangle]
pub extern "C" fn Java_com_inventyv_scannersigantureapp_NativeBridge_addSignaturePoint(
    _env: JNIEnv,
    _class: JClass,
    x: f32,
    y: f32,
    pressure: f32,
) {
    let mut state = APP_STATE.lock().unwrap();
    state.add_signature_point(x, y, pressure);
}

/// Clear all temporary signature points
#[no_mangle]
pub extern "C" fn Java_com_inventyv_scannersigantureapp_NativeBridge_clearSignature(
    _env: JNIEnv,
    _class: JClass,
) {
    let mut state = APP_STATE.lock().unwrap();
    state.clear_signature();
}

/// Save the current signature
#[no_mangle]
pub extern "C" fn Java_com_inventyv_scannersigantureapp_NativeBridge_saveSignature(
    mut env: JNIEnv,
    _class: JClass,
) -> jstring {
    let mut state = APP_STATE.lock().unwrap();
    state.save_current_signature();

    let response = json!({
        "status": "success",
        "message": "Signature saved",
        "signature_id": state.signature_data.as_ref().map(|s| &s.id)
    });

    env.new_string(response.to_string())
        .expect("Couldn't create java string!")
        .into_raw()
}

/// Navigate to next screen
#[no_mangle]
pub extern "C" fn Java_com_inventyv_scannersigantureapp_NativeBridge_nextScreen(
    mut env: JNIEnv,
    _class: JClass,
) -> jstring {
    let mut state = APP_STATE.lock().unwrap();
    state.next_screen();
    
    let response = json!({
        "status": "success",
        "current_screen": format!("{:?}", state.current_screen)
    });

    env.new_string(response.to_string())
        .expect("Couldn't create java string!")
        .into_raw()
}

/// Navigate to previous screen
#[no_mangle]
pub extern "C" fn Java_com_inventyv_scannersigantureapp_NativeBridge_previousScreen(
    mut env: JNIEnv,
    _class: JClass,
) -> jstring {
    let mut state = APP_STATE.lock().unwrap();
    state.previous_screen();
    
    let response = json!({
        "status": "success",
        "current_screen": format!("{:?}", state.current_screen)
    });

    env.new_string(response.to_string())
        .expect("Couldn't create java string!")
        .into_raw()
}

/// Reset the entire app state
#[no_mangle]
pub extern "C" fn Java_com_inventyv_scannersigantureapp_NativeBridge_resetApp(
    mut env: JNIEnv,
    _class: JClass,
) -> jstring {
    let mut state = APP_STATE.lock().unwrap();
    state.reset();

    let response = json!({
        "status": "reset",
        "message": "App state reset to default"
    });

    env.new_string(response.to_string())
        .expect("Couldn't create java string!")
        .into_raw()
}

/// Get signature points count
#[no_mangle]
pub extern "C" fn Java_com_inventyv_scannersigantureapp_NativeBridge_getSignaturePointsCount(
    _env: JNIEnv,
    _class: JClass,
) -> i32 {
    let state = APP_STATE.lock().unwrap();
    state.signature_points_count() as i32
}

/// Check if current signature is valid
#[no_mangle]
pub extern "C" fn Java_com_inventyv_scannersigantureapp_NativeBridge_isSignatureValid(
    _env: JNIEnv,
    _class: JClass,
) -> bool {
    let state = APP_STATE.lock().unwrap();
    state.is_signature_valid()
}