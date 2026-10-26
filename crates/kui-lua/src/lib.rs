//! Lua scripts as kui extensions: a script returns its view as a table tree and gets events back as tables.
//!
//! `kui-lua` loads a Lua script as an [`Extension`] a kui host can place in
//! its frame. The script defines `view(env, inst)` or `components = { … }`,
//! which return a plain table tree built with the `row` / `column` / `text` /
//! `button` prelude, and optionally `on_event(ev)`, which receives the events
//! the script's own nodes emit and may `emit` the host's messages. Nothing but data crosses
//! the boundary: the host gets nodes, the script gets event tables, and no
//! closure lives on either side. It sits beside [kui-ffi] (C plugins) and
//! [kui-node] (the Node.js package) as one of the bindings over
//! [`kui_core`]; most hosts run it inside [kui-native], the windowed runner.
//!
//! Two readers meet here. A Rust application wants scriptable panels or
//! plugins: it places the script's component in its own view, loads the
//! script under a namespace, and counts the messages that come back, without ever looking at
//! the script's UI. The author of such a script wants to know what `view`
//! is handed, which builders exist and which props a table takes; the
//! reference below is for them, and [`luals_meta`] turns it into completion
//! for lua-language-server, as [`teal_decls`] turns it into a declaration
//! file for Teal, the typed Lua dialect.
//!
//! [kui-ffi]: https://crates.io/crates/kui-ffi
//! [kui-node]: https://www.npmjs.com/package/@qxuken/kui
//! [kui-native]: https://crates.io/crates/kui-native
//!
//! # Quick start
//!
//! The Rust side: a kui-native app that places the script's panel and hears
//! its events (ADR 0067). `extension_as` names the script's namespace, so
//! the component the view places is `todos/panel` for a script whose
//! `components` table has a `panel`.
//!
//! ```rust,no_run
//! use kui_lua::LuaExtension;
//! use kui_native::{App, NodeSpec, Ui, UiEvent, Value};
//!
//! struct Host {
//!     toggles: u32,
//! }
//!
//! impl App for Host {
//!     fn view(&mut self, ui: &mut Ui<'_>) {
//!         ui.configure_root(NodeSpec::row().fill().pad(16.0).gap(16.0));
//!         // The script draws here. The props are its to read; `on_toggle`
//!         // is the message the host hears when the panel emits it.
//!         ui.component(
//!             "todos/panel",
//!             &Value::map([
//!                 ("title", "todos".into()),
//!                 ("on_toggle", Value::map([("kind", "toggled".into())])),
//!             ]),
//!         );
//!     }
//!
//!     fn on_event(&mut self, ev: UiEvent) {
//!         if ev.payload.get_str("kind") == Some("toggled") {
//!             self.toggles += 1;
//!         }
//!     }
//! }
//!
//! fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let script = LuaExtension::from_file("panel.lua")?;
//!     kui_native::app("todos")
//!         .extension_as("todos", script)
//!         .run(Host { toggles: 0 })
//! }
//! ```
//!
//! The Lua side, `panel.lua`: a todo list that keeps its own state, reads
//! the title the host passed, and raises `on_toggle` on a toggle — the host
//! hears it as the message it passed for that prop.
//!
//! ```lua
//! local todos = { "ship the layout solver", "wire up wgpu" }
//! local done = {}
//!
//! components = {}
//!
//! function components.panel(env, props, inst)
//!   local t = env.theme
//!   local items = {}
//!   for i, todo in ipairs(todos) do
//!     items[#items + 1] = row {
//!       gap = 8, pad = { t = 4, b = 4 },
//!       on_click = { kind = "toggle", index = i },
//!       text((done[i] and "[x] " or "[ ] ") .. todo, { size = 14, color = t.fg }),
//!     }
//!   end
//!   return column {
//!     width = 300, height = "grow", pad = 16, gap = 10,
//!     bg = t.surface, radius = 10, border = { w = 1, color = t.border },
//!     text(props.title or "todos", { size = 12, color = t.muted }),
//!     edit { key = "filter", label = "filter todos", initial = "", width = "grow" },
//!     column { height = "grow", scroll = true, table.unpack(items) },
//!     button { label = "add", on_click = { kind = "add" } },
//!   }
//! end
//!
//! function on_event(ev)
//!   if ev.kind == "toggle" then
//!     done[ev.index] = not done[ev.index]
//!     return emit("on_toggle", { index = ev.index, done = done[ev.index] })
//!   elseif ev.kind == "add" then
//!     todos[#todos + 1] = "todo #" .. (#todos + 1)
//!   end
//! end
//! ```
//!
//! A headless host (tests, tools) uses [`kui_core::Core`] directly: load
//! with [`LuaExtension::from_source`], push the extension into a
//! [`kui_core::Extensions`] list and build frames with
//! `Core::frame_with`. The `LuaExtension` docs show that path.
//!
//! # What a script gets
//!
//! ## Builders
//!
//! The prelude injects one global per element; each takes a table of props
//! whose integer keys are the children and returns it with `type` set.
//!
//! - Containers: `row { }`, `column { }`, `grid { }` (a table whose cells
//!   line up in columns; named `grid` because `table` is Lua's).
//! - Text: `text("plain", opts)` or `text({ "a ", { "b", bold = true } }, opts)`
//!   for spans; `tooltip("hint")` as a node, or `tooltip = "hint"` as a prop.
//! - Controls: `button { label = }`, `checkbox`, `radio`, `switch`,
//!   `radio_group { radio { }, ... }`, `slider { value_now =, value_min =,
//!   value_max =, on_change = }`, `input { label =, value =, on_change =,
//!   on_submit =, on_cancel = }` (single line with chrome), `edit { key =,
//!   value = | initial =, on_change = }` (bare editor), `dropdown {
//!   options =, current =, on_menu = }` (named `dropdown` because `select`
//!   is Lua's).
//! - Media: `image { id = }`, `fragment { id = }` (a WGSL-painted box),
//!   `line { from =, to = }`, `polygon { points = }`, `path { d = }` (any
//!   outline, SVG path data), `cells { }` (a terminal screen as one
//!   node), `audio { src = }`.
//! - Window chrome: `titlebar { title = }`, `window_buttons()`, `menu_bar
//!   { menu =, on_menu = }`, `latency_graph()`, `latency_hud { }`.
//! - Lists: `uniform_list(env, opts, row)` for rows of one height,
//!   `list(env, opts, measure, row)` for rows of different heights, sliced
//!   by a `row_heights(rows, estimate)` the script keeps between frames;
//!   `reveal_row(env, key, i, row_h)` and `rows_in_view(env, key, row_h)` go
//!   with them. `splitter(env, { key =, dir =, on_drag = })` is a draggable
//!   divider.
//! - Hosting: `component("ns/name", props, children)` places what a plugin
//!   this script loaded provides — kept and replayed while nothing it read
//!   moved (ADR 0060, ADR 0067); `devtools_tab { name =, label =,
//!   component =, props = | view = }` adds a tab to the core's devtools
//!   panel.
//!
//! ## The `env` table
//!
//! `env` is built fresh for every `view` call. Its values are the facts the
//! host wrote before `view` ran; its functions answer about the frame being
//! built and the last one finished.
//!
//! Facts:
//!
//! - Timing and size: `refresh_hz`, `frame_budget_ms`, `viewport_w`,
//!   `viewport_h`, `now` (the frame clock in seconds). A fact the host
//!   cannot tell is left out rather than nil.
//! - Focus: `focused` (the window has the keyboard), `focus` (the focused
//!   node's key, nil for none), `focus_visible`, `caret_visible`, `region`,
//!   `tab_moves_focus` (the window's "Tab moves focus" toggle, backlog C99).
//! - Window: `window.id`, `window.fullscreen`, `window.maximized`,
//!   `window.always_on_top`, `window.custom_chrome`, `window.controls_w` /
//!   `window.controls_h` (the keep-out extent of OS-drawn controls).
//! - System: `system.appearance`, `system.accent`, `system.locale`,
//!   `system.motion`, `system.assistive`; `audio.live`, `audio.device`.
//! - Palette: `env.theme` has one `0xRRGGBBAA` number per theme role
//!   (`bg`, `surface`, `fg`, `muted`, `border`, `accent`, ...) plus
//!   `appearance` and `disabled_opacity`; `env.metrics` the sizes the stock
//!   widgets use; `env.tokens.colors` / `env.tokens.lengths` /
//!   `env.tokens.families` the named tokens in force, the script's own over
//!   the host's (a family as the name its list picked). All read-only.
//!
//! Functions, by topic. `key` is an integer key an event carried or the
//! string label a node's `key` prop declared, resolved among the script's
//! own nodes:
//!
//! - Queries: `edit_text(key)`, `set_edit_text(key, text)`,
//!   `is_focused(key)`, `is_hovered(key)`, `is_pressed(key)`,
//!   `is_drop_target(key)`, `drop_target()`, `layout_of(key)`,
//!   `measure_text(s, opts, max_w)` (what layout gives the same `text`),
//!   `caret_in_text(s, opts, max_w, byte)` (where a byte of it lands),
//!   `extension_namespaces()`.
//! - Focus: `set_focus(key)`, `blur()`, `focus_next()`, `focus_prev()`,
//!   `focus_region(key)`, `announce(text, politeness)`.
//! - Time: `request_frame_at(at)`, a frame at a time on the frame clock
//!   (`env.now + 3` is a toast's expiry); `request_frame_for(why)`, a frame
//!   after this one, named in the recording.
//! - Exits: `exit_with(key, exit)`, the exit a node leaves by this frame.
//! - The recording (ADR 0058), each nothing while the devtools' Perf tab
//!   and `KUI_PERF_TRACE` are not listening (`perf_measuring()` says
//!   which): `span(name, fn)` runs `fn` inside a span and returns what it
//!   returned, `perf_count(name, n)` adds to the frame's counter,
//!   `perf_gauge(name, value)` sets a reading, `perf_mark(name)` puts an
//!   instant on the timeline, `perf_measure(name, ms)` a span timed
//!   elsewhere.
//! - Scrolling: `reveal(key)`, `scroll_offset(key)`, `scroll_geometry(key)`,
//!   `set_scroll(key, x, y)`, `shift_scroll(key, drawn, target)`,
//!   `edge_scroll(key, x, y, rect)`.
//! - Text and selection: `text_hit(key, x, y)`, `caret_rect(key, byte)`,
//!   `selection_text()`, `selection_html()`, `selection_ends()`,
//!   `cell_selection()`, `select_all_in(key)`, `clear_selection()`,
//!   `answer_selection_range(text)`.
//! - Clipboard and dialogs: `set_clipboard(text, html)`,
//!   `set_clipboard_secret(text)`, `request_copy()`, `request_paste()`,
//!   `awaiting_paste()`, `request_files(opts)`, `awaiting_files()`.
//! - Menus and windows: `open_menu(key, x, y, items)`, `close_menu()`,
//!   `set_window_size(window, w, h)`, `focus_window(window)`.
//! - Declarations: `set_tokens(decl)` replaces the script's token table;
//!   `add_extension(namespace, path)` loads a C plugin (see below).
//!
//! The root table may also carry host state beside its children:
//! `window_title`, `always_on_top`, `secure_input`, `option_as_alt`,
//! `ime_off`, a `windows` list of `name | { name, kind, width, height,
//! activates, anchor, on_window, on_dismiss }`, and `window_events = {
//! on_resize, on_focus, on_system_settings_change, on_fonts, on_modifiers,
//! on_open }`, the messages the window's own events arrive as (ADR 0048).
//! `window_events` on any other table is ignored, with a
//! `window-events-off-root` warning.
//!
//! ## The two focus names
//!
//! `env.focused` and `env.focus` are different facts. `focused` is a boolean,
//! whether this window has the keyboard at all. `focus` is the focused node's
//! key, nil for none. Because `focus` is a value, moving focus is
//! `env.set_focus(key)`; `blur`, `focus_next` and `focus_prev` keep the names
//! the other bindings use. `set_focus` and `blur` take effect at once;
//! `focus_next` / `focus_prev` resolve when the frame finishes, because the
//! Tab ring is built from a finished tree. `env.focus` still reads the focus
//! the frame opened with, so `env.is_focused(key)` is the query that answers
//! about now.
//!
//! ## Components and the `inst` argument
//!
//! A `components` table lists what the script provides (ADR 0060, 0067):
//! `components.panel = function(env, props, inst) … end` draws `ns/panel`
//! wherever the host places it, and `components["*"]` every name under the
//! namespace. `view(env, inst)` is the component `root`: with no
//! `components`, the host's frame draws it after its own view unless the
//! host places `ns/root` itself. `inst` says which instance is drawn:
//! `inst.name` (the component in the script's own vocabulary),
//! `inst.namespace` (what the host loaded the script under), `inst.key`
//! (the integer key events and `set_focus` use) and, for `view`,
//! `inst.params` (the host's props, nil when it passed none). A `slots`
//! global is refused at load (ADR 0067).
//!
//! ## Events
//!
//! `on_event(ev)` receives the message the handler declared, as a table
//! (`on_click = { kind = "toggle", index = i }`), with the event's data
//! in one field of it named for the event (ADR 0047): a drag declared
//! `on_drag = { kind = "pan" }` arrives as `{ kind = "pan", drag = {
//! phase, x, y, dx, dy, parent } }`, a plain click exactly as declared.
//! Beside the message the envelope says what happened and where: `event`
//! (the event kind: `"click"`, `"drag"`, `"change"`, …), `node_key` (the
//! emitting node's integer key), `window` (the window it came from; a
//! `window` event's own data is under that name, and it is the one kept)
//! `instance` (the key of the component instance the node was drawn in)
//! and `component` (that instance's full name). A message
//! that is not a table arrives as `{ tag = message }`, and none as `{}`,
//! so the envelope always has somewhere to go; a message's own field
//! under one of the envelope's names is the message's. The envelope also
//! comes alone as the second argument, `on_event(ev, at)`, which no
//! message can hide: `at.event`, `at.node_key`, `at.window`, and
//! `at.instance`, `at.component` and `at.from` where they apply.
//!
//! A node hears only the events it declares a handler for. An `edit`
//! reports through its own: `on_change` as `change = { text }` (the whole
//! text after the edit), `on_submit` (Enter) and `on_cancel` (Escape) as
//! declared, `on_boundary` as `boundary = { key, edge, word, doc, ctrl }`. Its
//! `value =` is the text the view declares every frame — the editor is
//! brought to it where they differ — so a script that stores
//! `ev.change.text` and declares it back owns the field's text; `initial`
//! only seeds a new one. The window's own events arrive the same way, as
//! the messages the root table declared for them (ADR 0048): a resize as
//! `window_events.on_resize` with `resize = { width, height, scale }`, a
//! window opening or closing as its `windows` entry's `on_window` with
//! `window = { phase, id }`, the menu bar's choice as its `on_menu` with
//! `menu = { role, item }`. What nobody declared does not arrive, and the
//! core never writes a `kind`: it is the script's own. What `on_event`
//! returns is the script's replies to the host: nothing, one table, or a
//! sequence of tables.
//!
//! ## Props
//!
//! Every row of the shared schema in [`kui_core::schema`] is reachable under
//! its snake_case name (`min_width`, `on_click`, `line_height`, ...), so Lua
//! and Node accept the same surface. Only the composites have Lua shapes:
//!
//! - `pad = 8` or `pad = { all =, x =, y =, l =, r =, t =, b = }`
//! - `border = { w = 1, color = 0x... }`
//! - `scroll`, `scroll_x`, `scroll_y`, `clip` as booleans
//! - `float = "below" | "above"` or `float = { anchor =, at =, self =, dx =,
//!   dy =, fit = }`
//! - sizing: `width = 300`, `"grow"`, `{ grow = 2 }`, `{ pct = 50 }`, or a
//!   size expression string
//! - `tooltip = "hint"` on a container (hover-gated)
//! - a colour is `0xRRGGBBAA` or a `"$token"` name
//!
//! A key no table claims is an `unknown-prop` warning the host can read.
//!
//! A number is any number. Where kui wants a whole one — a count, a
//! position, an index, a byte offset, a line: `cells`' `rows`, `cols`,
//! runs and `cursor_at`, `index`, a dropdown's `current`, `list`'s
//! `overscan`, `row_heights`, `env.caret_rect`'s byte — a fraction is
//! floored and below 0 is 0, so `rows = room_h / line_h` is the rows that
//! fit. A handle kui gave out — an `image`'s `id`, a `fragment`'s, an
//! `audio`'s `src`, a sound row, a window id — is a whole number: `3.0` is
//! handle 3, and `3.5` is refused by name, since no arithmetic makes a
//! handle (backlog F180).
//!
//! ## A script may host a plugin
//!
//! `env.add_extension(namespace, path)` loads an extension of the script's
//! own: another script (a `.lua` file, or a folder holding `init.lua`), or a
//! C extension, a `.so` / `.dylib` / `.dll` exporting the `kui_ext_*` entry
//! points kui-ffi's `kui.h` describes, which the host's crate opens for it
//! ([`set_native_loader`]; ADR 0059). `component("ns/panel", props)` places
//! what it provides among the script's children, and its emits reach
//! `on_event` with `from` set to the namespace.
//!
//! # Every host loads a script
//!
//! A `.lua` path given to any host's extension door — Node's
//! `addExtension`, C's `kui_ctx_add_extension`, Odin's
//! `ctx_add_extension`, a script's `env.add_extension` — loads as a
//! [`LuaExtension`] ([`is_lua_path`], [`LuaExtension::from_path`]).
//! [`LuaExtension::from_file`] names the chunk `@<path>` and puts the
//! file's folder first on `package.path`, so a script made of modules
//! `require`s the files beside it.
//!
//! # Where to look
//!
//! - [`LuaExtension`]: the script as an extension; [`LuaExtension::from_file`]
//!   and [`LuaExtension::from_source`] load it.
//! - [`luals_meta`]: a `---@meta` file for lua-language-server, generated
//!   from the schema.
//! - [`teal_decls`]: the same as a `kui.d.tl` for Teal, so a view written
//!   in Teal is checked by `tl check` and compiled by `tl gen` to the Lua
//!   this crate loads. Both files are committed under `packages/lua` in
//!   the repository, for an author to copy.
//! - [`lua_to_value`] / [`value_to_lua`]: the table <-> [`kui_core::Value`]
//!   conversion events and replies go through.
//! - [`parse_props`] and [`parse_tokens`]: the table readers, for a host that
//!   wants to parse a view table or a `tokens` table itself.
//! - [`MAX_VIEW_DEPTH`] / [`MAX_VALUE_DEPTH`]: the nesting limits.
//!
//! Book: <https://kui-book.qxuken.dev>. Repository: <https://github.com/qxuken/kui>
//! (the design records live under `docs/adr` there).

use kui_core::schema::{self, Kind, Parsed, PropsOut};
use kui_core::{
    Align, Color, Content, EditOptions, Extension, FloatConfig, Instance, Key, PadShorthand,
    Sizing, Span, Ui, UiEvent, Value, WindowConfig, WindowLifecycle, widgets,
};
use mlua::{Lua, Table};

mod channel;
mod meta;
mod rows;
mod teal;
pub use meta::luals_meta;
pub use teal::teal_decls;

const PRELUDE: &str = include_str!("prelude.lua");

/// What `env.add_extension` opens a path that is not Lua with: a C
/// library, in practice, by `kui_ffi`'s loader. kui-lua does not depend on
/// kui-ffi — kui-ffi depends on kui-lua, so every host that loads
/// extensions loads `.lua` ones (ADR 0059) — so the loader is handed in
/// by whichever crate links both: [`set_native_loader`].
pub type NativeLoader = fn(&std::path::Path) -> Result<Box<dyn Extension>, String>;

static NATIVE_LOADER: std::sync::OnceLock<NativeLoader> = std::sync::OnceLock::new();

/// Registers what a script's `env.add_extension` loads a non-Lua path
/// with. The first registration wins; later ones are ignored, since every
/// caller registers the same loader. `kui_ffi::open_extension` registers
/// kui-ffi's on its first call, and `kui_ffi::register_native_loader` does
/// it for a Rust host that builds [`LuaExtension`]s itself. With none
/// registered, a script asking for a C plugin is told so, by its path.
pub fn set_native_loader(loader: NativeLoader) {
    let _ = NATIVE_LOADER.set(loader);
}

/// Whether `path` names a Lua extension: a file ending in `.lua`, or a
/// directory holding `init.lua` (ADR 0059). Every host's one door loads
/// such a path as a [`LuaExtension`] and anything else as a C library.
pub fn is_lua_path(path: &std::path::Path) -> bool {
    path.extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("lua"))
        || (path.is_dir() && path.join("init.lua").is_file())
}

/// A Lua script loaded as a kui [`Extension`].
///
/// Load it with [`LuaExtension::from_file`] or [`LuaExtension::from_source`],
/// then hand it to a host: `kui_native::app(..).extension_as(ns, ext)` for a
/// window, or a [`kui_core::Extensions`] list for a headless
/// [`kui_core::Core`]. The host places the script's component `ns/<name>`;
/// the script's function runs inside the host's frame and its
/// `on_event(ev)` hears the events its own nodes emit.
///
/// ```
/// use kui_core::{Core, Extensions, NodeSpec, OriginId, Size, Value};
/// use kui_lua::LuaExtension;
///
/// let script = LuaExtension::from_source("panel.lua", r#"
///     components = {
///       panel = function(env, props)
///         return column { pad = 8, text(props.title) }
///       end,
///     }
///     function on_event(ev)
///       if ev.kind == "pick" then return emit("on_pick", {}) end
///     end
/// "#)?;
///
/// let mut exts = Extensions::new();
/// exts.push_as("fs", Box::new(script))?;
///
/// let mut core = Core::new();
/// let mut ui = core.frame_with(Size::new(400.0, 300.0), 1.0, &mut exts);
/// ui.configure_root(NodeSpec::row().fill());
/// ui.component("fs/panel", &Value::map([("title", "files".into())]));
/// ui.finish();
///
/// // The script's nodes carry its origin, so the host can tell them apart.
/// let drawn = core.access_tree().nodes.iter().filter(|n| n.origin == OriginId(1)).count();
/// assert!(drawn > 0);
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub struct LuaExtension {
    lua: Lua,
    name: String,
    /// The C extensions this script loaded (`env.add_extension`), in the
    /// order it asked for them. A `RefCell` because the loading happens
    /// inside `view`, where the script's own interpreter holds a shared
    /// borrow of everything else here.
    loaded: std::rc::Rc<std::cell::RefCell<Vec<Loaded>>>,
    /// The script's `env`, built once and shared by every fill (backlog
    /// C65): its functions reach the frame being drawn through
    /// [`CURRENT_FRAME`], and its facts are written again only when a fill
    /// reads them different from the last.
    env: std::cell::RefCell<Option<EnvCache>>,
    /// The full name of every instance this script has drawn, by its key
    /// — what `on_event` reads `ev.component` off, since an event carries the
    /// key and a script drawing one instance per pane routes by the name
    /// it was handed.
    instance_names: std::cell::RefCell<std::collections::HashMap<kui_core::Key, String>>,
    /// The script's `components` table's names, read once at load: what
    /// it provides as components (ADR 0060), each a function `(env,
    /// props, inst)` returning the instance's tree.
    components: Vec<String>,
    /// The script's `contributes` global, read once at load: the
    /// components it adds to contribution slots (ADR 0067).
    contributions: Vec<kui_core::Contribution>,
    /// The folder a file-loaded script came from, which an `svg`'s
    /// relative `src` is read against (ADR 0063); `None` for a script
    /// loaded from a string, whose paths are the process's.
    dir: Option<std::rc::Rc<str>>,
    /// The `tokens = { colors = …, lengths = …, families = … }` global the script
    /// declared at load, declared into the core under this
    /// extension's origin on the first `view` that finds none there;
    /// `env.set_tokens` replaces it from inside a view.
    tokens: Option<kui_core::Tokens>,
}

/// One plugin a script loaded: the namespace it chose, where it came
/// from, and the origin the frame's list gave it.
struct Loaded {
    namespace: String,
    path: std::path::PathBuf,
    origin: kui_core::OriginId,
}

impl LuaExtension {
    /// Loads a script from its source text; `name` is what errors call it.
    ///
    /// The prelude is injected first, then the script runs once, and its
    /// `components` and `tokens` globals are read. A script with no
    /// `components` provides `"root"`, its `view`.
    ///
    /// ```
    /// use kui_core::{Core, Extension, Size, Instance};
    /// use kui_lua::LuaExtension;
    ///
    /// let mut ext = LuaExtension::from_source("hello.lua", r#"
    ///     function view(env)
    ///       return column { pad = 8, text("hello from Lua") }
    ///     end
    /// "#)?;
    /// assert!(ext.components().is_empty());
    ///
    /// let mut core = Core::new();
    /// let mut ui = core.frame(Size::new(320.0, 240.0), 1.0);
    /// ext.view(&Instance::root(), &mut ui)?;
    /// ui.finish();
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn from_source(name: impl Into<String>, source: &str) -> mlua::Result<Self> {
        let name = name.into();
        let chunk = name.clone();
        Self::load(name, &chunk, source, None, None)
    }

    /// [`Self::from_source`], with `setup` run on the script's interpreter
    /// after the prelude and before the script: how a host gives its
    /// scripts modules of its own (the `kui` runner's `kui.*`, ADR 0064)
    /// or preloads the ones they `require`.
    pub fn from_source_with(
        name: impl Into<String>,
        source: &str,
        mut setup: impl FnMut(&Lua) -> mlua::Result<()>,
    ) -> mlua::Result<Self> {
        let name = name.into();
        let chunk = name.clone();
        Self::load(name, &chunk, source, None, Some(&mut setup))
    }

    /// The script loaded under `name`, its chunk called `chunk`, with `dir`
    /// first on `package.path` when it came from a file and `setup` run
    /// before it.
    fn load(
        name: String,
        chunk: &str,
        source: &str,
        dir: Option<&std::path::Path>,
        setup: Option<&mut dyn FnMut(&Lua) -> mlua::Result<()>>,
    ) -> mlua::Result<Self> {
        let lua = Lua::new();
        rows::register(&lua)?;
        channel::register(&lua)?;
        lua.load(PRELUDE).set_name("kui:prelude").exec()?;
        if let Some(dir) = dir {
            // `dir/?.lua;dir/?/init.lua;` ahead of the default path, so a
            // script made of modules `require`s the files beside it. A
            // script has no `debug` library to find its own folder with.
            let package: Table = lua.globals().get("package")?;
            let rest: String = package.get("path")?;
            let d = dir.to_string_lossy();
            package.set("path", format!("{d}/?.lua;{d}/?/init.lua;{rest}"))?;
            // What `env.load_tokens` resolves a relative path against.
            lua.set_named_registry_value("kui:dir", d.as_ref())?;
        }
        if let Some(setup) = setup {
            setup(&lua)?;
        }
        lua.load(source).set_name(chunk).exec()?;
        if !lua.globals().get::<mlua::Value>("slots")?.is_nil() {
            return Err(mlua::Error::runtime(
                "`slots` is gone (ADR 0067): list what this script provides as `components = { \
                 panel = function(env, props, inst) … end }` (`[\"*\"]` for any name), and draw \
                 `view` as its root",
            ));
        }
        let components = match lua.globals().get::<mlua::Value>("components")? {
            mlua::Value::Nil => Vec::new(),
            mlua::Value::Table(t) => {
                let mut names = Vec::new();
                for pair in t.pairs::<String, mlua::Value>() {
                    let (name, f) = pair?;
                    if !matches!(f, mlua::Value::Function(_)) {
                        return Err(mlua::Error::runtime(format!(
                            "`components.{name}` must be a function (env, props, inst) returning \
                             the instance's tree, not {}",
                            f.type_name()
                        )));
                    }
                    names.push(name);
                }
                names.sort();
                names
            }
            other => {
                return Err(mlua::Error::runtime(format!(
                    "`components` must be a table of functions by name, not {}",
                    other.type_name()
                )));
            }
        };
        let contributions = parse_contributions(&lua.globals().get::<mlua::Value>("contributes")?)?;
        let tokens = match lua.globals().get::<mlua::Value>("tokens")? {
            mlua::Value::Nil => None,
            mlua::Value::Table(t) => Some(parse_tokens(&t)?),
            other => {
                return Err(mlua::Error::runtime(format!(
                    "`tokens` must be a table {{ colors = …, lengths = …, families = … }}, not {}",
                    other.type_name()
                )));
            }
        };
        Ok(Self {
            lua,
            name,
            loaded: Default::default(),
            env: Default::default(),
            instance_names: Default::default(),
            components,
            contributions,
            dir: dir.map(|d| d.to_string_lossy().into()),
            tokens,
        })
    }

    /// The namespace an emit's origin names, for a plugin this script
    /// loaded: what `on_event` puts on the event as `from`.
    fn loaded_as(&self, origin: kui_core::OriginId) -> Option<String> {
        self.loaded
            .borrow()
            .iter()
            .find(|l| l.origin == origin)
            .map(|l| l.namespace.clone())
    }

    /// The script's own interpreter, for a host that wants to seed a global
    /// the script reads: the escape hatch for facts that are neither `env`
    /// nor events.
    pub fn lua(&self) -> &Lua {
        &self.lua
    }

    /// Loads a script from a file; its file name becomes the script's name.
    ///
    /// The chunk is named `@<path>`, so an error names the file, and the
    /// file's directory goes first on `package.path`, so the script's
    /// `require "skin"` finds `skin.lua` beside it (ADR 0059).
    pub fn from_file(path: impl AsRef<std::path::Path>) -> mlua::Result<Self> {
        Self::file(path.as_ref(), None)
    }

    /// [`Self::from_file`], with `setup` run before the script as
    /// [`Self::from_source_with`] runs it.
    pub fn from_file_with(
        path: impl AsRef<std::path::Path>,
        mut setup: impl FnMut(&Lua) -> mlua::Result<()>,
    ) -> mlua::Result<Self> {
        Self::file(path.as_ref(), Some(&mut setup))
    }

    fn file(
        path: &std::path::Path,
        setup: Option<&mut dyn FnMut(&Lua) -> mlua::Result<()>>,
    ) -> mlua::Result<Self> {
        let source = std::fs::read_to_string(path)
            .map_err(|e| mlua::Error::runtime(format!("{}: {e}", path.display())))?;
        let name = path
            .file_name()
            .map_or_else(|| "lua".into(), |n| n.to_string_lossy().into_owned());
        let dir = path.parent().filter(|d| !d.as_os_str().is_empty());
        let dir = dir.unwrap_or(std::path::Path::new("."));
        Self::load(
            name,
            &format!("@{}", path.display()),
            &source,
            Some(dir),
            setup,
        )
    }

    /// Loads a script from a `.lua` file, or from a directory holding
    /// `init.lua` — named for the directory, its modules beside `init.lua`.
    /// What every host's door does with a path [`is_lua_path`] accepts.
    pub fn from_path(path: impl AsRef<std::path::Path>) -> mlua::Result<Self> {
        let path = path.as_ref();
        if path.is_dir() {
            let mut ext = Self::from_file(path.join("init.lua"))?;
            if let Some(n) = path.file_name() {
                ext.name = n.to_string_lossy().into_owned();
            }
            Ok(ext)
        } else {
            Self::from_file(path)
        }
    }
}

impl Extension for LuaExtension {
    fn name(&self) -> &str {
        &self.name
    }

    fn view(&mut self, slot: &Instance<'_>, ui: &mut Ui<'_>) -> Result<(), String> {
        let view: mlua::Function = self
            .lua
            .globals()
            .get("view")
            .map_err(|_| "script defines no view()".to_string())?;
        self.draw(&view, slot, false, ui)
    }

    fn components(&self) -> &[String] {
        &self.components
    }

    fn contributions(&self) -> &[kui_core::Contribution] {
        &self.contributions
    }

    fn component(&mut self, inst: &Instance<'_>, ui: &mut Ui<'_>) -> Result<(), String> {
        self.draw_component(inst, ui)
    }

    /// The same interpreter under a second handle: a component function
    /// has returned before its tree is built, so a host's children placing
    /// this script's components while it builds call into an idle state.
    /// The handle keeps an env table of its own (the functions reach the
    /// frame through [`CURRENT_FRAME`] either way); its events come back
    /// to this one, whose origin they carry.
    fn reentry(&mut self) -> Option<Box<dyn Extension>> {
        Some(Box::new(LuaExtension {
            lua: self.lua.clone(),
            name: self.name.clone(),
            loaded: self.loaded.clone(),
            env: std::cell::RefCell::new(None),
            instance_names: Default::default(),
            components: self.components.clone(),
            contributions: Vec::new(),
            dir: self.dir.clone(),
            tokens: None,
        }))
    }

    fn on_event(&mut self, ev: &UiEvent) -> Vec<Value> {
        let Ok(f) = self.lua.globals().get::<mlua::Function>("on_event") else {
            return Vec::new();
        };
        let payload = event_payload(&self.lua, &ev.payload).and_then(|t| {
            // The envelope rides beside the message (ADR 0047): `event`
            // says what happened — the payload no longer does, since a
            // plain click and a submit can both arrive as the bare
            // message — and the rest say where. A message with a field of
            // the same name keeps its own.
            if !t.contains_key("event")? {
                t.set("event", ev.event.name())?;
            }
            // The node that emitted it: what `env.edit_text`, `set_focus`
            // and the rest take.
            if !t.contains_key("node_key")? {
                t.set("node_key", ev.key.0 as i64)?;
                // And which window it came from — the number `env.window.id`
                // reads in that window's view, as Node's `ev.window` and
                // C's `KuiEvent.window` carry (AR26: a panel drawn into
                // two windows could not tell which one clicked). A
                // `window` event's data is under the same name (ADR
                // 0048), and the data is the message's, so it wins.
                if !t.contains_key("window")? {
                    t.set("window", ev.window.0)?;
                }
                // And the instance the node was drawn in: its key, what
                // its fill was handed as `inst.key` (ADR 0060), and its
                // full name, so a script drawing one instance per pane
                // routes by pane without stamping every payload (backlog
                // K2). Absent for a node outside any fill.
                if let Some(k) = ev.instance {
                    t.set("instance", k.0 as i64)?;
                    if let Some(name) = self.instance_names.borrow().get(&k) {
                        t.set("component", name.as_str())?;
                    }
                }
                // And, when this is an emit from a plugin the script
                // loaded, who raised it: the namespace it chose in
                // `env.add_extension`. Absent for the script's own nodes,
                // which is what tells the two apart.
                if let Some(ns) = ev.from.clone().or_else(|| self.loaded_as(ev.origin)) {
                    t.set("from", ns)?;
                }
            }
            Ok(mlua::Value::Table(t))
        });
        // What `on_event` returns is the script's emits (ADR 0067):
        // nothing, one `emit(...)`, or a sequence of them — the list-or-map
        // reading `lua_to_value` already makes. Anything else is dropped by
        // the router.
        // The envelope again, whole, as the second argument: the fields
        // merged into the message above give way to the message's own, so
        // a message with an `event` or a `node_key` field of its own hid
        // what happened and where (the alpha.60 pre-tag pass, backlog
        // RG157). A script that names one argument never sees it.
        let at = self.envelope(ev);
        match payload
            .and_then(|p| Ok((p, at?)))
            .and_then(|(p, at)| f.call::<mlua::Value>((p, at)))
        {
            Ok(mlua::Value::Nil) => Vec::new(),
            Ok(v) => match lua_to_value(&v) {
                Ok(Value::List(emits)) => emits,
                Ok(emit) => vec![emit],
                Err(e) => {
                    eprintln!("kui-lua: '{}' on_event answer: {e}", self.name);
                    Vec::new()
                }
            },
            Err(e) => {
                eprintln!("kui-lua: '{}' on_event error: {e}", self.name);
                Vec::new()
            }
        }
    }
}

impl LuaExtension {
    /// The script's `env` for a fill: built the first time, its facts,
    /// palette, sizes and tokens written again when they read differently
    /// from the last fill's — read every fill, so a fill being kept notes
    /// them as it always did (ADR 0045).
    fn env_for(&self, ui: &mut Ui<'_>) -> mlua::Result<Table> {
        let facts = ui.env_facts();
        let theme = ui.theme();
        let metrics = ui.metrics();
        let tokens_rev = ui.core().tokens_rev();
        let mut cache = self.env.borrow_mut();
        let c = match cache.as_mut() {
            Some(c) => c,
            None => {
                let table = env_table(&self.lua, FrameUi, self.loaded.clone())?;
                cache.insert(EnvCache {
                    table,
                    facts: None,
                    theme: None,
                    metrics: None,
                    tokens_rev: u64::MAX,
                })
            }
        };
        if c.facts.as_ref() != Some(&facts) {
            env_facts_into(&self.lua, &c.table, &facts)?;
            c.facts = Some(facts);
        }
        if c.theme != Some(theme) {
            env_theme_into(&self.lua, &c.table, &theme)?;
            c.theme = Some(theme);
            // Colour tokens resolve against the palette's base.
            c.tokens_rev = u64::MAX;
        }
        if c.metrics != Some(metrics) {
            env_metrics_into(&self.lua, &c.table, &metrics)?;
            c.metrics = Some(metrics);
        }
        if c.tokens_rev != tokens_rev {
            c.table.set("tokens", tokens_table(&self.lua, ui)?)?;
            c.tokens_rev = tokens_rev;
        }
        Ok(c.table.clone())
    }

    /// Draws one component instance: the script's `components[name]`
    /// called as `(env, props, inst)` and its tree built (ADR 0060).
    fn draw_component(&self, inst: &Instance<'_>, ui: &mut Ui<'_>) -> Result<(), String> {
        // The function by its name, or the wildcard's for any name.
        let f: mlua::Function = self
            .lua
            .globals()
            .get::<Table>("components")
            .and_then(|t| match t.get::<Option<mlua::Function>>(inst.name)? {
                Some(f) => Ok(f),
                None => t.get(kui_core::ANY_COMPONENT),
            })
            .map_err(|_| format!("script defines no components.{}", inst.name))?;
        self.draw(&f, inst, true, ui)
    }

    /// A view or a component's function called and its tree built: the
    /// instance's table, the script's tokens declared under its origin once,
    /// the env scoped to the call, and — for a view — the host state the
    /// root may declare beside the tree.
    fn draw(
        &self,
        view: &mlua::Function,
        slot: &Instance<'_>,
        component: bool,
        ui: &mut Ui<'_>,
    ) -> Result<(), String> {
        self.instance_names
            .borrow_mut()
            .entry(slot.key)
            .or_insert_with(|| slot.full_name());
        // Which instance this is, as an argument rather than a field of
        // `env`: `env`'s value keys are pinned to `schema::ENV_FIELDS`
        // across every binding, and an instance is the host's fact, not
        // the driver's. A script written as `view(env)` never sees it.
        let inst_table = instance_table(&self.lua, slot).map_err(|e| format!("instance: {e}"))?;
        // The table the script was loaded with, declared once per core
        // under this origin — a script that declares from `view` through
        // `env.set_tokens` has already, and is not overwritten.
        let origin = ui.core().origin();
        if let Some(t) = &self.tokens
            && !ui.core().tokens_declared(origin)
        {
            ui.set_tokens(t.clone());
        }
        // The env is the script's, built once; its facts are read again
        // for this fill (and noted, for a fill being kept), and its
        // functions reach this frame while the call runs.
        let env = self.env_for(ui).map_err(|e| format!("env: {e}"))?;
        let root: Table = {
            with_frame(ui, || -> mlua::Result<Table> {
                if component {
                    // `(env, props, inst)`: the props the host passed,
                    // and the instance — its name, namespace and key.
                    let props = match slot.params {
                        Value::Null => self.lua.create_table()?,
                        p => match value_to_lua(&self.lua, p)? {
                            mlua::Value::Table(t) => t,
                            _ => self.lua.create_table()?,
                        },
                    };
                    inst_table.raw_remove("params")?;
                    view.call((env, props, inst_table))
                } else {
                    view.call((env, inst_table))
                }
            })
            .map_err(|e| {
                if component {
                    format!("component {}(): {e}", slot.name)
                } else {
                    format!("view(): {e}")
                }
            })?
        };
        if component {
            return self.build_root(&root, slot, ui);
        }
        // The root table may declare host state alongside the tree.
        if let Ok(Some(title)) = root.get::<Option<String>>("window_title") {
            ui.window_title(&title);
        }
        if let Ok(Some(true)) = root.get::<Option<bool>>("always_on_top") {
            ui.always_on_top(true);
        }
        // Secure keyboard entry, the same shape (backlog F85): a script at
        // a password prompt declares it on every view the prompt is up.
        if let Ok(Some(true)) = root.get::<Option<bool>>("secure_input") {
            ui.secure_input(true);
        }
        // Which Option keys are Alt on macOS (backlog F113), by name; a
        // name kui does not have is the script's mistake, said as one.
        // `"none"` declares nothing, as a root that leaves it out and as
        // Node's `optionAsAlt: 'none'` (`configure_root_from`): a script
        // writing its setting through does not take back the side a host
        // or another component declared this frame (backlog RG84).
        if let Some(name) = root
            .get::<Option<String>>("option_as_alt")
            .map_err(|e| format!("option_as_alt: {e}"))?
        {
            let v = kui_core::OptionAsAlt::from_name(&name).ok_or_else(|| {
                format!(
                    "option_as_alt: expected \"none\", \"left\", \"right\" or \"both\", got {name:?}"
                )
            })?;
            if v != kui_core::OptionAsAlt::None {
                ui.option_as_alt(v);
            }
        }
        // The input method off (backlog F125), the shape `secure_input`
        // has: a modal editor's normal mode declares it on every view.
        if let Ok(Some(true)) = root.get::<Option<bool>>("ime_off") {
            ui.ime_off(true);
        }
        declare_windows(ui, &root).map_err(|e| format!("windows: {e}"))?;
        // The messages the window's own events arrive as (ADR 0048),
        // declared here like the title. The walk under `build_node` reads
        // the key on every table, so a nested one reaches the core and is
        // `window-events-off-root`; `VIEW_ROOT` tells it this one is not.
        let events = root
            .get::<mlua::Value>("window_events")
            .map_err(|e| format!("window_events: {e}"))?;
        if !events.is_nil() {
            ui.window_events(window_events_of(&events).map_err(|e| format!("window_events: {e}"))?);
        }
        self.build_root(&root, slot, ui)
    }

    /// Builds a view's or an instance's tree, with what the build needs of
    /// the script at hand: its folder (a relative `svg` `src`) and itself
    /// under its namespace (a component of its own, ADR 0060).
    fn build_root(&self, root: &Table, slot: &Instance<'_>, ui: &mut Ui<'_>) -> Result<(), String> {
        let outer = VIEW_ROOT.replace(root.to_pointer());
        let outer_dir = BUILD_DIR.replace(self.dir.clone());
        let outer_ext = BUILD_EXT.replace(Some(BuildExt {
            ext: self as *const LuaExtension,
            namespace: slot.namespace.as_ptr(),
            namespace_len: slot.namespace.len(),
        }));
        let built = build_node(ui, root).map_err(|e| format!("view table: {e}"));
        BUILD_EXT.set(outer_ext);
        BUILD_DIR.set(outer_dir);
        VIEW_ROOT.set(outer);
        built
    }
}

impl LuaExtension {
    /// `on_event`'s second argument: the envelope alone, nothing of the
    /// message in it — `event`, `node_key`, `window`, and `instance`,
    /// `component` and `from` where they apply, as the first argument
    /// carries them when the message leaves them free.
    fn envelope(&self, ev: &UiEvent) -> mlua::Result<Table> {
        let t = self.lua.create_table()?;
        t.set("event", ev.event.name())?;
        t.set("node_key", ev.key.0 as i64)?;
        t.set("window", ev.window.0)?;
        if let Some(k) = ev.instance {
            t.set("instance", k.0 as i64)?;
            if let Some(name) = self.instance_names.borrow().get(&k) {
                t.set("component", name.as_str())?;
            }
        }
        if let Some(ns) = ev.from.clone().or_else(|| self.loaded_as(ev.origin)) {
            t.set("from", ns)?;
        }
        Ok(t)
    }
}

/// The table `on_event` is handed for a payload: the message as a table,
/// so the envelope (`event`, `node_key`, …) has somewhere to go. Every
/// payload the core builds is a map (ADR 0047, decision 3): a bare
/// `on_click = "mute"` arrives as `{ tag = "mute" }`, and a stock input's
/// submit, whose message is `Null`, as `{}`.
fn event_payload(lua: &Lua, payload: &Value) -> mlua::Result<Table> {
    match value_to_lua(lua, payload)? {
        mlua::Value::Table(t) => Ok(t),
        other => Err(mlua::Error::runtime(format!(
            "an event's payload is a map, not {}",
            other.type_name()
        ))),
    }
}

/// The `contributes` global (ADR 0067): `{ ["app.toolbar"] = "save" }`,
/// a slot to one component or a list of them, or a list of `{ slot =,
/// component =, order = }` — what the script adds to contribution slots.
fn parse_contributions(v: &mlua::Value) -> mlua::Result<Vec<kui_core::Contribution>> {
    let bad = |what: String| {
        mlua::Error::runtime(format!(
            "`contributes` {what}: write `{{ [\"app.toolbar\"] = \"save\" }}` or `{{ {{ slot = \
             \"app.toolbar\", component = \"save\", order = 10 }} }}`"
        ))
    };
    let t = match v {
        mlua::Value::Nil => return Ok(Vec::new()),
        mlua::Value::Table(t) => t,
        other => return Err(bad(format!("is a {}", other.type_name()))),
    };
    let mut out = Vec::new();
    if t.raw_len() > 0 {
        for entry in t.sequence_values::<Table>() {
            let e = entry.map_err(|_| bad("has an entry that is not a table".into()))?;
            let slot: Option<String> = e.get("slot")?;
            let component: Option<String> = e.get("component")?;
            let (Some(slot), Some(component)) = (slot, component) else {
                return Err(bad("has an entry without its slot and component".into()));
            };
            let order: Option<i64> = e.get("order")?;
            out.push(kui_core::Contribution::new(slot, component).order(order.unwrap_or(0) as i32));
        }
        return Ok(out);
    }
    for pair in t.pairs::<String, mlua::Value>() {
        let (slot, v) = pair.map_err(|_| bad("has a key that is not a slot name".into()))?;
        match v {
            mlua::Value::String(c) => {
                out.push(kui_core::Contribution::new(&slot, c.to_str()?.to_string()))
            }
            mlua::Value::Table(list) => {
                for c in list.sequence_values::<String>() {
                    out.push(kui_core::Contribution::new(&slot, c?));
                }
            }
            other => return Err(bad(format!("names {} for `{slot}`", other.type_name()))),
        }
    }
    // A table's pairs come in no order; load order within one script is
    // by slot, then as listed.
    out.sort_by(|a, b| a.slot.cmp(&b.slot));
    Ok(out)
}

/// `view`'s second argument and a component's third: `{ name = ...,
/// namespace = ..., params = ..., key = ... }` — the component in the
/// script's own vocabulary (`root` for `view`), the namespace the host
/// loaded the script under (what tells one instance from another), `params`
/// (`view`'s props, absent when the host passed none; a component gets
/// them as its own argument), and the instance's key as the integer the
/// events and `env.set_focus` use.
fn instance_table(lua: &Lua, slot: &Instance<'_>) -> mlua::Result<Table> {
    let t = lua.create_table()?;
    t.set("name", slot.name)?;
    t.set("namespace", slot.namespace)?;
    t.set("key", slot.key.0 as i64)?;
    if let Some(s) = slot.slot {
        t.set("slot", s)?;
    }
    if !matches!(slot.params, Value::Null) {
        t.set("params", value_to_lua(lua, slot.params)?)?;
    }
    Ok(t)
}

/// A node named either way a script can: the integer key an event carried,
/// or the string label its `key` field declared, resolved through the
/// frame so far and then the last finished one (`Ui::key_of`). A string no
/// node declared is an error naming both spellings, since nothing else
/// would — see [`key_query`] for the calls that answer instead.
fn key_arg(ui: &mut Ui<'_>, v: mlua::Value) -> mlua::Result<Key> {
    match v {
        mlua::Value::Integer(i) => Ok(Key(i as u64)),
        mlua::Value::String(s) => {
            let label = s.to_str()?;
            ui.key_of(&label).ok_or_else(|| {
                mlua::Error::runtime(format!(
                    "no node is keyed {:?}: pass the label a `key` field declared in this or the last \
                     frame, or the integer key an event carried",
                    &*label
                ))
            })
        }
        other => Err(mlua::Error::runtime(format!(
            "a node key is an integer or a declared label, not {}",
            other.type_name()
        ))),
    }
}

/// The two spellings for a *query* — `is_hovered`, `scroll_geometry`,
/// `edit_text` and the rest — where a name nothing declared is the answer
/// rather than an error. Every one of them already has a "no such node"
/// reply for a key no layout resolved (false, nil, a zero offset), and a
/// label is the spelling a view uses *before* the node exists: the first
/// frame of a `uniform_list` asks its own container for geometry that is
/// not there yet. The command verbs ([`key_arg`]) keep throwing, where a
/// typo is a bug worth naming. What a key may *be* is the
/// same question for both, so anything that is not an integer or a string
/// is refused here too.
fn key_query(ui: &mut Ui<'_>, v: mlua::Value) -> mlua::Result<Option<Key>> {
    match v {
        mlua::Value::Integer(i) => Ok(Some(Key(i as u64))),
        mlua::Value::String(s) => Ok(ui.key_of(&s.to_str()?)),
        other => Err(mlua::Error::runtime(format!(
            "a node key is an integer or a declared label, not {}",
            other.type_name()
        ))),
    }
}

/// A Lua sequence as a plain-data list — `lua_to_value` reads an empty
/// table as an empty map, and a list of rows is a list even when empty.
fn lua_list_to_value(t: &Table) -> mlua::Result<Value> {
    let mut items = Vec::with_capacity(t.raw_len());
    for item in t.sequence_values::<mlua::Value>() {
        items.push(lua_to_value(&item?)?);
    }
    Ok(Value::List(items))
}

/// The snake spellings a script may have learned first — `select_all`,
/// `look_up` — kept as aliases of the wire names in a row's `role`, the
/// way `direction` is one for `repeat`. The rest
/// of a row is the core's call (`MenuItem::from_value`).
fn alias_menu_role(row: &mut Value) {
    let Value::Map(fields) = row else { return };
    for (k, v) in fields.iter_mut() {
        if k == "role"
            && let Value::Str(name) = v
        {
            match name.as_str() {
                "select_all" => *name = kui_core::MenuRole::SelectAll.name().to_string(),
                "look_up" => *name = kui_core::MenuRole::LookUp.name().to_string(),
                _ => {}
            }
        }
        // A submenu's rows are rows too (backlog F128).
        if k == "items"
            && let Value::List(rows) = v
        {
            rows.iter_mut().for_each(alias_menu_role);
        }
    }
}

/// A menu's rows, from a Lua list of row tables, read by the core's one
/// row reader, with the keys of them no row reads (`MenuItem::stray_keys`).
fn menu_items(t: &mlua::Table) -> mlua::Result<(Vec<kui_core::MenuItem>, Vec<String>)> {
    let mut rows = lua_list_to_value(t)?;
    if let Value::List(rows) = &mut rows {
        rows.iter_mut().for_each(alias_menu_role);
    }
    let items = kui_core::MenuItem::list_from_value(&rows).map_err(mlua::Error::runtime)?;
    Ok((items, kui_core::MenuItem::stray_keys(&rows)))
}

/// Raises `unknown_menu_item_key` for each of `keys`, as a dropped prop is.
fn warn_stray_menu_keys(core: &mut kui_core::Core, keys: Vec<String>) {
    if core.diagnostics() {
        for k in keys {
            core.warn(kui_core::diag::unknown_menu_item_key(&k));
        }
    }
}

/// Host facts handed to `view(env)`, the reading `schema::ENV_FIELDS`
/// documents and `the_env_table_is_the_documented_env_shape` pins to it key
/// for key: `refresh_hz` (nil if unknown),
/// `frame_budget_ms`, `focused` (the *window*'s keyboard focus, a bool),
/// `system` (what the user set in the OS: `appearance`, `motion` and
/// `assistive` as strings, always there because "unknown" is one of their
/// readings, and `accent` (0xRRGGBBAA) / `locale` (a BCP-47 tag) only when
/// the host can tell), `focus` (the focused *node*'s key), `focus_visible`,
/// `caret_visible` (the blink phase a custom editor draws its caret on), `region`
/// (the `focus_region` node in effect, nil for the main ring), `theme`
/// (the palette derived from `system`: one 0xRRGGBBAA number per role in
/// `schema::THEME_ROLES`, plus `appearance` and `disabled_opacity`),
/// `viewport_w`/`viewport_h` (logical px), `window` chrome facts, the
/// queries `edit_text(key)`, `is_focused(key)`, `is_hovered(key)`,
/// `is_pressed(key)`, `scroll_offset(key)`, `scroll_geometry(key)`,
/// `text_hit(key, x, y)` and `caret_rect(key, byte)` (each takes either
/// spelling — the integer key an event carried or the label a `key` field
/// declared — and answers nil/false/zero for a name no frame declared, see
/// `key_query`; the verbs take the same two and refuse an undeclared name,
/// see `key_arg`), `label_of(key)` (the way back: the label a key was
/// opened under, nil for an auto-keyed node), the editor verb `set_edit_text(key_or_label, text)` (whose
/// label spelling reaches an editor this view is about to declare),
/// `measure_text(s, opts, max_w)` (see `measure_from_lua`),
/// `caret_in_text(s, opts, max_w, byte)` (see `caret_in_from_lua`), the
/// focus verbs `set_focus(key)` / `blur()` / `focus_next()` / `focus_prev()` / `focus_region(key)`,
/// `request_frame_at(at)` (a frame at a time on the frame clock, `now`),
/// `exit_with(key, exit)` (the exit a node leaves by this frame), the
/// recording's `span(name, fn)` / `perf_count(name, n)` / `perf_gauge(name,
/// value)` / `perf_mark(name)` / `perf_measure(name, ms)` /
/// `perf_measuring()` and `request_frame_for(why)` (ADR 0058)
/// and the scroll calls `reveal(key)` / `scroll_offset(key)` / `set_scroll(key, x, y)` /
/// `shift_scroll(key, drawn, target)` / `edge_scroll(key, x, y, rect)` /
/// `scroll_geometry(key)`, the text queries `text_hit(key, x, y)` /
/// `caret_rect(key, byte)`, the selection calls `selection_text()` /
/// `selection_html()` (the same words with the formatting they declared) /
/// `selection_ends()` (the anchor and the focus as row indices and
/// bytes) / `cell_selection()` (a grid's, as absolute lines and
/// columns) /
/// `request_copy()` + `answer_selection_range(text)` (a copy that reaches
/// rows a virtual list never built is asked of the app) /
/// `set_clipboard(text, html?)` + `request_paste()` (a key sink's own
/// Ctrl-c and Ctrl-v; the paste comes back as a `text` event, `ev.text =
/// { text, pasted = true }`, one ask at a time, with `concealed = true` /
/// `transient = true` in it where the pasteboard marked it so) + `awaiting_paste()` (whether one is
/// unanswered) + `set_clipboard_secret(text)` (a secret the host writes
/// marked concealed and transient) /
/// `select_all_in(key)` / `clear_selection()` (one selection
/// per window, a `selectable` scope's or the focused editor's), the menu
/// verbs `open_menu(key, x, y, items)` / `close_menu()` (whose chosen row
/// comes back as a `menu` event on that node, in its `on_menu` message), the
/// window requests `set_window_size(window,
/// w, h)` / `focus_window(window)`, and the two calls of a script that
/// hosts a plugin of its own: `add_extension(namespace, path)` and
/// `extension_namespaces()`.
/// The script's `env`: its functions, built once (backlog C65). Each reaches
/// the frame being drawn through [`FrameUi`], so `env` is shared by every
/// fill of the script and a function called outside one is an error rather
/// than a dangling borrow. The facts, the palette, the sizes and the tokens
/// are written by [`LuaExtension::env_for`] as they change.
/// Writes the facts of a fill into `env`, one row of `schema::ENV_FIELDS`
/// at a time, read by the row's own getter and filed under its snake path
/// (`system.appearance` is `env.system.appearance`). Lua's rule for a fact
/// the host cannot tell is `refresh_hz`'s: no key rather than a nil-shaped
/// one, so a `Null` reading is left out — and taken out, when a fact a
/// fill had goes. Two facts, one letter apart, are both here: `focused` is
/// the *window*'s keyboard, `focus` the focused *node*'s key — Lua cannot
/// converge on Node's `focused()` for the latter because the former has
/// been `focused` since env existed.
///
/// The one deliberate divergence the table names: `window.native_controls`
/// is a rect elsewhere and two numbers here, the keep-out extent of the
/// OS-drawn controls (macOS traffic lights) at the window origin.
fn env_facts_into(lua: &Lua, t: &Table, facts: &kui_core::schema::EnvFacts) -> mlua::Result<()> {
    let sub = |head: &str| -> mlua::Result<Table> {
        match t.raw_get::<Option<Table>>(head)? {
            Some(s) => Ok(s),
            None => {
                let s = lua.create_table()?;
                t.raw_set(head, &s)?;
                Ok(s)
            }
        }
    };
    for row in kui_core::schema::ENV_FIELDS {
        let value = (row.get)(facts);
        if row.name == "window.native_controls" {
            let win = sub("window")?;
            if value == Value::Null {
                win.raw_remove("controls_w")?;
                win.raw_remove("controls_h")?;
                continue;
            }
            let f = |k: &str| value.get(k).and_then(Value::as_float).unwrap_or(0.0) as f32;
            win.raw_set("controls_w", f("x") + f("w"))?;
            win.raw_set("controls_h", f("y") + f("h"))?;
            continue;
        }
        for path in row.lua {
            let lua_value = if value == Value::Null {
                mlua::Value::Nil
            } else {
                value_to_lua(lua, &value)?
            };
            match path.split_once('.') {
                None => t.raw_set(*path, lua_value)?,
                Some((head, leaf)) => sub(head)?.raw_set(leaf, lua_value)?,
            }
        }
    }
    Ok(())
}

/// The palette the core derived from `system`, as roles rather than
/// values (ADR 0019). Every key is a 0xRRGGBBAA number — the same spelling
/// a `color` prop takes — so `bg = env.theme.surface` needs no conversion;
/// `appearance` says which base it came from and `disabled_opacity` is a
/// multiplier, not a colour. The roles are generated from
/// `schema::THEME_ROLES`, so a script and a Rust view read the same list
/// under the same names. Read-only: a Lua script is a guest in someone
/// else's frame, and the theme is the host's to set.
fn env_theme_into(lua: &Lua, t: &Table, theme: &kui_core::Theme) -> mlua::Result<()> {
    let th = lua.create_table()?;
    for role in kui_core::schema::THEME_ROLES {
        th.set(role.name, (role.get)(theme).to_hex())?;
    }
    th.set("appearance", theme.appearance.name())?;
    th.set("disabled_opacity", theme.disabled_opacity)?;
    t.set("theme", th)
}

/// The sizes the stock widgets are built from (backlog T2), generated from
/// `schema::METRIC_ROLES` the same way: `radius = env.metrics.radius` makes
/// a script's control agree with the host's button. Read-only for the same
/// reason the theme is.
fn env_metrics_into(
    lua: &Lua,
    t: &Table,
    metrics: &kui_core::metrics::Metrics,
) -> mlua::Result<()> {
    let mt = lua.create_table()?;
    for role in kui_core::schema::METRIC_ROLES {
        mt.set(role.name, (role.get)(metrics))?;
    }
    t.set("metrics", mt)
}

/// What [`LuaExtension::env_for`] keeps: the table, and what its facts,
/// palette, sizes and tokens were last written from.
struct EnvCache {
    table: Table,
    facts: Option<kui_core::schema::EnvFacts>,
    theme: Option<kui_core::Theme>,
    metrics: Option<kui_core::metrics::Metrics>,
    tokens_rev: u64,
}

type FrameCell = std::cell::RefCell<&'static mut Ui<'static>>;

thread_local! {
    /// The frame a script's `env` functions reach while one of its fills
    /// runs: set by [`with_frame`] around the call and put back after,
    /// null between — so a function a script kept and calls later errors
    /// rather than touching a frame that is gone.
    static CURRENT_FRAME: std::cell::Cell<*const FrameCell> =
        const { std::cell::Cell::new(std::ptr::null()) };
}

/// The handle `env`'s functions hold in place of a borrow of the frame:
/// what they borrow through, for the length of one call, while
/// [`with_frame`] has a frame up.
#[derive(Clone, Copy)]
struct FrameUi;

impl FrameUi {
    fn cell(self) -> mlua::Result<&'static FrameCell> {
        let p = CURRENT_FRAME.get();
        if p.is_null() {
            return Err(mlua::Error::runtime(
                "env's functions answer only while a view or a component's function runs",
            ));
        }
        // SAFETY: `with_frame` keeps the cell alive for as long as the
        // pointer is set, and puts the previous one back before it goes.
        Ok(unsafe { &*p })
    }

    fn borrow(self) -> mlua::Result<std::cell::Ref<'static, &'static mut Ui<'static>>> {
        self.cell()?
            .try_borrow()
            .map_err(|_| mlua::Error::runtime("env re-entered while it was drawing"))
    }

    fn borrow_mut(self) -> mlua::Result<std::cell::RefMut<'static, &'static mut Ui<'static>>> {
        self.cell()?
            .try_borrow_mut()
            .map_err(|_| mlua::Error::runtime("env re-entered while it was drawing"))
    }
}

/// Runs `f` with `ui` reachable by `env`'s functions, and the previous
/// frame (a fill around this one) back after, however `f` ends.
fn with_frame<R>(ui: &mut Ui<'_>, f: impl FnOnce() -> R) -> R {
    struct Restore(*const FrameCell);
    impl Drop for Restore {
        fn drop(&mut self) {
            CURRENT_FRAME.set(self.0);
        }
    }
    let cell = std::cell::RefCell::new(ui);
    // The lifetimes are erased for the thread-local's sake; the pointer
    // never outlives `cell`, which `Restore` guarantees on every way out.
    let erased = std::ptr::from_ref(&cell).cast::<FrameCell>();
    let _restore = Restore(CURRENT_FRAME.replace(erased));
    f()
}

fn env_table(
    lua: &Lua,
    ui: FrameUi,
    loaded: std::rc::Rc<std::cell::RefCell<Vec<Loaded>>>,
) -> mlua::Result<Table> {
    let t = lua.create_table()?;
    // An editor's text, by the label its `key` field declares or by the
    // integer key an event carried — either spelling, like every query
    // beside it (AR26: it took the integer alone, against its own doc,
    // and the one example kept a key from an editor's event to work
    // around it). A label no frame declared answers nil.
    t.set(
        "edit_text",
        lua.create_function(move |_, key: mlua::Value| {
            let mut ui = ui.borrow_mut()?;
            let Some(key) = key_query(&mut ui, key)? else {
                return Ok(None);
            };
            Ok(ui.edit_text(key))
        })?,
    )?;
    // Replaces an editor's text, caret at the end. Named by the label its
    // `key` field declares as well as by the integer key, and the label is
    // the spelling an `update` that opens the field can use: the key comes
    // from an event the editor has not fired yet (backlog F32). A label no
    // frame has declared is held for the next frame that declares it —
    // seeding a new editor over `initial`, replacing a retained one's draft
    // — and dropped with an `edit-text-without-editor` warning if that
    // frame declares nothing under it.
    t.set(
        "set_edit_text",
        lua.create_function(move |_, (key, text): (mlua::Value, String)| {
            let mut ui = ui.borrow_mut()?;
            match key {
                mlua::Value::String(label) => {
                    ui.set_edit_text_by_label(&label.to_str()?, &text);
                }
                other => {
                    let key = key_arg(&mut ui, other)?;
                    ui.set_edit_text(key, &text);
                }
            }
            Ok(())
        })?,
    )?;
    // Takes a label too (`key_arg`): a view styles the row it declares by
    // the name it gives it, without an event having told it the key.
    t.set(
        "is_focused",
        lua.create_function(move |_, key: mlua::Value| {
            let mut ui = ui.borrow_mut()?;
            let Some(key) = key_query(&mut ui, key)? else {
                return Ok(false);
            };
            Ok(ui.is_focused(key))
        })?,
    )?;
    t.set(
        "is_hovered",
        lua.create_function(move |_, key: mlua::Value| {
            let mut ui = ui.borrow_mut()?;
            let Some(key) = key_query(&mut ui, key)? else {
                return Ok(false);
            };
            Ok(ui.is_hovered(key))
        })?,
    )?;
    // Held down: the press started on this node and the pointer is still
    // over it (or it captured a drag). Goes with `is_hovered` — a script
    // that draws its own button styles the pressed state from this.
    t.set(
        "is_pressed",
        lua.create_function(move |_, key: mlua::Value| {
            let mut ui = ui.borrow_mut()?;
            let Some(key) = key_query(&mut ui, key)? else {
                return Ok(false);
            };
            Ok(ui.is_pressed(key))
        })?,
    )?;
    // A hover group (`hover_group = "name"`) by its name: any member hovered,
    // or holding a press (backlog C92) — what a widget drawn from several
    // pieces styles itself from, the way the core paints `hover_bg` /
    // `pressed_bg` on a member.
    t.set(
        "is_group_hovered",
        lua.create_function(move |_, group: String| {
            let ui = ui.borrow_mut()?;
            Ok(ui.is_group_hovered(kui_core::NodeSpec::hover_group_id(&group)))
        })?,
    )?;
    t.set(
        "is_group_pressed",
        lua.create_function(move |_, group: String| {
            let ui = ui.borrow_mut()?;
            Ok(ui.is_group_pressed(kui_core::NodeSpec::hover_group_id(&group)))
        })?,
    )?;
    // Files dragged in from the OS are over this node (ADR 0031): for
    // drop-dependent *layout*; the colour swap is the `drop_bg` prop.
    t.set(
        "is_drop_target",
        lua.create_function(move |_, key: mlua::Value| {
            let mut ui = ui.borrow_mut()?;
            let Some(key) = key_query(&mut ui, key)? else {
                return Ok(false);
            };
            Ok(ui.is_drop_target(key))
        })?,
    )?;
    // The `on_drop` zone the dragged files are over — its key, nil for
    // none.
    t.set(
        "drop_target",
        lua.create_function(move |_, ()| {
            let ui = ui.borrow()?;
            Ok(ui.drop_target().map(|k| k.0 as i64))
        })?,
    )?;
    // The reverse of naming a node by its label (ADR 0047): the label the
    // integer key an event carried was opened under, in this frame so far
    // and then the last one — nil for an auto-keyed node or a key no frame
    // declared. The label is the local one, so two rows of a list under
    // different parents answer the same.
    t.set(
        "label_of",
        lua.create_function(move |_, key: mlua::Value| {
            let mut ui = ui.borrow_mut()?;
            let Some(key) = key_query(&mut ui, key)? else {
                return Ok(None);
            };
            Ok(ui.core().label_of(key).map(str::to_owned))
        })?,
    )?;
    // Moving focus from the script, the imperative half of `key_focus`.
    // `set_focus`, not `focus`: `env.focus` is already the reading above
    // and alpha.5 shipped it, so the verb takes the longer name rather
    // than change what a name means under a script that already runs.
    // `blur` / `focus_next` / `focus_prev` match Node and C exactly.
    // The key is an integer or a declared label (`key_arg`): "focus the
    // editor I just created" is `env.set_focus("editor")`, with no event
    // from it needed first.
    // A frame at a time on the frame clock (backlog F135): `env.now + 3`
    // is a toast's expiry, with nothing owed until then.
    t.set(
        "request_frame_at",
        lua.create_function(move |_, at: f64| {
            ui.borrow_mut()?.request_frame_at(at);
            Ok(())
        })?,
    )?;
    // The recording (ADR 0058): spans, counters, gauges, marks and
    // measures under the script's origin, and a frame asked for by name.
    // Off, each is one test of a flag in the core.
    // `env.span(name, fn)` ends its span however `fn` leaves, a throw
    // included, and returns what `fn` returned; the `Ui` is not held
    // while `fn` runs, so `fn` calls `env` as the view does.
    t.set(
        "span",
        lua.create_function(move |_, (name, f): (String, mlua::Function)| {
            let id = ui.borrow_mut()?.core().span_begin(&name);
            let r = f.call::<mlua::MultiValue>(());
            ui.borrow_mut()?.core().span_end(id);
            r
        })?,
    )?;
    t.set(
        "perf_measure",
        lua.create_function(move |_, (name, ms): (String, f64)| {
            ui.borrow_mut()?.core().perf_measure(&name, ms);
            Ok(())
        })?,
    )?;
    t.set(
        "perf_count",
        lua.create_function(move |_, (name, n): (String, f64)| {
            ui.borrow_mut()?.core().perf_count(&name, n.floor() as i64);
            Ok(())
        })?,
    )?;
    t.set(
        "perf_gauge",
        lua.create_function(move |_, (name, value): (String, f64)| {
            ui.borrow_mut()?.core().perf_gauge(&name, value);
            Ok(())
        })?,
    )?;
    t.set(
        "perf_mark",
        lua.create_function(move |_, name: String| {
            ui.borrow_mut()?.core().perf_mark(&name);
            Ok(())
        })?,
    )?;
    t.set(
        "perf_measuring",
        lua.create_function(move |_, ()| Ok(ui.borrow_mut()?.core().perf_measuring()))?,
    )?;
    t.set(
        "request_frame_for",
        lua.create_function(move |_, why: String| {
            ui.borrow_mut()?.request_frame_for(&why);
            Ok(())
        })?,
    )?;
    // The exit a node leaves by if it leaves this frame (backlog F136),
    // over the one it declared: `env.exit_with("card", { dx = 400,
    // opacity = 0 })` aims a throw before the view drops the card.
    t.set(
        "exit_with",
        lua.create_function(move |_, (key, exit): (mlua::Value, mlua::Value)| {
            let mut ui = ui.borrow_mut()?;
            let key = key_arg(&mut ui, key)?;
            let e = kui_core::enter::parse(&lua_to_value(&exit)?)
                .map_err(|m| mlua::Error::runtime(format!("exit_with: {m}")))?;
            ui.exit_with(key, e);
            Ok(())
        })?,
    )?;
    t.set(
        "set_focus",
        lua.create_function(move |_, key: mlua::Value| {
            let mut ui = ui.borrow_mut()?;
            let key = key_arg(&mut ui, key)?;
            ui.focus(key);
            Ok(())
        })?,
    )?;
    t.set(
        "blur",
        lua.create_function(move |_, ()| {
            ui.borrow_mut()?.blur();
            Ok(())
        })?,
    )?;
    // What Tab and Shift-Tab do: the next / previous focusable node in
    // tree order, wrapping. A script that binds Tab in an `on_key` sink
    // calls these to hand the keyboard on. Like `reveal`, the step
    // resolves when *this* frame finishes — the ring is made of a
    // finished tree, and the script is still declaring one — so a view
    // can step onto a row it is declaring right now.
    t.set(
        "focus_next",
        lua.create_function(move |_, ()| {
            ui.borrow_mut()?.focus_next();
            Ok(())
        })?,
    )?;
    t.set(
        "focus_prev",
        lua.create_function(move |_, ()| {
            ui.borrow_mut()?.focus_prev();
            Ok(())
        })?,
    )?;
    // Enters a focus region — `env.focus_region("dock")`, the label or the
    // integer key of a node declared `focus_region = true` — or the main
    // ring for nil (`docs/adr/0022-focus-regions.md`). Resolves when this
    // frame finishes, like `focus_next`, so a script may name the region
    // it is declaring right now — call it after the region's node.
    t.set(
        "focus_region",
        lua.create_function(move |_, key: mlua::Value| {
            let mut ui = ui.borrow_mut()?;
            let key = match key {
                mlua::Value::Nil => None,
                v => Some(key_arg(&mut ui, v)?),
            };
            ui.focus_region(key);
            Ok(())
        })?,
    )?;
    // `env.announce(text, politeness)` says something once, with no node
    // behind it (`docs/adr/0008-live-regions-and-announcements.md`).
    // `politeness` is "polite" (the default) or "assertive"; "off" and an
    // empty text are no-ops. A region whose message is on screen is the
    // `live` prop instead.
    //
    // `env` exists only inside `view`, and a view runs every frame, so a
    // call here needs a guard the script clears — `on_event` sets a field,
    // `view` announces it and clears it. The core reports the unguarded
    // case as `announcement-repeated`.
    t.set(
        "announce",
        lua.create_function(move |_, (text, live): (String, Option<String>)| {
            let live = live.unwrap_or_else(|| "polite".to_string());
            let Some(i) = schema::LIVE.iter().position(|v| *v == live) else {
                return Err(mlua::Error::runtime(format!(
                    "bad politeness {live:?} (one of {})",
                    schema::LIVE.join(" | ")
                )));
            };
            ui.borrow_mut()?
                .announce(&text, kui_core::Live::from_index(i));
            Ok(())
        })?,
    )?;
    // Scrolling from the script: `env.reveal(key)` scrolls whatever
    // contains a node so it shows, and `env.scroll_offset` /
    // `env.set_scroll` read and write a container's retained offset.
    // `view` runs while the frame is being built, so a reveal resolves
    // against *this* frame's layout when it finishes — which is what lets
    // a script reveal a row it is declaring right now. A key the frame
    // does not declare, or one with nothing scrollable above it, is a
    // no-op; the request is not kept for a later frame. A label no node
    // has declared yet — the row this `view` declares further down, or a
    // pane's first frame — is resolved when the frame finishes, and one
    // that frame does not declare either is a `label-without-node`
    // warning (backlog DX15), where it was an error scripts wrapped in
    // `pcall` and retried.
    t.set(
        "reveal",
        lua.create_function(move |_, key: mlua::Value| {
            let mut ui = ui.borrow_mut()?;
            match key {
                mlua::Value::String(s) if ui.key_of(&s.to_str()?).is_none() => {
                    ui.reveal_label(&s.to_str()?);
                }
                key => {
                    let key = key_arg(&mut ui, key)?;
                    ui.reveal(key);
                }
            }
            Ok(())
        })?,
    )?;
    // `{x, y}` as the last layout clamped it (positive = content moved up
    // / left); zeroes for a node that never scrolled. Stash it and hand it
    // back to `set_scroll` to restore a position.
    t.set(
        "scroll_offset",
        lua.create_function(move |lua, key: mlua::Value| {
            let mut ui = ui.borrow_mut()?;
            let off = match key_query(&mut ui, key)? {
                Some(key) => ui.scroll_offset(key),
                None => kui_core::Vec2::ZERO,
            };
            let r = lua.create_table()?;
            r.set("x", off.x)?;
            r.set("y", off.y)?;
            Ok(r)
        })?,
    )?;
    // Everything the last layout resolved for a container: its box
    // `{x, y, w, h}`, its content `{content_w, content_h}` and the clamped
    // `{offset = {x, y}}`; nil for a key no layout has resolved as one.
    // This is what lets a script draw a long list affordably — the core
    // builds every child the view declares, so a script that knows `h` and
    // `offset.y` declares the rows that fit plus two spacers holding the
    // space of the rest. It describes the frame before this one, so a
    // resize slices one frame late; declare a row or two extra at each end.
    // Where a point (the `x`, `y` a click or drag event carried) lands in
    // the text a keyed node drew: `{ byte, line }` — the byte offset into
    // that text, across the node's runs in order (a `role="none"` subtree
    // skipped), and the visual row within the node, counted across its
    // runs (AR30) — or nil for a key that drew no text. Answered from the frame that
    // finished, which is the layout the pointer was over.
    t.set(
        "text_hit",
        lua.create_function(move |lua, (key, x, y): (mlua::Value, f32, f32)| {
            let mut ui = ui.borrow_mut()?;
            let Some(key) = key_query(&mut ui, key)? else {
                return Ok(mlua::Value::Nil);
            };
            let Some(h) = ui.text_hit(key, kui_core::Vec2::new(x, y)) else {
                return Ok(mlua::Value::Nil);
            };
            value_to_lua(lua, &h.to_value())
        })?,
    )?;
    // Opens a context menu at (x, y) over a keyed node, its items a list
    // of tables: `{ label=, role=, enabled=, checked=, id=, accel= }`, all
    // but `label` optional, `role` one of "custom" (the default),
    // "separator", "cut", "copy", "paste", "selectAll", "lookUp" — the
    // spelling the `menu` event reports back ("select_all" / "look_up"
    // are taken too). Choosing a row posts the node's `on_menu` message
    // with `menu = { role, item }` in it (nothing, on a node that declares
    // none) and closes the menu (docs/adr/0017-selection-as-a-scope.md).
    t.set(
        "open_menu",
        lua.create_function(
            move |_, (key, x, y, items): (mlua::Value, f32, f32, mlua::Table)| {
                let mut ui = ui.borrow_mut()?;
                let Some(target) = key_query(&mut ui, key)? else {
                    return Ok(false);
                };
                let (items, stray) = menu_items(&items)?;
                warn_stray_menu_keys(ui.core(), stray);
                ui.open_menu(kui_core::Menu::new(
                    target,
                    kui_core::Vec2::new(x, y),
                    items,
                ));
                Ok(true)
            },
        )?,
    )?;
    // Closes whatever menu is open; true when there was one.
    t.set(
        "close_menu",
        lua.create_function(move |_, ()| {
            let mut ui = ui.borrow_mut()?;
            Ok(ui.close_menu())
        })?,
    )?;
    // The window's selected text: what a `selectable` scope holds, or the
    // focused `edit`'s selection — one per window, so there is no choice
    // to make. Nil with no selection, `""` when one exists and covers
    // nothing (docs/adr/0017-selection-as-a-scope.md).
    t.set(
        "selection_text",
        lua.create_function(move |_, ()| {
            let ui = ui.borrow()?;
            Ok(ui.selection_text())
        })?,
    )?;
    // Asks for the selection as text: returns the text, or nil and true
    // when the app was asked instead — a selection that reaches rows a
    // virtual list never built posts `selectionrange = { from = {index,
    // byte}, to = {index, byte} }` on the scope, in its
    // `on_selection_range` message or alone, and the app answers with
    // `answer_selection_range` (docs/adr/0017-selection-as-a-scope.md).
    t.set(
        "request_copy",
        lua.create_function(move |_, ()| {
            let mut ui = ui.borrow_mut()?;
            Ok(match ui.request_copy() {
                kui_core::CopyRequest::Ready(text) => (Some(text), false),
                kui_core::CopyRequest::Asked => (None, true),
                kui_core::CopyRequest::Nothing => (None, false),
            })
        })?,
    )?;
    // Answers a `typeahead` ask (ADR 0070) with the data index of the row
    // the search names, in the composite that asked. False when it asked
    // nothing, or focus has left it.
    t.set(
        "answer_typeahead",
        lua.create_function(move |_, (key, index): (mlua::Value, u64)| {
            let mut ui = ui.borrow_mut()?;
            let Some(key) = key_query(&mut ui, key)? else {
                return Ok(false);
            };
            Ok(ui.answer_typeahead(key, index))
        })?,
    )?;
    // Registers a WGSL fragment function and returns its id, for `fragment
    // { id = }` (ADR 0015, backlog C111): nil, with a
    // `fragment-rejected` warning naming the line, when it does not
    // compile. Idempotent by source, so a view or a component's function
    // may call it every frame and get the same id back; the fragment lasts
    // as long as the session.
    t.set(
        "add_fragment",
        lua.create_function(move |_, wgsl: String| {
            let mut ui = ui.borrow_mut()?;
            Ok(ui.core().add_fragment(&wgsl).map(|id| id.to_ffi()))
        })?,
    )?;
    // Answers a `selectionrange` ask with the text for the range it named,
    // whole. False when nothing asked.
    t.set(
        "answer_selection_range",
        lua.create_function(move |_, text: String| {
            let mut ui = ui.borrow_mut()?;
            Ok(ui.answer_selection_range(&text))
        })?,
    )?;
    // The clipboard for a script that owns its text (backlog C33): a key
    // sink hears the raw Ctrl-c / Ctrl-v and binds them here. Both queue
    // the action a menu's Copy or Paste would, for the host to apply at
    // its next drain; a paste comes back as a `text` event on the focused
    // sink (or as typing into a focused editor).
    t.set(
        "set_clipboard",
        lua.create_function(move |_, (text, html): (String, Option<String>)| {
            ui.borrow_mut()?.set_clipboard(text, html);
            Ok(())
        })?,
    )?;
    // A secret, which the host writes marked concealed and transient the
    // way a password manager does, so no clipboard manager shows or keeps
    // it (backlog F84).
    t.set(
        "set_clipboard_secret",
        lua.create_function(move |_, text: String| {
            ui.borrow_mut()?.set_clipboard_secret(text);
            Ok(())
        })?,
    )?;
    t.set(
        "request_paste",
        lua.create_function(move |_, ()| {
            ui.borrow_mut()?.request_paste();
            Ok(())
        })?,
    )?;
    t.set(
        "awaiting_paste",
        lua.create_function(move |_, ()| Ok(ui.borrow()?.awaiting_paste()))?,
    )?;
    // The platform's Open, Save or folder dialog (backlog C51): `{ mode =
    // "open"|"save"|"folder", multiple, title, filters = {{ name, extensions
    // = {...} }}, directory, file_name, tag }`, every field optional. The
    // answer is a `files` event to this script: `tag` the message, the
    // answer in its `files = { paths }` field — `paths` empty when the user
    // cancelled. False when one is already out.
    t.set(
        "request_files",
        lua.create_function(move |_, opts: Option<mlua::Value>| {
            let v = match &opts {
                Some(o) => lua_to_value(o)?,
                None => Value::Null,
            };
            let dialog = kui_core::FileDialog::from_value(&v)
                .map_err(|e| mlua::Error::runtime(format!("request_files: {e}")))?;
            Ok(ui.borrow_mut()?.request_files(dialog))
        })?,
    )?;
    t.set(
        "awaiting_files",
        lua.create_function(move |_, ()| Ok(ui.borrow()?.awaiting_files()))?,
    )?;
    // The selection as HTML: the formatting the text declared (bold,
    // The text selection's two ends as the drag made them: `{anchor =
    // {index, byte}, focus = {index, byte}}`, `index` the data index of
    // the virtualised row the end is in (nil outside one) and `byte` the
    // offset in that row's own text. Directed, so a Shift-click that kept
    // the anchor reads as one (ADR 0029). Nil with no text selection.
    t.set(
        "selection_ends",
        lua.create_function(move |lua, ()| {
            let ui = ui.borrow()?;
            let Some((a, f)) = ui.selection_ends() else {
                return Ok(mlua::Value::Nil);
            };
            let end = |e: kui_core::RangeEnd| -> mlua::Result<mlua::Table> {
                let t = lua.create_table()?;
                if let Some(r) = e.row {
                    t.set("index", r)?;
                }
                t.set("byte", e.byte)?;
                Ok(t)
            };
            let out = lua.create_table()?;
            out.set("anchor", end(a)?)?;
            out.set("focus", end(f)?)?;
            Ok(mlua::Value::Table(out))
        })?,
    )?;
    // A `cells` grid's selection, the window's when it lives in one:
    // `{node, anchor = {line, col}, focus = {line, col}, block}`, the
    // lines absolute (`origin_line` plus the row, so a scroll does not
    // move them) and the ends as the drag made them (ADR 0017, decision
    // 4). Nil when the window's selection is not a grid's; a text
    // selection's ends are `selection_ends()`. The row ADR 0017 §4
    // offered and only Rust had (backlog B1a).
    t.set(
        "cell_selection",
        lua.create_function(move |lua, ()| {
            let ui = ui.borrow()?;
            match ui.cell_selection() {
                Some(sel) => value_to_lua(lua, &sel.to_value(kui_core::Handles::INT)),
                None => Ok(mlua::Value::Nil),
            }
        })?,
    )?;
    // italic, a span's own colour) and not the node's colour, which is
    // the theme's. Nil with no text selection. A second clipboard flavour
    // beside the plain text, never instead of it.
    t.set(
        "selection_html",
        lua.create_function(move |_, ()| {
            let ui = ui.borrow()?;
            Ok(ui.selection_html())
        })?,
    )?;
    // Selects every run inside the scope a keyed node declared, first
    // byte to last — Select All, scoped. False for a node that drew no
    // text or is not a scope. Runs the frame built but never drew are
    // part of it.
    t.set(
        "select_all_in",
        lua.create_function(move |_, key: mlua::Value| {
            let mut ui = ui.borrow_mut()?;
            let Some(key) = key_query(&mut ui, key)? else {
                return Ok(false);
            };
            Ok(ui.select_all_in(key))
        })?,
    )?;
    // Drops the window's selection, whichever it is; true when there was
    // one to drop.
    t.set(
        "clear_selection",
        lua.create_function(move |_, ()| {
            let mut ui = ui.borrow_mut()?;
            Ok(ui.clear_selection())
        })?,
    )?;
    // The caret rect for a byte offset in that text: `{ x, y, w, h }`,
    // logical viewport px, zero wide, one line tall; nil for a key that
    // drew no text. A byte past the text is the end.
    t.set(
        "caret_rect",
        lua.create_function(move |lua, (key, byte): (mlua::Value, mlua::Value)| {
            let byte = measure_arg(&byte, "caret_rect: byte")?;
            let mut ui = ui.borrow_mut()?;
            let Some(key) = key_query(&mut ui, key)? else {
                return Ok(mlua::Value::Nil);
            };
            let Some(r) = ui.caret_rect(key, byte) else {
                return Ok(mlua::Value::Nil);
            };
            value_to_lua(lua, &r.to_value())
        })?,
    )?;
    t.set(
        "scroll_geometry",
        lua.create_function(move |lua, key: mlua::Value| {
            let mut ui = ui.borrow_mut()?;
            let Some(key) = key_query(&mut ui, key)? else {
                return Ok(mlua::Value::Nil);
            };
            let Some(g) = ui.scroll_geometry(key) else {
                return Ok(mlua::Value::Nil);
            };
            // The core's shape, whose keys are already Lua's spelling.
            value_to_lua(lua, &g.to_value())
        })?,
    )?;
    // The rect the last frame laid an `on_layout` node out at — the
    // `layout` event's numbers without the event (backlog C26 step 2).
    t.set(
        "layout_of",
        lua.create_function(move |lua, key: mlua::Value| {
            let mut ui = ui.borrow_mut()?;
            let Some(key) = key_query(&mut ui, key)? else {
                return Ok(mlua::Value::Nil);
            };
            let Some(r) = ui.layout_of(key) else {
                return Ok(mlua::Value::Nil);
            };
            value_to_lua(lua, &r.to_value())
        })?,
    )?;
    // The wheel's move by hand; the next layout clamps it, so 0,0 is "jump
    // to the top" and a huge y is "jump to the end".
    t.set(
        "set_scroll",
        lua.create_function(move |_, (key, x, y): (mlua::Value, f32, f32)| {
            let mut ui = ui.borrow_mut()?;
            let at = kui_core::Vec2::new(x, y);
            // A label not declared yet waits for the frame's end, as
            // `reveal`'s does (backlog DX15).
            match key {
                mlua::Value::String(s) if ui.key_of(&s.to_str()?).is_none() => {
                    ui.set_scroll_label(&s.to_str()?, at);
                }
                key => {
                    let key = key_arg(&mut ui, key)?;
                    ui.set_scroll(key, at);
                }
            }
            Ok(())
        })?,
    )?;
    // A correction by content that moved under the list, on y, with no
    // ease: what the prelude's `list` asks for when the rows it measured
    // came out another height than their estimate (RG18, backlog C46). A
    // key nothing declared yet is the first frame, with nothing to correct.
    t.set(
        "shift_scroll",
        lua.create_function(move |_, (key, drawn, target): (mlua::Value, f32, f32)| {
            let mut ui = ui.borrow_mut()?;
            if let Some(key) = key_query(&mut ui, key)? {
                ui.shift_scroll(
                    key,
                    kui_core::Vec2::new(0.0, drawn),
                    kui_core::Vec2::new(0.0, target),
                );
            }
            Ok(())
        })?,
    )?;
    // A drag the script holds, near a container's edge (ADR 0029): the
    // container scrolls at the edge scroll's feel for a pointer at x, y
    // in the edge zone inside `rect` (`{x, y, w, h}`, usually the
    // container's `scroll_geometry`), and the call returns what moved,
    // `{x, y}`, asking for the next frame; nil out of the zones, at the
    // end that way, or for a key nothing declared.
    t.set(
        "edge_scroll",
        lua.create_function(
            move |lua, (key, x, y, rect): (mlua::Value, f32, f32, Table)| {
                let mut ui = ui.borrow_mut()?;
                let Some(key) = key_query(&mut ui, key)? else {
                    return Ok(None);
                };
                let rect = kui_core::Rect::new(
                    rect.get("x")?,
                    rect.get("y")?,
                    rect.get("w")?,
                    rect.get("h")?,
                );
                let Some(d) = ui.edge_scroll(key, kui_core::Vec2::new(x, y), rect) else {
                    return Ok(None);
                };
                let r = lua.create_table()?;
                r.set("x", d.x)?;
                r.set("y", d.y)?;
                Ok(Some(r))
            },
        )?,
    )?;
    // Window requests from the script: `env.set_window_size(window, w, h)`
    // and `env.focus_window(window)` queue commands the driver applies on
    // its next pump — after this frame, since `view` runs inside one — and
    // a headless driver never drains. Requests, not declarations: the user
    // owns a window's size once it exists (ADR 0004 decision 5).
    // `env.window.id` is the window the script is drawing, and the only
    // one there is until step 3.
    t.set(
        "set_window_size",
        lua.create_function(move |_, (window, w, h): (mlua::Value, f32, f32)| {
            let window = window_arg(&window, "set_window_size: window")?;
            ui.borrow_mut()?
                .set_window_size(kui_core::WindowId(window), kui_core::Size::new(w, h));
            Ok(())
        })?,
    )?;
    // Declare this script's tokens from inside a view (ADR 0027): the
    // same shape as the `tokens` global, replacing this origin's table
    // whole, in effect for the nodes the same view opens after the call.
    // `env.load_tokens(path, opts)` → `{ tokens, names, warnings,
    // resolver }`: a DTCG file or resolver (ADR 0062) in the shape
    // `env.set_tokens` takes, a relative path from the script's folder.
    t.set("load_tokens", lua.create_function(load_tokens)?)?;
    // A script whose lengths follow a tier declares on each change.
    let env = t.clone();
    t.set(
        "set_tokens",
        lua.create_function(move |lua, decl: Table| {
            let tokens = parse_tokens(&decl)?;
            ui.borrow_mut()?.set_tokens(tokens);
            env.set("tokens", tokens_table(lua, &**ui.borrow()?)?)
        })?,
    )?;
    t.set(
        "focus_window",
        lua.create_function(move |_, window: mlua::Value| {
            let window = window_arg(&window, "focus_window: window")?;
            ui.borrow_mut()?.focus_window(kui_core::WindowId(window));
            Ok(())
        })?,
    )?;
    t.set(
        "measure_text",
        lua.create_function(
            move |lua, (s, opts, max_w): (mlua::Value, Option<Table>, Option<f32>)| {
                let mut guard = ui.borrow_mut()?;
                let m = measure_from_lua(&mut guard, &s, opts.as_ref(), max_w)?;
                value_to_lua(lua, &m.to_value())
            },
        )?,
    )?;
    t.set(
        "caret_in_text",
        lua.create_function(
            move |lua, args: (mlua::Value, Option<Table>, Option<f32>, mlua::Value)| {
                let (s, opts, max_w, byte) = args;
                let byte = measure_arg(&byte, "caret_in_text: byte")?;
                let mut guard = ui.borrow_mut()?;
                match caret_in_from_lua(&mut guard, &s, opts.as_ref(), max_w, byte)? {
                    Some(r) => value_to_lua(lua, &r.to_value()),
                    None => Ok(mlua::Value::Nil),
                }
            },
        )?,
    )?;
    let loaded_names = loaded.clone();
    // `env.add_extension(namespace, path)` → true, or nil and a message:
    // the script hosting an extension of its own (ADR 0014, and
    // `kui_core::slot`). A `.lua` file or a folder with `init.lua` is
    // another script, loaded here; anything else is a C shared library —
    // a `.so` / `.dylib` / `.dll` exporting the seven `kui_ext_*` entry
    // points `crates/kui-ffi/include/kui.h` describes — opened by the
    // native loader the host's crate registered (ADR 0059). Either is the
    // same extension a Rust, C or Node host loads: the script is just
    // another host.
    //
    // Called from `view`, because that is where a script knows what it
    // wants, and idempotent by (namespace, path) so the honest spelling
    // is to call it every frame. The namespace joins the frame's *one*
    // map, so it can collide with the host's; a taken one is the error.
    // Nothing here is a new capability: `Lua::new` has `package`, so a
    // script could already `package.loadlib` anything on the disk. What
    // this adds is a plugin that draws in the script's own tree.
    t.set(
        "add_extension",
        lua.create_function(move |_, (namespace, path): (String, String)| {
            let path = std::path::PathBuf::from(path);
            if let Some(prev) = loaded.borrow().iter().find(|l| l.namespace == namespace) {
                return Ok(if prev.path == path {
                    // The every-frame call, already answered.
                    (Some(true), None)
                } else {
                    (
                        None,
                        Some(format!(
                            "`{namespace}` is already {} in this script; a second plugin needs a \
                             second namespace",
                            prev.path.display()
                        )),
                    )
                });
            }
            // A `.lua` path is loaded here; anything else goes to the
            // native loader the host's crate registered (ADR 0059). Its
            // code runs in this process on this frame: a script naming
            // one is trusting it as the host trusts the script.
            let ext: Box<dyn Extension> = if is_lua_path(&path) {
                match LuaExtension::from_path(&path) {
                    Ok(ext) => Box::new(ext),
                    Err(e) => return Ok((None, Some(e.to_string()))),
                }
            } else if let Some(open) = NATIVE_LOADER.get() {
                match open(&path) {
                    Ok(ext) => ext,
                    Err(e) => return Ok((None, Some(e))),
                }
            } else {
                return Ok((
                    None,
                    Some(format!(
                        "{}: this host loads no C plugins from a script; a Rust host \
                         registers the loader with `kui_ffi::register_native_loader()`",
                        path.display()
                    )),
                ));
            };
            let origin = match ui.borrow_mut()?.add_extension(&namespace, ext) {
                Ok(origin) => origin,
                Err(e) => return Ok((None, Some(e))),
            };
            loaded.borrow_mut().push(Loaded {
                namespace,
                path,
                origin,
            });
            Ok((Some(true), None))
        })?,
    )?;
    // The namespaces this script loaded, in the order it asked for them —
    // what it can place with `component`, and what an emit's `from` will say.
    t.set(
        "extension_namespaces",
        lua.create_function(move |lua, ()| {
            lua.create_sequence_from(loaded_names.borrow().iter().map(|l| l.namespace.clone()))
        })?,
    )?;
    Ok(t)
}

/// `env.load_tokens(path, opts)`: the DTCG loader (ADR 0062) for a script,
/// answering the table `env.set_tokens` takes plus where each name came
/// from and what was skipped. Errors are Lua errors: a script asking for a
/// brand file that is not there should hear it.
fn load_tokens(lua: &Lua, (path, opts): (String, Option<Table>)) -> mlua::Result<Table> {
    use kui_core::dtcg::{self, DtcgOptions, DtcgValue};
    let mut o = DtcgOptions::default();
    if let Some(t) = &opts {
        for pair in t.clone().pairs::<String, mlua::Value>() {
            let (k, v) = pair?;
            match (k.as_str(), &v) {
                ("prefix", mlua::Value::String(s)) => o.prefix = s.to_str()?.to_string(),
                ("modifier", mlua::Value::String(s)) => o.modifier = s.to_str()?.to_string(),
                ("light", mlua::Value::String(s)) => o.light = s.to_str()?.to_string(),
                ("dark", mlua::Value::String(s)) => o.dark = s.to_str()?.to_string(),
                ("rem", mlua::Value::Number(n)) => o.rem_px = *n as f32,
                ("rem", mlua::Value::Integer(n)) => o.rem_px = *n as f32,
                ("inputs", mlua::Value::Table(m)) => {
                    for p in m.clone().pairs::<String, String>() {
                        let (mk, mv) = p?;
                        o.inputs.push((mk, mv));
                    }
                }
                (k, _) => {
                    return Err(mlua::Error::runtime(format!(
                        "load_tokens: unknown option {k:?} (prefix, modifier, light, dark, rem, inputs)"
                    )));
                }
            }
        }
    }
    let mut p = std::path::PathBuf::from(&path);
    if p.is_relative()
        && let Ok(dir) = lua.named_registry_value::<String>("kui:dir")
    {
        p = std::path::Path::new(&dir).join(p);
    }
    let loaded =
        dtcg::from_path(&p, &o).map_err(|e| mlua::Error::runtime(format!("load_tokens: {e}")))?;
    let (colors, lengths, families) = (
        lua.create_table()?,
        lua.create_table()?,
        lua.create_table()?,
    );
    let names = lua.create_table()?;
    for n in &loaded.names {
        let kind = match &n.value {
            DtcgValue::Color { light, dark } if light == dark => {
                colors.set(n.name.as_str(), light.to_hex())?;
                "color"
            }
            DtcgValue::Color { light, dark } => {
                let half = lua.create_table()?;
                half.set("light", light.to_hex())?;
                half.set("dark", dark.to_hex())?;
                colors.set(n.name.as_str(), half)?;
                "color"
            }
            DtcgValue::Length(px) => {
                lengths.set(n.name.as_str(), *px)?;
                "length"
            }
            DtcgValue::Family(list) => {
                families.set(
                    n.name.as_str(),
                    lua.create_sequence_from(list.iter().map(String::as_str))?,
                )?;
                "family"
            }
        };
        let row = lua.create_table()?;
        row.set("name", n.name.as_str())?;
        row.set("path", n.path.as_str())?;
        row.set("kind", kind)?;
        names.push(row)?;
    }
    let tokens = lua.create_table()?;
    tokens.set("colors", colors)?;
    tokens.set("lengths", lengths)?;
    tokens.set("families", families)?;
    let out = lua.create_table()?;
    out.set("tokens", tokens)?;
    out.set("names", names)?;
    out.set(
        "warnings",
        lua.create_sequence_from(loaded.warnings.iter().map(|w| w.to_string()))?,
    )?;
    out.set("resolver", loaded.resolver)?;
    Ok(out)
}

/// `env.measure_text(s, opts, max_w)` → `{ width, height, lines }` (logical
/// px): what layout would give a text node with that content and style,
/// wrapped to `max_w` when given. `s` is a string, a span list (the same
/// shape `text({...})` takes) or a whole `text(...)` node table, whose own
/// props are then the style; `opts` is a style table (`size`, `font`,
/// `wrap`, `max_lines`, `ellipsis`, ...). The metrics do not scale
/// linearly: `measured × zoom` is not `measure(size × zoom)`, because
/// shaping rounds per size, so anything that zooms measures at the size it
/// draws.
/// `env.tokens`: the frame's resolved tokens, own over host, as three
/// tables of name → value: colours as `0xRRGGBBAA`, lengths, and each
/// family token's pick (`sans` for a list nothing matched).
fn tokens_table(lua: &Lua, ui: &Ui<'_>) -> mlua::Result<Table> {
    let look = ui.tokens();
    let tokens = lua.create_table()?;
    let colors = lua.create_table()?;
    for (name, c) in look.colors() {
        colors.set(name, c.to_hex())?;
    }
    let lengths = lua.create_table()?;
    for (name, v) in look.lengths() {
        lengths.set(name, v)?;
    }
    let families = lua.create_table()?;
    for (name, f) in look.families() {
        families.set(name, f.picked().unwrap_or("sans"))?;
    }
    tokens.set("colors", colors)?;
    tokens.set("lengths", lengths)?;
    tokens.set("families", families)?;
    Ok(tokens)
}

fn measure_from_lua(
    ui: &mut Ui<'_>,
    s: &mlua::Value,
    opts: Option<&Table>,
    max_w: Option<f32>,
) -> mlua::Result<kui_core::TextMetrics> {
    text_from_lua(
        ui,
        s,
        opts,
        "measure_text",
        |ui, content, style| ui.measure_text(content, style, max_w),
        |ui, spans, style| ui.measure_rich_text(spans, style, max_w),
    )
}

/// `env.caret_in_text(s, opts, max_w, byte)` → `{ x, y, w, h }` (logical
/// px from the text's own top-left), or nil: where byte `byte` of the text
/// `measure_text` would measure lands once laid out at `max_w` — the rect
/// `caret_rect` gives when a node draws that text at that width, for a
/// caret on the frame its text is new (backlog F171).
fn caret_in_from_lua(
    ui: &mut Ui<'_>,
    s: &mlua::Value,
    opts: Option<&Table>,
    max_w: Option<f32>,
    byte: usize,
) -> mlua::Result<Option<kui_core::Rect>> {
    text_from_lua(
        ui,
        s,
        opts,
        "caret_in_text",
        |ui, content, style| ui.caret_in_text(content, style, max_w, byte),
        |ui, spans, style| ui.caret_in_rich_text(spans, style, max_w, byte),
    )
}

/// The text a query names — a string, a span list or a whole `text(...)`
/// node table, whose own props are then the style — handed to `plain` or
/// `rich` in its style.
fn text_from_lua<R>(
    ui: &mut Ui<'_>,
    s: &mlua::Value,
    opts: Option<&Table>,
    name: &str,
    plain: impl Fn(&mut Ui<'_>, &str, &kui_core::TextStyle) -> R,
    rich: impl Fn(&mut Ui<'_>, &[Span<'_>], &kui_core::TextStyle) -> R,
) -> mlua::Result<R> {
    let style = match opts {
        Some(t) => with_refs(ui, |refs| parse_props(t, false, refs))?.style,
        None => kui_core::TextStyle::default(),
    };
    let of_spans = |ui: &mut Ui<'_>, spans: &Table, style: &kui_core::TextStyle| {
        let parts = with_refs(ui, |refs| collect_spans(spans, refs))?;
        let spans: Vec<Span<'_>> = parts.iter().map(span_of).collect();
        Ok(rich(ui, &spans, style))
    };
    match s {
        mlua::Value::String(s) => Ok(plain(ui, &s.to_str()?, &style)),
        mlua::Value::Table(t) => {
            if t.get::<Option<String>>("type")?.as_deref() == Some("text") {
                let style = with_refs(ui, |refs| parse_props(t, false, refs))?.style;
                match t.get::<Option<Table>>("spans")? {
                    Some(spans) => of_spans(ui, &spans, &style),
                    None => {
                        let value: String = t.get("value")?;
                        Ok(plain(ui, &value, &style))
                    }
                }
            } else {
                of_spans(ui, t, &style)
            }
        }
        other => Err(bad(format!(
            "{name}: expected a string, a span list or a text node, got {}",
            other.type_name()
        ))),
    }
}

// ---------------------------------------------------------------------------
// Table tree -> IR

fn bad(msg: impl std::fmt::Display) -> mlua::Error {
    mlua::Error::runtime(msg.to_string())
}

/// A node's children, holes skipped: `{ a, cond and b or nil, c }` is the
/// idiom for a child that is sometimes there, and stopping at the first
/// nil, as `ipairs` does, dropped every child after a missing one without
/// a word. A constructor leaves `#t` at its last positional value; the
/// prelude's containers close the gaps where their own `t.type =` rehash
/// would have moved it (`seal` in prelude.lua).
fn each_child(t: &Table, mut f: impl FnMut(Table) -> mlua::Result<()>) -> mlua::Result<()> {
    for i in 1..=t.raw_len() {
        match t.raw_get::<mlua::Value>(i)? {
            mlua::Value::Nil => {}
            mlua::Value::Table(child) => f(child)?,
            other => {
                return Err(bad(format!(
                    "child {i} is a {}, not a node",
                    other.type_name()
                )));
            }
        }
    }
    Ok(())
}

fn build_children(ui: &mut Ui<'_>, t: &Table) -> mlua::Result<()> {
    each_child(t, |child| build_node(ui, &child))
}

/// Builds `t`'s children inside a widget's content closure, carrying the
/// first error out (widget closures can't return one).
fn with_children(
    ui: &mut Ui<'_>,
    t: &Table,
    widget: impl FnOnce(&mut Ui<'_>, &mut dyn FnMut(&mut Ui<'_>)),
) -> mlua::Result<()> {
    let mut result = Ok(());
    widget(ui, &mut |ui| result = build_children(ui, t));
    result
}

/// The `ELEMENTS` row a Lua node type is: the two container constructors are
/// one element, `input` is the chrome around an `edit`, and the widget
/// functions spell their names with underscores.
fn element_of(ty: &str) -> &str {
    match ty {
        "row" | "column" => "box",
        "grid" => "table",
        "input" => "edit",
        "dropdown" => "select",
        "radio_group" => "radioGroup",
        "window_buttons" => "windowButtons",
        "menu_bar" => "menuBar",
        "latency_graph" | "latency_hud" => "latencyGraph",
        other => other,
    }
}

/// `menu_bar { menu = { … }, on_menu = msg, standard_menus = true }`:
/// the bar the core reads from plain data, the message a choice from it
/// arrives as, with `menu = { role, item }` in it (ADR 0048), and whether
/// the platform's standard menus stay around it (ADR 0052). A bar with no
/// `on_menu` posts nothing.
fn menu_bar_of(t: &Table) -> mlua::Result<(kui_core::MenuBar, Vec<String>)> {
    let (mut bar, stray) = menus_of(t)?;
    let on_menu = t.get::<mlua::Value>("on_menu")?;
    if !on_menu.is_nil() {
        bar = bar.on_menu(lua_to_value(&on_menu)?);
    }
    bar.standard_menus = t.get::<Option<bool>>("standard_menus")?.unwrap_or(false);
    Ok((bar, stray))
}

fn menus_of(t: &Table) -> mlua::Result<(kui_core::MenuBar, Vec<String>)> {
    let Some(list) = t.get::<Option<Table>>("menu")? else {
        return Ok((kui_core::MenuBar::default(), Vec::new()));
    };
    let mut menus = lua_list_to_value(&list)?;
    if let Value::List(menus) = &mut menus {
        for menu in menus.iter_mut() {
            let Value::Map(fields) = menu else { continue };
            for (k, items) in fields.iter_mut() {
                if k != "items" {
                    continue;
                }
                // An empty Lua table read as a map is an empty row list.
                if matches!(items, Value::Map(m) if m.is_empty()) {
                    *items = Value::List(Vec::new());
                }
                if let Value::List(rows) = items {
                    rows.iter_mut().for_each(alias_menu_role);
                }
            }
        }
    }
    let bar = kui_core::MenuBar::from_value(&menus).map_err(mlua::Error::runtime)?;
    Ok((bar, kui_core::MenuBar::stray_keys(&menus)))
}

fn declare_windows(ui: &mut Ui<'_>, root: &Table) -> mlua::Result<()> {
    let Some(list) = root.get::<Option<Table>>("windows")? else {
        return Ok(());
    };
    // Plain data with a fixed shape, read by the core (`WindowConfig::
    // from_value`, `WindowLifecycle::from_entry`): a name, or `{name,
    // kind?, width?, height?, activates?, anchor?, on_window?,
    // on_dismiss?}`, the last two the messages its opening and closing
    // and a popup's dismissal arrive as (ADR 0048).
    for entry in list.sequence_values::<mlua::Value>() {
        let v = lua_to_value(&entry?)?;
        let (name, cfg) = WindowConfig::from_value(&v).map_err(mlua::Error::runtime)?;
        ui.window_with(&name, cfg, WindowLifecycle::from_entry(&v, true));
    }
    Ok(())
}

/// `window_events = { on_resize = …, … }`: the messages the window's own
/// events arrive as (ADR 0048), read by the core's one reader in Lua's
/// spelling, so a key the group does not have is an error naming the six.
fn window_events_of(v: &mlua::Value) -> mlua::Result<kui_core::WindowEvents> {
    kui_core::WindowEvents::from_value(&lua_to_value(v)?, true).map_err(bad)
}

/// Warns about every key in the node table that no table claims — the
/// binding is about to drop it (see `diag::UNKNOWN_PROP`). Only string keys:
/// children sit at the integer ones.
fn check_props(ui: &mut Ui<'_>, t: &Table, element: &str) -> mlua::Result<()> {
    if !ui.core().diagnostics() {
        return Ok(());
    }
    for pair in t.pairs::<mlua::Value, mlua::Value>() {
        let (k, _) = pair?;
        let mlua::Value::String(k) = k else { continue };
        let name = k.to_str()?;
        // `type` is the prelude's element tag, not a prop.
        if name.as_ref() == "type" || schema::known_prop(element, &name, schema::Spelling::Snake) {
            continue;
        }
        let w = kui_core::diag::unknown_prop(element, &name, schema::Spelling::Snake);
        ui.core().warn(w);
    }
    Ok(())
}

/// `devtools_tab { name = , label = , component = , props = }` or
/// `devtools_tab { name = , label = , view = function() … end }`: a tab in
/// the core's devtools panel, an extension's component placed in its body
/// with `props` and the panel's facts (ADR 0067), or the script's own
/// through `view`, which is called only while the tab is on show — the
/// same rule the Rust closure and the C open answer — and whose returned
/// tree is the tab's content. Not a node and not a schema element. A
/// declaration the converter cannot read as either form warns
/// `bad-devtools-tab` and declares nothing.
fn build_devtools_tab(ui: &mut Ui<'_>, t: &Table) -> mlua::Result<()> {
    let name: String = t.get::<Option<String>>("name")?.unwrap_or_default();
    let refuse = |ui: &mut Ui<'_>, why: &str| {
        ui.core().warn(kui_core::diag::bad_devtools_tab(&name, why));
        Ok(())
    };
    if name.is_empty() {
        return refuse(ui, "it needs a name (its identity)");
    }
    let label: String = t
        .get::<Option<String>>("label")?
        .unwrap_or_else(|| name.clone());
    for pair in t.pairs::<mlua::Value, mlua::Value>() {
        let (k, _) = pair?;
        let mlua::Value::String(k) = k else { continue };
        let k = k.to_str()?;
        if !matches!(
            k.as_ref(),
            "type" | "name" | "label" | "component" | "props" | "view"
        ) {
            return refuse(
                ui,
                &format!(
                    "it takes name, label and component (with props) or view, not {:?}",
                    k.as_ref()
                ),
            );
        }
    }
    let component = t.get::<Option<String>>("component")?;
    let view = t.get::<mlua::Value>("view")?;
    match (component, view) {
        (Some(component), mlua::Value::Nil) => {
            if !component.contains(kui_core::NAMESPACE_SEPARATOR) {
                return refuse(
                    ui,
                    &format!("bad component {component:?} (a full \"namespace/name\")"),
                );
            }
            let props = match t.get::<mlua::Value>("props")? {
                mlua::Value::Nil => Value::Null,
                v => lua_to_value(&v)?,
            };
            ui.devtools_tab(&name, &label, &component, &props);
            Ok(())
        }
        (None, mlua::Value::Function(view)) => {
            let mut result = Ok(());
            ui.devtools_tab_with(&name, &label, |ui| {
                result = view
                    .call::<Table>(())
                    .and_then(|tree| build_node(ui, &tree));
            });
            result
        }
        (Some(_), mlua::Value::Function(_)) => {
            refuse(ui, "it takes a component or a view, not both")
        }
        (None, mlua::Value::Nil) => refuse(ui, "it needs a component or a view function"),
        (_, other) => refuse(
            ui,
            &format!("view is a {}, not a function", other.type_name()),
        ),
    }
}

/// How deep a view table may nest. Past it, or on a table that is its own
/// ancestor (`t[1] = t`), the view is refused with an error rather than
/// recursing off the stack.
pub const MAX_VIEW_DEPTH: usize = 128;

thread_local! {
    /// The view tables [`build_node`] is inside, outermost first. The
    /// widget closures between a node and its children cannot carry the
    /// path as an argument.
    static VIEW_PATH: std::cell::RefCell<Vec<*const std::ffi::c_void>> =
        const { std::cell::RefCell::new(Vec::new()) };
    /// The table `view` returned, while it is built: the one table whose
    /// `window_events` the view declared itself, which [`node_props`]
    /// therefore drops rather than handing the core as one off the root.
    static VIEW_ROOT: std::cell::Cell<*const std::ffi::c_void> =
        const { std::cell::Cell::new(std::ptr::null()) };
    /// The folder of the script whose view table is being built, for a
    /// relative `svg` `src` (ADR 0063): set around the build as
    /// `VIEW_ROOT` is.
    static BUILD_DIR: std::cell::Cell<Option<std::rc::Rc<str>>> =
        const { std::cell::Cell::new(None) };
    /// The script whose tree is being built and the namespace it fills
    /// under: a `component` node of its own namespace is filled by it in
    /// place (`Ui::component_by`), since the extension list cannot ask a
    /// script that is busy drawing (ADR 0060). Set around the build as
    /// `VIEW_ROOT` is; the pointers live as long as that build.
    static BUILD_EXT: std::cell::Cell<Option<BuildExt>> = const { std::cell::Cell::new(None) };
}

fn build_node(ui: &mut Ui<'_>, t: &Table) -> mlua::Result<()> {
    let at = t.to_pointer();
    VIEW_PATH.with_borrow_mut(|path| {
        if path.contains(&at) {
            return Err(bad("a view node that holds itself"));
        }
        if path.len() >= MAX_VIEW_DEPTH {
            return Err(bad(format!("a view nested past {MAX_VIEW_DEPTH} nodes")));
        }
        path.push(at);
        Ok(())
    })?;
    let built = build_one(ui, t);
    VIEW_PATH.with_borrow_mut(|path| path.pop());
    built
}

/// [`BUILD_EXT`]'s entry: the script and its namespace, as raw parts,
/// valid for the build that set them.
#[derive(Clone, Copy)]
struct BuildExt {
    ext: *const LuaExtension,
    namespace: *const u8,
    namespace_len: usize,
}

/// A `component` node (ADR 0060): `name` (`namespace/component`),
/// `props` (where `key` and `fresh` are read too), and `children` — its
/// array part the unnamed group, a named group under its name. A
/// component of the building script's own namespace is filled by it in
/// place; any other goes to the extension list.
fn build_component(ui: &mut Ui<'_>, t: &Table) -> mlua::Result<()> {
    let name: String = t
        .get::<Option<String>>("name")?
        .ok_or_else(|| bad("component needs a name, \"namespace/component\""))?;
    let props = match t.get::<mlua::Value>("props")? {
        mlua::Value::Nil => Value::Null,
        v => lua_to_value(&v)?,
    };
    let groups: Option<Table> = t.get("children")?;
    let failed: std::cell::RefCell<Option<mlua::Error>> = Default::default();
    // The script building this node: its children are its own, built when
    // the component asks — by then the component's own build has set the
    // thread-locals to itself, and a child naming the component's
    // namespace would be taken for the component's own, drawn in place
    // under this script's origin.
    let host_ext = BUILD_EXT.get();
    let host_dir = {
        let d = BUILD_DIR.take();
        BUILD_DIR.set(d.clone());
        d
    };
    let children = |group: &str, ui: &mut Ui<'_>| {
        let Some(groups) = &groups else {
            return;
        };
        let outer_ext = BUILD_EXT.replace(host_ext);
        let outer_dir = BUILD_DIR.replace(host_dir.clone());
        struct Restore(Option<BuildExt>, Option<std::rc::Rc<str>>);
        impl Drop for Restore {
            fn drop(&mut self) {
                BUILD_EXT.set(self.0);
                BUILD_DIR.set(self.1.take());
            }
        }
        let _restore = Restore(outer_ext, outer_dir);
        let list = if group.is_empty() {
            Some(groups.clone())
        } else {
            match groups.get::<Option<Table>>(group) {
                Ok(l) => l,
                Err(e) => {
                    failed.borrow_mut().get_or_insert(e);
                    None
                }
            }
        };
        let Some(list) = list else {
            return;
        };
        if let Err(e) = each_child(&list, |n| build_node(ui, &n)) {
            failed.borrow_mut().get_or_insert(e);
        }
    };
    let own = BUILD_EXT.get().filter(|b| {
        // SAFETY: set by `build_root` around this build from a `&str` that
        // outlives it.
        let ns = unsafe { std::slice::from_raw_parts(b.namespace, b.namespace_len) };
        kui_core::slot::split_name(&name).0.as_bytes() == ns
    });
    match own {
        Some(b) => {
            ui.component_by(&name, &props, children, |inst, ui| {
                // SAFETY: the script is borrowed for the build `build_root`
                // runs, which this is inside.
                let ext = unsafe { &*b.ext };
                if let Err(e) = ext.draw_component(inst, ui) {
                    failed.borrow_mut().get_or_insert(mlua::Error::runtime(e));
                }
            });
        }
        // No host children: a placement a kept fill can journal, so the
        // fill placing it stays replayable.
        None if groups.is_none() => {
            ui.component(&name, &props);
        }
        None => {
            ui.component_with(&name, &props, children);
        }
    }
    match failed.into_inner() {
        Some(e) => Err(e),
        None => Ok(()),
    }
}

/// A path an element names, read against the folder of the script being
/// built when it is relative.
fn script_relative(src: &str) -> String {
    let p = std::path::Path::new(src);
    if p.is_relative() {
        let dir = BUILD_DIR.take();
        let joined = dir.as_deref().map(|d| {
            std::path::Path::new(d)
                .join(p)
                .to_string_lossy()
                .into_owned()
        });
        BUILD_DIR.set(dir);
        if let Some(j) = joined {
            return j;
        }
    }
    src.to_owned()
}

/// One view node. The containers are the path a deep view recurses
/// down, so their frame stays small: the props are parsed in
/// [`open_box`] and every other node type in [`build_widget`], each
/// frame gone before the children are built. One function holding the
/// whole match was 64 KB of stack a level in a debug build, and 32
/// nested columns overflowed a test thread.
fn build_one(ui: &mut Ui<'_>, t: &Table) -> mlua::Result<()> {
    // The lazy form as a bare child table (ADR 0069): no constructor set
    // its `type`, and its `render` says what it is.
    let ty: Option<mlua::LuaString> = t.get("type")?;
    let Some(ty) = ty else {
        if matches!(t.get::<mlua::Value>("render")?, mlua::Value::Function(_)) {
            return build_lazy(ui, t);
        }
        return Err(bad(
            "a child table with no `type`: build it with a constructor (row { }, text(...))",
        ));
    };
    let ty = ty.to_str()?;
    let ty: &str = &ty;
    match ty {
        "devtools_tab" => build_devtools_tab(ui, t),
        "row" | "column" | "grid" => {
            open_box(ui, t, ty)?;
            build_children(ui, t)?;
            ui.close();
            Ok(())
        }
        "fragment" => {
            open_fragment(ui, t)?;
            build_children(ui, t)?;
            ui.close();
            Ok(())
        }
        "titlebar" | "tooltip" | "radio_group" => build_holder(ui, t, ty),
        "component" => build_component(ui, t),
        // A contribution slot (ADR 0067): every component the loaded
        // extensions contribute to `name`, placed here as siblings.
        "slot" => {
            let name: String = t.get::<Option<String>>("name")?.ok_or_else(|| {
                mlua::Error::runtime("slot needs a name: slot(\"owner.place\", props)")
            })?;
            let props = match t.get::<mlua::Value>("props")? {
                mlua::Value::Nil => Value::Null,
                v => lua_to_value(&v)?,
            };
            ui.slot(&name, &props);
            Ok(())
        }
        "lazy" => build_lazy(ui, t),
        // Where a component's fill puts its host's children (ADR 0060).
        "children" => {
            let group = t.get::<Option<String>>("group")?.unwrap_or_default();
            ui.children(&group);
            Ok(())
        }
        _ => build_widget(ui, t, ty),
    }
}

/// A lazy branch (ADR 0069): `needs = { w =, h = }` or `query = { … }`,
/// `render` and `fallback` (functions returning a node, or a list of
/// them), `on_query` and `key`. The core decides from the last layout's
/// room, and the function it picks is called while the branch's node is
/// open; neither is called before any layout.
#[inline(never)]
fn build_lazy(ui: &mut Ui<'_>, t: &Table) -> mlua::Result<()> {
    let mut spec = match t.get::<mlua::Value>("query")? {
        mlua::Value::Nil => {
            let needs: Option<Table> = t.get("needs")?;
            let needs = needs.ok_or_else(|| bad("a lazy branch needs `needs` or `query`"))?;
            let axis = |k: &str| -> mlua::Result<Option<f32>> {
                Ok(needs
                    .get::<Option<f64>>(k)?
                    .map(|v| v as f32)
                    .filter(|v| *v > 0.0))
            };
            kui_core::LazySpec::needs(kui_core::Needs {
                w: axis("w")?,
                h: axis("h")?,
            })
        }
        q => {
            let tokens = ui.core().token_lookup();
            let mut refs = kui_core::NameRefs::new(tokens);
            let cond = kui_core::rules::parse_cond(&lua_to_value(&q)?, Some(&mut refs))
                .map_err(|e| bad(format!("lazy query: {e}")))?;
            kui_core::LazySpec::query(cond)
        }
    };
    if let v @ (mlua::Value::Table(_) | mlua::Value::String(_) | mlua::Value::Integer(_)) =
        t.get::<mlua::Value>("on_query")?
    {
        spec = spec.on_query(lua_to_value(&v)?);
    }
    let built = match t.get::<Option<String>>("key")? {
        Some(k) => ui.core().lazy_open_keyed(&k, spec),
        None => ui.core().lazy_open(spec),
    };
    let pick = match built {
        Some(true) => t.get::<Option<mlua::Function>>("render")?,
        Some(false) => t.get::<Option<mlua::Function>>("fallback")?,
        None => None,
    };
    let result = (|| {
        let Some(f) = pick else {
            return Ok(());
        };
        match f.call::<mlua::Value>(())? {
            mlua::Value::Nil => Ok(()),
            mlua::Value::Table(node) => {
                if matches!(node.get::<mlua::Value>("type")?, mlua::Value::Nil)
                    && !matches!(node.get::<mlua::Value>("render")?, mlua::Value::Function(_))
                {
                    // A list of nodes.
                    each_child(&node, |n| build_node(ui, &n))
                } else {
                    build_node(ui, &node)
                }
            }
            other => Err(bad(format!(
                "a lazy branch's function returned a {}, not a node",
                other.type_name()
            ))),
        }
    })();
    ui.core().lazy_close();
    result
}

#[inline(never)]
fn open_box(ui: &mut Ui<'_>, t: &Table, ty: &str) -> mlua::Result<()> {
    let (mut p, _) = node_props(ui, t, ty == "row", element_of(ty))?;
    // A grid is a column whose rows' cells line up (ADR 0033).
    p.spec.layout.table = ty == "grid";
    ui.core().open_from(p, Content::Box);
    Ok(())
}

#[inline(never)]
fn open_fragment(ui: &mut Ui<'_>, t: &Table) -> mlua::Result<()> {
    // Handle from the host (kui_fragment_add / Core::add_fragment) or
    // the script's own `env.add_fragment` (backlog C111), a plain integer
    // like an image's — and `image`, the image handle the function
    // samples (backlog V1), absent or 0 for none.
    let id = handle_at(t, "id", "fragment id")?
        .ok_or_else(|| bad("fragment needs an id, the handle the host added it as"))?;
    let image = handle_at(t, "image", "fragment image")?;
    let params: Vec<f32> = match t.get::<Option<Table>>("params")? {
        Some(list) => list.sequence_values::<f32>().collect::<mlua::Result<_>>()?,
        None => Vec::new(),
    };
    let (p, _) = node_props(ui, t, false, element_of("fragment"))?;
    let frag = kui_core::FragmentRef {
        id: kui_core::FragmentId::from_ffi(id),
        image: image.filter(|i| *i != 0).map(kui_core::ImageId::from_ffi),
    };
    ui.core().open_from(p, Content::Fragment(frag, &params));
    Ok(())
}

/// The widgets that hold children besides the boxes, apart from the
/// rest for the same reason as [`open_box`].
#[inline(never)]
fn build_holder(ui: &mut Ui<'_>, t: &Table, ty: &str) -> mlua::Result<()> {
    if ty != "radio_group" {
        check_props(ui, t, element_of(ty))?;
    }
    match ty {
        "titlebar" => {
            if t.raw_len() > 0 {
                with_children(ui, t, |ui, body| widgets::titlebar_with(ui, body))
            } else {
                let title: String = t.get::<Option<String>>("title")?.unwrap_or_default();
                widgets::titlebar(ui, &title);
                Ok(())
            }
        }
        "tooltip" => {
            if t.raw_len() > 0 {
                with_children(ui, t, |ui, body| widgets::tooltip_with(ui, body))
            } else {
                let value: String = t.get("value")?;
                widgets::tooltip(ui, &value);
                Ok(())
            }
        }
        "radio_group" => {
            // Every box row; the role, the name and, with no `gap`, the
            // stock spacing are the group's (`widgets::radio_group_with`).
            let label: String = t.get("label")?;
            let (p, _) = node_props(ui, t, false, element_of(ty))?;
            let mut result = Ok(());
            widgets::radio_group_with(ui, &label, p.spec, |ui| result = build_children(ui, t));
            result
        }
        other => unreachable!("{other} holds no children"),
    }
}

#[inline(never)]
fn build_widget(ui: &mut Ui<'_>, t: &Table, ty: &str) -> mlua::Result<()> {
    // Set by the arms whose props are the node table's own: the
    // unknown-prop check was made in their walk (`node_props`); the
    // others are checked here after.
    let mut checked = false;
    let built = match ty {
        "text" => {
            checked = true;
            let (p, walked) = node_props(ui, t, false, element_of(ty))?;
            let style = p.style;
            // What the walk found, in the shapes nearly every text has;
            // anything else goes through `get`'s own conversion (a number
            // as its string) and its errors, as before the walk.
            let spans = match walked.spans {
                Some(mlua::Value::Table(spans)) => Some(spans),
                Some(_) => t.get::<Option<Table>>("spans")?,
                None => None,
            };
            if let Some(spans) = spans {
                let parts = with_refs(ui, |refs| collect_spans(&spans, refs))?;
                let spans: Vec<Span<'_>> = parts.iter().map(span_of).collect();
                ui.rich_text(&spans, style);
            } else if let Some(mlua::Value::String(value)) = walked.value {
                ui.text(&value.to_str()?, style);
            } else {
                let value: String = t.get("value")?;
                ui.text(&value, style);
            }
            Ok(())
        }
        "image" => {
            // Handle from the host (kui_image_add / Resources::add_image),
            // passed to scripts as a plain integer. `sampling` and `fit`
            // are the rows ADR 0025 gives the element, by name.
            let id = handle_at(t, "id", "image id")?
                .ok_or_else(|| bad("image needs an id, the handle the host added it as"))?;
            // A leaf: its `tooltip` floats beside it (backlog RG113).
            let spec = props_of(ui, t, ty, &mut checked)?.for_leaf().spec;
            let named = |row: &str, names: &[&str]| -> mlua::Result<usize> {
                match t.get::<Option<String>>(row)? {
                    None => Ok(0),
                    Some(s) => names
                        .iter()
                        .position(|n| *n == s)
                        .ok_or_else(|| bad(format!("{row} must be one of {}", names.join(", ")))),
                }
            };
            let sampling_names: Vec<&str> =
                kui_core::Sampling::ALL.iter().map(|s| s.name()).collect();
            let fit_names: Vec<&str> = kui_core::ImageFit::ALL.iter().map(|f| f.name()).collect();
            let opts = kui_core::ImageOpts {
                sampling: kui_core::Sampling::ALL[named("sampling", &sampling_names)?],
                fit: kui_core::ImageFit::ALL[named("fit", &fit_names)?],
            };
            ui.image_with(kui_core::ImageId::from_ffi(id), opts, spec);
            Ok(())
        }
        "polygon" => {
            // `points`, each a `{x, y}` pair; the fill is the `bg` row, read
            // by parse_props like any node's (ADR 0025, decision 6).
            let p = props_of(ui, t, ty, &mut checked)?;
            let Some(list) = t.get::<Option<Table>>("points")? else {
                return Err(bad("polygon needs points"));
            };
            let points: Vec<kui_core::Vec2> = list
                .sequence_values::<mlua::Value>()
                .map(|v| {
                    let mlua::Value::Table(pt) = v? else {
                        return Err(bad("a polygon point is a {x, y} table"));
                    };
                    Ok(kui_core::Vec2::new(pt.get(1)?, pt.get(2)?))
                })
                .collect::<mlua::Result<_>>()?;
            ui.core().open_from(p, Content::Polygon(&points));
            Ok(())
        }
        "svg" => {
            // `data` (the markup) or `src` (a file), `mode` and `fit` by
            // name; `color` is the text-colour row, read off the parsed
            // style, and what `currentColor` paints (ADR 0063).
            let p = props_of(ui, t, ty, &mut checked)?;
            let named = |row: &str, names: &[&str]| -> mlua::Result<usize> {
                match t.get::<Option<String>>(row)? {
                    None => Ok(0),
                    Some(s) => names.iter().position(|n| *n == s).ok_or_else(|| {
                        bad(format!("svg {row} must be one of {}", names.join(", ")))
                    }),
                }
            };
            let mode = kui_core::SvgMode::ALL[named("mode", &kui_core::SvgMode::NAMES)?];
            let fit = kui_core::SvgFit::ALL[named("fit", &kui_core::SvgFit::NAMES)?];
            let data = t.get::<Option<mlua::LuaString>>("data")?;
            let src = t.get::<Option<String>>("src")?;
            match (&data, src) {
                (Some(d), _) => {
                    let bytes = d.as_bytes();
                    ui.core().open_from(
                        p,
                        Content::Svg(kui_core::SvgSource::Data(&bytes), mode, fit),
                    );
                }
                (None, Some(src)) => {
                    let path = script_relative(&src);
                    ui.core()
                        .open_from(p, Content::Svg(kui_core::SvgSource::File(&path), mode, fit));
                }
                (None, None) => return Err(bad("svg needs data (the markup) or src (a file)")),
            }
            Ok(())
        }
        "path" => {
            // `d` (SVG path data, parsed in the core) or `ops` (the flat op
            // form, a list of numbers); the fill is the `bg` row, `fill_rule`
            // its rule; `width` and `color` are the stroke's, as a line's,
            // and no `width` is no stroke (ADR 0040).
            let mut p = props_of(ui, t, ty, &mut checked)?;
            // A path's `rotate` is its own (ADR 0041), not the node's
            // (ADR 0043, decision 1): the generic walk above read it as
            // the row every other element takes, and that reading goes.
            // The node's `scale` and pivot stay, as Node's encoder keeps
            // them.
            if let Some(i) = p.spec.interact.as_deref_mut()
                && let Some(tr) = i.transform.as_deref_mut()
            {
                tr.rotate = 0.0;
                if *tr == kui_core::TransformSpec::NONE {
                    i.transform = None;
                }
            }
            let rule = match t.get::<Option<String>>("fill_rule")? {
                Some(name) => kui_core::FillRule::parse(&name)
                    .ok_or_else(|| bad("fill_rule is \"nonzero\" or \"evenodd\""))?,
                None => kui_core::FillRule::NonZero,
            };
            let width = match t.get::<mlua::Value>("width")? {
                mlua::Value::Nil => None,
                v => with_refs(ui, |refs| length_of(&v, refs))?,
            };
            let dash = dash_of(t)?;
            let stroke = width.filter(|w| *w > 0.0).map(|w| {
                kui_core::Stroke::new(w, p.style.color.unwrap_or(ui.theme().fg)).dashed(dash)
            });
            // `rotate` in turns and `pivot = {x, y}`: the turn is the
            // quad's, and either row asks for the box the turn sweeps
            // (ADR 0041).
            let turns = t.get::<Option<f32>>("rotate")?;
            let pivot = match t.get::<Option<Table>>("pivot")? {
                Some(p) => Some(kui_core::Vec2::new(p.get::<f32>(1)?, p.get::<f32>(2)?)),
                None => None,
            };
            let turn = (turns.is_some() || pivot.is_some()).then_some(kui_core::Turn {
                turns: turns.unwrap_or(0.0),
                pivot,
            });
            if let Some(d) = t.get::<Option<String>>("d")? {
                ui.core()
                    .open_from(p, Content::PathD(&d, rule, stroke, turn));
            } else if let Some(list) = t.get::<Option<Table>>("ops")? {
                let floats: Vec<f32> =
                    list.sequence_values::<f32>().collect::<mlua::Result<_>>()?;
                ui.core()
                    .open_from(p, Content::PathFlat(&floats, rule, stroke, turn));
            } else {
                return Err(bad("path needs d or ops"));
            }
            Ok(())
        }
        "line" => {
            // `from`/`to` or `points`, each point a `{x, y}` pair. `width`
            // parses as a sizing row too, harmlessly: the core overrides a
            // line's sizing with its own box. `color` is the text-colour
            // row, read off the parsed style, so it defaults to the
            // foreground like a text node's.
            let p = props_of(ui, t, ty, &mut checked)?;
            let point = |v: mlua::Value| -> mlua::Result<kui_core::Vec2> {
                let mlua::Value::Table(pt) = v else {
                    return Err(bad("a line point is a {x, y} table"));
                };
                Ok(kui_core::Vec2::new(pt.get(1)?, pt.get(2)?))
            };
            let points: Vec<kui_core::Vec2> = match t.get::<Option<Table>>("points")? {
                Some(list) => list
                    .sequence_values::<mlua::Value>()
                    .map(|v| point(v?))
                    .collect::<mlua::Result<_>>()?,
                None => {
                    let (Some(from), Some(to)) = (
                        t.get::<Option<mlua::Value>>("from")?,
                        t.get::<Option<mlua::Value>>("to")?,
                    ) else {
                        return Err(bad("line needs from and to, or points"));
                    };
                    vec![point(from)?, point(to)?]
                }
            };
            // A `$name` width is a length token; one that misses is the
            // default stroke, 1 px (AR14).
            let width = match t.get::<mlua::Value>("width")? {
                mlua::Value::Nil => None,
                v => with_refs(ui, |refs| length_of(&v, refs))?,
            }
            .unwrap_or(1.0);
            // No `color` is the theme's foreground, as for a text run.
            let stroke_color = p.style.color.unwrap_or(ui.theme().fg);
            let mut stroke = kui_core::Stroke::new(width, stroke_color);
            stroke.curve = t.get::<Option<bool>>("curve")?.unwrap_or(false);
            stroke.dash = dash_of(t)?;
            ui.core().open_from(p, Content::Line(&points, stroke));
            Ok(())
        }
        "cells" => {
            // A string per row in `lines`, cells past a row's end blank;
            // `runs` of {row, col, len, fg, bg, flags, ul} colour and
            // attribute spans over them (0 keeps the default; `ul` is the
            // underline's own colour, backlog K4). The style rows size
            // the cells; the node rows are the node's.
            let p = props_of(ui, t, ty, &mut checked)?;
            // Counts and positions are measures (backlog F180): `rows =
            // room_h / line_h` is the rows that fit. A window shorter
            // than one line fits none, and 0 rows or 0 cols is an empty
            // grid — the node still declared under its key, sized as the
            // core sizes no cells, so its focus and selection outlive
            // the frame it had no room in (backlog RG160). Only a count
            // left out is refused.
            let (Some(rows), Some(cols)) = (
                measure_at(t, "rows", "cells rows")?,
                measure_at(t, "cols", "cells cols")?,
            ) else {
                return Err(bad("cells needs rows and cols"));
            };
            let (rows, cols) = (to_usize(rows), to_usize(cols));
            let default_fg = p.style.color.unwrap_or(ui.theme().fg).to_hex();
            // No more than the core draws (`MAX_CELLS`, which it warns
            // past): the grid it keeps is a prefix of this one, row-major
            // at `cols`, so a cell past the buffer is one it cuts anyway,
            // and a `rows = math.huge` is a warning, not an allocation.
            let n = rows.saturating_mul(cols).min(kui_core::cells::MAX_CELLS);
            let mut cells = vec![kui_core::Cell::new(' ', default_fg, 0); n];
            // Row `r` of the buffer, from its first cell to the buffer's
            // end, or `None` once the row starts past it. A row is found
            // by a checked multiply and a cell inside it by indexing the
            // slice, so `cols = math.huge` (a `usize::MAX`) cuts the rows
            // past the first instead of overflowing `r * cols + c` into a
            // panic, or in a release build into a wrong cell.
            fn row_of(
                cells: &mut [kui_core::Cell],
                r: usize,
                cols: usize,
            ) -> Option<&mut [kui_core::Cell]> {
                cells.get_mut(r.checked_mul(cols)?..)
            }
            if let Some(lines) = t.get::<Option<Table>>("lines")? {
                for (r, line) in lines.sequence_values::<String>().enumerate() {
                    if r >= rows {
                        break;
                    }
                    let line = line?;
                    let Some(row) = row_of(&mut cells, r, cols) else {
                        break;
                    };
                    for (cell, ch) in row.iter_mut().zip(line.chars().take(cols)) {
                        cell.ch = ch;
                    }
                }
            }
            if let Some(runs) = t.get::<Option<Table>>("runs")? {
                let word = |run: &Table, i: usize, what: &str| -> mlua::Result<u32> {
                    match run.get::<mlua::Value>(i)? {
                        mlua::Value::Nil => Ok(0),
                        v => color_word(&v).ok_or_else(|| {
                            bad(format!("cells run {what} must be a 0xRRGGBBAA number"))
                        }),
                    }
                };
                for run in runs.sequence_values::<Table>() {
                    let run = run?;
                    let at = |i: usize, what: &str| -> mlua::Result<usize> {
                        measure_arg(&run.get::<mlua::Value>(i)?, what)
                    };
                    let row = at(1, "cells run row")?;
                    let col = at(2, "cells run col")?;
                    let len = at(3, "cells run len")?;
                    let fg = word(&run, 4, "fg")?;
                    let bg = word(&run, 5, "bg")?;
                    // A bit set: its low byte, as C's and Node's carry it.
                    let flags = measure_at(&run, 6, "cells run flags")?.unwrap_or(0) as u8;
                    let ul = word(&run, 7, "ul")?;
                    if row >= rows {
                        continue;
                    }
                    let Some(line) = row_of(&mut cells, row, cols) else {
                        continue;
                    };
                    let end = col.saturating_add(len).min(cols);
                    for cell in line.iter_mut().take(end).skip(col) {
                        if fg != 0 {
                            cell.fg = fg;
                        }
                        if bg != 0 {
                            cell.bg = bg;
                        }
                        cell.flags |= flags;
                        if ul != 0 {
                            cell.ul = ul;
                        }
                    }
                }
            }
            let cursor = match t.get::<Option<Table>>("cursor_at")? {
                Some(cur) => {
                    // An unknown name is refused, as Node refuses it —
                    // not folded to a block (backlog AR40).
                    let shape = match t.get::<Option<String>>("cursor_shape")? {
                        None => kui_core::CellCursor::Block,
                        Some(s) => kui_core::CellCursor::from_name(&s).ok_or_else(|| {
                            bad(format!(
                                "cursor_shape must be {}, not {s:?}",
                                kui_core::CellCursor::NAMES.join(" | ")
                            ))
                        })?,
                    };
                    let color = match t.get::<mlua::Value>("cursor_color")? {
                        mlua::Value::Nil => None,
                        v => with_refs(ui, |refs| parse_color(&v, refs))?,
                    }
                    .unwrap_or(Color::rgb8(0xff, 0xff, 0xff));
                    let row = measure_arg(&cur.get::<mlua::Value>(1)?, "cells cursor_at row")?;
                    let col = measure_arg(&cur.get::<mlua::Value>(2)?, "cells cursor_at col")?;
                    Some((row, col, shape, color))
                }
                None => None,
            };
            // The absolute line row 0 is; 0 when the app says nothing. No
            // later than `MAX_ORIGIN_LINE`, so `math.huge` (a `u64::MAX`)
            // is a line the core can add its rows to.
            let origin_line = measure_at(t, "origin_line", "cells origin_line")?
                .unwrap_or(0)
                .min(MAX_ORIGIN_LINE);
            let grid = kui_core::CellGrid {
                rows,
                cols,
                cells: &cells,
                style: p.style,
                cursor,
                origin_line,
            };
            ui.core().open_from(p, Content::Cells(&grid));
            Ok(())
        }
        "audio" => {
            // Handle from the host (kui_sound_add / Core::add_sound), passed
            // to scripts as a plain integer, like images.
            let src = handle_at(t, "src", "audio src")?
                .ok_or_else(|| bad("audio needs a src, the handle the host added the sound as"))?;
            let mut spec = kui_core::AudioSpec::new(kui_core::SoundId::from_ffi(src));
            if let Some(v) = t.get::<Option<f32>>("volume")? {
                spec = spec.volume(v);
            }
            if t.get::<Option<bool>>("loop")?.unwrap_or(false) {
                spec = spec.looped();
            }
            spec = spec.paused(t.get::<Option<bool>>("paused")?.unwrap_or(false));
            if t.get::<Option<bool>>("finish")?.unwrap_or(false) {
                spec = spec.finish();
            }
            if let Some(tag) = t.get::<Option<mlua::Value>>("tag")? {
                spec.tag = Some(lua_to_value(&tag)?);
            }
            match t.get::<Option<String>>("key")? {
                Some(k) => ui.audio_keyed(&k, spec),
                None => ui.audio(spec),
            };
            Ok(())
        }
        "input" => {
            // The stock field (`widgets::text_input_with`): its text is
            // `value` when the view declares one every frame, else the
            // `initial` seed, and its change, Enter and Escape arrive as
            // the messages given — each one left out is the field alone
            // on its key (ADR 0047).
            let label: String = t.get("label")?;
            let (text, value) = editor_text(t)?;
            let message = |name: &str| -> mlua::Result<Option<Value>> {
                t.get::<Option<mlua::Value>>(name)?
                    .map(|v| lua_to_value(&v))
                    .transpose()
            };
            let messages = widgets::InputMessages {
                change: message("on_change")?,
                submit: message("on_submit")?,
                cancel: message("on_cancel")?,
            };
            let key = widgets::text_input_with(ui, &label, &text, value.declared, &messages);
            value.warn(ui, key, &label);
            Ok(())
        }
        "dropdown" => {
            // The stock select (`widgets::select_items`): the options are
            // strings or the row tables `env.open_menu` takes, read by the
            // core's one reader; `current` counts from 1.
            let Some(label) = t.get::<Option<String>>("label")?.filter(|l| !l.is_empty()) else {
                return Err(bad("dropdown needs a label (its key and accessible name)"));
            };
            let Some(options) = t.get::<Option<Table>>("options")? else {
                return Err(bad(
                    "dropdown needs options, a list of strings or menu rows",
                ));
            };
            let mut rows = lua_list_to_value(&options)?;
            if let Value::List(rows) = &mut rows {
                rows.iter_mut().for_each(alias_menu_role);
            }
            // A key of a row table no row reads — `disabled` for
            // `enabled = false` — is dropped by the reader, so it is
            // reported as an unknown prop is (backlog RG10).
            if ui.core().diagnostics() {
                for k in kui_core::MenuItem::stray_option_keys(&rows) {
                    ui.core().warn(kui_core::diag::unknown_menu_item_key(&k));
                }
            }
            let items =
                kui_core::MenuItem::options_from_value(&rows).map_err(mlua::Error::runtime)?;
            let current = match t.get::<mlua::Value>("current")? {
                mlua::Value::Nil => None,
                v => match measure_arg(&v, "dropdown current")? {
                    0 => {
                        return Err(bad(format!(
                            "dropdown current is an index from 1, not {}",
                            v.to_string()?
                        )));
                    }
                    i => Some(i - 1),
                },
            };
            // `on_menu` is the message the choice arrives as, `menu = {
            // role, item }` in it; without one the field still reports,
            // as the field alone (ADR 0047).
            match t.get::<Option<mlua::Value>>("on_menu")? {
                Some(msg) => {
                    let msg = lua_to_value(&msg)?;
                    let (theme, m) = (ui.theme(), ui.metrics());
                    let spec = widgets::select_spec(&theme, &m).on_menu(msg);
                    let text = kui_core::TextStyle::new(m.chrome_text);
                    widgets::select_with(ui, &label, &items, current, spec, text);
                }
                None => {
                    widgets::select_items(ui, &label, &items, current);
                }
            }
            Ok(())
        }
        "edit" => {
            // A leaf: its `tooltip` floats beside it (backlog RG113).
            let p = props_of(ui, t, ty, &mut checked)?.for_leaf();
            let label = match p.key.clone().or(t.get::<Option<String>>("label")?) {
                Some(l) => l,
                None => return Err(bad("edit needs a key (state is retained by key)")),
            };
            let (text, value) = editor_text(t)?;
            let opts = EditOptions {
                value: value.declared,
                style: p.style,
                multiline: t.get::<Option<bool>>("multiline")?.unwrap_or(false),
                autofocus: t.get::<Option<bool>>("autofocus")?.unwrap_or(false),
                keep_tab: t.get::<Option<bool>>("keep_tab")?.unwrap_or(false),
                tab_moves_focus: t.get::<Option<bool>>("tab_moves_focus")?.unwrap_or(false),
                secure: t.get::<Option<bool>>("secure")?.unwrap_or(false),
                placeholder: t.get::<Option<String>>("placeholder")?,
                wrap: p.wrap,
                ..Default::default()
            };
            let key_focus = p.key_focus;
            let key = ui.text_edit(&label, &text, &opts, p.spec);
            // `key_focus` on an editor: the node takes focus on the frame
            // it starts being declared, as on any other node. The editor
            // path never opens through `open_from`, which is where the row
            // is honoured, and dropped it: a field asking for the keyboard
            // as it showed (Kit's `autofocus`) never got it.
            if key_focus {
                ui.core().set_key_focus(Some(key));
            }
            value.warn(ui, key, &label);
            Ok(())
        }
        "window_buttons" => {
            widgets::window_buttons(ui);
            Ok(())
        }
        "menu_bar" => {
            let (bar, stray) = menu_bar_of(t)?;
            warn_stray_menu_keys(ui.core(), stray);
            widgets::menu_bar(ui, bar);
            Ok(())
        }
        "latency_graph" => {
            widgets::latency_graph(ui);
            Ok(())
        }
        "latency_hud" => {
            let (mut x, mut y) = (Align::End, Align::End);
            if let Some(at) = t.get::<Option<Table>>("at")? {
                x = parse_align(&at.get::<String>(1)?)?;
                y = parse_align(&at.get::<String>(2)?)?;
            }
            widgets::latency_hud_at(ui, x, y);
            Ok(())
        }
        "button" => {
            // `label` is the accessible name, and the text too unless
            // `text` says otherwise — the one string a script always gave
            // its button is the row a reader hears first. The other rows
            // the stock button admits (`schema::BUTTON_ROWS_LUA`) are read
            // by name over `widgets::button_spec`, as the JSX encoder and
            // `kui_button_with` read them: the look stays the widget's.
            // The tooltip goes first so an explicit `description` wins
            // over the shorthand, as it does in C — a table has no order
            // to make "the later one" mean anything.
            let label: String = t.get("label")?;
            let text: String = t
                .get::<Option<String>>("text")?
                .unwrap_or_else(|| label.clone());
            let key: String = t
                .get::<Option<String>>("key")?
                .unwrap_or_else(|| label.clone());
            let payload = match t.get::<Option<mlua::Value>>("on_click")? {
                Some(v) => lua_to_value(&v)?,
                None => Value::Null,
            };
            let mut out = PropsOut::new();
            out.spec = widgets::button_spec(&ui.theme(), &ui.metrics())
                .on_click(payload)
                .label(label.as_str());
            if let Some(hint) = t.get::<Option<String>>("tooltip")? {
                out.apply_tooltip(&hint);
            }
            with_refs(ui, |refs| {
                for name in ["description", "disabled", "accent"] {
                    let v = t.get::<mlua::Value>(name)?;
                    if v.is_nil() {
                        continue;
                    }
                    let def = schema::by_snake_name(name).expect("a button row");
                    if let Some(parsed) =
                        parse_value(&def.kind, &v, refs).map_err(|e| bad(format!("{name}: {e}")))?
                    {
                        schema::apply(def, parsed, &mut out).map_err(bad)?;
                    }
                }
                Ok(())
            })?;
            // An `index` keys the button by its row, as it does a box
            // (backlog AR40): declared beside `key`, the index wins.
            match measure_at(t, "index", "button index")? {
                Some(i) => widgets::button_indexed(ui, i, &text, out.spec, out.tooltip.as_deref()),
                None => widgets::button_with(ui, &key, &text, out.spec, out.tooltip.as_deref()),
            }
            Ok(())
        }
        "checkbox" | "radio" | "switch" => {
            // The button's shape (docs/adr/0034): `label` is the name and
            // the text unless `text` says otherwise, and the rows the
            // toggle admits (`schema::TOGGLE_ROWS_LUA`) are read by name
            // over `widgets::toggle_spec`.
            let kind = match ty {
                "checkbox" => widgets::Toggle::Checkbox,
                "radio" => widgets::Toggle::Radio,
                _ => widgets::Toggle::Switch,
            };
            let label: String = t.get("label")?;
            let text: String = t
                .get::<Option<String>>("text")?
                .unwrap_or_else(|| label.clone());
            let key: String = t
                .get::<Option<String>>("key")?
                .unwrap_or_else(|| label.clone());
            let payload = match t.get::<Option<mlua::Value>>("on_click")? {
                Some(v) => lua_to_value(&v)?,
                None => Value::Null,
            };
            let mut out = PropsOut::new();
            out.spec = widgets::toggle_spec(&ui.metrics())
                .on_click(payload)
                .label(label.as_str());
            if let Some(hint) = t.get::<Option<String>>("tooltip")? {
                out.apply_tooltip(&hint);
            }
            apply_named_rows(
                ui,
                t,
                &[
                    "description",
                    "disabled",
                    "checked",
                    "mixed",
                    "wrap",
                    "max_lines",
                    "ellipsis",
                ],
                &mut out,
            )?;
            widgets::toggle_with_style(
                ui,
                kind,
                &key,
                &text,
                out.spec,
                out.tooltip.as_deref(),
                &out.style,
            );
            Ok(())
        }
        "slider" => {
            // Keyed by `label`, which is its name too; the value rows, its
            // change message and its width are read by name over
            // `widgets::slider_spec` (`schema::SLIDER_ROWS_LUA`).
            let label: String = t.get("label")?;
            let key: String = t
                .get::<Option<String>>("key")?
                .unwrap_or_else(|| label.clone());
            let mut out = PropsOut::new();
            out.spec = widgets::slider_spec(&ui.metrics()).label(label.as_str());
            if let Some(hint) = t.get::<Option<String>>("tooltip")? {
                out.apply_tooltip(&hint);
            }
            apply_named_rows(
                ui,
                t,
                &[
                    "description",
                    "disabled",
                    "value_now",
                    "value_min",
                    "value_max",
                    "value_step",
                    "value_text",
                    "on_change",
                    "width",
                    "min_width",
                    "max_width",
                ],
                &mut out,
            )?;
            widgets::slider_with(ui, &key, out.spec, out.tooltip.as_deref());
            Ok(())
        }
        other => Err(mlua::Error::runtime(format!("unknown node type '{other}'"))),
    };
    if !checked {
        check_props(ui, t, element_of(ty))?;
    }
    built
}

/// What an editor table declares its text as: `value`, declared every
/// frame and brought into the editor where they differ, or else the
/// `initial` seed (ADR 0047, decision 8). Returns the text to hand the
/// core and whether it is the value.
fn editor_text(t: &Table) -> mlua::Result<(String, EditorValue)> {
    let initial = t.get::<Option<String>>("initial")?;
    let value = t.get::<Option<String>>("value")?;
    let both = initial.is_some() && value.is_some();
    let declared = value.is_some();
    let text = value.or(initial).unwrap_or_default();
    Ok((text, EditorValue { declared, both }))
}

/// Whether an editor table declared a `value`, and whether it declared
/// `initial` beside it — which the value wins over, said as
/// `edit-value-and-initial` once the editor has its key.
struct EditorValue {
    declared: bool,
    both: bool,
}

impl EditorValue {
    fn warn(&self, ui: &mut Ui<'_>, key: Key, label: &str) {
        if self.both {
            ui.core()
                .warn(kui_core::diag::edit_value_and_initial(key, label));
        }
    }
}

/// Reads the rows `names` off `t` by their Lua names and applies them over
/// `out` through the schema, the way the stock button reads its rows: a
/// widget whose look is its spec takes a closed list of rows, never the
/// whole prop list.
fn apply_named_rows(
    ui: &mut Ui<'_>,
    t: &Table,
    names: &[&str],
    out: &mut PropsOut,
) -> mlua::Result<()> {
    with_refs(ui, |refs| {
        for name in names {
            let v = t.get::<mlua::Value>(*name)?;
            if v.is_nil() {
                continue;
            }
            let def = schema::by_snake_name(name).expect("a schema row");
            if let Some(parsed) =
                parse_value(&def.kind, &v, refs).map_err(|e| bad(format!("{name}: {e}")))?
            {
                schema::apply(def, parsed, out).map_err(bad)?;
            }
        }
        Ok(())
    })
}

struct SpanPart {
    text: String,
    bold: bool,
    italic: bool,
    underline: bool,
    /// `underline_color` / `underline_style`; either implies
    /// `underline`.
    underline_color: Option<Color>,
    underline_style: Option<kui_core::UnderlineStyle>,
    strikethrough: bool,
    color: Option<Color>,
    bg: Option<Color>,
    /// `bg_radius`: the background rounded, one shape with the ones it
    /// meets.
    bg_radius: f32,
    /// `family` (a stock name or an installed family's) or `font` (a
    /// handle, which wins): the span's own face.
    family: Option<kui_core::FontFamily>,
    /// `size`: the span's own, logical px.
    size: Option<f32>,
}

fn span_of(p: &SpanPart) -> Span<'_> {
    let mut s = Span::new(&p.text);
    if p.bold {
        s = s.bold();
    }
    if p.italic {
        s = s.italic();
    }
    if p.underline {
        s = s.underline();
    }
    if let Some(c) = p.underline_color {
        s = s.underline_color(c);
    }
    if let Some(st) = p.underline_style {
        s = s.underline_style(st);
    }
    if p.strikethrough {
        s = s.strikethrough();
    }
    if let Some(c) = p.color {
        s = s.color(c);
    }
    if let Some(c) = p.bg {
        s = s.bg(c);
    }
    if p.bg_radius > 0.0 {
        s = s.bg_radius(p.bg_radius);
    }
    if let Some(f) = p.family {
        s = s.family(f);
    }
    if let Some(px) = p.size {
        s = s.size(px);
    }
    s
}

/// `{ "plain", { "styled", bold = true, italic = true, color = 0x.. }, ... }`
fn collect_spans(spans: &Table, refs: &mut Refs<'_>) -> mlua::Result<Vec<SpanPart>> {
    let mut out = Vec::new();
    for item in spans.sequence_values::<mlua::Value>() {
        match item? {
            mlua::Value::String(s) => out.push(SpanPart {
                text: s.to_str()?.to_string(),
                bold: false,
                italic: false,
                underline: false,
                underline_color: None,
                underline_style: None,
                strikethrough: false,
                color: None,
                bg: None,
                bg_radius: 0.0,
                family: None,
                size: None,
            }),
            mlua::Value::Table(t) => out.push(span_part(&t, refs)?),
            other => {
                return Err(bad(format!(
                    "span must be a string or table, got {}",
                    other.type_name()
                )));
            }
        }
    }
    Ok(out)
}

/// One span table: its text at `[1]` and its style keys, read in one
/// `pairs` walk rather than a lookup for each of the twelve keys a span
/// may carry and seldom does (backlog F143: a text of spans looked up
/// thirteen keys a span, most of them absent). A key no span reads is
/// passed over, as a lookup never asked for it. A value in a shape the
/// walk does not take is read again through `get`, whose conversion and
/// errors are the ones a span had before.
fn span_part(t: &Table, refs: &mut Refs<'_>) -> mlua::Result<SpanPart> {
    let mut text = None;
    let (mut color, mut bg, mut underline_color) = (None, None, None);
    let (mut underline_style, mut font, mut family) = (None, None, None);
    let (mut size, mut bg_radius) = (None, None);
    let (mut bold, mut italic, mut underline, mut strikethrough) = (false, false, false, false);
    // A boolean key as `get::<Option<bool>>` reads it: nil false, a
    // boolean itself, anything else true.
    let flag = |v: &mlua::Value| !matches!(v, mlua::Value::Nil | mlua::Value::Boolean(false));
    t.for_each::<NodeKey, mlua::Value>(|k, v| {
        match &k {
            NodeKey::Int(1) => text = Some(v),
            NodeKey::Str { .. } => match k.bytes() {
                b"color" => color = Some(v),
                b"bg" => bg = Some(v),
                b"underline_color" => underline_color = Some(v),
                b"underline_style" => underline_style = Some(v),
                b"font" => font = Some(v),
                b"family" => family = Some(v),
                b"size" => size = Some(v),
                b"bg_radius" => bg_radius = Some(v),
                b"bold" => bold = flag(&v),
                b"italic" => italic = flag(&v),
                b"underline" => underline = flag(&v),
                b"strikethrough" => strikethrough = flag(&v),
                _ => {}
            },
            _ => {}
        }
        Ok(())
    })?;
    let text = match text {
        Some(mlua::Value::String(s)) => s.to_str()?.to_string(),
        _ => t
            .get::<Option<String>>(1)?
            .ok_or_else(|| bad("span table needs its text at [1]"))?,
    };
    let color = match color {
        Some(v) => parse_color(&v, refs)?,
        None => None,
    };
    let bg = match bg {
        Some(v) => parse_color(&v, refs)?,
        None => None,
    };
    let underline_color = match underline_color {
        Some(v) => parse_color(&v, refs)?,
        None => None,
    };
    let underline_style = match underline_style {
        None => None,
        Some(_) => {
            let name: String = t.get("underline_style")?;
            Some(
                kui_core::UnderlineStyle::NAMES
                    .iter()
                    .position(|n| *n == name)
                    .map(|i| kui_core::UnderlineStyle::from_index(i as u32))
                    .ok_or_else(|| {
                        bad(format!(
                            "underline_style must be one of {}, got {name:?}",
                            kui_core::UnderlineStyle::NAMES.join(" | ")
                        ))
                    })?,
            )
        }
    };
    // The span's face: `font` (a handle) over `family` (a name), as a
    // text's own style reads them.
    let family = match (font, family) {
        (Some(v), _) => match parse_value(&Kind::Resource, &v, refs)
            .map_err(|e| bad(format!("span font: {e}")))?
        {
            Some(Parsed::Resource(id)) => {
                Some(kui_core::FontFamily::Custom(kui_core::FontId::from_ffi(id)))
            }
            _ => None,
        },
        (None, Some(v)) => match parse_value(&Kind::Family, &v, refs)
            .map_err(|e| bad(format!("span family: {e}")))?
        {
            Some(Parsed::Family(f)) => Some(f),
            _ => None,
        },
        (None, None) => None,
    };
    let number = |v: Option<mlua::Value>, key: &str| -> mlua::Result<Option<f32>> {
        match v {
            None => Ok(None),
            Some(v) => match number(&v) {
                Some(n) => Ok(Some(n)),
                None => t.get::<Option<f32>>(key),
            },
        }
    };
    Ok(SpanPart {
        family,
        size: number(size, "size")?,
        text,
        bold,
        italic,
        underline,
        underline_color,
        underline_style,
        strikethrough,
        color,
        bg,
        bg_radius: number(bg_radius, "bg_radius")?.unwrap_or(0.0).max(0.0),
    })
}

/// Reads a `tokens` table as a [`kui_core::Tokens`].
///
/// The shape is `{ colors = { name = colour | { light =, dark = } | { from =,
/// ops = } }, lengths = { name = px }, families = { name = family | { family,
/// ... } } }`. A family is a name or a fallback list of them, the first that
/// resolves picked by the core (ADR 0049). A colour with `from` is derived
/// from an earlier token by its `ops`, a list of `{ verb, ... }` tuples.
/// Names are sorted, since a Lua table's iteration order is not one a
/// declaration can promise; derived tokens are declared after the values
/// they name, and one whose source never arrives is left for the core to
/// drop with `unknown-token`.
pub fn parse_tokens(t: &Table) -> mlua::Result<kui_core::Tokens> {
    let mut out = kui_core::Tokens::new();
    for pair in t.pairs::<String, mlua::Value>() {
        let (k, _) = pair?;
        if k != "colors" && k != "lengths" && k != "families" && k != "export" {
            return Err(bad(format!(
                "tokens: unknown key {k:?} (colors, lengths, families, export)"
            )));
        }
    }
    // Readable from every origin (an add-on's palette, Kit's `$kitPanel`).
    if t.get::<Option<bool>>("export")? == Some(true) {
        out = out.exported();
    }
    if let Some(colors) = t.get::<Option<Table>>("colors")? {
        let mut entries: Vec<(String, mlua::Value)> = colors
            .pairs::<String, mlua::Value>()
            .collect::<mlua::Result<_>>()?;
        entries.sort_by(|a, b| a.0.cmp(&b.0));
        // Values first; a derived token waits with its recipe parsed.
        let mut derived: Vec<(String, String, Vec<kui_core::ColorOp>)> = Vec::new();
        for (name, v) in entries {
            out = match &v {
                mlua::Value::Table(t) if t.contains_key("from")? => {
                    let (from, ops) = parse_recipe(&name, t)?;
                    derived.push((name, from, ops));
                    out
                }
                mlua::Value::Table(halves) => {
                    let half = |k: &str| -> mlua::Result<Color> {
                        match halves.get::<mlua::Value>(k)? {
                            mlua::Value::Nil => Err(bad(format!(
                                "tokens.colors.{name}: needs both light and dark"
                            ))),
                            v => parse_color_value(&v),
                        }
                    };
                    out.color_themed(&name, half("light")?, half("dark")?)
                }
                v => out.color(
                    &name,
                    parse_color_value(v).map_err(|e| bad(format!("tokens.colors.{name}: {e}")))?,
                ),
            };
        }
        // Then the derived, in name order among those whose sources are
        // all declared, until none can be; what is left goes in as is.
        while !derived.is_empty() {
            let ready = derived.iter().position(|(_, from, ops)| {
                let known = |s: &str| kui_core::tokens::is_role(s) || out.color_id(s).is_some();
                known(from)
                    && ops.iter().all(|op| match op {
                        kui_core::ColorOp::Mix(c, _) | kui_core::ColorOp::Readable(c, _) => {
                            known(c)
                        }
                        _ => true,
                    })
            });
            let (name, from, ops) = derived.remove(ready.unwrap_or(0));
            out = out.derive(&name, &from, ops);
        }
    }
    if let Some(lengths) = t.get::<Option<Table>>("lengths")? {
        let mut entries: Vec<(String, mlua::Value)> = lengths
            .pairs::<String, mlua::Value>()
            .collect::<mlua::Result<_>>()?;
        entries.sort_by(|a, b| a.0.cmp(&b.0));
        for (name, v) in entries {
            let px = number(&v)
                .ok_or_else(|| bad(format!("tokens.lengths.{name}: a length is a number")))?;
            out = out.length(&name, px);
        }
    }
    if let Some(families) = t.get::<Option<Table>>("families")? {
        let mut entries: Vec<(String, mlua::Value)> = families
            .pairs::<String, mlua::Value>()
            .collect::<mlua::Result<_>>()?;
        entries.sort_by(|a, b| a.0.cmp(&b.0));
        for (name, v) in entries {
            let bad_family = || {
                bad(format!(
                    "tokens.families.{name}: a family is a name or a list of names"
                ))
            };
            // `{}` is a list too, a token that is always sans with
            // `unknown-family`, as in the other bindings; a table of only
            // named keys is no list.
            let names: Vec<String> = match &v {
                mlua::Value::String(s) => vec![s.to_str()?.to_string()],
                mlua::Value::Table(list) if list.raw_len() > 0 || list.is_empty() => list
                    .sequence_values::<mlua::Value>()
                    .map(|n| match n? {
                        mlua::Value::String(s) => Ok(s.to_str()?.to_string()),
                        _ => Err(bad_family()),
                    })
                    .collect::<mlua::Result<_>>()?,
                _ => return Err(bad_family()),
            };
            out = out.family(&name, names);
        }
    }
    Ok(out)
}

/// A derived token's `{ from = "peach", ops = { { "lift", 0.3 }, … } }`:
/// the source name and the chain, each op a tuple in the array part — the
/// verb at `[1]`, a colour name at `[2]` for `mix` and `readable`, the
/// number last. `ops` may be one bare tuple, or absent for an alias.
fn parse_recipe(name: &str, t: &Table) -> mlua::Result<(String, Vec<kui_core::ColorOp>)> {
    for pair in t.pairs::<String, mlua::Value>() {
        let (k, _) = pair?;
        if k != "from" && k != "ops" {
            return Err(bad(format!(
                "tokens.colors.{name}: unknown key {k:?} (from, ops)"
            )));
        }
    }
    let from = match t.get::<mlua::Value>("from")? {
        mlua::Value::String(s) => s.to_str()?.to_string(),
        _ => {
            return Err(bad(format!(
                "tokens.colors.{name}.from names a colour token or role"
            )));
        }
    };
    let ops = match t.get::<mlua::Value>("ops")? {
        mlua::Value::Nil => Vec::new(),
        mlua::Value::Table(list) => {
            // A bare tuple starts with its verb; a list starts with a tuple.
            let tuples: Vec<Table> = match list.get::<mlua::Value>(1)? {
                mlua::Value::String(_) => vec![list],
                _ => list
                    .sequence_values::<Table>()
                    .collect::<mlua::Result<_>>()?,
            };
            tuples
                .iter()
                .map(|op| parse_color_op(name, op))
                .collect::<mlua::Result<_>>()?
        }
        _ => {
            return Err(bad(format!(
                "tokens.colors.{name}.ops is a list of {{ verb, … }} tuples"
            )));
        }
    };
    Ok((from, ops))
}

fn parse_color_op(name: &str, op: &Table) -> mlua::Result<kui_core::ColorOp> {
    let verb = match op.get::<mlua::Value>(1)? {
        mlua::Value::String(s) => s.to_str()?.to_string(),
        _ => {
            return Err(bad(format!(
                "tokens.colors.{name}.ops: an op starts with its verb"
            )));
        }
    };
    let Some(takes_color) = kui_core::ColorOp::takes_color(&verb) else {
        return Err(bad(format!(
            "tokens.colors.{name}.ops: unknown verb {verb:?} (lift, darken, raise, alpha, mix, readable)"
        )));
    };
    let arity = if takes_color { 3 } else { 2 };
    if op.raw_len() != arity {
        return Err(bad(format!(
            "tokens.colors.{name}.ops: {verb} takes {} — {{ \"{verb}\", {} }}",
            if takes_color {
                "a colour and a number"
            } else {
                "one number"
            },
            if takes_color { "token, t" } else { "t" }
        )));
    }
    let color = if takes_color {
        match op.get::<mlua::Value>(2)? {
            mlua::Value::String(s) => Some(s.to_str()?.to_string()),
            _ => {
                return Err(bad(format!(
                    "tokens.colors.{name}.ops: {verb}'s colour is a token or role name"
                )));
            }
        }
    } else {
        None
    };
    let n = number(&op.get::<mlua::Value>(arity as i64)?).ok_or_else(|| {
        bad(format!(
            "tokens.colors.{name}.ops: {verb}'s number is a number"
        ))
    })?;
    Ok(kui_core::ColorOp::parse(&verb, color.as_deref(), n).expect("checked above"))
}

/// What a `$name` in a prop resolves through while a table is parsed: the
/// core's token lookup for the running origin, plus the names that did not
/// resolve, which are raised as `unknown-token` warnings afterwards. A prop
/// whose name resolves to nothing is left out and keeps its default, rather
/// than becoming an explicit transparent or zero.
pub type Refs<'a> = kui_core::NameRefs<'a>;

/// Runs `f` with a [`Refs`] over the frame's lookup, then raises what did
/// not resolve. The lookup borrows the core for `f`'s duration and nothing
/// longer, so the caller can open the node it parsed right after.
fn with_refs<R>(
    ui: &mut Ui<'_>,
    f: impl FnOnce(&mut Refs<'_>) -> mlua::Result<R>,
) -> mlua::Result<R> {
    let (r, errors, families) = {
        let mut refs = Refs::new(ui.core().token_lookup());
        let r = f(&mut refs);
        (r, refs.take_missed(), refs.take_missed_families())
    };
    for e in errors {
        ui.core().warn_unknown_token(&e);
    }
    for name in families {
        ui.core().warn_unknown_family(&name);
    }
    r
}

/// A `$name` if `v` is one.
fn reference(v: &mlua::Value) -> mlua::Result<Option<String>> {
    if let mlua::Value::String(s) = v
        && let Some(name) = kui_core::tokens::reference(&s.to_str()?)
    {
        return Ok(Some(name.to_string()));
    }
    Ok(None)
}

/// A stroke's `dash` and `dash_offset` rows (backlog V2): one length
/// (marks and gaps alike), a `{mark, gap}` pair, or four lengths for a
/// dash-dot. No `dash` is a solid stroke.
fn dash_of(t: &Table) -> mlua::Result<kui_core::Dash> {
    let offset = t.get::<Option<f32>>("dash_offset")?.unwrap_or(0.0);
    let lengths: Vec<f32> = match t.get::<mlua::Value>("dash")? {
        mlua::Value::Nil => return Ok(kui_core::Dash::SOLID),
        mlua::Value::Table(list) => list.sequence_values::<f32>().collect::<mlua::Result<_>>()?,
        mlua::Value::Integer(n) => vec![n as f32],
        mlua::Value::Number(n) => vec![n as f32],
        _ => Vec::new(),
    };
    kui_core::Dash::of(&lengths)
        .map(|d| d.offset(offset))
        .ok_or_else(|| bad("dash is a length, {mark, gap} or {mark, gap, mark, gap}"))
}

/// A number, or a `$name` length token.
fn length_of(v: &mlua::Value, refs: &mut Refs<'_>) -> mlua::Result<Option<f32>> {
    if let Some(name) = reference(v)? {
        return Ok(refs.length(&name));
    }
    number(v)
        .map(Some)
        .ok_or_else(|| bad("expected a number or a \"$token\""))
}

/// Reads a node table's props into a [`PropsOut`].
///
/// Constructor-order specials come first (`dir` from the node type, `size`
/// before any style prop), then the Lua-shaped composites (`pad`, `border`,
/// `float`, sizing), then every schema row by its snake_case name. Keys the
/// schema does not own (`type`, `value`, `label`, the children) fall
/// through. `refs` is what a `$name` resolves through.
pub fn parse_props(t: &Table, is_row: bool, refs: &mut Refs<'_>) -> mlua::Result<PropsOut> {
    parse_node(t, is_row, refs, Check::Off).map(|(out, _)| out)
}

/// What one walk over a node table found besides its props: the keys no
/// table claims, collected when asked (the unknown-prop check, folded
/// into the walk rather than a second one: it was a quarter of a Lua
/// view's lowering, backlog F143), and a text's `value` and `spans`, so
/// the text arm reads them without looking them up again.
#[derive(Default)]
struct Walked {
    unknown: Vec<NodeKey>,
    value: Option<mlua::Value>,
    spans: Option<mlua::Value>,
}

/// A table's key as the walks read it (backlog F143): a string key's
/// bytes held inline, an integer key, or anything else. Read off the Lua
/// stack by [`Table::for_each`] through `from_stack`, a string key costs
/// no registry reference — each `pairs` step made one for every key, a
/// quarter of a Lua view's lowering spent creating and dropping them.
/// `from_lua` reads the same from a [`mlua::Value`], so a mlua that stops
/// calling `from_stack` loses the speed and nothing else.
#[derive(Clone)]
enum NodeKey {
    /// A string key of up to [`NodeKey::INLINE`] bytes: every prop name.
    Str {
        len: u8,
        buf: [u8; NodeKey::INLINE],
    },
    /// A longer string key, copied out: no prop is that long, so it is
    /// only ever unclaimed, and the check names it.
    Long(String),
    Int(i64),
    Other,
}

impl NodeKey {
    const INLINE: usize = 30;

    fn of_bytes(b: &[u8]) -> Self {
        if b.len() <= Self::INLINE {
            let mut buf = [0; Self::INLINE];
            buf[..b.len()].copy_from_slice(b);
            NodeKey::Str {
                len: b.len() as u8,
                buf,
            }
        } else {
            NodeKey::Long(String::from_utf8_lossy(b).into_owned())
        }
    }

    /// The key's bytes, for a string key that fits; empty otherwise.
    fn bytes(&self) -> &[u8] {
        match self {
            NodeKey::Str { len, buf } => &buf[..*len as usize],
            _ => &[],
        }
    }

    /// The key as a name for a warning: its text, or what it is.
    fn name(&self) -> String {
        match self {
            NodeKey::Str { .. } => String::from_utf8_lossy(self.bytes()).into_owned(),
            NodeKey::Long(name) => name.clone(),
            NodeKey::Int(i) => i.to_string(),
            NodeKey::Other => "<a key>".into(),
        }
    }
}

impl mlua::FromLua for NodeKey {
    fn from_lua(value: mlua::Value, _: &Lua) -> mlua::Result<Self> {
        Ok(match value {
            mlua::Value::String(s) => NodeKey::of_bytes(&s.as_bytes()),
            mlua::Value::Integer(i) => NodeKey::Int(i),
            _ => NodeKey::Other,
        })
    }

    unsafe fn from_stack(idx: std::ffi::c_int, lua: &mlua::state::RawLua) -> mlua::Result<Self> {
        use mlua::ffi;
        let state = lua.state();
        // SAFETY: `idx` is the key `lua_next` left on the stack. A string
        // is read where it is (`lua_tolstring` converts nothing on one,
        // so the traversal is undisturbed) and copied out before
        // anything can pop it; a number is read without conversion.
        unsafe {
            Ok(match ffi::lua_type(state, idx) {
                ffi::LUA_TSTRING => {
                    let mut len = 0;
                    let ptr = ffi::lua_tolstring(state, idx, &mut len);
                    NodeKey::of_bytes(std::slice::from_raw_parts(ptr as *const u8, len))
                }
                ffi::LUA_TNUMBER if ffi::lua_isinteger(state, idx) != 0 => {
                    NodeKey::Int(ffi::lua_tointeger(state, idx))
                }
                _ => NodeKey::Other,
            })
        }
    }
}

/// What the walk collects for the unknown-prop check: nothing (the
/// core's diagnostics are off), the keys no table claims (an element that
/// reads every row and composite, whose claimed keys are all known), or
/// every key but `type` (an element that reads only some rows — a text
/// — where a row it does not read warns too).
#[derive(Clone, Copy, PartialEq, Eq)]
enum Check {
    Off,
    Unclaimed,
    All,
}

/// A key of a node table, as the walk sorts it: a composite or special
/// `parse_props` reads itself, or a schema row.
enum Item {
    Pad,
    Border,
    Overflow(u32),
    Float,
    KeyFocus,
    Key,
    Index,
    RowCount,
    Tooltip,
    WindowEvents,
    Row(&'static schema::PropDef),
}

/// The walk under [`parse_props`]: one pass of `pairs` over the table,
/// every key sorted by its bytes (no `String`, no `from_utf8` for the
/// keys the table claims), the props applied after it so `size` and
/// `radius` still land first — `size` resets the text style and
/// `radius` sets the corners a `radius_tl` row then overrides, and a
/// table's iteration order is not one to rely on. `check` collects the
/// keys [`Check`] asks for, for [`node_props`] to judge against the
/// element.
fn parse_node(
    t: &Table,
    is_row: bool,
    refs: &mut Refs<'_>,
    check: Check,
) -> mlua::Result<(PropsOut, Walked)> {
    let mut out = PropsOut::new();
    if is_row {
        out.spec = kui_core::NodeSpec::row();
    }
    let mut walked = Walked::default();
    let mut size = None;
    let mut radius = None;
    let mut items: Vec<(Item, mlua::Value)> = Vec::with_capacity(8);
    t.for_each::<NodeKey, mlua::Value>(|k, v| {
        if !matches!(k, NodeKey::Str { .. } | NodeKey::Long(_)) {
            return Ok(());
        }
        if check == Check::All && k.bytes() != b"type" {
            walked.unknown.push(k.clone());
        }
        let item = match k.bytes() {
            b"size" => {
                size = Some(v);
                return Ok(());
            }
            b"radius" => {
                radius = Some(v);
                return Ok(());
            }
            b"pad" => Item::Pad,
            b"border" => Item::Border,
            b"clip" => Item::Overflow(kui_core::OVERFLOW_CLIP),
            b"scroll_x" => Item::Overflow(kui_core::OVERFLOW_SCROLL_X),
            b"scroll" | b"scroll_y" => Item::Overflow(kui_core::OVERFLOW_SCROLL_Y),
            b"float" => Item::Float,
            b"key_focus" => Item::KeyFocus,
            b"key" => Item::Key,
            b"index" => Item::Index,
            b"row_count" => Item::RowCount,
            b"tooltip" => Item::Tooltip,
            b"window_events" => Item::WindowEvents,
            name => {
                // `repeat` is a Lua keyword, so that row also answers to
                // CSS's own name for it (`schema::LUA_ALIASES`, which the
                // unknown-prop check reads too).
                let name = schema::LUA_ALIASES
                    .iter()
                    .find(|(alias, _)| alias.as_bytes() == name)
                    .map_or(name, |(_, real)| real.as_bytes());
                match schema::by_snake_bytes(name) {
                    Some(def) => Item::Row(def),
                    None => {
                        match name {
                            b"value" => walked.value = Some(v),
                            b"spans" => walked.spans = Some(v),
                            _ => {}
                        }
                        // `type` is the prelude's element tag, not a prop.
                        if check == Check::Unclaimed && name != b"type" {
                            walked.unknown.push(k);
                        }
                        return Ok(());
                    }
                }
            }
        };
        items.push((item, v));
        Ok(())
    })?;
    if let Some(v) = size
        && let Some(size) = length_of(&v, refs)?
    {
        out.style = kui_core::TextStyle::new(size);
    }
    if let Some(v) = radius
        && let Some(r) = length_of(&v, refs)?
    {
        out.with_spec(|s| s.radius(r));
    }
    // Overflow bits accumulate across the walk (`clip` and `scroll` are
    // separate keys) and are applied once, so nothing here has to know that
    // scrolling clips too.
    let mut overflow = 0;
    for (item, v) in items {
        match item {
            Item::Pad => {
                let pad = parse_pad(&v, refs)?;
                out.apply_pad(pad);
            }
            Item::Border => {
                let b = match v {
                    mlua::Value::Table(b) => b,
                    _ => return Err(bad("border must be a table {w=, color=}")),
                };
                let w = length_of(&b.get::<mlua::Value>("w")?, refs)?.unwrap_or(0.0);
                let c = parse_color(&b.get::<mlua::Value>("color")?, refs)?
                    .unwrap_or(Color::TRANSPARENT);
                out.with_spec(|s| s.border(w, c));
            }
            Item::Overflow(flag) => overflow |= bit(&v, flag),
            Item::Float => {
                let cfg = parse_float(&v)?;
                out.with_spec(|s| s.float(cfg));
            }
            Item::KeyFocus => out.key_focus = truthy(&v),
            Item::Key => {
                let mlua::Value::String(s) = &v else {
                    return Err(bad("key must be a string"));
                };
                out.key = Some(s.to_str()?.to_string());
            }
            Item::Index => {
                let Some(i) = measure(&v) else {
                    return Err(bad("index must be a number (the row's data index)"));
                };
                out.index = Some(i);
            }
            Item::RowCount => {
                let Some(n) = measure(&v) else {
                    return Err(bad(
                        "row_count must be a number (how many indexed rows the list has)",
                    ));
                };
                out.row_count = Some(n);
            }
            Item::Tooltip => {
                let mlua::Value::String(s) = &v else {
                    return Err(bad("tooltip must be a string"));
                };
                out.apply_tooltip(s.to_str()?.as_ref());
            }
            // The root's (ADR 0048), read on every table: the core
            // applies a root's and warns about any other's.
            Item::WindowEvents => {
                let events =
                    window_events_of(&v).map_err(|e| bad(format!("window_events: {e}")))?;
                out.window_events = Some(events);
            }
            Item::Row(def) => {
                if let Some(parsed) = parse_value(&def.kind, &v, refs)
                    .map_err(|e| bad(format!("{}: {e}", schema::snake_case(def.name))))?
                {
                    schema::apply(def, parsed, &mut out).map_err(bad)?;
                }
            }
        }
    }
    out.with_spec(|s| s.overflow_bits(overflow));
    Ok((out, walked))
}

/// [`node_props`] for a widget arm: the props alone, and `checked` set
/// so [`build_widget`] does not walk the table again for the check.
fn props_of(ui: &mut Ui<'_>, t: &Table, ty: &str, checked: &mut bool) -> mlua::Result<PropsOut> {
    *checked = true;
    node_props(ui, t, false, element_of(ty)).map(|(out, _)| out)
}

/// A node's props read under the frame's token lookup, the unknown-prop
/// check made in the same walk when the core's diagnostics are on: what
/// every element whose props are the node table's own goes through, in
/// place of [`check_props`] and a second walk.
fn node_props(
    ui: &mut Ui<'_>,
    t: &Table,
    is_row: bool,
    element: &str,
) -> mlua::Result<(PropsOut, Walked)> {
    let check = if !ui.core().diagnostics() {
        Check::Off
    } else if schema::element_rows(element, schema::Spelling::Snake).is_some() {
        Check::All
    } else {
        Check::Unclaimed
    };
    let (mut out, walked) = with_refs(ui, |refs| parse_node(t, is_row, refs, check))?;
    // The view's own table declared its group already (`view`); any
    // other table's goes on to `open_from`, which says it is off the root.
    if out.window_events.is_some() && VIEW_ROOT.get() == t.to_pointer() {
        out.window_events = None;
    }
    for k in &walked.unknown {
        let name = k.name();
        if !schema::known_prop(element, &name, schema::Spelling::Snake) {
            let w = kui_core::diag::unknown_prop(element, &name, schema::Spelling::Snake);
            ui.core().warn(w);
        }
    }
    Ok((out, walked))
}

fn truthy(v: &mlua::Value) -> bool {
    matches!(v, mlua::Value::Boolean(true))
}

/// `bit` when the flag is on, for ORing an overflow mask together.
fn bit(v: &mlua::Value, bit: u32) -> u32 {
    if truthy(v) { bit } else { 0 }
}

fn number(v: &mlua::Value) -> Option<f32> {
    match v {
        mlua::Value::Number(n) => Some(*n as f32),
        mlua::Value::Integer(n) => Some(*n as f32),
        _ => None,
    }
}

/// A count, a position, an index, a byte offset or a line number from
/// Lua: any number, floored, below 0 read as 0 and past `u64::MAX` as it
/// (backlog F180). Lua 5.5's `/` always yields a float, so a grid sized
/// `room_h / line_h` is 7.5 rows on an odd window, and 7 is the rows that
/// fit; mlua 0.12.2 raises on a fraction where Rust takes an integer, so
/// every such read goes through here and agrees with `index`, which has
/// always read a number. `None` for a value that is no number, for the
/// caller to name.
fn measure(v: &mlua::Value) -> Option<u64> {
    match *v {
        mlua::Value::Integer(i) => Some(i.max(0) as u64),
        // `as` floors a non-negative float and saturates; NaN is 0.
        mlua::Value::Number(n) => Some(n.max(0.0) as u64),
        _ => None,
    }
}

/// The latest line a grid's row 0 can be. The core adds a row to its
/// `origin_line` unchecked — to draw a selection, to read one back, to
/// select all — and keeps at most [`kui_core::cells::MAX_CELLS`] rows, so
/// a line no later than this leaves every row's line inside `i64`, which
/// is also the widest a line read back into Lua (`cell_selection`) stays
/// an integer. A measure saturates `math.huge` to `u64::MAX`, and the
/// row added to that overflowed.
const MAX_ORIGIN_LINE: u64 = i64::MAX as u64 - kui_core::cells::MAX_CELLS as u64;

/// A [`measure`] as a `usize`, saturating where that is narrower.
fn to_usize(n: u64) -> usize {
    usize::try_from(n).unwrap_or(usize::MAX)
}

/// A [`measure`] argument or field as a `usize`, refused naming `what`
/// when it is no number.
pub(crate) fn measure_arg(v: &mlua::Value, what: &str) -> mlua::Result<usize> {
    measure(v)
        .map(to_usize)
        .ok_or_else(|| bad(format!("{what} must be a number, not {}", v.type_name())))
}

/// `t[k]` as a [`measure`], `None` when absent, refused naming `what`
/// when it is no number.
fn measure_at(t: &Table, k: impl mlua::IntoLua, what: &str) -> mlua::Result<Option<u64>> {
    match t.get::<mlua::Value>(k)? {
        mlua::Value::Nil => Ok(None),
        v => measure(&v)
            .map(Some)
            .ok_or_else(|| bad(format!("{what} must be a number, not {}", v.type_name()))),
    }
}

/// A handle from Lua — an image's, a fragment's, a sound's, a window's:
/// a whole number, as the host handed it out (backlog F180). A float
/// that is one (`3.0`, out of a JSON decode) is that number; a fraction
/// is refused naming `what`, where a count is floored, since a handle
/// is never computed and flooring one names a different image. `None`
/// for a value that is no number, for the caller to name. Node keys are
/// not read here: a key is a 64-bit hash no float holds, so a float key
/// is already a wrong one and stays refused.
///
/// kui gives out no negative handle, so a negative one, an integer's or
/// a float's, is refused the same way: cast to the `u64` the core takes,
/// `-1` named the last sound there is room for, not none. A handle
/// narrower than that (a window's) is range-checked by its own reader,
/// [`window_arg`].
fn handle(v: &mlua::Value, what: &str) -> mlua::Result<Option<u64>> {
    match *v {
        mlua::Value::Integer(i) if i >= 0 => Ok(Some(i as u64)),
        // The range keeps a huge float from saturating into a handle
        // that happens to exist; `fract` is NaN for an infinity.
        mlua::Value::Number(n) if n.fract() == 0.0 && (0.0..2f64.powi(63)).contains(&n) => {
            Ok(Some(n as u64))
        }
        mlua::Value::Integer(i) => Err(bad(format!(
            "{what} is a handle, a whole number as kui gave it out, not {i}"
        ))),
        mlua::Value::Number(n) => Err(bad(format!(
            "{what} is a handle, a whole number as kui gave it out, not {n}"
        ))),
        _ => Ok(None),
    }
}

/// A window's [`handle_arg`]: a `u32`, as `WindowId` is, so a handle past
/// one is refused naming `what` where `as u32` wrapped it — `4294967297`
/// focused window 1.
fn window_arg(v: &mlua::Value, what: &str) -> mlua::Result<u32> {
    let h = handle_arg(v, what)?;
    u32::try_from(h).map_err(|_| {
        bad(format!(
            "{what} is a window handle, a whole number as kui gave it out, not {h}"
        ))
    })
}

/// A [`handle`] argument or field, refused naming `what` when it is no
/// number.
fn handle_arg(v: &mlua::Value, what: &str) -> mlua::Result<u64> {
    handle(v, what)?.ok_or_else(|| {
        bad(format!(
            "{what} must be a handle (integer), not {}",
            v.type_name()
        ))
    })
}

/// `t[k]` as a [`handle_arg`], `None` when absent.
fn handle_at(t: &Table, k: &str, what: &str) -> mlua::Result<Option<u64>> {
    match t.get::<mlua::Value>(k)? {
        mlua::Value::Nil => Ok(None),
        v => handle_arg(&v, what).map(Some),
    }
}

/// A `0xRRGGBBAA` word from a Lua number, as every colour number is read:
/// [`parse_color_value`]'s number arm, and a `cells` run's fg, bg and ul.
fn color_word(v: &mlua::Value) -> Option<u32> {
    match *v {
        mlua::Value::Integer(n) => Some(n as u32),
        mlua::Value::Number(n) => Some(n as u32),
        _ => None,
    }
}

/// One schema value from Lua, by kind. `None` = absent (a false flag).
fn parse_value(kind: &Kind, v: &mlua::Value, refs: &mut Refs<'_>) -> mlua::Result<Option<Parsed>> {
    Ok(Some(match kind {
        Kind::F32 => match length_of(v, refs)? {
            Some(px) => Parsed::F32(px),
            None => return Ok(None),
        },
        Kind::Color => match parse_color(v, refs)? {
            Some(c) => Parsed::Color(c),
            None => return Ok(None),
        },
        Kind::Flag => {
            if truthy(v) {
                Parsed::Flag
            } else {
                return Ok(None);
            }
        }
        Kind::Enum(names) => {
            let mlua::Value::String(s) = v else {
                return Err(bad(format!("expected one of {names:?}")));
            };
            Parsed::Enum(schema::enum_index(names, &s.to_str()?).map_err(bad)?)
        }
        Kind::Sizing => match parse_sizing(v, refs)? {
            Some(s) => Parsed::Sizing(s),
            None => return Ok(None),
        },
        // A `$name` is a fixed clamp of that many px, as a sizing's is;
        // one that misses leaves the row at its default (AR14). A string
        // is `"fit"` (a min's) or a size expression, a table the same
        // expression as data (backlog F109).
        Kind::Min | Kind::Max => Parsed::Bound(match v {
            v if reference(v)?.is_some() => match length_of(v, refs)? {
                Some(px) => kui_core::Bound::Px(px),
                None => return Ok(None),
            },
            // An expression the full table refused leaves the row at its
            // default too, and the core warns (backlog RG93).
            mlua::Value::String(s) => {
                let s = s.to_str()?;
                let b = if matches!(kind, Kind::Min) {
                    schema::min_str(&s)
                } else {
                    schema::max_str(&s)
                };
                match kept(b)? {
                    Some(b) => b,
                    None => return Ok(None),
                }
            }
            mlua::Value::Table(_) => match kept(kui_core::calc::bound_value(&size_value(v)?))? {
                Some(b) => b,
                None => return Ok(None),
            },
            v => kui_core::Bound::Px(
                number(v).ok_or_else(|| bad("expected a number, a string or a size table"))?,
            ),
        }),
        Kind::Msg | Kind::Handler => Parsed::Msg(lua_to_value(v)?),
        Kind::Str => {
            let mlua::Value::String(s) = v else {
                return Err(bad("expected a string"));
            };
            Parsed::Str(s.to_str()?.to_string())
        }
        // A stock family or an installed one by name, registered as it is
        // parsed (ADR 0037).
        Kind::Family => {
            let mlua::Value::String(s) = v else {
                return Err(bad("expected a family name"));
            };
            Parsed::Family(refs.family(&s.to_str()?))
        }
        Kind::Resource => match handle(v, "a resource")? {
            Some(n) => Parsed::Resource(n),
            None => return Err(bad("expected a resource handle (integer)")),
        },
        // A `$name` in a stop resolves through the same refs as a prop's
        // and misses the same way (AR14).
        Kind::Keyframes => Parsed::Keyframes(
            kui_core::keyframes::parse_with(&lua_to_value(v)?, Some(refs)).map_err(bad)?,
        ),
        Kind::Enter => {
            Parsed::Enter(kui_core::enter::parse_with(&lua_to_value(v)?, Some(refs)).map_err(bad)?)
        }
        Kind::Gradient => Parsed::Gradient(
            kui_core::gradient::parse_with(&lua_to_value(v)?, Some(refs)).map_err(bad)?,
        ),
        // The entries' conditions and props read in the core, the way a
        // keyframe's stops do; a `$name` resolves through the same refs.
        Kind::Rules => {
            Parsed::Rules(kui_core::rules::parse_with(&lua_to_value(v)?, Some(refs)).map_err(bad)?)
        }
    }))
}

/// `0xRRGGBBAA` integers or `"#hex"` strings — a value, never a reference:
/// what a token declaration holds.
fn parse_color_value(v: &mlua::Value) -> mlua::Result<Color> {
    if let Some(n) = color_word(v) {
        return Ok(schema::color_num(n));
    }
    match v {
        mlua::Value::String(s) => schema::color_hex_str(&s.to_str()?).map_err(bad),
        _ => Err(bad("color must be a 0xRRGGBBAA integer or \"#hex\" string")),
    }
}

/// A colour prop: a value, or a `"$name"` token reference.
fn parse_color(v: &mlua::Value, refs: &mut Refs<'_>) -> mlua::Result<Option<Color>> {
    if let Some(name) = reference(v)? {
        return Ok(refs.color(&name));
    }
    parse_color_value(v)
        .map(Some)
        .map_err(|_| bad("color must be a 0xRRGGBBAA integer, a \"#hex\" string or a \"$token\""))
}

/// A size expression as data, as the core reads one — with
/// a percentage spelled `{ pct = n }` at any depth, Lua's word: `percent`
/// was the first cut's, refused since, and a size table taking it
/// would bring it back one level down.
fn size_value(v: &mlua::Value) -> mlua::Result<Value> {
    fn check(v: &Value) -> mlua::Result<()> {
        match v {
            Value::Map(m) => {
                for (k, x) in m {
                    if k == "percent" {
                        return Err(bad("a percentage is { pct = n } in Lua"));
                    }
                    check(x)?;
                }
                Ok(())
            }
            Value::List(xs) => xs.iter().try_for_each(check),
            _ => Ok(()),
        }
    }
    let value = lua_to_value(v)?;
    check(&value)?;
    Ok(value)
}

/// `Some` of a size expression, `None` for one the full table refused
/// ([`kui_core::calc::is_full`]) — the prop left undeclared, as a
/// `$name` that misses is — and the error for a bad one.
fn kept<T>(r: Result<T, String>) -> mlua::Result<Option<T>> {
    match r {
        Ok(v) => Ok(Some(v)),
        Err(e) if kui_core::calc::is_full(&e) => Ok(None),
        Err(e) => Err(bad(e)),
    }
}

fn parse_sizing(v: &mlua::Value, refs: &mut Refs<'_>) -> mlua::Result<Option<Sizing>> {
    if let Some(name) = reference(v)? {
        return Ok(refs.length(&name).map(Sizing::Fixed));
    }
    Ok(Some(match v {
        mlua::Value::Number(n) => Sizing::Fixed(*n as f32),
        mlua::Value::Integer(n) => Sizing::Fixed(*n as f32),
        mlua::Value::String(s) => match kept(schema::sizing_str(&s.to_str()?))? {
            Some(s) => s,
            None => return Ok(None),
        },
        mlua::Value::Table(t) => {
            if let Some(p) = t.get::<Option<f32>>("pct")? {
                Sizing::Percent(p / 100.0)
            } else if let Some(f) = t.get::<Option<f32>>("grow")? {
                Sizing::Grow(f)
            } else {
                // A size expression as data (backlog F109):
                // `{ clamp = { 400, { pct = 80 }, 1000 } }`.
                match kept(kui_core::calc::sizing_value(&size_value(v)?)).map_err(|e| {
                    bad(format!(
                        "sizing table needs pct, grow or a size expression: {e}"
                    ))
                })? {
                    Some(s) => s,
                    None => return Ok(None),
                }
            }
        }
        _ => return Err(bad("invalid sizing value")),
    }))
}

/// The `pad` prop as declared: a number is the all-round shorthand, a table
/// names any of the family (`x`, `y`, `l`, `r`, `t`, `b`). What a missing
/// edge falls back to is [`PadShorthand::resolve`]'s call.
fn parse_pad(v: &mlua::Value, refs: &mut Refs<'_>) -> mlua::Result<PadShorthand> {
    match v {
        mlua::Value::Table(t) => {
            // One walk over the edges the table names (backlog F143),
            // rather than a lookup for each of the seven it may.
            let mut edges: [Option<mlua::Value>; 7] = Default::default();
            t.for_each::<NodeKey, mlua::Value>(|k, v| {
                let at = match k.bytes() {
                    b"all" => 0,
                    b"x" => 1,
                    b"y" => 2,
                    b"l" => 3,
                    b"r" => 4,
                    b"t" => 5,
                    b"b" => 6,
                    _ => return Ok(()),
                };
                edges[at] = Some(v);
                Ok(())
            })?;
            let mut edge = |i: usize| -> mlua::Result<Option<f32>> {
                match edges[i].take() {
                    None => Ok(None),
                    Some(v) => length_of(&v, refs),
                }
            };
            Ok(PadShorthand {
                all: edge(0)?,
                x: edge(1)?,
                y: edge(2)?,
                l: edge(3)?,
                r: edge(4)?,
                t: edge(5)?,
                b: edge(6)?,
            })
        }
        v => Ok(PadShorthand {
            all: length_of(v, refs).map_err(|_| bad("invalid padding value"))?,
            ..PadShorthand::default()
        }),
    }
}

fn parse_align(s: &str) -> mlua::Result<Align> {
    schema::enum_index(schema::ALIGNS, s)
        .map(schema::align_idx)
        .map_err(bad)
}

/// A preset name, wherever Lua spells one: `float = "below"` and a float
/// table's `anchor`. The names and what each attaches to are core's.
fn float_preset(name: &str) -> mlua::Result<FloatConfig> {
    FloatConfig::preset(name).ok_or_else(|| {
        bad(format!(
            "bad float preset '{name}' (one of {})",
            kui_core::FLOAT_PRESETS.join(" | ")
        ))
    })
}

/// An `{ x, y }` attach point under `key`, or `None` when it is absent.
fn parse_attach(f: &Table, key: &str) -> mlua::Result<Option<(Align, Align)>> {
    let Some(at) = f.get::<Option<Table>>(key)? else {
        return Ok(None);
    };
    Ok(Some((
        parse_align(&at.get::<String>(1)?)?,
        parse_align(&at.get::<String>(2)?)?,
    )))
}

fn parse_float(v: &mlua::Value) -> mlua::Result<FloatConfig> {
    let f = match v {
        mlua::Value::String(s) => return float_preset(&s.to_str()?),
        mlua::Value::Table(f) => f,
        _ => return Err(bad("float must be a preset string or a table")),
    };
    // `self` is the name the other bindings use; `self_at` stays accepted
    // because Lua shipped with it.
    let self_at = match parse_attach(f, "self")? {
        Some(at) => Some(at),
        None => parse_attach(f, "self_at")?,
    };
    Ok(FloatConfig::build(
        match f.get::<Option<String>>("anchor")? {
            Some(name) => float_preset(&name)?,
            None => FloatConfig::parent(),
        },
        parse_attach(f, "at")?,
        self_at,
        f.get::<Option<f32>>("dx")?,
        f.get::<Option<f32>>("dy")?,
        f.get::<Option<bool>>("fit")?.unwrap_or(false),
        f.get::<Option<bool>>("clip")?.unwrap_or(false),
    ))
}

// ---------------------------------------------------------------------------
// Value <-> Lua

/// How deep a Lua value may nest before [`lua_to_value`] refuses it. A
/// table holding itself, or one nested deeper than this, is an error rather
/// than a stack overflow.
pub const MAX_VALUE_DEPTH: usize = 64;

/// Converts a Lua value to a [`kui_core::Value`], the shape event payloads
/// and replies travel in.
///
/// A table with sequence entries becomes a [`Value::List`], any other table
/// a [`Value::Map`] with string keys; nil, booleans, integers, floats and
/// strings map one to one. Functions and userdata are refused.
///
/// ```
/// use kui_core::Value;
/// use kui_lua::lua_to_value;
///
/// let lua = mlua::Lua::new();
/// let v: mlua::Value = lua.load(r#"{ kind = "toggle", index = 2 }"#).eval()?;
/// let payload = lua_to_value(&v)?;
/// assert_eq!(payload.get_str("kind"), Some("toggle"));
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub fn lua_to_value(v: &mlua::Value) -> mlua::Result<Value> {
    to_value(
        v,
        &mut ValuePath {
            depth: 0,
            seen: Vec::new(),
        },
    )
}

/// Tables nested this deep before [`ValuePath`] records which they are.
/// No payload a view means is this deep, and a table that holds itself
/// passes it and is caught a lap of its cycle later: asking each table
/// its identity costs a size table per row per frame 57 ns, +2.7% on
/// kui-lua's `lua_1000_rows/table per frame`.
const VALUE_TRACKED_PAST: usize = 8;

/// How deep [`to_value`] is, and past [`VALUE_TRACKED_PAST`] the tables
/// it is inside.
struct ValuePath {
    depth: usize,
    seen: Vec<*const std::ffi::c_void>,
}

/// [`lua_to_value`] with where it is: a table met again on its own path
/// holds itself. A table met twice off the path (the same list under two
/// keys) is copied twice, as before.
fn to_value(v: &mlua::Value, path: &mut ValuePath) -> mlua::Result<Value> {
    Ok(match v {
        mlua::Value::Nil => Value::Null,
        mlua::Value::Boolean(b) => Value::Bool(*b),
        mlua::Value::Integer(i) => Value::Int(*i),
        mlua::Value::Number(n) => Value::Float(*n),
        mlua::Value::String(s) => Value::Str(s.to_str()?.to_string()),
        mlua::Value::Table(t) => {
            let tracked = path.depth >= VALUE_TRACKED_PAST;
            if tracked {
                let at = t.to_pointer();
                if path.seen.contains(&at) {
                    return Err(bad("a table that holds itself cannot be a value"));
                }
                path.seen.push(at);
            }
            if path.depth >= MAX_VALUE_DEPTH {
                return Err(bad(format!("a value nested past {MAX_VALUE_DEPTH} tables")));
            }
            path.depth += 1;
            let len = t.raw_len();
            let value = if len > 0 {
                let mut list = Vec::with_capacity(len);
                for item in t.sequence_values::<mlua::Value>() {
                    list.push(to_value(&item?, path)?);
                }
                Value::List(list)
            } else {
                let mut map = Vec::new();
                for pair in t.pairs::<String, mlua::Value>() {
                    let (k, v) = pair?;
                    map.push((k, to_value(&v, path)?));
                }
                // An empty table is a map unless it was marked a list
                // ([`mark_array`]): what decides `[]` against `{}`.
                if map.is_empty() && is_marked_array(t) {
                    Value::List(Vec::new())
                } else {
                    Value::Map(map)
                }
            };
            path.depth -= 1;
            if tracked {
                path.seen.pop();
            }
            value
        }
        // A channel crosses as its handle (ADR 0065).
        mlua::Value::UserData(ud) if ud.is::<channel::LuaChannel>() => {
            ud.borrow::<channel::LuaChannel>()?.0.handle()
        }
        other => {
            return Err(mlua::Error::runtime(format!(
                "cannot convert {} to event payload",
                other.type_name()
            )));
        }
    })
}

/// The metatable that says an empty table is a list, one per state.
const ARRAY_MARK: &str = "kui.array";

/// Marks `t` a list, so that empty it converts to `[]` and not `{}`: the
/// one thing a Lua table cannot say for itself. An empty list decoded
/// from data comes back marked, so it round-trips.
pub fn mark_array(lua: &Lua, t: &Table) -> mlua::Result<()> {
    let mt = match lua.named_registry_value::<Option<Table>>(ARRAY_MARK)? {
        Some(mt) => mt,
        None => {
            let mt = lua.create_table()?;
            mt.raw_set("__kui_array", true)?;
            lua.set_named_registry_value(ARRAY_MARK, &mt)?;
            mt
        }
    };
    if t.metatable().is_none() {
        t.set_metatable(Some(mt))?;
    }
    Ok(())
}

fn is_marked_array(t: &Table) -> bool {
    t.metatable()
        .is_some_and(|mt| mt.raw_get::<bool>("__kui_array").unwrap_or(false))
}

/// Installs the data half of the prelude — `channel {}` (ADR 0065) and
/// `row_heights` — into a Lua state that draws nothing: a host's worker
/// thread (the `kui` runner's, ADR 0064) builds channels and hands their
/// handles back without the UI's builders.
pub fn install_data(lua: &Lua) -> mlua::Result<()> {
    rows::register(lua)?;
    channel::register(lua)
}

/// Converts a [`kui_core::Value`] to a Lua value: the inverse of
/// [`lua_to_value`], used to hand event payloads and slot params to a script.
pub fn value_to_lua(lua: &Lua, v: &Value) -> mlua::Result<mlua::Value> {
    Ok(match v {
        Value::Null => mlua::Value::Nil,
        Value::Bool(b) => mlua::Value::Boolean(*b),
        Value::Int(i) => mlua::Value::Integer(*i),
        Value::Float(f) => mlua::Value::Number(*f),
        Value::Str(s) => mlua::Value::String(lua.create_string(s)?),
        Value::List(items) => {
            let t = lua.create_table_with_capacity(items.len(), 0)?;
            for (i, item) in items.iter().enumerate() {
                t.set(i + 1, value_to_lua(lua, item)?)?;
            }
            // An empty list stays one on its way back.
            if items.is_empty() {
                mark_array(lua, &t)?;
            }
            mlua::Value::Table(t)
        }
        Value::Map(entries) => {
            // A channel's handle is the channel (ADR 0065).
            if let Some(ch) = channel::handle_to_lua(lua, v) {
                return ch;
            }
            let t = lua.create_table_with_capacity(0, entries.len())?;
            for (k, v) in entries {
                t.set(k.as_str(), value_to_lua(lua, v)?)?;
            }
            mlua::Value::Table(t)
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use kui_core::{
        Core, Edges, FontFamily, InputEvent, NodeSpec, OriginId, Rect, Size, TextStyle, Vec2,
        WindowButton, WindowId,
    };

    #[test]
    fn value_round_trips_through_lua() {
        let lua = Lua::new();
        let original = Value::map([
            ("kind", "inc".into()),
            ("by", Value::Int(2)),
            (
                "weights",
                Value::List(vec![Value::Float(0.5), Value::Float(1.5)]),
            ),
            ("enabled", Value::Bool(true)),
        ]);
        let lua_v = value_to_lua(&lua, &original).unwrap();
        let back = lua_to_value(&lua_v).unwrap();
        assert_eq!(back.get_str("kind"), Some("inc"));
        assert_eq!(back.get_int("by"), Some(2));
        assert_eq!(back.get_bool("enabled"), Some(true));
        match back.get("weights") {
            Some(Value::List(items)) => assert_eq!(items.len(), 2),
            other => panic!("expected list, got {other:?}"),
        }
    }

    /// A `Refs` over a bare core: no tokens declared, the roles resolve.
    /// Leaked on purpose — the lookup borrows the core, and a test parses
    /// one table and asserts, so a core per call is the simplest shape.
    fn test_refs() -> Refs<'static> {
        let core: &'static Core = Box::leak(Box::new(Core::new()));
        Refs::new(core.token_lookup())
    }

    fn eval_table(lua: &Lua, src: &str) -> Table {
        lua.load(src).eval().unwrap()
    }

    /// The whole schema surface from a Lua table equals the Rust builder.
    #[test]
    fn schema_props_match_the_rust_builder() {
        let lua = Lua::new();
        let t = eval_table(
            &lua,
            r##"{
                width = "grow", height = {pct = 50},
                min_width = 10, max_width = 500, min_height = 5, max_height = 300,
                pad = {l = 1, r = 2, t = 3, b = 4}, gap = 8,
                main_align = "center", cross_align = "end", center = false,
                bg = "#14161e", radius = 6, border = {w = 1, color = 0x2a2d3aff},
                clip = true, scroll = true, scroll_x = true,
                float = {anchor = "viewport", at = {"end", "end"}, self_at = {"end", "end"},
                         dx = -8, dy = -8, fit = true},
                hoverable = true, window = "close",
                on_click = {kind = "hit"}, on_drag = "d", on_key = 7, key_up = true,
                modal = "dlg", on_context_menu = {kind = "menu"},
                initial_focus = true,
                key = "panel", key_focus = true,
            }"##,
        );
        let p = parse_props(&t, true, &mut test_refs()).unwrap();
        let expected = NodeSpec::row()
            .grow_width()
            .height(Sizing::Percent(0.5))
            .min_width(10.0)
            .max_width(500.0)
            .min_height(5.0)
            .max_height(300.0)
            .padding(Edges {
                l: 1.0,
                r: 2.0,
                t: 3.0,
                b: 4.0,
            })
            .gap(8.0)
            .main_align(Align::Center)
            .cross_align(Align::End)
            .bg(Color::hex(0x14161eff))
            .radius(6.0)
            .border(1.0, Color::hex(0x2a2d3aff))
            .clip()
            .scroll_y()
            .scroll_x()
            .float(
                FloatConfig::viewport()
                    .inside(Align::End, Align::End)
                    .offset(-8.0, -8.0)
                    .fit(),
            )
            .hoverable()
            .window_button(WindowButton::Close)
            .on_click(Value::map([("kind", "hit".into())]))
            .on_drag("d")
            .on_key(Value::Int(7))
            .key_up()
            .modal("dlg")
            .initial_focus()
            .on_context_menu(Value::map([("kind", "menu".into())]));
        assert_eq!(p.spec, expected);
        assert_eq!(p.key.as_deref(), Some("panel"));
        assert!(p.key_focus);
    }

    /// A min is a number or `"fit"`, per axis, and nothing else: the
    /// sizing words a min cannot be are refused by name.
    #[test]
    fn a_min_is_a_number_or_fit() {
        let lua = Lua::new();
        let t = eval_table(&lua, r#"{ min_width = "fit", min_height = 3 }"#);
        let p = parse_props(&t, false, &mut test_refs()).unwrap();
        let expected = NodeSpec::column()
            .min_width(kui_core::Min::FIT)
            .min_height(3.0);
        assert_eq!(p.spec, expected);
        let t = eval_table(&lua, r#"{ min_width = "grow" }"#);
        let err = parse_props(&t, false, &mut test_refs())
            .unwrap_err()
            .to_string();
        assert!(err.contains("bad min"), "{err}");
    }

    /// The shapes the core decides on, spelled the Lua way: the pad family
    /// beyond `l/r/t/b`, `self` as the other bindings name it, and a preset
    /// as a base with one override — the untouched `dy` keeps below's gap.
    #[test]
    fn the_lua_composites_resolve_the_way_the_core_says() {
        let lua = Lua::new();
        let t = eval_table(
            &lua,
            r#"{ pad = { all = 4, x = 10, b = 1 },
                 float = { anchor = "below", dx = 6 } }"#,
        );
        let p = parse_props(&t, false, &mut test_refs()).unwrap();
        assert_eq!(
            p.spec.layout.padding,
            Edges {
                l: 10.0,
                r: 10.0,
                t: 4.0,
                b: 1.0,
            }
        );
        assert_eq!(
            p.spec.layout.float,
            Some(FloatConfig::build(
                FloatConfig::below(),
                None,
                None,
                Some(6.0),
                None,
                false,
                false
            ))
        );

        // `self` and the `self_at` Lua shipped with name the same point.
        let by_self = eval_table(&lua, r#"{ float = { self = {"end", "start"} } }"#);
        let by_self_at = eval_table(&lua, r#"{ float = { self_at = {"end", "start"} } }"#);
        assert_eq!(
            parse_props(&by_self, false, &mut test_refs())
                .unwrap()
                .spec
                .layout
                .float,
            parse_props(&by_self_at, false, &mut test_refs())
                .unwrap()
                .spec
                .layout
                .float
        );

        // An unknown preset names the ones that exist instead of silently
        // floating against the parent.
        let bad_preset = eval_table(&lua, r#"{ float = "beneath" }"#);
        let e = parse_props(&bad_preset, false, &mut test_refs())
            .unwrap_err()
            .to_string();
        assert!(e.contains("beneath") && e.contains("below"), "{e}");
    }

    #[test]
    fn text_style_props_match_the_rust_builder() {
        let lua = Lua::new();
        let t = eval_table(
            &lua,
            r#"{ size = 20, line_height = 30, color = 0x73d98cff, family = "mono" }"#,
        );
        let style = parse_props(&t, false, &mut test_refs()).unwrap().style;
        assert_eq!(
            style,
            TextStyle::new(20.0)
                .line_height(30.0)
                .color(Color::hex(0x73d98cff))
                .family(FontFamily::Mono)
        );
        let t = eval_table(&lua, r#"{ wrap = "none", max_lines = 2, ellipsis = true }"#);
        assert_eq!(
            parse_props(&t, false, &mut test_refs()).unwrap().style,
            TextStyle::default().nowrap().max_lines(2).ellipsis()
        );
        // `size` is applied first regardless of table iteration order, so a
        // color set alongside it survives the TextStyle::new reset.
        let t = eval_table(&lua, r##"{ color = "#fff", size = 12 }"##);
        assert_eq!(
            parse_props(&t, false, &mut test_refs()).unwrap().style,
            TextStyle::new(12.0).color(Color::hex(0xffffffff))
        );
    }

    #[test]
    fn bad_values_name_the_prop() {
        let lua = Lua::new();
        let t = eval_table(&lua, r#"{ main_align = "middle" }"#);
        let e = parse_props(&t, false, &mut test_refs())
            .unwrap_err()
            .to_string();
        assert!(e.contains("main_align"), "{e}");
        assert!(e.contains("middle"), "{e}");
    }

    fn frame(core: &mut Core, ext: &mut LuaExtension) -> usize {
        let mut ui = core.frame(Size::new(800.0, 600.0), 1.0);
        ui.set_origin(OriginId(1));
        ext.view(&Instance::root(), &mut ui).unwrap();
        ui.finish();
        core.output().0.quads.len()
    }

    #[test]
    fn script_view_builds_ir_nodes() {
        let mut ext = LuaExtension::from_source(
            "test",
            r#"
                count = 41
                function view()
                  return column { gap = 8, pad = 16, bg = 0x10121aff,
                    text("count: " .. count, { size = 20 }),
                    button { label = "bump", on_click = { kind = "bump" } },
                  }
                end
                function on_event(ev)
                  if ev.kind == "bump" then count = count + 1 end
                end
            "#,
        )
        .unwrap();

        let mut core = Core::new();
        let quads = frame(&mut core, &mut ext);
        // Panel bg + button bg + glyphs for two strings.
        assert!(quads > 10, "expected panel/button/glyph quads, got {quads}");

        // Events round-trip into Lua state.
        ext.on_event(&UiEvent::declared(
            kui_core::EventKind::Click,
            OriginId(1),
            Key::ROOT,
            Some(&Value::map([("kind", "bump".into())])),
            Value::Null,
        ));
        let count: i64 = ext.lua.globals().get("count").unwrap();
        assert_eq!(count, 42);
    }

    /// `on_event`'s second argument is the envelope alone: a message with
    /// its own `event` and `node_key` keeps them in the first, and the
    /// second still says what happened and where (backlog RG157).
    #[test]
    fn the_second_argument_is_the_envelope_whatever_the_message_has() {
        let mut ext = LuaExtension::from_source(
            "test",
            r#"
                function view() return column {} end
                function on_event(ev, at)
                  seen = { ev.event, ev.node_key, at.event, at.node_key, at.window }
                end
            "#,
        )
        .unwrap();
        let own = Value::map([
            ("kind", "log".into()),
            ("event", "mine".into()),
            ("node_key", Value::Int(-1)),
        ]);
        ext.on_event(&UiEvent::declared(
            kui_core::EventKind::Submit,
            OriginId(1),
            Key(77),
            Some(&own),
            Value::Null,
        ));
        let seen: Vec<mlua::Value> = ext.lua.globals().get("seen").unwrap();
        let s = |i: usize| seen[i].to_string().unwrap();
        assert_eq!(s(0), "mine", "the message's own field wins in the first");
        assert_eq!(s(1), "-1");
        assert_eq!(s(2), "submit");
        assert_eq!(s(3), "77");
        assert_eq!(s(4), "0");
    }

    /// `devtools_tab` (ADR 0032): the script's `view` is called only while
    /// its tab is on show, the tree it returns lands over the panel's tab
    /// body as the script's own nodes, the component form declares without
    /// calling anything, and a declaration of neither form is the
    /// `bad-devtools-tab` warning rather than a build error.
    #[test]
    fn a_devtools_tab_calls_its_view_only_while_on_show() {
        let mut ext = LuaExtension::from_source(
            "test",
            r#"
                calls = 0
                function view(env)
                  return column { gap = 8,
                    text("app"),
                    devtools_tab { name = "syntax", label = "Tree-sitter", view = function()
                      calls = calls + 1
                      return column { text("from lua"),
                        button { label = "jump", on_click = { kind = "jump" } } }
                    end },
                    devtools_tab { name = "plug", label = "Plugin", component = "ts/panel" },
                    devtools_tab { name = "bad", label = "Bad" },
                  }
                end
            "#,
        )
        .unwrap();
        let mut core = Core::new();
        core.set_devtools(true);
        core.set_devtools_dock(kui_core::DevtoolsDock::Right);
        core.set_inspect(true);
        frame(&mut core, &mut ext);
        frame(&mut core, &mut ext);
        let calls: i64 = ext.lua.globals().get("calls").unwrap();
        assert_eq!(calls, 0, "not on show: the view was not called");
        let ws = core.take_warnings();
        assert_eq!(
            ws.iter()
                .filter(|w| w.code == kui_core::diag::BAD_DEVTOOLS_TAB)
                .count(),
            1,
            "the tab with neither form warns once: {ws:?}"
        );
        // Ctrl+Shift+N three times: tree, perf, then Tree-sitter (the
        // first declared tab).
        let chord = || {
            kui_core::InputEvent::KeyDown(kui_core::KeyPress::new(
                kui_core::KeyCode::Char('N'),
                kui_core::KeyMods::NONE.with_ctrl().with_shift(),
            ))
        };
        core.handle_input(chord());
        core.handle_input(chord());
        core.handle_input(chord());
        frame(&mut core, &mut ext);
        let calls: i64 = ext.lua.globals().get("calls").unwrap();
        assert_eq!(calls, 1, "on show: called once a frame");
        frame(&mut core, &mut ext);
        let body = core
            .nodes()
            .iter()
            .find(|n| n.label.as_deref() == Some("kui-devtools/tab/syntax"))
            .map(|n| n.rect)
            .expect("the body");
        let jump = core
            .nodes()
            .iter()
            .find(|n| n.label.as_deref() == Some("jump"))
            .map(|n| n.rect)
            .expect("the script's button");
        assert!(
            jump.x >= body.x && jump.x + jump.w <= body.x + body.w,
            "{jump:?} in {body:?}"
        );
        assert!(
            core.nodes()
                .iter()
                .any(|n| n.text.as_deref() == Some("from lua")),
            "the script's text painted"
        );
        let evs = kui_core::testing::click_at(&mut core, jump.x + 2.0, jump.y + 2.0);
        assert_eq!(evs.len(), 1);
        assert_eq!(evs[0].origin, OriginId(1), "the script's own event");
        assert_eq!(evs[0].payload.get_str("kind"), Some("jump"));
    }

    /// The root table's `windows` list is `Ui::window` per entry: a name
    /// alone takes the defaults, a table its own size, and the commands
    /// the declared set produces come out of the core for the host to
    /// drain — this binding opens nothing itself.
    #[test]
    fn the_root_table_declares_windows() {
        use kui_core::{WindowCommand, WindowConfig};
        let mut ext = LuaExtension::from_source(
            "windows",
            r#"
                function view(env)
                  return column {
                    windows = { { name = "palette", width = 400, height = 300,
                                  activates = false }, "tools" },
                    text("main"),
                  }
                end
            "#,
        )
        .unwrap();
        let mut core = Core::new();
        frame(&mut core, &mut ext);
        let cmds = core.take_window_commands();
        assert_eq!(cmds.len(), 2, "{cmds:?}");
        assert_eq!(
            cmds[0],
            WindowCommand::Open {
                id: WindowId(1),
                owner: WindowId::MAIN,
                origin: OriginId(1),
                config: WindowConfig {
                    size: Size::new(400.0, 300.0),
                    activates: false,
                    ..WindowConfig::default()
                },
            }
        );
        assert_eq!(
            cmds[1],
            WindowCommand::Open {
                id: WindowId(2),
                owner: WindowId::MAIN,
                origin: OriginId(1),
                config: WindowConfig::default(),
            }
        );
        // Declared again: nothing new, and no unknown-prop line for the key.
        frame(&mut core, &mut ext);
        assert!(core.take_window_commands().is_empty());
        assert!(core.take_warnings().is_empty());
    }

    /// `kind = "popup"` is ADR 0004 decision 9's menu surface: the anchor
    /// rides through untouched, and it does not activate unless asked —
    /// a popup that takes OS focus blurs the field that opened it.
    #[test]
    fn a_windows_entry_declares_a_popup() {
        use kui_core::{Rect, WindowCommand, WindowConfig, WindowKind};
        let mut ext = LuaExtension::from_source(
            "windows-popup",
            r#"
                function view(env)
                  return column {
                    windows = { { name = "menu", kind = "popup",
                                  width = 160, height = 320,
                                  anchor = { x = 12, y = 40, w = 160, h = 24 } } },
                    text("main"),
                  }
                end
            "#,
        )
        .unwrap();
        let mut core = Core::new();
        frame(&mut core, &mut ext);
        assert_eq!(
            core.take_window_commands(),
            vec![WindowCommand::Open {
                id: WindowId(1),
                owner: WindowId::MAIN,
                origin: OriginId(1),
                config: WindowConfig {
                    kind: WindowKind::Popup,
                    size: Size::new(160.0, 320.0),
                    activates: false,
                    anchor: Rect::new(12.0, 40.0, 160.0, 24.0),
                },
            }]
        );
    }

    /// A kind kui does not have is refused where it is written rather than
    /// dropped: opening a normal window for it would read as the popup
    /// having worked. (C cannot do this — an integer field has no room to
    /// refuse in — so it warns `unknown-window-kind` a frame later.)
    #[test]
    fn a_windows_entry_cannot_name_an_unknown_kind() {
        let mut ext = LuaExtension::from_source(
            "windows-kind",
            r#"
                function view(env)
                  return column {
                    windows = { { name = "palette", kind = "sheet" } },
                    text("main"),
                  }
                end
            "#,
        )
        .unwrap();
        let mut core = Core::new();
        let mut ui = core.frame(Size::new(800.0, 600.0), 1.0);
        ui.set_origin(OriginId(1));
        let err = ext.view(&Instance::root(), &mut ui).unwrap_err();
        assert!(err.contains("sheet"), "{err}");
        assert!(err.contains("popup"), "{err}");
    }

    /// A stock button takes `index` as a box does, and the index wins over
    /// its label; a `cells` cursor with a shape nobody has is refused
    /// rather than folded to a block (backlog AR40).
    #[test]
    fn a_button_takes_an_index_and_a_cursor_shape_is_refused() {
        let mut ext = LuaExtension::from_source(
            "rows",
            r#"
                from = 0
                function view(env)
                  return column {
                    button { label = "Open", index = from, on_click = { row = from } },
                    button { label = "Open", index = from + 1, on_click = { row = from + 1 } },
                    button { label = "Keyed", key = "named", index = 7, on_click = "k" },
                  }
                end
            "#,
        )
        .unwrap();
        let mut core = Core::new();
        core.set_diagnostics(true);
        let frame = |core: &mut Core, ext: &mut LuaExtension| {
            let mut ui = core.frame(Size::new(300.0, 200.0), 1.0);
            ui.set_origin(OriginId(1));
            ext.view(&Instance::root(), &mut ui).unwrap();
            ui.finish();
        };
        frame(&mut core, &mut ext);
        assert!(core.take_warnings().is_empty(), "index is a button row");
        let buttons = |core: &mut Core| -> Vec<Key> {
            core.access_tree()
                .nodes
                .iter()
                .filter(|n| n.role == kui_core::Role::Button)
                .map(|n| n.key)
                .collect()
        };
        let before = buttons(&mut core);
        assert_eq!(before.len(), 3, "two rows with the same text are two nodes");
        assert!(
            core.key_of("named").is_none(),
            "declared beside `key`, the index wins"
        );
        ext.lua.globals().set("from", 1).unwrap();
        frame(&mut core, &mut ext);
        let after = buttons(&mut core);
        assert_eq!(
            after[0], before[1],
            "row 1 keeps its key as it moves up the list"
        );

        let mut ext = LuaExtension::from_source(
            "term",
            r#"
                function view(env)
                  return column { cells { key = "term", rows = 1, cols = 4, size = 14,
                    lines = { "abcd" }, cursor_at = { 1, 1 }, cursor_shape = "blob" } }
                end
            "#,
        )
        .unwrap();
        let mut ui = core.frame(Size::new(300.0, 200.0), 1.0);
        ui.set_origin(OriginId(1));
        let err = ext
            .view(&Instance::root(), &mut ui)
            .unwrap_err()
            .to_string();
        assert!(
            err.contains("block | bar | underline") && err.contains("blob"),
            "{err}"
        );
    }

    /// The root's `option_as_alt` names a side (backlog F113), and a name
    /// kui does not have is refused with the four it does.
    #[test]
    fn option_as_alt_is_a_side_by_name() {
        let mut core = Core::new();
        let mut ext = LuaExtension::from_source(
            "keys",
            r#"
                function view(env)
                  return column { option_as_alt = "left", text("x") }
                end
            "#,
        )
        .unwrap();
        frame(&mut core, &mut ext);
        assert_eq!(core.option_as_alt(), kui_core::OptionAsAlt::Left);

        let mut ext = LuaExtension::from_source(
            "keys",
            r#"
                function view(env)
                  return column { option_as_alt = "meta", text("x") }
                end
            "#,
        )
        .unwrap();
        let mut ui = core.frame(Size::new(300.0, 200.0), 1.0);
        ui.set_origin(OriginId(1));
        let err = ext
            .view(&Instance::root(), &mut ui)
            .unwrap_err()
            .to_string();
        assert!(
            err.contains("option_as_alt") && err.contains("\"both\"") && err.contains("meta"),
            "{err}"
        );
    }

    /// A value that holds itself, or nests past [`MAX_VALUE_DEPTH`],
    /// is an error, not a stack overflow (backlog RG95); one table under
    /// two keys is still two copies.
    #[test]
    fn a_value_that_holds_itself_or_nests_too_deep_is_refused() {
        let lua = Lua::new();
        let read = |src: &str| lua_to_value(&lua.load(src).eval::<mlua::Value>().unwrap());
        let err = |src: &str| read(src).unwrap_err().to_string();
        assert!(err("local t = {}; t[1] = t; return t").contains("holds itself"));
        assert!(err("local t = {}; t.me = { t }; return t").contains("holds itself"));
        let shared = read("local l = { 1, 2 }; return { a = l, b = l }").unwrap();
        assert_eq!(shared.get("a"), shared.get("b"));
        let nested = |n: usize| format!("local t = 1; for _ = 1, {n} do t = {{ t }} end; return t");
        assert!(read(&nested(MAX_VALUE_DEPTH)).is_ok());
        assert!(err(&nested(MAX_VALUE_DEPTH + 1)).contains("nested past 64"));
        // A handler's message crosses the same function.
        assert!(err(&nested(100_000)).contains("nested past 64"));
    }

    /// A view node that is its own ancestor, or a view nested past
    /// [`MAX_VIEW_DEPTH`], fails the view rather than the process
    /// (backlog RG95); the deepest one taken builds.
    #[test]
    fn a_view_that_holds_itself_or_nests_too_deep_is_refused() {
        let run = |body: &str| {
            let mut core = Core::new();
            let mut ext =
                LuaExtension::from_source("deep", &format!("function view(env)\n{body}\nend"))
                    .unwrap();
            let mut ui = core.frame(Size::new(300.0, 200.0), 1.0);
            ui.set_origin(OriginId(1));
            let built = ext.view(&Instance::root(), &mut ui);
            ui.finish();
            built
        };
        let err = run("local t = column {}; t[1] = row { t }; return t").unwrap_err();
        assert!(err.contains("holds itself"), "{err}");
        let nested = |n: usize| {
            format!(
                "local t = text('x')
                 for i = 2, {n} do
                   if i % 2 == 0 then t = radio_group {{ label = 'g', t }}
                   elseif i % 3 == 0 then t = tooltip {{ t }}
                   else t = column {{ t }} end
                 end
                 return t"
            )
        };
        run(&nested(MAX_VIEW_DEPTH)).unwrap();
        let err = run(&nested(MAX_VIEW_DEPTH + 1)).unwrap_err();
        assert!(err.contains("nested past 128"), "{err}");
        // The path empties on the error: the next view builds.
        run(&nested(8)).unwrap();
    }

    /// `"none"` declares nothing (backlog RG84): a host's side survives a
    /// script that writes its own setting through as `"none"`, as it does
    /// a Node view's `optionAsAlt: 'none'`.
    #[test]
    fn option_as_alt_none_leaves_the_hosts_side() {
        let mut core = Core::new();
        let mut ext = LuaExtension::from_source(
            "keys",
            r#"
                function view(env)
                  return column { option_as_alt = "none", text("x") }
                end
            "#,
        )
        .unwrap();
        let mut ui = core.frame(Size::new(300.0, 200.0), 1.0);
        ui.option_as_alt(kui_core::OptionAsAlt::Left);
        ui.set_origin(OriginId(1));
        ext.view(&Instance::root(), &mut ui).unwrap();
        ui.finish();
        assert_eq!(core.option_as_alt(), kui_core::OptionAsAlt::Left);
    }

    /// Every node type the prelude offers lowers without error and draws.
    #[test]
    fn every_node_type_lowers() {
        let mut ext = LuaExtension::from_source(
            "all",
            r##"
                function view(env)
                  return column { gap = 4, window_title = "all nodes", always_on_top = true,
                    secure_input = true, option_as_alt = "right", ime_off = true,
                    titlebar { text("custom title"), window_buttons() },
                    titlebar { title = "plain title" },
                    text({ "same IR as ", { "Rust", bold = true, color = "#73d98c" },
                           { " — flatter", italic = true } }, { size = 13 }),
                    edit { key = "note", initial = "hello", size = 14, width = 200,
                           multiline = true },
                    input { label = "name", initial = "" },
                    dropdown { label = "language", options = { "English", "Deutsch" }, current = 1 },
                    row { tooltip = "hover hint", pad = 4, text("badge") },
                    row { hoverable = true, text("legend"), tooltip("always shown") },
                    row { text("rich tip"), tooltip { text("a"), text("b") } },
                    latency_graph(),
                    latency_hud { at = { "start", "end" } },
                    button { label = "ok", on_click = "ok" },
                  }
                end
            "##,
        )
        .unwrap();
        let mut core = Core::new();
        let quads = frame(&mut core, &mut ext);
        assert!(quads > 60, "got {quads} quads");
        assert_eq!(core.window_title(), Some("all nodes"));
        assert!(core.always_on_top());
        assert!(core.secure_input());
        assert_eq!(core.option_as_alt(), kui_core::OptionAsAlt::Right);
        assert!(core.ime_off());
        // And every key above is one some table claims: this scene is the
        // allow-list's fixture, so a new element prop that nobody adds to
        // `ELEMENTS.lua_own` fails here instead of warning at a user.
        let unknown: Vec<String> = core
            .take_warnings()
            .into_iter()
            .filter(|w| w.code == kui_core::diag::UNKNOWN_PROP)
            .map(|w| w.message)
            .collect();
        assert!(unknown.is_empty(), "{unknown:#?}");
    }

    /// A key no table claims is thrown on the floor by the binding — so it
    /// says so, once, in the spelling Lua actually takes.
    #[test]
    fn unknown_props_warn_once_in_lua_spelling() {
        let mut ext = LuaExtension::from_source(
            "typos",
            r#"
                function view(env)
                  return column { pad = 8,
                    row { hoverBg = 0x333333ff, width = 10, height = 10 },
                    row { hoverBg = 0x333333ff, width = 10, height = 10 },
                    row { colour = 0x333333ff, width = 10, height = 10 },
                  }
                end
            "#,
        )
        .unwrap();
        let mut core = Core::new();
        frame(&mut core, &mut ext);
        let mut warned: Vec<String> = core
            .take_warnings()
            .into_iter()
            .filter(|w| w.code == kui_core::diag::UNKNOWN_PROP)
            .map(|w| w.message)
            .collect();
        warned.sort();
        assert_eq!(warned.len(), 2, "one per name, not per node: {warned:#?}");
        assert!(
            warned[1].contains("`hoverBg` is not a prop of box")
                && warned[1].contains("did you mean `hover_bg`?"),
            "{warned:#?}"
        );
        // Nothing near `colour`, so no guess is offered.
        assert!(warned[0].contains("`colour`") && !warned[0].contains("did you mean"));
        // The second frame is silent: (code, key) dedup, as for every check.
        frame(&mut core, &mut ext);
        assert!(core.take_warnings().is_empty());
    }

    /// AR13: a text reads its style rows and nothing else — `live`,
    /// `label`, `on_click`, `key` on one reach no tree and used to be
    /// dropped silently; they warn now, naming the rows a text does read.
    #[test]
    fn a_text_warns_about_the_rows_it_does_not_read() {
        let mut ext = LuaExtension::from_source(
            "textrows",
            r#"
                function view(env)
                  return column { pad = 8,
                    text("hi", { live = "polite", label = "x", on_click = "go", size = 14, line_height = 20, max_lines = 2 }),
                    text({ "a", { "b", bold = true, bg = 0x00ff00ff } }, { color = 0xff0000ff }),
                  }
                end
            "#,
        )
        .unwrap();
        let mut core = Core::new();
        frame(&mut core, &mut ext);
        let mut warned: Vec<String> = core
            .take_warnings()
            .into_iter()
            .filter(|w| w.code == kui_core::diag::UNKNOWN_PROP)
            .map(|w| w.message)
            .collect();
        warned.sort();
        assert_eq!(warned.len(), 3, "{warned:#?}");
        for (w, name) in warned.iter().zip(["label", "live", "on_click"]) {
            assert!(
                w.contains(&format!("`{name}`")) && w.contains("not one text reads"),
                "{w}"
            );
            assert!(w.contains("`max_lines`"), "names the rows it reads: {w}");
        }
    }

    /// The walk applies `size` before the style rows and `radius` before
    /// the corner rows whatever order the table hands its keys in (backlog
    /// F143: one pass, the props applied after it). Each table is built
    /// both ways round, and a table's iteration order follows its
    /// construction closely enough that one of the two meets the specific
    /// row first.
    #[test]
    fn size_and_radius_land_first_whatever_the_order() {
        let lua = Lua::new();
        for src in [
            "return { radius = 8, radius_tl = 2, size = 20, color = 0xff0000ff, line_height = 30 }",
            "return { line_height = 30, color = 0xff0000ff, size = 20, radius_tl = 2, radius = 8 }",
        ] {
            let t = eval_table(&lua, src);
            let p = parse_props(&t, false, &mut test_refs()).unwrap();
            assert_eq!(p.spec.style.radius, [2.0, 8.0, 8.0, 8.0], "{src}");
            assert_eq!(p.style.size, 20.0, "{src}");
            assert_eq!(p.style.line_height, 30.0, "the row over size's own: {src}");
            assert_eq!(p.style.color, Some(Color::hex(0xff0000ff)), "{src}");
        }
    }

    /// The unknown-prop check made in the props walk (backlog F143) warns
    /// on what the separate walk warned on, element by element: a key no
    /// table claims, once, on every element with props of its own; and on
    /// a text, a schema row it does not read.
    #[test]
    fn the_folded_check_warns_as_the_walk_did() {
        let mut ext = LuaExtension::from_source(
            "folded",
            r#"
                function view(env)
                  return column { bogus_box = 1,
                    row { bogus_row = 1 },
                    grid { bogus_grid = 1 },
                    text("t", { bogus_text = 1, on_click = "go" }),
                    image { id = 0, bogus_image = 1 },
                  }
                end
            "#,
        )
        .unwrap();
        let mut core = Core::new();
        frame(&mut core, &mut ext);
        let mut warned: Vec<String> = core
            .take_warnings()
            .into_iter()
            .filter(|w| w.code == kui_core::diag::UNKNOWN_PROP)
            .map(|w| {
                [
                    "bogus_box",
                    "bogus_row",
                    "bogus_grid",
                    "bogus_text",
                    "on_click",
                    "bogus_image",
                ]
                .into_iter()
                .find(|k| w.message.contains(&format!("`{k}`")))
                .unwrap_or("?")
                .to_string()
            })
            .collect();
        warned.sort();
        assert_eq!(
            warned,
            [
                "bogus_box",
                "bogus_grid",
                "bogus_image",
                "bogus_row",
                "bogus_text",
                "on_click"
            ]
        );
        // And nothing at all with the diagnostics off.
        let mut core = Core::new();
        core.set_diagnostics(false);
        frame(&mut core, &mut ext);
        assert!(core.take_warnings().is_empty());
    }

    /// `direction` is the Lua spelling of the `repeat` row (a Lua keyword),
    /// and the check reads the same alias table the parser remaps through.
    #[test]
    fn the_repeat_alias_does_not_warn() {
        let mut ext = LuaExtension::from_source(
            "alias",
            r#"
                function view(env)
                  return column {
                    row { width = 10, height = 10, keyframes = { { bg = 0x000000ff } },
                          transition = 100, direction = "alternate" },
                  }
                end
            "#,
        )
        .unwrap();
        let mut core = Core::new();
        frame(&mut core, &mut ext);
        assert!(core.take_warnings().is_empty());
    }

    /// The check is behind the same gate as every other diagnostic.
    #[test]
    fn unknown_props_stay_quiet_with_diagnostics_off() {
        let mut ext = LuaExtension::from_source(
            "quiet",
            r#"
                function view(env)
                  return column { hoverBg = 0x333333ff }
                end
            "#,
        )
        .unwrap();
        let mut core = Core::new();
        core.set_diagnostics(false);
        frame(&mut core, &mut ext);
        assert!(core.take_warnings().is_empty());
    }

    /// `tooltip = "hint"` on a container makes it hoverable and floats the
    /// hint only while the cursor is over it.
    #[test]
    fn tooltip_prop_is_hover_gated() {
        let mut ext = LuaExtension::from_source(
            "tip",
            r#"
                function view(env)
                  return column { pad = 10,
                    row { width = 100, height = 40, bg = 0x333333ff, tooltip = "a long hint" },
                  }
                end
            "#,
        )
        .unwrap();
        let mut core = Core::new();
        let idle = frame(&mut core, &mut ext);
        core.handle_input(InputEvent::CursorMoved(Vec2::new(50.0, 30.0)));
        let hovered = frame(&mut core, &mut ext);
        assert!(
            hovered > idle + 5,
            "hover should add the tooltip's quads ({idle} -> {hovered})"
        );
        core.handle_input(InputEvent::CursorLeft);
        assert_eq!(frame(&mut core, &mut ext), idle);
    }

    /// `role` / `label` / `checked` / `selected` / `expanded` / `value_*`
    /// The stock button reads the access rows and nothing else
    /// (`schema::BUTTON_ROWS_LUA`): `label` is the name and the text
    /// unless `text` says otherwise, `tooltip` is the description, and a
    /// row it would drop is warned about with the rows it does read.
    #[test]
    fn the_stock_button_admits_the_access_rows() {
        let mut ext = LuaExtension::from_source(
            "button",
            r#"
                function view(env)
                  return column { pad = 10, gap = 4,
                    button { label = "go", on_click = "go", description = "Starts the run" },
                    button { label = "Stop the run", text = "stop", on_click = "stop",
                             disabled = true, tooltip = "Nothing is running" },
                    button { label = "x", on_click = "x", radius = 12, hoverBg = 0x333333ff },
                  }
                end
            "#,
        )
        .unwrap();
        let mut core = Core::new();
        frame(&mut core, &mut ext);
        let tree = core.access_tree().clone();
        let named = |n: &str| {
            tree.nodes
                .iter()
                .find(|node| node.name.as_deref() == Some(n))
        };
        let go = named("go").expect("the button, named by its label");
        assert_eq!(go.role, kui_core::Role::Button);
        assert_eq!(go.description.as_deref(), Some("Starts the run"));
        assert!(!go.disabled);
        let stop = named("Stop the run").expect("named past its text");
        assert_eq!(stop.description.as_deref(), Some("Nothing is running"));
        assert!(stop.disabled);
        assert_eq!(tree.nodes.len(), 4, "window and three buttons");
        let ws = core.take_warnings();
        assert_eq!(ws.len(), 2, "{ws:?}");
        let radius = ws
            .iter()
            .find(|w| w.message.contains("`radius`"))
            .expect("a row the button does not read");
        assert!(
            radius
                .message
                .contains("is a prop, but not one button reads")
        );
        assert!(radius.message.contains("`description`"));
        // The camel spelling is a misspelling here, and the fix offered is
        // never a row the button would drop.
        let hover = ws
            .iter()
            .find(|w| w.message.contains("hoverBg"))
            .expect("a misspelling");
        assert!(hover.message.contains("is not a prop of button"));
        assert!(!hover.message.contains("did you mean"));
    }

    /// The one paint row the stock button takes: `accent` is a question
    /// put to the OS, not a colour, so a script that declares it gets the
    /// stock blue on a host that was never told what the accent is and
    /// the OS colour on one that was.
    #[test]
    fn an_accent_button_takes_the_colour_the_host_pushed() {
        let mut ext = LuaExtension::from_source(
            "accent",
            r#"
                function view(env)
                  return column { pad = 10,
                    button { label = "go", on_click = "go", accent = true },
                  }
                end
            "#,
        )
        .unwrap();
        let mut bg = |core: &mut Core| {
            frame(core, &mut ext);
            core.output().0.quads[0].color
        };
        let mut core = Core::new();
        assert_eq!(
            bg(&mut core),
            kui_core::Color::rgb8(0x3b, 0x5b, 0xd4),
            "no accent pushed, the stock button"
        );
        assert!(
            core.take_warnings().is_empty(),
            "and a row the button reads"
        );

        let accent = kui_core::Color::hex(0x007affff);
        core.env.system.accent = Some(accent);
        assert_eq!(bg(&mut core), accent);
    }

    /// are schema rows, so a script declares semantics like any other
    /// prop; the access tree shows them (and numbers a tab list itself),
    /// and an assistive request on a script's button emits its message.
    #[test]
    fn semantics_reach_the_access_tree() {
        let mut ext = LuaExtension::from_source(
            "a11y",
            r#"
                function view(env)
                  return column { pad = 10,
                    row { key = "save", on_click = "save", label = "Save", width = 20, height = 20 },
                    row { key = "check", role = "checkbox", checked = true, text("Remember") },
                    row { key = "vol", role = "slider", label = "Volume",
                          value_now = 3, value_min = 0, value_max = 10 },
                    row { key = "art", role = "none", on_click = "art", text("Art") },
                    row { key = "tip", tooltip = "more here", on_click = "t", text("Tip") },
                    row { key = "tabs", role = "tabList",
                      row { key = "t0", role = "tab", text("General") },
                      row { key = "t1", role = "tab", selected = true, text("Network") },
                    },
                    row { key = "adv", on_click = "adv", expanded = "collapsed",
                          label = "Advanced" },
                  }
                end
            "#,
        )
        .unwrap();
        let mut core = Core::new();
        frame(&mut core, &mut ext);
        let tree = core.access_tree().clone();
        let named = |n: &str| {
            tree.nodes
                .iter()
                .find(|node| node.name.as_deref() == Some(n))
                .cloned()
        };
        let save = named("Save").expect("labelled button");
        assert_eq!(save.role, kui_core::Role::Button);
        assert_eq!(save.origin, OriginId(1));
        let check = named("Remember").expect("checkbox named by its text");
        assert_eq!(check.role, kui_core::Role::Checkbox);
        assert_eq!(check.checked, Some(true));
        let vol = named("Volume").expect("slider");
        assert_eq!(
            (vol.number, vol.min, vol.max),
            (Some(3.0), Some(0.0), Some(10.0))
        );
        assert!(named("Art").is_none(), "role none hides a would-be button");
        let tip = named("Tip").expect("button");
        assert_eq!(tip.description.as_deref(), Some("more here"));
        // Every tab reports the state; the ordinals are the core's, not
        // the script's.
        let (t0, t1) = (named("General").unwrap(), named("Network").unwrap());
        assert_eq!((t0.selected, t1.selected), (Some(false), Some(true)));
        assert_eq!((t0.pos_in_set, t1.pos_in_set), (Some(0), Some(1)));
        let tabs = tree
            .nodes
            .iter()
            .find(|n| n.role == kui_core::Role::TabList)
            .expect("tab list");
        assert_eq!(tabs.set_size, Some(2));
        // An enum row, so a shut disclosure can say it is shut.
        let adv = named("Advanced").expect("disclosure");
        assert_eq!(adv.expanded, Some(false));

        let evs = core.handle_input(InputEvent::Access(kui_core::AccessRequest {
            key: save.key,
            action: kui_core::AccessAction::Click,
            value: None,
            anchor: None,
            focus: None,
        }));
        assert_eq!(evs.len(), 1);
        assert_eq!(evs[0].origin, OriginId(1));
        assert_eq!(evs[0].payload.get_str("tag"), Some("save"));
    }

    /// A script that draws its own editor: `role = "multilineTextInput"`
    /// on the sink, `role = "line"` rows with `caret` / `selection_anchor`
    /// byte offsets, and a selection request coming back as a table.
    #[test]
    fn a_custom_editor_in_lua_reaches_the_access_tree() {
        let mut ext = LuaExtension::from_source(
            "ed",
            r#"
                function view(env)
                  return column { key = "ed", role = "multilineTextInput", label = "Doc",
                    on_key = "keys",
                    row { role = "line", selection_anchor = 1, text("ab"), text("cd") },
                    row { role = "line", caret = 2, text("ef") },
                  }
                end
            "#,
        )
        .unwrap();
        let mut core = Core::new();
        frame(&mut core, &mut ext);
        let tree = core.access_tree().clone();
        let ed = tree
            .nodes
            .iter()
            .find(|n| n.name.as_deref() == Some("Doc"))
            .expect("the sink is the editor")
            .clone();
        assert_eq!(ed.role, kui_core::Role::MultilineTextInput);
        assert_eq!(ed.value.as_deref(), Some("abcd\nef"));
        assert_eq!(ed.runs.len(), 3);
        assert_eq!(ed.runs[1].text, "cd\n");
        assert_eq!(ed.caret, Some(7));
        assert_eq!(ed.selection, Some((1, 7)));
        let (a, f) = (ed.anchor.unwrap(), ed.focus.unwrap());
        assert_eq!((a.run, a.character), (ed.runs[0].key, 1));
        assert_eq!((f.run, f.character), (ed.runs[2].key, 2));

        let evs = core.handle_input(InputEvent::Access(
            kui_core::AccessRequest::new(ed.key, kui_core::AccessAction::SetTextSelection)
                .with_selection(
                    kui_core::TextPos {
                        run: ed.runs[1].key,
                        character: 1,
                    },
                    kui_core::TextPos {
                        run: ed.runs[2].key,
                        character: 0,
                    },
                ),
        ));
        assert_eq!(evs.len(), 1);
        assert_eq!(evs[0].origin, OriginId(1));
        // The request arrives on the editor's own sink as a `select` (ADR
        // 0047): the sink's message, the selection in its `select` field.
        assert_eq!(evs[0].event, kui_core::EventKind::Select);
        let msg = &evs[0].payload;
        assert_eq!(msg.get_str("tag"), Some("keys"));
        let p = evs[0].data().expect("the select field");
        let at = |k: &str, f: &str| p.get(k).and_then(|v| v.get(f)).and_then(Value::as_int);
        assert_eq!(
            (at("anchor", "line"), at("anchor", "offset")),
            (Some(0), Some(3))
        );
        assert_eq!(
            (at("focus", "line"), at("focus", "offset")),
            (Some(1), Some(0))
        );
    }

    /// `keep_tab` lowers: a field's Tab goes to the sink above it, and focus
    /// stays in the field (backlog F146).
    #[test]
    fn keep_tab_hands_a_fields_tab_to_the_sink_above() {
        let mut ext = LuaExtension::from_source(
            "keep_tab",
            r#"
                function view(env)
                  return column { on_key = "shell",
                    edit { key = "f", initial = "ab", autofocus = true, keep_tab = true, width = 200 },
                  }
                end
            "#,
        )
        .unwrap();
        let mut core = Core::new();
        frame(&mut core, &mut ext);
        frame(&mut core, &mut ext);
        let focused = core.focus();
        let events = core.handle_input(InputEvent::KeyDown(kui_core::KeyPress::new(
            kui_core::KeyCode::Tab,
            kui_core::KeyMods::default(),
        )));
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].event, kui_core::EventKind::Key);
        assert_eq!(events[0].data().unwrap().get_str("code"), Some("tab"));
        assert_eq!(events[0].payload.get_str("tag"), Some("shell"));
        core.handle_input(InputEvent::Key(kui_core::EditKey::Tab, Default::default()));
        assert_eq!(core.focus(), focused, "focus stays in the field");
    }

    /// `key_focus` on an editor takes focus as it first shows, from a
    /// control that holds it: the editor path dropped the row.
    #[test]
    fn key_focus_on_an_edit_takes_focus_as_it_shows() {
        let mut ext = LuaExtension::from_source(
            "key_focus_edit",
            r#"
                shown = false
                function view(env)
                  return column {
                    row { key = "b", role = "button", label = "B", on_click = "b", width = 40, height = 20 },
                    shown and edit { key = "f", initial = "", key_focus = true, width = 200 } or nil,
                  }
                end
            "#,
        )
        .unwrap();
        let mut core = Core::new();
        frame(&mut core, &mut ext);
        kui_core::testing::tab(&mut core, false);
        let button = core.focus();
        assert!(button.is_some(), "the button holds focus");
        ext.lua().globals().set("shown", true).unwrap();
        frame(&mut core, &mut ext);
        assert_ne!(core.focus(), button, "focus left the button");
        assert!(core.edit.focused().is_some(), "for the editor");
        assert_eq!(core.focus(), core.edit.focused());
    }

    /// `tab_moves_focus` lowers: a multiline editor's Tab leaves it for the
    /// next stop; without it Tab types (backlog C99).
    #[test]
    fn tab_moves_focus_leaves_a_document_and_without_it_tab_types() {
        for moves in [false, true] {
            let mut ext = LuaExtension::from_source(
                "tab_moves_focus",
                &format!(
                    r#"
                    function view(env)
                      return column {{
                        edit {{ key = "doc", initial = "", multiline = true, autofocus = true,
                                tab_moves_focus = {moves}, width = 200 }},
                        edit {{ key = "next", initial = "", width = 200 }},
                      }}
                    end
                "#
                ),
            )
            .unwrap();
            let mut core = Core::new();
            frame(&mut core, &mut ext);
            frame(&mut core, &mut ext);
            let doc = core.focus();
            core.handle_input(InputEvent::Key(kui_core::EditKey::Tab, Default::default()));
            if !moves {
                assert_eq!(core.focus(), doc, "focus stays");
                assert_eq!(core.edit_text(doc.unwrap()).as_deref(), Some("    "));
            } else {
                assert_ne!(core.focus(), doc, "tab leaves");
            }
        }
    }

    /// `secure` lowers: a password field, its text read back whole and
    /// its access node a password input of bullets (backlog F167).
    #[test]
    fn secure_makes_an_edit_a_password_field() {
        let mut ext = LuaExtension::from_source(
            "secure",
            r#"
                function view(env)
                  return column {
                    edit { key = "pw", initial = "hunter2", autofocus = true, secure = true,
                           label = "Password", width = 200 },
                  }
                end
            "#,
        )
        .unwrap();
        let mut core = Core::new();
        frame(&mut core, &mut ext);
        frame(&mut core, &mut ext);
        let warnings = core.take_warnings();
        assert!(warnings.is_empty(), "{warnings:?}");
        let node = core
            .access_tree()
            .nodes
            .iter()
            .find(|n| n.name.as_deref() == Some("Password"))
            .expect("the field")
            .clone();
        assert_eq!(node.role, kui_core::Role::PasswordInput);
        assert_eq!(node.value.as_deref(), Some("\u{2022}".repeat(7).as_str()));
        assert_eq!(core.edit_text(node.key).as_deref(), Some("hunter2"));
        assert!(core.secure_input() && core.ime_off());
    }

    /// Editors: autofocus, typing arrives as the `on_change` message with
    /// `change = { text }` in it, `event = "change"` and the node key beside
    /// it (ADR 0047), and `env.edit_text(key)` reads the buffer back next
    /// frame.
    #[test]
    fn edit_text_round_trips_through_events() {
        let mut ext = LuaExtension::from_source(
            "edit",
            r#"
                pending = nil
                heard = nil
                seen = nil
                by_label = nil
                window = nil
                function view(env)
                  if pending then seen = env.edit_text(pending) end
                  -- AR26: by the label its `key` declares, like every
                  -- query beside it; a label nothing declared is nil.
                  by_label = env.edit_text("note")
                  nothing = env.edit_text("nope")
                  return column {
                    edit { key = "note", initial = "hi", autofocus = true, width = 200,
                           on_change = { kind = "note" } },
                  }
                end
                function on_event(ev)
                  if ev.event == "change" and ev.kind == "note" then
                    pending = ev.node_key; window = ev.window; heard = ev.change.text
                  end
                end
            "#,
        )
        .unwrap();
        let mut core = Core::new();
        frame(&mut core, &mut ext);
        let events = core.handle_input(InputEvent::Text("!".into()));
        assert_eq!(
            events.len(),
            1,
            "typing into the autofocused editor emits one event"
        );
        assert_eq!(events[0].event, kui_core::EventKind::Change);
        for ev in &events {
            ext.on_event(ev);
        }
        let heard: Option<String> = ext.lua.globals().get("heard").unwrap();
        assert_eq!(heard.as_deref(), Some("hi!"), "the text rides on the event");
        frame(&mut core, &mut ext);
        // A single-line field opens with the caret after its seed (F20).
        let seen: Option<String> = ext.lua.globals().get("seen").unwrap();
        assert_eq!(seen.as_deref(), Some("hi!"));
        let by_label: Option<String> = ext.lua.globals().get("by_label").unwrap();
        assert_eq!(by_label.as_deref(), Some("hi!"), "the same text by label");
        let nothing: Option<String> = ext.lua.globals().get("nothing").unwrap();
        assert_eq!(nothing, None);
        // The event says which window it came from (AR26), the number
        // `env.window.id` reads: the main one here.
        let window: Option<i64> = ext.lua.globals().get("window").unwrap();
        assert_eq!(window, Some(0));
    }

    /// The editor at the model's text (ADR 0047): each row of a list
    /// declares the same `edit { key = "name" }` with its own message, so
    /// a change is routed by the message and not by a key; the script
    /// stores the text upper-cased and declares it back as `value`, which
    /// the editor is brought to, and Enter arrives as the row's
    /// `on_submit`.
    #[test]
    fn an_edit_takes_its_value_and_routes_by_its_message() {
        let mut ext = LuaExtension::from_source(
            "names",
            r#"
                names = { "ann", "bob" }
                heard = {}
                function view(env)
                  local rows = { gap = 4 }
                  for i, n in ipairs(names) do
                    rows[#rows + 1] = row { key = "r" .. i,
                      edit { key = "name", label = "name " .. i, value = n, width = 200,
                             on_change = { kind = "rename", id = i },
                             on_submit = { kind = "done", id = i } } }
                  end
                  return column(rows)
                end
                function on_event(ev)
                  heard[#heard + 1] = ev.event .. ":" .. ev.kind .. ":" .. ev.id
                  if ev.event == "change" then names[ev.id] = ev.change.text:upper() end
                end
            "#,
        )
        .unwrap();
        let mut core = Core::new();
        frame(&mut core, &mut ext);
        let bob = core
            .access_tree()
            .nodes
            .iter()
            .find(|n| n.name.as_deref() == Some("name 2"))
            .expect("the second row's editor")
            .key;
        assert_eq!(core.edit_text(bob).as_deref(), Some("bob"));
        core.set_focus(Some(bob));
        frame(&mut core, &mut ext);
        let feed = |core: &mut Core, ext: &mut LuaExtension, ev| {
            for e in core.handle_input(ev) {
                ext.on_event(&e);
            }
        };
        feed(&mut core, &mut ext, InputEvent::Text("x".into()));
        frame(&mut core, &mut ext);
        assert_eq!(
            core.edit_text(bob).as_deref(),
            Some("BOBX"),
            "brought to the declared value"
        );
        feed(
            &mut core,
            &mut ext,
            InputEvent::Key(kui_core::EditKey::Enter, Default::default()),
        );
        let heard: Vec<String> = ext.lua.globals().get("heard").unwrap();
        assert_eq!(heard, ["change:rename:2", "submit:done:2"]);
        let names: Vec<String> = ext.lua.globals().get("names").unwrap();
        assert_eq!(names, ["ann", "BOBX"]);
        let codes: Vec<_> = core.take_warnings().iter().map(|w| w.code).collect();
        assert!(codes.is_empty(), "{codes:?}");
    }

    /// The handler rows ADR 0047 added are table keys like `on_click`:
    /// each lands its message on the spec under its own row.
    #[test]
    fn the_new_handler_rows_are_table_keys() {
        let lua = Lua::new();
        let t = eval_table(
            &lua,
            r#"{ on_submit = { kind = "s" }, on_cancel = "c", on_boundary = { kind = "b" },
                 on_menu = { kind = "m" }, on_selection_range = { kind = "r" },
                 on_change = { kind = "ch" } }"#,
        );
        let p = parse_props(&t, false, &mut test_refs()).unwrap();
        let ev = p.spec.events();
        let kind = |v: Option<&Value>| v.and_then(|v| v.get_str("kind")).map(str::to_owned);
        assert_eq!(kind(ev.on_submit()).as_deref(), Some("s"));
        assert_eq!(ev.on_cancel(), Some(&Value::str("c")));
        assert_eq!(kind(ev.on_boundary()).as_deref(), Some("b"));
        assert_eq!(kind(ev.on_menu()).as_deref(), Some("m"));
        assert_eq!(kind(ev.on_selection_range()).as_deref(), Some("r"));
        assert_eq!(kind(ev.on_change()).as_deref(), Some("ch"));
    }

    /// `value` beside `initial` on one editor: the value wins, and the
    /// binding says so once as `edit-value-and-initial` — on `edit` and on
    /// the stock `input` alike.
    #[test]
    fn value_beside_initial_warns_and_wins() {
        let mut ext = LuaExtension::from_source(
            "both",
            r#"
                function view(env)
                  return column {
                    edit { key = "a", initial = "seed", value = "model", width = 100,
                           on_change = "a" },
                    input { label = "b", initial = "seed", value = "model" },
                  }
                end
            "#,
        )
        .unwrap();
        let mut core = Core::new();
        frame(&mut core, &mut ext);
        frame(&mut core, &mut ext);
        for label in ["a", "b"] {
            let key = core.key_of(label).unwrap();
            assert_eq!(core.edit_text(key).as_deref(), Some("model"), "{label}");
        }
        let both = core
            .take_warnings()
            .into_iter()
            .filter(|w| w.code == kui_core::diag::EDIT_VALUE_AND_INITIAL)
            .count();
        assert_eq!(both, 2, "once per editor");
    }

    /// `input { }` reports through the messages it is given — its change
    /// with `change = { text }`, Enter and Escape as declared — and a
    /// handler it is not given still reports, as the field alone: a table
    /// that is the envelope and nothing else (ADR 0047).
    #[test]
    fn the_stock_input_reports_in_its_messages() {
        let mut ext = LuaExtension::from_source(
            "input",
            r#"
                query = ""
                heard = {}
                function view(env)
                  return column {
                    input { label = "search", value = query,
                            on_change = { kind = "query" }, on_submit = { kind = "go" } },
                  }
                end
                function on_event(ev)
                  heard[#heard + 1] = ev.event .. ":" .. tostring(ev.kind)
                  if ev.event == "change" then query = ev.change.text end
                end
            "#,
        )
        .unwrap();
        let mut core = Core::new();
        frame(&mut core, &mut ext);
        let field = core.key_of("search").unwrap();
        core.set_focus(Some(field));
        frame(&mut core, &mut ext);
        let feed = |core: &mut Core, ext: &mut LuaExtension, ev| {
            for e in core.handle_input(ev) {
                assert_eq!(e.key, field);
                ext.on_event(&e);
            }
        };
        feed(&mut core, &mut ext, InputEvent::Text("kui".into()));
        frame(&mut core, &mut ext);
        feed(
            &mut core,
            &mut ext,
            InputEvent::Key(kui_core::EditKey::Enter, Default::default()),
        );
        feed(
            &mut core,
            &mut ext,
            InputEvent::Key(kui_core::EditKey::Escape, Default::default()),
        );
        let heard: Vec<String> = ext.lua.globals().get("heard").unwrap();
        assert_eq!(heard, ["change:query", "submit:go", "cancel:nil"]);
        let query: String = ext.lua.globals().get("query").unwrap();
        assert_eq!(query, "kui");
        assert_eq!(core.edit_text(field).as_deref(), Some("kui"));
    }

    /// A message that is not a table reaches `on_event` as one, so the
    /// envelope has somewhere to go: a string or a number under `tag`, as
    /// the core keeps one beside an event's data, and none as the envelope
    /// alone (ADR 0047, decision 3).
    #[test]
    fn a_message_that_is_not_a_table_arrives_as_one() {
        let mut ext = LuaExtension::from_source(
            "bare",
            r#"
                heard = {}
                function on_event(ev)
                  heard[#heard + 1] = ev.event .. ":" .. tostring(ev.tag) .. ":" .. ev.node_key
                end
                function view(env) return column {} end
            "#,
        )
        .unwrap();
        let key = Key::ROOT.str("x");
        for msg in [Some(Value::str("mute")), Some(Value::Int(7)), None] {
            ext.on_event(&UiEvent::declared(
                kui_core::EventKind::Click,
                OriginId(1),
                key,
                msg.as_ref(),
                Value::Null,
            ));
        }
        ext.on_event(&UiEvent::declared(
            kui_core::EventKind::Submit,
            OriginId(1),
            key,
            Some(&Value::Null),
            Value::Null,
        ));
        let heard: Vec<String> = ext.lua.globals().get("heard").unwrap();
        let k = key.0 as i64;
        assert_eq!(
            heard,
            [
                format!("click:mute:{k}"),
                format!("click:7:{k}"),
                format!("click:nil:{k}"),
                format!("submit:nil:{k}"),
            ]
        );
    }

    /// `dropdown { on_menu = }` names the message the choice arrives as,
    /// `menu = { role, item }` in it (ADR 0047), and the row is one the
    /// select reads, so it warns nothing.
    #[test]
    fn a_dropdown_reports_in_its_on_menu_message() {
        let mut ext = LuaExtension::from_source(
            "dd",
            r#"
                heard = nil
                function view(env)
                  return column { pad = 10,
                    dropdown { label = "language", options = { "English", "Deutsch" },
                               current = 1, on_menu = { kind = "lang" } } }
                end
                function on_event(ev)
                  heard = ev.event .. ":" .. ev.kind .. ":" .. ev.menu.item
                end
            "#,
        )
        .unwrap();
        let mut core = Core::new();
        frame(&mut core, &mut ext);
        let field = core.key_of("language").unwrap();
        core.handle_input(InputEvent::Access(kui_core::AccessRequest::new(
            field,
            kui_core::AccessAction::Click,
        )));
        frame(&mut core, &mut ext);
        for ev in core.activate_menu_item(1).expect("the row is enabled") {
            ext.on_event(&ev);
        }
        let heard: String = ext.lua.globals().get("heard").unwrap();
        assert_eq!(heard, "menu:lang:Deutsch");
        let codes: Vec<_> = core.take_warnings().iter().map(|w| w.code).collect();
        assert!(codes.is_empty(), "{codes:?}");
    }

    /// `dropdown { }` is the stock select (backlog F73): the click opens
    /// the core's menu under the field and reaches the script as nothing;
    /// a row chosen is one `menu` event on the field, its `item` the
    /// option's label or id, which the script draws back as `current`.
    #[test]
    fn a_dropdown_opens_the_cores_menu_and_hears_the_choice() {
        let mut ext = LuaExtension::from_source(
            "dd",
            r#"
                current = 1
                heard = {}
                function view(env)
                  return column { pad = 10,
                    dropdown { label = "language",
                               options = { "English", "Deutsch", { label = "Latin", id = "la" } },
                               current = current },
                  }
                end
                function on_event(ev)
                  heard[#heard + 1] = ev.event
                  if ev.event == "menu" then
                    if ev.menu.item == "Deutsch" then current = 2 end
                    if ev.menu.item == "la" then current = 3 end
                  end
                end
            "#,
        )
        .unwrap();
        let mut core = Core::new();
        frame(&mut core, &mut ext);
        let field = core
            .key_of("language")
            .expect("the field is keyed by its label");
        let node = |core: &mut Core| {
            core.access_tree()
                .nodes
                .iter()
                .find(|n| n.key == field)
                .cloned()
                .unwrap()
        };
        assert_eq!(node(&mut core).description.as_deref(), Some("English"));
        let events = core.handle_input(InputEvent::Access(kui_core::AccessRequest::new(
            field,
            kui_core::AccessAction::Click,
        )));
        assert!(
            events.is_empty(),
            "the field's click is the core's: {events:?}"
        );
        let menu = core.menu().expect("the menu opened");
        assert_eq!(menu.target, field);
        assert_eq!(menu.items.len(), 3);
        assert!(menu.items[0].checked);
        frame(&mut core, &mut ext);
        // The row, as a host's menu would answer it.
        let events = core.activate_menu_item(2).expect("the row is enabled");
        assert_eq!(events.len(), 1);
        for ev in &events {
            ext.on_event(ev);
        }
        frame(&mut core, &mut ext);
        assert_eq!(node(&mut core).description.as_deref(), Some("Latin"));
        let heard: Vec<String> = ext
            .lua
            .globals()
            .get::<Table>("heard")
            .unwrap()
            .sequence_values()
            .collect::<mlua::Result<_>>()
            .unwrap();
        assert_eq!(heard, vec!["menu".to_string()]);
        assert!(core.menu().is_none());
        // What the table refuses: no options, a current from 0.
        let mut ext = LuaExtension::from_source(
            "bad",
            r#"function view(env) return dropdown { label = "x" } end"#,
        )
        .unwrap();
        let mut ui = core.frame(Size::new(300.0, 200.0), 1.0);
        ui.set_origin(OriginId(1));
        let err = ext
            .view(&Instance::root(), &mut ui)
            .unwrap_err()
            .to_string();
        assert!(err.contains("needs options"), "{err}");
        ui.finish();
        let mut ext = LuaExtension::from_source(
            "bad",
            r#"function view(env) return dropdown { label = "x", options = { "a" }, current = 0 } end"#,
        )
        .unwrap();
        let mut ui = core.frame(Size::new(300.0, 200.0), 1.0);
        ui.set_origin(OriginId(1));
        let err = ext
            .view(&Instance::root(), &mut ui)
            .unwrap_err()
            .to_string();
        assert!(err.contains("index from 1"), "{err}");
        ui.finish();
        // And, by name (backlog RG10): no label, and no options at all —
        // the core's one reader refusing the empty list.
        let refused = |core: &mut Core, src: &str| {
            let mut ext = LuaExtension::from_source("bad", src).unwrap();
            let mut ui = core.frame(Size::new(300.0, 200.0), 1.0);
            ui.set_origin(OriginId(1));
            let err = ext
                .view(&Instance::root(), &mut ui)
                .unwrap_err()
                .to_string();
            ui.finish();
            err
        };
        let err = refused(
            &mut core,
            r#"function view(env) return dropdown { options = { "a" } } end"#,
        );
        assert!(err.contains("dropdown needs a label"), "{err}");
        let err = refused(
            &mut core,
            r#"function view(env) return dropdown { label = "x", options = {} } end"#,
        );
        assert!(err.contains("at least one option"), "{err}");
    }

    /// The select's checks the core makes for every binding, seen from
    /// Lua (backlog RG9, RG10): a row's key no row reads warns as an
    /// unknown prop does, `current` past the end or on a separator warns
    /// and is none, and a disabled option reported chosen is refused with
    /// the menu still open.
    /// A key no row reads warns inside a submenu too, and a dropdown's
    /// option is chosen, never opened: its `items` warn and are dropped
    /// (backlog RG150).
    #[test]
    fn a_stray_key_inside_a_submenu_warns_and_an_option_takes_no_items() {
        let mut ext = LuaExtension::from_source(
            "sub",
            r#"
                function view(env)
                  return column {
                    menu_bar { menu = { { label = "View", items = {
                      { label = "Sort by", items = { { label = "Name", disabled = true } } },
                    } } } },
                    dropdown { label = "sort",
                               options = { "Name", { label = "Date", items = { { label = "Newest" } } } } },
                  }
                end
            "#,
        )
        .unwrap();
        let mut core = Core::new();
        frame(&mut core, &mut ext);
        let warned = core.take_warnings();
        let messages: Vec<&str> = warned.iter().map(|w| w.message.as_str()).collect();
        assert_eq!(warned.len(), 2, "{messages:?}");
        assert!(
            messages[0].contains("`disabled` is not a key of a menu item"),
            "{messages:?}"
        );
        assert!(
            messages[1].contains("`items` on a select's option is dropped"),
            "{messages:?}"
        );
        // And dropped they are: the dropdown's menu opens with plain rows.
        let field = core.key_of("sort").unwrap();
        core.handle_input(InputEvent::Access(kui_core::AccessRequest::new(
            field,
            kui_core::AccessAction::Click,
        )));
        let menu = core.menu().expect("the dropdown opened");
        assert_eq!(menu.items.len(), 2);
        assert!(
            menu.items.iter().all(|i| !i.has_submenu()),
            "{:?}",
            menu.items
        );
    }

    #[test]
    fn a_dropdowns_bad_rows_and_current_are_warned_and_a_disabled_option_is_refused() {
        let mut ext = LuaExtension::from_source(
            "dd",
            r#"
                current = 4
                function view(env)
                  return column { pad = 10,
                    dropdown { label = "language",
                               options = { "English", { role = "separator" },
                                           { label = "Latin", id = "la", disabled = true } },
                               current = current },
                  }
                end
            "#,
        )
        .unwrap();
        let mut core = Core::new();
        frame(&mut core, &mut ext);
        let field = core.key_of("language").unwrap();
        let node = |core: &mut Core| {
            core.access_tree()
                .nodes
                .iter()
                .find(|n| n.key == field)
                .cloned()
                .unwrap()
        };
        assert_eq!(node(&mut core).description.as_deref(), Some(""));
        let warned = core.take_warnings();
        let codes: Vec<&str> = warned.iter().map(|w| w.code).collect();
        assert_eq!(
            codes,
            [
                kui_core::diag::UNKNOWN_PROP,
                kui_core::diag::SELECT_CURRENT_IGNORED
            ],
            "{warned:?}"
        );
        assert!(
            warned[0]
                .message
                .contains("`disabled` is not a key of a menu item")
                && warned[0].message.contains("`enabled: false`"),
            "{}",
            warned[0].message
        );
        assert_eq!(warned[1].key, field);
        assert!(
            warned[1].message.contains("names option 3 counted from 0")
                && warned[1].message.contains("the field has 3 options"),
            "{}",
            warned[1].message
        );
        // The separator in force: the core's index, so `current = 2`.
        ext.lua.globals().set("current", 2).unwrap();
        let mut core = Core::new();
        frame(&mut core, &mut ext);
        let warned = core.take_warnings();
        assert!(
            warned
                .iter()
                .any(|w| w.code == kui_core::diag::SELECT_CURRENT_IGNORED
                    && w.message
                        .contains("option 1 counted from 0, which is a separator")),
            "{warned:?}"
        );
        // `disabled` was dropped, so Latin is enabled and can be chosen:
        // spell it as the row reads it and the door refuses it.
        let mut ext = LuaExtension::from_source(
            "dd",
            r#"
                function view(env)
                  return column { pad = 10,
                    dropdown { label = "language",
                               options = { "English", { label = "Latin", id = "la", enabled = false } },
                               current = 1 },
                  }
                end
            "#,
        )
        .unwrap();
        let mut core = Core::new();
        frame(&mut core, &mut ext);
        let field = core.key_of("language").unwrap();
        core.handle_input(InputEvent::Access(kui_core::AccessRequest::new(
            field,
            kui_core::AccessAction::Click,
        )));
        assert!(core.menu().is_some());
        assert_eq!(core.activate_menu_item(1), None, "refused");
        assert!(core.menu().is_some(), "the menu stays open");
        let events = core
            .activate_menu_item(0)
            .expect("the enabled row is taken");
        assert_eq!(events.len(), 1);
        assert!(core.menu().is_none());
        assert!(core.take_warnings().is_empty());
    }

    /// `grid { }` is a table (ADR 0033): its rows' cells line up, each
    /// column as wide as its widest cell, a bare text a cell too.
    #[test]
    fn a_grid_lines_its_rows_cells_up() {
        let mut ext = LuaExtension::from_source(
            "grid",
            r#"
                function view(env)
                  return grid { key = "t", width = 300,
                    row { key = "r1", width = "grow", gap = 8,
                      text("ab", { size = 12 }),
                      column { key = "b1", width = 10, height = 10, bg = 0xff0000ff },
                      column { key = "c1", width = "grow", height = 10, bg = 0x00ff00ff },
                    },
                    row { key = "r2", width = "grow", gap = 8,
                      text("abcdef", { size = 12 }),
                      column { key = "b2", width = 50, height = 10, bg = 0xff0000ff },
                      column { key = "c2", width = 20, height = 10, bg = 0x00ff00ff },
                    },
                  }
                end
            "#,
        )
        .unwrap();
        let mut core = Core::new();
        core.set_inspect(true);
        frame(&mut core, &mut ext);
        let rect = |core: &mut Core, label: &str| {
            let key = core.key_of(label).unwrap_or_else(|| panic!("{label}"));
            core.nodes()
                .iter()
                .find(|n| n.key == key)
                .map(|n| n.rect)
                .unwrap_or_else(|| panic!("{label}"))
        };
        let (b1, b2) = (rect(&mut core, "b1"), rect(&mut core, "b2"));
        let (c1, c2) = (rect(&mut core, "c1"), rect(&mut core, "c2"));
        assert_eq!(
            b1.x, b2.x,
            "the fixed column starts after the longest label"
        );
        assert_eq!(b1.w, 50.0, "the fixed column is its widest cell");
        assert_eq!(b2.w, 50.0);
        assert_eq!(c1.x, c2.x);
        assert_eq!(c1.w, c2.w, "the grow column is one width in both rows");
        assert_eq!(c1.x + c1.w, 300.0, "and it takes the rest");
        let t = core.key_of("t").unwrap();
        assert!(
            core.nodes().iter().any(|n| n.key == t && n.table),
            "the grid is a table"
        );
        assert!(core.take_warnings().is_empty());
    }

    /// `env.set_edit_text` by the label the view declares: the spelling a
    /// script that is *opening* the editor can use, since the key comes
    /// from an event the editor has not fired (backlog F32). The frame
    /// that declares the field takes the held text over its `initial`.
    #[test]
    fn set_edit_text_by_label_seeds_the_editor_the_next_frame_declares() {
        let mut ext = LuaExtension::from_source(
            "edit",
            r#"
                frames = 0
                function view(env)
                  frames = frames + 1
                  if frames == 1 then return column {} end
                  if frames == 2 then
                    env.set_edit_text("note", "from the model")
                  end
                  return column {
                    edit { key = "note", initial = "ignored", width = 200 },
                  }
                end
            "#,
        )
        .unwrap();
        let mut core = Core::new();
        // A frame with no editor in it, then the one that opens the field:
        // `env` lives inside `view`, so the call is made from the frame
        // that declares the editor and its tree is what claims the text.
        frame(&mut core, &mut ext);
        frame(&mut core, &mut ext);
        let key = core.key_of("note").expect("the view declared the editor");
        assert_eq!(core.edit_text(key).as_deref(), Some("from the model"));
        let codes: Vec<&str> = core.take_warnings().iter().map(|w| w.code).collect();
        assert!(!codes.contains(&"edit-text-without-editor"), "{codes:?}");
        // And it is the seed, not a per-frame reset: the next frame's
        // typing is kept.
        core.set_focus(Some(key));
        core.handle_input(InputEvent::Text("!".into()));
        frame(&mut core, &mut ext);
        assert_eq!(core.edit_text(key).as_deref(), Some("from the model!"));
    }

    /// A script that owns its keyboard and asks for releases (`key_up`)
    /// sees both halves of a key in one `key` field of its message: a held
    /// key is `phase="down"` then `"up"`, and focus leaving while it is
    /// held delivers the `up` anyway.
    #[test]
    fn a_lua_key_sink_hears_press_and_release() {
        use kui_core::{KeyCode, KeyMods, KeyPress};
        let mut ext = LuaExtension::from_source(
            "game",
            r#"
                log = {}
                function view(env)
                  return column { key = "world", on_key = "keys", key_up = true,
                    key_focus = true, width = 400, height = 300 }
                end
                function on_event(ev)
                  if ev.event == "key" then
                    local k = ev.key
                    log[#log + 1] = k.phase .. ":" .. k.code ..
                      "@" .. k.physical ..
                      ":" .. tostring(k.text) .. ":" .. tostring(ev.tag)
                  end
                end
            "#,
        )
        .unwrap();
        let mut core = Core::new();
        frame(&mut core, &mut ext);
        let feed = |core: &mut Core, ext: &mut LuaExtension, ev| {
            for e in core.handle_input(ev) {
                ext.on_event(&e);
            }
        };
        let w = || KeyPress::new(KeyCode::Char('w'), KeyMods::default()).with_text("w");
        feed(&mut core, &mut ext, InputEvent::KeyDown(w()));
        feed(&mut core, &mut ext, InputEvent::KeyUp(w()));
        // The same key on a Russian layout, as a driver reports it: the
        // layout says "ц", the position says W. A script matching on
        // `ev.code` keeps working, and `ev.physical` is there for one that
        // would rather bind the position.
        let ru = || {
            KeyPress::from_layout(KeyCode::Char('ц'), KeyCode::Char('w'), KeyMods::default())
                .with_text("ц")
        };
        feed(&mut core, &mut ext, InputEvent::KeyDown(ru()));
        feed(&mut core, &mut ext, InputEvent::KeyUp(ru()));
        // Pressed again, then focus dropped while it is still down.
        feed(&mut core, &mut ext, InputEvent::KeyDown(w()));
        core.set_focus(None);
        for e in core.take_pending_events() {
            ext.on_event(&e);
        }
        let log: Vec<String> = ext.lua.globals().get("log").unwrap();
        assert_eq!(
            log,
            [
                "down:w@w:w:keys",
                // A release inserts nothing, so `text` is nil in Lua.
                "up:w@w:nil:keys",
                // The layout key never reaches `code`; the text it inserts
                // is still the layout's own.
                "down:w@w:ц:keys",
                "up:w@w:nil:keys",
                "down:w@w:w:keys",
                "up:w@w:nil:keys",
            ]
        );
    }

    /// A script reads the frame clock (backlog F134): `env.now` is the
    /// core's seconds, so a deadline a script keeps moves with the host's
    /// clock and a test's.
    /// A path's `rotate` is its own turn (ADR 0041); the node's `scale`
    /// beside it stays the node's, as Node's encoder keeps it.
    #[test]
    fn a_path_keeps_the_node_scale_beside_its_own_rotate() {
        let mut ext = LuaExtension::from_source(
            "p",
            r#"
                function view(env)
                  return column {
                    path { key = "p", d = "M0 0 H10 V10 H0 Z", bg = 0xffffffff,
                           rotate = 0.25, scale = 2 },
                  }
                end
            "#,
        )
        .unwrap();
        let mut core = Core::new();
        core.set_inspect(true);
        frame(&mut core, &mut ext);
        frame(&mut core, &mut ext);
        let node = core
            .nodes()
            .iter()
            .find(|n| n.kind == kui_core::NodeKind::Path)
            .cloned()
            .expect("the path");
        assert_eq!(node.scale, 2.0, "the node's scale stays");
        assert_eq!(node.rotate, 0.0, "the turn is the path's own");
    }

    #[test]
    fn a_script_reads_the_frame_clock() {
        let mut ext = LuaExtension::from_source(
            "clock",
            r#"
                function view(env)
                  if env.now < 2 then
                    return column { key = "toast", width = 10, height = 10 }
                  end
                  return column {}
                end
            "#,
        )
        .unwrap();
        let mut core = Core::new();
        core.set_time(1.5);
        frame(&mut core, &mut ext);
        assert!(core.key_of("toast").is_some(), "before the deadline");
        core.set_time(2.5);
        frame(&mut core, &mut ext);
        assert!(core.key_of("toast").is_none(), "after it");
    }

    /// A script asks for a frame at a time (backlog F135): the deadline is
    /// the host's to sleep to, and nothing is owed until then.
    #[test]
    fn a_script_asks_for_a_frame_at_a_time() {
        let mut ext = LuaExtension::from_source(
            "toast",
            r#"
                function view(env)
                  env.request_frame_at(env.now + 3)
                  return column { key = "toast", width = 10, height = 10 }
                end
            "#,
        )
        .unwrap();
        let mut core = Core::new();
        core.set_time(1.0);
        frame(&mut core, &mut ext);
        assert_eq!(core.next_frame_at(), Some(4.0));
        assert!(!core.animating());
    }

    /// A script records into the host's timeline (ADR 0058): `env.span`
    /// returns what its function returned and nests, ends on a throw,
    /// and the counter, gauge, mark and measure land beside it while the
    /// core records — and none of it while it does not.
    #[test]
    fn a_script_records_spans_and_counters() {
        use kui_core::kui_trace::Record;
        let mut ext = LuaExtension::from_source(
            "perf",
            r#"
                function view(env)
                  measuring = env.perf_measuring()
                  answer = env.span("rows", function()
                    env.perf_count("rows", 3)
                    return env.span("inner", function() return 41 end) + 1
                  end)
                  threw = not pcall(env.span, "boom", function() error("x") end)
                  env.span("after", function() end)
                  env.perf_gauge("bytes", 10)
                  env.perf_mark("opened")
                  env.perf_measure("parse", 2)
                  env.request_frame_for("blink")
                  return column { key = "root" }
                end
            "#,
        )
        .unwrap();
        let mut core = Core::new();
        frame(&mut core, &mut ext);
        let g = ext.lua.globals();
        assert!(!g.get::<bool>("measuring").unwrap(), "nothing listens");
        assert_eq!(g.get::<i64>("answer").unwrap(), 42, "off, span only calls");
        assert!(core.perf_trace_read().records.is_empty());

        core.set_perf_recording(true);
        frame(&mut core, &mut ext);
        frame(&mut core, &mut ext);
        core.set_perf_recording(false);
        let g = ext.lua.globals();
        assert!(g.get::<bool>("measuring").unwrap());
        assert_eq!(g.get::<i64>("answer").unwrap(), 42);
        assert!(g.get::<bool>("threw").unwrap());
        let t = core.perf_trace_read();
        let span = |name: &str| {
            t.records
                .iter()
                .rev()
                .find_map(|r| match r {
                    Record::Span(s) if t.name(s.name) == name => Some(s.clone()),
                    _ => None,
                })
                .unwrap_or_else(|| panic!("no span {name}"))
        };
        let (rows, inner) = (span("rows"), span("inner"));
        assert_eq!(inner.parent, rows.id, "inner nests in rows");
        let (boom, after) = (span("boom"), span("after"));
        assert_eq!(after.parent, boom.parent, "the throw ended its span");
        assert_eq!(span("parse").dur, 2000);
        let has = |want: &dyn Fn(&Record) -> bool| t.records.iter().any(want);
        assert!(has(
            &|r| matches!(r, Record::Counter { name, value: 3, .. } if t.name(*name) == "rows")
        ));
        assert!(has(
            &|r| matches!(r, Record::Gauge { name, .. } if t.name(*name) == "bytes")
        ));
        assert!(has(
            &|r| matches!(r, Record::Mark { name, .. } if t.name(*name) == "opened")
        ));
        assert!(has(
            &|r| matches!(r, Record::Request { why, .. } if t.name(*why) == "blink")
        ));
    }

    /// A script aims the exit a card leaves by in the frame that drops it
    /// (backlog F136): thrown right, over the fade it declared.
    #[test]
    fn a_script_names_the_exit_at_the_removal() {
        let mut ext = LuaExtension::from_source(
            "throw",
            r##"
                function view(env)
                  if env.now < 1 then
                    return column { pad = 20,
                      column { key = "card", width = 40, height = 40, bg = "#ffffff",
                        transition = 1000, easing = "linear", exit = { opacity = 0 } } }
                  end
                  if env.now < 1.1 then env.exit_with("card", { dx = 400, opacity = 0 }) end
                  return column { pad = 20 }
                end
            "##,
        )
        .unwrap();
        let mut core = Core::new();
        core.set_time(0.0);
        frame(&mut core, &mut ext);
        core.set_time(1.0);
        frame(&mut core, &mut ext);
        core.set_time(1.5);
        frame(&mut core, &mut ext);
        let xs: Vec<f32> = kui_core::testing::solids(&mut core)
            .iter()
            .map(|q| q.rect.x)
            .collect();
        assert!(
            xs.iter().any(|x| (x - 220.0).abs() < 1.0),
            "halfway along the throw: {xs:?}"
        );
    }

    /// A script's editor blinks (backlog C35): `env.caret_visible` is the
    /// phase, read in `view`; the `caret` row on its line is what the
    /// host's clock is armed on, kept through the off phase.
    #[test]
    fn a_lua_editor_draws_its_caret_on_the_phase_the_host_sets() {
        let mut ext = LuaExtension::from_source(
            "ed",
            r#"
                function view(env)
                  local caret = env.caret_visible and row { key = "caret", width = 2, height = 16 } or nil
                  return column { key = "editor", on_key = "ed", key_focus = true,
                    role = "multilineTextInput", label = "buf",
                    row { role = "line", caret = 3, text("let value", { family = "mono", size = 14 }), caret },
                  }
                end
            "#,
        )
        .unwrap();
        let mut core = Core::new();
        frame(&mut core, &mut ext);
        assert!(
            core.key_of("caret").is_some(),
            "the on phase draws the caret"
        );
        assert!(core.has_caret(), "the caret row is a caret to blink");
        core.set_caret_visible(false);
        frame(&mut core, &mut ext);
        assert!(core.key_of("caret").is_none(), "the off phase draws none");
        assert!(
            core.has_caret(),
            "and the row stays, so the clock stays armed"
        );
        core.set_caret_visible(true);
        frame(&mut core, &mut ext);
        assert!(core.key_of("caret").is_some());
    }

    /// A script's block caret is solid (backlog F68): `caret_solid = true`
    /// beside `caret` on the line keeps the IME anchor and the access
    /// tree's caret and arms no clock, so a script idling in normal mode
    /// draws no frame for it; the line without it blinks again.
    #[test]
    fn a_lua_editors_solid_caret_arms_no_clock() {
        let mut ext = LuaExtension::from_source(
            "ed",
            r#"
                function view(env)
                  return column { key = "editor", on_key = "ed", key_focus = true,
                    role = "multilineTextInput", label = "buf",
                    row { role = "line", caret = 3, caret_solid = true,
                      text("let value", { family = "mono", size = 14 }) },
                  }
                end
            "#,
        )
        .unwrap();
        let mut core = Core::new();
        frame(&mut core, &mut ext);
        assert!(!core.has_caret(), "a solid caret is not a caret to blink");
        assert!(core.ime_rect().is_some(), "and still the IME's anchor");
        let editor = core.key_of("editor").unwrap();
        assert_eq!(core.access_tree().get(editor).unwrap().caret, Some(3));
        assert!(
            core.warnings_raised().is_empty(),
            "{:?}",
            core.warnings_raised()
        );
    }

    /// A script that owns its text has a clipboard (backlog C33) and a
    /// mouse (C34): `y` in its keymap queues `env.set_clipboard` from the
    /// next view and `p` asks `env.request_paste`, whose answer arrives as
    /// a `text` event on the sink; a press inside the sink says which
    /// `role = "line"` row it landed on, where, and how many clicks.
    #[test]
    fn a_lua_editor_yanks_pastes_and_hears_where_a_press_landed() {
        use kui_core::testing::{press, release};
        use kui_core::{KeyCode, KeyMods, KeyPress, MenuAction, Vec2};
        let mut ext = LuaExtension::from_source(
            "ed",
            r#"
                yank = nil
                paste = false
                log = {}
                function view(env)
                  if yank then env.set_clipboard(yank, nil); yank = nil end
                  if secret then env.set_clipboard_secret(secret); secret = nil end
                  -- Asked on every view until the answer lands: the core
                  -- takes one ask at a time (AR34), so this is one paste.
                  if paste then env.request_paste() end
                  return column { key = "editor", on_key = "ed", on_drag = "sel",
                    key_focus = true, role = "multilineTextInput", label = "buf",
                    width = 400, height = 300,
                    row { role = "line", height = 20, text("hello world", { family = "mono", size = 14 }) },
                    row { role = "line", height = 20, text("second", { family = "mono", size = 14 }) },
                  }
                end
                function on_event(ev)
                  local key = ev.key
                  if key and key.code == "y" then yank = "hello world" end
                  if key and key.code == "s" then secret = "hunter2" end
                  if key and key.code == "p" then paste = true end
                  if ev.event == "text" then
                    local t = ev.text
                    paste = false
                    local marks = (t.concealed and ":concealed" or "") .. (t.transient and ":transient" or "")
                    log[#log + 1] = "text:" .. t.text .. marks
                  end
                  if ev.event == "drag" then
                    local d = ev.drag
                    log[#log + 1] = d.phase .. ":" .. d.line .. ":" .. d.byte .. ":" .. d.clicks
                  end
                end
            "#,
        )
        .unwrap();
        let mut core = Core::new();
        frame(&mut core, &mut ext);
        let feed = |core: &mut Core, ext: &mut LuaExtension, ev| {
            for e in core.handle_input(ev) {
                ext.on_event(&e);
            }
        };
        let key = |c| KeyPress::new(KeyCode::Char(c), KeyMods::default());
        feed(&mut core, &mut ext, InputEvent::KeyDown(key('y')));
        frame(&mut core, &mut ext);
        assert_eq!(
            core.take_menu_actions(),
            vec![MenuAction::SetClipboard {
                text: "hello world".into(),
                html: None
            }]
        );
        // A secret, for the host to write marked (backlog F84).
        feed(&mut core, &mut ext, InputEvent::KeyDown(key('s')));
        frame(&mut core, &mut ext);
        assert_eq!(
            core.take_menu_actions(),
            vec![MenuAction::SetClipboardSecret {
                text: "hunter2".into()
            }]
        );
        feed(&mut core, &mut ext, InputEvent::KeyDown(key('p')));
        frame(&mut core, &mut ext);
        frame(&mut core, &mut ext);
        assert_eq!(
            core.take_menu_actions(),
            vec![MenuAction::Paste],
            "two views asked, one paste queued"
        );
        assert!(core.awaiting_paste());
        // The host reads the clipboard and commits it.
        feed(
            &mut core,
            &mut ext,
            InputEvent::Commit("from the clipboard".into()),
        );
        assert!(!core.awaiting_paste());
        frame(&mut core, &mut ext);
        assert!(
            core.take_menu_actions().is_empty(),
            "answered: the script stopped asking"
        );
        // A paste the pasteboard marked: the script reads the markers.
        feed(
            &mut core,
            &mut ext,
            InputEvent::Paste {
                text: "s3cret".into(),
                marks: kui_core::ClipboardMarks::SECRET,
            },
        );
        // A double click on the second line, past its end.
        for e in press(&mut core, Vec2::new(390.0, 30.0)) {
            ext.on_event(&e);
        }
        for e in release(&mut core) {
            ext.on_event(&e);
        }
        for e in core.handle_input(InputEvent::mouse_down(2)) {
            ext.on_event(&e);
        }
        for e in release(&mut core) {
            ext.on_event(&e);
        }
        let log: Vec<String> = ext.lua.globals().get("log").unwrap();
        assert_eq!(
            log,
            [
                "text:from the clipboard",
                "text:s3cret:concealed:transient",
                "start:1:6:1",
                "end:1:6:1",
                "start:1:6:2",
                "end:1:6:2",
            ]
        );
    }

    /// `audio { }` nodes are retained playbacks: declared → play, declared
    /// again → nothing, gone → stop. The host hands the sound id to the
    /// script as an integer, like images.
    #[test]
    fn audio_nodes_drive_playback_commands() {
        use kui_core::AudioCommand;
        let mut ext = LuaExtension::from_source(
            "audio",
            r#"
                playing = true
                function view(env)
                  local items = {}
                  if playing then
                    items[1] = audio { src = SOUND, loop = true, volume = 0.5,
                                       key = "music", tag = { kind = "music" } }
                  end
                  return column { table.unpack(items) }
                end
            "#,
        )
        .unwrap();
        let mut core = Core::new();
        let sound = core.add_sound(vec![0; 8]);
        ext.lua
            .globals()
            .set("SOUND", sound.to_ffi() as i64)
            .unwrap();
        frame(&mut core, &mut ext);
        let cmds = core.take_audio_commands();
        assert!(
            matches!(
                cmds.as_slice(),
                [AudioCommand::Play { sound: s, looped: true, volume, .. }]
                    if *s == sound && *volume == 0.5
            ),
            "{cmds:?}"
        );
        frame(&mut core, &mut ext);
        assert!(
            core.take_audio_commands().is_empty(),
            "re-declaring is silent"
        );
        ext.lua.globals().set("playing", false).unwrap();
        frame(&mut core, &mut ext);
        assert!(matches!(
            core.take_audio_commands().as_slice(),
            [AudioCommand::Stop { .. }]
        ));
    }

    /// `env.caret_in_text` asked before any node drew the text answers the
    /// rect `env.caret_rect` gives once one lays it out at that width, on
    /// every visual line (backlog F171).
    #[test]
    fn caret_in_text_answers_where_layout_puts_each_byte() {
        let mut ext = LuaExtension::from_source(
            "caret_in",
            r#"
                asked, laid = nil, nil
                local style = { size = 14, family = "mono", line_height = 20, wrap = "word" }
                local spans = { "the quick ", { "brown", bold = true }, " fox jumps over the lazy dog" }
                local len = #"the quick brown fox jumps over the lazy dog"
                function view(env)
                  if not asked then
                    asked = {}
                    for b = 0, len do asked[b + 1] = env.caret_in_text(spans, style, 120, b) end
                    local node = env.caret_in_text(text(spans, style), nil, 120, len)
                    assert(node.x == asked[len + 1].x and node.y == asked[len + 1].y)
                    local plain = env.caret_in_text("one line", style, nil, 99)
                    assert(plain.y == 0 and plain.h == 20)
                    assert(plain.x == env.measure_text("one line", style).width)
                  else
                    laid = {}
                    for b = 0, len do laid[b + 1] = env.caret_rect("para", b) end
                  end
                  return column { column { key = "para", width = 120, text(spans, style) } }
                end
            "#,
        )
        .unwrap();
        let mut core = Core::new();
        frame(&mut core, &mut ext);
        frame(&mut core, &mut ext);
        let asked: Table = ext.lua.globals().get("asked").unwrap();
        let laid: Table = ext.lua.globals().get("laid").unwrap();
        let rect = |t: &Table, i: i64| {
            let r: Table = t.get(i).unwrap();
            ["x", "y", "w", "h"].map(|f| r.get::<f64>(f).unwrap())
        };
        let mut rows = std::collections::BTreeSet::new();
        for i in 1..=laid.raw_len() as i64 {
            assert_eq!(rect(&asked, i), rect(&laid, i), "byte {}", i - 1);
            rows.insert(rect(&laid, i)[1] as i64);
        }
        assert!(rows.len() >= 3, "wraps: {rows:?}");
    }

    /// `env.measure_text` answers what layout gives the same text, in every
    /// input shape, and `on_layout` rects arrive in `on_event` with the
    /// node key like any other event.
    #[test]
    fn measure_and_layout_events_reach_scripts() {
        let mut ext = LuaExtension::from_source(
            "measure",
            r#"
                seen = nil
                function view(env)
                  local plain = env.measure_text("hello world", { size = 14 })
                  local node = env.measure_text(text("hello world", { size = 14 }))
                  local rich = env.measure_text({ "hello ", { "world", bold = true } }, { size = 14 })
                  local narrow = env.measure_text("hello world", { size = 14 }, plain.width / 2)
                  assert(plain.width > 0 and plain.lines == 1)
                  assert(node.width == plain.width and node.height == plain.height)
                  assert(rich.width > 0)
                  assert(narrow.lines > 1 and narrow.width <= plain.width / 2 + 0.5)
                  return column {
                    row { key = "panel", width = plain.width, height = 20, on_layout = "panel" },
                  }
                end
                function on_event(ev)
                  if ev.event == "layout" then seen = ev end
                end
            "#,
        )
        .unwrap();
        let mut core = Core::new();
        frame(&mut core, &mut ext);
        let evs = core.take_pending_events();
        assert_eq!(evs.len(), 1, "one layout event on first sight");
        for ev in &evs {
            ext.on_event(ev);
        }
        let seen: Table = ext.lua.globals().get("seen").unwrap();
        assert_eq!(seen.get::<String>("tag").unwrap(), "panel");
        let layout: Table = seen.get("layout").unwrap();
        assert_eq!(layout.get::<f64>("h").unwrap(), 20.0);
        assert!(layout.get::<f64>("w").unwrap() > 0.0);
        assert_eq!(seen.get::<i64>("node_key").unwrap(), evs[0].key.0 as i64);
        // Unchanged next frame: silence.
        frame(&mut core, &mut ext);
        assert!(core.take_pending_events().is_empty());
    }

    /// Window requests from a script queue like a reveal — against the
    /// frame being built, drained by the driver after it — in call order,
    /// and once; a headless core keeps them and nothing else changes.
    #[test]
    fn scripts_queue_window_size_and_focus_requests() {
        let mut ext = LuaExtension::from_source(
            "win",
            r#"
                function view(env)
                  env.set_window_size(env.window.id, 640, 480)
                  env.focus_window(env.window.id)
                  return column { width = "grow", height = "grow" }
                end
            "#,
        )
        .unwrap();
        let mut core = Core::new();
        let mut ui = core.frame(Size::new(400.0, 200.0), 1.0);
        ui.set_origin(OriginId(1));
        ext.view(&Instance::root(), &mut ui).unwrap();
        ui.finish();
        assert_eq!(
            core.take_window_commands(),
            vec![
                kui_core::WindowCommand::SetSize {
                    window: WindowId::MAIN,
                    size: Size::new(640.0, 480.0),
                },
                kui_core::WindowCommand::Focus(WindowId::MAIN),
            ]
        );
        assert!(core.take_window_commands().is_empty());
        assert_eq!(core.viewport(), Size::new(400.0, 200.0));
    }

    /// A drag a script holds scrolls a container at its edge:
    /// `env.edge_scroll` with the pointer in the edge zone inside the
    /// container's own `scroll_geometry` moves it and returns how far;
    /// in the middle it moves nothing and returns nil.
    #[test]
    fn a_script_scrolls_a_container_at_its_edge() {
        let mut ext = LuaExtension::from_source(
            "edge",
            r#"
                drag, moved = nil, nil
                function view(env)
                  local list = { key = "list", width = "grow", height = 200,
                                 scroll_y = true }
                  for i = 0, 19 do
                    list[#list + 1] = row { width = "grow", height = 30 }
                  end
                  moved = nil
                  local g = env.scroll_geometry(list_key)
                  if drag and g then
                    moved = env.edge_scroll(list_key, drag[1], drag[2], g)
                  end
                  return column(list)
                end
            "#,
        )
        .unwrap();
        let list = Key::ROOT.str("list");
        ext.lua.globals().set("list_key", list.0 as i64).unwrap();
        let mut core = Core::new();
        let frame = |core: &mut Core, ext: &mut LuaExtension| {
            let mut ui = core.frame(Size::new(400.0, 400.0), 1.0);
            ui.set_origin(OriginId(1));
            ext.view(&Instance::root(), &mut ui).unwrap();
            ui.finish();
        };
        frame(&mut core, &mut ext);
        let at = |ext: &LuaExtension, y: f64| {
            let t = ext.lua.create_table().unwrap();
            t.set(1, 10.0).unwrap();
            t.set(2, y).unwrap();
            ext.lua.globals().set("drag", t).unwrap();
        };
        at(&ext, 100.0);
        frame(&mut core, &mut ext);
        assert!(
            ext.lua
                .globals()
                .get::<Option<Table>>("moved")
                .unwrap()
                .is_none()
        );
        at(&ext, 199.0);
        frame(&mut core, &mut ext);
        let moved: Table = ext.lua.globals().get("moved").unwrap();
        assert!(moved.get::<f32>("y").unwrap() > 0.0);
        frame(&mut core, &mut ext);
        assert!(core.scroll_offset(list).y > 0.0);
    }

    /// Scrolling from a script: `env.reveal` scrolls a row into view against
    /// the frame the script is building, and `env.set_scroll` /
    /// `env.scroll_offset` write and read the retained offset. Keys are the
    /// integers events carry, so a script reveals the row it got an
    /// `on_click` from.
    #[test]
    fn scripts_reveal_and_move_scroll_offsets() {
        let mut ext = LuaExtension::from_source(
            "scroll",
            r#"
                rows, want, jump, seen = 20, nil, nil, nil
                function view(env)
                  if want then env.reveal(want) end
                  if jump then env.set_scroll(jump[1], jump[2], jump[3]) end
                  want, jump = nil, nil
                  local list = { key = "list", width = "grow", height = "grow",
                                 scroll_y = true }
                  for i = 0, rows - 1 do
                    list[#list + 1] = row { key = "row" .. i, width = "grow",
                                            height = 30, bg = 0x282840ff }
                  end
                  seen = env.scroll_offset(list_key)
                  return column(list)
                end
            "#,
        )
        .unwrap();
        // 20 rows of 30 in a 200-tall window: 400 of overflow.
        let list = Key::ROOT.str("list");
        let row = |i: usize| list.str(&format!("row{i}"));
        ext.lua.globals().set("list_key", list.0 as i64).unwrap();
        let mut core = Core::new();
        let frame = |core: &mut Core, ext: &mut LuaExtension| {
            let mut ui = core.frame(Size::new(400.0, 200.0), 1.0);
            ui.set_origin(OriginId(1));
            ext.view(&Instance::root(), &mut ui).unwrap();
            ui.finish();
        };

        frame(&mut core, &mut ext);
        assert_eq!(core.scroll_offset(list), Vec2::ZERO);

        // A reveal made while the frame is being built resolves against that
        // same frame — the script does not have to wait a frame to see it.
        ext.lua.globals().set("want", row(15).0 as i64).unwrap();
        frame(&mut core, &mut ext);
        let after = core.scroll_offset(list);
        assert!(after.y > 0.0, "reveal moved nothing: {after:?}");
        assert!(after.y <= 15.0 * 30.0, "scrolled past row 15: {after:?}");
        // Spent: the next frame does not drift.
        frame(&mut core, &mut ext);
        assert_eq!(core.scroll_offset(list), after);

        // The script reads the offset back (as of the last layout).
        let seen: Table = ext.lua.globals().get("seen").unwrap();
        assert_eq!(seen.get::<f32>("y").unwrap(), after.y);
        assert_eq!(seen.get::<f32>("x").unwrap(), 0.0);

        // set_scroll jumps, and the layout clamps: "to the end", then home.
        let jump = |ext: &LuaExtension, y: f64| {
            let t = ext.lua.create_table().unwrap();
            t.set(1, list.0 as i64).unwrap();
            t.set(2, 0.0).unwrap();
            t.set(3, y).unwrap();
            ext.lua.globals().set("jump", t).unwrap();
        };
        jump(&ext, 1e9);
        frame(&mut core, &mut ext);
        assert_eq!(core.scroll_offset(list).y, 400.0);
        jump(&ext, -1e9);
        frame(&mut core, &mut ext);
        assert_eq!(core.scroll_offset(list), Vec2::ZERO);

        // A key the frame does not declare is a no-op, and is not kept.
        jump(&ext, 120.0);
        frame(&mut core, &mut ext);
        ext.lua
            .globals()
            .set("want", Key::ROOT.str("ghost").0 as i64)
            .unwrap();
        frame(&mut core, &mut ext);
        assert_eq!(core.scroll_offset(list).y, 120.0);
        frame(&mut core, &mut ext);
        assert_eq!(core.scroll_offset(list).y, 120.0);
    }

    /// `env.is_pressed(key)` completes the interaction trio next to
    /// `is_hovered` / `is_focused`: held down means the press landed on
    /// this node and the pointer is still on it.
    #[test]
    fn scripts_read_the_pressed_state() {
        let mut ext = LuaExtension::from_source(
            "press",
            r#"
                function view(env)
                  pressed = env.is_pressed(btn_key)
                  hovered = env.is_hovered(btn_key)
                  return column { key = "root", pad = 10,
                    row { key = "btn", width = 100, height = 40,
                          bg = 0x333333ff, on_click = "hit" },
                  }
                end
            "#,
        )
        .unwrap();
        let btn = Key::ROOT.str("root").str("btn");
        ext.lua.globals().set("btn_key", btn.0 as i64).unwrap();
        let mut core = Core::new();
        let pressed = |ext: &LuaExtension| ext.lua.globals().get::<bool>("pressed").unwrap();
        let hovered = |ext: &LuaExtension| ext.lua.globals().get::<bool>("hovered").unwrap();

        frame(&mut core, &mut ext);
        assert!(!pressed(&ext), "idle");

        // Hover alone is not a press.
        core.handle_input(InputEvent::CursorMoved(Vec2::new(50.0, 30.0)));
        frame(&mut core, &mut ext);
        assert!(hovered(&ext) && !pressed(&ext), "hover is not press");

        core.handle_input(InputEvent::MouseDown {
            button: kui_core::MouseButton::Primary,
            clicks: 1,
        });
        frame(&mut core, &mut ext);
        assert!(pressed(&ext), "held down");

        // The press is stuck to the node it started on, so wandering off a
        // node with no drag releases the pressed look while the button is
        // still down; the release clears it either way.
        core.handle_input(InputEvent::MouseUp {
            button: kui_core::MouseButton::Primary,
        });
        frame(&mut core, &mut ext);
        assert!(!pressed(&ext), "released");
    }

    /// A script declares a table of cells by its roles (ADR 0070): the
    /// cells are one Tab stop the arrows walk in two dimensions.
    #[test]
    fn a_scripts_grid_is_walked_by_rows_and_cells() {
        let mut ext = LuaExtension::from_source(
            "grid",
            r#"
                function view(env)
                  local rows = {}
                  for r = 0, 1 do
                    local cells = { key = "r" .. r, role = "row" }
                    for c = 0, 1 do
                      cells[#cells + 1] = row { key = "c" .. c, width = 40, height = 20,
                        role = "cell", focusable = true, label = r .. "," .. c }
                    end
                    rows[#rows + 1] = row(cells)
                  end
                  rows.key, rows.role, rows.label = "grid", "table", "table"
                  return column { key = "root", column(rows) }
                end
            "#,
        )
        .unwrap();
        let mut core = Core::new();
        frame(&mut core, &mut ext);
        let cell = |r: u32, c: u32| {
            Key::ROOT
                .str("root")
                .str("grid")
                .str(&format!("r{r}"))
                .str(&format!("c{c}"))
        };
        core.handle_input(InputEvent::Key(kui_core::EditKey::Tab, Default::default()));
        assert_eq!(core.focus(), Some(cell(0, 0)));
        core.handle_input(InputEvent::Key(kui_core::EditKey::Down, Default::default()));
        core.handle_input(InputEvent::Key(kui_core::EditKey::Right, Default::default()));
        assert_eq!(core.focus(), Some(cell(1, 1)));
    }

    /// `env.is_group_hovered(name)` / `env.is_group_pressed(name)` read a
    /// hover group by the name its members declare (backlog C92): a press
    /// on one member is the group's.
    #[test]
    fn scripts_read_a_hover_groups_state_by_name() {
        let mut ext = LuaExtension::from_source(
            "group",
            r#"
                function view(env)
                  g_hovered = env.is_group_hovered("knob")
                  g_pressed = env.is_group_pressed("knob")
                  return column { key = "root", pad = 10,
                    row { key = "track", width = 100, height = 20, hover_group = "knob",
                          hover_bg = 0x333333ff, on_click = "t" },
                    row { key = "handle", width = 20, height = 20, hover_group = "knob",
                          hover_bg = 0x444444ff },
                  }
                end
            "#,
        )
        .unwrap();
        let mut core = Core::new();
        let get = |ext: &LuaExtension, k: &str| ext.lua.globals().get::<bool>(k).unwrap();
        frame(&mut core, &mut ext);
        assert!(!get(&ext, "g_hovered") && !get(&ext, "g_pressed"));
        core.handle_input(InputEvent::CursorMoved(Vec2::new(20.0, 20.0)));
        frame(&mut core, &mut ext);
        assert!(get(&ext, "g_hovered") && !get(&ext, "g_pressed"));
        core.handle_input(InputEvent::MouseDown {
            button: kui_core::MouseButton::Primary,
            clicks: 1,
        });
        frame(&mut core, &mut ext);
        assert!(get(&ext, "g_pressed"), "a member's press is the group's");
        core.handle_input(InputEvent::MouseUp {
            button: kui_core::MouseButton::Primary,
        });
        frame(&mut core, &mut ext);
        assert!(!get(&ext, "g_pressed"));
    }

    /// `env.is_drop_target(key)` and `env.drop_target()` beside the trio
    /// (ADR 0031): a script that shows an insertion mark while files
    /// hover reads them; the colour alone is `drop_bg`, resolved in the
    /// core.
    #[test]
    fn scripts_read_the_drop_target() {
        let mut ext = LuaExtension::from_source(
            "drop",
            r#"
                function view(env)
                  over = env.is_drop_target(zone_key)
                  target = env.drop_target()
                  return column { key = "root", pad = 10,
                    row { key = "zone", width = 100, height = 40,
                          bg = 0x333333ff, drop_bg = 0x335533ff, on_drop = "files" },
                  }
                end
            "#,
        )
        .unwrap();
        let zone = Key::ROOT.str("root").str("zone");
        ext.lua.globals().set("zone_key", zone.0 as i64).unwrap();
        let mut core = Core::new();
        let over = |ext: &LuaExtension| ext.lua.globals().get::<bool>("over").unwrap();
        let target = |ext: &LuaExtension| ext.lua.globals().get::<Option<i64>>("target").unwrap();

        frame(&mut core, &mut ext);
        assert!(!over(&ext) && target(&ext).is_none(), "idle");

        let paths = vec!["/drop/1.txt".to_string()];
        let evs = core.handle_input(InputEvent::DragFiles {
            paths: paths.clone(),
            at: Vec2::new(50.0, 30.0),
        });
        assert_eq!(evs[0].payload.get_str("tag"), Some("files"));
        frame(&mut core, &mut ext);
        assert!(over(&ext), "the files are over the zone");
        assert_eq!(target(&ext), Some(zone.0 as i64));
        let (dl, _) = core.output();
        let lit = dl
            .quads
            .iter()
            .find(|q| q.rect.w == 100.0)
            .map(|q| q.color)
            .expect("zone quad");
        assert!(
            (lit.g - 0x55 as f32 / 255.0).abs() < 0.01,
            "drop_bg painted"
        );

        core.handle_input(InputEvent::DropFiles {
            paths,
            at: Vec2::new(50.0, 30.0),
        });
        frame(&mut core, &mut ext);
        assert!(
            !over(&ext) && target(&ext).is_none(),
            "a drop ends the hover"
        );
    }

    /// The focus verbs: `env.set_focus(key)` moves focus now,
    /// `env.focus_next` / `env.focus_prev` walk the Tab ring, `env.blur`
    /// drops it — and `env.focus` reads back the node's key, which is a
    /// different fact from `env.focused`, the window's.
    /// The table `view(env)` gets is the documented env reading and
    /// nothing else: its value keys are `schema::ENV_FIELDS`'s Lua
    /// spellings, key for key, under an env with every optional fact
    /// present. The queries and verbs beside them are pinned to the verb
    /// table's Lua column (`schema::DOORS`), so adding one is a row there
    /// with its C and Node cells beside it.
    #[test]
    fn the_env_table_is_the_documented_env_shape() {
        let mut ext = LuaExtension::from_source(
            "env",
            r#"
                function view(env)
                  values, calls = {}, {}
                  for k, v in pairs(env) do
                    if type(v) == "function" then calls[#calls + 1] = k
                    elseif type(v) == "table" then
                      for wk in pairs(v) do values[#values + 1] = k .. "." .. wk end
                    else values[#values + 1] = k end
                  end
                  return column { key = "root",
                    row { key = "a", focusable = true, width = 50, height = 20 },
                    column { key = "dock", focus_region = true,
                      row { key = "d", focusable = true, width = 50, height = 20 },
                    },
                  }
                end
            "#,
        )
        .unwrap();
        let mut core = Core::new();
        // Every key that only appears when set: a rate, an accent, a
        // locale, a controls rect, and a focused node (which the ring has
        // to see a frame first) — one inside a region, so the region in
        // effect is a reading too.
        core.env.refresh_hz = Some(60.0);
        core.env.system.accent = Some(kui_core::Color::hex(0x3b82f6ff));
        core.env.system.locale = kui_core::Locale::new("en-US");
        core.env.window.native_controls = Some(Rect::new(0.0, 0.0, 78.0, 28.0));
        frame(&mut core, &mut ext);
        core.set_focus(Some(Key::ROOT.str("root").str("dock").str("d")));
        frame(&mut core, &mut ext);

        let sorted = |name: &str| -> Vec<String> {
            let mut v: Vec<String> = ext.lua.globals().get(name).unwrap();
            v.sort();
            v
        };
        let mut documented: Vec<String> = kui_core::schema::ENV_FIELDS
            .iter()
            .flat_map(|f| f.lua.iter().map(|k| (*k).to_string()))
            .collect();
        // The palette rides in the same reading and is pinned the same
        // way, to `schema::THEME_ROLES` rather than to `ENV_FIELDS` — it
        // is derived from `system` and not reported by the host, so it is
        // not an `Env` field (ADR 0019). The two flat keys beside the
        // roles are the base it came from and the disabled multiplier.
        documented.extend(
            kui_core::schema::THEME_ROLES
                .iter()
                .map(|r| format!("theme.{}", r.name)),
        );
        documented.push("theme.appearance".into());
        documented.push("theme.disabled_opacity".into());
        // And the metrics beside it (backlog T2), pinned to
        // `schema::METRIC_ROLES` the same way.
        documented.extend(
            kui_core::schema::METRIC_ROLES
                .iter()
                .map(|r| format!("metrics.{}", r.name)),
        );
        // And the tokens (ADR 0027, 0049): three tables, the app's own
        // names under them, so the keys pinned here are the three kinds.
        documented.push("tokens.colors".into());
        documented.push("tokens.families".into());
        documented.push("tokens.lengths".into());
        documented.sort();
        assert_eq!(
            sorted("values"),
            documented,
            "env's value keys and schema::ENV_FIELDS + THEME_ROLES + METRIC_ROLES + tokens disagree"
        );
        // The queries and verbs are the verb table's Lua column, exactly
        // (`schema::DOORS`, backlog B1a): a function added to `env` is a
        // row there with its three other cells, and a row's Lua spelling
        // is a function here. The table carries the reasons for the
        // rows Lua has no door for — a guest's env is a reading, not a
        // handle on the host — so this test need not restate them.
        let mut doors: Vec<String> = kui_core::schema::DOORS
            .iter()
            .filter_map(|d| match d.lua {
                kui_core::schema::Cell::Is(name) => Some(name.to_string()),
                _ => None,
            })
            .collect();
        doors.sort();
        assert_eq!(
            sorted("calls"),
            doors,
            "env's queries and verbs and schema::DOORS's Lua column disagree; update env_table's doc too"
        );
    }

    /// `env.label_of` is the way back from a key (ADR 0047): the integer
    /// key an event carried answers the label the view opened it under —
    /// here from the last frame, since a script's table is lowered after
    /// `view` returns — and `key_of` of that label is the key again. A key
    /// nothing declared answers nil.
    #[test]
    fn env_label_of_turns_a_key_back_into_its_label() {
        let mut ext = LuaExtension::from_source(
            "labels",
            r#"
                function view(env)
                  local root = column { key = "root", row { key = "item", width = 50, height = 20 } }
                  if probe then
                    back = env.label_of(probe)
                    unknown = env.label_of(0x1234)
                  end
                  return root
                end
            "#,
        )
        .unwrap();
        let mut core = Core::new();
        frame(&mut core, &mut ext);
        let item = core.key_of("item").expect("the view declared it");
        ext.lua.globals().set("probe", item.0 as i64).unwrap();
        frame(&mut core, &mut ext);
        let back: Option<String> = ext.lua.globals().get("back").unwrap();
        assert_eq!(back.as_deref(), Some("item"));
        assert_eq!(
            core.key_of(&back.unwrap()),
            Some(item),
            "key_of(label_of(k)) is k"
        );
        let unknown: Option<String> = ext.lua.globals().get("unknown").unwrap();
        assert_eq!(unknown, None, "an unknown key answers nil");
    }

    /// ADR 0049 in Lua: `$disabled_opacity` dims a box by the theme's
    /// number, a family token picks the first name of its list that
    /// resolves and `family = "$code"` shapes in it, and `env.tokens
    /// .families` reads the pick back.
    #[test]
    fn a_script_dims_by_the_role_and_names_a_family_token() {
        let mut ext = LuaExtension::from_source(
            "families",
            r##"
                tokens = {
                  families = { code = { "kui-no-such-family-2026", "mono" }, gone = "kui-no-such-family-2026" },
                }
                function view(env)
                  seen = env.tokens.families
                  code_w = env.measure_text("iiiiiiii", { size = 14, family = "$code" }).width
                  mono_w = env.measure_text("iiiiiiii", { size = 14, family = "mono" }).width
                  sans_w = env.measure_text("iiiiiiii", { size = 14 }).width
                  return column {
                    row { width = 20, height = 20, bg = 0xff0000ff, opacity = "$disabled_opacity" },
                    text("x", { family = "$code" }),
                  }
                end
            "##,
        )
        .unwrap();
        let mut core = Core::new();
        frame(&mut core, &mut ext);
        let codes: Vec<_> = core.take_warnings().iter().map(|w| w.code).collect();
        assert_eq!(
            codes,
            [kui_core::diag::UNKNOWN_FAMILY],
            "the list nothing matched"
        );
        let seen: Table = ext.lua.globals().get("seen").unwrap();
        assert_eq!(seen.get::<String>("code").unwrap(), "mono");
        assert_eq!(seen.get::<String>("gone").unwrap(), "sans");
        let g = ext.lua.globals();
        let (code_w, mono_w, sans_w): (f32, f32, f32) = (
            g.get("code_w").unwrap(),
            g.get("mono_w").unwrap(),
            g.get("sans_w").unwrap(),
        );
        assert_eq!(code_w, mono_w);
        assert_ne!(mono_w, sans_w);
        let dim = core.theme().disabled_opacity;
        let dl = core.output().0;
        let red = dl
            .quads
            .iter()
            .find(|q| q.kind == kui_core::QuadKind::Solid && q.color.r == 1.0)
            .expect("the dimmed box");
        assert!(
            (red.color.a - dim).abs() < 1e-3,
            "{} against {dim}",
            red.color.a
        );
    }

    /// An empty family list is a token that is always sans, as in every
    /// binding, with `unknown-family` listing `[]`; a table of only named
    /// keys or a list holding a number is still refused. And a family
    /// token takes a colour's name, declared after it, so `bg = "$code"`
    /// paints nothing and warns `unknown-token` — what Node's encoder
    /// mirrors.
    #[test]
    fn an_empty_family_list_is_sans_and_a_family_takes_a_colour_s_name() {
        let mut ext = LuaExtension::from_source(
            "empty-family",
            r##"
                tokens = {
                  colors = { code = "#ff0000" },
                  families = { none = {}, code = "mono" },
                }
                function view(env)
                  seen = env.tokens
                  none_w = env.measure_text("iiiiiiii", { size = 14, family = "$none" }).width
                  sans_w = env.measure_text("iiiiiiii", { size = 14 }).width
                  return column { row { width = 20, height = 20, bg = "$code" } }
                end
            "##,
        )
        .unwrap();
        let mut core = Core::new();
        frame(&mut core, &mut ext);
        let ws = core.take_warnings();
        let mut codes: Vec<_> = ws.iter().map(|w| w.code).collect();
        codes.sort();
        assert_eq!(
            codes,
            [
                kui_core::diag::UNKNOWN_FAMILY,
                kui_core::diag::UNKNOWN_TOKEN
            ],
            "{ws:?}"
        );
        let family = ws
            .iter()
            .find(|w| w.code == kui_core::diag::UNKNOWN_FAMILY)
            .unwrap();
        assert!(
            family.message.contains("`$none` lists []"),
            "{}",
            family.message
        );
        let seen: Table = ext.lua.globals().get("seen").unwrap();
        let families: Table = seen.get("families").unwrap();
        assert_eq!(families.get::<String>("none").unwrap(), "sans");
        assert_eq!(families.get::<String>("code").unwrap(), "mono");
        let colors: Table = seen.get("colors").unwrap();
        assert_eq!(colors.get::<Option<u32>>("code").unwrap(), None);
        let g = ext.lua.globals();
        assert_eq!(
            g.get::<f32>("none_w").unwrap(),
            g.get::<f32>("sans_w").unwrap()
        );
        assert!(
            !core
                .output()
                .0
                .quads
                .iter()
                .any(|q| q.kind == kui_core::QuadKind::Solid && q.color.r == 1.0),
            "nothing painted from the colour the family took the name of"
        );
        let lua = mlua::Lua::new();
        for bad in ["{ code = { a = 1 } }", "{ code = { 3 } }", "{ code = 3 }"] {
            let t: Table = lua
                .load(format!("return {{ families = {bad} }}"))
                .eval()
                .unwrap();
            let e = parse_tokens(&t).unwrap_err().to_string();
            assert!(
                e.contains("a family is a name or a list of names"),
                "{bad}: {e}"
            );
        }
    }

    /// Tokens (ADR 0027): the `tokens` global is declared under the
    /// script's origin, a `$name` resolves in a colour, a length, a sizing,
    /// a pad edge, a border and a span, a themed colour follows the
    /// appearance, and `env.tokens` reads the frame's values back.
    #[test]
    fn a_script_declares_tokens_and_references_them_by_name() {
        let mut ext = LuaExtension::from_source(
            "tokens",
            r##"
                tokens = {
                  colors = { peach = "#ffcc99", ink = { light = "#111111", dark = "#eeeeee" } },
                  lengths = { side_w = 132, gap = 6 },
                }
                function view(env)
                  seen = env.tokens
                  return column { pad = "$gap", gap = "$gap",
                    row { key = "a", width = "$side_w", height = 20, bg = "$peach",
                          border = { w = "$gap", color = "$ink" }, radius = "$radius" },
                    row { key = "b", width = 20, height = "$side_w", bg = "$surface",
                          pad = { l = "$gap", r = 2 } },
                    text({ "x", { "y", color = "$peach", bg = "$ink" } }, { size = "$gap", color = "$peach" }),
                  }
                end
            "##,
        )
        .unwrap();
        let mut core = Core::new();
        frame(&mut core, &mut ext);
        assert!(core.take_warnings().is_empty());
        let peach = Color::hex(0xffcc99ff);
        let paper = Color::hex(0xeeeeeeff);
        let radius = core.metrics().radius;
        let surface = core.theme().surface;
        let dl = core.output().0;
        let a = dl
            .quads
            .iter()
            .find(|q| q.color == peach && q.kind == kui_core::QuadKind::Solid)
            .expect("the peach box");
        assert_eq!(a.rect.w, 132.0, "width from a length token");
        assert_eq!(a.border_w, 6.0);
        assert_eq!(
            a.border_color, paper,
            "the dark half on an unknown appearance"
        );
        assert_eq!(a.radius[0], radius, "$radius is the metric");
        let b = dl
            .quads
            .iter()
            .find(|q| q.color == surface && q.rect.h == 132.0)
            .expect("the $surface box, 132 tall");
        assert_eq!(b.rect.w, 20.0);
        let seen: Table = ext.lua.globals().get("seen").unwrap();
        let colors: Table = seen.get("colors").unwrap();
        let lengths: Table = seen.get("lengths").unwrap();
        assert_eq!(colors.get::<u32>("peach").unwrap(), 0xffcc99ff);
        assert_eq!(colors.get::<u32>("ink").unwrap(), 0xeeeeeeff);
        assert_eq!(lengths.get::<f32>("side_w").unwrap(), 132.0);
        // The light half, without the script changing.
        core.set_system(kui_core::SystemEnv {
            appearance: kui_core::Appearance::Light,
            ..Default::default()
        });
        frame(&mut core, &mut ext);
        let a = core
            .output()
            .0
            .quads
            .iter()
            .find(|q| q.color == peach && q.kind == kui_core::QuadKind::Solid)
            .unwrap();
        assert_eq!(a.border_color, Color::hex(0x111111ff));
    }

    /// Derived tokens (ADR 0028): a recipe over an earlier token, its ops
    /// tuples in the array part. Lua sorts its names, so `a_deep` (from
    /// `z_lit`) is declared after its source by dependency and not by
    /// position; `bad`'s missing source is the core's `unknown-token`; a
    /// bare tuple is one op; `env.tokens` reads the derived value back.
    #[test]
    fn a_script_derives_a_token_from_another() {
        let mut ext = LuaExtension::from_source(
            "derived",
            r##"
                tokens = {
                  colors = {
                    peach = "#ffcc99",
                    z_lit = { from = "peach", ops = { "lift", 0.5 } },
                    a_deep = { from = "z_lit", ops = { { "alpha", 0.5 }, { "mix", "peach", 0.0 } } },
                    up = { from = "surface", ops = { { "raise", 0.25 } } },
                    bad = { from = "nothing" },
                  },
                }
                function view(env)
                  seen = env.tokens.colors
                  return column {
                    row { key = "a", width = 20, height = 20, bg = "$a_deep" },
                    row { key = "b", width = 20, height = 20, bg = "$bad" },
                  }
                end
            "##,
        )
        .unwrap();
        let mut core = Core::new();
        frame(&mut core, &mut ext);
        let warnings = core.take_warnings();
        let codes: Vec<&str> = warnings.iter().map(|w| w.code).collect();
        assert_eq!(codes, ["unknown-token"], "{warnings:?}");
        assert!(
            warnings[0].message.contains("`$bad`") && warnings[0].message.contains("`$nothing`")
        );
        let seen: Table = ext.lua.globals().get("seen").unwrap();
        assert_eq!(seen.get::<u32>("z_lit").unwrap(), 0xffe6ccff);
        assert_eq!(seen.get::<u32>("a_deep").unwrap(), 0xffe6cc80);
        let surface = core.theme().surface;
        assert_eq!(
            seen.get::<u32>("up").unwrap(),
            surface.mix(Color::WHITE, 0.25).to_hex(),
            "a role source, raised toward the dark base's front"
        );
        assert!(seen.get::<mlua::Value>("bad").unwrap().is_nil());
        let dl = core.output().0;
        assert!(
            dl.quads.iter().any(|q| q.color.to_hex() == 0xffe6cc80),
            "$a_deep painted the derived colour"
        );

        // The parser refuses what the core could not name.
        let refused = |colors: &str| {
            LuaExtension::from_source(
                "bad",
                &format!(
                    "tokens = {{ colors = {{ {colors} }} }}\nfunction view() return column {{}} end"
                ),
            )
            .err()
            .map(|e| e.to_string())
            .unwrap_or_default()
        };
        assert!(
            refused(r#"x = { from = "peach", ops = { { "glow", 1 } } }"#)
                .contains("unknown verb \"glow\"")
        );
        assert!(
            refused(r#"x = { from = "peach", ops = { { "mix", 0.5 } } }"#)
                .contains("mix takes a colour and a number")
        );
        assert!(
            refused(r#"x = { from = "peach", ops = { { "lift", "lots" } } }"#)
                .contains("number is a number")
        );
        assert!(refused(r#"x = { from = 3 }"#).contains("from names a colour token or role"));
        assert!(refused(r#"x = { from = "peach", glow = 1 }"#).contains("unknown key \"glow\""));
    }

    /// A script's table is its own: its `$peach` is the host's until it
    /// declares one, its `grey` never reaches the host, and
    /// `env.set_tokens` from inside a view replaces the script's table for
    /// the nodes after the call.
    #[test]
    fn a_scripts_tokens_sit_over_the_hosts() {
        let mut ext = LuaExtension::from_source(
            "guest",
            r##"
                tokens = { colors = { grey = "#808080" } }
                function view(env)
                  first = { peach = env.tokens.colors.peach, grey = env.tokens.colors.grey }
                  if redeclare then
                    env.set_tokens { colors = { peach = "#ffaa77" }, lengths = { w = 40 } }
                  end
                  second = { peach = env.tokens.colors.peach, grey = env.tokens.colors.grey }
                  return column { row { key = "a", width = redeclare and "$w" or 10, height = 10, bg = "$peach" } }
                end
            "##,
        )
        .unwrap();
        let mut core = Core::new();
        core.set_tokens(kui_core::Tokens::new().color("peach", Color::hex(0xffcc99ff)));
        frame(&mut core, &mut ext);
        let first: Table = ext.lua.globals().get("first").unwrap();
        assert_eq!(
            first.get::<u32>("peach").unwrap(),
            0xffcc99ff,
            "the host's peach"
        );
        assert_eq!(
            first.get::<u32>("grey").unwrap(),
            0x808080ff,
            "its own grey"
        );
        assert!(
            core.token_lookup().color("grey").is_err(),
            "the guest's grey is not the host's"
        );
        assert!(core.take_warnings().is_empty());

        ext.lua.globals().set("redeclare", true).unwrap();
        frame(&mut core, &mut ext);
        let second: Table = ext.lua.globals().get("second").unwrap();
        assert_eq!(
            second.get::<u32>("peach").unwrap(),
            0xffaa77ff,
            "its own peach now"
        );
        assert!(
            second.get::<Option<u32>>("grey").unwrap().is_none(),
            "replaced whole"
        );
        let a = core
            .output()
            .0
            .quads
            .iter()
            .find(|q| q.color == Color::hex(0xffaa77ff))
            .expect("painted the script's peach");
        assert_eq!(a.rect.w, 40.0);
        assert_eq!(
            core.token_lookup().color("peach"),
            Ok(Color::hex(0xffcc99ff)),
            "the host's table did not move"
        );
    }

    /// A `$name` nothing declared warns once and leaves the slot at its
    /// default; a declared role name warns at the declaration.
    #[test]
    fn a_missing_token_warns_and_paints_nothing() {
        let mut ext = LuaExtension::from_source(
            "missing",
            r##"
                tokens = { colors = { surface = "#ff0000", peach = "#ffcc99" }, lengths = { gap = 6 } }
                function view(env)
                  return column {
                    row { key = "a", width = "$gap", height = 10, bg = "$peech", pad = "$peach" },
                    row { key = "b", width = "$peach", height = 10, bg = "$gap" },
                    text("still here", { color = "$peech", size = "$gap" }),
                    text("still here", { size = 6 }),
                    text("still here"),
                  }
                end
            "##,
        )
        .unwrap();
        let mut core = Core::new();
        core.set_inspect(true);
        frame(&mut core, &mut ext);
        frame(&mut core, &mut ext);
        let ws = core.take_warnings();
        let mut codes: Vec<&str> = ws.iter().map(|w| w.code).collect();
        codes.sort();
        assert_eq!(
            codes,
            [
                kui_core::diag::RESERVED_TOKEN,
                kui_core::diag::UNKNOWN_TOKEN,
                kui_core::diag::UNKNOWN_TOKEN,
                kui_core::diag::UNKNOWN_TOKEN,
            ],
            "surface refused once; peech, peach-as-length and gap-as-colour once each: {ws:?}"
        );
        assert!(
            ws.iter().any(|w| w
                .message
                .contains("`$gap` is a length token, and this slot takes a color")),
            "{ws:?}"
        );
        // No solid quad was painted: both bgs were left out. The text with
        // the mistyped colour is still there, in the theme's foreground and
        // at the declared size — a typo hides nothing.
        let fg = core.theme().fg;
        let dl = core.output().0;
        assert!(
            dl.quads
                .iter()
                .all(|q| q.kind != kui_core::QuadKind::Solid || q.color.a == 0.0)
        );
        let glyphs: Vec<_> = dl
            .quads
            .iter()
            .filter(|q| {
                matches!(
                    q.kind,
                    kui_core::QuadKind::GlyphMask | kui_core::QuadKind::GlyphSubpixel
                )
            })
            .collect();
        assert!(!glyphs.is_empty(), "the text painted");
        assert!(glyphs.iter().all(|q| q.color == fg), "in the foreground");
        // At the declared 6 px size, not 0 and not the default: the same
        // text laid out with a literal `size = 6` is exactly as tall, and
        // one at the default size is taller. Compared to a control rather
        // than to a number, since a glyph's height at 6 px is the font's.
        let heights: Vec<f32> = core
            .nodes()
            .iter()
            .filter(|n| n.text.as_deref() == Some("still here"))
            .map(|n| n.rect.h)
            .collect();
        assert_eq!(heights.len(), 3, "{heights:?}");
        assert!(heights[0] > 0.0, "{heights:?}");
        assert_eq!(heights[0], heights[1], "$gap is the literal 6: {heights:?}");
        assert!(heights[2] > heights[0], "and not the default: {heights:?}");
    }

    /// A gradient stop whose `$name` misses is left out, as a miss leaves
    /// any slot (backlog RG118, ADR 0042's amendment): one `unknown-token`
    /// and no error. Three stops less one still paint the other two; two
    /// less one have nothing to paint, and the box's `bg` is what shows.
    #[test]
    fn a_gradient_stop_that_misses_is_left_out() {
        let mut ext = LuaExtension::from_source(
            "gradient",
            r##"
                tokens = { colors = { peach = "#ffcc99" } }
                function view(env)
                  return column {
                    row { key = "three", width = 40, height = 10,
                          gradient = { stops = { "$peach", "$peech", "#0000ff" } } },
                    row { key = "two", width = 40, height = 10, bg = "#000000",
                          gradient = { stops = { "$peach", "$peech" } } },
                  }
                end
            "##,
        )
        .unwrap();
        let mut core = Core::new();
        frame(&mut core, &mut ext);
        frame(&mut core, &mut ext);
        let codes: Vec<&str> = core.take_warnings().iter().map(|w| w.code).collect();
        assert_eq!(codes, [kui_core::diag::UNKNOWN_TOKEN], "once, for the name");
        let dl = core.output().0;
        let images: Vec<_> = dl
            .quads
            .iter()
            .filter(|q| q.kind == kui_core::QuadKind::Image)
            .collect();
        assert_eq!(images.len(), 1, "the three-stop row's two");
        assert!(
            images[0].rect.y < 10.0,
            "on the first row: {:?}",
            images[0].rect
        );
        let black = dl
            .quads
            .iter()
            .filter(|q| q.kind == kui_core::QuadKind::Solid && q.color == Color::hex(0x000000ff))
            .count();
        assert_eq!(black, 1, "the second row's bg");
    }

    /// AR14: the three slots a `$name` could not reach, and the stops it
    /// failed the frame from — a `min_width`, a line's `width`, a
    /// keyframe's `bg` and an entrance's `width` — resolve like any prop,
    /// and a miss leaves the slot at its default with one `unknown-token`.
    #[test]
    fn a_token_reaches_a_min_a_stroke_and_a_stop_and_misses_by_leaving_the_slot() {
        let mut ext = LuaExtension::from_source(
            "everywhere",
            r##"
                tokens = { colors = { peach = "#ffcc99" }, lengths = { gap = 6, wide = 40 } }
                function view(env)
                  return column { pad = 4,
                    row { key = "clamp", width = 10, height = 10, min_width = "$wide" },
                    row { key = "typo", width = 10, height = 10, min_width = "$nope" },
                    line { key = "stroke", from = { 0, 0 }, to = { 30, 0 }, width = "$gap", color = "$peach" },
                    row { key = "anim", width = 10, height = 10, transition = 100,
                          keyframes = { { bg = "$peach", width = "$wide" }, { bg = "$peech", radius = "$gap" } },
                          enter = { width = "$nothing", bg = "$peach" } },
                  }
                end
            "##,
        )
        .unwrap();
        let mut core = Core::new();
        core.set_inspect(true);
        frame(&mut core, &mut ext);
        let nodes = core.nodes();
        let by = |label: &str| {
            nodes
                .iter()
                .find(|n| n.label.as_deref() == Some(label))
                .unwrap()
        };
        assert_eq!(by("clamp").rect.w, 40.0, "the clamp is the token's 40 px");
        assert_eq!(
            by("typo").rect.w,
            10.0,
            "a miss leaves the row at its default"
        );
        let dl = core.output().0;
        let seg = dl
            .quads
            .iter()
            .find(|q| q.kind == kui_core::QuadKind::Segment)
            .expect("the line drew");
        assert_eq!(seg.color, Color::hex(0xffcc99ff));
        assert_eq!(seg.border_w, 6.0, "the stroke is the token's width");
        let ws = core.take_warnings();
        let mut names: Vec<String> = ws
            .iter()
            .filter(|w| w.code == kui_core::diag::UNKNOWN_TOKEN)
            .map(|w| w.message.clone())
            .collect();
        names.sort();
        assert_eq!(names.len(), 3, "nope, peech, nothing: {ws:?}");
        assert!(names.iter().any(|m| m.contains("`$nope`")), "{names:?}");
        assert!(names.iter().any(|m| m.contains("`$peech`")), "{names:?}");
        assert!(names.iter().any(|m| m.contains("`$nothing`")), "{names:?}");
    }

    #[test]
    fn scripts_move_focus_with_the_verbs() {
        let mut ext = LuaExtension::from_source(
            "focus",
            r#"
                function view(env)
                  if cmd == "set" then env.set_focus(target)
                  elseif cmd == "blur" then env.blur()
                  elseif cmd == "next" then env.focus_next()
                  elseif cmd == "prev" then env.focus_prev() end
                  cmd = nil
                  -- `env.focus` is a value the host filled in before view()
                  -- ran, so it lags a verb called above by a frame;
                  -- `env.is_focused` is a call and answers about now.
                  seen_focus = env.focus
                  seen_live = env.is_focused(target or 0)
                  seen_window = env.focused
                  return column { key = "root", pad = 10,
                    row { key = "a", focusable = true, width = 50, height = 20 },
                    row { key = "b", focusable = true, width = 50, height = 20 },
                    row { key = "c", focusable = true, width = 50, height = 20 },
                  }
                end
            "#,
        )
        .unwrap();
        let root = Key::ROOT.str("root");
        let (a, b, c) = (root.str("a"), root.str("b"), root.str("c"));
        let mut core = Core::new();
        let cmd = |ext: &LuaExtension, c: &str| ext.lua.globals().set("cmd", c).unwrap();
        let seen = |ext: &LuaExtension| ext.lua.globals().get::<Option<i64>>("seen_focus").unwrap();
        let live = |ext: &LuaExtension| ext.lua.globals().get::<bool>("seen_live").unwrap();

        // The ring is the last finished frame's, so build one first.
        frame(&mut core, &mut ext);
        assert_eq!(core.focus(), None);
        assert_eq!(seen(&ext), None, "nil for no focus, not 0");

        // Straight to a node by key, the imperative form of `key_focus`.
        ext.lua.globals().set("target", b.0 as i64).unwrap();
        cmd(&ext, "set");
        frame(&mut core, &mut ext);
        assert_eq!(core.focus(), Some(b));
        // `env.focus` is a snapshot the host wrote before view() ran, so it
        // still holds what focus was when the frame opened; `env.is_focused`
        // is a query into the live frame and sees the move at once.
        assert_eq!(seen(&ext), None, "the value lags a same-frame verb");
        assert!(live(&ext), "the query does not");
        frame(&mut core, &mut ext);
        assert_eq!(
            seen(&ext),
            Some(b.0 as i64),
            "and the next frame carries it"
        );

        // Tab and Shift-Tab, wrapping.
        cmd(&ext, "next");
        frame(&mut core, &mut ext);
        assert_eq!(core.focus(), Some(c));
        cmd(&ext, "next");
        frame(&mut core, &mut ext);
        assert_eq!(core.focus(), Some(a), "wraps");
        cmd(&ext, "prev");
        frame(&mut core, &mut ext);
        assert_eq!(core.focus(), Some(c), "wraps back");

        cmd(&ext, "blur");
        frame(&mut core, &mut ext);
        assert_eq!(core.focus(), None);
        frame(&mut core, &mut ext);
        assert_eq!(seen(&ext), None);

        // By label: the node's own `key` string, with no event from it
        // first (F5). `c` was never clicked, tabbed to or reported.
        ext.lua.globals().set("target", "c").unwrap();
        cmd(&ext, "set");
        frame(&mut core, &mut ext);
        assert_eq!(core.focus(), Some(c), "set_focus(\"c\") resolves the label");
        assert!(live(&ext), "and is_focused(\"c\") answers about it");
        assert!(core.take_warnings().is_empty());
        // A label nothing declares is an error that names both spellings.
        ext.lua.globals().set("target", "nope").unwrap();
        cmd(&ext, "set");
        let mut ui = core.frame(Size::new(800.0, 600.0), 1.0);
        let e = ext
            .view(&Instance::root(), &mut ui)
            .unwrap_err()
            .to_string();
        ui.finish();
        assert!(
            e.contains("no node is keyed \"nope\"") && e.contains("integer key"),
            "{e}"
        );

        // `env.focused` is the window's focus, not the node's: it stays a
        // bool through all of the above, and follows the host env instead.
        assert!(ext.lua.globals().get::<bool>("seen_window").unwrap());
    }

    /// Two nodes on one label under different parents: the first in tree
    /// order is the one focused, and the frame says so once.
    #[test]
    fn a_shared_label_resolves_to_the_first_and_warns() {
        let mut ext = LuaExtension::from_source(
            "dup",
            r#"
                function view(env)
                  if go then env.set_focus("item"); go = nil end
                  return column { key = "root",
                    row { row { key = "item", focusable = true, width = 50, height = 20 } },
                    row { row { key = "item", focusable = true, width = 50, height = 20 } },
                  }
                end
            "#,
        )
        .unwrap();
        let mut core = Core::new();
        frame(&mut core, &mut ext);
        ext.lua.globals().set("go", true).unwrap();
        frame(&mut core, &mut ext);
        assert_eq!(
            core.focus(),
            Some(Key::ROOT.str("root").index(0).str("item")),
            "the first, under the auto-keyed first row"
        );
        let ws = core.take_warnings();
        assert_eq!(ws.len(), 1, "{ws:?}");
        assert_eq!(ws[0].code, kui_core::diag::AMBIGUOUS_KEY);
        assert!(ws[0].message.contains("\"item\""), "{}", ws[0].message);
    }

    /// A script opens a context menu with `env.open_menu` and hears the
    /// chosen row as a `menu` event on the node it named (ADR 0017): the
    /// node's `on_menu` message with `menu = { role, item }` in it (ADR
    /// 0047).
    #[test]
    fn scripts_open_a_menu_and_hear_the_row() {
        let mut ext = LuaExtension::from_source(
            "menu",
            r#"
                frames = 0
                function view(env)
                  frames = frames + 1
                  if frames == 2 then
                    opened = env.open_menu("card", 40, 30, {
                      { role = "copy" },
                      { role = "separator" },
                      -- The wire spelling the event reports, and the
                      -- snake one a script may have learned first.
                      { role = "selectAll" },
                      { role = "look_up" },
                      { label = "Wrap", checked = true },
                      { label = "Inspect", id = "inspect" },
                    })
                  end
                  return column {
                    column { key = "card", selectable = true, on_menu = { kind = "card" },
                      text("one", { size = 14 }) },
                  }
                end
            "#,
        )
        .unwrap();
        let mut core = Core::new();
        let frame = |core: &mut Core, ext: &mut LuaExtension| {
            let mut ui = core.frame(Size::new(400.0, 200.0), 1.0);
            ui.set_origin(OriginId(1));
            ext.view(&Instance::root(), &mut ui).unwrap();
            ui.finish();
        };
        frame(&mut core, &mut ext);
        frame(&mut core, &mut ext);
        assert!(ext.lua.globals().get::<bool>("opened").unwrap());
        let roles: Vec<_> = core
            .menu()
            .expect("open")
            .items
            .iter()
            .map(|i| (i.role, i.checked))
            .collect();
        assert_eq!(
            roles,
            [
                (kui_core::MenuRole::Copy, false),
                (kui_core::MenuRole::Separator, false),
                (kui_core::MenuRole::SelectAll, false),
                (kui_core::MenuRole::LookUp, false),
                (kui_core::MenuRole::Custom, true),
                (kui_core::MenuRole::Custom, false),
            ]
        );
        frame(&mut core, &mut ext);
        // The row the tree reports is the row the pointer can press, and
        // the checked one reads as checked there.
        let tree = core.access_tree();
        assert_eq!(
            tree.nodes
                .iter()
                .find(|n| n.name.as_deref() == Some("Wrap"))
                .expect("the checked row")
                .checked,
            Some(true)
        );
        let row = tree
            .nodes
            .iter()
            .find(|n| n.name.as_deref() == Some("Inspect"))
            .expect("the menu drew")
            .rect;
        let at = kui_core::Vec2::new(row.x + row.w / 2.0, row.y + row.h / 2.0);
        core.handle_input(InputEvent::CursorMoved(at));
        core.handle_input(InputEvent::mouse_down(1));
        let events = core.handle_input(InputEvent::mouse_up());
        assert_eq!(events.len(), 1, "{events:?}");
        assert_eq!(events[0].event, kui_core::EventKind::Menu);
        assert_eq!(events[0].payload.get_str("kind"), Some("card"));
        assert_eq!(events[0].data().unwrap().get_str("item"), Some("inspect"));
        assert_eq!(
            events[0].origin,
            OriginId(1),
            "posted with the origin that asked for the menu"
        );
    }

    /// A `selectable` container scopes one selection over the runs inside
    /// it, and a script reads it back by the scope's label (ADR 0017).
    #[test]
    fn scripts_select_and_read_a_scope() {
        let mut ext = LuaExtension::from_source(
            "sel",
            r#"
                frames = 0
                function view(env)
                  frames = frames + 1
                  if frames > 1 then
                    before = env.selection_text()
                    took = env.select_all_in("card")
                    text_out = env.selection_text()
                    not_a_scope = env.select_all_in("plain")
                  end
                  return column {
                    column { key = "card", selectable = true,
                      text("one", { size = 14 }),
                      text("two", { size = 14 }) },
                    row { key = "plain", width = 10, height = 10 },
                  }
                end
            "#,
        )
        .unwrap();
        let mut core = Core::new();
        let frame = |core: &mut Core, ext: &mut LuaExtension| {
            let mut ui = core.frame(Size::new(400.0, 100.0), 1.0);
            ui.set_origin(OriginId(1));
            ext.view(&Instance::root(), &mut ui).unwrap();
            ui.finish();
        };
        frame(&mut core, &mut ext);
        frame(&mut core, &mut ext);
        let g = ext.lua.globals();
        assert!(matches!(
            g.get::<mlua::Value>("before").unwrap(),
            mlua::Value::Nil
        ));
        assert!(g.get::<bool>("took").unwrap());
        assert_eq!(g.get::<String>("text_out").unwrap(), "one\ntwo");
        assert!(
            !g.get::<bool>("not_a_scope").unwrap(),
            "a node that drew no text is not a scope"
        );
    }

    /// A script turns a click into a caret with `env.text_hit` and a caret
    /// into a rect with `env.caret_rect` (backlog C18), both by the `line`
    /// row's label and across the runs inside it — answered, mid-build,
    /// from the frame that finished.
    #[test]
    fn scripts_map_points_to_bytes_on_a_line_of_runs() {
        let mut ext = LuaExtension::from_source(
            "hit",
            r#"
                frames = 0
                function view(env)
                  local mono = { size = 14, family = "mono" }
                  local w = env.measure_text("M", mono, 0).width
                  frames = frames + 1
                  -- A label nothing has declared yet is an error by name
                  -- (F5), so the first frame declares and the next asks.
                  if frames > 1 then
                    hit = env.text_hit("line", 7.2 * w, 5)
                    seam = env.caret_rect("line", 4)
                    far = env.text_hit("line", 390, 5)
                    none = env.caret_rect("plain", 0)
                  end
                  cell = w
                  return column {
                    row { key = "line",
                      text("let ", mono),
                      row { bg = 0x3b5bd455, text("value", mono) },
                      text(" = 1;", mono) },
                    row { key = "plain", width = 10, height = 10 },
                  }
                end
            "#,
        )
        .unwrap();
        let mut core = Core::new();
        let frame = |core: &mut Core, ext: &mut LuaExtension| {
            let mut ui = core.frame(Size::new(400.0, 100.0), 1.0);
            ui.set_origin(OriginId(1));
            ext.view(&Instance::root(), &mut ui).unwrap();
            ui.finish();
        };
        frame(&mut core, &mut ext);
        let hit: mlua::Value = ext.lua.globals().get("hit").unwrap();
        assert!(
            matches!(hit, mlua::Value::Nil),
            "nothing drawn before the first frame"
        );
        frame(&mut core, &mut ext);
        let hit: mlua::Table = ext.lua.globals().get("hit").unwrap();
        assert_eq!(
            hit.get::<usize>("byte").unwrap(),
            7,
            "the fourth cell of \"value\""
        );
        assert_eq!(hit.get::<u32>("line").unwrap(), 0);
        let cell: f32 = ext.lua.globals().get("cell").unwrap();
        let seam: mlua::Table = ext.lua.globals().get("seam").unwrap();
        let x: f32 = seam.get("x").unwrap();
        assert!((x - 4.0 * cell).abs() < 0.75, "{x} vs {}", 4.0 * cell);
        assert_eq!(seam.get::<f32>("w").unwrap(), 0.0);
        let far: mlua::Table = ext.lua.globals().get("far").unwrap();
        // 14 is also what pins the prelude's `text` copying its options:
        // the three runs share one `mono` table, and before the copy they
        // were one table holding the last string, so the line was three
        // times " = 1;" and 15 long.
        assert_eq!(
            far.get::<usize>("byte").unwrap(),
            14,
            "the end, across the runs"
        );
        let none: mlua::Value = ext.lua.globals().get("none").unwrap();
        assert!(matches!(none, mlua::Value::Nil), "a node that drew no text");
    }

    /// `cells { lines=, runs= }` is a terminal's screen as one node
    /// (backlog C20): the rows draw, a run colours its span, and the
    /// access tree reads the screen back.
    #[test]
    fn scripts_draw_a_screen_of_cells() {
        let mut ext = LuaExtension::from_source(
            "term",
            r#"
                function view(env)
                  return column { cells { key = "term", rows = 2, cols = 11, size = 14, family = "mono",
                    lines = { "hello world", "  bye" },
                    runs = { { 1, 2, 3, 0xff0000ff, 0x0000ffff, 1 } },
                    cursor_at = { 1, 4 }, cursor_shape = "underline" } }
                end
            "#,
        )
        .unwrap();
        let mut core = Core::new();
        let mut ui = core.frame(Size::new(400.0, 100.0), 1.0);
        ui.set_origin(OriginId(1));
        ext.view(&Instance::root(), &mut ui).unwrap();
        ui.finish();
        let (dl, _) = core.output();
        let glyphs = dl
            .quads
            .iter()
            .filter(|q| q.kind == kui_core::QuadKind::GlyphMask)
            .count();
        assert_eq!(glyphs, 13, "hello world + bye");
        let tree = core.access_tree();
        let term = tree
            .nodes
            .iter()
            .find(|n| n.role == kui_core::Role::Terminal)
            .expect("a terminal node");
        assert_eq!(term.value.as_deref(), Some("hello world\n  bye"));
    }

    /// Every count, position, index, byte and line the binding reads is
    /// a measure (backlog F180): Lua 5.5's `/` yields a float, so a grid
    /// sized `room_h / line_h` is 7.5 rows on an odd window, and under
    /// mlua 0.12.2 a fraction where Rust took an integer raised. The scene
    /// below is drawn twice, its numbers whole and then each with a half
    /// added, and draws, keys and answers the same — `index` on a box
    /// always read so, and now `cells`, its runs and cursor, a button's
    /// `index`, a dropdown's `current`, `list`'s `overscan`, `row_heights`
    /// and `caret_rect` / `caret_in_text` agree with it.
    #[test]
    fn a_count_from_a_division_is_floored_where_the_binding_reads_one() {
        const SCENE: &str = r#"
            heights = row_heights(10 + H, 20)
            frames = 0
            function view(env)
              frames = frames + 1
              heights:set(2 + H, 30)
              got = { heights:len(), heights:get(2 + H), heights:measured(2 + H),
                      heights:offset_of(3 + H) }
              if frames > 1 then
                local r = env.caret_rect("para", 3 + H)
                caret = { r.x, r.y }
                local c = env.caret_in_text("hello", { size = 14 }, nil, 3 + H)
                caret_in = { c.x, c.y }
              end
              return column {
                cells { key = "term", rows = 15 / 2 - 0.5 + H, cols = 4 + H,
                        size = 14, family = "mono", origin_line = 900 + H,
                        lines = { "abcdef", "ghij", "kl", "m", "n", "o", "p", "q" },
                        runs = { { 1 + H, 1 + H, 2 + H, 0xff0000ff + H, 0x0000ffff + H,
                                   1 + H, 0x00ff00ff + H } },
                        cursor_at = { 0 + H, 1 + H } },
                row { key = "para", text("hello", { size = 14 }) },
                button { label = "Open", index = 2 + H, on_click = "open" },
                dropdown { label = "language", options = { "A", "B", "C" }, current = 2 + H },
                column { key = "list", width = 200, height = 60,
                  list(env, { key = "rows", heights = heights, width = "grow", height = "grow",
                              overscan = 1 + H },
                    function(i, width) return 20 end,
                    function(i) return text("row " .. i) end) },
              }
            end
        "#;
        // What a frame drew, keyed and read back, and what the script
        // got from the calls.
        let scene = |half: bool| -> String {
            let src = SCENE.replace("H", if half { "0.5" } else { "0" });
            let mut ext = LuaExtension::from_source("whole", &src).unwrap();
            let mut core = Core::new();
            core.set_diagnostics(true);
            for _ in 0..2 {
                let mut ui = core.frame(Size::new(400.0, 400.0), 1.0);
                ui.set_origin(OriginId(1));
                ext.view(&Instance::root(), &mut ui).unwrap();
                ui.finish();
            }
            let (dl, _) = core.output();
            let quads = format!("{:?}", dl.quads);
            let tree: Vec<String> = core
                .access_tree()
                .nodes
                .iter()
                .map(|n| format!("{:?} {:?} {:?} {:?}", n.key, n.role, n.name, n.value))
                .collect();
            let g = ext.lua.globals();
            let got: Vec<f64> = ["got", "caret", "caret_in"]
                .iter()
                .flat_map(|name| g.get::<Vec<f64>>(*name).unwrap())
                .collect();
            let warnings: Vec<&str> = core.take_warnings().iter().map(|w| w.code).collect();
            format!("{quads}\n{tree:#?}\n{got:?}\n{warnings:?}")
        };
        let whole = scene(false);
        // The tree is printed `{:#?}` over `{:?}`, so its newlines are
        // escaped twice.
        assert!(
            whole.contains(r#"abcd\\nghij\\nkl\\nm\\nn\\no\\np\")"#),
            "the grid is 7 x 4: {whole}"
        );
        assert_eq!(
            scene(true),
            whole,
            "a half added to every number draws the same"
        );
    }

    /// A handle — an image's, a fragment's, a sound's, a window's — is a
    /// whole number as kui gave it out (backlog F180): a float that is one
    /// is that handle, and a fraction is refused by name, where a count is
    /// floored, since flooring a handle names a different image.
    #[test]
    fn a_handle_is_a_whole_number_and_a_fraction_is_refused_by_name() {
        let view = |body: &str| -> Result<(), String> {
            let src = format!("function view(env) {body} end");
            let mut ext = LuaExtension::from_source("handles", &src).unwrap();
            let mut core = Core::new();
            let mut ui = core.frame(Size::new(300.0, 200.0), 1.0);
            ui.set_origin(OriginId(1));
            let r = ext
                .view(&Instance::root(), &mut ui)
                .map_err(|e| e.to_string());
            ui.finish();
            r
        };
        for whole in [
            "return column { image { id = 3.0, width = 10, height = 10 } }",
            "return column { audio { src = 2.0 } }",
            "return column { fragment { id = 4.0, image = 3.0, width = 10, height = 10 } }",
            "return column { row { click_sound = 2.0, on_click = 'x', text('a') } }",
            "env.focus_window(env.window.id + 0.0) return column {}",
            "env.set_window_size(env.window.id * 1.0, 300, 200) return column {}",
        ] {
            assert_eq!(view(whole), Ok(()), "{whole}");
        }
        for (fraction, named) in [
            (
                "return column { image { id = 3.5, width = 10, height = 10 } }",
                "image id",
            ),
            ("return column { audio { src = 2.5 } }", "audio src"),
            ("return column { fragment { id = 4.5 } }", "fragment id"),
            (
                "return column { fragment { id = 4, image = 0.5 } }",
                "fragment image",
            ),
            (
                "return column { row { click_sound = 2.5, on_click = 'x', text('a') } }",
                "click_sound",
            ),
            (
                "env.focus_window(env.window.id + 0.5) return column {}",
                "focus_window: window",
            ),
            (
                "env.set_window_size(1 / 0, 300, 200) return column {}",
                "set_window_size: window",
            ),
            (
                "return column { image { id = 'a' } }",
                "image id must be a handle",
            ),
        ] {
            let err = view(fraction).unwrap_err();
            assert!(err.contains(named), "{fraction}: {err}");
        }
    }

    /// A grid's rows and cols are measures, so `math.huge` is one; the
    /// buffer the binding fills is cut to what the core draws (`MAX_CELLS`,
    /// with its warning), not `rows * cols` cells asked of the allocator.
    #[test]
    fn a_grid_past_what_the_core_draws_is_cut_not_allocated() {
        let mut ext = LuaExtension::from_source(
            "huge",
            r#"
                function view(env)
                  return column { cells { key = "term", rows = math.huge, cols = 2048,
                    size = 14, family = "mono", lines = { "top" } } }
                end
            "#,
        )
        .unwrap();
        let mut core = Core::new();
        core.set_diagnostics(true);
        let mut ui = core.frame(Size::new(400.0, 100.0), 1.0);
        ui.set_origin(OriginId(1));
        ext.view(&Instance::root(), &mut ui).unwrap();
        ui.finish();
        let codes: Vec<&str> = core.take_warnings().iter().map(|w| w.code).collect();
        assert_eq!(codes, [kui_core::diag::CELLS_TOO_LARGE]);
    }

    /// `cols = math.huge` is a `usize::MAX`, and the cell a line or a run
    /// lands on was `row * cols + col`: past the first row that overflowed,
    /// a panic in a debug build and a wrong cell in a release one. The
    /// rows past the buffer are cut as the core cuts them, and a run past
    /// it — a huge column, a huge length, a row the buffer never reaches
    /// — paints nothing.
    #[test]
    fn a_grid_with_a_huge_cols_is_cut_without_overflowing() {
        let mut ext = LuaExtension::from_source(
            "wide",
            r#"
                function view(env)
                  return column { cells { key = "term", rows = 3, cols = math.huge,
                    size = 14, family = "mono", lines = { "top", "middle", "bottom" },
                    runs = { { 0, 1, math.huge, 0xff0000ff },
                             { 1, 2, 3, 0x00ff00ff },
                             { 2, math.huge, math.huge, 0x0000ffff } },
                    cursor_at = { 2, math.huge } } }
                end
            "#,
        )
        .unwrap();
        let mut core = Core::new();
        core.set_diagnostics(true);
        let mut ui = core.frame(Size::new(400.0, 100.0), 1.0);
        ui.set_origin(OriginId(1));
        ext.view(&Instance::root(), &mut ui).unwrap();
        ui.finish();
        let codes: Vec<&str> = core.take_warnings().iter().map(|w| w.code).collect();
        assert_eq!(codes, [kui_core::diag::CELLS_TOO_LARGE]);
    }

    /// Counts are floored (backlog F180), so `rows = room_h / line_h` in a
    /// window shorter than one line is 0, and `cells` refused it, failing
    /// the whole view. 0 rows, 0 cols or half a row is an empty grid: the
    /// node is declared under its key with every row an app would pass
    /// it, nothing selects in it, and a press in its box, a double
    /// click's word, a triple click's row and a copy find no cell. A count left out is
    /// still refused (backlog RG160).
    #[test]
    fn a_grid_of_no_rows_or_no_cols_is_an_empty_grid() {
        for (rows, cols) in [("0", "12"), ("3", "0"), ("0.5", "12"), ("0", "0")] {
            let src = format!(
                r#"
                frames = 0
                function view(env)
                  frames = frames + 1
                  if frames == 2 then picked = env.select_all_in("term") end
                  sel = env.cell_selection()
                  return column {{ cells {{ key = "term", rows = {rows}, cols = {cols},
                    size = 14, family = "mono", line_height = 20, width = 200, height = 20,
                    origin_line = 40, selectable = true, on_click = "hit", on_layout = "laid",
                    lines = {{ "hello", "world" }}, runs = {{ {{ 0, 0, 5, 0xff0000ff }} }},
                    cursor_at = {{ 0, 2 }}, cursor_shape = "bar" }} }}
                end
            "#
            );
            let what = format!("rows = {rows}, cols = {cols}");
            let mut ext = LuaExtension::from_source("empty", &src).unwrap();
            let mut core = Core::new();
            core.set_diagnostics(true);
            let frame = |core: &mut Core, ext: &mut LuaExtension| {
                let mut ui = core.frame(Size::new(400.0, 100.0), 1.0);
                ui.set_origin(OriginId(1));
                ext.view(&Instance::root(), &mut ui).unwrap();
                ui.finish();
                core.output();
            };
            for _ in 0..3 {
                frame(&mut core, &mut ext);
            }
            let warnings = core.take_warnings();
            assert!(warnings.is_empty(), "{what}: {warnings:?}");
            let key = core.key_of("term").expect("the grid is declared");
            let r = core.layout_of(key).expect("the grid is laid out");
            let g = ext.lua.globals();
            assert!(
                !g.get::<bool>("picked").unwrap(),
                "{what}: nothing to select"
            );
            assert!(g.get::<Option<Table>>("sel").unwrap().is_none(), "{what}");

            // A press, then a double and a triple click, on the box sized
            // round no cells.
            let at = Vec2::new(r.x + 5.0, r.y + 5.0);
            core.handle_input(InputEvent::CursorMoved(at));
            for clicks in [1, 2, 3] {
                core.handle_input(InputEvent::MouseDown {
                    button: kui_core::MouseButton::Primary,
                    clicks,
                });
                let up = core.handle_input(InputEvent::MouseUp {
                    button: kui_core::MouseButton::Primary,
                });
                assert!(
                    up.iter().any(|e| e.key == key),
                    "{what}: the click is the grid's"
                );
                frame(&mut core, &mut ext);
            }
            assert_eq!(core.cell_selection_text(), None, "{what}");
            assert_eq!(core.key_of("term"), Some(key), "{what}");
        }

        let mut missing = LuaExtension::from_source(
            "missing",
            r#"function view(env) return column { cells { key = "term", cols = 12 } } end"#,
        )
        .unwrap();
        let mut core = Core::new();
        let mut ui = core.frame(Size::new(400.0, 100.0), 1.0);
        ui.set_origin(OriginId(1));
        let err = missing.view(&Instance::root(), &mut ui).unwrap_err();
        ui.finish();
        assert!(
            err.to_string().contains("cells needs rows and cols"),
            "{err}"
        );
    }

    /// The window grows back: a grid that fit no rows and then fits three
    /// is one node throughout, so the focus the app put on it — a
    /// terminal's key sink — is still there when the rows come back.
    #[test]
    fn a_grid_that_grows_from_no_rows_keeps_its_key_and_focus() {
        let mut ext = LuaExtension::from_source(
            "grow",
            r#"
                rows = 0
                function view(env)
                  return column { cells { key = "term", rows = rows, cols = 12,
                    size = 14, family = "mono", line_height = 20, on_key = "keys", on_layout = "laid",
                    lines = { "one", "two", "three" } } }
                end
            "#,
        )
        .unwrap();
        let mut core = Core::new();
        let frame = |core: &mut Core, ext: &mut LuaExtension| {
            let mut ui = core.frame(Size::new(400.0, 100.0), 1.0);
            ui.set_origin(OriginId(1));
            ext.view(&Instance::root(), &mut ui).unwrap();
            ui.finish();
            core.output();
        };
        frame(&mut core, &mut ext);
        let key = core.key_of("term").expect("an empty grid is declared");
        assert_eq!(core.layout_of(key).unwrap().h, 0.0);
        core.set_focus(Some(key));
        frame(&mut core, &mut ext);
        assert_eq!(core.focus(), Some(key));

        ext.lua.globals().set("rows", 3).unwrap();
        frame(&mut core, &mut ext);
        assert_eq!(core.key_of("term"), Some(key));
        assert_eq!(
            core.focus(),
            Some(key),
            "the focus outlived the empty frames"
        );
        assert_eq!(core.layout_of(key).unwrap().h, 60.0);
    }

    /// `origin_line = math.huge` saturates to `u64::MAX`, and the core adds
    /// a row to the origin unchecked to draw a selection and to read one
    /// back. The binding holds the origin to `MAX_ORIGIN_LINE`, so a
    /// selected grid there draws, and its lines read back as the integers
    /// they are.
    #[test]
    fn a_selected_grid_at_a_huge_origin_line_draws_and_reads_back() {
        let mut ext = LuaExtension::from_source(
            "far",
            r#"
                frames = 0
                function view(env)
                  frames = frames + 1
                  if frames == 2 then picked = env.select_all_in("term") end
                  sel = env.cell_selection()
                  return column { cells { key = "term", rows = 2, cols = 12,
                    size = 14, family = "mono", line_height = 20,
                    origin_line = math.huge, selectable = true,
                    lines = { "hello world", "bye there" } } }
                end
            "#,
        )
        .unwrap();
        let mut core = Core::new();
        let frame = |core: &mut Core, ext: &mut LuaExtension| {
            let mut ui = core.frame(Size::new(400.0, 100.0), 1.0);
            ui.set_origin(OriginId(1));
            ext.view(&Instance::root(), &mut ui).unwrap();
            ui.finish();
            core.output();
        };
        for _ in 0..3 {
            frame(&mut core, &mut ext);
        }
        let g = ext.lua.globals();
        assert!(g.get::<bool>("picked").unwrap(), "the grid is a scope");
        let line = |end: &str| -> i64 {
            let sel: Table = g.get("sel").unwrap();
            sel.get::<Table>(end).unwrap().get("line").unwrap()
        };
        assert_eq!(line("anchor"), MAX_ORIGIN_LINE as i64);
        assert_eq!(line("focus"), MAX_ORIGIN_LINE as i64 + 1);

        // A triple click selects a row by its absolute line, the core
        // adding the row to the origin again.
        core.handle_input(InputEvent::CursorMoved(Vec2::new(10.0, 30.0)));
        core.handle_input(InputEvent::MouseDown {
            button: kui_core::MouseButton::Primary,
            clicks: 3,
        });
        core.handle_input(InputEvent::MouseUp {
            button: kui_core::MouseButton::Primary,
        });
        frame(&mut core, &mut ext);
        frame(&mut core, &mut ext);
        assert_eq!(line("anchor"), MAX_ORIGIN_LINE as i64 + 1);
    }

    /// A row count is a measure, so `row_heights(math.huge)` was a
    /// `usize::MAX` and the core's `vec!` of that many heights panicked on
    /// its capacity. A count past `MAX_ROWS` is refused by name, at the
    /// constructor and at `set_len`; one inside it, fraction and all, is
    /// floored as before.
    #[test]
    fn a_row_count_past_what_a_list_holds_is_refused_by_name() {
        let run = |src: &str| -> Result<(), String> {
            let ext = LuaExtension::from_source("rows", src).map_err(|e| e.to_string())?;
            ext.lua.load("check()").exec().map_err(|e| e.to_string())
        };
        assert_eq!(
            run(
                "function check() local h = row_heights(10.5, 20) h:set_len(12.5) \
                 assert(h:len() == 12) end"
            ),
            Ok(())
        );
        for (src, named) in [
            (
                "function check() row_heights(math.huge, 20) end",
                "row_heights: rows",
            ),
            (
                "function check() row_heights(2^32, 20) end",
                "row_heights: rows",
            ),
            (
                "function check() row_heights(10, 20):set_len(math.huge) end",
                "set_len: rows",
            ),
        ] {
            let err = run(src).unwrap_err();
            assert!(err.contains(named), "{src}: {err}");
        }
    }

    /// `list`'s `overscan` is a measure, and the core adds it to a row
    /// index unchecked: `math.huge` overflowed there. Held to the row
    /// count, it builds every row, which is what an overscan that large
    /// asks for.
    #[test]
    fn a_huge_overscan_builds_every_row_without_overflowing() {
        let mut ext = LuaExtension::from_source(
            "overscan",
            r#"
                heights = row_heights(50, 20)
                built = 0
                function view(env)
                  built = 0
                  return column { key = "list", width = 200, height = 60,
                    list(env, { key = "rows", heights = heights, width = "grow",
                                height = "grow", overscan = math.huge },
                      function(i, width) return 20 end,
                      function(i) built = built + 1 return text("row " .. i) end) }
                end
            "#,
        )
        .unwrap();
        let mut core = Core::new();
        for _ in 0..2 {
            let mut ui = core.frame(Size::new(400.0, 400.0), 1.0);
            ui.set_origin(OriginId(1));
            ext.view(&Instance::root(), &mut ui).unwrap();
            ui.finish();
        }
        let built: i64 = ext.lua.globals().get("built").unwrap();
        assert_eq!(built, 50, "every row, the overscan held to the count");
    }

    /// kui gives out no negative handle and no window past a `u32`, so
    /// both are refused by name, an integer's as a float's: `as u32` wrapped
    /// `4294967297` to window 1 and `-1` to `u32::MAX`, and a negative
    /// sound or image cast to `u64` named a handle near the top.
    #[test]
    fn a_handle_out_of_its_range_is_refused_by_name() {
        let view = |body: &str| -> Result<(), String> {
            let src = format!("function view(env) {body} end");
            let mut ext = LuaExtension::from_source("handles", &src).unwrap();
            let mut core = Core::new();
            let mut ui = core.frame(Size::new(300.0, 200.0), 1.0);
            ui.set_origin(OriginId(1));
            let r = ext
                .view(&Instance::root(), &mut ui)
                .map_err(|e| e.to_string());
            ui.finish();
            r
        };
        for (body, named) in [
            (
                "env.focus_window(4294967297) return column {}",
                "focus_window: window",
            ),
            (
                "env.focus_window(4294967297.0) return column {}",
                "focus_window: window",
            ),
            (
                "env.focus_window(-1) return column {}",
                "focus_window: window",
            ),
            (
                "env.set_window_size(-1.0, 300, 200) return column {}",
                "set_window_size: window",
            ),
            (
                "env.set_window_size(math.maxinteger, 300, 200) return column {}",
                "set_window_size: window",
            ),
            ("return column { audio { src = -1 } }", "audio src"),
            ("return column { audio { src = -2.0 } }", "audio src"),
            (
                "return column { image { id = -3, width = 10, height = 10 } }",
                "image id",
            ),
            ("return column { fragment { id = -4 } }", "fragment id"),
            (
                "return column { fragment { id = 4, image = -1 } }",
                "fragment image",
            ),
            (
                "return column { row { click_sound = -2, on_click = 'x', text('a') } }",
                "click_sound",
            ),
        ] {
            let err = view(body).unwrap_err();
            assert!(err.contains(named), "{body}: {err}");
        }
        // The top of each range is still a handle.
        for body in [
            "env.focus_window(4294967295) return column {}",
            "return column { audio { src = math.maxinteger } }",
        ] {
            assert_eq!(view(body), Ok(()), "{body}");
        }
    }

    /// A `selectable` grid selects in cells, and the click count picks the
    /// grain — one a cell, two the word, three the whole row (ADR 0017,
    /// decision 4). Lua declares the scope and reads the result back
    /// through `env.selection_text`; the gesture itself is the core's, so
    /// what this pins is that a Lua-declared grid is a scope at all.
    #[test]
    fn a_lua_grid_selects_by_cell_word_and_row() {
        let mut ext = LuaExtension::from_source(
            "term",
            r#"
                function view(env)
                  said = env.selection_text()
                  return column { cells { key = "term", rows = 2, cols = 12,
                    size = 14, family = "mono", line_height = 20,
                    origin_line = 900, selectable = true,
                    lines = { "hello world", "bye there" } } }
                end
            "#,
        )
        .unwrap();
        let mut core = Core::new();
        let frame = |core: &mut Core, ext: &mut LuaExtension| {
            let mut ui = core.frame(Size::new(400.0, 100.0), 1.0);
            ui.set_origin(OriginId(1));
            ext.view(&Instance::root(), &mut ui).unwrap();
            ui.finish();
        };
        frame(&mut core, &mut ext);
        let said = |ext: &LuaExtension| ext.lua.globals().get::<Option<String>>("said").unwrap();

        // The middle of (row, col), from the grid's own metrics.
        let w = core
            .measure_text(
                "M",
                &kui_core::TextStyle::new(14.0)
                    .family(kui_core::FontFamily::Mono)
                    .line_height(20.0),
                None,
            )
            .width;
        let at = |r: usize, c: usize| Vec2::new(w * (c as f32 + 0.5), 20.0 * (r as f32 + 0.5));
        let click = |core: &mut Core, p: Vec2, clicks: u8| {
            core.handle_input(InputEvent::CursorMoved(p));
            core.handle_input(InputEvent::MouseDown {
                button: kui_core::MouseButton::Primary,
                clicks,
            });
            core.handle_input(InputEvent::MouseUp {
                button: kui_core::MouseButton::Primary,
            });
        };

        click(&mut core, at(0, 8), 2);
        frame(&mut core, &mut ext);
        assert_eq!(said(&ext).as_deref(), Some("world"), "a double click");

        click(&mut core, at(1, 1), 3);
        frame(&mut core, &mut ext);
        assert_eq!(said(&ext).as_deref(), Some("bye there"), "a triple click");
    }

    /// `env.cell_selection()` reads a grid's selection back the way
    /// `selection_ends()` reads a text's (backlog B1a): the ends as the
    /// drag made them, the lines absolute — row 1 of a screen whose row 0
    /// is line 900 is line 901 — and nil while the window's selection is
    /// not a grid's.
    #[test]
    fn a_grid_selection_reads_back_as_absolute_lines_and_columns() {
        let mut ext = LuaExtension::from_source(
            "term",
            r#"
                function view(env)
                  sel = env.cell_selection()
                  return column { cells { key = "term", rows = 2, cols = 12,
                    size = 14, family = "mono", line_height = 20,
                    origin_line = 900, selectable = true,
                    lines = { "hello world", "bye there" } } }
                end
            "#,
        )
        .unwrap();
        let mut core = Core::new();
        let frame = |core: &mut Core, ext: &mut LuaExtension| {
            let mut ui = core.frame(Size::new(400.0, 100.0), 1.0);
            ui.set_origin(OriginId(1));
            ext.view(&Instance::root(), &mut ui).unwrap();
            ui.finish();
        };
        frame(&mut core, &mut ext);
        assert!(
            ext.lua
                .globals()
                .get::<mlua::Value>("sel")
                .unwrap()
                .is_nil(),
            "nothing selected yet"
        );
        let w = core
            .measure_text(
                "M",
                &kui_core::TextStyle::new(14.0)
                    .family(kui_core::FontFamily::Mono)
                    .line_height(20.0),
                None,
            )
            .width;
        let at = |r: usize, c: usize| Vec2::new(w * (c as f32 + 0.5), 20.0 * (r as f32 + 0.5));
        core.handle_input(InputEvent::CursorMoved(at(1, 4)));
        core.handle_input(InputEvent::MouseDown {
            button: kui_core::MouseButton::Primary,
            clicks: 1,
        });
        core.handle_input(InputEvent::CursorMoved(at(0, 1)));
        core.handle_input(InputEvent::MouseUp {
            button: kui_core::MouseButton::Primary,
        });
        frame(&mut core, &mut ext);
        let sel: Table = ext.lua.globals().get("sel").unwrap();
        let end = |name: &str| -> (u64, usize) {
            let t: Table = sel.get(name).unwrap();
            (t.get("line").unwrap(), t.get("col").unwrap())
        };
        assert_eq!(end("anchor"), (901, 4), "the press, on the second row");
        assert_eq!(end("focus"), (900, 1), "the pointer, backwards");
        assert_eq!(
            sel.get::<i64>("node").unwrap(),
            core.key_of("term").unwrap().0 as i64
        );
        assert!(!sel.get::<bool>("block").unwrap());
    }

    /// A span's `underline`, `strikethrough` and `bg` reach the core
    /// (backlog C22): the frame carries the solid quads beside the glyphs.
    #[test]
    fn spans_carry_their_decorations() {
        let mut ext = LuaExtension::from_source(
            "deco",
            r#"
                function view(env)
                  return row { text({ "let ", { "value", bg = 0x3b5bd455, underline = true },
                                      " = 1;" }, { size = 14, family = "mono" }) }
                end
            "#,
        )
        .unwrap();
        let mut core = Core::new();
        let mut ui = core.frame(Size::new(400.0, 100.0), 1.0);
        ui.set_origin(OriginId(1));
        ext.view(&Instance::root(), &mut ui).unwrap();
        ui.finish();
        let (dl, _) = core.output();
        let solids = dl
            .quads
            .iter()
            .filter(|q| q.kind == kui_core::QuadKind::Solid)
            .count();
        assert_eq!(solids, 2, "a background and an underline");
    }

    /// An underline's own colour and shape (backlog K4): `underline_color`
    /// and `underline_style` on a span, on a text's options, and a run's
    /// seventh entry and the shape bits on cells. A wave is segment quads
    /// in the underline's colour; a solid coloured line is one solid.
    #[test]
    fn underlines_have_a_colour_and_a_shape_of_their_own() {
        let mut ext = LuaExtension::from_source(
            "k4",
            r#"
                function view(env)
                  return column {
                    text({ "let ", { "value", underline_color = 0xff0000ff, underline_style = "wavy" } },
                         { size = 14, family = "mono" }),
                    text("warn", { size = 14, family = "mono", underline_color = 0x00ff00ff }),
                    cells { rows = 1, cols = 3, size = 14, family = "mono",
                            lines = { "abc" }, runs = { { 0, 0, 3, 0, 0, 32, 0xff0000ff } } },
                  }
                end
            "#,
        )
        .unwrap();
        let mut core = Core::new();
        let mut ui = core.frame(Size::new(400.0, 200.0), 1.0);
        ui.set_origin(OriginId(1));
        ext.view(&Instance::root(), &mut ui).unwrap();
        ui.finish();
        let (dl, _) = core.output();
        let red = Color::hex(0xff0000ff);
        let segs: Vec<_> = dl
            .quads
            .iter()
            .filter(|q| q.kind == kui_core::QuadKind::Segment)
            .collect();
        assert!(
            segs.len() >= 6,
            "a wave under the span and an undercurl over the cells: {}",
            segs.len()
        );
        assert!(
            segs.iter().all(|q| q.color == red),
            "in the underline colour"
        );
        let solids: Vec<_> = dl
            .quads
            .iter()
            .filter(|q| q.kind == kui_core::QuadKind::Solid)
            .collect();
        assert_eq!(solids.len(), 1, "the text's solid line, coloured");
        assert_eq!(solids[0].color, Color::hex(0x00ff00ff));
        // A shape nobody spells is refused where it is declared.
        let mut bad = LuaExtension::from_source(
            "k4bad",
            r#"function view(env) return text({ { "x", underline_style = "squiggly" } }) end"#,
        )
        .unwrap();
        let mut ui = core.frame(Size::new(400.0, 200.0), 1.0);
        ui.set_origin(OriginId(1));
        let err = bad.view(&Instance::root(), &mut ui).unwrap_err();
        assert!(err.contains("solid | wavy | dotted"), "{err}");
    }

    /// `dash` on a line (backlog V2): one length is marks and gaps alike,
    /// a pair is a mark and a gap, and `dash_offset` starts into the
    /// pattern; a count nobody spells is refused where it is declared.
    #[test]
    fn a_line_takes_a_dash_as_a_length_a_pair_or_four() {
        let segments = |view: &str| -> Result<usize, String> {
            let mut ext = LuaExtension::from_source("dash", view).unwrap();
            let mut core = Core::new();
            let mut ui = core.frame(Size::new(400.0, 200.0), 1.0);
            ui.set_origin(OriginId(1));
            let lowered = ext.view(&Instance::root(), &mut ui);
            ui.finish();
            lowered?;
            let (dl, _) = core.output();
            Ok(dl
                .quads
                .iter()
                .filter(|q| q.kind == kui_core::QuadKind::Segment)
                .count())
        };
        let line = |dash: &str| {
            format!(
                "function view(env) return column {{ width = 200, height = 40,
                   line {{ from = {{0, 20}}, to = {{100, 20}}, width = 2, {dash} }} }} end"
            )
        };
        assert_eq!(segments(&line("")), Ok(1));
        // 5 on, 5 off: a mark every 10 px.
        assert_eq!(segments(&line("dash = 5")), Ok(10));
        assert_eq!(segments(&line("dash = {6, 4}")), Ok(10));
        assert_eq!(segments(&line("dash = {6, 4}, dash_offset = 5")), Ok(10));
        assert_eq!(segments(&line("dash = {10, 4, 2, 4}")), Ok(10));
        let err = segments(&line("dash = {1, 2, 3}")).unwrap_err();
        assert!(err.contains("dash is a length"), "{err}");
    }

    /// `features = "liga=0"` reaches the shaper through the same schema
    /// row every binding reads (backlog C23): a text with it and one
    /// without are shaped twice.
    #[test]
    fn features_are_a_text_option() {
        let mut ext = LuaExtension::from_source(
            "features",
            r#"
                function view(env)
                  return row { text("fi ->", { size = 16 }),
                               text("fi ->", { size = 16, features = "liga=0 calt=0" }) }
                end
            "#,
        )
        .unwrap();
        let mut core = Core::new();
        let mut ui = core.frame(Size::new(400.0, 100.0), 1.0);
        ui.set_origin(OriginId(1));
        ext.view(&Instance::root(), &mut ui).unwrap();
        ui.finish();
        assert_eq!(core.text_cache_len(), 2);
    }

    /// `text(s, opts)` copies its options. It used to write `type` and
    /// `value` into the table it was handed and return it, so a script
    /// that hoisted a style — `local mono = { size = 14 }` — and passed it
    /// to three texts built one table three times, showing the last string
    /// thrice. Found by the text-hit test above (backlog C18).
    #[test]
    fn a_style_table_shared_by_three_texts_is_three_texts() {
        let mut ext = LuaExtension::from_source(
            "shared",
            r#"
                local mono = { size = 14, family = "mono" }
                function view(env)
                  return row { text("a", mono), text("bb", mono), text("ccc", mono) }
                end
            "#,
        )
        .unwrap();
        let mut core = Core::new();
        let mut ui = core.frame(Size::new(400.0, 100.0), 1.0);
        ui.set_origin(OriginId(1));
        ext.view(&Instance::root(), &mut ui).unwrap();
        ui.finish();
        let names: Vec<String> = core
            .access_tree()
            .nodes
            .iter()
            .filter_map(|n| n.value.clone().or_else(|| n.name.clone()))
            .collect();
        let joined = names.join("|");
        assert!(
            joined.contains("a") && joined.contains("bb") && joined.contains("ccc"),
            "three different texts, got {joined:?}"
        );
        assert_eq!(
            core.text_cache_len(),
            3,
            "three strings shaped, not one three times"
        );
    }

    /// A script virtualizes a 10k-row list with nothing but
    /// `env.scroll_geometry`: it declares the rows crossing the window and
    /// two spacers holding the space of the rest, so the frame costs a
    /// screenful and the scrollbar still spans the whole list.
    #[test]
    fn scripts_slice_a_long_list_from_the_geometry() {
        let mut ext = LuaExtension::from_source(
            "virtual",
            r#"
                ROWS, ROW_H, built = 10000, 30, 0
                function view(env)
                  local g = env.scroll_geometry(list_key)
                  -- No layout yet: fall back to the window for one frame.
                  local h = g and g.h or 200
                  local top = g and g.offset.y or 0
                  local first = math.min(ROWS, math.max(0, math.floor(top / ROW_H)))
                  local last = math.min(ROWS, math.ceil((top + h) / ROW_H))
                  local list = { key = "list", width = "grow", height = "grow",
                                 scroll_y = true }
                  if first > 0 then
                    list[#list + 1] = row { key = "lead", width = "grow",
                                            height = first * ROW_H }
                  end
                  for i = first, last - 1 do
                    list[#list + 1] = row { key = "row" .. i, width = "grow",
                                            height = ROW_H, bg = 0x282840ff }
                  end
                  if last < ROWS then
                    list[#list + 1] = row { key = "tail", width = "grow",
                                            height = (ROWS - last) * ROW_H }
                  end
                  built = last - first
                  return column(list)
                end
            "#,
        )
        .unwrap();
        let list = Key::ROOT.str("list");
        ext.lua.globals().set("list_key", list.0 as i64).unwrap();
        let mut core = Core::new();
        let frame = |core: &mut Core, ext: &mut LuaExtension| {
            let mut ui = core.frame(Size::new(400.0, 200.0), 1.0);
            ui.set_origin(OriginId(1));
            ext.view(&Instance::root(), &mut ui).unwrap();
            ui.finish();
        };

        frame(&mut core, &mut ext);
        frame(&mut core, &mut ext);
        let built: i64 = ext.lua.globals().get("built").unwrap();
        assert_eq!(built, 7, "200 / 30 rounded up");

        // The spacers make it the same list: full travel, and "jump to the
        // end" lands on the last row even though it was never built.
        let g = core.scroll_geometry(list).expect("laid out");
        assert_eq!(g.content.h, 10_000.0 * 30.0);
        core.set_scroll(list, Vec2::new(0.0, 1e9));
        frame(&mut core, &mut ext);
        frame(&mut core, &mut ext);
        assert_eq!(core.scroll_offset(list).y, 10_000.0 * 30.0 - 200.0);
        let built: i64 = ext.lua.globals().get("built").unwrap();
        assert_eq!(built, 7, "still a screenful at the far end");
    }

    /// The same list as one call: `uniform_list` from the prelude owns the
    /// slicing, the two spacers and the row keys, and the script says what a
    /// row looks like. It names its container by label, which is why the
    /// queries had to answer for a name no frame has declared yet — the
    /// first frame asks before the container exists (backlog C25).
    #[test]
    fn the_prelude_virtualizes_a_long_list_in_one_call() {
        let mut ext = LuaExtension::from_source(
            "virtual",
            r#"
                ROWS, ROW_H, first_built, built = 10000, 30, -1, 0
                function view(env)
                  built, first_built = 0, -1
                  return uniform_list(env,
                    { key = "list", rows = ROWS, row_h = ROW_H,
                      width = "grow", height = "grow" },
                    function(i)
                      built = built + 1
                      if first_built < 0 then first_built = i end
                      return column { fill = true, bg = 0x282840ff,
                                      on_click = { kind = "pick", row = i } }
                    end)
                end
            "#,
        )
        .unwrap();
        let list = Key::ROOT.str("list");
        let mut core = Core::new();
        let frame = |core: &mut Core, ext: &mut LuaExtension| {
            let mut ui = core.frame(Size::new(400.0, 200.0), 1.0);
            ui.set_origin(OriginId(1));
            ext.view(&Instance::root(), &mut ui).unwrap();
            ui.finish();
        };

        frame(&mut core, &mut ext);
        frame(&mut core, &mut ext);
        let built: i64 = ext.lua.globals().get("built").unwrap();
        assert_eq!(built, 9, "200 / 30 rounded up, plus two rows of overscan");

        // The spacers make it the whole list, and a row keeps the key its
        // data index gives it however far the range has slid.
        let g = core.scroll_geometry(list).expect("laid out");
        assert_eq!(g.content.h, 10_000.0 * 30.0);
        core.set_scroll(list, Vec2::new(0.0, 300.0 * 30.0));
        frame(&mut core, &mut ext);
        frame(&mut core, &mut ext);
        let first: i64 = ext.lua.globals().get("first_built").unwrap();
        assert_eq!(first, 298, "two rows of overscan above row 300");
        let built: i64 = ext.lua.globals().get("built").unwrap();
        assert_eq!(built, 11, "a screenful with overscan on both sides now");
        // The row's own node carries the data index, which is the key a list
        // that built all ten thousand would have given it.
        assert!(
            core.access_tree()
                .nodes
                .iter()
                .any(|n| n.key == list.index(300).index(0)),
            "row 300 is not keyed by its data index"
        );
    }

    /// ADR 0037: a Lua view draws an installed family by naming it, with
    /// no host door, in the frame that names it — the face the host's
    /// handle draws, not sans — and a name nothing matches warns.
    #[test]
    fn a_view_names_a_family_and_draws_in_it() {
        let mut ext = LuaExtension::from_source(
            "fam",
            r#"
                named, sans, missed = nil, nil, nil
                function view(env)
                  named = env.measure_text("iiiWWW", { family = "Fixture Mono", size = 20 }).width
                  sans = env.measure_text("iiiWWW", { family = "sans", size = 20 }).width
                  return column {
                    text("iiiWWW", { family = "Fixture Mono", size = 20 }),
                    text("x", { family = "No Such Family 7" }),
                  }
                end
            "#,
        )
        .unwrap();
        let mut core = Core::new();
        core.add_font_data(kui_core::testing::font_face(
            "Fixture Mono",
            400,
            false,
            true,
        ))
        .unwrap();
        let mut ui = core.frame(Size::new(400.0, 100.0), 1.0);
        ui.set_origin(OriginId(1));
        ext.view(&Instance::root(), &mut ui).unwrap();
        ui.finish();
        let handle = core.add_system_font("Fixture Mono").unwrap();
        let expected = core
            .measure_text("iiiWWW", &kui_core::TextStyle::new(20.0).font(handle), None)
            .width;
        let g = ext.lua.globals();
        let (named, sans) = (
            g.get::<f32>("named").unwrap(),
            g.get::<f32>("sans").unwrap(),
        );
        assert_eq!(named, expected, "the face the host's handle draws");
        assert_ne!(named, sans, "and not sans");
        let codes: Vec<_> = core.take_warnings().iter().map(|w| w.code).collect();
        assert_eq!(codes, ["unknown-family"]);
    }

    /// DX22: the prelude's row spec, row reveal and divider, the three
    /// Rust gained as `uniform_list_with`, `reveal_row` and `splitter`.
    #[test]
    fn the_prelude_reveals_a_row_styles_rows_and_splits() {
        let mut ext = LuaExtension::from_source(
            "dx22",
            r#"
                target, scrolled, page = nil, nil, nil
                function view(env)
                  if target then scrolled = reveal_row(env, "list", target, 20) end
                  page = rows_in_view(env, "list", 20)
                  zero = rows_in_view(env, "list", 0)
                  return row { width = 300, height = 100,
                    uniform_list(env,
                      { key = "list", rows = 50, row_h = 20, width = 200, height = 100,
                        row_props = function(i) return { on_click = { kind = "pick", row = i } } end },
                      function(i) return text("row " .. i) end),
                    splitter(env, { key = "bar", on_drag = { kind = "split" } }),
                    column { width = "grow", height = "grow" },
                  }
                end
            "#,
        )
        .unwrap();
        let mut core = Core::new();
        let frame = |core: &mut Core, ext: &mut LuaExtension| {
            let mut ui = core.frame(Size::new(300.0, 100.0), 1.0);
            ui.set_origin(OriginId(1));
            ext.view(&Instance::root(), &mut ui).unwrap();
            ui.finish();
        };
        frame(&mut core, &mut ext);
        frame(&mut core, &mut ext);
        assert_eq!(ext.lua.globals().get::<i64>("page").unwrap(), 5);
        assert!(
            core.take_warnings().is_empty(),
            "every prop the three spell is known"
        );

        ext.lua.globals().set("target", 40).unwrap();
        frame(&mut core, &mut ext);
        assert!(ext.lua.globals().get::<bool>("scrolled").unwrap());
        let list = core.key_of("list").unwrap();
        assert_eq!(
            core.scroll_offset(list),
            Vec2::new(0.0, 760.0),
            "row 40 to the middle"
        );
        // Built for that offset in the same frame, each row clickable
        // through its own node.
        core.handle_input(InputEvent::CursorMoved(Vec2::new(10.0, 45.0)));
        core.handle_input(InputEvent::mouse_down(1));
        let up = core.handle_input(InputEvent::mouse_up());
        assert_eq!(up.iter().find_map(|e| e.payload.get_int("row")), Some(40));

        // The bar sits at 200..204; dragging it reports its parent's split.
        core.handle_input(InputEvent::CursorMoved(Vec2::new(202.0, 50.0)));
        core.handle_input(InputEvent::mouse_down(1));
        core.handle_input(InputEvent::CursorMoved(Vec2::new(225.0, 50.0)));
        let end = core.handle_input(InputEvent::mouse_up());
        let d = end.iter().find_map(|e| e.drag()).expect("the drag's end");
        assert_eq!(d.ratio().x, 0.75);

        // A row past the end, or no stride, scrolls nothing (backlog RG75).
        ext.lua.globals().set("target", 500).unwrap();
        frame(&mut core, &mut ext);
        assert!(!ext.lua.globals().get::<bool>("scrolled").unwrap());
        assert_eq!(core.scroll_offset(list), Vec2::new(0.0, 760.0));
        assert_eq!(ext.lua.globals().get::<i64>("zero").unwrap(), 0);
    }

    /// The same clamp as the JSX widget's: a list that shrank while scrolled
    /// slices past its own new end, and an unclamped `first` builds a lead
    /// spacer taller than the whole list with no rows in it.
    #[test]
    fn a_uniform_list_that_shrank_lands_in_one_frame() {
        let mut ext = LuaExtension::from_source(
            "virtual",
            r#"
                ROWS, first_built = 200, -1
                function view(env)
                  first_built = -1
                  return uniform_list(env,
                    { key = "list", rows = ROWS, row_h = 20,
                      width = "grow", height = "grow" },
                    function(i)
                      if first_built < 0 then first_built = i end
                      return column { fill = true, bg = 0x282840ff }
                    end)
                end
            "#,
        )
        .unwrap();
        let list = Key::ROOT.str("list");
        let mut core = Core::new();
        let frame = |core: &mut Core, ext: &mut LuaExtension| {
            let mut ui = core.frame(Size::new(400.0, 300.0), 1.0);
            ui.set_origin(OriginId(1));
            ext.view(&Instance::root(), &mut ui).unwrap();
            ui.finish();
        };
        frame(&mut core, &mut ext);
        frame(&mut core, &mut ext);
        core.set_scroll(list, Vec2::new(0.0, 3_000.0));
        frame(&mut core, &mut ext);
        frame(&mut core, &mut ext);
        let deep: i64 = ext.lua.globals().get("first_built").unwrap();
        assert!(
            deep > 100,
            "expected to be deep in the list, built from {deep}"
        );

        ext.lua.globals().set("ROWS", 10).unwrap();
        frame(&mut core, &mut ext);
        let g = core.scroll_geometry(list).expect("laid out");
        assert_eq!(
            g.content.h,
            10.0 * 20.0,
            "the content is the list it has now"
        );
        frame(&mut core, &mut ext);
        let first: i64 = ext.lua.globals().get("first_built").unwrap();
        assert_eq!(first, 0, "and every row of it is built");
        assert_eq!(core.scroll_offset(list).y, 0.0);
    }

    /// The variable-height list's script (backlog C46): rows 0..100 are 20
    /// px and the rest 60, so the estimate the first screenful produces is
    /// badly wrong for the middle of the list — which is what makes the
    /// anchor observable. `TRANSITION` puts RG18's glide on the container.
    const VARIABLE_LIST: &str = r#"
        heights = row_heights(1000, 20)
        measured = 0
        function view(env)
          return list(env,
            { key = "list", heights = heights, width = "grow", height = "grow",
              transition = TRANSITION },
            function(i, width)
              measured = measured + 1
              if i < 100 then return 20 else return 60 end
            end,
            function(i)
              return column { width = "grow", height = "grow", bg = 0x282840ff,
                              label = "row", on_click = i }
            end)
        end
    "#;

    /// One frame of `VARIABLE_LIST` at time `t`, 400 x 200.
    fn variable_frame(core: &mut Core, ext: &mut LuaExtension, t: f64) {
        core.set_time(t);
        let mut ui = core.frame(Size::new(400.0, 200.0), 1.0);
        ui.set_origin(OriginId(1));
        ext.view(&Instance::root(), &mut ui).unwrap();
        ui.finish();
    }

    /// Which row is under `y`, by a click rather than by arithmetic — the
    /// question "did the content move" asks of the pixels.
    fn row_under(core: &mut Core, y: f32) -> Option<i64> {
        core.handle_input(InputEvent::CursorMoved(Vec2::new(200.0, y)));
        core.handle_input(InputEvent::mouse_down(1));
        let evs = core.handle_input(InputEvent::mouse_up());
        evs.first()?.payload.get_int("tag")
    }

    /// The Lua port of `widgets::list`'s crux: measuring the rows a frame
    /// builds moves the estimate under every row above the window, and the
    /// row under the top edge stays the row under the top edge.
    #[test]
    fn a_lua_list_keeps_the_row_under_the_pointer_while_the_estimate_moves() {
        let mut ext = LuaExtension::from_source("variable", VARIABLE_LIST).unwrap();
        let list = Key::ROOT.str("list");
        let mut core = Core::new();
        variable_frame(&mut core, &mut ext, 0.0);
        variable_frame(&mut core, &mut ext, 0.0);
        let measured: i64 = ext.lua.globals().get("measured").unwrap();
        assert!(
            (10..=20).contains(&measured),
            "a screenful measured, not the list: {measured}"
        );

        core.set_scroll(list, Vec2::new(0.0, 5_000.0));
        variable_frame(&mut core, &mut ext, 0.0);
        let settled = row_under(&mut core, 4.0);
        assert!(settled.is_some(), "nothing under the top edge");
        for n in 0..4 {
            variable_frame(&mut core, &mut ext, 0.0);
            assert_eq!(
                row_under(&mut core, 4.0),
                settled,
                "the content slid on frame {n} as the estimate moved"
            );
        }
        let g = core.scroll_geometry(list).expect("laid out");
        assert!(
            g.content.h > 1000.0 * 20.0 * 1.5,
            "the list learned it is longer: {}",
            g.content.h
        );
    }

    /// RG18 through the Lua port: a long `set_scroll` on a container with a
    /// `transition` glides all the way to the row asked for, the heights of
    /// the rows it passes measured on the way.
    #[test]
    fn a_lua_list_glides_to_the_row_asked_for() {
        let mut ext =
            LuaExtension::from_source("variable", &format!("TRANSITION = 100\n{VARIABLE_LIST}"))
                .unwrap();
        let list = Key::ROOT.str("list");
        let mut core = Core::new();
        let mut t = 0.0;
        variable_frame(&mut core, &mut ext, t);
        variable_frame(&mut core, &mut ext, t);
        let target = 400;
        let offset: f32 = ext
            .lua
            .load(format!("return heights:offset_of({target})"))
            .eval()
            .unwrap();
        core.set_scroll(list, Vec2::new(0.0, offset));
        for _ in 0..30 {
            t += 1.0 / 60.0;
            variable_frame(&mut core, &mut ext, t);
        }
        assert_eq!(row_under(&mut core, 4.0), Some(target));
    }

    /// A script asks for a file dialog (backlog C51); the host takes the
    /// ask, answers it, and the answer comes back to the script that asked
    /// — its origin — as a `files` event, the message it gave with `files`
    /// in it.
    #[test]
    fn a_script_asks_for_a_file_dialog_and_hears_the_answer() {
        let mut ext = LuaExtension::from_source(
            "files",
            r#"
                ask, asked, again, waiting, heard = true, nil, nil, nil, nil
                function view(env)
                  if ask then
                    asked = env.request_files {
                      mode = "save", title = "Export", file_name = "notes.md",
                      filters = { { name = "Markdown", extensions = { "md" } } },
                      tag = "export",
                    }
                    again = env.request_files {}
                    waiting = env.awaiting_files()
                    ask = false
                  end
                  return column {}
                end
                function on_event(ev)
                  if ev.event == "files" then heard = ev.files.paths[1] .. "|" .. ev.tag end
                end
            "#,
        )
        .unwrap();
        let mut core = Core::new();
        variable_frame(&mut core, &mut ext, 0.0);
        let g = ext.lua.globals();
        assert!(g.get::<bool>("asked").unwrap(), "the first ask is taken");
        assert!(
            !g.get::<bool>("again").unwrap(),
            "a second while it is out is not"
        );
        assert!(g.get::<bool>("waiting").unwrap());

        let asks = core.take_file_requests();
        assert_eq!(asks.len(), 1);
        let d = &asks[0];
        assert_eq!(d.mode, kui_core::FileDialogMode::Save);
        assert_eq!(d.file_name.as_deref(), Some("notes.md"));
        assert_eq!(d.filters[0].extensions, vec!["md".to_string()]);

        let evs = core.handle_input(InputEvent::Files(vec!["/tmp/notes.md".into()]));
        assert_eq!(evs.len(), 1);
        assert_eq!(
            evs[0].origin,
            OriginId(1),
            "the answer goes to the script that asked"
        );
        for e in &evs {
            ext.on_event(e);
        }
        let heard: String = ext.lua.globals().get("heard").unwrap();
        assert_eq!(heard, "/tmp/notes.md|export");
    }

    /// The root table's `window_events` (ADR 0048): each key reaches the
    /// core in Lua's spelling, and each event the core drives arrives as
    /// the message declared for it, the data under the event's name — the
    /// documents the OS asked the app to open (backlog F124) as `open = {
    /// paths }`, a list of strings. Fonts are declared and not driven: no
    /// headless step changes the installed set.
    #[test]
    fn the_root_table_declares_the_window_s_events() {
        let mut ext = LuaExtension::from_source(
            "window-events",
            r#"
                heard = {}
                function view(env)
                  return column {
                    window_events = { on_resize = { kind = "resized" }, on_focus = "focus",
                                      on_system_settings_change = { kind = "settings" },
                                      on_fonts = { kind = "fonts" }, on_modifiers = { kind = "mods" },
                                      on_open = { kind = "docs" } },
                    text("main"),
                  }
                end
                function on_event(ev)
                  local data = ev[ev.event]
                  local said = ""
                  if ev.event == "resize" then said = string.format("%gx%g", data.width, data.height)
                  elseif ev.event == "window" then said = data.phase
                  elseif ev.event == "system" then said = data.appearance
                  elseif ev.event == "modifiers" then said = tostring(data.shift)
                  elseif ev.event == "open" then said = #data.paths .. "|" .. data.paths[2]
                  end
                  heard[#heard + 1] = ev.event .. " " .. (ev.kind or ev.tag) .. " " .. said
                end
            "#,
        )
        .unwrap();
        let mut core = Core::new();
        let sized = |core: &mut Core, ext: &mut LuaExtension, w: f32| {
            let mut ui = core.frame(Size::new(w, 200.0), 1.0);
            ui.set_origin(OriginId(1));
            ext.view(&Instance::root(), &mut ui).unwrap();
            ui.finish();
            for e in core.take_pending_events() {
                ext.on_event(&e);
            }
        };
        sized(&mut core, &mut ext, 400.0);
        let msg = |k: &str| Value::map([("kind", Value::str(k))]);
        assert_eq!(
            core.window_events(),
            Some(
                &kui_core::WindowEvents::default()
                    .on_resize(msg("resized"))
                    .on_focus("focus")
                    .on_system_settings_change(msg("settings"))
                    .on_fonts(msg("fonts"))
                    .on_modifiers(msg("mods"))
                    .on_open(msg("docs"))
            )
        );
        sized(&mut core, &mut ext, 300.0);
        core.set_focused(false);
        for e in core.take_pending_events() {
            ext.on_event(&e);
        }
        core.set_system(kui_core::SystemEnv {
            appearance: kui_core::Appearance::Dark,
            ..Default::default()
        });
        sized(&mut core, &mut ext, 300.0);
        let mut feed = |ev| {
            for e in core.handle_input(ev) {
                ext.on_event(&e);
            }
        };
        feed(InputEvent::Modifiers(kui_core::KeyMods::from_bits(
            kui_core::KeyMods::SHIFT,
        )));
        feed(InputEvent::Open(vec![
            "/tmp/a.txt".into(),
            "/tmp/b.md".into(),
        ]));
        let heard: Vec<String> = ext
            .lua
            .globals()
            .get::<Table>("heard")
            .unwrap()
            .sequence_values()
            .collect::<mlua::Result<_>>()
            .unwrap();
        assert_eq!(
            heard,
            [
                "resize resized 300x200",
                "window focus blurred",
                "system settings dark",
                "modifiers mods true",
                "open docs 2|/tmp/b.md",
            ]
        );
        assert!(
            core.handle_input(InputEvent::Open(Vec::new())).is_empty(),
            "no documents is no event"
        );
        assert!(core.take_warnings().is_empty());
        // A key the group does not have is the script's mistake, said
        // where it is written.
        let mut bad = LuaExtension::from_source(
            "bad",
            r#"function view(env) return column { window_events = { on_resize = 1, on_blur = 2 } } end"#,
        )
        .unwrap();
        let mut ui = core.frame(Size::new(300.0, 200.0), 1.0);
        let err = bad.view(&Instance::root(), &mut ui).unwrap_err();
        assert!(err.contains("on_blur"), "{err}");
        assert!(err.contains("on_system_settings_change"), "{err}");
        ui.finish();
    }

    /// No handler, no event (ADR 0048): a root that declares no
    /// `window_events` hears none of the window's own events, and there
    /// is no root event left to fall back on.
    #[test]
    fn a_root_that_declares_no_window_events_hears_none() {
        let mut ext = LuaExtension::from_source(
            "quiet",
            r#"
                heard = 0
                function view(env) return column { text("main") } end
                function on_event(ev) heard = heard + 1 end
            "#,
        )
        .unwrap();
        let mut core = Core::new();
        variable_frame(&mut core, &mut ext, 0.0);
        assert!(core.window_events().is_none());
        core.set_focused(false);
        assert!(core.take_pending_events().is_empty());
        let mut ui = core.frame(Size::new(300.0, 100.0), 1.0);
        ext.view(&Instance::root(), &mut ui).unwrap();
        ui.finish();
        assert!(core.take_pending_events().is_empty(), "no resize");
        assert!(
            core.handle_input(InputEvent::Open(vec!["/tmp/a".into()]))
                .is_empty()
        );
        assert!(
            core.handle_input(InputEvent::Modifiers(kui_core::KeyMods::from_bits(
                kui_core::KeyMods::SHIFT
            )))
            .is_empty()
        );
    }

    /// `window_events` is the window's, read on the view's own table:
    /// on any table under it the core is still handed it, and says it is
    /// ignored (`window-events-off-root`) rather than the key passing as
    /// one the box reads. The root's own declares and does not warn.
    #[test]
    fn window_events_off_the_root_table_warns() {
        let mut ext = LuaExtension::from_source(
            "off-root",
            r#"
                function view(env)
                  return column { window_events = { on_resize = { kind = "root" } },
                    column { key = "inner", window_events = { on_resize = { kind = "inner" } } },
                  }
                end
            "#,
        )
        .unwrap();
        let mut core = Core::new();
        frame(&mut core, &mut ext);
        let warned = core.take_warnings();
        let codes: Vec<_> = warned.iter().map(|w| w.code).collect();
        assert_eq!(
            codes,
            [kui_core::diag::WINDOW_EVENTS_OFF_ROOT],
            "{warned:?}"
        );
        assert_eq!(Some(warned[0].key), core.key_of("inner"));
        assert_eq!(
            core.window_events().and_then(|w| w.on_resize.as_ref()),
            Some(&Value::map([("kind", Value::str("root"))])),
            "the root's is the one in force"
        );
    }

    /// A `windows` entry's `on_window` and `on_dismiss` (ADR 0048): the
    /// opening and closing arrive as `on_window` with `window = { phase,
    /// id }`, a popup's dismissal as `on_dismiss` with `dismiss = {
    /// reason, id }`, and neither carries the name any more — the
    /// message was declared on the entry, so it names the window.
    #[test]
    fn a_windows_entry_declares_its_lifecycle_messages() {
        use kui_core::DismissReason;
        let mut ext = LuaExtension::from_source(
            "lifecycle",
            r#"
                open = true
                heard = {}
                function view(env)
                  local declared = nil
                  if open then
                    declared = { { name = "menu", kind = "popup", width = 100, height = 80,
                                   on_window = { kind = "menu" }, on_dismiss = "menu-away" } }
                  end
                  return column { windows = declared, text("main") }
                end
                function on_event(ev)
                  local data = ev[ev.event]
                  heard[#heard + 1] = ev.event .. " " .. (ev.kind or ev.tag) .. " "
                    .. (data.phase or data.reason) .. " " .. tostring(data.id)
                    .. " " .. tostring(data.name)
                end
            "#,
        )
        .unwrap();
        let mut core = Core::new();
        let drain = |core: &mut Core, ext: &mut LuaExtension| {
            frame(core, ext);
            for e in core.take_pending_events() {
                ext.on_event(&e);
            }
        };
        drain(&mut core, &mut ext);
        let id = match core.take_window_commands().as_slice() {
            [kui_core::WindowCommand::Open { id, .. }] => *id,
            other => panic!("{other:?}"),
        };
        core.dismiss_window(id, DismissReason::Escape);
        for e in core.take_pending_events() {
            ext.on_event(&e);
        }
        ext.lua.globals().set("open", false).unwrap();
        drain(&mut core, &mut ext);
        drain(&mut core, &mut ext);
        let heard: Vec<String> = ext
            .lua
            .globals()
            .get::<Table>("heard")
            .unwrap()
            .sequence_values()
            .collect::<mlua::Result<_>>()
            .unwrap();
        let n = id.0;
        assert_eq!(
            heard,
            [
                format!("window menu opened {n} nil"),
                format!("dismiss menu-away escape {n} nil"),
                format!("window menu closed {n} nil"),
            ]
        );
    }

    /// `menu_bar { on_menu = msg }` (ADR 0048): a choice from the bar,
    /// drawn or the platform's, arrives as the bar's own message with
    /// `menu = { role, item }` in it; a bar that declares none posts
    /// nothing.
    #[test]
    fn the_menu_bar_s_choice_arrives_as_its_on_menu() {
        let mut ext = LuaExtension::from_source(
            "bar",
            r#"
                with_message = true
                heard = nil
                function view(env)
                  return column {
                    menu_bar { menu = { { label = "File", items = {
                                 { label = "New", id = "file.new" } } } },
                               on_menu = with_message and { kind = "chose" } or nil },
                    text("body"),
                  }
                end
                function on_event(ev)
                  heard = ev.event .. " " .. ev.kind .. " " .. ev.menu.item .. " " .. ev.menu.role
                end
            "#,
        )
        .unwrap();
        let mut core = Core::new();
        frame(&mut core, &mut ext);
        let evs = core.activate_menu_bar_item(0, 0);
        assert_eq!(evs.len(), 1, "{evs:?}");
        for e in &evs {
            ext.on_event(e);
        }
        let heard: String = ext.lua.globals().get("heard").unwrap();
        assert_eq!(heard, "menu chose file.new custom");
        ext.lua.globals().set("with_message", false).unwrap();
        frame(&mut core, &mut ext);
        assert!(
            core.activate_menu_bar_item(0, 0).is_empty(),
            "no on_menu, no event"
        );
        assert!(core.take_warnings().is_empty());
    }

    /// `menu_bar { standard_menus = true }` (ADR 0052): the platform's
    /// standard menus stay around the declared ones; the bar the core
    /// holds says so, and holds the declared menus alone.
    #[test]
    fn standard_menus_on_the_menu_bar() {
        let mut ext = LuaExtension::from_source(
            "bar",
            r#"
                keep = true
                function view(env)
                  return column {
                    menu_bar { menu = { { label = "Packages", items = {
                                 { label = "Find", id = "find", accel = "mod+f" } } } },
                               standard_menus = keep },
                  }
                end
            "#,
        )
        .unwrap();
        let mut core = Core::new();
        frame(&mut core, &mut ext);
        let bar = core.menu_bar().expect("declared");
        assert!(bar.standard_menus);
        assert_eq!(bar.menus.len(), 1);
        ext.lua.globals().set("keep", false).unwrap();
        frame(&mut core, &mut ext);
        assert!(!core.menu_bar().unwrap().standard_menus);
        assert!(core.take_warnings().is_empty());
    }

    /// `reveal_on_hover` (ADR 0051) is a row like any other, and a
    /// toggle's `ellipsis` (backlog F184) cuts its label to one line: the
    /// checkbox keeps to its column and is named by the whole text.
    #[test]
    fn reveal_on_hover_and_a_cut_toggle_label() {
        let mut ext = LuaExtension::from_source(
            "rows",
            r#"
                function view(env)
                  return column {
                    row { key = "row", width = 120, hoverable = true,
                      checkbox { label = "a-very-long-cookie-name-that-will-not-fit",
                                 ellipsis = true, on_click = "pick" },
                      row { key = "actions", width = 20, height = 20, bg = 0xff0000ff,
                            reveal_on_hover = true },
                    },
                  }
                end
            "#,
        )
        .unwrap();
        let mut core = Core::new();
        frame(&mut core, &mut ext);
        frame(&mut core, &mut ext);
        let node = core
            .access_tree()
            .nodes
            .iter()
            .find(|n| n.role == kui_core::Role::Checkbox)
            .cloned()
            .expect("the checkbox");
        assert_eq!(
            node.name.as_deref(),
            Some("a-very-long-cookie-name-that-will-not-fit")
        );
        assert!(node.rect.w <= 100.5, "beside the actions: {:?}", node.rect);
        assert!(core.text_cut(node.key), "cut");
        // Nothing hovered: the actions' red is drawn at 0, or not at all.
        let red = core
            .output()
            .0
            .quads
            .iter()
            .filter(|q| q.color.r == 1.0 && q.color.g == 0.0)
            .map(|q| q.color.a)
            .fold(0.0f32, f32::max);
        assert_eq!(red, 0.0, "hidden while the row is not hovered");
        assert!(core.take_warnings().is_empty());
    }

    /// The script is told what it got wrong, not handed a Lua error from
    /// inside the prelude.
    #[test]
    fn a_lua_list_names_what_it_is_missing() {
        let mut ext = LuaExtension::from_source(
            "bad",
            r#"
                function view(env)
                  ok, err = pcall(list, env, { key = "list" }, function() return 1 end,
                    function() return column {} end)
                  return column {}
                end
            "#,
        )
        .unwrap();
        let mut core = Core::new();
        variable_frame(&mut core, &mut ext, 0.0);
        let err: String = ext.lua.globals().get("err").unwrap();
        assert!(err.contains("row_heights"), "{err}");
    }

    /// A query answers for a label nothing declared, where a command says it
    /// is a name nothing answers to. The first frame of any view that slices
    /// by geometry asks before its container exists.
    #[test]
    fn a_query_answers_for_an_undeclared_label_and_a_command_refuses() {
        let mut ext = LuaExtension::from_source(
            "queries",
            r#"
                function view(env)
                  geom = env.scroll_geometry("nothing")
                  off = env.scroll_offset("nothing")
                  hovered = env.is_hovered("nothing")
                  pressed = env.is_pressed("nothing")
                  focused = env.is_focused("nothing")
                  hit = env.text_hit("nothing", 1, 1)
                  caret = env.caret_rect("nothing", 0)
                  deferred = pcall(function() env.set_scroll("nothing", 0, 0) end)
                  refused_focus = not pcall(function() env.set_focus("nothing") end)
                  return column { width = "grow", height = "grow" }
                end
            "#,
        )
        .unwrap();
        let mut core = Core::new();
        let mut ui = core.frame(Size::new(200.0, 100.0), 1.0);
        ext.view(&Instance::root(), &mut ui).unwrap();
        ui.finish();
        let g = ext.lua.globals();
        assert_eq!(g.get::<mlua::Value>("geom").unwrap(), mlua::Value::Nil);
        assert_eq!(g.get::<mlua::Value>("hit").unwrap(), mlua::Value::Nil);
        assert_eq!(g.get::<mlua::Value>("caret").unwrap(), mlua::Value::Nil);
        assert!(!g.get::<bool>("hovered").unwrap());
        assert!(!g.get::<bool>("pressed").unwrap());
        assert!(!g.get::<bool>("focused").unwrap());
        let off: Table = g.get("off").unwrap();
        assert_eq!(off.get::<f32>("y").unwrap(), 0.0);
        // `set_scroll` waits for the frame's end (backlog DX15), and the
        // frame not declaring the label is the warning that names it.
        assert!(g.get::<bool>("deferred").unwrap());
        let codes: Vec<_> = core.take_warnings().iter().map(|w| w.code).collect();
        assert_eq!(codes, ["label-without-node"]);
        assert!(g.get::<bool>("refused_focus").unwrap());
    }

    #[test]
    fn bad_script_reports_error_not_panic() {
        let mut ext = LuaExtension::from_source("bad", "function view() return 5 end").unwrap();
        let mut core = Core::new();
        let mut ui = core.frame(Size::new(100.0, 100.0), 1.0);
        assert!(ext.view(&Instance::root(), &mut ui).is_err());
    }
}
