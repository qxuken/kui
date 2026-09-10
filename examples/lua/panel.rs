//! Host app + Lua extension sharing one frame. The host owns the left side;
//! panel.lua owns the right panel, keeps its own state, and gets its clicks
//! routed back by origin — the host never inspects the script's UI.
//!
//! Where the panel goes is a *slot* the host declares in its own view
//! (`docs/adr/0014-slots-an-extension-fills-in-place.md`): `ui.slot_with`
//! is a position among the host's children, filled then and there, and the
//! parameters it passes are the panel's title and the shape of the reply the
//! host wants when a todo is toggled. The script answers by returning that
//! reply from `on_event`, and the host counts what comes back.
//!
//! Run: cargo run -p kui-lua --example lua_panel

use kui::widgets;
use kui::{Align, App, NodeSpec, Sizing, TextStyle, Ui, UiEvent, Value};
use kui_lua::LuaExtension;

#[derive(Default)]
struct Host {
    clicks: i64,
    /// Replies from the panel: one per toggled todo.
    toggles: i64,
}

impl App for Host {
    fn view(&mut self, ui: &mut Ui<'_>) {
        // The host's own colours are theme roles, and so are the guest's:
        // a panel loaded into this frame reads the same palette, which is
        // what makes it look like it belongs (ADR 0019).
        let t = ui.theme();
        ui.configure_root(NodeSpec::row().fill().bg(t.bg).pad(16.0).gap(16.0));

        ui.with(
            NodeSpec::column()
                .width(Sizing::Grow(1.0))
                .height(Sizing::Grow(1.0))
                .pad(24.0)
                .gap(16.0)
                .bg(t.surface)
                .radius(10.0)
                .cross_align(Align::Center)
                .main_align(Align::Center),
            |ui| {
                ui.text("host (Rust)", TextStyle::new(12.0).color(t.muted));
                ui.text(&format!("{} clicks", self.clicks), TextStyle::new(40.0));
                ui.text(
                    &format!("{} toggles reported by the panel", self.toggles),
                    TextStyle::new(13.0).color(t.muted),
                );
                widgets::button(ui, "click me", Value::map([("kind", "click".into())]));
            },
        );

        // The panel's place: the second child of the root row, to the right
        // of the column above. The params are the title the host wants on
        // it and the reply it wants back on a toggle, as a template the
        // script fills in.
        // The name is `namespace/slot`: `todos` is what this host calls the
        // script when it loads it (`extension_as` below - the host decides,
        // like an importer picking an alias), `panel` the slot the script
        // lists.
        ui.slot_with(
            "todos/panel",
            &Value::map([
                ("title", "todos (from the host)".into()),
                ("on_toggle", Value::map([("kind", "toggled".into())])),
            ]),
        );

        kui::widgets::latency_hud_at(ui, Align::Start, Align::End);
    }

    fn on_event(&mut self, ev: UiEvent) {
        match ev.payload.get("kind").and_then(Value::as_str) {
            Some("click") => self.clicks += 1,
            // The script's reply, with its origin on the event.
            Some("toggled") => self.toggles += 1,
            _ => {}
        }
    }
}

fn main() {
    let script = concat!(env!("CARGO_MANIFEST_DIR"), "/../../examples/lua/panel.lua");
    let ext = LuaExtension::from_file(script).expect("load panel.lua");
    kui::app("kui — lua panel")
        .extension_as("todos", ext)
        .run(Host::default())
        .unwrap();
}
