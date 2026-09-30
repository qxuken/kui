//! The size-expression table full (backlog RG93): the table is the
//! process's, so this binary is its own — every test in it would share a
//! full table, and no other test must.
//!
//! Past [`calc::MAX_CALCS`] a new expression is refused as full, not bad
//! ([`calc::is_full`]); a keyframe or entrance slot holding one is left
//! unset rather than failing the stops; one expression already kept still
//! resolves; and a core says so once, as `size-expressions-full`.

use kui_core::calc::{self, Expr};
use kui_core::value::Value;
use kui_core::{Core, Size, diag};

#[test]
fn past_the_cap_an_expression_is_refused_as_full_and_warned_once() {
    // A splitter drag's worth of fractional px, one entry each.
    let drag = |i: usize| Expr::Max(vec![Expr::Px(i as f32 + 0.5), Expr::Pct(0.5)]);
    let first = calc::intern(drag(0)).unwrap();
    let mut filled = 1;
    let refused = loop {
        match calc::intern(drag(filled)) {
            Ok(_) => filled += 1,
            Err(e) => break e,
        }
    };
    assert_eq!(
        filled,
        calc::MAX_CALCS,
        "no other expression in this process"
    );
    assert!(calc::is_full(&refused), "{refused}");
    assert!(refused.contains("\"max(65536.5px, 50%)\""), "{refused}");

    // Kept ones resolve; a new one in any spelling is full, not bad.
    assert_eq!(first.resolve(1000.0), 500.0);
    let e = calc::sizing("clamp(1px, 50%, 2px)").unwrap_err();
    assert!(calc::is_full(&e), "{e}");
    let e = kui_core::schema::max_str("min(3px, 50%)").unwrap_err();
    assert!(calc::is_full(&e), "a binding's wording around it: {e}");
    assert!(!calc::is_full(&calc::sizing("clamp(1px)").unwrap_err()));
    // A kept spelling is found as before.
    assert!(calc::sizing("max(0.5px, 50%)").is_ok());
    assert_eq!(calc::refused().0, 3);

    // A keyframe's width is left unset; the stop's other slots stand.
    let stops = kui_core::keyframes::parse(&Value::List(vec![Value::map([
        ("width", Value::str("min(9px, 90%)")),
        ("opacity", Value::Float(0.5)),
    ])]))
    .unwrap();
    assert_eq!(stops[0].slots.width, None);
    assert_eq!(stops[0].slots.opacity, Some(0.5));

    // Said once per core, naming the last refused.
    let mut core = Core::new();
    let ui = core.frame(Size::new(100.0, 100.0), 1.0);
    ui.finish();
    let full: Vec<_> = core
        .take_warnings()
        .into_iter()
        .filter(|w| w.code == diag::SIZE_EXPRESSIONS_FULL)
        .collect();
    assert_eq!(full.len(), 1);
    assert!(
        full[0].message.contains("\"min(9px, 90%)\""),
        "{}",
        full[0].message
    );
    let ui = core.frame(Size::new(100.0, 100.0), 1.0);
    ui.finish();
    assert!(core.take_warnings().is_empty());
}
