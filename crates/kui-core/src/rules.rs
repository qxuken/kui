//! Props by condition (ADR 0069): a node's `when` list, the conditions
//! each entry tests and the props it sets while they hold.
//!
//! An entry ([`Rule`]) is a condition set ([`Cond`]) — every condition in
//! it must hold — and the props it sets ([`Patch`]). Every entry that
//! matches applies, in list order, over the node's own props, so a later
//! entry wins a prop an earlier one set. The conditions read four things
//! the core knows: the node's own size, the window's, the system's
//! settings and the node's interaction state.
//!
//! ```rust
//! use kui_core::{Color, Cond, NodeSpec};
//!
//! let card = NodeSpec::row()
//!     .width(kui_core::Sizing::Grow(1.0))
//!     .gap(16.0)
//!     .bg(Color::hex(0xffffffff))
//!     // Under 700 px wide, a column; the band keeps it one up to 716.
//!     .when(Cond::width_below(700.0).band(16.0), |p| {
//!         p.dir(kui_core::Dir::Column).gap(8.0)
//!     })
//!     .when(Cond::Dark, |p| p.bg(Color::hex(0x202020ff)))
//!     .when(Cond::Hovered, |p| p.border(1.0, Color::hex(0x4488ffff)));
//! assert_eq!(card.rules_spec().entries().len(), 3);
//! ```
//!
//! When they resolve: the window, environment and state entries as the
//! node opens, before its transition eases it, so a state entry is what
//! `hoverBg` always was — resolved by the core, eased by `transition`,
//! and never a reason to run the view again. Entries over the node's own
//! size resolve inside layout (`crate::layout`): a width entry once the
//! node's width is final and before its children are laid across it, a
//! height entry once its height is, with one more layout pass on the
//! frame a height match changes. Such an entry sets layout props only.

use crate::color::Color;
use crate::cursor::CursorShape;
use crate::env::ContrastPref;
use crate::geom::{Edges, Size};
use crate::key::Key;
use crate::spec::{Align, Dir, LayoutSpec, Min, NodeSpec, Sizing, VisualStyle};
use crate::tree::{NIL, Tree};
use crate::value::Value;

/// The most entries a node's `when` list may hold: a match is a 64-bit
/// set. The rest are dropped, and `rule-prop-refused` says so.
pub const MAX_RULES: usize = 64;

/// One side of a threshold, or both, over a length: `below` holds for a
/// size under it, `above` for a size at or over it. `band` is the
/// hysteresis: a `below` that holds keeps holding up to `below + band`,
/// an `above` down to `above - band`, so a size resting on a threshold
/// does not flicker between two layouts.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Range {
    pub below: Option<f32>,
    pub above: Option<f32>,
    pub band: f32,
}

impl Range {
    /// Whether `v` is in range, given whether it was last time.
    #[inline]
    pub fn holds(&self, v: f32, was: bool) -> bool {
        let band = if was { self.band.max(0.0) } else { 0.0 };
        self.below.is_none_or(|b| v < b + band) && self.above.is_none_or(|a| v >= a - band)
    }

    fn merge(self, other: Range) -> Range {
        Range {
            below: other.below.or(self.below),
            above: other.above.or(self.above),
            band: if other.band != 0.0 {
                other.band
            } else {
                self.band
            },
        }
    }

    fn describe(&self, what: &str) -> String {
        let mut s = match (self.above, self.below) {
            (Some(a), Some(b)) => format!("{a} ≤ {what} < {b}"),
            (Some(a), None) => format!("{what} ≥ {a}"),
            (None, Some(b)) => format!("{what} < {b}"),
            (None, None) => what.to_string(),
        };
        if self.band != 0.0 {
            s.push_str(&format!(" ±{}", self.band));
        }
        s
    }
}

/// The platforms a condition may name, as bits of [`Cond::platforms`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Platform {
    MacOs,
    Windows,
    Linux,
}

impl Platform {
    /// Every platform, in the spelling's order.
    pub const ALL: [Platform; 3] = [Platform::MacOs, Platform::Windows, Platform::Linux];

    /// `"macos"`, `"windows"`, `"linux"`.
    pub fn name(self) -> &'static str {
        match self {
            Platform::MacOs => "macos",
            Platform::Windows => "windows",
            Platform::Linux => "linux",
        }
    }

    pub fn parse(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|p| p.name() == name)
    }

    fn bit(self) -> u8 {
        1 << (self as u8)
    }

    /// The platform this build runs on; Linux for every other Unix.
    pub fn current() -> Platform {
        if cfg!(target_os = "macos") {
            Platform::MacOs
        } else if cfg!(target_os = "windows") {
            Platform::Windows
        } else {
            Platform::Linux
        }
    }
}

/// The state bits a condition reads, in [`Cond::state_mask`].
pub mod state {
    pub const HOVERED: u16 = 1;
    pub const PRESSED: u16 = 2;
    pub const FOCUSED: u16 = 4;
    pub const FOCUS_VISIBLE: u16 = 8;
    pub const DISABLED: u16 = 16;
    pub const SELECTED: u16 = 32;
    /// The node's hover group's state (backlog C106): the pointer over a
    /// member, a press held on one, focus on one, and that focus shown —
    /// what a mark inside a row reads of the row, the two sharing a
    /// `hoverGroup`. None holds for a node in no group.
    pub const GROUP_HOVERED: u16 = 64;
    pub const GROUP_PRESSED: u16 = 128;
    pub const GROUP_FOCUSED: u16 = 256;
    pub const GROUP_FOCUS_VISIBLE: u16 = 512;
    /// The names a binding spells them by, in bit order.
    pub const NAMES: [(&str, u16); 10] = [
        ("hovered", HOVERED),
        ("pressed", PRESSED),
        ("focused", FOCUSED),
        ("focus_visible", FOCUS_VISIBLE),
        ("disabled", DISABLED),
        ("selected", SELECTED),
        ("group_hovered", GROUP_HOVERED),
        ("group_pressed", GROUP_PRESSED),
        ("group_focused", GROUP_FOCUSED),
        ("group_focus_visible", GROUP_FOCUS_VISIBLE),
    ];
    /// The bits read off the node's hover group rather than the node.
    pub const GROUP: u16 = GROUP_HOVERED | GROUP_PRESSED | GROUP_FOCUSED | GROUP_FOCUS_VISIBLE;
}

/// A condition set: every condition in it must hold. Built from the
/// constructors and constants and joined with [`Cond::and`]; the default
/// is the empty set, which always holds.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Cond {
    /// The node's own width and height.
    pub width: Option<Range>,
    pub height: Option<Range>,
    /// The window's content size.
    pub window_width: Option<Range>,
    pub window_height: Option<Range>,
    /// The palette's appearance: `Some(true)` holds while it is dark.
    pub dark: Option<bool>,
    /// `Some(true)` holds while the palette is light.
    pub light: Option<bool>,
    /// `Some(true)` holds while the user asked for reduced motion.
    pub reduced_motion: Option<bool>,
    /// Holds while the contrast setting is this one.
    pub contrast: Option<ContrastPref>,
    /// Holds on any platform whose bit is set; 0 is any platform.
    pub platforms: u8,
    /// The window's scale factor.
    pub scale: Option<Range>,
    /// The state bits this set reads, and what each must be.
    pub state_mask: u16,
    pub state_want: u16,
}

#[allow(non_upper_case_globals)]
impl Cond {
    const fn flag(state: u16) -> Cond {
        Cond {
            width: None,
            height: None,
            window_width: None,
            window_height: None,
            dark: None,
            light: None,
            reduced_motion: None,
            contrast: None,
            platforms: 0,
            scale: None,
            state_mask: state,
            state_want: state,
        }
    }

    /// The empty set: always holds.
    pub const Always: Cond = Cond::flag(0);
    /// The palette is dark.
    pub const Dark: Cond = Cond {
        dark: Some(true),
        ..Cond::flag(0)
    };
    /// The palette is light.
    pub const Light: Cond = Cond {
        light: Some(true),
        ..Cond::flag(0)
    };
    /// The user asked for reduced motion.
    pub const ReducedMotion: Cond = Cond {
        reduced_motion: Some(true),
        ..Cond::flag(0)
    };
    /// The user asked for more contrast.
    pub const MoreContrast: Cond = Cond {
        contrast: Some(ContrastPref::More),
        ..Cond::flag(0)
    };
    /// The pointer is over the node or anything inside it, or over a
    /// member of its hover group, or a press holds it — as `hoverBg`
    /// reads it.
    pub const Hovered: Cond = Cond::flag(state::HOVERED);
    /// A press began on the node (or its group) and holds.
    pub const Pressed: Cond = Cond::flag(state::PRESSED);
    /// The node holds keyboard focus.
    pub const Focused: Cond = Cond::flag(state::FOCUSED);
    /// The node holds keyboard focus that shows (moved by the keyboard
    /// or assistive technology).
    pub const FocusVisible: Cond = Cond::flag(state::FOCUS_VISIBLE);
    /// The node is `disabled`.
    pub const Disabled: Cond = Cond::flag(state::DISABLED);
    /// The node's access spec says `selected`.
    pub const Selected: Cond = Cond::flag(state::SELECTED);
    /// The pointer is over a member of the node's hover group (backlog
    /// C106), or a press holds one.
    pub const GroupHovered: Cond = Cond::flag(state::GROUP_HOVERED);
    /// A press began on a member of the node's hover group and holds.
    pub const GroupPressed: Cond = Cond::flag(state::GROUP_PRESSED);
    /// A member of the node's hover group holds keyboard focus.
    pub const GroupFocused: Cond = Cond::flag(state::GROUP_FOCUSED);
    /// A member of the node's hover group holds keyboard focus that
    /// shows: a checkbox's mark drawing its tight ring while its row has
    /// the keyboard.
    pub const GroupFocusVisible: Cond = Cond::flag(state::GROUP_FOCUS_VISIBLE);

    /// The node's own width under `n`.
    pub fn width_below(n: f32) -> Cond {
        Cond {
            width: Some(Range {
                below: Some(n),
                ..Range::default()
            }),
            ..Cond::default()
        }
    }

    /// The node's own width at or over `n`.
    pub fn width_above(n: f32) -> Cond {
        Cond {
            width: Some(Range {
                above: Some(n),
                ..Range::default()
            }),
            ..Cond::default()
        }
    }

    /// The node's own height under `n`.
    pub fn height_below(n: f32) -> Cond {
        Cond {
            height: Some(Range {
                below: Some(n),
                ..Range::default()
            }),
            ..Cond::default()
        }
    }

    /// The node's own height at or over `n`.
    pub fn height_above(n: f32) -> Cond {
        Cond {
            height: Some(Range {
                above: Some(n),
                ..Range::default()
            }),
            ..Cond::default()
        }
    }

    /// The window's content width under `n`.
    pub fn window_width_below(n: f32) -> Cond {
        Cond {
            window_width: Some(Range {
                below: Some(n),
                ..Range::default()
            }),
            ..Cond::default()
        }
    }

    /// The window's content width at or over `n`.
    pub fn window_width_above(n: f32) -> Cond {
        Cond {
            window_width: Some(Range {
                above: Some(n),
                ..Range::default()
            }),
            ..Cond::default()
        }
    }

    /// The window's content height under `n`.
    pub fn window_height_below(n: f32) -> Cond {
        Cond {
            window_height: Some(Range {
                below: Some(n),
                ..Range::default()
            }),
            ..Cond::default()
        }
    }

    /// The window's content height at or over `n`.
    pub fn window_height_above(n: f32) -> Cond {
        Cond {
            window_height: Some(Range {
                above: Some(n),
                ..Range::default()
            }),
            ..Cond::default()
        }
    }

    /// The window's scale factor at or over `n`.
    pub fn scale_above(n: f32) -> Cond {
        Cond {
            scale: Some(Range {
                above: Some(n),
                ..Range::default()
            }),
            ..Cond::default()
        }
    }

    /// The window's scale factor under `n`.
    pub fn scale_below(n: f32) -> Cond {
        Cond {
            scale: Some(Range {
                below: Some(n),
                ..Range::default()
            }),
            ..Cond::default()
        }
    }

    /// The contrast setting is `c`.
    pub fn contrast(c: ContrastPref) -> Cond {
        Cond {
            contrast: Some(c),
            ..Cond::default()
        }
    }

    /// Running on one of `platforms`.
    pub fn platforms(platforms: &[Platform]) -> Cond {
        Cond {
            platforms: platforms.iter().fold(0, |m, p| m | p.bit()),
            ..Cond::default()
        }
    }

    /// The band (see [`Range::band`]) on every size range in this set.
    pub fn band(mut self, band: f32) -> Cond {
        for r in [
            &mut self.width,
            &mut self.height,
            &mut self.window_width,
            &mut self.window_height,
        ]
        .into_iter()
        .flatten()
        {
            r.band = band;
        }
        self
    }

    /// The state conditions of this set wanted `want` instead:
    /// `Cond::Hovered.is(false)` holds while the node is not hovered.
    pub fn is(mut self, want: bool) -> Cond {
        self.state_want = if want { self.state_mask } else { 0 };
        self
    }

    /// Both sets: every condition of each must hold. Two ranges on one
    /// size join (`width_above(300).and(width_below(700))`); for any
    /// other condition both name, `other`'s wins.
    pub fn and(mut self, other: Cond) -> Cond {
        let join = |a: Option<Range>, b: Option<Range>| match (a, b) {
            (Some(a), Some(b)) => Some(a.merge(b)),
            (a, b) => b.or(a),
        };
        self.width = join(self.width, other.width);
        self.height = join(self.height, other.height);
        self.window_width = join(self.window_width, other.window_width);
        self.window_height = join(self.window_height, other.window_height);
        self.scale = join(self.scale, other.scale);
        self.dark = other.dark.or(self.dark);
        self.light = other.light.or(self.light);
        self.reduced_motion = other.reduced_motion.or(self.reduced_motion);
        self.contrast = other.contrast.or(self.contrast);
        if other.platforms != 0 {
            self.platforms = other.platforms;
        }
        self.state_want = (self.state_want & !other.state_mask) | other.state_want;
        self.state_mask |= other.state_mask;
        self
    }

    /// Whether this set reads the node's own size.
    pub fn reads_size(&self) -> bool {
        self.width.is_some() || self.height.is_some()
    }

    /// Whether this set reads the node's interaction state.
    pub fn reads_state(&self) -> bool {
        self.state_mask != 0
    }

    /// The conditions as a person reads them, for the devtools:
    /// `width < 700 ±16 · dark · hovered`.
    pub fn describe(&self) -> String {
        let mut out: Vec<String> = Vec::new();
        if let Some(r) = self.width {
            out.push(r.describe("width"));
        }
        if let Some(r) = self.height {
            out.push(r.describe("height"));
        }
        if let Some(r) = self.window_width {
            out.push(r.describe("window width"));
        }
        if let Some(r) = self.window_height {
            out.push(r.describe("window height"));
        }
        if let Some(r) = self.scale {
            out.push(r.describe("scale"));
        }
        let yes = |b: bool, s: &str| if b { s.to_string() } else { format!("not {s}") };
        if let Some(b) = self.dark {
            out.push(yes(b, "dark"));
        }
        if let Some(b) = self.light {
            out.push(yes(b, "light"));
        }
        if let Some(b) = self.reduced_motion {
            out.push(yes(b, "reduced motion"));
        }
        if let Some(c) = self.contrast {
            out.push(format!("contrast {}", c.name()));
        }
        if self.platforms != 0 {
            let names: Vec<&str> = Platform::ALL
                .iter()
                .filter(|p| self.platforms & p.bit() != 0)
                .map(|p| p.name())
                .collect();
            out.push(names.join("|"));
        }
        for (name, bit) in state::NAMES {
            if self.state_mask & bit != 0 {
                out.push(yes(self.state_want & bit != 0, name));
            }
        }
        if out.is_empty() {
            "always".into()
        } else {
            out.join(" · ")
        }
    }

    /// Whether the conditions that are not the node's own size hold,
    /// against `facts`; `held` is what each window range answered last
    /// time (bit 0 the width, bit 1 the height), updated in place.
    pub(crate) fn holds_static(&self, facts: &Facts, held: &mut u8) -> bool {
        let mut ok = true;
        if let Some(r) = self.window_width {
            let h = r.holds(facts.viewport.w, *held & 1 != 0);
            *held = (*held & !1) | h as u8;
            ok &= h;
        }
        if let Some(r) = self.window_height {
            let h = r.holds(facts.viewport.h, *held & 2 != 0);
            *held = (*held & !2) | ((h as u8) << 1);
            ok &= h;
        }
        ok && self.dark.is_none_or(|b| b == facts.dark)
            && self.light.is_none_or(|b| b == !facts.dark)
            && self
                .reduced_motion
                .is_none_or(|b| b == facts.reduced_motion)
            && self.contrast.is_none_or(|c| c == facts.contrast)
            && (self.platforms == 0 || self.platforms & facts.platform.bit() != 0)
            && self.scale.is_none_or(|r| r.holds(facts.scale, false))
            && (facts.state & self.state_mask) == self.state_want
    }
}

/// What a frame knows that a condition other than the node's own size
/// reads: the window's content size and scale, the palette, the system's
/// settings and the node's state.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Facts {
    pub viewport: Size,
    pub scale: f32,
    pub dark: bool,
    pub reduced_motion: bool,
    pub contrast: ContrastPref,
    pub platform: Platform,
    pub state: u16,
}

/// The props an entry may set, as bits of [`Patch`]'s set.
pub mod field {
    pub const WIDTH: u32 = 1 << 0;
    pub const HEIGHT: u32 = 1 << 1;
    pub const MIN_W: u32 = 1 << 2;
    pub const MAX_W: u32 = 1 << 3;
    pub const MIN_H: u32 = 1 << 4;
    pub const MAX_H: u32 = 1 << 5;
    pub const DIR: u32 = 1 << 6;
    pub const PADDING: u32 = 1 << 7;
    pub const GAP: u32 = 1 << 8;
    pub const WRAP: u32 = 1 << 9;
    pub const CROSS_GAP: u32 = 1 << 10;
    pub const MAIN_ALIGN: u32 = 1 << 11;
    pub const CROSS_ALIGN: u32 = 1 << 12;
    pub const ASPECT: u32 = 1 << 13;
    pub const CLIP: u32 = 1 << 14;
    pub const SCROLL_X: u32 = 1 << 15;
    pub const SCROLL_Y: u32 = 1 << 16;
    pub const BG: u32 = 1 << 17;
    pub const BORDER_COLOR: u32 = 1 << 18;
    pub const BORDER_W: u32 = 1 << 19;
    pub const RADIUS: u32 = 1 << 20;
    pub const OPACITY: u32 = 1 << 21;
    pub const SHADOW: u32 = 1 << 22;
    pub const PIXEL_SNAP: u32 = 1 << 23;
    pub const CURSOR: u32 = 1 << 24;
    pub const TRANSITION: u32 = 1 << 25;
    pub const SLIDE: u32 = 1 << 26;
    pub const HIDDEN: u32 = 1 << 27;
    pub const MIN_COLUMN: u32 = 1 << 28;
    pub const HIDE: u32 = 1 << 29;
    /// The move off the node's laid place, each lane its own (backlog
    /// C105): an entry that sets `dy` keeps the node's `dx`.
    pub const DX: u32 = 1 << 30;
    pub const DY: u32 = 1 << 31;

    /// What an entry over the node's own size may set (ADR 0069,
    /// decision 7): the layout of its children, never its own size.
    pub const LAYOUT_OF_CHILDREN: u32 =
        DIR | PADDING | GAP | WRAP | CROSS_GAP | MAIN_ALIGN | CROSS_ALIGN | MIN_COLUMN | HIDE;
    /// The layout props among all of them: an entry that sets one of
    /// these lays the frame out with it.
    pub const LAYOUT: u32 = WIDTH
        | HEIGHT
        | MIN_W
        | MAX_W
        | MIN_H
        | MAX_H
        | ASPECT
        | CLIP
        | SCROLL_X
        | SCROLL_Y
        | HIDDEN
        | LAYOUT_OF_CHILDREN;

    /// Each field's name, for the devtools and the warnings.
    pub const NAMES: [(u32, &str); 32] = [
        (WIDTH, "width"),
        (HEIGHT, "height"),
        (MIN_W, "minWidth"),
        (MAX_W, "maxWidth"),
        (MIN_H, "minHeight"),
        (MAX_H, "maxHeight"),
        (DIR, "dir"),
        (PADDING, "pad"),
        (GAP, "gap"),
        (WRAP, "wrapChildren"),
        (CROSS_GAP, "crossGap"),
        (MAIN_ALIGN, "mainAlign"),
        (CROSS_ALIGN, "crossAlign"),
        (ASPECT, "aspectRatio"),
        (CLIP, "clip"),
        (SCROLL_X, "scrollX"),
        (SCROLL_Y, "scrollY"),
        (BG, "bg"),
        (BORDER_COLOR, "borderColor"),
        (BORDER_W, "borderW"),
        (RADIUS, "radius"),
        (OPACITY, "opacity"),
        (SHADOW, "shadow"),
        (PIXEL_SNAP, "pixelSnap"),
        (CURSOR, "cursor"),
        (TRANSITION, "transition"),
        (SLIDE, "slide"),
        (HIDDEN, "hidden"),
        (MIN_COLUMN, "minColumn"),
        (HIDE, "hidden (children)"),
        (DX, "dx"),
        (DY, "dy"),
    ];
}

/// The props one entry sets: which ones, and their values. Read off what
/// a builder did to a spec rather than declared field by field, so the
/// Rust closure, a Lua table and a JSX object all land through the same
/// `NodeSpec` builders (see [`Patch::capture`]).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Patch {
    /// The [`field`] bits set.
    pub set: u32,
    layout: LayoutSpec,
    style: VisualStyle,
    cursor: Option<CursorShape>,
    transition: Option<crate::anim::Transition>,
    slide: bool,
    hidden: bool,
    min_column: f32,
    /// The move off the node's laid place (`dx`, `dy`; backlog C105).
    offset: crate::geom::Vec2,
    /// Which of the shadow's lanes the entry sets, as `shadow_lane` bits
    /// under `field::SHADOW` (backlog C114): an entry that sets `shadowY`
    /// keeps the node's colour, blur and the rest.
    shadow_lanes: u8,
    /// The key labels of the children an own-size entry hides.
    hide: Vec<String>,
}

/// The lanes of a shadow, as bits of [`Patch`]'s `shadow_lanes`.
mod shadow_lane {
    pub const COLOR: u8 = 1 << 0;
    pub const DX: u8 = 1 << 1;
    pub const DY: u8 = 1 << 2;
    pub const BLUR: u8 = 1 << 3;
    pub const SPREAD: u8 = 1 << 4;
}

/// The lanes of `a` that differ from `b`'s.
fn shadow_lanes(a: &crate::spec::Shadow, b: &crate::spec::Shadow) -> u8 {
    use shadow_lane::*;
    [
        (a.color != b.color, COLOR),
        (a.dx != b.dx, DX),
        (a.dy != b.dy, DY),
        (a.blur != b.blur, BLUR),
        (a.spread != b.spread, SPREAD),
    ]
    .iter()
    .filter(|(moved, _)| *moved)
    .fold(0, |m, (_, bit)| m | bit)
}

/// The lanes `lanes` names copied from `src` onto `dst`.
fn put_shadow(dst: &mut crate::spec::Shadow, src: &crate::spec::Shadow, lanes: u8) {
    use shadow_lane::*;
    if lanes & COLOR != 0 {
        dst.color = src.color;
    }
    if lanes & DX != 0 {
        dst.dx = src.dx;
    }
    if lanes & DY != 0 {
        dst.dy = src.dy;
    }
    if lanes & BLUR != 0 {
        dst.blur = src.blur;
    }
    if lanes & SPREAD != 0 {
        dst.spread = src.spread;
    }
}

/// Two specs as unlike every default as a spec can be: a builder that
/// sets a field changes it in at least one of the two, whatever value it
/// sets.
fn sentinel() -> NodeSpec {
    const S: f32 = 7777.25;
    let mut spec = NodeSpec::default();
    let l = &mut spec.layout;
    l.width = Sizing::Fixed(S);
    l.height = Sizing::Fixed(S);
    l.min_w = Min::px(S);
    l.max_w = S;
    l.min_h = Min::px(S);
    l.max_h = S;
    l.dir = Dir::Row;
    l.padding = Edges::all(S);
    l.gap = S;
    l.wrap = true;
    l.cross_gap = S;
    l.main_align = Align::SpaceEvenly;
    l.cross_align = Align::Baseline;
    l.aspect = S;
    l.clip = true;
    l.scroll_x = true;
    l.scroll_y = true;
    let st = &mut spec.style;
    st.bg = Color::hex(0x12345677);
    st.border_color = Color::hex(0x12345677);
    st.border_w = S;
    st.radius = [S; 4];
    st.opacity = 0.123;
    st.shadow = crate::spec::Shadow {
        color: Color::hex(0x12345677),
        dx: S,
        dy: S,
        blur: S,
        spread: S,
    };
    st.pixel_snap = true;
    spec.cursor = Some(CursorShape::NotAllowed);
    spec.transition = Some(crate::anim::Transition::ms(S));
    spec.slide = true;
    spec.hidden = true;
    spec
}

/// The move a spec declares off its laid place (`dx`, `dy`).
fn offset_of(s: &NodeSpec) -> crate::geom::Vec2 {
    s.transform_spec()
        .map_or(crate::geom::Vec2::new(0.0, 0.0), |t| t.translate)
}

/// Whether two values differ. `==` is enough because each field is read
/// against two unlike bases: a value equal to one base's (a `0.0` gap on
/// the default spec, a `-0.0` floor beside a `0.0`) differs from the
/// other's.
#[inline]
fn differ<T: PartialEq>(a: &T, b: &T) -> bool {
    a != b
}

/// Whether a builder moved the interaction group of `out` from `base`'s,
/// its rules aside: `hide` and `min_column` write those, and they live in
/// that box.
fn interact_moved(out: &NodeSpec, base: &NodeSpec) -> bool {
    // And the move, which `dx` / `dy` write into the transform there
    // (backlog C105): a transform holding nothing else is no transform.
    let strip = |s: &NodeSpec| {
        let mut i = s.interact().clone();
        i.when = None;
        if let Some(t) = i.transform.as_deref_mut() {
            t.translate = crate::geom::Vec2::new(0.0, 0.0);
            if *t == crate::spec::TransformSpec::NONE {
                i.transform = None;
            }
        }
        i
    };
    match (&out.interact, &base.interact) {
        (None, None) => false,
        _ => strip(out) != strip(base),
    }
}

impl Patch {
    /// The props `f` sets, read off what it does to two unlike specs; and
    /// the name of what it set that no entry may — a handler, an access
    /// prop, a key — if it set any.
    pub fn capture(f: &dyn Fn(NodeSpec) -> NodeSpec) -> (Patch, Option<&'static str>) {
        let base_a = NodeSpec::default();
        static SENTINEL: std::sync::LazyLock<NodeSpec> = std::sync::LazyLock::new(sentinel);
        let base_b = SENTINEL.clone();
        let out_a = f(base_a.clone());
        let out_b = f(base_b.clone());
        let mut patch = Patch {
            layout: out_a.layout,
            style: out_a.style,
            cursor: out_a.cursor,
            transition: out_a.transition,
            slide: out_a.slide,
            hidden: out_a.hidden,
            min_column: out_a.rules_spec().min_column,
            offset: offset_of(&out_a),
            ..Patch::default()
        };
        let mut set = 0u32;
        let mut mark = |bit: u32, a: bool, b: bool| {
            if a || b {
                set |= bit;
            }
        };
        macro_rules! cmp {
            ($bit:expr, $($path:tt)+) => {
                mark(
                    $bit,
                    differ(&out_a.$($path)+, &base_a.$($path)+),
                    differ(&out_b.$($path)+, &base_b.$($path)+),
                )
            };
        }
        cmp!(field::WIDTH, layout.width);
        cmp!(field::HEIGHT, layout.height);
        cmp!(field::MIN_W, layout.min_w);
        cmp!(field::MAX_W, layout.max_w);
        cmp!(field::MIN_H, layout.min_h);
        cmp!(field::MAX_H, layout.max_h);
        cmp!(field::DIR, layout.dir);
        cmp!(field::PADDING, layout.padding);
        cmp!(field::GAP, layout.gap);
        cmp!(field::WRAP, layout.wrap);
        cmp!(field::CROSS_GAP, layout.cross_gap);
        cmp!(field::MAIN_ALIGN, layout.main_align);
        cmp!(field::CROSS_ALIGN, layout.cross_align);
        cmp!(field::ASPECT, layout.aspect);
        cmp!(field::CLIP, layout.clip);
        cmp!(field::SCROLL_X, layout.scroll_x);
        cmp!(field::SCROLL_Y, layout.scroll_y);
        cmp!(field::BG, style.bg);
        cmp!(field::BORDER_COLOR, style.border_color);
        cmp!(field::BORDER_W, style.border_w);
        cmp!(field::RADIUS, style.radius);
        cmp!(field::OPACITY, style.opacity);
        cmp!(field::SHADOW, style.shadow);
        // Lane by lane (backlog C114): a lane is set when either base saw
        // it move, and only those lanes are written.
        patch.shadow_lanes = shadow_lanes(&out_a.style.shadow, &base_a.style.shadow)
            | shadow_lanes(&out_b.style.shadow, &base_b.style.shadow);
        cmp!(field::PIXEL_SNAP, style.pixel_snap);
        cmp!(field::CURSOR, cursor);
        cmp!(field::TRANSITION, transition);
        cmp!(field::SLIDE, slide);
        cmp!(field::HIDDEN, hidden);
        // The move (backlog C105). Not in the sentinel: a move there gave
        // it an interaction box, which every capture then cloned and
        // compared, and `frame_1k_rules` read +70%. Both bases sit at 0, so
        // a lane set to 0 reads as unset: a builder that touched the
        // transform without moving a lane off 0 is asked once more, from a
        // base moved both ways.
        let (oa, ob) = (offset_of(&out_a), offset_of(&out_b));
        let mut dx = oa.x != 0.0 || ob.x != 0.0;
        let mut dy = oa.y != 0.0 || ob.y != 0.0;
        let touched = out_a.transform_spec().is_some() || out_b.transform_spec().is_some();
        if touched && !(dx && dy) {
            const S: f32 = 7777.25;
            let oc = offset_of(&f(NodeSpec::default().dx(S).dy(S)));
            dx |= oc.x != S;
            dy |= oc.y != S;
        }
        mark(field::DX, dx, false);
        mark(field::DY, dy, false);
        let (ra, rb) = (out_a.rules_spec(), out_b.rules_spec());
        mark(
            field::MIN_COLUMN,
            differ(&ra.min_column, &base_a.rules_spec().min_column),
            differ(&rb.min_column, &base_b.rules_spec().min_column),
        );
        if !ra.hide.is_empty() {
            set |= field::HIDE;
            patch.hide = ra.hide.clone();
        }
        patch.set = set;
        // Anything else that moved is a prop no entry sets.
        let other = [
            (
                differ(&out_a.events, &base_a.events) || differ(&out_b.events, &base_b.events),
                "a handler",
            ),
            (
                differ(&out_a.anim, &base_a.anim) || differ(&out_b.anim, &base_b.anim),
                "an animation prop",
            ),
            (
                differ(&out_a.access, &base_a.access) || differ(&out_b.access, &base_b.access),
                "an access prop",
            ),
            (
                interact_moved(&out_a, &base_a) || interact_moved(&out_b, &base_b),
                "an interaction prop",
            ),
            (
                !ra.entries.is_empty() || ra.lazy.is_some(),
                "a nested `when`",
            ),
            (
                out_a.layout.float != base_a.layout.float
                    || out_a.layout.table != base_a.layout.table
                    || out_a.layout.anchor != base_a.layout.anchor
                    || out_b.layout.float != base_b.layout.float
                    || out_b.layout.anchor != base_b.layout.anchor,
                "a float, table or anchor prop",
            ),
            (
                out_a.hoverable != base_a.hoverable
                    || out_a.animate != base_a.animate
                    || out_a.accent != base_a.accent
                    || out_a.window != base_a.window
                    || out_a.over_siblings != base_a.over_siblings
                    || out_a.floats_in_order != base_a.floats_in_order
                    || out_a.focusable != base_a.focusable
                    || out_a.initial_focus != base_a.initial_focus
                    || out_a.disabled != base_a.disabled,
                "a behaviour prop",
            ),
        ];
        let refused = other.iter().find(|(moved, _)| *moved).map(|(_, n)| *n);
        (patch, refused)
    }

    /// `other`'s props over this one's.
    pub fn merge(&mut self, other: &Patch) {
        let s = other.set;
        let (l, o) = (&mut self.layout, &other.layout);
        macro_rules! take {
            ($bit:expr, $dst:expr, $src:expr) => {
                if s & $bit != 0 {
                    $dst = $src;
                }
            };
        }
        take!(field::WIDTH, l.width, o.width);
        take!(field::HEIGHT, l.height, o.height);
        take!(field::MIN_W, l.min_w, o.min_w);
        take!(field::MAX_W, l.max_w, o.max_w);
        take!(field::MIN_H, l.min_h, o.min_h);
        take!(field::MAX_H, l.max_h, o.max_h);
        take!(field::DIR, l.dir, o.dir);
        take!(field::PADDING, l.padding, o.padding);
        take!(field::GAP, l.gap, o.gap);
        take!(field::WRAP, l.wrap, o.wrap);
        take!(field::CROSS_GAP, l.cross_gap, o.cross_gap);
        take!(field::MAIN_ALIGN, l.main_align, o.main_align);
        take!(field::CROSS_ALIGN, l.cross_align, o.cross_align);
        take!(field::ASPECT, l.aspect, o.aspect);
        take!(field::CLIP, l.clip, o.clip);
        take!(field::SCROLL_X, l.scroll_x, o.scroll_x);
        take!(field::SCROLL_Y, l.scroll_y, o.scroll_y);
        let (st, os) = (&mut self.style, &other.style);
        take!(field::BG, st.bg, os.bg);
        take!(field::BORDER_COLOR, st.border_color, os.border_color);
        take!(field::BORDER_W, st.border_w, os.border_w);
        take!(field::RADIUS, st.radius, os.radius);
        take!(field::OPACITY, st.opacity, os.opacity);
        if s & field::SHADOW != 0 {
            put_shadow(&mut st.shadow, &os.shadow, other.shadow_lanes);
            self.shadow_lanes |= other.shadow_lanes;
        }
        take!(field::PIXEL_SNAP, st.pixel_snap, os.pixel_snap);
        take!(field::CURSOR, self.cursor, other.cursor);
        take!(field::TRANSITION, self.transition, other.transition);
        take!(field::SLIDE, self.slide, other.slide);
        take!(field::HIDDEN, self.hidden, other.hidden);
        take!(field::MIN_COLUMN, self.min_column, other.min_column);
        take!(field::DX, self.offset.x, other.offset.x);
        take!(field::DY, self.offset.y, other.offset.y);
        if s & field::HIDE != 0 {
            self.hide.extend(other.hide.iter().cloned());
        }
        self.set |= s;
    }

    /// Writes the props among `mask` this entry sets onto `spec`.
    pub fn apply(&self, spec: &mut NodeSpec, mask: u32) {
        let s = self.set & mask;
        if s == 0 {
            return;
        }
        let (l, o) = (&mut spec.layout, &self.layout);
        macro_rules! put {
            ($bit:expr, $dst:expr, $src:expr) => {
                if s & $bit != 0 {
                    $dst = $src;
                }
            };
        }
        put!(field::WIDTH, l.width, o.width);
        put!(field::HEIGHT, l.height, o.height);
        put!(field::MIN_W, l.min_w, o.min_w);
        put!(field::MAX_W, l.max_w, o.max_w);
        put!(field::MIN_H, l.min_h, o.min_h);
        put!(field::MAX_H, l.max_h, o.max_h);
        put!(field::DIR, l.dir, o.dir);
        put!(field::PADDING, l.padding, o.padding);
        put!(field::GAP, l.gap, o.gap);
        put!(field::WRAP, l.wrap, o.wrap);
        put!(field::CROSS_GAP, l.cross_gap, o.cross_gap);
        put!(field::MAIN_ALIGN, l.main_align, o.main_align);
        put!(field::CROSS_ALIGN, l.cross_align, o.cross_align);
        put!(field::ASPECT, l.aspect, o.aspect);
        put!(field::CLIP, l.clip, o.clip);
        put!(field::SCROLL_X, l.scroll_x, o.scroll_x);
        put!(field::SCROLL_Y, l.scroll_y, o.scroll_y);
        let (st, os) = (&mut spec.style, &self.style);
        put!(field::BG, st.bg, os.bg);
        put!(field::BORDER_COLOR, st.border_color, os.border_color);
        put!(field::BORDER_W, st.border_w, os.border_w);
        put!(field::RADIUS, st.radius, os.radius);
        put!(field::OPACITY, st.opacity, os.opacity);
        if s & field::SHADOW != 0 {
            put_shadow(&mut st.shadow, &os.shadow, self.shadow_lanes);
        }
        put!(field::PIXEL_SNAP, st.pixel_snap, os.pixel_snap);
        put!(field::CURSOR, spec.cursor, self.cursor);
        put!(field::TRANSITION, spec.transition, self.transition);
        put!(field::SLIDE, spec.slide, self.slide);
        put!(field::HIDDEN, spec.hidden, self.hidden);
        if s & field::DX != 0 {
            spec.transform_mut().translate.x = self.offset.x;
        }
        if s & field::DY != 0 {
            spec.transform_mut().translate.y = self.offset.y;
        }
        if s & field::MIN_COLUMN != 0
            && let Some(r) = spec.rules_ref_mut()
        {
            r.min_column = self.min_column;
        }
    }

    /// The `min_column` this entry sets, if it sets one.
    pub fn min_column(&self) -> Option<f32> {
        (self.set & field::MIN_COLUMN != 0).then_some(self.min_column)
    }

    /// The key labels of the children this entry hides.
    pub fn hides(&self) -> &[String] {
        &self.hide
    }

    /// The names of the props this entry sets, for the devtools.
    pub fn names(&self) -> Vec<&'static str> {
        field::NAMES
            .iter()
            .filter(|(bit, _)| self.set & bit != 0)
            .map(|(_, n)| *n)
            .collect()
    }
}

/// One entry of a node's `when` list: a condition set and the props it
/// sets while every condition holds.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Rule {
    pub cond: Cond,
    pub patch: Patch,
    /// The props the entry named that it may not set, each with why; the
    /// rest of the entry applies, and the core warns once per node
    /// (`diag::RULE_PROP_REFUSED`).
    pub refused: Vec<String>,
}

impl Rule {
    /// The entry `cond` with the props `f` sets, those it may not set
    /// left out and named in [`Rule::refused`].
    pub fn new(cond: Cond, f: &dyn Fn(NodeSpec) -> NodeSpec) -> Rule {
        let (patch, other) = Patch::capture(f);
        let mut rule = Rule {
            cond,
            patch: Patch::default(),
            refused: Vec::new(),
        };
        if let Some(what) = other {
            rule.refused.push(format!("{what} (no entry sets one)"));
        }
        rule.add(patch);
        rule
    }

    /// Adds the props of `patch`, refusing what the entry's conditions
    /// say it may not set.
    pub fn add(&mut self, mut patch: Patch) {
        let allowed = self.allowed();
        let out = patch.set & !allowed;
        if out != 0 {
            for (bit, name) in field::NAMES {
                if out & bit != 0 {
                    self.refused.push(format!(
                        "`{name}` (an entry over the node's own size sets the layout of its \
                         children only)"
                    ));
                }
            }
            patch.set &= allowed;
        }
        if patch.set & field::HIDE == 0 {
            patch.hide.clear();
        }
        if self.patch.set == 0 {
            self.patch = patch;
        } else {
            self.patch.merge(&patch);
        }
    }

    /// The fields this entry may set: the layout of its children for one
    /// over the node's own size (decision 7), every field otherwise —
    /// but `hidden` children, which name what a size entry gives way to.
    pub fn allowed(&self) -> u32 {
        if self.cond.reads_size() {
            field::LAYOUT_OF_CHILDREN
        } else {
            !field::HIDE
        }
    }

    /// The entry as a person reads it: `width < 700 → dir, gap`.
    pub fn describe(&self) -> String {
        let props = self.patch.names().join(", ");
        format!("{} → {}", self.cond.describe(), props)
    }
}

/// A lazy branch's ask (ADR 0069, decisions 13–16): the room it needs in
/// each axis it names, or the condition its parent's size must meet.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct LazySpec {
    /// The width and the height the branch needs, each `None` when it
    /// does not name that axis.
    pub needs: Option<(Option<f32>, Option<f32>)>,
    /// The condition set over the parent's size (`width`, `height`), the
    /// window's and the environment's. State conditions are refused
    /// (`diag::QUERY_STATE`).
    pub query: Option<Cond>,
    /// Whether the branch built is `render` (`Some(true)`), `fallback`
    /// (`Some(false)`), or neither yet: the first frame, before any
    /// layout has said how much room there is.
    pub built: Option<bool>,
    /// The message `onQuery` arrives as when the decision changes.
    pub on_query: Option<Value>,
}

/// A lazy branch's room: what its parent offers it, in each axis.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Needs {
    pub w: Option<f32>,
    pub h: Option<f32>,
}

impl Needs {
    /// Needs `w` px of width.
    pub fn w(w: f32) -> Needs {
        Needs {
            w: Some(w),
            h: None,
        }
    }

    /// Needs `h` px of height.
    pub fn h(h: f32) -> Needs {
        Needs {
            w: None,
            h: Some(h),
        }
    }

    /// Needs `w` × `h`.
    pub fn wh(w: f32, h: f32) -> Needs {
        Needs {
            w: Some(w),
            h: Some(h),
        }
    }
}

impl LazySpec {
    /// A branch built when its parent offers it `needs`.
    pub fn needs(needs: Needs) -> LazySpec {
        LazySpec {
            needs: Some((needs.w, needs.h)),
            ..LazySpec::default()
        }
    }

    /// A branch built while `query` holds over its parent's size, the
    /// window's or the environment's.
    pub fn query(query: Cond) -> LazySpec {
        LazySpec {
            query: Some(query),
            ..LazySpec::default()
        }
    }

    /// The message `onQuery` arrives as when the decision changes.
    pub fn on_query(mut self, tag: impl Into<Value>) -> LazySpec {
        self.on_query = Some(tag.into());
        self
    }

    /// Whether it reads the parent's width (true) and height.
    pub(crate) fn axes(&self) -> (bool, bool) {
        match (&self.needs, &self.query) {
            (Some((w, h)), _) => (w.is_some(), h.is_some()),
            (None, Some(q)) => (q.width.is_some(), q.height.is_some()),
            (None, None) => (false, false),
        }
    }

    /// Whether the branch should be built against `room` (the parent's
    /// room for a `needs` branch, its size for a `query`), `facts` and
    /// `held` (the query's ranges last time: width, height, window width,
    /// window height), updated in place.
    pub(crate) fn decide(&self, room: Size, parent: Size, facts: &Facts, held: &mut u8) -> bool {
        if let Some((w, h)) = self.needs {
            return w.is_none_or(|w| room.w >= w) && h.is_none_or(|h| room.h >= h);
        }
        let Some(q) = &self.query else {
            return true;
        };
        let mut win = (*held >> 2) & 3;
        let mut ok = q.holds_static(&Facts { state: 0, ..*facts }, &mut win);
        let mut size = *held & 3;
        if let Some(r) = q.width {
            let h = r.holds(parent.w, size & 1 != 0);
            size = (size & !1) | h as u8;
            ok &= h;
        }
        if let Some(r) = q.height {
            let h = r.holds(parent.h, size & 2 != 0);
            size = (size & !2) | ((h as u8) << 1);
            ok &= h;
        }
        *held = size | (win << 2);
        ok
    }
}

/// The layout props an entry over the node's own size may set, as the
/// node declared them before any entry: what layout puts back before it
/// lays the matched entries over them again.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct Base {
    dir: Dir,
    padding: Edges,
    gap: f32,
    wrap: bool,
    cross_gap: f32,
    main_align: Align,
    cross_align: Align,
    min_column: f32,
}

impl Base {
    pub(crate) fn of(spec: &NodeSpec) -> Base {
        let l = &spec.layout;
        Base {
            dir: l.dir,
            padding: l.padding,
            gap: l.gap,
            wrap: l.wrap,
            cross_gap: l.cross_gap,
            main_align: l.main_align,
            cross_align: l.cross_align,
            min_column: spec.rules_spec().min_column,
        }
    }

    /// [`Base::of`] for a spec whose rules are held apart.
    pub(crate) fn of_parts(spec: &NodeSpec, rules: &RulesSpec) -> Base {
        Base {
            min_column: rules.min_column,
            ..Base::of(spec)
        }
    }

    /// The `min_column` as declared.
    pub(crate) fn min_column(&self) -> f32 {
        self.min_column
    }

    fn put(&self, spec: &mut NodeSpec) {
        let l = &mut spec.layout;
        l.dir = self.dir;
        l.padding = self.padding;
        l.gap = self.gap;
        l.wrap = self.wrap;
        l.cross_gap = self.cross_gap;
        l.main_align = self.main_align;
        l.cross_align = self.cross_align;
        if let Some(r) = spec.rules_ref_mut() {
            r.min_column = self.min_column;
        }
    }
}

/// What the core resolved for a node's entries this frame, kept in the
/// tree's copy of its rules: never declared, and not compared.
#[derive(Clone, Debug, Default)]
pub(crate) struct Live {
    /// The entries whose conditions other than the node's own size hold.
    pub static_ok: u64,
    /// What each entry's window width and height range answered.
    pub win_w: u64,
    pub win_h: u64,
    /// What each entry's own-size width and height range answered last.
    pub held_w: u64,
    pub held_h: u64,
    /// The entries applied.
    pub matched: u64,
    /// The size-entry props as declared.
    pub base: Base,
    /// The children the entries hide, by entry and key.
    pub hide: Vec<(u8, Key)>,
    /// Whether layout saw a height match change this frame.
    pub height_moved: bool,
}

/// A node's rules: its `when` list, `min_column`, and — for a lazy
/// branch's node — its ask. Boxed inside the interaction group
/// (`InteractSpec::when`), so a node with none pays nothing.
#[derive(Clone, Debug, Default)]
pub struct RulesSpec {
    /// The `when` list, shared: the tree's copy of a spec and a journal's
    /// hold the declaring spec's list rather than a copy of it.
    entries: std::sync::Arc<Vec<Rule>>,
    /// `min_column = n` on a row (ADR 0069, decision 17): it wraps into
    /// equal columns, as many as fit at `n` wide or more, every child a
    /// column wide. 0 is none.
    pub min_column: f32,
    /// The children a `hide` inside an entry's builder named: read by
    /// [`Patch::capture`], meaningless on a declared spec.
    pub(crate) hide: Vec<String>,
    /// The lazy branch this node holds, if it is one.
    pub(crate) lazy: Option<LazySpec>,
    /// Entries reading the node's own width, its height, the window, or
    /// the pointer, as bits.
    pub(crate) size_w: u64,
    pub(crate) size_h: u64,
    pub(crate) window: u64,
    pub(crate) tracks_hover: bool,
    pub(crate) live: Live,
}

/// What a node declared, not what the core resolved for it this frame.
impl PartialEq for RulesSpec {
    fn eq(&self, other: &Self) -> bool {
        self.entries == other.entries
            && self.min_column == other.min_column
            && self.hide == other.hide
            && self.lazy == other.lazy
    }
}

impl RulesSpec {
    /// The `when` list.
    pub fn entries(&self) -> &[Rule] {
        &self.entries
    }

    /// The lazy ask, for a lazy branch's node.
    pub fn lazy(&self) -> Option<&LazySpec> {
        self.lazy.as_ref()
    }

    /// Appends an entry. Past [`MAX_RULES`] the entry is dropped and the
    /// last one kept names it refused.
    pub fn push(&mut self, rule: Rule) {
        let i = self.entries.len();
        if i >= MAX_RULES {
            if let Some(last) = std::sync::Arc::make_mut(&mut self.entries).last_mut() {
                last.refused.push(format!(
                    "entry {} (a node holds {MAX_RULES} at most)",
                    i + 1
                ));
            }
            return;
        }
        let bit = 1u64 << i;
        if rule.cond.width.is_some() {
            self.size_w |= bit;
        }
        if rule.cond.height.is_some() {
            self.size_h |= bit;
        }
        if rule.cond.window_width.is_some() || rule.cond.window_height.is_some() {
            self.window |= bit;
        }
        self.tracks_hover |= rule.cond.state_mask & (state::HOVERED | state::PRESSED) != 0;
        std::sync::Arc::make_mut(&mut self.entries).push(rule);
    }

    /// The entries over the node's own size.
    pub(crate) fn sized(&self) -> u64 {
        self.size_w | self.size_h
    }

    /// The entries the last resolution applied, as a bit set.
    pub fn matched(&self) -> u64 {
        self.live.matched
    }

    /// Lays the matched entries' size-entry props over the declared ones,
    /// in list order.
    pub(crate) fn relay(spec: &mut NodeSpec) {
        // The rules held apart while their patches write the spec's
        // layout, and the `min_column` they set written to them after.
        let Some(mut rules) = spec.take_rules() else {
            return;
        };
        let matched = rules.live.matched;
        rules.live.base.put(spec);
        let mut min_column = rules.live.base.min_column;
        for (i, r) in rules.entries.iter().enumerate() {
            if matched & (1 << i) != 0 && r.patch.set & field::LAYOUT_OF_CHILDREN != 0 {
                r.patch
                    .apply(spec, field::LAYOUT_OF_CHILDREN & !field::MIN_COLUMN);
                if let Some(n) = r.patch.min_column() {
                    min_column = n;
                }
            }
        }
        rules.min_column = min_column;
        spec.put_rules(rules);
    }

    /// Which entries match, from what is known of each: the static
    /// conditions, and the own-size ranges as last answered.
    pub(crate) fn matching(&self) -> u64 {
        let l = &self.live;
        let mut m = l.static_ok;
        m &= !self.size_w | l.held_w;
        m &= !self.size_h | l.held_h;
        m
    }
}

// -- Layout --------------------------------------------------------------

/// Node `i`'s own-size entries over `width` (or its height), its size in
/// that axis now final: each range answered against it, the matched set
/// recomputed and, where it moved, the props laid again and the children
/// it hides marked. Called by the solver at the top of the node's
/// distribution in that axis, before its children are laid across it.
/// Returns whether the match moved.
pub(crate) fn settle_axis(tree: &mut Tree, i: usize, width: bool) -> bool {
    let size = if width {
        tree.size[i].w
    } else {
        tree.size[i].h
    };
    let Some(rules) = tree.specs[i].rules_ref_mut() else {
        return false;
    };
    let mask = if width { rules.size_w } else { rules.size_h };
    if mask == 0 {
        return false;
    }
    let mut held = if width {
        rules.live.held_w
    } else {
        rules.live.held_h
    };
    for (k, rule) in rules.entries.iter().enumerate() {
        let bit = 1u64 << k;
        if mask & bit == 0 {
            continue;
        }
        let range = if width {
            rule.cond.width
        } else {
            rule.cond.height
        };
        if let Some(r) = range {
            if r.holds(size, held & bit != 0) {
                held |= bit;
            } else {
                held &= !bit;
            }
        }
    }
    if width {
        rules.live.held_w = held;
    } else {
        rules.live.held_h = held;
    }
    let matched = rules.matching();
    let moved = matched != rules.live.matched;
    if moved {
        rules.live.matched = matched;
        if !width {
            rules.live.height_moved = true;
        }
        RulesSpec::relay(&mut tree.specs[i]);
    }
    // The children are in the tree now, which they were not when the
    // node opened with the match it came in with: marked whether or not
    // the match moved.
    mark_hidden(tree, i);
    moved
}

/// Marks the children of `i` its matched entries hide, and unmarks the
/// rest of those its entries name.
pub(crate) fn mark_hidden(tree: &mut Tree, i: usize) {
    let Some(rules) = tree.specs[i].rules_ref() else {
        return;
    };
    if rules.live.hide.is_empty() {
        return;
    }
    let matched = rules.live.matched;
    let hide: Vec<(bool, Key)> = rules
        .live
        .hide
        .iter()
        .map(|&(e, k)| (matched & (1 << e) != 0, k))
        .collect();
    if tree.rule_hidden.len() < tree.len() {
        tree.rule_hidden.resize(tree.len(), false);
    }
    let mut c = tree.first_child[i];
    while c != NIL {
        let key = tree.keys[c as usize];
        let mut named = false;
        let mut hidden = false;
        for &(on, k) in &hide {
            if k == key {
                named = true;
                hidden |= on;
            }
        }
        if named {
            tree.rule_hidden[c as usize] = hidden;
        }
        c = tree.next_sibling[c as usize];
    }
}

/// `min_column` on row `i`, its width final: every in-flow child as wide
/// as one of the equal columns that fit at `n` or more, the row wrapping
/// after each line of them.
pub(crate) fn columns(tree: &mut Tree, i: usize, is_out: impl Fn(&Tree, u32) -> bool) {
    let n = tree.specs[i].rules_spec().min_column;
    let l = tree.specs[i].layout;
    if n <= 0.0 || l.dir != Dir::Row {
        return;
    }
    let content = (tree.size[i].w - l.padding.x()).max(0.0);
    let gap = l.gap;
    let count = ((content + gap) / (n + gap)).floor().max(1.0);
    // A hair under the exact share, so a line of `count` columns is never
    // broken by the last bit of a float.
    let w = ((content - gap * (count - 1.0)) / count - 1e-3).max(0.0);
    tree.specs[i].layout.wrap = true;
    tree.any_wrap = true;
    let mut c = tree.first_child[i];
    while c != NIL {
        if !is_out(tree, c) {
            let cl = &mut tree.specs[c as usize].layout;
            cl.width = Sizing::Fixed(w);
            cl.min_w = Min::AUTO;
            cl.max_w = f32::INFINITY;
            tree.size[c as usize].w = w;
        }
        c = tree.next_sibling[c as usize];
    }
}

// -- Parsing ------------------------------------------------------------

/// Whether `name` (either spelling) is a condition key of an entry: a
/// `width` or `height` whose value is a range (`{ below, above, band }`)
/// rather than a sizing, or one of the other condition names.
pub fn is_condition_key(name: &str, v: &Value) -> bool {
    match name {
        "width" | "height" => is_range(v),
        "window" | "dark" | "light" | "reduced_motion" | "reducedMotion" | "contrast"
        | "platform" | "scale" => true,
        _ => state_bit(name).is_some(),
    }
}

fn state_bit(name: &str) -> Option<u16> {
    match name {
        "focusVisible" => Some(state::FOCUS_VISIBLE),
        "groupHovered" => Some(state::GROUP_HOVERED),
        "groupPressed" => Some(state::GROUP_PRESSED),
        "groupFocused" => Some(state::GROUP_FOCUSED),
        "groupFocusVisible" => Some(state::GROUP_FOCUS_VISIBLE),
        _ => state::NAMES
            .iter()
            .find(|(n, _)| *n == name)
            .map(|(_, b)| *b),
    }
}

/// Whether `v` is a range table: a map with `below`, `above` or `band`.
fn is_range(v: &Value) -> bool {
    v.get("below").is_some() || v.get("above").is_some() || v.get("band").is_some()
}

type Refs<'r, 'a> = Option<&'r mut crate::tokens::NameRefs<'a>>;

/// A length: a number, or a `"$name"` length token (ADR 0027). A token
/// that misses is remembered on the refs and reads as no threshold.
fn length(v: &Value, refs: &mut Refs<'_, '_>) -> Result<Option<f32>, String> {
    if let Some(n) = v.as_float() {
        return Ok(Some(n as f32));
    }
    if let Some(refs) = refs.as_deref_mut()
        && let Some(r) = refs.length_ref(v)
    {
        return Ok(r);
    }
    Err(format!(
        "a threshold is a number or a \"$length\" token, not {}",
        v.type_name()
    ))
}

fn range(v: &Value, refs: &mut Refs<'_, '_>) -> Result<Range, String> {
    let mut get = |k: &str| match v.get(k) {
        Some(x) => length(x, refs),
        None => Ok(None),
    };
    Ok(Range {
        below: get("below")?,
        above: get("above")?,
        band: get("band")?.unwrap_or(0.0),
    })
}

/// Reads one condition key into `cond`.
fn parse_condition(
    cond: &mut Cond,
    name: &str,
    v: &Value,
    refs: &mut Refs<'_, '_>,
) -> Result<(), String> {
    let flag = |v: &Value| {
        v.as_bool()
            .ok_or_else(|| format!("`{name}` is true or false"))
    };
    match name {
        "width" => cond.width = Some(range(v, refs)?),
        "height" => cond.height = Some(range(v, refs)?),
        "window" => {
            if let Some(w) = v.get("width") {
                cond.window_width = Some(range(w, refs)?);
            }
            if let Some(h) = v.get("height") {
                cond.window_height = Some(range(h, refs)?);
            }
        }
        "dark" => cond.dark = Some(flag(v)?),
        "light" => cond.light = Some(flag(v)?),
        "reduced_motion" | "reducedMotion" => cond.reduced_motion = Some(flag(v)?),
        "contrast" => {
            let s = v.as_str().ok_or("`contrast` is \"more\" or \"normal\"")?;
            cond.contrast = Some(
                ContrastPref::parse(s)
                    .ok_or_else(|| format!("`contrast` is \"more\" or \"normal\", not {s:?}"))?,
            );
        }
        "platform" => {
            let one = |v: &Value| -> Result<u8, String> {
                let s = v.as_str().ok_or("`platform` names a platform")?;
                Platform::parse(s).map(|p| p.bit()).ok_or_else(|| {
                    format!("`platform` is \"macos\", \"windows\" or \"linux\", not {s:?}")
                })
            };
            cond.platforms = match v.as_list() {
                Some(list) => list.iter().try_fold(0u8, |m, x| one(x).map(|b| m | b))?,
                None => one(v)?,
            };
        }
        "scale" => cond.scale = Some(range(v, refs)?),
        _ => {
            let bit = state_bit(name).ok_or_else(|| format!("`{name}` is no condition"))?;
            cond.state_mask |= bit;
            if flag(v)? {
                cond.state_want |= bit;
            } else {
                cond.state_want &= !bit;
            }
        }
    }
    Ok(())
}

/// A condition set from its plain-data form — every key a condition
/// key — for a lazy branch's `query`.
pub fn parse_cond(v: &Value, mut refs: Refs<'_, '_>) -> Result<Cond, String> {
    let Value::Map(fields) = v else {
        return Err("a query is a table of conditions".into());
    };
    let mut cond = Cond::default();
    for (k, x) in fields {
        parse_condition(&mut cond, k, x, &mut refs)?;
    }
    Ok(cond)
}

/// A builder that sets props on a spec.
pub type PropFn<'a> = Box<dyn Fn(NodeSpec) -> NodeSpec + 'a>;

/// A builder that sets one prop, with the prop's name for a refusal.
pub type PropStep<'a> = (String, PropFn<'a>);

/// Builds an entry from its conditions and its props one by one, so a
/// prop it may not set is refused by name.
pub fn entry(cond: Cond, props: &[PropStep<'_>]) -> Rule {
    let mut rule = Rule {
        cond,
        ..Rule::default()
    };
    for (name, f) in props {
        let (patch, other) = Patch::capture(&**f);
        if let Some(what) = other {
            rule.refused
                .push(format!("`{name}` ({what}: no entry sets one)"));
            continue;
        }
        let before = rule.refused.len();
        rule.add(patch);
        // Name the prop as the view spelled it, not the field.
        for r in &mut rule.refused[before..] {
            if let Some((_, why)) = r.split_once("` ") {
                *r = format!("`{name}` {why}");
            }
        }
    }
    rule
}

/// The props of an entry a binding names by itself rather than by a
/// `schema::PROPS` row: the composites every binding extracts by hand
/// (padding, border, overflow, direction) and `hidden` as a list of the
/// children an own-size entry hides. `None` for a name that is a row.
fn composite<'a>(
    name: &str,
    v: &'a Value,
    refs: &mut Refs<'_, '_>,
) -> Option<Result<PropFn<'a>, String>> {
    let num = |v: &Value, refs: &mut Refs<'_, '_>| -> Result<f32, String> {
        length(v, refs)?.ok_or_else(|| "a token that names no length".to_string())
    };
    let step: Box<dyn Fn(NodeSpec) -> NodeSpec> = match name {
        "dir" => {
            let d = match v.as_str() {
                Some("row") => Dir::Row,
                Some("column") => Dir::Column,
                _ => return Some(Err("`dir` is \"row\" or \"column\" in an entry".into())),
            };
            Box::new(move |s: NodeSpec| s.dir(d))
        }
        "pad" | "padX" | "padY" | "padL" | "padR" | "padT" | "padB" | "pad_x" | "pad_y" => {
            let mut p = crate::spec::PadShorthand::default();
            let r = (|| -> Result<(), String> {
                match (name, v) {
                    ("pad", Value::Map(m)) => {
                        for (k, x) in m {
                            let n = Some(num(x, refs)?);
                            match k.as_str() {
                                "all" => p.all = n,
                                "x" => p.x = n,
                                "y" => p.y = n,
                                "l" => p.l = n,
                                "r" => p.r = n,
                                "t" => p.t = n,
                                "b" => p.b = n,
                                _ => return Err(format!("`pad` has no `{k}`")),
                            }
                        }
                    }
                    ("pad", x) => p.all = Some(num(x, refs)?),
                    ("padX" | "pad_x", x) => p.x = Some(num(x, refs)?),
                    ("padY" | "pad_y", x) => p.y = Some(num(x, refs)?),
                    ("padL", x) => p.l = Some(num(x, refs)?),
                    ("padR", x) => p.r = Some(num(x, refs)?),
                    ("padT", x) => p.t = Some(num(x, refs)?),
                    (_, x) => p.b = Some(num(x, refs)?),
                }
                Ok(())
            })();
            if let Err(e) = r {
                return Some(Err(e));
            }
            let edges = p.resolve();
            Box::new(move |s: NodeSpec| s.padding(edges))
        }
        "border" | "borderW" | "borderColor" | "border_w" | "border_color" => {
            let (mut w, mut c) = (None, None);
            let color = |x: &Value, refs: &mut Refs<'_, '_>| -> Result<Color, String> {
                color_value(x, refs)?.ok_or_else(|| "a token that names no colour".to_string())
            };
            let r = (|| -> Result<(), String> {
                match name {
                    "border" => {
                        if let Some(x) = v.get("w") {
                            w = Some(num(x, refs)?);
                        }
                        if let Some(x) = v.get("color") {
                            c = Some(color(x, refs)?);
                        }
                    }
                    "borderW" | "border_w" => w = Some(num(v, refs)?),
                    _ => c = Some(color(v, refs)?),
                }
                Ok(())
            })();
            if let Err(e) = r {
                return Some(Err(e));
            }
            Box::new(move |mut s: NodeSpec| {
                if let Some(w) = w {
                    s.style.border_w = w;
                }
                if let Some(c) = c {
                    s.style.border_color = c;
                }
                s
            })
        }
        "clip" | "scrollX" | "scrollY" | "scroll_x" | "scroll_y" | "scroll" => {
            let on = v.as_bool().unwrap_or(false);
            let name = name.to_string();
            Box::new(move |mut s: NodeSpec| {
                match name.as_str() {
                    "clip" => s.layout.clip = on,
                    "scrollX" | "scroll_x" => s.layout.scroll_x = on,
                    _ => s.layout.scroll_y = on,
                }
                s
            })
        }
        "hidden" if v.as_list().is_some() => {
            let labels: Vec<String> = v
                .as_list()
                .unwrap_or_default()
                .iter()
                .filter_map(|x| x.as_str().map(str::to_string))
                .collect();
            Box::new(move |mut s: NodeSpec| {
                for l in &labels {
                    s = s.hide(l);
                }
                s
            })
        }
        _ => return None,
    };
    Some(Ok(step))
}

/// A colour: `0xRRGGBBAA`, `"#hex"` or a `"$name"` colour token.
fn color_value(v: &Value, refs: &mut Refs<'_, '_>) -> Result<Option<Color>, String> {
    if let Some(n) = v.as_int() {
        return Ok(Some(crate::schema::color_num(n as u32)));
    }
    if let Some(refs) = refs.as_deref_mut()
        && let Some(c) = refs.color_ref(v)
    {
        return Ok(c);
    }
    match v.as_str() {
        Some(s) => crate::schema::color_hex_str(s).map(Some),
        None => Err(format!("a colour, not {}", v.type_name())),
    }
}

/// One prop's value read through its `schema::PROPS` row, the way every
/// binding reads it from its own form: what an entry's props and C's
/// `when` list go through. `Ok(None)` for a value that sets nothing (a
/// flag that is false, a token that missed).
pub fn parse_value(
    def: &crate::schema::PropDef,
    v: &Value,
    refs: &mut Refs<'_, '_>,
) -> Result<Option<crate::schema::Parsed>, String> {
    use crate::schema::{Kind, Parsed};
    Ok(Some(match def.kind {
        Kind::F32 => match length(v, refs)? {
            Some(n) => Parsed::F32(n),
            None => return Ok(None),
        },
        Kind::Color => match color_value(v, refs)? {
            Some(c) => Parsed::Color(c),
            None => return Ok(None),
        },
        Kind::Flag => match v.as_bool() {
            Some(true) => Parsed::Flag,
            Some(false) => return Ok(None),
            None => return Err("expected true or false".into()),
        },
        Kind::Enum(names) => match (v.as_str(), v.as_int()) {
            (Some(s), _) => Parsed::Enum(crate::schema::enum_index(names, s)?),
            (None, Some(i)) if (i as usize) < names.len() => Parsed::Enum(i as usize),
            _ => return Err(format!("one of {names:?}")),
        },
        Kind::Sizing => match v {
            Value::Int(_) | Value::Float(_) => {
                Parsed::Sizing(Sizing::Fixed(v.as_float().unwrap_or(0.0) as f32))
            }
            Value::Str(s) if s.starts_with('$') => match length(v, refs)? {
                Some(n) => Parsed::Sizing(Sizing::Fixed(n)),
                None => return Ok(None),
            },
            Value::Str(s) => Parsed::Sizing(crate::schema::sizing_str(s)?),
            Value::Map(_) => {
                if let Some(g) = v.get_f32("grow") {
                    Parsed::Sizing(Sizing::Grow(g))
                } else if let Some(p) = v.get_f32("percent") {
                    Parsed::Sizing(Sizing::Percent(p))
                } else {
                    return Err("a sizing table is `{ grow = n }` or `{ percent = n }`".into());
                }
            }
            _ => return Err("expected a sizing".into()),
        },
        Kind::Min | Kind::Max => match v {
            Value::Int(_) | Value::Float(_) => {
                Parsed::Bound(crate::spec::Bound::Px(v.as_float().unwrap_or(0.0) as f32))
            }
            Value::Str(s) if s.starts_with('$') => match length(v, refs)? {
                Some(n) => Parsed::Bound(crate::spec::Bound::Px(n)),
                None => return Ok(None),
            },
            Value::Str(s) if matches!(def.kind, Kind::Min) => {
                Parsed::Bound(crate::schema::min_str(s)?)
            }
            Value::Str(s) => Parsed::Bound(crate::schema::max_str(s)?),
            _ => return Err("expected a number or a string".into()),
        },
        Kind::Msg | Kind::Handler => Parsed::Msg(v.clone()),
        Kind::Str => Parsed::Str(v.as_str().ok_or("expected a string")?.to_string()),
        Kind::Family => return Err("a family is a text's".into()),
        Kind::Resource => return Err("a resource is not an entry's".into()),
        Kind::Keyframes => Parsed::Keyframes(crate::keyframes::parse_with(v, refs.as_deref_mut())?),
        Kind::Enter => Parsed::Enter(crate::enter::parse_with(v, refs.as_deref_mut())?),
        Kind::Gradient => Parsed::Gradient(crate::gradient::parse_with(v, refs.as_deref_mut())?),
        Kind::Rules => return Err("a `when` inside a `when` entry".into()),
    }))
}

/// A `when` list from its plain-data form, the shape every binding
/// passes it in: a list of tables, each the conditions it tests and the
/// props it sets (`{ width = { below = 700 }, dark = true, gap = 8 }`),
/// prop names in either spelling. A condition that does not read, a
/// name that is neither a condition nor a prop, and a prop the entry may
/// not set are each named in the entry's [`Rule::refused`]; the rest of
/// the entry stands. `Err` only for a value that is not a list.
pub fn parse_with(v: &Value, mut refs: Refs<'_, '_>) -> Result<Vec<Rule>, String> {
    let Value::List(entries) = v else {
        return Err("`when` is a list of entries".into());
    };
    let mut out = Vec::with_capacity(entries.len());
    for (i, e) in entries.iter().enumerate() {
        let Value::Map(fields) = e else {
            let mut r = Rule::default();
            r.refused.push(format!("entry {i}, which is not a table"));
            out.push(r);
            continue;
        };
        let mut cond = Cond::default();
        let mut bad = Vec::new();
        let mut steps: Vec<PropStep<'_>> = Vec::new();
        for (k, x) in fields {
            if is_condition_key(k, x) {
                if let Err(e) = parse_condition(&mut cond, k, x, &mut refs) {
                    bad.push(format!("`{k}` ({e})"));
                }
                continue;
            }
            if let Some(step) = composite(k, x, &mut refs) {
                match step {
                    Ok(f) => steps.push((k.clone(), f)),
                    Err(e) => bad.push(format!("`{k}` ({e})")),
                }
                continue;
            }
            let def = crate::schema::by_name(k).or_else(|| crate::schema::by_snake_name(k));
            let Some(def) = def else {
                bad.push(format!("`{k}` (neither a condition nor a prop)"));
                continue;
            };
            if def.target() == crate::schema::Target::Style {
                bad.push(format!("`{k}` (a text's style, not a box's)"));
                continue;
            }
            match parse_value(def, x, &mut refs) {
                Ok(Some(parsed)) => steps.push((
                    k.clone(),
                    Box::new(move |s: NodeSpec| {
                        let mut out = crate::schema::PropsOut {
                            spec: s,
                            ..crate::schema::PropsOut::new()
                        };
                        let _ = crate::schema::apply(def, parsed.clone(), &mut out);
                        out.spec
                    }),
                )),
                Ok(None) => {}
                Err(e) => bad.push(format!("`{k}` ({e})")),
            }
        }
        let mut rule = entry(cond, &steps);
        rule.refused.extend(bad);
        out.push(rule);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_entry_sets_a_shadows_lanes_one_by_one() {
        // C114: an entry that moves the shadow keeps its colour and blur.
        let base = || {
            NodeSpec::row()
                .shadow_color(Color::hex(0x00000080))
                .shadow_blur(6.0)
                .shadow_offset(2.0, 2.0)
        };
        let (lift, refused) = Patch::capture(&|s| s.shadow_offset(3.0, 4.0));
        assert_eq!(refused, None);
        let mut spec = base();
        lift.apply(&mut spec, u32::MAX);
        let sh = spec.style.shadow;
        assert_eq!((sh.dx, sh.dy), (3.0, 4.0), "the lanes it sets");
        assert_eq!(sh.color, Color::hex(0x00000080), "the colour kept");
        assert_eq!(sh.blur, 6.0, "the blur kept");

        // Merged after an entry setting the colour, both land.
        let (tint, _) = Patch::capture(&|s| s.shadow_color(Color::hex(0xff000080)));
        let mut both = tint.clone();
        both.merge(&lift);
        let mut spec = base();
        both.apply(&mut spec, u32::MAX);
        let sh = spec.style.shadow;
        assert_eq!(sh.color, Color::hex(0xff000080));
        assert_eq!((sh.dx, sh.dy, sh.blur), (3.0, 4.0, 6.0));

        // A lane set to the default's value still counts as set.
        let (flat, _) = Patch::capture(&|s| s.shadow_y(0.0));
        let mut spec = base();
        flat.apply(&mut spec, u32::MAX);
        assert_eq!((spec.style.shadow.dx, spec.style.shadow.dy), (2.0, 0.0));
    }

    #[test]
    fn a_band_holds_a_range_past_its_threshold_once_it_holds() {
        let r = Range {
            below: Some(700.0),
            above: None,
            band: 16.0,
        };
        assert!(r.holds(699.0, false));
        assert!(!r.holds(705.0, false), "not yet holding: the threshold");
        assert!(r.holds(705.0, true), "holding: the band");
        assert!(!r.holds(716.0, true), "and out at the band's edge");
        let a = Range {
            below: None,
            above: Some(300.0),
            band: 10.0,
        };
        assert!(a.holds(300.0, false), "above holds at the threshold");
        assert!(!a.holds(295.0, false));
        assert!(a.holds(295.0, true));
        assert!(!a.holds(289.0, true));
    }

    #[test]
    fn a_patch_is_what_the_builder_set_whatever_the_value() {
        let (p, other) = Patch::capture(&|s: NodeSpec| s.gap(0.0).bg(Color::hex(0x000000ff)));
        assert!(other.is_none());
        assert_eq!(p.set, field::GAP | field::BG, "a gap of 0 is set too");
        let (p, other) = Patch::capture(&|s: NodeSpec| s.on_click("x"));
        assert_eq!(p.set, 0);
        assert_eq!(other, Some("a handler"));
    }

    #[test]
    fn a_size_entry_sets_the_layout_of_its_children_only() {
        let rule = Rule::new(Cond::width_below(500.0), &|s| {
            s.gap(4.0).bg(Color::hex(0xff))
        });
        assert_eq!(rule.patch.set, field::GAP);
        assert_eq!(rule.refused.len(), 1);
        assert!(rule.refused[0].contains("`bg`"), "{:?}", rule.refused);
        let rule = Rule::new(Cond::Dark, &|s| s.gap(4.0).bg(Color::hex(0xff)));
        assert_eq!(rule.patch.set, field::GAP | field::BG);
        assert!(rule.refused.is_empty());
    }

    #[test]
    fn conditions_join_and_read_back() {
        let c = Cond::width_above(300.0)
            .and(Cond::width_below(700.0))
            .band(8.0)
            .and(Cond::Dark)
            .and(Cond::Hovered.is(false));
        let w = c.width.unwrap();
        assert_eq!((w.above, w.below, w.band), (Some(300.0), Some(700.0), 8.0));
        assert_eq!(c.dark, Some(true));
        assert_eq!((c.state_mask, c.state_want), (state::HOVERED, 0));
        assert_eq!(c.describe(), "300 ≤ width < 700 ±8 · dark · not hovered");
    }
}
