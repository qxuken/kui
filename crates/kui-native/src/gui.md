The windowed kui runner: a winit window and a wgpu renderer around
`kui_core`, with the [`App`] trait an application implements.

kui is a Rust UI library whose view is a plain data tree rebuilt every
frame. `kui_core` holds the model, the layout and the widgets, `kui_wgpu`
draws them, and this crate puts a window around both; it is the crate
most apps depend on. It re-exports all of kui-core, so `kui_native::`
reaches everything, and adds the [`App`] trait, the [`app`] launcher
builder, multiple windows, the clipboard, file dialogs, audio, AccessKit
and a headless [`testing`] driver. `kui-derive` supplies
`#[derive(Message)]`; kui-lua, kui-ffi and kui-node are bindings over the
same core.

# Quick start

```rust,no_run
use kui_native::{App, TextStyle, Ui};

struct Hello;

impl App for Hello {
    // Called once per frame. Everything on screen is declared here,
    // from scratch, every time.
    fn view(&mut self, ui: &mut Ui<'_>) {
        let theme = ui.theme();
        ui.text("Hello, kui", TextStyle::new(24.0).color(theme.fg));
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    kui_native::app("Hello").size(360.0, 200.0).run(Hello)
}
```

# State and events

A button carries a message. After the frame, each event it produced
reaches [`App::on_event`], and [`UiEvent::message`] reads the message
back as the app's own enum. An event with data of its own — a drag, a
slider's `change` — arrives as the message its handler declared, with
the data inside under the event's name, and [`UiEvent::split`] reads
the message and a typed [`Event`] in one `match`.

```rust,no_run
use kui_native::widgets;
use kui_native::{App, Message, NodeSpec, TextStyle, Ui, UiEvent};

// `derive(Message)` turns each variant into plain data on the way out
// and back into `Msg` on the way in.
#[derive(Message, Clone, Debug, PartialEq)]
enum Msg {
    Inc,
    Dec,
}

#[derive(Default)]
struct Counter {
    count: i64,
}

impl App for Counter {
    fn view(&mut self, ui: &mut Ui<'_>) {
        let t = ui.theme();
        ui.with(NodeSpec::column().fill().center().gap(16.0).bg(t.bg), |ui| {
            ui.text(&self.count.to_string(), TextStyle::new(56.0).color(t.fg));
            ui.with(NodeSpec::row().gap(8.0), |ui| {
                widgets::button(ui, "-1", Msg::Dec);
                widgets::button(ui, "+1", Msg::Inc);
            });
        });
    }

    fn on_event(&mut self, ev: UiEvent) {
        match ev.message::<Msg>() {
            Some(Msg::Inc) => self.count += 1,
            Some(Msg::Dec) => self.count -= 1,
            // Not ours: a resize, a focus change, a window event.
            None => {}
        }
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    kui_native::app("Counter").size(360.0, 240.0).run(Counter::default())
}
```

The book walks from here through controls, lists, the keyboard, floating
windows, motion, effects and tests, one step per chapter.

# Where to look

- [`App`]: the trait an app implements; `view`, `on_event`, `setup`,
  `teardown`.
- [`app`] and [`Launcher`]: title, size, chrome, icon, extensions, then
  [`Launcher::run`]; [`Launcher::open`] gives a host that owns the loop a
  [`PumpRunner`] instead.
- [`Ui`]: what `view` is given; [`Ui::with`], [`Ui::text`],
  [`Ui::window`].
- [`NodeSpec`] and [`TextStyle`]: a box's layout, look and handlers; a
  text's size, font and colour.
- [`widgets`]: buttons, checkboxes, text inputs, selects, sliders,
  lists, tables and menus.
- [`UiEvent`]: what `on_event` receives, with the readers on it.
- [`testing`]: a headless driver for an `App`, for `cargo test`.
- [`capture`]: a core's finished frame as RGBA pixels, a window's or a
  headless one's.
- From kui-core: [`theme`], [`anim`] (transitions and springs),
  [`window`] (several windows), [`message`] and [`Value`] (the payload
  behind a message), [`Core`] (one window's runtime, for hosts and
  tests).

# Windows

One event loop, any number of windows. The launcher opens the main
window; a frame that declares another with [`Ui::window`] opens it on the
same session and GPU device, and a frame that stops declaring it closes
it. `view` runs once per open window per frame, [`Ui::window_name`] says
which (`"main"` for the launcher's), and every event carries its window's
[`WindowId`].

# Features

- `gui` (default) enables the runner, renderer and headless driver.
  With default features off, use `system` to prepare a core for your own host.
- `image-codecs` is enabled by `gui` and can be used independently.
- `audio` (default): plays sounds through kira and cpal. Off, audio
  commands are dropped and no audio library is linked (ALSA on Linux).
- `accesskit` (default): exposes the access tree to screen readers
  through AccessKit. Off, the tree is still built and nothing reaches
  the OS.
- `derive` (default): `#[derive(Message)]`. Off, messages are built and
  matched as [`Value`]s by hand.
- `dialogs` (default): the platform's file dialogs through rfd (the XDG
  portal on Linux). Off, every dialog answers at once as cancelled.
- `smoke`: honours `KUI_SMOKE_FRAMES=n`, which closes the window after
  `n` frames, in a release build. A debug build honours it regardless.

# Linux

Building needs `pkg-config` and ALSA's headers (`libasound2-dev` on
Debian and Ubuntu). A window loads the rest at run time: `libxkbcommon`
(and `libxkbcommon-x11` on X11); `libX11`, `libXcursor`, `libXrandr` and
`libXi` on X11 or `libwayland-client` on Wayland; `libvulkan` with a
driver, or `libEGL`. A missing one fails when the window opens, naming
the library. macOS and Windows need only a Rust toolchain.

The book: <https://kui-book.qxuken.dev>. Repository and design records
(under `docs/adr`): <https://github.com/qxuken/kui>.
