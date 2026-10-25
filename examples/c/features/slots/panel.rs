//! Host app + C extension sharing one frame — the mirror image of
//! `examples/c/apps/counter.c`, where C is the host and kui is the library.
//!
//! Here the Rust host owns the window and the left side; `panel.c`, beside this file,
//! is a shared library the host `dlopen`s, and it owns the right panel, keeps
//! its own state and gets its own clicks routed back by origin. The host never
//! inspects the plugin's UI, and the plugin links against nothing — it
//! resolves `kui_*` from the host executable at load, the way a Lua C module
//! resolves `lua_*`.
//!
//! Where the panel goes is a *component* the host places in its own view
//! (`docs/adr/0067-slots-fold-into-components.md`): `ui.component` below is
//! a position among the host's children, drawn then and there, and the
//! props it passes are the panel's title and the message the host wants
//! when a todo is toggled (`on_toggle`). The plugin raises it with
//! `kui_emit`, and the host counts what comes back. The host also declares
//! a contribution slot, `todos.toolbar`, with `ui.slot`: whatever loaded
//! plugin contributes to it (the panel's chip) is placed there, and its
//! press arrives as the host's `on_press` with `from` naming the provider.
//!
//! Run:
//!   cargo run -p kui-devtools --bin cbuild   # target/debug/panel.so (panel.dll on Windows)
//!   cargo run -p kui-ffi --example c_panel   # a window
//!   cargo run -p kui-ffi --example c_panel -- --headless
//!
//! `--headless` is the whole contract without a display: it builds a frame the
//! way the runner does, clicks the plugin's list through the access tree the
//! frame produced, and checks that the click reached the plugin and nothing
//! else — and that the plugin's `on_toggle` reached the host as the host's
//! own message, about the panel's instance. A path argument (before or after
//! the flag) loads a different plugin.

use kui_devtools::Example;
use kui_ffi::CExtension;
use kui_native::widgets;
use kui_native::{
    Align, App, Core, EventKind, Extension, Extensions, FillAnswer, InputEvent, Key, NodeSpec,
    OriginId, Size, TextStyle, Ui, UiEvent, Value, Vec2,
};

#[derive(Default)]
struct Host {
    clicks: i64,
    /// The panel's `on_toggle`, one per toggled todo: which row it said,
    /// and the instance the event is about.
    toggles: i64,
    last_toggle: Option<(i64, Key)>,
    /// How the last frame filled the panel.
    panel_fill: Option<FillAnswer>,
    /// How many contributions the toolbar slot placed last frame.
    toolbar: usize,
    /// The slot's `on_press`, and who raised each: `UiEvent::from`.
    chipped: Vec<Option<String>>,
    /// The plugin, loaded in `main` and taken by whichever of the window
    /// and the headless drive runs.
    ext: Option<CExtension>,
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
                // A contribution slot (ADR 0067): a place, not a plugin.
                // Whatever the loaded extensions contribute to it lands
                // here with these props; the event's `from` says who.
                self.toolbar = ui.slot(
                    "todos.toolbar",
                    &Value::map([("on_press", Value::map([("kind", "chipped".into())]))]),
                );
            },
        );

        // The panel's place: the second child of the root row, so it lands
        // to the right of the column above. The props are what the host
        // wants the plugin to know — its title — and the message it wants
        // back when a todo is toggled. The name is `namespace/component`:
        // `todos` is what this host calls the plugin (the host decides,
        // like an importer picking an alias; the plugin calls itself "c
        // panel"), and `panel` is a component the plugin lists. Replayed
        // while nothing it read moved (ADR 0067).
        self.panel_fill = ui.component(
            "todos/panel",
            &Value::map([
                ("title", "todos (from the host)".into()),
                ("on_toggle", Value::map([("kind", "toggled".into())])),
            ]),
        );
        ui.close();
    }

    fn on_event(&mut self, ev: UiEvent) {
        // The envelope says what happened; the payload is the message the
        // button declared, or the one this host passed the panel (ADR 0047).
        let kind = ev.payload.get("kind").and_then(Value::as_str);
        match (ev.event, kind) {
            (EventKind::Click, Some("click")) => self.clicks += 1,
            // The panel's `on_toggle`, raised from its own click: this
            // host's message, the plugin's fields under `toggle`, and the
            // key the instance it is about.
            (EventKind::Component, Some("toggled")) => {
                self.toggles += 1;
                let index = ev
                    .payload
                    .get("toggle")
                    .and_then(|t| t.get("index"))
                    .and_then(Value::as_int)
                    .unwrap_or(-1);
                self.last_toggle = Some((index, ev.key));
            }
            (EventKind::Component, Some("chipped")) => self.chipped.push(ev.from.clone()),
            _ => {}
        }
    }
}

/// Where the `cbuild` tool (examples/devtools/src/bin/cbuild.rs) leaves
/// the plugin:
/// `target/<profile>/`, which is one directory above this example's own
/// binary. Read off the executable rather than `CARGO_MANIFEST_DIR` so
/// that a release build finds the release plugin, and because the manifest
/// directory is a fact about where this was *compiled*.
///
/// Overridden by argv[1], which is the more honest reading of what this
/// example does: it loads a library chosen at runtime, not one it was built
/// with. `.so` on both unixes, because that is what `cbuild` names it and
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
const BUILD_HINT: &str = "cargo run -p kui-devtools --bin cbuild -- --run";

/// One frame, built the way the windowed runner builds it: the host's view
/// with the extensions as the filler, so the component it places is drawn
/// in place and `finish` draws `"ns/root"` for any extension that provides
/// no components.
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
/// plugin drew its panel with the host's title, that its click reached it
/// and not the host, that its `on_toggle` reached the host, and that the
/// host's click reached the host and not it.
fn headless(host: &mut Host, core: &mut Core, ext: CExtension) -> i32 {
    let name = ext.name().to_owned();
    // Loaded under the namespace this host chose for it, `todos`; its own
    // name is what it calls itself in logs.
    let mut exts = Extensions::new();
    if let Err(e) = exts.push_as("todos", Box::new(ext)) {
        eprintln!("FAIL: {e}");
        return 1;
    }
    frame(core, host, &mut exts);

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
    // Placed, and drawn by the plugin: its first frame has nothing kept.
    if host.panel_fill != Some(FillAnswer::NotKept) {
        eprintln!("FAIL: `todos/panel` answered {:?}", host.panel_fill);
        return 1;
    }
    // The title came through the props — it is the host's string, not the
    // plugin's default.
    if find(core, EXT, "todos (from the host)").is_none() {
        eprintln!("FAIL: the panel is not titled by the host's param");
        return 1;
    }
    if let Some(w) = core.take_warnings().first() {
        eprintln!("FAIL: warning [{}]: {}", w.code, w.message);
        return 1;
    }

    // A todo of the plugin's, clicked where the frame says it ended up.
    let Some(todo) = find(core, EXT, "[ ] wire up wgpu") else {
        eprintln!("FAIL: no unchecked 'wire up wgpu' row");
        return 1;
    };
    let evs = click(core, todo);
    if evs.len() != 1 || evs[0].origin != EXT {
        eprintln!("FAIL: expected one event on origin {EXT:?}, got {evs:?}");
        return 1;
    }
    // An event carries the origin of the node that emitted it, so the host
    // is never offered the plugin's clicks — and what the plugin *emits* is
    // the one thing that does reach the host, as the host's own message.
    // `Extensions::route` is the walk the runner takes, and the same one.
    exts.route(evs, |ev| host.on_event(ev));
    if host.toggles != 1 || !matches!(host.last_toggle, Some((1, _))) {
        eprintln!(
            "FAIL: expected one `toggled` for row 1, got {} toggles, last {:?}",
            host.toggles, host.last_toggle
        );
        return 1;
    }

    frame(core, host, &mut exts);
    if find(core, EXT, "[x] wire up wgpu").is_none() {
        eprintln!("FAIL: the toggle did not reach the plugin");
        return 1;
    }
    if host.clicks != 0 {
        eprintln!("FAIL: the host saw the plugin's click");
        return 1;
    }

    // And the other way round.
    let Some(button) = find(core, OriginId::HOST, "click me") else {
        eprintln!("FAIL: no host button");
        return 1;
    };
    let evs = click(core, button);
    if evs.len() != 1 || evs[0].origin != OriginId::HOST {
        eprintln!("FAIL: expected one host event, got {evs:?}");
        return 1;
    }
    host.on_event(evs[0].clone());
    frame(core, host, &mut exts);
    if host.clicks != 1 {
        eprintln!("FAIL: the host missed its own click");
        return 1;
    }
    if find(core, EXT, "[x] wire up wgpu").is_none() {
        eprintln!("FAIL: the host's click disturbed the plugin");
        return 1;
    }

    // The toolbar slot: the panel's chip, found by the name it took from
    // the slot it was placed in, and its press back with `from`.
    if host.toolbar != 1 {
        eprintln!(
            "FAIL: the toolbar slot placed {} contributions, not 1",
            host.toolbar
        );
        return 1;
    }
    let Some(chip) = find(core, EXT, "chip in todos.toolbar") else {
        eprintln!("FAIL: the panel's chip is not in the toolbar slot");
        return 1;
    };
    let evs = click(core, chip);
    exts.route(evs, |ev| host.on_event(ev));
    if host.chipped != [Some("todos".to_owned())] {
        eprintln!(
            "FAIL: expected one on_press from `todos`, got {:?}",
            host.chipped
        );
        return 1;
    }

    println!(
        "ok: clicks routed by origin both ways, the panel placed in place, its on_toggle \
         delivered, the toolbar chip's press from `todos`"
    );
    0
}

impl Example for Host {
    fn window(&self) -> kui_devtools::Window {
        kui_devtools::Window::default().size(900.0, 600.0)
    }

    /// The same namespace in the window as headless: the host decides it.
    fn extensions(&mut self) -> Extensions {
        let mut exts = Extensions::new();
        if let Some(ext) = self.ext.take()
            && let Err(e) = exts.push_as("todos", Box::new(ext))
        {
            eprintln!("kui: {e}");
            std::process::exit(1);
        }
        exts
    }

    fn headless(&mut self, core: &mut Core) -> Result<(), String> {
        let ext = self.ext.take().ok_or("no plugin loaded")?;
        match headless(self, core, ext) {
            0 => Ok(()),
            _ => Err("see above".into()),
        }
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
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
    kui_devtools::run(
        env!("CARGO_BIN_NAME"),
        Host {
            ext: Some(ext),
            ..Host::default()
        },
    );
}
