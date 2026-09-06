//! Node configuration: plain data, trivially constructible from any language.

use crate::anim::{Easing, Repeat, Transition};
use crate::color::Color;
use crate::cursor::CursorShape;
use crate::enter::Enter;
use crate::geom::Edges;
use crate::keyframes::Keyframe;
use crate::value::Value;
use crate::window::{WindowButton, WindowRole};

pub use crate::access::{Label, Role};

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

impl Sizing {
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

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Align {
    #[default]
    Start,
    Center,
    End,
}

/// What a floating node is positioned against.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum FloatAnchor {
    /// The parent node's border box.
    #[default]
    Parent,
    /// The whole viewport.
    Viewport,
}

/// Takes a node out of flex flow: it doesn't consume space in its parent,
/// sizes Grow/Percent against its anchor, is positioned by attach points,
/// draws on top of in-flow content, and escapes ancestor clips.
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
    /// moves it sideways without flattening the gap to zero.
    pub fn build(
        base: FloatConfig,
        anchor_at: Option<(Align, Align)>,
        self_at: Option<(Align, Align)>,
        dx: Option<f32>,
        dy: Option<f32>,
        fit: bool,
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
    pub min_w: f32,
    pub max_w: f32,
    pub min_h: f32,
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
    /// Space between wrap lines, across the main axis. `gap` is still the
    /// space between children along it.
    pub cross_gap: f32,
    /// Alignment of children along the main axis.
    pub main_align: Align,
    /// Alignment of children across the main axis.
    pub cross_align: Align,
    /// Clip children to this node's rect.
    pub clip: bool,
    /// Overflowing content scrolls (implies clipping). Offsets are retained
    /// across frames in the core, keyed by this node's `Key`.
    pub scroll_x: bool,
    pub scroll_y: bool,
    /// Out-of-flow positioning; see [`FloatConfig`].
    pub float: Option<FloatConfig>,
}

impl Default for LayoutSpec {
    fn default() -> Self {
        Self {
            width: Sizing::Fit,
            height: Sizing::Fit,
            min_w: 0.0,
            max_w: f32::INFINITY,
            min_h: 0.0,
            max_h: f32::INFINITY,
            dir: Dir::Column,
            padding: Edges::default(),
            gap: 0.0,
            wrap: false,
            cross_gap: 0.0,
            main_align: Align::Start,
            cross_align: Align::Start,
            clip: false,
            scroll_x: false,
            scroll_y: false,
            float: None,
        }
    }
}

impl LayoutSpec {
    /// Whether this node clips its children.
    pub fn clips(&self) -> bool {
        self.clip || self.scroll_x || self.scroll_y
    }
}

impl LayoutSpec {
    pub(crate) fn clamp_w(&self, w: f32) -> f32 {
        w.clamp(self.min_w, self.max_w.max(self.min_w))
    }

    pub(crate) fn clamp_h(&self, h: f32) -> f32 {
        h.clamp(self.min_h, self.max_h.max(self.min_h))
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
    /// Overrides the pointer shape over this node (see
    /// [`crate::cursor`]). Unset, the core derives one from what the node
    /// does — an editor is a caret, a clickable or focusable node a hand,
    /// an `on_drag` node a grab — so this is only for what the derivation
    /// cannot know: a splitter that resizes rather than moves, a disabled
    /// control that wants to say `notAllowed`.
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
    /// presses arrive as `UiEvent`s on it, with this payload merged in
    /// under `tag`. Clicking the node takes key focus.
    pub on_key: Option<Value>,
    /// Asks for a context menu: a secondary-button press over this node
    /// emits `{kind="contextmenu", x, y, tag}` on it with this payload
    /// under `tag`, and does nothing else — the press moves no focus,
    /// places no caret and produces no click, so right-clicking a
    /// selection leaves it selected. `x`/`y` are the press in logical
    /// viewport coordinates, which is where the menu goes. Routed like a
    /// click: the topmost node under the pointer is the one asked, so an
    /// interactive child takes the press unless it declares its own.
    /// Null = the behaviour without a tag.
    pub on_context_menu: Option<Value>,
    /// Hover events: the pointer entering or leaving this node emits
    /// `{kind="hover", phase="enter"|"leave", tag}` with this payload under
    /// `tag` — for hover-dependent *layout* (a close button that appears)
    /// where a color swap isn't enough. Implies hover tracking.
    pub on_hover: Option<Value>,
    /// Layout events: the rect layout gave this node arrives as
    /// `{kind="layout", x, y, w, h, parent: {x, y, w, h}, tag}` (logical px,
    /// viewport coordinates, after scrolling and position easing) — on the
    /// node's first frame and again whenever the rect changes, never on a
    /// frame that left it alone. The view reads the numbers layout already
    /// produced instead of re-deriving them; a transition that moves the
    /// node reports every frame it moves. Needs a stable key across frames.
    pub on_layout: Option<Value>,
    /// Modal: while this node is declared, the Tab ring is its subtree,
    /// everything outside it is inert to the pointer, the wheel and
    /// assistive technology, and Escape or a press outside emits
    /// `{kind="dismiss", reason, tag}` on it with this payload under
    /// `tag`. The last node declaring it in tree order is the one in
    /// effect (a confirm inside a dialog); see
    /// `docs/adr/0003-modal-surfaces.md`. Null = modal without a tag.
    pub modal: Option<Value>,
}

impl EventSpec {
    /// The group as a node that declares none of it — what
    /// `NodeSpec`'s accessor hands back when the box is `None`.
    pub const EMPTY: Self = Self {
        on_click: None,
        on_drag: None,
        on_key: None,
        on_context_menu: None,
        on_hover: None,
        on_layout: None,
        modal: None,
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
    /// The accessible description — what the `tooltip` prop sets in the
    /// bindings, read after the name.
    pub description: Option<Label>,
    /// For checkbox / radio / switch roles: the on state.
    pub checked: bool,
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
    /// On a `Role::Line` of a custom editor: the caret's byte offset into
    /// the line's text, and the byte offset of the selection's other end
    /// (see [`crate::access`]).
    pub caret: Option<u32>,
    pub selection_anchor: Option<u32>,
}

impl AccessSpec {
    /// The group as a node that declares none of it — what
    /// `NodeSpec`'s accessor hands back when the box is `None`.
    pub const EMPTY: Self = Self {
        role: None,
        label: None,
        description: None,
        checked: false,
        selected: false,
        expanded: None,
        value_now: None,
        value_min: None,
        value_max: None,
        caret: None,
        selection_anchor: None,
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
        self.hoverable
            || self.focusable
            // A modal's own background is not "outside" it: a press there
            // must find a region (see `docs/adr/0003-modal-surfaces.md`).
            || self.events().modal.is_some()
            || self.events().on_click.is_some()
            || self.events().on_drag.is_some()
            || self.events().on_key.is_some()
            || self.events().on_context_menu.is_some()
            || self.window.is_some()
            || self.interact().hover_bg.is_some()
            || self.interact().pressed_bg.is_some()
            || self.interact().hover_group.is_some()
            || self.events().on_hover.is_some()
            || self.interact().click_sound.is_some()
            || self.interact().hover_sound.is_some()
            // A `cursor` override has to be found under the pointer to be
            // read, even on an otherwise inert box.
            || self.cursor.is_some()
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

    pub fn width(mut self, s: Sizing) -> Self {
        self.layout.width = s;
        self
    }

    pub fn height(mut self, s: Sizing) -> Self {
        self.layout.height = s;
        self
    }

    /// Grow along both axes.
    pub fn fill(self) -> Self {
        self.width(Sizing::Grow(1.0)).height(Sizing::Grow(1.0))
    }

    pub fn min_width(mut self, v: f32) -> Self {
        self.layout.min_w = v;
        self
    }

    pub fn max_width(mut self, v: f32) -> Self {
        self.layout.max_w = v;
        self
    }

    pub fn min_height(mut self, v: f32) -> Self {
        self.layout.min_h = v;
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

    /// The spec half of the `tooltip` prop: the hint is hover-gated, so the
    /// node tracks hover, and it is what assistive technology should say, so
    /// it is the accessible description too. The third effect — floating the
    /// hint itself — is [`crate::schema::PropsOut::apply_tooltip`] for the
    /// parsers and `kui_close` for C, because only they know when the node
    /// is open.
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

    pub fn on_click(mut self, payload: impl Into<Value>) -> Self {
        self.events_mut().on_click = Some(payload.into());
        self
    }

    /// Background while hovered (see the `hover_bg` field).
    pub fn hover_bg(mut self, c: Color) -> Self {
        self.interact_mut().hover_bg = Some(c);
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
    pub fn modal(mut self, tag: Value) -> Self {
        self.events_mut().modal = Some(tag);
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

    /// Makes this node a key sink (see `NodeSpec::on_key` field). Pass a
    /// tag the handler can match on; `Value::Null` if the node key is
    /// identification enough.
    pub fn on_key(mut self, tag: impl Into<Value>) -> Self {
        self.events_mut().on_key = Some(tag.into());
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

    /// Overrides the pointer shape over this node (see the `cursor` field).
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
    /// stale handle shapes as sans-serif.
    Custom(crate::resources::FontId),
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
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TextStyle {
    pub size: f32,
    pub line_height: f32,
    pub color: Color,
    pub family: FontFamily,
    pub wrap: TextWrap,
    /// At most this many lines are laid out; 0 = unlimited.
    pub max_lines: u32,
    /// End the last line with "…" when the text was cut off. Alone it
    /// means a single line (`max_lines` 1); with `max_lines` it clamps.
    pub ellipsis: bool,
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
            color: Color::rgb8(0xe8, 0xe8, 0xea),
            family: FontFamily::Sans,
            wrap: TextWrap::Word,
            max_lines: 0,
            ellipsis: false,
        }
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
        self.color = c;
        self
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
