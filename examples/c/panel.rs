//! Host app + C extension sharing one frame — the mirror image of
//! `examples/c/counter.c`, where C is the host and kui is the library.
//!
//! Here the Rust host owns the window and the left side; `examples/c/panel.c`
//! is a shared library the host `dlopen`s, and it owns the right panel, keeps
//! its own state and gets its own clicks routed back by origin. The host never
//! inspects the plugin's UI or its events, and the plugin links against
//! nothing — it resolves `kui_*` from the host executable at load, the way a
//! Lua C module resolves `lua_*`.
//!
//! Run:
//!   ./examples/c/build.sh                    # builds examples/c/panel.so
//!   cargo run -p kui-ffi --example c_panel   # a window
//!   cargo run -p kui-ffi --example c_panel -- --headless
//!
//! `--headless` is the whole contract without a display: it builds a frame the
//! way the runner does, clicks the plugin's list through the access tree the
//! frame produced, and checks the click reached the plugin and nothing else.
//! A path argument (before or after the flag) loads a different plugin.

use kui::widgets;
use kui::{
    Align, App, Color, Core, Extension, InputEvent, NodeSpec, OriginId, Size, Sizing, TextStyle,
    Ui, UiEvent, Value, Vec2,
};
use kui_ffi::CExtension;

#[derive(Default)]
struct Host {
    clicks: i64,
}

impl App for Host {
    fn view(&mut self, ui: &mut Ui<'_>) {
        // Extensions append after the host, so the root row places the C
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
                ui.text(
                    "host (Rust)",
                    TextStyle::new(12.0).color(Color::rgb8(0x8a, 0x8f, 0xa3)),
                );
                ui.text(&format!("{} clicks", self.clicks), TextStyle::new(40.0));
                widgets::button(ui, "click me", Value::map([("kind", "click".into())]));
            },
        );

        kui::widgets::latency_hud_at(ui, Align::Start, Align::End);
    }

    fn on_event(&mut self, ev: UiEvent) {
        if ev.payload.get("kind").and_then(Value::as_str) == Some("click") {
            self.clicks += 1;
        }
    }
}

/// Where `examples/c/build.sh` leaves the plugin. Overridden by argv[1], which
/// is the more honest reading of what this example does: it loads a library
/// chosen at runtime, not one it was built with.
fn default_plugin() -> String {
    format!("{}/../../examples/c/panel.so", env!("CARGO_MANIFEST_DIR"))
}

/// One frame, built the way the windowed runner builds it: the host's view
/// first, then each extension under the origin the runner assigned it.
fn frame(core: &mut Core, host: &mut Host, ext: &mut CExtension) {
    let mut ui = core.frame(Size::new(900.0, 600.0), 1.0);
    host.view(&mut ui);
    ui.set_origin(EXT);
    ext.view(&mut ui).expect("extension view");
    ui.set_origin(OriginId::HOST);
    ui.finish();
}

/// The origin the runner would give the first extension.
const EXT: OriginId = OriginId(1);

/// The center of the first node the access tree reports under `origin` whose
/// name starts with `prefix` — a click target named the way a screen reader
/// would name it, rather than a quad picked out by its color.
fn find(core: &mut Core, origin: OriginId, prefix: &str) -> Option<Vec2> {
    core.access_tree().nodes.iter().find_map(|n| {
        let name = n.name.as_deref()?;
        (n.origin == origin && name.starts_with(prefix))
            .then(|| Vec2::new(n.rect.x + n.rect.w / 2.0, n.rect.y + n.rect.h / 2.0))
    })
}

/// Presses and releases at `at`, returning what the frame emitted.
fn click(core: &mut Core, at: Vec2) -> Vec<UiEvent> {
    core.handle_input(InputEvent::CursorMoved(at));
    core.handle_input(InputEvent::mouse_down(1));
    core.handle_input(InputEvent::mouse_up())
}

/// No window: builds frames, clicks through them, and checks that the
/// plugin drew, that its click reached it and not the host, and that the
/// host's click reached the host and not it.
fn headless(mut ext: CExtension) -> i32 {
    let mut host = Host::default();
    let mut core = Core::new();
    frame(&mut core, &mut host, &mut ext);

    let drawn = core
        .access_tree()
        .nodes
        .iter()
        .filter(|n| n.origin == EXT)
        .count();
    println!("{}: {drawn} nodes in the host's frame", ext.name());
    if drawn == 0 {
        eprintln!("FAIL: the extension drew nothing");
        return 1;
    }

    // A todo of the plugin's, clicked where the frame says it ended up.
    let Some(todo) = find(&mut core, EXT, "[ ] wire up wgpu") else {
        eprintln!("FAIL: no unchecked 'wire up wgpu' row");
        return 1;
    };
    let evs = click(&mut core, todo);
    if evs.len() != 1 || evs[0].origin != EXT {
        eprintln!("FAIL: expected one event on origin {EXT:?}, got {evs:?}");
        return 1;
    }
    // What the runner's route_events does with it, and the reason this
    // example exists: an event carries the origin of the node that emitted
    // it, so the host is never offered the plugin's clicks.
    ext.on_event(&evs[0]);

    frame(&mut core, &mut host, &mut ext);
    if find(&mut core, EXT, "[x] wire up wgpu").is_none() {
        eprintln!("FAIL: the toggle did not reach the plugin");
        return 1;
    }
    if host.clicks != 0 {
        eprintln!("FAIL: the host saw the plugin's click");
        return 1;
    }

    // And the other way round.
    let Some(button) = find(&mut core, OriginId::HOST, "click me") else {
        eprintln!("FAIL: no host button");
        return 1;
    };
    let evs = click(&mut core, button);
    if evs.len() != 1 || evs[0].origin != OriginId::HOST {
        eprintln!("FAIL: expected one host event, got {evs:?}");
        return 1;
    }
    host.on_event(evs[0].clone());
    frame(&mut core, &mut host, &mut ext);
    if host.clicks != 1 {
        eprintln!("FAIL: the host missed its own click");
        return 1;
    }
    if find(&mut core, EXT, "[x] wire up wgpu").is_none() {
        eprintln!("FAIL: the host's click disturbed the plugin");
        return 1;
    }

    println!("ok: clicks routed by origin, both ways");
    0
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let headless_mode = args.iter().any(|a| a == "--headless");
    let path = args
        .iter()
        .find(|a| !a.starts_with("--"))
        .cloned()
        .unwrap_or_else(default_plugin);

    // SAFETY: none available — the plugin runs in this process. The path is
    // the user's, and loading it is trusting it as much as linking it.
    let ext = match unsafe { CExtension::open(&path) } {
        Ok(ext) => ext,
        Err(err) => {
            eprintln!("kui: {err}");
            eprintln!("build it first: ./examples/c/build.sh");
            std::process::exit(1);
        }
    };
    if headless_mode {
        std::process::exit(headless(ext));
    }
    kui::run("kui — c panel", Host::default(), vec![Box::new(ext)]).unwrap();
}
