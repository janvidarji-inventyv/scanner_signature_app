pub mod state;
pub mod android_bridge;

// Screens are no longer needed for Android - UI handled by Android
// pub mod screens;

pub use state::{AppState, AppScreen, ScanData, SignatureData, Point};