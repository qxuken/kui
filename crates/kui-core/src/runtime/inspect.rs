//! The frame as a list a tool can read back: every node the last finished
//! frame laid out, with what it is, where layout put it, and the handful
//! of declarations that explain the rest. What a devtools tree view and
//! node inspector are built from (`docs/adr/0021`, the harness's dock).
//!
//! Off unless asked: a view runs every frame and the copy is O(nodes),
//! so a shipped app pays nothing. `Core::set_inspect(true)` turns the
//! snapshot on, and it is taken at the end of every finished frame from
//! then on — the tree itself is rebuilt from scratch next frame, so this
//! is the only reading of a frame that outlives it.

use super::*;
use crate::access::Role;
use crate::geom::{Edges, Rect};
use crate::spec::{Align, Dir, Sizing};
use crate::tree::{NIL, NodeContent, OriginId};

/// What kind of node a snapshot row is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NodeKind {
    Box,
    Text,
    Edit,
    Image,
    Line,
    Cells,
    Fragment,
    Polygon,
}

impl NodeKind {
    pub fn name(self) -> &'static str {
        match self {
            NodeKind::Box => "box",
            NodeKind::Text => "text",
            NodeKind::Edit => "edit",
            NodeKind::Image => "image",
            NodeKind::Line => "line",
            NodeKind::Cells => "cells",
            NodeKind::Fragment => "fragment",
            NodeKind::Polygon => "polygon",
        }
    }
}

/// One node of the last finished frame.
#[derive(Clone, Debug)]
pub struct NodeInfo {
    pub key: Key,
    pub parent: Option<Key>,
    /// Nesting depth; the root is 0.
    pub depth: u16,
    pub kind: NodeKind,
    /// The label it was opened under, when it was opened by one.
    pub label: Option<String>,
    /// Where layout put it, logical px — the host's viewport through
    /// [`Core::nodes`], the window's in the snapshot the panel reads.
    pub rect: Rect,
    pub dir: Dir,
    pub width: Sizing,
    pub height: Sizing,
    pub bg: Color,
    pub float: bool,
    /// The role the access tree reads for it — declared, or derived from
    /// what it is and does (a box with a click is a button). Plain
    /// structure has none.
    pub role: Option<Role>,
    /// A text node's content, cut to a line's worth.
    pub text: Option<String>,
    /// The declarations that make it interactive or special, by name:
    /// `click`, `drag`, `key`, `hover`, `hoverable`, `context-menu`,
    /// `modal`, `selectable`, `focusable`, `disabled`, `scroll`, `clip`,
    /// `transition`.
    pub flags: Vec<&'static str>,
    /// The paint layer it is in (ADR 0023): 0 in flow, else the rank of
    /// its float layer from the bottom, 1 being the first layer over the
    /// flow. What decides which of two nodes under one point is on top.
    pub layer: u16,
    /// Who declared it: the host, an extension, or the devtools.
    pub origin: OriginId,
    /// How many children it has.
    pub children: u32,
    /// The rest of the layout spec, for the inspector.
    pub padding: Edges,
    pub gap: f32,
    pub main_align: Align,
    pub cross_align: Align,
    pub wrap: bool,
    /// Whether it is a table (`LayoutSpec::table`): a column whose rows'
    /// cells line up.
    pub table: bool,
    /// The size floors and ceilings, as layout left them: a floor is the
    /// declared px, or the number a `fit` floor resolved to in the fit
    /// pass (the pass writes it back into the spec, so a declared `"fit"`
    /// reads as its measurement here, not as the word); `None` only for a
    /// fit floor the pass never measured. A ceiling is `None` when
    /// unbounded.
    pub min_w: Option<f32>,
    pub min_h: Option<f32>,
    pub max_w: Option<f32>,
    pub max_h: Option<f32>,
    /// The rest of the paint spec.
    pub radius: [f32; 4],
    pub border_w: f32,
    pub border_color: Color,
    pub opacity: f32,
    /// A scroller's offset, `None` for a node that does not scroll.
    pub scroll: Option<Vec2>,
    /// Every handler it declared, with the payload it would post:
    /// `click`, `drag`, `key` (the sink's tag), `hover`, `context-menu`,
    /// `force-click`, `layout`, `modal`.
    pub events: Vec<(&'static str, Value)>,
}

impl NodeInfo {
    /// The row as plain data, every field under its snake_case name —
    /// the key and parent spelled by `h`, sizing as [`Sizing::describe`],
    /// colours as hex, enums by their schema names, `events` a map of
    /// handler name to payload (backlog AR1).
    pub fn to_value(&self, h: crate::value::Handles) -> Value {
        Value::map([
            ("key", (h.key)(self.key)),
            ("parent", h.opt_key(self.parent)),
            ("depth", Value::Int(self.depth as i64)),
            ("kind", Value::str(self.kind.name())),
            ("label", Value::opt_str(&self.label)),
            ("rect", self.rect.to_value()),
            ("dir", Value::str(self.dir.name())),
            ("width", Value::Str(self.width.describe())),
            ("height", Value::Str(self.height.describe())),
            ("bg", Value::Int(self.bg.to_hex() as i64)),
            ("float", Value::Bool(self.float)),
            ("role", Value::opt(self.role, |r| Value::str(r.name()))),
            ("text", Value::opt_str(&self.text)),
            (
                "flags",
                Value::list(self.flags.iter().map(|f| Value::str(*f))),
            ),
            ("layer", Value::Int(self.layer as i64)),
            ("origin", Value::Int(self.origin.0 as i64)),
            ("children", Value::Int(self.children as i64)),
            (
                "padding",
                Value::map([
                    ("t", Value::float(self.padding.t)),
                    ("r", Value::float(self.padding.r)),
                    ("b", Value::float(self.padding.b)),
                    ("l", Value::float(self.padding.l)),
                ]),
            ),
            ("gap", Value::float(self.gap)),
            ("main_align", Value::str(self.main_align.name())),
            ("cross_align", Value::str(self.cross_align.name())),
            ("wrap", Value::Bool(self.wrap)),
            ("table", Value::Bool(self.table)),
            ("min_width", Value::opt_float(self.min_w)),
            ("min_height", Value::opt_float(self.min_h)),
            ("max_width", Value::opt_float(self.max_w)),
            ("max_height", Value::opt_float(self.max_h)),
            ("radius", Value::floats(&self.radius)),
            ("border_width", Value::float(self.border_w)),
            (
                "border_color",
                Value::Int(self.border_color.to_hex() as i64),
            ),
            ("opacity", Value::float(self.opacity)),
            ("scroll", Value::opt(self.scroll, Vec2::to_value)),
            (
                "events",
                Value::Map(
                    self.events
                        .iter()
                        .map(|(name, v)| (name.to_string(), v.clone()))
                        .collect(),
                ),
            ),
        ])
    }
}

const TEXT_CUT: usize = 60;

impl Core {
    /// Turns the per-frame snapshot on or off (see the module doc). Off by
    /// default; a devtool that reads [`Self::nodes`] turns it on once.
    /// The host's ask alone: the core's own devtools panel asks for the
    /// snapshot separately, per frame, while its tree tab shows or it is
    /// picking, and neither ask turns the other off (backlog AR38).
    pub fn set_inspect(&mut self, on: bool) {
        self.inspect = on;
        if !on && !self.dt_inspect {
            self.inspected.clear();
        }
    }

    /// Whether the host asked for the snapshot.
    pub fn inspect(&self) -> bool {
        self.inspect
    }

    /// The last finished frame's nodes, in tree order — empty until
    /// [`Self::set_inspect`] asked for them and a frame has finished since.
    /// Rects in the host's viewport coordinates, like every other readback
    /// (`layout_of`, `scroll_geometry`, `text_hit`): under a left dock the
    /// snapshot itself is kept in window px for the panel's outlines, and
    /// this is the translated copy (backlog AR36).
    pub fn nodes(&self) -> Vec<NodeInfo> {
        let mut out = self.snapshot().to_vec();
        let shift = self.dt_shift();
        if shift != Vec2::ZERO {
            for n in &mut out {
                n.rect.x -= shift.x;
                n.rect.y -= shift.y;
            }
        }
        out
    }

    /// The snapshot as kept: rects in window px, which is what the
    /// devtools panel outlines with, docked or not.
    pub(crate) fn snapshot(&self) -> &[NodeInfo] {
        &self.inspected
    }

    /// Called at the end of `finish_frame`, after layout.
    pub(crate) fn snapshot_nodes(&mut self) {
        if !self.inspect && !self.dt_inspect {
            // Nobody asked this frame: no copy, and nothing stale to read.
            self.inspected.clear();
            return;
        }
        let tree = &self.tree;
        let n = tree.len();
        let mut out = Vec::with_capacity(n);
        let mut depth = vec![0u16; n];
        // A float root's rank in the paint stack, bottom to top — the stack
        // `emit_frame` left, which is the order it painted the layers in.
        let layer_of: rustc_hash::FxHashMap<Key, u16> = self
            .float_stack
            .iter()
            .enumerate()
            .map(|(pos, &(k, _))| (k, pos as u16 + 1))
            .collect();
        let mut children = vec![0u32; n];
        for i in 1..n {
            children[tree.parent[i] as usize] += 1;
        }
        for i in 0..n {
            let parent = if i == 0 {
                None
            } else {
                let p = tree.parent[i] as usize;
                depth[i] = depth[p] + 1;
                Some(tree.keys[p])
            };
            let spec = &tree.specs[i];
            let (kind, text) = match tree.content[i] {
                NodeContent::Container => (NodeKind::Box, None),
                NodeContent::Text(id) => {
                    let s = self.text.content(id);
                    let cut = s.char_indices().nth(TEXT_CUT).map_or(s.len(), |(i, _)| i);
                    let mut t = s[..cut].to_string();
                    if cut < s.len() {
                        t.push('…');
                    }
                    (NodeKind::Text, Some(t))
                }
                NodeContent::Edit(_) => (NodeKind::Edit, None),
                NodeContent::Image(..) => (NodeKind::Image, None),
                NodeContent::Line(_) => (NodeKind::Line, None),
                NodeContent::Cells(_) => (NodeKind::Cells, None),
                NodeContent::Fragment(_) => (NodeKind::Fragment, None),
                NodeContent::Polygon(_) => (NodeKind::Polygon, None),
            };
            let rect = if i == 0 {
                Rect::new(0.0, 0.0, self.viewport.w, self.viewport.h)
            } else {
                Rect::from_pos_size(tree.pos[i], tree.size[i])
            };
            let ev = spec.events();
            let it = spec.interact();
            let mut flags = Vec::new();
            if ev.on_click.is_some() {
                flags.push("click");
            }
            if ev.on_drag.is_some() {
                flags.push("drag");
            }
            if ev.on_key.is_some() {
                flags.push("key");
            }
            if ev.on_hover.is_some() {
                flags.push("hover");
            }
            if ev.on_drop.is_some() {
                flags.push("drop");
            }
            if ev.on_context_menu.is_some() {
                flags.push("context-menu");
            }
            if ev.modal.is_some() {
                flags.push("modal");
            }
            if spec.hoverable {
                flags.push("hoverable");
            }
            if it.selectable {
                flags.push("selectable");
            }
            if spec.focusable {
                flags.push("focusable");
            }
            if spec.disabled {
                flags.push("disabled");
            }
            if spec.layout.scroll_x || spec.layout.scroll_y {
                flags.push("scroll");
            } else if spec.layout.clip {
                flags.push("clip");
            }
            if spec.transition.is_some() {
                flags.push("transition");
            }
            let mut events = Vec::new();
            for (name, v) in [
                ("click", &ev.on_click),
                ("drag", &ev.on_drag),
                ("key", &ev.on_key),
                ("hover", &ev.on_hover),
                ("drop", &ev.on_drop),
                ("context-menu", &ev.on_context_menu),
                ("force-click", &ev.on_force_click),
                ("layout", &ev.on_layout),
                ("modal", &ev.modal),
            ] {
                if let Some(v) = v {
                    events.push((name, v.clone()));
                }
            }
            let layer =
                if self.tree.any_float && self.float_root.len() > i && self.float_root[i] != NIL {
                    layer_of
                        .get(&tree.keys[self.float_root[i] as usize])
                        .copied()
                        .unwrap_or(0)
                } else {
                    0
                };
            let floor = |m: crate::spec::Min| (!m.is_fit()).then(|| m.resolved());
            let ceiling = |v: f32| v.is_finite().then_some(v);
            let l = &spec.layout;
            out.push(NodeInfo {
                layer,
                origin: tree.origins[i],
                children: children[i],
                padding: l.padding,
                gap: l.gap,
                main_align: l.main_align,
                cross_align: l.cross_align,
                wrap: l.wrap,
                table: l.table,
                min_w: floor(l.min_w),
                min_h: floor(l.min_h),
                max_w: ceiling(l.max_w),
                max_h: ceiling(l.max_h),
                radius: spec.style.radius,
                border_w: spec.style.border_w,
                border_color: spec.style.border_color,
                opacity: spec.style.opacity,
                scroll: (l.scroll_x || l.scroll_y).then(|| self.scroll.offset(tree.keys[i])),
                events,
                key: tree.keys[i],
                parent,
                depth: depth[i],
                kind,
                label: self.key_labels.label_of(tree.keys[i]).map(str::to_string),
                rect,
                dir: spec.layout.dir,
                width: spec.layout.width,
                height: spec.layout.height,
                bg: spec.style.bg,
                float: spec.layout.float.is_some(),
                role: crate::access::derived_role(tree, i),
                text,
                flags,
            });
        }
        self.inspected = out;
    }
}
