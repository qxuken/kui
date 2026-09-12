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
    let c = ui.token_color("peech");
    assert_eq!(c, Color::TRANSPARENT);
    assert_eq!(ui.token_color("peech"), Color::TRANSPARENT);
    assert_eq!(ui.token_length("peach"), 0.0, "a colour in a length slot");
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
