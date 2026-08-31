//! Host app + Lua extension sharing one frame. The host owns the left side;
//! panel.lua owns the right panel, keeps its own state, and gets its clicks
//! routed back by origin — the host never inspects the script's UI or events.
//!
//! Run: cargo run -p kui-lua --example lua_panel

use kui::widgets;
use kui::{App, Align, Color, NodeSpec, Sizing, TextStyle, Ui, UiEvent, Value};
use kui_lua::LuaExtension;

#[derive(Default)]
struct Host {
    clicks: i64,
}

impl App for Host {
    fn view(&mut self, ui: &mut Ui<'_>) {
        // Extensions append after the host, so the root row places the Lua
        // panel to the right of this column.
        ui.configure_root(NodeSpec::row().fill().pad(16.0).gap(16.0));

        ui.with(
            NodeSpec::column()
                .width(Sizing::Grow(1.0))
                .height(Sizing::Grow(1.0))
                .pad(24.0)
                .gap(16.0)
                .bg(Color::rgb8(0x16, 0x18, 0x20))
                .radius(10.0)
                .cross_align(Align::Center)
                .main_align(Align::Center),
            |ui| {
                ui.text("host (Rust)", TextStyle::new(12.0).color(Color::rgb8(0x8a, 0x8f, 0xa3)));
                ui.text(&format!("{} clicks", self.clicks), TextStyle::new(40.0));
                widgets::button(ui, "click me", Value::map([("kind", "click".into())]));
            },
        );
    }

    fn on_event(&mut self, ev: UiEvent) {
        if ev.payload.get("kind").and_then(Value::as_str) == Some("click") {
            self.clicks += 1;
        }
    }
}

fn main() {
    let script = concat!(env!("CARGO_MANIFEST_DIR"), "/examples/panel.lua");
    let ext = LuaExtension::from_file(script).expect("load panel.lua");
    kui::run("kui — lua panel", Host::default(), vec![Box::new(ext)]).unwrap();
}
