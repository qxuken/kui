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
use crate::geom::Rect;
use crate::spec::{Dir, Sizing};
use crate::tree::NodeContent;

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
    /// Where layout put it, logical viewport px.
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
}

const TEXT_CUT: usize = 60;

impl Core {
    /// Turns the per-frame snapshot on or off (see the module doc). Off by
    /// default; a devtool that reads [`Self::nodes`] turns it on once.
    pub fn set_inspect(&mut self, on: bool) {
        self.inspect = on;
        if !on {
            self.inspected.clear();
        }
    }

    /// The last finished frame's nodes, in tree order — empty until
    /// [`Self::set_inspect`] asked for them and a frame has finished since.
    pub fn nodes(&self) -> &[NodeInfo] {
        &self.inspected
    }

    /// Called at the end of `finish_frame`, after layout.
    pub(crate) fn snapshot_nodes(&mut self) {
        if !self.inspect {
            return;
        }
        let tree = &self.tree;
        let n = tree.len();
        let mut out = Vec::with_capacity(n);
        let mut depth = vec![0u16; n];
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
                NodeContent::Image(_) => (NodeKind::Image, None),
                NodeContent::Line(_) => (NodeKind::Line, None),
                NodeContent::Cells(_) => (NodeKind::Cells, None),
                NodeContent::Fragment(_) => (NodeKind::Fragment, None),
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
            out.push(NodeInfo {
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
