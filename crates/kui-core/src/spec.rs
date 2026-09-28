//! Node configuration: plain data, trivially constructible from any language.

use crate::anim::{Easing, Repeat, Transition};
use crate::color::Color;
use crate::cursor::CursorShape;
use crate::enter::Enter;
use crate::geom::Edges;
use crate::input::Buttons;
use crate::keyframes::Keyframe;
use crate::value::Value;
use crate::window::{WindowButton, WindowRole};

pub use crate::access::{Label, Live, Role};

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum Sizing {
    /// Size to content.
    #[default]
    Fit,
    /// Share leftover space, weighted by factor.
    Grow(f32),
    /// Absolute logical pixels.
    Fixed(f32),
    /// Fraction of the parent's content box (0.0..=1.0).
    Percent(f32),
}

/// A lower clamp on one axis: a number of logical px, or the node's own
/// fit size on that axis (`minWidth: "fit"`, [`Min::FIT`]). `FIT` is what
/// lets a `Grow` child keep a content floor — CSS's `flex: 1 0 auto`: the
/// tabs of an i3-style bar split the bar evenly while they fit and sit at
/// their label's width, scrolling, once they do not. Layout resolves it
/// to a number in the fit pass of its axis (`layout::fit_widths` /
/// `fit_heights`), so every later clamp reads one; until then it clamps
/// like no floor at all.
///
/// One `f32`, with `FIT` as a negative — the form `KuiSpec.min_w` takes
/// too (`KUI_MIN_FIT`) — rather than an enum with a tag: `LayoutSpec` is
/// copied per node per frame, and a tagged pair for two axes is eight
/// bytes on every node for a floor almost none declares (C15). A negative
/// floor never meant anything, so the slot was free.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Min(f32);

impl Min {
    /// The node's own fit size on this axis.
    pub const FIT: Min = Min(-1.0);

    /// A floor of `v` logical px; a negative is no floor.
    pub fn px(v: f32) -> Min {
        Min(v.max(0.0))
    }

    /// Whether this is the unresolved fit floor.
    pub fn is_fit(self) -> bool {
        self.0 < 0.0
    }

    /// The clamp as a number: the px it holds, or 0 for a `FIT` layout has
    /// not resolved yet (nothing to floor at).
    pub fn resolved(self) -> f32 {
        self.0.max(0.0)
    }
}

impl From<f32> for Min {
    fn from(v: f32) -> Self {
        Min::px(v)
    }
}

/// A number is that many logical px, so `.width(120.0)` is
/// `.width(Sizing::Fixed(120.0))`, the way `min_width(120.0)` already read.
impl From<f32> for Sizing {
    fn from(px: f32) -> Self {
        Sizing::Fixed(px)
    }
}

impl Sizing {
    /// `Grow(1.0)`: an equal share of the leftover space, the weight
    /// nearly every grow declares.
    pub const GROW: Sizing = Sizing::Grow(1.0);
}

impl Sizing {
    /// The sizing the way a spec spells it — `fit`, `grow(1)`, `120px`,
    /// `50%` — for a reader: the devtools' inspector and a `nodes()` row.
    pub fn describe(self) -> String {
        match self {
            Sizing::Fit => "fit".into(),
            Sizing::Grow(w) => format!("grow({w})"),
            Sizing::Fixed(px) => format!("{px}px"),
            Sizing::Percent(p) => format!("{}%", p * 100.0),
        }
    }

    /// The animatable number inside: a grow factor, a px size, a fraction.
    /// None for `Fit`, which has nothing to ease.
    pub fn amount(self) -> Option<f32> {
        match self {
            Sizing::Fit => None,
            Sizing::Grow(v) | Sizing::Fixed(v) | Sizing::Percent(v) => Some(v),
        }
    }

    /// The same form with a different amount (`Fit` stays `Fit`).
    pub fn with_amount(self, v: f32) -> Self {
        match self {
            Sizing::Fit => Sizing::Fit,
            Sizing::Grow(_) => Sizing::Grow(v),
            Sizing::Fixed(_) => Sizing::Fixed(v),
            Sizing::Percent(_) => Sizing::Percent(v),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Dir {
    Row,
    #[default]
    Column,
}

impl Dir {
    /// The `dir` row's spelling.
    pub fn name(self) -> &'static str {
        match self {
            Dir::Row => "row",
            Dir::Column => "column",
        }
    }
}

/// Where children sit along an axis, and where a float attaches.
///
/// The first three are every axis's. The rest were appended (backlog C13,
/// in `schema::ALIGNS` order, so the wire indices and `KUI_ALIGN_*` of the
/// first three did not move) and each means something on one axis only:
/// the three spreads on `main_align`, `Baseline` on a row's
/// `cross_align`. Anywhere else one lays out as `Start`
/// (`SpaceAround`/`SpaceEvenly` as `Center`), with a warning
/// (`diag::ALIGN_IGNORED`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Align {
    #[default]
    Start,
    Center,
    End,
    /// Main axis: the free space goes between the children, none at the
    /// ends — CSS's `space-between`. One child sits at the start.
    SpaceBetween,
    /// Main axis: each child gets an equal share of the free space, half
    /// on either side, so the ends get half what a gap does — CSS's
    /// `space-around`. One child is centred.
    SpaceAround,
    /// Main axis: the free space splits into equal gaps between the
    /// children and at both ends — CSS's `space-evenly`. One child is
    /// centred.
    SpaceEvenly,
    /// A row's cross axis: the first baselines of the children's text line
    /// up, so a label and a larger value on one row read as one line. A
    /// child with no text inside it aligns by its bottom edge; a `grow` or
    /// percent height fills the line and sits at its top.
    Baseline,
}

impl Align {
    /// The `align` rows' spelling (`schema::ALIGNS`).
    pub fn name(self) -> &'static str {
        crate::schema::ALIGNS[self as usize]
    }
}

/// What a floating node is positioned against.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum FloatAnchor {
    /// The parent node's border box.
    #[default]
    Parent,
    /// The whole viewport.
    Viewport,
    /// The border box of the node `Key` names, wherever it is in the
    /// tree — even after this one in preorder, which the five passes
    /// cannot serve, so a float anchored this way is laid out again in
    /// a sixth, once its anchor is placed (`layout::anchored`). What a
    /// devtools tab's content is: built in the app's part of the tree,
    /// shown over the panel's tab body (ADR 0032, decision 2). No
    /// binding spells it; the core builds it.
    Node(crate::key::Key),
}

/// Takes a node out of flex flow: it doesn't consume space in its parent,
/// sizes Grow/Percent against its anchor, is positioned by attach points,
/// and escapes ancestor clips unless [`FloatConfig::clip`] keeps it in its
/// parent's. It paints as a layer of its own — above the
/// in-flow tree and every float that opened before it, under every one
/// that opened after — and takes input in the same order
/// (`docs/adr/0023-layers-stack-in-the-order-they-open.md`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FloatConfig {
    pub anchor: FloatAnchor,
    /// Attach point on the anchor rect (horizontal, vertical).
    pub anchor_point: (Align, Align),
    /// Attach point on the floating node itself.
    pub self_point: (Align, Align),
    /// Extra offset applied after attaching, logical px.
    pub offset: Vec2Offset,
    /// Keep the float on screen: if the attached placement leaves the
    /// viewport on an axis, mirror the attachment across the anchor on that
    /// axis (below ↔ above, after ↔ before) when that fits better, then
    /// clamp whatever still overflows. Tooltips/menus want this.
    ///
    /// This is the *in-window* approximation of an OS popup, and it is the
    /// one to reach for first: a float costs one tree, one hit list and one
    /// draw call. What it cannot do is leave the window — a dropdown taller
    /// than the viewport, or a menu with nowhere in-window to go, gets
    /// clamped rather than placed. Those want a popup window — which this
    /// release does not have: it is `docs/adr/0004-multi-window.md`'s step
    /// 4, and the ADR's Consequences say so. So `fit` plus a `modal` float
    /// (`docs/adr/0003-modal-surfaces.md`) is not merely the first thing to
    /// reach for today, it is the only thing.
    pub fit: bool,
    /// Take the parent's clip, as a child does, instead of escaping every
    /// ancestor's: a node on a `clip` canvas panned past the canvas's edge
    /// is cut there, and its hit region with it, rather than drawn over
    /// and clicked through the toolbar beside it (backlog F90). Only a
    /// [`FloatAnchor::Parent`] float reads it — a viewport or node anchor
    /// is placed against something other than the parent, and escapes
    /// with it set or not. Paint order is unchanged: the float is still a
    /// layer of its own above its in-flow siblings, only cut. A `line` or
    /// `polygon` anchored in its parent's box has it set by the core (ADR
    /// 0010, decision 5, as amended).
    pub clip: bool,
}

/// Plain offset pair (kept separate from geometry to stay `Copy` + FFI-flat).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Vec2Offset {
    pub x: f32,
    pub y: f32,
}

impl Default for FloatConfig {
    fn default() -> Self {
        Self {
            anchor: FloatAnchor::Parent,
            anchor_point: (Align::Start, Align::Start),
            self_point: (Align::Start, Align::Start),
            offset: Vec2Offset::default(),
            fit: false,
            clip: false,
        }
    }
}

impl FloatConfig {
    pub fn parent() -> Self {
        Self::default()
    }

    pub fn viewport() -> Self {
        Self {
            anchor: FloatAnchor::Viewport,
            ..Self::default()
        }
    }

    /// Tooltip-style: hang below the parent, centered.
    pub fn below() -> Self {
        Self {
            anchor: FloatAnchor::Parent,
            anchor_point: (Align::Center, Align::End),
            self_point: (Align::Center, Align::Start),
            offset: Vec2Offset { x: 0.0, y: 6.0 },
            ..Self::default()
        }
    }

    /// Tooltip-style: hover above the parent, centered.
    pub fn above() -> Self {
        Self {
            anchor: FloatAnchor::Parent,
            anchor_point: (Align::Center, Align::Start),
            self_point: (Align::Center, Align::End),
            offset: Vec2Offset { x: 0.0, y: -6.0 },
            ..Self::default()
        }
    }

    pub fn at(mut self, x: Align, y: Align) -> Self {
        self.anchor_point = (x, y);
        self
    }

    pub fn self_at(mut self, x: Align, y: Align) -> Self {
        self.self_point = (x, y);
        self
    }

    /// [`Self::at`] and [`Self::self_at`] at the same point: the float sits
    /// inside the anchor against that edge or corner — `(End, End)` is a
    /// toast in the viewport's bottom-right, `(Center, Center)` a dialog.
    #[inline]
    pub fn inside(self, x: Align, y: Align) -> Self {
        self.at(x, y).self_at(x, y)
    }

    pub fn offset(mut self, x: f32, y: f32) -> Self {
        self.offset = Vec2Offset { x, y };
        self
    }

    /// Flip across the anchor / clamp as needed to stay in the viewport;
    /// see [`FloatConfig::fit`] for where that stops being enough.
    pub fn fit(mut self) -> Self {
        self.fit = true;
        self
    }

    /// Cut by the parent's clip instead of escaping it; see
    /// [`FloatConfig::clip`] for which anchors read it.
    pub fn clipped(mut self) -> Self {
        self.clip = true;
        self
    }

    /// Whether this float takes its parent's clip: [`FloatConfig::clip`]
    /// declared on a parent-anchored float. The one reading both paint
    /// passes and the hit regions share.
    pub fn clipped_by_parent(&self) -> bool {
        self.clip && self.anchor == FloatAnchor::Parent
    }

    /// A preset by its wire index — its position in [`FLOAT_PRESETS`], which
    /// is what the binary protocol carries and what `KUI_FLOAT_*` counts
    /// from. `None` for an index no preset claims.
    pub fn preset_at(i: usize) -> Option<Self> {
        Some(match i {
            0 => Self::parent(),
            1 => Self::viewport(),
            2 => Self::below(),
            3 => Self::above(),
            _ => return None,
        })
    }

    /// A preset by name. Every binding that spells a float as a word —
    /// `float="below"`, `float = "above"`, `kui_spec_float_preset` — resolves
    /// it here, so "below" cannot mean one thing in JS and another in Lua.
    pub fn preset(name: &str) -> Option<Self> {
        Self::preset_at(FLOAT_PRESETS.iter().position(|p| *p == name)?)
    }

    /// One config from the pieces a binding can extract without deciding
    /// anything: a `base` preset (from [`FloatConfig::preset`] or
    /// [`FloatConfig::preset_at`]) and the overrides that were actually
    /// declared. `None` leaves the base's own value — that is what lets
    /// `float="below"` keep its 6px gap while `{ anchor: "below", dx: 2 }`
    /// moves it sideways without flattening the gap to zero. `fit` and
    /// `clip` are ORed in: no preset sets either.
    pub fn build(
        base: FloatConfig,
        anchor_at: Option<(Align, Align)>,
        self_at: Option<(Align, Align)>,
        dx: Option<f32>,
        dy: Option<f32>,
        fit: bool,
        clip: bool,
    ) -> Self {
        let mut cfg = base;
        if let Some((x, y)) = anchor_at {
            cfg.anchor_point = (x, y);
        }
        if let Some((x, y)) = self_at {
            cfg.self_point = (x, y);
        }
        if let Some(x) = dx {
            cfg.offset.x = x;
        }
        if let Some(y) = dy {
            cfg.offset.y = y;
        }
        cfg.fit |= fit;
        cfg.clip |= clip;
        cfg
    }
}

/// The float preset names, in wire order: the index of a name here is what
/// the binary protocol writes for it and what `KUI_FLOAT_*` counts from.
pub const FLOAT_PRESETS: &[&str] = &["parent", "viewport", "below", "above"];

/// `overflow` as bits: the C struct's field, the binary wire's payload and
/// what the `clip` / `scrollX` / `scrollY` booleans OR together. One set of
/// values so a binding cannot invent its own numbering.
pub const OVERFLOW_CLIP: u32 = 1 << 0;
pub const OVERFLOW_SCROLL_X: u32 = 1 << 1;
pub const OVERFLOW_SCROLL_Y: u32 = 1 << 2;

/// The `pad` shorthand family as declared — any subset of the seven names,
/// each `None` when the frontend did not see it. [`PadShorthand::resolve`]
/// decides what a missing edge falls back to; a binding only reports what
/// it found.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct PadShorthand {
    /// `pad`: all four edges.
    pub all: Option<f32>,
    /// `padX`: left and right.
    pub x: Option<f32>,
    /// `padY`: top and bottom.
    pub y: Option<f32>,
    pub l: Option<f32>,
    pub r: Option<f32>,
    pub t: Option<f32>,
    pub b: Option<f32>,
}

impl PadShorthand {
    /// True when the frontend saw any of the seven names.
    pub fn declared(self) -> bool {
        [self.all, self.x, self.y, self.l, self.r, self.t, self.b]
            .iter()
            .any(Option::is_some)
    }

    /// Four edges: an edge falls back to its axis, an axis to the all-round
    /// `pad`, and `pad` to zero. The specific value always wins.
    pub fn resolve(self) -> Edges {
        let all = self.all.unwrap_or(0.0);
        let (x, y) = (self.x.unwrap_or(all), self.y.unwrap_or(all));
        Edges {
            l: self.l.unwrap_or(x),
            r: self.r.unwrap_or(x),
            t: self.t.unwrap_or(y),
            b: self.b.unwrap_or(y),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LayoutSpec {
    pub width: Sizing,
    pub height: Sizing,
    /// Clamps applied after `width`/`height` resolve (Fit, Grow, Percent and
    /// Fixed alike), so "grow but at most N" and "fit but at least N" work.
    /// A min may also be [`Min::FIT`]: "grow but never below my content".
    pub min_w: Min,
    pub max_w: f32,
    pub min_h: Min,
    pub max_h: f32,
    pub dir: Dir,
    pub padding: Edges,
    pub gap: f32,
    /// Row children that don't fit the main-axis content box start a new
    /// line instead of overflowing (or shrinking). Rows only: breaking
    /// needs a definite main size, and the pass order gives a row one —
    /// its width is final before its height is measured — where a column
    /// would need its height first. Ignored on a column and on a
    /// `scroll_x` row, both with a warning (`diag::WRAP_IGNORED`).
    pub wrap: bool,
    /// A column whose rows' children line up in columns (ADR 0033): the
    /// nth in-flow child of every in-flow row is a cell of column n, and
    /// a column is as wide as its widest cell — its cells' own `width`s
    /// say how the column sizes (a `Fixed` or `Fit` cell is content that
    /// sets the column's fit width, a `Grow` cell makes the column grow,
    /// a `Percent` one takes its cut of the row) and their `minWidth` /
    /// `maxWidth` clamp it. The rows are ordinary rows — a row's `gap` is
    /// the space between its cells, its `padding` its own, and it takes
    /// its background, its click and its hover as any row does — except
    /// that a row of a table never wraps (`diag::WRAP_IGNORED`). Set by
    /// [`NodeSpec::table`], which is a column; `dir` stays `Column`, and
    /// the flag is read on a column only ([`LayoutSpec::is_table`]): a
    /// `Row` carrying it — a shape only these public fields can build —
    /// is the row it says it is. The rows are the in-flow `Row` children;
    /// a column, a table or a leaf straight under the table is a child
    /// with its own width and no cells.
    pub table: bool,
    /// Space between wrap lines, across the main axis. `gap` is still the
    /// space between children along it.
    pub cross_gap: f32,
    /// Alignment of children along the main axis, the spreads
    /// (`SpaceBetween` / `SpaceAround` / `SpaceEvenly`) included.
    pub main_align: Align,
    /// Alignment of children across the main axis; `Baseline` on a row.
    pub cross_align: Align,
    /// Width over height (backlog C14, CSS's `aspect-ratio`); 0 = none.
    /// It sizes the axis whose sizing is `Fit`: a fit height is the final
    /// width over the ratio, and a fit width under a `Fixed` height is
    /// that height times it. With both axes declared it has nothing to set.
    /// The derived axis is neither shrunk nor fitted to the children,
    /// which overflow it — `min_h: Min::FIT` floors it at them.
    pub aspect: f32,
    /// Clip children to this node's rect.
    pub clip: bool,
    /// Overflowing content scrolls (implies clipping). Offsets are retained
    /// across frames in the core, keyed by this node's `Key`.
    pub scroll_x: bool,
    pub scroll_y: bool,
    /// Scroll anchoring (backlog C26 step 3, CSS's `overflow-anchor`): the
    /// first child in view keeps its place on screen when the content
    /// before it changes size — a chat that prepends history, a log that
    /// inserts above the viewport, a row whose estimate was corrected.
    /// On the scroll axis that is the container's main axis only.
    pub anchor: bool,
    /// Out-of-flow positioning; see [`FloatConfig`].
    pub float: Option<FloatConfig>,
}

impl Default for LayoutSpec {
    fn default() -> Self {
        Self {
            width: Sizing::Fit,
            height: Sizing::Fit,
            min_w: Min::px(0.0),
            max_w: f32::INFINITY,
            min_h: Min::px(0.0),
            max_h: f32::INFINITY,
            dir: Dir::Column,
            padding: Edges::default(),
            gap: 0.0,
            wrap: false,
            table: false,
            cross_gap: 0.0,
            main_align: Align::Start,
            cross_align: Align::Start,
            aspect: 0.0,
            clip: false,
            scroll_x: false,
            scroll_y: false,
            anchor: false,
            float: None,
        }
    }
}

impl LayoutSpec {
    /// Whether this node clips its children.
    pub fn clips(&self) -> bool {
        self.clip || self.scroll_x || self.scroll_y
    }

    /// Whether this node is a table (ADR 0033): the flag, on a column.
    /// Every reader — the solver, the diagnostics, `NodeInfo`, the
    /// corpus — asks this and not the field, so a `Row` with the field
    /// set is a row everywhere.
    pub fn is_table(&self) -> bool {
        self.table && self.dir == Dir::Column
    }
}

impl LayoutSpec {
    /// The width a declared aspect ratio gives a `Fit` width: its `Fixed`
    /// height times the ratio. `None` when the ratio has no say over the
    /// width.
    pub(crate) fn aspect_width(&self) -> Option<f32> {
        match (self.width, self.height) {
            (Sizing::Fit, Sizing::Fixed(h)) if self.aspect > 0.0 => {
                Some(self.clamp_h(h) * self.aspect)
            }
            _ => None,
        }
    }

    /// Whether a declared aspect ratio sizes the height: a `Fit` height,
    /// read off the final width. `aspect_width`'s pair.
    pub(crate) fn aspect_height(&self) -> bool {
        self.aspect > 0.0 && self.height == Sizing::Fit
    }

    pub(crate) fn clamp_w(&self, w: f32) -> f32 {
        let min = self.min_w.resolved();
        w.clamp(min, self.max_w.max(min))
    }

    pub(crate) fn clamp_h(&self, h: f32) -> f32 {
        let min = self.min_h.resolved();
        h.clamp(min, self.max_h.max(min))
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct VisualStyle {
    pub bg: Color,
    pub border_color: Color,
    pub border_w: f32,
    /// Corner radii (logical px), clockwise from the top-left:
    /// `[tl, tr, br, bl]`. `NodeSpec::radius` sets all four; the per-corner
    /// builders (`radius_tl`, ...) override one — later calls win, like CSS
    /// `border-radius` followed by `border-top-left-radius`.
    pub radius: [f32; 4],
    /// Group opacity, 0..=1 and 1 by default: multiplied into the alpha of
    /// every quad this node and its subtree emit, and into every
    /// descendant's own opacity. This is the *cheap* group opacity — a
    /// per-quad alpha multiply, not an offscreen composite — so a subtree
    /// whose own pieces overlap shows its seams through the fade where a
    /// compositing implementation would not. Nothing else changes: an
    /// invisible subtree still lays out, still takes clicks and is still
    /// read by assistive technology, exactly like CSS `opacity: 0`.
    pub opacity: f32,
    /// The drop shadow cast behind this node (see [`Shadow`]).
    pub shadow: Shadow,
    /// Paint the background, border, shadow and fragment with each edge on
    /// a whole physical pixel (`pixelSnap`). Off by default: a box is drawn where
    /// layout put it, so a gap between two boxes is there at any scale.
    /// On, its edges land where a text's backgrounds' do and where every
    /// other snapped box's do, so snapped boxes that share an edge in
    /// layout, and a snapped box beside a text's background, meet without
    /// a seam. Layout, hit-testing and the children are untouched.
    pub pixel_snap: bool,
}

impl Default for VisualStyle {
    fn default() -> Self {
        Self {
            bg: Color::TRANSPARENT,
            border_color: Color::TRANSPARENT,
            border_w: 0.0,
            radius: [0.0; 4],
            opacity: 1.0,
            shadow: Shadow::default(),
            pixel_snap: false,
        }
    }
}

/// One outer drop shadow: the node's rounded rect, moved by `dx`/`dy`,
/// grown by `spread` and its edge blurred over `blur`, painted in `color`
/// behind the node. CSS's `box-shadow` without the inset and multi-shadow
/// forms.
///
/// It draws whenever `color` is visible, so `shadowColor` alone is a hard
/// shadow sitting exactly behind the node. The shape is not knocked out of
/// the middle the way CSS knocks it out, so a translucent background shows
/// the shadow through itself.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Shadow {
    pub color: Color,
    /// Offset (logical px); positive `dy` casts downward.
    pub dx: f32,
    pub dy: f32,
    /// Blur radius (logical px): the edge ramps over this distance and the
    /// shadow reaches this far past its shape. 0 = a hard edge.
    pub blur: f32,
    /// Grows (or, negative, shrinks) the shape before blurring.
    pub spread: f32,
}

impl Shadow {
    /// Whether anything would be painted.
    pub fn is_visible(&self) -> bool {
        self.color.is_visible()
    }
}

/// Corner indices into `VisualStyle::radius` / `Quad::radius`.
pub mod corner {
    pub const TL: usize = 0;
    pub const TR: usize = 1;
    pub const BR: usize = 2;
    pub const BL: usize = 3;
}

/// Full per-node configuration. This — not any Rust trait — is the contract
/// every frontend (Rust builders, Lua, serialized UI) lowers into.
#[derive(Clone, Debug, Default)]
pub struct NodeSpec {
    pub layout: LayoutSpec,
    pub style: VisualStyle,
    /// Track pointer hover for this node (`Ui::is_hovered`) without making
    /// it clickable — tooltips on passive badges. Nodes with `on_click` /
    /// `on_key` / `window` are always hover-tracked; clicks on a merely
    /// hoverable node emit nothing.
    pub hoverable: bool,
    /// Ask the driver for another frame after this one, every frame this
    /// node is declared (`animate`). What a `fragment` reading `time`
    /// needs; opt-in, because it takes the loop off input-driven.
    pub animate: bool,
    /// Paint this node's background in the theme's accent
    /// (`docs/adr/0019-a-theme-derived-from-appearance-and-accent.md`) —
    /// the OS's where the host reported one, the app's where it pinned
    /// one, kui's blue otherwise — keeping the declared `bg` only as what
    /// a binding that never sets this row still gets.
    ///
    /// A question, not a colour: a view says *that* this node is the
    /// accented one and the palette says which colour that is. The stock
    /// button takes it further and repaints its hover, its pressed shade
    /// and its label from the same accent, so a light accent still reads
    /// (`crate::widgets::button_with`).
    pub accent: bool,
    /// Window-chrome role (drag handle / window button). A chrome node's
    /// interactions become `WindowCommand`s for the frame driver instead of
    /// `UiEvent`s; `on_click` is ignored on such nodes.
    pub window: Option<WindowRole>,
    /// Eases this node's sizing amounts, colors and radius toward what the
    /// view declares instead of snapping (see [`crate::anim`]). Keyed by
    /// node identity, so the node needs a stable key across frames.
    pub transition: Option<Transition>,
    /// With `transition`: also ease this node's laid-out *position*, moving
    /// its whole subtree — reordered siblings slide into their new slots.
    /// Opt-in because a node whose position follows an already-easing
    /// sibling (a split's second half) would lag twice.
    pub slide: bool,
    /// Reachable by Tab, and focused by a click or an assistive-technology
    /// request, without a click payload or a control role — a list row
    /// that opens on Enter, a card. Controls (editors, key sinks,
    /// `on_click` boxes, the control roles) are focusable already; see
    /// `docs/adr/0002-keyboard-focus-as-data.md`.
    pub focusable: bool,
    /// Where focus lands when the `modal` scope containing this node is
    /// entered: the first node in the modal's Tab ring declaring it,
    /// instead of simply the ring's first — a destructive confirm opening
    /// on its Cancel rather than on whichever control is declared first.
    /// Read on entry only, so a Tab press afterwards stands; a node the
    /// ring skips (disabled, decoration, not focusable) is not a
    /// candidate, and with no candidate the entry is the ring's first
    /// node as before. See `docs/adr/0003-modal-surfaces.md`.
    pub initial_focus: bool,
    /// Inert: keeps its hit region (so a tooltip can say why) and loses
    /// everything else — no click, drag or key sink, no hover / pressed /
    /// focus background, no place in the Tab ring; the access tree
    /// reports it disabled.
    pub disabled: bool,
    /// The pointer shape over this node (see [`crate::cursor`]). Unset,
    /// the pointer is the I-beam over an editor or a selection scope and
    /// the plain arrow over everything else — a clickable or draggable
    /// node included — so a hand over a button, a grab over a handle
    /// (`grabbing` while its drag runs), a splitter's resize arrows and a
    /// disabled control's `notAllowed` are all the view's to declare. The
    /// stock button declares `Pointer` itself.
    pub cursor: Option<CursorShape>,

    /// See [`EventSpec`]. `None` when the node declares none of it.
    pub events: Option<Box<EventSpec>>,

    /// See [`AnimSpec`]. `None` when the node declares none of it.
    pub anim: Option<Box<AnimSpec>>,

    /// See [`AccessSpec`]. `None` when the node declares none of it.
    pub access: Option<Box<AccessSpec>>,

    /// See [`InteractSpec`]. `None` when the node declares none of it.
    pub interact: Option<Box<InteractSpec>>,
}

/// Event payloads a node declares. Boxed on `NodeSpec` because most
/// nodes declare none, and seven `Option<Value>` inline cost 224 bytes
/// on every node built (see C15 in `docs/backlog/closed-2026-09.md`).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct EventSpec {
    /// Payload emitted as a `UiEvent` when this node is clicked.
    pub on_click: Option<Value>,
    /// Makes this node draggable: pressing it starts a pointer-captured
    /// drag, and cursor motion until release emits `UiEvent`s of the form
    /// `{kind="drag", phase="start"|"move"|"end", x, y, dx, dy, tag}` with
    /// this payload merged in under `tag`. A drag past the click slop
    /// suppresses the node's `on_click`, so both can coexist.
    pub on_drag: Option<Value>,
    /// Marks this node as a key sink: while it holds key focus, key
    /// presses arrive as `UiEvent`s on it as `{kind="key", phase="down",
    /// code, ...}`, with this payload merged in under `tag`. Clicking the
    /// node takes key focus. Releases are not delivered unless the sink
    /// also declares [`key_up`](Self::key_up): a keymap is the common
    /// case, and a keymap that heard both halves would run every binding
    /// twice.
    pub on_key: Option<Value>,
    /// With `on_key`: the sink hears releases too, as the same payload
    /// with `phase="up"` (`text` null, `repeat` false). For a held-key
    /// interaction — WASD, press-and-hold to preview, a key that arms a
    /// mode while it is down. A key only comes up where it went down: a
    /// release whose press the sink never got is dropped, and focus
    /// leaving while a key is held delivers the `up` first, so nothing is
    /// left stuck down. Without it a sink hears presses only, which is
    /// what a keymap wants.
    pub key_up: bool,
    /// Asks for a context menu: a secondary-button press over this node
    /// emits `{kind="contextmenu", x, y, tag}` on it with this payload
    /// under `tag`, and does nothing else — the press moves no focus,
    /// places no caret and produces no click, so right-clicking a
    /// selection leaves it selected. `x`/`y` are the press in logical
    /// viewport coordinates, which is where the menu goes. Asked of the
    /// topmost node under the pointer; when that node offers no menu the
    /// press reaches the nearest enclosing node that does, the way an
    /// unclaimed key reaches the enclosing sink (ADR 0011, backlog T1):
    /// the event carries the owner's key and tag, a nested declaration
    /// wins over its ancestor's, a disabled node's own is skipped, and
    /// the walk stops at the modal boundary. Null = the behaviour without
    /// a tag.
    pub on_context_menu: Option<Value>,
    /// Force-click events: a press that deepened past the second stage of
    /// a Force Touch trackpad over this node emits `{kind="forceclick",
    /// x, y, tag}` with this payload under `tag`. Routed as a secondary
    /// press is — no focus moved, no caret, no click — but asked of the
    /// topmost node only, with no walk to an enclosing declaration — and
    /// the ordinary click the press produces still follows,
    /// which is what macOS does (ADR 0017, decision 6).
    ///
    /// Text does not need this: a force click over an editor or a
    /// `selectable` scope selects the word and asks the host to look it
    /// up, which is what the gesture means on that platform. This is for
    /// what the core cannot guess — a force click on a chart, a map, a
    /// timeline.
    pub on_force_click: Option<Value>,
    /// The non-primary buttons as events (backlog F105): a press of a
    /// button in [`buttons`](Self::buttons) over this node emits
    /// `{kind="button", phase="press", button, x, y, clicks, tag}` on it
    /// with this payload under `tag`, and the button is then captured by
    /// the node — every pointer move while it is held arrives as
    /// `phase="move"` and its release as `phase="release"`, on this node
    /// wherever the pointer is. `button` is `"secondary"`, `"middle"` or,
    /// for a button past those, its [`crate::MouseButton::code`]; `x`/`y`
    /// are logical viewport coordinates, and on a `cells` grid the events
    /// carry `cell: {row, col}` as a click does. Several buttons may be
    /// held at once, each its own capture; a primary drag is untouched.
    ///
    /// Asked of the topmost node under the pointer, and when that node
    /// claims no such button the press reaches the nearest enclosing node
    /// that does, as a context menu's does: a disabled node's own is
    /// skipped and the walk stops at the modal boundary. A claimed
    /// secondary press is this event *instead of* a `contextmenu` event
    /// and the stock menu — a nearer `on_context_menu` still wins, being
    /// the nested declaration. Like every non-primary press it moves no
    /// focus, places no caret and touches no selection or scrollbar. What
    /// it is for: a terminal's middle-click paste, and the mouse reports a
    /// program in it asked for. Null = the behaviour without a tag.
    pub on_button: Option<Value>,
    /// Which non-primary buttons [`on_button`](Self::on_button) claims:
    /// all three kinds unless the node says otherwise. A pane that wants
    /// the middle button and leaves the secondary one to its context menu
    /// says [`Buttons::MIDDLE`].
    pub buttons: Buttons,
    /// Scroll events: the wheel over this node emits `{kind="scroll", x,
    /// y, dx, dy, lines, tag}` with this payload under `tag` — the delta
    /// in logical px as the driver reported it (positive `dy` is the wheel
    /// rolling up, toward earlier content), the pointer's position, and
    /// on a `cells` grid the whole lines the delta covers (`null` on any
    /// other node), the fraction carried to the next notch. The node
    /// *takes* the wheel on the axes [`scroll_axes`](Self::scroll_axes)
    /// names: a scroll gesture that starts over it is its own, and stays
    /// its own until it ends wherever the pointer goes (backlog F107); it
    /// reaches no scroll container above it, and a container inside it
    /// still takes the axes it scrolls while it can move that way,
    /// passing this node the rest: the other axis, and a gesture that
    /// begins with the container at its limit (unless it says
    /// [`Overscroll::Contain`]). The core moves nothing — a grid's
    /// `origin_line` and a canvas's zoom are the app's to change. A
    /// drag-select held past a
    /// grid's top or bottom edge arrives here too, as the lines the frame
    /// scrolled by (ADR 0029, decision 4).
    pub on_scroll: Option<Value>,
    /// Which axes [`on_scroll`](Self::on_scroll) takes (backlog F107):
    /// both unless the node says otherwise. A scroll gesture on an axis
    /// the node does not take passes it by, to the scroller around it —
    /// a terminal that scrolls its history on `y` says
    /// [`ScrollAxes::Y`], and a sideways swipe that meets it goes on
    /// moving the strip it sits in. Meaningless without `on_scroll`.
    pub scroll_axes: ScrollAxes,
    /// Hover events: the pointer entering or leaving this node emits
    /// `{kind="hover", phase="enter"|"leave", tag}` with this payload under
    /// `tag` — for hover-dependent *layout* (a close button that appears)
    /// where a color swap isn't enough. Implies hover tracking.
    pub on_hover: Option<Value>,
    /// Drop-zone events (`docs/adr/0031-a-drop-zone-is-a-row-and-the-files-are-an-event.md`):
    /// files dragged in from the OS over this node emit
    /// `{kind="drop", phase="enter"|"move"|"leave"|"drop", paths, x, y,
    /// tag}` with this payload under `tag` — `paths` the OS paths as
    /// strings, `x`/`y` the pointer in viewport coordinates (absent on
    /// `leave`). The zone under the files is the topmost *zone* by paint
    /// order: a node inside a zone resolves to it, and a node that is no
    /// zone and has none enclosing it is looked past, so an overlay shown
    /// on `enter` cannot make the zone lose the files. No `leave` follows
    /// a `drop`. Implies hover tracking.
    pub on_drop: Option<Value>,
    /// Layout events: the rect layout gave this node arrives as
    /// `{kind="layout", x, y, w, h, parent: {x, y, w, h}, tag}` (logical px,
    /// viewport coordinates, after scrolling and position easing) — on the
    /// node's first frame and again whenever the rect changes, never on a
    /// frame that left it alone. The view reads the numbers layout already
    /// produced instead of re-deriving them; a transition that moves the
    /// node reports every frame it moves. Needs a stable key across frames.
    pub on_layout: Option<Value>,
    /// Slider changes (`docs/adr/0034-stock-controls-over-the-roles.md`,
    /// decision 4): on a node whose role is `Slider`, the core turns a
    /// press into the value under the pointer, a drag into the value under
    /// it, the arrows and assistive technology's Increment / Decrement
    /// into one `value_step`, PageUp / PageDown into ten, Home / End into
    /// the range's ends — clamped to `value_min..value_max`, snapped to the
    /// step — and emits `{kind="change", value, phase="move"|"end", tag}`
    /// with this payload under `tag`. The value is proposed, never
    /// applied: nothing moves until the view declares it as `value_now`.
    /// Without it a slider's keys reach the app as the `access` nudge
    /// (ADR 0007, decision 13). Ignored on any other role.
    pub on_change: Option<Value>,
    /// Modal: while this node is declared, the Tab ring is its subtree,
    /// everything outside it is inert to the pointer, the wheel and
    /// assistive technology, and Escape or a press outside emits
    /// `{kind="dismiss", reason, tag}` on it with this payload under
    /// `tag`. The last node declaring it in tree order is the one in
    /// effect (a confirm inside a dialog); see
    /// `docs/adr/0003-modal-surfaces.md`. Null = modal without a tag.
    pub modal: Option<Value>,
    /// A press on this node, or anywhere inside it, leaves keyboard focus
    /// where it was (`keepFocus`): a toolbar button, a tab, a divider
    /// that acts without taking the keyboard from the editor beside it
    /// (backlog DX10). Its click, drag and hover are unchanged, and Tab
    /// and assistive technology still reach a focusable node in it — it
    /// is the pointer's press alone that stops moving focus.
    pub keep_focus: bool,
    /// Keyboard focus entering or leaving this node's subtree — the node
    /// itself or anything focused inside it — emits `{kind="focus",
    /// phase="in"|"out", by, tag}` with this payload under `tag`, where
    /// `by` is `"pointer"`, `"keyboard"`, `"assistive"` or `"program"`:
    /// what moved it (backlog DX18). Reported once the move has settled —
    /// after the input that made it, or at the end of the frame that
    /// declared it — so a view reads a change instead of diffing
    /// `key_focus` every frame. Declares nothing interactive.
    pub on_focus: Option<Value>,
}

impl EventSpec {
    /// The group as a node that declares none of it — what
    /// `NodeSpec`'s accessor hands back when the box is `None`.
    pub const EMPTY: Self = Self {
        on_click: None,
        on_drag: None,
        on_key: None,
        key_up: false,
        on_context_menu: None,
        on_force_click: None,
        on_button: None,
        buttons: Buttons::ALL,
        on_scroll: None,
        scroll_axes: ScrollAxes::Both,
        on_hover: None,
        on_drop: None,
        on_layout: None,
        on_change: None,
        modal: None,
        keep_focus: false,
        on_focus: None,
    };
}

/// Per-node animation declarations. Boxed on `NodeSpec`: `enter` and
/// `exit` are 60 bytes each and almost every node has neither.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct AnimSpec {
    /// CSS-style stops for the animatable slots (see [`crate::keyframes`]):
    /// with a `transition`, the slots a stop names cycle through the stops
    /// over the transition's duration, in its `repeat` direction, offset by
    /// its `delay_ms` — forever, and without the view redrawing. Slots no
    /// stop names still tween toward what the view declares. Empty = none.
    pub keyframes: Vec<Keyframe>,
    /// Where this node's slots start the first frame it is seen (see
    /// [`crate::enter`]): with a `transition`, the slots it names ease in
    /// from there instead of snapping — `dx`/`dy` slide the node in from
    /// that far away, `bg` fades it in. A node that vanishes and returns
    /// enters again. None = first sight snaps.
    pub enter: Option<Enter>,
    /// Where this node's slots *end* the frame after the view stops
    /// declaring it (see [`crate::depart`]). An exit is an [`Enter`] read
    /// the other way: with a `transition`, the departing subtree is copied
    /// out of the last frame that had it and replayed — frozen where
    /// layout left it, on top, inert — while the slots this names ease
    /// from where they were toward what it declares. Needs a stable key
    /// across frames, and a `transition` with a duration; without both, a
    /// removed node vanishes at once as it always did.
    pub exit: Option<Enter>,
}

impl AnimSpec {
    /// The group as a node that declares none of it — what
    /// `NodeSpec`'s accessor hands back when the box is `None`.
    pub const EMPTY: Self = Self {
        keyframes: Vec::new(),
        enter: None,
        exit: None,
    };
}

/// Accessibility properties a view states outright, as opposed to the
/// ones the core derives (see [`crate::access`]). Boxed on `NodeSpec`:
/// read only while an access tree is being built, and unset on nearly
/// every node.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct AccessSpec {
    /// What this node is to assistive technology (see [`crate::access`]).
    /// Unset, the core derives one: a node with `on_click` is a button,
    /// an editor a text input, a scrolling container a scroll view, and
    /// a plain box is structure that leaves no trace. `Role::None` hides
    /// the node and its subtree from the access tree (decoration).
    pub role: Option<Role>,
    /// The accessible name. Without one a button, link, tab or heading is
    /// named by the text inside it; an image or an icon button has no
    /// name at all, and the core says so (`control-without-name`,
    /// `image-without-label` warnings).
    pub label: Option<Label>,
    /// The accessible description, read after the name — what the
    /// `description` prop sets, and what the `tooltip` prop sets on the way
    /// to drawing the same string.
    pub description: Option<Label>,
    /// For checkbox / radio / switch roles: the on state.
    pub checked: bool,
    /// For a checkbox: neither on nor off — the select-all box over a
    /// list some of whose rows are selected (ADR 0034, decision 3). Wins
    /// over `checked`, which it leaves as it was.
    pub mixed: bool,
    /// The current one of a set: a `Role::Tab`, a picked `Role::ListItem`,
    /// the `Role::Link` for the page you are on. A tab reports the state
    /// either way; a row or a link reports it only where it is set (see
    /// [`crate::access`]).
    pub selected: bool,
    /// For a node that shows and hides something: expanded, or collapsed.
    /// None = it does not expand, and says nothing about it.
    pub expanded: Option<bool>,
    /// For a slider role: the current value and its range, so assistive
    /// technology can read the position (the drawing stays the view's).
    pub value_now: Option<f32>,
    pub value_min: Option<f32>,
    pub value_max: Option<f32>,
    /// For a slider role: what the position *reads as* (ARIA's
    /// `aria-valuetext`). Without one a reader has only the three numbers
    /// above and says a percentage — 25 in [5..60] is "36 percent" — so a
    /// value whose unit matters says it here: "25 minutes". It replaces
    /// the number rather than joining it (see [`crate::access`]), and a
    /// nudge announces the new text, not the new number
    /// (`docs/adr/0008-live-regions-and-announcements.md`).
    pub value_text: Option<Label>,
    /// For a slider role: how far one arrow key moves it, and the grid a
    /// value set by the pointer snaps to (ADR 0034, decision 4). None =
    /// a hundredth of the range.
    pub value_step: Option<f32>,
    /// On a `Role::Line` of a custom editor: the caret's byte offset into
    /// the line's text, and the byte offset of the selection's other end
    /// (see [`crate::access`]).
    pub caret: Option<u32>,
    pub selection_anchor: Option<u32>,
    /// With `caret`: the caret this line declares does not blink — a
    /// block caret in a modal editor's normal mode — so a driver's blink
    /// clock is not armed on it (`Core::has_caret` leaves it out) while
    /// the offset still anchors the IME and reads to assistive
    /// technology. Without it a declared caret is a caret to blink.
    pub caret_solid: bool,
    /// Float `description` below the node while it is hovered — the
    /// `tooltip` prop's third effect, set by [`NodeSpec::tooltip`]. The core
    /// reads it as the node opens (`Core::hint`); a binding that lowers the
    /// prop floats the hint itself and leaves this off.
    pub tooltip: bool,
    /// When the text inside this node changes, a reader reads the change
    /// without being asked (ARIA's `aria-live`). Off by default; a node
    /// that declares it is semantic, so a plain box marked live is not
    /// elided (see `docs/adr/0008-live-regions-and-announcements.md`).
    pub live: Live,
}

impl AccessSpec {
    /// The group as a node that declares none of it — what
    /// `NodeSpec`'s accessor hands back when the box is `None`.
    pub const EMPTY: Self = Self {
        role: None,
        label: None,
        description: None,
        checked: false,
        mixed: false,
        selected: false,
        expanded: None,
        value_now: None,
        value_min: None,
        value_max: None,
        value_text: None,
        value_step: None,
        caret: None,
        selection_anchor: None,
        caret_solid: false,
        tooltip: false,
        live: Live::Off,
    };
}

/// Hover / pressed / focus styling and the sounds that go with them.
/// Boxed on `NodeSpec` so the common node — which declares none of it —
/// costs one null check in `resolve_hover_style` instead of reading
/// several `Option`s spread across the struct.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct InteractSpec {
    /// Background while the pointer hovers this node (or any node sharing
    /// its `hover_group`). Resolved by the core when the node opens, so a
    /// data-only view gets hover styling without querying `is_hovered` —
    /// and with `transition` the swap eases. Implies hover tracking.
    pub hover_bg: Option<Color>,
    /// Background while this node (or its group) is pressed. Implies hover
    /// tracking. Without a `hover_bg`, hover keeps the plain `bg`. An
    /// `on_drag` node holds this state for the whole captured drag, even
    /// while the cursor is off it.
    pub pressed_bg: Option<Color>,
    /// Hover group: nodes sharing an id count as one for `hover_bg` /
    /// `pressed_bg` — a two-piece elbow, a split button, a row whose cells
    /// highlight together. The id is a hash of a name (`hover_group`).
    /// Implies hover tracking.
    pub hover_group: Option<u64>,
    /// A registered sound played when this node is clicked (see
    /// [`crate::audio`]); the click itself still emits `on_click` if one is
    /// declared. Implies hover tracking.
    pub click_sound: Option<crate::resources::SoundId>,
    /// A registered sound played when the pointer enters this node.
    /// Implies hover tracking.
    pub hover_sound: Option<crate::resources::SoundId>,
    /// Background while this node holds keyboard-visible focus (focus
    /// moved by Tab or by assistive technology, not by a click).
    /// Declaring one replaces the ring the core draws by default. Pressed
    /// wins over focus wins over hover; eases with `transition`.
    pub focus_bg: Option<Color>,
    /// Background while files dragged in from the OS are over this node
    /// (ADR 0031, decision 3). Wins over pressed, focus and hover — a press
    /// cannot be held while the OS holds a drag — and eases with
    /// `transition`; clears when the files leave, land or the drag is
    /// cancelled. Implies hover tracking.
    pub drop_bg: Option<Color>,
    /// Makes this node a *selection scope*: the text of every node inside
    /// it is one selectable run of text, in tree order, and a press-drag
    /// inside it selects across all of them (see
    /// `docs/adr/0017-selection-as-a-scope.md`). Declared on the container
    /// rather than on each label, because what a reader selects is a
    /// paragraph or a card, not one run of it.
    ///
    /// Scopes do not nest: the innermost one containing a run owns it, and
    /// an outer one is warned about (`nested-selection-scope`). An `edit`
    /// is already its own scope and ignores this.
    pub selectable: bool,
    /// Makes this node's subtree a *focus region*: a Tab ring of its own
    /// that the ring outside never enters, and that never leaves — a
    /// devtools dock, an inspector beside the app (see
    /// `docs/adr/0022-focus-regions.md`). Entered on purpose:
    /// `Ui::focus_region`, a press inside it, or an explicit focus on a
    /// node in it. Nothing else about the node changes — it lays out,
    /// paints, takes the pointer and appears in the access tree as before,
    /// and keys bubble through it to the sink above.
    pub focus_region: bool,
    /// How this node's scrollbars look, when it scrolls (see
    /// [`Scrollbar`]). Cold: read once per scroller when its layer's
    /// chrome is emitted, which is why it lives in this box rather than
    /// beside `scroll_y` on every node.
    pub scrollbar: Scrollbar,
    /// Whether a scroll gesture that starts over this scroller while it
    /// is at its limit that way goes on to the scroller around it (see
    /// [`Overscroll`], backlog F107). Cold like `scrollbar`: read once
    /// per scroller a gesture starts over.
    pub overscroll: Overscroll,
    /// On a table (ADR 0033): lines of this colour between its columns and
    /// between its rows, drawn with the table's own box, under its cells,
    /// down the middle of each gap — so a table with a `gap` of at least
    /// `rule_w` gets a grid with no rule cells (backlog DX21). The
    /// columns come from the row with the most cells; the lines run the
    /// table's content box. Ignored on anything that is not a table.
    pub rules: Option<Color>,
    /// The rules' width in logical px; 0 is 1.
    pub rule_w: f32,
}

/// A scrolling node's bars, per node. Every field's default is the stock
/// bar — the theme's `scrollbar` / `scrollbar_active` colours, 4 px at
/// rest and 6 px under the pointer, always drawn while the content
/// overflows — so a binding that sets none of the four rows gets exactly
/// what it always had. The bars are overlays and take no layout space
/// whatever their width; the grabbable track is at least as wide as the
/// active thumb plus its inset.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Scrollbar {
    /// Whether the bars are drawn at all, and when (see [`ScrollbarMode`]).
    pub mode: ScrollbarMode,
    /// The thumb's width at rest, logical px; under the pointer or dragged
    /// it is 2 px wider. `None` is the stock 4.
    pub width: Option<f32>,
    /// The thumb at rest; `None` is `theme.scrollbar`.
    pub color: Option<Color>,
    /// The thumb under the pointer or dragged; `None` is
    /// `theme.scrollbar_active`.
    pub active_color: Option<Color>,
}

impl Scrollbar {
    pub const DEFAULT: Self = Self {
        mode: ScrollbarMode::Visible,
        width: None,
        color: None,
        active_color: None,
    };
}

/// When a scrolling node's bars are drawn. Spelled by the `scrollbar` row
/// (`crate::schema::SCROLLBARS`, in this order).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ScrollbarMode {
    /// The stock bar: drawn whenever the content overflows.
    #[default]
    Visible,
    /// No thumb is drawn and no track takes the press. The wheel, the
    /// keyboard, `reveal` and the caret still scroll the node — a list
    /// drawing its own indicator, or one whose bar would sit on a border.
    Hidden,
    /// The bar shows while the scroll state is changing — the offset or
    /// the content's extent moved since the last frame, the pointer is on
    /// its track, or a thumb is being dragged — and for a second after,
    /// then fades out over a quarter of one. A node first seen shows its
    /// bar the same second. What an overlay bar does on macOS. Needs the
    /// driver's clock (`Core::set_time`); without one it is `Visible`,
    /// since a fade with no clock could never end.
    Auto,
}

impl ScrollbarMode {
    /// Every mode, in the `scrollbar` row's order: what a binding that
    /// spells modes as numbers indexes (C's `KUI_SCROLLBAR_*` are these
    /// plus one).
    pub const ALL: [ScrollbarMode; 3] = [
        ScrollbarMode::Visible,
        ScrollbarMode::Hidden,
        ScrollbarMode::Auto,
    ];
}

/// What a scroll gesture that starts over a scroller already at its limit
/// does (backlog F107) — CSS's `overscroll-behavior`, spelled by the
/// `overscroll` row (`crate::schema::OVERSCROLLS`, in this order).
///
/// A gesture picks its target when it starts: the innermost scroller
/// under the pointer that can still move the way it goes. One at its
/// limit is passed by, to the scroller around it — *chaining* — and
/// `Contain` stops that: the gesture is this scroller's, and moves
/// nothing until it turns back. Only on the axes the node scrolls, so a
/// `scroll_y` list that contains still passes a sideways swipe to the
/// strip it sits in (DX13), where CSS would stop that too. Decided at
/// the start only: a gesture that reaches a limit midway stops there
/// whatever this says, as a browser's does.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Overscroll {
    /// The gesture goes on to the scroller around this one when this one
    /// is at its limit that way.
    #[default]
    Auto,
    /// A gesture that starts here stays here: a panel, a popup's list or
    /// a sheet whose scrolling must never move the page behind it.
    Contain,
}

impl Overscroll {
    /// Every value, in the `overscroll` row's order (C's
    /// `KUI_OVERSCROLL_*` are these plus one).
    pub const ALL: [Overscroll; 2] = [Overscroll::Auto, Overscroll::Contain];
}

/// Which axes an `on_scroll` node takes (backlog F107), spelled by the
/// `scrollAxes` row (`crate::schema::SCROLL_AXES`, in this order).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ScrollAxes {
    /// Both: the node hears every scroll gesture that starts over it.
    #[default]
    Both,
    /// Only sideways; a vertical gesture passes it by.
    X,
    /// Only vertical; a sideways gesture passes it by — a terminal's
    /// history, a log.
    Y,
}

impl ScrollAxes {
    /// Every value, in the `scrollAxes` row's order (C's
    /// `KUI_SCROLL_AXES_*` are these plus one).
    pub const ALL: [ScrollAxes; 3] = [ScrollAxes::Both, ScrollAxes::X, ScrollAxes::Y];

    /// Whether the node takes `x` (true) or `y` (false).
    pub fn takes(self, x: bool) -> bool {
        match self {
            ScrollAxes::Both => true,
            ScrollAxes::X => x,
            ScrollAxes::Y => !x,
        }
    }
}

impl InteractSpec {
    /// The group as a node that declares none of it — what
    /// `NodeSpec`'s accessor hands back when the box is `None`.
    pub const EMPTY: Self = Self {
        hover_bg: None,
        pressed_bg: None,
        hover_group: None,
        click_sound: None,
        hover_sound: None,
        focus_bg: None,
        drop_bg: None,
        selectable: false,
        focus_region: false,
        scrollbar: Scrollbar::DEFAULT,
        overscroll: Overscroll::Auto,
        rules: None,
        rule_w: 0.0,
    };
}

impl NodeSpec {
    // -- Boxed groups ------------------------------------------------------
    // The four cold groups are behind a pointer each (see C15): a node that
    // declares none of a group pays 8 bytes for it rather than its full
    // width. Reads go through the `&` accessor, which hands back a shared
    // empty group instead of allocating, so a caller reads
    // `spec.events().on_click` exactly as it used to read `spec.on_click`.
    // Writes go through the `_mut` accessor, which allocates on first use.

    /// Event payloads this node declares, empty if it declares none.
    #[inline]
    pub fn events(&self) -> &EventSpec {
        static EMPTY: EventSpec = EventSpec::EMPTY;
        self.events.as_deref().unwrap_or(&EMPTY)
    }

    /// Allocates the group on first write.
    #[inline]
    pub fn events_mut(&mut self) -> &mut EventSpec {
        self.events.get_or_insert_with(Box::default)
    }

    /// Animation declarations, empty if this node has none.
    #[inline]
    pub fn anim(&self) -> &AnimSpec {
        static EMPTY: AnimSpec = AnimSpec::EMPTY;
        self.anim.as_deref().unwrap_or(&EMPTY)
    }

    /// Allocates the group on first write.
    #[inline]
    pub fn anim_mut(&mut self) -> &mut AnimSpec {
        self.anim.get_or_insert_with(Box::default)
    }

    /// Declared accessibility properties, empty if this node declares none.
    #[inline]
    pub fn access(&self) -> &AccessSpec {
        static EMPTY: AccessSpec = AccessSpec::EMPTY;
        self.access.as_deref().unwrap_or(&EMPTY)
    }

    /// Allocates the group on first write.
    #[inline]
    pub fn access_mut(&mut self) -> &mut AccessSpec {
        self.access.get_or_insert_with(Box::default)
    }

    /// Hover / pressed / focus styling, empty if this node declares none.
    #[inline]
    pub fn interact(&self) -> &InteractSpec {
        static EMPTY: InteractSpec = InteractSpec::EMPTY;
        self.interact.as_deref().unwrap_or(&EMPTY)
    }

    /// Allocates the group on first write.
    #[inline]
    pub fn interact_mut(&mut self) -> &mut InteractSpec {
        self.interact.get_or_insert_with(Box::default)
    }

    /// Whether the core registers a hit region for this node (any of the
    /// interaction props, or an explicit `hoverable`).
    pub fn hover_tracked(&self) -> bool {
        // Asked of every node at emission; the inline flags first, then
        // each boxed group once — a node that declares neither group is
        // answered by two null checks rather than a read per field (C15).
        self.hoverable
            || self.focusable
            || self.window.is_some()
            // A `cursor` override has to be found under the pointer to be
            // read, even on an otherwise inert box.
            || self.cursor.is_some()
            || self.events.as_deref().is_some_and(|e| {
                // A modal's own background is not "outside" it: a press
                // there must find a region (see
                // `docs/adr/0003-modal-surfaces.md`).
                e.modal.is_some()
                    || e.on_click.is_some()
                    || e.on_drag.is_some()
                    || e.on_key.is_some()
                    || e.on_context_menu.is_some()
                    || e.on_force_click.is_some()
                    || e.on_button.is_some()
                    || e.on_hover.is_some()
                    || e.on_drop.is_some()
                    || e.on_change.is_some()
            })
            || self.interact.as_deref().is_some_and(|i| {
                i.hover_bg.is_some()
                    || i.pressed_bg.is_some()
                    || i.drop_bg.is_some()
                    || i.hover_group.is_some()
                    || i.click_sound.is_some()
                    || i.hover_sound.is_some()
                    // A selection scope has to be found under the pointer:
                    // the press that starts a drag-select lands on it.
                    || i.selectable
                    // So does a focus region: a press on its dead space
                    // settles the ring there (`docs/adr/0022`, decision 3).
                    || i.focus_region
            })
    }

    /// The group id `hover_group(name)` assigns — reproducible from any
    /// binding (the same FNV mix as `Key`).
    pub fn hover_group_id(name: &str) -> u64 {
        crate::key::Key::ROOT.str(name).0
    }

    pub fn row() -> Self {
        Self {
            layout: LayoutSpec {
                dir: Dir::Row,
                ..Default::default()
            },
            ..Default::default()
        }
    }

    pub fn column() -> Self {
        Self {
            layout: LayoutSpec {
                dir: Dir::Column,
                ..Default::default()
            },
            ..Default::default()
        }
    }

    /// A table (ADR 0033): a column whose rows' children line up in
    /// columns. Its in-flow children are the rows and each row's in-flow
    /// children its cells; the nth cell of every row is column n, and a
    /// column is as wide as its widest cell — so a label column sits at
    /// its longest label with nothing measured and no width picked by
    /// hand. A cell's `width` says how its column sizes: `Fit` (the
    /// default) and `Fixed` are content the column's fit width is the
    /// max of, `Grow` makes the whole column grow with the table, and a
    /// column's `minWidth` / `maxWidth` are the strictest its cells
    /// declared. The rows are the `Row` children — give them `width:
    /// grow` for the columns to grow into; a `Fit` row sits at the
    /// columns' fit width — with their own `gap` between cells, their own
    /// padding, background, click and hover; a row of a table never
    /// wraps. A bare text is a cell too, kept at its column's width, so
    /// `ui.text` straight inside a row is a column; an image straight in
    /// a row is a cell the same way, its box the column wide and its own
    /// aspect tall. A text, a column or a table straight under the table
    /// is a child with its own width and no cells. The table's own `Fit`
    /// width is its columns', whatever the rows' sizing, and a `scroll_x`
    /// table's rows are at least as wide as its columns. Everything else
    /// is a column's: `gap` is the space between rows, `scrollY` scrolls
    /// them.
    pub fn table() -> Self {
        Self {
            layout: LayoutSpec {
                dir: Dir::Column,
                table: true,
                ..Default::default()
            },
            ..Default::default()
        }
    }

    /// A [`Sizing`], or a number of px (`.width(120.0)`).
    #[inline]
    pub fn width(mut self, s: impl Into<Sizing>) -> Self {
        self.layout.width = s.into();
        self
    }

    /// A [`Sizing`], or a number of px (`.height(24.0)`).
    #[inline]
    pub fn height(mut self, s: impl Into<Sizing>) -> Self {
        self.layout.height = s.into();
        self
    }

    /// Both axes at once: `.size(20.0, 20.0)` for a fixed box,
    /// `.size(Sizing::GROW, 24.0)` for a strip.
    #[inline]
    pub fn size(self, w: impl Into<Sizing>, h: impl Into<Sizing>) -> Self {
        self.width(w).height(h)
    }

    /// An equal share of the leftover width: `.width(Sizing::GROW)`.
    #[inline]
    pub fn grow_width(self) -> Self {
        self.width(Sizing::GROW)
    }

    /// An equal share of the leftover height: `.height(Sizing::GROW)`.
    #[inline]
    pub fn grow_height(self) -> Self {
        self.height(Sizing::GROW)
    }

    /// Grow along both axes.
    #[inline]
    pub fn fill(self) -> Self {
        self.grow_width().grow_height()
    }

    /// A number of px, or [`Min::FIT`] for the node's own fit width.
    pub fn min_width(mut self, v: impl Into<Min>) -> Self {
        self.layout.min_w = v.into();
        self
    }

    pub fn max_width(mut self, v: f32) -> Self {
        self.layout.max_w = v;
        self
    }

    /// A number of px, or [`Min::FIT`] for the node's own fit height.
    pub fn min_height(mut self, v: impl Into<Min>) -> Self {
        self.layout.min_h = v.into();
        self
    }

    /// Clip children to this node's rect without scrolling. A `radius` on
    /// the same node rounds the clip, so children stay inside its corners
    /// (see `display::Clip`).
    pub fn clip(mut self) -> Self {
        self.layout.clip = true;
        self
    }

    /// Vertical scrolling (and clipping) for overflowing content.
    pub fn scroll_y(mut self) -> Self {
        self.layout.scroll_y = true;
        self
    }

    /// Horizontal scrolling (and clipping) for overflowing content.
    pub fn scroll_x(mut self) -> Self {
        self.layout.scroll_x = true;
        self
    }

    /// Scroll anchoring on this container (see [`LayoutSpec::anchor`]):
    /// the first child in view stays where it is on screen when the
    /// content before it changes size. Needs `scroll_y` on a column or
    /// `scroll_x` on a row.
    pub fn anchor(mut self) -> Self {
        self.layout.anchor = true;
        self
    }

    /// Take this node out of flex flow; see [`FloatConfig`].
    pub fn float(mut self, cfg: FloatConfig) -> Self {
        self.layout.float = Some(cfg);
        self
    }

    /// Apply the overflow bits ([`OVERFLOW_CLIP`] and friends). What a bit
    /// means is decided here: a binding ORs together whatever its own
    /// surface spells (`clip`, `scrollX`, `overflow`) and hands the number
    /// over, rather than each one re-deciding that scrolling clips too.
    pub fn overflow_bits(mut self, bits: u32) -> Self {
        if bits & OVERFLOW_CLIP != 0 {
            self = self.clip();
        }
        if bits & OVERFLOW_SCROLL_X != 0 {
            self = self.scroll_x();
        }
        if bits & OVERFLOW_SCROLL_Y != 0 {
            self = self.scroll_y();
        }
        self
    }

    /// The `tooltip` prop whole, for a Rust view: the node tracks hover,
    /// the hint is its accessible description, and the core floats the
    /// hint below it while it is hovered (`widgets::hover_hint`, the float
    /// every binding's tooltip is). What `tooltip="…"` is in JSX and Lua
    /// and `KuiSpec.tooltip` in C.
    #[inline]
    pub fn tooltip(self, hint: &str) -> Self {
        let mut spec = self.apply_tooltip(hint);
        spec.access_mut().tooltip = true;
        spec
    }

    /// The spec half of the `tooltip` prop: the hint is hover-gated, so the
    /// node tracks hover, and it is what assistive technology should say, so
    /// it is the accessible description too — and nothing floats. For a
    /// caller that floats the hint itself: [`crate::schema::PropsOut::apply_tooltip`]
    /// for the parsers, `kui_close` for C, a widget that draws its own. A
    /// Rust view wants [`Self::tooltip`], which is all three.
    pub fn apply_tooltip(self, hint: &str) -> Self {
        self.hoverable().description(hint)
    }

    pub fn max_height(mut self, v: f32) -> Self {
        self.layout.max_h = v;
        self
    }

    pub fn pad(mut self, v: f32) -> Self {
        self.layout.padding = Edges::all(v);
        self
    }

    pub fn pad_xy(mut self, x: f32, y: f32) -> Self {
        self.layout.padding = Edges::xy(x, y);
        self
    }

    pub fn padding(mut self, e: Edges) -> Self {
        self.layout.padding = e;
        self
    }

    pub fn gap(mut self, v: f32) -> Self {
        self.layout.gap = v;
        self
    }

    /// Wrap overflowing children onto more lines; see [`LayoutSpec::wrap`].
    pub fn wrap(mut self) -> Self {
        self.layout.wrap = true;
        self
    }

    /// Space between wrap lines (across the main axis).
    pub fn cross_gap(mut self, v: f32) -> Self {
        self.layout.cross_gap = v;
        self
    }

    pub fn main_align(mut self, a: Align) -> Self {
        self.layout.main_align = a;
        self
    }

    pub fn cross_align(mut self, a: Align) -> Self {
        self.layout.cross_align = a;
        self
    }

    /// Width over height (CSS's `aspect-ratio`): see
    /// [`LayoutSpec::aspect`]. `16.0 / 9.0` for a video, `1.0` for a
    /// square. A ratio that is not positive and finite clears it.
    pub fn aspect_ratio(mut self, ratio: f32) -> Self {
        self.layout.aspect = if ratio.is_finite() && ratio > 0.0 {
            ratio
        } else {
            0.0
        };
        self
    }

    /// Center children on both axes.
    pub fn center(self) -> Self {
        self.main_align(Align::Center).cross_align(Align::Center)
    }

    pub fn bg(mut self, c: Color) -> Self {
        self.style.bg = c;
        self
    }

    /// Rounds all four corners by `r`. On a node that also clips or
    /// scrolls it rounds the clip too, so its children are cut to the same
    /// corners rather than poking square out of them.
    pub fn radius(mut self, r: f32) -> Self {
        self.style.radius = [r; 4];
        self
    }

    /// Per-corner radii, clockwise from the top-left.
    pub fn radii(mut self, tl: f32, tr: f32, br: f32, bl: f32) -> Self {
        self.style.radius = [tl, tr, br, bl];
        self
    }

    pub fn radius_tl(mut self, r: f32) -> Self {
        self.style.radius[corner::TL] = r;
        self
    }

    pub fn radius_tr(mut self, r: f32) -> Self {
        self.style.radius[corner::TR] = r;
        self
    }

    pub fn radius_br(mut self, r: f32) -> Self {
        self.style.radius[corner::BR] = r;
        self
    }

    pub fn radius_bl(mut self, r: f32) -> Self {
        self.style.radius[corner::BL] = r;
        self
    }

    /// Rounds the two top corners (tabs, headers).
    pub fn radius_top(self, r: f32) -> Self {
        self.radius_tl(r).radius_tr(r)
    }

    /// Rounds the two bottom corners.
    pub fn radius_bottom(self, r: f32) -> Self {
        self.radius_br(r).radius_bl(r)
    }

    pub fn border(mut self, w: f32, c: Color) -> Self {
        self.style.border_w = w;
        self.style.border_color = c;
        self
    }

    /// Paints this node's background, border, shadow and fragment on whole pixels
    /// (see `VisualStyle::pixel_snap`).
    pub fn pixel_snap(mut self) -> Self {
        self.style.pixel_snap = true;
        self
    }

    /// Fades this node and everything under it (see `VisualStyle::opacity`);
    /// clamped to 0..=1.
    pub fn opacity(mut self, o: f32) -> Self {
        self.style.opacity = o.clamp(0.0, 1.0);
        self
    }

    /// The whole drop shadow at once: color, offset, blur, spread.
    pub fn shadow(mut self, shadow: Shadow) -> Self {
        self.style.shadow = shadow;
        self
    }

    /// The shadow's color — on its own, a hard shadow exactly behind the
    /// node. Nothing else about a shadow draws without it.
    pub fn shadow_color(mut self, c: Color) -> Self {
        self.style.shadow.color = c;
        self
    }

    pub fn shadow_blur(mut self, blur: f32) -> Self {
        self.style.shadow.blur = blur;
        self
    }

    pub fn shadow_offset(mut self, dx: f32, dy: f32) -> Self {
        self.style.shadow.dx = dx;
        self.style.shadow.dy = dy;
        self
    }

    pub fn shadow_x(mut self, dx: f32) -> Self {
        self.style.shadow.dx = dx;
        self
    }

    pub fn shadow_y(mut self, dy: f32) -> Self {
        self.style.shadow.dy = dy;
        self
    }

    pub fn shadow_spread(mut self, spread: f32) -> Self {
        self.style.shadow.spread = spread;
        self
    }

    /// Hover-track this node without making it clickable; see the
    /// `hoverable` field.
    pub fn hoverable(mut self) -> Self {
        self.hoverable = true;
        self
    }

    /// Ask for another frame after this one, for as long as this node is
    /// declared. See the `animate` prop.
    pub fn animate(mut self) -> Self {
        self.animate = true;
        self
    }

    /// Background from the OS accent colour where the host knows it; see
    /// the field.
    pub fn accent(mut self) -> Self {
        self.accent = true;
        self
    }

    pub fn on_click(mut self, payload: impl Into<Value>) -> Self {
        self.events_mut().on_click = Some(payload.into());
        self
    }

    /// Background while hovered (see the `hover_bg` field).
    pub fn hover_bg(mut self, c: Color) -> Self {
        self.interact_mut().hover_bg = Some(c);
        self
    }

    /// Background while dragged files are over this node (see the
    /// `drop_bg` field, ADR 0031).
    pub fn drop_bg(mut self, c: Color) -> Self {
        self.interact_mut().drop_bg = Some(c);
        self
    }

    /// A table's grid rules in `c` (see the `rules` field).
    pub fn rules(mut self, c: Color) -> Self {
        self.interact_mut().rules = Some(c);
        self
    }

    /// The rules' width in logical px; 1 when unset (see `rules`).
    pub fn rule_width(mut self, w: f32) -> Self {
        self.interact_mut().rule_w = w;
        self
    }

    /// Makes this node a selection scope (see the `selectable` field):
    /// the text inside it becomes one selectable run.
    pub fn selectable(mut self) -> Self {
        self.interact_mut().selectable = true;
        self
    }

    /// Makes this node's subtree a focus region (see the `focus_region`
    /// field): a Tab ring of its own, entered on purpose.
    pub fn focus_region(mut self) -> Self {
        self.interact_mut().focus_region = true;
        self
    }

    /// Whether a scroll gesture starting over this scroller at its limit
    /// goes on to the one around it (see [`Overscroll`]):
    /// `Overscroll::Contain` keeps it here.
    pub fn overscroll(mut self, o: Overscroll) -> Self {
        self.interact_mut().overscroll = o;
        self
    }

    /// When this node's scrollbars are drawn (see [`ScrollbarMode`]).
    /// Scrolling itself is unchanged whatever the mode.
    pub fn scrollbar(mut self, mode: ScrollbarMode) -> Self {
        self.interact_mut().scrollbar.mode = mode;
        self
    }

    /// The thumb's width at rest, logical px (see [`Scrollbar::width`]).
    pub fn scrollbar_width(mut self, w: f32) -> Self {
        self.interact_mut().scrollbar.width = Some(w);
        self
    }

    /// The thumb's colour at rest (see [`Scrollbar::color`]).
    pub fn scrollbar_color(mut self, c: Color) -> Self {
        self.interact_mut().scrollbar.color = Some(c);
        self
    }

    /// The thumb's colour under the pointer or dragged (see
    /// [`Scrollbar::active_color`]).
    pub fn scrollbar_active_color(mut self, c: Color) -> Self {
        self.interact_mut().scrollbar.active_color = Some(c);
        self
    }

    /// Background while pressed (see the `pressed_bg` field).
    pub fn pressed_bg(mut self, c: Color) -> Self {
        self.interact_mut().pressed_bg = Some(c);
        self
    }

    /// Background while keyboard-visibly focused (see the `focus_bg`
    /// field); replaces the default focus ring.
    pub fn focus_bg(mut self, c: Color) -> Self {
        self.interact_mut().focus_bg = Some(c);
        self
    }

    /// Puts this node in the Tab ring (see the `focusable` field).
    pub fn focusable(mut self) -> Self {
        self.focusable = true;
        self
    }

    /// Hears focus entering and leaving this subtree (see the `on_focus`
    /// field).
    pub fn on_focus(mut self, tag: impl Into<Value>) -> Self {
        self.events_mut().on_focus = Some(tag.into());
        self
    }

    /// A press here leaves keyboard focus where it was (see the
    /// `keep_focus` field).
    pub fn keep_focus(mut self) -> Self {
        self.events_mut().keep_focus = true;
        self
    }

    /// Makes this node the modal scope's entry focus (see the
    /// `initial_focus` field).
    pub fn initial_focus(mut self) -> Self {
        self.initial_focus = true;
        self
    }

    /// Makes this node inert (see the `disabled` field).
    pub fn disabled(mut self, disabled: bool) -> Self {
        self.disabled = disabled;
        self
    }

    /// Makes this node the frame's modal surface (see the `modal` field).
    #[inline]
    pub fn modal(mut self, tag: impl Into<Value>) -> Self {
        self.events_mut().modal = Some(tag.into());
        self
    }

    /// Joins the hover group `name` (see the `hover_group` field).
    pub fn hover_group(mut self, name: &str) -> Self {
        self.interact_mut().hover_group = Some(Self::hover_group_id(name));
        self
    }

    /// Emits enter/leave events for this node (see the `on_hover` field).
    /// Pass a tag the handler can match on; `Value::Null` if the node key
    /// is identification enough.
    pub fn on_hover(mut self, tag: impl Into<Value>) -> Self {
        self.events_mut().on_hover = Some(tag.into());
        self
    }

    /// Makes this node a drop zone for files dragged in from the OS (see
    /// the `on_drop` field, ADR 0031).
    pub fn on_drop(mut self, tag: impl Into<Value>) -> Self {
        self.events_mut().on_drop = Some(tag.into());
        self
    }

    /// Plays a registered sound when this node is clicked (see the
    /// `click_sound` field).
    pub fn click_sound(mut self, sound: crate::resources::SoundId) -> Self {
        self.interact_mut().click_sound = Some(sound);
        self
    }

    /// Plays a registered sound when the pointer enters this node (see the
    /// `hover_sound` field).
    pub fn hover_sound(mut self, sound: crate::resources::SoundId) -> Self {
        self.interact_mut().hover_sound = Some(sound);
        self
    }

    /// Makes this node draggable (see the `on_drag` field). Pass a tag the
    /// handler can match on; `Value::Null` if the node key is enough.
    pub fn on_drag(mut self, tag: impl Into<Value>) -> Self {
        self.events_mut().on_drag = Some(tag.into());
        self
    }

    /// Reports this node's laid-out rect as an event whenever it changes
    /// (see the `on_layout` field). Pass a tag the handler can match on;
    /// `Value::Null` if the node key is identification enough.
    pub fn on_layout(mut self, tag: impl Into<Value>) -> Self {
        self.events_mut().on_layout = Some(tag.into());
        self
    }

    /// What this node is to assistive technology (see the `role` field).
    pub fn role(mut self, role: Role) -> Self {
        self.access_mut().role = Some(role);
        self
    }

    /// The accessible name (see the `label` field). Takes an `Arc<str>`
    /// as well as a `&str`, so a view can keep one and hand it out every
    /// frame without allocating.
    pub fn label(mut self, label: impl Into<Label>) -> Self {
        self.access_mut().label = Some(label.into());
        self
    }

    /// The accessible description (see the `description` field).
    pub fn description(mut self, description: impl Into<Label>) -> Self {
        self.access_mut().description = Some(description.into());
        self
    }

    /// The on state for a checkbox / radio / switch role.
    pub fn checked(mut self, checked: bool) -> Self {
        self.access_mut().checked = checked;
        self
    }

    /// A checkbox that is neither on nor off (see the `mixed` field).
    pub fn mixed(mut self, mixed: bool) -> Self {
        self.access_mut().mixed = mixed;
        self
    }

    /// The current one of a set (see the `selected` field).
    pub fn selected(mut self, selected: bool) -> Self {
        self.access_mut().selected = selected;
        self
    }

    /// A disclosure's state (see the `expanded` field).
    pub fn expanded(mut self, expanded: bool) -> Self {
        self.access_mut().expanded = Some(expanded);
        self
    }

    /// Marks this node a live region (see the `live` field).
    pub fn live(mut self, live: Live) -> Self {
        self.access_mut().live = live;
        self
    }

    /// A slider role's current value.
    pub fn value_now(mut self, v: f32) -> Self {
        self.access_mut().value_now = Some(v);
        self
    }

    pub fn value_min(mut self, v: f32) -> Self {
        self.access_mut().value_min = Some(v);
        self
    }

    pub fn value_max(mut self, v: f32) -> Self {
        self.access_mut().value_max = Some(v);
        self
    }

    /// How far one arrow key moves a slider (see the `value_step` field).
    /// A step that is not positive and finite clears it.
    pub fn value_step(mut self, v: f32) -> Self {
        self.access_mut().value_step = (v.is_finite() && v > 0.0).then_some(v);
        self
    }

    /// Asks the core for a slider's changes (see the `on_change` field).
    pub fn on_change(mut self, tag: impl Into<Value>) -> Self {
        self.events_mut().on_change = Some(tag.into());
        self
    }

    /// What a slider's position reads as (see the `value_text` field).
    pub fn value_text(mut self, text: impl Into<Label>) -> Self {
        self.access_mut().value_text = Some(text.into());
        self
    }

    /// On a `Role::Line` of a custom editor: the caret's byte offset into
    /// this line's text (see the `caret` field).
    pub fn caret(mut self, offset: u32) -> Self {
        self.access_mut().caret = Some(offset);
        self
    }

    /// On a `Role::Line` of a custom editor: the byte offset where the
    /// selection's other end sits (see the `selection_anchor` field).
    pub fn selection_anchor(mut self, offset: u32) -> Self {
        self.access_mut().selection_anchor = Some(offset);
        self
    }

    /// On a `Role::Line` declaring `caret`: the caret is solid, not a
    /// caret to blink (see the `caret_solid` field). A block caret in a
    /// modal editor's normal mode — the one thing that otherwise asks an
    /// idle app for a frame twice a second.
    pub fn caret_solid(mut self) -> Self {
        self.access_mut().caret_solid = true;
        self
    }

    /// Makes this node a key sink (see `NodeSpec::on_key` field). Pass a
    /// tag the handler can match on; `Value::Null` if the node key is
    /// identification enough.
    pub fn on_key(mut self, tag: impl Into<Value>) -> Self {
        self.events_mut().on_key = Some(tag.into());
        self
    }

    /// A key sink with no tag — `on_key(Value::Null)`, for a handler that
    /// knows the node by its key.
    #[inline]
    pub fn key_sink(self) -> Self {
        self.on_key(Value::Null)
    }

    /// Delivers releases to this key sink as well as presses (see the
    /// `key_up` field). Meaningless without `on_key`.
    pub fn key_up(mut self) -> Self {
        self.events_mut().key_up = true;
        self
    }

    /// Asks for force-click events on this node (see the
    /// `on_force_click` field).
    pub fn on_force_click(mut self, tag: impl Into<Value>) -> Self {
        self.events_mut().on_force_click = Some(tag.into());
        self
    }

    /// Asks for the non-primary buttons pressed over this node as events,
    /// each captured by it until its release (see the `on_button` field):
    /// `{kind="button", phase, button, x, y, tag}`. Every such button
    /// unless [`NodeSpec::buttons`] narrows it.
    pub fn on_button(mut self, tag: impl Into<Value>) -> Self {
        self.events_mut().on_button = Some(tag.into());
        self
    }

    /// Which non-primary buttons `on_button` claims (see the `buttons`
    /// field): `Buttons::MIDDLE`, `Buttons::SECONDARY | Buttons::MIDDLE`.
    /// Meaningless without `on_button`.
    pub fn buttons(mut self, buttons: Buttons) -> Self {
        self.events_mut().buttons = buttons;
        self
    }

    /// Asks for the wheel over this node as events (see the `on_scroll`
    /// field): `{kind="scroll", x, y, dx, dy, lines, tag}`.
    pub fn on_scroll(mut self, tag: impl Into<Value>) -> Self {
        self.events_mut().on_scroll = Some(tag.into());
        self
    }

    /// Which axes `on_scroll` takes (see the `scroll_axes` field):
    /// `ScrollAxes::Y` for a terminal's history, so a sideways gesture
    /// passes it by. Meaningless without `on_scroll`.
    pub fn scroll_axes(mut self, axes: ScrollAxes) -> Self {
        self.events_mut().scroll_axes = axes;
        self
    }

    /// Asks for a context menu on this node (see the `on_context_menu`
    /// field). Pass a tag the handler can match on; `Value::Null` if the
    /// node key is identification enough.
    pub fn on_context_menu(mut self, tag: impl Into<Value>) -> Self {
        self.events_mut().on_context_menu = Some(tag.into());
        self
    }

    /// Animates changes to this node's sizing amounts, colors and radius
    /// over `duration_ms` (cubic ease-out); see [`crate::anim`]. Only the
    /// duration: an easing or repeat already declared survives, so the
    /// transition props compose in any order.
    ///
    /// On a scroll container it also eases the offset a `reveal` or a
    /// `set_scroll` moves it to (backlog F80) — the wheel, the thumb and
    /// a drag past the edge still land whole, being the hand's own.
    pub fn transition(mut self, duration_ms: f32) -> Self {
        let t = self.transition.get_or_insert(Transition::ms(duration_ms));
        t.duration_ms = duration_ms;
        self
    }

    pub fn transition_with(mut self, t: Transition) -> Self {
        self.transition = Some(t);
        self
    }

    /// Also ease this node's position (see the `slide` field); sets a
    /// default 200ms transition if none was declared yet.
    pub fn slide(mut self) -> Self {
        self.transition.get_or_insert(Transition::ms(200.0));
        self.slide = true;
        self
    }

    /// Easing for the node's transition (sets a default 200ms one if none
    /// was declared yet).
    pub fn easing(mut self, easing: Easing) -> Self {
        let t = self.transition.get_or_insert(Transition::ms(200.0));
        t.easing = easing;
        self
    }

    /// How this node's `keyframes` cycle (CSS's `animation-direction`);
    /// sets a default 200ms transition if none was declared yet.
    pub fn repeat(mut self, repeat: Repeat) -> Self {
        let t = self.transition.get_or_insert(Transition::ms(200.0));
        t.repeat = repeat;
        self
    }

    /// Holds this node's keyframe cycle back by `delay_ms` (CSS's
    /// `animation-delay`), so siblings given different delays run out of
    /// phase — a cascade, a chase light. Sets a default 200ms transition if
    /// none was declared yet.
    pub fn delay(mut self, delay_ms: f32) -> Self {
        let t = self.transition.get_or_insert(Transition::ms(200.0));
        t.delay_ms = delay_ms;
        self
    }

    /// Cycles the slots these stops name (see the `keyframes` field); sets
    /// a default 200ms transition if none was declared yet.
    pub fn keyframes(mut self, stops: Vec<Keyframe>) -> Self {
        self.transition.get_or_insert(Transition::ms(200.0));
        self.anim_mut().keyframes = stops;
        self
    }

    /// Eases this node in from `enter` on its first sight (see the `enter`
    /// field); sets a default 200ms transition if none was declared yet.
    pub fn enter(mut self, enter: Enter) -> Self {
        self.transition.get_or_insert(Transition::ms(200.0));
        self.anim_mut().enter = Some(enter);
        self
    }

    /// Eases this node out to `exit` after the view stops declaring it
    /// (see the `exit` field); sets a default 200ms transition if none was
    /// declared yet.
    pub fn exit(mut self, exit: Enter) -> Self {
        self.transition.get_or_insert(Transition::ms(200.0));
        self.anim_mut().exit = Some(exit);
        self
    }

    /// Pressing this node starts an OS window drag (drivers promote a quick
    /// second press to a maximize toggle).
    pub fn window_drag(mut self) -> Self {
        self.window = Some(WindowRole::Drag);
        self
    }

    /// Clicking this node emits the button's `WindowCommand`.
    pub fn window_button(mut self, button: WindowButton) -> Self {
        self.window = Some(WindowRole::Button(button));
        self
    }

    /// The pointer shape over this node (see the `cursor` field).
    pub fn cursor(mut self, shape: CursorShape) -> Self {
        self.cursor = Some(shape);
        self
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum FontFamily {
    #[default]
    Sans,
    Serif,
    Mono,
    /// A font registered with the core (`Core::add_font_data` from file
    /// bytes, or `Core::add_system_font` by installed family name). A
    /// stale handle shapes as sans-serif. Not on the wire as a family:
    /// the `font` row carries the handle.
    Custom(crate::resources::FontId),
}

impl FontFamily {
    /// The three stock families, in wire order: `schema::FAMILIES` is
    /// `ALL` by `name`, and a binding sends the index (backlog AR42).
    /// `Custom` is not a family on the wire — the `font` row is.
    pub const ALL: &'static [FontFamily] = &[FontFamily::Sans, FontFamily::Serif, FontFamily::Mono];

    /// The spelling every binding uses; a registered font has none.
    pub fn name(self) -> Option<&'static str> {
        match self {
            FontFamily::Sans => Some("sans"),
            FontFamily::Serif => Some("serif"),
            FontFamily::Mono => Some("mono"),
            FontFamily::Custom(_) => None,
        }
    }

    /// The variant `schema::FAMILIES` index `i` names; `Sans` for an
    /// index this build lacks.
    pub fn from_index(i: usize) -> FontFamily {
        Self::ALL.get(i).copied().unwrap_or_default()
    }
}

/// How a text node breaks lines at its width.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TextWrap {
    /// Break between words; a word wider than the line breaks by glyph.
    #[default]
    Word,
    /// Break anywhere (URLs, hashes, code).
    Glyph,
    /// Never break at the width: one line per paragraph, clipped to the
    /// node's box (explicit newlines still break).
    None,
    /// `Word`, but whitespace takes its room like any glyph — CSS's
    /// `white-space: break-spaces`: a space that does not fit starts the
    /// next row instead of hanging past the edge or vanishing at the
    /// break, so every byte has a place inside the box. An editor's
    /// wrapped line, whose caret stands on each space. A text with line
    /// breaks of its own, a `max_lines` or an `ellipsis` wraps as `Word`.
    BreakSpaces,
}

/// The OpenType features a style asks the shaper for (backlog C23): up to
/// [`FontFeatures::MAX`] four-letter tags with a value each — `liga` 0 to
/// keep a coding font from joining `->`, `tnum` 1 for tabular figures in a
/// gutter, `ss01` 1 for a stylistic set. Plain data and `Copy`, since a
/// `TextStyle` is; the spelling every binding shares is
/// [`FontFeatures::parse`]'s.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct FontFeatures {
    tags: [[u8; 4]; FontFeatures::MAX],
    values: [u32; FontFeatures::MAX],
    len: u8,
}

impl FontFeatures {
    /// How many features a style can carry. Eight is more than any one
    /// text asks for and keeps the style small.
    pub const MAX: usize = 8;

    pub const fn new() -> Self {
        Self {
            tags: [[b' '; 4]; Self::MAX],
            values: [0; Self::MAX],
            len: 0,
        }
    }

    /// Adds or replaces `tag` (four ASCII characters; shorter is padded
    /// with spaces, longer is cut) with `value`. Past `MAX` the feature is
    /// dropped rather than the style refused.
    pub fn set(mut self, tag: &str, value: u32) -> Self {
        let mut t = [b' '; 4];
        for (i, b) in tag.bytes().take(4).enumerate() {
            t[i] = b;
        }
        let n = self.len as usize;
        if let Some(i) = self.tags[..n].iter().position(|x| *x == t) {
            self.values[i] = value;
        } else if n < Self::MAX {
            self.tags[n] = t;
            self.values[n] = value;
            self.len += 1;
        }
        self
    }

    /// The spelling every binding shares: tags separated by whitespace or
    /// commas, each `tag=value`, a bare `tag` meaning 1 and `-tag` meaning
    /// 0 — `"liga=0 calt=0 tnum"`, `"-liga,-calt"`. Anything else in the
    /// string is skipped.
    pub fn parse(s: &str) -> Self {
        let mut out = Self::new();
        for item in s.split(|c: char| c.is_whitespace() || c == ',') {
            if item.is_empty() {
                continue;
            }
            let (tag, value) = if let Some((t, v)) = item.split_once('=') {
                (t, v.trim().parse::<u32>().unwrap_or(0))
            } else if let Some(t) = item.strip_prefix('-') {
                (t, 0)
            } else if let Some(t) = item.strip_prefix('+') {
                (t, 1)
            } else {
                (item, 1)
            };
            let tag = tag.trim();
            if !tag.is_empty() {
                out = out.set(tag, value);
            }
        }
        out
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn len(&self) -> usize {
        self.len as usize
    }

    /// Every feature, in the order set, as `(tag, value)`.
    pub fn iter(&self) -> impl Iterator<Item = (&[u8; 4], u32)> + '_ {
        let n = self.len as usize;
        self.tags[..n].iter().zip(self.values[..n].iter().copied())
    }

    /// What `parse` reads: `tag=value` pairs joined by spaces.
    pub fn to_string_spelling(&self) -> String {
        self.iter()
            .map(|(t, v)| format!("{}={}", String::from_utf8_lossy(t).trim_end(), v))
            .collect::<Vec<_>>()
            .join(" ")
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TextStyle {
    pub size: f32,
    pub line_height: f32,
    /// `None` is "the theme's foreground" — which is what the schema has
    /// always said this row means ("default foreground when omitted") and,
    /// since ADR 0019, what it does. Filled in as the text enters the
    /// tree, so nothing downstream of that ever sees a `None`.
    pub color: Option<Color>,
    pub family: FontFamily,
    pub wrap: TextWrap,
    /// At most this many lines are laid out; 0 = unlimited.
    pub max_lines: u32,
    /// End the last line with "…" when the text was cut off. Alone it
    /// means a single line (`max_lines` 1); with `max_lines` it clamps.
    pub ellipsis: bool,
    /// OpenType features for the shaper; none by default, which is the
    /// font's own defaults (ligatures on, where it has them).
    pub features: FontFeatures,
    /// A line under every glyph, where the face puts its underline (backlog
    /// C22). Paint only: not part of what the text is shaped as.
    pub underline: bool,
    /// The underline's own colour; `None` is the text's (backlog K4).
    pub underline_color: Option<Color>,
    /// The underline's shape: a line, a wave, dots (backlog K4).
    pub underline_style: UnderlineStyle,
    /// A line through every glyph, where the face puts its strikeout.
    pub strikethrough: bool,
}

/// The shape of an underline (backlog K4): the face's line, a wave under a
/// diagnostic, dots. Where it goes and how thick it is are the face's
/// recommendation either way; a wave is three strokes tall around the
/// line's centre with a six-stroke period, dots two strokes across and
/// four apart (`crate::deco`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum UnderlineStyle {
    #[default]
    Solid,
    Wavy,
    Dotted,
}

impl UnderlineStyle {
    /// The spellings, in discriminant order (`underlineStyle` /
    /// `underline_style`; C's `KUI_UNDERLINE_*`).
    pub const NAMES: &[&str] = &["solid", "wavy", "dotted"];

    pub fn from_index(i: u32) -> Self {
        match i {
            1 => Self::Wavy,
            2 => Self::Dotted,
            _ => Self::Solid,
        }
    }

    pub fn name(self) -> &'static str {
        Self::NAMES[self as usize]
    }
}

impl Default for TextStyle {
    fn default() -> Self {
        Self::new(16.0)
    }
}

impl TextStyle {
    pub fn new(size: f32) -> Self {
        Self {
            size,
            line_height: (size * 1.35).round(),
            color: None,
            family: FontFamily::Sans,
            wrap: TextWrap::Word,
            max_lines: 0,
            ellipsis: false,
            features: FontFeatures::new(),
            underline: false,
            underline_color: None,
            underline_style: UnderlineStyle::Solid,
            strikethrough: false,
        }
    }

    pub fn underline(mut self) -> Self {
        self.underline = true;
        self
    }

    /// An underline in its own colour rather than the text's — a
    /// diagnostic's red under keyword-coloured text (backlog K4). Turns
    /// the underline on.
    pub fn underline_color(mut self, c: Color) -> Self {
        self.underline = true;
        self.underline_color = Some(c);
        self
    }

    /// An underline of this shape (backlog K4). Turns the underline on.
    pub fn underline_style(mut self, s: UnderlineStyle) -> Self {
        self.underline = true;
        self.underline_style = s;
        self
    }

    pub fn strikethrough(mut self) -> Self {
        self.strikethrough = true;
        self
    }

    /// OpenType features for the shaper; see [`FontFeatures`].
    pub fn features(mut self, f: FontFeatures) -> Self {
        self.features = f;
        self
    }

    pub fn family(mut self, f: FontFamily) -> Self {
        self.family = f;
        self
    }

    pub fn mono(self) -> Self {
        self.family(FontFamily::Mono)
    }

    /// Shape with a registered font (see `FontFamily::Custom`).
    pub fn font(self, id: crate::resources::FontId) -> Self {
        self.family(FontFamily::Custom(id))
    }

    pub fn line_height(mut self, lh: f32) -> Self {
        self.line_height = lh;
        self
    }

    pub fn wrap(mut self, wrap: TextWrap) -> Self {
        self.wrap = wrap;
        self
    }

    /// One line per paragraph, clipped to the node (`TextWrap::None`).
    pub fn nowrap(self) -> Self {
        self.wrap(TextWrap::None)
    }

    /// Lay out at most `n` lines (0 = unlimited).
    pub fn max_lines(mut self, n: u32) -> Self {
        self.max_lines = n;
        self
    }

    /// Truncate with "…" instead of overflowing: a single line unless
    /// `max_lines` says otherwise.
    pub fn ellipsis(mut self) -> Self {
        self.ellipsis = true;
        self
    }

    pub fn color(mut self, c: Color) -> Self {
        self.color = Some(c);
        self
    }

    /// This style with `fg` where it named no colour of its own: what the
    /// core stamps on as the text enters the tree, so the shaping caches,
    /// the display list and every binding see a resolved colour and never
    /// the question mark (ADR 0019).
    pub fn or_fg(mut self, fg: Color) -> Self {
        self.color = Some(self.color.unwrap_or(fg));
        self
    }

    /// The colour this style paints in, falling back to what kui painted
    /// before there were themes. For the handful of places that hold a
    /// style the core never stamped.
    pub fn color_or_default(&self) -> Color {
        self.color.unwrap_or(crate::theme::Theme::DEFAULT_FG)
    }
}

/// Hand-written so that a group whose box was allocated but left at its
/// defaults — `.checked(false)` on a fresh spec — equals one whose box was
/// never allocated at all. The derived version compared `None` against
/// `Some(EMPTY)` and called them different, which is a difference the boxing
/// introduced rather than one a caller declared.
impl PartialEq for NodeSpec {
    fn eq(&self, other: &Self) -> bool {
        self.layout == other.layout
            && self.style == other.style
            && self.hoverable == other.hoverable
            && self.animate == other.animate
            && self.accent == other.accent
            && self.window == other.window
            && self.transition == other.transition
            && self.slide == other.slide
            && self.focusable == other.focusable
            && self.initial_focus == other.initial_focus
            && self.disabled == other.disabled
            && self.cursor == other.cursor
            && self.events() == other.events()
            && self.anim() == other.anim()
            && self.access() == other.access()
            && self.interact() == other.interact()
    }
}

#[cfg(test)]
mod size_tests {
    use super::*;

    /// `NodeSpec` is moved by value for every node a frame builds — through
    /// the builder chain, `Core::open`, `open_with_key` and into
    /// `Vec<NodeSpec>` in `Tree::push` — so its size is a per-node cost that
    /// every app pays whether or not it declares the fields. It reached 728
    /// bytes one feature at a time and cost ~2.5x on the frame benches before
    /// anyone measured it (C15 in `docs/backlog/closed-2026-09.md`).
    ///
    /// This is the number a review can fail. Adding a prop is fine; adding it
    /// *inline* past this bound is the thing to notice. Put cold fields in one
    /// of the boxed groups instead, and only raise this if the field is read
    /// on every node of every frame.
    #[test]
    fn node_spec_stays_small() {
        const BOUND: usize = 256;
        let size = std::mem::size_of::<NodeSpec>();
        assert!(
            size <= BOUND,
            "NodeSpec is {size} bytes, over the {BOUND}-byte bound. It is copied \
             per node per frame; put cold fields in EventSpec / AnimSpec / \
             AccessSpec / InteractSpec rather than inline. See C15."
        );
    }

    /// The groups exist to be absent: a node declaring none of them carries
    /// four null pointers, not four structs.
    #[test]
    fn an_undeclared_group_costs_a_pointer() {
        let spec = NodeSpec::default();
        assert!(spec.events.is_none() && spec.anim.is_none());
        assert!(spec.access.is_none() && spec.interact.is_none());
        // and reads still work, without allocating
        assert!(spec.events().on_click.is_none());
        assert_eq!(spec.access().role, None);
    }

    /// An allocated-but-default group is still equal to no group at all.
    #[test]
    fn an_empty_group_equals_no_group() {
        assert_eq!(NodeSpec::default().checked(false), NodeSpec::default());
    }
}
