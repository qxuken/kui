//! Host app + C extension sharing one frame — the mirror image of
//! `examples/c/counter.c`, where C is the host and kui is the library.
//!
//! Here the Rust host owns the window and the left side; `examples/c/panel.c`
//! is a shared library the host `dlopen`s, and it owns the right panel, keeps
//! its own state and gets its own clicks routed back by origin. The host never
//! inspects the plugin's UI, and the plugin links against nothing — it
//! resolves `kui_*` from the host executable at load, the way a Lua C module
//! resolves `lua_*`.
//!
//! Where the panel goes is a *slot* the host declares in its own view
//! (`docs/adr/0014-slots-an-extension-fills-in-place.md`): `ui.slot_with`
//! below is a position among the host's children, filled then and there,
//! and the parameters it passes are the panel's title and the shape of the
//! reply the host wants when a todo is toggled. The plugin answers with
//! `kui_reply`, and the host counts what comes back.
//!
//! Run:
//!   ./examples/c/build.sh                    # target/debug/panel.so
//!   pwsh examples/c/build.ps1                # or panel.dll, on Windows
//!   cargo run -p kui-ffi --example c_panel   # a window
//!   cargo run -p kui-ffi --example c_panel -- --headless
//!
//! `--headless` is the whole contract without a display: it builds a frame the
//! way the runner does, clicks the plugin's list through the access tree the
//! frame produced, and checks that the click reached the plugin and nothing
//! else — and that the plugin's reply reached the host with the plugin's
//! origin on it. A path argument (before or after the flag) loads a
//! different plugin.

use kui::widgets;
use kui::{
    Align, App, Core, Extension, Extensions, InputEvent, NodeSpec, OriginId, Size, Sizing,
    TextStyle, Ui, UiEvent, Value, Vec2,
};
use kui_ffi::CExtension;

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

        // The panel's place: the second child of the root row, so it lands
        // to the right of the column above. The params are what the host
        // wants the plugin to know — its title — and the reply it wants
        // back when a todo is toggled, as a template the plugin fills in.
        // The name is `namespace/slot`: `todos` is what this host calls
        // the plugin (the host decides, like an importer picking an alias;
        // the plugin calls itself "c panel"), and `panel` is the slot the
        // plugin lists.
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
            // A reply the panel made from its own click: the origin says
            // which extension, the key which of its nodes.
            Some("toggled") => {
                self.toggles += 1;
                self.last_reply_from = Some(ev.origin);
            }
            _ => {}
        }
    }
}

/// Where `examples/c/build.sh` — or `build.ps1` — leaves the plugin:
/// `target/<profile>/`, which is one directory above this example's own
/// binary. Read off the executable rather than `CARGO_MANIFEST_DIR` so
/// that a release build finds the release plugin, and because the manifest
/// directory is a fact about where this was *compiled*.
///
/// Overridden by argv[1], which is the more honest reading of what this
/// example does: it loads a library chosen at runtime, not one it was built
/// with. `.so` on both unixes, because that is what `build.sh` names it and
/// `dlopen` does not care; `.dll` on Windows, because `LoadLibraryW` does.
fn default_plugin() -> String {
    let exe = std::env::current_exe().unwrap_or_default();
    let dir = exe
        .parent()
        .and_then(|examples| examples.parent())
        .unwrap_or(std::path::Path::new("target/debug"))
        .to_path_buf();
    dir.join(format!("panel.{PLUGIN_EXT}"))
        .display()
        .to_string()
}

/// What the plugin is called on this platform, and what builds it.
const PLUGIN_EXT: &str = if cfg!(target_os = "windows") {
    "dll"
} else {
    "so"
};
const BUILD_HINT: &str = if cfg!(target_os = "windows") {
    "pwsh examples/c/build.ps1 -Run"
} else {
    "./examples/c/build.sh --run"
};

/// One frame, built the way the windowed runner builds it: the host's view
/// with the extensions as the filler, so the slot it declares is filled in
/// place and `finish` fills `"ns/root"` for any extension that names no
/// slot.
fn frame(core: &mut Core, host: &mut Host, exts: &mut Extensions) {
    let mut ui = core.frame_with(Size::new(900.0, 600.0), 1.0, exts);
    host.view(&mut ui);
    ui.finish();
}

/// The origin the runner gives the first extension.
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
/// plugin drew in the slot with the host's title, that its click reached it
/// and not the host, that its reply reached the host, and that the host's
/// click reached the host and not it.
fn headless(ext: CExtension) -> i32 {
    let name = ext.name().to_owned();
    // Loaded under the namespace this host chose for it, `todos`; its own
    // name is what it calls itself in logs.
    let mut exts = Extensions::new();
    if let Err(e) = exts.push_as("todos", Box::new(ext)) {
        eprintln!("FAIL: {e}");
        return 1;
    }
    let mut host = Host::default();
    let mut core = Core::new();
    frame(&mut core, &mut host, &mut exts);

    let drawn = core
        .access_tree()
        .nodes
        .iter()
        .filter(|n| n.origin == EXT)
        .count();
    println!("{name}: {drawn} nodes in the host's frame");
    if drawn == 0 {
        eprintln!("FAIL: the extension drew nothing");
        return 1;
    }
    // The slot is a named position: `key_of` answers it like a keyed node.
    if core.key_of("todos/panel").is_none() {
        eprintln!("FAIL: the slot `todos/panel` was not declared");
        return 1;
    }
    // The title came through the params — it is the host's string, not the
    // plugin's default.
    if find(&mut core, EXT, "todos (from the host)").is_none() {
        eprintln!("FAIL: the panel is not titled by the host's param");
        return 1;
    }
    if let Some(w) = core.take_warnings().first() {
        eprintln!("FAIL: warning [{}]: {}", w.code, w.message);
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
    // An event carries the origin of the node that emitted it, so the host
    // is never offered the plugin's clicks — and what the plugin *replies*
    // is the one thing that does reach the host. `Extensions::route` is
    // the walk the runner takes, and the same one.
    exts.route(evs, |ev| host.on_event(ev));
    if host.toggles != 1 || host.last_reply_from != Some(EXT) {
        eprintln!(
            "FAIL: expected one `toggled` reply from {EXT:?}, got {} toggles from {:?}",
            host.toggles, host.last_reply_from
        );
        return 1;
    }

    frame(&mut core, &mut host, &mut exts);
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
    frame(&mut core, &mut host, &mut exts);
    if host.clicks != 1 {
        eprintln!("FAIL: the host missed its own click");
        return 1;
    }
    if find(&mut core, EXT, "[x] wire up wgpu").is_none() {
        eprintln!("FAIL: the host's click disturbed the plugin");
        return 1;
    }

    println!(
        "ok: clicks routed by origin both ways, the slot filled in place, the reply delivered"
    );
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
            eprintln!("build it first: {BUILD_HINT}");
            std::process::exit(1);
        }
    };
    if headless_mode {
        std::process::exit(headless(ext));
    }
    // The same namespace in the window as headless: the host decides it.
    kui::app("kui — c panel")
        .extension_as("todos", ext)
        .run(Host::default())
        .unwrap();
}
