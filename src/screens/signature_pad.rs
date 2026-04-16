use crate::state::{AppScreen, AppState};
use xilem::prelude::*;

pub fn signature_pad_screen() -> impl View<AppState> {
    flex_column((
        // Header
        text("✍️ Draw Your Signature")
            .style(|s| {
                s.font_size(28.0)
                    .font_weight(Weight::BOLD)
                    .text_color(Color::rgb8(51, 51, 51))
            }),
        spacer(0.0, 15.0),

        // Instructions
        text("Sign below using your finger or stylus")
            .style(|s| {
                s.font_size(14.0)
                    .text_color(Color::rgb8(100, 100, 100))
            }),
        spacer(0.0, 20.0),

        // Status Info
        text(format!("Points: {}", state.signature_points_count()))
            .style(|s| {
                s.font_size(12.0)
                    .text_color(Color::rgb8(150, 150, 150))
            }),
        spacer(0.0, 15.0),

        // Canvas Container
        container(
            text("🖊️ Signature Canvas\n(Touch to draw)")
                .style(|s| {
                    s.font_size(14.0)
                        .text_color(Color::rgb8(150, 150, 150))
                        .line_height(1.8)
                })
        )
        .style(|s| {
            s.width(Length::Fill)
                .height(Length::Fixed(280.0))
                .background(Color::rgb8(255, 255, 255))
                .border(2.0, Color::rgb8(200, 200, 200))
                .border_radius(10.0)
                .padding(15.0)
        }),
        spacer(0.0, 25.0),

        // Action Buttons
        flex_row((
            button("✕ Cancel", |state: &mut AppState| {
                state.current_screen = AppScreen::SignatureInfo;
                state.clear_signature();
            })
            .style(|s| {
                s.width(Length::Fill)
                    .padding(12.0)
                    .background(Color::rgb8(220, 53, 69))
                    .border_radius(8.0)
                    .font_weight(Weight::BOLD)
            }),
            spacer(8.0, 0.0),
            button("🗑️ Clear", |state: &mut AppState| {
                state.clear_signature();
            })
            .style(|s| {
                s.width(Length::Fill)
                    .padding(12.0)
                    .background(Color::rgb8(255, 193, 7))
                    .border_radius(8.0)
                    .font_weight(Weight::BOLD)
            }),
            spacer(8.0, 0.0),
            button("✓ Accept →", |state: &mut AppState| {
                if state.is_signature_valid() {
                    state.save_current_signature();
                    state.current_screen = AppScreen::SignaturePreview;
                } else {
                    state.set_error(format!("Draw at least 5 points. Current: {}", state.signature_points_count()));
                }
            })
            .style(|s| {
                s.width(Length::Fill)
                    .padding(12.0)
                    .background(Color::rgb8(40, 167, 69))
                    .border_radius(8.0)
                    .font_weight(Weight::BOLD)
            }),
        ))
        .gap(6.0)
        .width(Length::Fill),
        spacer(0.0, 15.0),

        // Error Message
        if let Some(error) = &state.error_message {
            container(
                text(format!("⚠️ {}", error))
                    .style(|s| {
                        s.font_size(12.0)
                            .text_color(Color::rgb8(255, 255, 255))
                    })
            )
            .style(|s| {
                s.width(Length::Fill)
                    .padding(10.0)
                    .background(Color::rgb8(220, 53, 69))
                    .border_radius(6.0)
            })
            .boxed()
        } else {
            spacer(0.0, 0.0).boxed()
        },
    ))
    .padding(20.0)
    .gap(0.0)
    .width(Length::Fill)
    .height(Length::Fill)
    .center()
}