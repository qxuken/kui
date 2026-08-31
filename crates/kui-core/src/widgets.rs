//! Opinionated helpers composed purely from primitives — the pattern custom
//! widgets should follow (composition over traits, state by key), which is
//! what keeps them reachable from scripting frontends.

use crate::color::Color;
use crate::spec::{NodeSpec, TextStyle};
use crate::ui::Ui;
use crate::value::Value;

pub fn label(ui: &mut Ui<'_>, text: &str) {
    ui.text(text, TextStyle::default());
}

pub fn button(ui: &mut Ui<'_>, text: &str, payload: impl Into<Value>) {
    let key = ui.child_key(text);
    let bg = if ui.is_pressed(key) {
        Color::rgb8(0x2f, 0x54, 0xc4)
    } else if ui.is_hovered(key) {
        Color::rgb8(0x47, 0x6c, 0xe0)
    } else {
        Color::rgb8(0x3b, 0x5b, 0xd4)
    };
    ui.with_keyed(
        text,
        NodeSpec::row().pad_xy(14.0, 8.0).bg(bg).radius(6.0).center().on_click(payload.into()),
        |ui| ui.text(text, TextStyle::new(15.0).color(Color::WHITE)),
    );
}
