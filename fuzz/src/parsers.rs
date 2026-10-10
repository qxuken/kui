//! The string parsers every binding hands a prop's spelling to: size
//! expressions, accelerators, the name lists (`buttons`, `scrollMods`),
//! font features, key and role names, gradient sides.
//!
//! The first byte picks the parser, the rest is the string (invalid UTF-8
//! is skipped: every door hands these a `&str`). Beyond "no panic", each
//! checks the round trip its docs promise.

use kui_core::{
    Accel, AccessAction, Buttons, CursorShape, EditKey, FontFeatures, KeyCode, KeyMods, Role, Side,
    calc,
};

pub fn run(data: &[u8]) {
    let Some((&which, rest)) = data.split_first() else {
        return;
    };
    let Ok(s) = std::str::from_utf8(rest) else {
        return;
    };
    match which % 4 {
        0 => size(s),
        1 => accel(s),
        2 => names(s),
        _ => {
            let _ = FontFeatures::parse(s);
        }
    }
}

/// `calc::parse` and the two reductions that intern. An expression the
/// grammar accepts resolves to a number at any room, and its canonical
/// spelling (`Display`, what `Calc::describe` shows) is one the grammar
/// accepts again.
fn size(s: &str) {
    let Ok(e) = calc::parse(s) else {
        // The reductions refuse what the grammar refuses.
        assert!(calc::sizing(s).is_err(), "sizing accepted {s:?}");
        return;
    };
    for room in [0.0, 1.0, 640.0, 1.0e6] {
        let v = e.resolve(room);
        assert!(!v.is_nan() && v >= 0.0, "{s:?} at {room}: {v}");
    }
    let spelled = e.to_string();
    assert!(
        calc::parse(&spelled).is_ok(),
        "{s:?} spells as {spelled:?}, which does not parse"
    );
    // Interning is process-wide and capped; a full table is a refusal the
    // bindings expect, anything else is not.
    if let Err(err) = calc::sizing(s) {
        assert!(calc::is_full(&err), "{s:?} parses but sizing says {err}");
    }
    if let Err(err) = calc::bound(s) {
        assert!(calc::is_full(&err), "{s:?} parses but bound says {err}");
    }
}

/// `Accel::parse`: whatever parses reads back to the same chord from its
/// portable spelling and from its display (`declare_menu_bar` normalizes
/// a declaration into the display, and the platform bar parses that).
fn accel(s: &str) {
    let _ = Accel::label(s);
    let Some(a) = Accel::parse(s) else {
        return;
    };
    let spelled = a.spelling();
    assert_eq!(
        Accel::parse(&spelled),
        Some(a),
        "{s:?} spells as {spelled:?}"
    );
    // Play/Pause's pause key is written as Pause's, and reads back as
    // Pause: menu.rs's own round-trip test skips it for that reason.
    if a.code == KeyCode::MediaPause {
        return;
    }
    let shown = a.display();
    assert_eq!(Accel::parse(&shown), Some(a), "{s:?} displays as {shown:?}");
}

/// The name parsers. A name that parses is the name its value writes.
fn names(s: &str) {
    let _ = Buttons::parse(s);
    let _ = KeyMods::parse(s);
    let _ = Side::parse(s);
    let _ = kui_core::path::FillRule::parse(s);
    // Names are read leniently (`f01` is F1), so it is the code's own
    // name that must read back to it.
    if let Some(code) = KeyCode::from_name(s) {
        assert_eq!(
            KeyCode::from_name(&code.name()),
            Some(code),
            "{s:?} reads as {code:?}"
        );
    }
    if let Some(k) = EditKey::from_name(s) {
        assert_eq!(k.name(), s);
    }
    if let Some(r) = Role::parse(s) {
        assert_eq!(r.name(), s);
    }
    if let Some(a) = AccessAction::parse(s) {
        assert_eq!(a.name(), s);
    }
    if let Some(c) = CursorShape::parse(s) {
        assert_eq!(c.name(), s);
    }
}
