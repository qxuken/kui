//! Node configuration: plain data, trivially constructible from any language.

use crate::color::Color;
use crate::geom::Edges;
use crate::value::Value;

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
        }
    }
}

impl FloatConfig {
    pub fn parent() -> Self {
        Self::default()
    }

    pub fn viewport() -> Self {
        Self { anchor: FloatAnchor::Viewport, ..Self::default() }
    }

    /// Tooltip-style: hang below the parent, centered.
    pub fn below() -> Self {
        Self {
            anchor: FloatAnchor::Parent,
            anchor_point: (Align::Center, Align::End),
            self_point: (Align::Center, Align::Start),
            offset: Vec2Offset { x: 0.0, y: 6.0 },
        }
    }

    /// Tooltip-style: hover above the parent, centered.
    pub fn above() -> Self {
        Self {
            anchor: FloatAnchor::Parent,
            anchor_point: (Align::Center, Align::Start),
            self_point: (Align::Center, Align::End),
            offset: Vec2Offset { x: 0.0, y: -6.0 },
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
    pub radius: f32,
}

/// Full per-node configuration. This — not any Rust trait — is the contract
/// every frontend (Rust builders, Lua, serialized UI) lowers into.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct NodeSpec {
    pub layout: LayoutSpec,
    pub style: VisualStyle,
    /// Payload emitted as a `UiEvent` when this node is clicked.
    pub on_click: Option<Value>,
}

impl NodeSpec {
    pub fn row() -> Self {
        Self { layout: LayoutSpec { dir: Dir::Row, ..Default::default() }, ..Default::default() }
    }

    pub fn column() -> Self {
        Self { layout: LayoutSpec { dir: Dir::Column, ..Default::default() }, ..Default::default() }
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

    pub fn radius(mut self, r: f32) -> Self {
        self.style.radius = r;
        self
    }

    pub fn border(mut self, w: f32, c: Color) -> Self {
        self.style.border_w = w;
        self.style.border_color = c;
        self
    }

    pub fn on_click(mut self, payload: impl Into<Value>) -> Self {
        self.on_click = Some(payload.into());
        self
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum FontFamily {
    #[default]
    Sans,
    Serif,
    Mono,
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

    pub fn line_height(mut self, lh: f32) -> Self {
        self.line_height = lh;
        self
    }

    pub fn color(mut self, c: Color) -> Self {
        self.color = c;
        self
    }
}
