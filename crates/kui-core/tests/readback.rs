//! The readback shapes as plain data (backlog AR1): every query answer a
//! binding hands out — `ScrollGeometry`, `TextHit`, `Rect`, `TextMetrics`,
//! `NodeInfo`, `AccessTree`, `WindowCommand`, `Warning`, `AudioCommand`
//! — has a `to_value` in the core, keyed in snake_case, so a binding
//! converts once (Node camelises, Lua takes the keys as they are) and the
//! shape is testable here, with no addon in the way. A handle inside one
//! — a node key, a resource id — is spelled by the binding's `Handles`.

use kui_core::{
    AccessAction, AccessRequest, AudioCommand, Core, EditOptions, Handles, Key, NodeSpec,
    PlaybackId, Role, Size, Sizing, SoundId, TextPos, TextStyle, Value, Vec2, Warning,
    WindowCommand, WindowConfig, WindowId, WindowKind,
};

/// Every map key at every depth, `a.b[].c` style.
fn keys(v: &Value, prefix: &str, out: &mut Vec<String>) {
    match v {
        Value::Map(m) => {
            for (k, v) in m {
                let path = if prefix.is_empty() {
                    k.clone()
                } else {
                    format!("{prefix}.{k}")
                };
                out.push(path.clone());
                keys(v, &path, out);
            }
        }
        Value::List(a) => {
            for v in a {
                keys(v, &format!("{prefix}[]"), out);
            }
        }
        _ => {}
    }
}

fn all_keys(v: &Value) -> Vec<String> {
    let mut out = Vec::new();
    keys(v, "", &mut out);
    out.sort();
    out.dedup();
    out
}

fn draw(core: &mut Core) -> Key {
    let mut ui = core.frame(Size::new(400.0, 300.0), 1.0);
    ui.configure_root(NodeSpec::column().fill());
    let mut doc = Key::ROOT;
    ui.with_keyed(
        "scroller",
        NodeSpec::column()
            .scroll_y()
            .width(Sizing::Fixed(300.0))
            .height(Sizing::Fixed(40.0)),
        |ui| {
            doc = ui.text_edit(
                "doc",
                "hello world\nsecond\nthird\nfourth",
                &EditOptions {
                    multiline: true,
                    autofocus: true,
                    ..Default::default()
                },
                NodeSpec::column().width(Sizing::Fixed(280.0)),
            );
        },
    );
    ui.with_keyed(
        "vol",
        NodeSpec::row()
            .role(Role::Slider)
            .label("Volume")
            .value_now(0.4)
            .value_min(0.0)
            .value_max(1.0)
            .on_drag(Value::str("vol")),
        |ui| ui.text("vol", TextStyle::new(12.0)),
    );
    ui.finish();
    doc
}

#[test]
fn every_readback_key_is_snake_case() {
    let mut core = Core::new();
    core.set_inspect(true);
    let doc = draw(&mut core);
    let run = core.access_tree().get(doc).unwrap().runs[0].key;
    core.handle_input(kui_core::InputEvent::Access(
        AccessRequest::new(doc, AccessAction::SetTextSelection)
            .with_selection(TextPos { run, character: 0 }, TextPos { run, character: 3 }),
    ));
    draw(&mut core);
    let mut shapes = vec![
        core.scroll_geometry(Key::ROOT.str("scroller"))
            .unwrap()
            .to_value(),
        core.text_hit(Key::ROOT.str("vol"), Vec2::new(2.0, 50.0))
            .unwrap()
            .to_value(),
        core.caret_rect(Key::ROOT.str("vol"), 1).unwrap().to_value(),
        core.measure_text("x", &TextStyle::new(12.0), None)
            .to_value(),
        core.access_tree().to_value(Handles::INT),
        WindowCommand::Open {
            id: WindowId(1),
            owner: WindowId::MAIN,
            origin: kui_core::OriginId::HOST,
            config: WindowConfig::of_kind(WindowKind::Popup),
        }
        .to_value(),
        AudioCommand::Play {
            playback: PlaybackId(1),
            sound: SoundId::from_ffi(1),
            volume: 1.0,
            looped: false,
            fade_in_ms: 0.0,
        }
        .to_value(Handles::INT),
        Warning {
            code: kui_core::diag::DUPLICATE_KEY,
            key: Key::ROOT,
            message: String::new(),
        }
        .to_value(Handles::INT),
    ];
    shapes.extend(core.nodes().iter().map(|n| n.to_value(Handles::INT)));
    for shape in &shapes {
        for k in all_keys(shape) {
            let leaf = k.rsplit('.').next().unwrap();
            // An event name inside `events` is the handler's row name,
            // which is the app's vocabulary and not this shape's.
            if k.contains("events.") {
                continue;
            }
            assert!(
                leaf.chars()
                    .all(|c| c.is_ascii_lowercase() || c == '_' || c == '[' || c == ']'),
                "{k} is not snake_case in {shape:?}"
            );
        }
    }
}

#[test]
fn the_slider_reads_by_its_row_names_and_a_handle_by_the_bindings_spelling() {
    let mut core = Core::new();
    draw(&mut core);
    let vol = Key::ROOT.str("vol");
    let node = core.access_tree().get(vol).unwrap();
    let int = node.to_value(Handles::INT);
    assert_eq!(int.get("value_now"), Some(&Value::Float(0.4f32 as f64)));
    assert_eq!(int.get("value_min"), Some(&Value::Float(0.0)));
    assert_eq!(int.get("value_max"), Some(&Value::Float(1.0)));
    assert_eq!(int.get("key"), Some(&Value::Int(vol.0 as i64)));
    let hex = node.to_value(Handles::HEX);
    assert_eq!(
        hex.get("key"),
        Some(&Value::Str(format!("{:016x}", vol.0))),
        "Node spells a key as sixteen hex digits"
    );
    assert_eq!(hex.get("value_now"), int.get("value_now"));
    // A resource id takes the same spelling: its `to_ffi` bits, as the
    // integer or as the sixteen digits.
    let sound = SoundId::from_ffi(0xabc);
    let unload = AudioCommand::Unload { sound };
    assert_eq!(
        unload.to_value(Handles::HEX).get("sound"),
        Some(&Value::Str(format!("{:016x}", sound.to_ffi())))
    );
    assert_eq!(
        unload.to_value(Handles::INT).get("sound"),
        Some(&Value::Int(sound.to_ffi() as i64))
    );
}

#[test]
fn a_sizing_describes_itself_the_way_a_spec_spells_it() {
    assert_eq!(Sizing::Fit.describe(), "fit");
    assert_eq!(Sizing::Grow(1.0).describe(), "grow(1)");
    assert_eq!(Sizing::Fixed(120.0).describe(), "120px");
    assert_eq!(Sizing::Fixed(12.5).describe(), "12.5px");
    assert_eq!(Sizing::Percent(0.5).describe(), "50%");
}
