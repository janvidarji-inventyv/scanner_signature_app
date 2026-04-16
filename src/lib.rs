mod android_bridge;
mod state;
mod ui_components;

use state::{AppScreen, AppState, ScanData, SignatureData};
use uuid::Uuid;
use xilem::prelude::*;

pub fn app_view() -> impl View<AppState> {
    flex_column((
        choose_screen(),
    ))
}

fn choose_screen() -> impl View<AppState> {
    match_view(
        |(state, _): &(AppState, _)| state.current_screen.clone(),
        |screen| {
            match screen {
                AppScreen::AppLaunch => launch_screen().boxed(),
                AppScreen::Scanner => scanner_screen().boxed(),
                AppScreen::SignatureInfo => signature_info_screen().boxed(),
                AppScreen::SignaturePad => signature_pad_screen().boxed(),
                AppScreen::SignaturePreview => preview_screen().boxed(),
                AppScreen::Success => success_screen().boxed(),
            }
        },
    )
}

fn launch_screen() -> impl View<AppState> {
    flex_column((
        label("📱 Document Scanner & Signature"),
        spacer(0.0, 10.0),
        label("v1.0.0"),
        spacer(0.0, 20.0),
        label("Tap below to scan a document"),
        spacer(0.0, 30.0),
        button("📸 Start Scan", |state: &mut AppState| {
            state.current_screen = AppScreen::Scanner;
            state.scan_data = Some(ScanData {
                scan_id: Uuid::new_v4().to_string(),
                scan_result: "Document scanned successfully".to_string(),
                timestamp: chrono::Local::now().to_rfc3339(),
            });
        }),
    ))
}

fn scanner_screen() -> impl View<AppState> {
    flex_column((
        label("🔍 Scanning Document..."),
        spacer(0.0, 150.0),
        label("Camera preview would appear here"),
        spacer(0.0, 150.0),
        button("Continue to Signature", |state: &mut AppState| {
            state.current_screen = AppScreen::SignatureInfo;
        }),
        button("Cancel", |state: &mut AppState| {
            state.current_screen = AppScreen::AppLaunch;
        }),
    ))
}

fn signature_info_screen() -> impl View<AppState> {
    flex_column((
        label("✍️ Signature Required"),
        spacer(0.0, 10.0),
        label("Please provide your signature to complete the process"),
        spacer(0.0, 30.0),
        button("Draw Signature", |state: &mut AppState| {
            state.current_screen = AppScreen::SignaturePad;
            state.temp_signature_points.clear();
        }),
        button("Back", |state: &mut AppState| {
            state.current_screen = AppScreen::AppLaunch;
        }),
    ))
}

fn signature_pad_screen() -> impl View<AppState> {
    flex_column((
        label("✍️ Draw Your Signature"),
        spacer(0.0, 200.0),
        label("Canvas area - signature drawing happens here"),
        spacer(0.0, 100.0),
        flex_row((
            button("Cancel", |state: &mut AppState| {
                state.current_screen = AppScreen::SignatureInfo;
                state.temp_signature_points.clear();
            }),
            spacer(50.0, 0.0),
            button("Clear", |state: &mut AppState| {
                state.temp_signature_points.clear();
            }),
            spacer(50.0, 0.0),
            button("Accept", |state: &mut AppState| {
                if !state.temp_signature_points.is_empty() {
                    state.signature_data = Some(SignatureData {
                        id: Uuid::new_v4().to_string(),
                        points: state.temp_signature_points.clone(),
                        width: 400,
                        height: 200,
                    });
                    state.current_screen = AppScreen::SignaturePreview;
                }
            }),
        )),
    ))
}

fn preview_screen() -> impl View<AppState> {
    flex_column((
        label("Preview Signature"),
        spacer(0.0, 150.0),
        label("Signature preview would appear here"),
        spacer(0.0, 100.0),
        flex_row((
            button("Edit", |state: &mut AppState| {
                state.current_screen = AppScreen::SignaturePad;
            }),
            spacer(50.0, 0.0),
            button("Save", |state: &mut AppState| {
                state.current_screen = AppScreen::Success;
            }),
        )),
    ))
}

fn success_screen() -> impl View<AppState> {
    flex_column((
        label("✅ Success!"),
        spacer(0.0, 20.0),
        label("Document and signature saved successfully"),
        spacer(0.0, 30.0),
        button("Home", |state: &mut AppState| {
            *state = AppState::default();
        }),
    ))
}