//! The AccessKit bridge: the core's access tree (data) becomes the
//! platform accessibility tree (UIA, NSAccessibility, AT-SPI) through
//! `accesskit_winit`, and the platform's requests come back as
//! `InputEvent::Access`. See `docs/adr/0001-accessibility-as-data.md`.
//!
//! Nothing here runs until assistive technology asks for the tree: the
//! adapter reports activation through the event loop, and only then does
//! the shell derive and push trees — on frames whose tree hash changed.

#[cfg(all(feature = "accesskit", not(target_arch = "wasm32")))]
mod imp {
    use accesskit::{
        Action, ActionData, Affine, Live as AkLive, Node, NodeId, Orientation as AkOrientation,
        Rect, Role as AkRole, TextDirection, TextPosition, TextSelection, Toggled, TreeId,
        TreeInfo, TreeUpdate,
    };
    use accesskit_winit::{Adapter, Event, WindowEvent as AkWindowEvent};
    use kui_core::{
        AccessAction, AccessRequest, AccessTree, Announcement, Assistive, Key, Live, Orientation,
        Role, TextPos,
    };
    use winit::event::WindowEvent;
    use winit::event_loop::{ActiveEventLoop, EventLoopProxy};
    use winit::window::Window;

    /// The runner's event-loop user event: what AccessKit sends back, and
    /// the one an app's own thread sends through a [`crate::Waker`].
    #[derive(Debug)]
    pub enum UserEvent {
        Access(Event),
        /// Something the app owns changed off the loop's thread: draw.
        Wake,
    }

    impl From<Event> for UserEvent {
        fn from(e: Event) -> Self {
            UserEvent::Access(e)
        }
    }

    /// Which window the platform is talking about: every adapter is one
    /// window's, and the shell routes to that window's bridge. A wake is
    /// nobody's window.
    pub fn window_of(ev: &UserEvent) -> Option<winit::window::WindowId> {
        match ev {
            UserEvent::Access(ev) => Some(ev.window_id),
            UserEvent::Wake => None,
        }
    }

    pub struct Bridge {
        adapter: Adapter,
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
        /// is derived from
        /// (`docs/adr/0008-live-regions-and-announcements.md`).
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
                announced: Vec::new(),
                announce_gen: 0,
                next_id: u64::MAX,
            })
        }

        pub fn process_event(&mut self, window: &Window, event: &WindowEvent) {
            self.adapter.process_event(window, event);
        }

        pub fn active(&self) -> bool {
            self.active
        }

        /// The reading `env.system.assistive` carries (backlog F48): a
        /// client has asked for the tree, or none has. Whether it ever
        /// falls back is the adapter's: only `accesskit_unix` sends
        /// `AccessibilityDeactivated`; the macOS and Windows adapters in
        /// `accesskit_winit` take the deactivation handler and never call
        /// it, so there the reading rises once and stays.
        pub fn assistive(&self) -> Assistive {
            if self.active {
                Assistive::Listening
            } else {
                Assistive::None
            }
        }

        /// Folds an AccessKit event in; an action request comes back as
        /// the core's input to dispatch.
        pub fn on_event(&mut self, ev: UserEvent) -> Option<AccessRequest> {
            let UserEvent::Access(ev) = ev else {
                return None;
            };
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
        pub fn publish(&mut self, tree: &AccessTree, scale: f32, said: &[Announcement]) {
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
            let said = &self.announced;
            self.adapter
                .update_if_active(|| tree_update(tree, scale, said));
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
    fn tree_update(tree: &AccessTree, scale: f32, said: &[(NodeId, String, Live)]) -> TreeUpdate {
        let root_key = tree.root().map_or(Key::ROOT, |r| r.key);
        let root = NodeId(root_key.0);
        let mut nodes = Vec::with_capacity(tree.nodes.len().max(1));
        if tree.nodes.is_empty() {
            // No frame yet: a bare window, so the platform has a root.
            nodes.push((root, Node::new(AkRole::Window)));
        }
        for n in &tree.nodes {
            // A live region with no role of its own is ARIA's `status`:
            // an AXGroup with the AXApplicationStatus subrole on macOS,
            // rather than a bare group.
            let role = if n.live != Live::Off && n.role == Role::Group {
                AkRole::Status
            } else {
                role_of(n.role)
            };
            let mut node = Node::new(role);
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
}

#[cfg(any(not(feature = "accesskit"), target_arch = "wasm32"))]
mod imp {
    use kui_core::{AccessRequest, AccessTree, Announcement, Assistive};
    use winit::event::WindowEvent;
    use winit::event_loop::{ActiveEventLoop, EventLoopProxy};
    use winit::window::Window;

    /// Without the `accesskit` feature the only user event is the wake an
    /// app's own thread sends through a [`crate::Waker`].
    #[derive(Debug)]
    pub enum UserEvent {
        /// Something the app owns changed off the loop's thread: draw.
        Wake,
    }

    pub fn window_of(ev: &UserEvent) -> Option<winit::window::WindowId> {
        match ev {
            UserEvent::Wake => None,
        }
    }

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

        /// Never constructed (`new` answers `None`), so a pane built
        /// without the feature reads `Unknown` through the `map_or` in
        /// `sync_env` rather than through here.
        pub fn assistive(&self) -> Assistive {
            Assistive::Unknown
        }

        pub fn on_event(&mut self, ev: UserEvent) -> Option<AccessRequest> {
            match ev {
                UserEvent::Wake => None,
            }
        }

        pub fn publish(&mut self, _tree: &AccessTree, _scale: f32, _said: &[Announcement]) {}
    }
}

pub use imp::{Bridge, UserEvent, window_of};
