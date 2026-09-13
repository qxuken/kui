//! Per-frame UI tree: flat arrays rebuilt every frame, capacities retained.
//! Nodes are stored in DFS preorder (a parent always precedes its children,
//! and preorder equals paint order), linked via first_child/next_sibling.

use crate::geom::{Size, Vec2};
use crate::key::Key;
use crate::spec::NodeSpec;

pub const NIL: u32 = u32::MAX;

/// Which frontend produced a node: 0 is the host app, extensions get 1+.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct OriginId(pub u16);

impl OriginId {
    pub const HOST: OriginId = OriginId(0);
    /// The core's own devtools panel (`docs/adr/0024`): a node opened
    /// under it is the panel's, and an event that carries it is acted on
    /// inside `handle_input` and never handed out. Reserved at the top of
    /// the range so no extension list ever reaches it.
    pub const DEVTOOLS: OriginId = OriginId(u16::MAX);
    /// The core's own context menu (`docs/adr/0017`, decision 5) and the
    /// menu bar it draws (`docs/adr/0018`): the same isolation the
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
    /// box as `opts` say (ADR 0025).
    Image(crate::resources::ImageId, crate::resources::ImageOpts),
    /// A stroke through a run of points: one segment quad per straight
    /// piece (see `crate::line`). The node is a float sized to the
    /// stroke's bounding box, and its `bg` is the stroke colour.
    Line(crate::line::LineId),
    /// A cell grid (see `crate::cells`): a terminal's screen as one node.
    Cells(crate::cells::CellsId),
    /// A box a registered WGSL function paints (see `crate::fragment` and
    /// `docs/adr/0015-a-fragment-element-and-the-painter-it-is-not.md`).
    /// The handle and the sixteen parameters live in the frame's
    /// `FragmentList`; the node carries only where.
    Fragment(crate::fragment::FragmentDrawId),
    /// A filled polygon (ADR 0025, decision 6): a float sized to its own
    /// bounding box like a line, painted by the stock polygon fragment
    /// whose draw sits in the frame's `FragmentList` like any fragment's,
    /// its `bg` the fill. No hit region, no access row.
    Polygon(crate::fragment::FragmentDrawId),
}

impl Tree {
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

    // Set by `push`, cleared by `clear`: what this frame declared at all,
    // so a pass whose work exists for one feature can skip it wholesale
    // when no node asked for that feature. A float check in a layout pass
    // is a scattered read through the spec of every child of every node;
    // behind a flag that is false on nearly every frame it is one
    // predicted branch (C15).
    /// Whether any node declares `float`.
    pub any_float: bool,
    /// Whether any node declares `wrap_children`.
    pub any_wrap: bool,
    /// Whether any node is text (a `Text` or `Edit` content).
    pub any_text: bool,
    /// Whether any node is a `role="line"` row — what a pointer payload
    /// inside a key sink is resolved against (backlog C34), so a frame
    /// without a custom editor never walks a sink's subtree for one.
    pub any_line: bool,
    /// Whether any node clips (`clip`, or an overflow that scrolls).
    pub any_clip: bool,
    /// Whether any node clips *and* has a radius, so the clip its
    /// descendants inherit is rounded. Separate from `any_clip`: the
    /// per-corner bookkeeping is skipped for the ordinary square clip.
    pub any_rounded_clip: bool,
    /// Whether any node fades (`opacity` below one).
    pub any_opacity: bool,
    /// Whether any node declares `modal` (ADR 0003).
    pub any_modal: bool,
    /// Whether any node declares `focus_region` (ADR 0022).
    pub any_region: bool,
    /// Whether any node eases its position (`slide`, or an `enter` with
    /// an offset) under a transition.
    pub any_slide: bool,
    /// Whether any node declares `on_layout`, so the rect report can skip
    /// the walk.
    pub any_layout: bool,
    /// Whether any node declares `on_context_menu`, so a hit region's
    /// walk for the menu it inherits (backlog T1) is skipped wholesale on
    /// a frame that offers none.
    pub any_context_menu: bool,
    /// Some node declared `on_scroll`; emission reads the row per node
    /// only then (ADR 0029, decision 4).
    pub any_scroll_handler: bool,
    /// Whether any node declares a workable `exit` (one under a
    /// transition). Gates the tree swap and the key diff.
    pub any_exit: bool,
    /// Whether any node asked for the next frame (`animate`): one node
    /// asking is the whole window asking.
    pub any_animate: bool,
    /// The data index of every node opened with one (`open_indexed`), by
    /// node. A side list rather than a column, because it is a virtual
    /// list's rows and nothing else: a frame that builds none is one empty
    /// `Vec` (C15's rule about what every node pays for).
    ///
    /// What it is for: a selection endpoint in a row that is *not built*
    /// can still be ordered against the rows that are, because a row's
    /// index says where it sits in the data even when nothing on screen
    /// says where it sits in the frame (ADR 0017, decisions 2 and 3).
    pub indexed: Vec<(u32, u64)>,
    /// Whether any node declares `selectable` (ADR 0017). False on every
    /// frame of an app that never asks for one, which is what keeps the
    /// scope walk and the off-screen places of tier 2 off those frames
    /// entirely.
    pub any_selectable: bool,
    /// The box a `FloatConfig::viewport()` float of the host's resolves
    /// against, in window coordinates: the whole window, or what the
    /// devtools' dock leaves of it (`docs/adr/0024`). A zero rect means
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
        self.any_float = false;
        self.any_wrap = false;
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
        self.any_exit = false;
        self.any_animate = false;
        self.indexed.clear();
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
        self.any_float |= spec.layout.float.is_some();
        self.any_wrap |= spec.layout.wrap;
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
