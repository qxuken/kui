//! Conversions from the repr(C) mirrors into the core's own types, and the
//! two helpers every entry point is written with: `guard` (catch panics,
//! return a default) and `ctx` (null-check the opaque pointer).

use super::*;
use kui_core::Min;
use kui_core::spec::Handler;

// ---------------------------------------------------------------------------
// Conversion helpers

pub(crate) fn kstr<'a>(s: KuiStr) -> std::borrow::Cow<'a, str> {
    if s.ptr.is_null() || s.len == 0 {
        return "".into();
    }
    let bytes = unsafe { std::slice::from_raw_parts(s.ptr, s.len) };
    // The check first: it reads ASCII a word at a time, where the lossy
    // walk goes chunk by chunk, and every string a host passes is valid
    // but for the odd one — a row's text cost a third of `kui_text` in
    // the walk (backlog F281). The answer is the same either way.
    match std::str::from_utf8(bytes) {
        Ok(s) => s.into(),
        Err(_) => String::from_utf8_lossy(bytes),
    }
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
    if e.set & KUI_ENTER_ROTATE != 0 {
        en = en.rotate(e.rotate);
    }
    if e.set & KUI_ENTER_SCALE != 0 {
        en = en.scale(e.scale);
    }
    if e.set & KUI_ENTER_SCALE_X != 0 {
        en = en.scale_x(e.scale_x);
    }
    if e.set & KUI_ENTER_SCALE_Y != 0 {
        en = en.scale_y(e.scale_y);
    }
    if e.set & KUI_ENTER_KEEP_ROOM != 0 {
        en = en.keep_room();
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
    if k.set & KUI_KF_ROTATE != 0 {
        kf = kf.rotate(k.rotate);
    }
    if k.set & KUI_KF_SCALE != 0 {
        kf = kf.scale(k.scale);
    }
    if k.set & KUI_KF_SCALE_X != 0 {
        kf = kf.scale_x(k.scale_x);
    }
    if k.set & KUI_KF_SCALE_Y != 0 {
        kf = kf.scale_y(k.scale_y);
    }
    if k.set & KUI_KF_OFFSET != 0 {
        kf = kf.offset(k.dx, k.dy);
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
    // The fields every spec carries go straight in, not through the
    // builders: each of those takes the 224-byte spec and hands it back,
    // a copy out of line per call across the crate boundary, and the
    // chain of them was a third of this function (backlog F281). Each
    // store is what the builder it stands in for did.
    let l = &mut spec.layout;
    // Not through `min_width` either, whose `Bound` has no word for
    // undeclared: a 0 there is a declared 0.
    l.min_w = min_of(s.min_w);
    l.min_h = min_of(s.min_h);
    l.width = sizing_of(s.width);
    l.height = sizing_of(s.height);
    // `max_width`'s ceiling of a positive px is the px itself.
    l.max_w = if s.max_w > 0.0 {
        s.max_w
    } else {
        f32::INFINITY
    };
    l.max_h = if s.max_h > 0.0 {
        s.max_h
    } else {
        f32::INFINITY
    };
    l.padding = Edges {
        l: s.pad_l,
        r: s.pad_r,
        t: s.pad_t,
        b: s.pad_b,
    };
    l.gap = s.gap;
    l.cross_gap = s.cross_gap;
    l.main_align = align_of(s.main_align);
    l.cross_align = align_of(s.cross_align);
    spec.style.bg = color_of(s.bg);
    spec.style.radius = [s.radius; 4];
    // Unconditional: the shadow draws only where its color is visible, and
    // that check belongs at emission, not here — a zeroed struct is the
    // default shadow either way.
    spec.style.shadow = kui_core::Shadow {
        color: color_of(s.shadow_color),
        dx: s.shadow_x,
        dy: s.shadow_y,
        blur: s.shadow_blur,
        spread: s.shadow_spread,
    };
    let bounds = [s.min_w_size, s.max_w_size, s.min_h_size, s.max_h_size];
    if bounds.iter().any(|b| b.tag != 0) {
        spec = spec.with_bounds(
            crate::size::bound_of(s.min_w_size),
            crate::size::bound_of(s.max_w_size),
            crate::size::bound_of(s.min_h_size),
            crate::size::bound_of(s.max_h_size),
        );
    }
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
    if s.overflow != 0 {
        spec = spec.overflow_bits(s.overflow);
    }
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
            // `float_clip` is what a parent-anchored float does now (ADR
            // 0074): not read.
            s.float_escape != 0,
        ));
    }
    if s.hoverable != 0 {
        spec.hoverable = true;
    }
    if s.accent != 0 {
        spec.accent = true;
    }
    if s.animate != 0 {
        spec.animate = true;
    }
    if s.pixel_snap != 0 {
        spec.style.pixel_snap = true;
    }
    if s.keep_focus != 0 {
        spec.events_mut().keep_focus = true;
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
    if s.host_hit != 0
        && let Some(h) = kui_core::HostHit::ALL.get(s.host_hit as usize - 1)
    {
        spec = spec.host_hit(*h);
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
    if s.over_siblings != 0 {
        spec = spec.over_siblings();
    }
    if s.hover_bg != 0 {
        spec.interact_mut().hover_bg = Some(color_of(s.hover_bg));
    }
    if s.pressed_bg != 0 {
        spec.interact_mut().pressed_bg = Some(color_of(s.pressed_bg));
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
    if s.iterations > 0.0 {
        spec = spec.iterations(s.iterations);
    }
    if s.rotate != 0.0 {
        spec = spec.rotate(s.rotate);
    }
    if s.scale != 0.0 && s.scale != 1.0 {
        spec = spec.scale(s.scale);
    }
    if s.scale_x != 0.0 && s.scale_x != 1.0 {
        spec = spec.scale_x(s.scale_x);
    }
    if s.scale_y != 0.0 && s.scale_y != 1.0 {
        spec = spec.scale_y(s.scale_y);
    }
    if s.pivot_set & KUI_PIVOT_X != 0 {
        let fy = spec.transform_spec().map_or(0.5, |t| t.pivot.y);
        spec = spec.pivot(s.pivot_x, fy);
    }
    if s.pivot_set & KUI_PIVOT_Y != 0 {
        let fx = spec.transform_spec().map_or(0.5, |t| t.pivot.x);
        spec = spec.pivot(fx, s.pivot_y);
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
    if let Some(msg) = unsafe { s.on_change.as_ref() } {
        spec = spec.on_change(msg.0.clone());
    }
    // An editor's other handler rows, and the menu and selection-range
    // ones (ADR 0047): borrowed, like `on_change`.
    if let Some(msg) = unsafe { s.on_submit.as_ref() } {
        spec = spec.on_submit(msg.0.clone());
    }
    if let Some(msg) = unsafe { s.on_cancel.as_ref() } {
        spec = spec.on_cancel(msg.0.clone());
    }
    if let Some(msg) = unsafe { s.on_boundary.as_ref() } {
        spec = spec.on_boundary(msg.0.clone());
    }
    if let Some(msg) = unsafe { s.on_menu.as_ref() } {
        spec = spec.on_menu(msg.0.clone());
    }
    if let Some(msg) = unsafe { s.on_selection_range.as_ref() } {
        spec = spec.on_selection_range(msg.0.clone());
    }
    if let Some(msg) = unsafe { s.on_range.as_ref() } {
        spec = spec.on_range(msg.0.clone());
    }
    if let Some(msg) = unsafe { s.on_typeahead.as_ref() } {
        spec = spec.on_typeahead(msg.0.clone());
    }
    if let Some(msg) = unsafe { s.on_item_key.as_ref() } {
        spec = spec.on_item_key(msg.0.clone());
    }
    if let Some(i) = unsafe { s.active_index.as_ref() }.and_then(|v| v.0.as_int())
        && i >= 0
    {
        spec = spec.active_index(i as u64);
    }
    if s.reveal_on_hover != 0 {
        spec = spec.reveal_on_hover();
    }
    if s.floats_in_order != 0 {
        spec = spec.floats_in_order();
    }
    if s.cursor != 0 {
        // KUI_CURSOR_* = schema index + 1, so zero can mean "derive".
        spec.cursor = Some(kui_core::schema::cursor_idx(s.cursor as usize - 1));
    }
    if let Some(role) = role_of_code(s.role) {
        spec.access_mut().role = Some(role);
    }
    if !s.label.ptr.is_null() && s.label.len > 0 {
        spec = spec.label(kstr(s.label).as_ref());
    }
    if s.checked != 0 {
        spec.access_mut().checked = true;
    }
    if s.selected != 0 {
        spec.access_mut().selected = true;
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
        spec.focusable = true;
    }
    if s.initial_focus != 0 {
        spec.initial_focus = true;
    }
    if s.disabled != 0 {
        spec.disabled = true;
    }
    if s.focus_bg != 0 {
        spec.interact_mut().focus_bg = Some(color_of(s.focus_bg));
    }
    if s.focus_ring != 0 {
        spec.interact_mut().focus_ring = Some(color_of(s.focus_ring));
    }
    if s.text_color != 0 {
        spec.interact_mut().text_color = Some(color_of(s.text_color));
    }
    if s.min_column > 0.0 {
        spec = spec.min_column(s.min_column);
    }
    if s.hidden != 0 {
        spec.hidden = true;
    }
    if s.essential_motion != 0 {
        spec = spec.essential_motion();
    }
    if s.dx != 0.0 {
        spec = spec.dx(s.dx);
    }
    if s.dy != 0.0 {
        spec = spec.dy(s.dy);
    }
    if s.opens_on_press != 0 {
        spec = spec.opens_on_press();
    }
    if let Some(msg) = unsafe { s.on_query.as_ref() } {
        spec = spec.on_query(msg.0.clone());
    }
    if let Some(list) = unsafe { s.when.as_ref() } {
        // A value that is not a list is no `when`, as a bad colour is no
        // colour; an entry's own trouble rides on the entry and is raised
        // as the node opens.
        if let Ok(rules) = kui_core::rules::parse_with(&list.0, None) {
            for r in rules {
                spec.rules_mut().push(r);
            }
        }
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
        spec.events_mut().set(Handler::Click, v);
    }
    if let Some(v) = take_msg(on_drag) {
        spec.events_mut().set(Handler::Drag, v);
    }
    if s.modifier_keys != 0 {
        spec = spec.modifier_keys();
    }
    if s.key_up != 0 {
        spec = spec.key_up();
    }
    if let Some(v) = take_msg(on_key) {
        spec.events_mut().set(Handler::Key, v);
    }
    if let Some(v) = take_msg(on_hover) {
        spec.events_mut().set(Handler::Hover, v);
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
    if s.bold != 0 {
        style = style.bold();
    }
    if s.initial_letter > 0 {
        style = style.initial_letter(s.initial_letter);
    }
    style
}

pub(crate) fn guard<T>(default: T, f: impl FnOnce() -> T) -> T {
    catch_unwind(AssertUnwindSafe(f)).unwrap_or(default)
}

pub(crate) unsafe fn ctx<'a>(ptr: *mut KuiCtx) -> Option<&'a mut KuiCtx> {
    unsafe { ptr.as_mut() }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn zeroed() -> KuiSpec {
        unsafe { std::mem::zeroed() }
    }

    /// `NodeSpec` has no `PartialEq`; its `Debug` prints every field.
    fn same(a: &NodeSpec, b: &NodeSpec) {
        assert_eq!(format!("{a:?}"), format!("{b:?}"));
    }

    /// A zeroed struct is the column a builder starts from: the fields
    /// `spec_of` stores without asking (backlog F281) store the defaults.
    #[test]
    fn a_zeroed_spec_is_a_plain_column() {
        same(
            &spec_of(&zeroed(), NONE, NONE, NONE, NONE),
            &NodeSpec::column(),
        );
    }

    /// The stores `spec_of` makes in place of builders land where the
    /// builders put them.
    #[test]
    fn spec_of_stores_what_the_builders_would() {
        let mut s = zeroed();
        s.dir = 1;
        s.width = KuiSizing { tag: 1, value: 2.0 };
        s.height = KuiSizing {
            tag: 2,
            value: 30.0,
        };
        s.min_w = 10.0;
        s.max_w = 300.0;
        s.max_h = -1.0;
        s.pad_l = 1.0;
        s.pad_r = 2.0;
        s.pad_t = 3.0;
        s.pad_b = 4.0;
        s.gap = 5.0;
        s.cross_gap = 6.0;
        s.main_align = 1;
        s.cross_align = 2;
        s.bg = 0x102030ff;
        s.radius = 7.0;
        s.shadow_color = 0x00000080;
        s.shadow_y = 2.0;
        s.shadow_blur = 8.0;
        s.hoverable = 1;
        s.accent = 1;
        s.animate = 1;
        s.pixel_snap = 1;
        s.keep_focus = 1;
        s.hover_bg = 0x203040ff;
        s.pressed_bg = 0x304050ff;
        s.focus_bg = 0x405060ff;
        s.focus_ring = 0x506070ff;
        s.text_color = 0x607080ff;
        s.role = 1;
        s.checked = 1;
        s.selected = 1;
        s.focusable = 1;
        s.initial_focus = 1;
        s.disabled = 1;
        s.hidden = 1;
        s.cursor = 2;
        let click = Box::into_raw(Box::new(KuiValue(Value::str("click"))));
        let drag = Box::into_raw(Box::new(KuiValue(Value::str("drag"))));
        let key = Box::into_raw(Box::new(KuiValue(Value::str("key"))));
        let hover = Box::into_raw(Box::new(KuiValue(Value::str("hover"))));
        let got = spec_of(&s, click, drag, key, hover);

        let mut want = NodeSpec::row()
            .width(Sizing::Grow(2.0))
            .height(Sizing::Fixed(30.0))
            .max_width(300.0)
            .padding(Edges {
                l: 1.0,
                r: 2.0,
                t: 3.0,
                b: 4.0,
            })
            .gap(5.0)
            .cross_gap(6.0)
            .main_align(align_of(1))
            .cross_align(align_of(2))
            .bg(Color::hex(0x102030ff))
            .radius(7.0)
            .shadow(kui_core::Shadow {
                color: Color::hex(0x00000080),
                dx: 0.0,
                dy: 2.0,
                blur: 8.0,
                spread: 0.0,
            })
            .hoverable()
            .accent()
            .animate()
            .pixel_snap()
            .keep_focus()
            .hover_bg(Color::hex(0x203040ff))
            .pressed_bg(Color::hex(0x304050ff))
            .cursor(kui_core::schema::cursor_idx(1))
            .role(role_of_code(1).unwrap())
            .checked(true)
            .selected(true)
            .focusable()
            .initial_focus()
            .disabled(true)
            .focus_bg(Color::hex(0x405060ff))
            .focus_ring(Color::hex(0x506070ff))
            .text_color(Color::hex(0x607080ff))
            .hidden(true)
            .on_click("click")
            .on_drag("drag")
            .on_key("key")
            .on_hover("hover");
        want.layout.min_w = Min::px(10.0);
        same(&got, &want);
    }
}
