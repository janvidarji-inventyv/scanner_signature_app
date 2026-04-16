use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ScanData {
    pub scan_id: String,
    pub scan_result: String,
    pub timestamp: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SignatureData {
    pub id: String,
    pub points: Vec<(f32, f32)>,
    pub width: u32,
    pub height: u32,
}

#[derive(Clone, Debug, PartialEq)]
pub enum AppScreen {
    AppLaunch,
    Scanner,
    SignatureInfo,
    SignaturePad,
    SignaturePreview,
    Success,
}

#[derive(Clone, Debug)]
pub struct AppState {
    pub current_screen: AppScreen,
    pub scan_data: Option<ScanData>,
    pub signature_data: Option<SignatureData>,
    pub temp_signature_points: Vec<(f32, f32)>,
    pub error_message: Option<String>,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            current_screen: AppScreen::AppLaunch,
            scan_data: None,
            signature_data: None,
            temp_signature_points: Vec::new(),
            error_message: None,
        }
    }
}