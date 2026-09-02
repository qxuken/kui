//! The prop schema: the single source of truth for the per-node surface
//! every frontend lowers into. One `PROPS` row declares a prop's name, wire
//! id, value kind, apply function, and doc — and every binding interprets
//! that row instead of restating it:
//!
//! - `kui-node` parses JSON and its binary stream by kind, exports the rows
//!   as `protocol()` so the JS encoder writes by kind, and generates the TS
//!   prop types from them (`npm run gen`);
//! - `kui-lua` walks a node table and looks each key up by snake_case name
//!   (`minWidth` is `min_width` in Lua);
//! - `kui-ffi` mirrors the rows in a `repr(C)` struct — that layout has to be
//!   static, so it stays hand-written and pins itself to this table with a
//!   parity test that fails when a row has no C counterpart.
//!
//! Adding a simple prop = one row here (+ `npm run gen` for TS, + a field in
//! `KuiSpec` when the parity test says so). Composite props with real logic
//! (the pad shorthand family, border, overflow bits, float configs) and the
//! constructor-ordering specials (`dir`, `size`, `key`, `title`, `keyFocus`)
//! stay hand-written per binding; `CUSTOM` lists them by name and wire id so
//! transports still agree on identity.

use std::sync::LazyLock;

use crate::anim::{Easing, Repeat};
use crate::color::Color;
use crate::keyframes::Keyframe;
use crate::spec::{Align, FontFamily, NodeSpec, Sizing, TextStyle, TextWrap};
use crate::value::Value;
use crate::window::WindowButton;

// Wire ids, stable within a binary protocol version (see kui-node).
pub const P_DIR: u32 = 1;
pub const P_WIDTH: u32 = 2;
pub const P_HEIGHT: u32 = 3;
pub const P_MIN_W: u32 = 4;
pub const P_MAX_W: u32 = 5;
pub const P_MIN_H: u32 = 6;
pub const P_MAX_H: u32 = 7;
pub const P_PAD: u32 = 8;
pub const P_GAP: u32 = 9;
pub const P_MAIN_ALIGN: u32 = 10;
pub const P_CROSS_ALIGN: u32 = 11;
pub const P_BG: u32 = 12;
pub const P_BORDER: u32 = 13;
pub const P_RADIUS: u32 = 14;
pub const P_OVERFLOW: u32 = 15;
pub const P_FLOAT: u32 = 16;
pub const P_HOVERABLE: u32 = 17;
pub const P_ON_CLICK: u32 = 18;
pub const P_ON_DRAG: u32 = 19;
pub const P_ON_KEY: u32 = 20;
pub const P_WINDOW: u32 = 21;
pub const P_KEY_FOCUS: u32 = 22;
pub const P_CENTER: u32 = 23;
pub const P_SIZE: u32 = 24;
pub const P_LINE_HEIGHT: u32 = 25;
pub const P_COLOR: u32 = 26;
pub const P_FAMILY: u32 = 27;
pub const P_KEY: u32 = 28;
pub const P_TITLE: u32 = 29;
pub const P_TOOLTIP: u32 = 30;
pub const P_TRANSITION: u32 = 31;
pub const P_EASING: u32 = 32;
pub const P_SLIDE: u32 = 33;
pub const P_HOVER_BG: u32 = 34;
pub const P_PRESSED_BG: u32 = 35;
pub const P_HOVER_GROUP: u32 = 36;
pub const P_ON_HOVER: u32 = 37;
pub const P_RADIUS_TL: u32 = 38;
pub const P_RADIUS_TR: u32 = 39;
pub const P_RADIUS_BR: u32 = 40;
pub const P_RADIUS_BL: u32 = 41;
pub const P_FONT: u32 = 42;
pub const P_KEYFRAMES: u32 = 43;
pub const P_REPEAT: u32 = 44;
pub const P_DELAY: u32 = 45;
pub const P_WRAP: u32 = 46;
pub const P_MAX_LINES: u32 = 47;
pub const P_ELLIPSIS: u32 = 48;

pub const ALIGNS: &[&str] = &["start", "center", "end"];
pub const WINDOW_ROLES: &[&str] = &["drag", "close", "minimize", "maximize"];
pub const FAMILIES: &[&str] = &["sans", "serif", "mono"];
pub const WRAPS: &[&str] = &["word", "glyph", "none"];
pub const EASINGS: &[&str] = &[
    "easeOut",
    "linear",
    "easeIn",
    "easeInOut",
    "spring",
    "bouncy",
];

/// CSS's `animation-direction` values, in `Repeat`'s order.
pub const REPEATS: &[&str] = &["normal", "reverse", "alternate", "alternateReverse"];

pub fn repeat_idx(i: usize) -> Repeat {
    match i {
        1 => Repeat::Reverse,
        2 => Repeat::Alternate,
        3 => Repeat::AlternateReverse,
        _ => Repeat::Normal,
    }
}

pub fn easing_idx(i: usize) -> Easing {
    match i {
        1 => Easing::Linear,
        2 => Easing::EaseIn,
        3 => Easing::EaseInOut,
        4 => Easing::Spring,
        5 => Easing::Bouncy,
        _ => Easing::EaseOut,
    }
}

/// How a prop's value is parsed (per transport) and encoded (binary slots).
pub enum Kind {
    /// One number. Binary: 1 slot.
    F32,
    /// One color: `0xRRGGBBAA` number or `#hex` string. Binary: 1 slot (u32).
    Color,
    /// Marker, present-or-absent (`false` = absent). Binary: 0 slots.
    Flag,
    /// One of a closed name list. Binary: 1 slot (index).
    Enum(&'static [&'static str]),
    /// A sizing: number | "fit" | "grow" | "N%" | {grow} | {percent}.
    /// Binary: 2 slots (mode, value).
    Sizing,
    /// An arbitrary message payload (a `Value`). Binary: strref to JSON.
    Msg,
    /// A plain string (a name, not a message). Binary: strref.
    Str,
    /// A registered resource handle (a font id): the integer form of the
    /// slotmap key. JSON/binary carry it as the 16-hex string the addon
    /// hands out; Lua as an integer; C as a `uint64_t`.
    Resource,
    /// A keyframe stop list (`crate::keyframes::parse` reads the plain-data
    /// form). JSON/binary/Lua carry it like a `Msg` and parse it in the
    /// core; C passes a `KuiKeyframe` array.
    Keyframes,
}

/// Where a parsed value lands. `PropDef::target` derives from this.
pub enum Apply {
    SpecF32(fn(NodeSpec, f32) -> NodeSpec),
    SpecColor(fn(NodeSpec, Color) -> NodeSpec),
    SpecFlag(fn(NodeSpec) -> NodeSpec),
    SpecEnum(fn(NodeSpec, usize) -> NodeSpec),
    SpecSizing(fn(NodeSpec, Sizing) -> NodeSpec),
    SpecMsg(fn(NodeSpec, Value) -> NodeSpec),
    SpecStr(fn(NodeSpec, &str) -> NodeSpec),
    SpecKeyframes(fn(NodeSpec, Vec<Keyframe>) -> NodeSpec),
    StyleF32(fn(TextStyle, f32) -> TextStyle),
    StyleColor(fn(TextStyle, Color) -> TextStyle),
    StyleEnum(fn(TextStyle, usize) -> TextStyle),
    StyleFlag(fn(TextStyle) -> TextStyle),
    StyleResource(fn(TextStyle, u64) -> TextStyle),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Target {
    /// Lands on the node's `NodeSpec` (containers, edits, images).
    Spec,
    /// Lands on a `TextStyle` (text, rich text, edits).
    Style,
}

pub struct PropDef {
    /// Canonical camelCase name (the JSX prop). Lua uses `snake_name()`.
    pub name: &'static str,
    pub id: u32,
    pub kind: Kind,
    pub apply: Apply,
    pub doc: &'static str,
}

impl PropDef {
    pub fn target(&self) -> Target {
        match self.apply {
            Apply::StyleF32(_)
            | Apply::StyleColor(_)
            | Apply::StyleEnum(_)
            | Apply::StyleFlag(_)
            | Apply::StyleResource(_) => Target::Style,
            _ => Target::Spec,
        }
    }

    /// The Lua-side name: `minWidth` → `min_width`.
    pub fn snake_name(&self) -> &'static str {
        SNAKE_NAMES[self.index()]
    }

    fn index(&self) -> usize {
        // By name, not by address: `PROPS` is a const, so another crate
        // iterating it sees its own copy of the rows.
        PROPS
            .iter()
            .position(|d| d.name == self.name)
            .expect("PropDef not from PROPS")
    }
}

pub fn align_idx(i: usize) -> Align {
    match i {
        1 => Align::Center,
        2 => Align::End,
        _ => Align::Start,
    }
}

/// Binary sizing decode: (mode, value) → Sizing.
pub fn sizing_num(mode: u32, value: f64) -> Sizing {
    match mode {
        1 => Sizing::Fit,
        2 => Sizing::Grow(value as f32),
        3 => Sizing::Percent(value as f32),
        _ => Sizing::Fixed(value as f32),
    }
}

pub const PROPS: &[PropDef] = &[
    PropDef {
        name: "width",
        id: P_WIDTH,
        kind: Kind::Sizing,
        apply: Apply::SpecSizing(|s, v| s.width(v)),
        doc: "Main-axis size: px | \"fit\" | \"grow\" | \"N%\".",
    },
    PropDef {
        name: "height",
        id: P_HEIGHT,
        kind: Kind::Sizing,
        apply: Apply::SpecSizing(|s, v| s.height(v)),
        doc: "Cross-axis size: px | \"fit\" | \"grow\" | \"N%\".",
    },
    PropDef {
        name: "minWidth",
        id: P_MIN_W,
        kind: Kind::F32,
        apply: Apply::SpecF32(|s, v| s.min_width(v)),
        doc: "Lower width clamp (logical px).",
    },
    PropDef {
        name: "maxWidth",
        id: P_MAX_W,
        kind: Kind::F32,
        apply: Apply::SpecF32(|s, v| s.max_width(v)),
        doc: "Upper width clamp; grow+maxWidth is the responsive-width pattern.",
    },
    PropDef {
        name: "minHeight",
        id: P_MIN_H,
        kind: Kind::F32,
        apply: Apply::SpecF32(|s, v| s.min_height(v)),
        doc: "Lower height clamp (logical px).",
    },
    PropDef {
        name: "maxHeight",
        id: P_MAX_H,
        kind: Kind::F32,
        apply: Apply::SpecF32(|s, v| s.max_height(v)),
        doc: "Upper height clamp (logical px).",
    },
    PropDef {
        name: "gap",
        id: P_GAP,
        kind: Kind::F32,
        apply: Apply::SpecF32(|s, v| s.gap(v)),
        doc: "Space between children along the main axis.",
    },
    PropDef {
        name: "mainAlign",
        id: P_MAIN_ALIGN,
        kind: Kind::Enum(ALIGNS),
        apply: Apply::SpecEnum(|s, i| s.main_align(align_idx(i))),
        doc: "Child alignment along the main axis.",
    },
    PropDef {
        name: "crossAlign",
        id: P_CROSS_ALIGN,
        kind: Kind::Enum(ALIGNS),
        apply: Apply::SpecEnum(|s, i| s.cross_align(align_idx(i))),
        doc: "Child alignment across the main axis.",
    },
    PropDef {
        name: "bg",
        id: P_BG,
        kind: Kind::Color,
        apply: Apply::SpecColor(|s, c| s.bg(c)),
        doc: "Background fill.",
    },
    PropDef {
        name: "radius",
        id: P_RADIUS,
        kind: Kind::F32,
        apply: Apply::SpecF32(|s, v| s.radius(v)),
        doc: "Corner radius for all four corners (logical px); the per-corner props override it when listed after it.",
    },
    PropDef {
        name: "radiusTL",
        id: P_RADIUS_TL,
        kind: Kind::F32,
        apply: Apply::SpecF32(|s, v| s.radius_tl(v)),
        doc: "Top-left corner radius (logical px).",
    },
    PropDef {
        name: "radiusTR",
        id: P_RADIUS_TR,
        kind: Kind::F32,
        apply: Apply::SpecF32(|s, v| s.radius_tr(v)),
        doc: "Top-right corner radius (logical px).",
    },
    PropDef {
        name: "radiusBR",
        id: P_RADIUS_BR,
        kind: Kind::F32,
        apply: Apply::SpecF32(|s, v| s.radius_br(v)),
        doc: "Bottom-right corner radius (logical px).",
    },
    PropDef {
        name: "radiusBL",
        id: P_RADIUS_BL,
        kind: Kind::F32,
        apply: Apply::SpecF32(|s, v| s.radius_bl(v)),
        doc: "Bottom-left corner radius (logical px).",
    },
    PropDef {
        name: "center",
        id: P_CENTER,
        kind: Kind::Flag,
        apply: Apply::SpecFlag(|s| s.center()),
        doc: "Center children on both axes.",
    },
    PropDef {
        name: "hoverable",
        id: P_HOVERABLE,
        kind: Kind::Flag,
        apply: Apply::SpecFlag(|s| s.hoverable()),
        doc: "Hover-track without a click payload (for isHovered-driven styling).",
    },
    PropDef {
        name: "hoverBg",
        id: P_HOVER_BG,
        kind: Kind::Color,
        apply: Apply::SpecColor(|s, c| s.hover_bg(c)),
        doc: "Background while hovered (or while any node in its hoverGroup is); implies hover tracking, eases with `transition`.",
    },
    PropDef {
        name: "pressedBg",
        id: P_PRESSED_BG,
        kind: Kind::Color,
        apply: Apply::SpecColor(|s, c| s.pressed_bg(c)),
        doc: "Background while pressed (or while its hoverGroup is); implies hover tracking.",
    },
    PropDef {
        name: "hoverGroup",
        id: P_HOVER_GROUP,
        kind: Kind::Str,
        apply: Apply::SpecStr(|s, name| s.hover_group(name)),
        doc: "Nodes sharing a group name show hoverBg/pressedBg together (a split button, a multi-piece shape).",
    },
    PropDef {
        name: "onHover",
        id: P_ON_HOVER,
        kind: Kind::Msg,
        apply: Apply::SpecMsg(|s, v| s.on_hover(v)),
        doc: "Hover tag: the pointer entering/leaving emits {kind:\"hover\", phase:\"enter\"|\"leave\", tag} events.",
    },
    PropDef {
        name: "onClick",
        id: P_ON_CLICK,
        kind: Kind::Msg,
        apply: Apply::SpecMsg(|s, v| s.on_click(v)),
        doc: "Message emitted when clicked (data, not a callback).",
    },
    PropDef {
        name: "onDrag",
        id: P_ON_DRAG,
        kind: Kind::Msg,
        apply: Apply::SpecMsg(|s, v| s.on_drag(v)),
        doc: "Drag tag: emits {kind:\"drag\", phase, x, y, dx, dy, parent, tag} events.",
    },
    PropDef {
        name: "onKey",
        id: P_ON_KEY,
        kind: Kind::Msg,
        apply: Apply::SpecMsg(|s, v| s.on_key(v)),
        doc: "Key-sink tag: with key focus held, presses arrive as {kind:\"key\", ...} events.",
    },
    PropDef {
        name: "window",
        id: P_WINDOW,
        kind: Kind::Enum(WINDOW_ROLES),
        apply: Apply::SpecEnum(|s, i| match i {
            0 => s.window_drag(),
            1 => s.window_button(WindowButton::Close),
            2 => s.window_button(WindowButton::Minimize),
            3 => s.window_button(WindowButton::Maximize),
            _ => s,
        }),
        doc: "Window-chrome role: interactions become window commands, not events.",
    },
    PropDef {
        name: "transition",
        id: P_TRANSITION,
        kind: Kind::F32,
        apply: Apply::SpecF32(|s, v| s.transition(v)),
        doc: "Animate sizing/colors/radius changes over this many ms (needs a stable key).",
    },
    PropDef {
        name: "easing",
        id: P_EASING,
        kind: Kind::Enum(EASINGS),
        apply: Apply::SpecEnum(|s, i| s.easing(easing_idx(i))),
        doc: "Easing for `transition` (default easeOut); spring/bouncy integrate with momentum.",
    },
    PropDef {
        name: "slide",
        id: P_SLIDE,
        kind: Kind::Flag,
        apply: Apply::SpecFlag(|s| s.slide()),
        doc: "With transition: also ease the node's position (reordered siblings slide).",
    },
    PropDef {
        name: "keyframes",
        id: P_KEYFRAMES,
        kind: Kind::Keyframes,
        apply: Apply::SpecKeyframes(|s, k| s.keyframes(k)),
        doc: "CSS-style stops `[{ at?, width?, height?, bg?, radius? }, …]`: the slots they name cycle through them over `transition` ms, forever, without the view redrawing; `at` is 0..1 and spreads evenly when omitted.",
    },
    PropDef {
        name: "repeat",
        id: P_REPEAT,
        kind: Kind::Enum(REPEATS),
        apply: Apply::SpecEnum(|s, i| s.repeat(repeat_idx(i))),
        doc: "How `keyframes` cycle (CSS `animation-direction`, default normal). Lua: `direction`, since `repeat` is a keyword.",
    },
    PropDef {
        name: "delay",
        id: P_DELAY,
        kind: Kind::F32,
        apply: Apply::SpecF32(|s, v| s.delay(v)),
        doc: "Holds the `keyframes` cycle back by this many ms (CSS `animation-delay`); siblings with different delays run out of phase.",
    },
    PropDef {
        name: "lineHeight",
        id: P_LINE_HEIGHT,
        kind: Kind::F32,
        apply: Apply::StyleF32(|t, v| t.line_height(v)),
        doc: "Line height (logical px); default size * 1.35.",
    },
    PropDef {
        name: "color",
        id: P_COLOR,
        kind: Kind::Color,
        apply: Apply::StyleColor(|t, c| t.color(c)),
        doc: "Text color; default foreground when omitted.",
    },
    PropDef {
        name: "family",
        id: P_FAMILY,
        kind: Kind::Enum(FAMILIES),
        apply: Apply::StyleEnum(|t, i| match i {
            1 => t.family(FontFamily::Serif),
            2 => t.family(FontFamily::Mono),
            _ => t,
        }),
        doc: "Font family.",
    },
    PropDef {
        name: "font",
        id: P_FONT,
        kind: Kind::Resource,
        apply: Apply::StyleResource(|t, id| t.font(crate::resources::FontId::from_ffi(id))),
        doc: "A registered font handle (addFont / addSystemFont); overrides `family`.",
    },
    PropDef {
        name: "wrap",
        id: P_WRAP,
        kind: Kind::Enum(WRAPS),
        apply: Apply::StyleEnum(|t, i| match i {
            1 => t.wrap(TextWrap::Glyph),
            2 => t.wrap(TextWrap::None),
            _ => t.wrap(TextWrap::Word),
        }),
        doc: "Line breaking at the node's width: between words (default), anywhere, or never (one line per paragraph, clipped to the node).",
    },
    PropDef {
        name: "maxLines",
        id: P_MAX_LINES,
        kind: Kind::F32,
        apply: Apply::StyleF32(|t, v| t.max_lines(v.max(0.0) as u32)),
        doc: "Lay out at most this many lines (0 = unlimited); with `ellipsis`, a line clamp.",
    },
    PropDef {
        name: "ellipsis",
        id: P_ELLIPSIS,
        kind: Kind::Flag,
        apply: Apply::StyleFlag(|t| t.ellipsis()),
        doc: "End the last line with an ellipsis when the text is cut off: a single line unless `maxLines` says otherwise.",
    },
];

/// A prop every binding handles by hand (a composite with real logic, or a
/// constructor-order special), with its wire id so transports agree on
/// identity and its per-binding spelling so the docs can say so.
pub struct CustomProp {
    pub name: &'static str,
    pub id: u32,
    /// How JSX spells it.
    pub jsx: &'static str,
    /// How a Lua table spells it.
    pub lua: &'static str,
    /// Where it lands in C.
    pub c: &'static str,
    pub doc: &'static str,
}

pub const CUSTOM: &[CustomProp] = &[
    CustomProp {
        name: "dir",
        id: P_DIR,
        jsx: "`dir=\"row\" | \"column\"`",
        lua: "`row { }` / `column { }`",
        c: "`dir` (`KUI_ROW` / `KUI_COLUMN`)",
        doc: "Main axis; column is the default.",
    },
    CustomProp {
        name: "size",
        id: P_SIZE,
        jsx: "`size` (text)",
        lua: "`size`",
        c: "`KuiTextStyle.size`",
        doc: "Font size in logical px; the text style is constructed from it, so declare it for the other style props to apply at that size.",
    },
    CustomProp {
        name: "pad",
        id: P_PAD,
        jsx: "`pad`, `padX`, `padY`, `padL`, `padR`, `padT`, `padB`",
        lua: "`pad = n` or `pad = { l, r, t, b }`",
        c: "`pad_l`, `pad_r`, `pad_t`, `pad_b`",
        doc: "Padding; the shorthands resolve to four edges, the specific ones win.",
    },
    CustomProp {
        name: "border",
        id: P_BORDER,
        jsx: "`borderW`, `borderColor`",
        lua: "`border = { w, color }`",
        c: "`border_w`, `border_color`",
        doc: "Border width and color (drawn inside the rect).",
    },
    CustomProp {
        name: "overflow",
        id: P_OVERFLOW,
        jsx: "`clip`, `scrollX`, `scrollY`",
        lua: "`clip`, `scroll_x`, `scroll_y` (`scroll` = `scroll_y`)",
        c: "`overflow` bits `KUI_CLIP` | `KUI_SCROLL_X` | `KUI_SCROLL_Y`",
        doc: "Clip children; scroll (implies clip) with retained offsets and live scrollbars.",
    },
    CustomProp {
        name: "float",
        id: P_FLOAT,
        jsx: "`float=\"below\" | \"above\" | \"parent\" | \"viewport\"` or `{ anchor, at, self, dx, dy, fit }`",
        lua: "`float = \"below\"` or `float = { anchor=, at=, self=, dx=, dy=, fit= }`",
        c: "`float_mode`, `float_anchor_x/y`, `float_self_x/y`, `float_dx/dy`, `float_fit`",
        doc: "Out-of-flow positioning against the parent or the viewport; `fit` flips/clamps to stay on screen.",
    },
    CustomProp {
        name: "keyFocus",
        id: P_KEY_FOCUS,
        jsx: "`keyFocus`",
        lua: "`key_focus`",
        c: "`kui_set_key_focus`",
        doc: "Routes the keyboard at this `onKey` sink this frame.",
    },
    CustomProp {
        name: "key",
        id: P_KEY,
        jsx: "`key`",
        lua: "`key`",
        c: "`kui_open_keyed` label",
        doc: "Stable identity for retained state (scroll offsets, editors, transitions; keys are hashes of the path from the root).",
    },
    CustomProp {
        name: "title",
        id: P_TITLE,
        jsx: "`title` (root box only)",
        lua: "`title` (root table)",
        c: "`kui_set_window_title`",
        doc: "Declares the window title for this frame; the driver diffs and applies.",
    },
    CustomProp {
        name: "tooltip",
        id: P_TOOLTIP,
        jsx: "`tooltip=\"hint\"`",
        lua: "`tooltip = \"hint\"`",
        c: "`kui_tooltip` inside a hoverable node, gated on `kui_is_hovered`",
        doc: "Floats a hint below the node while hovered (implies hover tracking).",
    },
];

/// Where a schema row lands in C when it is not simply the `KuiSpec` /
/// `KuiTextStyle` field of the row's snake_case name (`c_field`).
pub const C_FIELDS: &[(&str, &str)] = &[
    ("width", "`width` (KuiSizing)"),
    ("height", "`height` (KuiSizing)"),
    ("center", "`main_align` + `cross_align` = `KUI_CENTER`"),
    ("window", "`window_role` (`KUI_WINDOW_*`)"),
    ("transition", "`transition_ms`"),
    ("easing", "`easing` (`KUI_EASE_*`)"),
    (
        "keyframes",
        "`keyframes` + `keyframes_len` (`KuiKeyframe[]`)",
    ),
    ("repeat", "`repeat` (`KUI_REPEAT_*`)"),
    ("delay", "`delay_ms`"),
    ("radiusTL", "`radius_tl` with `per_corner`"),
    ("radiusTR", "`radius_tr` with `per_corner`"),
    ("radiusBR", "`radius_br` with `per_corner`"),
    ("radiusBL", "`radius_bl` with `per_corner`"),
    ("hoverGroup", "`hover_group` (KuiStr)"),
    (
        "onClick",
        "`on_click` argument of `kui_open` / `kui_open_with`",
    ),
    (
        "onDrag",
        "`on_drag` argument of `kui_open_draggable` / `kui_open_with`",
    ),
    ("onKey", "`on_key` argument of `kui_open_with`"),
    ("onHover", "`on_hover` argument of `kui_open_with`"),
    ("family", "`KuiTextStyle.family` (`KUI_FONT_*`)"),
    ("font", "`KuiTextStyle.font` (from `kui_font_add*`)"),
    ("lineHeight", "`KuiTextStyle.line_height`"),
    ("wrap", "`KuiTextStyle.wrap` (`KUI_WRAP_*`)"),
    ("maxLines", "`KuiTextStyle.max_lines`"),
    ("ellipsis", "`KuiTextStyle.ellipsis`"),
    ("color", "`KuiTextStyle.color`"),
];

/// The C spelling of a schema row, for docs.
pub fn c_field(def: &PropDef) -> String {
    C_FIELDS
        .iter()
        .find(|(n, _)| *n == def.name)
        .map(|(_, c)| (*c).to_string())
        .unwrap_or_else(|| format!("`{}`", def.snake_name()))
}

/// An element (node type) and its spelling in each binding. Elements are
/// hand-lowered per binding (their shapes differ: JSX children, Lua
/// tables, C calls with body callbacks), so this table is documentation
/// and a checklist, not a code generator's input.
pub struct ElementDef {
    pub name: &'static str,
    pub jsx: &'static str,
    pub lua: &'static str,
    pub c: &'static str,
    pub doc: &'static str,
}

pub const ELEMENTS: &[ElementDef] = &[
    ElementDef {
        name: "box",
        jsx: "`<box>`",
        lua: "`row { }`, `column { }`",
        c: "`kui_open*` … `kui_close`",
        doc: "A container: every container prop applies.",
    },
    ElementDef {
        name: "text",
        jsx: "`<text>` with `<span bold italic color>` children",
        lua: "`text(\"s\", {…})`, `text({ \"a\", { \"b\", bold = true } })`",
        c: "`kui_text`, `kui_rich_text`",
        doc: "Plain or rich text; spans shape as one paragraph, so wrapping crosses style boundaries. `wrap`, `maxLines` and `ellipsis` control line breaking.",
    },
    ElementDef {
        name: "button",
        jsx: "`<button onClick>`",
        lua: "`button { label=, on_click= }`",
        c: "`kui_button`",
        doc: "The stock button: `widgets::button_spec()` with hover/pressed colors declared on the node.",
    },
    ElementDef {
        name: "edit",
        jsx: "`<edit key initial multiline autofocus>`",
        lua: "`edit { key=, initial=, … }`, `input { label= }`",
        c: "`kui_text_edit`, `kui_text_input`",
        doc: "Retained editor state by key; read it back with `editText(key)` after a `changed` event.",
    },
    ElementDef {
        name: "image",
        jsx: "`<image src={id}>`",
        lua: "`image { id= }`",
        c: "`kui_image`",
        doc: "A registered RGBA image; `fit` takes the pixel size, a fit height against a resolved width keeps the aspect, radius rounds it.",
    },
    ElementDef {
        name: "titlebar",
        jsx: "`<titlebar title>` or `<titlebar>…</titlebar>`",
        lua: "`titlebar { title= }` / `titlebar { … }`",
        c: "`kui_titlebar`, `kui_titlebar_with`",
        doc: "Adaptive titlebar for custom chrome: drag strip, native-control inset, window buttons.",
    },
    ElementDef {
        name: "windowButtons",
        jsx: "`<windowButtons/>`",
        lua: "`window_buttons()`",
        c: "`kui_window_buttons`",
        doc: "Just the min/max/close buttons, for fully custom titlebars.",
    },
    ElementDef {
        name: "tooltip",
        jsx: "`tooltip=\"hint\"` prop (see composites)",
        lua: "`tooltip(\"hint\")` / `tooltip { … }` nodes, or the prop",
        c: "`kui_tooltip`, `kui_tooltip_with`",
        doc: "A float hanging below the parent; the node form always draws, the prop form is hover-gated.",
    },
    ElementDef {
        name: "latencyGraph",
        jsx: "`<latencyGraph/>`, `<latencyHud at/>`",
        lua: "`latency_graph()`, `latency_hud { at= }`",
        c: "`kui_latency_graph`, `kui_latency_hud`",
        doc: "Per-phase frame timing (windowed drivers fill it; headless shows the chrome empty).",
    },
];

/// An event kind hosts receive, with its payload shape.
pub struct EventDef {
    pub kind: &'static str,
    pub payload: &'static str,
    pub doc: &'static str,
}

pub const EVENTS: &[EventDef] = &[
    EventDef {
        kind: "click",
        payload: "the `onClick` payload as-is",
        doc: "A press and release on the node (suppressed when a drag moved past the slop).",
    },
    EventDef {
        kind: "drag",
        payload: "`{ kind: \"drag\", phase: \"start\" | \"move\" | \"end\", x, y, dx, dy, parent: { x, y, w, h }, tag }`",
        doc: "A pointer-captured drag on an `onDrag` node; `parent` is the container rect, so fractions need no geometry query.",
    },
    EventDef {
        kind: "key",
        payload: "`{ kind: \"key\", code, shift, ctrl, alt, super, text, repeat, tag }`",
        doc: "A key press on the focused `onKey` sink; `code` is a character or a name (`\"left\"`, `\"f5\"`).",
    },
    EventDef {
        kind: "hover",
        payload: "`{ kind: \"hover\", phase: \"enter\" | \"leave\", tag }`",
        doc: "The pointer entered or left an `onHover` node — also when a new frame moved it under a still cursor.",
    },
    EventDef {
        kind: "resize",
        payload: "`{ kind: \"resize\", width, height, scale }`",
        doc: "The viewport changed size or DPI (logical px, delivered to the host on the root); `KuiWindow.size()` queries the same numbers.",
    },
    EventDef {
        kind: "modifiers",
        payload: "`{ kind: \"modifiers\", shift, ctrl, alt, super }`",
        doc: "The physical modifier state changed (delivered to the host on the root).",
    },
    EventDef {
        kind: "changed / submit",
        payload: "`{ kind: \"changed\" }` / `{ kind: \"submit\" }`, with the editor's key on the event",
        doc: "An editor's text changed / Enter in a single-line editor.",
    },
];

/// A host-registered resource and how each binding registers it.
pub struct ResourceDef {
    pub what: &'static str,
    pub node: &'static str,
    pub lua: &'static str,
    pub c: &'static str,
}

pub const RESOURCES: &[ResourceDef] = &[
    ResourceDef {
        what: "image",
        node: "`ctx.addImage(w, h, rgba)` → id for `<image src>`",
        lua: "the host registers; `image { id }`",
        c: "`kui_image_add` → `kui_image`",
    },
    ResourceDef {
        what: "font from bytes",
        node: "`ctx.addFont(buffer)` → id for `font`",
        lua: "the host registers; `font = id`",
        c: "`kui_font_add` → `KuiTextStyle.font`",
    },
    ResourceDef {
        what: "font file by path",
        node: "`ctx.loadFontFile(\"fonts/Antonio.ttf\")` → id for `font`",
        lua: "the host registers; `font = id`",
        c: "`kui_font_load_file`",
    },
    ResourceDef {
        what: "a folder of fonts",
        node: "`ctx.loadFontsDir(\"fonts\")`, then pick by name",
        lua: "the host loads",
        c: "`kui_font_load_dir`",
    },
    ResourceDef {
        what: "font by family name (installed or loaded)",
        node: "`ctx.addSystemFont(\"Antonio\")` (see `systemFontFamilies()`)",
        lua: "the host registers; `font = id`",
        c: "`kui_font_add_system`",
    },
];

static SNAKE_NAMES: LazyLock<Vec<&'static str>> = LazyLock::new(|| {
    PROPS
        .iter()
        .map(|d| Box::leak(snake_case(d.name).into_boxed_str()) as &'static str)
        .collect()
});

/// `minWidth` → `min_width`, `radiusTL` → `radius_tl` (a run of capitals
/// is one word); names without capitals pass through.
pub fn snake_case(name: &str) -> String {
    let mut out = String::with_capacity(name.len() + 2);
    let mut prev_upper = false;
    for c in name.chars() {
        if c.is_ascii_uppercase() {
            if !prev_upper {
                out.push('_');
            }
            out.push(c.to_ascii_lowercase());
            prev_upper = true;
        } else {
            out.push(c);
            prev_upper = false;
        }
    }
    out
}

pub fn by_name(name: &str) -> Option<&'static PropDef> {
    PROPS.iter().find(|d| d.name == name)
}

pub fn by_snake_name(name: &str) -> Option<&'static PropDef> {
    SNAKE_NAMES
        .iter()
        .position(|n| *n == name)
        .map(|i| &PROPS[i])
}

pub fn by_id(id: u32) -> Option<&'static PropDef> {
    PROPS.iter().find(|d| d.id == id)
}

/// A parsed prop value, transport-independent.
pub enum Parsed {
    F32(f32),
    Color(Color),
    Flag,
    Enum(usize),
    Sizing(Sizing),
    Msg(Value),
    Str(String),
    Resource(u64),
    Keyframes(Vec<Keyframe>),
}

/// Everything a prop list can carry; elements pick the parts they use.
#[derive(Clone, Debug, PartialEq)]
pub struct PropsOut {
    pub spec: NodeSpec,
    pub style: TextStyle,
    pub key: Option<String>,
    pub title: Option<String>,
    pub key_focus: bool,
    /// Hover hint: the element lowering floats `widgets::tooltip` below the
    /// node while it is hovered (the parser also marks the spec hoverable).
    pub tooltip: Option<String>,
}

impl PropsOut {
    pub fn new() -> Self {
        PropsOut {
            spec: NodeSpec::column(),
            style: TextStyle::new(16.0),
            key: None,
            title: None,
            key_focus: false,
            tooltip: None,
        }
    }

    /// Applies a builder step to the spec in place.
    pub fn with_spec(&mut self, f: impl FnOnce(NodeSpec) -> NodeSpec) {
        self.spec = f(std::mem::take(&mut self.spec));
    }
}

impl Default for PropsOut {
    fn default() -> Self {
        Self::new()
    }
}

/// Applies one parsed value through its row. Errors on a kind mismatch,
/// which can only come from a transport bug (each transport parses by the
/// same `Kind`).
pub fn apply(def: &PropDef, value: Parsed, out: &mut PropsOut) -> Result<(), String> {
    let spec = std::mem::take(&mut out.spec);
    let style = out.style;
    match (&def.apply, value) {
        (Apply::SpecF32(f), Parsed::F32(v)) => out.spec = f(spec, v),
        (Apply::SpecColor(f), Parsed::Color(v)) => out.spec = f(spec, v),
        (Apply::SpecFlag(f), Parsed::Flag) => out.spec = f(spec),
        (Apply::SpecEnum(f), Parsed::Enum(v)) => out.spec = f(spec, v),
        (Apply::SpecSizing(f), Parsed::Sizing(v)) => out.spec = f(spec, v),
        (Apply::SpecMsg(f), Parsed::Msg(v)) => out.spec = f(spec, v),
        (Apply::SpecStr(f), Parsed::Str(v)) => out.spec = f(spec, &v),
        (Apply::SpecKeyframes(f), Parsed::Keyframes(v)) => out.spec = f(spec, v),
        (Apply::StyleF32(f), Parsed::F32(v)) => {
            out.spec = spec;
            out.style = f(style, v);
        }
        (Apply::StyleColor(f), Parsed::Color(v)) => {
            out.spec = spec;
            out.style = f(style, v);
        }
        (Apply::StyleEnum(f), Parsed::Enum(v)) => {
            out.spec = spec;
            out.style = f(style, v);
        }
        (Apply::StyleFlag(f), Parsed::Flag) => {
            out.spec = spec;
            out.style = f(style);
        }
        (Apply::StyleResource(f), Parsed::Resource(v)) => {
            out.spec = spec;
            out.style = f(style, v);
        }
        _ => {
            out.spec = spec;
            return Err(format!("prop {} value/kind mismatch", def.name));
        }
    }
    Ok(())
}

/// Looks an enum name up in its row's list.
pub fn enum_index(names: &[&str], s: &str) -> Result<usize, String> {
    names
        .iter()
        .position(|n| *n == s)
        .ok_or_else(|| format!("bad value {s:?} (one of {names:?})"))
}

// ---------------------------------------------------------------------------
// Value parsing shared by transports

/// 0 = transparent, anything else 0xRRGGBBAA.
pub fn color_num(hex: u32) -> Color {
    if hex == 0 {
        Color::TRANSPARENT
    } else {
        Color::hex(hex)
    }
}

/// `#rgb` / `#rrggbb` / `#rrggbbaa`.
pub fn color_hex_str(s: &str) -> Result<Color, String> {
    let bad = || format!("bad color {s:?}");
    let hex = s.strip_prefix('#').ok_or_else(bad)?;
    let expanded = match hex.len() {
        3 => hex
            .chars()
            .flat_map(|c| [c, c])
            .chain("ff".chars())
            .collect::<String>(),
        6 => format!("{hex}ff"),
        8 => hex.to_string(),
        _ => return Err(bad()),
    };
    let n = u32::from_str_radix(&expanded, 16).map_err(|_| bad())?;
    Ok(Color::hex(n))
}

/// The string forms of a sizing: "fit" | "grow" | "N%".
pub fn sizing_str(s: &str) -> Result<Sizing, String> {
    match s {
        "fit" => Ok(Sizing::Fit),
        "grow" => Ok(Sizing::Grow(1.0)),
        s if s.ends_with('%') => {
            let pct: f32 = s[..s.len() - 1]
                .parse()
                .map_err(|_| format!("bad percent {s:?}"))?;
            Ok(Sizing::Percent(pct / 100.0))
        }
        _ => Err(format!("bad sizing {s:?} (fit | grow | number | \"N%\")")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_and_names_are_unique() {
        let all: Vec<(&str, u32)> = PROPS
            .iter()
            .map(|d| (d.name, d.id))
            .chain(CUSTOM.iter().map(|c| (c.name, c.id)))
            .collect();
        for (i, (name, id)) in all.iter().enumerate() {
            for (other_name, other_id) in &all[i + 1..] {
                assert_ne!(name, other_name, "duplicate prop name");
                assert_ne!(id, other_id, "duplicate wire id for {name} / {other_name}");
            }
        }
    }

    #[test]
    fn snake_names_round_trip() {
        assert_eq!(by_snake_name("min_width").unwrap().name, "minWidth");
        assert_eq!(by_snake_name("on_click").unwrap().name, "onClick");
        assert_eq!(by_snake_name("bg").unwrap().name, "bg");
        assert!(by_snake_name("minWidth").is_none());
        assert_eq!(by_name("lineHeight").unwrap().snake_name(), "line_height");
        assert_eq!(by_name("radiusTL").unwrap().snake_name(), "radius_tl");
        assert_eq!(by_snake_name("radius_bl").unwrap().name, "radiusBL");
    }

    #[test]
    fn every_row_applies_a_sample_of_its_kind() {
        for def in PROPS {
            let sample = match def.kind {
                Kind::F32 => Parsed::F32(7.0),
                Kind::Color => Parsed::Color(Color::hex(0x11223344)),
                Kind::Flag => Parsed::Flag,
                Kind::Enum(_) => Parsed::Enum(1),
                Kind::Sizing => Parsed::Sizing(Sizing::Percent(0.5)),
                Kind::Msg => Parsed::Msg(Value::Int(1)),
                Kind::Str => Parsed::Str("name".into()),
                Kind::Resource => Parsed::Resource(7),
                Kind::Keyframes => Parsed::Keyframes(vec![Keyframe::default().radius(7.0)]),
            };
            let mut out = PropsOut::new();
            apply(def, sample, &mut out).unwrap();
            let changed = match def.target() {
                Target::Spec => out.spec != NodeSpec::column(),
                Target::Style => out.style != TextStyle::new(16.0),
            };
            assert!(changed, "{} applied a sample but nothing changed", def.name);
        }
    }

    #[test]
    fn colors_and_sizings_parse() {
        assert_eq!(color_hex_str("#fff").unwrap(), Color::hex(0xffffffff));
        assert_eq!(color_hex_str("#11223344").unwrap(), Color::hex(0x11223344));
        assert!(color_hex_str("fff").is_err());
        assert_eq!(sizing_str("50%").unwrap(), Sizing::Percent(0.5));
        assert_eq!(sizing_str("grow").unwrap(), Sizing::Grow(1.0));
        assert!(sizing_str("wide").is_err());
    }
}
