//! Node configuration: plain data, trivially constructible from any language.

use crate::anim::{Easing, Transition};
use crate::color::Color;
use crate::geom::Edges;
use crate::value::Value;
use crate::window::{WindowButton, WindowRole};

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

    /// Flip across the anchor / clamp as needed to stay in the viewport.
    pub fn fit(mut self) -> Self {
        self.fit = true;
        self
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

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct VisualStyle {
    pub bg: Color,
    pub border_color: Color,
    pub border_w: f32,
    /// Corner radii (logical px), clockwise from the top-left:
    /// `[tl, tr, br, bl]`. `NodeSpec::radius` sets all four; the per-corner
    /// builders (`radius_tl`, ...) override one — later calls win, like CSS
    /// `border-radius` followed by `border-top-left-radius`.
    pub radius: [f32; 4],
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
#[derive(Clone, Debug, Default, PartialEq)]
pub struct NodeSpec {
    pub layout: LayoutSpec,
    pub style: VisualStyle,
    /// Payload emitted as a `UiEvent` when this node is clicked.
    pub on_click: Option<Value>,
    /// Track pointer hover for this node (`Ui::is_hovered`) without making
    /// it clickable — tooltips on passive badges. Nodes with `on_click` /
    /// `on_key` / `window` are always hover-tracked; clicks on a merely
    /// hoverable node emit nothing.
    pub hoverable: bool,
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
    /// Background while the pointer hovers this node (or any node sharing
    /// its `hover_group`). Resolved by the core when the node opens, so a
    /// data-only view gets hover styling without querying `is_hovered` —
    /// and with `transition` the swap eases. Implies hover tracking.
    pub hover_bg: Option<Color>,
    /// Background while this node (or its group) is pressed. Implies hover
    /// tracking. Without a `hover_bg`, hover keeps the plain `bg`.
    pub pressed_bg: Option<Color>,
    /// Hover group: nodes sharing an id count as one for `hover_bg` /
    /// `pressed_bg` — a two-piece elbow, a split button, a row whose cells
    /// highlight together. The id is a hash of a name (`hover_group`).
    /// Implies hover tracking.
    pub hover_group: Option<u64>,
    /// Hover events: the pointer entering or leaving this node emits
    /// `{kind="hover", phase="enter"|"leave", tag}` with this payload under
    /// `tag` — for hover-dependent *layout* (a close button that appears)
    /// where a color swap isn't enough. Implies hover tracking.
    pub on_hover: Option<Value>,
}

impl NodeSpec {
    /// Whether the core registers a hit region for this node (any of the
    /// interaction props, or an explicit `hoverable`).
    pub fn hover_tracked(&self) -> bool {
        self.hoverable
            || self.on_click.is_some()
            || self.on_drag.is_some()
            || self.on_key.is_some()
            || self.window.is_some()
            || self.hover_bg.is_some()
            || self.pressed_bg.is_some()
            || self.hover_group.is_some()
            || self.on_hover.is_some()
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

    /// Clip children to this node's rect without scrolling.
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

    /// Rounds all four corners by `r`.
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

    /// Hover-track this node without making it clickable; see the
    /// `hoverable` field.
    pub fn hoverable(mut self) -> Self {
        self.hoverable = true;
        self
    }

    pub fn on_click(mut self, payload: impl Into<Value>) -> Self {
        self.on_click = Some(payload.into());
        self
    }

    /// Background while hovered (see the `hover_bg` field).
    pub fn hover_bg(mut self, c: Color) -> Self {
        self.hover_bg = Some(c);
        self
    }

    /// Background while pressed (see the `pressed_bg` field).
    pub fn pressed_bg(mut self, c: Color) -> Self {
        self.pressed_bg = Some(c);
        self
    }

    /// Joins the hover group `name` (see the `hover_group` field).
    pub fn hover_group(mut self, name: &str) -> Self {
        self.hover_group = Some(Self::hover_group_id(name));
        self
    }

    /// Emits enter/leave events for this node (see the `on_hover` field).
    /// Pass a tag the handler can match on; `Value::Null` if the node key
    /// is identification enough.
    pub fn on_hover(mut self, tag: impl Into<Value>) -> Self {
        self.on_hover = Some(tag.into());
        self
    }

    /// Makes this node draggable (see the `on_drag` field). Pass a tag the
    /// handler can match on; `Value::Null` if the node key is enough.
    pub fn on_drag(mut self, tag: impl Into<Value>) -> Self {
        self.on_drag = Some(tag.into());
        self
    }

    /// Makes this node a key sink (see `NodeSpec::on_key` field). Pass a
    /// tag the handler can match on; `Value::Null` if the node key is
    /// identification enough.
    pub fn on_key(mut self, tag: impl Into<Value>) -> Self {
        self.on_key = Some(tag.into());
        self
    }

    /// Animates changes to this node's sizing amounts, colors and radius
    /// over `duration_ms` (cubic ease-out); see [`crate::anim`].
    pub fn transition(mut self, duration_ms: f32) -> Self {
        self.transition = Some(Transition::ms(duration_ms));
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

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TextStyle {
    pub size: f32,
    pub line_height: f32,
    pub color: Color,
    pub family: FontFamily,
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

    pub fn color(mut self, c: Color) -> Self {
        self.color = c;
        self
    }
}
