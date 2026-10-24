//! Kit's M0 spike: three Kit components, written in Teal under
//! `addons/kit/src` and loaded as one Lua extension, in a Rust host. A
//! toolbar, a table of 100,000 rows and a trace waterfall of 18,000 bars,
//! painted by the Ledger skin from the theme's appearance, every event
//! arriving as the app's own message.
//!
//! What it stands in for, until the core changes in ADRs A and B exist:
//! instances are slots named `kit/<component>#n`, the toolbar's buttons
//! are data in its params rather than host children, and the declared
//! messages are merged by the script. `scene.rs` says how.
//!
//! Run: cargo run -p kui-lua --example lua_kit_spike [-- --headless]
//! Rebuild the Lua after editing the Teal: lua addons/kit/build.lua

// The benches use the scene's other button placements.
#[allow(dead_code)]
mod scene;

use kui_devtools::Example;
use kui_native::{App, Core, Extensions, InputEvent, OriginId, Size, Theme, Ui, UiEvent, Vec2};
use scene::{Msg, Scene};

struct Spike(Scene);

impl App for Spike {
    fn view(&mut self, ui: &mut Ui<'_>) {
        self.0.view(ui);
    }

    fn on_event(&mut self, ev: UiEvent) {
        if let Some(m) = ev.message::<Msg>() {
            self.0.on_message(m);
        }
    }
}

fn load() -> Extensions {
    let mut exts = Extensions::new();
    if let Err(e) = exts.push_as("kit", Box::new(scene::load_kit())) {
        eprintln!("kui: {e}");
        std::process::exit(1);
    }
    exts
}

const KIT: OriginId = OriginId(1);

fn frame(core: &mut Core, app: &mut Spike, exts: &mut Extensions) {
    let mut ui = core.frame_with(Size::new(1280.0, 820.0), 1.0, exts);
    app.view(&mut ui);
    ui.finish();
}

fn find(core: &mut Core, prefix: &str) -> Option<Vec2> {
    core.access_tree().nodes.iter().find_map(|n| {
        let name = n.name.as_deref()?;
        (n.origin == KIT && name.starts_with(prefix))
            .then(|| Vec2::new(n.rect.x + n.rect.w / 2.0, n.rect.y + n.rect.h / 2.0))
    })
}

fn click(core: &mut Core, exts: &mut Extensions, app: &mut Spike, at: Vec2) {
    core.handle_input(InputEvent::CursorMoved(at));
    core.handle_input(InputEvent::mouse_down(1));
    let evs = core.handle_input(InputEvent::mouse_up());
    exts.route(evs, |ev| app.on_event(ev));
}

fn clean(core: &mut Core, when: &str) -> Result<(), String> {
    match core.take_warnings().first() {
        Some(w) => Err(format!("{when}: warning [{}]: {}", w.code, w.message)),
        None => Ok(()),
    }
}

impl Example for Spike {
    fn window(&self) -> kui_devtools::Window {
        kui_devtools::Window::default().size(1280.0, 820.0)
    }

    fn extensions(&mut self) -> Extensions {
        load()
    }

    /// The same checks in both appearances: Kit drew, raised no warning,
    /// and its button, row and bar each reached the app as its message.
    fn headless(&mut self, core: &mut Core) -> Result<(), String> {
        for (name, theme) in [("light", Theme::light()), ("dark", Theme::dark())] {
            core.set_theme(theme);
            let mut app = Spike(Scene::new(100_000, 3_000));
            let mut exts = load();
            frame(core, &mut app, &mut exts);
            frame(core, &mut app, &mut exts);
            clean(core, name)?;
            let drawn = core
                .access_tree()
                .nodes
                .iter()
                .filter(|n| n.origin == KIT)
                .count();
            if drawn == 0 {
                return Err(format!("{name}: Kit drew nothing"));
            }

            let run = find(core, "Run").ok_or("no Run button")?;
            click(core, &mut exts, &mut app, run);
            if app.0.runs != 1 {
                return Err(format!("{name}: Run reached the app {} times", app.0.runs));
            }

            let row = find(core, "5 · ").ok_or("no row 5")?;
            click(core, &mut exts, &mut app, row);
            if app.0.selected != Some(5) {
                return Err(format!("{name}: selected {:?}, not row 5", app.0.selected));
            }

            // Zoomed to the first frames, so the bars are wide enough to click.
            app.0.window = Some((0.0, 60.0));
            frame(core, &mut app, &mut exts);
            frame(core, &mut app, &mut exts);
            let bar = find(core, "frame").ok_or("no frame bar")?;
            click(core, &mut exts, &mut app, bar);
            if app.0.picked.as_deref() != Some("f0/frame") {
                return Err(format!("{name}: picked {:?}, not f0/frame", app.0.picked));
            }

            // Settings re-declare the skin's tokens: still clean after.
            let shadows = find(core, "Shadows").ok_or("no Shadows button")?;
            click(core, &mut exts, &mut app, shadows);
            frame(core, &mut app, &mut exts);
            clean(core, name)?;
            println!(
                "{name}: {drawn} Kit nodes; Run, row 5 and bar f0 arrived as the app's messages"
            );
        }
        println!("ok: Kit's toolbar, table and waterfall drew and answered in both appearances");
        Ok(())
    }
}

kui_devtools::main!(Spike(Scene::new(100_000, 3_000)));
