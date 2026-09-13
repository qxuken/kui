//! Tokens beside the theme (`docs/adr/0027-tokens-beside-the-theme.md`):
//! a table per origin, a colour with a light and a dark half picked by the
//! theme, a length in px, the roles reachable by the same spelling, and
//! the two warnings — a name nothing declared, and a declared name a role
//! owns.

use std::cell::RefCell;
use std::rc::Rc;

use kui_core::diag::{RESERVED_TOKEN, UNKNOWN_TOKEN};
use kui_core::testing::codes;
use kui_core::tokens::{COLOR_ROLES, LENGTH_ROLES};
use kui_core::{
    Appearance, Color, Core, Extension, Extensions, NodeSpec, Size, Sizing, Slot, SystemEnv,
    TokenError, TokenKind, TokenRef, Tokens, Ui, UiEvent, Value,
};

const PEACH: Color = Color {
    r: 1.0,
    g: 0.8,
    b: 0.6,
    a: 1.0,
};
const INK: Color = Color {
    r: 0.1,
    g: 0.1,
    b: 0.1,
    a: 1.0,
};
const PAPER: Color = Color {
    r: 0.95,
    g: 0.95,
    b: 0.95,
    a: 1.0,
};

fn host_tokens() -> Tokens {
    Tokens::new()
        .color("peach", PEACH)
        .color_themed("ink", INK, PAPER)
        .length("side_w", 132.0)
}

#[test]
fn a_name_resolves_to_its_value_and_a_role_to_the_themes() {
    let mut core = Core::new();
    core.set_tokens(host_tokens());
    let t = *core.theme();
    let m = *core.metrics();
    let look = core.token_lookup();
    assert_eq!(look.color("peach"), Ok(PEACH));
    assert_eq!(look.length("side_w"), Ok(132.0));
    // Roles under either spelling, and in front of the app's indices.
    assert_eq!(look.color("surface"), Ok(t.surface));
    assert_eq!(look.color("borderStrong"), Ok(t.border_strong));
    assert_eq!(look.length("control_pad_x"), Ok(m.control_pad_x));
    assert_eq!(look.resolve("surface"), Some(TokenRef::ColorRole(1)));
    assert_eq!(look.resolve("peach"), Some(TokenRef::Color(0)));
    assert_eq!(look.color_at(COLOR_ROLES as u32), Some(PEACH));
    assert_eq!(look.color_at(1), Some(t.surface));
    assert_eq!(look.length_at(LENGTH_ROLES as u32), Some(132.0));
    assert_eq!(look.length_at(LENGTH_ROLES as u32 + 1), None);
    // The wrong kind is its own error, so the message can say so.
    assert_eq!(
        look.color("side_w"),
        Err(TokenError::Kind {
            name: "side_w".into(),
            is: TokenKind::Length,
            wanted: TokenKind::Color,
        })
    );
    assert_eq!(
        look.length("nothing"),
        Err(TokenError::Unknown("nothing".into()))
    );
}

/// The dark half on the dark base and on an unknown appearance, the light
/// half on the light one — frame-stable through the theme it reads.
#[test]
fn a_themed_colour_follows_the_appearance() {
    let mut core = Core::new();
    core.set_tokens(host_tokens());
    // `ink` is dark text on a light base and paper-coloured on a dark one.
    assert_eq!(
        core.token_lookup().color("ink"),
        Ok(PAPER),
        "unknown is dark"
    );
    core.set_system(SystemEnv {
        appearance: Appearance::Light,
        ..Default::default()
    });
    assert_eq!(core.token_lookup().color("ink"), Ok(INK));
    assert_eq!(
        core.token_lookup().color("peach"),
        Ok(PEACH),
        "unthemed is both"
    );
    core.set_system(SystemEnv {
        appearance: Appearance::Dark,
        ..Default::default()
    });
    assert_eq!(core.token_lookup().color("ink"), Ok(PAPER));
}

/// `ui.token_color` on a name nothing declared warns once and paints
/// nothing; a declared role name warns at the declaration.
#[test]
fn the_two_warnings() {
    let mut core = Core::new();
    core.set_tokens(
        Tokens::new()
            .color("peach", PEACH)
            .color("surface", PEACH)
            .length("radius", 1.0),
    );
    let mut ui = core.frame(Size::new(100.0, 100.0), 1.0);
    // A miss is `None` — the view leaves the row at its default, as a
    // `$name` in a prop does in every binding (AR14) — never an explicit
    // transparent or zero that would hide the node the typo was on.
    let c = ui.token_color("peech");
    assert_eq!(c, None);
    assert_eq!(ui.token_color("peech"), None);
    assert_eq!(ui.token_length("peach"), None, "a colour in a length slot");
    assert_eq!(ui.token_color("peach"), Some(PEACH));
    ui.finish();
    let warnings = core.take_warnings();
    let raised = codes(&warnings);
    assert_eq!(
        raised.iter().filter(|c| **c == RESERVED_TOKEN).count(),
        2,
        "surface and radius: {raised:?}"
    );
    assert_eq!(
        raised.iter().filter(|c| **c == UNKNOWN_TOKEN).count(),
        2,
        "peech once (deduplicated), and peach-as-length once: {raised:?}"
    );
    let msg = &warnings
        .iter()
        .find(|w| w.code == UNKNOWN_TOKEN && w.message.contains("peech"))
        .expect("the unknown name")
        .message;
    assert!(msg.contains("names no token"), "{msg}");
    // The reserved names never entered the table.
    assert_eq!(
        core.token_lookup().resolve("surface"),
        Some(TokenRef::ColorRole(1))
    );
}

/// An extension that declares a table of its own, reads the host's
/// through it, and records what it saw.
struct Guest {
    declare: Option<Tokens>,
    seen: Rc<RefCell<Vec<(String, Color)>>>,
}

impl Extension for Guest {
    fn name(&self) -> &str {
        "guest"
    }
    fn slots(&self) -> &[String] {
        &[]
    }
    fn view(&mut self, _slot: &Slot<'_>, ui: &mut Ui<'_>) -> Result<(), String> {
        if let Some(t) = self.declare.take() {
            ui.set_tokens(t);
        }
        for name in ["peach", "grey", "side_w"] {
            let c = match ui.tokens().color(name) {
                Ok(c) => c,
                Err(_) => Color::TRANSPARENT,
            };
            self.seen.borrow_mut().push((name.to_string(), c));
        }
        ui.with(
            NodeSpec::column()
                .width(Sizing::Fixed(10.0))
                .height(Sizing::Fixed(10.0)),
            |_| {},
        );
        Ok(())
    }
    fn on_event(&mut self, _ev: &UiEvent) -> Vec<Value> {
        Vec::new()
    }
}

/// A guest reads the host's `peach` until it declares its own; its `grey`
/// is its own and never the host's; and its declaration leaves the host's
/// table exactly as it was.
/// AR14: a `$name` in a keyframe stop or an entrance is a token like any
/// other — resolved through the same lookup, and a miss leaves that slot
/// unnamed and is remembered for the binding to raise, where before it
/// was an error that failed the whole frame.
#[test]
fn a_stop_and_an_entrance_resolve_tokens_and_miss_by_leaving_the_slot() {
    use kui_core::{NameRefs, Sizing, enter, keyframes};
    let mut core = Core::new();
    core.set_tokens(host_tokens());
    let mut refs = NameRefs::new(core.token_lookup());
    let stops = Value::List(vec![
        Value::map([
            ("bg", Value::str("$peach")),
            ("width", Value::str("$side_w")),
        ]),
        Value::map([
            ("bg", Value::str("$peech")),
            ("radius", Value::str("$side_w")),
        ]),
        Value::map([("height", Value::str("$peach"))]),
    ]);
    let frames = keyframes::parse_with(&stops, Some(&mut refs)).expect("a miss is not an error");
    assert_eq!(frames.len(), 3);
    assert_eq!(frames[0].slots.bg, Some(PEACH));
    assert_eq!(frames[0].slots.width, Some(Sizing::Fixed(132.0)));
    assert_eq!(frames[1].slots.bg, None, "the miss leaves the slot unnamed");
    assert_eq!(frames[1].slots.radius, Some(132.0));
    assert_eq!(frames[2].slots.height, None, "a colour in a length slot");
    let e = enter::parse_with(
        &Value::map([
            ("bg", Value::str("$ink")),
            ("dx", Value::Float(4.0)),
            ("width", Value::str("$nothing")),
        ]),
        Some(&mut refs),
    )
    .unwrap();
    assert_eq!(e.dx, 4.0);
    assert_eq!(
        e.slots.bg,
        Some(PAPER),
        "the dark half under the default appearance"
    );
    assert_eq!(e.slots.width, None);
    let missed = refs.take_missed();
    assert_eq!(missed.len(), 3, "{missed:?}");
    assert!(matches!(&missed[0], TokenError::Unknown(n) if n == "peech"));
    assert!(matches!(&missed[1], TokenError::Kind { name, .. } if name == "peach"));
    assert!(matches!(&missed[2], TokenError::Unknown(n) if n == "nothing"));
    // Without a lookup a `$name` is what it always was: not a colour.
    assert!(keyframes::parse(&stops).is_err());
    // And a plain value beside a reference still parses as itself.
    let plain = keyframes::parse_with(
        &Value::List(vec![Value::map([
            ("bg", Value::Int(0x11223344)),
            ("opacity", Value::Float(0.5)),
        ])]),
        Some(&mut refs),
    )
    .unwrap();
    assert_eq!(plain[0].slots.opacity, Some(0.5));
    assert!(refs.take_missed().is_empty());
}

#[test]
fn a_guest_reads_its_own_table_over_the_hosts() {
    let seen = Rc::new(RefCell::new(Vec::new()));
    let grey = Color::rgb8(0x80, 0x80, 0x80);
    let own_peach = Color::rgb8(0xff, 0xaa, 0x77);
    let guest = Guest {
        declare: Some(Tokens::new().color("grey", grey).color("peach", own_peach)),
        seen: seen.clone(),
    };
    let mut exts = Extensions::try_from(vec![Box::new(guest) as Box<dyn Extension>]).unwrap();
    let mut core = Core::new();
    core.set_tokens(host_tokens());

    // Frame 1: the guest declares during its view, so the reads after the
    // declaration see its own peach.
    let mut ui = core.frame_with(Size::new(200.0, 100.0), 1.0, &mut exts);
    ui.configure_root(NodeSpec::row().fill());
    ui.finish();
    let got = seen.borrow().clone();
    assert_eq!(
        got,
        vec![
            ("peach".to_string(), own_peach),
            ("grey".to_string(), grey),
            ("side_w".to_string(), Color::TRANSPARENT),
        ]
    );
    // The host's table did not move.
    let look = core.token_lookup();
    assert_eq!(look.color("peach"), Ok(PEACH));
    assert!(
        look.color("grey").is_err(),
        "the guest's grey is not the host's"
    );
    assert_eq!(look.length("side_w"), Ok(132.0));
    assert!(core.take_warnings().is_empty());
}

/// A guest that declares nothing reads the host's names through the
/// fallback — the merged readback lists them under the guest too.
#[test]
fn a_guest_with_no_table_reads_the_hosts() {
    let seen = Rc::new(RefCell::new(Vec::new()));
    let guest = Guest {
        declare: None,
        seen: seen.clone(),
    };
    let mut exts = Extensions::try_from(vec![Box::new(guest) as Box<dyn Extension>]).unwrap();
    let mut core = Core::new();
    core.set_tokens(host_tokens());
    let mut ui = core.frame_with(Size::new(200.0, 100.0), 1.0, &mut exts);
    ui.configure_root(NodeSpec::row().fill());
    ui.finish();
    assert_eq!(seen.borrow()[0], ("peach".to_string(), PEACH));
}

/// The readback merges own over host and names a painted value.
#[test]
fn readback_and_reverse_lookup() {
    let mut core = Core::new();
    core.set_tokens(
        Tokens::new()
            .color("peach", PEACH)
            .color("also_peach", PEACH)
            .length("gap", 6.0),
    );
    let look = core.token_lookup();
    assert_eq!(look.colors(), vec![("peach", PEACH), ("also_peach", PEACH)]);
    assert_eq!(look.lengths(), vec![("gap", 6.0)]);
    assert_eq!(look.color_names(PEACH), vec!["peach", "also_peach"]);
    assert_eq!(look.length_names(6.0), vec!["gap"]);
    assert!(look.length_names(7.0).is_empty());
}

/// Declaring again replaces the table whole: a name left out is gone.
#[test]
fn a_declaration_replaces_the_table() {
    let mut core = Core::new();
    core.set_tokens(host_tokens());
    core.set_tokens(Tokens::new().length("side_w", 78.0));
    let look = core.token_lookup();
    assert_eq!(look.length("side_w"), Ok(78.0));
    assert!(look.color("peach").is_err());
}

// -- derived tokens (`docs/adr/0028-derived-tokens.md`) ---------------------

/// The corpus table's derived tokens, through the core: a chain over a
/// value, a `raise` off a role that turns with the base, a step off a
/// derived token, and a `readable` that moves only where it has to.
#[test]
fn a_derived_token_follows_its_source_through_the_flip() {
    use kui_core::ColorOp;
    let table = || {
        Tokens::new()
            .color("peach", PEACH)
            .color_themed("ink", INK, PAPER)
            .derive("lit", "peach", [ColorOp::Lift(0.3)])
            .derive("up", "surface", [ColorOp::Raise(0.25)])
            .derive("deep", "lit", [ColorOp::Alpha(0.5)])
            .derive("read", "peach", [ColorOp::Readable("ink".into(), 4.5)])
    };
    // A derived colour is rounded to eight bits a channel (what a C host
    // reads back), so expectations are built the same way.
    let q = |c: Color| Color::hex(c.to_hex());
    let mut core = Core::new();
    core.set_tokens(table());
    let dark = *core.theme();
    let look = core.token_lookup();
    let lit = q(PEACH.mix(Color::WHITE, 0.3));
    assert_eq!(look.color("lit"), Ok(lit));
    assert_eq!(
        look.color("up"),
        Ok(q(dark.surface.mix(Color::WHITE, 0.25)))
    );
    assert_eq!(look.color("deep"), Ok(q(lit.with_alpha(0.5))));
    let read = look.color("read").unwrap();
    assert!(
        read.contrast(PAPER) >= 4.5,
        "moved toward black to read on paper"
    );
    assert_ne!(read, PEACH);
    assert_eq!(
        look.resolve("read"),
        Some(TokenRef::Color(5)),
        "declaration order"
    );

    core.set_system(SystemEnv {
        appearance: Appearance::Light,
        ..Default::default()
    });
    let light = *core.theme();
    let look = core.token_lookup();
    assert_eq!(
        look.color("up"),
        Ok(q(light.surface.mix(Color::BLACK, 0.25))),
        "raise turns"
    );
    assert_eq!(look.color("read"), Ok(PEACH), "peach already reads on ink");
    // The reverse lookup names a derived value like any other.
    assert_eq!(look.color_names(lit), vec!["lit"]);
}

/// A derived token whose source is nothing is dropped at the declaration
/// with `unknown-token` — keyed by the derived name, so a later `$bad`
/// reference adds no second warning — and the table keeps its other
/// entries at their indices.
#[test]
fn an_unresolved_source_warns_at_declaration() {
    use kui_core::ColorOp;
    let mut core = Core::new();
    core.set_tokens(
        Tokens::new()
            .color("peach", PEACH)
            .derive("bad", "nothing", [ColorOp::Lift(0.1)])
            .derive("lit", "peach", [ColorOp::Lift(0.3)]),
    );
    assert_eq!(core.token_lookup().resolve("lit"), Some(TokenRef::Color(1)));
    assert_eq!(
        core.token_lookup().color("bad"),
        Err(TokenError::Unknown("bad".into()))
    );
    core.warn_unknown_token(&TokenError::Unknown("bad".into()));
    let warnings = core.take_warnings();
    let unknown: Vec<_> = warnings
        .iter()
        .filter(|w| w.code == UNKNOWN_TOKEN)
        .collect();
    assert_eq!(unknown.len(), 1, "{warnings:?}");
    assert!(unknown[0].message.contains("`$bad`") && unknown[0].message.contains("`$nothing`"));
}
