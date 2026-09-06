use super::*;
use kui_core::schema::{Kind, PROPS, Parsed, PropsOut, Target, apply};

fn zeroed_spec() -> KuiSpec {
    // Zero-initialized is the documented C default.
    unsafe { std::mem::zeroed() }
}

fn zeroed_style() -> KuiTextStyle {
    unsafe { std::mem::zeroed() }
}

fn msg(v: Value) -> *mut KuiValue {
    Box::into_raw(Box::new(KuiValue(v)))
}

/// C reaches the same four presets by name as JSX and Lua, and the
/// fields it fills round-trip back through `spec_of` to the config the
/// core would have built. A preset is a starting point, not a mode: the
/// fields stay writable afterwards.
#[test]
fn a_named_float_preset_round_trips_through_the_struct() {
    for name in kui_core::FLOAT_PRESETS {
        let mut spec = zeroed_spec();
        assert!(kui_spec_float_preset(
            &mut spec,
            KuiStr {
                ptr: name.as_ptr(),
                len: name.len(),
            }
        ));
        assert_eq!(
            spec_of(&spec, NONE, NONE, NONE, NONE).layout.float,
            FloatConfig::preset(name),
            "{name}: the C fields do not rebuild the preset"
        );
    }

    // An unknown name changes nothing, so a typo leaves a node in flow
    // rather than floating it somewhere arbitrary.
    let mut spec = zeroed_spec();
    assert!(!kui_spec_float_preset(
        &mut spec,
        KuiStr {
            ptr: "beneath".as_ptr(),
            len: 7,
        }
    ));
    assert_eq!(spec.float_mode, KUI_FLOAT_NONE);
    assert_eq!(spec_of(&spec, NONE, NONE, NONE, NONE).layout.float, None);
}

#[test]
fn zeroed_structs_are_the_schema_defaults() {
    let out = PropsOut::new();
    assert_eq!(spec_of(&zeroed_spec(), NONE, NONE, NONE, NONE), out.spec);
    assert_eq!(text_style_of(&zeroed_style()), out.style);
}

/// Every role reaches a spec from C, including the ones only a custom
/// editor declares. The row above pins the header's *names* to these
/// codes; this pins the codes to the roles, so the two ends of
/// `KuiSpec.role` cannot agree on a number that means something else
/// by the time it is a `Spec`.
#[test]
fn every_role_round_trips_through_the_c_code() {
    for role in kui_core::Role::ALL {
        let mut spec = zeroed_spec();
        spec.role = role_code(role);
        assert_eq!(
            spec_of(&spec, NONE, NONE, NONE, NONE).access().role,
            Some(role),
            "{}: KUI_ROLE_{} does not arrive as itself",
            role.name(),
            role_code(role),
        );
    }
    // Zero is "unset" and stays the way a plain box is declared, and a
    // code past the end is ignored rather than folded onto a real role.
    assert_eq!(role_of_code(0), None);
    assert_eq!(role_of_code(kui_core::Role::ALL.len() as u32 + 1), None);
}

/// C's `env` is two setters and no reading, so the header's prototypes
/// are the whole of what a host sees of the shape. This holds them to
/// `schema::ENV_FIELDS`'s C column: each setter's parameter list, in
/// order, is exactly the arguments the rows name for it, and every
/// stored fact (a row from `Env` or `WindowEnv`) is written by one of
/// the two. An argument added to a prototype, or a field added to the
/// structs and not to a setter, fails here by name.
#[test]
fn the_env_setters_take_exactly_the_documented_fields() {
    use kui_core::schema::ENV_FIELDS;
    let header = include_str!("../include/kui.h");
    let params = |name: &str| -> Vec<String> {
        let decl = format!("void {name}(KuiCtx *ctx,");
        let start = header
            .find(&decl)
            .unwrap_or_else(|| panic!("{name}'s prototype is not in kui.h"));
        let rest = &header[start + decl.len()..];
        rest[..rest.find(");").unwrap()]
            .split(',')
            .map(|a| a.split_whitespace().last().unwrap().to_string())
            .collect()
    };
    for setter in ["kui_env_set", "kui_env_set_window"] {
        let documented: Vec<String> = ENV_FIELDS
            .iter()
            .filter_map(|f| f.c.strip_prefix(&format!("`{setter}(")))
            .flat_map(|rest| rest[..rest.find(')').unwrap()].split(", "))
            .map(str::to_string)
            .collect();
        assert_eq!(
            params(setter),
            documented,
            "{setter}: the header's parameters and schema::ENV_FIELDS's C column disagree"
        );
    }
    for f in ENV_FIELDS.iter().filter(|f| !f.from.contains('(')) {
        assert!(
            f.c.starts_with("`kui_env_set(") || f.c.starts_with("`kui_env_set_window("),
            "{}: a stored env fact C cannot write",
            f.name
        );
    }
}

/// Every `PROPS` row, applied with a sample value through the schema,
/// must be reproducible by setting a `KuiSpec`/`KuiTextStyle` field (or
/// passing a message pointer). The `match` is the C-side mapping; a new
/// row without an arm panics with instructions.
#[test]
fn every_schema_prop_has_a_c_counterpart() {
    const F: f32 = 37.0;
    const C: u32 = 0x11223344;
    for def in PROPS {
        // `opacity` is a 0..=1 slot whose default is the top of the
        // range, so the shared sample would clamp back to it.
        let f = if def.name == "opacity" { 0.5 } else { F };
        let sample = match def.kind {
            Kind::F32 => Parsed::F32(f),
            Kind::Color => Parsed::Color(Color::hex(C)),
            Kind::Flag => Parsed::Flag,
            Kind::Enum(_) => Parsed::Enum(1),
            Kind::Sizing => Parsed::Sizing(Sizing::Percent(0.5)),
            Kind::Msg | Kind::Tag => Parsed::Msg(Value::Int(7)),
            Kind::Str => Parsed::Str("name".into()),
            Kind::Resource => Parsed::Resource(7),
            Kind::Keyframes => Parsed::Keyframes(vec![Keyframe::default().at(0.5).radius(F)]),
            Kind::Enter => Parsed::Enter(Enter::from(-F, 0.0).radius(F)),
        };
        let mut expected = PropsOut::new();
        apply(def, sample, &mut expected).unwrap();
        let layout_tag = KuiValue(Value::Int(7));

        let stops = [KuiKeyframe {
            set: KUI_KF_AT | KUI_KF_RADIUS,
            at: 0.5,
            width: KuiSizing { tag: 0, value: 0.0 },
            height: KuiSizing { tag: 0, value: 0.0 },
            bg: 0,
            radius: F,
            opacity: 0.0,
        }];
        let mut s = zeroed_spec();
        let mut t = zeroed_style();
        let (mut click, mut drag, mut key, mut hover) = (NONE, NONE, NONE, NONE);
        let pct = KuiSizing { tag: 3, value: 0.5 };
        let name = KuiStr {
            ptr: "name".as_ptr(),
            len: 4,
        };
        match def.name {
            "width" => s.width = pct,
            "height" => s.height = pct,
            "minWidth" => s.min_w = F,
            "maxWidth" => s.max_w = F,
            "minHeight" => s.min_h = F,
            "maxHeight" => s.max_h = F,
            "gap" => s.gap = F,
            "crossGap" => s.cross_gap = F,
            "wrapChildren" => s.wrap_children = 1,
            "radius" => s.radius = F,
            "radiusTL" => (s.per_corner, s.radius_tl) = (1, F),
            "radiusTR" => (s.per_corner, s.radius_tr) = (1, F),
            "radiusBR" => (s.per_corner, s.radius_br) = (1, F),
            "radiusBL" => (s.per_corner, s.radius_bl) = (1, F),
            // A 0..=1 slot whose default is the top of the range, so
            // the shared sample would clamp back to it.
            "opacity" => (s.opacity_set, s.opacity) = (1, 0.5),
            "shadowColor" => s.shadow_color = C,
            "shadowBlur" => s.shadow_blur = F,
            "shadowX" => s.shadow_x = F,
            "shadowY" => s.shadow_y = F,
            "shadowSpread" => s.shadow_spread = F,
            "mainAlign" => s.main_align = 1,
            "crossAlign" => s.cross_align = 1,
            "center" => (s.main_align, s.cross_align) = (1, 1),
            "bg" => s.bg = C,
            "hoverable" => s.hoverable = 1,
            "window" => s.window_role = 2, // KUI_WINDOW_* = schema index + 1
            "transition" => s.transition_ms = F,
            "easing" => s.easing = 1,
            "slide" => s.slide = 1,
            "keyframes" => (s.keyframes, s.keyframes_len) = (stops.as_ptr(), 1),
            "enter" => {
                s.enter = KuiEnter {
                    set: KUI_ENTER_OFFSET | KUI_ENTER_RADIUS,
                    dx: -F,
                    dy: 0.0,
                    width: KuiSizing { tag: 0, value: 0.0 },
                    height: KuiSizing { tag: 0, value: 0.0 },
                    bg: 0,
                    radius: F,
                    opacity: 0.0,
                }
            }
            "exit" => {
                s.exit = KuiEnter {
                    set: KUI_ENTER_OFFSET | KUI_ENTER_RADIUS,
                    dx: -F,
                    dy: 0.0,
                    width: KuiSizing { tag: 0, value: 0.0 },
                    height: KuiSizing { tag: 0, value: 0.0 },
                    bg: 0,
                    radius: F,
                    opacity: 0.0,
                }
            }
            "repeat" => s.repeat = 1,
            "delay" => s.delay_ms = F,
            "onClick" => click = msg(Value::Int(7)),
            "onDrag" => drag = msg(Value::Int(7)),
            "onKey" => key = msg(Value::Int(7)),
            "onHover" => hover = msg(Value::Int(7)),
            "onLayout" => s.on_layout = &layout_tag,
            "modal" => s.modal = &layout_tag,
            "onContextMenu" => s.on_context_menu = &layout_tag,
            "cursor" => s.cursor = 2, // KUI_CURSOR_* = schema index + 1
            "hoverBg" => s.hover_bg = C,
            "pressedBg" => s.pressed_bg = C,
            "hoverGroup" => s.hover_group = name,
            "focusable" => s.focusable = 1,
            "initialFocus" => s.initial_focus = 1,
            "disabled" => s.disabled = 1,
            "focusBg" => s.focus_bg = C,
            "clickSound" => s.click_sound = 7,
            "hoverSound" => s.hover_sound = 7,
            "role" => s.role = 2, // KUI_ROLE_* = Role::ALL index + 1; ROLES[1] = button
            "label" => s.label = name,
            "checked" => s.checked = 1,
            "selected" => s.selected = 1,
            "expanded" => s.expanded = KUI_EXPANDED_EXPANDED,
            "live" => s.live = KUI_LIVE_POLITE, // the parity index is 1; LIVE[1] = polite
            "valueNow" => (s.value_set, s.value_now) = (KUI_VALUE_NOW, F),
            "valueMin" => (s.value_set, s.value_min) = (KUI_VALUE_MIN, F),
            "valueMax" => (s.value_set, s.value_max) = (KUI_VALUE_MAX, F),
            "caret" => (s.value_set, s.caret) = (KUI_VALUE_CARET, F as u32),
            "selectionAnchor" => (s.value_set, s.selection_anchor) = (KUI_VALUE_ANCHOR, F as u32),
            "lineHeight" => t.line_height = F,
            "color" => t.color = C,
            "family" => t.family = 1,
            "font" => t.font = 7,
            "wrap" => t.wrap = 1,
            "maxLines" => t.max_lines = F as u32,
            "ellipsis" => t.ellipsis = 1,
            other => panic!(
                "schema prop {other:?} has no C counterpart: add a KuiSpec/KuiTextStyle \
                     field (append-only — the struct is ABI), mirror it in include/kui.h, \
                     apply it in spec_of/text_style_of, and map it here"
            ),
        }
        match def.target() {
            Target::Spec => assert_eq!(
                spec_of(&s, click, drag, key, hover),
                expected.spec,
                "{}: C mapping disagrees with the schema",
                def.name
            ),
            Target::Style => assert_eq!(
                text_style_of(&t),
                expected.style,
                "{}: C mapping disagrees with the schema",
                def.name
            ),
        }
    }
}

/// The hand-written composites (dir, pad, border, overflow, float) and
/// the whole struct at once against the Rust builder.
#[test]
fn fully_populated_spec_matches_the_rust_builder() {
    let stops = [
        KuiKeyframe {
            set: KUI_KF_WIDTH | KUI_KF_BG,
            at: 0.0,
            width: KuiSizing { tag: 1, value: 0.0 },
            height: KuiSizing { tag: 0, value: 0.0 },
            bg: 0x11_22_33_ff,
            radius: 0.0,
            opacity: 0.0,
        },
        KuiKeyframe {
            set: KUI_KF_AT | KUI_KF_WIDTH | KUI_KF_HEIGHT | KUI_KF_RADIUS,
            at: 0.75,
            width: KuiSizing { tag: 1, value: 1.0 },
            height: KuiSizing { tag: 3, value: 0.5 },
            bg: 0,
            radius: 9.0,
            opacity: 0.0,
        },
    ];
    let modal_tag = KuiValue(Value::str("m"));
    let menu_tag = KuiValue(Value::str("cm"));
    let s = KuiSpec {
        width: KuiSizing { tag: 1, value: 2.0 },
        height: KuiSizing {
            tag: 2,
            value: 120.0,
        },
        min_w: 10.0,
        max_w: 500.0,
        min_h: 5.0,
        max_h: 300.0,
        dir: 1,
        pad_l: 1.0,
        pad_r: 2.0,
        pad_t: 3.0,
        pad_b: 4.0,
        gap: 8.0,
        main_align: 1,
        cross_align: 2,
        bg: 0x14161eff,
        border_color: 0x2a2d3aff,
        border_w: 1.0,
        radius: 6.0,
        overflow: 1 | 2 | 4,
        float_mode: 2,
        float_anchor_x: 2,
        float_anchor_y: 2,
        float_self_x: 2,
        float_self_y: 2,
        float_dx: -8.0,
        float_dy: -8.0,
        float_fit: 1,
        hoverable: 1,
        window_role: 1,
        transition_ms: 150.0,
        easing: 3,
        slide: 1,
        hover_bg: 0x47_6c_e0_ff,
        pressed_bg: 0x2f_54_c4_ff,
        hover_group: KuiStr {
            ptr: "grp".as_ptr(),
            len: 3,
        },
        per_corner: 1,
        radius_tl: 1.0,
        radius_tr: 2.0,
        radius_br: 3.0,
        radius_bl: 4.0,
        repeat: 2,
        delay_ms: 50.0,
        keyframes: stops.as_ptr(),
        keyframes_len: stops.len(),
        enter: unsafe { std::mem::zeroed() },
        click_sound: 0,
        hover_sound: 0,
        on_layout: std::ptr::null(),
        role: 2,
        label: KuiStr {
            ptr: "lbl".as_ptr(),
            len: 3,
        },
        checked: 1,
        value_set: KUI_VALUE_NOW | KUI_VALUE_MIN | KUI_VALUE_MAX,
        value_now: 3.0,
        value_min: 0.0,
        value_max: 10.0,
        caret: 0,
        selection_anchor: 0,
        focusable: 1,
        disabled: 1,
        focus_bg: 0x11_22_33_ff,
        tooltip: KuiStr {
            ptr: "hint".as_ptr(),
            len: 4,
        },
        modal: &modal_tag,
        on_context_menu: &menu_tag,
        cursor: 7, // KUI_CURSOR_EW_RESIZE
        selected: 1,
        expanded: KUI_EXPANDED_EXPANDED,
        live: KUI_LIVE_ASSERTIVE,
        opacity_set: 1,
        opacity: 0.4,
        shadow_color: 0x00_00_00_66,
        shadow_blur: 12.0,
        shadow_x: 0.0,
        shadow_y: 4.0,
        shadow_spread: -2.0,
        wrap_children: 1,
        cross_gap: 6.0,
        initial_focus: 1,
        exit: unsafe { std::mem::zeroed() },
    };
    let expected = NodeSpec::row()
        .width(Sizing::Grow(2.0))
        .height(Sizing::Fixed(120.0))
        .min_width(10.0)
        .max_width(500.0)
        .min_height(5.0)
        .max_height(300.0)
        .padding(Edges {
            l: 1.0,
            r: 2.0,
            t: 3.0,
            b: 4.0,
        })
        .gap(8.0)
        .wrap()
        .cross_gap(6.0)
        .main_align(Align::Center)
        .cross_align(Align::End)
        .bg(Color::hex(0x14161eff))
        .radii(1.0, 2.0, 3.0, 4.0)
        .border(1.0, Color::hex(0x2a2d3aff))
        .clip()
        .scroll_x()
        .scroll_y()
        .float(
            FloatConfig::viewport()
                .at(Align::End, Align::End)
                .self_at(Align::End, Align::End)
                .offset(-8.0, -8.0)
                .fit(),
        )
        .hoverable()
        .window_drag()
        .transition(150.0)
        .easing(kui_core::Easing::EaseInOut)
        .slide()
        .hover_bg(Color::hex(0x47_6c_e0_ff))
        .pressed_bg(Color::hex(0x2f_54_c4_ff))
        .hover_group("grp")
        .focusable()
        .initial_focus()
        .disabled(true)
        .focus_bg(Color::hex(0x11_22_33_ff))
        .description("hint")
        .role(kui_core::Role::Button)
        .label("lbl")
        .checked(true)
        .selected(true)
        .expanded(true)
        .live(kui_core::Live::Assertive)
        .value_now(3.0)
        .value_min(0.0)
        .value_max(10.0)
        .opacity(0.4)
        .shadow(kui_core::Shadow {
            color: Color::hex(0x00_00_00_66),
            dx: 0.0,
            dy: 4.0,
            blur: 12.0,
            spread: -2.0,
        })
        .repeat(kui_core::Repeat::Alternate)
        .delay(50.0)
        .keyframes(vec![
            Keyframe::default()
                .width(Sizing::Grow(0.0))
                .bg(Color::hex(0x11_22_33_ff)),
            Keyframe::default()
                .at(0.75)
                .width(Sizing::Grow(1.0))
                .height(Sizing::Percent(0.5))
                .radius(9.0),
        ])
        .on_click(Value::str("c"))
        .on_drag(Value::str("d"))
        .on_key(Value::str("k"))
        .on_hover(Value::str("h"))
        .modal(Value::str("m"))
        .on_context_menu(Value::str("cm"))
        .cursor(kui_core::CursorShape::EwResize);
    let got = spec_of(
        &s,
        msg("c".into()),
        msg("d".into()),
        msg("k".into()),
        msg("h".into()),
    );
    assert_eq!(got, expected);
}
