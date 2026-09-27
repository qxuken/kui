//! Input dispatch: one event in, the UI events it resolved to out.
//!
//! `handle_input` routes pointer, wheel, key, text and access events
//! against the last finished frame — its hit regions, edit buffers and
//! focus — and stamps each result with this window. Focus motion itself
//! lives in `focus`, the arrow-key patterns in `composites`.

use super::*;
use crate::input::Target;

impl Core {
    /// Feeds one input event; returns any UI events it resolved to,
    /// hit-tested against the previous frame's layout.
    pub fn handle_input(&mut self, ev: InputEvent) -> Vec<UiEvent> {
        // What a focus move this input makes is reported as (DX18).
        let by = match &ev {
            InputEvent::CursorMoved(_)
            | InputEvent::CursorLeft
            | InputEvent::MouseDown { .. }
            | InputEvent::MouseUp { .. }
            | InputEvent::Scroll(_) => "pointer",
            InputEvent::Access(_) => "assistive",
            InputEvent::Key(..)
            | InputEvent::KeyDown(_)
            | InputEvent::KeyUp(_)
            | InputEvent::Text(_)
            | InputEvent::Commit(_)
            | InputEvent::Preedit(..) => "keyboard",
            _ => "program",
        };
        // The devtools' chords are acted on before anything is routed
        // (ADR 0024, decision 4): the press goes no further, and what
        // was pending still goes out.
        let mut out = if self.devtools_intercept(&ev) {
            std::mem::take(&mut self.pending)
        } else {
            self.route_input(ev)
        };
        // Whatever the event itself made pending — the synthetic key
        // releases a focus move forces — belongs to this batch, not to the
        // next frame's drain.
        out.append(&mut self.pending);
        self.report_focus(by, &mut out);
        // A click on a select field opens its menu and is nobody's
        // (`widgets::select`); before the menu's own consumer, since the
        // menu it opens is that one's from here on.
        self.consume_select_events(&mut out);
        // A row of the core's own context menu is not the app's click, and
        // neither is that menu's dismissal: taken back here, acted on, and
        // reported as one `menu` event on the node the menu was about (ADR
        // 0017, decision 5). Here rather than inside `route_input` because
        // several of its arms return early — the modal press among them,
        // which is exactly the one that dismisses a menu.
        self.consume_menu_events(&mut out);
        // And the drawn menu bar's own nodes, on the same terms: its
        // titles and rows post ordinary clicks, and none of them is the
        // app's (`docs/adr/0018-a-menu-bar-the-app-declares.md`).
        self.consume_menu_bar_events(&mut out);
        self.outbound(&mut out);
        out
    }

    /// The way out for every batch of events the app is about to hear,
    /// whichever door made them — an input, or a host's own menu
    /// answering (`activate_menu_item`), which is not an input and used
    /// to skip this: a devtools select chosen from a native menu posted
    /// its action to the app instead of the panel. The devtools panel's
    /// own controls are taken back (nobody's but the core's); what is
    /// left is translated, stamped with its window and logged on its way
    /// out (ADR 0024, decision 4).
    pub(crate) fn outbound(&mut self, out: &mut Vec<UiEvent>) {
        self.devtools_consume(out);
        self.devtools_translate(out);
        self.stamp(out);
        self.devtools_log(out);
        // An event the app was handed is "the user did something": what
        // separates a message repeated on purpose from a view announcing
        // every frame (`announce`).
        if !out.is_empty() {
            self.events_answered += 1;
        }
    }

    /// A click or drag says where it landed, in the terms of the node it
    /// landed on — the one pass both shapes go through (AR11), run on
    /// the `n` events at the end of `out` that `Interaction::handle` just
    /// made from a press, and on nothing else: a click Enter, Space or a
    /// screen reader made has no point and no count, and the cursor is
    /// wherever the mouse happens to rest. Only a map payload can carry
    /// the fields.
    ///
    /// On a `cells` grid, `cell: {row, col}` — the same arithmetic a
    /// selection uses (`cell_row_col`), so the app never divides by a
    /// cell size it did not choose (backlog C20). Inside a key sink that
    /// draws `role="line"` rows, `line` — the ordinal among the sink's
    /// lines, the numbering its `access` events use — `byte` — where the
    /// point falls in that line's text, what `text_hit` would answer —
    /// and `clicks` — the press's count, so a double click is a word
    /// without a timer the app keeps (backlog C34); a point above the
    /// first line is the first, below the last the last, and one in a
    /// gutter is the line beside it. The point is the event's own for a
    /// drag and the cursor's for a click. Not opt-in, like `cell`: the
    /// fields appear wherever the shape they describe is drawn, and a
    /// handler that does not read them is not slower for their being
    /// there.
    fn attach_pointer(&mut self, out: &mut [UiEvent], n: usize) {
        if n == 0 || self.tree.is_empty() || !(self.tree.any_text || self.tree.any_line) {
            return;
        }
        let clicks = self.interaction.press_clicks();
        let from = out.len() - n;
        for ev in &mut out[from..] {
            let Some(point) = self.pointer_point(ev) else {
                continue;
            };
            let Some(i) = self.tree.index_of(ev.key) else {
                continue;
            };
            if let Some((row, col)) = self.cell_row_col(ev.key, point)
                && let Value::Map(entries) = &mut ev.payload
            {
                entries.push((
                    "cell".to_string(),
                    Value::map([
                        ("row", Value::Int(row as i64)),
                        ("col", Value::Int(col as i64)),
                    ]),
                ));
            }
            if !self.tree.any_line {
                continue;
            }
            // The sink: this node, or the nearest above it.
            let mut sink = i;
            while self.tree.specs[sink].events().on_key.is_none() {
                let p = self.tree.parent[sink];
                if p == crate::tree::NIL {
                    break;
                }
                sink = p as usize;
            }
            if self.tree.specs[sink].events().on_key.is_none() {
                continue;
            }
            let lines = crate::access::lines_under(&self.tree, sink);
            // Nearest vertically — inside one is a gap of zero — ties to
            // the earlier line.
            let gap = |l: usize| {
                let (top, h) = (self.tree.pos[l].y, self.tree.size[l].h);
                (top - point.y).max(point.y - (top + h)).max(0.0)
            };
            let Some((line, l)) = lines
                .iter()
                .enumerate()
                .map(|(n, &l)| (n, l))
                .min_by(|a, b| {
                    gap(a.1)
                        .partial_cmp(&gap(b.1))
                        .unwrap_or(std::cmp::Ordering::Equal)
                })
            else {
                continue;
            };
            let byte = self
                .text
                .hit_at(self.tree.keys[l], point, self.building)
                .map_or(0, |h| h.byte);
            if let Value::Map(entries) = &mut ev.payload {
                entries.push(("line".to_string(), Value::Int(line as i64)));
                entries.push(("byte".to_string(), Value::Int(byte as i64)));
                entries.push(("clicks".to_string(), Value::Int(clicks as i64)));
            }
        }
    }

    /// Where a pointer-made event happened: the `x` / `y` its payload
    /// carries (a drag's), else the cursor (a click's), else nowhere —
    /// and nowhere for a payload that is not a map, which can carry no
    /// field anyway.
    fn pointer_point(&self, ev: &UiEvent) -> Option<Vec2> {
        let Value::Map(entries) = &ev.payload else {
            return None;
        };
        let field = |name: &str| {
            entries
                .iter()
                .find(|(k, _)| k == name)
                .and_then(|(_, v)| match v {
                    Value::Float(f) => Some(*f as f32),
                    Value::Int(n) => Some(*n as f32),
                    _ => None,
                })
        };
        match (field("x"), field("y")) {
            (Some(x), Some(y)) => Some(Vec2::new(x, y)),
            _ => self.interaction.cursor(),
        }
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
    /// Most producers cannot: hit-testing and the edit buffer are below
    /// the level at which a window exists. A `Core` is one window, though,
    /// and the driver already told it which one (`env.window.id`, beside
    /// `maximized` and the rest of the window facts) — so one assignment
    /// at each of the two exits covers every event every binding will
    /// ever see, and ADR 0004's step 3 has only to hand each core its id.
    ///
    /// The one producer that does know is the audio store, whose mounts
    /// are per window (AR7) and whose `ended` / `refused` events are folded
    /// back through whichever core the driver holds — the main one, in the
    /// runner. An event it stamped with another window keeps that stamp; a
    /// `MAIN` one is indistinguishable from an unstamped one and takes the
    /// draining core's, which is right wherever the draining core is the
    /// main window, as every driver's is.
    pub(crate) fn stamp(&self, out: &mut [UiEvent]) {
        let id = self.env.window.id;
        // The slot a node was filled into, from the last frame's fill
        // ranges (`Tree::fills`): a frame with no extension recorded none,
        // and pays one emptiness check per batch.
        if !self.tree.fills.is_empty() {
            for ev in out.iter_mut() {
                if ev.slot.is_none()
                    && let Some(i) = self.tree.index_of(ev.key)
                {
                    ev.slot = self.tree.slot_of(i);
                }
            }
        }
        if id == WindowId::MAIN {
            // What producers already wrote. Skipped rather than written so
            // the single-window case stays free.
            return;
        }
        for ev in out {
            if ev.window == WindowId::MAIN {
                ev.window = id;
            }
        }
    }

    fn route_input(&mut self, ev: InputEvent) -> Vec<UiEvent> {
        let mut out = std::mem::take(&mut self.pending);
        // Chrome commands say which window they are about, and a hit
        // region does not know: the interaction store reads it from here.
        self.interaction.window = self.env.window.id;
        // The press this event is the second channel of, if it is one:
        // a `KeyDown` leaves its modifiers for the `Key` or `Text` that
        // follows it, and anything else is not that (AR10).
        let pressed = self.pressed_mods.take();
        // Whether a modifier other than Shift was down on that press — the
        // question `route_key` asked, asked again here so the two channels
        // agree about a chord (`docs/adr/0011`, decision 3). Without a
        // press, the editing `Mods` are what there is.
        let chord = |mods: Option<crate::input::Mods>| match pressed {
            Some(m) => m.ctrl || m.alt || m.super_key,
            None => mods.is_some_and(|m| m.word || m.doc),
        };
        // A paste's answer is a commit with the pasteboard's markers beside
        // it (backlog F84), and a bare commit is one whose pasteboard marked
        // nothing — the answer an older driver sends — so the two are one
        // arm below.
        let (ev, marks, paste) = match ev {
            InputEvent::Paste { text, marks } => (InputEvent::Commit(text), marks, true),
            ev => (ev, crate::input::ClipboardMarks::default(), false),
        };
        match ev {
            InputEvent::Scroll(delta) => {
                // Wheel up (positive y) reveals earlier content: offset decreases.
                // An `on_scroll` node under the pointer hears the notch
                // instead — the whole lines it covers on a grid, the
                // fraction carried to the next notch on the same node
                // (ADR 0029, decision 4).
                //
                // A scroller takes the axes it scrolls on and passes the
                // rest to the region under it (backlog DX13): a vertical
                // list inside a horizontal strip moves the strip on a
                // sideways swipe, where the list used to swallow the x it
                // could do nothing with. A handler takes the whole notch,
                // as it always did — the app said it wanted the wheel.
                let regions: Vec<_> = self.interaction.scroll_regions_at().copied().collect();
                let mut rest = delta;
                for r in regions {
                    if rest == Vec2::ZERO {
                        break;
                    }
                    if r.handler {
                        let p = self.interaction.cursor().unwrap_or(Vec2::ZERO);
                        if let Some(ev) = self.scroll_event(r.key, p, rest) {
                            out.push(ev);
                        }
                        break;
                    }
                    let layout = &self.tree.specs[r.node as usize].layout;
                    let take = Vec2::new(
                        if layout.scroll_x { rest.x } else { 0.0 },
                        if layout.scroll_y { rest.y } else { 0.0 },
                    );
                    if take != Vec2::ZERO {
                        self.scroll.scroll_by(r.key, Vec2::new(-take.x, -take.y));
                        rest = Vec2::new(rest.x - take.x, rest.y - take.y);
                    }
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
                    //
                    // Unless the control claims neither and a sink above it
                    // does: the raw press already bubbled there, and the
                    // two channels have to agree about who owns the key
                    // (`docs/adr/0011`, decision 3).
                    let code = match s.chars().next() {
                        Some(c) => KeyCode::Char(c),
                        None => KeyCode::Unknown,
                    };
                    if !self.bubbles(i, code, chord(None))
                        && !self.type_ahead(i, &s, &mut out)
                        && s == " "
                    {
                        // The press shows the focus, as Enter's does
                        // (`docs/adr/0002`, decision 4a).
                        self.focus_visible = true;
                        self.click_node(self.tree.keys[i], &mut out);
                    }
                }
            }
            InputEvent::Files(paths) => {
                if let Some(ev) = self.file_ask.answer(&paths) {
                    out.push(ev);
                }
            }
            InputEvent::Commit(s) => {
                // The paste's answer, when one was asked — a driver answers
                // every ask, with an empty commit for an empty clipboard,
                // which is what lets the next ask through (AR34). The same
                // rule says whether this *is* the answer: a `Paste`, or any
                // commit while an ask is out (backlog DX14).
                let pasted = std::mem::replace(&mut self.awaiting_paste, false) || paste;
                if let Some(key) = self.edit.focused() {
                    if self.edit_with_fonts(|edit, fs| edit.apply_text(key, &s, fs)) {
                        self.push_edit_event(key, "changed", &mut out);
                    }
                } else {
                    // A custom editor: the composition's result as data,
                    // on the sink the focused node reports to (backlog
                    // C17). Never reaches the `Text` arm above, so a
                    // sink hears a commit once and a keystroke once. A
                    // marker rides along only when it is set, so a sink
                    // that never heard of them sees the payload it always
                    // did.
                    let mut fields = vec![("kind", Value::str("text")), ("text", Value::Str(s))];
                    if pasted {
                        fields.push(("pasted", Value::Bool(true)));
                    }
                    if marks.concealed {
                        fields.push(("concealed", Value::Bool(true)));
                    }
                    if marks.transient {
                        fields.push(("transient", Value::Bool(true)));
                    }
                    self.sink_event(Value::map(fields), &mut out);
                }
            }
            InputEvent::Preedit(s, cursor) => {
                if let Some(key) = self.edit.focused() {
                    self.edit_with_fonts(|edit, fs| edit.set_preedit(key, &s, cursor, fs));
                } else {
                    let cursor = match cursor {
                        Some((a, b)) => {
                            Value::List(vec![Value::Int(a as i64), Value::Int(b as i64)])
                        }
                        None => Value::Null,
                    };
                    self.sink_event(
                        Value::map([
                            ("kind", Value::str("preedit")),
                            ("text", Value::Str(s)),
                            ("cursor", cursor),
                        ]),
                        &mut out,
                    );
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
                    self.editor_took_selection(key);
                    if changed {
                        self.push_edit_event(key, "changed", &mut out);
                    }
                    if submit {
                        self.push_edit_event(key, "submit", &mut out);
                    }
                    if ek == EditKey::Escape {
                        self.move_focus(None);
                    }
                } else if let Some(i) = self.focused_control()
                    // A key this control does not claim has already gone to
                    // the sink above it as a raw press, so it must not act
                    // here as well (`docs/adr/0011`, decision 3). With no
                    // sink above, nothing bubbled and every arm below runs
                    // as it always did — including Escape, which is how a
                    // control with no shortcut layer over it is let go of.
                    && !edit_key_code(ek).is_some_and(|c| self.bubbles(i, c, chord(Some(mods))))
                {
                    // A control that is neither an editor nor a sink:
                    // Enter presses it, the arrows nudge a slider (the
                    // same events assistive technology produces), Escape
                    // lets go.
                    use crate::slider::SliderMove;
                    let slider =
                        self.tree.specs[i].access().role == Some(crate::access::Role::Slider);
                    let changes = slider && self.tree.specs[i].events().on_change.is_some();
                    // Each key the core acts with shows the focus first
                    // (`docs/adr/0002`, decision 4a): pointer focus is
                    // unshown, but the moment the keyboard uses it the
                    // user is owed the answer to "which node did that?" —
                    // a button pressed with Space after a click otherwise
                    // emits its event with nothing on screen naming it.
                    // Escape acts by letting go, and a ring around nothing
                    // is not a ring; a key the control does not claim went
                    // to the sink above and never arrives here at all.
                    // Shift with a horizontal motion on a node inside a
                    // `selectable` scope moves the scope's selection
                    // (backlog AR28) — the keyboard's half of what a
                    // drag does, and the one way a keyboard user selects
                    // a label. Under a sink the press already bubbled and
                    // never arrives here, like every other motion.
                    let scope = self.scopes.get(i).copied().flatten();
                    match ek {
                        EditKey::Left | EditKey::Right | EditKey::Home | EditKey::End
                            if mods.shift && scope.is_some() =>
                        {
                            self.focus_visible = true;
                            self.keyboard_select(scope.unwrap_or(Key::ROOT), ek, mods);
                        }
                        EditKey::Enter => {
                            self.focus_visible = true;
                            self.click_node(self.tree.keys[i], &mut out);
                        }
                        EditKey::Escape => self.move_focus(None),
                        EditKey::Right | EditKey::Up if slider => {
                            self.focus_visible = true;
                            self.nudge(i, SliderMove::Step(1), &mut out);
                        }
                        EditKey::Left | EditKey::Down if slider => {
                            self.focus_visible = true;
                            self.nudge(i, SliderMove::Step(-1), &mut out);
                        }
                        // A slider that asked for its changes takes the
                        // rest of the keys a range has (ADR 0034,
                        // decision 4); one that did not leaves them be.
                        EditKey::PageUp | EditKey::PageDown | EditKey::Home | EditKey::End
                            if changes =>
                        {
                            self.focus_visible = true;
                            let mv = match ek {
                                EditKey::PageUp => SliderMove::Page(1),
                                EditKey::PageDown => SliderMove::Page(-1),
                                EditKey::Home => SliderMove::Home,
                                _ => SliderMove::End,
                            };
                            self.nudge(i, mv, &mut out);
                        }
                        // Inside a composite the arrows, Home and End move
                        // focus among the items instead (see
                        // `docs/adr/0007-composite-keyboard-patterns.md`),
                        // showing the focus where they land; on anything
                        // else they do nothing, as before.
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
                self.pressed_mods = Some(kp.mods);
                if self.route_key(&kp, KeyPhase::Down, &mut out) {
                    // Held from here until its release, focus moving, or
                    // the window losing the keyboard. A repeat of a key
                    // already down is the same key, not a second one —
                    // matched by position, since Shift moving mid-hold
                    // changes the repeat's `code` (`KeyPress::same_key`).
                    if !self.keys_held.iter().any(|h| h.same_key(&kp)) {
                        self.keys_held.push(kp.released());
                    }
                }
            }
            InputEvent::KeyUp(kp) => {
                // Only a key whose press was delivered has a release to
                // deliver: one pressed while an editor held focus, or
                // already let go of synthetically, resolves nothing.
                if let Some(i) = self.keys_held.iter().position(|h| h.same_key(&kp)) {
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
                // A scrollbar wins what it was painted over — its own
                // scroller's content, not a float over it (ADR 0023): a
                // thumb press starts a drag, a track press jumps there
                // first. Neither blurs the focused edit.
                if primary
                    && let Some(p) = self.interaction.cursor()
                    && let Some(Target::Bar(bar)) = self.interaction.target_at(p)
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
                    let hit = self.interaction.hit_at(p).map(|h| {
                        // A region that does something with a press
                        // claims it: a button inside a selectable
                        // card is a button first (ADR 0017).
                        let claimed = h.payload.is_some()
                            || h.drag.is_some()
                            || h.key_sink.is_some()
                            || h.window.is_some();
                        let scope = h.select_scope.filter(|_| !claimed);
                        (h.key, h.edit_origin, h.focusable, scope, h.origin)
                    });
                    // While a modal is up, a press outside it never
                    // touches focus: one that finds no region asks the
                    // modal to go away (a modal is hit-tracked, so its own
                    // background is not "outside"), and one that finds the
                    // only live thing out there — window chrome — is the
                    // platform's business, not the app's.
                    if let Some(key) = self.modal()
                        && !hit.as_ref().is_some_and(|(k, ..)| self.within_modal(*k))
                    {
                        if hit.is_none() {
                            self.dismiss(key, "outside", &mut out);
                        }
                        let n = self
                            .interaction
                            .handle(InputEvent::MouseDown { button, clicks }, &mut out);
                        self.attach_pointer(&mut out, n);
                        return out;
                    }
                    // A press moves focus (to a focusable node, or to the
                    // key sink the press landed inside) or drops it; either
                    // way it is pointer focus, not shown.
                    if primary {
                        // A press anywhere ends the last selection; the
                        // arms below start whichever new one it begins.
                        // One selection per window (ADR 0017) — except a
                        // press inside the core's own context menu, which
                        // is *about* that selection: a Copy row that
                        // cleared what it was going to copy would be a
                        // menu that never works.
                        // The menu bar's Edit menu is about the selection
                        // for the same reason, so a press in it is spared
                        // the same way.
                        let origin = hit.as_ref().map(|(.., o)| *o);
                        let in_bar = origin == Some(OriginId::MENU_BAR);
                        // A Shift-press inside the scope the selection is
                        // in keeps its anchor and moves the live end: it
                        // extends, so it clears nothing (ADR 0029,
                        // decision 3). Anywhere else Shift is a press.
                        let shift = self.interaction.modifiers().shift;
                        let extends = shift
                            && hit.as_ref().is_some_and(|(_, _, _, scope, _)| {
                                scope.is_some()
                                    && (self.selection.map(|s| s.scope) == *scope
                                        || self.cell_selection.map(|c| c.node) == *scope)
                            });
                        if origin != Some(OriginId::MENU) && !in_bar && !extends {
                            self.clear_selection();
                        }
                        // And the field a menu-bar menu will be about: this
                        // press is about to move focus onto the title, so
                        // the answer has to be taken before it does. Only
                        // on the way *in* — a press with a menu already
                        // open is a row or a second title, and focus is
                        // inside the bar by then, so asking again would
                        // record "no field" over the real answer.
                        if in_bar && self.menu_bar_open().is_none() {
                            self.note_menu_bar_editor();
                        }
                        match hit {
                            Some((key, Some(origin), true, _, _)) => {
                                // A Shift-press in the focused editor
                                // extends from its caret: cosmic-text's
                                // `Drag` is the action that moves the
                                // cursor and keeps (or seeds) the
                                // selection, which is the whole gesture.
                                let extend = shift && self.edit.focused() == Some(key);
                                self.move_focus(Some(key));
                                let local = Vec2::new(p.x - origin.x, p.y - origin.y);
                                self.edit_with_fonts(|edit, fs| {
                                    edit.click(key, local, clicks, extend, fs)
                                });
                                self.edit.dragging = Some((key, origin));
                                self.arm_follow(key, p);
                            }
                            // Inside a selection scope, with nothing else
                            // claiming the press: start a drag-select.
                            // One click places both ends together, two
                            // take the word, three the whole run.
                            Some((key, _, focusable, Some(scope), _)) => {
                                let target = self.press_focus(key, focusable);
                                self.move_focus(target);
                                self.settle_region(Some(key));
                                // The press arms the drag with what the
                                // click count says it moves by: a second
                                // click held and dragged selects word by
                                // word, a third run by run — in bytes or,
                                // for a grid, in cells; the arming knows.
                                // A Shift-press keeps the anchor instead
                                // and goes on by characters.
                                if self.arm_select_drag(scope, p, clicks, extends) {
                                    self.arm_follow(scope, p);
                                }
                            }
                            // Everything else: a plain node, and a
                            // disabled editor (no caret to place).
                            Some((key, _, focusable, None, _)) => {
                                let target = self.press_focus(key, focusable);
                                self.move_focus(target);
                                // Whatever the press did to focus, Tab
                                // afterwards enters the ring under the
                                // pointer (`docs/adr/0022`, decision 3).
                                self.settle_region(Some(key));
                            }
                            None => {
                                self.move_focus(None);
                                self.settle_region(None);
                            }
                        }
                        self.focus_visible = false;
                    }
                }
                let n = self
                    .interaction
                    .handle(InputEvent::MouseDown { button, clicks }, &mut out);
                self.attach_pointer(&mut out, n);
                // A right-click the app did not claim with `onContextMenu`
                // gets the stock menu, where there is anything standard to
                // put in one (ADR 0017, decision 5).
                if button == MouseButton::Secondary
                    && let Some(p) = self.interaction.cursor()
                {
                    let claimed = out.iter().any(|e| e.kind() == Some("contextmenu"));
                    self.auto_menu(p, claimed);
                }
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
                self.rehit(p);
                self.follow_point(p);
                let n = self
                    .interaction
                    .handle(InputEvent::CursorMoved(p), &mut out);
                self.attach_pointer(&mut out, n);
            }
            InputEvent::MouseUp { button } => {
                if button == MouseButton::Primary {
                    self.end_follow();
                    self.edit.dragging = None;
                    self.select_dragging = None;
                    self.interaction.scrollbar_drag = None;
                }
                let n = self
                    .interaction
                    .handle(InputEvent::MouseUp { button }, &mut out);
                self.attach_pointer(&mut out, n);
            }
            InputEvent::ForceClick(p) => self.force_click(p, &mut out),
            InputEvent::Access(req) => self.handle_access(req, &mut out),
            other => {
                self.interaction.handle(other, &mut out);
            }
        }
        self.flush_sound_requests();
        // Input moves focus, carets and scroll offsets: an access tree
        // derived earlier this frame no longer describes it.
        self.access_built = 0;
        out
    }

    /// A force click (ADR 0017, decision 6). Over text — an editor or a
    /// `selectable` scope — it selects the word under it and asks the host
    /// for its definition panel, which is what the gesture means on the
    /// one platform that has it. Anywhere else it reaches a node
    /// declaring `on_force_click`, and over anything else it does
    /// nothing at all.
    ///
    /// It moves no focus and places no caret: it happens *during* a press
    /// that is still running, and stealing the caret out from under a
    /// drag would be a gesture fighting itself.
    fn force_click(&mut self, p: Vec2, out: &mut Vec<UiEvent>) {
        let Some(region) = self.interaction.hit_at(p) else {
            return;
        };
        let (key, origin) = (region.key, region.origin);
        // Read off the tree rather than carried on the region: a force
        // click is one event in a session, and a tag on `HitRegion` is a
        // clone on every region of every frame (C15's rule — a node pays
        // for props it does not declare).
        let tag = self
            .tree
            .keys
            .iter()
            .position(|k| *k == key)
            .and_then(|i| self.tree.specs[i].events().on_force_click.clone());
        let editor = region.edit_origin.map(|origin| (key, origin));
        let scope = region.select_scope;
        // Text first: the word under the pointer, selected, and looked up.
        // The press that deepened into this force click is still running,
        // and its drag would overwrite the word the moment the finger
        // moved a pixel — which is what "the panel says one word and the
        // highlight is one character" looks like. The gesture takes the
        // press over: no caret drag, no selection drag.
        self.select_dragging = None;
        self.edit.dragging = None;
        self.drag_follow = None;
        if let Some((key, content_origin)) = editor {
            let local = Vec2::new(p.x - content_origin.x, p.y - content_origin.y);
            self.move_focus(Some(key));
            self.edit_with_fonts(|edit, fs| edit.click(key, local, 2, false, fs));
            self.menu_editor = Some(key);
            if let Some(action) = self.lookup_action() {
                self.menu_actions.push(action);
            }
            return;
        }
        if let Some(scope) = scope {
            // A force click is a double click that also asks for a
            // definition, so it takes the same word the second click
            // would have.
            if !self.select_word_under(scope, p) {
                return;
            }
            // A force click between words is a force click on nothing:
            // looking up a space would put a dictionary panel over the
            // page for no reason, which is not what the gesture does
            // anywhere else on the platform.
            if self.copy_selection().is_none_or(|t| t.trim().is_empty()) {
                self.clear_selection();
                return;
            }
            if let Some(action) = self.lookup_action() {
                self.menu_actions.push(action);
            }
            return;
        }
        // Not text: the node's own event, if it asked for one.
        let Some(tag) = tag else { return };
        let payload = Value::map([
            ("kind", Value::str("forceclick")),
            ("x", Value::Float(p.x as f64)),
            ("y", Value::Float(p.y as f64)),
        ]);
        out.push(UiEvent::on(origin, key, payload).tagged(Some(&tag)));
    }

    /// Resolves a request from assistive technology against the last
    /// frame the way the pointer or keyboard equivalent would be (see
    /// [`crate::access::AccessAction`]).
    fn handle_access(&mut self, req: crate::access::AccessRequest, out: &mut Vec<UiEvent>) {
        use crate::access::AccessAction;
        let key = req.key;
        let idx = self.tree.index_of(key);
        // The gates every other channel obeys (AR18): a node outside the
        // modal is inert (ADR 0003 decision 5) and a disabled one takes no
        // action. Only `Click` resolved against the hit list before; a
        // reader edited, nudged and scrolled the page behind a dialog,
        // and `Focus` on an editor there routed typing to it until the
        // next frame's containment.
        if let Some(i) = idx
            && !self.interactive(i)
        {
            // The access tree is not pruned (ADR 0003 decision 7), so a
            // reader can name a node behind the modal, and its click is
            // the press outside: the modal is asked to go away, and the
            // node hears nothing (decision 6). Dropped on the floor, a
            // select's field clicked a second time left its own menu
            // open where the pointer closes it (backlog RG13). Every
            // other request behind a modal does nothing — nothing a
            // pointer does to the page behind a dialog moves its text.
            if req.action == AccessAction::Click
                && let Some(modal) = self.modal()
            {
                self.dismiss(modal, "outside", out);
            }
            return;
        }
        // Disabled: what the access tree refuses to advertise, so a
        // request naming one is a reader working from a stale tree, or a
        // headless test.
        if let Some(i) = idx
            && self.tree.specs[i].disabled
        {
            return;
        }
        match req.action {
            AccessAction::Click => self.click_node(key, out),
            AccessAction::Focus => {
                // The reader's cursor lands where Tab would — on a node it
                // can see (decoration is not in its tree); show it.
                let exposed = self.access_tree().get(key).is_some();
                if exposed && idx.is_some_and(|i| crate::access::focusable(&self.tree, i)) {
                    self.move_focus(Some(key));
                    self.focus_visible = true;
                }
            }
            AccessAction::Blur => {
                if self.focus == Some(key) {
                    self.move_focus(None);
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
                    let payload = Value::map([
                        ("kind", Value::str("access")),
                        ("action", Value::str(req.action.name())),
                        ("text", Value::str(req.value.unwrap_or_default())),
                    ]);
                    out.push(
                        UiEvent::on(self.tree.origins[i], key, payload)
                            .tagged(self.access_tag(i).as_ref()),
                    );
                } else if let Some(i) = idx {
                    self.set_slider(i, req.value.as_deref().unwrap_or_default(), out);
                }
            }
            AccessAction::Increment | AccessAction::Decrement => {
                let Some(i) = idx else { return };
                let n = if req.action == AccessAction::Increment {
                    1
                } else {
                    -1
                };
                self.nudge(i, crate::slider::SliderMove::Step(n), out);
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
                            self.editor_took_selection(key);
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
                    out.push(
                        UiEvent::on(self.tree.origins[i], key, Value::Map(entries))
                            .tagged(tag.as_ref()),
                    );
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
            let src = crate::access::Sources {
                text: &self.text,
                cells: &self.cells,
                edit: &self.edit,
                scroll: &self.scroll,
                title: self.window_title.as_deref(),
                focus: self.focus,
                modal: self.modal(),
                viewport: self.viewport,
                scale: self.scale,
                clips: &self.clips,
            };
            // Deriving the tree is about 480 µs on a 10,000-node frame and
            // is paid on every frame a screen reader is attached; hashing
            // what it reads is about 105 µs, because three quarters of the
            // work is making the nodes rather than walking to them. So a
            // frame that changed nothing this tree can see — a pointer
            // moving across hover backgrounds, a colour transition — keeps
            // the one it had. See `access::inputs_hash` for the invariant
            // that makes it safe, and ADR 0016 decision 3 for why this is
            // the one thing in the frame that gets cached.
            let hash = crate::access::inputs_hash(&self.tree, &src);
            if hash.is_none() || self.access_inputs != hash {
                self.access = crate::access::build(&self.tree, &src);
                self.access_rebuilds += 1;
            }
            self.access_inputs = hash;
            self.access_built = self.frame_no;
        }
        &self.access
    }

    /// Delivers one key event to the sink it resolves to, tagged with that
    /// sink's `on_key` payload; returns whether anything took it. The
    /// focused edit widget owns the keyboard (it takes the Text/EditKey
    /// path), so a sink only hears while no editor is focused and it is
    /// still in the last frame's hit list. *Which* sink is
    /// [`Self::key_target`]'s answer: the focused one, or — when a control
    /// holds focus and does not claim this key — the nearest one above it
    /// (`docs/adr/0011-keys-bubble-to-the-enclosing-sink.md`).
    fn route_key(&mut self, kp: &KeyPress, phase: KeyPhase, out: &mut Vec<UiEvent>) -> bool {
        if self.edit.focused().is_some() {
            return false;
        }
        let Some(target) =
            self.key_target(kp.code, kp.mods.ctrl || kp.mods.alt || kp.mods.super_key)
        else {
            return false;
        };
        // A sink hears releases only by asking (`key_up`): press-only is
        // the keymap case, and a keymap handed both halves runs every
        // binding twice. The key is still tracked as held either way, so
        // a sink that opts in mid-hold hears the release it is owed.
        if phase == KeyPhase::Up
            && !self
                .sink_node(target)
                .is_some_and(|i| self.tree.specs[i].events().key_up)
        {
            return false;
        }
        self.deliver_to_sink(target, kp.to_value(phase), out)
    }

    /// The node the sink `key` names, in the last frame's tree: one that
    /// declares `on_key`, is not disabled, and is not shut out by a
    /// modal.
    ///
    /// Asked of the tree and not of the hit list, because a key is not
    /// pointer input. The hit list is where a *point* finds a node, and
    /// a node outside its scroller's clip is not under any point, so it
    /// has no region there (`emit_node` is never reached for it) — while
    /// the keyboard reaches a node by having focus, which a node keeps
    /// wherever it is drawn. Before F79 the delivery read the hit list
    /// like a click, so a focused sink scrolled out of view, or drawn
    /// part-way to its place by `slide` or an `enter` offset, dropped
    /// every key typed at it until it came back.
    fn sink_node(&self, key: Key) -> Option<usize> {
        let i = (0..self.tree.len())
            .rev()
            .find(|&i| self.tree.keys[i] == key)?;
        let spec = &self.tree.specs[i];
        (spec.events().on_key.is_some() && !spec.disabled && self.interactive(i)).then_some(i)
    }

    /// Hands `payload` to the sink `target` names with the sink's tag
    /// merged in — the one delivery both key channels end in. False when
    /// the last frame declared no such sink.
    fn deliver_to_sink(&self, target: Key, payload: Value, out: &mut Vec<UiEvent>) -> bool {
        let Some(i) = self.sink_node(target) else {
            return false;
        };
        out.push(
            UiEvent::on(self.tree.origins[i], self.tree.keys[i], payload)
                .tagged(self.tree.specs[i].events().on_key.as_ref()),
        );
        true
    }

    /// Delivers `payload` to the sink the focused node reports to — the
    /// focused sink itself, or the nearest one above a focused control —
    /// with the sink's tag merged in, the way a `key` event is. A
    /// composition is never a control's to claim, so unlike `route_key`
    /// nothing is asked about the key. False with no sink to hear it.
    fn sink_event(&mut self, payload: Value, out: &mut Vec<UiEvent>) -> bool {
        let Some(i) = self.focus_index() else {
            // With nothing focused, the root sink that hears every
            // unclaimed key (`key_target`) hears this too — a paste a
            // shell asked for with nothing focused would otherwise
            // vanish (backlog C33). Not under a modal.
            if self.tree.is_empty() || self.modal.is_some() {
                return false;
            }
            let root = &self.tree.specs[0];
            if root.events().on_key.is_none() || root.disabled {
                return false;
            }
            return self.deliver_to_sink(self.tree.keys[0], payload, out);
        };
        let target = if self.tree.specs[i].events().on_key.is_some() {
            self.tree.keys[i]
        } else {
            match self.enclosing_sink(i) {
                Some(j) => self.tree.keys[j],
                None => return false,
            }
        };
        self.deliver_to_sink(target, payload, out)
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

    /// The driver's report that this window gained or lost the keyboard:
    /// `env.focused`, plus the one rule that rides on it — a window that
    /// lost the keyboard lets go of every key its sink was holding, since
    /// the OS stops delivering key events to it and the release would
    /// never arrive. The rule lives here rather than in each driver so a
    /// Node test's `setEnv({focused: false})` and a C host's `kui_env_set`
    /// do what the windowed runner does, instead of each remembering to.
    /// The synthetic `up`s are pending, like `release_held_keys`'s.
    pub fn set_focused(&mut self, focused: bool) {
        if self.env.focused == focused {
            return;
        }
        self.env.focused = focused;
        // The app hears it as a window event rather than diffing
        // `env.focused` every frame (backlog DX18).
        let (name, id) = (self.window_name(), self.env.window.id);
        self.push_window_event(if focused { "focused" } else { "blurred" }, &name, id);
        if !focused {
            self.release_held_keys();
            // The modifiers go with the keys: a Shift released in another
            // window never reaches this one, and a host that does not
            // resend the state on the way back (winit does; a C loop may
            // not) would otherwise leave every later press an extending
            // one. The app hears it as the `modifiers` event it is.
            let mut out = Vec::new();
            self.interaction.handle(
                InputEvent::Modifiers(crate::input::KeyMods::default()),
                &mut out,
            );
            self.pending.append(&mut out);
            // And a held drag's follow: the release will not come here,
            // and a scroller stepping toward a pointer nobody holds any
            // more is a window asking for frames until one does.
            self.drag_follow = None;
        }
    }

    /// The sink a chord pressed now would reach, if any: the focused sink,
    /// the nearest one above the focused control, or the root's with
    /// nothing focused (ADR 0011, decision 1; ADR 0022, decision 8). What a
    /// driver asks before greying a menu row that spells a chord — a sink
    /// that would hear ⌘C may do anything with it, so the row stays lit.
    pub fn chord_sink(&self) -> Option<Key> {
        if self.edit.focused().is_some() {
            return None;
        }
        self.key_target(KeyCode::Char('c'), true)
    }

    /// Which node hears a raw press: the focused sink, the nearest sink
    /// above a focused control that does not claim the key, or nothing
    /// (`docs/adr/0011-keys-bubble-to-the-enclosing-sink.md`, decision 1).
    ///
    /// `chord` is whether a modifier other than Shift is down. A chord is
    /// never a control's key — it is what a shortcut layer is made of — so
    /// it bubbles whatever the focused control would have done with the
    /// bare key.
    fn key_target(&self, code: KeyCode, chord: bool) -> Option<Key> {
        // With nothing focused there is nothing to claim, and the sink
        // that hears every unclaimed key in the tree — one on the root —
        // hears this one too (`docs/adr/0022`, decision 8). Not under a
        // modal, where the root is inert like everything outside it. A
        // shell used to take focus on the root to get this.
        let Some(i) = self.focus_index() else {
            // Tab is still the ring's: it enters, and the sink does not
            // hear it — a chord on it bubbles as any chord does.
            if self.tree.is_empty() || self.modal.is_some() || (!chord && code == KeyCode::Tab) {
                return None;
            }
            let root = &self.tree.specs[0];
            return (root.events().on_key.is_some() && !root.disabled).then_some(self.tree.keys[0]);
        };
        // A sink that holds focus keeps everything, as it always has
        // (`docs/adr/0002`, decision 3).
        if self.tree.specs[i].events().on_key.is_some() {
            return Some(self.tree.keys[i]);
        }
        if !chord && self.claims(i, code) {
            return None;
        }
        self.enclosing_sink(i).map(|j| self.tree.keys[j])
    }

    /// Whether the key `code` pressed on the focused node `i` reaches a
    /// sink above it instead of the node itself — the question the
    /// `EditKey` and `Text` channels ask, so that both agree with the raw
    /// press channel about who owns the key. False with no sink above, so
    /// a key nothing claims does exactly what it did before.
    fn bubbles(&self, i: usize, code: KeyCode, chord: bool) -> bool {
        (chord || !self.claims(i, code)) && self.enclosing_sink(i).is_some()
    }

    /// Whether the focused node `i` takes `code` for itself: the keys the
    /// core acts on *for that node*, which are exactly the keys that never
    /// bubble (`docs/adr/0011`, decision 2). Static — a press is resolved
    /// on its way down, before the channel that would act on it arrives,
    /// so the question has to be answerable from the node and the key
    /// alone rather than from what a handler did.
    fn claims(&self, i: usize, code: KeyCode) -> bool {
        use crate::access::Role;
        // A space bar reported as a character is still the space bar.
        let code = match code {
            KeyCode::Char(' ') => KeyCode::Space,
            c => c,
        };
        // Tab belongs to the ring wherever focus is: a shell sink that
        // heard every Tab would be this ADR's own bug in reverse.
        if code == KeyCode::Tab {
            return true;
        }
        // Only a control the core presses itself claims anything else; a
        // plain box someone focused by hand claims nothing.
        if self.focused_control() != Some(i) {
            return false;
        }
        let key = self.tree.keys[i];
        // Enter and Space activate what there is to activate: a node with
        // no click payload has nothing, so its Space is free to bubble.
        let activates = self
            .interaction
            .hits
            .iter()
            .rev()
            .find(|h| h.key == key)
            .is_some_and(|h| h.payload.is_some() || h.window.is_some());
        let item = crate::composite::owner(&self.tree, i, &mut Vec::new()).is_some();
        let slider = self.tree.specs[i].access().role == Some(Role::Slider);
        let changes = slider && self.tree.specs[i].events().on_change.is_some();
        match code {
            KeyCode::Enter => activates,
            // Inside a composite, Space either extends a type-ahead search
            // or presses the item (`docs/adr/0007`, decision 9).
            KeyCode::Space => activates || item,
            KeyCode::Left | KeyCode::Right | KeyCode::Up | KeyCode::Down => slider || item,
            KeyCode::Home | KeyCode::End => item || changes,
            KeyCode::PageUp | KeyCode::PageDown => changes,
            // Type-ahead inside a composite; nothing anywhere else.
            KeyCode::Char(_) => item,
            _ => false,
        }
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
        // Not a press: the payload gains no `cell` and no `line` /
        // `byte` / `clicks` from wherever the pointer rests — only what
        // `Interaction::handle` made from a press does (`attach_pointer`)
        // — and the count the last press carried describes nothing now.
        self.interaction.note_synthetic_click();
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
                slot: None,
            }),
            _ => {}
        }
        if takes_focus {
            self.move_focus(Some(key));
        }
    }

    /// The modal `key` was asked to go away — Escape, or a press outside
    /// it. Reaches the app as `{kind="dismiss", reason, tag}` on the modal
    /// node; what happens next is the app's, since only it can stop
    /// declaring the node (see `docs/adr/0003-modal-surfaces.md`).
    fn dismiss(&mut self, key: Key, reason: &str, out: &mut Vec<UiEvent>) {
        let Some(i) = self.tree.index_of(key) else {
            return;
        };
        let payload = Value::map([
            ("kind", Value::str("dismiss")),
            ("reason", Value::str(reason)),
        ]);
        out.push(
            UiEvent::on(self.tree.origins[i], key, payload)
                .tagged(self.tree.specs[i].events().modal.as_ref()),
        );
    }

    pub(crate) fn push_edit_event(&self, key: Key, kind: &str, out: &mut Vec<UiEvent>) {
        out.push(UiEvent {
            origin: self.edit.origin_of(key).unwrap_or(OriginId::HOST),
            window: WindowId::MAIN,
            key,
            payload: Value::map([("kind", kind.into())]),
            slot: None,
        });
    }

    /// The window's selected text, for clipboard integration: the
    /// selection in a `selectable` scope when there is one, else the
    /// focused editor's. Only one of the two exists at a time — starting
    /// either clears the other (`docs/adr/0017-selection-as-a-scope.md`)
    /// — so this asks in that order rather than merging them.
    pub fn copy_selection(&self) -> Option<String> {
        if self.selection.is_some() {
            return self.selection_text();
        }
        if self.cell_selection.is_some() {
            return self.cell_selection_text();
        }
        self.edit.copy_selection(self.edit.focused()?)
    }

    /// One selection per window (ADR 0017 decision 1), in the direction
    /// `set_selection` does not cover: an editor's selection started by
    /// the keyboard — Select All, Shift+arrows, a reader's
    /// `setTextSelection` — takes the window's one selection with it, so
    /// a scope's or a grid's highlight goes and Cmd-A / Cmd-C read the
    /// editor and not the label dragged over before Tab (AR24).
    fn editor_took_selection(&mut self, key: Key) {
        if self.edit.has_selection(key) {
            self.selection = None;
            self.cell_selection = None;
        }
    }

    /// Cuts the focused editor's selection, returning the removed text.
    /// A cut is an edit like any other, so the editor's `changed` is
    /// pending for the caller to route — like a `resize`, since the
    /// caller is not answering an input event (AR15: the runner used to
    /// build one by hand here, and the menu path posted none).
    pub fn cut_selection(&mut self) -> Option<String> {
        let key = self.edit.focused()?;
        let text = self.cut_editor(key)?;
        let mut out = Vec::new();
        self.push_edit_event(key, "changed", &mut out);
        self.pending.append(&mut out);
        Some(text)
    }

    /// Deletes editor `key`'s selection, returning what was there — the
    /// one mutation both cuts share; the `changed` is the caller's to
    /// post where its batch goes.
    pub(crate) fn cut_editor(&mut self, key: Key) -> Option<String> {
        let text = self.edit.copy_selection(key)?;
        self.edit_with_fonts(|edit, fs| edit.delete_selection(key, fs));
        Some(text)
    }

    /// Current text of an editor by key.
    pub fn edit_text(&self, key: Key) -> Option<String> {
        self.edit.text(key)
    }

    /// Replaces an editor's text, leaving the caret at the end.
    ///
    /// The key need not have an editor behind it yet: an `update` that
    /// opens a rename field runs a frame ahead of the view that declares
    /// it, so the text is held and seeds the editor the next frame
    /// declares under this key, over its `initial`. Held for that one
    /// frame — a key nothing declares on it drops its text and raises
    /// [`crate::diag::EDIT_TEXT_WITHOUT_EDITOR`].
    ///
    /// Returns whether the text reached an editor now. `false` is the
    /// held case: nothing on screen changed, and a driver that redraws on
    /// it re-lowers the tree that declares no editor, which is the frame
    /// the hold expires on (backlog F42) — so a binding asks for a redraw
    /// only on `true`.
    pub fn set_edit_text(&mut self, key: Key, text: &str) -> bool {
        let sess = &mut *self.session.state();
        self.edit
            .set_text(key, text, &mut sess.fonts, &sess.resources)
    }

    /// The same call by the name the view declares — an editor's `key`
    /// prop / `label` — for the app that has no key to give: the hex key
    /// comes from an event the node fired, and an editor a rename opens
    /// for the first time has fired none (backlog F32).
    ///
    /// A label some frame declared resolves now ([`Core::key_of`]) and
    /// this is [`Core::set_edit_text`] on that key. One nothing has
    /// declared — a first open, or a second one, since an editor closed
    /// in between was in no recent frame — is held for the next frame
    /// that declares an editor under it, and seeds it there. An editor
    /// retained while its key was off screen takes the text over its
    /// draft, which is what a `set_edit_text` by key cannot say.
    ///
    /// Held for that one frame: a label nothing declares on it drops its
    /// text and raises [`crate::diag::EDIT_TEXT_WITHOUT_EDITOR`].
    ///
    /// Returns whether the text reached an editor now, as
    /// [`Core::set_edit_text`] does; a label held is `false`.
    pub fn set_edit_text_by_label(&mut self, label: &str, text: &str) -> bool {
        match self.key_of(label) {
            Some(key) => self.set_edit_text(key, text),
            None => {
                self.edit.hold_label(label, text);
                false
            }
        }
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

/// The raw key an editing key came from, for the keys a focused control
/// acts on — the one place the two input channels have to name the same
/// press (`docs/adr/0011`, decision 3). `None` for the editing vocabulary
/// with no control behaviour behind it (Backspace, PageUp, Undo): those
/// arms do nothing on a control either way.
fn edit_key_code(ek: EditKey) -> Option<KeyCode> {
    Some(match ek {
        EditKey::Enter => KeyCode::Enter,
        EditKey::Escape => KeyCode::Escape,
        EditKey::Tab => KeyCode::Tab,
        EditKey::Left => KeyCode::Left,
        EditKey::Right => KeyCode::Right,
        EditKey::Up => KeyCode::Up,
        EditKey::Down => KeyCode::Down,
        EditKey::Home => KeyCode::Home,
        EditKey::End => KeyCode::End,
        _ => return None,
    })
}

/// ADR 0016 decision 3's gate: the access tree is cached on a hash of what
/// derives it, so every input that hash misses is a frame that serves a
/// stale reading. One case per input `access::build` reads, each mutating
/// only that input and asserting the tree was derived again *and* came out
/// different; plus the case the cache exists for, where a frame changes
/// something the tree cannot see and keeps the one it had.
#[cfg(test)]
mod access_cache {
    use crate::access::{Live, Role};
    use crate::*;

    /// Builds a frame from `spec` on a keyed node with a text child, reads
    /// the access tree, and reports (how many derivations have happened,
    /// the tree's own hash).
    fn frame(core: &mut Core, spec: NodeSpec) -> (u64, u64) {
        let mut ui = core.frame(Size::new(200.0, 100.0), 1.0);
        ui.configure_root(NodeSpec::column());
        ui.text_in_keyed("node", spec, "hello", TextStyle::new(12.0));
        ui.finish();
        let hash = core.access_tree().hash;
        (core.access_rebuilds, hash)
    }

    /// The base node: a button, so it is a semantic node with a name.
    fn base() -> NodeSpec {
        NodeSpec::column()
            .size(40.0, 20.0)
            .on_click(Value::from(1.0))
            .label("Save")
    }

    /// Draws `first`, then `second`, and says whether the second frame
    /// derived the tree again and whether the tree changed.
    fn change(first: NodeSpec, second: NodeSpec) -> (bool, bool) {
        let mut core = Core::new();
        let (b0, h0) = frame(&mut core, first.clone());
        let (b1, h1) = frame(&mut core, second);
        (b1 > b0, h1 != h0)
    }

    /// The case the cache is for: a pointer moving over a hover background
    /// changes `style.bg` before the node is pushed, and the access tree
    /// cannot see a background. The tree is kept, not rebuilt.
    #[test]
    fn a_change_the_tree_cannot_see_keeps_the_tree() {
        let (rebuilt, moved) = change(base().bg(Color::WHITE), base().bg(Color::BLACK));
        assert!(!rebuilt, "a colour the access tree never reads rebuilt it");
        assert!(!moved, "and the tree would have been the same anyway");
    }

    /// And the same frame twice: no input moved at all.
    #[test]
    fn an_identical_frame_keeps_the_tree() {
        assert_eq!(change(base(), base()), (false, false));
    }

    macro_rules! moves_the_tree {
        ($($name:ident: $first:expr => $second:expr;)*) => {$(
            #[test]
            fn $name() {
                let (rebuilt, moved) = change($first, $second);
                assert!(rebuilt, "the tree was served from the cache");
                assert!(moved, "it was derived again but came out the same");
            }
        )*};
    }

    moves_the_tree! {
        label:        base().label("Save") => base().label("Open");
        description:  base() => base().description("Writes the file");
        role:         base() => base().role(Role::Checkbox);
        clickable:    NodeSpec::column().label("x") => NodeSpec::column().label("x").on_click(Value::from(1.0));
        disabled:     base() => base().disabled(true);
        live:         base() => base().live(Live::Polite);
        checked:      base().role(Role::Checkbox) => base().role(Role::Checkbox).checked(true);
        selected:     base().role(Role::Tab) => base().role(Role::Tab).selected(true);
        expanded:     base() => base().expanded(true);
        value_now:    base().role(Role::Slider) => base().role(Role::Slider).value_now(3.0);
        value_min:    base().role(Role::Slider).value_now(3.0) => base().role(Role::Slider).value_now(3.0).value_min(1.0);
        value_text:   base().role(Role::Slider) => base().role(Role::Slider).value_text("three");
        focusable:    NodeSpec::column().role(Role::Group).label("g")
                          => NodeSpec::column().role(Role::Group).label("g").focusable();
        rect:         base() => base().width(80.0);
    }

    /// The text a node is named by is not in its spec at all — it is the
    /// child text node's content, reached through the text system.
    #[test]
    fn the_text_a_node_reads_moves_the_tree() {
        let mut core = Core::new();
        let mut draw = |s: &str| {
            let mut ui = core.frame(Size::new(200.0, 100.0), 1.0);
            ui.configure_root(NodeSpec::column());
            ui.text(s, TextStyle::new(12.0));
            ui.finish();
            let h = core.access_tree().hash;
            (core.access_rebuilds, h)
        };
        let (b0, h0) = draw("hello");
        let (b1, h1) = draw("goodbye");
        assert!(b1 > b0 && h1 != h0, "a changed string kept its old node");
    }

    /// The clip a node was emitted under is not in its spec either: a
    /// clipping parent narrowing cuts the rect of a node whose own box
    /// stayed where it was (F93).
    #[test]
    fn a_parents_clip_moves_the_tree() {
        let mut core = Core::new();
        let mut draw = |w: f32| {
            let mut ui = core.frame(Size::new(200.0, 100.0), 1.0);
            ui.configure_root(NodeSpec::column());
            let clipper = NodeSpec::column().size(w, 40.0).clip();
            ui.with(clipper, |ui| {
                ui.leaf_keyed("node", base());
            });
            ui.finish();
            let hash = core.access_tree().hash;
            (core.access_rebuilds, hash)
        };
        let (b0, h0) = draw(100.0);
        let (b1, h1) = draw(30.0);
        assert!(b1 > b0 && h1 != h0, "the clip moved and the tree did not");
    }

    /// Focus is the core's, not any node's spec.
    #[test]
    fn focus_moves_the_tree() {
        let mut core = Core::new();
        let (b0, h0) = frame(&mut core, base().focusable());
        core.set_key_focus(Some(Key::ROOT.str("node")));
        let (b1, h1) = frame(&mut core, base().focusable());
        assert!(b1 > b0 && h1 != h0, "focus moved and the tree did not");
    }

    /// So is the scroll offset, which is retained across frames and only
    /// ever reaches the tree through the store.
    #[test]
    fn a_scroll_offset_moves_the_tree() {
        let mut core = Core::new();
        // Tall content, or the offset clamps to zero and nothing moved.
        let scroller = || NodeSpec::column().size(40.0, 20.0).scroll_y().label("list");
        let draw = |core: &mut Core| {
            let mut ui = core.frame(Size::new(200.0, 100.0), 1.0);
            ui.configure_root(NodeSpec::column());
            ui.with_keyed("node", scroller(), |ui| {
                ui.leaf_keyed("tall", NodeSpec::column().height(400.0));
            });
            ui.finish();
            let h = core.access_tree().hash;
            (core.access_rebuilds, h)
        };
        let (b0, h0) = draw(&mut core);
        core.set_scroll(Key::ROOT.str("node"), Vec2::new(0.0, 7.0));
        let (b1, h1) = draw(&mut core);
        assert!(b1 > b0 && h1 != h0, "the offset moved and the tree did not");
    }

    /// An editor's text reaches the tree through the store, and its shaped
    /// runs through a version that stands in for them.
    #[test]
    fn editor_text_moves_the_tree() {
        let mut core = Core::new();
        let draw = |core: &mut Core, text: &str| {
            let mut ui = core.frame(Size::new(200.0, 100.0), 1.0);
            ui.configure_root(NodeSpec::column());
            let key = ui.text_edit(
                "name",
                text,
                &Default::default(),
                NodeSpec::column().size(120.0, 20.0),
            );
            ui.finish();
            let h = core.access_tree().hash;
            (core.access_rebuilds, h, key)
        };
        let (b0, h0, key) = draw(&mut core, "one");
        core.frame(Size::new(200.0, 100.0), 1.0).finish();
        core.set_edit_text(key, "typed");
        let (b1, h1, _) = draw(&mut core, "one");
        assert!(
            b1 > b0 && h1 != h0,
            "the editor's text moved and the tree did not"
        );
    }

    /// And the viewport, which is the root node's whole rect.
    #[test]
    fn the_viewport_moves_the_tree() {
        let mut core = Core::new();
        let mut draw = |w: f32| {
            let mut ui = core.frame(Size::new(w, 100.0), 1.0);
            ui.configure_root(NodeSpec::column());
            ui.leaf_keyed("node", base());
            ui.finish();
            let h = core.access_tree().hash;
            (core.access_rebuilds, h)
        };
        let (b0, h0) = draw(200.0);
        let (b1, h1) = draw(300.0);
        assert!(
            b1 > b0 && h1 != h0,
            "the viewport moved and the tree did not"
        );
    }
}
