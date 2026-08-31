//! Rich text: styled spans shaped and wrapped as one paragraph flow —
//! bold/italic/colored runs and emoji share lines and wrap mid-sentence.
//!
//! Run: cargo run -p kui --example rich_text

use kui::{App, Align, Color, NodeSpec, Sizing, Span, TextStyle, Ui};

const MUTED: Color = Color { r: 0.54, g: 0.56, b: 0.64, a: 1.0 };
const ACCENT: Color = Color { r: 0.42, g: 0.62, b: 1.0, a: 1.0 };
const GREEN: Color = Color { r: 0.45, g: 0.85, b: 0.55, a: 1.0 };
const AMBER: Color = Color { r: 0.95, g: 0.72, b: 0.35, a: 1.0 };

struct RichText;

impl App for RichText {
    fn view(&mut self, ui: &mut Ui<'_>) {
        // Scrollable root: when the window is shorter than the content, the
        // wheel/trackpad scrolls it (with a scrollbar indicator).
        ui.configure_root(NodeSpec::column().fill().cross_align(Align::Center).pad(24.0).scroll_y());

        ui.with(
            NodeSpec::column()
                // Track the window, but cap at a comfortable reading width —
                // shrink the window and the paragraphs rewrap live.
                .width(Sizing::Grow(1.0))
                .max_width(560.0)
                .pad(36.0)
                .gap(18.0)
                .bg(Color::rgb8(0x16, 0x18, 0x20))
                .radius(12.0)
                .border(1.0, Color::rgb8(0x2a, 0x2d, 0x3a)),
            |ui| {
                ui.text("Rich text in kui", TextStyle::new(28.0));
                ui.text(
                    "one paragraph, many styles, single shaping flow",
                    TextStyle::new(13.0).color(MUTED),
                );

                ui.rich_text(
                    &[
                        Span::new("Spans are "),
                        Span::new("plain data").bold().color(ACCENT),
                        Span::new(", so every frontend — Rust, "),
                        Span::new("Lua").color(GREEN),
                        Span::new(", or C — can build them. The whole paragraph is shaped "),
                        Span::new("together").italic(),
                        Span::new(
                            ", which means wrapping crosses style boundaries correctly \
                             instead of breaking at every run: ",
                        ),
                        Span::new("bold").bold(),
                        Span::new(" and "),
                        Span::new("italic").italic(),
                        Span::new(" and "),
                        Span::new("bold-italic").bold().italic().color(AMBER),
                        Span::new(" all sit on the same baselines. Emoji ride along via the \
                                   color-glyph atlas path: 🦀🔥✨"),
                    ],
                    TextStyle::new(16.0).line_height(26.0),
                );

                ui.rich_text(
                    &[
                        Span::new("Per-span color overrides the node color at glyph level — "),
                        Span::new("red").color(Color::rgb8(0xe8, 0x5d, 0x5d)),
                        Span::new(", "),
                        Span::new("green").color(GREEN),
                        Span::new(", "),
                        Span::new("blue").color(ACCENT),
                        Span::new(" — while unstyled runs inherit it."),
                    ],
                    TextStyle::new(16.0).line_height(26.0).color(MUTED),
                );

                ui.with(NodeSpec::row().width(Sizing::Grow(1.0)).main_align(Align::End), |ui| {
                    ui.text("resize the window to watch it rewrap", TextStyle::new(12.0).color(MUTED));
                });
            },
        );
    }
}

fn main() {
    kui::run("kui — rich text", RichText, vec![]).unwrap();
}
