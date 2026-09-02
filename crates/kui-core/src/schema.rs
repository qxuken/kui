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

use crate::anim::Easing;
use crate::color::Color;
use crate::spec::{Align, FontFamily, NodeSpec, Sizing, TextStyle};
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

pub const ALIGNS: &[&str] = &["start", "center", "end"];
pub const WINDOW_ROLES: &[&str] = &["drag", "close", "minimize", "maximize"];
pub const FAMILIES: &[&str] = &["sans", "serif", "mono"];
pub const EASINGS: &[&str] = &[
    "easeOut",
    "linear",
    "easeIn",
    "easeInOut",
    "spring",
    "bouncy",
];

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
}

/// Where a parsed value lands. `PropDef::target` derives from this.
pub enum Apply {
    SpecF32(fn(NodeSpec, f32) -> NodeSpec),
    SpecColor(fn(NodeSpec, Color) -> NodeSpec),
    SpecFlag(fn(NodeSpec) -> NodeSpec),
    SpecEnum(fn(NodeSpec, usize) -> NodeSpec),
    SpecSizing(fn(NodeSpec, Sizing) -> NodeSpec),
    SpecMsg(fn(NodeSpec, Value) -> NodeSpec),
    StyleF32(fn(TextStyle, f32) -> TextStyle),
    StyleColor(fn(TextStyle, Color) -> TextStyle),
    StyleEnum(fn(TextStyle, usize) -> TextStyle),
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
            Apply::StyleF32(_) | Apply::StyleColor(_) | Apply::StyleEnum(_) => Target::Style,
            _ => Target::Spec,
        }
    }

    /// The Lua-side name: `minWidth` → `min_width`.
    pub fn snake_name(&self) -> &'static str {
        SNAKE_NAMES[self.index()]
    }

    fn index(&self) -> usize {
        PROPS
            .iter()
            .position(|d| std::ptr::eq(d, self))
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
        doc: "Corner radius (logical px).",
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
];

/// Props every binding handles by hand (composites and constructor-order
/// specials), with their wire ids so transports agree on identity.
pub const CUSTOM: &[(&str, u32)] = &[
    ("dir", P_DIR),
    ("size", P_SIZE),
    ("pad", P_PAD),
    ("border", P_BORDER),
    ("overflow", P_OVERFLOW),
    ("float", P_FLOAT),
    ("keyFocus", P_KEY_FOCUS),
    ("key", P_KEY),
    ("title", P_TITLE),
    ("tooltip", P_TOOLTIP),
];

static SNAKE_NAMES: LazyLock<Vec<&'static str>> = LazyLock::new(|| {
    PROPS
        .iter()
        .map(|d| Box::leak(snake_case(d.name).into_boxed_str()) as &'static str)
        .collect()
});

/// `minWidth` → `min_width`; names without capitals pass through.
pub fn snake_case(name: &str) -> String {
    let mut out = String::with_capacity(name.len() + 2);
    for c in name.chars() {
        if c.is_ascii_uppercase() {
            out.push('_');
            out.push(c.to_ascii_lowercase());
        } else {
            out.push(c);
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
            .chain(CUSTOM.iter().copied())
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
