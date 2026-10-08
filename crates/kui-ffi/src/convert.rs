//! Conversions from the repr(C) mirrors into the core's own types, and the
//! two helpers every entry point is written with: `guard` (catch panics,
//! return a default) and `ctx` (null-check the opaque pointer).

use super::*;
use kui_core::Min;

// ---------------------------------------------------------------------------
// Conversion helpers

pub(crate) fn kstr<'a>(s: KuiStr) -> std::borrow::Cow<'a, str> {
    if s.ptr.is_null() || s.len == 0 {
        return "".into();
    }
    let bytes = unsafe { std::slice::from_raw_parts(s.ptr, s.len) };
    String::from_utf8_lossy(bytes)
}

pub(crate) fn color_of(hex: u32) -> Color {
    if hex == 0 {
        Color::TRANSPARENT
    } else {
        Color::hex(hex)
    }
}

pub(crate) fn sizing_of(s: KuiSizing) -> Sizing {
    match s.tag {
        1 => Sizing::Grow(s.value),
        2 => Sizing::Fixed(s.value),
        3 => Sizing::Percent(s.value),
        // A size expression by its number (`kui_size_*`);
        // one the table does not have is fit.
        4 => kui_core::Calc::from_id(s.value as u32).map_or(Sizing::Fit, Sizing::Calc),
        _ => Sizing::Fit,
    }
}

pub(crate) fn enter_of(e: &KuiEnter) -> Enter {
    let mut en = Enter::default();
    if e.set & KUI_ENTER_OFFSET != 0 {
        en = en.offset(e.dx, e.dy);
    }
    if e.set & KUI_ENTER_WIDTH != 0 {
        en = en.width(sizing_of(e.width));
    }
    if e.set & KUI_ENTER_HEIGHT != 0 {
        en = en.height(sizing_of(e.height));
    }
    if e.set & KUI_ENTER_BG != 0 {
        en = en.bg(color_of(e.bg));
    }
    if e.set & KUI_ENTER_RADIUS != 0 {
        en = en.radius(e.radius);
    }
    if e.set & KUI_ENTER_OPACITY != 0 {
        en = en.opacity(e.opacity);
    }
    en
}

pub(crate) fn keyframe_of(k: &KuiKeyframe) -> Keyframe {
    let mut kf = Keyframe::default();
    if k.set & KUI_KF_AT != 0 {
        kf = kf.at(k.at);
    }
    if k.set & KUI_KF_WIDTH != 0 {
        kf = kf.width(sizing_of(k.width));
    }
    if k.set & KUI_KF_HEIGHT != 0 {
        kf = kf.height(sizing_of(k.height));
    }
    if k.set & KUI_KF_BG != 0 {
        kf = kf.bg(color_of(k.bg));
    }
    if k.set & KUI_KF_RADIUS != 0 {
        kf = kf.radius(k.radius);
    }
    if k.set & KUI_KF_OPACITY != 0 {
        kf = kf.opacity(k.opacity);
    }
    kf
}

/// `KUI_START`.. `KUI_BASELINE` are `schema::ALIGNS` indices, as the
/// Lua and Node wires carry them.
pub(crate) fn align_of(a: u32) -> Align {
    kui_core::schema::align_idx(a as usize)
}

pub(crate) fn align_code(a: Align) -> u32 {
    a as u32
}

/// Null-able, consumed message payloads (`KuiValue*` owned by the caller
/// until passed here).
pub(crate) const NONE: *mut KuiValue = std::ptr::null_mut();

pub(crate) fn take_msg(p: *mut KuiValue) -> Option<Value> {
    // Consumes the value.
    (!p.is_null()).then(|| unsafe { Box::from_raw(p) }.0)
}

/// A `KuiStr` as a borrowed `&str`, or None when it is empty/NULL.
pub(crate) fn opt_str<'a>(s: KuiStr) -> Option<std::borrow::Cow<'a, str>> {
    (!s.ptr.is_null() && s.len > 0).then(|| kstr(s))
}

/// `KuiSpec.min_w` / `min_h`: 0 is undeclared, as a zeroed struct leaves
/// it — the content's floor in a share's row, 0 elsewhere —
/// `KUI_MIN_NONE` a declared 0, and any other negative (`KUI_MIN_FIT`) the
/// fit floor.
pub(crate) fn min_of(v: f32) -> Min {
    if v == crate::types::KUI_MIN_NONE {
        Min::px(0.0)
    } else if v < 0.0 {
        Min::FIT
    } else if v == 0.0 {
        Min::AUTO
    } else {
        Min::px(v)
    }
}

/// [`spec_of`] for a leaf — `kui_image`, `kui_polygon`, `kui_path`,
/// `kui_polyline`, `kui_cells`, `kui_text_edit` — whose `tooltip` the core
/// floats beside it while hovered (backlog RG113). A box's hint is the
/// third effect `kui_close` adds once the node closes; a leaf is never
/// opened, so its spec asks the leaf's door for the float instead, the
/// way `PropsOut::for_leaf` asks it for JSX and Lua.
pub(crate) fn leaf_spec_of(
    s: &KuiSpec,
    on_click: *mut KuiValue,
    on_drag: *mut KuiValue,
    on_key: *mut KuiValue,
    on_hover: *mut KuiValue,
) -> NodeSpec {
    let mut spec = spec_of(s, on_click, on_drag, on_key, on_hover);
    if opt_str(s.tooltip).is_some() {
        spec.access_mut().tooltip = true;
    }
    spec
}

pub(crate) fn spec_of(
    s: &KuiSpec,
    on_click: *mut KuiValue,
    on_drag: *mut KuiValue,
    on_key: *mut KuiValue,
    on_hover: *mut KuiValue,
) -> NodeSpec {
    let mut spec = match s.dir {
        1 => NodeSpec::row(),
        2 => NodeSpec::table(),
        _ => NodeSpec::column(),
    };
    // Straight into the spec, not through `min_width`, whose `Bound` has
    // no word for undeclared: a 0 there is a declared 0.
    spec.layout.min_w = min_of(s.min_w);
    spec.layout.min_h = min_of(s.min_h);
    spec = spec
        .width(sizing_of(s.width))
        .height(sizing_of(s.height))
        .max_width(if s.max_w > 0.0 {
            s.max_w
        } else {
            f32::INFINITY
        })
        .max_height(if s.max_h > 0.0 {
            s.max_h
        } else {
            f32::INFINITY
        })
        .with_bounds(
            crate::size::bound_of(s.min_w_size),
            crate::size::bound_of(s.max_w_size),
            crate::size::bound_of(s.min_h_size),
            crate::size::bound_of(s.max_h_size),
        )
        .padding(Edges {
            l: s.pad_l,
            r: s.pad_r,
            t: s.pad_t,
            b: s.pad_b,
        })
        .gap(s.gap)
        .cross_gap(s.cross_gap)
        .main_align(align_of(s.main_align))
        .cross_align(align_of(s.cross_align))
        .bg(color_of(s.bg))
        .radius(s.radius);
    if s.per_corner != 0 {
        spec = spec.radii(s.radius_tl, s.radius_tr, s.radius_br, s.radius_bl);
    }
    if s.border_w > 0.0 {
        spec = spec.border(s.border_w, color_of(s.border_color));
    }
    if s.opacity_set != 0 {
        spec = spec.opacity(s.opacity);
    }
    if s.wrap_children != 0 {
        spec = spec.wrap();
    }
    // Unconditional: the shadow draws only where its color is visible, and
    // that check belongs at emission, not here — a zeroed struct is the
    // default shadow either way.
    spec = spec.shadow(kui_core::Shadow {
        color: color_of(s.shadow_color),
        dx: s.shadow_x,
        dy: s.shadow_y,
        blur: s.shadow_blur,
        spread: s.shadow_spread,
    });
    spec = spec.overflow_bits(s.overflow);
    if s.float_mode != 0 {
        // The struct spells every piece out, so every piece is "declared";
        // `kui_spec_float_preset` is how a caller gets a preset's numbers
        // into these fields without knowing what "below" means.
        spec = spec.float(FloatConfig::build(
            FloatConfig::preset_at(s.float_mode as usize - 1).unwrap_or_default(),
            Some((align_of(s.float_anchor_x), align_of(s.float_anchor_y))),
            Some((align_of(s.float_self_x), align_of(s.float_self_y))),
            Some(s.float_dx),
            Some(s.float_dy),
            s.float_fit != 0,
            s.float_clip != 0,
        ));
    }
    if s.hoverable != 0 {
        spec = spec.hoverable();
    }
    if s.accent != 0 {
        spec = spec.accent();
    }
    if s.animate != 0 {
        spec = spec.animate();
    }
    if s.pixel_snap != 0 {
        spec = spec.pixel_snap();
    }
    if s.keep_focus != 0 {
        spec = spec.keep_focus();
    }
    if let Some(tag) = unsafe { s.on_focus.as_ref() } {
        spec = spec.on_focus(tag.0.clone());
    }
    if s.rules != 0 {
        spec = spec.rules(Color::hex(s.rules));
    }
    if s.rule_w != 0.0 {
        spec = spec.rule_width(s.rule_w);
    }
    if let Some(tag) = unsafe { s.on_button.as_ref() } {
        spec = spec.on_button(tag.0.clone());
    }
    // A zeroed field is all three, the field a host that never set it
    // left; `Buttons::from_bits` reads zero as none.
    if s.buttons != 0 {
        spec = spec.buttons(kui_core::Buttons::from_bits(s.buttons));
    }
    if s.overscroll != 0
        && let Some(o) = kui_core::Overscroll::ALL.get(s.overscroll as usize - 1)
    {
        spec = spec.overscroll(*o);
    }
    if s.scroll_axes != 0
        && let Some(a) = kui_core::ScrollAxes::ALL.get(s.scroll_axes as usize - 1)
    {
        spec = spec.scroll_axes(*a);
    }
    if s.selectable != 0 {
        spec = spec.selectable();
    }
    if s.focus_region != 0 {
        spec = spec.focus_region();
    }
    if s.scrollbar != 0
        && let Some(mode) = kui_core::ScrollbarMode::ALL.get(s.scrollbar as usize - 1)
    {
        spec = spec.scrollbar(*mode);
    }
    if s.scrollbar_width != 0.0 {
        spec = spec.scrollbar_width(s.scrollbar_width);
    }
    if s.scrollbar_color != 0 {
        spec = spec.scrollbar_color(color_of(s.scrollbar_color));
    }
    if s.scrollbar_active_color != 0 {
        spec = spec.scrollbar_active_color(color_of(s.scrollbar_active_color));
    }
    if s.anchor != 0 {
        spec = spec.anchor();
    }
    match s.window_role {
        1 => spec = spec.window_drag(),
        2 => spec = spec.window_button(WindowButton::Close),
        3 => spec = spec.window_button(WindowButton::Minimize),
        4 => spec = spec.window_button(WindowButton::Maximize),
        _ => {}
    }
    if s.transition_ms > 0.0 {
        spec = spec.transition(s.transition_ms);
    }
    if s.easing != 0 {
        spec = spec.easing(kui_core::schema::easing_idx(s.easing as usize));
    }
    if s.bounce != 0.0 {
        spec = spec.bounce(s.bounce);
    }
    if s.slide != 0 {
        spec = spec.slide();
    }
    if s.hover_bg != 0 {
        spec = spec.hover_bg(color_of(s.hover_bg));
    }
    if s.pressed_bg != 0 {
        spec = spec.pressed_bg(color_of(s.pressed_bg));
    }
    if !s.hover_group.ptr.is_null() && s.hover_group.len > 0 {
        spec = spec.hover_group(&kstr(s.hover_group));
    }
    if s.repeat != 0 {
        spec = spec.repeat(kui_core::schema::repeat_idx(s.repeat as usize));
    }
    if s.delay_ms != 0.0 {
        spec = spec.delay(s.delay_ms);
    }
    if !s.keyframes.is_null() && s.keyframes_len > 0 {
        let stops = unsafe { std::slice::from_raw_parts(s.keyframes, s.keyframes_len) };
        spec = spec.keyframes(stops.iter().map(keyframe_of).collect());
    }
    if s.backdrop_blur > 0.0 {
        spec = spec.backdrop_blur(s.backdrop_blur);
    }
    if s.scroll_mods != 0 {
        spec = spec.scroll_mods(kui_core::KeyMods::from_bits(s.scroll_mods));
    }
    if !s.gradient.is_null() {
        let g = unsafe { &*s.gradient };
        let stops: &[KuiGradientStop] = if g.stops.is_null() {
            &[]
        } else {
            unsafe { std::slice::from_raw_parts(g.stops, g.stops_len) }
        };
        let stops = stops.iter().map(|s| kui_core::GradientStop {
            color: color_of(s.color),
            at: (s.at >= 0.0).then_some(s.at),
        });
        spec = spec.gradient(match g.kind {
            KUI_GRADIENT_RADIAL => {
                kui_core::Gradient::radial_at(kui_core::Vec2::new(g.at_x, g.at_y), stops)
            }
            _ => kui_core::Gradient::angle(g.angle, stops),
        });
    }
    if s.enter.set != 0 {
        spec = spec.enter(enter_of(&s.enter));
    }
    if s.exit.set != 0 {
        spec = spec.exit(enter_of(&s.exit));
    }
    if s.click_sound != 0 {
        spec = spec.click_sound(kui_core::SoundId::from_ffi(s.click_sound));
    }
    if s.hover_sound != 0 {
        spec = spec.hover_sound(kui_core::SoundId::from_ffi(s.hover_sound));
    }
    if let Some(tag) = unsafe { s.on_layout.as_ref() } {
        // Borrowed, unlike the message arguments: the spec is const.
        spec = spec.on_layout(tag.0.clone());
    }
    if let Some(tag) = unsafe { s.modal.as_ref() } {
        spec = spec.modal(tag.0.clone());
    }
    if let Some(tag) = unsafe { s.on_context_menu.as_ref() } {
        spec = spec.on_context_menu(tag.0.clone());
    }
    if let Some(tag) = unsafe { s.on_force_click.as_ref() } {
        spec = spec.on_force_click(tag.0.clone());
    }
    if let Some(tag) = unsafe { s.on_scroll.as_ref() } {
        spec = spec.on_scroll(tag.0.clone());
    }
    if let Some(tag) = unsafe { s.on_drop.as_ref() } {
        spec = spec.on_drop(tag.0.clone());
    }
    if s.drop_bg != 0 {
        spec = spec.drop_bg(Color::hex(s.drop_bg));
    }
    if s.aspect_ratio > 0.0 {
        spec = spec.aspect_ratio(s.aspect_ratio);
    }
    if s.mixed != 0 {
        spec = spec.mixed(true);
    }
    if s.value_set & KUI_VALUE_STEP != 0 {
        spec = spec.value_step(s.value_step);
    }
    if let Some(tag) = unsafe { s.on_change.as_ref() } {
        spec = spec.on_change(tag.0.clone());
    }
    if s.cursor != 0 {
        // KUI_CURSOR_* = schema index + 1, so zero can mean "derive".
        spec = spec.cursor(kui_core::schema::cursor_idx(s.cursor as usize - 1));
    }
    if let Some(role) = role_of_code(s.role) {
        spec = spec.role(role);
    }
    if !s.label.ptr.is_null() && s.label.len > 0 {
        spec = spec.label(kstr(s.label).as_ref());
    }
    if s.checked != 0 {
        spec = spec.checked(true);
    }
    if s.selected != 0 {
        spec = spec.selected(true);
    }
    if s.expanded != 0 {
        // KUI_EXPANDED_* = schema index + 1, so zero can mean "unset".
        spec = spec.expanded(s.expanded == KUI_EXPANDED_EXPANDED);
    }
    if s.live != 0 {
        // KUI_LIVE_* is the schema index itself: "off" and "unset" are the
        // same thing, so zero needs no reservation.
        spec = spec.live(kui_core::Live::from_index(s.live as usize));
    }
    if s.value_set & KUI_VALUE_NOW != 0 {
        spec = spec.value_now(s.value_now);
    }
    if s.value_set & KUI_VALUE_MIN != 0 {
        spec = spec.value_min(s.value_min);
    }
    if s.value_set & KUI_VALUE_MAX != 0 {
        spec = spec.value_max(s.value_max);
    }
    if !s.value_text.ptr.is_null() && s.value_text.len > 0 {
        spec = spec.value_text(kstr(s.value_text).into_owned());
    }
    if s.value_set & KUI_VALUE_CARET != 0 {
        spec = spec.caret(s.caret);
    }
    if s.value_set & KUI_VALUE_ANCHOR != 0 {
        spec = spec.selection_anchor(s.selection_anchor);
    }
    if s.value_set & KUI_VALUE_CARET_SOLID != 0 {
        spec = spec.caret_solid();
    }
    if s.focusable != 0 {
        spec = spec.focusable();
    }
    if s.initial_focus != 0 {
        spec = spec.initial_focus();
    }
    if s.disabled != 0 {
        spec = spec.disabled(true);
    }
    if s.focus_bg != 0 {
        spec = spec.focus_bg(color_of(s.focus_bg));
    }
    if let Some(hint) = opt_str(s.tooltip) {
        // Two of the prop's three effects; `kui_close` adds the third (the
        // float, while hovered) once it knows the node closed, and a
        // leaf's door adds it for a leaf (`leaf_spec_of`).
        spec = spec.apply_tooltip(&hint);
    }
    if let Some(d) = opt_str(s.description) {
        // The description on its own — spoken, never drawn. After the
        // tooltip, so the explicit field wins over the shorthand rather
        // than the order of two `if`s deciding it.
        spec = spec.description(d);
    }
    if let Some(v) = take_msg(on_click) {
        spec = spec.on_click(v);
    }
    if let Some(v) = take_msg(on_drag) {
        spec = spec.on_drag(v);
    }
    if s.modifier_keys != 0 {
        spec = spec.modifier_keys();
    }
    if s.key_up != 0 {
        spec = spec.key_up();
    }
    if let Some(v) = take_msg(on_key) {
        spec = spec.on_key(v);
    }
    if let Some(v) = take_msg(on_hover) {
        spec = spec.on_hover(v);
    }
    spec
}

pub(crate) fn text_style_of(s: &KuiTextStyle) -> TextStyle {
    let mut style = TextStyle::new(if s.size > 0.0 { s.size } else { 16.0 });
    if s.line_height > 0.0 {
        style = style.line_height(s.line_height);
    }
    if s.color != 0 {
        style = style.color(Color::hex(s.color));
    }
    style = style.family(match s.family {
        1 => kui_core::FontFamily::Serif,
        2 => kui_core::FontFamily::Mono,
        _ => kui_core::FontFamily::Sans,
    });
    if s.font != 0 {
        style = style.font(kui_core::FontId::from_ffi(s.font));
    }
    style = style.wrap(match s.wrap {
        1 => kui_core::TextWrap::Glyph,
        2 => kui_core::TextWrap::None,
        3 => kui_core::TextWrap::BreakSpaces,
        _ => kui_core::TextWrap::Word,
    });
    if s.max_lines > 0 {
        style = style.max_lines(s.max_lines);
    }
    if s.ellipsis != 0 {
        style = style.ellipsis();
    }
    if let Some(features) = opt_str(s.features) {
        style = style.features(kui_core::FontFeatures::parse(&features));
    }
    if s.decoration & 1 != 0 {
        style = style.underline();
    }
    if s.decoration & 2 != 0 {
        style = style.strikethrough();
    }
    if s.underline_color != 0 {
        style = style.underline_color(Color::hex(s.underline_color));
    }
    if s.underline_style != 0 {
        style = style.underline_style(kui_core::UnderlineStyle::from_index(s.underline_style));
    }
    style
}

pub(crate) fn guard<T>(default: T, f: impl FnOnce() -> T) -> T {
    catch_unwind(AssertUnwindSafe(f)).unwrap_or(default)
}

pub(crate) unsafe fn ctx<'a>(ptr: *mut KuiCtx) -> Option<&'a mut KuiCtx> {
    unsafe { ptr.as_mut() }
}
