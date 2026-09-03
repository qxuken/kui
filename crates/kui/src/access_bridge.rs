//! The AccessKit bridge: the core's access tree (data) becomes the
//! platform accessibility tree (UIA, NSAccessibility, AT-SPI) through
//! `accesskit_winit`, and the platform's requests come back as
//! `InputEvent::Access`. See `docs/adr/0001-accessibility-as-data.md`.
//!
//! Nothing here runs until assistive technology asks for the tree: the
//! adapter reports activation through the event loop, and only then does
//! the shell derive and push trees — on frames whose tree hash changed.

#[cfg(feature = "accesskit")]
mod imp {
    use accesskit::{
        Action, ActionData, Affine, Node, NodeId, Rect, Role as AkRole, TextDirection,
        TextPosition, TextSelection, Toggled, TreeId, TreeInfo, TreeUpdate,
    };
    use accesskit_winit::{Adapter, Event, WindowEvent as AkWindowEvent};
    use kui_core::{AccessAction, AccessRequest, AccessTree, Key, Role, TextPos};
    use winit::event::WindowEvent;
    use winit::event_loop::{ActiveEventLoop, EventLoopProxy};
    use winit::window::Window;

    /// The runner's event-loop user event: what AccessKit sends back.
    #[derive(Debug)]
    pub enum UserEvent {
        Access(Event),
    }

    impl From<Event> for UserEvent {
        fn from(e: Event) -> Self {
            UserEvent::Access(e)
        }
    }

    pub struct Bridge {
        adapter: Adapter,
        /// Assistive technology asked for the tree and has not gone away.
        active: bool,
        /// Hash of the tree last pushed; a frame with the same one is silent.
        sent: Option<u64>,
    }

    impl Bridge {
        /// Must run before the window is shown (the platform adapters hook
        /// the window at creation).
        pub fn new(
            event_loop: &ActiveEventLoop,
            window: &Window,
            proxy: EventLoopProxy<UserEvent>,
        ) -> Option<Self> {
            Some(Bridge {
                adapter: Adapter::with_event_loop_proxy(event_loop, window, proxy),
                active: false,
                sent: None,
            })
        }

        pub fn process_event(&mut self, window: &Window, event: &WindowEvent) {
            self.adapter.process_event(window, event);
        }

        pub fn active(&self) -> bool {
            self.active
        }

        /// Folds an AccessKit event in; an action request comes back as
        /// the core's input to dispatch.
        pub fn on_event(&mut self, ev: UserEvent) -> Option<AccessRequest> {
            let UserEvent::Access(ev) = ev;
            match ev.window_event {
                AkWindowEvent::InitialTreeRequested => {
                    self.active = true;
                    self.sent = None;
                    None
                }
                AkWindowEvent::AccessibilityDeactivated => {
                    self.active = false;
                    None
                }
                AkWindowEvent::ActionRequested(req) => {
                    let action = match req.action {
                        Action::Click => AccessAction::Click,
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
            }
        }

        /// Pushes the tree when assistive technology is attached and the
        /// tree differs from the last one sent.
        pub fn publish(&mut self, tree: &AccessTree, scale: f32) {
            if !self.active || self.sent == Some(tree.hash) {
                return;
            }
            self.adapter.update_if_active(|| tree_update(tree, scale));
            self.sent = Some(tree.hash);
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

    fn role_of(role: Role) -> AkRole {
        match role {
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
            Role::Window => AkRole::Window,
            Role::TitleBar => AkRole::TitleBar,
            Role::StaticText => AkRole::Label,
            Role::TextInput => AkRole::TextInput,
            Role::MultilineTextInput => AkRole::MultilineTextInput,
            Role::ScrollView => AkRole::ScrollView,
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
    /// its semantic children.
    fn tree_update(tree: &AccessTree, scale: f32) -> TreeUpdate {
        let root_key = tree.root().map_or(Key::ROOT, |r| r.key);
        let root = NodeId(root_key.0);
        let mut nodes = Vec::with_capacity(tree.nodes.len().max(1));
        if tree.nodes.is_empty() {
            // No frame yet: a bare window, so the platform has a root.
            nodes.push((root, Node::new(AkRole::Window)));
        }
        for n in &tree.nodes {
            let mut node = Node::new(role_of(n.role));
            if let Some(name) = &n.name {
                node.set_label(name.as_str());
            }
            if let Some(d) = &n.description {
                node.set_description(d.as_str());
            }
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
            node.set_children(children);
            for a in n.action_list() {
                node.add_action(action_of(a));
            }
            if let Some(v) = &n.value {
                node.set_value(v.as_str());
            }
            if let (Some(anchor), Some(focus)) = (n.anchor, n.focus) {
                node.set_text_selection(TextSelection {
                    anchor: ak_pos(anchor),
                    focus: ak_pos(focus),
                });
            }
            if let Some(c) = n.checked {
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
            // AccessKit puts the count on the container and the
            // zero-based ordinal on the item (see `AccessNode::set_size`).
            if let Some(p) = n.pos_in_set {
                node.set_position_in_set(p);
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
            if let Some(s) = n.scroll {
                node.set_scroll_x(s.x as f64);
                node.set_scroll_x_min(0.0);
                node.set_scroll_x_max(s.max_x as f64);
                node.set_scroll_y(s.y as f64);
                node.set_scroll_y_min(0.0);
                node.set_scroll_y_max(s.max_y as f64);
            }
            nodes.push((NodeId(n.key.0), node));
        }
        TreeUpdate {
            nodes,
            tree: Some(TreeInfo::new(root)),
            tree_id: TreeId::ROOT,
            focus: NodeId(tree.focus.unwrap_or(root_key).0),
        }
    }
}

#[cfg(not(feature = "accesskit"))]
mod imp {
    use kui_core::{AccessRequest, AccessTree};
    use winit::event::WindowEvent;
    use winit::event_loop::{ActiveEventLoop, EventLoopProxy};
    use winit::window::Window;

    /// Without the `accesskit` feature nothing sends user events; the
    /// type still exists so the event loop has one.
    #[derive(Debug)]
    pub enum UserEvent {}

    pub struct Bridge {}

    impl Bridge {
        pub fn new(
            _event_loop: &ActiveEventLoop,
            _window: &Window,
            _proxy: EventLoopProxy<UserEvent>,
        ) -> Option<Self> {
            None
        }

        pub fn process_event(&mut self, _window: &Window, _event: &WindowEvent) {}

        pub fn active(&self) -> bool {
            false
        }

        pub fn on_event(&mut self, ev: UserEvent) -> Option<AccessRequest> {
            match ev {}
        }

        pub fn publish(&mut self, _tree: &AccessTree, _scale: f32) {}
    }
}

pub use imp::{Bridge, UserEvent};
