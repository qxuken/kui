//! The verb table: one row per call an app or host makes (register a
//! resource, move focus, open a menu, size a window) with its spelling in
//! Rust, C, Odin, Node and Lua, or the reason a binding has none.
//!
//! A Rust caller uses the methods on `Ui`, `Core` and friends directly;
//! this table exists so the bindings stay in step with them and with each
//! other. Each binding's test checks its names against [`DOORS`], so a
//! verb added to one binding is a row here with its other cells, or a red
//! test. A [`Cell::No`] says once why a binding lacks a verb instead of
//! every reader re-deriving it: Lua is a guest with a view-time env,
//! Node's `Ctx` is a driver and its `KuiWindow` refuses input, C is both,
//! and Odin's doors are C's. The Odin column is checked by the Odin
//! binding's generator (packages/odin/gen), which reads this table through
//! `examples/rust/tools/schema-dump.rs`.
//!
//! ```rust
//! use kui_core::schema::{Cell, DOORS};
//!
//! let open_menu = DOORS.iter().find(|d| d.rust == "Ui::open_menu").expect("a verb");
//! assert!(matches!(open_menu.c, Cell::Is(_)));
//! ```

/// One binding's cell.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Cell {
    /// The binding has the verb under this name: a `kui_*` function in C;
    /// a procedure of package `kui` in Odin; a method in Node, on both
    /// classes unless prefixed `Ctx.` or `KuiWindow.`; a function on `env`
    /// in Lua. Checked by the binding's test.
    Is(&'static str),
    /// The binding has the same thing in another form — a prop, a
    /// reading of `env`, a callback, a constructor option — and the text
    /// says which.
    As(&'static str),
    /// The binding does not have it, and this is why.
    No(&'static str),
}

/// One verb.
pub struct Door {
    /// The Rust spelling: `Ui::x` (view-time), `Core::x` (a host between
    /// frames), `SharedResources::x` (the session's registry, reached as
    /// `core.resources`), `Tokens::x` (a table), `Launcher::x` (the runner).
    pub rust: &'static str,
    pub c: Cell,
    /// A procedure in the Odin binding's package `kui` (packages/odin),
    /// checked by its generator. Odin's doors are kui.h's, so its column
    /// follows C's: the same function under its name without `kui_`, the
    /// Odin spelling of what C has another way, and `No` where C has none.
    pub odin: Cell,
    pub node: Cell,
    pub lua: Cell,
    pub doc: &'static str,
}

use Cell::{As, Is, No};

/// The reason most of Lua's column is `No`: a script's env is a *reading*
/// the host hands it for one `view`, not a handle on the host.
/// It declares a tree and answers events; what it registers, drives,
/// times or reads back is the host's.
pub const GUEST: &str = "a script is a guest in the host's frame (ADR 0014): its env is the view's reading, and registering, driving, pacing and reading back are the host's";

/// The reason for Lua's `No` on the resource rows.
const NO_HANDLE: &str = "a script owns no handle: the host registers and the script names the id it was given (`image { id = }`, `font = id`, `audio { src = id }`)";

/// The reason for Odin's `No`: every Odin door is a kui.h function, so
/// what C has no door for, Odin has none for either - for C's reason, in
/// C's cell.
const ODIN_IS_C: &str =
    "Odin's doors are kui.h's (packages/odin): C has none, for the reason in C's cell";

/// The reason for Node's `No` on the renderer rows (not done
/// here).
const NEVER_PAINTS: &str = "a Node host never paints: the renderer behind `KuiWindow` is the runner's, and a headless `Ctx` has none";

pub const DOORS: &[Door] = &[
    // -- Resources ---------------------------------------------------------
    Door {
        rust: "SharedResources::add_image",
        c: Is("kui_image_add"),
        odin: Is("image_add"),
        node: Is("addImage"),
        lua: No(NO_HANDLE),
        doc: "Registers RGBA pixels and mints an id for `<image src>`.",
    },
    Door {
        rust: "Core::update_image",
        c: Is("kui_image_update"),
        odin: Is("image_update"),
        node: Is("updateImage"),
        lua: No(NO_HANDLE),
        doc: "Replaces the pixels behind a live id, keeping the id (ADR 0025).",
    },
    Door {
        rust: "Core::remove_image",
        c: Is("kui_image_remove"),
        odin: Is("image_remove"),
        node: Is("removeImage"),
        lua: No(NO_HANDLE),
        doc: "Drops an image; every window's atlas lets it go (backlog AR8).",
    },
    Door {
        rust: "Core::image_pixels",
        c: Is("kui_image_pixels"),
        odin: Is("image_pixels"),
        node: No(NEVER_PAINTS),
        lua: No(NO_HANDLE),
        doc: "The pixels behind a handle, for a renderer meeting a texture quad.",
    },
    Door {
        rust: "Core::parse_path",
        c: Is("kui_path_parse"),
        odin: Is("path_parse"),
        node: As("`d` on `<path>` is the string; the addon hands it to this parser"),
        lua: As("`d` on `path { }` is the string; the host hands it to this parser"),
        doc: "SVG path data to the flat op form a `path` draws (ADR 0040): one parser, so every binding draws the same shape.",
    },
    Door {
        rust: "Core::add_fragment",
        c: Is("kui_fragment_add"),
        odin: Is("fragment_add"),
        node: Is("addFragment"),
        lua: No(NO_HANDLE),
        doc: "Registers a WGSL function and mints an id for `<fragment src>` (ADR 0015).",
    },
    Door {
        rust: "Core::remove_fragment",
        c: Is("kui_fragment_remove"),
        odin: Is("fragment_remove"),
        node: Is("removeFragment"),
        lua: No(NO_HANDLE),
        doc: "Drops a fragment; the renderer drops its pipelines (backlog AR8).",
    },
    Door {
        rust: "Core::fragment_module_source",
        c: Is("kui_fragment_source"),
        odin: Is("fragment_source"),
        node: No(NEVER_PAINTS),
        lua: No(NO_HANDLE),
        doc: "The whole WGSL module behind a handle, which is what a renderer compiles.",
    },
    Door {
        rust: "Core::add_font_data",
        c: Is("kui_font_add"),
        odin: Is("font_add"),
        node: Is("addFont"),
        lua: No(NO_HANDLE),
        doc: "Registers a font's bytes and mints an id for `font`.",
    },
    Door {
        rust: "Core::add_system_font",
        c: Is("kui_font_add_system"),
        odin: Is("font_add_system"),
        node: Is("addSystemFont"),
        lua: No(NO_HANDLE),
        doc: "Registers an installed family by name.",
    },
    Door {
        rust: "Core::load_font_file",
        c: Is("kui_font_load_file"),
        odin: Is("font_load_file"),
        node: Is("loadFontFile"),
        lua: No(NO_HANDLE),
        doc: "Registers a font file by path.",
    },
    Door {
        rust: "Core::load_fonts_dir",
        c: Is("kui_font_load_dir"),
        odin: Is("font_load_dir"),
        node: Is("loadFontsDir"),
        lua: No(NO_HANDLE),
        doc: "Registers every font file in a directory.",
    },
    Door {
        rust: "Core::reload_system_fonts",
        c: Is("kui_font_reload_system"),
        odin: Is("font_reload_system"),
        node: Is("reloadSystemFonts"),
        lua: No(GUEST),
        doc: "Scans the system's fonts again, so a font installed while the app runs is found (the scan is otherwise once a process); returns how many faces came and went.",
    },
    Door {
        rust: "Core::set_fallback_fonts",
        c: Is("kui_font_set_fallback"),
        odin: Is("font_set_fallback"),
        node: Is("setFallbackFonts"),
        lua: No(NO_HANDLE),
        doc: "The fonts asked, in order, for a character the text's own family lacks, before the platform's fallback list (backlog F121).",
    },
    Door {
        rust: "Core::fallback_fonts",
        c: No("the list is the one the host set"),
        odin: No(ODIN_IS_C),
        node: No("the list is the one the host set"),
        lua: No(NO_HANDLE),
        doc: "The families `set_fallback_fonts` named, in order.",
    },
    Door {
        rust: "Core::remove_font",
        c: Is("kui_font_remove"),
        odin: Is("font_remove"),
        node: Is("removeFont"),
        lua: No(NO_HANDLE),
        doc: "Drops a font.",
    },
    Door {
        rust: "Core::system_font_families",
        c: Is("kui_font_families"),
        odin: Is("font_families"),
        node: Is("systemFontFamilies"),
        lua: No(NO_HANDLE),
        doc: "The installed family names `add_system_font` accepts.",
    },
    Door {
        rust: "Core::system_fonts",
        c: Is("kui_system_fonts"),
        odin: Is("system_fonts"),
        node: Is("systemFonts"),
        lua: No(NO_HANDLE),
        doc: "The same families, each with what the font database read off its faces: `monospaced` (every face fixed-pitch), `weights`, `italic` (backlog F97) — a font picker's monospaced-first list without a file loaded or a glyph shaped.",
    },
    Door {
        rust: "Core::add_sound",
        c: Is("kui_sound_add"),
        odin: Is("sound_add"),
        node: Is("addSound"),
        lua: No(NO_HANDLE),
        doc: "Registers a sound's bytes and mints an id for `<audio src>`, `clickSound` and `play`.",
    },
    Door {
        rust: "Core::remove_sound",
        c: Is("kui_sound_remove"),
        odin: Is("sound_remove"),
        node: Is("removeSound"),
        lua: No(NO_HANDLE),
        doc: "Drops a sound.",
    },
    Door {
        rust: "Core::set_text_cache_budget",
        c: Is("kui_set_text_cache_budget"),
        odin: Is("set_text_cache_budget"),
        node: Is("setTextCacheBudget"),
        lua: No(GUEST),
        doc: "The shaped-text cache's byte budget (backlog C16).",
    },
    Door {
        rust: "Core::text_cache_bytes",
        c: Is("kui_text_cache_bytes"),
        odin: Is("text_cache_bytes"),
        node: Is("textCacheBytes"),
        lua: No(GUEST),
        doc: "What the shaped-text cache holds.",
    },
    // -- Audio -------------------------------------------------------------
    Door {
        rust: "Ui::play",
        c: Is("kui_play"),
        odin: Is("play"),
        node: Is("play"),
        lua: No(
            "a script owns no sound handle, and its env is the view's: a playback started there would start again every frame — `audio { src = id }` is the declarative form, and what a script has",
        ),
        doc: "Starts a playback of a registered sound, outside any node; answers the playback id.",
    },
    Door {
        rust: "Core::stop",
        c: Is("kui_stop"),
        odin: Is("stop"),
        node: Is("stop"),
        lua: No("as `play`: a script declares `audio { }` and stops it by not declaring it"),
        doc: "Stops a playback, with an optional fade.",
    },
    Door {
        rust: "Core::set_volume",
        c: Is("kui_set_volume"),
        odin: Is("set_volume"),
        node: Is("setVolume"),
        lua: As("`audio { volume = }` applies live"),
        doc: "A playback's volume, with an optional tween.",
    },
    Door {
        rust: "Core::pause",
        c: Is("kui_pause"),
        odin: Is("pause"),
        node: Is("pause"),
        lua: As("`audio { paused = true }` applies live"),
        doc: "Pauses a playback.",
    },
    Door {
        rust: "Core::resume",
        c: Is("kui_resume"),
        odin: Is("resume"),
        node: Is("resume"),
        lua: As("`audio { paused = false }`"),
        doc: "Resumes a paused playback.",
    },
    Door {
        rust: "Core::set_master_volume",
        c: Is("kui_set_master_volume"),
        odin: Is("set_master_volume"),
        node: Is("setMasterVolume"),
        lua: No(GUEST),
        doc: "The device's master volume, with an optional tween.",
    },
    // -- Assistive ---------------------------------------------------------
    Door {
        rust: "Ui::announce",
        c: Is("kui_announce"),
        odin: Is("announce"),
        node: Is("announce"),
        lua: Is("announce"),
        doc: "Says something once with no node behind it (ADR 0001).",
    },
    Door {
        rust: "Core::take_announcements",
        c: Is("kui_take_announcements"),
        odin: Is("take_announcements"),
        node: Is("Ctx.announcements"),
        lua: No(GUEST),
        doc: "Drains what was announced, for a host bridging assistive technology; a `KuiWindow`'s bridge is the runner's.",
    },
    Door {
        rust: "Core::access_tree",
        c: Is("kui_access_tree"),
        odin: Is("access_tree"),
        node: Is("accessTree"),
        lua: No(GUEST),
        doc: "The access tree of the last finished frame (ADR 0001).",
    },
    // -- Focus -------------------------------------------------------------
    Door {
        rust: "Ui::focus",
        c: Is("kui_focus"),
        odin: Is("focus"),
        node: Is("focus"),
        lua: Is("set_focus"),
        doc: "Moves focus to a node now; an app's move stands over a modal's restore (backlog AR17). `keyFocus` is the declarative, edge-triggered form.",
    },
    Door {
        rust: "Ui::blur",
        c: As("`kui_focus(ctx, 0)`"),
        odin: As("`focus(ui, 0)`"),
        node: Is("blur"),
        lua: Is("blur"),
        doc: "Drops focus.",
    },
    Door {
        rust: "Ui::focus_next",
        c: Is("kui_focus_next"),
        odin: Is("focus_next"),
        node: Is("focusNext"),
        lua: Is("focus_next"),
        doc: "Steps the Tab ring forward (ADR 0002).",
    },
    Door {
        rust: "Ui::focus_prev",
        c: As("`kui_focus_next(ctx, false)`"),
        odin: As("`focus_next(ui, false)`"),
        node: Is("focusPrev"),
        lua: Is("focus_prev"),
        doc: "Steps the Tab ring backward.",
    },
    Door {
        rust: "Ui::focus_region",
        c: Is("kui_focus_region"),
        odin: Is("focus_region"),
        node: Is("focusRegion"),
        lua: Is("focus_region"),
        doc: "Enters a `focusRegion`'s ring, or leaves it for the main one (ADR 0022).",
    },
    Door {
        rust: "Ui::region",
        c: Is("kui_region"),
        odin: Is("region"),
        node: Is("region"),
        lua: As("`env.region`, a reading"),
        doc: "The region in effect.",
    },
    Door {
        rust: "Ui::focused",
        c: Is("kui_focused"),
        odin: Is("focused"),
        node: Is("focused"),
        lua: As("`env.focus`, a reading"),
        doc: "The focused node's key.",
    },
    Door {
        rust: "Ui::is_focused",
        c: Is("kui_is_focused"),
        odin: Is("is_focused"),
        node: Is("isFocused"),
        lua: Is("is_focused"),
        doc: "Whether a node has focus.",
    },
    Door {
        rust: "Ui::focus_visible",
        c: Is("kui_focus_visible"),
        odin: Is("focus_visible"),
        node: Is("focusVisible"),
        lua: As("`env.focus_visible`, a reading"),
        doc: "Whether focus came from the keyboard and the ring should show.",
    },
    Door {
        rust: "Ui::key_of",
        c: Is("kui_key_of"),
        odin: Is("key_of"),
        node: Is("keyOf"),
        lua: As("every query and verb takes the label itself (`key_query`)"),
        doc: "The key a label names this frame.",
    },
    Door {
        rust: "Core::label_of",
        c: No(
            "the label is the app's own word for the node, and every door names a node by it or by the key an event carried; the one reader is the devtools' inspector, in the core",
        ),
        odin: No(ODIN_IS_C),
        node: No("the same reason as C's"),
        lua: No("the same reason as C's"),
        doc: "The label a key was opened under.",
    },
    Door {
        rust: "Ui::caret_visible",
        c: Is("kui_caret_visible"),
        odin: Is("caret_visible"),
        node: Is("caretVisible"),
        lua: As("`env.caret_visible`, a reading"),
        doc: "The blink phase a custom editor draws its caret on.",
    },
    Door {
        rust: "Core::set_caret_visible",
        c: Is("kui_set_caret_visible"),
        odin: Is("set_caret_visible"),
        node: Is("setCaretVisible"),
        lua: No(GUEST),
        doc: "The host's blink clock writes the phase.",
    },
    Door {
        rust: "Core::has_caret",
        c: Is("kui_has_caret"),
        odin: Is("has_caret"),
        node: Is("hasCaret"),
        lua: No(GUEST),
        doc: "Whether anything focused draws a caret to blink — a `caretSolid` line's is not one — which arms a host's blink clock.",
    },
    Door {
        rust: "Core::caret_stamp",
        c: Is("kui_caret_stamp"),
        odin: Is("caret_stamp"),
        node: As(
            "the loop in `index.js` runs the blink from `nextDeadlineMs`; a headless `Ctx` never blinks",
        ),
        lua: No(GUEST),
        doc: "Changes when the caret moves or focus does, which re-arms the clock solid.",
    },
    // -- Queries -----------------------------------------------------------
    Door {
        rust: "Ui::is_hovered",
        c: Is("kui_is_hovered"),
        odin: Is("is_hovered"),
        node: Is("isHovered"),
        lua: Is("is_hovered"),
        doc: "Whether the pointer is over a node.",
    },
    Door {
        rust: "Ui::is_pressed",
        c: Is("kui_is_pressed"),
        odin: Is("is_pressed"),
        node: Is("isPressed"),
        lua: Is("is_pressed"),
        doc: "Whether a press started on a node and the pointer is still over it.",
    },
    Door {
        rust: "Ui::is_drop_target",
        c: Is("kui_is_drop_target"),
        odin: Is("is_drop_target"),
        node: Is("isDropTarget"),
        lua: Is("is_drop_target"),
        doc: "Whether files dragged in from the OS are over a node (ADR 0031) — for drop-dependent layout; the colour is `drop_bg`.",
    },
    Door {
        rust: "Core::drop_target",
        c: Is("kui_drop_target"),
        odin: Is("drop_target"),
        node: Is("dropTarget"),
        lua: Is("drop_target"),
        doc: "The drop zone the dragged files are over, if any — what a driver answers the OS with, and what a test reads to say a zone was found (ADR 0031, decision 5).",
    },
    Door {
        rust: "Ui::is_group_hovered",
        c: As(
            "`hoverBg` / `pressedBg` on a `hoverGroup` member paint it; the reader is what the Rust widgets ask when they paint by hand",
        ),
        odin: As("`hover_bg` / `pressed_bg` on a `hover_group` member paint it"),
        node: As("the same form as C's"),
        lua: As("the same form as C's"),
        doc: "Whether any member of a hover group is hovered (`is_group_pressed` the same for a press).",
    },
    Door {
        rust: "Core::cursor",
        c: No("the pointer's position is the driver's own fact — it injected it"),
        odin: No(ODIN_IS_C),
        node: No("the same reason as C's"),
        lua: No("the same reason as C's, one step removed"),
        doc: "Where the pointer is, in logical viewport px.",
    },
    Door {
        rust: "Core::cursor_shape",
        c: Is("kui_cursor_shape"),
        odin: Is("cursor_shape"),
        node: Is("cursorShape"),
        lua: No(GUEST),
        doc: "The pointer shape the frame asks for, which the host sets on its window.",
    },
    Door {
        rust: "Ui::layout_of",
        c: Is("kui_layout_of"),
        odin: Is("layout_of"),
        node: Is("layoutOf"),
        lua: Is("layout_of"),
        doc: "Where layout put a node last frame (backlog C26).",
    },
    Door {
        rust: "Ui::scroll_offset",
        c: Is("kui_scroll_offset"),
        odin: Is("scroll_offset"),
        node: Is("scrollOffset"),
        lua: Is("scroll_offset"),
        doc: "A scrolling node's offset.",
    },
    Door {
        rust: "Ui::scroll_geometry",
        c: Is("kui_scroll_geometry"),
        odin: Is("scroll_geometry"),
        node: Is("scrollGeometry"),
        lua: Is("scroll_geometry"),
        doc: "A scrolling node's viewport and content sizes.",
    },
    Door {
        rust: "Ui::set_scroll",
        c: Is("kui_set_scroll"),
        odin: Is("set_scroll"),
        node: Is("setScroll"),
        lua: Is("set_scroll"),
        doc: "Scrolls a node to an offset.",
    },
    Door {
        rust: "Ui::shift_scroll",
        c: Is("kui_shift_scroll"),
        odin: Is("shift_scroll"),
        node: Is("shiftScroll"),
        lua: Is("shift_scroll"),
        doc: "Moves a node's scroll by content that moved under it, with no ease: a variable-height list's anchor (backlog C46).",
    },
    Door {
        rust: "Ui::reveal",
        c: Is("kui_reveal"),
        odin: Is("reveal"),
        node: Is("reveal"),
        lua: Is("reveal"),
        doc: "Scrolls whatever encloses a node until it is in view.",
    },
    Door {
        rust: "Ui::text_hit",
        c: Is("kui_text_hit"),
        odin: Is("text_hit"),
        node: Is("textHit"),
        lua: Is("text_hit"),
        doc: "The byte and line under a point in a node's text (backlog C18).",
    },
    Door {
        rust: "Ui::caret_rect",
        c: Is("kui_caret_rect"),
        odin: Is("caret_rect"),
        node: Is("caretRect"),
        lua: Is("caret_rect"),
        doc: "The caret rect for a byte offset in a node's text.",
    },
    Door {
        rust: "Core::ime_rect",
        c: Is("kui_ime_rect"),
        odin: Is("ime_rect"),
        node: Is("imeRect"),
        lua: No(GUEST),
        doc: "Where the OS candidate window goes, which the host hands to the platform (backlog C17).",
    },
    Door {
        rust: "Ui::measure_text",
        c: Is("kui_measure_text"),
        odin: Is("measure_text"),
        node: Is("measureText"),
        lua: Is("measure_text"),
        doc: "Shapes text in a style at a width and answers its size and line count.",
    },
    Door {
        rust: "Ui::measure_rich_text",
        c: Is("kui_measure_rich_text"),
        odin: Is("measure_rich_text"),
        node: As("`measureText` takes spans too"),
        lua: As("`measure_text` takes spans too"),
        doc: "The same for spans, shaped as one paragraph.",
    },
    Door {
        rust: "Ui::edit_text",
        c: Is("kui_edit_text"),
        odin: Is("edit_text"),
        node: Is("editText"),
        lua: Is("edit_text"),
        doc: "An editor's text, by key or by label.",
    },
    Door {
        rust: "Ui::set_edit_text",
        c: Is("kui_edit_set_text"),
        odin: Is("edit_set_text"),
        node: Is("setEditText"),
        lua: Is("set_edit_text"),
        doc: "Replaces an editor's text, caret at the end.",
    },
    Door {
        rust: "Ui::set_edit_text_by_label",
        c: Is("kui_edit_set_text_label"),
        odin: Is("edit_set_text_label"),
        node: As("`setEditText` takes the label too"),
        lua: As("`set_edit_text` takes the label too"),
        doc: "The same by the label an editor's `key` declares, which reaches one the frame is about to declare (backlog AR26).",
    },
    Door {
        rust: "Core::animating",
        c: Is("kui_animating"),
        odin: Is("animating"),
        node: Is("animating"),
        lua: No(GUEST),
        doc: "Whether the last frame left a transition mid-flight, so the host draws another without waiting for input.",
    },
    Door {
        rust: "Core::owed",
        c: Is("kui_owed"),
        odin: Is("owed"),
        node: Is("owed"),
        lua: No(GUEST),
        doc: "The same by kind — a finite transition, a keyframe cycle, a departing ghost, a requested frame, an autoscroll — so a test can wait for the transitions to run out under a cycle that never ends; Node's loop has `quiet()` for that wait (backlog F64).",
    },
    Door {
        rust: "Core::set_frame_trace",
        c: Is("kui_set_frame_trace"),
        odin: Is("set_frame_trace"),
        node: Is("setFrameTrace"),
        lua: No(GUEST),
        doc: "Turns on the trace of why frames run: who holds an owed frame, and whether a frame changed what is drawn (backlog F111).",
    },
    Door {
        rust: "Core::frame_cause",
        c: Is("kui_frame_cause"),
        odin: Is("frame_cause"),
        node: Is("frameCause"),
        lua: No(GUEST),
        doc: "Why the frame being built runs: the input it answers by kind, what the driver noted, and `owed` after a frame that owed one (backlog F111).",
    },
    Door {
        rust: "Core::begin_frame_cause",
        c: No(
            "a C host's view runs between `kui_frame_begin` and `kui_frame_finish`, inside the frame it builds, so it reads that frame already",
        ),
        odin: No(ODIN_IS_C),
        node: Is("Ctx.beginFrameCause"),
        lua: No(GUEST),
        doc: "Starts the next frame's record ahead of its `begin_frame`, for a driver whose view runs before the frame it is for — Node's loop — so `frame_cause` and `owed_by` read from the view answer that frame (backlog RG81).",
    },
    Door {
        rust: "Core::note_frame_cause",
        c: Is("kui_note_frame_cause"),
        odin: Is("note_frame_cause"),
        node: No(
            "the drivers that note a reason are kui-native's, which a `KuiWindow` runs on; a `Ctx` driven by hand has nothing but the input the core already records",
        ),
        lua: No(GUEST),
        doc: "A driver adds a reason the core cannot see — a wake, a resize, a blink, a retry — to the next frame's (backlog F111).",
    },
    Door {
        rust: "Core::owed_by",
        c: No(
            "lists of named holders are strings the library would own across calls, an [out-array] struct and an ABI bump for a reading that is a debugging aid; a C host reads the kinds from `kui_owed`",
        ),
        odin: No(ODIN_IS_C),
        node: Is("owedBy"),
        lua: No(GUEST),
        doc: "Who holds the frame the last one left owed: `owed` with the nodes, slots and calling lines named (backlog F111).",
    },
    Door {
        rust: "Core::frame_unchanged",
        c: Is("kui_frame_unchanged"),
        odin: Is("frame_unchanged"),
        node: Is("frameUnchanged"),
        lua: No(GUEST),
        doc: "Whether the last finished frame drew exactly what the one before drew, traced (backlog F111).",
    },
    Door {
        rust: "Ui::request_frame",
        c: As("`animate` on a node, and `kui_animating` for the driver to read"),
        odin: As("`animate` on a node, and `animating` for the driver to read"),
        node: As("the same form as C's"),
        lua: As("the same form as C's"),
        doc: "Asks for a frame after this one; the driver paces off `animating()`.",
    },
    Door {
        rust: "Ui::modifiers",
        c: No(
            "the held modifiers ride on every key and pointer event's `mods`; the reader is what the stock editor's Shift-drag asks, inside the core",
        ),
        odin: No(ODIN_IS_C),
        node: No("the same reason as C's"),
        lua: No("the same reason as C's"),
        doc: "The modifier keys held now.",
    },
    // -- Selection ---------------------------------------------------------
    Door {
        rust: "Ui::selection_text",
        c: Is("kui_selection_text"),
        odin: Is("selection_text"),
        node: Is("selectionText"),
        lua: Is("selection_text"),
        doc: "The window's selected text — a scope's, a grid's or the focused editor's.",
    },
    Door {
        rust: "Ui::selection_html",
        c: Is("kui_selection_html"),
        odin: Is("selection_html"),
        node: Is("selectionHtml"),
        lua: Is("selection_html"),
        doc: "The same with the formatting the text declared.",
    },
    Door {
        rust: "Ui::selection_ends",
        c: Is("kui_selection_ends"),
        odin: Is("selection_ends"),
        node: Is("selectionEnds"),
        lua: Is("selection_ends"),
        doc: "A text selection's anchor and focus as row indices and bytes (ADR 0029).",
    },
    Door {
        rust: "Ui::cell_selection",
        c: Is("kui_cell_selection"),
        odin: Is("cell_selection"),
        node: Is("cellSelection"),
        lua: Is("cell_selection"),
        doc: "A `cells` grid's selection: its ends as absolute lines and columns, and whether it is a block (ADR 0017 §4).",
    },
    Door {
        rust: "Ui::select_all_in",
        c: Is("kui_select_all_in"),
        odin: Is("select_all_in"),
        node: Is("selectAllIn"),
        lua: Is("select_all_in"),
        doc: "Select All, scoped to a `selectable` node or a grid.",
    },
    Door {
        rust: "Ui::clear_selection",
        c: Is("kui_clear_selection"),
        odin: Is("clear_selection"),
        node: Is("clearSelection"),
        lua: Is("clear_selection"),
        doc: "Drops the window's selection.",
    },
    Door {
        rust: "Ui::request_copy",
        c: Is("kui_request_copy"),
        odin: Is("request_copy"),
        node: Is("requestCopy"),
        lua: Is("request_copy"),
        doc: "Asks for the selection as a copy, which may come back as a `selectionrange` question.",
    },
    Door {
        rust: "Ui::answer_selection_range",
        c: Is("kui_answer_selection_range"),
        odin: Is("answer_selection_range"),
        node: Is("answerSelectionRange"),
        lua: Is("answer_selection_range"),
        doc: "The app's answer to that question.",
    },
    Door {
        rust: "Ui::set_clipboard",
        c: Is("kui_set_clipboard"),
        odin: Is("set_clipboard"),
        node: Is("setClipboard"),
        lua: Is("set_clipboard"),
        doc: "A key sink's own Ctrl-C: posts a clipboard action for the host (backlog C33).",
    },
    Door {
        rust: "Ui::set_clipboard_secret",
        c: Is("kui_set_clipboard_secret"),
        odin: Is("set_clipboard_secret"),
        node: Is("setClipboardSecret"),
        lua: Is("set_clipboard_secret"),
        doc: "Posts a secret for the clipboard, which the host writes marked concealed and transient the way a password manager does, so no clipboard manager shows or keeps it (backlog F84).",
    },
    Door {
        rust: "Ui::request_paste",
        c: Is("kui_request_paste"),
        odin: Is("request_paste"),
        node: Is("requestPaste"),
        lua: Is("request_paste"),
        doc: "A key sink's own Ctrl-V: the clipboard comes back as a commit, marked `concealed` / `transient` when the pasteboard said so (backlog F84). One ask at a time — a second while one is unanswered is dropped.",
    },
    Door {
        rust: "Ui::awaiting_paste",
        c: Is("kui_awaiting_paste"),
        odin: Is("awaiting_paste"),
        node: Is("awaitingPaste"),
        lua: Is("awaiting_paste"),
        doc: "Whether a paste asked for is still unanswered (backlog AR34).",
    },
    Door {
        rust: "Ui::request_files",
        c: Is("kui_request_files"),
        odin: Is("request_files"),
        node: Is("requestFiles"),
        lua: Is("request_files"),
        doc: "Asks for the platform's Open, Save or folder dialog; the answer is a `files` event to whoever asked. One at a time — a second while one is out is dropped (backlog C51).",
    },
    Door {
        rust: "Ui::awaiting_files",
        c: Is("kui_awaiting_files"),
        odin: Is("awaiting_files"),
        node: Is("awaitingFiles"),
        lua: Is("awaiting_files"),
        doc: "Whether a file dialog asked for is still unanswered.",
    },
    Door {
        rust: "Core::take_file_requests",
        c: As("`kui_take_file_request`, then `kui_file_request_filter` per filter"),
        odin: As("`take_file_request`, then `file_request_filter` per filter"),
        node: Is("takeFileRequests"),
        lua: No(GUEST),
        doc: "Drains the dialog asked for, for a host that shows it itself; the runner does. The answer goes back as input (`Ctx.answerFiles`, `kui_input_files`).",
    },
    Door {
        rust: "Core::set_lookup_available",
        c: Is("kui_set_lookup_available"),
        odin: Is("set_lookup_available"),
        node: Is("setLookupAvailable"),
        lua: No(GUEST),
        doc: "Whether the host can show the platform's definition panel, which decides whether Look Up is offered.",
    },
    // -- Menus -------------------------------------------------------------
    Door {
        rust: "Ui::open_menu",
        c: Is("kui_open_menu"),
        odin: Is("open_menu"),
        node: Is("openMenu"),
        lua: Is("open_menu"),
        doc: "Opens a context menu on a node at a point.",
    },
    Door {
        rust: "Ui::close_menu",
        c: Is("kui_close_menu"),
        odin: Is("close_menu"),
        node: Is("closeMenu"),
        lua: Is("close_menu"),
        doc: "Closes it.",
    },
    Door {
        rust: "Core::take_menu_actions",
        c: Is("kui_take_menu_action"),
        odin: Is("take_menu_action"),
        node: Is("takeMenuActions"),
        lua: As(
            "a chosen row comes back as a `menu` event on the node; the clipboard actions are the host's",
        ),
        doc: "Drains what a menu (or a chord, or the standard bar) asked of the host: a clipboard write, a paste, a Look Up.",
    },
    Door {
        rust: "Core::menu",
        c: As(
            "`kui_menu_item_count` / `kui_menu_item`, one row at a time, and a submenu's by path with `kui_menu_submenu_count` / `kui_menu_item_path`",
        ),
        odin: As(
            "`menu_item_count` / `menu_item`, one row at a time, and a submenu's by path with `menu_submenu_count` / `menu_item_path`",
        ),
        node: Is("menu"),
        lua: No(GUEST),
        doc: "The open menu, for a host showing it natively.",
    },
    Door {
        rust: "Core::set_native_menus",
        c: Is("kui_set_native_menus"),
        odin: Is("set_native_menus"),
        node: Is("setNativeMenus"),
        lua: No(GUEST),
        doc: "Whether the host shows menus itself; the core then draws none.",
    },
    Door {
        rust: "Core::activate_menu_item",
        c: Is("kui_activate_menu_item"),
        odin: Is("activate_menu_item"),
        node: Is("activateMenuItem"),
        lua: No(GUEST),
        doc: "Reports that the host's own menu chose a row; a row that cannot be chosen (disabled, a separator) is refused and the menu stays open.",
    },
    Door {
        rust: "Core::activate_menu_path",
        c: Is("kui_activate_menu_path"),
        odin: Is("activate_menu_path"),
        node: Is("activateMenuPath"),
        lua: No(GUEST),
        doc: "`activate_menu_item` for a row inside a submenu, by its path through the rows' submenus (backlog F128); a row that opens a submenu is refused.",
    },
    Door {
        rust: "Core::menu_bar",
        c: As(
            "`kui_menu_bar_menu_count` / `kui_menu_bar_menu` / `kui_menu_bar_item`, one row at a time, and a submenu's by path with `kui_menu_bar_submenu_count` / `kui_menu_bar_item_path`",
        ),
        odin: As(
            "`menu_bar_menu_count` / `menu_bar_menu` / `menu_bar_item`, one row at a time, and a submenu's by path with `menu_bar_submenu_count` / `menu_bar_item_path`",
        ),
        node: Is("menuBar"),
        lua: No(GUEST),
        doc: "The declared menu bar, for a host handing it to the OS.",
    },
    Door {
        rust: "Core::set_native_menu_bar",
        c: Is("kui_set_native_menu_bar"),
        odin: Is("set_native_menu_bar"),
        node: Is("setNativeMenuBar"),
        lua: No(GUEST),
        doc: "Whether the host owns the bar; the core then draws no strip.",
    },
    Door {
        rust: "Core::activate_menu_bar_item",
        c: Is("kui_activate_menu_bar_item"),
        odin: Is("activate_menu_bar_item"),
        node: Is("activateMenuBarItem"),
        lua: No(GUEST),
        doc: "Reports that the OS bar chose a row.",
    },
    Door {
        rust: "Core::activate_menu_bar_path",
        c: Is("kui_activate_menu_bar_path"),
        odin: Is("activate_menu_bar_path"),
        node: Is("activateMenuBarPath"),
        lua: No(GUEST),
        doc: "`activate_menu_bar_item` for a row inside a submenu, by its path (backlog F128).",
    },
    // -- Windows -----------------------------------------------------------
    Door {
        rust: "Ui::window",
        c: Is("kui_window_declare"),
        odin: Is("window_declare"),
        node: As("the root's `windows` prop"),
        lua: As("the root's `windows` field"),
        doc: "Declares that a named window exists this frame (ADR 0003 step 3).",
    },
    Door {
        rust: "Core::windows",
        c: As("the ids arrive on `KUI_CMD_OPEN`; a host keeps the list it opened"),
        odin: As(
            "the ids arrive on `take_window_command`'s `.Open`; a host keeps the list it opened",
        ),
        node: Is("windows"),
        lua: No(GUEST),
        doc: "The names of the windows open now.",
    },
    Door {
        rust: "Ui::window_name",
        c: Is("kui_ctx_window_name"),
        odin: Is("ctx_window_name"),
        node: Is("windowName"),
        lua: As("`env.window.name`, a reading"),
        doc: "The name of the window this context draws.",
    },
    Door {
        rust: "Ui::set_window_size",
        c: Is("kui_set_window_size"),
        odin: Is("set_window_size"),
        node: Is("setWindowSize"),
        lua: Is("set_window_size"),
        doc: "Asks the driver to resize a window.",
    },
    Door {
        rust: "Ui::focus_window",
        c: Is("kui_focus_window"),
        odin: Is("focus_window"),
        node: Is("focusWindow"),
        lua: Is("focus_window"),
        doc: "Asks the driver to bring a window to the front.",
    },
    Door {
        rust: "Ui::window_title",
        c: Is("kui_window_title"),
        odin: Is("window_title"),
        node: As("the root's `title` prop"),
        lua: As("the root's `title` field"),
        doc: "Declares the window's title this frame.",
    },
    Door {
        rust: "Ui::always_on_top",
        c: Is("kui_set_always_on_top"),
        odin: Is("set_always_on_top"),
        node: As("the root's `alwaysOnTop` prop"),
        lua: As("the root's `always_on_top` field"),
        doc: "Declares that the window sits above every other app's this frame (backlog C30).",
    },
    Door {
        rust: "Ui::secure_input",
        c: Is("kui_set_secure_input"),
        odin: Is("set_secure_input"),
        node: As("the root's `secureInput` prop"),
        lua: As("the root's `secure_input` field"),
        doc: "Declares that this frame wants secure keyboard entry while the window has the keyboard — a password prompt (backlog F85).",
    },
    Door {
        rust: "Ui::option_as_alt",
        c: Is("kui_set_option_as_alt"),
        odin: Is("set_option_as_alt"),
        node: As("the root's `optionAsAlt` prop"),
        lua: As("the root's `option_as_alt` field"),
        doc: "Declares which Option keys act as Alt in this window on macOS, so a dead key like ⌥u arrives as `<A-u>` (backlog F113).",
    },
    Door {
        rust: "Ui::ime_off",
        c: Is("kui_set_ime_off"),
        odin: Is("set_ime_off"),
        node: As("the root's `imeOff` prop"),
        lua: As("the root's `ime_off` field"),
        doc: "Declares that this window takes the keyboard as keys, with the input method off — no composition, and on a Mac no dead keys and no press-and-hold, so a held letter repeats (backlog F125).",
    },
    Door {
        rust: "Ui::window_command",
        c: As(
            "the chrome roles (`KuiSpec.window_role`) are the door; the verb is what `widgets::window_buttons` lowers to",
        ),
        odin: As("`window` on a node (`Spec.window`, the chrome roles) is the door"),
        node: As(
            "`KuiWindow.close()` for the one command the runner takes from outside a frame; the rest are `windowRole`",
        ),
        lua: As("`window_role`"),
        doc: "Minimize, toggle-maximize, start-drag, close — what a chrome node asks for on a press.",
    },
    Door {
        rust: "Core::window_title",
        c: Is("kui_window_title_get"),
        odin: Is("window_title_get"),
        node: Is("Ctx.windowTitle"),
        lua: No(GUEST),
        doc: "What the frame declared, for a driver applying it; a `KuiWindow` applies its own.",
    },
    Door {
        rust: "Core::always_on_top",
        c: Is("kui_always_on_top_get"),
        odin: Is("always_on_top_get"),
        node: Is("Ctx.alwaysOnTop"),
        lua: As("`env.window.always_on_top`, a reading"),
        doc: "The same for the level.",
    },
    Door {
        rust: "Core::secure_input",
        c: Is("kui_secure_input_get"),
        odin: Is("secure_input_get"),
        node: Is("Ctx.secureInput"),
        lua: No(GUEST),
        doc: "The same for the secure-input ask: what a driver with its own loop reads to make the platform call; the runner makes it for a `KuiWindow` and `kui_run`.",
    },
    Door {
        rust: "Core::option_as_alt",
        c: Is("kui_option_as_alt_get"),
        odin: Is("option_as_alt_get"),
        node: Is("Ctx.optionAsAlt"),
        lua: No(GUEST),
        doc: "The same for the Option-as-Alt ask: what a driver with its own loop reads to apply it to its window; the runner applies it for a `KuiWindow` and `kui_run`.",
    },
    Door {
        rust: "Core::ime_off",
        c: Is("kui_ime_off_get"),
        odin: Is("ime_off_get"),
        node: Is("Ctx.imeOff"),
        lua: No(GUEST),
        doc: "The same for the input-method ask: what a driver with its own loop reads to apply it to its window; the runner applies it for a `KuiWindow` and `kui_run`.",
    },
    Door {
        rust: "Core::take_window_commands",
        c: Is("kui_take_window_command"),
        odin: Is("take_window_command"),
        node: Is("Ctx.windowCommands"),
        lua: No(GUEST),
        doc: "Drains what the frame asked of the driver: open, close, resize, focus, redraw.",
    },
    Door {
        rust: "Core::window_closed",
        c: Is("kui_window_closed"),
        odin: Is("window_closed"),
        node: Is("Ctx.windowClosed"),
        lua: No(GUEST),
        doc: "The driver reports a window gone.",
    },
    Door {
        rust: "Core::dismiss_window",
        c: Is("kui_window_dismissed"),
        odin: Is("window_dismissed"),
        node: Is("Ctx.windowDismissed"),
        lua: No(GUEST),
        doc: "The driver reports a popup dismissed, with why (ADR 0003 step 4).",
    },
    // -- Theme, metrics, tokens --------------------------------------------
    Door {
        rust: "Core::set_theme",
        c: Is("kui_theme_set"),
        odin: Is("theme_set"),
        node: Is("setTheme"),
        lua: No("read-only: the palette is the host's (ADR 0019)"),
        doc: "Pins a whole palette.",
    },
    Door {
        rust: "Core::set_accent",
        c: Is("kui_theme_set_accent"),
        odin: Is("theme_set_accent"),
        node: Is("setAccent"),
        lua: No("as `set_theme`"),
        doc: "Pins an accent and keeps the OS's base.",
    },
    Door {
        rust: "Ui::theme",
        c: Is("kui_theme"),
        odin: Is("theme"),
        node: Is("theme"),
        lua: As("`env.theme`, a reading"),
        doc: "The palette in effect (`THEME_ROLES`).",
    },
    Door {
        rust: "Core::set_metrics",
        c: Is("kui_metrics_set"),
        odin: Is("metrics_set"),
        node: Is("setMetrics"),
        lua: No("as `set_theme` (backlog T2)"),
        doc: "Pins the stock widgets' sizes.",
    },
    Door {
        rust: "Ui::metrics",
        c: Is("kui_metrics"),
        odin: Is("metrics"),
        node: Is("metrics"),
        lua: As("`env.metrics`, a reading"),
        doc: "The sizes in effect (`METRIC_ROLES`).",
    },
    Door {
        rust: "Ui::set_tokens",
        c: Is("kui_tokens_set"),
        odin: Is("tokens_set"),
        node: Is("setTokens"),
        lua: Is("set_tokens"),
        doc: "Declares the origin's colour and length tokens (ADR 0027).",
    },
    Door {
        rust: "Tokens::derive",
        c: Is("kui_tokens_derive"),
        odin: Is("tokens_derive"),
        node: As("a colour with `from` in `setTokens`"),
        lua: As("a colour with `from` in `set_tokens`"),
        doc: "Adds derived colours to the declared ones (ADR 0028).",
    },
    Door {
        rust: "Ui::tokens",
        c: As("`kui_token_color` / `kui_token_length`, one name at a time"),
        odin: As("`token_color` / `token_length`, one name at a time"),
        node: Is("tokens"),
        lua: As("`env.tokens`, a reading"),
        doc: "The tokens in effect, resolved for the appearance.",
    },
    Door {
        rust: "Core::tokens_declared",
        c: No(
            "a plugin declares in every `kui_ext_view` and pays the parse; a reader that lets it skip the second is one line, once a plugin asks for it",
        ),
        odin: No(ODIN_IS_C),
        node: No("an app declares once, before its loop"),
        lua: No("the `tokens` global is declared once, at load"),
        doc: "Whether an origin declared tokens.",
    },
    // -- Diagnostics and devtools ------------------------------------------
    Door {
        rust: "Core::set_diagnostics",
        c: Is("kui_set_diagnostics"),
        odin: Is("set_diagnostics"),
        node: Is("setDiagnostics"),
        lua: No(GUEST),
        doc: "Turns the per-frame checks on.",
    },
    Door {
        rust: "Core::take_warnings",
        c: Is("kui_take_warnings"),
        odin: Is("take_warnings"),
        node: Is("warnings"),
        lua: No("the host drains and the Lua runner prints"),
        doc: "Drains the warnings raised since the last call.",
    },
    Door {
        rust: "Core::warnings_raised",
        c: No(
            "the C smoke round drains `kui_take_warnings` after each frame; a non-draining reader waits for a C harness that needs one",
        ),
        odin: No(ODIN_IS_C),
        node: Is("warningsRaised"),
        lua: No(GUEST),
        doc: "The warnings raised so far, undrained, which is what an example's self-check reads (ADR 0021).",
    },
    Door {
        rust: "Core::set_devtools",
        c: Is("kui_set_devtools"),
        odin: Is("set_devtools"),
        node: Is("setDevtools"),
        lua: No(GUEST),
        doc: "Turns the devtools panel on.",
    },
    Door {
        rust: "Core::devtools",
        c: Is("kui_devtools"),
        odin: Is("devtools"),
        node: Is("devtools"),
        lua: No(GUEST),
        doc: "Whether it is on.",
    },
    Door {
        rust: "Core::set_devtools_dock",
        c: Is("kui_set_devtools_dock"),
        odin: Is("set_devtools_dock"),
        node: Is("setDevtoolsDock"),
        lua: No(GUEST),
        doc: "Where it sits.",
    },
    Door {
        rust: "Core::devtools_dock",
        c: Is("kui_devtools_dock"),
        odin: Is("devtools_dock"),
        node: Is("devtoolsDock"),
        lua: No(GUEST),
        doc: "Where it sits, read back.",
    },
    Door {
        rust: "Core::host_rect",
        c: Is("kui_host_rect"),
        odin: Is("host_rect"),
        node: Is("hostArea"),
        lua: No(GUEST),
        doc: "Where the frame laid the host out in the window, logical px: the viewport with its origin, which is what tells the app's quads from the dock's (backlog F92).",
    },
    Door {
        rust: "Core::set_devtools_theme",
        c: Is("kui_set_devtools_theme"),
        odin: Is("set_devtools_theme"),
        node: Is("setDevtoolsTheme"),
        lua: No(GUEST),
        doc: "Seeds the panel's theme override.",
    },
    Door {
        rust: "Core::set_devtools_key",
        c: Is("kui_set_devtools_key"),
        odin: Is("set_devtools_key"),
        node: Is("setDevtoolsKey"),
        lua: No(GUEST),
        doc: "Respells the chord that moves the keyboard into the panel (`Ctrl+Shift+I` by default).",
    },
    Door {
        rust: "Core::devtools_key",
        c: Is("kui_devtools_key"),
        odin: Is("devtools_key"),
        node: Is("devtoolsKey"),
        lua: No(GUEST),
        doc: "That chord, read back in its portable spelling.",
    },
    Door {
        rust: "Ui::devtools_tab",
        c: Is("kui_devtools_tab"),
        odin: Is("devtools_tab"),
        node: As("`<devtoolsTab name label slot/>`"),
        lua: As("`devtools_tab { name=, label=, slot= }`"),
        doc: "Declares a devtools tab an extension fills through the slot named (ADR 0032).",
    },
    Door {
        rust: "Ui::devtools_tab_with",
        c: Is("kui_devtools_tab_open"),
        odin: Is("devtools_tab_open"),
        node: As(
            "`<devtoolsTab name label>{() => …}</devtoolsTab>`, the function child called only while the tab is on show",
        ),
        lua: As(
            "`devtools_tab { name=, label=, view = function(env) … end }`, called only while the tab is on show",
        ),
        doc: "Declares a devtools tab the host draws itself, and draws it only while it is on show.",
    },
    Door {
        rust: "Core::devtools_shown_tab",
        c: No(
            "a C host's open answers whether the tab is on show (`kui_devtools_tab_open`); nothing encodes ahead of the core there",
        ),
        odin: No(ODIN_IS_C),
        node: Is("devtoolsShownTab"),
        lua: No("the runner's converter reads it for the script (ADR 0032, decision 3)"),
        doc: "The declared devtools tab on show, which a data binding reads once a frame to call the tab's function.",
    },
    Door {
        rust: "Core::devtools_selected",
        c: Is("kui_devtools_selected"),
        odin: Is("devtools_selected"),
        node: Is("devtoolsSelected"),
        lua: No(GUEST),
        doc: "The node the panel's tree tab has selected (ADR 0032, decision 4).",
    },
    Door {
        rust: "Core::devtools_hovered",
        c: Is("kui_devtools_hovered"),
        odin: Is("devtools_hovered"),
        node: Is("devtoolsHovered"),
        lua: No(GUEST),
        doc: "The tree row under the pointer.",
    },
    Door {
        rust: "Core::devtools_picked",
        c: Is("kui_devtools_picked"),
        odin: Is("devtools_picked"),
        node: Is("devtoolsPicked"),
        lua: No(GUEST),
        doc: "The node the picker is over.",
    },
    Door {
        rust: "Core::set_devtools_pick",
        c: Is("kui_set_devtools_pick"),
        odin: Is("set_devtools_pick"),
        node: Is("setDevtoolsPick"),
        lua: No(GUEST),
        doc: "Raises the panel's picker from outside it, or puts it away; raised from a declared tab, the pick lands in `devtools_selected` and the tab stays up.",
    },
    Door {
        rust: "Core::devtools_picking",
        c: Is("kui_devtools_picking"),
        odin: Is("devtools_picking"),
        node: Is("devtoolsPicking"),
        lua: No(GUEST),
        doc: "Whether the picker is up.",
    },
    Door {
        rust: "Core::set_devtools_selected",
        c: Is("kui_set_devtools_selected"),
        odin: Is("set_devtools_selected"),
        node: Is("setDevtoolsSelected"),
        lua: No(GUEST),
        doc: "Selects and reveals a node in the tree tab from outside the panel.",
    },
    Door {
        rust: "Core::set_devtools_tab",
        c: Is("kui_set_devtools_tab"),
        odin: Is("set_devtools_tab"),
        node: Is("setDevtoolsTab"),
        lua: No(GUEST),
        doc: "Shows the panel's tab named — one of its own, in any case, or a declared one, as declared — from outside the panel, as the strip's click does; a hidden panel comes back docked.",
    },
    Door {
        rust: "Core::devtools_current_tab",
        c: Is("kui_devtools_current_tab"),
        odin: Is("devtools_current_tab"),
        node: Is("devtoolsCurrentTab"),
        lua: No(GUEST),
        doc: "The tab the panel is on, by name.",
    },
    Door {
        rust: "Core::set_devtools_legend",
        c: Is("kui_set_devtools_legend"),
        odin: Is("set_devtools_legend"),
        node: Is("setDevtoolsLegend"),
        lua: No(GUEST),
        doc: "The key legend the panel's facts tab shows.",
    },
    Door {
        rust: "Core::set_inspect",
        c: Is("kui_set_inspect"),
        odin: Is("set_inspect"),
        node: Is("setInspect"),
        lua: No(GUEST),
        doc: "Turns the per-frame node snapshot behind `nodes` on.",
    },
    Door {
        rust: "Core::nodes",
        c: Is("kui_nodes"),
        odin: Is("nodes"),
        node: Is("nodes"),
        lua: No(GUEST),
        doc: "The last frame's nodes with what layout and the declarations made of them — a tree view's and an inspector's data.",
    },
    // -- Extensions --------------------------------------------------------
    Door {
        rust: "Ui::add_extension",
        c: Is("kui_ctx_add_extension"),
        odin: Is("ctx_add_extension"),
        node: Is("Ctx.addExtension"),
        lua: Is("add_extension"),
        doc: "Loads a plugin under a namespace; a `KuiWindow` takes its list at construction (`extensions`).",
    },
    Door {
        rust: "Launcher::extensions",
        c: As("`kui_ctx_extension_count` / `kui_ctx_extension_namespace`, one at a time"),
        odin: As("`ctx_extension_count` / `ctx_extension_namespace`, one at a time"),
        node: Is("Ctx.extensionNamespaces"),
        lua: Is("extension_namespaces"),
        doc: "The namespaces loaded.",
    },
    // -- The driver's half: what a host does to run a core ----------------
    // Lua has none of these, for the one reason `GUEST` states; Node's are
    // on `Ctx` alone because a `KuiWindow`'s driver is the runner.
    Door {
        rust: "Core::frame",
        c: As("`kui_frame_begin` … `kui_frame_finish`"),
        odin: As("`frame_begin` … `frame_finish`, or `kui.frame` around a view"),
        node: Is("Ctx.frame"),
        lua: No(GUEST),
        doc: "Runs one frame: the view, layout, the draw list; `KuiWindow.setView` is the windowed form, the runner calling it.",
    },
    Door {
        rust: "Core::output",
        c: Is("kui_draw_data"),
        odin: Is("draw_data"),
        node: Is("quads"),
        lua: No(GUEST),
        doc: "The draw list: quads, clips, fragment and texture draws (`clips`, `fragmentDraws`, `textureDraws` beside `quads` in Node) and the frame's stats.",
    },
    Door {
        rust: "Core::take_pending_events",
        c: Is("kui_poll_event"),
        odin: Is("poll_event"),
        node: Is("pollEvents"),
        lua: As("`on_event(ev)`, pushed after each frame"),
        doc: "What the frame and the input since produced, for `update`.",
    },
    Door {
        rust: "Core::set_time",
        c: Is("kui_set_time"),
        odin: Is("set_time"),
        node: Is("Ctx.setTime"),
        lua: No(GUEST),
        doc: "The clock the tweens read; a window's runner sets it from the display.",
    },
    Door {
        rust: "Ui::exit_with",
        c: Is("kui_exit_with"),
        odin: Is("exit_with"),
        node: Is("exitWith"),
        lua: Is("exit_with"),
        doc: "Names the exit a node leaves by if this frame stops declaring it, over the one it declared: a throw a button aims.",
    },
    Door {
        rust: "Ui::request_frame_at",
        c: Is("kui_request_frame_at"),
        odin: Is("request_frame_at"),
        node: Is("requestFrameAt"),
        lua: Is("request_frame_at"),
        doc: "Asks for a frame at a time on the frame clock — a toast's expiry, a sequence's beat — with nothing owed until then.",
    },
    Door {
        rust: "Core::next_frame_at",
        c: Is("kui_next_frame_at"),
        odin: Is("next_frame_at"),
        node: Is("nextFrameAt"),
        lua: No(GUEST),
        doc: "The time a driver should next draw at for a frame asked at a time; infinity in C and Odin, null in Node, for none.",
    },
    Door {
        rust: "Ui::now",
        c: Is("kui_now"),
        odin: Is("now"),
        node: Is("now"),
        lua: As("`env.now`, a reading"),
        doc: "The frame clock in seconds, the one the tweens read: what a view times its own deadlines by.",
    },
    Door {
        rust: "Core::env",
        c: As("`kui_env_set` and its four siblings, `ENV_FIELDS`' C column"),
        odin: As("`env_set` and its four siblings"),
        node: Is("Ctx.setEnv"),
        lua: No(GUEST),
        doc: "The host facts written in (the `env` field); a `KuiWindow`'s runner writes its own.",
    },
    Door {
        rust: "Ui::env",
        c: No("C is the host, so it writes the facts and has no reading (`ENV_FIELDS`)"),
        odin: No(ODIN_IS_C),
        node: Is("env"),
        lua: As("`env`, the view's argument"),
        doc: "The facts read back, `ENV_FIELDS` row for row.",
    },
    Door {
        rust: "Core::handle_input",
        c: As("`kui_input_cursor` … `kui_input_access`, one per `InputEvent`"),
        odin: As(
            "`input_cursor` … `input_access`, one per input event; `click` and `press` for a drive",
        ),
        node: As(
            "`Ctx.cursor` … `Ctx.access`, one per `InputEvent`; a `KuiWindow` refuses injection",
        ),
        lua: No(GUEST),
        doc: "Pointer, wheel, key, text, IME, assistive and OS file-drag input; a wheel gesture's latching is `scroll_gesture` (`kui_input_scroll_gesture`, `Ctx.scrollGesture`, backlog F107); `press` / `release` are a click by label (`kui_input_press`, `Ctx.press`); the file drag is `drag_files` / `drop_files` / `drag_cancel` (ADR 0031); a file dialog's answer is `answer_files` (`kui_input_files`, `Ctx.answerFiles`, backlog C51); the documents the OS asked the app to open are `InputEvent::Open` (`kui_input_open`, `Ctx.openDocuments`, backlog F124).",
    },
    Door {
        rust: "Core::modifiers",
        c: Is("kui_input_modifiers"),
        odin: Is("input_modifiers"),
        node: Is("Ctx.modifiers"),
        lua: No(GUEST),
        doc: "The modifier state, reported on its own when the OS does (backlog AR22).",
    },
    Door {
        rust: "Core::release_held_keys",
        c: Is("kui_release_held_keys"),
        odin: Is("release_held_keys"),
        node: As(
            "`Ctx.setEnv({focused: false})` releases, as losing the keyboard does for every driver (ADR 0020)",
        ),
        lua: No(GUEST),
        doc: "Lets go of every key the focused sink holds.",
    },
    Door {
        rust: "Core::set_subpixel_text",
        c: Is("kui_set_subpixel_text"),
        odin: Is("set_subpixel_text"),
        node: No(NEVER_PAINTS),
        lua: No(GUEST),
        doc: "LCD subpixel coverage for outline glyphs, for a renderer that blends per channel.",
    },
    Door {
        rust: "Core::take_audio_commands",
        c: Is("kui_take_audio_commands"),
        odin: Is("take_audio_commands"),
        node: Is("Ctx.audioCommands"),
        lua: No(GUEST),
        doc: "Drains what the frame asked of the audio device; a `KuiWindow`'s device is the runner's.",
    },
    Door {
        rust: "Core::audio_ended",
        c: Is("kui_audio_ended"),
        odin: Is("audio_ended"),
        node: Is("Ctx.audioEnded"),
        lua: No(GUEST),
        doc: "The device reports a playback over.",
    },
    Door {
        rust: "Core::audio_truncated",
        c: Is("kui_audio_truncated"),
        odin: Is("audio_truncated"),
        node: Is("Ctx.audioTruncated"),
        lua: No(GUEST),
        doc: "The device reports a stop that cut a playback short — a one-shot node's removal becomes `truncated-playback`.",
    },
    Door {
        rust: "Core::audio_refused",
        c: Is("kui_audio_refused"),
        odin: Is("audio_refused"),
        node: Is("Ctx.audioRefused"),
        lua: No(GUEST),
        doc: "The device reports a play it would not take — a `refused` sound event and `playback-refused`.",
    },
    // -- The runner's options (`Launcher` in Rust, `WindowOptions` in Node,
    // `kui_run_with` in C). One row for the set, since they are one
    // decision: what a window opens as.
    Door {
        rust: "Launcher::size",
        c: As("`width` / `height` in the `KuiRunConfig` `kui_run_with` takes"),
        odin: As("`width` / `height` in the `Run_Config` `kui.run` takes"),
        node: As("`width` / `height` in `WindowOptions`"),
        lua: No(GUEST),
        doc: "The window's opening size; `min_size` / `max_size` / `chrome` / `text_aa` / `diagnostics` / `frame_latency` / `backdrop` are the rest of the set, and each binding's form carries them all (`min_w`, `chrome`, `text_aa`, `diagnostics`, `frame_latency`, `backdrop` in C and Odin; `minWidth`, `chrome`, `textAa`, `diagnostics`, `frameLatency`, `backdrop` in Node). `Launcher::devtools` and `Launcher::core` are the two the others reach another way: `kui_set_devtools` / `setDevtools` on the context, and the context handed to `kui_run_with` *is* the core.",
    },
    Door {
        rust: "Launcher::icon",
        c: Is("kui_set_icon"),
        odin: Is("set_icon"),
        node: As("`icon` in `WindowOptions`"),
        lua: No(GUEST),
        doc: "The icon every window of the app is created with — RGBA pixels and their size — shown by Windows in the title bar, Alt-Tab and the taskbar and by X11's window manager; macOS (the bundle's `.icns`) and Wayland (the `.desktop` file's) have no window icon (backlog F86). `Launcher::icon_resource` is the Windows executable's own icon resource, which wins there — C's `resource` argument, Node's `icon.resource`. C's is a free function called before `kui_run`, for `kui_on_teardown`'s reason.",
    },
    Door {
        rust: "App::teardown",
        c: Is("kui_on_teardown"),
        odin: As("the `teardown` procedure `kui.run` takes"),
        node: Is("KuiWindow.onTeardown"),
        lua: No(GUEST),
        doc: "The window going for good — its close button, Quit from the menu or the dock, a close command on it, a pumped runner ended — heard once, before `run` returns or the process exits, with nothing drawing: the place to keep what the app would lose with the window (backlog F74, the other two hosts under RG1). On macOS a Quit ends the process from inside the loop, so this is the only thing an app runs on ⌘Q — nothing after `run`, `kui_run` or `await runWindowed(...)` does, not even `process.on('exit')`. C's is a free function called before `kui_run`, with the run's `user`, since `kui_run`'s app is three arguments and not a struct. Node's is the window's door, called from inside the pump that saw the window go; `runWindowed` registers its config's `teardown(model)` there, and `createApp`'s `app.teardown()` runs the same one for a headless drive.",
    },
];

#[cfg(test)]
mod tests {
    use super::*;

    /// A row's Rust spelling is a `pub fn` in the file its prefix names —
    /// `Ui::` in `ui.rs`, `Core::` under `runtime/`, `SharedResources::`
    /// in `session.rs`, `Tokens::` in `tokens.rs`, `Launcher::` in the
    /// `kui-native` crate, and `App::` a method of that crate's `App` trait (a
    /// trait's `fn` is public without the word) — so a renamed or
    /// removed verb is a red row and not a stale one, which is the pin
    /// Rust's column can have without reflection.
    #[test]
    fn every_rust_spelling_is_a_public_fn() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let read = |p: std::path::PathBuf| std::fs::read_to_string(&p).unwrap_or_default();
        let mut runtime = read(root.join("runtime.rs"));
        for dir in ["runtime", "runtime/devtools"] {
            for entry in std::fs::read_dir(root.join(dir)).unwrap() {
                let p = entry.unwrap().path();
                if p.extension().is_some_and(|e| e == "rs") {
                    runtime.push_str(&read(p));
                }
            }
        }
        let ui = read(root.join("ui.rs"));
        let session = read(root.join("session.rs"));
        let tokens = read(root.join("tokens.rs"));
        let launcher = read(root.join("../../kui-native/src/lib.rs"));
        // The `App` trait's body: a method of it is a callback the app
        // writes, spelled `fn name(` and public by being the trait's.
        let app_trait = launcher
            .split_once("pub trait App {")
            .map(|(_, rest)| rest.split_once("\n}\n").map_or(rest, |(body, _)| body))
            .unwrap_or_default();
        for d in DOORS {
            let (ty, name) = d.rust.split_once("::").expect(d.rust);
            let src = match ty {
                "Ui" => &ui,
                "Core" => &runtime,
                "SharedResources" => &session,
                "Tokens" => &tokens,
                "Launcher" => &launcher,
                "App" => app_trait,
                other => panic!("{}: {other} is not a prefix the table knows", d.rust),
            };
            let (public, any) = (
                format!("{}fn {name}(", if ty == "App" { "" } else { "pub " }),
                format!("fn {name}("),
            );
            assert!(
                src.contains(&public) || (ty == "Core" && name == "env"),
                "{}: no `{public}` in {ty}'s sources{}",
                d.rust,
                if src.contains(&any) {
                    " (a private fn is)"
                } else {
                    ""
                }
            );
        }
    }

    /// The table is one row per verb, and a `No` says why in a sentence
    /// rather than in a word — the reasons are what ADR 0020 said a table
    /// would be made of, and the point of building one.
    #[test]
    fn rows_are_unique_and_every_no_has_a_reason() {
        let mut seen = std::collections::BTreeSet::new();
        for d in DOORS {
            assert!(seen.insert(d.rust), "{} is two rows", d.rust);
            assert!(!d.doc.is_empty(), "{} has no doc", d.rust);
            for (binding, cell) in [("C", d.c), ("Node", d.node), ("Lua", d.lua)] {
                match cell {
                    Is(name) => assert!(
                        !name.is_empty() && !name.contains(' '),
                        "{} in {binding}: {name:?} is not a name",
                        d.rust
                    ),
                    As(how) | No(how) => assert!(
                        how.len() >= 12,
                        "{} in {binding}: {how:?} is not a reason",
                        d.rust
                    ),
                }
            }
        }
    }

    /// The C column's spellings are the header's: `kui_` and snake case.
    /// Node's are camelCase, optionally under one of the two classes;
    /// Lua's snake case. A cell in the wrong column's spelling is a pasted
    /// row.
    #[test]
    fn cells_are_spelled_in_their_bindings_case() {
        let snake = |s: &str| {
            s.bytes()
                .all(|b| b.is_ascii_lowercase() || b == b'_' || b.is_ascii_digit())
        };
        for d in DOORS {
            if let Is(c) = d.c {
                assert!(
                    c.starts_with("kui_") && snake(c),
                    "{}: C cell {c:?}",
                    d.rust
                );
            }
            if let Is(n) = d.node {
                let n = n
                    .strip_prefix("Ctx.")
                    .or_else(|| n.strip_prefix("KuiWindow."))
                    .unwrap_or(n);
                assert!(
                    !n.contains('_') && n.starts_with(|c: char| c.is_ascii_lowercase()),
                    "{}: Node cell {n:?}",
                    d.rust
                );
            }
            if let Is(l) = d.lua {
                assert!(snake(l), "{}: Lua cell {l:?}", d.rust);
            }
        }
    }
}
