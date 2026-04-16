use jni::JNIEnv;
use jni::objects::{JClass, JString};
use jni::sys::jstring;
use std::ffi::CStr;

#[no_mangle]
pub extern "C" fn Java_com_example_scanner_1signature_NativeBridge_initApp(
    env: JNIEnv,
    _class: JClass,
) -> jstring {
    let response = "App initialized with Xilem";
    env.new_string(response)
        .expect("Couldn't create java string!")
        .into_raw()
}

#[no_mangle]
pub extern "C" fn Java_com_example_scanner_1signature_NativeBridge_startScanner(
    env: JNIEnv,
    _class: JClass,
) {
    // Call scanner intent from Kotlin
    println!("Scanner initiated from Rust");
}

#[no_mangle]
pub extern "C" fn Java_com_example_scanner_1signature_NativeBridge_processScanResult(
    env: JNIEnv,
    _class: JClass,
    scan_result: JString,
) {
    let result_str: String = env
        .get_string(&scan_result)
        .expect("Couldn't get java string!")
        .into();
    println!("Scan result received: {}", result_str);
}

#[no_mangle]
pub extern "C" fn Java_com_example_scanner_1signature_NativeBridge_saveSignature(
    env: JNIEnv,
    _class: JClass,
    signature_json: JString,
) {
    let sig_str: String = env
        .get_string(&signature_json)
        .expect("Couldn't get java string!")
        .into();
    println!("Signature saved: {}", sig_str);
}