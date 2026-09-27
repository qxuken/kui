//! Host app + Lua extension sharing one frame. The host owns the left side;
//! `panel.lua`, beside this file, owns the right panel, keeps its own
//! state, and gets its clicks routed back by origin — the host never
//! inspects the script's UI.
//!
//! Where the panel goes is a *slot* the host declares in its own view
//! (`docs/adr/0014-slots-an-extension-fills-in-place.md`): `ui.slot_with`
//! is a position among the host's children, filled then and there, and the
//! parameters it passes are the panel's title and the shape of the reply the
//! host wants when a todo is toggled. The script answers by returning that
//! reply from `on_event`, and the host counts what comes back.
//!
//! The panel is deliberately the same panel as `examples/c/features/slots/panel.c`:
//! the extension contract is the contract and the language is a detail. And
//! once that plugin is built, the script loads it *itself* with
//! `env.add_extension` and places it with `fill` — three languages deep.
//!
//! Run: cargo run -p kui-lua --example lua_panel [-- --headless]

use kui_devtools::Example;
use kui_lua::LuaExtension;
use kui_native::widgets;
use kui_native::{
    Align, App, Core, Extensions, InputEvent, NodeSpec, OriginId, Size, TextStyle, Ui, UiEvent,
    Value, Vec2,
};

#[derive(Default)]
struct Host {
    clicks: i64,
    /// Replies from the panel: one per toggled todo, and who sent it.
    toggles: i64,
    last_reply_from: Option<OriginId>,
}

impl App for Host {
    fn view(&mut self, ui: &mut Ui<'_>) {
        // The host's own colours are theme roles, and so are the guest's:
        // a panel loaded into this frame reads the same palette, which is
        // what makes it look like it belongs (ADR 0019).
        let t = ui.theme();
        ui.open(NodeSpec::row().fill().bg(t.bg).pad(16.0).gap(16.0));

        ui.with(
            NodeSpec::column()
                .fill()
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
        // script when it loads it (`extensions` below - the host decides,
        // like an importer picking an alias), `panel` the slot the script
        // lists.
        ui.slot_with(
            "todos/panel",
            &Value::map([
                ("title", "todos (from the host)".into()),
                ("on_toggle", Value::map([("kind", "toggled".into())])),
            ]),
        );
        ui.close();
    }

    fn on_event(&mut self, ev: UiEvent) {
        match ev.kind() {
            Some("click") => self.clicks += 1,
            // The script's reply, with its origin on the event.
            Some("toggled") => {
                self.toggles += 1;
                self.last_reply_from = Some(ev.origin);
            }
            _ => {}
        }
    }
}

/// The script, loaded under the namespace this host chose for it.
fn load() -> Extensions {
    let script = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../examples/lua/features/slots/panel.lua"
    );
    let ext = LuaExtension::from_file(script).expect("load panel.lua");
    let mut exts = Extensions::new();
    if let Err(e) = exts.push_as("todos", Box::new(ext)) {
        eprintln!("kui: {e}");
        std::process::exit(1);
    }
    exts
}

/// The origin the runner gives the first extension.
const EXT: OriginId = OriginId(1);

/// One frame, built the way the windowed runner builds it: the host's view
/// with the extensions as the filler, so the slot it declares is filled in
/// place.
fn frame(core: &mut Core, host: &mut Host, exts: &mut Extensions) {
    let mut ui = core.frame_with(Size::new(900.0, 600.0), 1.0, exts);
    host.view(&mut ui);
    ui.finish();
}

/// The centre of the first node the access tree reports under `origin`
/// whose name starts with `prefix`.
fn find(core: &mut Core, origin: OriginId, prefix: &str) -> Option<Vec2> {
    core.access_tree().nodes.iter().find_map(|n| {
        let name = n.name.as_deref()?;
        (n.origin == origin && name.starts_with(prefix))
            .then(|| Vec2::new(n.rect.x + n.rect.w / 2.0, n.rect.y + n.rect.h / 2.0))
    })
}

fn click(core: &mut Core, at: Vec2) -> Vec<UiEvent> {
    core.handle_input(InputEvent::CursorMoved(at));
    core.handle_input(InputEvent::mouse_down(1));
    core.handle_input(InputEvent::mouse_up())
}

impl Example for Host {
    fn window(&self) -> kui_devtools::Window {
        // Wide enough for the script's two 300-wide panes beside the
        // host's column: with the C panel loaded the frame is three
        // languages deep, and at 900 the third pane sat past the edge.
        kui_devtools::Window::default().size(1240.0, 640.0)
    }

    fn extensions(&mut self) -> Extensions {
        load()
    }

    /// No window: the same checks the C host runs on the same panel — the
    /// script drew in the slot with the host's title, its click reached
    /// it and not the host, its reply reached the host, and the host's
    /// click reached the host and not it.
    fn headless(&mut self, core: &mut Core) -> Result<(), String> {
        let mut exts = load();
        frame(core, self, &mut exts);
        let drawn = core
            .access_tree()
            .nodes
            .iter()
            .filter(|n| n.origin == EXT)
            .count();
        println!("lua panel: {drawn} nodes in the host's frame");
        if drawn == 0 {
            return Err("the script drew nothing".into());
        }
        if core.key_of("todos/panel").is_none() {
            return Err("the slot `todos/panel` was not declared".into());
        }
        if find(core, EXT, "todos (from the host)").is_none() {
            return Err("the panel is not titled by the host's param".into());
        }
        if let Some(w) = core.take_warnings().first() {
            return Err(format!("warning [{}]: {}", w.code, w.message));
        }

        let todo = find(core, EXT, "[ ] wire up wgpu").ok_or("no unchecked 'wire up wgpu' row")?;
        let evs = click(core, todo);
        if evs.len() != 1 || evs[0].origin != EXT {
            return Err(format!("expected one event on origin {EXT:?}, got {evs:?}"));
        }
        // The runner's routing: the script hears its own click, the host
        // hears the script's reply.
        exts.route(evs, |ev| self.on_event(ev));
        if self.toggles != 1 || self.last_reply_from != Some(EXT) {
            return Err(format!(
                "expected one `toggled` reply from {EXT:?}, got {} from {:?}",
                self.toggles, self.last_reply_from
            ));
        }
        frame(core, self, &mut exts);
        if find(core, EXT, "[x] wire up wgpu").is_none() {
            return Err("the toggle did not reach the script".into());
        }
        if self.clicks != 0 {
            return Err("the host saw the script's click".into());
        }

        let button = find(core, OriginId::HOST, "click me").ok_or("no host button")?;
        let evs = click(core, button);
        if evs.len() != 1 || evs[0].origin != OriginId::HOST {
            return Err(format!("expected one host event, got {evs:?}"));
        }
        self.on_event(evs[0].clone());
        frame(core, self, &mut exts);
        if self.clicks != 1 {
            return Err("the host missed its own click".into());
        }
        if find(core, EXT, "[x] wire up wgpu").is_none() {
            return Err("the host's click disturbed the script".into());
        }
        println!(
            "ok: clicks routed by origin both ways, the slot filled in place, the reply delivered"
        );
        Ok(())
    }
}

kui_devtools::main!(Host::default());
