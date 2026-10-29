//! The translation every bridge shares: the core's access tree as one
//! AccessKit `TreeUpdate`, AccessKit's action requests as the core's
//! `AccessRequest`, and the [`Feed`] that decides when a window's tree is
//! sent and carries its announcements.
//!
//! Nothing here touches a platform. The runner's bridge
//! (`kui-native/src/access_bridge.rs`) hands what it builds to
//! `accesskit_winit`; the host's ([`crate::HostBridge`]) to the platform
//! adapter on the host's own window.

use accesskit::{
    Action, ActionData, ActionRequest, Affine, Live as AkLive, Node, NodeId,
    Orientation as AkOrientation, Rect, Role as AkRole, TextDirection, TextPosition, TextSelection,
    Toggled, TreeId, TreeInfo, TreeUpdate,
};
use kui_core::{
    AccessAction, AccessNode, AccessRequest, AccessTree, Announcement, Assistive, Key, Live,
    Orientation, Role, TextPos,
};

/// What one window has told its platform adapter: whether a client is
/// listening, which tree went last, and the announcements standing in it.
/// The bridge owns the adapter; this owns when it is spoken to.
pub struct Feed {
    /// Assistive technology asked for the tree and has not gone away.
    active: bool,
    /// Hash of the tree last pushed, and the announcement generation
    /// that went with it; a frame matching both is silent.
    sent: Option<(u64, u64)>,
    /// The nodes standing in for the most recent announcing frame, and
    /// the generation counter that numbers them. AccessKit has no
    /// announcement API — its whole event surface is a tree update —
    /// so an announcement is delivered as a **live node the adapters
    /// see appear**, which is what every platform's live-region event
    /// is derived from.
    announced: Vec<(NodeId, String, Live)>,
    announce_gen: u64,
    /// The next synthetic node id, counting down from `u64::MAX`. A
    /// **fresh** id per announcement is the point: the adapters
    /// announce a live node on `node_added` unconditionally, but on
    /// `node_updated` only when its label changed — so reusing one id
    /// would swallow the same message said twice in a row. `Key` is a
    /// hash of a tree path and never issues from this end.
    next_id: u64,
}

impl Default for Feed {
    fn default() -> Self {
        Self::new()
    }
}

impl Feed {
    pub fn new() -> Self {
        Feed {
            active: false,
            sent: None,
            announced: Vec::new(),
            announce_gen: 0,
            next_id: u64::MAX,
        }
    }

    /// A client asked for the tree: the next [`Feed::publish`] sends all
    /// of it, whatever was sent before.
    pub fn activate(&mut self) {
        self.active = true;
        self.sent = None;
    }

    /// The platform said the client left. Only `accesskit_unix` ever
    /// does; the macOS and Windows adapters take no deactivation handler
    /// (or never call it), so there the reading rises once and stays.
    pub fn deactivate(&mut self) {
        self.active = false;
    }

    pub fn active(&self) -> bool {
        self.active
    }

    /// A client is listening and has not had a tree yet: the frame that
    /// sends one is owed, even if nothing on screen changed.
    pub fn owes_tree(&self) -> bool {
        self.active && self.sent.is_none()
    }

    /// The reading `env.system.assistive` carries: a client has asked
    /// for the tree, or none has.
    pub fn assistive(&self) -> Assistive {
        if self.active {
            Assistive::Listening
        } else {
            Assistive::None
        }
    }

    /// Hands `push` the tree when assistive technology is attached and
    /// the tree, or what was announced, differs from the last one sent.
    /// `said` is this frame's announcements, already taken from the core:
    /// a caller drains them every frame, listening or not.
    pub fn publish(
        &mut self,
        tree: &AccessTree,
        scale: f32,
        said: &[Announcement],
        push: impl FnOnce(TreeUpdate),
    ) {
        if !self.active {
            return;
        }
        if !said.is_empty() {
            // The previous frame's announcement nodes go away as these
            // arrive, and not before: UIA's live-region event carries
            // no text, so the client reads the name back afterwards
            // and a node removed in the update it was announced in is
            // a race.
            self.announce_gen += 1;
            self.announced.clear();
            for a in said {
                self.next_id -= 1;
                self.announced
                    .push((NodeId(self.next_id), a.text.clone(), a.live));
            }
        }
        let stamp = (tree.hash, self.announce_gen);
        if self.sent == Some(stamp) {
            return;
        }
        push(tree_update(tree, scale, &self.announced));
        self.sent = Some(stamp);
    }
}

fn live_of(live: Live) -> AkLive {
    match live {
        Live::Off => AkLive::Off,
        Live::Polite => AkLive::Polite,
        Live::Assertive => AkLive::Assertive,
    }
}

fn pos_of(p: TextPosition) -> TextPos {
    TextPos {
        run: Key(p.node.0),
        character: p.character_index,
    }
}

fn ak_pos(p: TextPos) -> TextPosition {
    TextPosition {
        node: NodeId(p.run.0),
        character_index: p.character,
    }
}

/// The platform role of a node: the one table every bridge reads, so a
/// role added to the core gets its arm here once.
pub fn role_of(n: &AccessNode) -> AkRole {
    match n.role {
        // A live region with no role of its own is ARIA's `status`: an
        // AXGroup with the AXApplicationStatus subrole on macOS, rather
        // than a bare group.
        Role::Group if n.live != Live::Off => AkRole::Status,
        Role::None | Role::Line => AkRole::Unknown,
        Role::Button => AkRole::Button,
        Role::Checkbox => AkRole::CheckBox,
        Role::Radio => AkRole::RadioButton,
        Role::Switch => AkRole::Switch,
        Role::Slider => AkRole::Slider,
        Role::Tab => AkRole::Tab,
        Role::TabList => AkRole::TabList,
        Role::Link => AkRole::Link,
        Role::Heading => AkRole::Heading,
        Role::List => AkRole::List,
        Role::ListItem => AkRole::ListItem,
        Role::Image => AkRole::Image,
        Role::Dialog => AkRole::Dialog,
        Role::Group => AkRole::Group,
        // `accesskit_macos` 0.27 spells these three AXRadioGroup,
        // AXMenu and AXMenuItem (docs/adr/0007).
        Role::RadioGroup => AkRole::RadioGroup,
        Role::Menu => AkRole::Menu,
        Role::MenuItem => AkRole::MenuItem,
        Role::Terminal => AkRole::Terminal,
        Role::Window => AkRole::Window,
        Role::TitleBar => AkRole::TitleBar,
        Role::StaticText => AkRole::Label,
        Role::TextInput => AkRole::TextInput,
        Role::MultilineTextInput => AkRole::MultilineTextInput,
        Role::PasswordInput => AkRole::PasswordInput,
        Role::ScrollView => AkRole::ScrollView,
        // A table of cells (ADR 0070); one whose cells take focus is a
        // reader's grid.
        Role::Table if n.cells_focus => AkRole::Grid,
        Role::Table => AkRole::Table,
        Role::Row => AkRole::Row,
        Role::Cell if n.cells_focus => AkRole::GridCell,
        Role::Cell => AkRole::Cell,
        Role::ColumnHeader => AkRole::ColumnHeader,
        Role::RowHeader => AkRole::RowHeader,
    }
}

fn action_of(action: AccessAction) -> Action {
    match action {
        AccessAction::Click => Action::Click,
        AccessAction::Focus => Action::Focus,
        AccessAction::Blur => Action::Blur,
        AccessAction::SetValue => Action::SetValue,
        AccessAction::Increment => Action::Increment,
        AccessAction::Decrement => Action::Decrement,
        AccessAction::ScrollIntoView => Action::ScrollIntoView,
        AccessAction::ScrollUp => Action::ScrollUp,
        AccessAction::ScrollDown => Action::ScrollDown,
        AccessAction::ScrollLeft => Action::ScrollLeft,
        AccessAction::ScrollRight => Action::ScrollRight,
        AccessAction::SetTextSelection => Action::SetTextSelection,
        AccessAction::ReplaceSelectedText => Action::ReplaceSelectedText,
    }
}

/// A platform's action request as the core's. Expand and Collapse are
/// a click: a node carries `expanded` because clicking it shows or
/// hides something (a disclosure, a select's list), and UIA's
/// ExpandCollapse pattern — the only one AccessKit gives such a node,
/// in place of Invoke — asks for the state it does not have yet, which
/// AccessKit checks before it sends. They were dropped, so a
/// disclosure could not be opened from Narrator.
pub fn request_of(req: ActionRequest) -> Option<AccessRequest> {
    let action = match req.action {
        Action::Click | Action::Expand | Action::Collapse => AccessAction::Click,
        Action::Focus => AccessAction::Focus,
        Action::Blur => AccessAction::Blur,
        Action::SetValue => AccessAction::SetValue,
        Action::Increment => AccessAction::Increment,
        Action::Decrement => AccessAction::Decrement,
        Action::ScrollIntoView => AccessAction::ScrollIntoView,
        Action::ScrollUp => AccessAction::ScrollUp,
        Action::ScrollDown => AccessAction::ScrollDown,
        Action::ScrollLeft => AccessAction::ScrollLeft,
        Action::ScrollRight => AccessAction::ScrollRight,
        Action::SetTextSelection => AccessAction::SetTextSelection,
        Action::ReplaceSelectedText => AccessAction::ReplaceSelectedText,
        _ => return None,
    };
    let mut out = AccessRequest::new(Key(req.target_node.0), action);
    match req.data {
        Some(ActionData::Value(s)) => out.value = Some(s.to_string()),
        Some(ActionData::NumericValue(n)) => out.value = Some(n.to_string()),
        Some(ActionData::SetTextSelection(sel)) => {
            out.anchor = Some(pos_of(sel.anchor));
            out.focus = Some(pos_of(sel.focus));
        }
        _ => {}
    }
    Some(out)
}

fn rect_of(r: kui_core::Rect) -> Rect {
    Rect {
        x0: r.x as f64,
        y0: r.y as f64,
        x1: (r.x + r.w) as f64,
        y1: (r.y + r.h) as f64,
    }
}

/// The whole tree as one update. Rects are logical px in viewport
/// coordinates; the root carries the scale so the platform sees
/// physical px. An editor's runs become `TextRun` children, ahead of
/// its semantic children. `said` are the standing announcement nodes
/// ([`Feed`] keeps them), hung off the root.
pub fn tree_update(tree: &AccessTree, scale: f32, said: &[(NodeId, String, Live)]) -> TreeUpdate {
    let root_key = tree.root().map_or(Key::ROOT, |r| r.key);
    let root = NodeId(root_key.0);
    let mut nodes = Vec::with_capacity(tree.nodes.len().max(1));
    if tree.nodes.is_empty() {
        // No frame yet: a bare window, so the platform has a root.
        nodes.push((root, Node::new(AkRole::Window)));
    }
    for n in &tree.nodes {
        let role = role_of(n);
        let mut node = Node::new(role);
        if let Some(name) = &n.name {
            node.set_label(name.as_str());
        }
        if let Some(d) = &n.description {
            node.set_description(d.as_str());
        }
        // Already cut to the node's clip in the core (F93): AccessKit's
        // hit test reads raw bounds, and `clips_children` cannot say it
        // (an escaping float is the clipper's semantic child, and a
        // plain clip box is not a node at all).
        node.set_bounds(rect_of(n.rect));
        if n.role == Role::Window {
            node.set_transform(Affine::scale(scale as f64));
        }
        let mut children: Vec<NodeId> = Vec::with_capacity(n.runs.len());
        for r in &n.runs {
            let mut run = Node::new(AkRole::TextRun);
            run.set_value(r.text.as_str());
            run.set_bounds(rect_of(r.rect));
            run.set_character_lengths(r.char_lengths.clone());
            run.set_character_positions(r.char_positions.clone());
            run.set_character_widths(r.char_widths.clone());
            run.set_word_starts(r.word_starts.clone());
            run.set_text_direction(if r.rtl {
                TextDirection::RightToLeft
            } else {
                TextDirection::LeftToRight
            });
            let id = NodeId(r.key.0);
            children.push(id);
            nodes.push((id, run));
        }
        children.extend(tree.children(n.key).map(|c| NodeId(c.key.0)));
        if n.key == root_key {
            // The standing announcements hang off the root, after
            // everything the view declared: a reader walking to the end
            // of the window finds the last status message there, which
            // is what an ARIA `status` region is.
            children.extend(said.iter().map(|(id, _, _)| *id));
        }
        node.set_children(children);
        for a in n.action_list() {
            node.add_action(action_of(a));
        }
        if let Some(v) = &n.value {
            node.set_value(v.as_str());
        } else if role == AkRole::Label
            && let Some(name) = &n.name
        {
            // A `Label`'s name is its *value* to AccessKit
            // (`label_comes_from_value`): UIA's Name and AT-SPI's
            // name read the value alone, so a static text with only a
            // label was nameless on Windows and Linux — every line of
            // text, a list row's content, a drawn title. macOS reads
            // the label first and the value after, the same string.
            node.set_value(name.as_str());
        }
        if let (Some(anchor), Some(focus)) = (n.anchor, n.focus) {
            node.set_text_selection(TextSelection {
                anchor: ak_pos(anchor),
                focus: ak_pos(focus),
            });
        }
        if n.mixed {
            node.set_toggled(Toggled::Mixed);
        } else if let Some(c) = n.checked {
            node.set_toggled(if c { Toggled::True } else { Toggled::False });
        }
        // `toggled` and `selected` are different states to AccessKit:
        // a switch is on, a tab is the current one. Both are absent
        // rather than false where the concept does not apply.
        if let Some(c) = n.selected {
            node.set_selected(c);
        }
        if let Some(c) = n.expanded {
            node.set_expanded(c);
        }
        // How a composite arranges its items, so the platform can say
        // so (macOS AXOrientation). Derived from the container's `dir`
        // (docs/adr/0007-composite-keyboard-patterns.md).
        if let Some(o) = n.orientation {
            node.set_orientation(match o {
                Orientation::Horizontal => AkOrientation::Horizontal,
                Orientation::Vertical => AkOrientation::Vertical,
            });
        }
        // AccessKit puts the count on the container and the
        // zero-based ordinal on the item (see `AccessNode::set_size`).
        if let Some(p) = n.pos_in_set {
            node.set_position_in_set(p);
        }
        // A table's rows and columns, and a list's rows (ADR 0070):
        // what a reader announces moving through them, and how it
        // finds a cell's column header (by its column index).
        if let Some(v) = n.row_index {
            node.set_row_index(v);
        }
        if let Some(v) = n.column_index {
            node.set_column_index(v);
        }
        if let Some(v) = n.row_count {
            node.set_row_count(v);
        }
        if let Some(v) = n.column_count {
            node.set_column_count(v);
        }
        if let Some(sz) = n.set_size {
            node.set_size_of_set(sz);
        }
        if n.disabled {
            node.set_disabled();
        }
        if n.modal {
            // aria-modal: a reader confines its cursor to this node
            // (docs/adr/0003-modal-surfaces.md).
            node.set_modal();
        }
        if let Some(v) = n.number {
            node.set_numeric_value(v as f64);
        }
        if let Some(v) = n.min {
            node.set_min_numeric_value(v as f64);
        }
        if let Some(v) = n.max {
            node.set_max_numeric_value(v as f64);
        }
        if let Some(v) = n.step {
            node.set_numeric_value_step(v as f64);
        }
        if let Some(s) = n.scroll {
            node.set_scroll_x(s.x as f64);
            node.set_scroll_x_min(0.0);
            node.set_scroll_x_max(s.max_x as f64);
            node.set_scroll_y(s.y as f64);
            node.set_scroll_y_min(0.0);
            node.set_scroll_y_max(s.max_y as f64);
        }
        if n.live != Live::Off {
            // Liveness inherits down the subtree inside
            // `accesskit_consumer`, so the string a reader hears is
            // the changed descendant's name — kui sets it exactly
            // where the view declared it (ADR 0008, decision 3).
            node.set_live(live_of(n.live));
        }
        nodes.push((NodeId(n.key.0), node));
    }
    for (id, text, live) in said {
        // `Status`, not `Label`: on macOS a `Role::Label`'s announcement
        // is derived from its *value* rather than its label
        // (`label_comes_from_value`), so a live label never speaks.
        let mut node = Node::new(AkRole::Status);
        node.set_label(text.as_str());
        node.set_live(live_of(*live));
        nodes.push((*id, node));
    }
    TreeUpdate {
        nodes,
        tree: Some(TreeInfo::new(root)),
        tree_id: TreeId::ROOT,
        focus: NodeId(tree.focus.unwrap_or(root_key).0),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kui_core::{Core, NodeSpec, Size, TextStyle};

    /// A static text reaches every platform with its words: AccessKit
    /// names a `Label` by its value, which is what UIA's Name and
    /// AT-SPI's name read, so the text is the value as well as the
    /// label. A row's text was nameless under UI Automation, and so
    /// was every line of text in every window.
    #[test]
    fn a_static_text_is_named_by_its_value() {
        let mut core = Core::new();
        let mut ui = core.frame(Size::new(200.0, 100.0), 1.0);
        ui.text("Hello", TextStyle::new(14.0));
        ui.leaf_keyed("b", NodeSpec::row().on_click("b").label("Save"));
        ui.finish();
        let update = tree_update(core.access_tree(), 1.0, &[]);
        let label = update
            .nodes
            .iter()
            .map(|(_, n)| n)
            .find(|n| n.role() == AkRole::Label)
            .expect("a Label for the text");
        assert_eq!(label.value(), Some("Hello"));
        assert_eq!(label.label(), Some("Hello"));
        // A button keeps its name where its role reads it, and no value.
        let button = update
            .nodes
            .iter()
            .map(|(_, n)| n)
            .find(|n| n.role() == AkRole::Button)
            .expect("a Button");
        assert_eq!((button.label(), button.value()), (Some("Save"), None));
    }

    /// UIA's Expand and Collapse reach the node as the click that
    /// shows or hides what it discloses; an action kui has no answer
    /// for still reaches nothing.
    #[test]
    fn expand_and_collapse_are_a_click() {
        let req = |action| ActionRequest {
            action,
            target_tree: TreeId::ROOT,
            target_node: NodeId(7),
            data: None,
        };
        for a in [Action::Click, Action::Expand, Action::Collapse] {
            let out = request_of(req(a)).expect("a request");
            assert_eq!(
                (out.key, out.action),
                (Key(7), AccessAction::Click),
                "{a:?}"
            );
        }
        assert!(request_of(req(Action::ShowContextMenu)).is_none());
    }

    /// The feed is silent until a client asks, sends the whole tree on
    /// the ask, and after that only a frame whose tree or announcements
    /// changed: a frame that changed nothing pushes nothing.
    #[test]
    fn the_feed_sends_on_the_ask_and_on_change_alone() {
        let mut core = Core::new();
        let frame = |core: &mut Core, label: &str| {
            let mut ui = core.frame(Size::new(200.0, 100.0), 2.0);
            ui.leaf_keyed("b", NodeSpec::row().on_click("b").label(label));
            ui.finish();
        };
        frame(&mut core, "Save");
        let mut feed = Feed::new();
        let mut pushed = 0;
        feed.publish(core.access_tree(), 2.0, &[], |_| pushed += 1);
        assert_eq!(
            (pushed, feed.assistive()),
            (0, Assistive::None),
            "nobody asked"
        );

        feed.activate();
        assert!(feed.owes_tree());
        feed.publish(core.access_tree(), 2.0, &[], |u| {
            pushed += 1;
            assert!(u.tree.is_some(), "the first update is a whole tree");
        });
        assert_eq!((pushed, feed.owes_tree()), (1, false));
        assert_eq!(feed.assistive(), Assistive::Listening);

        frame(&mut core, "Save");
        feed.publish(core.access_tree(), 2.0, &[], |_| pushed += 1);
        assert_eq!(pushed, 1, "the same tree again is silent");

        let said = [Announcement {
            text: "Saved".into(),
            live: Live::Polite,
        }];
        feed.publish(core.access_tree(), 2.0, &said, |u| {
            pushed += 1;
            let status = u.nodes.iter().find(|(_, n)| n.role() == AkRole::Status);
            assert_eq!(status.and_then(|(_, n)| n.label()), Some("Saved"));
        });
        assert_eq!(pushed, 2, "an announcement is a change");

        frame(&mut core, "Saved");
        feed.publish(core.access_tree(), 2.0, &[], |_| pushed += 1);
        assert_eq!(pushed, 3, "so is a new name");
    }
}
