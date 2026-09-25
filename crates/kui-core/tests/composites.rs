//! The composite props' *decisions*, where they now live. Every binding
//! extracts these pieces from its own value type — a Lua table, a JSON map,
//! a binary stream, a C struct — and then calls the functions pinned here,
//! so a rule proved once holds in four languages.

use kui_core::{
    Align, Edges, FLOAT_PRESETS, FloatAnchor, FloatConfig, NodeSpec, OVERFLOW_CLIP,
    OVERFLOW_SCROLL_X, OVERFLOW_SCROLL_Y, PadShorthand, Vec2Offset, schema::PropsOut,
};

/// Both spellings of a preset — the name a frontend read and the index the
/// wire carried — reach the same config.
#[test]
fn preset_names_and_wire_indices_agree() {
    for (i, name) in FLOAT_PRESETS.iter().enumerate() {
        assert_eq!(
            FloatConfig::preset(name),
            FloatConfig::preset_at(i),
            "{name} disagrees with wire index {i}"
        );
    }
    assert_eq!(FloatConfig::preset("parent"), Some(FloatConfig::parent()));
    assert_eq!(
        FloatConfig::preset("viewport"),
        Some(FloatConfig::viewport())
    );
    assert_eq!(FloatConfig::preset("below"), Some(FloatConfig::below()));
    assert_eq!(FloatConfig::preset("above"), Some(FloatConfig::above()));
    assert_eq!(FloatConfig::preset("beneath"), None);
    assert_eq!(FloatConfig::preset_at(FLOAT_PRESETS.len()), None);
}

/// An override a frontend did not see leaves the preset's own value alone.
/// This is what keeps a tooltip's 6px gap when the app only nudged it
/// sideways — the bug a per-binding `offset(dx ?? 0, dy ?? 0)` shipped.
#[test]
fn an_undeclared_override_keeps_the_presets_value() {
    let below = FloatConfig::below();
    let nudged = FloatConfig::build(below, None, None, Some(4.0), None, false, false);
    assert_eq!(nudged.offset, Vec2Offset { x: 4.0, y: 6.0 });
    assert_eq!(nudged.anchor_point, below.anchor_point);
    assert_eq!(nudged.self_point, below.self_point);

    // Nothing declared at all is the preset, unchanged.
    assert_eq!(
        FloatConfig::build(below, None, None, None, None, false, false),
        below
    );
}

#[test]
fn declared_overrides_win_over_the_base() {
    let cfg = FloatConfig::build(
        FloatConfig::viewport(),
        Some((Align::End, Align::End)),
        Some((Align::Center, Align::Start)),
        Some(-8.0),
        Some(-8.0),
        true,
        true,
    );
    assert_eq!(cfg.anchor, FloatAnchor::Viewport);
    assert_eq!(cfg.anchor_point, (Align::End, Align::End));
    assert_eq!(cfg.self_point, (Align::Center, Align::Start));
    assert_eq!(cfg.offset, Vec2Offset { x: -8.0, y: -8.0 });
    assert!(cfg.fit);
    // Declared, and read by no viewport float: the anchor decides.
    assert!(cfg.clip);
    assert!(!cfg.clipped_by_parent());
}

/// An edge falls back to its axis, an axis to the all-round `pad`, and the
/// specific value always wins.
#[test]
fn the_pad_shorthand_resolves_specific_over_axis_over_all() {
    let pad = PadShorthand {
        all: Some(4.0),
        x: Some(10.0),
        b: Some(1.0),
        ..PadShorthand::default()
    };
    assert_eq!(
        pad.resolve(),
        Edges {
            l: 10.0, // padX
            r: 10.0, // padX
            t: 4.0,  // pad, no padY
            b: 1.0,  // padB wins over pad
        }
    );
    assert_eq!(PadShorthand::default().resolve(), Edges::all(0.0));
}

/// Nothing declared is not "pad: 0": a spec that got its padding elsewhere
/// keeps it, which is why `apply_pad` asks first.
#[test]
fn an_undeclared_pad_family_leaves_the_spec_alone() {
    assert!(!PadShorthand::default().declared());
    assert!(
        PadShorthand {
            t: Some(0.0),
            ..PadShorthand::default()
        }
        .declared()
    );

    let mut out = PropsOut::new();
    out.with_spec(|s| s.pad(7.0));
    out.apply_pad(PadShorthand::default());
    assert_eq!(out.spec.layout.padding, Edges::all(7.0));
}

/// Scrolling clips, and the bits say so once rather than in four parsers.
#[test]
fn the_overflow_bits_mean_the_same_everywhere() {
    let scrolled = NodeSpec::column().overflow_bits(OVERFLOW_SCROLL_Y);
    assert!(scrolled.layout.scroll_y);
    assert!(scrolled.layout.clips(), "scrolling clips");

    let all =
        NodeSpec::column().overflow_bits(OVERFLOW_CLIP | OVERFLOW_SCROLL_X | OVERFLOW_SCROLL_Y);
    assert_eq!(all, NodeSpec::column().clip().scroll_x().scroll_y());

    assert_eq!(NodeSpec::column().overflow_bits(0), NodeSpec::column());
}

/// A hint is three things at once, and no caller gets to pick two.
#[test]
fn a_tooltip_hint_is_hover_gated_described_and_floated() {
    let mut out = PropsOut::new();
    out.apply_tooltip("undo");
    assert!(out.spec.hover_tracked(), "hover-gated");
    assert_eq!(
        out.spec
            .access()
            .description
            .as_ref()
            .map(|d| d.to_string()),
        Some("undo".to_string()),
        "and said aloud"
    );
    assert_eq!(out.tooltip.as_deref(), Some("undo"), "and floated");

    // The spec half on its own is the same two effects, for C, which floats
    // the hint from `kui_close` instead.
    assert_eq!(NodeSpec::column().apply_tooltip("undo"), out.spec);
}
