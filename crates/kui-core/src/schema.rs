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
//! need per-binding *extraction* — a Lua table, a serde_json map, a binary
//! stream and a C struct are genuinely different to read — but not
//! per-binding *decisions*: what "below" attaches to, what `padX` falls
//! back to, what a scroll bit implies and what a tooltip means all live in
//! one place ([`crate::spec::FloatConfig::build`],
//! [`crate::spec::PadShorthand`], [`crate::spec::NodeSpec::overflow_bits`],
//! [`PropsOut::apply_tooltip`]), and a binding pulls typed scalars out of
//! its own value type and calls them. `CUSTOM` lists them by name and wire
//! id so transports agree on identity, and `crate::conformance` makes them
//! agree on behaviour — every `CUSTOM` and `ELEMENTS` row has to appear in
//! a scene that all four bindings reproduce byte for byte, or the build
//! fails.
//!
//! Those rows are also the allow-list: a dynamic binding drops a name it
//! cannot place, so the names it may legitimately drop have to be written
//! down somewhere both bindings read. `CUSTOM` carries every spelling of
//! each composite and `ELEMENTS` the props an element lowers itself
//! (`<edit initial>`, `<image src>`), each in both conventions;
//! [`known_prop`] answers from them and everything else is a
//! `diag::UNKNOWN_PROP` warning.

use std::sync::LazyLock;

use rustc_hash::FxHashSet;

use crate::access::Role;
use crate::anim::{Easing, Repeat};
use crate::color::Color;
use crate::cursor::CursorShape;
use crate::enter::Enter;
use crate::keyframes::Keyframe;
use crate::spec::{Align, FontFamily, Min, NodeSpec, PadShorthand, Sizing, TextStyle, TextWrap};
use crate::value::Value;
use crate::window::{WindowButton, WindowConfig};

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

pub const ALIGNS: &[&str] = &["start", "center", "end"];
pub const WINDOW_ROLES: &[&str] = &["drag", "close", "minimize", "maximize"];
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
pub const WRAPS: &[&str] = &["word", "glyph", "none"];
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
    // Appended by ADR 0007. The tail is the only free position (ADR 0006),
    // which is what makes "the first fifteen can be declared" a list rather
    // than a range.
    "radioGroup",
    "menu",
    "menuItem",
];

/// How a composite container arranges its items
/// (`crate::access::Orientation::name` spellings), in wire order. Derived
/// from the container's `dir` and reported on its access node, never
/// declared — so unlike [`ROLES`] this is not a prop's enum, only a list
/// the C header restates.
pub const ORIENTATIONS: &[&str] = &["horizontal", "vertical"];

/// The roles no view can declare, because the core derives them itself
/// ([`crate::access::derived_role`]), with what derives each one. Every
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
    /// A lower clamp: number | "fit" (the node's own fit size on that axis).
    /// Binary: 2 slots (mode, value), a sizing's first two modes.
    Min,
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
}

/// Where a parsed value lands. `PropDef::target` derives from this.
pub enum Apply {
    SpecF32(fn(NodeSpec, f32) -> NodeSpec),
    SpecColor(fn(NodeSpec, Color) -> NodeSpec),
    SpecFlag(fn(NodeSpec) -> NodeSpec),
    SpecEnum(fn(NodeSpec, usize) -> NodeSpec),
    SpecSizing(fn(NodeSpec, Sizing) -> NodeSpec),
    SpecMin(fn(NodeSpec, Min) -> NodeSpec),
    SpecMsg(fn(NodeSpec, Value) -> NodeSpec),
    SpecStr(fn(NodeSpec, &str) -> NodeSpec),
    SpecKeyframes(fn(NodeSpec, Vec<Keyframe>) -> NodeSpec),
    SpecEnter(fn(NodeSpec, Enter) -> NodeSpec),
    SpecResource(fn(NodeSpec, u64) -> NodeSpec),
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

/// Binary min decode: (mode, value) → Min, the first two sizing modes.
pub fn min_num(mode: u32, value: f64) -> Min {
    match mode {
        1 => Min::FIT,
        _ => Min::px(value as f32),
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
        doc: "Horizontal size: px | \"fit\" | \"grow\" | \"N%\".",
    },
    PropDef {
        name: "height",
        id: P_HEIGHT,
        kind: Kind::Sizing,
        apply: Apply::SpecSizing(|s, v| s.height(v)),
        doc: "Vertical size: px | \"fit\" | \"grow\" | \"N%\".",
    },
    PropDef {
        name: "minWidth",
        id: P_MIN_W,
        kind: Kind::Min,
        apply: Apply::SpecMin(|s, v| s.min_width(v)),
        doc: "Lower width clamp: logical px, or \"fit\" for the node's own fit width. \
              \"fit\" under `width=\"grow\"` is a content floor — CSS's `flex: 1 0 auto` — \
              which is what an i3-style tab bar is: tabs that split the bar evenly \
              while they fit and sit at their label's width, scrolling, once they do \
              not. Opt-in, because a fit width is the unwrapped one: a paragraph in a \
              grow column would stop wrapping under it.",
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
        kind: Kind::Min,
        apply: Apply::SpecMin(|s, v| s.min_height(v)),
        doc: "Lower height clamp: logical px, or \"fit\" for the node's own fit height (see `minWidth`).",
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
        name: "focusable",
        id: P_FOCUSABLE,
        kind: Kind::Flag,
        apply: Apply::SpecFlag(|s| s.focusable()),
        doc: "Reachable by Tab (and focused by a click) without a click payload or a control role — a row that opens on Enter. Editors, key sinks, `onClick` boxes and the control roles are focusable already.",
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
        doc: "Drag tag: emits {kind:\"drag\", phase, x, y, dx, dy, parent, tag} events, `dx`/`dy` measured from the press point in every phase.",
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
        name: "onContextMenu",
        id: P_ON_CONTEXT_MENU,
        kind: Kind::Tag,
        apply: Apply::SpecMsg(|s, v| s.on_context_menu(v)),
        doc: "Context-menu tag: a secondary-button (right) press emits {kind:\"contextmenu\", x, y, tag} on the node, at the logical viewport point to open the menu at. The press moves no focus, places no caret and produces no click, so right-clicking a selection keeps it; the topmost node under the pointer is the one asked, as for a click.",
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
        doc: "Overrides the pointer shape over this node. Unset, the core derives one from what the node does — an editor is `text`, an `onClick` or `focusable` node `pointer`, an `onDrag` node `grab` (`grabbing` while dragging), window chrome and a plain box `default` — so this is for what that cannot know: a splitter (`ewResize` / `nsResize`), a `disabled` control that says `notAllowed`.",
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
        doc: "With transition: also ease the node's position (reordered siblings slide). While it eases, the node is drawn between where it was and where this frame put it — not at the declared `dx`/`dy`, or its slot in the row — so anything else positioned from those numbers drifts for the transition's length: a canvas of floats eases everything or nothing.",
    },
    PropDef {
        name: "keyframes",
        id: P_KEYFRAMES,
        kind: Kind::Keyframes,
        apply: Apply::SpecKeyframes(|s, k| s.keyframes(k)),
        doc: "CSS-style stops `[{ at?, width?, height?, bg?, radius?, opacity? }, …]`: the slots they name cycle through them over `transition` ms, forever, without the view redrawing; `at` is 0..1 and spreads evenly when omitted.",
    },
    PropDef {
        name: "enter",
        id: P_ENTER,
        kind: Kind::Enter,
        apply: Apply::SpecEnter(|s, e| s.enter(e)),
        doc: "Where the node starts the first frame it is seen `{ dx?, dy?, width?, height?, bg?, radius?, opacity? }`: those slots ease in from there over `transition` ms instead of snapping (`dx`/`dy` slide it in from that far away, `opacity: 0` fades the whole subtree in).",
    },
    PropDef {
        name: "exit",
        id: P_EXIT,
        kind: Kind::Enter,
        apply: Apply::SpecEnter(|s, e| s.exit(e)),
        doc: "Where the node ends the frame after the view stops declaring it `{ dx?, dy?, width?, height?, bg?, radius?, opacity? }` — an `enter` read the other way. With a `transition`, the departing subtree is copied out of the last frame that had it and replayed frozen, in its place (the pass it painted in, just under the node that painted after it — a panel under a HUD leaves under it) and inert (no clicks, no Tab stop, no access row) while those slots ease from where they were, then dropped; without one it vanishes at once as it always did. `width`/`height` resize the departing node's own box only — the subtree inside it is a picture and is not laid out again. Needs a stable key across frames.",
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
    PropDef {
        name: "role",
        id: P_ROLE,
        kind: Kind::Enum(ROLES),
        apply: Apply::SpecEnum(|s, i| s.role(role_idx(i))),
        doc: "What the node is to assistive technology. Unset, the core derives one (an `onClick` node is a button, an editor a text input, a scrolling box a scroll view, a plain box nothing); `none` hides the node and its subtree from the access tree.",
    },
    PropDef {
        name: "label",
        id: P_LABEL,
        kind: Kind::Str,
        apply: Apply::SpecStr(|s, v| s.label(v)),
        doc: "The accessible name. Without one a button, link, tab or heading is named by the text inside it; an image, an icon-only button and a `modal` dialog have none, and the core warns (`image-without-label`, `control-without-name`, `modal-without-name`).",
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
    pub doc: &'static str,
}

pub const CUSTOM: &[CustomProp] = &[
    CustomProp {
        name: "dir",
        id: P_DIR,
        jsx_names: &["dir"],
        lua_names: &[],
        jsx: "`dir=\"row\" | \"column\"`",
        lua: "`row { }` / `column { }`",
        c: "`dir` (`KUI_ROW` / `KUI_COLUMN`)",
        doc: "Main axis; column is the default.",
    },
    CustomProp {
        name: "size",
        id: P_SIZE,
        jsx_names: &["size"],
        lua_names: &["size"],
        jsx: "`size` (text)",
        lua: "`size`",
        c: "`KuiTextStyle.size`",
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
        doc: "Clip children; scroll (implies clip) with retained offsets and live scrollbars. A `radius` on the same node rounds the clip, so a rounded card does not show square corners poking out of it; nesting two rounded clippers keeps only the corners neither of them moved, and hit-testing stays rectangular. Every frontend ORs the same bits and hands them to `NodeSpec::overflow_bits`. An offset is kept while the key is declared; an undeclared one is kept until the budget needs the room (1024 undeclared entries, longest-undeclared evicted first).",
    },
    CustomProp {
        name: "float",
        id: P_FLOAT,
        jsx_names: &["float"],
        lua_names: &["float"],
        jsx: "`float=\"below\" | \"above\" | \"parent\" | \"viewport\"` or `{ anchor, at, self, dx, dy, fit }`",
        lua: "`float = \"below\"` or `float = { anchor=, at=, self=, dx=, dy=, fit= }`",
        c: "`float_mode`, `float_anchor_x/y`, `float_self_x/y`, `float_dx/dy`, `float_fit`; `kui_spec_float_preset` fills them from a preset name",
        doc: "Out-of-flow positioning against the parent or the viewport; `fit` flips/clamps to stay on screen. The four preset names resolve in `FloatConfig::preset`, and `anchor` takes any of them — an override left out keeps the preset's own value, so `{ anchor: \"below\", dx: 4 }` still hangs below with its 6px gap.",
    },
    CustomProp {
        name: "keyFocus",
        id: P_KEY_FOCUS,
        jsx_names: &["keyFocus"],
        lua_names: &["key_focus"],
        jsx: "`keyFocus`",
        lua: "`key_focus`",
        c: "`kui_set_key_focus`",
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
        doc: "Stable identity for retained state (scroll offsets, editors, transitions; keys are hashes of the path from the root). Retained state outlives the key's absence, under a budget on the states nobody declares (see `<edit>` and the overflow props).",
    },
    CustomProp {
        name: "title",
        id: P_TITLE,
        jsx_names: &["title"],
        lua_names: &["window_title"],
        jsx: "`title` (root box only)",
        lua: "`window_title` (root table)",
        c: "`kui_window_title`",
        doc: "Declares the window title for this frame; the driver diffs and applies.",
    },
    CustomProp {
        name: "windows",
        id: P_WINDOWS,
        jsx_names: &["windows"],
        lua_names: &["windows"],
        jsx: "`windows={[{ name, kind?, anchor?, width?, height?, activates? }]}` (root box only; `windows: (model) => [...]` in the loop config)",
        lua: "`windows = { { name=, kind=, anchor=, width=, height=, activates= } }` (root table)",
        c: "`kui_window_declare`",
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
        doc: "Floats a hint below the node while hovered. All three effects — hover tracking, the accessible description, and the float itself — come from `PropsOut::apply_tooltip` / `NodeSpec::apply_tooltip`, so no frontend can implement two of them. The `description` row is that middle effect on its own, for a hint that is spoken and never drawn.",
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
    ("enter", "`enter` (`KuiEnter`, with `set` bits)"),
    ("exit", "`exit` (`KuiEnter`, with `set` bits)"),
    ("opacity", "`opacity` with `opacity_set`"),
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
        "modal",
        "`modal` (a borrowed `KuiValue*`, cloned while the node opens)",
    ),
    ("onHover", "`on_hover` argument of `kui_open_with`"),
    (
        "onLayout",
        "`on_layout` (a borrowed `KuiValue*`, cloned while the node opens)",
    ),
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
    pub doc: &'static str,
}

/// The rows the stock button reads (`ElementDef::jsx_rows` / `lua_rows`):
/// the click, the identity, and the access rows — what a button *is*
/// and what a reader says of it, never what it looks like. The two lists
/// are the same rows in each spelling, index for index.
pub const BUTTON_ROWS_JSX: &[&str] = &[
    "onClick",
    "key",
    "label",
    "description",
    "tooltip",
    "disabled",
];
pub const BUTTON_ROWS_LUA: &[&str] = &[
    "on_click",
    "key",
    "label",
    "description",
    "tooltip",
    "disabled",
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
        doc: "A container: every container prop applies.",
    },
    ElementDef {
        name: "text",
        jsx_own: &["bold", "italic"],
        lua_own: &["value", "spans"],
        jsx_rows: None,
        lua_rows: None,
        jsx: "`<text>` with `<span bold italic color>` children",
        lua: "`text(\"s\", {…})`, `text({ \"a\", { \"b\", bold = true } })`",
        c: "`kui_text`, `kui_rich_text`",
        doc: "Plain or rich text; spans shape as one paragraph, so wrapping crosses style boundaries. `wrap`, `maxLines` and `ellipsis` control line breaking.",
    },
    ElementDef {
        name: "button",
        jsx_own: &[],
        lua_own: &["text"],
        jsx_rows: Some(BUTTON_ROWS_JSX),
        lua_rows: Some(BUTTON_ROWS_LUA),
        jsx: "`<button onClick label description tooltip disabled>`",
        lua: "`button { label=, on_click=, text=, description=, tooltip=, disabled= }`",
        c: "`kui_button`, `kui_button_with`",
        doc: "The stock button: `widgets::button_spec()` with hover/pressed colors declared on the node, keyed by its text (`key` overrides). Its look is its spec, so the layout and paint rows are closed — declared, they are dropped with an `unknown-prop` warning naming the rows it does read — and those are the access rows: `label` when the text is not the name, `description`, `tooltip`, and `disabled` (inert, and dimmed to half). In Lua `label` is the name and the text both unless `text` says otherwise; in C the rows ride a `KuiSpec` whose other fields `kui_button_with` ignores. A button that needs any other row is a box with `role=\"button\"` and the same rows spelled out.",
    },
    ElementDef {
        name: "edit",
        jsx_own: &["id", "initial", "multiline", "autofocus"],
        lua_own: &["initial", "multiline", "autofocus"],
        jsx_rows: None,
        lua_rows: None,
        jsx: "`<edit key initial multiline autofocus>`",
        lua: "`edit { key=, initial=, … }`, `input { label= }`",
        c: "`kui_text_edit`, `kui_text_input`",
        doc: "Retained editor state by key; read it back with `editText(key)` after a `changed` event. `initial` seeds a new editor only — a key declared again keeps the draft the user typed, and `setEditText(key, text)` is what resets one (it leaves the caret at the end). A single-line editor opens with the caret after its seeded text, as a native field does; a multiline one is a document and opens at its top. State is kept while the key is declared; an undeclared one is kept until the budget needs the room (256 undeclared editors, longest-undeclared evicted first).",
    },
    ElementDef {
        name: "image",
        jsx_own: &["src"],
        lua_own: &["id"],
        jsx_rows: None,
        lua_rows: None,
        jsx: "`<image src={id}>`",
        lua: "`image { id= }`",
        c: "`kui_image`",
        doc: "A registered RGBA image; `fit` takes the pixel size, a fit height against a resolved width keeps the aspect, radius rounds it.",
    },
    ElementDef {
        name: "line",
        // `width` and `color` are schema rows already (a sizing and the text
        // colour); on a line they are the stroke's width and colour.
        jsx_own: &["from", "to", "points", "curve"],
        lua_own: &["from", "to", "points", "curve"],
        jsx_rows: None,
        lua_rows: None,
        jsx: "`<line from={[x,y]} to={[x,y]} width color/>`, `<line points={[[x,y],…]} curve/>`",
        lua: "`line { from={x,y}, to={x,y}, width=, color= }`, `line { points={{x,y},…}, curve=true }`",
        c: "`kui_line`, `kui_polyline`",
        doc: "A round-capped stroke: one segment, a polyline through `points`, or a smooth curve through them with `curve`. Always a float in its parent's box space (`float=\"viewport\"` for viewport space), sized to its own bounding box, so it takes no room in a row or column. `width` is the stroke width (default 1) and `color` the stroke colour; `transition` eases the colour, and with `slide` beside it the stroke's position too — the points ride its box, so a stroke whose ends all move together slides with them, while one whose ends move apart resizes at once (a canvas of floats eases everything or nothing, connectors included). Takes no pointer input and has no access row (`docs/adr/0010-a-segment-primitive.md`). What it costs: one quad per segment, and a curve is flattened in the core at one piece per 6 logical px of chord (at most 32 per span) — fixed rather than tolerance-driven so every binding gets the same pieces and the corpus can pin them — so a nine-point curve over ~50 px spans is ~60 quads, and a `quadCount` budget should expect it.",
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
        doc: "Adaptive titlebar for custom chrome: drag strip, native-control inset, window buttons.",
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
        doc: "Just the min/max/close buttons, for fully custom titlebars.",
    },
    ElementDef {
        name: "tooltip",
        jsx_own: &[],
        lua_own: &["value"],
        jsx_rows: None,
        lua_rows: None,
        jsx: "`tooltip=\"hint\"` prop (see composites)",
        lua: "`tooltip(\"hint\")` / `tooltip { … }` nodes, or the prop",
        c: "`kui_tooltip`, `kui_tooltip_with`",
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
        doc: "A playback retained by node key: present = playing (once, or looped), gone = stopped; `volume` / `paused` apply live, a changed `src` restarts; a `tag` brings back `{kind:\"sound\", phase:\"ended\", tag}`. Draws nothing. `finish` changes what *gone* means: the node's removal releases the playback rather than stopping it, so a one-shot plays to its end and the view need not know the asset's length to declare the node for it (a loop still stops on removal — there is no end to reach — and a paused playback released has nothing to finish). Without it, the way to play a sound whole is to hold the node declared until the `tag`'s `ended` message arrives.",
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
        doc: "A pointer-captured drag on an `onDrag` node. `x`/`y` are where the pointer is; `dx`/`dy` are its displacement **from the press point**, in every phase — `start` carries zero, a `move` how far the pointer is from where it pressed, `end` the whole distance — so a handler sets `value = start + dx` rather than summing deltas, and can commit from `end` alone. Nothing is dropped under the click slop (3 px, measured from the press): the first `move` already carries the whole distance. `parent` is the container rect, so fractions need no geometry query.",
    },
    EventDef {
        kind: "key",
        payload: "`{ kind: \"key\", phase: \"down\" | \"up\", code, physical, shift, ctrl, alt, super, text, repeat, tag }`",
        doc: "A key press or release on the focused `onKey` sink; `code` is a character or a name (`\"left\"`, `\"f5\"`) and is what a keymap binds against. `physical` is the US-QWERTY key at that *position*, spelled the same way — bind it instead when you want the finger rather than the label (WASD stays a square on every layout). `code` follows the layout while the layout speaks ASCII, so a chord lands on the key the user can see (Dvorak's `⌥v` on the key printed V); on a layout that does not (Cyrillic, Greek, Hebrew, Arabic) the position's US letter stands in, so a Latin keymap keeps matching instead of matching nothing. `repeat` marks a press the OS auto-repeated; `text` is what the press would insert — always the layout's own character — and is null on every release. A key only comes up where it went down: a release whose press the sink never got is dropped, and focus leaving while a key is held delivers the `up` first, so a held-key binding (WASD, press-and-hold) cannot be left stuck down.",
    },
    EventDef {
        kind: "contextmenu",
        payload: "`{ kind: \"contextmenu\", x, y, tag }`",
        doc: "A secondary-button press on an `onContextMenu` node, on the press rather than the release; `x`/`y` are logical viewport coordinates — where the menu goes. The core opens nothing: the app declares the menu (a `modal` float) and stops declaring it on `dismiss`.",
    },
    EventDef {
        kind: "hover",
        payload: "`{ kind: \"hover\", phase: \"enter\" | \"leave\", tag }`",
        doc: "The pointer entered or left an `onHover` node — also when a new frame moved it under a still cursor.",
    },
    EventDef {
        kind: "layout",
        payload: "`{ kind: \"layout\", x, y, w, h, parent: { x, y, w, h }, tag }`",
        doc: "The rect layout gave an `onLayout` node (logical px, viewport coords, after scrolling and easing): on its first frame and whenever it changes, never on a frame that left it alone.",
    },
    EventDef {
        kind: "resize",
        payload: "`{ kind: \"resize\", width, height, scale }`",
        doc: "The viewport changed size or DPI (logical px, delivered to the host on the root); `KuiWindow.size()` queries the same numbers.",
    },
    EventDef {
        kind: "window",
        payload: "`{ kind: \"window\", phase: \"opened\" | \"closed\", name, id }`",
        doc: "A declared window opened (the diff queued its `Open`) or closed — because nothing declares it any more, or because the user closed it, in which case it stays closed while still declared: stop declaring `name`, then declare it again to reopen. `id` is what its events carry; the event itself is on the root of whichever window's frame noticed.",
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
    EventDef {
        kind: "sound",
        payload: "`{ kind: \"sound\", phase: \"ended\", playback, tag }`",
        doc: "A tagged playback (`play(id, { tag })` or `<audio tag>`) finished on its own — never when something stopped it.",
    },
    EventDef {
        kind: "dismiss",
        payload: "`{ kind: \"dismiss\", reason: \"escape\" | \"outside\", tag }` on a node; `{ kind: \"dismiss\", reason, name, id }` on the root for a popup window",
        doc: "The user asked for a surface to go away — Escape, or a press that landed outside it. The core closes nothing: the app stops declaring the surface (or asks first). A `modal` node gets one on the node, carrying its tag, and only the modal in effect does; a `kind: \"popup\"` window gets one on the root, carrying the window's `name` and `id`, reported by the driver because a press outside a window and a key sent to a non-activating one are both facts only the OS has. The two are the same event, so a dropdown that graduates from a modal float to a popup window changes its declaration and not its handler.",
    },
    EventDef {
        kind: "access",
        payload: "`{ kind: \"access\", action, tag, text?, anchor?: { line, offset }, focus?: { line, offset } }`",
        doc: "Assistive technology — or the keyboard — asked for what only the app can do: `increment` / `decrement` on a `slider` role (a reader's nudge, or the arrow keys on the focused slider); `setValue` / `replaceSelectedText` (with `text`) / `setTextSelection` (with `anchor` and `focus` as line ordinals and byte offsets) on a custom editor. `tag` is the node's `onClick` payload (or its `onDrag` / `onKey` tag). Every other request resolves in the core and arrives as the events a pointer would have produced.",
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
}

pub const ENV_FIELDS: &[EnvField] = &[
    EnvField {
        name: "refresh_hz",
        from: "`Env::refresh_hz`",
        node: &["refreshHz"],
        lua: &["refresh_hz"],
        c: "`kui_env_set(refresh_hz)`",
        doc: "Display refresh rate in Hz. \"The host cannot tell\" is `null` in Node (a stable shape to destructure, typed `number | null`), an absent key in Lua, and a rate at or below zero in C.",
    },
    EnvField {
        name: "frame_budget_ms",
        from: "`Env::frame_budget_ms()`, derived",
        node: &["frameBudgetMs"],
        lua: &["frame_budget_ms"],
        c: "—",
        doc: "One vsync interval at `refresh_hz`, or at 120 Hz when the host cannot tell: the per-frame time budget, and what the latency HUD draws its line at. Derived, and in the reading anyway, so no view restates the fallback. A C host is the one that knows the rate and computes its own.",
    },
    EnvField {
        name: "focused",
        from: "`Env::focused`",
        node: &["focused"],
        lua: &["focused"],
        c: "`kui_env_set(focused)`",
        doc: "Whether the *window* has the keyboard at all. Not the focused node — that is the `focus` row.",
    },
    EnvField {
        name: "window.id",
        from: "`WindowEnv::id`",
        node: &["window.id"],
        lua: &["window.id"],
        c: "`kui_env_set_window(window)`, read back by `kui_ctx_window`",
        doc: "Which window this frame draws, assigned by the driver: 0 for the window the app starts in. Every event from it carries the same number.",
    },
    EnvField {
        name: "window.custom_chrome",
        from: "`WindowEnv::custom_chrome`",
        node: &["window.customChrome"],
        lua: &["window.custom_chrome"],
        c: "`kui_env_set_window(custom_chrome)`",
        doc: "The host asked the app to draw its own chrome, so there is no native titlebar to sit under. `<titlebar>` and `<windowButtons>` build nothing when this is false.",
    },
    EnvField {
        name: "window.maximized",
        from: "`WindowEnv::maximized`",
        node: &["window.maximized"],
        lua: &["window.maximized"],
        c: "`kui_env_set_window(maximized)`",
        doc: "The window is maximized — what picks the restore glyph over the maximize one.",
    },
    EnvField {
        name: "window.fullscreen",
        from: "`WindowEnv::fullscreen`",
        node: &["window.fullscreen"],
        lua: &["window.fullscreen"],
        c: "`kui_env_set_window(fullscreen)`",
        doc: "The window is fullscreen.",
    },
    EnvField {
        name: "window.native_controls",
        from: "`WindowEnv::native_controls`",
        node: &["window.nativeControls"],
        lua: &["window.controls_w", "window.controls_h"],
        c: "`kui_env_set_window(controls_w, controls_h)`",
        doc: "Area (logical px, window coordinates) covered by controls the OS still draws over our content — the macOS traffic lights under custom chrome. Keep out of it. Node hands back the `Rect` the core holds (`{x, y, w, h}`, or `null` for none); Lua and C flatten it to a width and height anchored at the window origin (absent in Lua, `0` in C, for none), which is the shape C's two numbers can express and where the one real instance sits.",
    },
    EnvField {
        name: "viewport.w",
        from: "`Core::viewport()`, the frame's",
        node: &["viewport.width"],
        lua: &["viewport_w"],
        c: "`kui_frame_begin(w)`",
        doc: "The logical width the current (or last) frame was begun with — the other host fact a view wants at the same moment, so it rides in the same reading. Node's `viewport` is the `WindowSize` shape `runWindowed` already uses.",
    },
    EnvField {
        name: "viewport.h",
        from: "`Core::viewport()`, the frame's",
        node: &["viewport.height"],
        lua: &["viewport_h"],
        c: "`kui_frame_begin(h)`",
        doc: "Its logical height.",
    },
    EnvField {
        name: "scale",
        from: "`Core::scale()`, the frame's",
        node: &["viewport.scale"],
        lua: &[],
        c: "`kui_frame_begin(scale)`",
        doc: "Device pixels per logical px. Lua has no reading: a script sees logical px only.",
    },
    EnvField {
        name: "focus",
        from: "`Core::focus()`, the frame's",
        node: &[],
        lua: &["focus"],
        c: "`kui_focused()`",
        doc: "The focused *node*'s key, as events carry it (absent for none). A value the host wrote before the view ran, so it lags a same-frame verb by one frame; `env.is_focused(key)` is the live query. Node spells it as the call `focused()` on the context, and C as `kui_focused`, rather than a key on `env`.",
    },
    EnvField {
        name: "focus_visible",
        from: "`Core::focus_visible()`, the frame's",
        node: &[],
        lua: &["focus_visible"],
        c: "`kui_focus_visible()`",
        doc: "Whether focus shows — it got there by Tab or assistive technology rather than a click. Node: `focusVisible()` on the context.",
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
    Min(Min),
    Msg(Value),
    Str(String),
    Resource(u64),
    Keyframes(Vec<Keyframe>),
    Enter(Enter),
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
    /// The windows the root declared (`Core::declare_window`, in order).
    pub windows: Vec<(String, WindowConfig)>,
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
            windows: Vec::new(),
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
    match (&def.apply, value) {
        (Apply::SpecF32(f), Parsed::F32(v)) => out.spec = f(spec, v),
        (Apply::SpecColor(f), Parsed::Color(v)) => out.spec = f(spec, v),
        (Apply::SpecFlag(f), Parsed::Flag) => out.spec = f(spec),
        (Apply::SpecEnum(f), Parsed::Enum(v)) => out.spec = f(spec, v),
        (Apply::SpecSizing(f), Parsed::Sizing(v)) => out.spec = f(spec, v),
        (Apply::SpecMin(f), Parsed::Min(v)) => out.spec = f(spec, v),
        (Apply::SpecMsg(f), Parsed::Msg(v)) => out.spec = f(spec, v),
        (Apply::SpecStr(f), Parsed::Str(v)) => out.spec = f(spec, &v),
        (Apply::SpecKeyframes(f), Parsed::Keyframes(v)) => out.spec = f(spec, v),
        (Apply::SpecEnter(f), Parsed::Enter(v)) => out.spec = f(spec, v),
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
/// The string form of a min: only `"fit"`. A number arrives as a number.
pub fn min_str(s: &str) -> Result<Min, String> {
    match s {
        "fit" => Ok(Min::FIT),
        _ => Err(format!("bad min {s:?} (number | \"fit\")")),
    }
}

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

    /// `ENV_FIELDS` restates `Env` and `WindowEnv`; this pins the three
    /// together. The exhaustive patterns are the pin on the structs — a
    /// field added to either stops this compiling until it is named here
    /// — and the list beside them is then checked against the table in
    /// both directions, so a row cannot be forgotten and a row cannot
    /// claim a field that does not exist.
    #[test]
    fn env_fields_restate_env_and_window_env_exactly() {
        use crate::env::Env;
        use crate::window::WindowEnv;
        let Env {
            refresh_hz: _,
            focused: _,
            window,
        } = Env::default();
        let WindowEnv {
            id: _,
            custom_chrome: _,
            maximized: _,
            fullscreen: _,
            native_controls: _,
        } = window;
        let stored = [
            "refresh_hz",
            "focused",
            "window.id",
            "window.custom_chrome",
            "window.maximized",
            "window.fullscreen",
            "window.native_controls",
        ];
        // A stored fact's `from` is the struct and the field, spelled the
        // one way; everything else in the column is a call.
        let from_structs: Vec<&str> = ENV_FIELDS
            .iter()
            .filter(|f| !f.from.contains('('))
            .map(|f| {
                let (strukt, field) = match f.name.strip_prefix("window.") {
                    Some(field) => ("WindowEnv", field),
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
            // the row a closed element leaves out.
            assert!(jsx.contains(&"label"), "{}: `label` missing", e.name);
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
                Kind::Min => Parsed::Min(Min::FIT),
                Kind::Msg | Kind::Tag => Parsed::Msg(Value::Int(1)),
                Kind::Str => Parsed::Str("name".into()),
                Kind::Resource => Parsed::Resource(7),
                Kind::Keyframes => Parsed::Keyframes(vec![Keyframe::default().radius(7.0)]),
                Kind::Enter => Parsed::Enter(Enter::from(-7.0, 0.0)),
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
        ui.with_keyed("titlebar", NodeSpec::row().window_drag(), |_| {});
        ui.with_keyed(
            "scroll",
            NodeSpec::column().height(Sizing::Fixed(40.0)).scroll_y(),
            |ui| ui.text("hello", TextStyle::new(12.0)),
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
