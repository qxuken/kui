//! The prop schema: the one table of node props, elements, events and
//! readings that every kui binding is generated from or checked against.
//!
//! A Rust app does not read this module; it builds a `NodeSpec` with its
//! methods. The tables here are for the bindings and for tooling. Each
//! [`PROPS`] row names a prop (camelCase, with the snake_case spelling
//! derived), its wire id, its value kind, how it applies to a `NodeSpec`
//! and its documentation: `kui-node` parses JSON and its binary stream by
//! kind and generates the TypeScript types from the rows, `kui-lua` looks
//! each table key up by name, and `kui-ffi` mirrors the rows in a C struct
//! that a parity test pins to this table. [`CUSTOM`] lists the composite
//! props a binding extracts itself (padding shorthands, border, overflow,
//! floats), [`ELEMENTS`] the props each element lowers, [`DOORS`] the
//! verbs (calls rather than props) with their spelling in every binding,
//! and [`known_prop`] answers whether a name is any of these, which is
//! what an `unknown-prop` warning checks against.
//!
//! ```rust
//! use kui_core::schema::{PROPS, P_WIDTH};
//!
//! let width = PROPS.iter().find(|p| p.id == P_WIDTH).expect("a core prop");
//! assert_eq!(width.name, "width");
//! assert!(!width.doc.is_empty());
//! ```
//!
//! Adding a simple prop is one row here (plus `npm run gen` for the TS
//! types, and a field in the C struct when the parity test says so).
//! Composite props need per-binding extraction but not per-binding
//! decisions: what a shorthand means lives in one place in `spec`, and
//! `crate::conformance` makes every binding agree on behaviour.

use std::sync::LazyLock;

use rustc_hash::{FxHashMap, FxHashSet};

use crate::access::Role;
use crate::anim::{Easing, Repeat};
use crate::color::Color;
use crate::cursor::CursorShape;
use crate::enter::Enter;
use crate::keyframes::Keyframe;
use crate::spec::{
    Align, Bound, FontFamily, NodeSpec, PadShorthand, Sizing, TextStyle, TextWrap, UnderlineStyle,
};
use crate::value::Value;
use crate::window::{WindowButton, WindowConfig};

mod doors;
pub use doors::{Cell, DOORS, Door, GUEST};

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
pub const P_ENTER: u32 = 49;
pub const P_CLICK_SOUND: u32 = 50;
pub const P_HOVER_SOUND: u32 = 51;
pub const P_ON_LAYOUT: u32 = 52;
pub const P_ROLE: u32 = 53;
pub const P_LABEL: u32 = 54;
pub const P_CHECKED: u32 = 55;
pub const P_VALUE_NOW: u32 = 56;
pub const P_VALUE_MIN: u32 = 57;
pub const P_VALUE_MAX: u32 = 58;
pub const P_CARET: u32 = 59;
pub const P_SELECTION_ANCHOR: u32 = 60;
pub const P_FOCUSABLE: u32 = 61;
pub const P_DISABLED: u32 = 62;
pub const P_FOCUS_BG: u32 = 63;
pub const P_MODAL: u32 = 64;
pub const P_ON_CONTEXT_MENU: u32 = 65;
pub const P_CURSOR: u32 = 66;
pub const P_SELECTED: u32 = 67;
pub const P_EXPANDED: u32 = 68;
pub const P_OPACITY: u32 = 69;
pub const P_SHADOW_COLOR: u32 = 70;
pub const P_SHADOW_BLUR: u32 = 71;
pub const P_SHADOW_X: u32 = 72;
pub const P_SHADOW_Y: u32 = 73;
pub const P_SHADOW_SPREAD: u32 = 74;
pub const P_WRAP_CHILDREN: u32 = 75;
pub const P_CROSS_GAP: u32 = 76;
pub const P_INITIAL_FOCUS: u32 = 77;
pub const P_EXIT: u32 = 78;
pub const P_WINDOWS: u32 = 79;
pub const P_LIVE: u32 = 80;
pub const P_KEY_UP: u32 = 81;
pub const P_VALUE_TEXT: u32 = 82;
pub const P_DESCRIPTION: u32 = 83;
pub const P_FEATURES: u32 = 84;
pub const P_UNDERLINE: u32 = 85;
pub const P_STRIKETHROUGH: u32 = 86;
pub const P_ANIMATE: u32 = 87;
pub const P_ACCENT: u32 = 88;
pub const P_INDEX: u32 = 89;
pub const P_SELECTABLE: u32 = 90;
pub const P_ON_FORCE_CLICK: u32 = 91;
pub const P_FOCUS_REGION: u32 = 92;
pub const P_SCROLLBAR: u32 = 93;
pub const P_SCROLLBAR_WIDTH: u32 = 94;
pub const P_SCROLLBAR_COLOR: u32 = 95;
pub const P_SCROLLBAR_ACTIVE_COLOR: u32 = 96;
pub const P_ANCHOR: u32 = 97;
pub const P_ALWAYS_ON_TOP: u32 = 98;
pub const P_ON_SCROLL: u32 = 99;
pub const P_ROW_COUNT: u32 = 100;
pub const P_UNDERLINE_COLOR: u32 = 101;
pub const P_UNDERLINE_STYLE: u32 = 102;
pub const P_ON_DROP: u32 = 103;
pub const P_DROP_BG: u32 = 104;
pub const P_CARET_SOLID: u32 = 105;
pub const P_SECURE_INPUT: u32 = 106;
pub const P_ASPECT_RATIO: u32 = 107;
pub const P_MIXED: u32 = 108;
pub const P_VALUE_STEP: u32 = 109;
pub const P_ON_CHANGE: u32 = 110;
pub const P_PIXEL_SNAP: u32 = 111;
pub const P_KEEP_FOCUS: u32 = 112;
pub const P_ON_FOCUS: u32 = 113;
pub const P_RULES: u32 = 114;
pub const P_RULE_WIDTH: u32 = 115;
pub const P_ON_BUTTON: u32 = 116;
pub const P_BUTTONS: u32 = 117;
pub const P_OVERSCROLL: u32 = 118;
pub const P_SCROLL_AXES: u32 = 119;
pub const P_MODIFIER_KEYS: u32 = 120;
pub const P_OPTION_AS_ALT: u32 = 121;
pub const P_BOUNCE: u32 = 122;
pub const P_GRADIENT: u32 = 123;
pub const P_SCROLL_MODS: u32 = 124;
pub const P_IME_OFF: u32 = 125;
pub const P_BACKDROP_BLUR: u32 = 126;
pub const P_ROTATE: u32 = 127;
pub const P_SCALE: u32 = 128;
pub const P_PIVOT_X: u32 = 129;
pub const P_PIVOT_Y: u32 = 130;
pub const P_ITERATIONS: u32 = 131;
pub const P_BOLD: u32 = 132;

/// The `mainAlign` / `crossAlign` rows and a float's attach points, in
/// `Align`'s order. Append-only: the Lua and Node wires carry the index,
/// and C's `KUI_ALIGN_*` is it. The spreads mean something on `mainAlign`
/// and `baseline` on a row's `crossAlign` only.
pub const ALIGNS: &[&str] = &[
    "start",
    "center",
    "end",
    "spaceBetween",
    "spaceAround",
    "spaceEvenly",
    "baseline",
];
pub const WINDOW_ROLES: &[&str] = &["drag", "close", "minimize", "maximize"];
/// The `scrollbar` row, in `ScrollbarMode::ALL`'s order: the stock
/// overlay bar, none, or one that fades out when the scroll state has not
/// changed. C spells it as the index plus one (`KUI_SCROLLBAR_*`), so a
/// zeroed field is "unset".
pub const SCROLLBARS: &[&str] = &["visible", "hidden", "auto"];
/// The `overscroll` row, in `Overscroll::ALL`'s order: a
/// scroll gesture starting over a scroller at its limit goes on to the one
/// around it, or stays. C spells it as the index plus one
/// (`KUI_OVERSCROLL_*`), a zeroed field being `auto`.
pub const OVERSCROLLS: &[&str] = &["auto", "contain"];
/// The `scrollAxes` row, in `ScrollAxes::ALL`'s order: the
/// axes an `onScroll` node takes. C spells it as the index plus one
/// (`KUI_SCROLL_AXES_*`), a zeroed field being `both`.
pub const SCROLL_AXES: &[&str] = &["both", "x", "y"];
/// The stock families (`FontFamily::name` spellings, in `FontFamily::ALL`
/// order); a registered font travels as the `font` row's handle instead.
pub const FAMILIES: &[&str] = &["sans", "serif", "mono"];
/// The pointer shapes a view can declare (`CursorShape::name` spellings, in
/// `CursorShape::ALL` order — a `cursor.rs` test pins the two together).
pub const CURSORS: &[&str] = &[
    "default",
    "text",
    "pointer",
    "grab",
    "grabbing",
    "notAllowed",
    "ewResize",
    "nsResize",
    "nwseResize",
    "neswResize",
];

pub fn cursor_idx(i: usize) -> CursorShape {
    CURSORS
        .get(i)
        .and_then(|n| CursorShape::parse(n))
        .unwrap_or(CursorShape::Default)
}
pub const WRAPS: &[&str] = &["word", "glyph", "none", "break-spaces"];
/// `underlineStyle` / `underline_style`; `UnderlineStyle::NAMES`.
pub const UNDERLINE_STYLES: &[&str] = UnderlineStyle::NAMES;
/// `expanded` names its state rather than being a flag: a disclosure that
/// is shut has to say "collapsed", and an absent flag cannot — absent has
/// to keep meaning "this node does not expand" (AccessKit's `expanded`,
/// ARIA's `aria-expanded`, are three-state for the same reason).
pub const EXPANDED: &[&str] = &["collapsed", "expanded"];
/// How urgently a reader should read a change it was not asked to read
/// (`crate::access::Live::name` spellings, in wire order — a binding
/// sends the index). `off` is the default and means "not a live region".
pub const LIVE: &[&str] = &["off", "polite", "assertive"];
/// The roles a view can declare (`crate::access::Role::name` spellings),
/// in wire order — a binding sends the index. The purely derived roles are
/// the ones [`DERIVED_ONLY`] names, and every other `Role::ALL` variant is
/// here; `textInput`, `multilineTextInput` and `line` are, because an app
/// that draws its own text declares them.
pub const ROLES: &[&str] = &[
    "none",
    "button",
    "checkbox",
    "radio",
    "switch",
    "slider",
    "tab",
    "tabList",
    "link",
    "heading",
    "list",
    "listItem",
    "image",
    "dialog",
    "group",
    "textInput",
    "multilineTextInput",
    "line",
    // Appended later. The tail is the only free position,
    // which is what makes "the first fifteen can be declared" a list rather
    // than a range.
    "radioGroup",
    "menu",
    "menuItem",
    // Appended later: a cell grid's derived role.
    "terminal",
];

/// How a composite container arranges its items
/// (`crate::access::Orientation::name` spellings), in wire order. Derived
/// from the container's `dir` and reported on its access node, never
/// declared — so unlike [`ROLES`] this is not a prop's enum, only a list
/// the C header restates.
pub const ORIENTATIONS: &[&str] = &["horizontal", "vertical"];

/// The OS light/dark setting (`crate::env::Appearance::name` spellings, in
/// `Appearance::ALL` order — an `env.rs` test pins the two together). Like
/// [`ORIENTATIONS`] this is not a prop's enum: it is a fact a host pushes
/// and every binding spells the same way. Index 0 is `unknown`, so a
/// zeroed C call reports what it actually knows.
pub const APPEARANCES: &[&str] = &["unknown", "light", "dark"];

/// The OS reduce-motion setting (`crate::env::MotionPref::name` spellings, in
/// `MotionPref::ALL` order), `unknown` first for the same reason.
pub const MOTIONS: &[&str] = &["unknown", "full", "reduced"];

/// Whether assistive technology is listening (`crate::env::Assistive::name`
/// spellings, in `Assistive::ALL` order), `unknown` first for the same
/// reason: a host with no bridge reports that it cannot tell.
pub const ASSISTIVE: &[&str] = &["unknown", "none", "listening"];

/// The audio output device's state (`crate::env::AudioDevice::name`
/// spellings, in `AudioDevice::ALL` order), `closed` first so a zeroed C
/// call reports the default.
pub const AUDIO_DEVICES: &[&str] = &["closed", "opening", "open", "failed"];

/// What is behind a window's transparent pixels
/// (`crate::window::Backdrop::name` spellings, in `Backdrop::ALL` order),
/// `opaque` first so a zeroed C call reports the default.
pub const BACKDROPS: &[&str] = &["opaque", "transparent", "blur", "tinted"];

/// The roles no view can declare, because the core derives them itself,
/// with what derives each one. Every
/// [`Role::ALL`] variant is on this list or in [`ROLES`], and
/// `every_role_is_declarable_or_derived` keeps both halves honest: a role
/// exempted here has to be one a frame really does derive, so the list
/// cannot absorb a variant that was simply forgotten from `ROLES`.
pub const DERIVED_ONLY: &[(&str, &str)] = &[
    (
        "window",
        "the root node of a frame, named by the window title",
    ),
    ("titleBar", "a `windowDrag` node"),
    ("staticText", "a text node"),
    ("scrollView", "a `scrollX` / `scrollY` node"),
];

/// The role at wire index `i` — `ROLES`' order is the protocol, so this
/// and [`ROLES`] are pinned to each other by
/// `every_declarable_role_name_is_a_real_role`.
///
/// Neither way of missing can happen today. A name in `ROLES` that no
/// longer parses is what that test catches, by construction rather than
/// at run time. Out of range cannot arrive through a transport: Node's
/// binary reader rejects `i >= names.len()` before it builds a
/// `Parsed::Enum`, Node's JSON and Lua resolve a *name* through
/// [`enum_index`], and C carries `Role::ALL` positions that
/// [`crate::access::Role`] itself bounds.
///
/// So the fallback is reachable only from a future transport that forgets
/// its check, and it is `Role::Group` rather than `Role::None` for that
/// reader: `None` takes the node *and its whole subtree* out of the access
/// tree, which is a destructive answer to "an index I do not have", while
/// a group is what the core already derives for a box that is merely
/// somewhere focus can land — the node keeps its children, and a wrong
/// role is recoverable where a missing subtree is not. It stays silent
/// because this is a pure schema function with no warning sink, and the
/// transports' own errors name the prop and the index, which is a better
/// report than a warning here.
pub fn role_idx(i: usize) -> Role {
    ROLES
        .get(i)
        .and_then(|n| Role::parse(n))
        .unwrap_or(Role::Group)
}
/// The easing curves (`Easing::name` spellings, in `Easing::ALL` order —
/// the test below pins the two together, as `CURSORS` is pinned).
pub const EASINGS: &[&str] = &[
    "easeOut",
    "linear",
    "easeIn",
    "easeInOut",
    "spring",
    "bouncy",
    "smooth",
    "snappy",
];

/// CSS's `animation-direction` values, in `Repeat::ALL`'s order.
pub const REPEATS: &[&str] = &["normal", "reverse", "alternate", "alternateReverse"];

pub fn repeat_idx(i: usize) -> Repeat {
    Repeat::from_index(i)
}

pub fn easing_idx(i: usize) -> Easing {
    Easing::from_index(i)
}

/// How a prop's value is parsed (per transport) and encoded (binary slots).
pub enum Kind {
    /// One number, or a `"$length"` token. Binary: 1 slot.
    F32,
    /// One color: `0xRRGGBBAA` number or `#hex` string, or a `"$color"`
    /// token. Binary: 1 slot (u32).
    Color,
    /// Marker, present-or-absent (`false` = absent). Binary: 0 slots.
    Flag,
    /// One of a closed name list. Binary: 1 slot (index).
    Enum(&'static [&'static str]),
    /// A sizing: number | "fit" | "grow" | "N%" | a size expression
    /// (`"clamp(400px, 80%, 1000px)"`, [`crate::calc`]) | {grow} |
    /// {percent}, or a `"$length"` token (a fixed length). Binary: 2 slots
    /// (mode, value), or with mode [`SIZE_MODE_CALC`] the mode and a
    /// strref, the expression's spelling (v19); tagged, 1 slot (the index).
    Sizing,
    /// A lower clamp: number | "fit" (the node's own fit size on that axis)
    /// | a size expression, or a `"$length"` token. Binary: 2 slots (mode,
    /// value), a sizing's first two modes, or [`SIZE_MODE_CALC`] and a
    /// strref; tagged, 1 slot (the index).
    Min,
    /// An upper clamp: number | a size expression, or a `"$length"` token.
    /// Binary as a `Min` (v19; a plain number before): mode 0 and px, or
    /// [`SIZE_MODE_CALC`] and a strref; tagged, 1 slot (the index).
    Max,
    /// An arbitrary message payload (a `Value`). Binary: strref to JSON.
    Msg,
    /// A message merged into the core's own event under `tag` (`onDrag`,
    /// `onKey`, `onHover`, `onLayout`). Parsed and carried exactly like a
    /// `Msg`, but null is a legal value: the node still gets the behaviour
    /// (a key sink, a drag source) and its events simply carry no `tag` —
    /// so an app whose messages are a typed union needs no inert member
    /// just to name a sink.
    Tag,
    /// A plain string (a name, not a message). Binary: strref.
    Str,
    /// A font family: one of the stock names (`FAMILIES`) or an installed
    /// or loaded family's name, which the parser registers and turns into
    /// its handle. Binary: strref (v18).
    Family,
    /// A registered resource handle (a font or sound id): the integer form
    /// of the slotmap key. JSON/binary carry it as the 16-hex string the
    /// addon hands out; Lua as an integer; C as a `uint64_t`.
    Resource,
    /// A keyframe stop list (`crate::keyframes::parse` reads the plain-data
    /// form). JSON/binary/Lua carry it like a `Msg` and parse it in the
    /// core; C passes a `KuiKeyframe` array.
    Keyframes,
    /// An entrance (`crate::enter::parse` reads the plain-data form).
    /// Carried like a `Msg` and parsed in the core; C fills a `KuiEnter`.
    Enter,
    /// A gradient (`crate::gradient::parse` reads the plain-data form).
    /// Carried like a `Msg` and parsed in the core; C fills a
    /// `KuiGradient`.
    Gradient,
}

/// Where a parsed value lands. `PropDef::target` derives from this.
pub enum Apply {
    SpecF32(fn(NodeSpec, f32) -> NodeSpec),
    SpecColor(fn(NodeSpec, Color) -> NodeSpec),
    SpecFlag(fn(NodeSpec) -> NodeSpec),
    SpecEnum(fn(NodeSpec, usize) -> NodeSpec),
    SpecSizing(fn(NodeSpec, Sizing) -> NodeSpec),
    SpecBound(fn(NodeSpec, Bound) -> NodeSpec),
    SpecMsg(fn(NodeSpec, Value) -> NodeSpec),
    SpecStr(fn(NodeSpec, &str) -> NodeSpec),
    SpecKeyframes(fn(NodeSpec, Vec<Keyframe>) -> NodeSpec),
    SpecEnter(fn(NodeSpec, Enter) -> NodeSpec),
    SpecGradient(fn(NodeSpec, crate::gradient::Gradient) -> NodeSpec),
    SpecResource(fn(NodeSpec, u64) -> NodeSpec),
    StyleF32(fn(TextStyle, f32) -> TextStyle),
    StyleColor(fn(TextStyle, Color) -> TextStyle),
    StyleEnum(fn(TextStyle, usize) -> TextStyle),
    StyleFlag(fn(TextStyle) -> TextStyle),
    StyleResource(fn(TextStyle, u64) -> TextStyle),
    StyleStr(fn(TextStyle, &str) -> TextStyle),
    StyleFamily(fn(TextStyle, FontFamily) -> TextStyle),
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
            | Apply::StyleResource(_)
            | Apply::StyleStr(_)
            | Apply::StyleFamily(_) => Target::Style,
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
        3 => Align::SpaceBetween,
        4 => Align::SpaceAround,
        5 => Align::SpaceEvenly,
        6 => Align::Baseline,
        _ => Align::Start,
    }
}

/// Binary min or max decode: (mode, value) → a clamp, the first two
/// sizing modes ([`SIZE_MODE_CALC`] is read by the transport, which
/// holds the strref).
pub fn min_num(mode: u32, value: f64) -> Bound {
    match mode {
        1 => Bound::Fit,
        _ => Bound::Px(value as f32),
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
        doc: "Horizontal size: px | \"fit\" | \"grow\" | \"N%\" | a size expression — \
              `\"clamp(400px, 80%, 1000px)\"`, `\"min(720px, 100%)\"`, `\"max(50%, 300)\"`, \
              nested — which layout resolves against the parent's content box, the box a \
              percentage takes its cut of (backlog F109). An expression with no percentage \
              in it is a length; a calc does not ease under `transition`. A percentage \
              or an expression gives, with the fit children, when its parent overflows \
              — two `\"50%\"` children and a gap fit their row (backlog F110) — where a \
              px size keeps its own. A row holding one gives as CSS's flex items do: \
              every child that can give gives in proportion to its size, and stops at \
              its content — the widest thing in it that cannot wrap, a label's longest \
              word — unless `minWidth` says otherwise (backlog RG92). The process keeps \
              65 536 distinct expressions and never lets one go: past that a new one leaves \
              its prop at its default, with a `size-expressions-full` warning, so declare \
              one per layout — a px size for the part that moves each frame, a splitter's \
              drag — not one per frame (backlog RG93).",
    },
    PropDef {
        name: "height",
        id: P_HEIGHT,
        kind: Kind::Sizing,
        apply: Apply::SpecSizing(|s, v| s.height(v)),
        doc: "Vertical size: px | \"fit\" | \"grow\" | \"N%\" | a size expression (see `width`).",
    },
    PropDef {
        name: "minWidth",
        id: P_MIN_W,
        kind: Kind::Min,
        apply: Apply::SpecBound(|s, v| s.min_width(v)),
        doc: "Lower width clamp: logical px, a size expression (see `width`; a percentage \
              clamp is none until the parent's width is known, as in CSS), or \"fit\" for \
              the node's own fit width. \
              \"fit\" under `width=\"grow\"` is a content floor — CSS's `flex: 1 0 auto` — \
              which is what an i3-style tab bar is: tabs that split the bar evenly \
              while they fit and sit at their label's width, scrolling, once they do \
              not. Opt-in, because a fit width is the unwrapped one: a paragraph in a \
              grow column would stop wrapping under it. Left out, a child giving in an \
              overflowing row that holds a percentage or a size expression stops at its \
              content, CSS's `min-width: auto`; `0` lets it go below, CSS's \
              `min-width: 0` (backlog RG92). A fit node across a column is no wider \
              than the column's box — CSS's `fit-content` — down to this floor, 0 left \
              out, unless the column scrolls x, so a text one wrapper deep in a capped \
              card wraps there; \"fit\" keeps its content's width and runs past \
              (backlog F116).",
    },
    PropDef {
        name: "maxWidth",
        id: P_MAX_W,
        kind: Kind::Max,
        apply: Apply::SpecBound(|s, v| s.max_width(v)),
        doc: "Upper width clamp: logical px or a size expression (see `width`); \
              grow+maxWidth is the responsive-width pattern.",
    },
    PropDef {
        name: "minHeight",
        id: P_MIN_H,
        kind: Kind::Min,
        apply: Apply::SpecBound(|s, v| s.min_height(v)),
        doc: "Lower height clamp: logical px, a size expression, or \"fit\" for the node's own fit height. Undeclared, it is the node's content where its column overflows — CSS's `min-height: auto`, none for a node that scrolls or clips — so a row keeps the height of its text; `0` asks for the squeeze back (see `minWidth`).",
    },
    PropDef {
        name: "maxHeight",
        id: P_MAX_H,
        kind: Kind::Max,
        apply: Apply::SpecBound(|s, v| s.max_height(v)),
        doc: "Upper height clamp: logical px or a size expression (see `width`).",
    },
    PropDef {
        name: "gap",
        id: P_GAP,
        kind: Kind::F32,
        apply: Apply::SpecF32(|s, v| s.gap(v)),
        doc: "Space between children along the main axis.",
    },
    // Not `wrap`: that name is taken, by the text prop that picks where a
    // line breaks inside one paragraph. One name cannot mean both, and the
    // two meet on `<edit>`, which takes container and text props at once.
    PropDef {
        name: "wrapChildren",
        id: P_WRAP_CHILDREN,
        kind: Kind::Flag,
        apply: Apply::SpecFlag(NodeSpec::wrap),
        doc: "Children that don't fit the main axis start a new line instead of overflowing or shrinking. Rows only (a column is ignored, with a warning), and never on a scrollX row.",
    },
    PropDef {
        name: "crossGap",
        id: P_CROSS_GAP,
        kind: Kind::F32,
        apply: Apply::SpecF32(|s, v| s.cross_gap(v)),
        doc: "Space between wrap lines, across the main axis (`gap` stays the space along it).",
    },
    PropDef {
        name: "mainAlign",
        id: P_MAIN_ALIGN,
        kind: Kind::Enum(ALIGNS),
        apply: Apply::SpecEnum(|s, i| s.main_align(align_idx(i))),
        doc: "Child alignment along the main axis. `start`, `center` and `end` put the children together; `spaceBetween` deals the free space out between them (none at the ends), `spaceAround` gives each child an equal share split to its two sides, and `spaceEvenly` makes every gap and both ends equal — CSS's `justify-content`. The spread is added to `gap`, and there is none when nothing is free: a `grow` child takes it all, and an overflowing run keeps its gaps. `baseline` means nothing here and lays out as `start`, with a warning.",
    },
    PropDef {
        name: "crossAlign",
        id: P_CROSS_ALIGN,
        kind: Kind::Enum(ALIGNS),
        apply: Apply::SpecEnum(|s, i| s.cross_align(align_idx(i))),
        doc: "Child alignment across the main axis. On a row, `baseline` lines up the first baselines of the children's text, so a label and a larger value read as one line; a child with no text aligns by its bottom edge, a `grow` or percent height fills the line from its top, and a fit-height row grows to hold the aligned children. A column lays `baseline` out as `start` (as CSS does), and the three spreads mean nothing across an axis — each with a warning.",
    },
    PropDef {
        name: "aspectRatio",
        id: P_ASPECT_RATIO,
        kind: Kind::F32,
        apply: Apply::SpecF32(|s, v| s.aspect_ratio(v)),
        doc: "Width over height — `16/9`, `1` for a square — CSS's `aspect-ratio`. It sizes the axis left `fit`: a fit height is the final width over the ratio (so `width: grow` and a ratio is a box that keeps its shape as the window resizes), and a fit width under a fixed height is that height times it. With both axes declared, or a fit width under a `grow` or percent height, it has nothing it can set and warns. The derived axis is neither shrunk nor fitted to the children, which overflow it; `minHeight: 'fit'` floors it at them. On an image it wins over the pixels' own aspect.",
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
        doc: "Corner radius for all four corners (logical px); the per-corner props override it when listed after it. On a node that also clips or scrolls it rounds the clip as well, so children stay inside the corners.",
    },
    PropDef {
        name: "opacity",
        id: P_OPACITY,
        kind: Kind::F32,
        apply: Apply::SpecF32(|s, v| s.opacity(v)),
        doc: "Group opacity 0..1 (default 1): fades this node and its whole subtree. A per-quad alpha multiply rather than an offscreen composite, so overlapping pieces of one subtree show their seams through the fade. Layout, hit-testing and the access tree are untouched; eases with `transition`, and `enter: { opacity: 0 }` fades a panel in.",
    },
    PropDef {
        name: "pixelSnap",
        id: P_PIXEL_SNAP,
        kind: Kind::Flag,
        apply: Apply::SpecFlag(|s| s.pixel_snap()),
        doc: "Paint this node's background, border, shadow and fragment with each edge on a whole physical pixel: `x` and `x + width` rounded on their own, from where layout put them, as a text's span backgrounds are. Off by default, and a box is drawn where layout put it, so a 1 px `gap` between boxes is there at any scale. On, boxes that share an edge in layout meet on one pixel line, where a join inside a pixel was drawn by halves and left a seam — rows of a band stacked at a pitch that is not whole pixels, or a box that continues a text's selection. Layout, hit-testing, the clip and the children are untouched. A snapped box can draw up to half a pixel from its layout edge and its size can differ by a pixel, so a snapped hairline is 1 or 2 px thick by where it sits.",
    },
    PropDef {
        name: "shadowColor",
        id: P_SHADOW_COLOR,
        kind: Kind::Color,
        apply: Apply::SpecColor(|s, c| s.shadow_color(c)),
        doc: "Drop-shadow color; nothing else about a shadow draws without it. On its own it is a hard shadow exactly behind the node — add `shadowBlur` / `shadowY` to lift it. Outer shadows only, and the shape is not knocked out of the middle, so a translucent background shows it through.",
    },
    PropDef {
        name: "shadowBlur",
        id: P_SHADOW_BLUR,
        kind: Kind::F32,
        apply: Apply::SpecF32(|s, v| s.shadow_blur(v)),
        doc: "Drop-shadow blur radius (logical px): the edge ramps over this distance and reaches this far past the shape. 0 = a hard edge.",
    },
    PropDef {
        name: "shadowX",
        id: P_SHADOW_X,
        kind: Kind::F32,
        apply: Apply::SpecF32(|s, v| s.shadow_x(v)),
        doc: "Drop-shadow horizontal offset (logical px).",
    },
    PropDef {
        name: "shadowY",
        id: P_SHADOW_Y,
        kind: Kind::F32,
        apply: Apply::SpecF32(|s, v| s.shadow_y(v)),
        doc: "Drop-shadow vertical offset (logical px); positive casts downward.",
    },
    PropDef {
        name: "shadowSpread",
        id: P_SHADOW_SPREAD,
        kind: Kind::F32,
        apply: Apply::SpecF32(|s, v| s.shadow_spread(v)),
        doc: "Grows (or, negative, shrinks) the drop shadow's shape before blurring (logical px).",
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
        name: "animate",
        id: P_ANIMATE,
        kind: Kind::Flag,
        apply: Apply::SpecFlag(|s| s.animate()),
        doc: "Ask for another frame after this one, every frame this node is declared. What a `fragment` that reads `time` needs, and what anything driving itself off the clock rather than off input needs. Opt-in like `exit`, and for the same reason: it takes the loop off input-driven and onto the display's cadence for as long as it is declared, so a still node must not carry it. One node asking is enough for the whole window.",
    },
    PropDef {
        name: "accent",
        id: P_ACCENT,
        kind: Kind::Flag,
        apply: Apply::SpecFlag(|s| s.accent()),
        doc: "Paint this node's background in the OS accent colour — `env.system.accent` — keeping the declared `bg` on a host that cannot tell what it is. The one prop whose paint depends on the environment, which is why it is opt-in: the same tree is a different colour on two machines, and that is the point here and a surprise anywhere else. On the stock button it does the whole job — the hover and pressed shades are derived from the accent, and the label goes black or white by its luminance, so a yellow accent is still readable — which is what `<button accent>` is for.",
    },
    PropDef {
        name: "selectable",
        id: P_SELECTABLE,
        kind: Kind::Flag,
        apply: Apply::SpecFlag(|s| s.selectable()),
        doc: "Makes this node a selection scope: the text of every node inside it is one selectable run, in tree order, and a press-drag across them selects the lot — as do Shift with Left / Right / Home / End on a focused node inside it, a character or a word at a time, a scope with nothing selected anchoring at its start (`docs/adr/0017-selection-as-a-scope.md`). Declared on the container and not on each label, because what a reader selects is a paragraph or a card rather than one run of it — three labels in a column under one `selectable` select as three lines of one text. The selection is the window's: starting one anywhere clears the last, an editor's included. Scopes do not nest; an outer one around an inner one is warned about (`nested-selection-scope`) and the innermost owns the text. Text scrolled out of view inside the scope is still part of it — selection and copy reach it, hit-testing does not. On a `cells` grid the scope selects in cells rather than in bytes: a drag takes lines (with a modifier, a rectangle), a double click the word under the pointer and a triple click the whole row, its ends are absolute lines so a scroll does not move them, and a copy trims each line's trailing blanks.",
    },
    PropDef {
        name: "focusable",
        id: P_FOCUSABLE,
        kind: Kind::Flag,
        apply: Apply::SpecFlag(|s| s.focusable()),
        doc: "Reachable by Tab (and focused by a click) without a click payload or a control role — a row that opens on Enter. Editors, key sinks, `onClick` boxes and the control roles are focusable already.",
    },
    PropDef {
        name: "keepFocus",
        id: P_KEEP_FOCUS,
        kind: Kind::Flag,
        apply: Apply::SpecFlag(|s| s.keep_focus()),
        doc: "A press on this node, or anywhere inside it, leaves keyboard focus where it was: a toolbar button, a tab or a divider that acts without taking the keyboard from the editor or key sink that had it. Without it a press on an `onClick` node focuses the node, and the app's keys stop reaching the sink until it takes focus back. The press also leaves a text or cell selection and the Tab ring where they were, so a Copy button copies what was selected. An `<edit>` inside still takes its caret and focus, as the keyboard's own owner. The click, drag and hover are unchanged, and Tab and assistive technology still reach the node.",
    },
    PropDef {
        name: "focusRegion",
        id: P_FOCUS_REGION,
        kind: Kind::Flag,
        apply: Apply::SpecFlag(|s| s.focus_region()),
        doc: "Makes this node's subtree a focus region: a Tab ring of its own that the ring outside never enters and that never leaves — a devtools dock, an inspector beside the app (`docs/adr/0022-focus-regions.md`). Entered on purpose: `focusRegion(name)` (`Ui::focus_region`, `env.focus_region`, `kui_focus_region`) moves focus in — to the focus the region last held, else its `initialFocus`, else its first stop — and `focusRegion(null)` moves it back to the main ring the same way; a press inside the region, or an explicit focus on a node in it, enters it too. Tab then walks that ring alone, wrapping inside it; with nothing focused, Tab enters the ring of the region in effect (`region()`). A region that stops being declared hands focus back to what the main ring last held. Only the ring is scoped: keys still bubble through the boundary to the sink above (a region that wants its own keymap is an `onKey` sink), the pointer and assistive technology see a plain node, and a `modal` in effect is the ring wherever it sits. Nested regions are skipped by the outer ring the way the main ring skips them.",
    },
    PropDef {
        name: "scrollbar",
        id: P_SCROLLBAR,
        kind: Kind::Enum(SCROLLBARS),
        apply: Apply::SpecEnum(|s, i| s.scrollbar(crate::spec::ScrollbarMode::ALL[i])),
        doc: "When a scrolling node draws its bars: `visible` (the default — the stock overlay thumb, drawn while the content overflows), `hidden` (no thumb, no track to press; the wheel, the keyboard, `reveal` and the caret still scroll it — for a list that draws its own indicator, or a pane whose bar would sit on a border), or `auto` (shown while the scroll state is changing — the offset or the content's extent moved, the pointer is on the track, a thumb is dragged — and for a second after, then faded out over a quarter of one; a node first seen shows it the same second; what an overlay bar does on macOS). `auto` needs the driver's clock and is `visible` without one. The bars are overlays and take no layout space in any mode. From the last change until it has faded — a second and a quarter — an `auto` bar asks for frames the way a transition of that length would (nothing else could wake the core when the hold ends); while the pointer holds it, it asks for none.",
    },
    PropDef {
        name: "anchor",
        id: P_ANCHOR,
        kind: Kind::Flag,
        apply: Apply::SpecFlag(|s| s.anchor()),
        doc: "Scroll anchoring on a scrolling node (backlog C26, CSS's `overflow-anchor`): the first child in view keeps its place on screen when the content before it changes size — a chat that prepends history, a log that inserts rows above the viewport, a list whose row heights are corrected as they are measured — with no `setScroll` and no arithmetic in the view. The core remembers which child was first in view and where its edge was, and moves the offset by however far that edge moved in the next layout, before the offset is clamped; a wheel notch or a `setScroll` between the frames is kept and the correction added to it. The child is found by key, so give the rows stable keys (a `key` or an `index`); a child that is gone anchors nothing that frame. On the scroll axis that is the node's main axis only — `scrollY` on a column, `scrollX` on a row — and content appended *after* the anchor moves nothing, so a log that is tailing still asks for the end itself.",
    },
    PropDef {
        name: "scrollbarWidth",
        id: P_SCROLLBAR_WIDTH,
        kind: Kind::F32,
        apply: Apply::SpecF32(|s, v| s.scrollbar_width(v)),
        doc: "The thumb's width at rest, logical px (default 4); under the pointer or dragged it is 2 px wider. The grabbable track grows to fit a wide thumb.",
    },
    PropDef {
        name: "scrollbarColor",
        id: P_SCROLLBAR_COLOR,
        kind: Kind::Color,
        apply: Apply::SpecColor(|s, c| s.scrollbar_color(c)),
        doc: "The thumb at rest; the default is the theme's `scrollbar` role, a translucent wash over whatever it sits on.",
    },
    PropDef {
        name: "scrollbarActiveColor",
        id: P_SCROLLBAR_ACTIVE_COLOR,
        kind: Kind::Color,
        apply: Apply::SpecColor(|s, c| s.scrollbar_active_color(c)),
        doc: "The thumb under the pointer or while dragged; the default is the theme's `scrollbar_active` role.",
    },
    PropDef {
        name: "overscroll",
        id: P_OVERSCROLL,
        kind: Kind::Enum(OVERSCROLLS),
        apply: Apply::SpecEnum(|s, i| s.overscroll(crate::spec::Overscroll::ALL[i])),
        doc: "What a scroll gesture that starts over this scroller does when it is already at its limit that way (backlog F107, CSS's `overscroll-behavior`): `auto` (the default) passes the gesture on to the scroller around it, `contain` keeps it here, moving nothing until it turns back. A gesture picks its target once, when it starts — the innermost scroller under the pointer that can still move the way it goes — and keeps it until it ends, wherever the pointer or the content has gone; one that reaches a limit midway stops there, whatever this says. Only on the axes the node scrolls: a `scrollY` list that contains still passes a sideways swipe to the strip around it. For a panel or a popup's list whose scrolling must never move what is behind it.",
    },
    PropDef {
        name: "disabled",
        id: P_DISABLED,
        kind: Kind::Flag,
        apply: Apply::SpecFlag(|s| s.disabled(true)),
        doc: "Inert: no click, drag or key sink, no hover / pressed / focus background, skipped by Tab, reported disabled to assistive technology; hover tracking stays so a `tooltip` can say why.",
    },
    PropDef {
        name: "hoverBg",
        id: P_HOVER_BG,
        kind: Kind::Color,
        apply: Apply::SpecColor(|s, c| s.hover_bg(c)),
        doc: "Background while hovered (or while any node in its hoverGroup is); implies hover tracking, eases with `transition`.",
    },
    PropDef {
        name: "gradient",
        id: P_GRADIENT,
        kind: Kind::Gradient,
        apply: Apply::SpecGradient(|s, g| s.gradient(g)),
        doc: "A gradient painted over the node's `bg` and under its border and its children (`docs/adr/0042-a-gradient-is-an-image-the-core-paints.md`): `{ to: 'bottom', stops: [...] }` towards a side or a corner (`right`, `bottom left`, …; the default is `bottom`), `{ angle: 0.125, stops }` in turns clockwise from east, or `{ radial: true, at: [0.5, 0], stops }` out from a centre (fractions of the box, the middle by default) to its farthest corner. A stop is a colour — a `$token` too — or `[colour, position]` with the position 0 to 1; stops without one are spaced evenly between those with. Two stops at one position are a hard edge. The gradient is defined on the box's unit square and stretched to it, so a side or a corner is CSS's and any other `angle` runs corner to corner at an eighth of a turn whatever the box's aspect, where CSS's pixel-measured `45deg` does not. Stops mix in straight sRGB with the alpha premultiplied, as CSS's do. What it costs is one image quad: the core rasterizes each distinct gradient once into the glyph atlas — a 256-texel strip along an axis, a 128-texel square otherwise, within half an 8-bit level of the gradient computed per pixel for a linear one and 1.2 for a radial — keyed by the gradient and not the box, so a box that resizes and a thousand boxes that share one rasterize nothing, and a host that draws an image draws it; a gradient box costs about 55 ns over a flat one, so ten thousand of them are half a millisecond. A hard edge is as soft as the raster stretched to the box (a 256th of its length along a strip); stripes are boxes. It does not tween — `transition` eases the `bg` under it and `opacity` fades it — and `hoverBg` and the other state backgrounds replace `bg`, not the gradient; one that changes every frame is a raster a frame, and a shimmer is a `fragment`'s. Ignored on a `line`, a `polygon` and a `path`. Fewer than two stops are an error in JSX and Lua, as a malformed `keyframes` is, and draw nothing in Rust and C. A stop whose `$token` misses is not an error: it is raised as `unknown-token` and left out, as a miss leaves any slot unset, and the rest are spaced as if it had not been declared — so a gradient left with fewer than two stops, a two-stop one with a typo, draws nothing over its `bg` (backlog RG118).",
    },
    PropDef {
        name: "backdropBlur",
        id: P_BACKDROP_BLUR,
        kind: Kind::F32,
        apply: Apply::SpecF32(|s, v| s.backdrop_blur(v)),
        doc: "Blur what was drawn beneath the node, inside its rounded box, by this radius in logical px — CSS's `backdrop-filter: blur()`, the radius its standard deviation (backlog F129). What blurs is everything painted before the node: its ancestors' backgrounds, the siblings under it, content scrolling beneath it, the window's `backdrop` where the window has one. The node's own `bg`, border and children paint over the blur, so a translucent `bg` (`#ffffff40`) makes frosted glass and an opaque one hides it. Clipped as the node is, faded by its `opacity`; 0 is none. The GPU renderer reads back only the box (and a margin of three radii around it) and blurs it at reduced resolution, so it costs a copy and three small passes per blurred node on a frame that has one and nothing on a frame that does not. A renderer that cannot read back what it drew — a host's own, or anything older — leaves the node over an unblurred backdrop; the display list carries it as a `backdrop` quad (`KUI_QUAD_BACKDROP` in C) either way.",
    },
    PropDef {
        name: "rotate",
        id: P_ROTATE,
        kind: Kind::F32,
        apply: Apply::SpecF32(|s, v| s.rotate(v)),
        doc: "Turns this node and everything under it, in turns clockwise (0.25 is a quarter turn right), about its pivot — the centre unless `pivotX` / `pivotY` say — after layout (`docs/adr/0043-a-node-turns-about-its-pivot.md`). Paint-only: the node takes the room its upright self takes, nothing around it moves, `onLayout` reports the layout rect. Everything the subtree draws turns with it — backgrounds, borders, shadows, text, images, strokes, fragments — and so does what it clips: a child cut by a turned card's rounded corners stays inside them. Hit where drawn: a tilted card is grabbed on its tilted edge and a press in its box past its edge falls through; drag payloads stay in viewport px. The access rect is the bounding box. Nests by composition. Tweens with `transition` as one slot with `scale`, and an entrance, an exit or a keyframe stop may name it (`enter: { rotate: -0.02 }`, `keyframes: [{ rotate: 0 }, { rotate: 1 }]` spins a box). A float anchored to the parent turns with it; a viewport float does not. On a `path` this is the path's own turn (ADR 0041), which does not tween — wrap it in a box for one that does. Text under a turn leaves the pixel grid, as a turned mask does. `backdropBlur` under a turn blurs the upright box.",
    },
    PropDef {
        name: "scale",
        id: P_SCALE,
        kind: Kind::F32,
        apply: Apply::SpecF32(|s, v| s.scale(v)),
        doc: "Scales this node and everything under it by this factor, uniformly, about its pivot, after layout (`docs/adr/0043-a-node-turns-about-its-pivot.md`); 1 is none. 0 draws nothing in Rust, Node and Lua; in C and Odin, where a zeroed field is unset, 0 is 1. Paint-only, as `rotate` is: layout, the room taken and `onLayout` are the upright node's; what it draws, clips and hits scales. Tweens with `transition` as one slot with `rotate`; `enter: { scale: 0.8 }` settles a chip in, `keyframes: [{ scale: 1.05, at: 0.5 }]` pulses. The edge ramps scale with the box, so a box scaled far up reads soft.",
    },
    PropDef {
        name: "pivotX",
        id: P_PIVOT_X,
        kind: Kind::F32,
        apply: Apply::SpecF32(|s, v| {
            let fy = s.transform_spec().map_or(0.5, |t| t.pivot.y);
            s.pivot(v, fy)
        }),
        doc: "Where across the box `rotate` and `scale` are about, as a fraction of its width: 0 the left edge, 0.5 (the default) the middle, 1 the right edge; outside 0..1 is a point past the box. C: `pivot_x` with `pivot_set`.",
    },
    PropDef {
        name: "pivotY",
        id: P_PIVOT_Y,
        kind: Kind::F32,
        apply: Apply::SpecF32(|s, v| {
            let fx = s.transform_spec().map_or(0.5, |t| t.pivot.x);
            s.pivot(fx, v)
        }),
        doc: "Where down the box `rotate` and `scale` are about, as a fraction of its height: 0 the top, 0.5 (the default) the middle, 1 the bottom. C: `pivot_y` with `pivot_set`.",
    },
    PropDef {
        name: "rules",
        id: P_RULES,
        kind: Kind::Color,
        apply: Apply::SpecColor(|s, c| s.rules(c)),
        doc: "On a table (`dir=\"table\"`, ADR 0033): grid lines of this colour between its columns and between its rows (backlog DX21) — down the middle of each gap between the columns of its widest row, from the first row's top to the last row's bottom, and across the middle of each gap between rows, the content box wide. Drawn with the table's box, under its cells and on whole pixels, so give the table and its rows a `gap` at least `ruleWidth` for the lines to show between cells; the outer edge is the table's `border`. Ignored on anything but a table.",
    },
    PropDef {
        name: "ruleWidth",
        id: P_RULE_WIDTH,
        kind: Kind::F32,
        apply: Apply::SpecF32(|s, v| s.rule_width(v)),
        doc: "The width of a table's `rules` in logical px; 1 when unset.",
    },
    PropDef {
        name: "dropBg",
        id: P_DROP_BG,
        kind: Kind::Color,
        apply: Apply::SpecColor(|s, c| s.drop_bg(c)),
        doc: "Background while files dragged in from the OS are over this node (ADR 0031); wins over pressedBg, focusBg and hoverBg, clears when they leave, land or the drag is cancelled. Implies hover tracking, eases with `transition`.",
    },
    PropDef {
        name: "pressedBg",
        id: P_PRESSED_BG,
        kind: Kind::Color,
        apply: Apply::SpecColor(|s, c| s.pressed_bg(c)),
        doc: "Background while pressed (or while its hoverGroup is); implies hover tracking.",
    },
    PropDef {
        name: "focusBg",
        id: P_FOCUS_BG,
        kind: Kind::Color,
        apply: Apply::SpecColor(|s, c| s.focus_bg(c)),
        doc: "Background while the node holds keyboard-visible focus (moved there by Tab or assistive technology, not a click); replaces the default focus ring. Pressed wins over focus wins over hover; eases with `transition`.",
    },
    PropDef {
        name: "modal",
        id: P_MODAL,
        kind: Kind::Tag,
        apply: Apply::SpecMsg(|s, v| s.modal(v)),
        doc: "Modal surface: the Tab ring becomes this node's subtree, everything outside it is inert to the pointer, the wheel and assistive technology, and Escape or a press outside emits {kind:\"dismiss\", reason:\"escape\"|\"outside\", tag} on it — the app stops declaring the node. The last one declared in tree order is the one in effect (a confirm inside a dialog); a modal that must cover the app is a float. The access tree is not pruned to the modal: it keeps every node of the frame and marks the one in effect `modal` (`docs/adr/0003-modal-surfaces.md`, decision 7), which is what assistive technology acts on.",
    },
    PropDef {
        name: "initialFocus",
        id: P_INITIAL_FOCUS,
        kind: Kind::Flag,
        apply: Apply::SpecFlag(|s| s.initial_focus()),
        doc: "Where focus lands when the enclosing `modal` scope is entered: the first node in the modal\'s Tab ring declaring it, so a destructive confirm opens on its Cancel rather than on whichever control is declared first. Read on entry only — a Tab press afterwards stands, and the scope re-entered (a nested confirm closing) leaves focus where it was. Declared on nothing, or only on nodes the ring skips (disabled, `role=\"none\"`, not focusable), entry stays the ring\'s first node.",
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
        kind: Kind::Tag,
        apply: Apply::SpecMsg(|s, v| s.on_hover(v)),
        doc: "Hover tag: the pointer entering/leaving emits {kind:\"hover\", phase:\"enter\"|\"leave\", tag} events.",
    },
    PropDef {
        name: "onDrop",
        id: P_ON_DROP,
        kind: Kind::Tag,
        apply: Apply::SpecMsg(|s, v| s.on_drop(v)),
        doc: "Drop-zone tag (`docs/adr/0031-a-drop-zone-is-a-row-and-the-files-are-an-event.md`): files dragged in from the OS over this node emit {kind:\"drop\", phase:\"enter\"|\"move\"|\"leave\"|\"drop\", paths, x, y, tag} — `paths` the OS paths as strings, `x`/`y` the pointer in logical viewport coordinates (absent on `leave`). The zone under the files is the topmost zone by paint order: a node inside a zone is the zone's (a button in it, a field in it), and a node that is no zone and has none enclosing it is looked past, so an overlay shown on `enter` cannot make the zone lose the files. No `leave` follows a `drop`; a drop off every zone is refused by the driver. Implies hover tracking. No access row — a screen-reader user's way in is a button beside the zone. On Windows and Linux the position is the OS cursor at enter and release only, so `move` never fires there.",
    },
    PropDef {
        name: "onLayout",
        id: P_ON_LAYOUT,
        kind: Kind::Tag,
        apply: Apply::SpecMsg(|s, v| s.on_layout(v)),
        doc: "Layout tag: the node's laid-out rect arrives as {kind:\"layout\", x, y, w, h, parent, tag} on its first frame and whenever it changes (needs a stable key).",
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
        kind: Kind::Tag,
        apply: Apply::SpecMsg(|s, v| s.on_drag(v)),
        doc: "Drag tag: emits {kind:\"drag\", phase, x, y, dx, dy, parent, tag} events, `dx`/`dy` measured from the press point in every phase. On a `cells` grid the events also carry `cell: {row, col}`; inside an `onKey` sink that draws `role=\"line\"` rows they carry `line`, `byte` and `clicks` — see the events table.",
    },
    PropDef {
        name: "onKey",
        id: P_ON_KEY,
        kind: Kind::Tag,
        apply: Apply::SpecMsg(|s, v| s.on_key(v)),
        doc: "Key-sink tag: with key focus held, presses arrive as {kind:\"key\", phase:\"down\", code, ...} events. Releases only with `keyUp` beside it.",
    },
    PropDef {
        name: "keyUp",
        id: P_KEY_UP,
        kind: Kind::Flag,
        apply: Apply::SpecFlag(|s| s.key_up()),
        doc: "With `onKey`: releases arrive too, as the same payload with phase:\"up\" (`text` null, `repeat` false) — for a held-key interaction (WASD, press-and-hold, a key that arms a mode while it is down). A key only comes up where it went down: a release whose press the sink never got is dropped, and focus leaving while a key is held delivers the `up` first, so nothing is left stuck down. Without it a sink hears presses only, which is what a keymap wants — one that heard both halves would run every binding twice.",
    },
    PropDef {
        name: "modifierKeys",
        id: P_MODIFIER_KEYS,
        kind: Kind::Flag,
        apply: Apply::SpecFlag(|s| s.modifier_keys()),
        doc: "With `onKey`: the modifier and lock keys arrive as keys of their own (backlog F108) — `code` \"shift\", \"ctrl\", \"alt\", \"super\", \"capslock\", \"numlock\", \"scrolllock\", which side in `location` (\"left\" / \"right\"), releases too with `keyUp`. Without it a modifier is only ever held — the next key's `shift`, `ctrl`, … and the `modifiers` event — so a keymap mid-sequence never reads a Shift as a key between two others. For a terminal speaking kitty's keyboard protocol, or a game that binds a lone Shift.",
    },
    PropDef {
        name: "onContextMenu",
        id: P_ON_CONTEXT_MENU,
        kind: Kind::Tag,
        apply: Apply::SpecMsg(|s, v| s.on_context_menu(v)),
        doc: "Context-menu tag: a secondary-button (right) press emits {kind:\"contextmenu\", x, y, tag} on the node, at the logical viewport point to open the menu at. The press moves no focus, places no caret and produces no click, so right-clicking a selection keeps it. Asked of the topmost node under the pointer, and when that node offers no menu the press reaches the nearest enclosing node that does — a container declaring a menu for everything inside it is the common case — the way an unclaimed key reaches the enclosing sink (`docs/adr/0011`): the event carries the *owner's* key and tag, a nested declaration wins over its ancestor's, a disabled node's own is skipped, and the walk stops at the modal boundary.",
    },
    PropDef {
        name: "onForceClick",
        id: P_ON_FORCE_CLICK,
        kind: Kind::Tag,
        apply: Apply::SpecMsg(|s, v| s.on_force_click(v)),
        doc: "Force-click tag: a press that deepens past the second stage of a Force Touch trackpad emits {kind:\"forceclick\", x, y, tag} on the node, at the logical viewport point it happened at (`docs/adr/0017-selection-as-a-scope.md`). Routed as a secondary press is — no focus moved, no caret placed, no click — but asked of the topmost node only, with no walk to an enclosing declaration — and the ordinary click the press is still producing arrives afterwards, as it does on macOS. Text needs none of this: a force click over an `edit` or a `selectable` scope selects the word under it and asks the host for its Look Up panel. macOS-only in practice, and there the user can switch the gesture off, so nothing may declare itself the only way to reach something.",
    },
    PropDef {
        name: "onButton",
        id: P_ON_BUTTON,
        kind: Kind::Tag,
        apply: Apply::SpecMsg(|s, v| s.on_button(v)),
        doc: "Button tag (backlog F105): a press of a non-primary button — middle, secondary, or one past those — emits {kind:\"button\", phase:\"press\", button, x, y, clicks, tag} on the node, and the button is then captured by it: every pointer move while it is held arrives as phase:\"move\" and its release as phase:\"release\", on this node wherever the pointer is. `button` is `\"secondary\"`, `\"middle\"` or a further button's number (3 and up); `x`/`y` are logical viewport coordinates, and on a `cells` grid each event carries `cell: {row, col}` as a click does. Several buttons can be held at once, each its own capture, and a primary drag is untouched. `buttons` says which buttons it claims — all of them unless it narrows them. Asked of the topmost node under the pointer, and when that node claims no such button the press reaches the nearest enclosing node that does, the way a context menu's does: a disabled node's own is skipped and the walk stops at the modal boundary. A claimed secondary press is this event *instead of* a `contextmenu` event and the stock menu (a nearer `onContextMenu` still wins, being the nested declaration). Like every non-primary press it moves no focus, places no caret and touches no selection or scrollbar. For a terminal's middle-click paste, and the mouse reports a program in it asked for.",
    },
    PropDef {
        name: "buttons",
        id: P_BUTTONS,
        kind: Kind::Str,
        apply: Apply::SpecStr(|s, v| s.buttons(crate::input::Buttons::parse(v))),
        doc: "Which non-primary buttons `onButton` claims (backlog F105): `\"secondary\"`, `\"middle\"` and `\"other\"` (every button past those), separated by spaces or commas — `\"middle\"`, `\"secondary middle\"`. Unset, all three: a node that wants the middle button and leaves the secondary one to its context menu says `\"middle\"`. A word that is none of the three is skipped, so a string of none of them claims nothing, and a typo never takes the secondary button from a context menu. Meaningless without `onButton`.",
    },
    PropDef {
        name: "onScroll",
        id: P_ON_SCROLL,
        kind: Kind::Tag,
        apply: Apply::SpecMsg(|s, v| s.on_scroll(v)),
        doc: "Scroll tag: the wheel over this node emits {kind:\"scroll\", x, y, dx, dy, lines, tag} on it instead of scrolling anything — `dx`/`dy` the delta in logical px as the driver reported it (positive `dy` is the wheel rolling up, toward earlier content), `x`/`y` the pointer, and `lines` on a `cells` grid the whole lines the delta covers (positive = later history, the sign `originLine` grows in; the fraction is carried to the next notch so a trackpad's small steps add up) and null on any other node. The node takes the wheel on the axes `scrollAxes` names (both unless it narrows them): a gesture that starts over it is its own whether or not it has anywhere to go — except on an axis the node also scrolls (`scrollX`/`scrollY`, its offset the app's to set), where it is answered by its room as a container is, so at its edge a gesture that way passes to the scroller around it (backlog F118) — and stays its own until it ends, wherever the pointer goes (backlog F107); it reaches no scroll container above it, and a scroller inside it still takes the axes it scrolls while it can move that way, passing this node the rest — the other axis, and a gesture that begins with that scroller at its limit (`overscroll: \"contain\"` on the scroller keeps it there). The core moves nothing — a grid re-declares `originLine`, a canvas zooms. A drag-select held past a `cells` grid's top or bottom edge arrives here too, once a frame with the lines that frame scrolled by (`docs/adr/0029-a-selection-follows-the-pointer-past-the-edge.md`).",
    },
    PropDef {
        name: "scrollAxes",
        id: P_SCROLL_AXES,
        kind: Kind::Enum(SCROLL_AXES),
        apply: Apply::SpecEnum(|s, i| s.scroll_axes(crate::spec::ScrollAxes::ALL[i])),
        doc: "Which axes `onScroll` takes (backlog F107): `both` (the default), `x` or `y`. A scroll gesture on an axis the node does not take passes it by, to the scroller around it, and hears nothing here: a terminal that scrolls its history says `y`, and a sideways swipe that starts over it moves the strip it sits in. (A swipe that started elsewhere is not the node's either way: a gesture keeps the target it started with.) Meaningless without `onScroll`.",
    },
    PropDef {
        name: "scrollMods",
        id: P_SCROLL_MODS,
        kind: Kind::Str,
        apply: Apply::SpecStr(|s, v| s.scroll_mods(crate::input::KeyMods::parse(v))),
        doc: "The modifiers `onScroll` is for (backlog F122): `\"shift\"`, `\"ctrl\"`, `\"alt\"` and `\"super\"` (⌘, the Windows key), separated by spaces or commas — `\"ctrl super\"`. With any named, the node hears only a scroll gesture that began with one of them held, and hears it first: ahead of every scroll container and every `onScroll` that names none, wherever under the pointer the gesture began, the innermost such node winning — so a Ctrl-wheel zoom declared on the window's root is heard over a list, and the list does not scroll. A wheel with none of them held passes the node by, as if it had no `onScroll`: a node that scrolls as well (`overflow`) scrolls for it as any container does. Its `scroll` events carry `mods`, the modifiers held when the gesture began; the gesture stays the node's to the end of its glide, whatever is let go meanwhile, and one begun without them never becomes its. A word that is none of the four is skipped. Unset, a handler like any other. Meaningless without `onScroll`.",
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
        name: "cursor",
        id: P_CURSOR,
        kind: Kind::Enum(CURSORS),
        apply: Apply::SpecEnum(|s, i| s.cursor(cursor_idx(i))),
        doc: "The pointer shape over this node. Unset, the pointer is `text` over an editor or a `selectable` scope and `default` over everything else — an `onClick`, `focusable` or `onDrag` node included, as a native button is — so a hand (`pointer`) over a control, a `grab` over a handle (and `grabbing` while its drag runs, which the view declares as its drag state changes), a splitter's `ewResize` / `nsResize` and a `disabled` control's `notAllowed` are all declared. The stock `button` declares `pointer` itself. A captured drag keeps the dragged node's shape wherever the pointer goes.",
    },
    PropDef {
        name: "transition",
        id: P_TRANSITION,
        kind: Kind::F32,
        apply: Apply::SpecF32(|s, v| s.transition(v)),
        doc: "Animate sizing/colors/radius changes over this many ms — and, on a scroll container, the offset a reveal or a set_scroll moves it to (needs a stable key).",
    },
    PropDef {
        name: "easing",
        id: P_EASING,
        kind: Kind::Enum(EASINGS),
        apply: Apply::SpecEnum(|s, i| s.easing(easing_idx(i))),
        doc: "Easing for `transition` (default easeOut). The springs — `smooth` (no overshoot), `snappy`, `spring` and `bouncy` (the most), each a `bounce` of its own — integrate with momentum, so a value retargeted mid-flight keeps moving the way it was; `transition` is then about how long one takes to get there. Between `keyframes` stops a spring is drawn as `easeOut` (see that row).",
    },
    PropDef {
        name: "bounce",
        id: P_BOUNCE,
        kind: Kind::F32,
        apply: Apply::SpecF32(|s, v| s.bounce(v)),
        doc: "How far a spring overshoots its target: 0 glides in with none, 0.5 bounces visibly, and values past 0.9 are held there (a spring at 1 would never settle). It replaces a spring `easing`'s own bounce (`smooth` 0, `snappy` 0.15, `spring` 0.25, `bouncy` 0.5), and on a timed easing makes the transition a spring — so `transition` plus `bounce` is a spring of that length and bounce. It does nothing between `keyframes` stops, which are sampled off the clock (see that row).",
    },
    PropDef {
        name: "slide",
        id: P_SLIDE,
        kind: Kind::Flag,
        apply: Apply::SpecFlag(|s| s.slide()),
        doc: "With transition: also ease the node's position (reordered siblings slide). While it eases, the node is drawn between where it was and where this frame put it — not at the declared `dx`/`dy`, or its slot in the row — so anything else positioned from those numbers drifts for the transition's length: a canvas of floats eases everything or nothing.",
    },
    PropDef {
        name: "keyframes",
        id: P_KEYFRAMES,
        kind: Kind::Keyframes,
        apply: Apply::SpecKeyframes(|s, k| s.keyframes(k)),
        doc: "CSS-style stops `[{ at?, dx?, dy?, width?, height?, bg?, radius?, opacity?, rotate?, scale? }, …]`: the slots they name cycle through them over `transition` ms, for ever unless `iterations` says how many times, without the view redrawing; `at` is 0..1 and spreads evenly when omitted. `dx` / `dy` are logical px from where layout put the node (backlog F132), as an entrance's are: the node and its subtree are drawn and hit that far away at the stop, a lane a stop leaves out is 0, and the offset adds to a `slide`'s, so `[{ dy: 0 }, { dy: -6 }]` with `repeat: 'alternate'` bobs a box and a sparkle drifts up its stops. Paint, hit and access only: layout and the room the node takes are its own place's, and an `onLayout` node reports its layout rect, not the cycle, which would post an event every frame it runs. The `easing` applies to each step between two stops, as CSS applies its timing function per keyframe; a spring easing there is drawn as `easeOut` and `bounce` does nothing, since a cycle is sampled off the clock and a spring has to be integrated (backlog F141) — an overshoot in a cycle is a stop past the target, `[{ scale: 1 }, { scale: 1.15, at: 0.6 }, { scale: 1 }]`.",
    },
    PropDef {
        name: "enter",
        id: P_ENTER,
        kind: Kind::Enter,
        apply: Apply::SpecEnter(|s, e| s.enter(e)),
        doc: "Where the node starts the first frame it is seen `{ dx?, dy?, width?, height?, bg?, radius?, opacity?, rotate?, scale? }`: those slots ease in from there over `transition` ms instead of snapping (`dx`/`dy` slide it in from that far away, `opacity: 0` fades the whole subtree in, `scale: 0.8` settles it in).",
    },
    PropDef {
        name: "exit",
        id: P_EXIT,
        kind: Kind::Enter,
        apply: Apply::SpecEnter(|s, e| s.exit(e)),
        doc: "Where the node ends the frame after the view stops declaring it `{ dx?, dy?, width?, height?, bg?, radius?, opacity?, rotate?, scale? }` — an `enter` read the other way. It plays when the node itself is removed, its parent still declared; a node that goes because an ancestor went — a tab switched away, a panel closed around it — goes at once with it, unless that ancestor has an `exit` of its own, whose picture carries it (backlog DX19; React's `AnimatePresence` rule). With a `transition`, the departing subtree is copied out of the last frame that had it and replayed frozen, in its place (the pass it painted in, just under the node that painted after it — a panel under a HUD leaves under it) and inert (no clicks, no Tab stop, no access row) while those slots ease from where they were, then dropped; without one it vanishes at once as it always did. `width`/`height` resize the departing node's own box only — the subtree inside it is a picture and is not laid out again. The exit read is the one the last frame that had the node declared, unless `exit_with` named another for the frame it went in: a card a button throws left or right is aimed by the handler that removes it, with no frame drawn first to point it. Needs a stable key across frames.",
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
        name: "iterations",
        id: P_ITERATIONS,
        kind: Kind::F32,
        apply: Apply::SpecF32(|s, v| s.iterations(v)),
        doc: "How many times the `keyframes` cycle runs (CSS `animation-iteration-count`, backlog F133); left out, for ever. A finite cycle plays from the first frame the node is declared with it — a node that leaves and comes back plays again — holds its first stop through its `delay`, and rests where its last iteration ended (CSS's fill `both`): `1` plays a burst or a shake once, `2` with `repeat: \"alternate\"` goes out and back and ends where it began, `0.5` stops halfway. Once it is over the node owes no frame, so `animating()` and `owed()` go quiet as a settled transition's do; `delay` plus `iterations: 1` staggers one-shots. A count that is not a positive number is for ever. C: 0 is for ever.",
    },
    PropDef {
        name: "clickSound",
        id: P_CLICK_SOUND,
        kind: Kind::Resource,
        apply: Apply::SpecResource(|s, id| s.click_sound(crate::resources::SoundId::from_ffi(id))),
        doc: "A registered sound (addSound) played when the node is clicked; implies hover tracking.",
    },
    PropDef {
        name: "hoverSound",
        id: P_HOVER_SOUND,
        kind: Kind::Resource,
        apply: Apply::SpecResource(|s, id| s.hover_sound(crate::resources::SoundId::from_ffi(id))),
        doc: "A registered sound (addSound) played when the pointer enters the node; implies hover tracking.",
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
        kind: Kind::Family,
        apply: Apply::StyleFamily(|t, f| t.family(f)),
        doc: "Font family: `sans`, `serif` or `mono`, kui's own, or the name of an installed family or one loaded with `loadFontsDir` / `loadFontFile` — `\"Berkeley Mono\"` — drawn in its face in the frame that names it (ADR 0037). A name is matched as `addSystemFont` matches it and registered in the session on first sight, exactly as the font database spells it (`\"menlo\"` is not `\"Menlo\"`); the session's first registration of any font maps the installed font files once (~30 ms on a Mac, backlog DX24), which a family named in a view pays in that frame. `systemFonts()` lists the names there are. A name nothing matches shapes as sans and raises `unknown-family`. It and `font` set the same thing, so declare one.",
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
            3 => t.wrap(TextWrap::BreakSpaces),
            _ => t.wrap(TextWrap::Word),
        }),
        doc: "Line breaking at the node's width: between words (default), anywhere, never (one line per paragraph, clipped to the node), or between words with whitespace taking its room (`break-spaces`: a space that does not fit starts the next row rather than hanging past the edge — an editor's wrapped line). On a single-line `edit` — a field, which otherwise takes one line and scrolls it — declaring it is what makes the field fold to its width like a document, by this mode, while Enter still submits (see `edit`).",
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
    PropDef {
        name: "underline",
        id: P_UNDERLINE,
        kind: Kind::Flag,
        apply: Apply::StyleFlag(|t| t.underline()),
        doc: "A line under the text, where the face puts its underline and as thick as it says, in the text colour. Paint only. On a `<span>` it covers the span alone and follows it across a wrap, one rect per line. `underlineColor` gives it a colour of its own and `underlineStyle` a shape; either implies it.",
    },
    PropDef {
        name: "underlineColor",
        id: P_UNDERLINE_COLOR,
        kind: Kind::Color,
        apply: Apply::StyleColor(|t, c| t.underline_color(c)),
        doc: "The underline's own colour — a diagnostic's red under keyword-coloured text (backlog K4). Implies `underline`. On a `<span>` the span's; a span with no colour of its own takes the text's.",
    },
    PropDef {
        name: "underlineStyle",
        id: P_UNDERLINE_STYLE,
        kind: Kind::Enum(UNDERLINE_STYLES),
        apply: Apply::StyleEnum(|t, i| t.underline_style(UnderlineStyle::from_index(i as u32))),
        doc: "The underline's shape (backlog K4): `solid` (the face's line), `wavy` (three strokes tall around the line, a six-stroke period — a diagnostic's squiggle, a terminal's undercurl) or `dotted` (dots two strokes across, four apart). Implies `underline`. A wave or dots are runs of the segment primitive a `line` draws, so no backend learns a kind; the cost is two quads per period.",
    },
    PropDef {
        name: "strikethrough",
        id: P_STRIKETHROUGH,
        kind: Kind::Flag,
        apply: Apply::StyleFlag(|t| t.strikethrough()),
        doc: "A line through the text, where the face puts its strikeout. Paint only; on a `<span>` the span alone, per line.",
    },
    PropDef {
        name: "bold",
        id: P_BOLD,
        kind: Kind::Flag,
        apply: Apply::StyleFlag(|t| t.bold()),
        doc: "The family's bold, on a whole text or an editor (backlog F150) — a heading, a table's header, a title field: its bold face, or its regular drawn bold where the family has none, as a `<span bold>` is. A span inside a bold text is bold too.",
    },
    PropDef {
        name: "features",
        id: P_FEATURES,
        kind: Kind::Str,
        apply: Apply::StyleStr(|t, s| t.features(crate::spec::FontFeatures::parse(s))),
        doc: "OpenType features for the shaper, as `tag=value` pairs separated by spaces or commas — a bare `tag` is 1, `-tag` is 0: `\"liga=0 calt=0\"` keeps a coding font from joining `->` and `!=` (what a terminal built on runs needs to hold its grid), `\"tnum\"` lines figures up in a gutter, `\"ss01\"` picks a stylistic set. Unset, the font's own defaults apply. At most 8; part of what the text is shaped as, so two texts differing only here are shaped twice.",
    },
    PropDef {
        name: "role",
        id: P_ROLE,
        kind: Kind::Enum(ROLES),
        apply: Apply::SpecEnum(|s, i| s.role(role_idx(i))),
        doc: "What the node is to assistive technology. Unset, the core derives one (an `onClick` node is a button, an editor a text input, a scrolling box a scroll view, a plain box nothing); `none` hides the node and its subtree from the access tree — the decorative door, for what a reader need not hear: an icon beside the text that says the same, or a caption under the button it repeats. A text takes no `role` (it has no box), so `none` goes on the box around it; `ambiguous-name` points at it when a caption and its control share a name (backlog F140). Where the caption is all a control says, a `label` on the control that says what it does is the better answer. A `radio` belongs inside a `radioGroup` and a `tab` inside a `tabList`, labelled with what the choice is: the pair is a composite (`docs/adr/0007-composite-keyboard-patterns.md`) — one Tab stop for the set, the arrows, Home and End moving the choice inside it (each step is the item's click, so the choice follows focus), and a screen reader reading \"2 of 3\". A `radio` or `tab` with no container above it is a Tab stop of its own that no arrow moves, and the core warns (`item-outside-container`). `menu` holds `menuItem`s and `list` holds `listItem`s the same way.",
    },
    PropDef {
        name: "label",
        id: P_LABEL,
        kind: Kind::Str,
        apply: Apply::SpecStr(|s, v| s.label(v)),
        doc: "The accessible name. Without one a button, link, tab or heading is named by the text inside it; an image, an icon-only button and a `modal` dialog have none, and the core warns (`image-without-label`, `control-without-name`, `modal-without-name`). Not the key label a `key` prop or `with_keyed` declares, which `key_of` looks up and a reader never hears; `key_named` looks a node up by this name.",
    },
    PropDef {
        name: "description",
        id: P_DESCRIPTION,
        kind: Kind::Str,
        apply: Apply::SpecStr(|s, v| s.description(v)),
        doc: "The accessible description: the extra sentence a reader says after the name, for what the name cannot say on its own — what a button will do, why a control is disabled, what format a field wants. `tooltip` is the shorthand that also draws the string and hover-tracks the node; this is the description alone, for a hint that is spoken and never drawn. Both write the one slot, so a node declaring both keeps whichever its binding applied last. It reads only on a node that reaches the access tree — a role, a label, a control — since a plain box is elided and takes its description with it.",
    },
    PropDef {
        name: "checked",
        id: P_CHECKED,
        kind: Kind::Flag,
        apply: Apply::SpecFlag(|s| s.checked(true)),
        doc: "The on state of a `checkbox` / `radio` / `switch` role.",
    },
    PropDef {
        name: "mixed",
        id: P_MIXED,
        kind: Kind::Flag,
        apply: Apply::SpecFlag(|s| s.mixed(true)),
        doc: "A `checkbox` that is neither on nor off — the select-all box over a list some of whose rows are selected (ADR 0034). Read as mixed by assistive technology whatever `checked` says, and drawn as a dash by the stock `<checkbox>`. Meaningful on the checkbox role alone.",
    },
    PropDef {
        name: "selected",
        id: P_SELECTED,
        kind: Kind::Flag,
        apply: Apply::SpecFlag(|s| s.selected(true)),
        doc: "The current one of a set: which `tab` a `tabList` shows, which `listItem` a list has picked, which `link` is the page you are on. A `tab` always carries the state — its siblings read as \"not selected\" — while a list row or a link carries it only where it is set, since an ordinary list or navigation bar is not a selection and a reader saying \"not selected\" on every row of it is noise.",
    },
    PropDef {
        name: "expanded",
        id: P_EXPANDED,
        kind: Kind::Enum(EXPANDED),
        apply: Apply::SpecEnum(|s, i| s.expanded(i == 1)),
        doc: "A disclosure's state: what a node that shows and hides something (a twisty, an accordion header, a menu button) reads as. Unset, the node does not expand at all — which is why this names its state instead of being a flag.",
    },
    PropDef {
        name: "live",
        id: P_LIVE,
        kind: Kind::Enum(LIVE),
        apply: Apply::SpecEnum(|s, i| s.live(crate::access::Live::from_index(i))),
        doc: "Marks this node a live region: when the text inside it changes, a screen reader reads the change without being asked — `polite` at the next pause, `assertive` interrupting. Put it on the smallest node that holds the message, since everything inside a live node is live. For a one-off with no node behind it (\"Saved\") the binding's `announce` verb is the other half.",
    },
    PropDef {
        name: "valueNow",
        id: P_VALUE_NOW,
        kind: Kind::F32,
        apply: Apply::SpecF32(|s, v| s.value_now(v)),
        doc: "A `slider` role's current value (the drawing stays yours; this is what assistive technology reads).",
    },
    PropDef {
        name: "valueMin",
        id: P_VALUE_MIN,
        kind: Kind::F32,
        apply: Apply::SpecF32(|s, v| s.value_min(v)),
        doc: "A `slider` role's minimum.",
    },
    PropDef {
        name: "valueMax",
        id: P_VALUE_MAX,
        kind: Kind::F32,
        apply: Apply::SpecF32(|s, v| s.value_max(v)),
        doc: "A `slider` role's maximum.",
    },
    PropDef {
        name: "valueText",
        id: P_VALUE_TEXT,
        kind: Kind::Str,
        apply: Apply::SpecStr(|s, v| s.value_text(v)),
        doc: "What a `slider` role's position reads as (ARIA's `aria-valuetext`). Without one a reader has only `valueNow` and the range and says a percentage — 25 in [5..60] is \"36 percent\" — so a value whose unit carries the meaning says it here: \"25 minutes\". It replaces the number in the reading rather than joining it, and a nudge announces the new text. Meaningful on the slider role alone, like the three numbers; putting the reading in `label` instead renames the control on every nudge, which is the wrong attribute.",
    },
    PropDef {
        name: "valueStep",
        id: P_VALUE_STEP,
        kind: Kind::F32,
        apply: Apply::SpecF32(|s, v| s.value_step(v)),
        doc: "How far one arrow key moves a `slider` role, and the grid a value the pointer sets snaps to (ADR 0034). Unset, a hundredth of the range. PageUp / PageDown move ten steps. Read by the core only where the slider declares `onChange`; reported to assistive technology either way.",
    },
    PropDef {
        name: "onFocus",
        id: P_ON_FOCUS,
        kind: Kind::Tag,
        apply: Apply::SpecMsg(|s, v| s.on_focus(v)),
        doc: "Keyboard focus entering or leaving this node's subtree — the node itself, or anything focused inside it — emits `{kind:\"focus\", phase:\"in\"|\"out\", by, tag}` (backlog DX18). `by` is what moved it: `pointer` (a press), `keyboard` (Tab, a key a control answered), `assistive` (a screen reader's request) or `program` (the view or the app — `keyFocus`, `setFocus`, a modal's entry). Reported once the move settles, after the input that made it or at the end of the frame that declared it, so an app hears a pane taking the keyboard instead of diffing the focused key every frame. Leaving is reported innermost first, entering outermost first. It makes nothing focusable or interactive.",
    },
    PropDef {
        name: "onChange",
        id: P_ON_CHANGE,
        kind: Kind::Tag,
        apply: Apply::SpecMsg(|s, v| s.on_change(v)),
        doc: "A `slider` role's changes (ADR 0034): the core turns a press on the node into the value under the pointer, a drag into the value under it, the arrows and assistive technology's increment / decrement into one `valueStep`, PageUp / PageDown into ten, Home / End into the range's ends — clamped to `valueMin`..`valueMax` (0..100 unset) and snapped to the step — and emits `{kind:\"change\", value, phase, tag}`: `phase` is `\"move\"` while the pointer holds the slider and `\"end\"` when it lets go or a key moved it. The value is proposed and never applied; the slider moves when the view declares it as `valueNow`. A key that lands where the slider already is proposes nothing. The pointer reads the node's content box along its main axis, so a `dir=\"column\"` slider runs bottom to top. Without it a slider's arrows reach the app as `{kind:\"access\", action}`. Ignored on any other role.",
    },
    PropDef {
        name: "caret",
        id: P_CARET,
        kind: Kind::F32,
        apply: Apply::SpecF32(|s, v| s.caret(v.max(0.0) as u32)),
        doc: "On a `line` of a custom editor (a `textInput` / `multilineTextInput` role drawn by the app): the caret's byte offset into that line's text.",
    },
    PropDef {
        name: "selectionAnchor",
        id: P_SELECTION_ANCHOR,
        kind: Kind::F32,
        apply: Apply::SpecF32(|s, v| s.selection_anchor(v.max(0.0) as u32)),
        doc: "On a `line` of a custom editor: the byte offset where the selection's other end sits (the caret is `caret`, possibly on another line).",
    },
    PropDef {
        name: "caretSolid",
        id: P_CARET_SOLID,
        kind: Kind::Flag,
        apply: Apply::SpecFlag(|s| s.caret_solid()),
        doc: "On a `line` declaring `caret`: the caret is solid — a block caret in a modal editor's normal mode — so the driver's blink clock is not armed on it and `caretVisible` stays true, while the offset still anchors the IME and reads to assistive technology. Without it a declared `caret` is a caret to blink, and the one thing that asks an idle app for a frame twice a second; an editor whose caret only blinks while typing declares this on every other mode's line.",
    },
];

/// A prop every binding handles by hand (a composite with real logic, or a
/// constructor-order special), with its wire id so transports agree on
/// identity and its per-binding spelling so the docs can say so.
pub struct CustomProp {
    pub name: &'static str,
    pub id: u32,
    /// Every prop name a JSX view may write for it — the machine-readable
    /// half of `jsx`, which is prose for the docs. A composite is spelled
    /// differently in each binding (`borderW` here, `border = {…}` there),
    /// so the two lists are separate; together with `PROPS` and an element's
    /// own props they are the whole allow-list a binding checks a view
    /// against (see [`known_prop`]).
    pub jsx_names: &'static [&'static str],
    /// The same for a Lua node table.
    pub lua_names: &'static [&'static str],
    /// How JSX spells it.
    pub jsx: &'static str,
    /// How a Lua table spells it.
    pub lua: &'static str,
    /// Where it lands in C.
    pub c: &'static str,
    /// Where it lands in Odin (packages/odin): a `Spec` / `Text_Style`
    /// field, or a `kui.` procedure for a window-level row. The Odin
    /// generator checks every name here is one its package has.
    pub odin: &'static str,
    pub doc: &'static str,
}

pub const CUSTOM: &[CustomProp] = &[
    CustomProp {
        name: "dir",
        id: P_DIR,
        jsx_names: &["dir"],
        lua_names: &[],
        jsx: "`dir=\"row\" | \"column\" | \"table\"`",
        lua: "`row { }` / `column { }` / `grid { }`",
        c: "`dir` (`KUI_ROW` / `KUI_COLUMN` / `KUI_TABLE`)",
        odin: "`Spec.dir` (`.Column` / `.Row` / `.Table`); `kui.row` and `kui.column` set it",
        doc: "Main axis; column is the default. `table` is a column whose rows' children line up in columns (the `table` element).",
    },
    CustomProp {
        name: "size",
        id: P_SIZE,
        jsx_names: &["size"],
        lua_names: &["size"],
        jsx: "`size` (text)",
        lua: "`size`",
        c: "`KuiTextStyle.size`; `KuiSpan.size`",
        odin: "`Text_Style.size`",
        doc: "Font size in logical px; the text style is constructed from it, so declare it for the other style props to apply at that size.",
    },
    CustomProp {
        name: "pad",
        id: P_PAD,
        jsx_names: &["pad", "padX", "padY", "padL", "padR", "padT", "padB"],
        lua_names: &["pad"],
        jsx: "`pad`, `padX`, `padY`, `padL`, `padR`, `padT`, `padB`",
        lua: "`pad = n` or `pad = { all=, x=, y=, l=, r=, t=, b= }`",
        c: "`pad_l`, `pad_r`, `pad_t`, `pad_b`",
        odin: "`Spec.pad`: `kui.pad(16)`, `kui.pad(16, 8)`, or `{l = .., r = .., t = .., b = ..}`",
        doc: "Padding; a frontend reports the names it saw and `PadShorthand::resolve` turns them into four edges — an edge falls back to its axis, an axis to the all-round `pad`, and the specific one always wins.",
    },
    CustomProp {
        name: "border",
        id: P_BORDER,
        jsx_names: &["borderW", "borderColor"],
        lua_names: &["border"],
        jsx: "`borderW`, `borderColor`",
        lua: "`border = { w=, color= }`",
        c: "`border_w`, `border_color`",
        odin: "`Spec.border_w`, `Spec.border_color`",
        doc: "Border width and color (drawn inside the rect).",
    },
    CustomProp {
        name: "overflow",
        id: P_OVERFLOW,
        jsx_names: &["clip", "scrollX", "scrollY"],
        lua_names: &["clip", "scroll", "scroll_x", "scroll_y"],
        jsx: "`clip`, `scrollX`, `scrollY`",
        lua: "`clip`, `scroll_x`, `scroll_y` (`scroll` = `scroll_y`)",
        c: "`overflow` bits `KUI_CLIP` | `KUI_SCROLL_X` | `KUI_SCROLL_Y`",
        odin: "`Spec.overflow`: `{.Clip}`, `{.Scroll_X}`, `{.Scroll_Y}`",
        doc: "Clip children; scroll (implies clip) with retained offsets and live scrollbars. A `radius` on the same node rounds the clip, so a rounded card does not show square corners poking out of it; nesting two rounded clippers keeps only the corners neither of them moved, and hit-testing stays rectangular. Every frontend ORs the same bits and hands them to `NodeSpec::overflow_bits`. The wheel goes to the scroller under the pointer on the axes it scrolls, and the rest of the notch to the one around it: a `scrollY` list inside a `scrollX` strip moves the strip on a sideways swipe (backlog DX13). A scroll gesture — a swipe and its glide, a wheel spun without a pause — picks that scroller when it starts, skipping one already at its limit that way for the one around it (unless it says `overscroll: contain`), and keeps it until it ends, so content moving under a still pointer does not hand the rest of a swipe to what came under it (backlog F107). An offset is kept while the key is declared; an undeclared one is kept until the budget needs the room (1024 undeclared entries, longest-undeclared evicted first).",
    },
    CustomProp {
        name: "float",
        id: P_FLOAT,
        jsx_names: &["float"],
        lua_names: &["float"],
        jsx: "`float=\"below\" | \"above\" | \"parent\" | \"viewport\"` or `{ anchor, at, self, dx, dy, fit, clip }`",
        lua: "`float = \"below\"` or `float = { anchor=, at=, self=, dx=, dy=, fit=, clip= }`",
        c: "`float_mode`, `float_anchor_x/y`, `float_self_x/y`, `float_dx/dy`, `float_fit`, `float_clip`; `kui_spec_float_preset` fills them from a preset name",
        odin: "`Spec.float`, a `Float` (`mode`, `anchor_x/y`, `self_x/y`, `dx/dy`, `fit`, `clip`); `kui.float_preset` fills one from a preset name",
        doc: "Out-of-flow positioning against the parent or the viewport; `fit` flips/clamps to stay on screen. A float escapes every ancestor's clip — a tooltip is not cut by the scroller it hangs from — unless it declares `clip` and is anchored to its parent (`parent`, `below`, `above`): then the parent's clip holds it as it holds a child, so a node on a `clip` canvas panned past the canvas's edge is cut there and cannot be hit past it; it still paints as a layer over its in-flow siblings. A `line`, `polygon` or `path` in its parent's box is always clipped this way. The four preset names resolve in `FloatConfig::preset`, and `anchor` takes any of them — an override left out keeps the preset's own value, so `{ anchor: \"below\", dx: 4 }` still hangs below with its 6px gap. A float is a layer of its own: above the in-flow tree and every float that opened before it, under every float that opened after, and hit-tested in the same order — so a tooltip that appears over an open menu is over it, and a popover over a scroller's bar takes the press there. A scroller's bars and the focus ring belong to the layer that owns them. There is no z-index; a float declared under a fresh key reopens on top (`docs/adr/0023-layers-stack-in-the-order-they-open.md`).",
    },
    CustomProp {
        name: "keyFocus",
        id: P_KEY_FOCUS,
        jsx_names: &["keyFocus"],
        lua_names: &["key_focus"],
        jsx: "`keyFocus`",
        lua: "`key_focus`",
        c: "`kui_set_key_focus`",
        odin: "`Spec.key_focus`, which calls `kui.set_key_focus` on the node",
        doc: "Focuses this node (an `onKey` sink, an editor, any focusable node) when it starts being declared: declared every frame it takes focus once, so a later Tab press is not clobbered. Declaring it on the frame a `modal` stops being declared is how a view says where focus lands on the way out — the edge stands, and the focus the modal displaced is not handed back over it (`docs/adr/0003-modal-surfaces.md`, decision 4). To move focus at any time call the binding's focus verb (`ctx.focus`, `kui_focus`, `env.set_focus`).",
    },
    CustomProp {
        name: "key",
        id: P_KEY,
        jsx_names: &["key"],
        lua_names: &["key"],
        jsx: "`key`",
        lua: "`key`",
        c: "`kui_open_keyed` label",
        odin: "`Spec.key`, which opens the node keyed",
        doc: "Stable identity for retained state (scroll offsets, editors, transitions; keys are hashes of the path from the root). Retained state outlives the key's absence, under a budget on the states nobody declares (see `<edit>` and the overflow props).",
    },
    CustomProp {
        name: "index",
        id: P_INDEX,
        jsx_names: &["index"],
        lua_names: &["index"],
        jsx: "`index`",
        lua: "`index`",
        c: "`kui_open_indexed`",
        odin: "`Spec.index`, a `Maybe(u64)`, which opens the node indexed",
        doc: "Stable identity by *data* index rather than by name: the key auto-keying would have given this node as the `i`th child, given to it wherever it actually sits. What a virtualised list is for — a view that builds rows 900..930 of ten thousand opens each with its own row number, so the row keeps its hover, focus, edit buffer and tweens as the built range slides over it, and a list that builds every row agrees with one that builds a screenful. Wherever `key` names a node this numbers it (a box, a `line`, a `cells`, a `fragment`); declared beside `key` the index wins. Indices and names are separate namespaces, so a spacer keyed `\"lead\"` cannot collide with row 0 — but two rows on one index do, exactly as two on one name would.",
    },
    CustomProp {
        name: "rowCount",
        id: P_ROW_COUNT,
        jsx_names: &["rowCount"],
        lua_names: &["row_count"],
        jsx: "`rowCount`",
        lua: "`row_count`",
        c: "`kui_row_count`",
        odin: "`Spec.row_count`, a `Maybe(u64)`, which calls `kui.row_count` on the node",
        doc: "How many `index`ed rows this node's virtual list has, built or not. `uniformList` / `uniform_list` / `widgets::uniform_list` and `widgets::list` declare it on their container; a list composed by hand says it beside `scrollY`. What it buys: Select All (Cmd/Ctrl-A, the menu's row) inside a `selectable` virtual list selects the *data*, rows `0..rowCount`, rather than the rows the frame built, and the copy is a `selectionrange` ask whose `to.byte` is past the last row's length when that row is not built — cut it to the row. Without it Select All is the built rows, which is all the core can see.",
    },
    CustomProp {
        name: "title",
        id: P_TITLE,
        jsx_names: &["title"],
        lua_names: &["window_title"],
        jsx: "`title` (root box only)",
        lua: "`window_title` (root table)",
        c: "`kui_window_title`",
        odin: "`kui.window_title`",
        doc: "Declares the window title for this frame; the driver diffs and applies.",
    },
    CustomProp {
        name: "alwaysOnTop",
        id: P_ALWAYS_ON_TOP,
        jsx_names: &["alwaysOnTop"],
        lua_names: &["always_on_top"],
        jsx: "`alwaysOnTop` (root box only)",
        lua: "`always_on_top = true` (root table)",
        c: "`kui_set_always_on_top`",
        odin: "`kui.set_always_on_top`",
        doc: "Declares that this frame wants the window kept above every other app's — a floating palette, a picture-in-picture player, a timer (backlog C30). Frame state the way `title` is, applied by the driver on change and free on the frames it does not change, but with a default of false rather than \"leave as-is\": a frame that stops declaring it lowers the window again, so a pin button is a toggle on the app's own state and nothing has to remember to undo it. Whether the platform has a level to set is `env.window.always_on_top`, which is what the pin button should draw its state from — Wayland has no call for it at all, so there the window never moves and the reading says so; it is the driver's record of what it set, not a query, so a level the OS dropped afterwards (a fullscreen space, a tiling manager) is not reported. A popup keeps its own level whatever its owner declares.",
    },
    CustomProp {
        name: "secureInput",
        id: P_SECURE_INPUT,
        jsx_names: &["secureInput"],
        lua_names: &["secure_input"],
        jsx: "`secureInput` (root box only)",
        lua: "`secure_input = true` (root table)",
        c: "`kui_set_secure_input`",
        odin: "`kui.set_secure_input`",
        doc: "Declares that this frame wants the keyboard to this window kept from every other process while the window has it — macOS's Secure Keyboard Entry, what a terminal turns on at a password prompt (backlog F85). Frame state the way `alwaysOnTop` is, default false: declare it on every frame the prompt is up, and the frame that stops is what turns it off, so nothing has to remember to undo it. The runner owns the platform call and its balance: `EnableSecureEventInput` is process-wide and counted, and the runner holds one count while a window whose frame asked has the keyboard, giving it back when that window loses the keyboard, closes or stops asking, and at exit — Apple's rule, since while it is on no other process can read the keyboard at all (a launcher's hotkey, a text expander, an accessibility tool). Nothing on Windows or Linux, which have no such switch. A C host with its own loop reads the ask with `kui_secure_input_get` and makes the call itself.",
    },
    CustomProp {
        name: "optionAsAlt",
        id: P_OPTION_AS_ALT,
        jsx_names: &["optionAsAlt"],
        lua_names: &["option_as_alt"],
        jsx: "`optionAsAlt=\"left\"` — `\"none\"`, `\"left\"`, `\"right\"`, `\"both\"` (root box only)",
        lua: "`option_as_alt = \"left\"` (root table)",
        c: "`kui_set_option_as_alt` (`KUI_OPTION_AS_ALT_*`)",
        odin: "`kui.set_option_as_alt` (an `Option_As_Alt`)",
        doc: "Declares which Option keys act as Alt in this window on macOS (backlog F113). On a Mac, Option composes: ⌥m types `µ`, and ⌥u, ⌥e, ⌥i, ⌥n and ⌥` are dead keys that start an accent and wait for the next key, so the press never reaches the app as a key and a keymap binding `<A-u>` never hears it. An Option key named here is Alt instead: it composes nothing and types nothing, and a key under it arrives as a chord of the key the layout prints unmodified — a terminal's \"Option as Meta\", an editor's Alt bindings. `\"left\"` or `\"right\"` leaves the other side composing, so a user keeps `ü` on one Option; `\"both\"` takes both; `\"none\"`, the default, is the Mac's own behaviour. Frame state the way `alwaysOnTop` is: declare it on every frame, and the frame that stops gives the Option keys back to the layout; the runner applies it to the window on change, never per frame. A popup's keys arrive through its owner, so the owner's declaration is the one they are read under. Nothing on Windows or Linux, whose Alt composes nothing. A C host with its own loop reads the ask with `kui_option_as_alt_get` and applies it itself.",
    },
    CustomProp {
        name: "imeOff",
        id: P_IME_OFF,
        jsx_names: &["imeOff"],
        lua_names: &["ime_off"],
        jsx: "`imeOff` (root box only)",
        lua: "`ime_off = true` (root table)",
        c: "`kui_set_ime_off`",
        odin: "`kui.set_ime_off`",
        doc: "Declares that this window takes the keyboard as keys, with the platform's input method off (backlog F125): no composition and no candidate window, and on a Mac no dead key waiting for the next and no press-and-hold — an input method too, so a held letter repeats instead of opening the accent picker, whatever the user's `ApplePressAndHoldEnabled` says. A `key` event's `text` is still the layout's character; what goes is everything the OS would have composed from it. What a modal editor's normal mode wants — `jjjj` is how one moves, and an IME left on eats the keymap — while its insert mode stops declaring it and gets accents, dead keys and the IME back. Frame state the way `alwaysOnTop` is, default false: declare it on every frame the mode wants it, and the frame that stops gives the input method back; the runner applies it to the window on change, never per frame, and a composition in progress when it turns off ends without a commit, as an empty `preedit`. The window's, not a node's: a stock editor focused under it composes nothing either. A popup's keys arrive through its owner, so the owner's declaration is the one they are read under. On Windows and Linux the window's IME is disabled the same way, and only that: their dead keys are the layout's, and still compose. A C host with its own loop reads the ask with `kui_ime_off_get` and applies it itself.",
    },
    CustomProp {
        name: "windows",
        id: P_WINDOWS,
        jsx_names: &["windows"],
        lua_names: &["windows"],
        jsx: "`windows={[{ name, kind?, anchor?, width?, height?, activates? }]}` (root box only; `windows: (model) => [...]` in the loop config)",
        lua: "`windows = { { name=, kind=, anchor=, width=, height=, activates= } }` (root table)",
        c: "`kui_window_declare`",
        odin: "`kui.window_declare`",
        doc: "Declares which windows exist this frame, by stable name (`docs/adr/0004-multi-window.md`). A window opens on the first frame any window's frame declares it — its config is read then and never again, since the user owns its geometry once it exists — and closes on the first frame none does. The driver drains the `Open` / `Close` that result, and the app sees `{kind:\"window\", phase, name, id}`. A window the user closed does not reopen while it is still declared: stop declaring it, then declare it again. `kind: \"popup\"` makes it a menu surface instead: borderless, off the taskbar, owned by the window that declared it and closed with it, placed in screen coordinates against `anchor` — the `{x, y, w, h}` an `onLayout` node reported — and non-activating unless `activates` says otherwise, so the field that opened it keeps the focus ring while the arrows walk the list. A press outside it or Escape raises `{kind:\"dismiss\", reason, name, id}` and closes nothing, exactly as a `modal` node's does: stop declaring the window. Reach for a popup only for the placements a float cannot make — a list taller than the window, a menu with nowhere in-window to go, a panel beside the app; everything else stays `fit` plus a `modal` float, which costs one tree instead of an OS surface.",
    },
    CustomProp {
        name: "tooltip",
        id: P_TOOLTIP,
        jsx_names: &["tooltip"],
        lua_names: &["tooltip"],
        jsx: "`tooltip=\"hint\"`",
        lua: "`tooltip = \"hint\"`",
        c: "`KuiSpec.tooltip` (`kui_tooltip` / `kui_tooltip_with` draw a hint that is not hover-gated)",
        odin: "`Spec.tooltip` (`kui.tooltip` / `kui.tooltip_with` draw a hint that is not hover-gated)",
        doc: "Floats a hint below the node while hovered. All three effects — hover tracking, the accessible description, and the float itself — come from `PropsOut::apply_tooltip`, so no frontend can implement two of them; a Rust view has all three in `NodeSpec::tooltip` (`NodeSpec::apply_tooltip` is the spec half, for a caller that floats the hint itself). The `description` row is that middle effect on its own, for a hint that is spoken and never drawn. On a box or a `fragment` the float is the node's last child; a leaf holds no children — a `line`, `polygon`, `path`, `cells` grid, `image` or `edit` — and its hint floats beside it instead, anchored to it, and lands below its box the same way, out of every clip and flipping above near the window's bottom (backlog RG113; `PropsOut::for_leaf`). A leaf draws its description, which is the hint unless a `description` applied after it overwrote the slot. A `line`, `polygon` or `path` is hovered by its shape, so its hint shows while the pointer is on the stroke or inside the outline, not anywhere in its box.",
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
    ("gradient", "`gradient` (`const KuiGradient *`)"),
    ("repeat", "`repeat` (`KUI_REPEAT_*`)"),
    ("delay", "`delay_ms`"),
    ("iterations", "`iterations` (0 is for ever)"),
    ("enter", "`enter` (`KuiEnter`, with `set` bits)"),
    ("exit", "`exit` (`KuiEnter`, with `set` bits)"),
    ("opacity", "`opacity` with `opacity_set`"),
    ("scale", "`scale` (0 is 1)"),
    ("pivotX", "`pivot_x` with `pivot_set`"),
    ("pivotY", "`pivot_y` with `pivot_set`"),
    ("radiusTL", "`radius_tl` with `per_corner`"),
    ("radiusTR", "`radius_tr` with `per_corner`"),
    ("radiusBR", "`radius_br` with `per_corner`"),
    ("radiusBL", "`radius_bl` with `per_corner`"),
    ("hoverGroup", "`hover_group` (KuiStr)"),
    ("role", "`role` (`KUI_ROLE_*`)"),
    ("expanded", "`expanded` (`KUI_EXPANDED_*`)"),
    ("live", "`live` (`KUI_LIVE_*`)"),
    ("cursor", "`cursor` (`KUI_CURSOR_*`)"),
    ("label", "`label` (KuiStr)"),
    (
        "valueNow",
        "`value_now` with `KUI_VALUE_NOW` in `value_set`",
    ),
    (
        "valueMin",
        "`value_min` with `KUI_VALUE_MIN` in `value_set`",
    ),
    (
        "valueMax",
        "`value_max` with `KUI_VALUE_MAX` in `value_set`",
    ),
    ("valueText", "`value_text` (KuiStr)"),
    ("caret", "`caret` with `KUI_VALUE_CARET` in `value_set`"),
    (
        "selectionAnchor",
        "`selection_anchor` with `KUI_VALUE_ANCHOR` in `value_set`",
    ),
    (
        "onClick",
        "`on_click` argument of `kui_open` / `kui_open_with`",
    ),
    (
        "onDrag",
        "`on_drag` argument of `kui_open_draggable` / `kui_open_with`",
    ),
    ("onKey", "`on_key` argument of `kui_open_with`"),
    (
        "onContextMenu",
        "`on_context_menu` (a borrowed `KuiValue*`, cloned while the node opens)",
    ),
    (
        "onButton",
        "`on_button` (a borrowed `KuiValue*`, cloned while the node opens)",
    ),
    (
        "buttons",
        "`buttons` (`KUI_BUTTONS_*` bits; zeroed, all three)",
    ),
    (
        "modal",
        "`modal` (a borrowed `KuiValue*`, cloned while the node opens)",
    ),
    (
        "onScroll",
        "`on_scroll` (a borrowed `KuiValue*`, cloned while the node opens)",
    ),
    ("onHover", "`on_hover` argument of `kui_open_with`"),
    (
        "onFocus",
        "`on_focus` (a borrowed `KuiValue*`, cloned while the node opens)",
    ),
    ("ruleWidth", "`rule_w`"),
    (
        "overscroll",
        "`overscroll` (`KUI_OVERSCROLL_*`; zeroed, auto)",
    ),
    (
        "scrollAxes",
        "`scroll_axes` (`KUI_SCROLL_AXES_*`; zeroed, both)",
    ),
    (
        "scrollMods",
        "`scroll_mods` (`KUI_KMOD_*` bits; zeroed, none)",
    ),
    (
        "onDrop",
        "`on_drop` (a borrowed `KuiValue*`, cloned while the node opens)",
    ),
    (
        "onLayout",
        "`on_layout` (a borrowed `KuiValue*`, cloned while the node opens)",
    ),
    (
        "family",
        "`KuiTextStyle.family` (`KUI_FONT_*`); `KuiSpan.family` (with `KUI_SPAN_FAMILY`)",
    ),
    (
        "font",
        "`KuiTextStyle.font` (from `kui_font_add*`); `KuiSpan.font`",
    ),
    ("lineHeight", "`KuiTextStyle.line_height`"),
    ("wrap", "`KuiTextStyle.wrap` (`KUI_WRAP_*`)"),
    ("maxLines", "`KuiTextStyle.max_lines`"),
    ("ellipsis", "`KuiTextStyle.ellipsis`"),
    (
        "features",
        "`KuiTextStyle.features` (a `KuiStr`, the same spelling)",
    ),
    (
        "underline",
        "`KuiTextStyle.decoration` (`KUI_DECO_UNDERLINE`); `KuiSpan.flags` (`KUI_SPAN_UNDERLINE`)",
    ),
    (
        "strikethrough",
        "`KuiTextStyle.decoration` (`KUI_DECO_STRIKETHROUGH`); `KuiSpan.flags` (`KUI_SPAN_STRIKETHROUGH`)",
    ),
    (
        "bold",
        "`KuiTextStyle.bold`; `KuiSpan.flags` (`KUI_SPAN_BOLD`)",
    ),
    (
        "underlineColor",
        "`KuiTextStyle.underline_color`; `KuiSpan.underline_color`",
    ),
    (
        "underlineStyle",
        "`KuiTextStyle.underline_style` (`KUI_UNDERLINE_*`); `KuiSpan.underline_style`",
    ),
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

/// The Odin spelling of a schema row: the `Spec` or `Text_Style` field of
/// its snake_case name, which is what the Odin generator names every field.
pub fn odin_field(def: &PropDef) -> String {
    let record = match def.target() {
        Target::Spec => "Spec",
        Target::Style => "Text_Style",
    };
    format!("`{record}.{}`", def.snake_name())
}

/// A C cell in Odin's words, for the rows where Odin's door is C's: the
/// binding's procedures are kui.h's functions under `kui.` without their
/// `kui_`, and its node and text fields are `Spec.` / `Text_Style.`.
/// `RESOURCES` and `ENV_FIELDS` take their Odin column from here, and the
/// Odin generator checks each name it produces.
pub fn odin_from_c(c: &str) -> String {
    c.replace("kui_", "kui.")
        .replace("KuiSpec.", "Spec.")
        .replace("KuiTextStyle.", "Text_Style.")
}

/// An element (node type) and its spelling in each binding. Elements are
/// hand-lowered per binding (their shapes differ: JSX children, Lua
/// tables, C calls with body callbacks), so this table is documentation
/// and a checklist, not a code generator's input.
pub struct ElementDef {
    pub name: &'static str,
    /// The props this element lowers itself, which are therefore in neither
    /// `PROPS` nor `CUSTOM`: `<edit initial multiline>`, `<image src>`. They
    /// ride in the same prop list as the node's, so a binding needs them to
    /// tell a legitimate element prop from a misspelling (see
    /// [`known_prop`]) — JSX's spellings here, Lua's below.
    pub jsx_own: &'static [&'static str],
    /// The same for a Lua node table.
    pub lua_own: &'static [&'static str],
    /// The schema rows this element reads, or `None` for every one. A
    /// composite whose look *is* its spec — the stock button, whose
    /// padding, colours and radius are `widgets::button_spec` — cannot take
    /// the whole prop list: `dir` alone rebuilds the spec from nothing. It
    /// names the rows it reads instead, and a binding drops the rest with a
    /// warning that says which rows it does take ([`known_prop`],
    /// `diag::unknown_prop`). JSX spellings here, Lua's below.
    pub jsx_rows: Option<&'static [&'static str]>,
    pub lua_rows: Option<&'static [&'static str]>,
    pub jsx: &'static str,
    pub lua: &'static str,
    pub c: &'static str,
    /// The Odin procedures (`kui.` and the name); checked by the Odin
    /// generator like the composites' column.
    pub odin: &'static str,
    pub doc: &'static str,
}

/// The rows a `text` reads (`ElementDef::jsx_rows` / `lua_rows`): the
/// `TextStyle` rows and `size`, the composite the style is built from —
/// and nothing else, because every door lowers a text as content plus a
/// style and no spec (`Core::text_node`), so a container or access row
/// on it reaches no tree. Before AR13 the element admitted every shared
/// row, and `<text live="polite">`, `<text role="heading">`, `<text
/// label>` and `<text onClick>` were dropped silently by all four
/// bindings — no `unknown-prop`, and `live-region-without-name` could
/// never fire for them. Pinned equal to the `Target::Style` rows by a
/// test, so a style row added to `PROPS` is a row here or a red test.
pub const TEXT_ROWS_JSX: &[&str] = &[
    "size",
    "lineHeight",
    "color",
    "family",
    "font",
    "wrap",
    "maxLines",
    "ellipsis",
    "underline",
    "underlineColor",
    "underlineStyle",
    "strikethrough",
    "bold",
    "features",
];
pub const TEXT_ROWS_LUA: &[&str] = &[
    "size",
    "line_height",
    "color",
    "family",
    "font",
    "wrap",
    "max_lines",
    "ellipsis",
    "underline",
    "underline_color",
    "underline_style",
    "strikethrough",
    "bold",
    "features",
];

/// The rows the stock button reads (`ElementDef::jsx_rows` / `lua_rows`):
/// the click, the identity (`key`, or `index` in a virtual list — declared
/// beside `key` the index wins, as on a box), the access
/// rows — what a button *is* and what a reader says of it — and the one
/// paint row it takes, `accent`, which is not a colour but a question put
/// to the OS. The two lists are the same rows in each spelling, index for
/// index.
pub const BUTTON_ROWS_JSX: &[&str] = &[
    "onClick",
    "key",
    "index",
    "label",
    "description",
    "tooltip",
    "disabled",
    "accent",
];
pub const BUTTON_ROWS_LUA: &[&str] = &[
    "on_click",
    "key",
    "index",
    "label",
    "description",
    "tooltip",
    "disabled",
    "accent",
];

/// The rows a stock toggle — `checkbox`, `radio`, `switch` — reads
/// (`widgets::toggle_with`): the button's access rows, its state
/// and no layout or paint row, since its look is its spec. `mixed` means
/// something on a checkbox alone.
pub const TOGGLE_ROWS_JSX: &[&str] = &[
    "onClick",
    "key",
    "label",
    "description",
    "tooltip",
    "disabled",
    "checked",
    "mixed",
];
pub const TOGGLE_ROWS_LUA: &[&str] = &[
    "on_click",
    "key",
    "label",
    "description",
    "tooltip",
    "disabled",
    "checked",
    "mixed",
];
/// The rows the stock slider reads (`widgets::slider_with`): the
/// value rows, its change tag, the access rows, and its width — the one
/// piece of its look an app sizes.
pub const SLIDER_ROWS_JSX: &[&str] = &[
    "key",
    "label",
    "description",
    "tooltip",
    "disabled",
    "valueNow",
    "valueMin",
    "valueMax",
    "valueStep",
    "valueText",
    "onChange",
    "width",
    "minWidth",
    "maxWidth",
];
pub const SLIDER_ROWS_LUA: &[&str] = &[
    "key",
    "label",
    "description",
    "tooltip",
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
];

pub const ELEMENTS: &[ElementDef] = &[
    ElementDef {
        name: "box",
        jsx_own: &[],
        lua_own: &[],
        jsx_rows: None,
        lua_rows: None,
        jsx: "`<box>`",
        lua: "`row { }`, `column { }`",
        c: "`kui_open*` … `kui_close`",
        odin: "`kui.box` / `kui.row` / `kui.column` in an `if`, closed at its end; `kui.open` … `kui.close` unscoped",
        doc: "A container: every container prop applies.",
    },
    ElementDef {
        name: "table",
        jsx_own: &[],
        lua_own: &[],
        jsx_rows: None,
        lua_rows: None,
        jsx: "`<box dir=\"table\">`",
        lua: "`grid { }`",
        c: "`kui_open*` with `dir = KUI_TABLE`",
        odin: "`kui.box` with `dir = .Table`",
        doc: "A column whose rows' children line up in columns (`docs/adr/0033-a-table-is-a-column-whose-cells-align.md`): its children are the rows, each row's in-flow children its cells, the nth cell of every row column n, and a column as wide as its widest cell — so a label column sits at its longest label with nothing measured and no width picked by hand, in every binding, since the alignment is the layout's and not a widget's. A cell's `width` says how its column sizes: `fit` (the default) and a fixed number are content the column's fit width is the max of; `grow` makes the whole column grow with the table, `grow` factors splitting the room the fit columns leave; a percent takes its cut of the row; and a column's `minWidth` / `maxWidth` are the strictest its cells declared. Fit columns that overflow the row are compressed toward their floors largest first, as a row's children are, unless the table scrolls x; a fixed column never is. A bare text is a cell too, held at its column's width, so a text straight inside a row is a column; an image straight in a row is a cell the same way, its box the column wide and its own aspect tall, the pixels meeting the box by its `fit` row (wrap an icon in a box to keep its own width). The rows are the table's `row` children, ordinary rows — give them `width=\"grow\"` for the columns to grow into (a `fit` row sits at the columns' width) — with their own `gap` between cells, their own padding, background, click, hover and access rows; a row of a table never wraps (`wrap-ignored`). Anything else straight under the table — a text, a `column` section, another table — is a child with its own width and no cells. The table's own `fit` width is its columns', whatever its rows' sizing, so a table with no width is the aligned list; a `scrollX` table's rows are at least as wide as its columns, and it scrolls to them. Everything else is a column's: `gap` is the space between rows, `scrollY` scrolls them, a float in a row is not a cell. Spelled `grid { }` in Lua, since `table` is Lua's own.",
    },
    ElementDef {
        name: "text",
        jsx_own: &["bold", "italic", "bg", "bgRadius"],
        lua_own: &["value", "spans"],
        jsx_rows: Some(TEXT_ROWS_JSX),
        lua_rows: Some(TEXT_ROWS_LUA),
        jsx: "`<text>` with `<span bold italic underline strikethrough bg bgRadius color family font size>` children",
        lua: "`text(\"s\", {…})`, `text({ \"a\", { \"b\", bold = true, underline = true, bg = 0x.., bg_radius = 4 }, { \"code\", family = \"mono\", size = 13 } })`",
        c: "`kui_text`, `kui_rich_text`",
        odin: "`kui.text`, `kui.rich_text`",
        doc: "Plain or rich text; spans shape as one paragraph, so wrapping crosses style boundaries. A text is content plus a style and no box of its own, so the rows it reads are the style rows (`size`, `lineHeight`, `color`, `family`, `font`, `wrap`, `maxLines`, `ellipsis`, `underline`, `strikethrough`, `features`) and nothing else: a container row, an access row (`label`, `role`, `live`), `key` or `onClick` on a text is dropped with an `unknown-prop` warning naming the rows it does take — put them on the box around it. `wrap`, `maxLines` and `ellipsis` control line breaking. A span's `bg` is a background behind its glyphs alone, one rect per line it spans, so it follows the span across a wrap the way a box around a run cannot. With `bgRadius` (`bg_radius` in Lua, `KuiSpan.bg_radius` in C, `Span::bg_radius` in Rust; logical px) the background is rounded and joined into one shape with every rounded background of the same colour and radius it meets: a piece whose edge touches it exactly on the line above or below and overlaps it sideways, or that meets it end to end on its own line, in this text or another. Its corners are then convex where a line reaches past its neighbour, a fillet where it falls short, and round where nothing meets it — a selection over many rows, or over the wrapped lines of a paragraph, is one rounded outline, joined after every text of the frame is laid out and painted, so it is never a frame behind. Nothing names the shape: two that touch are one; `underline` and `strikethrough` on a span or on the whole text are lines where the face puts them. A span takes a face and a size of its own: `family` (a stock name or an installed family's, as the text's) or `font` (a handle, which wins) — inline code in `mono` inside a sans paragraph — and `size` in logical px, its line height scaled at the paragraph's ratio (`Span::family` / `Span::mono` / `Span::size` in Rust, `KuiSpan.family` with `KUI_SPAN_FAMILY`, `.font` and `.size` in C). Its glyphs are shaped in that face, so a caret, a hit, a selection and the measurement read the same glyphs and byte positions stay exact across the change; a line is as tall as its tallest span, and a span's background is its own height around its glyphs. A paragraph with a sized span is shaped whole, never in chunks. A text with no line breaks that is 4096 bytes or longer (and no `maxLines` or `ellipsis`), plain or spans alike, is shaped in ~1 KB chunks as they come on screen, so a minified bundle or a log line with a blob in it costs the screenful it shows and a keystroke into it — or a span moving along it, an editor's caret — costs the chunk it lands in; wrapped, the rows are broken from the chunks' positions, so a 100k-character paragraph costs the rows it shows. Its size is estimated from the first chunk until the rest shape (exact under monospace), and the access tree carries its value without its runs.",
    },
    ElementDef {
        name: "button",
        jsx_own: &[],
        lua_own: &["text"],
        jsx_rows: Some(BUTTON_ROWS_JSX),
        lua_rows: Some(BUTTON_ROWS_LUA),
        jsx: "`<button onClick key|index label description tooltip disabled accent>`",
        lua: "`button { label=, on_click=, key= | index=, text=, description=, tooltip=, disabled=, accent= }`",
        c: "`kui_button`, `kui_button_with`",
        odin: "`kui.button`",
        doc: "The stock button: `widgets::button_spec(&theme, &metrics)` — the theme's accent trio as its three backgrounds, declared on the node and resolved by the core — keyed by its text (`key` overrides). It paints from the palette like every stock widget (backlog AR41): the OS's accent where the host reports one, the app's where it set or pinned one, kui's blue otherwise; the label goes black or white by the background's luminance. Its look is its spec, so the layout and paint rows are closed — declared, they are dropped with an `unknown-prop` warning naming the rows it does read — and those are the access rows: `label` when the text is not the name, `description`, `tooltip`, and `disabled` (inert, and dimmed to half). The one paint row it takes is `accent`, which on a button changes nothing (it is the accent already) and is kept for the box's sake. In Lua `label` is the name and the text both unless `text` says otherwise; in C the rows ride a `KuiSpec` whose other fields `kui_button_with` ignores. A button that needs any other row is a box with `role=\"button\"` and the same rows spelled out.",
    },
    ElementDef {
        name: "edit",
        jsx_own: &[
            "id",
            "initial",
            "multiline",
            "autofocus",
            "keepTab",
            "placeholder",
        ],
        lua_own: &[
            "initial",
            "multiline",
            "autofocus",
            "keep_tab",
            "placeholder",
        ],
        jsx_rows: None,
        lua_rows: None,
        jsx: "`<edit key initial multiline autofocus keepTab placeholder>`, `<input label initial>`",
        lua: "`edit { key=, initial=, keep_tab=, placeholder=, … }`, `input { label=, initial= }`",
        c: "`kui_text_edit`, `kui_text_edit_placeholder`, `kui_text_input`",
        odin: "`kui.text_edit`, `kui.text_input`",
        doc: "Retained editor state by key; read it back with `editText(key)` after a `changed` event. `initial` seeds a new editor only — a key declared again keeps the draft the user typed, and `setEditText(name, text)` is what resets one (it leaves the caret at the end). Name it by the label its `key` prop declares — `setEditText(\'note\', text)` — or by the hex key an event carried. It reaches an editor that does not exist yet: the text is held for the frame that declares that name and seeds it there, over `initial`, so the `update` that opens a rename field can fill it in the same turn, which is what the label spelling is for — the hex key comes from an event an editor being opened has not fired. A name nothing declares on that frame drops its text with an `edit-text-without-editor` warning. A single-line editor is a field and a `multiline` one a document, which decides how each is laid out as well as how it reads: a field takes one line whatever its box, sizes to the text it holds when its width is `fit`, and scrolls that line under the caret when it is not, while a document wraps to its box. The one exception is a field with `wrap` declared (`wrap=\"word\"` or `\"glyph\"`): it folds to its width the way a document does and keeps a field\'s keyboard — Enter still submits, a newline is still never admitted, the caret still opens at the end — so a rename field breaks where the label it renames breaks, and with `width=\"fit\"` plus `maxWidth` it sizes to its wrapped draft on the keystroke frame. A single-line editor opens with the caret after its seeded text, as a native field does; a multiline one is a document and opens at its top — a held `setEditText` is the call, not a seed, so it opens at the end either way. State is kept while the key is declared; an undeclared one is kept until the budget needs the room (256 undeclared editors, longest-undeclared evicted first). `autofocus` asks once: the editor takes focus on the frame the flag starts being declared — a new editor, or one whose flag just turned on — and only while nothing holds focus, so a blur afterwards stands and a focused control is never robbed (`docs/adr/0022-focus-regions.md`, decision 9); `focus(key)` is the call for taking it at any other time. A focused editor keeps the keys it acts on — the editing keys, what it types, the clipboard and undo chords — and a chord it does not (⌘N, Ctrl+K) goes to the nearest `onKey` sink above it, as a chord bubbles from a control (`docs/adr/0011-keys-bubble-to-the-enclosing-sink.md`, amended 2026-10-09). `keepTab` makes a field's Tab the app's: Tab and Shift-Tab do not walk the focus ring from it, and the press goes to the sink above it the same way, so a list or an outline built from fields indents on Tab; a `multiline` editor keeps Tab for indentation either way. An arrow, Backspace or Delete that meets the edge of the text emits `boundary` (see the events). `placeholder` is what an empty editor shows where its text would be, in the theme's `faint` — never part of the value, gone with the first character typed or composed, and read as the field's accessible description when it declares none.",
    },
    ElementDef {
        name: "select",
        jsx_own: &["label", "options", "current"],
        lua_own: &["label", "options", "current"],
        // Its look is its own, like the button's, and it has no rows of
        // the box's to read: the three it takes are all its own.
        jsx_rows: Some(&[]),
        lua_rows: Some(&[]),
        jsx: "`<select label options={[…]} current>`",
        lua: "`dropdown { label=, options={…}, current= }`",
        c: "`kui_select`",
        odin: "`kui.select`",
        doc: "The stock select (`widgets::select_items`, backlog F72): a field showing the choice in force that, clicked, opens the core's own menu of the options under it with the current one checked — the menu a right-click opens, drawn in the frame or the platform's where the host shows menus itself, dismissed by Escape or a press outside, its rows walked by the arrows and read as a menu. `label` is the key and the accessible name both; `options` is a list whose entries are strings (an option by its label, posting it) or menu-item objects `{ label, id, enabled }` (posting `id`), and a `{ role: \"separator\" }` is a separator; `current` is the index in force, counted from 0 in JSX and C and from 1 in Lua, or none — one past the options or on a separator is none, with a `select-current-ignored` warning on the field; an empty `options` is refused, and a key of an option object no row reads (`disabled`, where the key is `enabled`) is an `unknown-prop` warning. The app holds no open state: the choice arrives as the `menu` event a menu row posts, on the field's key — `{kind: \"menu\", role: \"custom\", item: <the option>}` — and drawing the field again with the new `current` is the whole loop. A reader hears a button named by the field, described by its choice, expanded while the menu is open. Its look is its spec, so it reads no other row: a layout, paint or access row on it is dropped with an `unknown-prop` warning. Lua spells it `dropdown`, since `select` is Lua's own.",
    },
    ElementDef {
        name: "checkbox",
        jsx_own: &[],
        lua_own: &["text"],
        jsx_rows: Some(TOGGLE_ROWS_JSX),
        lua_rows: Some(TOGGLE_ROWS_LUA),
        jsx: "`<checkbox checked mixed onClick key label description tooltip disabled>text</checkbox>`",
        lua: "`checkbox { label=, checked=, mixed=, on_click=, key=, text=, description=, tooltip=, disabled= }`",
        c: "`kui_checkbox`",
        odin: "`kui.checkbox`",
        doc: "The stock checkbox (`widgets::toggle_with`, ADR 0034): a box drawn from the state the view declares — `checked`, or `mixed` for the select-all box over a list some of whose rows are selected, drawn as a dash and read as mixed — and its label beside it, keyed by its text (`key` overrides). The state is the app's: a press by the pointer, Space, Enter or assistive technology posts `onClick`, and the view flips its model and draws it again. Its look is its spec, so the layout and paint rows are closed and dropped with an `unknown-prop` warning; the rows it reads are its state and the access rows. In Lua `label` is the name and the text both unless `text` says otherwise. The box is the metrics' control text plus one (16 px comfortable), so `compact` and `scaled` move it with the stock button.",
    },
    ElementDef {
        name: "radio",
        jsx_own: &[],
        lua_own: &["text"],
        jsx_rows: Some(TOGGLE_ROWS_JSX),
        lua_rows: Some(TOGGLE_ROWS_LUA),
        jsx: "`<radio checked onClick key label description tooltip disabled>text</radio>`",
        lua: "`radio { label=, checked=, on_click=, key=, text=, description=, tooltip=, disabled= }`",
        c: "`kui_radio`",
        odin: "`kui.radio`",
        doc: "The stock radio (`widgets::toggle_with`, ADR 0034): a circle drawn from `checked`, and its label, keyed by its text. Put radios in a `radioGroup`, which makes them one Tab stop whose arrows, Home and End move the choice and press the radio they land on (ADR 0007), so radios whose `onClick` each set the choice answer the keyboard with no more code. The state is the app's, as a checkbox's is; the rows are the checkbox's, `mixed` aside.",
    },
    ElementDef {
        name: "radioGroup",
        jsx_own: &[],
        lua_own: &[],
        jsx_rows: None,
        lua_rows: None,
        jsx: "`<radioGroup label>…radios…</radioGroup>`",
        lua: "`radio_group { label=, … }`",
        c: "`kui_radio_group_open` … `kui_close`",
        odin: "`kui.radio_group` in an `if`, closed at its end",
        doc: "A container of radios (`widgets::radio_group_with`, ADR 0034): the `radioGroup` role, named by its `label`, laid out as a column with the stock gap — a `dir=\"row\"` lays the radios across, and its arrows run across with it. It reads every box row; the role and the name are its own whatever the rows say.",
    },
    ElementDef {
        name: "switch",
        jsx_own: &[],
        lua_own: &["text"],
        jsx_rows: Some(TOGGLE_ROWS_JSX),
        lua_rows: Some(TOGGLE_ROWS_LUA),
        jsx: "`<switch checked onClick key label description tooltip disabled>text</switch>`",
        lua: "`switch { label=, checked=, on_click=, key=, text=, description=, tooltip=, disabled= }`",
        c: "`kui_switch`",
        odin: "`kui.toggle`",
        doc: "The stock switch (`widgets::toggle_with`, ADR 0034): a track and a knob drawn from `checked`, the knob sliding across when it changes, and its label; read as a switch, on or off. The state is the app's, as a checkbox's is; the rows are the checkbox's, `mixed` aside.",
    },
    ElementDef {
        name: "slider",
        jsx_own: &[],
        lua_own: &[],
        jsx_rows: Some(SLIDER_ROWS_JSX),
        lua_rows: Some(SLIDER_ROWS_LUA),
        jsx: "`<slider label valueNow valueMin valueMax valueStep valueText onChange width description tooltip disabled/>`",
        lua: "`slider { label=, value_now=, value_min=, value_max=, value_step=, value_text=, on_change=, width=, … }`",
        c: "`kui_slider`",
        odin: "`kui.slider`",
        doc: "The stock slider (`widgets::slider_with`, ADR 0034): a track, a fill to `valueNow` and a thumb, as wide as a menu (`width` sizes it), keyed by its `label`, which is also its accessible name. With `onChange` the core does the arithmetic: a press proposes the value under the pointer, a drag each new step, the arrows one `valueStep`, PageUp / PageDown ten, Home / End the ends, all clamped to `valueMin`..`valueMax` (0..100 unset) and snapped to the step, as `{kind:\"change\", value, phase:\"move\"|\"end\", tag}`. The value is proposed, never applied: the view stores it and declares it as `valueNow`. Its look is its spec, so the rows it reads are the value rows, the access rows and its width.",
    },
    ElementDef {
        name: "image",
        jsx_own: &["src", "sampling", "fit"],
        lua_own: &["id", "sampling", "fit"],
        jsx_rows: None,
        lua_rows: None,
        jsx: "`<image src={id} sampling fit>`",
        lua: "`image { id=, sampling=, fit= }`",
        c: "`kui_image`, `kui_image_with`",
        odin: "`kui.image`, `kui.image_with`",
        doc: "A registered RGBA image. Sizing: `width=\"fit\"` takes the pixel size, a fit height against a resolved width keeps the aspect, `radius` rounds it. Two rows say how the pixels meet the box (`docs/adr/0025-the-image-is-the-canvas.md`): `sampling` is `linear` (the default) or `nearest` — pixel art, an emulator, a data grid that must stay square under zoom; `fit` is `fill` (the default: the pixels stretch to the box), `contain` (the largest rect of the image's aspect that fits, centred, the rest of the box showing what is behind) or `cover` (the box filled and the pixels that do not fit cropped, centred). The box — its layout, its hit region, its access rect — is the same in every mode. The pixels come from the atlas, or from a texture of the image's own once `updateImage` has replaced them or when no atlas page could hold them; the node cannot tell and need not. Drawn at less than half its texels a pixel, a `linear` image is drawn from a level the core halved it to (`docs/adr/0044-an-image-drawn-smaller-is-drawn-from-a-level.md`): the deepest level with at least a texel a pixel, the 2×2 means taken in linear light, made once per session at the first draw that wants it and kept in the atlas in place of the whole image — so a photo registered as decoded and shown on a card is neither resampled by the app nor aliased on screen. The display's scale and any `scale` the node is drawn through count. `nearest` and an image ever updated (a stream) draw the whole image.",
    },
    ElementDef {
        name: "polygon",
        // `bg` is a schema row already; on a polygon it is the fill.
        jsx_own: &["points"],
        lua_own: &["points"],
        jsx_rows: None,
        lua_rows: None,
        jsx: "`<polygon points={[[x,y],…]} bg/>`",
        lua: "`polygon { points={{x,y},…}, bg= }`",
        c: "`kui_polygon`",
        odin: "`kui.polygon`, its points a `[][2]f32`",
        doc: "A filled polygon through up to eight `points`, the fill in `bg` (`docs/adr/0025-the-image-is-the-canvas.md`, decision 6): an arrowhead, a pie slice, the area under a curve. Placed as a `line` is — always a float in its parent's box space (`float=\"viewport\"` for viewport space), sized to its own bounding box a pixel out on each side, so it takes no room in a row or column, and painted in the parent's layer at its place in the tree, over the siblings before it and under those after (backlog F123). A polygon in its parent's box space is held by the parent's clip as a child is, its hit region with it, so it is cut at a scroller's edge with the row it is drawn in; a declared float is held that way only when it declares `clip` with a parent anchor, and a `float=\"viewport\"` polygon escapes (backlog F78, `docs/adr/0010-a-segment-primitive.md` decision 5). `transition` eases the fill and, with `slide`, its position. The outline may be concave; a self-intersecting one fills even-odd, its overlaps unfilled. Hit by its outline (`docs/adr/0026-hit-testing-by-shape.md`): with `onClick`, `onDrag`, `onHover` or `hoverable` a press inside the outline hits it and one in its box past the outline falls through, so a pie's wedges need no hit boxes; with none it takes no input and has no access row, and with input it derives one as a box would (a clickable wedge is a button), so name it. A ninth point and later are dropped with `polygon-points-truncated`; fewer than three draw nothing; no `bg`, no fill. On the wire it is one `fragment` quad painted by a WGSL function the core registers itself, so a host that draws the list gets its source from `kui_fragment_source` like any other; what it costs is that quad and one pipeline switch per run of polygons. A stroked outline is a closed `line` over it.",
    },
    ElementDef {
        name: "path",
        // `bg` is a schema row already; on a path it is the fill. `width`
        // and `color` are rows too; on a path they are the stroke's, as on
        // a line.
        jsx_own: &["d", "fillRule", "rotate", "pivot", "dash", "dashOffset"],
        lua_own: &[
            "d",
            "ops",
            "fill_rule",
            "rotate",
            "pivot",
            "dash",
            "dash_offset",
        ],
        jsx_rows: None,
        lua_rows: None,
        jsx: "`<path d=\"M … Z\" bg width color fillRule rotate pivot dash dashOffset/>`",
        lua: "`path { d = \"M … Z\", bg=, width=, color=, fill_rule=, rotate=, pivot=, dash=, dash_offset= }`",
        c: "`kui_path`",
        odin: "`kui.path`, `kui.path_d`",
        doc: "Any outline — SVG's `d`, a pie wedge with a round arc, a map's region, an icon — filled with `bg` by `fillRule` (`nonzero`, the default, or `evenodd`) and stroked `width` wide in `color` when `width` is given, the stroke over the fill (`docs/adr/0040-a-path-is-a-mask-in-the-atlas.md`). `d` is SVG path data (`M L H V C S Q T A Z`, absolute or relative), parsed by one parser in the core, so every binding draws the same shape; one that does not parse raises `path-malformed` and draws nothing. JSX also takes `d` as a flat number array of op codes and operands, Lua the same as `ops`, and C only that form (`kui_path_parse` turns a string into it). Placed as a `line` is — always a float in its parent's box space (`float=\"viewport\"` for viewport space), sized to its own bounding box two pixels out on each side (half the stroke's width further), so it takes no room in a row or column, held by the parent's clip as a child is and painted in the parent's layer at its place in the tree (backlog F123). `transition` eases the fill and, with `slide`, its position; the stroke's colour does not tween, as a box's border does not. Hit by its outline under the fill rule (`docs/adr/0026-hit-testing-by-shape.md`): with `onClick`, `onDrag`, `onHover` or `hoverable` a press inside hits it and one in its box past the outline falls through, so a pie's wedges need no hit boxes; a stroke with no fill is hit by its stroke as a line is; with input it derives an access row as a box would (a clickable wedge is a button), so name it. On the wire it is one glyph-mask quad per paint, fill and stroke: the outline is rasterized once per shape, scale and quarter-pixel position into the glyph atlas and tinted like a glyph, so a host that draws text draws paths, and nothing is re-rasterized for a colour tween, a hover or a slide. The fill bleeds half a pixel, so two paths sharing an edge meet without the background showing through; a chart that wants separators gaps its own geometry. A mask a quarter of the biggest atlas page or more, or a path whose ops change twice within a few frames, draws from a texture of its own instead (a `texture` quad), and one past 8192 px on a side draws nothing, with `path-too-large`. `rotate` turns the path, in turns clockwise, about `pivot` — a point in the path's own coordinates, the centre of its box without one (`docs/adr/0041-a-mask-turns-about-its-centre.md`): the turn is the quad's and not the mask's, so a path that only turns — a spinner's arc about its circle's centre — is rasterized once and stays in the atlas at every angle. A path with `rotate` or `pivot` is boxed by the square the turn sweeps, its mask centred on the pivot on a whole pixel, and it is hit where it is drawn; `rotate` does not tween. `dash` and `dashOffset` cut the stroke into marks and gaps as a `line`'s do (backlog V2) — lengths as seen, round-capped marks, a gap the dots overlap closed — restarting at every subpath as SVG's do; the pattern is part of the stroke's mask, so a dashed stroke costs what a solid one does, a stroke with no fill is still hit along its gaps, and a `dashOffset` that changes every frame is a shape that changes every frame: the path leaves the atlas for a texture of its own while it marches.",
    },
    ElementDef {
        name: "fragment",
        jsx_own: &["src", "image", "params", "animate"],
        lua_own: &["id", "image", "params", "animate"],
        jsx_rows: None,
        lua_rows: None,
        jsx: "`<fragment src={id} image={id} params={[…]} animate>`",
        lua: "`fragment { id=, image=, params={…}, animate= }`",
        c: "`kui_fragment`, `kui_fragment_with`",
        odin: "`kui.fragment`, `kui.fragment_with`; `kui.fragment_open` in an `if` holds children",
        doc: "A box a registered WGSL function paints (`docs/adr/0015-a-fragment-element-and-the-painter-it-is-not.md`): a conic or a moving gradient, rings, noise, shimmer — anything the paint vocabulary has no prop for. An ordinary node otherwise — it lays out, rounds, clips, fades, takes input and holds children, which paint over it — but with **no intrinsic size**, so give it a `width`/`height` or `fill` or it is zero by zero. `src` is a handle from `add_fragment`, which validates the source and warns rather than minting one that cannot compile. `params` is up to sixteen numbers the shader reads as four `vec4<f32>`; more are dropped with a warning. `image` is a registered image the function reads — `kui_sample(uv)` (bilinear) and `kui_sample_nearest(uv)` return its texels at `uv` in `[0,1]²`, and `in.image` is its texel rect, `zw` the size — which is what makes a replaced image a waveform, a heatmap, a 50k-point line or an image effect from one quad (`docs/adr/0025-the-image-is-the-canvas.md`, decision 7); the core binds the atlas or the image's own texture, whichever holds it, and a fragment whose image is not live draws nothing, as one whose `src` is not does. `animate` asks for a frame every frame, which is what a fragment that reads `time` needs and what a still one must not declare.",
    },
    ElementDef {
        name: "cells",
        jsx_own: &[
            "rows",
            "cols",
            "cells",
            "cursorAt",
            "cursorShape",
            "cursorColor",
            "originLine",
        ],
        lua_own: &[
            "rows",
            "cols",
            "lines",
            "runs",
            "cursor_at",
            "cursor_shape",
            "cursor_color",
            "origin_line",
        ],
        jsx_rows: None,
        lua_rows: None,
        jsx: "`<cells rows cols cells={Uint32Array} cursorAt={[row, col]} cursorShape cursorColor size family lineHeight/>`",
        lua: "`cells { rows=, cols=, lines={\"row text\", …}, runs={{row, col, len, fg, bg, flags}, …}, cursor_at={row, col}, cursor_shape=, cursor_color=, size=, family= }`",
        c: "`kui_cells`",
        odin: "`kui.cells`",
        doc: "A terminal's screen as one node (backlog C20): `rows × cols` cells, each a character, a foreground and background as `0xRRGGBBAA` (0 = no background), and attribute bits — 1 bold, 2 italic, 4 underline, 8 strikethrough, 16 wide (the glyph spans this cell and the next, which the app leaves blank), 32 the underline is a wave (a terminal's undercurl, SGR 4:3) and 64 dotted (SGR 4:4), either implying it — plus, optionally, the underline's own colour (SGR 58), 0 for the foreground (backlog K4). A glyph is shaped once per character and style variant and thereafter placed at `col × cell_w` without shaping, so a screen whose every cell is new each frame costs what a still one costs (~60 µs for 200 × 50). The cell width is `M`'s advance in the style's font snapped to whole pixels, the height its `lineHeight`; a cell is a cell, so ligatures never form. A character the family has no glyph for is asked of a monospaced face before the platform's fallback list, shaped smaller where it is still wider than its cells (two under wide), and drawn in their middle (backlog F120); the private use area's icons are left as they fall. Box drawing and block elements (U+2500–U+259F) and the Powerline separators (U+E0B0–U+E0BF: the arrows, and the Powerline Extra half circles and wedges) are not shaped at all but drawn from the cell box — a font's are its own line box tall, a cell is `lineHeight` tall, and through the font every `│` was a dash with a gap under it (backlog F66) and a rounded cap a fallback font's squiggle (F112) — so a TUI's frames and rounded rows are seamless in any font, and bold does not thicken a light line (the set has its heavy variants). JSX passes the cells as a `Uint32Array` (or number array) of four entries per cell — codepoint, fg, bg, flags — or five, with the underline colour, in row-major order; Lua a string per row in `lines` plus `runs` of `{row, col, len, fg, bg, flags, ul}` over them (a run's fg, bg or ul of 0 keeps the default: the style's colour, no background, the foreground); C a `KuiCell` array with `ul`. `cursorAt` (`cursor_at`) names a cell to paint under its glyph in `cursorColor` as a `block` (default), `bar` or `underline` — its own name, since `cursor` is the pointer shape. `originLine` (`origin_line`) is the absolute line number of row 0: a grid is one screenful of the app's own history, so a row number means a different line after every scroll, and stamping where the screen sits is what lets a selection keep its ends across one (`docs/adr/0017-selection-as-a-scope.md`). Saying nothing is 0, and a selection then holds only while the screen does not move. The node's own rows apply — an `onKey` makes it the terminal's sink, an `onClick` or `onDrag` carries `cell: {row, col}` on its events — and its access row is `terminal`, the rows joined as its value.",
    },
    ElementDef {
        name: "line",
        // `width` and `color` are schema rows already (a sizing and the text
        // colour); on a line they are the stroke's width and colour.
        jsx_own: &["from", "to", "points", "curve", "dash", "dashOffset"],
        lua_own: &["from", "to", "points", "curve", "dash", "dash_offset"],
        jsx_rows: None,
        lua_rows: None,
        jsx: "`<line from={[x,y]} to={[x,y]} width color/>`, `<line points={[[x,y],…]} curve dash={[6, 4]} dashOffset/>`",
        lua: "`line { from={x,y}, to={x,y}, width=, color= }`, `line { points={{x,y},…}, curve=true, dash={6, 4}, dash_offset= }`",
        c: "`kui_line`, `kui_polyline`",
        odin: "`kui.line`, `kui.polyline` (points, `[][2]f32`)",
        doc: "A round-capped stroke: one segment, a polyline through `points`, or a smooth curve through them with `curve`. Always a float in its parent's box space (`float=\"viewport\"` for viewport space), sized to its own bounding box, so it takes no room in a row or column — but a float for the room alone: in its parent's box space it paints in the parent's layer at its place in the tree, over the siblings declared before it and under those after, as a child does, and opens no layer of its own (backlog F123; a connector meant to sit under two cards is declared before them). A stroke in its parent's box space is held by the parent's clip as a child is, its hit region with it, so it is cut at a scroller's edge with the row it is drawn in; a declared float is held that way only when it declares `clip` with a parent anchor, and a `float=\"viewport\"` stroke escapes, and is a layer of its own (backlog F78, `docs/adr/0010-a-segment-primitive.md` decision 5). `width` is the stroke width (default 1) and `color` the stroke colour; `transition` eases the colour, and with `slide` beside it the stroke's position too — the points ride its box, so a stroke whose ends all move together slides with them, while one whose ends move apart resizes at once (a canvas of floats eases everything or nothing, connectors included). Hit by its shape (`docs/adr/0026-hit-testing-by-shape.md`): with `onClick`, `onDrag`, `onHover` or `hoverable` a press within half the width of any piece hits it — at least 4 px of grab, so a hairline is a target — and a press elsewhere in its bounding box falls through to what is under; with none it takes no input and has no access row, and with input it derives one as a box would (a clickable connector is a button), so name it. What it costs: one quad per segment, and a curve is flattened in the core at one piece per 6 logical px of chord (at most 32 per span) — fixed rather than tolerance-driven so every binding gets the same pieces and the corpus can pin them — so a nine-point curve over ~50 px spans is ~60 quads, and a `quadCount` budget should expect it. `dash` cuts the stroke into marks and gaps (backlog V2): one length (marks and gaps alike), a mark and a gap, or four lengths for a dash-dot, in px **as seen** — every mark is a short stroke with the stroke's round caps, so `dash` 6, 4 is 6 px of ink and 4 px of nothing at any width up to 6 (SVG's `stroke-dasharray` measures the centre line instead, so with round caps its `4 4` at a width of 4 is solid; this pattern is SVG's `mark − width, gap + width`). A mark no longer than the stroke is wide is a dot as wide as the stroke, in the same period, so its gap is that much shorter; where a mark and its gap together come to no more than the width the dots meet and the gap closes — the marks either side of it are one, and a pattern with no gap left, `dash` 2, 2 at a width of 8, draws solid (backlog RG118). The pattern runs along the stroke's whole length, so it keeps its phase round the corners of a polyline and the pieces of a curve, and `dashOffset` starts that far into it — growing it moves the marks towards the first point, a marquee's marching ants; neither tweens. A pattern with no gap, a mark and gap under a physical pixel together, or more than 16384 marks draws solid. A dashed stroke is hit along its whole length, gaps included, and costs a quad per mark per piece the mark lies on.",
    },
    ElementDef {
        name: "titlebar",
        jsx_own: &[],
        // The window's own title is `window_title` on the root table; this
        // is the string the titlebar draws.
        lua_own: &["title"],
        jsx_rows: None,
        lua_rows: None,
        jsx: "`<titlebar title>` or `<titlebar>…</titlebar>`",
        lua: "`titlebar { title= }` / `titlebar { … }`",
        c: "`kui_titlebar`, `kui_titlebar_with`",
        odin: "`kui.titlebar`, `kui.titlebar_with`",
        doc: "Adaptive titlebar for custom chrome: drag strip, native-control inset, window buttons.",
    },
    ElementDef {
        name: "menuBar",
        jsx_own: &["menu"],
        lua_own: &["menu"],
        jsx_rows: None,
        lua_rows: None,
        jsx: "`<menuBar menu={[{ label, items: [{ label, id?, role?, accel?, enabled?, checked?, items? }] }]}/>`",
        lua: "`menu_bar { menu = { { label=, items= { { label=, id=, role=, accel=, enabled=, checked=, items= } } } } }`",
        c: "`kui_menu_bar`",
        odin: "`kui.menu_bar`",
        doc: "The application menu (`docs/adr/0018-a-menu-bar-the-app-declares.md`): `menu` is what it *is*, and where this element sits is where its titles go when they have to be drawn in the window. One call and not two, because declaring the menu and placing the strip are one decision. It draws **nothing** where the platform owns the bar — macOS, where the driver hands the same declaration to the OS — so the frame has still said what the app's menu is and the strip simply is not there; that is the contract `windowButtons` has under native decorations, and it is what makes one view portable. Its rows are the rows a context menu has: the same `role`s the core performs itself (`copy`, `paste`, `selectAll`, `cut`, `lookUp`), the same `id` payload, the same `accel` text, plus `checked` for a setting — and choosing one posts the same `{kind:\"menu\", role, item}` event, so an app handles one thing whichever menu it came from. Declared every frame and diffed: an unchanged menu costs a comparison, and an empty list takes it away. An accelerator kui can parse is rewritten into the platform's spelling, so `\"mod+s\"` reads as `⌘S` on macOS and `Ctrl+S` elsewhere and binds that key in the platform's own bar. On macOS the first menu is the application menu, which the OS titles with the app's own name whatever the label says. While a menu is open the bar is the frame's modal scope, so hovering across the titles moves the open menu, a press on the open title closes it, and Escape or a press in the app below closes it and reaches nothing else.",
    },
    ElementDef {
        name: "windowButtons",
        jsx_own: &[],
        lua_own: &[],
        jsx_rows: None,
        lua_rows: None,
        jsx: "`<windowButtons/>`",
        lua: "`window_buttons()`",
        c: "`kui_window_buttons`",
        odin: "`kui.window_buttons`",
        doc: "Just the min/max/close buttons, for fully custom titlebars.",
    },
    ElementDef {
        name: "tooltip",
        jsx_own: &["value"],
        lua_own: &["value"],
        jsx_rows: None,
        lua_rows: None,
        jsx: "`<tooltip value=\"hint\"/>` / `<tooltip>…</tooltip>` nodes, or the `tooltip=\"hint\"` prop (see composites)",
        lua: "`tooltip(\"hint\")` / `tooltip { … }` nodes, or the prop",
        c: "`kui_tooltip`, `kui_tooltip_with`",
        odin: "`kui.tooltip`, `kui.tooltip_with`",
        doc: "A float hanging below the parent; the node form always draws, the prop form is hover-gated.",
    },
    ElementDef {
        name: "latencyGraph",
        jsx_own: &["at"],
        lua_own: &["at"],
        jsx_rows: None,
        lua_rows: None,
        jsx: "`<latencyGraph/>`, `<latencyHud at/>`",
        lua: "`latency_graph()`, `latency_hud { at= }`",
        c: "`kui_latency_graph`, `kui_latency_hud`",
        odin: "`kui.latency_graph`, `kui.latency_hud`",
        doc: "Per-phase frame timing (windowed drivers fill it; headless shows the chrome empty).",
    },
    ElementDef {
        name: "audio",
        jsx_own: &["src", "loop", "volume", "paused", "finish", "tag"],
        lua_own: &["src", "loop", "volume", "paused", "finish", "tag"],
        jsx_rows: None,
        lua_rows: None,
        jsx: "`<audio src={id} loop volume paused finish tag/>`",
        lua: "`audio { src=, loop=, volume=, paused=, finish=, tag= }`",
        c: "`kui_audio`",
        odin: "`kui.audio`",
        doc: "A playback retained by node key: present = playing (once, or looped), gone = stopped; `volume` / `paused` apply live, a changed `src` restarts; a `tag` brings back `{kind:\"sound\", phase:\"ended\", tag}`. Draws nothing. `finish` changes what *gone* means: the node's removal releases the playback rather than stopping it, so a one-shot plays to its end and the view need not know the asset's length to declare the node for it (a loop still stops on removal — there is no end to reach — and a paused playback released has nothing to finish). Without it, the way to play a sound whole is to hold the node declared until the `tag`'s `ended` message arrives. A released playback is not free: it holds one of the device's 128 voices until its file ends, and the 129th play is refused — reported as a `playback-refused` warning and, for a `tag`, `phase: \"refused\"` rather than a wait that never returns. In the app's units, voices held = sound length × release rate: a 1.4 s chime released four times a second holds 6 of the 128 at any moment, a 10 s ambience released once a second holds 10, and every playback still declared (a loop included) counts beside them — a `refused` `sound` event is what arriving at 128 sounds like.",
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
        doc: "A press and release on the node (suppressed when a drag moved past the slop). A map payload gains fields where the node can say more: `cell: {row, col}` on a `cells` grid, and inside an `onKey` sink that draws `role=\"line\"` rows, `line`, `byte` and `clicks` as a `drag` inside one carries them.",
    },
    EventDef {
        kind: "drag",
        payload: "`{ kind: \"drag\", phase: \"start\" | \"move\" | \"end\", x, y, dx, dy, parent: { x, y, w, h }, tag }`",
        doc: "A pointer-captured drag on an `onDrag` node. `x`/`y` are where the pointer is; `dx`/`dy` are its displacement **from the press point**, in every phase — `start` carries zero, a `move` how far the pointer is from where it pressed, `end` the whole distance — so a handler sets `value = start + dx` rather than summing deltas, and can commit from `end` alone. Nothing is dropped under the click slop (3 px, measured from the press): the first `move` already carries the whole distance. `parent` is the container rect, so fractions need no geometry query. On a `cells` grid every phase also carries `cell: {row, col}`. Inside an `onKey` sink that draws `role=\"line\"` rows — an editor the app owns — every phase carries `line` (the ordinal among the sink's lines, the numbering its `access` events use), `byte` (where the point falls in that line's text, what `textHit` would answer) and `clicks` (the press's count), so click-to-caret, drag-select and double-click-word are arithmetic on the event with no query and no frame of lag; a point above the first line is the first, below the last the last, and one in a `role=\"none\"` gutter is the line beside it. Nothing is added where the sink draws no lines.",
    },
    EventDef {
        kind: "key",
        payload: "`{ kind: \"key\", phase: \"down\" | \"up\", code, physical, shift, ctrl, alt, super, text, repeat, location, caps_lock, num_lock, tag }`",
        doc: "A key press or release on the focused `onKey` sink; `code` is a character or a name (`\"left\"`, `\"f5\"`) and is what a keymap binds against. `physical` is the US-QWERTY key at that *position*, spelled the same way — bind it instead when you want the finger rather than the label (WASD stays a square on every layout). `code` follows the layout while the layout speaks ASCII, so a chord lands on the key the user can see (Dvorak's `⌥v` on the key printed V); on a layout that does not (Cyrillic, Greek, Hebrew, Arabic) the position's US key stands in, as Shift prints it (`J`, `:`; unshifted under Alt, as a chord reads it), so a Latin keymap keeps matching instead of matching nothing. A shifted letter arrives as the upper-case letter — `Z` with `shift` set for ⇧⌘Z, `physical` staying `z` — so a keymap that binds letters folds a one-character `code` to lower case under a chord; a headless press is spelled the same way, since no door re-spells it (`\"z\"` with `shift` is a chord no keyboard produces). `repeat` marks a press the OS auto-repeated; `text` is what the press would insert — always the layout's own character — and is null on every release. A key only comes up where it went down: a release whose press the sink never got is dropped, and focus leaving while a key is held delivers the `up` first, so a held-key binding (WASD, press-and-hold) cannot be left stuck down. `location` says which of a key's twins it was (backlog F108): `\"left\"` or `\"right\"` for a modifier, `\"numpad\"` for the keypad's digits, operators, Enter and (Num Lock off) arrows — `code` still `\"1\"`, `\"enter\"` — else `\"standard\"`; `caps_lock` and `num_lock` what the lock keys held. The keys F13–F35, `printscreen`, `pause`, `menu`, `clear` and the media keys (`mediaplaypause`, `volumeup`, …) are keys like any other; the modifier and lock keys themselves (`shift`, `ctrl`, `alt`, `super`, `capslock`, `numlock`, `scrolllock`) reach only a sink that says `modifierKeys`.",
    },
    EventDef {
        kind: "text",
        payload: "`{ kind: \"text\", text, pasted?: true, concealed?: true, transient?: true, tag }`",
        doc: "Text an IME committed at the end of a composition — or the clipboard's text, when the app asked for a paste with `requestPaste` / `request_paste` / `kui_request_paste` — on the focused `onKey` sink (or the nearest one above the focused control, or the root sink with nothing focused) — the one committed text the platform never reports as a key press carrying `text`, so a custom editor inserts it as it would a key's `text`. Plain typing does not arrive this way: the `key` event already carries what the press would insert, and a sink hearing both would type every character twice. A focused `<edit>` takes the commit itself and reports `changed`. A paste's answer carries `pasted: true` (backlog DX14) — an answer being the host's paste reply, or any commit while a paste is outstanding — so a sink that asked tells the clipboard's text from an IME's commit without keeping its own flag; an IME commit has no `pasted`. It also says what the pasteboard marked it (backlog F84): `concealed: true` for a secret — a password manager's copy, which a view should not show, log or keep — and `transient: true` for text not to keep in a history, after the nspasteboard.org convention (on Windows the clipboard's exclusion formats); a marker that is not set is absent, never false. The runner reads them on macOS and Windows; on Linux, and from a host that answers with a bare commit, a paste arrives unmarked.",
    },
    EventDef {
        kind: "preedit",
        payload: "`{ kind: \"preedit\", text, cursor: [start, end] | null, tag }`",
        doc: "An in-progress IME composition on the focused `onKey` sink: `text` is the uncommitted string to show inline at the caret, `cursor` the byte range inside it the IME's own caret covers (null when it does not say), and an empty `text` means the composition ended without a commit, so what was shown goes away. The OS candidate window is anchored for you: the `line` carrying `caret` says where. A focused `<edit>` draws the composition itself.",
    },
    EventDef {
        kind: "selectionrange",
        payload: "`{ kind: \"selectionrange\", from: { index, byte }, to: { index, byte } }` on the scope",
        doc: "A copy reached rows of a `selectable` virtual list that no frame built (`docs/adr/0017-selection-as-a-scope.md`, tier 3): the core cannot read text it never laid out, so it asks the app for the range — `index` is a row's data index (its `index` prop), `byte` an offset into that row's text, past the row's length for \"the whole row\" (a Select All over a list that declared `rowCount` ends that way, on its last row) — and the app answers with `answerSelectionRange(text)` / `answer_selection_range` / `kui_answer_selection_range`, which is what reaches the clipboard as a `setClipboard` action. Raised only while a copy is outstanding (`requestCopy()` answered `asked`, or the runner's Cmd/Ctrl-C did); a late answer changes nothing. `examples/rust/features/clipboard.rs` and its Node twin show the round trip.",
    },
    EventDef {
        kind: "contextmenu",
        payload: "`{ kind: \"contextmenu\", x, y, tag }`",
        doc: "A secondary-button press on an `onContextMenu` node, on the press rather than the release; `x`/`y` are logical viewport coordinates — where the menu goes. The core opens nothing: the app declares the menu (a `modal` float) and stops declaring it on `dismiss`.",
    },
    EventDef {
        kind: "menu",
        payload: "`{ kind: \"menu\", role, item }`",
        doc: "A row of the core's own context menu was chosen (`openMenu` / `open_menu` / `kui_open_menu`), on the node the menu was about. `item` is the row's `id`, or its label when it declared none; `role` is the row's standard role or `custom`. Every chosen row posts, the standard ones included: a `cut` or `paste` role is carried out by the core (its clipboard work queued for the host) *and* reported, so an app can hear its editor being cut from and is free to ignore it (`docs/adr/0017-selection-as-a-scope.md`, decision 5).",
    },
    EventDef {
        kind: "forceclick",
        payload: "`{ kind: \"forceclick\", x, y, tag }`",
        doc: "A press that deepened past the second stage of a Force Touch trackpad, on an `onForceClick` node, at the logical viewport point it happened at. Routed as a secondary press is — no focus moved, no caret placed, no click — but asked of the topmost node only, and the ordinary click the press is still producing arrives afterwards. Text needs none of this: over an `edit` or a `selectable` scope the core selects the word under it and asks the host for its Look Up panel instead. macOS-only in practice.",
    },
    EventDef {
        kind: "button",
        payload: "`{ kind: \"button\", phase: \"press\" | \"move\" | \"release\", button: \"secondary\" | \"middle\" | number, x, y, clicks, cell?: { row, col }, line?, byte?, tag }`",
        doc: "A non-primary button on an `onButton` node that claims it (`buttons`), backlog F105: `press` where it went down — with the driver's click count, `clicks`, which the native runner keeps for the primary button alone and so always reports as 1 here — then `move` for every pointer move while it is held and `release` where it came up, both on the same node wherever the pointer went, since the press captured the button. `button` is the button's name, or for one past the middle button its number (`3 + n`, as `kui_input_mouse_button` takes it; Node's `ctx.mouse` takes the three names only); `x`/`y` are logical viewport coordinates. On a `cells` grid each carries `cell: {row, col}`, clamped to the grid, and inside an `onKey` sink that draws `role=\"line\"` rows `line` and `byte` as a drag does. A claimed secondary press is this event instead of `contextmenu`; the press moves no focus, caret, selection or scrollbar.",
    },
    EventDef {
        kind: "scroll",
        payload: "`{ kind: \"scroll\", x, y, dx, dy, lines, mods?: { shift, ctrl, alt, super }, tag }`",
        doc: "The wheel over an `onScroll` node, or a drag-select held past a `cells` grid's top or bottom edge: `dx`/`dy` the delta in logical px as the driver reported it (positive `dy` is the wheel rolling up, toward earlier content), `x`/`y` the pointer in logical viewport coordinates, `lines` the whole lines a `cells` grid's `dy` covers — positive is later history, the sign `originLine` grows in, the fraction carried to the next notch — and null on any other node. The core scrolls nothing for it: the app re-declares the grid's `originLine`, or zooms its canvas. From the edge drag it comes once a frame while the pointer is held past the edge, with the lines that frame's step covers, and the selection's absolute lines survive the scroll the app answers with (`docs/adr/0029-a-selection-follows-the-pointer-past-the-edge.md`).",
    },
    EventDef {
        kind: "focus",
        payload: "`{ kind: \"focus\", phase: \"in\" | \"out\", by: \"pointer\" | \"keyboard\" | \"assistive\" | \"program\", tag }`",
        doc: "Keyboard focus entered or left an `onFocus` node's subtree (backlog DX18). `by` is what moved it — a press, a key, a screen reader's request, or the view and the app — so a pane that follows a click into its sink tells that apart from a move the app made itself.",
    },
    EventDef {
        kind: "hover",
        payload: "`{ kind: \"hover\", phase: \"enter\" | \"leave\", by: \"pointer\" | \"content\", tag }`",
        doc: "The pointer entered or left an `onHover` node — also when a new frame moved it under a still cursor. `by` says which (backlog DX20): `pointer` when the pointer moved or left the window, `content` when it stayed and what is under it changed — a list scrolled by the wheel or the keys, a row that grew, a float that opened. A picker whose selection follows the pointer ignores `content`, or the rows sliding under a still pointer as the keys scroll the list drag the selection with them.",
    },
    EventDef {
        kind: "drop",
        payload: "`{ kind: \"drop\", phase: \"enter\" | \"move\" | \"leave\" | \"drop\", paths: string[], x, y, tag }`",
        doc: "Files dragged in from the OS over an `onDrop` node (`docs/adr/0031-a-drop-zone-is-a-row-and-the-files-are-an-event.md`): `enter` when they come over the zone, `move` while they move over it (never twice for one point), `leave` when they go to another zone, to no zone or out of the window, `drop` when they land — and no `leave` after a `drop`. `paths` are the OS paths as strings; `x`/`y` the pointer in logical viewport coordinates, absent on `leave`. The zone is the topmost one under the pointer by paint order; a node inside it is its, and a node that is no zone is looked past (an overlay shown on `enter` does not end the hover). Nothing is re-resolved when a frame lands: only the driver's next report moves the files, so a zone the view stops declaring hears its `leave` then.",
    },
    EventDef {
        kind: "files",
        payload: "`{ kind: \"files\", paths: string[], tag }`",
        doc: "A file dialog's answer (backlog C51): what an Open, Save or folder dialog asked for with `requestFiles` / `request_files` / `kui_request_files` picked — the OS paths, as a `drop` carries them — or no paths when the user cancelled. `tag` is the dialog's own. It reaches whoever asked: the host, or the extension whose fill asked; one dialog is out at a time.",
    },
    EventDef {
        kind: "open",
        payload: "`{ kind: \"open\", paths: string[] }` on the root",
        doc: "The OS asked the app to open documents (backlog F124): the Finder's Open With, a file dropped on the Dock icon, `open -a App file`, a double-click on a document of a type the app's `Info.plist` declares under `CFBundleDocumentTypes`. Delivered to the host on the root whether or not anything asked — unlike `files`, which answers an ask. `paths` are file-system paths as strings; a URL of a scheme the app registers is not one, and is not carried (room is left for a `urls` beside `paths`). The macOS runner sends it at launch — held until the main window has opened, since AppKit hands the documents over before it exists and puts none in the arguments — and while the app runs; a host driving its own window sends it as input (`openDocuments`, `kui_input_open`, `InputEvent::Open`). Windows and Linux pass documents in the process's arguments instead, and nothing sends it there.",
    },
    EventDef {
        kind: "layout",
        payload: "`{ kind: \"layout\", x, y, w, h, parent: { x, y, w, h }, scale, tag }`",
        doc: "The rect layout gave an `onLayout` node (logical px, viewport coords, after scrolling and easing): on its first frame and whenever it changes, never on a frame that left it alone. `scale` is physical px per logical px at the node — `w × scale` by `h × scale` is how many pixels to render for it before `updateImage` (the frame's scale today; where a zoom would compose in).",
    },
    EventDef {
        kind: "resize",
        payload: "`{ kind: \"resize\", width, height, scale }`",
        doc: "The viewport changed size or DPI (logical px, delivered to the host on the root): the window less the devtools' dock while the panel is docked (`docs/adr/0024`), so a dock coming, going or being dragged is a resize too. `KuiWindow.size()` queries the same numbers, before the first frame as well (backlog F43). The first frame establishes the viewport rather than reporting it, so a dock present at launch posts none.",
    },
    EventDef {
        kind: "window",
        payload: "`{ kind: \"window\", phase: \"opened\" | \"closed\" | \"focused\" | \"blurred\", name, id }`",
        doc: "A declared window opened (the diff queued its `Open`) or closed — because nothing declares it any more, or because the user closed it, in which case it stays closed while still declared: stop declaring `name`, then declare it again to reopen. `id` is what its events carry; the event itself is on the root of whichever window's frame noticed. `focused` and `blurred` are this window gaining and losing the keyboard (backlog DX18) — `env.focused` changing, as the driver reports it — so an app that saves on blur or re-reads the clipboard on return hears it once instead of diffing `env.focused` every frame.",
    },
    EventDef {
        kind: "system",
        payload: "`{ kind: \"system\", appearance, accent, motion, locale, assistive }`",
        doc: "An OS setting the user changed while the app was open — the light/dark appearance, the accent colour, reduced motion, or the UI language — or assistive technology starting to listen (delivered to the host on the root, one per window that noticed). The payload is `env.system` as it now reads, in the same spellings and with the same nulls, so a handler can keep the whole reading or take the one field it branches on. The first frame establishes the reading rather than reporting it, the way the viewport does; a host whose view is a function the runner calls every frame can equally re-read `env` and ignore this, but a host that retains the tree it was handed (Node, C, Lua) only re-runs its view for a message, so this is how a palette follows the OS.",
    },
    EventDef {
        kind: "fonts",
        payload: "`{ kind: \"fonts\" }`",
        doc: "The system's installed fonts changed: a rescan found a face installed or removed since the last one (delivered to the host on the root, one per window that noticed). The winit runner rescans itself when macOS or Windows says the set changed; elsewhere it is the app calling `reloadSystemFonts`. Re-read `systemFonts()` / `systemFontFamilies()` here if the model holds them; a view naming a family by `family` needs nothing, it resolves on the same frame. A font the app loads itself raises none, and the first frame establishes the set rather than reporting it.",
    },
    EventDef {
        kind: "modifiers",
        payload: "`{ kind: \"modifiers\", shift, ctrl, alt, super }`",
        doc: "The physical modifier state changed (delivered to the host on the root).",
    },
    EventDef {
        kind: "changed",
        payload: "`{ kind: \"changed\" }`, with the editor's key on the event",
        doc: "An editor's text changed.",
    },
    EventDef {
        kind: "submit",
        payload: "`{ kind: \"submit\" }`, with the editor's key on the event",
        doc: "Enter in a single-line editor.",
    },
    EventDef {
        kind: "boundary",
        payload: "`{ kind: \"boundary\", key: \"up\" | \"down\" | \"left\" | \"right\" | \"backspace\" | \"delete\", edge: \"start\" | \"end\", word, doc }`, with the editor's key on the event",
        doc: "An editing key that met the edge of an editor's text and did nothing: ↑ on the first line, ← or Backspace at the start (`edge: \"start\"`), ↓ on the last line, → or Delete at the end (`\"end\"`) — from a caret with no selection and without Shift. What a block editor joins blocks and moves between fields by. A field has one line, so its ↑ and ↓ always report. `word` and `doc` are the editing modifiers the key carried (Option or Ctrl, ⌘ or Ctrl — `docs/adr/0002`).",
    },
    EventDef {
        kind: "sound",
        payload: "`{ kind: \"sound\", phase: \"ended\" | \"refused\", playback, tag }`",
        doc: "A tagged playback (`play(id, { tag })` or `<audio tag>`) finished on its own — never when something stopped it. `phase: \"refused\"` instead when the device would not take the play at all (its 128 voices are all held, or the sound did not decode): that playback never starts and so never ends, so this is what arrives in place of the `ended` a view would otherwise wait forever for.",
    },
    EventDef {
        kind: "dismiss",
        payload: "`{ kind: \"dismiss\", reason: \"escape\" | \"outside\", tag }` on a node; `{ kind: \"dismiss\", reason, name, id }` on the root for a popup window",
        doc: "The user asked for a surface to go away — Escape, or a press that landed outside it. The core closes nothing: the app stops declaring the surface (or asks first). A `modal` node gets one on the node, carrying its tag, and only the modal in effect does; a `kind: \"popup\"` window gets one on the root, carrying the window's `name` and `id`, reported by the driver because a press outside a window and a key sent to a non-activating one are both facts only the OS has. The two are the same event, so a dropdown that graduates from a modal float to a popup window changes its declaration and not its handler.",
    },
    EventDef {
        kind: "access",
        payload: "`{ kind: \"access\", action, tag, text?, value?, anchor?: { line, offset }, focus?: { line, offset } }`",
        doc: "Assistive technology — or the keyboard — asked for what only the app can do: `increment` / `decrement` on a `slider` role that declared no `onChange` (a reader's nudge, or the arrow keys on the focused slider), and `setValue` there with the number asked for as `value` (Windows' UI Automation sets a slider rather than nudging it); `setValue` / `replaceSelectedText` (with `text`) / `setTextSelection` (with `anchor` and `focus` as line ordinals and byte offsets) on a custom editor. `tag` is the node's `onClick` payload (or its `onDrag` / `onKey` tag). Every other request resolves in the core and arrives as the events a pointer would have produced.",
    },
    EventDef {
        kind: "change",
        payload: "`{ kind: \"change\", value, phase: \"move\" | \"end\", tag }`",
        doc: "A `slider` that declared `onChange` (the stock `<slider>`, ADR 0034): the core turned a press into the value under the pointer, a drag into each new step, an arrow or assistive technology's increment / decrement into one `valueStep` and its set value into that value snapped, PageUp / PageDown into ten, Home / End into the ends — clamped to `valueMin`..`valueMax` and snapped to the step, the decimal the step names. `phase` is `move` while the pointer holds the slider and `end` when it lets go or a key moved it; a key that lands where the slider is proposes nothing. The value is proposed: declare it as `valueNow`. `tag` is the `onChange` payload.",
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
        what: "fragment (WGSL)",
        node: "`ctx.addFragment(src)` → id for `<fragment src>`",
        lua: "the host registers; `fragment { id }`",
        c: "`kui_fragment_add` → `kui_fragment`",
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
        c: "`kui_font_add_system` (see `kui_font_families`)",
    },
    ResourceDef {
        what: "sound (wav / ogg / mp3 / flac bytes)",
        node: "`ctx.addSound(buffer)` → id for `<audio src>`, `clickSound`, `play(id)`",
        lua: "the host registers; `audio { src = id }`, `click_sound = id`",
        c: "`kui_sound_add` → `kui_audio`, `KuiSpec.click_sound`, `kui_play`",
    },
];

/// One value in the host-environment reading a view gets — `ui.env()` in
/// Rust, `view(env)` in Lua, `ctx.env()` / `win.env()` in Node — and the
/// key each binding puts it under. C has no reading: a C host is the frame
/// driver, so it is the *writer* (`kui_env_set`, `kui_env_set_window`), and
/// its column names the argument that carries the fact in.
///
/// `Env` and `WindowEnv` are the shape; this table is the cross-binding
/// restatement of it, and every restatement is pinned to it rather than
/// trusted: `schema`'s tests check the rows against the two structs, the
/// Lua and Node key-set tests check each binding's reading against the
/// `lua` / `node` columns, and kui-ffi checks the header's two prototypes
/// against the `c` column. The frame facts a view wants at the same moment
/// (the viewport, the focused node) ride in the same reading and have rows
/// here too, marked as the frame's rather than `Env`'s.
///
/// Two divergences are deliberate and named in their rows: `native_controls`
/// is the `Rect` the core holds in Node and a width/height at the window
/// origin in Lua and C, and `frame_budget_ms` is derived from `refresh_hz`
/// but part of the reading everywhere.
pub struct EnvField {
    /// The canonical name: the Rust field path (`window.custom_chrome`).
    pub name: &'static str,
    /// Where it comes from — the Rust struct and field for a stored fact,
    /// the call for a derived or frame fact.
    pub from: &'static str,
    /// The key path(s) it occupies in Node's `ctx.env()`; empty when Node
    /// carries the fact as a call on the context instead (the `doc` says
    /// which) or not at all.
    pub node: &'static [&'static str],
    /// The same for Lua's `view(env)` table. Two entries where Lua flattens
    /// one fact into two keys.
    pub lua: &'static [&'static str],
    /// The C spelling: `function(argument)` for the setter argument that
    /// writes it — the first thing in the cell, so kui-ffi can parse it —
    /// or the call that reads it.
    pub c: &'static str,
    pub doc: &'static str,
    /// The fact's reading, as plain data: `Null` for "the host cannot
    /// tell", an enum as its wire name, a key as its integer, a rect as
    /// `{x, y, w, h}`. What makes the table a reader and not only a pin —
    /// a binding iterates the rows and asks each for its value, then
    /// spells the key its own way, so a fact added here reaches every
    /// reading with no reader restated beside it (the way
    /// [`ThemeRole::get`] does for the palette).
    pub get: fn(&EnvFacts) -> Value,
}

/// Everything an env reading is taken from: the stored [`Env`](crate::env::Env) and the
/// frame's own facts beside it (`Core::env_facts`).
#[derive(Clone, Copy, Debug)]
pub struct EnvFacts {
    pub env: crate::env::Env,
    pub viewport: crate::geom::Size,
    pub scale: f32,
    pub focus: Option<crate::key::Key>,
    pub focus_visible: bool,
    pub region: Option<crate::key::Key>,
    pub caret_visible: bool,
    /// The frame clock in seconds (`Core::now`, backlog F134).
    pub now: f64,
}

fn key_value(k: Option<crate::key::Key>) -> Value {
    k.map_or(Value::Null, |k| Value::Int(k.0 as i64))
}

fn rect_value(r: crate::geom::Rect) -> Value {
    Value::map([
        ("x", Value::Float(r.x as f64)),
        ("y", Value::Float(r.y as f64)),
        ("w", Value::Float(r.w as f64)),
        ("h", Value::Float(r.h as f64)),
    ])
}

pub const ENV_FIELDS: &[EnvField] = &[
    EnvField {
        name: "refresh_hz",
        get: |f| {
            f.env
                .refresh_hz
                .map_or(Value::Null, |hz| Value::Float(hz as f64))
        },
        from: "`Env::refresh_hz`",
        node: &["refreshHz"],
        lua: &["refresh_hz"],
        c: "`kui_env_set(refresh_hz)`",
        doc: "Display refresh rate in Hz. \"The host cannot tell\" is `null` in Node (a stable shape to destructure, typed `number | null`), an absent key in Lua, and a rate at or below zero in C.",
    },
    EnvField {
        name: "frame_budget_ms",
        get: |f| Value::Float(f.env.frame_budget_ms() as f64),
        from: "`Env::frame_budget_ms()`, derived",
        node: &["frameBudgetMs"],
        lua: &["frame_budget_ms"],
        c: "—",
        doc: "One vsync interval at `refresh_hz`, or at 120 Hz when the host cannot tell: the per-frame time budget, and what the latency HUD draws its line at. Derived, and in the reading anyway, so no view restates the fallback. A C host is the one that knows the rate and computes its own.",
    },
    EnvField {
        name: "focused",
        get: |f| Value::Bool(f.env.focused),
        from: "`Env::focused`",
        node: &["focused"],
        lua: &["focused"],
        c: "`kui_env_set(focused)`",
        doc: "Whether the *window* has the keyboard at all. Not the focused node — that is the `focus` row.",
    },
    EnvField {
        name: "system.appearance",
        get: |f| Value::str(f.env.system.appearance.name()),
        from: "`SystemEnv::appearance`",
        node: &["system.appearance"],
        lua: &["system.appearance"],
        c: "`kui_env_set_system(appearance)`",
        doc: "The OS light/dark setting: `\"light\"`, `\"dark\"`, or `\"unknown\"` when the host has no way to ask (`KUI_APPEARANCE_*` in C, where unknown is 0). Unknown is a real answer and the default — a view picks its own palette for it rather than being handed a guess. The core acts on it in one way: the theme is derived from it (ADR 0019), so the stock widgets and a `<text>` with no colour follow the setting — and nothing of the app's own repaints, because only the view knows which of its colours is the background.",
    },
    EnvField {
        name: "system.accent",
        get: |f| {
            f.env
                .system
                .accent
                .map_or(Value::Null, |c| Value::Int(c.to_hex() as i64))
        },
        from: "`SystemEnv::accent`",
        node: &["system.accent"],
        lua: &["system.accent"],
        c: "`kui_env_set_system(accent)`",
        doc: "The OS accent/highlight colour as `0xRRGGBBAA`, ready to pass straight back as a `bg` or `color`. \"The host cannot tell\" is `null` in Node, an absent key in Lua, and 0 in C — a fully transparent accent is not a colour anyone was given, the way a refresh rate of zero is not a rate. Node's `setEnv` also takes the `\"#rrggbb\"` spelling a prop takes.",
    },
    EnvField {
        name: "system.motion",
        get: |f| Value::str(f.env.system.motion.name()),
        from: "`SystemEnv::motion`",
        node: &["system.motion"],
        lua: &["system.motion"],
        c: "`kui_env_set_system(motion)`",
        doc: "The OS reduce-motion setting: `\"reduced\"` when the user asked for less animation, `\"full\"` when they did not, `\"unknown\"` when nobody asked the OS (`KUI_MOTION_*` in C, unknown 0). Spelled as what the user wants rather than as a `reduceMotion` boolean, because the third reading has no place in a boolean. Nothing in the core shortens an animation for it — a view that honours it does so where it declares one.",
    },
    EnvField {
        name: "system.locale",
        get: |f| {
            f.env
                .system
                .locale
                .map_or(Value::Null, |l| Value::str(l.as_str()))
        },
        from: "`SystemEnv::locale`",
        node: &["system.locale"],
        lua: &["system.locale"],
        c: "`kui_env_set_system(locale)`",
        doc: "The UI language as a BCP-47 tag (`\"en\"`, `\"en-US\"`, `\"zh-Hant-HK\"`), for whatever the view formats dates and numbers with; kui does not parse it. Carried inline (31 ASCII bytes, `Locale`) so the reading stays `Copy`, and anything that does not fit reads back as unknown: `null` in Node, an absent key in Lua, an empty `KuiStr` in C.",
    },
    EnvField {
        name: "system.assistive",
        get: |f| Value::str(f.env.system.assistive.name()),
        from: "`SystemEnv::assistive`",
        node: &["system.assistive"],
        lua: &["system.assistive"],
        c: "`kui_env_set_assistive(assistive)`",
        doc: "Whether assistive technology is listening: `\"listening\"` once an accessibility client has asked this window for its tree, `\"none\"` while the bridge is up and nobody has, `\"unknown\"` where there is no bridge — a headless `Ctx`, a runner built without `accesskit`, a C host that never called the setter (`KUI_ASSISTIVE_*`, unknown 0). The reading that changes what a view *says* rather than what it draws: an alert that announces when something is listening and blinks when nothing is. Reported through the `system` event when it changes, like the other four. Two limits are the platform's, not kui's. *Any* client counts — a probe, an accessibility inspector, a test driving the AX API and VoiceOver alike all ask for the tree, and nothing tells them apart — so it says something is listening, not that a person is. And it falls back to `\"none\"` only where the adapter reports deactivation, which in the pinned AccessKit is AT-SPI alone (the session's accessibility bus going away); on macOS and Windows nothing reports a client leaving, so once it has risen it stays `\"listening\"` for the window's life.",
    },
    EnvField {
        name: "window.id",
        get: |f| Value::Int(f.env.window.id.0 as i64),
        from: "`WindowEnv::id`",
        node: &["window.id"],
        lua: &["window.id"],
        c: "`kui_env_set_window(window)`, read back by `kui_ctx_window`",
        doc: "Which window this frame draws, assigned by the driver: 0 for the window the app starts in. Every event from it carries the same number.",
    },
    EnvField {
        name: "window.custom_chrome",
        get: |f| Value::Bool(f.env.window.custom_chrome),
        from: "`WindowEnv::custom_chrome`",
        node: &["window.customChrome"],
        lua: &["window.custom_chrome"],
        c: "`kui_env_set_window(custom_chrome)`",
        doc: "The host asked the app to draw its own chrome, so there is no native titlebar to sit under. `<titlebar>` and `<windowButtons>` build nothing when this is false.",
    },
    EnvField {
        name: "window.maximized",
        get: |f| Value::Bool(f.env.window.maximized),
        from: "`WindowEnv::maximized`",
        node: &["window.maximized"],
        lua: &["window.maximized"],
        c: "`kui_env_set_window(maximized)`",
        doc: "The window is maximized — what picks the restore glyph over the maximize one.",
    },
    EnvField {
        name: "window.fullscreen",
        get: |f| Value::Bool(f.env.window.fullscreen),
        from: "`WindowEnv::fullscreen`",
        node: &["window.fullscreen"],
        lua: &["window.fullscreen"],
        c: "`kui_env_set_window(fullscreen)`",
        doc: "The window is fullscreen.",
    },
    EnvField {
        name: "window.always_on_top",
        get: |f| Value::Bool(f.env.window.always_on_top),
        from: "`WindowEnv::always_on_top`",
        node: &["window.alwaysOnTop"],
        lua: &["window.always_on_top"],
        c: "`kui_env_set_always_on_top(always_on_top)`",
        doc: "The window is above every other app's: the level the driver set after the frame asked for it (`alwaysOnTop` / `always_on_top` / `kui_set_always_on_top`, backlog C30), on a platform that has one. On Wayland winit has no call for it, so a driver there reports false however often the app asks — which is why a pin button draws its state from this and not from the app's own flag. It is the driver's record of what it set and not a query (winit has no level getter), so a level the OS dropped afterwards — a fullscreen space, a tiling manager — is not seen here. A C host reports it through its own setter rather than an argument on `kui_env_set_window`, the way `kui_env_set_assistive` is, so an older host that never applies a level has nothing to recompile.",
    },
    EnvField {
        name: "window.native_controls",
        get: |f| f.env.window.native_controls.map_or(Value::Null, rect_value),
        from: "`WindowEnv::native_controls`",
        node: &["window.nativeControls"],
        lua: &["window.controls_w", "window.controls_h"],
        c: "`kui_env_set_window(controls_w, controls_h)`",
        doc: "Area (logical px, window coordinates) covered by controls the OS still draws over our content — the macOS traffic lights under custom chrome. Keep out of it. Node hands back the `Rect` the core holds (`{x, y, w, h}`, or `null` for none); Lua and C flatten it to a width and height anchored at the window origin (absent in Lua, `0` in C, for none), which is the shape C's two numbers can express and where the one real instance sits.",
    },
    EnvField {
        name: "window.backdrop",
        get: |f| Value::str(f.env.window.backdrop.name()),
        from: "`WindowEnv::backdrop`",
        node: &["window.backdrop"],
        lua: &["window.backdrop"],
        c: "`kui_env_set_backdrop(backdrop)`; read back with `kui_ctx_backdrop()`",
        doc: "What is behind the window's transparent pixels, as the driver got it (backlog F126): `\"opaque\"` (the default, and every headless core's), `\"transparent\"` (the desktop as it is), `\"blur\"` (a live blur of what is behind the window) or `\"tinted\"` (the desktop's colour, not live) — `KUI_BACKDROP_*` in C, opaque 0. The app asks with `Launcher::backdrop` and decides which regions show it by painting them with alpha; this is the answer, which is less where the platform has less — a blur asked of GNOME reads `\"tinted\"`, the wallpaper kui draws itself, or `\"opaque\"` where none could be read — so a view paints its translucent regions opaque when this says so. A C host reports it through its own setter, as `always_on_top` is; a C app asks with `KuiRunConfig.backdrop` and its view reads the answer with `kui_ctx_backdrop`.",
    },
    EnvField {
        name: "audio.device",
        get: |f| Value::str(f.env.audio.device.name()),
        from: "`AudioEnv::device`",
        node: &["audio.device"],
        lua: &["audio.device"],
        c: "`kui_env_set_audio(device)`",
        doc: "What the driver's output device is doing: `\"closed\"` (the default, and a headless driver's answer), `\"opening\"` (the ~90 ms open, on its own thread), `\"open\"`, or `\"failed\"` (it refused, and commands are dropped) — `KUI_AUDIO_DEVICE_*` in C, closed 0. A fact and not a verb: nothing lets a view close it, the driver does that itself once it has been idle a while. Worth reading because an open stream is a real-time thread whether or not anything plays, which is the whole of an idle app's CPU once a session has held a sound.",
    },
    EnvField {
        name: "audio.live",
        get: |f| Value::Int(f.env.audio.live as i64),
        from: "`AudioEnv::live`",
        node: &["audio.live"],
        lua: &["audio.live"],
        c: "`kui_env_set_audio(live)`",
        doc: "Playbacks started and not yet ended, plus any waiting on the device to open. Zero with the device still `\"open\"` is the idle stream the row above is about. A play that arrives while the device is `\"opening\"` counts here from the frame it was asked, until the open answers: if the device refuses, the play is refused on the next apply — `{kind:\"sound\", phase:\"refused\"}` for a tagged one — and leaves the count with it, so what a machine with no output device shows is `opening`/1 then `failed`/0 with the refusal between (backlog F63).",
    },
    EnvField {
        name: "viewport.w",
        get: |f| Value::Float(f.viewport.w as f64),
        from: "`Core::viewport()`, the frame's",
        node: &["viewport.width"],
        lua: &["viewport_w"],
        c: "`kui_frame_begin(w)`",
        doc: "The logical width of the current (or last) frame's viewport — the window less the devtools' dock while the panel is docked (`docs/adr/0024`), the same number a `resize` reports and Node's `KuiWindow.size()` answers — the other host fact a view wants at the same moment, so it rides in the same reading. Zero before the first frame, since the frame establishes it (backlog F43: this row once read the window instead, so an app under `KUI_DEVTOOLS` sized itself to a viewport it did not have). Node's `viewport` is the `WindowSize` shape `runWindowed` already uses.",
    },
    EnvField {
        name: "viewport.h",
        get: |f| Value::Float(f.viewport.h as f64),
        from: "`Core::viewport()`, the frame's",
        node: &["viewport.height"],
        lua: &["viewport_h"],
        c: "`kui_frame_begin(h)`",
        doc: "Its logical height.",
    },
    EnvField {
        name: "scale",
        get: |f| Value::Float(f.scale as f64),
        from: "`Core::scale()`, the frame's",
        node: &["viewport.scale"],
        lua: &[],
        c: "`kui_frame_begin(scale)`",
        doc: "Device pixels per logical px. Lua has no reading: a script sees logical px only.",
    },
    EnvField {
        name: "focus",
        get: |f| key_value(f.focus),
        from: "`Core::focus()`, the frame's",
        node: &[],
        lua: &["focus"],
        c: "`kui_focused()`",
        doc: "The focused *node*'s key, as events carry it (absent for none). A value the host wrote before the view ran, so it lags a same-frame verb by one frame; `env.is_focused(key)` is the live query. Node spells it as the call `focused()` on the context, and C as `kui_focused`, rather than a key on `env`.",
    },
    EnvField {
        name: "focus_visible",
        get: |f| Value::Bool(f.focus_visible),
        from: "`Core::focus_visible()`, the frame's",
        node: &[],
        lua: &["focus_visible"],
        c: "`kui_focus_visible()`",
        doc: "Whether focus shows — the keyboard or assistive technology put it where it is, or acted on it there; a click alone does not. Node: `focusVisible()` on the context.",
    },
    EnvField {
        name: "caret_visible",
        get: |f| Value::Bool(f.caret_visible),
        from: "`Core::caret_visible()`, the frame's",
        node: &[],
        lua: &["caret_visible"],
        c: "`kui_caret_visible()`",
        doc: "The caret's blink phase — `true` draws it (backlog C35). The driver's clock sets it while there is a caret to blink: a focused `edit`'s, or the `caret` a `line` under a focused `onKey` sink declares; a custom editor draws its caret node on the on phase and skips it on the off, keeping the `caret` row on its `line` either way, so it blinks in step with the stock editor and, in a window without the keyboard, not at all. Always `true` headless. Node: `caretVisible()` on the context (`setCaretVisible` is the driver's half, for a test that drives the phase).",
    },
    EnvField {
        name: "now",
        get: |f| Value::Float(f.now),
        from: "`Core::now()`, the frame's",
        node: &[],
        lua: &["now"],
        c: "`kui_now()`",
        doc: "The frame clock in seconds (backlog F134): the driver's monotonic clock, any origin, the one `transition` and `keyframes` read this frame — 0 before a driver sets one. Read a deadline off it (a toast's expiry, a sequence's beats) rather than off a clock of the app's own, so a test that moves the frame clock moves both. Node: `now()` on the context.",
    },
    EnvField {
        name: "region",
        get: |f| key_value(f.region),
        from: "`Core::region()`, the frame's",
        node: &[],
        lua: &["region"],
        c: "`kui_region()`",
        doc: "The focus region in effect — the key of the `focusRegion` node whose ring Tab walks (absent for the main ring; `docs/adr/0022-focus-regions.md`). What a chord that toggles between a dock and the app reads to know which way it is going. Node spells it as the call `region()` on the context, and C as `kui_region`.",
    },
];

/// One colour role in a [`crate::theme::Theme`], with the spelling each
/// binding reads it under and the reading itself.
///
/// The same pin `ENV_FIELDS` is: the palette is one contract, so the roles
/// are written down once and every binding's reading is generated from
/// this table rather than restated beside it. `get` is what makes that
/// possible — a binding iterates the rows and asks each one for its
/// colour, so adding a token is one row here and nothing anywhere else.
/// The test below destructures [`crate::theme::Theme`] exhaustively, so a
/// field added to it stops the crate compiling until it has a row.
pub struct ThemeRole {
    /// The Rust field, and the name used everywhere but Node.
    pub name: &'static str,
    /// What Node calls it (camelCase).
    pub node: &'static str,
    pub doc: &'static str,
    /// This role's colour out of a theme.
    pub get: fn(&crate::theme::Theme) -> crate::color::Color,
    /// The same colour written: what a binding's theme setter uses, so
    /// writing a palette is the table's business the way reading one is.
    pub set: fn(&mut crate::theme::Theme, crate::color::Color),
}

pub const THEME_ROLES: &[ThemeRole] = &[
    ThemeRole {
        name: "bg",
        node: "bg",
        get: |t| t.bg,
        set: |t, c| t.bg = c,
        doc: "The window behind everything.",
    },
    ThemeRole {
        name: "surface",
        node: "surface",
        get: |t| t.surface,
        set: |t, c| t.surface = c,
        doc: "A card, panel or list sitting on `bg`.",
    },
    ThemeRole {
        name: "raised",
        node: "raised",
        get: |t| t.raised,
        set: |t, c| t.raised = c,
        doc: "A surface floating above content: a menu, a tooltip, a popover. Under a light theme it is no lighter than `surface` — a float on a white page separates by its border.",
    },
    ThemeRole {
        name: "sunken",
        node: "sunken",
        get: |t| t.sunken,
        set: |t, c| t.sunken = c,
        doc: "A well cut into a surface: a text field, a code block, a track.",
    },
    ThemeRole {
        name: "border",
        node: "border",
        get: |t| t.border,
        set: |t, c| t.border = c,
        doc: "The hairline between two surfaces.",
    },
    ThemeRole {
        name: "border_strong",
        node: "borderStrong",
        get: |t| t.border_strong,
        set: |t, c| t.border_strong = c,
        doc: "A border that has to be seen — a float's edge, a focused field.",
    },
    ThemeRole {
        name: "fg",
        node: "fg",
        get: |t| t.fg,
        set: |t, c| t.fg = c,
        doc: "Body text, and what a `color`-less text run resolves to.",
    },
    ThemeRole {
        name: "muted",
        node: "muted",
        get: |t| t.muted,
        set: |t, c| t.muted = c,
        doc: "Secondary text: captions, hints, an accelerator beside a label.",
    },
    ThemeRole {
        name: "faint",
        node: "faint",
        get: |t| t.faint,
        set: |t, c| t.faint = c,
        doc: "Text that is barely there: a placeholder, a gutter number.",
    },
    ThemeRole {
        name: "accent",
        node: "accent",
        get: |t| t.accent,
        set: |t, c| t.accent = c,
        doc: "The one saturated colour: the OS accent where the host reports one, the app's where it pinned one, kui's blue otherwise. The `accent` prop paints from this.",
    },
    ThemeRole {
        name: "accent_hover",
        node: "accentHover",
        get: |t| t.accent_hover,
        set: |t, c| t.accent_hover = c,
        doc: "`accent` under a pointer.",
    },
    ThemeRole {
        name: "accent_pressed",
        node: "accentPressed",
        get: |t| t.accent_pressed,
        set: |t, c| t.accent_pressed = c,
        doc: "`accent` under a press.",
    },
    ThemeRole {
        name: "on_accent",
        node: "onAccent",
        get: |t| t.on_accent,
        set: |t, c| t.on_accent = c,
        doc: "Black or white — whichever a reader can see on `accent`. What a button's label is.",
    },
    ThemeRole {
        name: "accent_soft",
        node: "accentSoft",
        get: |t| t.accent_soft,
        set: |t, c| t.accent_soft = c,
        doc: "The accent as a translucent wash rather than a fill: a selected menu row, a chosen tab, a highlighted list item. Keeps `fg` readable over it on both bases, which a fill does not.",
    },
    ThemeRole {
        name: "selection",
        node: "selection",
        get: |t| t.selection,
        set: |t, c| t.selection = c,
        doc: "What a text selection is painted under, in an editor and over a `selectable` scope alike.",
    },
    ThemeRole {
        name: "focus_ring",
        node: "focusRing",
        get: |t| t.focus_ring,
        set: |t, c| t.focus_ring = c,
        doc: "The default keyboard focus ring (ADR 0002).",
    },
    ThemeRole {
        name: "hover",
        node: "hover",
        get: |t| t.hover,
        set: |t, c| t.hover = c,
        doc: "A translucent wash over a hovered neutral control. An overlay, not a fill, so one value works on every surface.",
    },
    ThemeRole {
        name: "pressed",
        node: "pressed",
        get: |t| t.pressed,
        set: |t, c| t.pressed = c,
        doc: "The same over a pressed one, and the firmer of the two on both bases.",
    },
    ThemeRole {
        name: "success",
        node: "success",
        get: |t| t.success,
        set: |t, c| t.success = c,
        doc: "A good outcome. Readable on `surface` on both bases, which is why it is not one colour for both.",
    },
    ThemeRole {
        name: "warning",
        node: "warning",
        get: |t| t.warning,
        set: |t, c| t.warning = c,
        doc: "Something that wants attention.",
    },
    ThemeRole {
        name: "danger",
        node: "danger",
        get: |t| t.danger,
        set: |t, c| t.danger = c,
        doc: "A destructive action or a failure. The close button's hover, too.",
    },
    ThemeRole {
        name: "scrollbar",
        node: "scrollbar",
        get: |t| t.scrollbar,
        set: |t, c| t.scrollbar = c,
        doc: "The scrollbar thumb at rest.",
    },
    ThemeRole {
        name: "scrollbar_active",
        node: "scrollbarActive",
        get: |t| t.scrollbar_active,
        set: |t, c| t.scrollbar_active = c,
        doc: "The thumb while hovered or dragged.",
    },
];

/// One size the stock widgets are built from (`crate::metrics::Metrics`), pinned the way [`ThemeRole`] pins a colour: a binding
/// iterates the rows to read or write a set, so a field added to
/// `Metrics` is one row here and nothing anywhere else. The test below
/// destructures the struct exhaustively.
pub struct MetricRole {
    /// The Rust field, and the name used everywhere but Node.
    pub name: &'static str,
    /// What Node calls it (camelCase).
    pub node: &'static str,
    pub doc: &'static str,
    pub get: fn(&crate::metrics::Metrics) -> f32,
    pub set: fn(&mut crate::metrics::Metrics, f32),
    /// `Some` for a row whose stock value is the platform's own rather
    /// than a density's: the value on Windows, then everywhere else.
    /// `get` answers for the running platform; a generator prints the
    /// pair, so `docs/props.md` reads the same whichever machine wrote it.
    /// Stock and compact share it — the test below pins
    /// that `compact()` leaves such a row alone.
    pub platform: Option<PlatformValue>,
}

/// A metric's value per platform (see [`MetricRole::platform`]).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PlatformValue {
    pub windows: f32,
    pub elsewhere: f32,
}

impl PlatformValue {
    /// The member in force on the platform this was compiled for.
    pub const fn here(self) -> f32 {
        if cfg!(target_os = "windows") {
            self.windows
        } else {
            self.elsewhere
        }
    }
}

macro_rules! metric_role {
    ($field:ident, $node:literal, $doc:literal) => {
        MetricRole {
            name: stringify!($field),
            node: $node,
            get: |m| m.$field,
            set: |m, v| m.$field = v,
            doc: $doc,
            platform: None,
        }
    };
    ($field:ident, $node:literal, $doc:literal, windows $w:expr, elsewhere $e:expr) => {
        MetricRole {
            name: stringify!($field),
            node: $node,
            get: |m| m.$field,
            set: |m, v| m.$field = v,
            doc: $doc,
            platform: Some(PlatformValue {
                windows: $w,
                elsewhere: $e,
            }),
        }
    };
}

pub const METRIC_ROLES: &[MetricRole] = &[
    metric_role!(
        control_text,
        "controlText",
        "A stock control's label: the button's text size."
    ),
    metric_role!(
        chrome_text,
        "chromeText",
        "The chrome's text: a menu row, a menu-bar title, the titlebar's title."
    ),
    metric_role!(hint_text, "hintText", "A tooltip's text."),
    metric_role!(
        radius,
        "radius",
        "The corner of every stock surface: a button, a field, a menu, a tooltip."
    ),
    metric_role!(
        radius_inner,
        "radiusInner",
        "The corner of a row inside one: a menu row, a menu-bar title."
    ),
    metric_role!(
        control_pad_x,
        "controlPadX",
        "A button's horizontal padding."
    ),
    metric_role!(control_pad_y, "controlPadY", "A button's vertical padding."),
    metric_role!(
        field_pad_x,
        "fieldPadX",
        "A text field's horizontal padding."
    ),
    metric_role!(field_pad_y, "fieldPadY", "A text field's vertical padding."),
    metric_role!(hint_pad_x, "hintPadX", "A tooltip's horizontal padding."),
    metric_role!(hint_pad_y, "hintPadY", "A tooltip's vertical padding."),
    metric_role!(
        menu_pad_x,
        "menuPadX",
        "A menu row's horizontal padding, and a menu-bar title's."
    ),
    metric_role!(
        menu_pad_y,
        "menuPadY",
        "A menu row's vertical padding; a menu-bar title's is two px less."
    ),
    metric_role!(menu_width, "menuWidth", "A menu panel's width."),
    metric_role!(menu_bar_h, "menuBarH", "The drawn menu bar's height."),
    metric_role!(
        titlebar_h,
        "titlebarH",
        "The titlebar strip's height: the platform's caption height, 32 on Windows and 34 elsewhere, or the window's own where the runner knows it (backlog W22) — 40 and 52 off macOS for a launcher's `medium` or `tall` titlebar under custom chrome, and under macOS custom chrome the OS's own titlebar as `window.native_controls` measures it (32, 40 or 52 on macOS 27). The stock number stands for that window's height, so a set of the app's keeps it unless it names a number of its own. Under macOS custom chrome the strip is drawn at the measured height whatever this row says (`widgets::titlebar_height`).",
        windows crate::metrics::TITLEBAR_H_WINDOWS,
        elsewhere crate::metrics::TITLEBAR_H_ELSEWHERE
    ),
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

/// The rows by their snake_case name, for a binding that looks one up per
/// prop per node every frame (kui-lua's walk, backlog F143): a scan of the
/// ~200 names was 8% of a Lua view's lowering.
static BY_SNAKE: LazyLock<FxHashMap<&'static [u8], usize>> = LazyLock::new(|| {
    SNAKE_NAMES
        .iter()
        .enumerate()
        .map(|(i, n)| (n.as_bytes(), i))
        .collect()
});

pub fn by_snake_name(name: &str) -> Option<&'static PropDef> {
    by_snake_bytes(name.as_bytes())
}

/// [`by_snake_name`] by the name's bytes: a key read off a Lua table is
/// bytes, and is looked up without being checked as UTF-8 first.
pub fn by_snake_bytes(name: &[u8]) -> Option<&'static PropDef> {
    BY_SNAKE.get(name).map(|&i| &PROPS[i])
}

pub fn by_id(id: u32) -> Option<&'static PropDef> {
    PROPS.iter().find(|d| d.id == id)
}

/// The spelling a binding writes prop names in: JSX's camelCase rows, or the
/// snake_case ones a Lua table takes. The allow-list below is per spelling,
/// so `hover_bg` in JSX and `hoverBg` in Lua are each as unknown as a typo —
/// which is what they are: neither binding reads the other's spelling.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Spelling {
    Camel,
    Snake,
}

/// A name one binding takes for a row it cannot spell the usual way, with
/// the row's own snake_case name: `repeat` is a Lua keyword, so that row
/// also answers to CSS's own name for it. The Lua binding remaps through
/// this table, and the check below accepts both sides of it.
pub const LUA_ALIASES: &[(&str, &str)] = &[("direction", "repeat")];

/// `direction` → `repeat`: the schema name a Lua table key stands for, when
/// it is not the name itself.
pub fn lua_alias(name: &str) -> Option<&'static str> {
    LUA_ALIASES
        .iter()
        .find(|(alias, _)| *alias == name)
        .map(|(_, real)| *real)
}

/// Every prop name that is not tied to one element, per spelling: the schema
/// rows, every spelling of every composite, and the aliases.
static SHARED_NAMES: LazyLock<[FxHashSet<&'static str>; 2]> = LazyLock::new(|| {
    let mut camel: FxHashSet<&'static str> = PROPS.iter().map(|d| d.name).collect();
    let mut snake: FxHashSet<&'static str> = PROPS.iter().map(|d| d.snake_name()).collect();
    for c in CUSTOM {
        camel.extend(c.jsx_names);
        snake.extend(c.lua_names);
    }
    snake.extend(LUA_ALIASES.iter().map(|(alias, _)| *alias));
    [camel, snake]
});

fn shared_names(spelling: Spelling) -> &'static FxHashSet<&'static str> {
    &SHARED_NAMES[(spelling == Spelling::Snake) as usize]
}

/// The props only this element takes, in `spelling`. An element the table
/// does not know has none.
pub fn element_own(element: &str, spelling: Spelling) -> &'static [&'static str] {
    match ELEMENTS.iter().find(|e| e.name == element) {
        Some(e) if spelling == Spelling::Camel => e.jsx_own,
        Some(e) => e.lua_own,
        None => &[],
    }
}

/// The schema rows `element` reads, in `spelling`, when it does not read
/// them all (`ElementDef::jsx_rows`); `None` for an element that takes
/// every row, and for one the table does not know.
pub fn element_rows(element: &str, spelling: Spelling) -> Option<&'static [&'static str]> {
    let e = ELEMENTS.iter().find(|e| e.name == element)?;
    if spelling == Spelling::Camel {
        e.jsx_rows
    } else {
        e.lua_rows
    }
}

/// Is `name` a row every element reads — a schema row, a composite, an
/// alias — as opposed to a misspelling? What [`known_prop`] answers for an
/// element that admits only some rows still depends on this: a row the
/// element does not read is dropped like a misspelling, but the warning
/// can say so instead of hunting for a nearer spelling.
pub fn shared_prop(name: &str, spelling: Spelling) -> bool {
    shared_names(spelling).contains(name)
}

/// Is `name` a prop `element` reads — a schema row, a composite, an alias,
/// or one of the element's own? A binding drops everything else on the
/// floor, so everything else is a `diag::UNKNOWN_PROP` warning. An element
/// that names its rows reads those and its own, and nothing else.
pub fn known_prop(element: &str, name: &str, spelling: Spelling) -> bool {
    if element_own(element, spelling).contains(&name) {
        return true;
    }
    match element_rows(element, spelling) {
        Some(rows) => rows.contains(&name),
        None => shared_names(spelling).contains(name),
    }
}

/// The name an unknown one was probably meant to be: the same word in the
/// other convention (`hoverBg` for `hover_bg`, `onClick` for `onclick`),
/// which is what a wrong spelling almost always is. Nothing fuzzier — a
/// confident suggestion or none.
pub fn suggest(element: &str, name: &str, spelling: Spelling) -> Option<&'static str> {
    let squash = |s: &str| s.replace('_', "").to_ascii_lowercase();
    let want = squash(name);
    let near = |c: &&&str| squash(c) == want;
    let own = element_own(element, spelling).iter();
    // An element that names its rows is not sent to a row it would drop.
    match element_rows(element, spelling) {
        Some(rows) => rows.iter().chain(own).find(near).copied(),
        None => shared_names(spelling).iter().chain(own).find(near).copied(),
    }
}

/// A parsed prop value, transport-independent.
pub enum Parsed {
    F32(f32),
    Color(Color),
    Flag,
    Enum(usize),
    Sizing(Sizing),
    /// A `Min` or a `Max` row's clamp.
    Bound(Bound),
    Msg(Value),
    Str(String),
    Resource(u64),
    Keyframes(Vec<Keyframe>),
    Enter(Enter),
    Gradient(crate::gradient::Gradient),
    /// A family, already resolved by the parser (`NameRefs::family`).
    Family(FontFamily),
}

/// Everything a prop list can carry; elements pick the parts they use.
#[derive(Clone, Debug, PartialEq)]
pub struct PropsOut {
    pub spec: NodeSpec,
    pub style: TextStyle,
    pub key: Option<String>,
    /// `index`: the data index this node is opened under, which beats `key`
    /// when a binding is handed both.
    pub index: Option<u64>,
    /// `rowCount`: how many indexed rows the node's virtual list has
    /// (`Core::row_count`).
    pub row_count: Option<u64>,
    pub title: Option<String>,
    /// `alwaysOnTop`: the root asked for the window above every other
    /// app's this frame (`Core::set_always_on_top`).
    pub always_on_top: bool,
    /// `secureInput`: the root asked for secure keyboard entry while the
    /// window has the keyboard (`Core::set_secure_input`).
    pub secure_input: bool,
    /// `optionAsAlt`: which Option keys the root asked to act as Alt this
    /// frame (`Core::set_option_as_alt`).
    pub option_as_alt: crate::OptionAsAlt,
    /// `imeOff`: the root asked for the platform's input method off in
    /// its window this frame (`Core::set_ime_off`).
    pub ime_off: bool,
    pub key_focus: bool,
    /// Hover hint: the element lowering floats `widgets::tooltip` below the
    /// node while it is hovered (the parser also marks the spec hoverable).
    pub tooltip: Option<String>,
    /// The windows the root declared (`Core::declare_window`, in order).
    pub windows: Vec<(String, WindowConfig)>,
    /// Whether the `wrap` row was declared: the mode is in `style.wrap`,
    /// whose default is `Word`, so the style alone cannot say. A
    /// single-line editor folds to its width when it was
    /// (`EditOptions::wrap`); nothing else reads it.
    pub wrap: bool,
}

/// Which key a prop list opens its node under: the next auto key, the
/// `key` label, or — beating the label when a binding is handed both — the
/// `index` a virtual list opens its rows by.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Identity<'a> {
    Auto,
    Label(&'a str),
    Index(u64),
}

impl PropsOut {
    /// The identity the list gave the node (see [`Identity`]).
    pub fn identity(&self) -> Identity<'_> {
        match (self.index, &self.key) {
            (Some(i), _) => Identity::Index(i),
            (None, Some(label)) => Identity::Label(label),
            (None, None) => Identity::Auto,
        }
    }

    pub fn new() -> Self {
        PropsOut {
            spec: NodeSpec::column(),
            style: TextStyle::new(16.0),
            key: None,
            index: None,
            row_count: None,
            title: None,
            always_on_top: false,
            secure_input: false,
            option_as_alt: crate::OptionAsAlt::None,
            ime_off: false,
            key_focus: false,
            tooltip: None,
            windows: Vec::new(),
            wrap: false,
        }
    }

    /// Applies a builder step to the spec in place.
    pub fn with_spec(&mut self, f: impl FnOnce(NodeSpec) -> NodeSpec) {
        self.spec = f(std::mem::take(&mut self.spec));
    }

    /// All three effects of the `tooltip` prop at once: hover tracking and
    /// the accessible description (both [`NodeSpec::apply_tooltip`]), plus
    /// the hint the element lowering floats. A parser that only extracted
    /// the string cannot end up implementing two of the three.
    pub fn apply_tooltip(&mut self, hint: impl Into<String>) {
        let hint = hint.into();
        self.with_spec(|s| s.apply_tooltip(&hint));
        self.tooltip = Some(hint);
    }

    /// The list for a leaf — a `line`, `polygon`, `path`, `cells` grid,
    /// `image` or `edit` — whose `tooltip` the core floats beside it rather
    /// than as its last child, since a leaf holds none (backlog RG113): the
    /// spec asks for the hint drawn ([`crate::spec::AccessSpec::tooltip`]), and
    /// the leaf's door floats it while the leaf is hovered. `Core::open_from`
    /// calls it for the leaves it opens; a binding that lowers an `image`
    /// or an `edit` through its own door calls it before handing the spec
    /// over. A list with no tooltip comes back as it went in.
    pub fn for_leaf(mut self) -> Self {
        if self.tooltip.is_some() {
            self.spec.access_mut().tooltip = true;
        }
        self
    }

    /// Resolves the `pad` shorthand family and applies it — a no-op when the
    /// frontend saw none of the seven names, so a spec built from another
    /// source keeps its padding.
    pub fn apply_pad(&mut self, pad: PadShorthand) {
        if pad.declared() {
            let edges = pad.resolve();
            self.with_spec(|s| s.padding(edges));
        }
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
    // Declared at all is a fact of its own for one element (see
    // `PropsOut::wrap`); the mode still lands in the style below.
    out.wrap |= def.id == P_WRAP;
    match (&def.apply, value) {
        (Apply::SpecF32(f), Parsed::F32(v)) => out.spec = f(spec, v),
        (Apply::SpecColor(f), Parsed::Color(v)) => out.spec = f(spec, v),
        (Apply::SpecFlag(f), Parsed::Flag) => out.spec = f(spec),
        (Apply::SpecEnum(f), Parsed::Enum(v)) => out.spec = f(spec, v),
        (Apply::SpecSizing(f), Parsed::Sizing(v)) => out.spec = f(spec, v),
        (Apply::SpecBound(f), Parsed::Bound(v)) => out.spec = f(spec, v),
        (Apply::SpecMsg(f), Parsed::Msg(v)) => out.spec = f(spec, v),
        (Apply::SpecStr(f), Parsed::Str(v)) => out.spec = f(spec, &v),
        (Apply::SpecKeyframes(f), Parsed::Keyframes(v)) => out.spec = f(spec, v),
        (Apply::SpecEnter(f), Parsed::Enter(v)) => out.spec = f(spec, v),
        (Apply::SpecGradient(f), Parsed::Gradient(v)) => out.spec = f(spec, v),
        (Apply::SpecResource(f), Parsed::Resource(v)) => out.spec = f(spec, v),
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
        (Apply::StyleStr(f), Parsed::Str(v)) => {
            out.spec = spec;
            out.style = f(style, &v);
        }
        (Apply::StyleFamily(f), Parsed::Family(v)) => {
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

/// The mode a sizing's, a min's or a max's first binary slot holds for a
/// size expression, whose spelling follows as a strref (v19).
pub const SIZE_MODE_CALC: u32 = 4;

/// The mode for a size expression as data: a count of slots follows,
/// then the expression in prefix code ([`crate::calc::from_code`]) —
/// what the Node encoder sends for `{ clamp: [...] }`, so the addon
/// reads numbers and parses no text (v19).
pub const SIZE_MODE_TREE: u32 = 5;

/// The string forms of a min: `"fit"`, or a size expression
/// ([`crate::calc`]). A number arrives as a number.
pub fn min_str(s: &str) -> Result<Bound, String> {
    match s {
        "fit" => Ok(Bound::Fit),
        _ => crate::calc::bound(s)
            .map_err(|e| format!("bad min {s:?} (number | \"fit\" | a size expression): {e}")),
    }
}

/// The string forms of a max: a size expression.
pub fn max_str(s: &str) -> Result<Bound, String> {
    crate::calc::bound(s).map_err(|e| format!("bad max {s:?} (number | a size expression): {e}"))
}

/// The string forms of a sizing: "fit" | "grow" | "N%" | a size
/// expression ([`crate::calc`]).
pub fn sizing_str(s: &str) -> Result<Sizing, String> {
    match s {
        "fit" => Ok(Sizing::Fit),
        "grow" => Ok(Sizing::Grow(1.0)),
        _ => crate::calc::sizing(s).map_err(|e| {
            format!("bad sizing {s:?} (fit | grow | number | \"N%\" | a size expression): {e}")
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// An enum row's name list is the wire order — a binding sends the
    /// index — so each list is its enum's `ALL` by `name`, and the index
    /// map is a lookup in `ALL` and not a second table (`Easing`, `Repeat`, `Live` and `FontFamily` were hand maps that a
    /// variant appended or a list reordered put one off, with nothing to
    /// say so). `CURSORS` has the same pin in `cursor.rs`, `ROLES` its
    /// own below.
    #[test]
    fn every_enum_list_is_its_enum_s_all_by_name() {
        fn names(it: &[&'static str]) -> Vec<&'static str> {
            it.to_vec()
        }
        assert_eq!(
            Easing::ALL.iter().map(|e| e.name()).collect::<Vec<_>>(),
            names(EASINGS)
        );
        assert_eq!(
            Repeat::ALL.iter().map(|r| r.name()).collect::<Vec<_>>(),
            names(REPEATS)
        );
        assert_eq!(
            crate::access::Live::ALL
                .iter()
                .map(|l| l.name())
                .collect::<Vec<_>>(),
            names(LIVE)
        );
        assert_eq!(
            FontFamily::ALL
                .iter()
                .map(|f| f.name().expect("a stock family has a name"))
                .collect::<Vec<_>>(),
            names(FAMILIES)
        );
        // And the index map round-trips through the list, every index.
        for (i, e) in Easing::ALL.iter().enumerate() {
            assert_eq!(easing_idx(i), *e);
        }
        for (i, r) in Repeat::ALL.iter().enumerate() {
            assert_eq!(repeat_idx(i), *r);
        }
        for (i, l) in crate::access::Live::ALL.iter().enumerate() {
            assert_eq!(crate::access::Live::from_index(i), *l);
        }
        for (i, f) in FontFamily::ALL.iter().enumerate() {
            assert_eq!(FontFamily::from_index(i), *f);
        }
        // An index this build lacks is the default, not a panic.
        assert_eq!(easing_idx(99), Easing::default());
        assert_eq!(repeat_idx(99), Repeat::default());
        assert_eq!(FontFamily::from_index(99), FontFamily::Sans);
    }

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

    /// `menu_bar_h` → `menuBarH`: what a role's Node spelling is.
    fn snake_to_camel(name: &str) -> String {
        let mut out = String::new();
        let mut up = false;
        for ch in name.chars() {
            if ch == '_' {
                up = true;
            } else if up {
                out.extend(ch.to_uppercase());
                up = false;
            } else {
                out.push(ch);
            }
        }
        out
    }

    /// `METRIC_ROLES` restates `Metrics` the way `THEME_ROLES` restates
    /// `Theme`, and this is the same pin: an exhaustive pattern on the
    /// struct, then the names both ways.
    #[test]
    fn metric_roles_restate_the_metrics_exactly() {
        use crate::metrics::Metrics;
        let m = Metrics::compact();
        let Metrics {
            control_text,
            chrome_text,
            hint_text,
            radius,
            radius_inner,
            control_pad_x,
            control_pad_y,
            field_pad_x,
            field_pad_y,
            hint_pad_x,
            hint_pad_y,
            menu_pad_x,
            menu_pad_y,
            menu_width,
            menu_bar_h,
            titlebar_h,
        } = m;
        let fields: &[(&str, f32)] = &[
            ("control_text", control_text),
            ("chrome_text", chrome_text),
            ("hint_text", hint_text),
            ("radius", radius),
            ("radius_inner", radius_inner),
            ("control_pad_x", control_pad_x),
            ("control_pad_y", control_pad_y),
            ("field_pad_x", field_pad_x),
            ("field_pad_y", field_pad_y),
            ("hint_pad_x", hint_pad_x),
            ("hint_pad_y", hint_pad_y),
            ("menu_pad_x", menu_pad_x),
            ("menu_pad_y", menu_pad_y),
            ("menu_width", menu_width),
            ("menu_bar_h", menu_bar_h),
            ("titlebar_h", titlebar_h),
        ];
        assert_eq!(METRIC_ROLES.len(), fields.len(), "a metric has no row");
        for (name, value) in fields {
            let row = METRIC_ROLES
                .iter()
                .find(|r| r.name == *name)
                .unwrap_or_else(|| panic!("no METRIC_ROLES row for {name}"));
            assert_eq!((row.get)(&m), *value, "{name}'s row reads another field");
            let mut w = m;
            (row.set)(&mut w, 1.0);
            assert_eq!((row.get)(&w), 1.0, "{name}'s row writes another field");
        }
        for row in METRIC_ROLES {
            assert!(
                fields.iter().any(|(n, _)| *n == row.name),
                "METRIC_ROLES names a field Metrics does not have: {}",
                row.name
            );
            let camel = snake_to_camel(row.name);
            assert_eq!(row.node, camel, "{}'s Node spelling", row.name);
            // A platform row's pair is what the struct reads on this
            // platform, in both sets: the generator prints the pair for
            // both columns on the strength of this.
            if let Some(p) = row.platform {
                assert_eq!(
                    (row.get)(&Metrics::default()),
                    p.here(),
                    "{}'s stock value",
                    row.name
                );
                assert_eq!(
                    (row.get)(&m),
                    p.here(),
                    "{}: compact leaves a platform row alone",
                    row.name
                );
            }
        }
        assert!(
            METRIC_ROLES.iter().any(|r| r.platform.is_some()),
            "titlebar_h is the platform's; its row says so"
        );
    }

    /// `THEME_ROLES` restates `Theme`; this pins the two together the way
    /// `ENV_FIELDS` pins `Env`. The exhaustive pattern is the pin on the
    /// struct — a role added to `Theme` stops this compiling until it has
    /// a row — and the names are then checked in both directions, so a
    /// row cannot be forgotten and a row cannot name a field that is not
    /// there. `appearance` and `disabled_opacity` are not colours and are
    /// carried beside the roles rather than among them.
    #[test]
    fn theme_roles_restate_the_theme_exactly() {
        use crate::theme::Theme;
        let t = Theme::dark();
        let Theme {
            appearance: _,
            disabled_opacity: _,
            bg,
            surface,
            raised,
            sunken,
            border,
            border_strong,
            fg,
            muted,
            faint,
            accent,
            accent_hover,
            accent_pressed,
            on_accent,
            accent_soft,
            selection,
            focus_ring,
            hover,
            pressed,
            success,
            warning,
            danger,
            scrollbar,
            scrollbar_active,
        } = t;
        let fields: &[(&str, crate::color::Color)] = &[
            ("bg", bg),
            ("surface", surface),
            ("raised", raised),
            ("sunken", sunken),
            ("border", border),
            ("border_strong", border_strong),
            ("fg", fg),
            ("muted", muted),
            ("faint", faint),
            ("accent", accent),
            ("accent_hover", accent_hover),
            ("accent_pressed", accent_pressed),
            ("on_accent", on_accent),
            ("accent_soft", accent_soft),
            ("selection", selection),
            ("focus_ring", focus_ring),
            ("hover", hover),
            ("pressed", pressed),
            ("success", success),
            ("warning", warning),
            ("danger", danger),
            ("scrollbar", scrollbar),
            ("scrollbar_active", scrollbar_active),
        ];
        assert_eq!(THEME_ROLES.len(), fields.len(), "a role has no row");
        for (name, value) in fields {
            let row = THEME_ROLES
                .iter()
                .find(|r| r.name == *name)
                .unwrap_or_else(|| panic!("no THEME_ROLES row for {name}"));
            assert_eq!((row.get)(&t), *value, "{name}'s row reads another field");
        }
        for row in THEME_ROLES {
            assert!(
                fields.iter().any(|(n, _)| *n == row.name),
                "{} names no field",
                row.name
            );
            // Node's spelling is this one in camelCase, always.
            let camel = {
                let mut out = String::new();
                let mut up = false;
                for ch in row.name.chars() {
                    if ch == '_' {
                        up = true;
                    } else if up {
                        out.extend(ch.to_uppercase());
                        up = false;
                    } else {
                        out.push(ch);
                    }
                }
                out
            };
            assert_eq!(row.node, camel, "{}'s Node spelling", row.name);
        }
    }

    /// `ENV_FIELDS` restates `Env` and `WindowEnv`; this pins the three
    /// together. The exhaustive patterns are the pin on the structs — a
    /// field added to either stops this compiling until it is named here
    /// — and the list beside them is then checked against the table in
    /// both directions, so a row cannot be forgotten and a row cannot
    /// claim a field that does not exist.
    #[test]
    fn env_fields_restate_env_and_window_env_exactly() {
        use crate::env::{AudioEnv, Env, SystemEnv};
        use crate::window::WindowEnv;
        let Env {
            refresh_hz: _,
            focused: _,
            system,
            window,
            audio,
        } = Env::default();
        let AudioEnv { device: _, live: _ } = audio;
        let SystemEnv {
            appearance: _,
            accent: _,
            motion: _,
            locale: _,
            assistive: _,
        } = system;
        let WindowEnv {
            id: _,
            custom_chrome: _,
            maximized: _,
            fullscreen: _,
            always_on_top: _,
            native_controls: _,
            backdrop: _,
        } = window;
        let stored = [
            "refresh_hz",
            "focused",
            "system.appearance",
            "system.accent",
            "system.motion",
            "system.locale",
            "system.assistive",
            "window.id",
            "window.custom_chrome",
            "window.maximized",
            "window.fullscreen",
            "window.always_on_top",
            "window.native_controls",
            "window.backdrop",
            "audio.device",
            "audio.live",
        ];
        // A stored fact's `from` is the struct and the field, spelled the
        // one way; everything else in the column is a call.
        let from_structs: Vec<&str> = ENV_FIELDS
            .iter()
            .filter(|f| !f.from.contains('('))
            .map(|f| {
                let (strukt, field) = match f.name.split_once('.') {
                    Some(("window", field)) => ("WindowEnv", field),
                    Some(("system", field)) => ("SystemEnv", field),
                    Some(("audio", field)) => ("AudioEnv", field),
                    Some((group, _)) => panic!("{}: no struct holds a {group} fact", f.name),
                    None => ("Env", f.name),
                };
                assert_eq!(f.from, format!("`{strukt}::{field}`"), "{}", f.name);
                f.name
            })
            .collect();
        assert_eq!(from_structs, stored);
        for f in ENV_FIELDS.iter().filter(|f| f.from.contains('(')) {
            assert!(
                f.from.contains("derived") || f.from.contains("the frame's"),
                "{}: a call in `from` is derived or the frame's, and says which",
                f.name
            );
        }
    }

    /// Every spelling in `ENV_FIELDS` is unique per binding — two rows
    /// cannot land on one key — and every row says something in every
    /// column, so the generated table has no blank cells to wonder about.
    #[test]
    fn env_field_spellings_are_unique_and_complete() {
        let mut names: Vec<&str> = ENV_FIELDS.iter().map(|f| f.name).collect();
        let mut node: Vec<&str> = ENV_FIELDS
            .iter()
            .flat_map(|f| f.node.iter().copied())
            .collect();
        let mut lua: Vec<&str> = ENV_FIELDS
            .iter()
            .flat_map(|f| f.lua.iter().copied())
            .collect();
        for list in [&mut names, &mut node, &mut lua] {
            let before = list.len();
            list.sort_unstable();
            list.dedup();
            assert_eq!(list.len(), before, "a spelling is used twice");
        }
        for f in ENV_FIELDS {
            assert!(!f.c.is_empty() && !f.doc.is_empty(), "{}", f.name);
            // A binding that has no key for a fact says where it went
            // instead, so the empty cell is explained by the row itself.
            if f.node.is_empty() {
                assert!(
                    f.doc.contains("Node"),
                    "{}: where does Node carry it?",
                    f.name
                );
            }
            if f.lua.is_empty() {
                assert!(
                    f.doc.contains("Lua"),
                    "{}: where does Lua carry it?",
                    f.name
                );
            }
        }
    }

    /// An element's admitted rows are real rows in each spelling and the
    /// same rows in both, so the JSX check and the Lua check admit the
    /// same button.
    #[test]
    fn admitted_rows_are_shared_rows_in_both_spellings() {
        for e in ELEMENTS {
            let (Some(jsx), Some(lua)) = (e.jsx_rows, e.lua_rows) else {
                assert!(
                    e.jsx_rows.is_none() && e.lua_rows.is_none(),
                    "{}: one spelling only",
                    e.name
                );
                continue;
            };
            assert_eq!(
                jsx.len(),
                lua.len(),
                "{}: the lists differ in length",
                e.name
            );
            for (j, l) in jsx.iter().zip(lua) {
                assert!(
                    shared_prop(j, Spelling::Camel),
                    "{}: `{j}` is not a row",
                    e.name
                );
                assert!(
                    shared_prop(l, Spelling::Snake),
                    "{}: `{l}` is not a row",
                    e.name
                );
                assert_eq!(
                    snake_case(j),
                    *l,
                    "{}: `{j}` and `{l}` are not one row",
                    e.name
                );
            }
            // The row a reader hears first: a stock button without a name
            // is the warning `control-without-name`, so `label` is never
            // the row a closed *control* leaves out. A text is its own
            // name.
            if jsx.contains(&"onClick") {
                assert!(jsx.contains(&"label"), "{}: `label` missing", e.name);
            }
        }
        assert!(known_prop("button", "description", Spelling::Camel));
        assert!(known_prop("button", "on_click", Spelling::Snake));
        assert!(known_prop("button", "text", Spelling::Snake));
        assert!(!known_prop("button", "radius", Spelling::Camel));
        assert!(!known_prop("button", "text", Spelling::Camel));
        assert!(known_prop("box", "radius", Spelling::Camel));
        // The other spelling is looked for among the rows it reads: a row
        // it would drop is not offered as the fix.
        assert_eq!(
            suggest("button", "on_click", Spelling::Camel),
            Some("onClick")
        );
        assert_eq!(suggest("button", "hover_bg", Spelling::Camel), None);
        assert_eq!(suggest("box", "hover_bg", Spelling::Camel), Some("hoverBg"));
    }

    /// AR13: a text is content plus a style, so the rows it admits are
    /// exactly the rows that land on a `TextStyle`, plus `size`, the
    /// composite the style is built from. Every other row — a container's,
    /// an access row, `key` — is the `unknown-prop` warning in every
    /// binding rather than a silent drop.
    #[test]
    fn text_admits_exactly_the_style_rows() {
        let style: Vec<&str> = PROPS
            .iter()
            .filter(|d| d.target() == Target::Style)
            .map(|d| d.name)
            .collect();
        let mut expect = vec!["size"];
        expect.extend(style);
        let mut got = TEXT_ROWS_JSX.to_vec();
        expect.sort_unstable();
        got.sort_unstable();
        assert_eq!(got, expect, "TEXT_ROWS_JSX is not the style rows");
        for name in ["lineHeight", "color", "wrap", "size"] {
            assert!(known_prop("text", name, Spelling::Camel), "{name}");
        }
        for name in ["live", "role", "label", "onClick", "key", "pad", "bg"] {
            assert_eq!(
                known_prop("text", name, Spelling::Camel),
                name == "bg",
                "{name}: `bg` is the element's own (a span's), the rest are not rows it reads"
            );
        }
        assert!(known_prop("text", "line_height", Spelling::Snake));
        assert!(known_prop("text", "value", Spelling::Snake));
        assert!(!known_prop("text", "live", Spelling::Snake));
        assert!(!known_prop("text", "on_click", Spelling::Snake));
        // A misspelling is still steered to a row it reads, and to nothing
        // it would drop.
        assert_eq!(
            suggest("text", "max_lines", Spelling::Camel),
            Some("maxLines")
        );
        assert_eq!(suggest("text", "hover_bg", Spelling::Camel), None);
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

    /// The allow-list is per spelling, so each binding's own names pass and
    /// the other's do not — which is the point: neither binding reads the
    /// other's, so `hover_bg` in JSX is as dropped as `hoverBgg` would be.
    #[test]
    fn the_allow_list_is_per_spelling() {
        use Spelling::{Camel, Snake};
        assert!(known_prop("box", "hoverBg", Camel));
        assert!(known_prop("box", "hover_bg", Snake));
        assert!(!known_prop("box", "hover_bg", Camel));
        assert!(!known_prop("box", "hoverBg", Snake));
        // Composites, each in the spelling its binding takes.
        assert!(known_prop("box", "padX", Camel) && !known_prop("box", "padX", Snake));
        assert!(known_prop("box", "scroll", Snake) && !known_prop("box", "scroll", Camel));
        // `repeat` is a Lua keyword; the alias stands in for the row.
        assert!(known_prop("box", "direction", Snake));
        assert_eq!(lua_alias("direction"), Some("repeat"));
        // An element's own props are its own: `initial` is an editor's.
        assert!(known_prop("edit", "initial", Camel));
        assert!(!known_prop("box", "initial", Camel));
        // A shared text row on an editor is the editor's too: `wrap` is
        // what folds a single-line one, and it must not warn.
        assert!(known_prop("edit", "wrap", Camel) && known_prop("edit", "wrap", Snake));
        assert!(known_prop("image", "src", Camel) && known_prop("image", "id", Snake));
        // And nothing claims a typo.
        assert!(!known_prop("box", "colour", Camel));
    }

    #[test]
    fn a_suggestion_is_the_same_word_in_the_right_convention() {
        use Spelling::{Camel, Snake};
        assert_eq!(suggest("box", "hoverBg", Snake), Some("hover_bg"));
        assert_eq!(suggest("box", "onclick", Camel), Some("onClick"));
        assert_eq!(suggest("box", "SCROLL_X", Camel), Some("scrollX"));
        assert_eq!(suggest("edit", "Initial", Camel), Some("initial"));
        // Nothing fuzzy: a guess or nothing.
        assert_eq!(suggest("box", "colour", Camel), None);
    }

    /// Every name a binding hand-lowers has to be in one of the tables, or
    /// the binding warns about a prop it reads perfectly well.
    #[test]
    fn every_composite_and_element_prop_is_in_the_allow_list() {
        for c in CUSTOM {
            for n in c.jsx_names {
                assert!(known_prop("box", n, Spelling::Camel), "jsx `{n}`");
            }
            for n in c.lua_names {
                assert!(known_prop("box", n, Spelling::Snake), "lua `{n}`");
            }
        }
        for e in ELEMENTS {
            for n in e.jsx_own {
                assert!(known_prop(e.name, n, Spelling::Camel), "{}.{n}", e.name);
            }
            for n in e.lua_own {
                assert!(known_prop(e.name, n, Spelling::Snake), "{}.{n}", e.name);
            }
        }
    }

    #[test]
    fn every_row_applies_a_sample_of_its_kind() {
        for def in PROPS {
            // `opacity` is a 0..=1 slot whose default is the top of the
            // range, so the shared sample would clamp back to it.
            let f = if def.name == "opacity" { 0.5 } else { 7.0 };
            let sample = match def.kind {
                Kind::F32 => Parsed::F32(f),
                Kind::Color => Parsed::Color(Color::hex(0x11223344)),
                Kind::Flag => Parsed::Flag,
                Kind::Enum(_) => Parsed::Enum(1),
                Kind::Sizing => Parsed::Sizing(Sizing::Percent(0.5)),
                Kind::Min => Parsed::Bound(Bound::Fit),
                Kind::Max => Parsed::Bound(Bound::Px(10.0)),
                Kind::Msg | Kind::Tag => Parsed::Msg(Value::Int(1)),
                // A list of four names, and a string of none of them is
                // the default: none.
                Kind::Str if def.name == "scrollMods" => Parsed::Str("ctrl".into()),
                Kind::Str => Parsed::Str("name".into()),
                Kind::Family => Parsed::Family(FontFamily::Mono),
                Kind::Resource => Parsed::Resource(7),
                Kind::Keyframes => Parsed::Keyframes(vec![Keyframe::default().radius(7.0)]),
                Kind::Enter => Parsed::Enter(Enter::from(-7.0, 0.0)),
                Kind::Gradient => Parsed::Gradient(crate::gradient::Gradient::to(
                    crate::gradient::Side::Right,
                    [Color::hex(0x11223344), Color::WHITE],
                )),
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

    /// A null tag keeps the behaviour and drops the `tag` field; the
    /// contract every transport relies on to accept `null` for `Tag` rows.
    #[test]
    fn a_null_tag_still_declares_the_behaviour() {
        let mut out = PropsOut::new();
        apply(
            by_name("onKey").unwrap(),
            Parsed::Msg(Value::Null),
            &mut out,
        )
        .unwrap();
        assert_eq!(out.spec.events().on_key, Some(Value::Null));
        assert!(out.spec.hover_tracked());
        let mut out = PropsOut::new();
        apply(
            by_name("onLayout").unwrap(),
            Parsed::Msg(Value::Null),
            &mut out,
        )
        .unwrap();
        assert_eq!(out.spec.events().on_layout, Some(Value::Null));
    }

    /// `ROLES` is the wire order for the `role` enum — a binding sends the
    /// index — so a typo, or a spelling that drifted from `access.rs`, does
    /// not fail anywhere: the name simply stops parsing, and every view
    /// declaring that role silently gets [`role_idx`]'s fallback instead.
    /// Until this test that fallback was `Role::None`, which takes the node
    /// *and its whole subtree* out of the access tree — a drifted spelling
    /// would have removed part of the app from every screen reader, with
    /// nothing failing and nothing warning. Both halves are pinned here:
    /// each name is a real role, and index `i` still means `ROLES[i]`.
    #[test]
    fn every_declarable_role_name_is_a_real_role() {
        for (i, name) in ROLES.iter().enumerate() {
            let role = Role::parse(name)
                .unwrap_or_else(|| panic!("ROLES[{i}] = {name:?} is not a Role::ALL name"));
            assert_eq!(role_idx(i), role, "role index {i} lowers to the wrong role");
            assert_eq!(role_idx(i).name(), *name);
        }
    }

    /// The C header pins its role enum to `Role::ALL` by construction
    /// (`kui-ffi`'s `abi_enum!`), so a new role fails the C build until the
    /// header names it. Lua, Node and JSX have no such pin — they read
    /// `ROLES` — and a role added to `Role::ALL` and forgotten there is
    /// simply undeclarable from all three, silently. This is that pin: a
    /// role is declarable or derived, never neither and never both, and an
    /// exemption has to be one a frame really does derive, so `DERIVED_ONLY`
    /// cannot become somewhere to put a forgotten row.
    #[test]
    fn every_role_is_declarable_or_derived() {
        for role in Role::ALL {
            let declarable = ROLES.contains(&role.name());
            let derived = DERIVED_ONLY.iter().any(|(n, _)| *n == role.name());
            assert!(
                declarable || derived,
                "Role::{role:?} is in Role::ALL but no view can declare it and \
                 DERIVED_ONLY does not say the core derives it — add {:?} to \
                 ROLES, or to DERIVED_ONLY with the reason",
                role.name()
            );
            assert!(
                !(declarable && derived),
                "Role::{role:?} is in ROLES, so it is declarable — drop it from \
                 DERIVED_ONLY"
            );
        }
        for (name, why) in DERIVED_ONLY {
            assert!(!why.is_empty(), "{name:?} is exempted without a reason");
            assert!(
                Role::parse(name).is_some(),
                "DERIVED_ONLY names {name:?}, which is not a Role::ALL name"
            );
        }
        let derived = roles_one_frame_derives();
        for (name, _) in DERIVED_ONLY {
            assert!(
                derived.contains(&Role::parse(name).unwrap()),
                "DERIVED_ONLY keeps {name:?} out of ROLES because the core \
                 derives it, but no frame does any more — either it belongs in \
                 ROLES now, or the exemption is stale"
            );
        }
    }

    /// One frame that derives each of `DERIVED_ONLY`'s roles: the root is
    /// the window, a `window_drag` row the title bar, a scrolling box the
    /// scroll view, and the text inside it static text.
    fn roles_one_frame_derives() -> Vec<Role> {
        let mut core = crate::Core::new();
        let mut ui = core.frame(crate::Size::new(200.0, 200.0), 1.0);
        ui.window_title("Demo");
        ui.configure_root(NodeSpec::column().fill());
        ui.leaf_keyed("titlebar", NodeSpec::row().window_drag());
        ui.text_in_keyed(
            "scroll",
            NodeSpec::column().height(40.0).scroll_y(),
            "hello",
            TextStyle::new(12.0),
        );
        ui.finish();
        core.access_tree().nodes.iter().map(|n| n.role).collect()
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
