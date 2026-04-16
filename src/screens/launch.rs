use crate::state::AppState;
use xilem::prelude::*;

pub fn launch_screen() -> impl View<AppState> {
    flex_column((
        // Logo/Header
        text("🎯 App Starting")
            .style(|s| {
                s.font_size(40.0)
                    .font_weight(Weight::BOLD)
                    .text_color(Color::rgb8(98, 0, 238))
            }),
        spacer(0.0, 20.0),

        // Title
        text("Scanner & Signature App")
            .style(|s| {
                s.font_size(28.0)
                    .font_weight(Weight::BOLD)
                    .text_color(Color::rgb8(51, 51, 51))
            }),
        spacer(0.0, 15.0),

        // App Version
        text("Version 1.0.0")
            .style(|s| {
                s.font_size(14.0)
                    .text_color(Color::rgb8(128, 128, 128))
            }),
        spacer(0.0, 40.0),

        // Description
        text("Complete document scanning and digital signature solution")
            .style(|s| {
                s.font_size(16.0)
                    .text_color(Color::rgb8(80, 80, 80))
                    .line_height(1.6)
            }),
        spacer(0.0, 60.0),

        // Main Scan Button
        button("📸 Start Scanning", |state: &mut AppState| {
            state.current_screen = crate::state::AppScreen::Scanner;
            state.is_processing = true;
        })
        .style(|s| {
            s.width(Length::Fill)
                .padding(16.0)
                .background(Color::rgb8(98, 0, 238))
                .border_radius(10.0)
                .font_size(16.0)
        }),
        spacer(0.0, 20.0),

        // Info Text
        text("📋 Process: Scan document → Sign → Complete")
            .style(|s| {
                s.font_size(12.0)
                    .text_color(Color::rgb8(150, 150, 150))
                    .line_height(1.5)
            }),
    ))
    .padding(20.0)
    .gap(0.0)
    .width(Length::Fill)
    .height(Length::Fill)
    .center()
}