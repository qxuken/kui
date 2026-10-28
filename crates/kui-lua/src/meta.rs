//! The view DSL described for lua-language-server: a `---@meta` file of
//! the prelude's constructors and the props a node table takes, generated
//! from kui-core's schema so it cannot fall behind it. A host writes it
//! somewhere and puts that directory on the server's `workspace.library`;
//! a script then completes `row {` with its props, and a misspelt one is a
//! diagnostic where it is typed rather than a warning at runtime.
//!
//! What the schema cannot say — the aliases' shapes, the composites'
//! types, which constructor takes nil — is written here by hand, and
//! the tests hold each of those against the parser and the prelude
//! rather than against this file (an earlier cut typed `key` as
//! `integer|string`, `{ percent = n }` as a sizing and `repeat` as a
//! field, and every one of them failed at runtime).

use std::collections::BTreeMap;
use std::fmt::Write as _;

use kui_core::schema::{self, Kind};

use crate::PRELUDE;

/// A type the file names once: its name, its doc, the LuaLS type it
/// stands for, and the strings its `string` member means — a bare
/// `string` says less than the parser takes, so the tests sample these
/// instead of any string.
pub(crate) struct Alias {
    pub(crate) name: &'static str,
    pub(crate) doc: &'static str,
    pub(crate) ty: &'static str,
    #[cfg_attr(not(test), allow(dead_code))]
    pub(crate) strings: &'static [&'static str],
}

/// `parse_color`, `length_of`, `parse_sizing` and the `Kind::Min` /
/// `Kind::Max` arm of `parse_value`, in LuaLS's words.
pub(crate) const ALIASES: &[Alias] = &[
    Alias {
        name: "kui.Color",
        doc: "A colour: `0xRRGGBBAA`, `\"#hex\"`, or a `\"$token\"` the host declared.",
        ty: "integer|string",
        strings: &["\"#336699\"", "\"#336699cc\"", "\"$accent\""],
    },
    Alias {
        name: "kui.Length",
        doc: "A length in logical px — any number where the slot is no length, such as `opacity` — or a `\"$token\"` (`\"$disabled_opacity\"` among the roles).",
        ty: "number|string",
        strings: &["\"$gap\""],
    },
    Alias {
        name: "kui.SizeArg",
        doc: "A size expression's part: px, a spelling (`\"80%\"`, `\"min(…)\"`), `{ pct = n }` or `{ px = n }`.",
        ty: "number|string|{ pct: number }|{ px: number }",
        strings: &["\"80%\""],
    },
    Alias {
        name: "kui.Size",
        doc: "A size expression (backlog F109), resolved against the parent's content box: px, `\"Npx\"`, `\"N%\"`, `\"min(…)\"`, `\"max(…)\"`, `\"clamp(MIN, TARGET, MAX)\"`, nested — or the same as data, which is never parsed: `{ pct = n }`, `{ px = n }`, `{ min = { … } }`, `{ max = { … } }`, `{ clamp = { MIN, TARGET, MAX } }` (a part a table again, or a spelling). The process keeps 65 536 distinct expressions and never lets one go: past that a new one leaves its prop at its default, with a `size-expressions-full` warning — declare one per layout, not one per frame (backlog RG93).",
        ty: "kui.SizeArg|{ min: kui.SizeArg[] }|{ max: kui.SizeArg[] }|{ clamp: kui.SizeArg[] }",
        strings: &["\"clamp(400px, 80%, 1000px)\""],
    },
    Alias {
        name: "kui.Sizing",
        doc: "A sizing: px, `\"fit\"`, `\"grow\"`, `\"N%\"`, `{ grow = n }`, `{ pct = n }` (n percent), a size expression (`kui.Size`), or a `\"$length\"`.",
        ty: "\"fit\"|\"grow\"|{ grow: number }|kui.Size",
        strings: &["\"50%\"", "\"$gap\"", "\"clamp(400px, 80%, 1000px)\""],
    },
    Alias {
        name: "kui.Min",
        doc: "A lower clamp: px, `\"fit\"`, a size expression (`kui.Size`), or a `\"$length\"`.",
        ty: "\"fit\"|kui.Size",
        strings: &["\"$gap\"", "\"50%\""],
    },
    Alias {
        name: "kui.Max",
        doc: "An upper clamp: px, a size expression (`kui.Size`), or a `\"$length\"`.",
        ty: "kui.Size",
        strings: &["\"$gap\"", "\"min(720px, 100%)\""],
    },
];

/// The words the Lua the prelude runs in reserves (5.5's `global` is
/// not among them: the vendored build keeps it a name): a prop under
/// one of these cannot be written `name = …` in a table constructor, so
/// the file gives only the spelling `schema::LUA_ALIASES` has for it.
pub(crate) const LUA_KEYWORDS: &[&str] = &[
    "and", "break", "do", "else", "elseif", "end", "false", "for", "function", "goto", "if", "in",
    "local", "nil", "not", "or", "repeat", "return", "then", "true", "until", "while",
];

/// What `uniform_list` reads off its `opts` beyond a container's
/// props: each name, whether the prelude refuses to go on without it,
/// its type and doc.
pub(crate) const UNIFORM_LIST: &[(&str, bool, &str, &str)] = &[
    (
        "key",
        true,
        "string",
        "The container's key, which its scroll geometry is read back by.",
    ),
    (
        "row_h",
        true,
        "number",
        "One row's height: the whole stride, spacing included.",
    ),
    (
        "rows",
        false,
        "number",
        "How many rows the list has; none when left out.",
    ),
    (
        "overscan",
        false,
        "number",
        "Rows built past each end of the viewport; two when left out.",
    ),
    (
        "row_props",
        false,
        "fun(i: integer): kui.Props",
        "Each row's own node's props — its click, stripe, hover; its `index` and `height` stay the list's.",
    ),
];

/// What `list` reads off its `opts` beyond a container's props, as
/// [`UNIFORM_LIST`] is for `uniform_list`.
pub(crate) const LIST: &[(&str, bool, &str, &str)] = &[
    (
        "key",
        true,
        "string",
        "The container's key, which its scroll geometry is read back by.",
    ),
    (
        "heights",
        true,
        "kui.RowHeights",
        "The heights the list slices by, made once with `row_heights` and kept.",
    ),
    (
        "overscan",
        false,
        "number",
        "Rows built past each end of the viewport; two when left out.",
    ),
];

/// `row_heights`' methods: what a script calls on the heights it keeps.
/// The three `slice_*` steps are `list`'s and left out. A row or a count
/// is a `number`, floored as `index` is (backlog F180).
pub(crate) const ROW_HEIGHTS: &[(&str, &str, &str)] = &[
    ("len", "", "integer"),
    ("set_len", "rows: number", "nil"),
    ("clear", "", "nil"),
    ("set", "i: number, h: number", "nil"),
    ("measured", "i: number", "number?"),
    ("get", "i: number", "number"),
    ("estimate", "", "number"),
    ("total", "", "number"),
    ("offset_of", "i: number", "number"),
    ("row_at", "y: number", "integer"),
];

/// A channel's methods (ADR 0065): what a script calls on one it was
/// handed or made. Rows and columns are 0-based; a column may be named.
pub(crate) const CHANNEL: &[(&str, &str, &str)] = &[
    (
        "get",
        "row: number, col: string | integer",
        "number | integer | string | boolean | nil",
    ),
    ("row", "row: number", "table<string, any>"),
    ("column", "col: string | integer", "any[]"),
    (
        "reduce",
        "col: string | integer, from: number, to: number, n: number",
        "kui.ChannelReduction",
    ),
    ("publish", "columns: table<string, any[]>", "kui.Channel"),
    (
        "append",
        "columns: table<string, any[]>, cap?: integer",
        "kui.Channel",
    ),
    (
        "filter",
        "col: string | integer, match: kui.ChannelMatch, rows?: integer[]",
        "integer[]",
    ),
    (
        "sort",
        "col: string | integer, descending?: boolean, rows?: integer[]",
        "integer[]",
    ),
    ("take", "rows: integer[]", "kui.Channel"),
    (
        "lanes",
        "start: string | integer, len?: string | integer, lane?: string | integer",
        "kui.ChannelLanes",
    ),
];

/// A class's fields: each one's name, whether it is always there, and
/// its LuaLS type.
pub(crate) type Fields = &'static [(&'static str, bool, &'static str)];

/// A shape an event's data names more than once, as a class of its own:
/// its name and its fields.
pub(crate) const SHAPES: &[(&str, Fields)] = &[
    (
        "kui.Rect",
        &[
            ("x", true, "number"),
            ("y", true, "number"),
            ("w", true, "number"),
            ("h", true, "number"),
        ],
    ),
    (
        "kui.Cell",
        &[("row", true, "integer"), ("col", true, "integer")],
    ),
    (
        "kui.LinePos",
        &[("line", true, "integer"), ("offset", true, "integer")],
    ),
    (
        "kui.RangeEnd",
        &[("index", false, "integer"), ("byte", true, "integer")],
    ),
    (
        "kui.Needs",
        &[("w", false, "number"), ("h", false, "number")],
    ),
    (
        "kui.Lazy",
        &[
            ("needs", false, "kui.Needs"),
            ("query", false, "table"),
            ("render", true, "fun(): kui.Node"),
            ("fallback", false, "fun(): kui.Node"),
            ("on_query", false, "any"),
            ("key", false, "string"),
        ],
    ),
];

/// One event's data (ADR 0047, decision 10): the class it is, the field
/// of the handler's message it rides in, the `kui.With…` class that names
/// that field alone, and its fields. `change` has
/// two, a slider's and an editor's; `submit` and `cancel` carry none and
/// have no class. Held against `schema::EVENTS` by the tests: every field
/// a row names is here, and every field here is a word of its row's doc.
pub(crate) const EVENT_DATA: &[(&str, &str, &str, Fields)] = &[
    (
        "kui.ClickEvent",
        "click",
        "kui.WithClick",
        &[
            ("cell", false, "kui.Cell"),
            ("line", false, "integer"),
            ("byte", false, "integer"),
            ("inside", false, "boolean"),
            ("clicks", false, "integer"),
        ],
    ),
    (
        "kui.DragEvent",
        "drag",
        "kui.WithDrag",
        &[
            ("phase", true, "\"start\"|\"move\"|\"end\""),
            ("x", true, "number"),
            ("y", true, "number"),
            ("dx", true, "number"),
            ("dy", true, "number"),
            ("parent", true, "kui.Rect"),
            ("cell", false, "kui.Cell"),
            ("line", false, "integer"),
            ("byte", false, "integer"),
            ("inside", false, "boolean"),
            ("clicks", false, "integer"),
        ],
    ),
    (
        "kui.KeyEvent",
        "key",
        "kui.WithKey",
        &[
            ("phase", true, "\"down\"|\"up\""),
            ("code", true, "string"),
            ("physical", true, "string"),
            ("shift", true, "boolean"),
            ("ctrl", true, "boolean"),
            ("alt", true, "boolean"),
            ("super", true, "boolean"),
            ("text", false, "string"),
            ("repeat", true, "boolean"),
            (
                "location",
                true,
                "\"standard\"|\"left\"|\"right\"|\"numpad\"",
            ),
            ("caps_lock", true, "boolean"),
            ("num_lock", true, "boolean"),
        ],
    ),
    (
        "kui.TextEvent",
        "text",
        "kui.WithText",
        &[
            ("text", true, "string"),
            ("pasted", false, "boolean"),
            ("concealed", false, "boolean"),
            ("transient", false, "boolean"),
        ],
    ),
    (
        "kui.PreeditEvent",
        "preedit",
        "kui.WithPreedit",
        &[("text", true, "string"), ("cursor", false, "integer[]")],
    ),
    (
        "kui.SelectEvent",
        "select",
        "kui.WithSelect",
        &[
            ("anchor", true, "kui.LinePos"),
            ("focus", true, "kui.LinePos"),
        ],
    ),
    (
        "kui.SelectionRangeEvent",
        "selectionrange",
        "kui.WithSelectionRange",
        &[("from", true, "kui.RangeEnd"), ("to", true, "kui.RangeEnd")],
    ),
    (
        "kui.ContextMenuEvent",
        "contextmenu",
        "kui.WithContextMenu",
        &[("x", true, "number"), ("y", true, "number")],
    ),
    (
        "kui.MenuEvent",
        "menu",
        "kui.WithMenu",
        &[("role", true, "string"), ("item", true, "any")],
    ),
    (
        "kui.ForceClickEvent",
        "forceclick",
        "kui.WithForceClick",
        &[("x", true, "number"), ("y", true, "number")],
    ),
    (
        "kui.ButtonEvent",
        "button",
        "kui.WithButton",
        &[
            ("phase", true, "\"press\"|\"move\"|\"release\""),
            ("button", true, "\"secondary\"|\"middle\"|integer"),
            ("x", true, "number"),
            ("y", true, "number"),
            ("clicks", true, "integer"),
            ("cell", false, "kui.Cell"),
            ("line", false, "integer"),
            ("byte", false, "integer"),
            ("inside", false, "boolean"),
        ],
    ),
    (
        "kui.ScrollEvent",
        "scroll",
        "kui.WithScroll",
        &[
            ("x", true, "number"),
            ("y", true, "number"),
            ("dx", true, "number"),
            ("dy", true, "number"),
            ("lines", false, "integer"),
            (
                "mods",
                false,
                "{ shift: boolean, ctrl: boolean, alt: boolean, super: boolean }",
            ),
            ("zoom", false, "number"),
        ],
    ),
    (
        "kui.FocusEvent",
        "focus",
        "kui.WithFocus",
        &[
            ("phase", true, "\"in\"|\"out\""),
            (
                "by",
                true,
                "\"pointer\"|\"keyboard\"|\"assistive\"|\"program\"",
            ),
        ],
    ),
    (
        "kui.HoverEvent",
        "hover",
        "kui.WithHover",
        &[
            ("phase", true, "\"enter\"|\"leave\""),
            ("by", true, "\"pointer\"|\"content\""),
        ],
    ),
    (
        "kui.DropEvent",
        "drop",
        "kui.WithDrop",
        &[
            ("phase", true, "\"enter\"|\"move\"|\"leave\"|\"drop\""),
            ("paths", true, "string[]"),
            ("x", true, "number"),
            ("y", true, "number"),
        ],
    ),
    (
        "kui.TypeaheadEvent",
        "typeahead",
        "kui.WithTypeahead",
        &[("text", true, "string")],
    ),
    (
        "kui.ItemKeyEvent",
        "itemkey",
        "kui.WithItemKey",
        &[
            ("phase", true, "\"down\""),
            ("code", true, "string"),
            ("physical", true, "string"),
            ("shift", true, "boolean"),
            ("ctrl", true, "boolean"),
            ("alt", true, "boolean"),
            ("super", true, "boolean"),
            ("text", false, "string"),
            ("repeat", true, "boolean"),
            (
                "location",
                true,
                "\"standard\"|\"left\"|\"right\"|\"numpad\"",
            ),
            ("caps_lock", true, "boolean"),
            ("num_lock", true, "boolean"),
            ("item", true, "integer"),
            ("index", true, "integer"),
            ("column", false, "integer"),
        ],
    ),
    (
        "kui.FilesEvent",
        "files",
        "kui.WithFiles",
        &[("paths", true, "string[]")],
    ),
    (
        "kui.RangeEvent",
        "range",
        "kui.WithRange",
        &[("first", true, "integer"), ("last", true, "integer")],
    ),
    (
        "kui.QueryEvent",
        "query",
        "kui.WithQuery",
        &[
            ("matched", true, "integer[]"),
            ("width", true, "number"),
            ("height", true, "number"),
        ],
    ),
    (
        "kui.LayoutEvent",
        "layout",
        "kui.WithLayout",
        &[
            ("x", true, "number"),
            ("y", true, "number"),
            ("w", true, "number"),
            ("h", true, "number"),
            ("parent", true, "kui.Rect"),
            ("scale", true, "number"),
        ],
    ),
    (
        "kui.SliderChangeEvent",
        "change",
        "kui.WithSliderChange",
        &[
            ("value", true, "number"),
            ("phase", true, "\"move\"|\"end\""),
        ],
    ),
    (
        "kui.EditChangeEvent",
        "change",
        "kui.WithEditChange",
        &[("text", true, "string")],
    ),
    (
        "kui.BoundaryEvent",
        "boundary",
        "kui.WithBoundary",
        &[
            (
                "key",
                true,
                "\"up\"|\"down\"|\"left\"|\"right\"|\"backspace\"|\"delete\"",
            ),
            ("edge", true, "\"start\"|\"end\""),
            ("word", true, "boolean"),
            ("doc", true, "boolean"),
            ("ctrl", true, "boolean"),
        ],
    ),
    (
        "kui.SoundEvent",
        "sound",
        "kui.WithSound",
        &[
            ("phase", true, "\"ended\"|\"refused\""),
            ("playback", true, "integer"),
        ],
    ),
    (
        "kui.CaptureEvent",
        "capture",
        "kui.WithCapture",
        &[
            ("phase", true, "\"saved\"|\"failed\""),
            ("path", true, "string"),
            ("width", true, "integer"),
            ("height", true, "integer"),
            ("frames", true, "integer"),
            ("seconds", true, "number"),
            ("error", false, "string"),
        ],
    ),
    (
        "kui.DismissEvent",
        "dismiss",
        "kui.WithDismiss",
        &[
            ("reason", true, "\"escape\"|\"outside\"|\"accelerator\""),
            ("id", false, "integer"),
        ],
    ),
    (
        "kui.ResizeEvent",
        "resize",
        "kui.WithResize",
        &[
            ("width", true, "number"),
            ("height", true, "number"),
            ("scale", true, "number"),
        ],
    ),
    (
        "kui.WindowEvent",
        "window",
        "kui.WithWindow",
        &[
            (
                "phase",
                true,
                "\"opened\"|\"closed\"|\"focused\"|\"blurred\"",
            ),
            ("id", false, "integer"),
        ],
    ),
    (
        "kui.SystemEvent",
        "system",
        "kui.WithSystem",
        &[
            ("appearance", true, "\"unknown\"|\"light\"|\"dark\""),
            ("accent", false, "integer"),
            ("motion", true, "\"unknown\"|\"full\"|\"reduced\""),
            ("locale", false, "string"),
            ("assistive", true, "\"unknown\"|\"none\"|\"listening\""),
            ("contrast", true, "\"unknown\"|\"normal\"|\"more\""),
        ],
    ),
    (
        "kui.ModifiersEvent",
        "modifiers",
        "kui.WithModifiers",
        &[
            ("shift", true, "boolean"),
            ("ctrl", true, "boolean"),
            ("alt", true, "boolean"),
            ("super", true, "boolean"),
        ],
    ),
    (
        "kui.OpenEvent",
        "open",
        "kui.WithOpen",
        &[("paths", true, "string[]")],
    ),
];

/// The root table's `window_events` keys (ADR 0048), each with the
/// `kui.With…` class of what its message arrives with: `on_fonts` carries
/// no data, so it has none. In `WindowEvents::KEYS` order, which the tests
/// hold it to.
pub(crate) const WINDOW_EVENTS: &[(&str, &str, &str)] = &[
    (
        "on_resize",
        "kui.WithResize",
        "The window changed size or scale.",
    ),
    (
        "on_focus",
        "kui.WithWindow",
        "The window gained or lost the keyboard: `phase` is `\"focused\"` or `\"blurred\"`.",
    ),
    (
        "on_system_settings_change",
        "kui.WithSystem",
        "An OS setting changed: the appearance, the accent, reduced motion, the language, or assistive technology listening.",
    ),
    (
        "on_fonts",
        "",
        "The system's installed fonts changed; the message arrives as declared.",
    ),
    (
        "on_modifiers",
        "kui.WithModifiers",
        "The physical modifier state changed.",
    ),
    (
        "on_open",
        "kui.WithOpen",
        "The OS asked the app to open documents; read on the main window's root alone.",
    ),
];

/// A `windows` entry's keys as a table: what `WindowConfig::from_value`
/// and `WindowLifecycle::from_entry` read, typed.
pub(crate) const WINDOW_ENTRY: Fields = &[
    ("name", true, "string"),
    ("kind", false, "\"normal\"|\"popup\""),
    ("width", false, "number"),
    ("height", false, "number"),
    ("activates", false, "boolean"),
    ("anchor", false, "kui.Rect"),
    ("on_window", false, "any"),
    ("on_dismiss", false, "any"),
];

/// The events one handler delivers several of, as the union of their
/// `kui.With…` classes: `on_key` hears a key, a commit, a composition
/// and a reader's selection (ADR 0047, decision 5). `update` tells them
/// apart by which field the message has, or by `ev.event`.
pub(crate) const WITH_UNIONS: &[(&str, &str, &[&str])] = &[(
    "kui.WithKeySink",
    "What an `on_key` sink hears: one of these fields, by `ev.event`.",
    &[
        "kui.WithKey",
        "kui.WithText",
        "kui.WithPreedit",
        "kui.WithSelect",
    ],
)];

/// The `---@meta` file for lua-language-server, as text.
///
/// Write it to a directory on the server's `workspace.library` and scripts
/// get completion and diagnostics for the prelude's builders and every prop.
///
/// ```no_run
/// std::fs::write("lua/kui.lua", kui_lua::luals_meta())?;
/// # Ok::<(), std::io::Error>(())
/// ```
pub fn luals_meta() -> String {
    let mut out = String::new();
    out.push_str(
        "---@meta kui\n\
         -- kui-lua's view DSL for lua-language-server, generated from\n\
         -- kui-core's schema by `kui_lua::luals_meta()`. Regenerate it\n\
         -- rather than editing it.\n\n",
    );
    for a in ALIASES {
        let _ = writeln!(out, "---{}\n---@alias {} {}", a.doc, a.name, a.ty);
    }
    out.push('\n');

    // Every prop a node table takes, the schema's and the composites'.
    out.push_str(
        "---Every prop a node table may carry; its children at 1, 2, ….\n---@class kui.Props\n",
    );
    for (name, ty, doc) in props_fields() {
        field(&mut out, name, &ty, doc);
    }
    out.push_str("---@field [integer] kui.Node|string\n\n");
    out.push_str("---A node: what a constructor returns and a view gives back.\n---@class kui.Node: kui.Props\n---@field type string\n\n");

    // An element's own props, on a class of its own.
    let class_of = class_of();
    for e in schema::ELEMENTS.iter().filter(|e| !e.lua_own.is_empty()) {
        let _ = writeln!(out, "---@class kui.{}: kui.Props", e.name);
        for own in e.lua_own {
            field(&mut out, own, "any", e.doc);
        }
        out.push('\n');
    }

    // `uniform_list`'s options: a container's props and its own.
    out.push_str("---What `uniform_list` reads off its options; every other key is the container's.\n---@class kui.UniformList: kui.Props\n");
    for (name, required, ty, doc) in UNIFORM_LIST {
        let opt = if *required { "" } else { "?" };
        let _ = writeln!(out, "---@field {name}{opt} {ty} {doc}");
    }
    out.push('\n');

    // `list`'s options, and the heights it slices by.
    out.push_str("---What `list` reads off its options; every other key is the container's.\n---@class kui.List: kui.Props\n");
    for (name, required, ty, doc) in LIST {
        let opt = if *required { "" } else { "?" };
        let _ = writeln!(out, "---@field {name}{opt} {ty} {doc}");
    }
    out.push_str(
        "\n---A variable-height list's row heights: measured where known, the mean of those elsewhere (rows are 0-based).\n---@class kui.RowHeights\n",
    );
    for (name, params, ret) in ROW_HEIGHTS {
        let _ = writeln!(
            out,
            "---@field {name} fun(self: kui.RowHeights{}{params}): {ret}",
            if params.is_empty() { "" } else { ", " }
        );
    }
    out.push_str(
        "\n---`rows` rows, none measured, each at `estimate` logical px (20 when left out) until measured.\n---@param rows number\n---@param estimate? number\n---@return kui.RowHeights\nfunction row_heights(rows, estimate) end\n\n",
    );
    out.push_str(
        "---Bulk data the core owns, read by row and column (ADR 0065): `#ch` rows, 0-based; `rev`, `id` and `names` beside.\n---@class kui.Channel\n---@field rev integer\n---@field id integer\n---@field names string[]\n",
    );
    for (name, params, ret) in CHANNEL {
        let _ = writeln!(
            out,
            "---@field {name} fun(self: kui.Channel, {params}): {ret}"
        );
    }
    out.push_str(
        "\n---A channel's intervals on lanes (ADR 0075): `#lanes` lanes, read a window at a time at a level of detail.\n---@class kui.ChannelLanes\n---@field lo number\n---@field hi number\n---@field len integer\n---@field window fun(self: kui.ChannelLanes, lane: integer, v0: number, v1: number, min: number): kui.ChannelRuns\n\n---What `lanes:window` answers: a list per field, a run per index; `row` 0-based, -1 for a strip of `n`.\n---@class kui.ChannelRuns\n---@field s number[]\n---@field e number[]\n---@field row integer[]\n---@field n integer[]\n",
    );
    out.push_str(
        "\n---What `ch:filter` keeps a row for (backlog C148): `eq`, a cell equal to it; `contains`, a string cell holding it (`case` true to match its letters' case); `min` / `max`, a number cell in that range, either end open.\n---@class kui.ChannelMatch\n---@field eq? number | integer | string | boolean\n---@field contains? string\n---@field case? boolean\n---@field min? number\n---@field max? number\n",
    );
    out.push_str(
        "\n---What `ch:reduce(col, from, to, n)` answers (backlog C95): rows `from` up to `to` in `n` buckets (one a row when fewer), each field a list with a bucket per index; NaN skipped, a bucket of only NaN NaN. Rows are the channel's, 0-based.\n---@class kui.ChannelReduction\n---@field min number[]\n---@field max number[]\n---@field first number[]\n---@field last number[]\n---@field min_row integer[]\n---@field max_row integer[]\n---@field first_row integer[]\n---@field last_row integer[]\n",
    );
    out.push_str(
        "\n---A channel of `columns`: each key a column's name, each value its rows, all of one kind and one length.\n---@param columns table<string, any[]>\n---@return kui.Channel\nfunction channel(columns) end\n\n",
    );

    // The events a view's `on_event` hears: the envelope, then each
    // event's data and the field that carries it (ADR 0047).
    out.push_str(events_meta().as_str());

    // The prelude's constructors, each with the comment above it.
    for f in prelude_functions() {
        for line in f.doc.lines() {
            let _ = writeln!(out, "---{line}");
        }
        let (params, ret) = signature(&f, &class_of);
        for (p, optional, ty) in &params {
            let opt = if *optional { "?" } else { "" };
            let _ = writeln!(out, "---@param {p}{opt} {ty}");
        }
        let _ = writeln!(
            out,
            "---@return {ret}\nfunction {}({}) end\n",
            f.name,
            f.params.join(", ")
        );
    }
    out
}

/// The class each constructor's table is: an element with props of its
/// own has `kui.<element>`, every other one `kui.Props`.
pub(crate) fn class_of() -> BTreeMap<&'static str, String> {
    let mut class_of = BTreeMap::new();
    for e in schema::ELEMENTS {
        let class = if e.lua_own.is_empty() {
            "kui.Props".to_string()
        } else {
            format!("kui.{}", e.name)
        };
        for ctor in constructors(e.lua) {
            class_of.insert(ctor, class.clone());
        }
    }
    class_of
}

/// A prelude function's parameters — each one's name, whether it takes
/// nil, and its LuaLS type — and what it returns. A table parameter is
/// optional only where the body defaults or tests it before indexing it:
/// `button(nil)` is an error in the prelude, so it is one where it is
/// typed too.
pub(crate) fn signature(
    f: &PreludeFn,
    class_of: &BTreeMap<&str, String>,
) -> (Vec<(String, bool, String)>, &'static str) {
    let element = class_of.get(f.name.as_str());
    let params = f
        .params
        .iter()
        .map(|p| {
            let (optional, ty) = match p.as_str() {
                "opts" if f.name == "uniform_list" => (false, "kui.UniformList".into()),
                "opts" if f.name == "list" => (false, "kui.List".into()),
                "t" if f.name == "lazy" => (false, "kui.Lazy".into()),
                "measure" => (false, "fun(i: integer, width: number): number".into()),
                "t" | "opts" => (
                    tolerates_nil(&f.body, p),
                    element.map_or("kui.Props", String::as_str).to_string(),
                ),
                // Spans, or anything `tostring` makes the text of.
                "s" => (false, "string|number|table".into()),
                "env" => (false, "table".into()),
                "row" => (false, "fun(i: integer): kui.Node".into()),
                "key" => (false, "string|integer".into()),
                "i" => (false, "integer".into()),
                "row_h" => (false, "number".into()),
                _ => (false, "any".into()),
            };
            (p.clone(), optional, ty)
        })
        .collect();
    // Most of the prelude builds a node; the two list readings answer.
    let ret = match f.name.as_str() {
        "reveal_row" => "boolean",
        "rows_in_view" => "integer",
        _ => "kui.Node",
    };
    (params, ret)
}

/// `ty`'s union members, split at the top level only.
pub(crate) fn members(ty: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let (mut depth, mut start) = (0, 0);
    for (i, c) in ty.char_indices() {
        match c {
            '{' | '(' => depth += 1,
            '}' | ')' => depth -= 1,
            '|' if depth == 0 => {
                out.push(ty[start..i].trim());
                start = i + 1;
            }
            _ => {}
        }
    }
    out.push(ty[start..].trim());
    out
}

/// The event classes: `kui.Event` (the envelope beside the message), the
/// shapes the data names, each event's data class, its `kui.With…` field
/// class and the unions of those. An app's message names the field it
/// carries by inheriting it — `---@class Rename: kui.Event,
/// kui.WithEditChange` — since LuaLS has no intersection type.
fn events_meta() -> String {
    let mut out = String::new();
    out.push_str(
        "---An event `on_event` hears: the message its handler declared, the event's data in the\n\
         ---field named for the event (`drag`, `change`, …), and the envelope beside them.\n\
         ---An app's message names its field by inheriting it: `---@class Pan: kui.Event, kui.WithDrag`.\n\
         ---@class kui.Event\n",
    );
    let kinds: Vec<String> = schema::EVENTS
        .iter()
        .map(|e| format!("\"{}\"", e.kind))
        .collect();
    let _ = writeln!(
        out,
        "---@field event {} What happened: the event kind, beside the message rather than in it.",
        kinds.join("|")
    );
    out.push_str(
        "---@field node_key integer The emitting node's key, what `env.edit_text` and the rest take.\n\
         ---@field window integer|kui.WindowEvent The window it came from, as `env.window.id` reads; a `window` event's data, which has the name.\n\
         ---@field instance? integer The key of the component instance the node was drawn in.\n\
         ---@field component? string That instance's full name, `ns/name`.\n\
         ---@field from? string The namespace of the plugin that raised it.\n\
         ---@field kind? string The message's own `kind`: the core never writes one.\n\
         ---@field tag? any A message that is not a table, kept under `tag`.\n\
         ---@field [string] any\n\n\
         ---`on_event`'s second argument: the envelope alone, whatever fields the message has.\n\
         ---The first argument's envelope fields give way to the message's own; these never do.\n\
         ---@class kui.Envelope\n",
    );
    let _ = writeln!(out, "---@field event {}", kinds.join("|"));
    out.push_str(
        "---@field node_key integer\n\
         ---@field window integer The window it came from, as `env.window.id` reads.\n\
         ---@field instance? integer\n\
         ---@field component? string\n\
         ---@field from? string\n\n",
    );
    let shape_line = |out: &mut String, name: &str, required: bool, ty: &str| {
        let opt = if required { "" } else { "?" };
        let _ = writeln!(out, "---@field {name}{opt} {ty}");
    };
    for (class, fields) in SHAPES {
        let _ = writeln!(out, "---@class {class}");
        for (name, required, ty) in *fields {
            shape_line(&mut out, name, *required, ty);
        }
        out.push('\n');
    }
    for (class, field, with, fields) in EVENT_DATA {
        let _ = writeln!(out, "---A `{field}` event's data.\n---@class {class}");
        for (name, required, ty) in *fields {
            shape_line(&mut out, name, *required, ty);
        }
        let _ = writeln!(
            out,
            "\n---A message that carries a `{field}` event: its `{field}` field alone.\n\
             ---@class {with}\n---@field {field} {class}\n"
        );
    }
    for (name, doc, members) in WITH_UNIONS {
        let _ = writeln!(out, "---{doc}\n---@alias {name} {}\n", members.join("|"));
    }
    // The root table's group, and a `windows` entry: where the window's
    // own events are declared (ADR 0048).
    out.push_str(
        "---The messages the window's own events arrive as (ADR 0048): `window_events` on the root table.\n\
         ---A key left out is an event the window does not hear.\n\
         ---@class kui.WindowEvents\n",
    );
    for (key, with, doc) in WINDOW_EVENTS {
        let carries = if with.is_empty() {
            String::new()
        } else {
            format!(" It arrives with `{with}`.")
        };
        let _ = writeln!(out, "---@field {key}? any {doc}{carries}");
    }
    out.push_str(
        "\n---A `windows` entry as a table: the window's name and config, and the messages its opening and\n\
         ---closing (`kui.WithWindow`) and a popup's dismissal (`kui.WithDismiss`) arrive as (ADR 0048).\n\
         ---@class kui.WindowEntry\n",
    );
    for (name, required, ty) in WINDOW_ENTRY {
        shape_line(&mut out, name, *required, ty);
    }
    out.push('\n');
    out
}

/// Every field of `kui.Props`: its Lua name, type and doc. A schema
/// row under each name a Lua table can spell it by, a composite under
/// each of its Lua names.
pub(crate) fn props_fields() -> Vec<(&'static str, String, &'static str)> {
    let mut out = Vec::new();
    for p in schema::PROPS {
        for name in lua_names(p.snake_name()) {
            out.push((name, type_of(&p.kind), p.doc));
        }
    }
    for c in schema::CUSTOM {
        for name in c.lua_names {
            out.push((*name, composite_type(name), c.doc));
        }
    }
    out
}

/// The names a Lua table writes the schema row `snake` under: the
/// aliases `schema::LUA_ALIASES` gives it, and its own name unless that
/// is a keyword no table constructor can hold.
fn lua_names(snake: &'static str) -> Vec<&'static str> {
    let mut names: Vec<&'static str> = Vec::new();
    if !LUA_KEYWORDS.contains(&snake) {
        names.push(snake);
    }
    names.extend(
        schema::LUA_ALIASES
            .iter()
            .filter(|(_, real)| *real == snake)
            .map(|(alias, _)| *alias),
    );
    names
}

/// A composite's type, by what `parse_props` (or, for the window's
/// rows, the root read in `lib.rs`) takes under that name. `any` for a
/// name this does not know, which the tests refuse.
fn composite_type(name: &str) -> String {
    match name {
        "size" => "kui.Length".into(),
        "pad" => "kui.Length|{ all?: kui.Length, x?: kui.Length, y?: kui.Length, l?: kui.Length, r?: kui.Length, t?: kui.Length, b?: kui.Length }".into(),
        "border" => "{ w?: kui.Length, color?: kui.Color }".into(),
        "float" => {
            let presets: Vec<String> = kui_core::FLOAT_PRESETS
                .iter()
                .map(|p| format!("\"{p}\""))
                .collect();
            format!("{}|table", presets.join("|"))
        }
        "key" | "tooltip" | "window_title" => "string".into(),
        "index" | "row_count" => "number".into(),
        "windows" => "(string|kui.WindowEntry)[]".into(),
        "window_events" => "kui.WindowEvents".into(),
        "option_as_alt" => {
            let names: Vec<String> = kui_core::OptionAsAlt::ALL
                .iter()
                .map(|v| format!("\"{}\"", v.name()))
                .collect();
            names.join("|")
        }
        "clip" | "scroll" | "scroll_x" | "scroll_y" | "key_focus" | "always_on_top"
        | "secure_input" | "ime_off" => "boolean".into(),
        _ => "any".into(),
    }
}

/// A field line: its name, type and the doc flattened to one line.
fn field(out: &mut String, name: &str, ty: &str, doc: &str) {
    let doc = doc.split_whitespace().collect::<Vec<_>>().join(" ");
    let _ = writeln!(out, "---@field {name}? {ty} {doc}");
}

/// A schema row's type, by the `parse_value` arm its kind takes.
fn type_of(kind: &Kind) -> String {
    match kind {
        Kind::F32 => "kui.Length".into(),
        Kind::Color => "kui.Color".into(),
        Kind::Flag => "boolean".into(),
        Kind::Enum(names) => names
            .iter()
            .map(|n| format!("\"{n}\""))
            .collect::<Vec<_>>()
            .join("|"),
        Kind::Sizing => "kui.Sizing".into(),
        Kind::Min => "kui.Min".into(),
        Kind::Max => "kui.Max".into(),
        Kind::Msg | Kind::Handler => "any".into(),
        Kind::Str => "string".into(),
        // The stock three, for an editor to offer, or any installed name.
        Kind::Family => "\"sans\"|\"serif\"|\"mono\"|string".into(),
        Kind::Resource => "integer".into(),
        Kind::Keyframes | Kind::Enter | Kind::Gradient | Kind::Rules => "table".into(),
    }
}

/// The constructors an element's `lua` column names: the identifier
/// opening each backticked form, `row { }` → `row`.
fn constructors(lua: &str) -> Vec<&str> {
    lua.split('`')
        .skip(1)
        .step_by(2)
        .filter_map(|form| {
            let end = form.find(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))?;
            let rest = form[end..].trim_start();
            (end > 0 && (rest.starts_with('{') || rest.starts_with('('))).then(|| &form[..end])
        })
        .collect()
}

/// A global function the prelude defines.
pub(crate) struct PreludeFn {
    pub(crate) name: String,
    pub(crate) params: Vec<String>,
    /// The comment block right above it.
    pub(crate) doc: String,
    /// Its lines up to the `end` that closes it at the margin.
    pub(crate) body: String,
}

/// Whether a prelude function tolerates `p` being nil: it defaults it
/// (`t = t or {}`) or tests it (`if opts then`) before indexing it.
fn tolerates_nil(body: &str, p: &str) -> bool {
    body.contains(&format!("{p} = {p} or ")) || body.contains(&format!("if {p} then"))
}

/// Every global function the prelude defines, in its order.
pub(crate) fn prelude_functions() -> Vec<PreludeFn> {
    let mut out: Vec<PreludeFn> = Vec::new();
    let mut doc: Vec<&str> = Vec::new();
    let mut open = false;
    for line in PRELUDE.lines() {
        if open {
            if line == "end" {
                open = false;
            } else if let Some(f) = out.last_mut() {
                f.body.push_str(line);
                f.body.push('\n');
            }
            continue;
        }
        if let Some(c) = line.strip_prefix("--") {
            doc.push(c.strip_prefix(' ').unwrap_or(c));
            continue;
        }
        if let Some(sig) = line.strip_prefix("function ")
            && let Some((name, rest)) = sig.split_once('(')
            && let Some((params, _)) = rest.split_once(')')
            && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
        {
            let params = params
                .split(',')
                .map(|p| p.trim().to_string())
                .filter(|p| !p.is_empty())
                .collect();
            out.push(PreludeFn {
                name: name.to_string(),
                params,
                doc: doc.join("\n"),
                body: String::new(),
            });
            open = true;
        }
        doc.clear();
    }
    out
}

/// A `{ a: T, b?: U }` shape's fields: name, whether required, type.
pub(crate) fn shape(member: &str) -> Vec<(&str, bool, &str)> {
    let inner = member.trim_start_matches('{').trim_end_matches('}');
    let mut out = Vec::new();
    let (mut depth, mut start) = (0, 0);
    let mut fields = Vec::new();
    for (i, c) in inner.char_indices() {
        match c {
            '{' | '(' => depth += 1,
            '}' | ')' => depth -= 1,
            ',' if depth == 0 => {
                fields.push(&inner[start..i]);
                start = i + 1;
            }
            _ => {}
        }
    }
    fields.push(&inner[start..]);
    for f in fields {
        let Some((name, ty)) = f.split_once(':') else {
            continue;
        };
        let name = name.trim();
        let (name, required) = match name.strip_suffix('?') {
            Some(n) => (n, false),
            None => (name, true),
        };
        out.push((name, required, ty.trim()));
    }
    out
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use mlua::{Lua, Table};

    /// Every prop the schema has and every constructor the prelude
    /// defines is in the file, each constructor typed by its element.
    #[test]
    fn the_meta_covers_the_schema_and_the_prelude() {
        let meta = luals_meta();
        assert!(meta.starts_with("---@meta kui\n"));
        for p in schema::PROPS {
            let names = lua_names(p.snake_name());
            assert!(!names.is_empty(), "{} has no Lua spelling", p.snake_name());
            for name in names {
                assert!(
                    meta.contains(&format!("---@field {name}? ")),
                    "{name} missing"
                );
            }
        }
        // `repeat` is a keyword: the row is there as `direction` only.
        assert!(meta.contains("---@field direction? "));
        assert!(!meta.contains("---@field repeat? "));
        for f in prelude_functions() {
            assert!(
                meta.contains(&format!("function {}(", f.name)),
                "{} missing",
                f.name
            );
        }
        assert!(meta.contains("---@param t? kui.Props\n---@return kui.Node\nfunction row(t) end"));
        assert!(meta.contains("---@param t kui.edit\n"), "edit's own props");
        assert!(meta.contains("---@param opts kui.UniformList\n"));
        assert!(meta.contains("---@field initial? any"));
        assert_eq!(constructors("`row { }`, `column { }`"), ["row", "column"]);
        assert_eq!(constructors("`text(\"s\", {…})`"), ["text"]);
        assert!(constructors("`size`").is_empty());
    }

    /// The event classes are the schema's events (ADR 0047): every row
    /// whose payload names a field has a data class and a `kui.With…`
    /// class for that field, no class names a field no row has, every
    /// field of a data class is a word its row's payload or doc uses, and
    /// every `kui.` type a field names is a class the file defines.
    #[test]
    fn the_event_classes_are_the_schema_s_events() {
        let meta = luals_meta();
        for e in schema::EVENTS {
            let Some(field) = e.field else { continue };
            let carries = e.payload.contains(&format!("`{field}: "));
            let classes: Vec<_> = EVENT_DATA.iter().filter(|d| d.1 == field).collect();
            assert_eq!(
                !classes.is_empty(),
                carries,
                "{field}: the payload {} data and the file has {} class",
                if carries { "carries" } else { "carries no" },
                classes.len()
            );
            let words: Vec<&str> = e
                .payload
                .split(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
                .chain(
                    e.doc
                        .split(|c: char| !(c.is_ascii_alphanumeric() || c == '_')),
                )
                .collect();
            for (class, _, with, fields) in classes {
                assert!(meta.contains(&format!("---@class {class}\n")), "{class}");
                assert!(
                    meta.contains(&format!("---@class {with}\n---@field {field} {class}\n")),
                    "{with} carries {field}"
                );
                for (name, _, ty) in *fields {
                    assert!(
                        words.contains(name),
                        "{class}.{name}: not in the {field} row"
                    );
                    for m in ty.split('|').map(|m| m.trim_end_matches("[]")) {
                        if m.starts_with("kui.") {
                            assert!(SHAPES.iter().any(|(s, _)| *s == m), "{class}.{name}: {m}");
                        }
                    }
                }
            }
        }
        for (_, field, ..) in EVENT_DATA {
            assert!(
                schema::EVENTS.iter().any(|e| e.field == Some(*field)),
                "{field} is no event's field"
            );
        }
        for (name, _, members) in WITH_UNIONS {
            for m in *members {
                assert!(EVENT_DATA.iter().any(|d| d.2 == *m), "{name}: {m}");
            }
        }
        // The `on_key` union is the set the schema says that row delivers.
        let on_key: Vec<String> = schema::EVENTS
            .iter()
            .filter(|e| e.handler == "onKey")
            .filter_map(|e| e.field)
            .map(|f| EVENT_DATA.iter().find(|d| d.1 == f).unwrap().2.to_string())
            .collect();
        assert_eq!(on_key, WITH_UNIONS[0].2);
        let kinds = schema::EVENTS.iter().map(|e| format!("\"{}\"", e.kind));
        for k in kinds {
            assert!(meta.contains(&k), "{k} is no `event`");
        }
        assert!(meta.contains("---@field event \"click\"|\"drag\"|"));
        // The root's group is the core's six keys, in its order, each
        // naming a class the file defines; an entry's kinds are the
        // core's (ADR 0048).
        let keys: Vec<&str> = kui_core::WindowEvents::KEYS.iter().map(|k| k.1).collect();
        let ours: Vec<&str> = WINDOW_EVENTS.iter().map(|k| k.0).collect();
        assert_eq!(ours, keys);
        for (key, with, _) in WINDOW_EVENTS {
            assert!(meta.contains(&format!("---@field {key}? any ")), "{key}");
            assert!(
                with.is_empty() || EVENT_DATA.iter().any(|d| d.2 == *with),
                "{key}: {with}"
            );
        }
        let kinds: Vec<String> = kui_core::WindowKind::ALL
            .iter()
            .map(|k| format!("\"{}\"", k.name()))
            .collect();
        let kind = WINDOW_ENTRY.iter().find(|f| f.0 == "kind").unwrap().2;
        assert_eq!(kind, kinds.join("|"));
        assert!(meta.contains("---@class kui.WindowEntry\n---@field name string\n"));
    }

    /// The data a script is handed is what its class says: real events,
    /// read back off the table `on_event` got, have only the fields the
    /// file gives the event's class, and the envelope beside them.
    #[test]
    fn a_delivered_event_has_the_fields_its_class_names() {
        use kui_core::{Core, Extension, InputEvent, Instance, Size, Vec2};
        let mut ext = crate::LuaExtension::from_source(
            "events",
            r#"
                heard = {}
                function view(env)
                  return column { pad = 10, gap = 6,
                    window_events = { on_resize = { kind = "rs" }, on_focus = { kind = "fo" },
                                      on_system_settings_change = { kind = "sy" },
                                      on_modifiers = { kind = "mo" }, on_open = { kind = "op" } },
                    windows = { { name = "pal", on_window = { kind = "pal" } } },
                    column { key = "pad", width = 100, height = 40, on_drag = { kind = "pan" },
                             on_hover = { kind = "hov" }, on_layout = { kind = "lay" } },
                    edit { key = "note", initial = "", width = 200, autofocus = true,
                           on_change = { kind = "note" } },
                  }
                end
                function on_event(ev)
                  local keys = {}
                  for k, v in pairs(ev) do
                    if type(v) == "table" and k == ev.event then
                      for f in pairs(v) do keys[#keys + 1] = f end
                    end
                  end
                  table.sort(keys)
                  heard[#heard + 1] = { event = ev.event, kind = ev.kind,
                                        fields = table.concat(keys, ","),
                                        node_key = ev.node_key, window = ev.window }
                end
            "#,
        )
        .unwrap();
        let mut core = Core::new();
        let sized = |core: &mut Core, ext: &mut crate::LuaExtension, w: f32| {
            let mut ui = core.frame(Size::new(w, 200.0), 1.0);
            ext.view(&Instance::root(), &mut ui).unwrap();
            ui.finish();
            for e in core.take_pending_events() {
                ext.on_event(&e);
            }
        };
        let frame = |core: &mut Core, ext: &mut crate::LuaExtension| sized(core, ext, 400.0);
        frame(&mut core, &mut ext);
        frame(&mut core, &mut ext);
        // The window's own events (ADR 0048), each as the root declared.
        sized(&mut core, &mut ext, 380.0);
        core.set_focused(false);
        for e in core.take_pending_events() {
            ext.on_event(&e);
        }
        core.set_system(kui_core::SystemEnv {
            appearance: kui_core::Appearance::Dark,
            ..Default::default()
        });
        frame(&mut core, &mut ext);
        let feed = |core: &mut Core, ext: &mut crate::LuaExtension, ev| {
            for e in core.handle_input(ev) {
                ext.on_event(&e);
            }
        };
        feed(&mut core, &mut ext, InputEvent::Text("y".into()));
        feed(
            &mut core,
            &mut ext,
            InputEvent::CursorMoved(Vec2::new(30.0, 30.0)),
        );
        feed(&mut core, &mut ext, InputEvent::mouse_down(1));
        feed(
            &mut core,
            &mut ext,
            InputEvent::CursorMoved(Vec2::new(50.0, 30.0)),
        );
        feed(&mut core, &mut ext, InputEvent::mouse_up());
        feed(
            &mut core,
            &mut ext,
            InputEvent::Modifiers(kui_core::KeyMods::from_bits(kui_core::KeyMods::SHIFT)),
        );
        feed(&mut core, &mut ext, InputEvent::Open(vec!["/tmp/a".into()]));
        let heard: mlua::Table = ext.lua().globals().get("heard").unwrap();
        let mut events = Vec::new();
        for row in heard.sequence_values::<mlua::Table>() {
            let row = row.unwrap();
            let event: String = row.get("event").unwrap();
            let kind: Option<String> = row.get("kind").unwrap();
            let fields: String = row.get("fields").unwrap();
            assert!(row.get::<i64>("node_key").is_ok(), "{event}: a node key");
            // A `window` event's data is where the envelope's window
            // would be, and wins (ADR 0048).
            if event != "window" {
                assert_eq!(row.get::<i64>("window").unwrap(), 0, "{event}: the window");
            }
            let class = EVENT_DATA
                .iter()
                .filter(|d| d.1 == event)
                .find(|d| {
                    fields
                        .split(',')
                        .all(|f| d.3.iter().any(|(name, ..)| *name == f))
                })
                .unwrap_or_else(|| panic!("{event} {{{fields}}} matches no class"));
            for (name, required, _) in class.3 {
                assert!(
                    !required || fields.split(',').any(|f| f == *name),
                    "{event}: {} needs {name}, got {{{fields}}}",
                    class.0
                );
            }
            events.push(format!("{event} {}", kind.unwrap_or_default()));
        }
        for want in [
            "layout lay",
            "change note",
            "hover hov",
            "drag pan",
            "window pal",
            "resize rs",
            "window fo",
            "system sy",
            "modifiers mo",
            "open op",
        ] {
            assert!(events.iter().any(|e| e == want), "{want} in {events:?}");
        }
    }

    /// A fresh Lua with the prelude loaded.
    fn prelude_lua() -> Lua {
        let lua = Lua::new();
        lua.load(PRELUDE).exec().unwrap();
        lua
    }

    /// `props` through the parser a view's tables go through, over a
    /// bare core: no tokens declared, so a `$name` misses (a warning, a
    /// default) rather than failing the parse.
    pub(crate) fn parses(lua: &Lua, props: &str) -> Result<(), String> {
        let t: Table = lua
            .load(format!("return {{ {props} }}"))
            .eval()
            .map_err(|e| format!("does not compile: {e}"))?;
        // One core per thread, leaked: a core is slow to make and the
        // lookup only borrows it.
        thread_local! {
            static CORE: &'static kui_core::Core = Box::leak(Box::new(kui_core::Core::new()));
        }
        let core: &'static kui_core::Core = CORE.with(|c| *c);
        let mut refs = crate::Refs::new(core.token_lookup());
        crate::parse_props(&t, false, &mut refs)
            .map(|_| ())
            .map_err(|e| e.to_string())
    }

    /// The composites the root table carries and `parse_props` never
    /// reads: the parser takes anything under these, so neither
    /// direction below says anything about them.
    pub(crate) const ROOT_ONLY: &[&str] = &[
        "window_title",
        "always_on_top",
        "secure_input",
        "option_as_alt",
        "ime_off",
        "windows",
    ];

    fn alias(name: &str) -> Option<&'static Alias> {
        ALIASES.iter().find(|a| a.name == name)
    }

    /// A Lua value of every member of `ty` the parser can be asked
    /// about: literals and shapes from the type itself, `string` as the
    /// enclosing alias means it. `any`, `table` and functions give none.
    pub(crate) fn samples(ty: &str, strings: &[&str]) -> Vec<String> {
        let mut out = Vec::new();
        for m in members(ty) {
            match m {
                "number" => out.push("12.5".into()),
                "integer" => out.push("3".into()),
                "boolean" => out.push("true".into()),
                "string" if strings.is_empty() => out.push("\"x\"".into()),
                "string" => out.extend(strings.iter().map(|s| s.to_string())),
                "any" | "table" => {}
                m if m.starts_with('"') => out.push(m.into()),
                m if m.starts_with('{') => {
                    let fields: Vec<String> = shape(m)
                        .into_iter()
                        .map(|(name, _, ty)| {
                            let v = samples(ty, &[]).into_iter().next().unwrap();
                            format!("{name} = {v}")
                        })
                        .collect();
                    out.push(format!("{{ {} }}", fields.join(", ")));
                }
                // A list: three of its first member, which is what every
                // list a size takes (`clamp`'s three, `min`'s any) admits.
                m if m.ends_with("[]") => {
                    let one = samples(&m[..m.len() - 2], &[]).into_iter().next().unwrap();
                    out.push(format!("{{ {one}, {one}, {one} }}"));
                }
                // The group, every key declared.
                "kui.WindowEvents" => {
                    let keys: Vec<String> = WINDOW_EVENTS
                        .iter()
                        .map(|(k, ..)| format!("{k} = {{ kind = \"{k}\" }}"))
                        .collect();
                    out.push(format!("{{ {} }}", keys.join(", ")));
                }
                m if m.starts_with("kui.") => {
                    let a = alias(m).unwrap_or_else(|| panic!("{m} is no alias"));
                    out.extend(samples(a.ty, a.strings));
                }
                m => panic!("no sample for {m}"),
            }
        }
        out
    }

    /// Every value the file says a prop takes, the parser takes: each
    /// member of each field's type, sampled, under the field's name —
    /// which the table constructor compiling also holds to being a name
    /// Lua can write.
    #[test]
    fn every_annotated_type_parses() {
        let lua = Lua::new();
        for c in schema::CUSTOM {
            for name in c.lua_names {
                assert_ne!(
                    composite_type(name),
                    "any",
                    "{name}: a composite the meta does not type"
                );
            }
        }
        let mut failed = Vec::new();
        for (name, ty, _) in props_fields() {
            if ROOT_ONLY.contains(&name) {
                continue;
            }
            for v in samples(&ty, &[]) {
                if let Err(e) = parses(&lua, &format!("{name} = {v}")) {
                    failed.push(format!("{name} = {v} ({ty}): {e}"));
                }
            }
        }
        assert!(failed.is_empty(), "{}", failed.join("\n"));
    }

    /// A probe value, and what a type has to have to admit it.
    pub(crate) enum Probe {
        Int,
        Float,
        Bool,
        Str(&'static str),
        Table(&'static [&'static str]),
    }

    pub(crate) const PROBES: &[(&str, Probe)] = &[
        ("3", Probe::Int),
        ("12.5", Probe::Float),
        ("false", Probe::Bool),
        ("\"x\"", Probe::Str("x")),
        ("\"$gap\"", Probe::Str("$gap")),
        ("\"#336699\"", Probe::Str("#336699")),
        ("\"50%\"", Probe::Str("50%")),
        ("\"fit\"", Probe::Str("fit")),
        ("\"grow\"", Probe::Str("grow")),
        ("\"below\"", Probe::Str("below")),
        ("{}", Probe::Table(&[])),
        ("{ grow = 2 }", Probe::Table(&["grow"])),
        ("{ pct = 50 }", Probe::Table(&["pct"])),
        ("{ percent = 50 }", Probe::Table(&["percent"])),
    ];

    /// Whether `ty` admits `p`. `integer` admits a float: the parser
    /// truncates one where a colour goes and takes a whole one where a
    /// handle does (backlog F180), and the annotation says which the
    /// value means rather than every number it survives. A count or a
    /// position is `number`, since any number is floored to one.
    fn admits(ty: &str, p: &Probe) -> bool {
        members(ty).into_iter().any(|m| match (m, p) {
            ("any", _) => true,
            ("table", Probe::Table(_)) => true,
            ("number" | "integer", Probe::Int | Probe::Float) => true,
            ("boolean", Probe::Bool) => true,
            ("string", Probe::Str(_)) => true,
            (m, Probe::Str(s)) if m.starts_with('"') => m == format!("\"{s}\""),
            (m, Probe::Table(keys)) if m.starts_with('{') => shape(m)
                .iter()
                .all(|(name, required, _)| !required || keys.contains(name)),
            ("kui.WindowEvents", Probe::Table(keys)) => keys
                .iter()
                .all(|k| WINDOW_EVENTS.iter().any(|(name, ..)| name == k)),
            (m, p) if m.starts_with("kui.") => alias(m).is_some_and(|a| admits(a.ty, p)),
            _ => false,
        })
    }

    /// Every value the parser takes for a prop, the file admits: a probe
    /// set of numbers, strings and table shapes under each field, and a
    /// probe the parser accepts that the field's type refuses is a type
    /// narrower than the truth (`size` typed `number` while a `"$token"`
    /// parses). Flags are left out — anything but `true` is off, so the
    /// parser takes every value and the annotation says what turns one
    /// on — as are the root's composites, which it never reads.
    #[test]
    fn every_parsed_value_is_admitted() {
        let lua = Lua::new();
        let mut failed = Vec::new();
        for (name, ty, _) in props_fields() {
            if ty == "boolean" || ROOT_ONLY.contains(&name) {
                continue;
            }
            for (src, p) in PROBES {
                if parses(&lua, &format!("{name} = {src}")).is_ok() && !admits(&ty, p) {
                    failed.push(format!("{name} = {src} parses; {ty} refuses it"));
                }
            }
        }
        assert!(failed.is_empty(), "{}", failed.join("\n"));
    }

    /// Each word `LUA_KEYWORDS` lists is one the Lua the prelude runs in
    /// refuses as a table key, and an ordinary name is not.
    #[test]
    fn the_keywords_are_lua_s() {
        let lua = Lua::new();
        for k in LUA_KEYWORDS {
            assert!(
                lua.load(format!("return {{ {k} = 1 }}")).exec().is_err(),
                "{k} compiles as a key"
            );
        }
        assert!(lua.load("return { direction = 1 }").exec().is_ok());
    }

    /// A table parameter the file marks optional is one the prelude
    /// really takes nil for, and one it marks required is one nil fails:
    /// every prelude function with a `t` or `opts`, called with it nil.
    #[test]
    fn a_param_is_optional_where_the_prelude_takes_nil() {
        let lua = prelude_lua();
        let meta = luals_meta();
        let mut checked = 0;
        for f in prelude_functions() {
            for p in f.params.iter().filter(|p| *p == "t" || *p == "opts") {
                let args: Vec<&str> = f
                    .params
                    .iter()
                    .map(|q| match q.as_str() {
                        q if q == p => "nil",
                        "s" => "\"x\"",
                        "env" => "{ scroll_geometry = function() end, viewport_h = 100 }",
                        "row" => "function(i) return text(tostring(i)) end",
                        _ => "nil",
                    })
                    .collect();
                let src = format!("return {}({})", f.name, args.join(", "));
                let takes_nil = lua.load(&src).exec().is_ok();
                let optional = section(&meta, &f.name).contains(&format!("---@param {p}? "));
                assert_eq!(
                    optional,
                    takes_nil,
                    "{}: `{p}` marked {} but the prelude {} nil",
                    f.name,
                    if optional { "optional" } else { "required" },
                    if takes_nil { "takes" } else { "refuses" }
                );
                checked += 1;
            }
        }
        // Both answers are exercised: `row` defaults `t`, `button` indexes it.
        assert!(section(&meta, "row").contains("---@param t? "));
        assert!(section(&meta, "button").contains("---@param t kui.button"));
        assert!(checked > 20, "{checked} parameters");
        // `s` is the text or its spans, and a number is text: `text(i)`
        // in a row builder is the common case.
        assert!(lua.load("return text(3), tooltip(3)").exec().is_ok());
        assert!(section(&meta, "text").contains("---@param s string|number|table\n"));
    }

    /// The lines the file gives `name`: from the blank line before its
    /// doc to its `function` line.
    fn section<'a>(meta: &'a str, name: &str) -> &'a str {
        let end = meta
            .find(&format!("\nfunction {name}("))
            .unwrap_or_else(|| panic!("{name} missing"));
        let start = meta[..end].rfind("\n\n").map_or(0, |i| i + 2);
        &meta[start..end]
    }

    /// `kui.UniformList` is what `uniform_list` reads: every field
    /// is an `opts.` read in its body, every read is a field or a prop,
    /// and a required field is one the prelude refuses to go on without.
    #[test]
    fn uniform_list_s_class_is_what_it_reads() {
        let f = prelude_functions()
            .into_iter()
            .find(|f| f.name == "uniform_list")
            .unwrap();
        let reads: Vec<&str> = f
            .body
            .split("opts.")
            .skip(1)
            .map(|r| {
                let end = r
                    .find(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
                    .unwrap_or(r.len());
                &r[..end]
            })
            .collect();
        let props: Vec<&str> = props_fields().into_iter().map(|(n, _, _)| n).collect();
        for (name, _, _, _) in UNIFORM_LIST {
            assert!(reads.contains(name), "{name} is never read");
        }
        for r in &reads {
            assert!(
                props.contains(r) || UNIFORM_LIST.iter().any(|(n, ..)| n == r),
                "opts.{r} is read and typed nowhere"
            );
        }
        let lua = prelude_lua();
        let env = "{ scroll_geometry = function() end, viewport_h = 100 }";
        for (name, required, _, _) in UNIFORM_LIST {
            let opts: Vec<String> = UNIFORM_LIST
                .iter()
                .filter(|(n, ..)| n != name)
                .map(|(n, _, ty, _)| {
                    let v = if *ty == "string" {
                        "\"log\""
                    } else if ty.starts_with("fun") {
                        "function() return {} end"
                    } else {
                        "4"
                    };
                    format!("{n} = {v}")
                })
                .collect();
            let src = format!(
                "return uniform_list({env}, {{ {} }}, function(i) return text(i) end)",
                opts.join(", ")
            );
            assert_eq!(
                lua.load(&src).exec().is_err(),
                *required,
                "uniform_list without {name}"
            );
        }
        // Its `pad` is the prop's every shape: it read `pad_t` and
        // `pad_y`, which no Lua table carries, and did arithmetic on a
        // table `pad`.
        for pad in ["8", "{ t = 8 }", "{ y = 8 }", "{ all = 8 }", "\"$gap\""] {
            let src = format!(
                "return uniform_list({env}, {{ key = \"log\", row_h = 4, rows = 9, pad = {pad} }}, \
                 function(i) return text(i) end)"
            );
            assert!(lua.load(&src).exec().is_ok(), "pad = {pad}");
        }
    }

    /// The same checks catch what RG33 found in the first cut.
    #[test]
    fn the_checks_catch_the_first_cut_s_types() {
        let lua = Lua::new();
        // `{ percent = n }` as a sizing, `key` as a number, `index` as a string.
        assert!(parses(&lua, "width = { percent = 50 }").is_err());
        assert!(parses(&lua, "key = 3").is_err());
        assert!(parses(&lua, "index = \"x\"").is_err());
        // `size` as a bare number while a token parses.
        assert!(parses(&lua, "size = \"$gap\"").is_ok());
        assert!(!admits("number", &Probe::Str("$gap")));
        // `repeat` as a field name.
        assert!(parses(&lua, "repeat = \"alternate\"").is_err());
        assert!(parses(&lua, "direction = \"alternate\"").is_ok());
    }
}
