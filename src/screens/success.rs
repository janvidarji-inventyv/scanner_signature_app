use crate::state::AppState;
use xilem::prelude::*;

pub fn success_screen() -> impl View<AppState> {
    flex_column((
        spacer(0.0, 40.0),

        // Success Icon
        text("✅")
            .style(|s| {
                s.font_size(80.0)
                    .text_color(Color::rgb8(40, 167, 69))
            }),
        spacer(0.0, 25.0),

        // Title
        text("Successfully Completed!")
            .style(|s| {
                s.font_size(32.0)
                    .font_weight(Weight::BOLD)
                    .text_color(Color::rgb8(40, 167, 69))
            }),
        spacer(0.0, 15.0),

        // Message
        text("Document scanned and signature saved successfully!")
            .style(|s| {
                s.font_size(16.0)
                    .text_color(Color::rgb8(100, 100, 100))
                    .line_height(1.6)
            }),
        spacer(0.0, 35.0),

        // Summary Box
        container(
            flex_column((
                text("📋 Transaction Summary")
                    .style(|s| {
                        s.font_size(14.0)
                            .font_weight(Weight::BOLD)
                            .text_color(Color::rgb8(51, 51, 51))
                    }),
                spacer(0.0, 12.0),
                // Scan Details
                if let Some(scan) = &state.scan_data {
                    flex_column((
                        flex_row((
                            text("Scan ID: ")
                                .style(|s| s.font_weight(Weight::BOLD).width(Length::Fixed(85.0))),
                            text(&scan.scan_id[..12]),
                        ))
                        .gap(8.0)
                        .width(Length::Fill),
                        spacer(0.0, 6.0),
                        flex_row((
                            text("Type: ")
                                .style(|s| s.font_weight(Weight::BOLD).width(Length::Fixed(85.0))),
                            text(&scan.scan_type),
                        ))
                        .gap(8.0)
                        .width(Length::Fill),
                    ))
                    .width(Length::Fill)
                    .boxed()
                } else {
                    spacer(0.0, 0.0).boxed()
                },
                spacer(0.0, 10.0),
                // Signature Details
                if let Some(sig) = &state.signature_data {
                    flex_column((
                        flex_row((
                            text("Signature ID: ")
                                .style(|s| s.font_weight(Weight::BOLD).width(Length::Fixed(85.0))),
                            text(&sig.id[..12]),
                        ))
                        .gap(8.0)
                        .width(Length::Fill),
                        spacer(0.0, 6.0),
                        flex_row((
                            text("Points: ")
                                .style(|s| s.font_weight(Weight::BOLD).width(Length::Fixed(85.0))),
                            text(format!("{}", sig.points.len()))
                                .style(|s| s.text_color(Color::rgb8(40, 167, 69))),
                        ))
                        .gap(8.0)
                        .width(Length::Fill),
                    ))
                    .width(Length::Fill)
                    .boxed()
                } else {
                    spacer(0.0, 0.0).boxed()
                },
            ))
            .width(Length::Fill)
        )
        .style(|s| {
            s.width(Length::Fill)
                .padding(15.0)
                .background(Color::rgb8(240, 248, 255))
                .border_radius(8.0)
        }),
        spacer(0.0, 35.0),

        // Button
        button("🏠 Back to Home", |state: &mut AppState| {
            state.reset();
        })
        .style(|s| {
            s.width(Length::Fill)
                .padding(16.0)
                .background(Color::rgb8(98, 0, 238))
                .border_radius(10.0)
                .font_size(16.0)
                .font_weight(Weight::BOLD)
        }),
        spacer(0.0, 15.0),
    ))
    .padding(20.0)
    .gap(0.0)
    .width(Length::Fill)
    .height(Length::Fill)
    .center()
}