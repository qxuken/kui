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
    /// A host-registered image (see `Resources`), drawn via the atlas.
    Image(crate::resources::ImageId),
    /// A stroke through a run of points: one segment quad per straight
    /// piece (see `crate::line`). The node is a float sized to the
    /// stroke's bounding box, and its `bg` is the stroke colour.
    Line(crate::line::LineId),
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
    }

    pub fn push(
        &mut self,
        parent: u32,
        key: Key,
        origin: OriginId,
        spec: NodeSpec,
        content: NodeContent,
    ) -> u32 {
        let idx = self.keys.len() as u32;
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
