use crate::state::{AppScreen, AppState, ScanData};
use xilem::prelude::*;
use uuid::Uuid;

pub fn scanner_screen() -> impl View<AppState> {
    flex_column((
        // Header
        text("🔍 Scanning Document...")
            .style(|s| {
                s.font_size(28.0)
                    .font_weight(Weight::BOLD)
                    .text_color(Color::rgb8(51, 51, 51))
            }),
        spacer(0.0, 30.0),

        // Scanning Info
        text("Opening camera to scan document...")
            .style(|s| {
                s.font_size(16.0)
                    .text_color(Color::rgb8(100, 100, 100))
            }),
        spacer(0.0, 20.0),

        // Scanner Area (Placeholder)
        container(
            text("📷 Camera Stream\n(Powered by Android)")
                .style(|s| s.font_size(16.0).text_color(Color::rgb8(150, 150, 150)))
        )
        .style(|s| {
            s.width(Length::Fill)
                .height(Length::Fixed(300.0))
                .background(Color::rgb8(240, 240, 240))
                .border_radius(10.0)
                .padding(20.0)
        }),
        spacer(0.0, 30.0),

        // Status Message
        text("Processing barcode/QR code...")
            .style(|s| {
                s.font_size(14.0)
                    .text_color(Color::rgb8(98, 0, 238))
                    .font_weight(Weight::BOLD)
            }),
        spacer(0.0, 40.0),

        // Continue Button
        flex_row((
            button("✓ Use Scan Result", |state: &mut AppState| {
                // Create scan data
                state.scan_data = Some(ScanData {
                    scan_id: Uuid::new_v4().to_string(),
                    scan_result: "DOC-2026-04-16-001".to_string(),
                    scan_type: "barcode".to_string(),
                    timestamp: chrono::Local::now().to_rfc3339(),
                    confidence: 0.95,
                });
                state.current_screen = AppScreen::SignatureInfo;
                state.is_processing = false;
            })
            .style(|s| {
                s.width(Length::Fill)
                    .padding(14.0)
                    .background(Color::rgb8(40, 167, 69))
                    .border_radius(8.0)
                    .font_weight(Weight::BOLD)
            }),
            spacer(10.0, 0.0),
            button("✕ Cancel", |state: &mut AppState| {
                state.previous_screen();
            })
            .style(|s| {
                s.width(Length::Fill)
                    .padding(14.0)
                    .background(Color::rgb8(220, 53, 69))
                    .border_radius(8.0)
            }),
        ))
        .gap(10.0)
        .width(Length::Fill),
    ))
    .padding(20.0)
    .gap(0.0)
    .width(Length::Fill)
    .height(Length::Fill)
    .center()
}