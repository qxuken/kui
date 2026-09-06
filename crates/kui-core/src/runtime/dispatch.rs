//! Input dispatch: one event in, the UI events it resolved to out.
//!
//! `handle_input` routes pointer, wheel, key, text and access events
//! against the last finished frame — its hit regions, edit buffers and
//! focus — and stamps each result with this window. Focus motion itself
//! lives in `focus`, the arrow-key patterns in `composites`.

use super::*;

impl Core {
    /// Feeds one input event; returns any UI events it resolved to,
    /// hit-tested against the previous frame's layout.
    pub fn handle_input(&mut self, ev: InputEvent) -> Vec<UiEvent> {
        let mut out = self.route_input(ev);
        // Whatever the event itself made pending — the synthetic key
        // releases a focus move forces — belongs to this batch, not to the
        // next frame's drain.
        out.append(&mut self.pending);
        self.stamp(&mut out);
        out
    }

    /// One whole key going down: both channels, in the order a window
    /// drives them (backlog F6). The press reaches whatever holds key
    /// focus, and then [`KeyPress::edit_event`] asks the core for what
    /// that key *means* — Escape dismisses a modal, Tab walks the ring,
    /// an arrow nudges a focused slider, a printable character reaches
    /// the focused editor.
    ///
    /// This is what a driver with a real keyboard does, so it is what a
    /// headless test should do too. [`Core::handle_input`] with a bare
    /// `KeyDown` is still the way to drive one channel on purpose.
    pub fn press(&mut self, key: KeyPress) -> Vec<UiEvent> {
        // Read before the move, and before the press: the key's meaning is
        // a property of the key, not of what the first channel did with it.
        let edit = key.edit_event();
        let mut out = self.handle_input(InputEvent::KeyDown(key));
        if let Some(ev) = edit {
            out.extend(self.handle_input(ev));
        }
        out
    }

    /// The same key coming up. One channel, because only one has a second
    /// half: the editing keys act on the way down. Paired with
    /// [`Core::press`] so a held key is a press and a release, and a sink
    /// that asked for `key_up` hears both.
    pub fn release(&mut self, key: KeyPress) -> Vec<UiEvent> {
        self.handle_input(InputEvent::KeyUp(key.released()))
    }

    /// Says which window every event on its way out came from.
    ///
    /// The producers cannot: hit-testing, the edit buffer and the audio
    /// queue are all below the level at which a window exists. A `Core` is
    /// one window, though, and the driver already told it which one
    /// (`env.window.id`, beside `maximized` and the rest of the window
    /// facts) — so one assignment at each of the two exits covers every
    /// event every binding will ever see, and ADR 0004's step 3 has only to
    /// hand each core its id.
    pub(crate) fn stamp(&self, out: &mut [UiEvent]) {
        let id = self.env.window.id;
        if id == WindowId::MAIN {
            // What producers already wrote. Skipped rather than written so
            // the single-window case stays free.
            return;
        }
        for ev in out {
            ev.window = id;
        }
    }

    fn route_input(&mut self, ev: InputEvent) -> Vec<UiEvent> {
        let mut out = std::mem::take(&mut self.pending);
        // Chrome commands say which window they are about, and a hit
        // region does not know: the interaction store reads it from here.
        self.interaction.window = self.env.window.id;
        match ev {
            InputEvent::Scroll(delta) => {
                // Wheel up (positive y) reveals earlier content: offset decreases.
                if let Some(key) = self.interaction.scroll_target() {
                    self.scroll.scroll_by(key, Vec2::new(-delta.x, -delta.y));
                }
            }
            InputEvent::Text(s) => {
                if let Some(key) = self.edit.focused() {
                    if self.edit_with_fonts(|edit, fs| edit.apply_text(key, &s, fs)) {
                        self.push_edit_event(key, "changed", &mut out);
                    }
                } else if let Some(i) = self.focused_control() {
                    // Inside a composite, printable characters search the
                    // items by name; Space extends a search already under
                    // way rather than pressing (`docs/adr/0007`, decision
                    // 9). Otherwise Space presses the focused control (a
                    // sink would have taken the press as data; an editor
                    // took the text).
                    if !self.type_ahead(i, &s, &mut out) && s == " " {
                        self.click_node(self.tree.keys[i], &mut out);
                    }
                }
            }
            InputEvent::Preedit(s, cursor) => {
                if let Some(key) = self.edit.focused() {
                    self.edit_with_fonts(|edit, fs| edit.set_preedit(key, &s, cursor, fs));
                }
            }
            InputEvent::Key(ek, mods) => {
                // A modal owns Escape: it asks to go away, and nothing
                // else happens (see `docs/adr/0003-modal-surfaces.md`).
                // The core closes nothing — the app stops declaring it.
                if ek == EditKey::Escape
                    && let Some(key) = self.modal()
                {
                    self.dismiss(key, "escape", &mut out);
                    return out;
                }
                // Tab walks the focus ring (Shift-Tab backwards) unless a
                // multiline editor holds focus — there Tab stays
                // indentation — or a key sink does: a sink is an app that
                // owns its keyboard, Tab included (it hands focus on with
                // `focus_next`). With nothing focused Tab enters the ring.
                let sink = self.focused_sink();
                let traverse = ek == EditKey::Tab
                    && match self.edit.focused() {
                        Some(k) => !self.edit.is_multiline(k),
                        None => !sink,
                    };
                if traverse {
                    self.focus_next(!mods.shift);
                } else if let Some(key) = self.edit.focused() {
                    let (changed, submit) =
                        self.edit_with_fonts(|edit, fs| edit.apply_key(key, ek, mods, fs));
                    if changed {
                        self.push_edit_event(key, "changed", &mut out);
                    }
                    if submit {
                        self.push_edit_event(key, "submit", &mut out);
                    }
                    if ek == EditKey::Escape {
                        self.set_focus(None);
                    }
                } else if let Some(i) = self.focused_control() {
                    // A control that is neither an editor nor a sink:
                    // Enter presses it, the arrows nudge a slider (the
                    // same events assistive technology produces), Escape
                    // lets go.
                    use crate::access::AccessAction;
                    let slider =
                        self.tree.specs[i].access().role == Some(crate::access::Role::Slider);
                    match ek {
                        EditKey::Enter => self.click_node(self.tree.keys[i], &mut out),
                        EditKey::Escape => self.set_focus(None),
                        EditKey::Right | EditKey::Up if slider => {
                            self.nudge(i, AccessAction::Increment, &mut out)
                        }
                        EditKey::Left | EditKey::Down if slider => {
                            self.nudge(i, AccessAction::Decrement, &mut out)
                        }
                        // Inside a composite the arrows, Home and End move
                        // focus among the items instead (see
                        // `docs/adr/0007-composite-keyboard-patterns.md`);
                        // on anything else they do nothing, as before.
                        EditKey::Left
                        | EditKey::Right
                        | EditKey::Up
                        | EditKey::Down
                        | EditKey::Home
                        | EditKey::End => self.composite_step(i, ek, &mut out),
                        _ => {}
                    }
                }
            }
            InputEvent::KeyDown(kp) => {
                if self.route_key(&kp, KeyPhase::Down, &mut out) {
                    // Held from here until its release, focus moving, or
                    // the window losing the keyboard. A repeat of a key
                    // already down is the same key, not a second one.
                    if !self.keys_held.iter().any(|h| h.code == kp.code) {
                        self.keys_held.push(kp.released());
                    }
                }
            }
            InputEvent::KeyUp(kp) => {
                // Only a key whose press was delivered has a release to
                // deliver: one pressed while an editor held focus, or
                // already let go of synthetically, resolves nothing.
                if let Some(i) = self.keys_held.iter().position(|h| h.code == kp.code) {
                    self.keys_held.remove(i);
                    self.route_key(&kp.released(), KeyPhase::Up, &mut out);
                }
            }
            InputEvent::MouseDown { button, clicks } => {
                // Only the primary button moves anything: a secondary
                // press asks for a context menu where it landed and leaves
                // focus, the caret and the scrollbars exactly as they were
                // (a right-click on a selection has to keep it).
                let primary = button == MouseButton::Primary;
                // Scrollbars win over everything under them (they draw on
                // top): a thumb press starts a drag, a track press jumps
                // there first. Neither blurs the focused edit.
                if primary
                    && let Some(p) = self.interaction.cursor()
                    && let Some(bar) = self.interaction.scrollbar_at(p)
                {
                    let (pos, thumb_start) = match bar.axis {
                        ScrollAxis::X => (p.x, bar.thumb.x),
                        ScrollAxis::Y => (p.y, bar.thumb.y),
                    };
                    let grab = if pos >= thumb_start && pos <= thumb_start + bar.bar_len {
                        pos - thumb_start
                    } else {
                        let center = bar.bar_len / 2.0;
                        let off = bar.offset_for(p, center);
                        self.set_scroll_axis(bar.key, bar.axis, off);
                        center
                    };
                    self.interaction.scrollbar_drag = Some((bar.key, bar.axis, grab));
                    return out;
                }
                // Click-to-focus / caret placement / start drag-selection,
                // against the previous frame's layout.
                if let Some(p) = self.interaction.cursor() {
                    let hit = self
                        .interaction
                        .hit_at(p)
                        .map(|h| (h.key, h.edit_origin, h.focusable));
                    // While a modal is up, a press outside it never
                    // touches focus: one that finds no region asks the
                    // modal to go away (a modal is hit-tracked, so its own
                    // background is not "outside"), and one that finds the
                    // only live thing out there — window chrome — is the
                    // platform's business, not the app's.
                    if let Some(key) = self.modal()
                        && !hit.as_ref().is_some_and(|(k, _, _)| self.within_modal(*k))
                    {
                        if hit.is_none() {
                            self.dismiss(key, "outside", &mut out);
                        }
                        self.interaction
                            .handle(InputEvent::MouseDown { button, clicks }, &mut out);
                        return out;
                    }
                    // A press moves focus (to a focusable node, or to the
                    // key sink the press landed inside) or drops it; either
                    // way it is pointer focus, not shown.
                    if primary {
                        match hit {
                            Some((key, Some(origin), true)) => {
                                self.set_focus(Some(key));
                                let local = Vec2::new(p.x - origin.x, p.y - origin.y);
                                self.edit_with_fonts(|edit, fs| edit.click(key, local, clicks, fs));
                                self.edit.dragging = Some((key, origin));
                            }
                            // Everything else: a plain node, and a
                            // disabled editor (no caret to place).
                            Some((key, _, focusable)) => {
                                let target = self.press_focus(key, focusable);
                                self.set_focus(target);
                            }
                            None => self.set_focus(None),
                        }
                        self.focus_visible = false;
                    }
                }
                self.interaction
                    .handle(InputEvent::MouseDown { button, clicks }, &mut out);
            }
            InputEvent::CursorMoved(p) => {
                if let Some((key, axis, grab)) = self.interaction.scrollbar_drag
                    && let Some(bar) = self
                        .interaction
                        .scrollbars
                        .iter()
                        .rev()
                        .find(|b| b.key == key && b.axis == axis)
                        .copied()
                {
                    // Thumb drag: geometry is last frame's, which is fine —
                    // track length only changes with the container.
                    let off = bar.offset_for(p, grab);
                    self.set_scroll_axis(key, axis, off);
                }
                if let Some((key, origin)) = self.edit.dragging {
                    let local = Vec2::new(p.x - origin.x, p.y - origin.y);
                    self.edit_with_fonts(|edit, fs| edit.drag(key, local, fs));
                }
                self.interaction
                    .handle(InputEvent::CursorMoved(p), &mut out);
            }
            InputEvent::MouseUp { button } => {
                if button == MouseButton::Primary {
                    self.edit.dragging = None;
                    self.interaction.scrollbar_drag = None;
                }
                self.interaction
                    .handle(InputEvent::MouseUp { button }, &mut out);
            }
            InputEvent::Access(req) => self.handle_access(req, &mut out),
            other => self.interaction.handle(other, &mut out),
        }
        self.flush_sound_requests();
        // Input moves focus, carets and scroll offsets: an access tree
        // derived earlier this frame no longer describes it.
        self.access_built = 0;
        out
    }

    /// Resolves a request from assistive technology against the last
    /// frame the way the pointer or keyboard equivalent would be (see
    /// [`crate::access::AccessAction`]).
    fn handle_access(&mut self, req: crate::access::AccessRequest, out: &mut Vec<UiEvent>) {
        use crate::access::AccessAction;
        let key = req.key;
        let idx = self.tree.keys.iter().position(|k| *k == key);
        match req.action {
            AccessAction::Click => self.click_node(key, out),
            AccessAction::Focus => {
                // The reader's cursor lands where Tab would — on a node it
                // can see (decoration is not in its tree); show it.
                let exposed = self.access_tree().get(key).is_some();
                if exposed && idx.is_some_and(|i| crate::access::focusable(&self.tree, i)) {
                    self.set_focus(Some(key));
                    self.focus_visible = true;
                }
            }
            AccessAction::Blur => {
                if self.focus == Some(key) {
                    self.set_focus(None);
                }
            }
            AccessAction::SetValue => {
                if self.edit.contains(key) {
                    let value = req.value.unwrap_or_default();
                    if self.edit.text(key).as_deref() != Some(value.as_str()) {
                        self.set_edit_text(key, &value);
                        self.push_edit_event(key, "changed", out);
                    }
                } else if let Some(i) = idx
                    && crate::access::is_custom_editor(&self.tree, i)
                {
                    // The app owns the text: hand the request over as data.
                    let mut entries = vec![
                        ("kind".to_string(), Value::str("access")),
                        ("action".to_string(), Value::str(req.action.name())),
                        (
                            "text".to_string(),
                            Value::str(req.value.unwrap_or_default()),
                        ),
                    ];
                    if let Some(tag) = self.access_tag(i) {
                        entries.push(("tag".to_string(), tag));
                    }
                    out.push(UiEvent {
                        origin: self.tree.origins[i],
                        window: WindowId::MAIN,
                        key,
                        payload: Value::Map(entries),
                    });
                }
            }
            AccessAction::Increment | AccessAction::Decrement => {
                let Some(i) = idx else { return };
                self.nudge(i, req.action, out);
            }
            AccessAction::SetTextSelection | AccessAction::ReplaceSelectedText => {
                let Some(i) = idx else { return };
                if self.edit.contains(key) {
                    match req.action {
                        AccessAction::SetTextSelection => {
                            let (Some(anchor), Some(focus)) = (req.anchor, req.focus) else {
                                return;
                            };
                            // Run positions resolve against the tree of
                            // the last frame, which is what the request
                            // was made from.
                            let tree = self.access_tree();
                            let Some(node) = tree.get(key) else { return };
                            let (Some(a), Some(f)) =
                                (node.line_offset(anchor), node.line_offset(focus))
                            else {
                                return;
                            };
                            self.edit.set_selection(key, a, f);
                        }
                        _ => {
                            let text = req.value.unwrap_or_default();
                            if self
                                .edit_with_fonts(|edit, fs| edit.replace_selection(key, &text, fs))
                            {
                                self.push_edit_event(key, "changed", out);
                            }
                        }
                    }
                } else if crate::access::is_custom_editor(&self.tree, i) {
                    // The app owns the text: hand the request over as data.
                    let mut entries = vec![
                        ("kind".to_string(), Value::str("access")),
                        ("action".to_string(), Value::str(req.action.name())),
                    ];
                    if let Some(text) = req.value {
                        entries.push(("text".to_string(), Value::str(text)));
                    }
                    if let (Some(anchor), Some(focus)) = (req.anchor, req.focus) {
                        let tree = self.access_tree();
                        let Some(node) = tree.get(key) else { return };
                        let (Some(a), Some(f)) =
                            (node.line_offset(anchor), node.line_offset(focus))
                        else {
                            return;
                        };
                        let pos = |(line, offset): (usize, usize)| {
                            Value::map([
                                ("line", Value::Int(line as i64)),
                                ("offset", Value::Int(offset as i64)),
                            ])
                        };
                        entries.push(("anchor".to_string(), pos(a)));
                        entries.push(("focus".to_string(), pos(f)));
                    }
                    let ev = self.tree.specs[i].events();
                    let tag = ev
                        .on_click
                        .clone()
                        .or_else(|| ev.on_drag.clone())
                        .or_else(|| ev.on_key.clone());
                    if let Some(tag) = tag.filter(|t| *t != Value::Null) {
                        entries.push(("tag".to_string(), tag));
                    }
                    out.push(UiEvent {
                        origin: self.tree.origins[i],
                        window: WindowId::MAIN,
                        key,
                        payload: Value::Map(entries),
                    });
                }
            }
            AccessAction::ScrollIntoView => {
                let Some(i) = idx else { return };
                let rect = Rect::from_pos_size(self.tree.pos[i], self.tree.size[i]);
                self.scroll_rect_into_view(i, rect, false);
            }
            AccessAction::ScrollUp
            | AccessAction::ScrollDown
            | AccessAction::ScrollLeft
            | AccessAction::ScrollRight => {
                let Some(i) = idx else { return };
                let size = self.tree.size[i];
                let delta = match req.action {
                    AccessAction::ScrollUp => Vec2::new(0.0, -size.h * 0.8),
                    AccessAction::ScrollDown => Vec2::new(0.0, size.h * 0.8),
                    AccessAction::ScrollLeft => Vec2::new(-size.w * 0.8, 0.0),
                    _ => Vec2::new(size.w * 0.8, 0.0),
                };
                self.scroll.scroll_by(key, delta);
            }
        }
    }

    /// The `tag` an `access` event on node `i` carries: its click payload,
    /// else its drag or key tag; None when there is none (or it is null).
    pub(crate) fn access_tag(&self, i: usize) -> Option<Value> {
        let ev = self.tree.specs[i].events();
        ev.on_click
            .clone()
            .or_else(|| ev.on_drag.clone())
            .or_else(|| ev.on_key.clone())
            .filter(|t| *t != Value::Null)
    }

    /// The access tree of the last finished frame (see [`crate::access`]):
    /// derived on the first call after a frame, then reused. A driver that
    /// never asks pays nothing.
    pub fn access_tree(&mut self) -> &crate::access::AccessTree {
        if self.access_built != self.frame_no {
            self.access = crate::access::build(
                &self.tree,
                &crate::access::Sources {
                    text: &self.text,
                    edit: &self.edit,
                    scroll: &self.scroll,
                    title: self.window_title.as_deref(),
                    focus: self.focus,
                    modal: self.modal(),
                    viewport: self.viewport,
                    scale: self.scale,
                },
            );
            self.access_built = self.frame_no;
        }
        &self.access
    }

    /// Delivers one key event to the focused sink, tagged with the sink's
    /// `on_key` payload; returns whether anything took it. The focused edit
    /// widget owns the keyboard (it takes the Text/EditKey path), so a sink
    /// only hears while no editor is focused and it is still in the last
    /// frame's hit list.
    fn route_key(&mut self, kp: &KeyPress, phase: KeyPhase, out: &mut Vec<UiEvent>) -> bool {
        if self.edit.focused().is_some() {
            return false;
        }
        let Some(focus) = self.focus else {
            return false;
        };
        let Some(h) = self
            .interaction
            .hits
            .iter()
            .rev()
            .find(|h| h.key == focus && h.key_sink.is_some())
        else {
            return false;
        };
        // A sink hears releases only by asking (`key_up`): press-only is
        // the keymap case, and a keymap handed both halves runs every
        // binding twice. The key is still tracked as held either way, so
        // a sink that opts in mid-hold hears the release it is owed.
        if phase == KeyPhase::Up && !h.key_up {
            return false;
        }
        let mut payload = kp.to_value(phase);
        if let Some(tag) = &h.key_sink
            && *tag != Value::Null
            && let Value::Map(entries) = &mut payload
        {
            entries.push(("tag".to_string(), tag.clone()));
        }
        out.push(UiEvent {
            origin: h.origin,
            window: WindowId::MAIN,
            key: h.key,
            payload,
        });
        true
    }

    /// Lets go of every key the focused sink is holding, as if the user
    /// had released them: each becomes a `{kind="key", phase="up"}` on the
    /// sink that took the press. Called when focus moves — a keymap that
    /// armed a mode on the way down has to hear the way up, and the node
    /// it moved to never saw the press — and by drivers when the window
    /// loses the keyboard (Cmd-Tab while a key is down otherwise leaves it
    /// stuck down forever).
    pub fn release_held_keys(&mut self) {
        if self.keys_held.is_empty() {
            return;
        }
        let mut out = Vec::new();
        for kp in std::mem::take(&mut self.keys_held) {
            self.route_key(&kp, KeyPhase::Up, &mut out);
        }
        // Pending rather than returned: the writers are `set_focus` and the
        // driver's window-focus report, neither of which is answering an
        // input event. `handle_input` appends it before returning, so a
        // click that moved focus and the release it forced arrive together.
        self.pending.append(&mut out);
    }

    /// Whether the focused node is a key sink (it owns its keys).
    fn focused_sink(&self) -> bool {
        self.focus_index()
            .is_some_and(|i| self.tree.specs[i].events().on_key.is_some())
    }

    /// The focused node when it is a control the core presses itself:
    /// not an editor, not a key sink, and still focusable.
    fn focused_control(&self) -> Option<usize> {
        let i = self.focus_index()?;
        let spec = &self.tree.specs[i];
        let editor = matches!(self.tree.content[i], NodeContent::Edit(_));
        (!editor && spec.events().on_key.is_none() && crate::access::focusable(&self.tree, i))
            .then_some(i)
    }

    /// Activates node `key` the way a pointer click would — against the
    /// last frame's hit regions, so a disabled node emits nothing — for
    /// Enter, Space and an assistive-technology `click`. Focus follows
    /// into an editor or a sink, as a click's would.
    pub(crate) fn click_node(&mut self, key: Key, out: &mut Vec<UiEvent>) {
        let Some(h) = self.interaction.hits.iter().rev().find(|h| h.key == key) else {
            return;
        };
        let (origin, payload, window, sound) =
            (h.origin, h.payload.clone(), h.window, h.click_sound);
        let takes_focus = h.focusable && (h.edit_origin.is_some() || h.key_sink.is_some());
        if let Some(sound) = sound {
            self.interaction.sound_requests.push(sound);
        }
        match (window, payload) {
            (Some(crate::window::WindowRole::Button(b)), _) => self
                .interaction
                .window_commands
                .push(b.command(self.env.window.id)),
            (None, Some(payload)) => out.push(UiEvent {
                origin,
                window: WindowId::MAIN,
                key,
                payload,
            }),
            _ => {}
        }
        if takes_focus {
            self.set_focus(Some(key));
        }
    }

    /// The modal `key` was asked to go away — Escape, or a press outside
    /// it. Reaches the app as `{kind="dismiss", reason, tag}` on the modal
    /// node; what happens next is the app's, since only it can stop
    /// declaring the node (see `docs/adr/0003-modal-surfaces.md`).
    fn dismiss(&mut self, key: Key, reason: &str, out: &mut Vec<UiEvent>) {
        let Some(i) = self.tree.keys.iter().position(|k| *k == key) else {
            return;
        };
        let mut entries = vec![
            ("kind".to_string(), Value::str("dismiss")),
            ("reason".to_string(), Value::str(reason)),
        ];
        if let Some(tag) = self.tree.specs[i]
            .events()
            .modal
            .clone()
            .filter(|t| *t != Value::Null)
        {
            entries.push(("tag".to_string(), tag));
        }
        out.push(UiEvent {
            origin: self.tree.origins[i],
            window: WindowId::MAIN,
            key,
            payload: Value::Map(entries),
        });
    }

    fn push_edit_event(&self, key: Key, kind: &str, out: &mut Vec<UiEvent>) {
        out.push(UiEvent {
            origin: self.edit.origin_of(key).unwrap_or(OriginId::HOST),
            window: WindowId::MAIN,
            key,
            payload: Value::map([("kind", kind.into())]),
        });
    }

    /// Selected text of the focused editor (for clipboard integration).
    pub fn copy_selection(&self) -> Option<String> {
        self.edit.copy_selection(self.edit.focused()?)
    }

    /// Cuts the focused editor's selection, returning the removed text.
    pub fn cut_selection(&mut self) -> Option<String> {
        let key = self.edit.focused()?;
        let text = self.edit.copy_selection(key)?;
        self.edit_with_fonts(|edit, fs| edit.delete_selection(key, fs));
        Some(text)
    }

    /// Current text of an editor by key.
    pub fn edit_text(&self, key: Key) -> Option<String> {
        self.edit.text(key)
    }

    pub fn set_edit_text(&mut self, key: Key, text: &str) {
        let sess = &mut *self.session.state();
        self.edit
            .set_text(key, text, &mut sess.fonts, &sess.resources);
    }

    /// The pointer shape for wherever the pointer is now, derived from the
    /// frame's hit regions (see [`crate::cursor`]). Per-frame output like
    /// the window commands, but a query rather than a drain: it is a state,
    /// not a queue, so a driver reads it after each input and each frame and
    /// only touches the window when the answer changes. Headless drivers
    /// never read it, and the core stays device-free.
    pub fn cursor_shape(&self) -> crate::cursor::CursorShape {
        self.interaction.cursor_shape()
    }
}
