//! The `text` element, and the paragraph it becomes with spans: styled
//! runs shaped and wrapped as one flow — bold, italic and coloured runs
//! and emoji share lines and wrap mid-sentence — plus what a single run
//! can ask for on its own: a font family, decorations (underline,
//! strikethrough, a span background), a line height, `nowrap`,
//! `max_lines` with an `ellipsis`. Shrink the window and every paragraph
//! rewraps live.
//!
//! Nothing here is selectable: that is `features/selection`, which puts
//! a scope over exactly this kind of text.
//!
//! Run: cargo run -p kui --example text

use kui::{Align, App, FontFamily, NodeSpec, Sizing, Span, TextStyle, Theme, Ui};
use kui_devtools::Example;

struct Text;

/// A card in the theme's own surface and edge.
fn card(t: &Theme, title: &str, ui: &mut Ui<'_>, body: impl FnOnce(&mut Ui<'_>)) {
    ui.with(
        NodeSpec::column()
            .width(Sizing::Grow(1.0))
            .max_width(560.0)
            .pad(24.0)
            .gap(12.0)
            .bg(t.surface)
            .radius(12.0)
            .border(1.0, t.border),
        |ui| {
            ui.text(title, TextStyle::new(12.0).color(t.muted));
            body(ui);
        },
    );
}

impl App for Text {
    fn view(&mut self, ui: &mut Ui<'_>) {
        let t = ui.theme();
        // Scrollable page: when the window is shorter than the content,
        // the wheel scrolls it.
        ui.with(
            NodeSpec::column()
                .fill()
                .cross_align(Align::Center)
                .pad(24.0)
                .gap(16.0)
                .scroll_y(),
            |ui| {
                card(
                    &t,
                    "SPANS · one paragraph, many styles, one shaping flow",
                    ui,
                    |ui| {
                        ui.rich_text(
                            &[
                                Span::new("Spans are "),
                                Span::new("plain data").bold().color(t.accent),
                                Span::new(", so every frontend — Rust, "),
                                Span::new("Lua").color(t.success),
                                Span::new(
                                    ", or C — can build them. The whole paragraph is shaped ",
                                ),
                                Span::new("together").italic(),
                                Span::new(
                                    ", which means wrapping crosses style boundaries correctly \
                                 instead of breaking at every run: ",
                                ),
                                Span::new("bold").bold(),
                                Span::new(" and "),
                                Span::new("italic").italic(),
                                Span::new(" and "),
                                Span::new("bold-italic").bold().italic().color(t.warning),
                                Span::new(
                                    " all sit on the same baselines. Emoji ride along via the \
                                 color-glyph atlas path: 🦀🔥✨",
                                ),
                            ],
                            TextStyle::new(16.0).line_height(26.0),
                        );
                        ui.rich_text(
                            &[
                                Span::new(
                                    "Per-span colour overrides the node colour at glyph level — ",
                                ),
                                Span::new("red").color(t.danger),
                                Span::new(", "),
                                Span::new("green").color(t.success),
                                Span::new(", "),
                                Span::new("blue").color(t.accent),
                                Span::new(" — while unstyled runs inherit it."),
                            ],
                            TextStyle::new(16.0).line_height(26.0).color(t.muted),
                        );
                    },
                );

                card(&t, "DECORATIONS · paint, not layout", ui, |ui| {
                    ui.rich_text(
                        &[
                            Span::new("An "),
                            Span::new("underline").underline(),
                            Span::new(", a "),
                            Span::new("strikethrough").strikethrough(),
                            Span::new(", a "),
                            Span::new("highlight").bg(t.accent_soft),
                            Span::new(" behind a span, and all three at once: "),
                            Span::new("marked")
                                .underline()
                                .strikethrough()
                                .bg(t.warning.with_alpha(0.25)),
                            Span::new(". A decorated span measures like a plain one."),
                        ],
                        TextStyle::new(16.0).line_height(26.0),
                    );
                });

                card(&t, "FAMILIES · the same run in each", ui, |ui| {
                    for (name, family) in [
                        ("sans", FontFamily::Sans),
                        ("serif", FontFamily::Serif),
                        ("mono", FontFamily::Mono),
                    ] {
                        // The row grows so the run can wrap at the card's
                        // edge; in a fit row a growing column has no width.
                        ui.with(
                            NodeSpec::row()
                                .width(Sizing::Grow(1.0))
                                .gap(12.0)
                                .cross_align(Align::Center),
                            |ui| {
                                ui.with(NodeSpec::row().width(Sizing::Fixed(44.0)), |ui| {
                                    ui.text(name, TextStyle::new(11.0).color(t.muted));
                                });
                                ui.with(NodeSpec::column().width(Sizing::Grow(1.0)), |ui| {
                                    ui.text(
                                        "The quick brown fox jumps over the lazy dog 0123456789",
                                        TextStyle::new(15.0).family(family),
                                    );
                                });
                            },
                        );
                    }
                });

                card(
                    &t,
                    "WRAP · nowrap, and max_lines with an ellipsis",
                    ui,
                    |ui| {
                        ui.text(
                            "A run that may not wrap keeps going past its box, and the box \
                         clips it or does not — nowrap is a promise about lines, not width.",
                            TextStyle::new(14.0).nowrap(),
                        );
                        ui.text(
                            "Two lines at most, and an ellipsis where the second one ends: a \
                         card title that has to stay a title however long the string behind \
                         it grows, which is what a list of files or messages needs from a \
                         label — the layout stays where it was and the text yields.",
                            TextStyle::new(14.0)
                                .line_height(20.0)
                                .max_lines(2)
                                .ellipsis(),
                        );
                        ui.text(
                            "Line height is the run's, too: this paragraph asks for 30 px per \
                         line and gets the air that comes with it, whatever the size of \
                         the glyphs on the line.",
                            TextStyle::new(14.0).line_height(30.0).color(t.muted),
                        );
                    },
                );
            },
        );
    }
}

impl Example for Text {}

kui_devtools::main!(Text);
