//! The keyboard, as winit reports it and as the core wants it: the
//! physical-code table, and `Shell::on_key` — the two channels a press
//! travels on (the raw key to a sink, the editing key the same press
//! means), the clipboard chords the runner performs itself, and IME.
//! Split off `lib.rs` as a pure move.

use super::*;

/// The US-QWERTY key at a physical position, in kui's own vocabulary (see
/// [`kui_core::KeyPress::physical`]). Letters and digits are their US
/// characters, punctuation the character US-QWERTY prints there, and the
/// named keys their names — no layout is consulted, which is the point.
///
/// The numeric keypad reports the digit and operator it always bears; kui
/// has no separate numpad vocabulary, and `code` already conflates the two
/// (winit's logical key for `Numpad1` is `"1"`).
pub(crate) fn physical_code(key: winit::keyboard::PhysicalKey) -> KeyCode {
    use winit::keyboard::{KeyCode as Phys, PhysicalKey};
    let PhysicalKey::Code(c) = key else {
        return KeyCode::Unknown;
    };
    // Letters and digits, in winit's own declaration order.
    const LETTERS: [(Phys, char); 26] = [
        (Phys::KeyA, 'a'),
        (Phys::KeyB, 'b'),
        (Phys::KeyC, 'c'),
        (Phys::KeyD, 'd'),
        (Phys::KeyE, 'e'),
        (Phys::KeyF, 'f'),
        (Phys::KeyG, 'g'),
        (Phys::KeyH, 'h'),
        (Phys::KeyI, 'i'),
        (Phys::KeyJ, 'j'),
        (Phys::KeyK, 'k'),
        (Phys::KeyL, 'l'),
        (Phys::KeyM, 'm'),
        (Phys::KeyN, 'n'),
        (Phys::KeyO, 'o'),
        (Phys::KeyP, 'p'),
        (Phys::KeyQ, 'q'),
        (Phys::KeyR, 'r'),
        (Phys::KeyS, 's'),
        (Phys::KeyT, 't'),
        (Phys::KeyU, 'u'),
        (Phys::KeyV, 'v'),
        (Phys::KeyW, 'w'),
        (Phys::KeyX, 'x'),
        (Phys::KeyY, 'y'),
        (Phys::KeyZ, 'z'),
    ];
    const DIGITS: [(Phys, char); 20] = [
        (Phys::Digit0, '0'),
        (Phys::Digit1, '1'),
        (Phys::Digit2, '2'),
        (Phys::Digit3, '3'),
        (Phys::Digit4, '4'),
        (Phys::Digit5, '5'),
        (Phys::Digit6, '6'),
        (Phys::Digit7, '7'),
        (Phys::Digit8, '8'),
        (Phys::Digit9, '9'),
        (Phys::Numpad0, '0'),
        (Phys::Numpad1, '1'),
        (Phys::Numpad2, '2'),
        (Phys::Numpad3, '3'),
        (Phys::Numpad4, '4'),
        (Phys::Numpad5, '5'),
        (Phys::Numpad6, '6'),
        (Phys::Numpad7, '7'),
        (Phys::Numpad8, '8'),
        (Phys::Numpad9, '9'),
    ];
    const PUNCT: [(Phys, char); 17] = [
        (Phys::Backquote, '`'),
        (Phys::Minus, '-'),
        (Phys::Equal, '='),
        (Phys::BracketLeft, '['),
        (Phys::BracketRight, ']'),
        (Phys::Backslash, '\\'),
        (Phys::Semicolon, ';'),
        (Phys::Quote, '\''),
        (Phys::Comma, ','),
        (Phys::Period, '.'),
        (Phys::Slash, '/'),
        (Phys::NumpadDivide, '/'),
        (Phys::NumpadMultiply, '*'),
        (Phys::NumpadSubtract, '-'),
        (Phys::NumpadAdd, '+'),
        (Phys::NumpadDecimal, '.'),
        (Phys::NumpadEqual, '='),
    ];
    for (p, ch) in LETTERS.iter().chain(&DIGITS).chain(&PUNCT) {
        if *p == c {
            return KeyCode::Char(*ch);
        }
    }
    match c {
        Phys::Space => KeyCode::Space,
        Phys::Enter | Phys::NumpadEnter => KeyCode::Enter,
        Phys::Tab => KeyCode::Tab,
        Phys::Backspace | Phys::NumpadBackspace => KeyCode::Backspace,
        Phys::Delete => KeyCode::Delete,
        Phys::Escape => KeyCode::Escape,
        Phys::Insert => KeyCode::Insert,
        Phys::Home => KeyCode::Home,
        Phys::End => KeyCode::End,
        Phys::PageUp => KeyCode::PageUp,
        Phys::PageDown => KeyCode::PageDown,
        Phys::ArrowLeft => KeyCode::Left,
        Phys::ArrowRight => KeyCode::Right,
        Phys::ArrowUp => KeyCode::Up,
        Phys::ArrowDown => KeyCode::Down,
        Phys::F1 => KeyCode::F(1),
        Phys::F2 => KeyCode::F(2),
        Phys::F3 => KeyCode::F(3),
        Phys::F4 => KeyCode::F(4),
        Phys::F5 => KeyCode::F(5),
        Phys::F6 => KeyCode::F(6),
        Phys::F7 => KeyCode::F(7),
        Phys::F8 => KeyCode::F(8),
        Phys::F9 => KeyCode::F(9),
        Phys::F10 => KeyCode::F(10),
        Phys::F11 => KeyCode::F(11),
        Phys::F12 => KeyCode::F(12),
        _ => KeyCode::Unknown,
    }
}

impl<A: App> Shell<A> {
    pub(super) fn on_key(
        &mut self,
        event_loop: &ActiveEventLoop,
        i: usize,
        event: winit::event::KeyEvent,
    ) {
        let pressed = event.state == ElementState::Pressed;
        // Full-keyboard path: every press *and release* travels as data to
        // the key-focused sink (`NodeSpec::on_key`) — the core delivers the
        // release only to a sink that said `key_up`, and drops both when
        // an edit widget holds focus instead. Everything below this block
        // is the editor path, which is press-only.
        let kmods = self.panes[i].kmods();
        let plain = !kmods.ctrl && !kmods.alt && !kmods.super_key;
        // With Alt held the logical key is the composed character on some
        // layouts (macOS ⌥o → "ø"); chords want the layout key, so report
        // the modifier-stripped one instead.
        let logical = if kmods.alt {
            use winit::platform::modifier_supplement::KeyEventExtModifierSupplement;
            event.key_without_modifiers()
        } else {
            event.logical_key.clone()
        };
        // Where the key *is*, which no layout moves.
        let physical = physical_code(event.physical_key);
        let (logical_code, ktext) = match &logical {
            WinitKey::Character(s) => (
                KeyCode::Char(s.chars().next().unwrap_or('\u{fffd}')),
                plain.then(|| s.to_string()),
            ),
            WinitKey::Named(n) => (
                match n {
                    NamedKey::Space => KeyCode::Space,
                    NamedKey::ArrowLeft => KeyCode::Left,
                    NamedKey::ArrowRight => KeyCode::Right,
                    NamedKey::ArrowUp => KeyCode::Up,
                    NamedKey::ArrowDown => KeyCode::Down,
                    NamedKey::Home => KeyCode::Home,
                    NamedKey::End => KeyCode::End,
                    NamedKey::PageUp => KeyCode::PageUp,
                    NamedKey::PageDown => KeyCode::PageDown,
                    NamedKey::Backspace => KeyCode::Backspace,
                    NamedKey::Delete => KeyCode::Delete,
                    NamedKey::Enter => KeyCode::Enter,
                    NamedKey::Tab => KeyCode::Tab,
                    NamedKey::Escape => KeyCode::Escape,
                    NamedKey::Insert => KeyCode::Insert,
                    NamedKey::F1 => KeyCode::F(1),
                    NamedKey::F2 => KeyCode::F(2),
                    NamedKey::F3 => KeyCode::F(3),
                    NamedKey::F4 => KeyCode::F(4),
                    NamedKey::F5 => KeyCode::F(5),
                    NamedKey::F6 => KeyCode::F(6),
                    NamedKey::F7 => KeyCode::F(7),
                    NamedKey::F8 => KeyCode::F(8),
                    NamedKey::F9 => KeyCode::F(9),
                    NamedKey::F10 => KeyCode::F(10),
                    NamedKey::F11 => KeyCode::F(11),
                    NamedKey::F12 => KeyCode::F(12),
                    _ => KeyCode::Unknown,
                },
                (plain && *n == NamedKey::Space).then(|| " ".to_string()),
            ),
            _ => (KeyCode::Unknown, None),
        };
        // `KeyPress::from_layout` resolves the two into the code a keymap
        // binds against — the layout's key while it speaks ASCII, the
        // US-QWERTY letter at that position when it does not. Every driver
        // goes through it, so a C host with its own windowing gets the same
        // rule as this one.
        // The Alt branch above took the logical key with every modifier
        // stripped, Shift included, so its fallback is resolved the same
        // way: ⌥⇧ on the key printed J is `j` on a US layout and stays
        // `j` on a Russian one, rather than `J` on the second alone.
        let resolve_mods = KeyMods {
            shift: kmods.shift && !kmods.alt,
            ..kmods
        };
        let kp = KeyPress::from_layout(logical_code, physical, resolve_mods);
        let kp = KeyPress {
            mods: kmods,
            text: ktext,
            repeat: event.repeat,
            ..kp
        };
        // Rebound after each dispatch: a chord the app answers by closing
        // a window moves every pane behind it down one (backlog AR39).
        let mut i = i;
        if kp.code != KeyCode::Unknown {
            let Some(still) = self.dispatch(
                event_loop,
                i,
                if pressed {
                    InputEvent::KeyDown(kp.clone())
                } else {
                    InputEvent::KeyUp(kp.clone().released())
                },
            ) else {
                return;
            };
            i = still;
        }
        if !pressed {
            return;
        }

        // Clipboard + select-all shortcuts (edit widgets and selection
        // scopes — a key sink gets the raw chord and brings its own
        // bindings).
        if self.panes[i].primary()
            && let WinitKey::Character(c) = &event.logical_key
        {
            let lower = c.to_lowercase();
            let mut chars = lower.chars();
            if let (Some(letter), None) = (chars.next(), chars.next())
                && self.edit_chord(event_loop, i, letter, self.panes[i].modifiers.shift_key())
            {
                return;
            }
        }

        // The editor channel: the same press again, as what the core is
        // asked to *do* with that key. The table lives in the core
        // (`KeyPress::edit_event`) rather than here, so this runner and
        // every headless injector send the same second event for the same
        // key — pressing Escape dismissed a modal in a window and did
        // nothing in a test for as long as there were two copies of it
        // (backlog F6).
        if let Some(ev) = kp.edit_event() {
            // A popup owns Escape the way a modal node does, and for the
            // same reason (ADR 0003, one level up): it asks to go away, and
            // nothing else happens. A modal *inside* the popup is asked
            // first, which is the core's own precedence read at this level
            // — the surface nearest the user answers.
            let pane = &self.panes[i];
            if matches!(ev, InputEvent::Key(EditKey::Escape, _))
                && pane.kind == WindowKind::Popup
                && pane.core.modal().is_none()
            {
                let id = pane.id;
                self.dismiss(id, DismissReason::Escape);
                return;
            }
            self.dispatch(event_loop, i, ev);
            return;
        }
        // Plain typed text (IME commits arrive via WindowEvent::Ime). Not
        // the core's table's business: this is the *composed* character
        // the platform produced, which a chord-view `KeyPress` does not
        // carry — macOS's ⌥o is "ø" here and no text at all there.
        let pane = &self.panes[i];
        if !pane.primary()
            && !pane.modifiers.control_key()
            && let Some(text) = &event.text
            && text.chars().any(|c| !c.is_control())
        {
            self.dispatch(event_loop, i, InputEvent::Text(text.to_string()));
        }
    }

    /// The clipboard chords the runner performs itself — ⌘C/X/V/A, ⌘Z and
    /// ⇧⌘Z, ⌘Y — for the primary modifier plus `letter`, whether the
    /// keyboard sent it or the standard Edit menu spelled it (ADR 0030).
    /// True when the chord was one of these and was performed, so the
    /// caller's editor channel does not see the press again.
    ///
    /// A window with a selection in a `selectable` node copies it with
    /// the same Cmd-C an editor does: there is one selection per window
    /// and `copy_selection` answers for whichever it is (ADR 0017).
    pub(super) fn edit_chord(
        &mut self,
        event_loop: &ActiveEventLoop,
        i: usize,
        letter: char,
        shift: bool,
    ) -> bool {
        let pane = &mut self.panes[i];
        // Whichever scope the window's selection is in — a `selectable`
        // node's, or a `cells` grid's, which is a scope too. Reading only
        // the text one left Cmd-C over a terminal doing nothing unless
        // some editor elsewhere happened to hold focus.
        let scope = pane
            .core
            .selection()
            .map(|s| s.scope)
            .or_else(|| pane.core.cell_selection().map(|s| s.node));
        if pane.core.edit.focused().is_none() && scope.is_none() {
            return false;
        }
        match letter {
            'c' => {
                // A selection that reaches rows a virtual list never
                // built is answered by the app, not by the core: the
                // ask goes out with the pending events and the answer
                // comes back as a clipboard action (ADR 0017, tier 3).
                match pane.core.request_copy() {
                    CopyRequest::Ready(text) => {
                        let html = pane.core.selection_html();
                        set_clipboard(self.clipboard.as_mut(), text, html);
                    }
                    CopyRequest::Asked => {
                        let events = self.panes[i].core.take_pending_events();
                        self.route_events(events);
                        self.apply_menu_actions(event_loop, i);
                    }
                    CopyRequest::Nothing => {}
                }
            }
            'x' => {
                if let Some(text) = pane.core.cut_selection() {
                    set_clipboard(self.clipboard.as_mut(), text, None);
                    self.after_direct_edit(i);
                }
            }
            'v' => {
                if let Some(text) = self.clipboard.as_mut().and_then(|cb| cb.get_text().ok()) {
                    self.dispatch(event_loop, i, InputEvent::Text(text));
                }
            }
            // Select All inside a selection scope stays in that scope;
            // with none, it is the editor's as before.
            'a' => match scope {
                Some(scope) => {
                    pane.core.select_all_in(scope);
                    pane.window.request_redraw();
                }
                None => {
                    self.dispatch(
                        event_loop,
                        i,
                        InputEvent::Key(EditKey::SelectAll, Mods::default()),
                    );
                }
            },
            'z' => {
                let key = if shift { EditKey::Redo } else { EditKey::Undo };
                self.dispatch(event_loop, i, InputEvent::Key(key, Mods::default()));
            }
            'y' => {
                self.dispatch(
                    event_loop,
                    i,
                    InputEvent::Key(EditKey::Redo, Mods::default()),
                );
            }
            _ => return false,
        }
        true
    }

    /// A row of the standard Edit menu was chosen (ADR 0030, decision 3):
    /// the chord it spells, replayed exactly as the keyboard would have
    /// sent it — the press to the key-focused sink, the runner's own half
    /// of the chord, the release — so an app that binds ⌘C itself hears
    /// the same thing from the menu, and an editor copies through the
    /// same code the key takes. AppKit consumed the key before winit saw
    /// it, which is why nothing arrives here twice.
    #[cfg(target_os = "macos")]
    pub(super) fn replay_edit_chord(
        &mut self,
        event_loop: &ActiveEventLoop,
        i: usize,
        chord: macos_menu::EditChord,
    ) {
        let mods = KeyMods {
            shift: chord.shift,
            super_key: true,
            ..KeyMods::default()
        };
        // What the keyboard would have carried: the layout's character
        // heeds Shift (⇧⌘Z arrives as `Z`, winit's `logical_key`), the
        // physical key is the letter either way.
        let code = if chord.shift {
            chord.letter.to_ascii_uppercase()
        } else {
            chord.letter
        };
        let kp =
            KeyPress::new(KeyCode::Char(code), mods).with_physical(KeyCode::Char(chord.letter));
        // Each step may close the window the next is for (backlog AR39):
        // re-found by id after every one.
        let here = self.panes[i].id;
        let Some(i) = self.dispatch(event_loop, i, InputEvent::KeyDown(kp.clone())) else {
            return;
        };
        self.edit_chord(event_loop, i, chord.letter, chord.shift);
        let Some(i) = self.pane_of(here) else { return };
        let Some(i) = self.dispatch(event_loop, i, InputEvent::KeyUp(kp.released())) else {
            return;
        };
        // `dispatch` owed the frame for whatever reached the app; what is
        // left is what a chosen declared row also does after its events.
        self.apply_menu_actions(event_loop, i);
        self.apply_window_commands(event_loop);
        if let Some(i) = self.pane_of(here) {
            self.panes[i].window.request_redraw();
        }
    }
}
