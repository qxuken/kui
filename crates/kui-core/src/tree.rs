//! [`Tree`]: the per-frame flat tree the core builds, lays out and emits.
//!
//! The tree is a set of parallel `Vec`s, one entry per node, rebuilt from
//! scratch every frame with their capacities retained. Nodes are stored in
//! DFS preorder (a parent always precedes its children, and preorder is
//! paint order) and linked through `parent`, `first_child` and
//! `next_sibling` indices. Layout writes `size` and `pos` into it; emission
//! reads them. [`OriginId`] says which frontend (the host app, an
//! extension, the core's own surfaces) opened each node.
//!
//! An app never touches this type: it builds through [`Ui`](crate::ui::Ui)
//! and reads back through `Core`. It is public for a custom runner or a
//! test that drives [`layout::compute`](crate::layout::compute) directly.

use crate::geom::{Size, Vec2};
use crate::key::Key;
use crate::spec::NodeSpec;

pub const NIL: u32 = u32::MAX;

/// Which frontend produced a node: 0 is the host app, extensions get 1+.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct OriginId(pub u16);

impl OriginId {
    pub const HOST: OriginId = OriginId(0);
    /// The core's own devtools panel: a node opened
    /// under it is the panel's, and an event that carries it is acted on
    /// inside `handle_input` and never handed out. Reserved at the top of
    /// the range so no extension list ever reaches it.
    pub const DEVTOOLS: OriginId = OriginId(u16::MAX);
    /// The core's own context menu and the menu bar it draws: the same
    /// isolation the
    /// devtools have — a node opened under one of these is the surface's,
    /// its events are taken back inside `handle_input` and never handed
    /// out, and no key list has to remember which nodes those were.
    pub const MENU: OriginId = OriginId(u16::MAX - 1);
    pub const MENU_BAR: OriginId = OriginId(u16::MAX - 2);

    /// Whether nodes of this origin are one of the core's own surfaces.
    pub fn is_core_surface(self) -> bool {
        matches!(self, Self::DEVTOOLS | Self::MENU | Self::MENU_BAR)
    }
}

/// Index into the frame's text list (owned by `TextSystem`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TextId(pub u32);

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum NodeContent {
    Container,
    Text(TextId),
    /// Editable text; retained state lives in the core's `EditStore`.
    Edit(Key),
    /// A host-registered image (see `Resources`), drawn from the atlas or
    /// from a texture of its own as the entry's backing says, met by its
    /// box as `opts` say.
    Image(crate::resources::ImageId, crate::resources::ImageOpts),
    /// A stroke through a run of points: one segment quad per straight
    /// piece (see `crate::line`). The node is a float sized to the
    /// stroke's bounding box, so it takes no room, and its `bg` is the
    /// stroke colour. In its parent's box space it paints in the parent's
    /// layer at its place in the tree ([`NodeContent::drawn_in_parent`]).
    Line(crate::line::LineId),
    /// A cell grid (see `crate::cells`): a terminal's screen as one node.
    Cells(crate::cells::CellsId),
    /// A box a registered WGSL function paints (see `crate::fragment`).
    /// The handle and the sixteen parameters live in the frame's
    /// `FragmentList`; the node carries only where.
    Fragment(crate::fragment::FragmentDrawId),
    /// A filled polygon: a float sized to its own
    /// bounding box like a line, painted by the stock polygon fragment
    /// whose draw sits in the frame's `FragmentList` like any fragment's,
    /// its `bg` the fill. No hit region, no access row.
    Polygon(crate::fragment::FragmentDrawId),
    /// A filled and/or stroked outline of any shape (see `crate::path`):
    /// a float sized to its own bounding box like a line, its ops in the
    /// frame's `PathStore`, drawn as glyph-mask quads from the atlas. Its
    /// `bg` is the fill, its border width and colour the stroke.
    Path(crate::path::PathId),
}

impl NodeContent {
    /// Whether this is a shape the core floats for its own reasons — a
    /// `line`, `polygon` or `path`, a float only so it takes no room in a
    /// row or column (ADR 0010 decision 5). Such a node is its parent's
    /// content as a child is: the parent's clip holds it (F78), and it
    /// paints in the parent's layer at its place in the tree rather than
    /// opening a layer of its own (F123) — a glyph drawn into a title bar
    /// is not over a toast that opened before the bar did.
    pub fn drawn_in_parent(self) -> bool {
        matches!(self, Self::Line(_) | Self::Polygon(_) | Self::Path(_))
    }
}

impl Tree {
    /// Whether node `i` opens a float layer of its own (ADR 0023): a float
    /// a view declared, or a stroke anchored to the viewport. A `line`,
    /// `polygon` or `path` in its parent's box space does not — the core
    /// made its float so it takes no room, and it paints where a child
    /// would ([`NodeContent::drawn_in_parent`]). Reads the float's
    /// anchor through [`FloatConfig::clipped_by_parent`], which the core
    /// sets for every such shape, so the one rule serves the clip and the
    /// layer.
    ///
    /// [`FloatConfig::clipped_by_parent`]: crate::spec::FloatConfig::clipped_by_parent
    pub fn opens_layer(&self, i: usize) -> bool {
        self.specs[i]
            .layout
            .float
            .is_some_and(|f| !(f.clipped_by_parent() && self.content[i].drawn_in_parent()))
    }

    /// Whether node `i` is drawn in its parent's layer, as a child is,
    /// although it floats: [`Tree::opens_layer`]'s complement for a node
    /// that declares `float` at all.
    pub fn floats_in_parent(&self, i: usize) -> bool {
        self.specs[i].layout.float.is_some() && !self.opens_layer(i)
    }

    /// One past the last node of `i`'s subtree. Preorder storage makes a
    /// subtree a contiguous index range ending at the next node that is a
    /// sibling of `i` or of one of its ancestors.
    pub fn subtree_end(&self, i: usize) -> usize {
        let mut n = i as u32;
        loop {
            if self.next_sibling[n as usize] != NIL {
                return self.next_sibling[n as usize] as usize;
            }
            n = self.parent[n as usize];
            if n == NIL {
                return self.len();
            }
        }
    }
}

/// One frame's nodes as parallel arrays in preorder; see the
/// [module docs](self).
#[derive(Default)]
pub struct Tree {
    pub keys: Vec<Key>,
    pub origins: Vec<OriginId>,
    pub specs: Vec<NodeSpec>,
    pub content: Vec<NodeContent>,

    pub parent: Vec<u32>,
    pub first_child: Vec<u32>,
    pub last_child: Vec<u32>,
    pub next_sibling: Vec<u32>,

    // Filled by the layout pass.
    pub size: Vec<Size>,
    pub pos: Vec<Vec2>,
    /// Max scroll offset per axis (zero for non-scroll nodes).
    pub scroll_max: Vec<Vec2>,
    /// Which wrap line of its parent a node sits on, from the main-axis
    /// pass. Zero everywhere but under a wrapping container, and the
    /// in-flow children of one line are always a contiguous sibling run,
    /// so a line is a range rather than a list.
    pub line: Vec<u32>,
    /// Each node's first baseline below its top, logical px, where text
    /// measured one (`NaN` elsewhere). Filled by the fit-height pass, and
    /// only on a frame with a baseline row (`any_baseline`); empty
    /// otherwise.
    pub baseline: Vec<f32>,

    // Set by `push`, cleared by `clear`: what this frame declared at all,
    // so a pass whose work exists for one feature can skip it wholesale
    // when no node asked for that feature. A float check in a layout pass
    // is a scattered read through the spec of every child of every node;
    // behind a flag that is false on nearly every frame it is one
    // predicted branch.
    /// Whether any node declares `float`.
    pub any_float: bool,
    /// Whether any node declares a size expression as a clamp (a
    /// negative `max_w`, a `Min::calc`): layout resolves
    /// them only then.
    pub any_calc_bound: bool,
    /// Whether any float is anchored to a node by key
    /// (`FloatAnchor::Node`): the sixth layout pass runs only then.
    pub any_node_float: bool,
    /// Whether any node declares `wrap_children`.
    pub any_wrap: bool,
    /// Whether any node lines its children up by their baselines
    /// (`cross_align: Baseline`): the layout measures baselines only then.
    pub any_baseline: bool,
    /// Whether any node is a table (`LayoutSpec::table`): the column
    /// alignment in the layout passes runs only on a frame that has one.
    pub any_table: bool,
    /// Whether any node declares a `gradient`: emission looks for one
    /// only on a frame that has one.
    pub any_gradient: bool,
    /// Whether any node is text (a `Text` or `Edit` content).
    pub any_text: bool,
    /// Whether any node is a `role="line"` row — what a pointer payload
    /// inside a key sink is resolved against, so a frame
    /// without a custom editor never walks a sink's subtree for one.
    pub any_line: bool,

    /// Scratch for the layout pass's freeze loop (`distribute_run`): one
    /// byte per in-flow child of the run being resolved, in child order.
    /// Sized per run and never cleared, so the allocation is made once
    /// and reused by every run of every frame.
    pub grow_scratch: Vec<u8>,
    /// Scratch for the shrink CSS's way (`layout::shrink_as_css`): one
    /// entry per child of the run giving, made once and reused
    /// by every overflowing run of every frame, as `grow_scratch` is.
    pub(crate) shrink_scratch: Vec<crate::layout::Give>,
    /// Whether any node clips (`clip`, or an overflow that scrolls).
    pub any_clip: bool,
    /// Whether any node clips *and* has a radius, so the clip its
    /// descendants inherit is rounded. Separate from `any_clip`: the
    /// per-corner bookkeeping is skipped for the ordinary square clip.
    pub any_rounded_clip: bool,
    /// Whether any node fades (`opacity` below one).
    pub any_opacity: bool,
    /// Whether any node declares `modal`.
    pub any_modal: bool,
    /// Whether any node declares `focus_region`.
    pub any_region: bool,
    /// Whether any node eases its position (`slide`, or an `enter` with
    /// an offset) under a transition.
    pub any_slide: bool,
    /// Whether any node declares `on_layout`, so the rect report can skip
    /// the walk.
    pub any_layout: bool,
    /// Whether any node declares `on_context_menu`, so a hit region's
    /// walk for the menu it inherits is skipped wholesale on
    /// a frame that offers none.
    pub any_context_menu: bool,
    /// Some node declared `on_scroll`; emission reads the row per node
    /// only then.
    pub any_scroll_handler: bool,
    /// Some node declared `on_drop`: a hit region's walk for
    /// the zone it inherits is skipped wholesale on a frame with none.
    pub any_drop: bool,
    /// Whether any node declares a workable `exit` (one under a
    /// transition). Gates the tree swap and the key diff.
    pub any_exit: bool,
    /// Whether any node asked for the next frame (`animate`): one node
    /// asking is the whole window asking.
    pub any_animate: bool,
    /// The data index of every node opened with one (`open_indexed`), by
    /// node. A side list rather than a column, because it is a virtual
    /// list's rows and nothing else: a frame that builds none is one empty
    /// `Vec`.
    ///
    /// What it is for: a selection endpoint in a row that is *not built*
    /// can still be ordered against the rows that are, because a row's
    /// index says where it sits in the data even when nothing on screen
    /// says where it sits in the frame.
    pub indexed: Vec<(u32, u64)>,
    /// How many indexed rows a node's virtual list has, built or not
    /// (`rowCount`), by node. A side list for the reason `indexed` is one.
    /// What it is for: Select All inside a `selectable` virtual list is
    /// the *data*, rows `0..count`, not the rows the frame happened to
    /// build — and the count is the one thing about the data the core
    /// cannot see.
    pub row_counts: Vec<(u32, u64)>,
    /// The node range every slot fill opened, by the slot's key: `(slot,
    /// first, end)` over node indices, innermost fill first (a fill
    /// records itself after the fills inside it). A side list for the
    /// reason `indexed` is one — a frame with no extension is one empty
    /// `Vec` — and what stamps `UiEvent::slot`, so a host that fills many
    /// slots from one extension can route an event by the slot it came
    /// from without stamping every payload.
    pub fills: Vec<(Key, u32, u32)>,
    /// Whether any node declares `selectable`. False on every
    /// frame of an app that never asks for one, which is what keeps the
    /// scope walk and the off-screen places of tier 2 off those frames
    /// entirely.
    pub any_selectable: bool,
    /// The box a `FloatConfig::viewport()` float of the host's resolves
    /// against, in window coordinates: the whole window, or what the
    /// devtools' dock leaves of it. A zero rect means
    /// the window. The devtools' own nodes always use the window.
    pub host_area: crate::geom::Rect,
}

impl Tree {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn len(&self) -> usize {
        self.keys.len()
    }

    pub fn is_empty(&self) -> bool {
        self.keys.is_empty()
    }

    /// The index of the node `key` names in this frame, if it is here. A
    /// linear scan: the one place to swap it for a map if a profile asks.
    #[inline]
    pub fn index_of(&self, key: Key) -> Option<usize> {
        self.keys.iter().position(|k| *k == key)
    }

    /// The parent an ancestor walk that means "where is this shown"
    /// takes: the node's parent, except for a float anchored to a node by
    /// key, whose walk continues from the anchor (`FloatAnchor::Node`).
    /// `NIL` past the root, and for an anchor the frame does not have.
    #[inline]
    pub fn region_parent(&self, i: usize) -> u32 {
        if self.any_node_float
            && let Some(crate::spec::FloatConfig {
                anchor: crate::spec::FloatAnchor::Node(key),
                ..
            }) = self.specs[i].layout.float
        {
            return self.index_of(key).map_or(NIL, |a| a as u32);
        }
        self.parent[i]
    }

    /// Clears contents but keeps allocations for the next frame.
    pub fn clear(&mut self) {
        self.keys.clear();
        self.origins.clear();
        self.specs.clear();
        self.content.clear();
        self.parent.clear();
        self.first_child.clear();
        self.last_child.clear();
        self.next_sibling.clear();
        self.size.clear();
        self.pos.clear();
        self.scroll_max.clear();
        self.line.clear();
        self.baseline.clear();
        self.any_float = false;
        self.any_calc_bound = false;
        self.any_baseline = false;
        self.any_node_float = false;
        self.any_wrap = false;
        self.any_table = false;
        self.any_gradient = false;
        self.any_text = false;
        self.any_line = false;
        self.any_selectable = false;
        self.any_clip = false;
        self.any_rounded_clip = false;
        self.any_opacity = false;
        self.any_modal = false;
        self.any_region = false;
        self.any_slide = false;
        self.any_layout = false;
        self.any_context_menu = false;
        self.any_scroll_handler = false;
        self.any_drop = false;
        self.any_exit = false;
        self.any_animate = false;
        self.indexed.clear();
        self.row_counts.clear();
        self.fills.clear();
    }

    /// The innermost slot fill node `i` was opened inside, if any.
    pub fn slot_of(&self, i: usize) -> Option<Key> {
        let i = i as u32;
        self.fills
            .iter()
            .find(|(_, first, end)| (*first..*end).contains(&i))
            .map(|(slot, _, _)| *slot)
    }

    /// Notes what a spec asks of the frame, so a pass whose work exists
    /// for one feature can skip it when no node declared that feature.
    /// Called by `push` for every node, and by the root paths that
    /// replace a spec in place — the one door, so a leaf cannot forget a
    /// flag a box would have set (an `image` once set two of these and
    /// painted opaque when it was the frame's only fade).
    ///
    /// Each boxed group is tested once, not once per flag it can set: a
    /// node declaring no events and no animation is done after two null
    /// checks (C15).
    #[inline]
    pub fn note(&mut self, spec: &NodeSpec, content: &NodeContent) {
        if let Some(f) = spec.layout.float {
            self.any_float = true;
            self.any_node_float |= matches!(f.anchor, crate::spec::FloatAnchor::Node(_));
        }
        self.any_wrap |= spec.layout.wrap;
        let l = &spec.layout;
        self.any_calc_bound |= l.max_w < 0.0
            || l.max_h < 0.0
            || l.min_w.as_calc().is_some()
            || l.min_h.as_calc().is_some();
        self.any_table |= spec.layout.is_table();
        self.any_gradient |= spec.interact().gradient.is_some();
        self.any_baseline |= spec.layout.cross_align == crate::spec::Align::Baseline;
        self.any_text |= matches!(
            content,
            NodeContent::Text(_) | NodeContent::Edit(_) | NodeContent::Cells(_)
        );
        // Through the box rather than through `interact()`: a node that
        // declares no interaction group is answered by one null check
        // instead of a read through the empty static (C15).
        if let Some(i) = spec.interact.as_deref() {
            self.any_selectable |= i.selectable;
            self.any_region |= i.focus_region;
        }
        if let Some(a) = spec.access.as_deref() {
            self.any_line |= a.role == Some(crate::access::Role::Line);
        }
        if spec.layout.clips() {
            self.any_clip = true;
            self.any_rounded_clip |= spec.style.radius != crate::display::SQUARE;
        }
        self.any_opacity |= spec.style.opacity < 1.0;
        self.any_animate |= spec.animate;
        if let Some(events) = spec.events.as_deref() {
            self.any_modal |= events.modal.is_some();
            self.any_layout |= events.on_layout.is_some();
            self.any_context_menu |= events.on_context_menu.is_some();
            self.any_scroll_handler |= events.on_scroll.is_some();
            self.any_drop |= events.on_drop.is_some();
        }
        if spec.transition.is_some() {
            match spec.anim.as_deref() {
                Some(anim) => {
                    self.any_slide |= spec.slide || anim.enter.is_some_and(|e| e.offsets());
                    self.any_exit |= anim.exit.is_some();
                }
                None => self.any_slide |= spec.slide,
            }
        }
    }

    #[inline]
    pub fn push(
        &mut self,
        parent: u32,
        key: Key,
        origin: OriginId,
        spec: NodeSpec,
        content: NodeContent,
    ) -> u32 {
        let idx = self.keys.len() as u32;
        // Read before the move, while the spec is in cache anyway.
        self.note(&spec, &content);
        self.keys.push(key);
        self.origins.push(origin);
        self.specs.push(spec);
        self.content.push(content);
        self.parent.push(parent);
        self.first_child.push(NIL);
        self.last_child.push(NIL);
        self.next_sibling.push(NIL);
        self.size.push(Size::ZERO);
        self.pos.push(Vec2::ZERO);
        self.scroll_max.push(Vec2::ZERO);
        self.line.push(0);

        if parent != NIL {
            let p = parent as usize;
            if self.first_child[p] == NIL {
                self.first_child[p] = idx;
            } else {
                let last = self.last_child[p] as usize;
                self.next_sibling[last] = idx;
            }
            self.last_child[p] = idx;
        }
        idx
    }

    pub fn children(&self, i: u32) -> ChildIter<'_> {
        ChildIter {
            tree: self,
            next: self.first_child[i as usize],
        }
    }
}

pub struct ChildIter<'a> {
    tree: &'a Tree,
    next: u32,
}

impl Iterator for ChildIter<'_> {
    type Item = u32;

    fn next(&mut self) -> Option<u32> {
        if self.next == NIL {
            return None;
        }
        let cur = self.next;
        self.next = self.tree.next_sibling[cur as usize];
        Some(cur)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sibling_links() {
        let mut t = Tree::new();
        let root = t.push(
            NIL,
            Key::ROOT,
            OriginId::HOST,
            NodeSpec::default(),
            NodeContent::Container,
        );
        let a = t.push(
            root,
            Key::ROOT.index(0),
            OriginId::HOST,
            NodeSpec::default(),
            NodeContent::Container,
        );
        let a1 = t.push(
            a,
            Key::ROOT.index(0).index(0),
            OriginId::HOST,
            NodeSpec::default(),
            NodeContent::Container,
        );
        let b = t.push(
            root,
            Key::ROOT.index(1),
            OriginId::HOST,
            NodeSpec::default(),
            NodeContent::Container,
        );

        assert_eq!(t.children(root).collect::<Vec<_>>(), vec![a, b]);
        assert_eq!(t.children(a).collect::<Vec<_>>(), vec![a1]);
        assert_eq!(t.children(b).collect::<Vec<_>>(), Vec::<u32>::new());
        // Preorder invariant: parents precede children.
        assert!(root < a && a < a1 && a1 < b);
    }
}
