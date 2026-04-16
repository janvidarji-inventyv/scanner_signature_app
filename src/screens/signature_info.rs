use crate::state::{AppScreen, AppState};
use xilem::prelude::*;

/// Screen that shows signature requirements and scan information
pub fn signature_info_screen() -> impl View<AppState> {
    flex_column((
        // Header
        text("✍️ Signature Required")
            .style(|s| {
                s.font_size(28.0)
                    .font_weight(Weight::BOLD)
                    .text_color(Color::rgb8(51, 51, 51))
            }),
        spacer(0.0, 20.0),

        // Info Box
        container(
            text("Your digital signature is required to verify and complete the scanned document.")
                .style(|s| {
                    s.font_size(14.0)
                        .text_color(Color::rgb8(80, 80, 80))
                        .line_height(1.6)
                })
        )
        .style(|s| {
            s.width(Length::Fill)
                .padding(15.0)
                .background(Color::rgb8(220, 237, 255))
                .border_radius(8.0)
        }),
        spacer(0.0, 30.0),

        // Scan Details
        if let Some(scan) = &state.scan_data {
            flex_column((
                text("📋 Scan Details:")
                    .style(|s| {
                        s.font_size(14.0)
                            .font_weight(Weight::BOLD)
                            .text_color(Color::rgb8(51, 51, 51))
                    }),
                spacer(0.0, 12.0),
                flex_column((
                    flex_row((
                        text("ID: ")
                            .style(|s| s.font_weight(Weight::BOLD).width(Length::Fixed(60.0))),
                        text(&scan.scan_id)
                            .style(|s| s.text_color(Color::rgb8(100, 100, 100))),
                    ))
                    .gap(8.0)
                    .width(Length::Fill),
                    spacer(0.0, 6.0),
                    flex_row((
                        text("Type: ")
                            .style(|s| s.font_weight(Weight::BOLD).width(Length::Fixed(60.0))),
                        text(&scan.scan_type)
                            .style(|s| s.text_color(Color::rgb8(100, 100, 100))),
                    ))
                    .gap(8.0)
                    .width(Length::Fill),
                    spacer(0.0, 6.0),
                    flex_row((
                        text("Result: ")
                            .style(|s| s.font_weight(Weight::BOLD).width(Length::Fixed(60.0))),
                        text(&scan.scan_result)
                            .style(|s| s.text_color(Color::rgb8(100, 100, 100))),
                    ))
                    .gap(8.0)
                    .width(Length::Fill),
                ))
                .width(Length::Fill),
            ))
            .width(Length::Fill)
            .boxed()
        } else {
            spacer(0.0, 0.0).boxed()
        },
        spacer(0.0, 40.0),

        // Action Buttons
        flex_row((
            button("← Back", |state: &mut AppState| {
                state.previous_screen();
            })
            .style(|s| {
                s.width(Length::Fill)
                    .padding(12.0)
                    .background(Color::rgb8(220, 53, 69))
                    .border_radius(8.0)
            }),
            spacer(10.0, 0.0),
            button("Draw Signature →", |state: &mut AppState| {
                state.current_screen = AppScreen::SignaturePad;
            })
            .style(|s| {
                s.width(Length::Fill)
                    .padding(12.0)
                    .background(Color::rgb8(40, 167, 69))
                    .border_radius(8.0)
                    .font_weight(Weight::BOLD)
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
