use serde::{Deserialize, Serialize};
use std::sync::Arc;
use uuid::Uuid;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum AppScreen {
    AppLaunch,
    Scanner,
    SignatureInfo,
    SignaturePad,
    SignaturePreview,
    Success,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Point {
    pub x: f32,
    pub y: f32,
    pub pressure: f32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ScanData {
    pub scan_id: String,
    pub scan_result: String,
    pub scan_type: String, // barcode, qr_code, document
    pub timestamp: String,
    pub confidence: f32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SignatureData {
    pub id: String,
    pub points: Vec<Point>,
    pub width: u32,
    pub height: u32,
    pub created_at: String,
    pub signature_image: Option<Vec<u8>>, // PNG encoded
}

#[derive(Clone, Debug, Serialize)]
pub struct AppState {
    pub current_screen: AppScreen,
    pub scan_data: Option<ScanData>,
    pub signature_data: Option<SignatureData>,
    pub temp_signature_points: Vec<Point>,
    pub error_message: Option<String>,
    pub is_processing: bool,
    pub app_version: String,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            current_screen: AppScreen::AppLaunch,
            scan_data: None,
            signature_data: None,
            temp_signature_points: Vec::new(),
            error_message: None,
            is_processing: false,
            app_version: "1.0.0".to_string(),
        }
    }
}

impl AppState {
    pub fn reset(&mut self) {
        *self = Self::default();
    }

    pub fn next_screen(&mut self) {
        self.current_screen = match self.current_screen {
            AppScreen::AppLaunch => AppScreen::Scanner,
            AppScreen::Scanner => AppScreen::SignatureInfo,
            AppScreen::SignatureInfo => AppScreen::SignaturePad,
            AppScreen::SignaturePad => AppScreen::SignaturePreview,
            AppScreen::SignaturePreview => AppScreen::Success,
            AppScreen::Success => AppScreen::AppLaunch,
        };
        self.error_message = None;
    }

    pub fn previous_screen(&mut self) {
        self.current_screen = match self.current_screen {
            AppScreen::AppLaunch => AppScreen::AppLaunch,
            AppScreen::Scanner => AppScreen::AppLaunch,
            AppScreen::SignatureInfo => AppScreen::Scanner,
            AppScreen::SignaturePad => AppScreen::SignatureInfo,
            AppScreen::SignaturePreview => AppScreen::SignaturePad,
            AppScreen::Success => AppScreen::AppLaunch,
        };
        self.error_message = None;
    }

    pub fn add_signature_point(&mut self, x: f32, y: f32, pressure: f32) {
        self.temp_signature_points.push(Point { x, y, pressure });
    }

    pub fn clear_signature(&mut self) {
        self.temp_signature_points.clear();
    }

    pub fn save_current_signature(&mut self) {
        if !self.temp_signature_points.is_empty() {
            let sig_data = SignatureData {
                id: Uuid::new_v4().to_string(),
                points: self.temp_signature_points.clone(),
                width: 400,
                height: 250,
                created_at: chrono::Local::now().to_rfc3339(),
                signature_image: None,
            };
            self.signature_data = Some(sig_data);
            self.temp_signature_points.clear();
        }
    }

    pub fn go_back_to_edit(&mut self) {
        if let Some(ref sig) = self.signature_data {
            self.temp_signature_points = sig.points.clone();
        }
        self.current_screen = AppScreen::SignaturePad;
    }

    pub fn set_error(&mut self, message: String) {
        self.error_message = Some(message);
    }

    pub fn is_signature_valid(&self) -> bool {
        self.temp_signature_points.len() > 5
    }

    pub fn signature_points_count(&self) -> usize {
        self.temp_signature_points.len()
    }
}