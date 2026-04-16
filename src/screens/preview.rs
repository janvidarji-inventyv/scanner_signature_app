use crate::state::{AppScreen, AppState};
use xilem::prelude::*;

pub fn preview_screen() -> impl View<AppState> {
    flex_column((
        // Header
        text("👁️ Signature Preview")
            .style(|s| {
                s.font_size(28.0)
                    .font_weight(Weight::BOLD)
                    .text_color(Color::rgb8(51, 51, 51))
            }),
        spacer(0.0, 20.0),

        // Preview Area
        container(
            text("✍️ Signature Canvas\n(Preview)")
                .style(|s| {
                    s.font_size(14.0)
                        .text_color(Color::rgb8(150, 150, 150))
                        .line_height(1.8)
                })
        )
        .style(|s| {
            s.width(Length::Fill)
                .height(Length::Fixed(250.0))
                .background(Color::rgb8(245, 245, 245))
                .border(2.0, Color::rgb8(200, 200, 200))
                .border_radius(10.0)
                .padding(15.0)
        }),
        spacer(0.0, 30.0),

        // Signature Info
        if let Some(sig) = &state.signature_data {
            flex_column((
                text("📊 Signature Details:")
                    .style(|s| {
                        s.font_size(14.0)
                            .font_weight(Weight::BOLD)
                            .text_color(Color::rgb8(51, 51, 51))
                    }),
                spacer(0.0, 10.0),
                flex_column((
                    flex_row((
                        text("ID: ")
                            .style(|s| s.font_weight(Weight::BOLD).width(Length::Fixed(80.0))),
                        text(&sig.id[..8])
                            .style(|s| s.text_color(Color::rgb8(100, 100, 100))),
                    ))
                    .gap(8.0)
                    .width(Length::Fill),
                    spacer(0.0, 6.0),
                    flex_row((
                        text("Points: ")
                            .style(|s| s.font_weight(Weight::BOLD).width(Length::Fixed(80.0))),
                        text(format!("{}", sig.points.len()))
                            .style(|s| s.text_color(Color::rgb8(40, 167, 69)).font_weight(Weight::BOLD)),
                    ))
                    .gap(8.0)
                    .width(Length::Fill),
                    spacer(0.0, 6.0),
                    flex_row((
                        text("Size: ")
                            .style(|s| s.font_weight(Weight::BOLD).width(Length::Fixed(80.0))),
                        text(format!("{}x{}", sig.width, sig.height))
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
            button("✏️ Edit", |state: &mut AppState| {
                state.go_back_to_edit();
            })
            .style(|s| {
                s.width(Length::Fill)
                    .padding(12.0)
                    .background(Color::rgb8(255, 193, 7))
                    .border_radius(8.0)
                    .font_weight(Weight::BOLD)
            }),
            spacer(10.0, 0.0),
            button("💾 Save & Complete →", |state: &mut AppState| {
                state.current_screen = AppScreen::Success;
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