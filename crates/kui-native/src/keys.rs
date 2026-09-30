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
/// The numeric keypad reports the digit and operator it always bears, as
/// winit's logical key for `Numpad1` is `"1"`: which of the two it was is
/// the press's [`kui_core::KeyLocation`] (backlog F108), not its code.
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
        Phys::F13 => KeyCode::F(13),
        Phys::F14 => KeyCode::F(14),
        Phys::F15 => KeyCode::F(15),
        Phys::F16 => KeyCode::F(16),
        Phys::F17 => KeyCode::F(17),
        Phys::F18 => KeyCode::F(18),
        Phys::F19 => KeyCode::F(19),
        Phys::F20 => KeyCode::F(20),
        Phys::F21 => KeyCode::F(21),
        Phys::F22 => KeyCode::F(22),
        Phys::F23 => KeyCode::F(23),
        Phys::F24 => KeyCode::F(24),
        Phys::F25 => KeyCode::F(25),
        Phys::F26 => KeyCode::F(26),
        Phys::F27 => KeyCode::F(27),
        Phys::F28 => KeyCode::F(28),
        Phys::F29 => KeyCode::F(29),
        Phys::F30 => KeyCode::F(30),
        Phys::F31 => KeyCode::F(31),
        Phys::F32 => KeyCode::F(32),
        Phys::F33 => KeyCode::F(33),
        Phys::F34 => KeyCode::F(34),
        Phys::F35 => KeyCode::F(35),
        Phys::PrintScreen => KeyCode::PrintScreen,
        Phys::Pause => KeyCode::Pause,
        Phys::ContextMenu => KeyCode::Menu,
        Phys::NumpadClear => KeyCode::Clear,
        Phys::ShiftLeft | Phys::ShiftRight => KeyCode::Shift,
        Phys::ControlLeft | Phys::ControlRight => KeyCode::Ctrl,
        Phys::AltLeft | Phys::AltRight => KeyCode::Alt,
        Phys::SuperLeft | Phys::SuperRight => KeyCode::Super,
        Phys::CapsLock => KeyCode::CapsLock,
        Phys::NumLock => KeyCode::NumLock,
        Phys::ScrollLock => KeyCode::ScrollLock,
        Phys::MediaPlayPause => KeyCode::MediaPlayPause,
        Phys::MediaStop => KeyCode::MediaStop,
        Phys::MediaTrackNext => KeyCode::MediaNext,
        Phys::MediaTrackPrevious => KeyCode::MediaPrev,
        Phys::AudioVolumeUp => KeyCode::VolumeUp,
        Phys::AudioVolumeDown => KeyCode::VolumeDown,
        Phys::AudioVolumeMute => KeyCode::VolumeMute,
        _ => KeyCode::Unknown,
    }
}

/// The named keys past the editing block, as winit's logical key names
/// them (backlog F108): F13–F35, the system keys, the modifier and lock
/// keys themselves, and the media keys.
fn named_code(n: &NamedKey) -> KeyCode {
    let f = [
        NamedKey::F13,
        NamedKey::F14,
        NamedKey::F15,
        NamedKey::F16,
        NamedKey::F17,
        NamedKey::F18,
        NamedKey::F19,
        NamedKey::F20,
        NamedKey::F21,
        NamedKey::F22,
        NamedKey::F23,
        NamedKey::F24,
        NamedKey::F25,
        NamedKey::F26,
        NamedKey::F27,
        NamedKey::F28,
        NamedKey::F29,
        NamedKey::F30,
        NamedKey::F31,
        NamedKey::F32,
        NamedKey::F33,
        NamedKey::F34,
        NamedKey::F35,
    ];
    if let Some(i) = f.iter().position(|k| k == n) {
        return KeyCode::F(13 + i as u8);
    }
    match n {
        NamedKey::PrintScreen => KeyCode::PrintScreen,
        NamedKey::Pause => KeyCode::Pause,
        NamedKey::ContextMenu => KeyCode::Menu,
        NamedKey::Clear => KeyCode::Clear,
        NamedKey::Shift => KeyCode::Shift,
        NamedKey::Control => KeyCode::Ctrl,
        // AltGr is the right Alt on the keyboards that have it: named as
        // itself it was `unknown`, reported as Alt only through the
        // physical fallback and never recorded as held (backlog RG96).
        NamedKey::Alt | NamedKey::AltGraph => KeyCode::Alt,
        NamedKey::Super | NamedKey::Meta => KeyCode::Super,
        NamedKey::CapsLock => KeyCode::CapsLock,
        NamedKey::NumLock => KeyCode::NumLock,
        NamedKey::ScrollLock => KeyCode::ScrollLock,
        NamedKey::MediaPlay => KeyCode::MediaPlay,
        NamedKey::MediaPause => KeyCode::MediaPause,
        NamedKey::MediaPlayPause => KeyCode::MediaPlayPause,
        NamedKey::MediaStop => KeyCode::MediaStop,
        NamedKey::MediaTrackNext => KeyCode::MediaNext,
        NamedKey::MediaTrackPrevious => KeyCode::MediaPrev,
        NamedKey::MediaRecord => KeyCode::MediaRecord,
        NamedKey::MediaFastForward => KeyCode::MediaFastForward,
        NamedKey::MediaRewind => KeyCode::MediaRewind,
        NamedKey::AudioVolumeUp => KeyCode::VolumeUp,
        NamedKey::AudioVolumeDown => KeyCode::VolumeDown,
        NamedKey::AudioVolumeMute => KeyCode::VolumeMute,
        _ => KeyCode::Unknown,
    }
}

/// A modifier key held: where it is (`physical_code` and its side) and
/// what the layout says it means.
pub(crate) type HeldModifier = (KeyCode, KeyLocation, KeyCode);

/// `mods` with a modifier key's own bit set to the state after its
/// event (backlog F108): on while it or its twin is `down`, which this
/// keeps. Any other key passes `mods` through.
///
/// `code` is what the layout says the key is, and its bit is the one
/// set; `at` is where the key is, and the record goes by that. A press
/// and its release can disagree on the first — X11 may read the left Alt
/// pressed after Shift as `Meta_L` and its release as `Alt_L` — and a
/// record by meaning kept Super held for good (backlog RG96). Going by
/// the meaning and not the place for the bit is what keeps a key the
/// layout remapped (Caps Lock as Ctrl) the modifier it acts as.
pub(crate) fn modifier_after(
    down: &mut Vec<HeldModifier>,
    code: KeyCode,
    at: KeyCode,
    location: KeyLocation,
    pressed: bool,
    mut mods: KeyMods,
) -> KeyMods {
    if !holds_as_modifier(code) {
        return mods;
    }
    down.retain(|&(a, l, _)| (a, l) != (at, location));
    if pressed {
        down.push((at, location, code));
    }
    let on = down.iter().any(|&(_, _, c)| c == code);
    match code {
        KeyCode::Shift => mods.shift = on,
        KeyCode::Ctrl => mods.ctrl = on,
        KeyCode::Alt => mods.alt = on,
        _ => mods.super_key = on,
    }
    mods
}

/// Which of a key's twins winit says this is.
fn location_of(l: winit::keyboard::KeyLocation) -> KeyLocation {
    use winit::keyboard::KeyLocation as L;
    match l {
        L::Standard => KeyLocation::Standard,
        L::Left => KeyLocation::Left,
        L::Right => KeyLocation::Right,
        L::Numpad => KeyLocation::Numpad,
    }
}

/// Whether `code` is one of the four modifiers a held record keeps.
fn holds_as_modifier(code: KeyCode) -> bool {
    matches!(
        code,
        KeyCode::Shift | KeyCode::Ctrl | KeyCode::Alt | KeyCode::Super
    )
}

/// What a key means for the modifier record: the layout's reading, or
/// where the key is when that reading has no name. X11 reads the left
/// Alt pressed after Shift as `Meta_L`, which winit leaves unnamed, and
/// its own press carried the state before it (backlog RG101).
fn meaning_of(logical: KeyCode, physical: KeyCode) -> KeyCode {
    if logical == KeyCode::Unknown {
        physical
    } else {
        logical
    }
}

/// Which of its twins a modifier key is: winit's word, or the side the
/// key is on when winit says `Standard`. winit reads a side off the
/// keysym, and X11's AltGr (`ISO_Level3_Shift`) names none, so the right
/// Alt was reported as neither twin (backlog RG101).
fn side_of(
    key: winit::keyboard::PhysicalKey,
    meaning: KeyCode,
    reported: KeyLocation,
) -> KeyLocation {
    use winit::keyboard::{KeyCode as Phys, PhysicalKey};
    if reported != KeyLocation::Standard || !holds_as_modifier(meaning) {
        return reported;
    }
    match key {
        PhysicalKey::Code(
            Phys::ShiftLeft | Phys::ControlLeft | Phys::AltLeft | Phys::SuperLeft,
        ) => KeyLocation::Left,
        PhysicalKey::Code(
            Phys::ShiftRight | Phys::ControlRight | Phys::AltRight | Phys::SuperRight,
        ) => KeyLocation::Right,
        _ => reported,
    }
}

/// Caps Lock and Num Lock at a press (backlog F108). winit reports
/// neither, so the OS is asked where it answers cheaply — macOS's
/// `NSEvent.modifierFlags` (a Mac has no Num Lock, so it reads off, as a
/// Mac terminal reports it), Windows' `GetKeyState` — and anywhere
/// else the state is `tracked` from the lock keys' own presses, which
/// knows nothing of a lock set before the app's first window opened.
pub(crate) fn lock_state(tracked: KeyLocks) -> KeyLocks {
    #[cfg(target_os = "macos")]
    {
        use objc2_app_kit::{NSEvent, NSEventModifierFlags};
        let flags = NSEvent::modifierFlags_class();
        let _ = tracked;
        KeyLocks {
            caps: flags.contains(NSEventModifierFlags::CapsLock),
            num: false,
        }
    }
    #[cfg(target_os = "windows")]
    {
        use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
            GetKeyState, VK_CAPITAL, VK_NUMLOCK,
        };
        let _ = tracked;
        // SAFETY: GetKeyState reads the calling thread's key state and
        // takes a virtual-key code; any value is sound.
        let on = |vk: u16| unsafe { GetKeyState(vk as i32) } & 1 != 0;
        KeyLocks {
            caps: on(VK_CAPITAL),
            num: on(VK_NUMLOCK),
        }
    }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        tracked
    }
}

/// `tracked` after a key: a lock key's press turns its lock.
pub(crate) fn locks_after(mut tracked: KeyLocks, code: KeyCode, press: bool) -> KeyLocks {
    if press {
        match code {
            KeyCode::CapsLock => tracked.caps = !tracked.caps,
            KeyCode::NumLock => tracked.num = !tracked.num,
            _ => {}
        }
    }
    tracked
}

impl DynShell<'_> {
    /// A key event for pane `i`, which the OS delivered to pane `from` —
    /// the same pane, or the owner lending its keyboard to a popup.
    pub(super) fn on_key(
        &mut self,
        event_loop: &ActiveEventLoop,
        from: usize,
        i: usize,
        event: winit::event::KeyEvent,
    ) {
        let pressed = event.state == ElementState::Pressed;
        // Whether an Option the window made Alt is held (backlog F113):
        // `from`'s setting and `from`'s reading of which Options are down,
        // since winit rewrote the press in that window's view by the
        // event's own flags. Not the held-key record below: a modifier
        // pressed in the owner and let go in a popup it lent the keyboard
        // to, or held while the window was away, left that record wrong,
        // and a record that says Option is down swallows every key typed
        // after it (backlog RG83).
        let option_alt = cfg!(target_os = "macos")
            && crate::pane::option_is_alt(
                self.panes[from].applied_option_as_alt,
                self.panes[from].alt_held,
            );

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
                    other => named_code(other),
                },
                (plain && *n == NamedKey::Space).then(|| " ".to_string()),
            ),
            _ => (KeyCode::Unknown, None),
        };
        // `KeyPress::from_layout` resolves the two into the code a keymap
        // binds against — the layout's key while it speaks ASCII, the
        // US-QWERTY key at that position as Shift prints it when it does
        // not, unshifted under Alt (the branch above stripped Shift from
        // the logical key too, so ⌥⇧ on the key printed J is `j` on every
        // layout). Every driver goes through it, so a C or Node host with
        // its own windowing gets the same rule as this one. `ktext` is the
        // layout's own character, never the stand-in.
        // A modifier key's own press and release carry the state after
        // it (backlog F108): its bit on as it goes down, off as it comes
        // up unless its twin is still held — what a terminal speaking
        // kitty's protocol reports. winit's `ModifiersChanged` arrives
        // after the key, so the mirrored state is the one before it.
        // What the key means to the modifier record, and which twin it is
        // — each from where the key is when the layout's reading has no
        // answer (backlog RG101).
        let meaning = meaning_of(logical_code, physical);
        let location = side_of(event.physical_key, meaning, location_of(event.location));
        // Rebound after each dispatch: a chord the app answers by closing
        // a window moves every pane behind it down one (backlog AR39).
        let mut i = i;
        // Recorded on `from`, the window the OS holds the keyboard for:
        // a popup borrowing it is not a keyboard of its own, and a side
        // pressed before it opened comes up while it is the target
        // (backlog RG83); `from` also forgets them all as it loses focus.
        let kmods = self.panes[from].modifier_key(meaning, physical, location, pressed, kmods);
        let kp = KeyPress::from_layout(logical_code, physical, kmods);
        // The lock keys' own presses turn what is tracked where the OS
        // is not asked (`lock_state`), before the press reads it: Caps
        // Lock's own press says the state it made, as macOS's flags and
        // Windows' `GetKeyState` answer it, where it said what it found
        // (backlog RG96). One keyboard, so one record for the app: a
        // popup reads what its owner toggled.
        self.locks = locks_after(self.locks, kp.code, pressed && !event.repeat);
        let found = lock_state(self.locks);
        let kp = KeyPress {
            text: ktext,
            repeat: event.repeat,
            location,
            locks: found,
            ..kp
        };
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
        //
        // Nor under an Option the window made Alt (backlog F113): winit
        // hands such a press the layout's unmodified character, which
        // is the chord's and not something typed — Alt types nothing
        // there, as Control types nothing anywhere.
        let pane = &self.panes[i];
        if !pane.primary()
            && !pane.modifiers.control_key()
            && !option_alt
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
                // Only a focused editor takes the runner's paste: with none,
                // the window's selection scope holds nothing to paste into,
                // and a key sink has already heard the raw ⌘V and pastes by
                // its own binding (`request_paste`) — pasting for it too
                // pasted twice. Asked rather than read, so the answer is
                // the `Paste` a menu's Paste row gets, with the
                // pasteboard's markers (backlog F84); a bare `Text` here
                // dropped them, and with no editor it went nowhere, or into
                // a focused list's type-ahead (RG37).
                if pane.core.edit.focused().is_some() {
                    pane.core.request_paste();
                    self.apply_menu_actions(event_loop, i);
                }
            }
            // Select All inside a selection scope stays in that scope;
            // with none, it is the editor's as before.
            'a' => match scope {
                Some(scope) => {
                    pane.core.select_all_in(scope);
                    pane.redraw_for(FrameCause::KEY);
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
            self.panes[i].redraw_for(FrameCause::KEY);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use winit::keyboard::{KeyCode as Phys, KeyLocation as L, PhysicalKey};

    /// The keys backlog F108 named, as winit reports them: the modifier
    /// and lock keys, F13 onward, the system and media keys — by position
    /// and by the layout's name alike — and each place a key can be.
    #[test]
    fn the_whole_keyboard_has_a_name() {
        for (p, k) in [
            (Phys::ShiftLeft, KeyCode::Shift),
            (Phys::ShiftRight, KeyCode::Shift),
            (Phys::ControlRight, KeyCode::Ctrl),
            (Phys::AltLeft, KeyCode::Alt),
            (Phys::SuperRight, KeyCode::Super),
            (Phys::CapsLock, KeyCode::CapsLock),
            (Phys::NumLock, KeyCode::NumLock),
            (Phys::F13, KeyCode::F(13)),
            (Phys::F35, KeyCode::F(35)),
            (Phys::PrintScreen, KeyCode::PrintScreen),
            (Phys::ContextMenu, KeyCode::Menu),
            (Phys::NumpadClear, KeyCode::Clear),
            (Phys::MediaPlayPause, KeyCode::MediaPlayPause),
            (Phys::AudioVolumeMute, KeyCode::VolumeMute),
            (Phys::Numpad1, KeyCode::Char('1')),
            (Phys::NumpadEnter, KeyCode::Enter),
        ] {
            assert_eq!(physical_code(PhysicalKey::Code(p)), k, "{p:?}");
        }
        for (n, k) in [
            (NamedKey::Shift, KeyCode::Shift),
            (NamedKey::Control, KeyCode::Ctrl),
            (NamedKey::Super, KeyCode::Super),
            (NamedKey::ScrollLock, KeyCode::ScrollLock),
            (NamedKey::F24, KeyCode::F(24)),
            (NamedKey::F35, KeyCode::F(35)),
            (NamedKey::Pause, KeyCode::Pause),
            (NamedKey::Clear, KeyCode::Clear),
            (NamedKey::MediaTrackNext, KeyCode::MediaNext),
            (NamedKey::AudioVolumeUp, KeyCode::VolumeUp),
        ] {
            assert_eq!(named_code(&n), k, "{n:?}");
        }
        assert_eq!(location_of(L::Numpad), KeyLocation::Numpad);
        assert_eq!(location_of(L::Left), KeyLocation::Left);
        assert_eq!(location_of(L::Right), KeyLocation::Right);
        assert_eq!(location_of(L::Standard), KeyLocation::Standard);
        assert_eq!(named_code(&NamedKey::AltGraph), KeyCode::Alt, "RG96");
    }

    /// The record goes by where a key is, the bit by what it means
    /// (backlog RG96): X11's left Alt read as `Meta_L` down and `Alt_L`
    /// up leaves nothing held, and Caps Lock remapped to Ctrl is a Ctrl
    /// held and let go.
    #[test]
    fn a_held_modifier_is_recorded_by_where_it_is() {
        let mut down: Vec<HeldModifier> = Vec::new();
        let none = KeyMods::NONE;
        let alt = KeyCode::Alt;
        let meant_super = modifier_after(
            &mut down,
            KeyCode::Super,
            alt,
            KeyLocation::Left,
            true,
            none,
        );
        assert!(meant_super.super_key);
        modifier_after(&mut down, alt, alt, KeyLocation::Left, false, none);
        assert!(down.is_empty(), "no Super left held: {down:?}");
        let caps = KeyCode::CapsLock;
        let ctrl = modifier_after(
            &mut down,
            KeyCode::Ctrl,
            caps,
            KeyLocation::Standard,
            true,
            none,
        );
        assert!(ctrl.ctrl);
        let up = modifier_after(
            &mut down,
            KeyCode::Ctrl,
            caps,
            KeyLocation::Standard,
            false,
            none,
        );
        assert!(!up.ctrl);
        assert!(down.is_empty());
    }

    /// X11's readings a window found (backlog RG101): ⇧ then the left
    /// Alt is `Meta_L`, which winit leaves unnamed, so the key means what
    /// it is; AltGr (`ISO_Level3_Shift`) names no side, so the right Alt
    /// is on the right. A key the layout remapped keeps its reading and
    /// its side, and a key that is no modifier keeps `Standard`.
    #[test]
    fn a_modifier_the_layout_does_not_name_is_where_it_is() {
        let alt_left = PhysicalKey::Code(Phys::AltLeft);
        let alt_right = PhysicalKey::Code(Phys::AltRight);
        assert_eq!(meaning_of(KeyCode::Unknown, KeyCode::Alt), KeyCode::Alt);
        assert_eq!(meaning_of(KeyCode::Ctrl, KeyCode::CapsLock), KeyCode::Ctrl);
        let mut down: Vec<HeldModifier> = Vec::new();
        let press = modifier_after(
            &mut down,
            meaning_of(KeyCode::Unknown, KeyCode::Alt),
            KeyCode::Alt,
            side_of(alt_left, KeyCode::Alt, KeyLocation::Left),
            true,
            KeyMods::NONE.with_shift(),
        );
        assert!(press.alt, "the press carries the state after it");
        assert_eq!(
            side_of(alt_right, KeyCode::Alt, KeyLocation::Standard),
            KeyLocation::Right
        );
        assert_eq!(
            side_of(
                PhysicalKey::Code(Phys::CapsLock),
                KeyCode::Ctrl,
                KeyLocation::Left
            ),
            KeyLocation::Left
        );
        assert_eq!(
            side_of(alt_right, KeyCode::Char('@'), KeyLocation::Standard),
            KeyLocation::Standard,
            "compose on the right Alt is not a modifier"
        );
    }

    /// Caps Lock's own press reports the lock it made, as the OS answers
    /// it on macOS and Windows (backlog RG96); its release and a repeat
    /// turn nothing.
    #[test]
    fn a_lock_keys_press_reports_the_state_it_made() {
        let off = KeyLocks::default();
        let on = locks_after(off, KeyCode::CapsLock, true);
        assert!(on.caps && !on.num);
        assert_eq!(locks_after(on, KeyCode::CapsLock, false), on);
        assert_eq!(locks_after(on, KeyCode::Char('a'), true), on);
        assert!(locks_after(off, KeyCode::NumLock, true).num);
        assert_eq!(locks_after(on, KeyCode::CapsLock, true), off);
    }

    /// A modifier key's own event carries the state after it — Shift's
    /// press its Shift, its release none, unless the other Shift is still
    /// down — whatever order winit reported the modifiers in.
    #[test]
    fn a_modifier_keys_event_carries_the_state_after_it() {
        let mut down: Vec<HeldModifier> = Vec::new();
        let mut step = |code, at, pressed: bool, mods: KeyMods| {
            modifier_after(&mut down, code, code, at, pressed, mods)
        };
        let none = KeyMods::NONE;
        assert!(step(KeyCode::Shift, KeyLocation::Left, true, none).shift);
        assert!(step(KeyCode::Shift, KeyLocation::Right, true, none.with_shift()).shift);
        assert!(
            step(KeyCode::Shift, KeyLocation::Left, false, none.with_shift()).shift,
            "the right one still down"
        );
        assert!(!step(KeyCode::Shift, KeyLocation::Right, false, none.with_shift()).shift);
        // Any other key passes through.
        assert!(
            step(
                KeyCode::Char('a'),
                KeyLocation::Standard,
                true,
                none.with_ctrl()
            )
            .ctrl
        );
    }
}
