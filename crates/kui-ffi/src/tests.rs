//! Headless tests of the entry points, over a standalone context.

use super::*;

/// A borrowed `KuiStr` over a Rust string, for the length of the call
/// that takes it — the one helper every module below wants.
fn ks(s: &str) -> KuiStr {
    KuiStr {
        ptr: s.as_ptr(),
        len: s.len(),
    }
}

#[cfg(test)]
mod widgets_headless {
    use super::*;

    extern "C" fn tab(_user: *mut c_void, ctx: *mut KuiCtx) {
        let style: KuiTextStyle = unsafe { std::mem::zeroed() };
        kui_text(ctx, ks("tab"), &style);
    }

    /// Every widget entry point builds through a standalone context, the
    /// body callbacks re-enter through the same pointer, and the editor
    /// created by kui_text_input reads back.
    #[test]
    fn widgets_build_and_draw() {
        let ctx = kui_ctx_new();
        assert!(!ctx.is_null());
        kui_frame_begin(ctx, 800.0, 600.0, 1.0);
        let root: KuiSpec = unsafe { std::mem::zeroed() };
        kui_root(ctx, &root);
        kui_titlebar_with(ctx, tab, std::ptr::null_mut());
        kui_titlebar(ctx, ks("plain"));
        kui_window_buttons(ctx);
        kui_latency_graph(ctx);
        kui_latency_hud(ctx, 2, 2);
        let mut badge: KuiSpec = unsafe { std::mem::zeroed() };
        badge.hoverable = 1;
        kui_open(ctx, &badge, NONE);
        kui_tooltip(ctx, ks("hint"));
        kui_tooltip_with(ctx, tab, std::ptr::null_mut());
        kui_close(ctx);
        let key = kui_text_input(ctx, ks("name"), ks("init"));
        assert_ne!(key, 0);
        kui_frame_finish(ctx);

        let mut text = KuiStr {
            ptr: std::ptr::null(),
            len: 0,
        };
        assert!(kui_edit_text(ctx, key, &mut text));
        assert_eq!(&*kstr(text), "init");

        let mut draw = KuiDrawData::default();
        kui_draw_data(ctx, &mut draw);
        assert!(draw.quad_count > 20, "got {} quads", draw.quad_count);
        kui_ctx_free(ctx);
    }

    /// `kui_select`: the field is keyed by its label, a
    /// click on it opens the core's menu of the rows under it — no event
    /// for the host — and a row chosen, as the host's own menu answers,
    /// is one `menu` event on the field naming the option.
    #[test]
    fn a_select_opens_the_menu_and_posts_the_choice() {
        let ctx = kui_ctx_new();
        let la = kui_value_str(ks("la"));
        let items = [
            KuiMenuItem {
                label: ks("English"),
                role: KUI_MENU_CUSTOM,
                enabled: 1,
                id: std::ptr::null(),
                accel: ks(""),
                checked: 0,
                submenu: std::ptr::null(),
                submenu_count: 0,
            },
            KuiMenuItem {
                label: ks("Latin"),
                role: KUI_MENU_CUSTOM,
                enabled: 1,
                id: la,
                accel: ks(""),
                checked: 0,
                submenu: std::ptr::null(),
                submenu_count: 0,
            },
        ];
        let build = |current: i64| {
            kui_frame_begin(ctx, 320.0, 240.0, 1.0);
            let mut root: KuiSpec = unsafe { std::mem::zeroed() };
            root.pad_l = 10.0;
            root.pad_r = 10.0;
            root.pad_t = 10.0;
            root.pad_b = 10.0;
            kui_root(ctx, &root);
            let key = kui_select(ctx, ks("language"), items.as_ptr(), items.len(), current);
            kui_frame_finish(ctx);
            key
        };
        let key = build(0);
        assert_ne!(key, 0);
        // A field the frame refused: no label, no rows.
        kui_frame_begin(ctx, 320.0, 240.0, 1.0);
        assert_eq!(kui_select(ctx, ks(""), items.as_ptr(), items.len(), 0), 0);
        assert_eq!(kui_select(ctx, ks("x"), items.as_ptr(), 0, 0), 0);
        kui_frame_finish(ctx);
        build(0);
        kui_input_cursor(ctx, 30.0, 20.0);
        kui_input_mouse(ctx, true, 1);
        kui_input_mouse(ctx, false, 1);
        let mut ev = KuiEvent::default();
        assert!(
            !kui_poll_event(ctx, &raw mut ev),
            "the field's click is the core's"
        );
        let (mut target, mut x, mut y) = (0u64, 0.0f32, 0.0f32);
        assert_eq!(kui_menu_item_count(ctx, &mut target, &mut x, &mut y), 2);
        assert_eq!(target, key, "the menu is about the field");
        assert_eq!((x, y), (10.0, 44.0), "under it");
        build(0);
        assert!(kui_activate_menu_item(ctx, 1));
        assert!(kui_poll_event(ctx, &raw mut ev));
        assert_eq!(ev.key, key);
        let payload = unsafe { &(*ev.payload).0 };
        assert_eq!(payload.get_str("kind"), Some("menu"));
        assert_eq!(payload.get_str("item"), Some("la"));
        assert!(!kui_poll_event(ctx, &raw mut ev), "one event, no more");
        assert_eq!(kui_menu_item_count(ctx, &mut target, &mut x, &mut y), 0);
        kui_value_free(la);
        kui_ctx_free(ctx);
    }

    /// The select's checks the core makes for every binding, through C:
    /// a disabled option reported chosen is refused —
    /// false, nothing posted, the menu still open — and a `current` past
    /// the end or on a separator is none, with one warning on the field.
    /// `count == 0` is refused at the door as it was: C's rows never pass
    /// the shared reader, so the check is the door's own.
    #[test]
    fn a_selects_disabled_option_is_refused_and_a_bad_current_is_warned() {
        let ctx = kui_ctx_new();
        kui_set_diagnostics(ctx, true);
        let la = kui_value_str(ks("la"));
        let row = |label: &'static str, role: u32, enabled: u32, id: *const KuiValue| KuiMenuItem {
            label: ks(label),
            role,
            enabled,
            id,
            accel: ks(""),
            checked: 0,
            submenu: std::ptr::null(),
            submenu_count: 0,
        };
        let items = [
            row("English", KUI_MENU_CUSTOM, 1, std::ptr::null()),
            row("", KUI_MENU_SEPARATOR, 1, std::ptr::null()),
            row("Latin", KUI_MENU_CUSTOM, 0, la),
        ];
        let build = |current: i64| {
            kui_frame_begin(ctx, 320.0, 240.0, 1.0);
            let mut root: KuiSpec = unsafe { std::mem::zeroed() };
            root.pad_l = 10.0;
            root.pad_t = 10.0;
            kui_root(ctx, &root);
            let key = kui_select(ctx, ks("language"), items.as_ptr(), items.len(), current);
            kui_frame_finish(ctx);
            key
        };
        let mut out = [KuiWarning {
            code: KuiStr {
                ptr: std::ptr::null(),
                len: 0,
            },
            key: 0,
            message: KuiStr {
                ptr: std::ptr::null(),
                len: 0,
            },
        }; 8];
        // Past the end, then on the separator: one line each, on the field.
        let key = build(9);
        let n = kui_take_warnings(ctx, out.as_mut_ptr(), out.len());
        assert_eq!(n, 1, "{:?}", kstr(out[0].message));
        assert_eq!(kstr(out[0].code), "select-current-ignored");
        assert_eq!(out[0].key, key);
        assert!(kstr(out[0].message).contains("the field has 3 options"));
        build(9);
        assert_eq!(
            kui_take_warnings(ctx, out.as_mut_ptr(), out.len()),
            0,
            "once"
        );
        let ctx2 = kui_ctx_new();
        kui_set_diagnostics(ctx2, true);
        kui_frame_begin(ctx2, 320.0, 240.0, 1.0);
        let root: KuiSpec = unsafe { std::mem::zeroed() };
        kui_root(ctx2, &root);
        kui_select(ctx2, ks("language"), items.as_ptr(), items.len(), 1);
        kui_frame_finish(ctx2);
        let n = kui_take_warnings(ctx2, out.as_mut_ptr(), out.len());
        assert_eq!(n, 1);
        assert!(kstr(out[0].message).contains("which is a separator"));
        kui_ctx_free(ctx2);
        // A disabled row through the door: refused, the menu still open;
        // the enabled one taken, and `-1` past the end taken as a close.
        build(0);
        kui_input_cursor(ctx, 30.0, 20.0);
        kui_input_mouse(ctx, true, 1);
        kui_input_mouse(ctx, false, 1);
        let (mut target, mut x, mut y) = (0u64, 0.0f32, 0.0f32);
        assert_eq!(kui_menu_item_count(ctx, &mut target, &mut x, &mut y), 3);
        build(0);
        assert!(!kui_activate_menu_item(ctx, 2), "disabled");
        assert!(!kui_activate_menu_item(ctx, 1), "a separator");
        let mut ev = KuiEvent::default();
        assert!(!kui_poll_event(ctx, &raw mut ev), "nothing posted");
        assert_eq!(
            kui_menu_item_count(ctx, &mut target, &mut x, &mut y),
            3,
            "still open"
        );
        assert!(kui_activate_menu_item(ctx, 0));
        assert!(kui_poll_event(ctx, &raw mut ev));
        assert_eq!(ev.key, key);
        assert_eq!(kui_menu_item_count(ctx, &mut target, &mut x, &mut y), 0);
        assert!(!kui_activate_menu_item(ctx, 0), "no menu open");
        assert_eq!(kui_take_warnings(ctx, out.as_mut_ptr(), out.len()), 0);
        kui_value_free(la);
        kui_ctx_free(ctx);
    }

    /// `dir = KUI_TABLE`: the rows' cells line up, each
    /// column as wide as its widest cell, a grow cell growing its column,
    /// a bare text a cell too. Read back through `kui_layout_of` on the
    /// cells that declared `on_layout`; and `kui_nodes` says which node
    /// is the table — `table: true` on a `dir: "column"` map, the same
    /// `NodeInfo` row Node and Lua read (the map is the core's `to_value`,
    /// so C needs no reader of its own).
    #[test]
    fn a_table_lines_its_rows_cells_up() {
        let ctx = kui_ctx_new();
        let tag = KuiValue(Value::str("cell"));
        kui_set_inspect(ctx, true);
        kui_frame_begin(ctx, 300.0, 200.0, 1.0);
        let root: KuiSpec = unsafe { std::mem::zeroed() };
        kui_root(ctx, &root);
        let mut table: KuiSpec = unsafe { std::mem::zeroed() };
        table.dir = 2; // KUI_TABLE
        table.width = KuiSizing {
            tag: 2,
            value: 300.0,
        };
        kui_open(ctx, &table, NONE);
        let mut keys = Vec::new();
        for (label, w) in [("ab", 10.0), ("abcdef", 50.0)] {
            let mut row: KuiSpec = unsafe { std::mem::zeroed() };
            row.dir = 1;
            row.width = KuiSizing { tag: 1, value: 1.0 };
            row.gap = 8.0;
            kui_open(ctx, &row, NONE);
            let mut style: KuiTextStyle = unsafe { std::mem::zeroed() };
            style.size = 12.0;
            kui_text(ctx, ks(label), &style);
            let mut fixed: KuiSpec = unsafe { std::mem::zeroed() };
            fixed.width = KuiSizing { tag: 2, value: w };
            fixed.height = KuiSizing {
                tag: 2,
                value: 10.0,
            };
            fixed.on_layout = &tag;
            let b = kui_open(ctx, &fixed, NONE);
            kui_close(ctx);
            let mut grow: KuiSpec = unsafe { std::mem::zeroed() };
            grow.width = KuiSizing { tag: 1, value: 1.0 };
            grow.height = KuiSizing {
                tag: 2,
                value: 10.0,
            };
            grow.on_layout = &tag;
            let c = kui_open(ctx, &grow, NONE);
            kui_close(ctx);
            kui_close(ctx);
            keys.push((b, c));
        }
        kui_close(ctx);
        kui_frame_finish(ctx);
        let rect = |key: u64| {
            let mut r = KuiLayoutRect::default();
            assert!(kui_layout_of(ctx, key, &mut r));
            r
        };
        let (b1, c1) = (rect(keys[0].0), rect(keys[0].1));
        let (b2, c2) = (rect(keys[1].0), rect(keys[1].1));
        assert_eq!(
            b1.x, b2.x,
            "the fixed column starts after the longest label"
        );
        assert!(b1.x > 8.0, "past a label and the gap: {}", b1.x);
        assert_eq!(
            (b1.w, b2.w),
            (50.0, 50.0),
            "the fixed column is its widest cell"
        );
        assert_eq!(c1.x, c2.x);
        assert_eq!(c1.w, c2.w, "the grow column is one width in both rows");
        assert_eq!(c1.x + c1.w, 300.0, "and it takes the rest");
        let empty = KuiStr {
            ptr: std::ptr::null(),
            len: 0,
        };
        let mut out = [KuiWarning {
            code: empty,
            key: 0,
            message: empty,
        }; 4];
        assert_eq!(kui_take_warnings(ctx, out.as_mut_ptr(), out.len()), 0);
        // The node list names the table: one map with `table` true, its
        // `dir` a column's; its rows and cells say false.
        let list = kui_nodes(ctx);
        let n = kui_value_len(list);
        let mut tables = 0;
        let mut rows = 0;
        for i in 0..n {
            let node = kui_value_at(list, i);
            let mut is_table = false;
            assert!(
                kui_value_as_bool(kui_value_get(node, ks("table")), &mut is_table),
                "every node map carries `table`"
            );
            let mut dir = empty;
            assert!(kui_value_as_str(kui_value_get(node, ks("dir")), &mut dir));
            if is_table {
                tables += 1;
                assert_eq!(&*kstr(dir), "column", "a table is a column");
            } else if &*kstr(dir) == "row" {
                rows += 1;
            }
        }
        assert_eq!(tables, 1, "one table among {n} nodes");
        assert_eq!(rows, 2, "its rows are rows, not tables");
        kui_ctx_free(ctx);
    }

    /// The metrics cross the boundary both ways: the stock
    /// set reads back as the constants, a set the host wrote is what the
    /// next button is built from, NULL restores the stock set, and a short
    /// reservation is refused as every `[out]` struct's is.
    #[test]
    fn metrics_cross_the_boundary_both_ways() {
        let ctx = kui_ctx_new();
        let mut m = KuiMetrics {
            radius: 0.0,
            ..Default::default()
        };
        assert!(kui_metrics(ctx, &mut m));
        assert_eq!(m.radius, 6.0);
        assert_eq!(m.control_text, 15.0);
        assert_eq!(m.menu_width, 200.0);

        let button_h = |ctx: *mut KuiCtx| {
            kui_frame_begin(ctx, 800.0, 600.0, 1.0);
            let root: KuiSpec = unsafe { std::mem::zeroed() };
            kui_root(ctx, &root);
            kui_button(ctx, ks("OK"), std::ptr::null_mut());
            kui_frame_finish(ctx);
            let mut draw = KuiDrawData::default();
            kui_draw_data(ctx, &mut draw);
            unsafe { (*draw.quads).h }
        };
        let stock = button_h(ctx);
        m.control_pad_y = 2.0;
        m.control_text = 10.0;
        kui_metrics_set(ctx, &m);
        let mut back = KuiMetrics::default();
        assert!(kui_metrics(ctx, &mut back));
        assert_eq!(back.control_pad_y, 2.0);
        assert!(button_h(ctx) < stock);
        kui_metrics_set(ctx, std::ptr::null());
        assert_eq!(button_h(ctx), stock);

        let mut short = KuiMetrics {
            size: 4,
            ..Default::default()
        };
        assert!(!kui_metrics(ctx, &mut short));
        kui_ctx_free(ctx);
    }

    /// Tokens cross as two `[in]` arrays and read back by name:
    /// a themed colour answers the dark half on an unknown appearance, a
    /// role's name answers the role, an unknown or wrong-kind name is
    /// false with `unknown-token` raised once, a role's name in a
    /// declaration is refused with `reserved-token`, and NULL arrays with
    /// zero counts clear the table.
    #[test]
    fn tokens_cross_the_boundary_both_ways() {
        let ctx = kui_ctx_new();
        kui_set_diagnostics(ctx, true);
        let colors = [
            KuiColorToken {
                name: ks("peach"),
                light: 0xffcc99ff,
                dark: 0xffcc99ff,
            },
            KuiColorToken {
                name: ks("ink"),
                light: 0x111111ff,
                dark: 0xeeeeeeff,
            },
            KuiColorToken {
                name: ks("surface"),
                light: 0xff0000ff,
                dark: 0xff0000ff,
            },
        ];
        let lengths = [KuiLengthToken {
            name: ks("side_w"),
            value: 132.0,
        }];
        kui_tokens_set(
            ctx,
            colors.as_ptr(),
            colors.len(),
            lengths.as_ptr(),
            lengths.len(),
        );
        let mut c = 0u32;
        assert!(kui_token_color(ctx, ks("peach"), &mut c));
        assert_eq!(c, 0xffcc99ff);
        assert!(kui_token_color(ctx, ks("ink"), &mut c));
        assert_eq!(c, 0xeeeeeeff, "the dark half on an unknown appearance");
        let mut theme = KuiTheme::default();
        assert!(kui_theme(ctx, &mut theme));
        assert!(kui_token_color(ctx, ks("surface"), &mut c));
        assert_eq!(c, theme.surface, "the role's, not the refused red");
        let mut v = 0.0f32;
        assert!(kui_token_length(ctx, ks("side_w"), &mut v));
        assert_eq!(v, 132.0);
        assert!(kui_token_length(ctx, ks("radius"), &mut v));
        let mut m = KuiMetrics::default();
        assert!(kui_metrics(ctx, &mut m));
        assert_eq!(v, m.radius);
        assert!(!kui_token_color(ctx, ks("peech"), &mut c));
        assert!(
            !kui_token_color(ctx, ks("peech"), &mut c),
            "twice, warned once"
        );
        assert!(
            !kui_token_length(ctx, ks("peach"), &mut v),
            "a colour is not a length"
        );
        assert!(!kui_token_color(ctx, ks("peach"), std::ptr::null_mut()));

        let mut out = [KuiWarning {
            code: KuiStr {
                ptr: std::ptr::null(),
                len: 0,
            },
            key: 0,
            message: KuiStr {
                ptr: std::ptr::null(),
                len: 0,
            },
        }; 8];
        let n = kui_take_warnings(ctx, out.as_mut_ptr(), out.len());
        let mut codes: Vec<String> = out[..n].iter().map(|w| kstr(w.code).into_owned()).collect();
        codes.sort();
        assert_eq!(codes, ["reserved-token", "unknown-token", "unknown-token"]);

        kui_tokens_set(ctx, std::ptr::null(), 0, std::ptr::null(), 0);
        assert!(!kui_token_color(ctx, ks("peach"), &mut c), "cleared");
        kui_ctx_free(ctx);
    }

    /// Derived tokens through C: a chain folds over a declared
    /// token, a role is a source, the value read back is the rounded one
    /// every binding paints, a missing source drops that token alone with
    /// `unknown-token`, and a malformed op refuses the whole call with
    /// nothing added.
    #[test]
    fn a_derived_token_crosses_and_a_malformed_op_is_refused() {
        let ctx = kui_ctx_new();
        kui_set_diagnostics(ctx, true);
        let colors = [KuiColorToken {
            name: ks("peach"),
            light: 0xffcc99ff,
            dark: 0xffcc99ff,
        }];
        kui_tokens_set(ctx, colors.as_ptr(), 1, std::ptr::null(), 0);
        let none = KuiStr {
            ptr: std::ptr::null(),
            len: 0,
        };
        let wash = [
            KuiColorOp {
                op: KUI_OP_LIFT,
                t: 0.5,
                other: none,
            },
            KuiColorOp {
                op: KUI_OP_ALPHA,
                t: 0.5,
                other: none,
            },
        ];
        let up = [KuiColorOp {
            op: KUI_OP_RAISE,
            t: 0.25,
            other: none,
        }];
        let toward = [KuiColorOp {
            op: KUI_OP_MIX,
            t: 1.0,
            other: ks("peach"),
        }];
        let derived = [
            KuiDerivedToken {
                name: ks("wash"),
                from: ks("peach"),
                ops: wash.as_ptr(),
                op_count: 2,
            },
            KuiDerivedToken {
                name: ks("up"),
                from: ks("surface"),
                ops: up.as_ptr(),
                op_count: 1,
            },
            KuiDerivedToken {
                name: ks("bad"),
                from: ks("nothing"),
                ops: std::ptr::null(),
                op_count: 0,
            },
            KuiDerivedToken {
                name: ks("all_the_way"),
                from: ks("up"),
                ops: toward.as_ptr(),
                op_count: 1,
            },
        ];
        assert!(kui_tokens_derive(ctx, derived.as_ptr(), derived.len()));
        let mut c = 0u32;
        assert!(kui_token_color(ctx, ks("wash"), &mut c));
        assert_eq!(c, 0xffe6cc80);
        let mut theme = KuiTheme::default();
        assert!(kui_theme(ctx, &mut theme));
        assert!(kui_token_color(ctx, ks("up"), &mut c));
        let raised = kui_core::Color::hex(theme.surface).mix(kui_core::Color::WHITE, 0.25);
        assert_eq!(
            c,
            raised.to_hex(),
            "a role source, raised toward the dark base's front"
        );
        assert!(kui_token_color(ctx, ks("all_the_way"), &mut c));
        assert_eq!(
            c, 0xffcc99ff,
            "mixed all the way to peach, from a derived source"
        );
        assert!(
            !kui_token_color(ctx, ks("bad"), &mut c),
            "dropped at declaration"
        );
        assert!(
            kui_token_color(ctx, ks("peach"), &mut c),
            "the values stayed"
        );

        // A malformed op: the call is refused and the table is as it was.
        let glow = [KuiColorOp {
            op: 9,
            t: 0.5,
            other: none,
        }];
        let no_other = [KuiColorOp {
            op: KUI_OP_MIX,
            t: 0.5,
            other: none,
        }];
        let stray_other = [KuiColorOp {
            op: KUI_OP_LIFT,
            t: 0.5,
            other: ks("peach"),
        }];
        for ops in [&glow[..], &no_other[..], &stray_other[..]] {
            let d = [KuiDerivedToken {
                name: ks("x"),
                from: ks("peach"),
                ops: ops.as_ptr(),
                op_count: 1,
            }];
            assert!(!kui_tokens_derive(ctx, d.as_ptr(), 1));
            assert!(!kui_token_color(ctx, ks("x"), &mut c));
        }
        assert!(
            kui_tokens_derive(ctx, std::ptr::null(), 0),
            "nothing to add is fine"
        );

        let mut out = [KuiWarning {
            code: KuiStr {
                ptr: std::ptr::null(),
                len: 0,
            },
            key: 0,
            message: KuiStr {
                ptr: std::ptr::null(),
                len: 0,
            },
        }; 8];
        let n = kui_take_warnings(ctx, out.as_mut_ptr(), out.len());
        let named: Vec<(String, String)> = out[..n]
            .iter()
            .map(|w| (kstr(w.code).into_owned(), kstr(w.message).into_owned()))
            .collect();
        assert!(
            named.iter().any(|(c, m)| c == "unknown-token"
                && m.contains("`$bad`")
                && m.contains("`$nothing`")),
            "{named:?}"
        );
        kui_ctx_free(ctx);
    }
}

#[cfg(test)]
mod window_commands_headless {
    use super::*;

    /// `MAIN`, which the header spells for C.
    const MAIN: u32 = WindowId::MAIN.0;

    fn drain(ctx: *mut KuiCtx) -> Vec<(u32, u32, f32, f32)> {
        let mut out = Vec::new();
        let mut cmd = KuiWindowCommand::default();
        while kui_take_window_command(ctx, &raw mut cmd) {
            out.push((cmd.kind, cmd.window, cmd.width, cmd.height));
        }
        out
    }

    /// A size request carries its size through the drain, and a focus
    /// request the window it names; both leave in the order they were
    /// queued, behind whatever the chrome produced, and once.
    #[test]
    fn size_and_focus_requests_drain_with_their_payload() {
        let ctx = kui_ctx_new();
        kui_set_window_size(ctx, MAIN, 640.0, 480.0);
        kui_focus_window(ctx, MAIN);
        assert_eq!(
            drain(ctx),
            vec![
                (KUI_CMD_SET_SIZE, MAIN, 640.0, 480.0),
                (KUI_CMD_FOCUS, MAIN, 0.0, 0.0),
            ]
        );
        assert!(drain(ctx).is_empty(), "drained once");
        kui_ctx_free(ctx);
    }

    /// A kind this build does not have still opens a window — a host built
    /// against a later header degrades to a window rather than to nothing —
    /// but it says so. C is the only binding that can name a kind at all:
    /// `windows` in JSX and Lua has no `kind` key.
    #[test]
    fn an_unknown_window_kind_opens_a_normal_window_and_warns() {
        let ctx = kui_ctx_new();
        kui_set_diagnostics(ctx, true);
        let cfg = KuiWindowConfig {
            kind: KUI_WINDOW_KIND_POPUP + 1,
            width: 320.0,
            height: 240.0,
            activates: 1,
            ..Default::default()
        };
        kui_frame_begin(ctx, 300.0, 100.0, 1.0);
        kui_window_declare(ctx, ks("palette"), &cfg);
        kui_frame_finish(ctx);

        let opened: Vec<_> = drain(ctx)
            .into_iter()
            .filter(|c| c.0 == KUI_CMD_OPEN)
            .collect();
        assert_eq!(opened.len(), 1, "it still opens");

        let mut out = [KuiWarning {
            code: KuiStr {
                ptr: std::ptr::null(),
                len: 0,
            },
            key: 0,
            message: KuiStr {
                ptr: std::ptr::null(),
                len: 0,
            },
        }; 4];
        let n = kui_take_warnings(ctx, out.as_mut_ptr(), out.len());
        assert_eq!(n, 1, "and the normalisation is not silent");
        assert_eq!(&*kstr(out[0].code), "unknown-window-kind");
        assert!(kstr(out[0].message).contains("palette"));
        kui_ctx_free(ctx);
    }

    /// Every kind the header does spell passes without a line.
    #[test]
    fn the_kinds_the_header_has_do_not_warn() {
        let ctx = kui_ctx_new();
        kui_set_diagnostics(ctx, true);
        let cfg = KuiWindowConfig {
            kind: KUI_WINDOW_KIND_NORMAL,
            ..Default::default()
        };
        let popup = KuiWindowConfig {
            kind: KUI_WINDOW_KIND_POPUP,
            width: 160.0,
            height: 320.0,
            anchor_x: 12.0,
            anchor_y: 40.0,
            anchor_w: 160.0,
            anchor_h: 24.0,
            ..Default::default()
        };
        kui_frame_begin(ctx, 300.0, 100.0, 1.0);
        kui_window_declare(ctx, ks("palette"), &cfg);
        kui_window_declare(ctx, ks("menu"), &popup);
        kui_window_declare(ctx, ks("tools"), std::ptr::null());
        kui_frame_finish(ctx);
        let mut out = [KuiWarning {
            code: KuiStr {
                ptr: std::ptr::null(),
                len: 0,
            },
            key: 0,
            message: KuiStr {
                ptr: std::ptr::null(),
                len: 0,
            },
        }; 4];
        assert_eq!(kui_take_warnings(ctx, out.as_mut_ptr(), out.len()), 0);
        kui_ctx_free(ctx);
    }

    /// An append past `config` is the compatible kind: a host that reserved
    /// only through `config` still drains, still reads the verb and the
    /// window, and simply never sees the size — which it cannot need, since
    /// only its own `kui_set_window_size` produces the verb that fills it.
    ///
    /// Written for ABI 6's `width`/`height` against an ABI-5 host, and it
    /// asserts the same thing for ABI 7's `owner`; what it can no longer
    /// say is "every ABI-5 build", because ABI 7 grew `KuiWindowConfig`
    /// itself and so moved this floor — see the test below.
    #[test]
    fn a_host_that_reserved_through_config_drains_without_the_appended_tail() {
        let ctx = kui_ctx_new();
        kui_set_window_size(ctx, MAIN, 640.0, 480.0);
        let mut cmd = KuiWindowCommand {
            size: abi_through!(KuiWindowCommand, config, KuiWindowConfig),
            width: 12.5,
            ..Default::default()
        };
        assert!(kui_take_window_command(ctx, &raw mut cmd));
        assert_eq!((cmd.kind, cmd.window), (KUI_CMD_SET_SIZE, MAIN));
        assert_eq!(
            cmd.size,
            KuiWindowCommand::ABI_V1_SIZE,
            "the prefix it asked for"
        );
        assert_eq!(cmd.width, 12.5, "and nothing written past it");
        kui_ctx_free(ctx);
    }

    /// The one growth the size handshake cannot absorb, asserted rather
    /// than only described (`abi.rs`'s note on ABI 7).
    /// `KuiWindowCommand` embeds a `KuiWindowConfig` **by value**, so the
    /// four `anchor_*` floats appended to the config moved every field
    /// after it and lifted this struct's floor past the whole size of the
    /// ABI-6 struct. An ABI-6 host is therefore refused — not
    /// short-written, which is the handshake working — and its drain loop
    /// sees an empty queue. `kui_abi_version()` is the only thing that
    /// turns that into a message, which is why this test exists next to it.
    #[test]
    fn an_abi_6_reservation_is_refused_because_the_config_grew_inside() {
        const ABI_6_SIZE: u32 = 40;
        // Constant on both sides, deliberately: the number 40 is what an
        // ABI-6 header laid out, and nothing in this build can recompute
        // it, so it is written down and compared.
        #[allow(
            clippy::assertions_on_constants,
            reason = "the constant is the assertion"
        )]
        {
            assert!(
                KuiWindowCommand::ABI_V1_SIZE > ABI_6_SIZE,
                "the floor moved past the whole ABI-6 struct"
            );
        }
        let ctx = kui_ctx_new();
        kui_set_window_size(ctx, MAIN, 640.0, 480.0);
        let mut cmd = KuiWindowCommand {
            size: ABI_6_SIZE,
            kind: 0xdead,
            ..Default::default()
        };
        assert!(!kui_take_window_command(ctx, &raw mut cmd));
        assert_eq!(cmd.kind, 0xdead, "nothing was written");
        // And the command is still there for a host that recompiled.
        let mut cmd = KuiWindowCommand::default();
        assert!(kui_take_window_command(ctx, &raw mut cmd));
        assert_eq!(cmd.kind, KUI_CMD_SET_SIZE);
        kui_ctx_free(ctx);
    }

    /// Popups through the C surface: the kind and the anchor
    /// ride the declaration out to the `KUI_CMD_OPEN` untouched, `owner`
    /// names the window whose frame declared it, and a dismissal reported
    /// by the host is an event and nothing else — no command, and the
    /// window still open and still declared.
    #[test]
    fn a_popup_declaration_and_its_dismissal_cross_the_c_surface() {
        let ctx = kui_ctx_new();
        let popup = KuiWindowConfig {
            kind: KUI_WINDOW_KIND_POPUP,
            width: 160.0,
            height: 320.0,
            activates: 0,
            anchor_x: 12.0,
            anchor_y: 40.0,
            anchor_w: 160.0,
            anchor_h: 24.0,
        };
        kui_frame_begin(ctx, 300.0, 100.0, 1.0);
        kui_window_declare(ctx, ks("menu"), &popup);
        kui_frame_finish(ctx);

        let mut cmd = KuiWindowCommand::default();
        assert!(kui_take_window_command(ctx, &raw mut cmd));
        assert_eq!((cmd.kind, cmd.window, cmd.owner), (KUI_CMD_OPEN, 1, MAIN));
        assert_eq!(cmd.config.kind, KUI_WINDOW_KIND_POPUP);
        assert_eq!(cmd.config.activates, 0);
        assert_eq!(
            (
                cmd.config.anchor_x,
                cmd.config.anchor_y,
                cmd.config.anchor_w,
                cmd.config.anchor_h
            ),
            (12.0, 40.0, 160.0, 24.0)
        );
        assert!(!kui_take_window_command(ctx, &raw mut cmd));

        // The frame's own `{kind:"window", phase:"opened"}` comes first.
        let mut ev = KuiEvent::default();
        assert!(kui_poll_event(ctx, &raw mut ev));
        assert_eq!(unsafe { &(*ev.payload).0 }.get_str("kind"), Some("window"));
        assert!(!kui_poll_event(ctx, &raw mut ev));

        kui_window_dismissed(ctx, 1, KUI_DISMISS_ESCAPE);
        assert!(kui_poll_event(ctx, &raw mut ev));
        let payload = unsafe { &(*ev.payload).0 };
        assert_eq!(payload.get_str("kind"), Some("dismiss"));
        assert_eq!(payload.get_str("reason"), Some("escape"));
        assert_eq!(payload.get_str("name"), Some("menu"));
        assert!(!kui_poll_event(ctx, &raw mut ev), "one event, no more");
        assert!(
            !kui_take_window_command(ctx, &raw mut cmd),
            "and nothing closed: only the app can stop declaring it"
        );
        kui_ctx_free(ctx);
    }
}

#[cfg(test)]
mod audio_headless {
    use super::*;

    /// A click on a `click_sound` node and an audio node both surface as
    /// commands through the C drain; an ended tagged playback polls out as
    /// a `sound` event.
    #[test]
    fn sounds_flow_through_the_c_api() {
        let ctx = kui_ctx_new();
        let wav = b"RIFF....WAVE";
        let sound = kui_sound_add(ctx, wav.as_ptr(), wav.len());
        assert_ne!(sound, 0);

        let mut spec = unsafe { std::mem::zeroed::<KuiSpec>() };
        spec.width = KuiSizing {
            tag: 2,
            value: 40.0,
        };
        spec.height = KuiSizing {
            tag: 2,
            value: 20.0,
        };
        spec.click_sound = sound;
        let audio = KuiAudio {
            src: sound,
            volume: 0.5,
            looped: 1,
            paused: 0,
            finish: 0,
        };
        let frame = |ctx: *mut KuiCtx| {
            kui_frame_begin(ctx, 200.0, 100.0, 1.0);
            kui_open(ctx, &spec, NONE);
            kui_close(ctx);
            kui_audio(ctx, KUI_EMPTY, &audio, kui_value_str(KUI_STR_TEST));
            kui_frame_finish(ctx);
        };
        frame(ctx);
        let mut out = [KuiAudioCommand::default(); 8];
        let n = kui_take_audio_commands(ctx, out.as_mut_ptr(), out.len());
        assert_eq!(n, 1, "the audio node started once");
        assert_eq!((out[0].kind, out[0].sound, out[0].looped), (1, sound, 1));
        assert_eq!(out[0].volume, 0.5);
        let music = out[0].playback;

        kui_input_cursor(ctx, 5.0, 5.0);
        kui_input_mouse(ctx, true, 1);
        kui_input_mouse(ctx, false, 1);
        let n = kui_take_audio_commands(ctx, out.as_mut_ptr(), out.len());
        assert_eq!(n, 1, "the click played its sound");
        assert_eq!((out[0].kind, out[0].sound), (1, sound));

        // Re-declaring is silent; the driver reporting the music ended
        // surfaces the tag as an event.
        frame(ctx);
        assert_eq!(kui_take_audio_commands(ctx, out.as_mut_ptr(), out.len()), 0);
        kui_audio_ended(ctx, music);
        let mut ev = KuiEvent::default();
        assert!(kui_poll_event(ctx, &mut ev));
        let payload = unsafe { &*ev.payload };
        assert_eq!(payload.0.get_str("kind"), Some("sound"));
        assert_eq!(payload.0.get_str("tag"), Some("music"));
        assert_eq!(payload.0.get_int("playback"), Some(music as i64));
        kui_ctx_free(ctx);
    }

    const KUI_EMPTY: KuiStr = KuiStr {
        ptr: std::ptr::null(),
        len: 0,
    };
    const KUI_STR_TEST: KuiStr = KuiStr {
        ptr: "music".as_ptr(),
        len: 5,
    };

    /// The two answers a host with its own device owes besides `ended`,
    /// which only the runner could give before: a stop it
    /// found still playing names the one-shot node that went away in
    /// `truncated-playback`, and a play its device refused is a `refused`
    /// sound event plus `playback-refused` on the node — the same
    /// warnings the runner raises, through the C door.
    #[test]
    fn a_c_host_s_device_reports_a_truncation_and_a_refusal() {
        let ctx = kui_ctx_new();
        kui_set_diagnostics(ctx, true);
        let wav = b"RIFF....WAVE";
        let sound = kui_sound_add(ctx, wav.as_ptr(), wav.len());
        let one_shot = KuiAudio {
            src: sound,
            volume: 1.0,
            looped: 0,
            paused: 0,
            finish: 0,
        };
        let frame = |ctx: *mut KuiCtx, declare: bool| -> u64 {
            kui_frame_begin(ctx, 200.0, 100.0, 1.0);
            let node = if declare {
                kui_audio(ctx, ks("jingle"), &one_shot, kui_value_str(ks("jingle")))
            } else {
                0
            };
            kui_frame_finish(ctx);
            node
        };
        let mut out = [KuiAudioCommand::default(); 8];
        let mut warnings = [KuiWarning {
            code: ks(""),
            key: 0,
            message: ks(""),
        }; 8];
        let codes = |ctx: *mut KuiCtx, warnings: &mut [KuiWarning; 8]| -> Vec<String> {
            let n = kui_take_warnings(ctx, warnings.as_mut_ptr(), warnings.len());
            warnings[..n]
                .iter()
                .map(|w| kstr(w.code).into_owned())
                .collect()
        };

        // Declared once, gone the next frame: a stop the device answers
        // for as "still running" is the truncation warning, on the node.
        let node = frame(ctx, true);
        assert_ne!(node, 0);
        assert_eq!(kui_take_audio_commands(ctx, out.as_mut_ptr(), out.len()), 1);
        let playback = out[0].playback;
        frame(ctx, false);
        assert_eq!(kui_take_audio_commands(ctx, out.as_mut_ptr(), out.len()), 1);
        assert_eq!(out[0].kind, 2, "a stop");
        assert!(
            codes(ctx, &mut warnings).is_empty(),
            "no device has answered"
        );
        kui_audio_truncated(ctx, playback, 0.5);
        assert_eq!(codes(ctx, &mut warnings), ["truncated-playback"]);
        assert_eq!(warnings[0].key, node);
        assert!(kstr(warnings[0].message).contains("0.50s"));

        // Declared again, and this time the device will not take it: the
        // tag comes back as a refused sound event, the node is warned.
        let node = frame(ctx, true);
        assert_eq!(kui_take_audio_commands(ctx, out.as_mut_ptr(), out.len()), 1);
        let playback = out[0].playback;
        kui_audio_refused(ctx, playback);
        let mut ev = KuiEvent::default();
        assert!(kui_poll_event(ctx, &mut ev));
        let payload = unsafe { &*ev.payload };
        assert_eq!(payload.0.get_str("kind"), Some("sound"));
        assert_eq!(payload.0.get_str("phase"), Some("refused"));
        assert_eq!(payload.0.get_str("tag"), Some("jingle"));
        assert!(!kui_poll_event(ctx, &mut ev), "one event");
        assert_eq!(codes(ctx, &mut warnings), ["playback-refused"]);
        assert_eq!(warnings[0].key, node);
        kui_ctx_free(ctx);
    }
}

#[cfg(test)]
mod queries_headless {
    use super::*;

    /// A host that never saw an event from a node names it by the label it
    /// opened it under: `kui_key_of` hands back the key the
    /// build gave, through the auto-keyed ancestors the host cannot
    /// spell, and `kui_focus` takes it from there.
    #[test]
    fn a_label_resolves_to_the_key_the_build_gave_it() {
        let ctx = kui_ctx_new();
        kui_set_diagnostics(ctx, true);
        let mut spec = unsafe { std::mem::zeroed::<KuiSpec>() };
        spec.width = KuiSizing {
            tag: 2,
            value: 100.0,
        };
        spec.height = KuiSizing {
            tag: 2,
            value: 50.0,
        };
        spec.focusable = 1;
        let plain = unsafe { std::mem::zeroed::<KuiSpec>() };
        let build = |ctx: *mut KuiCtx, twice: bool| -> (u64, u64) {
            kui_frame_begin(ctx, 200.0, 100.0, 1.0);
            kui_open(ctx, &plain, NONE); // auto-keyed
            let first = kui_open_with(ctx, ks("item"), &spec, NONE, NONE, NONE, NONE);
            kui_close(ctx);
            kui_close(ctx);
            let mut second = 0;
            if twice {
                kui_open(ctx, &plain, NONE);
                second = kui_open_with(ctx, ks("item"), &spec, NONE, NONE, NONE, NONE);
                kui_close(ctx);
                kui_close(ctx);
            }
            kui_frame_finish(ctx);
            (first, second)
        };
        let (first, _) = build(ctx, false);
        assert_ne!(first, 0);
        assert_eq!(kui_key_of(ctx, ks("item")), first);
        assert_eq!(kui_key_of(ctx, ks("nope")), 0, "an undeclared label is 0");
        kui_focus(ctx, kui_key_of(ctx, ks("item")));
        assert!(kui_is_focused(ctx, first), "focused with no event from it");

        // Two nodes on one label under different parents: the first in
        // tree order, and the frame says so once.
        let (first, second) = build(ctx, true);
        assert_ne!(first, second);
        assert_eq!(kui_key_of(ctx, ks("item")), first);
        let mut out = [KuiWarning {
            code: KuiStr {
                ptr: std::ptr::null(),
                len: 0,
            },
            key: 0,
            message: KuiStr {
                ptr: std::ptr::null(),
                len: 0,
            },
        }; 4];
        let n = kui_take_warnings(ctx, out.as_mut_ptr(), out.len());
        assert_eq!(n, 1, "one ambiguous-key line");
        assert_eq!(&*kstr(out[0].code), "ambiguous-key");
        assert_eq!(out[0].key, first);
        kui_ctx_free(ctx);
    }

    /// Raw keys cross as strings: a C host drives an `on_key` sink with
    /// `kui_input_key_down` / `_up`, gets both halves back as one
    /// `{kind="key"}` payload apart by `phase`, and lets go of what is
    /// held when its window loses the keyboard.
    #[test]
    fn raw_keys_and_their_releases_cross_the_boundary() {
        let ctx = kui_ctx_new();
        let mut spec = unsafe { std::mem::zeroed::<KuiSpec>() };
        spec.width = KuiSizing {
            tag: 2,
            value: 100.0,
        };
        spec.height = KuiSizing {
            tag: 2,
            value: 50.0,
        };
        // Releases are opt-in: without this the sink hears presses only.
        spec.key_up = 1;
        kui_frame_begin(ctx, 200.0, 100.0, 1.0);
        let sink = kui_open_with(
            ctx,
            ks("sink"),
            &spec,
            NONE,
            NONE,
            kui_value_str(ks("keys")),
            NONE,
        );
        kui_close(ctx);
        kui_set_key_focus(ctx, sink);
        kui_frame_finish(ctx);

        let null = KuiStr {
            ptr: std::ptr::null(),
            len: 0,
        };
        // A NULL text means "whatever this key inserts", and a NULL
        // physical means "the key I just named".
        kui_input_key_down(ctx, ks("w"), null, 0, null, false);
        kui_input_key_down(ctx, ks("w"), null, 0, null, true);
        kui_input_key_up(ctx, ks("w"), null, 0);
        // Held when the window loses the keyboard: the release is made up
        // by the report itself, the way the windowed runner's is, so the
        // host owes no second call. The bools on the payload read as
        // bools: `ctrl` is the modifier the press carried.
        kui_input_key_down(ctx, ks("f5"), null, KUI_KMOD_CTRL, null, false);
        kui_env_set(ctx, 60.0, false);
        // An unknown name is ignored rather than delivered as "unknown".
        kui_input_key_down(ctx, ks("nonsense"), null, 0, null, false);

        let mut ev = KuiEvent::default();
        let mut seen = Vec::new();
        let mut window = Vec::new();
        while kui_poll_event(ctx, &mut ev) {
            let get = |k: &str| {
                let v = kui_value_get(ev.payload, ks(k));
                let mut out = KuiStr {
                    ptr: std::ptr::null(),
                    len: 0,
                };
                kui_value_as_str(v, &mut out).then(|| kstr(out).into_owned())
            };
            // Losing the keyboard is the window's event too (DX18).
            if get("kind").as_deref() == Some("window") {
                window.push(get("phase").unwrap_or_default());
                continue;
            }
            assert_eq!(ev.key, sink);
            assert_eq!(get("kind").as_deref(), Some("key"));
            assert_eq!(get("tag").as_deref(), Some("keys"));
            let mut ctrl = false;
            assert!(kui_value_as_bool(
                kui_value_get(ev.payload, ks("ctrl")),
                &mut ctrl
            ));
            let mut repeat = false;
            assert!(kui_value_as_bool(
                kui_value_get(ev.payload, ks("repeat")),
                &mut repeat
            ));
            seen.push((
                get("phase").unwrap_or_default(),
                get("code").unwrap_or_default(),
                get("text"),
                ctrl,
                repeat,
            ));
        }
        assert_eq!(
            seen,
            [
                ("down".into(), "w".into(), Some("w".into()), false, false),
                ("down".into(), "w".into(), Some("w".into()), false, true),
                ("up".into(), "w".into(), None, false, false),
                ("down".into(), "f5".into(), None, true, false),
                ("up".into(), "f5".into(), None, true, false),
            ]
        );
        assert_eq!(window, ["blurred"]);
        kui_ctx_free(ctx);
    }

    /// Where a key is and what the locks hold cross in the same `kmods`
    /// word as the modifiers (`KUI_KLOC_*`, `KUI_KLOCK_*`),
    /// and a sink whose spec sets `modifier_keys` hears the modifier keys
    /// themselves.
    #[test]
    fn a_keys_place_and_the_locks_cross_in_the_modifier_word() {
        let ctx = kui_ctx_new();
        let mut spec = unsafe { std::mem::zeroed::<KuiSpec>() };
        spec.width = KuiSizing {
            tag: 2,
            value: 100.0,
        };
        spec.height = KuiSizing {
            tag: 2,
            value: 50.0,
        };
        spec.modifier_keys = 1;
        kui_frame_begin(ctx, 200.0, 100.0, 1.0);
        let sink = kui_open_with(ctx, ks("sink"), &spec, NONE, NONE, kui_value_int(1), NONE);
        kui_close(ctx);
        kui_set_key_focus(ctx, sink);
        kui_frame_finish(ctx);
        let null = KuiStr {
            ptr: std::ptr::null(),
            len: 0,
        };
        kui_input_key_down(
            ctx,
            ks("1"),
            null,
            KUI_KLOC_NUMPAD | KUI_KLOCK_NUM | KUI_KLOCK_CAPS,
            null,
            false,
        );
        kui_input_key_down(
            ctx,
            ks("shift"),
            null,
            KUI_KMOD_SHIFT | KUI_KLOC_RIGHT,
            null,
            false,
        );
        kui_input_key_down(ctx, ks("a"), null, 0, null, false);
        let mut ev = KuiEvent::default();
        let mut seen = Vec::new();
        while kui_poll_event(ctx, &mut ev) {
            let str_of = |k: &str| {
                let mut out = KuiStr {
                    ptr: std::ptr::null(),
                    len: 0,
                };
                kui_value_as_str(kui_value_get(ev.payload, ks(k)), &mut out);
                kstr(out).into_owned()
            };
            let bool_of = |k: &str| {
                let mut b = false;
                kui_value_as_bool(kui_value_get(ev.payload, ks(k)), &mut b);
                b
            };
            seen.push((
                str_of("code"),
                str_of("location"),
                bool_of("caps_lock"),
                bool_of("num_lock"),
            ));
        }
        assert_eq!(
            seen,
            [
                ("1".into(), "numpad".into(), true, true),
                ("shift".into(), "right".into(), false, false),
                ("a".into(), "standard".into(), false, false),
            ]
        );
        kui_ctx_free(ctx);
    }

    /// The US stand-in F76 made Shift-aware is for the keymap alone: a
    /// NULL `text` is what the layout's key types (Russian shift-Ж types
    /// `Ж`, where it typed the stand-in's `:`; RG28), and under Alt the
    /// stand-in is the unshifted position — the `j` the winit runner
    /// reads for alt-shift-J with every modifier stripped, where a host
    /// passing the composed `Ô` got `J` (RG27).
    #[test]
    fn the_stand_in_is_for_the_keymap_and_alt_keeps_it_unshifted() {
        let ctx = kui_ctx_new();
        let mut spec = unsafe { std::mem::zeroed::<KuiSpec>() };
        spec.width = KuiSizing {
            tag: 2,
            value: 100.0,
        };
        spec.height = KuiSizing {
            tag: 2,
            value: 50.0,
        };
        kui_frame_begin(ctx, 200.0, 100.0, 1.0);
        let sink = kui_open_with(
            ctx,
            ks("sink"),
            &spec,
            NONE,
            NONE,
            kui_value_str(ks("keys")),
            NONE,
        );
        kui_close(ctx);
        kui_set_key_focus(ctx, sink);
        kui_frame_finish(ctx);

        let null = KuiStr {
            ptr: std::ptr::null(),
            len: 0,
        };
        kui_input_key_down(ctx, ks("Ж"), ks(";"), KUI_KMOD_SHIFT, null, false);
        kui_input_key_down(
            ctx,
            ks("Ô"),
            ks("j"),
            KUI_KMOD_ALT | KUI_KMOD_SHIFT,
            null,
            false,
        );
        // macOS Russian's `]` on the key printed `` ` ``: the layout's own
        // ASCII judged by itself, the US key on a layout said non-Latin
        // (F115) — and either way the press types the layout's `]`.
        kui_input_key_down(ctx, ks("]"), ks("`"), 0, null, false);
        kui_input_key_down(ctx, ks("]"), ks("`"), KUI_KLAYOUT_NONLATIN, null, false);
        let mut ev = KuiEvent::default();
        let mut seen = Vec::new();
        while kui_poll_event(ctx, &mut ev) {
            let get = |k: &str| {
                let v = kui_value_get(ev.payload, ks(k));
                let mut out = KuiStr {
                    ptr: std::ptr::null(),
                    len: 0,
                };
                kui_value_as_str(v, &mut out).then(|| kstr(out).into_owned())
            };
            seen.push((
                get("code").unwrap_or_default(),
                get("physical").unwrap_or_default(),
                get("text"),
            ));
        }
        assert_eq!(
            seen,
            [
                (":".into(), ";".into(), Some("Ж".into())),
                ("j".into(), "j".into(), None),
                ("]".into(), "`".into(), Some("]".into())),
                ("`".into(), "`".into(), Some("]".into())),
            ]
        );
        kui_ctx_free(ctx);

        // The whole press into a focused editor types the layout's key.
        let ctx = kui_ctx_new();
        let spec = unsafe { std::mem::zeroed::<KuiSpec>() };
        kui_frame_begin(ctx, 400.0, 200.0, 1.0);
        let key = kui_text_edit(
            ctx,
            ks("doc"),
            ks(""),
            std::ptr::null(),
            2, // KUI_EDIT_AUTOFOCUS
            &spec,
        );
        kui_frame_finish(ctx);
        kui_input_press(ctx, ks("Ж"), ks(";"), KUI_KMOD_SHIFT, null, false);
        let mut text = KuiStr {
            ptr: std::ptr::null(),
            len: 0,
        };
        assert!(kui_edit_text(ctx, key, &mut text));
        assert_eq!(kstr(text).as_ref(), "Ж");
        kui_ctx_free(ctx);
    }

    /// An editor's runs cross as rows with borrowed arrays, and a text
    /// request addresses them: select "world" by run positions, type
    /// over it, read the text back.
    #[test]
    fn editor_runs_and_text_requests_cross_the_boundary() {
        let ctx = kui_ctx_new();
        let mut spec = unsafe { std::mem::zeroed::<KuiSpec>() };
        spec.width = KuiSizing {
            tag: 2,
            value: 300.0,
        };
        spec.label = ks("Doc");
        kui_frame_begin(ctx, 400.0, 200.0, 1.0);
        let key = kui_text_edit(
            ctx,
            ks("doc"),
            ks("hello world"),
            std::ptr::null(),
            2, // KUI_EDIT_AUTOFOCUS
            &spec,
        );
        kui_frame_finish(ctx);

        let mut nodes = [unsafe { std::mem::zeroed::<KuiAccessNode>() }; 4];
        assert_eq!(kui_access_tree(ctx, nodes.as_mut_ptr(), nodes.len()), 2);
        let ed = nodes[1];
        assert_eq!(ed.key, key);
        assert_eq!(ed.role, role_code(kui_core::Role::TextInput));
        assert_eq!(ed.run_count, 1);
        assert_ne!(ed.flags & KUI_ACCESS_HAS_TEXT_SELECTION, 0);
        assert_ne!(ed.flags & KUI_ACCESS_FOCUSED, 0);
        // A single-line field opens with the caret after its seed (F20).
        assert_eq!((ed.focus_char, ed.anchor_char), (11, 11));

        let mut runs = [unsafe { std::mem::zeroed::<KuiAccessRun>() }; 4];
        assert_eq!(kui_access_runs(ctx, key, runs.as_mut_ptr(), runs.len()), 1);
        let r = runs[0];
        assert_eq!(r.key, ed.focus_run);
        assert_eq!(kstr(r.text).as_ref(), "hello world");
        assert_eq!((r.line, r.start, r.end), (0, 0, 11));
        assert_eq!(r.char_count, 11);
        let starts =
            unsafe { std::slice::from_raw_parts(r.word_starts, r.word_start_count as usize) };
        assert_eq!(starts, [0, 6]);
        let positions =
            unsafe { std::slice::from_raw_parts(r.char_positions, r.char_count as usize) };
        assert_eq!(positions[0], 0.0);
        assert!(positions[6] > positions[0]);
        assert_eq!(kui_access_runs(ctx, 12345, runs.as_mut_ptr(), 4), 0);

        kui_input_access_text(
            ctx,
            key,
            kui_core::AccessAction::SetTextSelection.bit(),
            r.key,
            6,
            r.key,
            11,
            ks(""),
        );
        kui_input_access_text(
            ctx,
            key,
            kui_core::AccessAction::ReplaceSelectedText.bit(),
            0,
            0,
            0,
            0,
            ks("there"),
        );
        let mut text = KuiStr {
            ptr: std::ptr::null(),
            len: 0,
        };
        assert!(kui_edit_text(ctx, key, &mut text));
        assert_eq!(kstr(text).as_ref(), "hello there");
        let mut ev = KuiEvent::default();
        assert!(kui_poll_event(ctx, &mut ev));
        assert_eq!(unsafe { &*ev.payload }.0.get_str("kind"), Some("changed"));
        kui_ctx_free(ctx);
    }

    /// A C editor's caret blinks: the `caret` on a
    /// `KUI_ROLE_LINE` under the focused sink is a caret to blink, the
    /// stamp moves with it, and the phase a host's clock sets reads back.
    /// With KUI_VALUE_CARET_SOLID beside it (F68) it is no caret to
    /// blink — a block caret in normal mode — while it still anchors.
    #[test]
    fn a_c_sinks_caret_line_arms_the_blink_and_the_phase_reads_back() {
        let ctx = kui_ctx_new();
        let draw = |ctx: *mut KuiCtx, caret: u32, value_set: u32| {
            kui_frame_begin(ctx, 400.0, 200.0, 1.0);
            let mut spec = unsafe { std::mem::zeroed::<KuiSpec>() };
            spec.width = KuiSizing {
                tag: 2,
                value: 300.0,
            };
            spec.height = KuiSizing {
                tag: 2,
                value: 60.0,
            };
            spec.role = 20; // KUI_ROLE_MULTILINE_TEXT_INPUT
            spec.label = ks("buf");
            let sink = kui_open_with(
                ctx,
                ks("editor"),
                &spec,
                NONE,
                NONE,
                kui_value_str(ks("ed")),
                NONE,
            );
            let mut row = unsafe { std::mem::zeroed::<KuiSpec>() };
            row.dir = 1;
            row.height = KuiSizing {
                tag: 2,
                value: 20.0,
            };
            row.role = 22; // KUI_ROLE_LINE
            row.caret = caret;
            row.value_set = value_set;
            kui_open(ctx, &row, NONE);
            let mut style: KuiTextStyle = unsafe { std::mem::zeroed() };
            style.size = 14.0;
            style.family = 2;
            kui_text(ctx, ks("let value"), &style);
            kui_close(ctx);
            kui_close(ctx);
            kui_set_key_focus(ctx, sink);
            kui_frame_finish(ctx);
        };
        assert!(!kui_has_caret(ctx), "nothing drawn, nothing to blink");
        assert!(kui_caret_visible(ctx), "solid until a clock says otherwise");
        draw(ctx, 3, KUI_VALUE_CARET);
        assert!(kui_has_caret(ctx), "the caret row under the focused sink");
        let stamp = kui_caret_stamp(ctx);
        draw(ctx, 3, KUI_VALUE_CARET);
        assert_eq!(
            kui_caret_stamp(ctx),
            stamp,
            "the same caret again: no restamp"
        );
        draw(ctx, 5, KUI_VALUE_CARET);
        assert_ne!(kui_caret_stamp(ctx), stamp, "the caret moved: restamped");
        kui_set_caret_visible(ctx, false);
        assert!(!kui_caret_visible(ctx));
        kui_set_caret_visible(ctx, true);
        assert!(kui_caret_visible(ctx));
        // Escape to normal mode: the same caret, solid. Not one to blink,
        // not a move, and still the IME's anchor.
        let stamp = kui_caret_stamp(ctx);
        let mut anchor = KuiCaretRect::default();
        assert!(kui_ime_rect(ctx, &mut anchor));
        draw(ctx, 5, KUI_VALUE_CARET | KUI_VALUE_CARET_SOLID);
        assert!(!kui_has_caret(ctx), "a solid caret is not a caret to blink");
        assert_eq!(kui_caret_stamp(ctx), stamp, "bar to block is not a move");
        let mut held = anchor;
        assert!(kui_ime_rect(ctx, &mut held), "the anchor held");
        assert_eq!((held.x, held.y), (anchor.x, anchor.y));
        draw(ctx, 5, KUI_VALUE_CARET);
        assert!(kui_has_caret(ctx), "back in insert mode: a caret to blink");
        kui_ctx_free(ctx);
    }

    /// A C editor's clipboard and mouse: the two
    /// clipboard doors queue what a menu's Copy and Paste would, a paste
    /// the host commits reaches the sink as `text`, and a press inside
    /// the sink carries `line`, `byte` and `clicks`.
    #[test]
    fn a_c_sink_has_a_clipboard_and_a_press_in_it_names_the_line() {
        let ctx = kui_ctx_new();
        let sized = |w: f32, h: f32| {
            let mut spec = unsafe { std::mem::zeroed::<KuiSpec>() };
            spec.width = KuiSizing { tag: 2, value: w };
            spec.height = KuiSizing { tag: 2, value: h };
            spec
        };
        let mut style: KuiTextStyle = unsafe { std::mem::zeroed() };
        style.size = 14.0;
        style.line_height = 20.0;
        style.family = 2; // KUI_FONT_MONO
        kui_frame_begin(ctx, 400.0, 200.0, 1.0);
        let mut spec = sized(300.0, 60.0);
        spec.role = 20; // KUI_ROLE_MULTILINE_TEXT_INPUT
        spec.label = ks("buf");
        let sink = kui_open_with(
            ctx,
            ks("editor"),
            &spec,
            NONE,
            kui_value_str(ks("sel")),
            kui_value_str(ks("ed")),
            NONE,
        );
        for line in ["hello world", "second"] {
            let mut row = sized(300.0, 20.0);
            row.dir = 1; // KUI_DIR_ROW
            row.role = 22; // KUI_ROLE_LINE
            kui_open(ctx, &row, NONE);
            kui_text(ctx, ks(line), &style);
            kui_close(ctx);
        }
        kui_close(ctx);
        kui_set_key_focus(ctx, sink);
        kui_frame_finish(ctx);

        kui_set_clipboard(ctx, ks("yanked"), ks(""));
        kui_set_clipboard_secret(ctx, ks("hunter2"));
        kui_request_paste(ctx);
        let mut action = KuiMenuAction {
            size: std::mem::size_of::<KuiMenuAction>() as u32,
            ..unsafe { std::mem::zeroed() }
        };
        assert!(kui_take_menu_action(ctx, &mut action));
        assert_eq!(action.kind, KUI_MENU_ACTION_SET_CLIPBOARD);
        assert_eq!(kstr(action.text).as_ref(), "yanked");
        assert_eq!(action.html.len, 0);
        // A secret is its own kind, for the host to write marked
        // concealed and transient.
        assert!(kui_take_menu_action(ctx, &mut action));
        assert_eq!(action.kind, KUI_MENU_ACTION_SET_CLIPBOARD_SECRET);
        assert_eq!(kstr(action.text).as_ref(), "hunter2");
        assert_eq!(action.html.len, 0);
        assert!(kui_take_menu_action(ctx, &mut action));
        assert_eq!(action.kind, KUI_MENU_ACTION_PASTE);
        assert!(!kui_take_menu_action(ctx, &mut action));

        // The host answers the paste with a commit; the sink hears it.
        kui_input_commit(ctx, ks("from the clipboard"));
        // And a paste the pasteboard marked, which the sink hears with
        // its markers — only those set.
        kui_input_paste(ctx, ks("s3cret"), KUI_PASTE_CONCEALED | KUI_PASTE_TRANSIENT);
        kui_input_paste(ctx, ks("brief"), KUI_PASTE_TRANSIENT);
        // A double click on the second line, past its end.
        kui_input_cursor(ctx, 290.0, 30.0);
        kui_input_mouse(ctx, true, 2);
        kui_input_mouse(ctx, false, 2);

        let mut ev = KuiEvent::default();
        let mut seen = Vec::new();
        while kui_poll_event(ctx, &mut ev) {
            let get_str = |k: &str| {
                let v = kui_value_get(ev.payload, ks(k));
                let mut out = KuiStr {
                    ptr: std::ptr::null(),
                    len: 0,
                };
                kui_value_as_str(v, &mut out).then(|| kstr(out).into_owned())
            };
            let get_int = |k: &str| {
                let mut out = 0i64;
                kui_value_as_int(kui_value_get(ev.payload, ks(k)), &mut out).then_some(out)
            };
            assert_eq!(ev.key, sink);
            let get_bool = |k: &str| {
                let mut out = false;
                kui_value_as_bool(kui_value_get(ev.payload, ks(k)), &mut out).then_some(out)
            };
            match get_str("kind").as_deref() {
                Some("text") => seen.push(format!(
                    "text:{}{}{}",
                    get_str("text").unwrap(),
                    match get_bool("concealed") {
                        Some(true) => ":concealed",
                        Some(false) => ":concealed=false",
                        None => "",
                    },
                    match get_bool("transient") {
                        Some(true) => ":transient",
                        Some(false) => ":transient=false",
                        None => "",
                    },
                )),
                Some("drag") => seen.push(format!(
                    "{}:{}:{}:{}",
                    get_str("phase").unwrap(),
                    get_int("line").unwrap(),
                    get_int("byte").unwrap(),
                    get_int("clicks").unwrap()
                )),
                other => panic!("{other:?}"),
            }
        }
        assert_eq!(
            seen,
            [
                "text:from the clipboard",
                "text:s3cret:concealed:transient",
                "text:brief:transient",
                "start:1:6:2",
                "end:1:6:2"
            ]
        );
        kui_ctx_free(ctx);
    }

    /// `kui_edit_set_text_label` names the editor the *next* frame will
    /// declare, which is the only name a host opening one for the first
    /// time has: `kui_key_of` answers 0 for it, and no event has carried
    /// its key. The frame that declares it takes the text over its
    /// `initial`; a label no frame declares is a warning, not a hold.
    #[test]
    fn an_editor_is_seeded_by_the_label_the_next_frame_declares() {
        let ctx = kui_ctx_new();
        let empty = |ctx: *mut KuiCtx| {
            kui_frame_begin(ctx, 400.0, 200.0, 1.0);
            kui_frame_finish(ctx);
        };
        let with_editor = |ctx: *mut KuiCtx| -> u64 {
            kui_frame_begin(ctx, 400.0, 200.0, 1.0);
            let mut spec = unsafe { std::mem::zeroed::<KuiSpec>() };
            spec.width = KuiSizing {
                tag: 2,
                value: 300.0,
            };
            spec.label = ks("Name");
            let key = kui_text_edit(ctx, ks("name"), ks("initial"), std::ptr::null(), 0, &spec);
            kui_frame_finish(ctx);
            key
        };
        let read = |ctx: *mut KuiCtx, key: u64| -> String {
            let mut text = KuiStr {
                ptr: std::ptr::null(),
                len: 0,
            };
            assert!(kui_edit_text(ctx, key, &mut text));
            kstr(text).into_owned()
        };

        empty(ctx);
        assert_eq!(kui_key_of(ctx, ks("name")), 0, "nothing declared it yet");
        kui_edit_set_text_label(ctx, ks("name"), ks("from the model"));
        let key = with_editor(ctx);
        assert_eq!(read(ctx, key), "from the model");

        // A label the last frame declared resolves, so this lands now.
        kui_edit_set_text_label(ctx, ks("name"), ks("renamed"));
        assert_eq!(read(ctx, key), "renamed");

        // And one no frame declares is dropped, with a line saying so.
        kui_set_diagnostics(ctx, true);
        kui_edit_set_text_label(ctx, ks("nmae"), ks("nowhere"));
        empty(ctx);
        let mut warnings = [unsafe { std::mem::zeroed::<KuiWarning>() }; 4];
        let n = kui_take_warnings(ctx, warnings.as_mut_ptr(), warnings.len());
        assert_eq!(n, 1);
        assert_eq!(kstr(warnings[0].code).as_ref(), "edit-text-without-editor");
        assert!(
            kstr(warnings[0].message).contains("nmae"),
            "the label is in the message: {}",
            kstr(warnings[0].message)
        );
        kui_ctx_free(ctx);
    }

    /// `kui_button_with` reads the rows the stock button admits off the
    /// spec — label, description, tooltip, disabled — and nothing else:
    /// a width on the same spec changes no quad, since the look is
    /// `widgets::button_spec`'s. A NULL spec is `kui_button`.
    #[test]
    fn the_stock_button_reads_its_rows_off_a_spec() {
        let ctx = kui_ctx_new();
        let frame = |ctx: *mut KuiCtx, rows: bool| {
            kui_frame_begin(ctx, 200.0, 100.0, 1.0);
            let mut go = unsafe { std::mem::zeroed::<KuiSpec>() };
            go.description = ks("Starts the run");
            // A row the button does not read: no effect, by design.
            go.width = KuiSizing {
                tag: 2,
                value: 180.0,
            };
            let mut stop = unsafe { std::mem::zeroed::<KuiSpec>() };
            stop.label = ks("Stop the run");
            stop.tooltip = ks("Nothing is running");
            stop.disabled = 1;
            if rows {
                kui_button_with(ctx, ks("go"), &go, kui_value_str(ks("go")));
                kui_button_with(ctx, ks("stop"), &stop, kui_value_str(ks("stop")));
            } else {
                kui_button_with(ctx, ks("go"), std::ptr::null(), kui_value_str(ks("go")));
                kui_button(ctx, ks("stop"), kui_value_str(ks("stop")));
            }
            kui_frame_finish(ctx);
        };
        frame(ctx, true);
        let mut out = [unsafe { std::mem::zeroed::<KuiAccessNode>() }; 4];
        assert_eq!(kui_access_tree(ctx, out.as_mut_ptr(), out.len()), 3);
        assert_eq!(out[1].role, role_code(kui_core::Role::Button));
        assert_eq!(kstr(out[1].name).as_ref(), "go");
        assert_eq!(kstr(out[1].description).as_ref(), "Starts the run");
        assert_eq!(out[1].flags & KUI_ACCESS_DISABLED, 0);
        assert_eq!(
            kstr(out[2].name).as_ref(),
            "Stop the run",
            "named past its text"
        );
        assert_eq!(
            kstr(out[2].description).as_ref(),
            "Nothing is running",
            "the tooltip is the description"
        );
        assert_ne!(out[2].flags & KUI_ACCESS_DISABLED, 0);
        // Each solid quad's box and alpha; the glyphs are the same either way.
        let boxes = |ctx: *mut KuiCtx| {
            let mut draw = KuiDrawData::default();
            kui_draw_data(ctx, &mut draw);
            unsafe { std::slice::from_raw_parts(draw.quads, draw.quad_count) }
                .iter()
                .filter(|q| q.kind == kui_core::QuadKind::Solid as u32)
                .map(|q| (q.x, q.y, q.w, q.h, q.color[3]))
                .collect::<Vec<_>>()
        };
        let with_rows = boxes(ctx);

        // The same two buttons with no rows: the width was never read, so
        // the only difference the rows made is the dimming of the
        // disabled one.
        frame(ctx, false);
        let plain = boxes(ctx);
        assert_eq!(with_rows.len(), 2);
        assert_eq!(plain.len(), 2);
        assert_eq!(
            with_rows[0], plain[0],
            "the go button's box is the widget's, not the spec's width"
        );
        assert_eq!(with_rows[1].0..=with_rows[1].3, plain[1].0..=plain[1].3);
        assert!(
            with_rows[1].4 < plain[1].4,
            "the disabled button is dimmed: {} vs {}",
            with_rows[1].4,
            plain[1].4
        );
        kui_ctx_free(ctx);
    }

    /// The access tree crosses as rows (plain boxes elided), and an
    /// assistive request comes back in as input: a click on a labelled
    /// button emits its payload, a slider nudge arrives as an `access`
    /// event.
    #[test]
    fn access_tree_and_requests_cross_the_boundary() {
        let ctx = kui_ctx_new();
        let mut button = unsafe { std::mem::zeroed::<KuiSpec>() };
        button.width = KuiSizing {
            tag: 2,
            value: 40.0,
        };
        button.height = KuiSizing {
            tag: 2,
            value: 20.0,
        };
        button.label = ks("Save");
        let mut slider = unsafe { std::mem::zeroed::<KuiSpec>() };
        slider.width = KuiSizing {
            tag: 2,
            value: 100.0,
        };
        slider.height = KuiSizing {
            tag: 2,
            value: 10.0,
        };
        slider.role = role_code(kui_core::Role::Slider);
        slider.label = ks("Volume");
        slider.value_set = KUI_VALUE_NOW | KUI_VALUE_MAX;
        slider.value_now = 3.0;
        slider.value_max = 10.0;
        let plain = unsafe { std::mem::zeroed::<KuiSpec>() };
        kui_frame_begin(ctx, 200.0, 100.0, 1.0);
        kui_open_keyed(ctx, ks("save"), &button, kui_value_str(ks("save")));
        kui_close(ctx);
        kui_open_keyed(ctx, ks("plain"), &plain, NONE);
        kui_close(ctx);
        kui_open_keyed(ctx, ks("vol"), &slider, NONE);
        kui_close(ctx);
        kui_frame_finish(ctx);

        assert_eq!(
            kui_access_tree(ctx, std::ptr::null_mut(), 0),
            3,
            "window, button, slider: the plain box is elided"
        );
        let mut out = [unsafe { std::mem::zeroed::<KuiAccessNode>() }; 8];
        assert_eq!(kui_access_tree(ctx, out.as_mut_ptr(), out.len()), 3);
        assert_eq!(out[0].role, role_code(kui_core::Role::Window));
        assert_eq!(out[0].parent, 0);
        assert_eq!(out[1].role, role_code(kui_core::Role::Button));
        assert_eq!(kstr(out[1].name).as_ref(), "Save");
        assert_eq!(out[1].parent, out[0].key);
        assert_ne!(out[1].actions & kui_core::AccessAction::Click.bit(), 0);
        assert_eq!(
            (out[1].x, out[1].y, out[1].w, out[1].h),
            (0.0, 0.0, 40.0, 20.0)
        );
        assert_eq!(out[2].role, role_code(kui_core::Role::Slider));
        assert_eq!(kstr(out[2].name).as_ref(), "Volume");
        assert_eq!(
            out[2].flags & (KUI_ACCESS_HAS_NUMBER | KUI_ACCESS_HAS_MIN | KUI_ACCESS_HAS_MAX),
            KUI_ACCESS_HAS_NUMBER | KUI_ACCESS_HAS_MAX
        );
        assert_eq!((out[2].value_now, out[2].value_max), (3.0, 10.0));
        // A short buffer still reports the total.
        assert_eq!(kui_access_tree(ctx, out.as_mut_ptr(), 1), 3);

        kui_input_access(ctx, out[1].key, kui_core::AccessAction::Click.bit(), ks(""));
        let mut ev = KuiEvent::default();
        assert!(kui_poll_event(ctx, &mut ev));
        assert_eq!(ev.key, out[1].key);
        assert_eq!(unsafe { &*ev.payload }.0.as_str(), Some("save"));
        kui_input_access(
            ctx,
            out[2].key,
            kui_core::AccessAction::Increment.bit(),
            ks(""),
        );
        assert!(kui_poll_event(ctx, &mut ev));
        let payload = unsafe { &*ev.payload };
        assert_eq!(payload.0.get_str("kind"), Some("access"));
        assert_eq!(payload.0.get_str("action"), Some("increment"));
        assert!(!kui_poll_event(ctx, &mut ev));
        // An unknown action bit is ignored, not a crash.
        kui_input_access(ctx, out[1].key, 1 << 30, ks(""));
        assert!(!kui_poll_event(ctx, &mut ev));
        kui_ctx_free(ctx);
    }

    /// Measurement, layout events and warnings all reach C: the measured
    /// width of a label is what layout gives its node, a `layout` event
    /// polls out with the node's rect, and a lone weighted grow child
    /// warns once.
    #[test]
    fn measure_layout_and_warnings_flow_through_the_c_api() {
        let ctx = kui_ctx_new();
        // Standalone contexts start quiet; a host opts in.
        kui_set_diagnostics(ctx, true);
        let style: KuiTextStyle = unsafe { std::mem::zeroed() };
        let mut m = KuiTextMetrics::default();
        assert!(kui_measure_text(ctx, ks("hello"), &style, 0.0, &mut m));
        assert!(m.width > 0.0 && m.height > 0.0 && m.lines == 1);
        let mut wrapped = KuiTextMetrics::default();
        assert!(kui_measure_text(
            ctx,
            ks("hello world again"),
            &style,
            m.width,
            &mut wrapped
        ));
        assert!(
            wrapped.lines > 1,
            "wraps at the width of one word: {}",
            wrapped.lines
        );

        let tag = KuiValue(Value::str("panel"));
        let mut spec: KuiSpec = unsafe { std::mem::zeroed() };
        spec.width = KuiSizing { tag: 1, value: 2.0 };
        spec.height = KuiSizing {
            tag: 2,
            value: 20.0,
        };
        spec.on_layout = &tag;
        kui_frame_begin(ctx, 300.0, 100.0, 1.0);
        let mut root: KuiSpec = unsafe { std::mem::zeroed() };
        root.dir = 1;
        root.width = KuiSizing { tag: 1, value: 1.0 };
        kui_root(ctx, &root);
        let key = kui_open_keyed(ctx, ks("panel"), &spec, NONE);
        kui_close(ctx);
        kui_frame_finish(ctx);

        let mut ev = KuiEvent::default();
        assert!(kui_poll_event(ctx, &mut ev));
        assert_eq!(ev.key, key);
        let payload = unsafe { &*ev.payload };
        assert_eq!(payload.0.get_str("kind"), Some("layout"));
        assert_eq!(payload.0.get_float("w"), Some(300.0));
        assert_eq!(payload.0.get_str("tag"), Some("panel"));
        // The same rect as a query, and nothing for
        // a key that declared no on_layout.
        let mut rect = KuiLayoutRect::default();
        assert!(kui_layout_of(ctx, key, &mut rect));
        assert_eq!((rect.x, rect.y, rect.w, rect.h), (0.0, 0.0, 300.0, 20.0));
        assert!(!kui_layout_of(ctx, key + 1, &mut rect));

        let mut out = [KuiWarning {
            code: KUI_EMPTY,
            key: 0,
            message: KUI_EMPTY,
        }; 4];
        let n = kui_take_warnings(ctx, out.as_mut_ptr(), out.len());
        assert_eq!(n, 1, "the lone grow-2 child warns");
        assert_eq!(&*kstr(out[0].code), "grow-weight-ignored");
        assert_eq!(out[0].key, key);
        assert!(kstr(out[0].message).contains("only grow child"));
        kui_ctx_free(ctx);
    }

    const KUI_EMPTY: KuiStr = KuiStr {
        ptr: std::ptr::null(),
        len: 0,
    };
}

/// C is the only binding that writes the env and never reads it back, so
/// the readings the other three hand their views come from these calls
/// and nothing else checks them. Every fact, its "cannot tell" spelling,
/// and the two ways a host can be wrong. The header's names for the codes
/// are pinned to these same list positions by `abi_parity`.
#[cfg(test)]
mod env_headless {
    use super::*;

    fn system(ctx: *mut KuiCtx) -> kui_core::SystemEnv {
        unsafe { ctx.as_mut() }.unwrap().core().env.system
    }

    #[test]
    fn the_system_setter_writes_every_fact_and_its_unknown() {
        let ctx = kui_ctx_new();
        // A host that never calls it says so, rather than "light".
        assert_eq!(system(ctx), kui_core::SystemEnv::default());

        kui_env_set_system(
            ctx,
            Appearance::Dark.code(),
            0x3b82f6ff,
            MotionPref::Reduced.code(),
            ks("pt-BR"),
        );
        let sys = system(ctx);
        assert_eq!(sys.appearance, Appearance::Dark);
        assert_eq!(sys.accent.map(|c| c.to_hex()), Some(0x3b82f6ff));
        assert_eq!(sys.motion, MotionPref::Reduced);
        assert_eq!(
            sys.locale.map(|l| l.as_str().to_string()).as_deref(),
            Some("pt-BR")
        );

        // The zeroed call is the whole "I cannot tell" reading — what a
        // host that has stopped knowing pushes, and what a host that
        // zero-initializes its arguments says by accident and truthfully.
        kui_env_set_system(ctx, 0, 0, 0, ks(""));
        assert_eq!(system(ctx), kui_core::SystemEnv::default());

        // A code this build has no name for is ignored rather than folded
        // onto a real setting, and a tag that is not one is not stored
        // half-written.
        kui_env_set_system(ctx, 99, 0, 99, ks(&"x".repeat(64)));
        let sys = system(ctx);
        assert_eq!(sys.appearance, Appearance::Unknown);
        assert_eq!(sys.motion, MotionPref::Unknown);
        assert_eq!(sys.locale, None);

        kui_ctx_free(ctx);
    }

    /// The audio row: a host with no device never calls it and reads
    /// closed; one that does writes the state and the count, and a code
    /// this build has no name for is closed rather than a guess.
    #[test]
    fn the_audio_setter_writes_the_device_state_and_the_live_count() {
        use kui_core::{AudioDevice, AudioEnv};
        let ctx = kui_ctx_new();
        let audio = |ctx: *mut KuiCtx| unsafe { ctx.as_mut() }.unwrap().core().env.audio;
        assert_eq!(audio(ctx), AudioEnv::default());

        kui_env_set_audio(ctx, AudioDevice::Open.code(), 2);
        assert_eq!(
            audio(ctx),
            AudioEnv {
                device: AudioDevice::Open,
                live: 2
            }
        );
        kui_env_set_audio(ctx, 0, 0);
        assert_eq!(audio(ctx), AudioEnv::default());
        kui_env_set_audio(ctx, 99, 1);
        assert_eq!(audio(ctx).device, AudioDevice::Closed);
        assert_eq!(audio(ctx).live, 1);
        kui_ctx_free(ctx);
    }

    /// The assistive row: a host with no bridge never calls
    /// it and reads unknown; one that bridges the platform's accessibility
    /// API pushes the reading, and the settings setter — which a host
    /// re-runs on every OS notification — leaves it alone.
    #[test]
    fn the_assistive_setter_is_its_own_door_and_the_system_setter_keeps_it() {
        use kui_core::Assistive;
        let ctx = kui_ctx_new();
        assert_eq!(system(ctx).assistive, Assistive::Unknown);

        kui_env_set_assistive(ctx, Assistive::Listening.code());
        assert_eq!(system(ctx).assistive, Assistive::Listening);
        // The appearance flipped and the host pushed the four settings
        // again: the screen reader did not go away.
        kui_env_set_system(ctx, Appearance::Dark.code(), 0, 0, ks(""));
        assert_eq!(system(ctx).appearance, Appearance::Dark);
        assert_eq!(system(ctx).assistive, Assistive::Listening);
        // A platform that reports the client leaving says so.
        kui_env_set_assistive(ctx, Assistive::None.code());
        assert_eq!(system(ctx).assistive, Assistive::None);
        // A code this build has no name for is unknown, not a guess.
        kui_env_set_assistive(ctx, 99);
        assert_eq!(system(ctx).assistive, Assistive::Unknown);
        kui_ctx_free(ctx);
    }

    /// The secure-input ask is frame state: false until a
    /// frame declares it, false again on the frame that stops, and nothing
    /// on a bad context.
    #[test]
    fn secure_input_is_asked_per_frame() {
        let ctx = kui_ctx_new();
        assert!(!kui_secure_input_get(ctx));
        kui_frame_begin(ctx, 100.0, 100.0, 1.0);
        kui_set_secure_input(ctx, true);
        kui_frame_finish(ctx);
        assert!(kui_secure_input_get(ctx));
        kui_frame_begin(ctx, 100.0, 100.0, 1.0);
        kui_frame_finish(ctx);
        assert!(
            !kui_secure_input_get(ctx),
            "a frame that stops asking turns it off"
        );
        kui_set_secure_input(std::ptr::null_mut(), true);
        assert!(!kui_secure_input_get(std::ptr::null_mut()));
        kui_ctx_free(ctx);
    }

    /// The input-method ask is frame state: false until a frame declares
    /// it, false again on the frame that stops, and nothing on a bad
    /// context.
    #[test]
    fn ime_off_is_asked_per_frame() {
        let ctx = kui_ctx_new();
        assert!(!kui_ime_off_get(ctx));
        kui_frame_begin(ctx, 100.0, 100.0, 1.0);
        kui_set_ime_off(ctx, true);
        kui_frame_finish(ctx);
        assert!(kui_ime_off_get(ctx));
        kui_frame_begin(ctx, 100.0, 100.0, 1.0);
        kui_frame_finish(ctx);
        assert!(
            !kui_ime_off_get(ctx),
            "a frame that stops asking gives the input method back"
        );
        kui_set_ime_off(std::ptr::null_mut(), true);
        assert!(!kui_ime_off_get(std::ptr::null_mut()));
        kui_ctx_free(ctx);
    }

    /// Option as Alt is frame state: none until a frame
    /// declares it, none again on the frame that stops, a number the
    /// header does not name is none, and nothing on a bad context.
    #[test]
    fn option_as_alt_is_asked_per_frame() {
        let ctx = kui_ctx_new();
        assert_eq!(kui_option_as_alt_get(ctx), KUI_OPTION_AS_ALT_NONE);
        for want in [
            KUI_OPTION_AS_ALT_LEFT,
            KUI_OPTION_AS_ALT_RIGHT,
            KUI_OPTION_AS_ALT_BOTH,
        ] {
            kui_frame_begin(ctx, 100.0, 100.0, 1.0);
            kui_set_option_as_alt(ctx, want);
            kui_frame_finish(ctx);
            assert_eq!(kui_option_as_alt_get(ctx), want);
        }
        kui_frame_begin(ctx, 100.0, 100.0, 1.0);
        kui_frame_finish(ctx);
        assert_eq!(
            kui_option_as_alt_get(ctx),
            KUI_OPTION_AS_ALT_NONE,
            "a frame that stops asking gives the Option keys back"
        );
        kui_frame_begin(ctx, 100.0, 100.0, 1.0);
        kui_set_option_as_alt(ctx, KUI_OPTION_AS_ALT_BOTH + 1);
        kui_frame_finish(ctx);
        assert_eq!(kui_option_as_alt_get(ctx), KUI_OPTION_AS_ALT_NONE);
        kui_set_option_as_alt(std::ptr::null_mut(), KUI_OPTION_AS_ALT_LEFT);
        assert_eq!(
            kui_option_as_alt_get(std::ptr::null_mut()),
            KUI_OPTION_AS_ALT_NONE
        );
        kui_ctx_free(ctx);
    }

    /// The level is two facts with two doors: the frame's
    /// ask, frame-scoped and false by default, which the host reads after
    /// the frame; and what the host did, which it writes back and the
    /// ask never touches — `kui_env_set_window` leaves it alone the way
    /// `kui_env_set_system` leaves `assistive`.
    #[test]
    fn the_level_is_asked_per_frame_and_reported_through_its_own_door() {
        let ctx = kui_ctx_new();
        let window = |ctx: *mut KuiCtx| unsafe { ctx.as_mut() }.unwrap().core().env.window;
        assert!(!kui_always_on_top_get(ctx));
        assert!(!window(ctx).always_on_top);

        kui_frame_begin(ctx, 100.0, 100.0, 1.0);
        kui_set_always_on_top(ctx, true);
        kui_frame_finish(ctx);
        assert!(kui_always_on_top_get(ctx));
        // Asking is not having: the reading is the host's to write.
        assert!(!window(ctx).always_on_top);

        kui_env_set_always_on_top(ctx, true);
        assert!(window(ctx).always_on_top);
        kui_env_set_window(ctx, 0, true, false, false, 0.0, 0.0);
        assert!(window(ctx).always_on_top, "the window setter keeps it");
        assert!(window(ctx).custom_chrome);

        // A frame that stops asking is the lowering; the report stays
        // until the host says otherwise.
        kui_frame_begin(ctx, 100.0, 100.0, 1.0);
        kui_frame_finish(ctx);
        assert!(!kui_always_on_top_get(ctx));
        assert!(window(ctx).always_on_top);
        kui_env_set_always_on_top(ctx, false);
        assert!(!window(ctx).always_on_top);
        kui_ctx_free(ctx);
    }

    /// The backdrop is a fact the host reports through its own door
    /// (backlog F126): opaque until it says otherwise, kept by the window
    /// setter, and a code past the end ignored rather than trusted.
    #[test]
    fn the_backdrop_is_reported_through_its_own_door() {
        let ctx = kui_ctx_new();
        let window = |ctx: *mut KuiCtx| unsafe { ctx.as_mut() }.unwrap().core().env.window;
        assert_eq!(window(ctx).backdrop, kui_core::Backdrop::Opaque);
        kui_env_set_backdrop(ctx, 2);
        assert_eq!(window(ctx).backdrop, kui_core::Backdrop::Blur);
        kui_env_set_window(ctx, 0, true, false, false, 0.0, 0.0);
        assert_eq!(
            window(ctx).backdrop,
            kui_core::Backdrop::Blur,
            "the window setter keeps it"
        );
        kui_env_set_backdrop(ctx, 99);
        assert_eq!(window(ctx).backdrop, kui_core::Backdrop::Blur, "ignored");
        kui_env_set_backdrop(ctx, 0);
        assert_eq!(window(ctx).backdrop, kui_core::Backdrop::Opaque);
        kui_ctx_free(ctx);
    }

    /// A null context is a no-op, like every other entry point.
    #[test]
    fn a_null_context_is_survivable() {
        kui_env_set_system(std::ptr::null_mut(), 1, 0, 1, ks("en"));
        kui_env_set_audio(std::ptr::null_mut(), 2, 1);
        kui_env_set_assistive(std::ptr::null_mut(), 2);
        kui_set_always_on_top(std::ptr::null_mut(), true);
        assert!(!kui_always_on_top_get(std::ptr::null_mut()));
        kui_env_set_always_on_top(std::ptr::null_mut(), true);
        kui_env_set_backdrop(std::ptr::null_mut(), 2);
    }
}

#[cfg(test)]
mod parity_headless {
    use super::*;

    fn s(k: KuiStr) -> String {
        kstr(k).into_owned()
    }

    /// Every shape a payload can take reads back from C by its own reader:
    /// a bool as a bool (not a number, which `kui_value_as_int` refuses), a
    /// float whole (not truncated), a list by index and length, a map by
    /// entry, null as null. These are the shapes the events carry — a key
    /// event's `shift`, a drag's `dx`, a preedit's `cursor` — so a C host
    /// reads what a Node or Lua host reads, and not a subset of it.
    #[test]
    fn every_payload_shape_reads_back_through_its_own_door() {
        let map = kui_value_map();
        kui_value_map_set(map, ks("held"), kui_value_bool(true));
        kui_value_map_set(map, ks("dx"), kui_value_float(12.75));
        kui_value_map_set(map, ks("n"), kui_value_int(3));
        let list = kui_value_list();
        kui_value_list_push(list, kui_value_int(4));
        kui_value_list_push(list, kui_value_int(9));
        kui_value_map_set(map, ks("cursor"), list);
        kui_value_map_set(map, ks("text"), kui_value_null());

        let mut b = false;
        assert!(kui_value_as_bool(kui_value_get(map, ks("held")), &mut b) && b);
        let mut i = 0;
        assert!(
            !kui_value_as_int(kui_value_get(map, ks("held")), &mut i),
            "a bool is not coerced to a number"
        );
        let mut f = 0.0;
        assert!(kui_value_as_float(kui_value_get(map, ks("dx")), &mut f));
        assert_eq!(f, 12.75, "read whole, where as_int gives 12");
        assert!(kui_value_as_float(kui_value_get(map, ks("n")), &mut f));
        assert_eq!(f, 3.0, "an integer widens");

        let cursor = kui_value_get(map, ks("cursor"));
        assert_eq!(kui_value_len(cursor), 2);
        assert!(kui_value_as_int(kui_value_at(cursor, 1), &mut i) && i == 9);
        assert!(kui_value_at(cursor, 2).is_null(), "past the end is NULL");
        assert!(kui_value_at(map, 0).is_null(), "a map is not a list");
        assert_eq!(
            kui_value_len(kui_value_get(map, ks("n"))),
            0,
            "a scalar has no length"
        );

        assert!(kui_value_is_null(kui_value_get(map, ks("text"))));
        assert!(
            kui_value_is_null(kui_value_get(map, ks("missing"))),
            "absent reads as null"
        );
        assert!(!kui_value_is_null(kui_value_get(map, ks("n"))));

        // A map walks in the order it was set.
        assert_eq!(kui_value_len(map), 5);
        let mut key = ks("");
        let v = kui_value_entry(map, 1, &mut key);
        assert_eq!(s(key), "dx");
        assert!(kui_value_as_float(v, &mut f) && f == 12.75);
        assert!(kui_value_entry(map, 5, &mut key).is_null());

        // Pushing onto a non-list drops the value rather than leaking or
        // panicking.
        let scalar = kui_value_int(1);
        kui_value_list_push(scalar, kui_value_int(2));
        assert_eq!(kui_value_len(scalar), 0);
        kui_value_free(scalar);
        kui_value_free(map);
    }

    /// A host that shows menus itself reads the open one back the way it
    /// reads the bar: count and rows, each row spelled as
    /// `kui_menu_bar_item` spells it — the role's accelerator where the
    /// row declared none, the checked flag a row can carry — because the
    /// two readers are one function. Before this, `kui_set_native_menus`
    /// told a C host to read what is open and gave it nothing to read it
    /// with.
    #[test]
    fn the_open_menu_reads_back_row_for_row() {
        let ctx = kui_ctx_new();
        let mut spec = unsafe { std::mem::zeroed::<KuiSpec>() };
        spec.width = KuiSizing {
            tag: 2,
            value: 100.0,
        };
        spec.height = KuiSizing {
            tag: 2,
            value: 50.0,
        };
        kui_frame_begin(ctx, 200.0, 100.0, 1.0);
        let key = kui_open_with(ctx, ks("card"), &spec, NONE, NONE, NONE, NONE);
        kui_close(ctx);
        kui_frame_finish(ctx);
        assert_ne!(key, 0);

        assert_eq!(
            kui_menu_item_count(
                ctx,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut()
            ),
            0
        );
        assert!(!kui_menu_item(
            ctx,
            0,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            std::ptr::null_mut()
        ));

        kui_set_native_menus(ctx, true);
        let items = [
            KuiMenuItem {
                label: ks(""),
                role: KUI_MENU_COPY,
                enabled: 1,
                id: std::ptr::null(),
                accel: ks(""),
                checked: 0,
                submenu: std::ptr::null(),
                submenu_count: 0,
            },
            KuiMenuItem {
                label: ks("Wrap"),
                role: KUI_MENU_CUSTOM,
                enabled: 1,
                id: std::ptr::null(),
                accel: ks("⌥Z"),
                checked: 1,
                submenu: std::ptr::null(),
                submenu_count: 0,
            },
            KuiMenuItem {
                label: ks("Gone"),
                role: KUI_MENU_CUSTOM,
                enabled: 0,
                id: std::ptr::null(),
                accel: ks(""),
                checked: 0,
                submenu: std::ptr::null(),
                submenu_count: 0,
            },
        ];
        assert!(kui_open_menu(
            ctx,
            key,
            40.0,
            30.0,
            items.as_ptr(),
            items.len()
        ));

        let (mut target, mut x, mut y) = (0u64, 0.0f32, 0.0f32);
        assert_eq!(kui_menu_item_count(ctx, &mut target, &mut x, &mut y), 3);
        assert_eq!((target, x, y), (key, 40.0, 30.0));

        let read = |i: usize| -> (String, String, u32, u32) {
            let (mut label, mut accel, mut role, mut flags) = (ks(""), ks(""), 0u32, 0u32);
            assert!(kui_menu_item(
                ctx, i, &mut label, &mut accel, &mut role, &mut flags
            ));
            (s(label), s(accel), role, flags)
        };
        // The role's default is the platform's (`⌘C` here, `Ctrl+C` on
        // CI's Linux runner), so the test asks the core rather than
        // spelling it.
        assert_eq!(
            read(0),
            (
                "Copy".into(),
                kui_core::MenuRole::Copy.default_accel().into(),
                KUI_MENU_COPY,
                KUI_MENU_ITEM_ENABLED
            ),
            "a standard row reads with its role's wording and shortcut"
        );
        // A declared accelerator kui can parse reads in the platform's
        // spelling too, as the drawn menu shows it (backlog F127).
        assert_eq!(
            read(1),
            (
                "Wrap".into(),
                if cfg!(target_os = "macos") {
                    "⌥Z"
                } else {
                    "Alt+Z"
                }
                .into(),
                KUI_MENU_CUSTOM,
                KUI_MENU_ITEM_ENABLED | KUI_MENU_ITEM_CHECKED
            ),
            "a checked custom row carries its flag and its own accelerator"
        );
        assert_eq!(read(2), ("Gone".into(), String::new(), KUI_MENU_CUSTOM, 0));
        assert!(!kui_menu_item(
            ctx,
            3,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            std::ptr::null_mut()
        ));

        assert!(kui_close_menu(ctx));
        assert_eq!(
            kui_menu_item_count(
                ctx,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut()
            ),
            0
        );
        kui_ctx_free(ctx);
    }

    /// The family names `kui_font_add_system` can take, listed — what
    /// Node's `systemFontFamilies()` answers. A short array is filled as
    /// far as it goes and the total still comes back, like every other
    /// array the library fills.
    #[test]
    fn font_families_list_what_add_system_can_take() {
        let ctx = kui_ctx_new();
        let total = kui_font_families(ctx, std::ptr::null_mut(), 0);
        assert!(total > 0, "the bundled face is always there");
        let mut out = vec![ks(""); total];
        assert_eq!(kui_font_families(ctx, out.as_mut_ptr(), out.len()), total);
        let names: Vec<String> = out.iter().map(|k| s(*k)).collect();
        let mut sorted = names.clone();
        sorted.sort();
        sorted.dedup();
        assert_eq!(names, sorted, "sorted and without repeats");
        assert_ne!(
            kui_font_add_system(ctx, ks(&names[0])),
            0,
            "each name registers"
        );

        let mut one = [ks("")];
        assert_eq!(kui_font_families(ctx, one.as_mut_ptr(), 1), total);
        assert_eq!(s(one[0]), names[0]);
        kui_ctx_free(ctx);
    }

    /// The same families with what their faces say they are
    /// — what Node's `systemFonts()` answers: in `kui_font_families`'
    /// order, each with its weights sorted and once, the two flags 0 or 1
    /// and the core's reading of them. A short array is filled as far as
    /// it goes, and a NULL one asks for the count.
    #[test]
    fn system_fonts_are_the_families_with_what_they_are() {
        let ctx = kui_ctx_new();
        let total = kui_font_families(ctx, std::ptr::null_mut(), 0);
        let mut names = vec![ks(""); total];
        kui_font_families(ctx, names.as_mut_ptr(), total);
        let names: Vec<String> = names.iter().map(|k| s(*k)).collect();

        assert_eq!(kui_system_fonts(ctx, std::ptr::null_mut(), 0), total);
        let blank = KuiSystemFont {
            family: ks(""),
            weights: std::ptr::null(),
            weight_count: 0,
            monospaced: 7,
            italic: 7,
        };
        let mut out = vec![blank; total];
        assert_eq!(kui_system_fonts(ctx, out.as_mut_ptr(), total), total);
        let expected = unsafe { ctx.as_mut() }.unwrap().core().system_fonts();
        for ((font, name), want) in out.iter().zip(&names).zip(&expected) {
            assert_eq!(&s(font.family), name, "kui_font_families' order");
            let weights =
                unsafe { std::slice::from_raw_parts(font.weights, font.weight_count as usize) };
            assert!(!weights.is_empty(), "{name}: a face has a weight");
            assert!(
                weights.windows(2).all(|w| w[0] < w[1]),
                "{name}: {weights:?}"
            );
            assert_eq!(weights, want.weights.as_slice());
            assert_eq!(font.monospaced, u32::from(want.monospaced), "{name}");
            assert_eq!(font.italic, u32::from(want.italic), "{name}");
        }

        let mut one = [blank];
        assert_eq!(kui_system_fonts(ctx, one.as_mut_ptr(), 1), total);
        assert_eq!(s(one[0].family), names[0]);
        assert_eq!(
            kui_system_fonts(std::ptr::null_mut(), one.as_mut_ptr(), 1),
            0
        );
        kui_ctx_free(ctx);
    }
}

#[cfg(test)]
mod follow_headless {
    use super::*;

    fn fixed(w: f32, h: f32) -> KuiSpec {
        let mut spec = unsafe { std::mem::zeroed::<KuiSpec>() };
        spec.width = KuiSizing { tag: 2, value: w };
        spec.height = KuiSizing { tag: 2, value: h };
        spec
    }

    /// An underline's own colour and shape cross the boundary (ABI 17):
    /// `KuiSpan.underline_color` / `.underline_style`, the
    /// same two on `KuiTextStyle`, and `KuiCell.ul` with the shape bits —
    /// a wave is segment quads in that colour, a coloured solid line one
    /// solid quad.
    #[test]
    fn underlines_carry_a_colour_and_a_shape_across_the_boundary() {
        let ctx = kui_ctx_new();
        kui_frame_begin(ctx, 400.0, 200.0, 1.0);
        let mut mono = unsafe { std::mem::zeroed::<KuiTextStyle>() };
        mono.size = 14.0;
        mono.family = 2;
        let mut wavy = unsafe { std::mem::zeroed::<KuiSpan>() };
        wavy.text = ks("value");
        wavy.underline_color = 0xff0000ff;
        wavy.underline_style = KUI_UNDERLINE_WAVY;
        let mut plain = unsafe { std::mem::zeroed::<KuiSpan>() };
        plain.text = ks("let ");
        kui_rich_text(ctx, [plain, wavy].as_ptr(), 2, &mono);
        let mut lined = mono;
        lined.underline_color = 0x00ff00ff;
        kui_text(ctx, ks("warn"), &lined);
        let screen = [KuiCell {
            ch: 'a' as u32,
            fg: 0xffffffff,
            bg: 0,
            flags: kui_core::cells::flags::WAVY as u32,
            ul: 0xff0000ff,
        }; 3];
        let term = fixed(200.0, 20.0);
        kui_cells(
            ctx,
            ks("term"),
            1,
            3,
            screen.as_ptr(),
            3,
            &mono,
            &term,
            NONE,
            NONE,
            NONE,
            0,
            0,
            0,
            0,
            0,
        );
        kui_frame_finish(ctx);
        let mut draw = KuiDrawData::default();
        kui_draw_data(ctx, &mut draw);
        let quads = unsafe { std::slice::from_raw_parts(draw.quads, draw.quad_count) };
        let segs: Vec<_> = quads
            .iter()
            .filter(|q| q.kind == kui_core::QuadKind::Segment as u32)
            .collect();
        assert!(segs.len() >= 6, "a wave and an undercurl: {}", segs.len());
        assert!(segs.iter().all(|q| q.color == [1.0, 0.0, 0.0, 1.0]));
        let solids: Vec<_> = quads
            .iter()
            .filter(|q| q.kind == kui_core::QuadKind::Solid as u32)
            .collect();
        assert_eq!(solids.len(), 1, "the text's coloured line");
        assert_eq!(solids[0].color, [0.0, 1.0, 0.0, 1.0]);
        kui_ctx_free(ctx);
    }

    /// `KuiSpan.bg_radius` crosses the boundary: two rows'
    /// rounded backgrounds are two pieces of the stock join fragment, the
    /// first told there is a row below it and the second one above; a
    /// zeroed field is the square background.
    #[test]
    fn a_span_s_bg_radius_joins_its_background_across_the_boundary() {
        let ctx = kui_ctx_new();
        let mut mono = unsafe { std::mem::zeroed::<KuiTextStyle>() };
        mono.size = 14.0;
        mono.family = 2;
        mono.line_height = 20.0;
        let draw = |radius: f32| -> Vec<(u32, [f32; 16])> {
            kui_frame_begin(ctx, 400.0, 200.0, 1.0);
            for s in ["first line", "second"] {
                let mut span = unsafe { std::mem::zeroed::<KuiSpan>() };
                span.text = ks(s);
                span.bg = 0x3b5bd466;
                span.bg_radius = radius;
                kui_rich_text(ctx, &span, 1, &mono);
            }
            kui_frame_finish(ctx);
            let mut draw = KuiDrawData::default();
            kui_draw_data(ctx, &mut draw);
            let quads = unsafe { std::slice::from_raw_parts(draw.quads, draw.quad_count) };
            let frags = if draw.fragment_count == 0 {
                &[][..]
            } else {
                unsafe { std::slice::from_raw_parts(draw.fragments, draw.fragment_count) }
            };
            let (solid, fragment) = (
                kui_core::QuadKind::Solid as u32,
                kui_core::QuadKind::Fragment as u32,
            );
            quads
                .iter()
                .filter(|q| q.kind == solid || q.kind == fragment)
                .map(|q| {
                    let params = if q.kind == fragment {
                        frags[q.uv[0] as usize].params
                    } else {
                        [0.0; 16]
                    };
                    (q.kind, params)
                })
                .collect()
        };
        let joined = draw(4.0);
        assert_eq!(joined.len(), 2, "a piece a row");
        let fragment = kui_core::QuadKind::Fragment as u32;
        assert!(joined.iter().all(|(k, _)| *k == fragment));
        // params[7] says which neighbours there are: 2 below, 1 above.
        assert_eq!(joined[0].1[7], 2.0);
        assert_eq!(joined[1].1[7], 1.0);
        assert_eq!(joined[0].1[6], 4.0, "the radius, physical px at 1x");
        let square = draw(0.0);
        assert_eq!(square.len(), 2);
        let solid = kui_core::QuadKind::Solid as u32;
        assert!(square.iter().all(|(k, _)| *k == solid));
        kui_ctx_free(ctx);
    }

    /// A press through the C surface with Shift held (`kui_input_modifiers`)
    /// keeps the anchor, which `kui_selection_ends` reads back as the
    /// directed pair; an `on_scroll` grid hears
    /// the wheel as `{kind="scroll", lines}` with the fraction carried,
    /// and nothing else scrolls for it.
    #[test]
    fn a_shift_press_extends_and_an_on_scroll_grid_hears_the_wheel_in_lines() {
        let ctx = kui_ctx_new();
        kui_set_diagnostics(ctx, true);
        let tag = kui_value_map();
        kui_value_map_set(tag, ks("kind"), kui_value_str(ks("term")));
        let build = |ctx: *mut KuiCtx| -> u64 {
            kui_frame_begin(ctx, 300.0, 200.0, 1.0);
            let mut card = fixed(200.0, 60.0);
            card.selectable = 1;
            let card_key = kui_open_keyed(ctx, ks("card"), &card, NONE);
            let mut style = unsafe { std::mem::zeroed::<KuiTextStyle>() };
            style.size = 14.0;
            kui_text(ctx, ks("one"), &style);
            kui_text(ctx, ks("two"), &style);
            kui_text(ctx, ks("three"), &style);
            kui_close(ctx);
            let screen = [KuiCell {
                ch: 'x' as u32,
                fg: 0xffffffff,
                bg: 0,
                flags: 0,
                ul: 0,
            }; 33];
            let mut mono = unsafe { std::mem::zeroed::<KuiTextStyle>() };
            mono.size = 13.0;
            mono.family = 2; // KUI_FONT_MONO
            mono.line_height = 18.0;
            let mut term = fixed(200.0, 54.0);
            term.on_scroll = tag;
            kui_cells(
                ctx,
                ks("term"),
                3,
                11,
                screen.as_ptr(),
                33,
                &mono,
                &term,
                NONE,
                NONE,
                NONE,
                0,
                0,
                0,
                0,
                100,
            );
            kui_frame_finish(ctx);
            card_key
        };
        build(ctx);
        // A click in the first run, then a Shift-click in the third.
        kui_input_cursor(ctx, 1.0, 8.0);
        kui_input_mouse(ctx, true, 1);
        kui_input_mouse(ctx, false, 1);
        let (mut ai, mut ab, mut fi, mut fb) = (-2i64, 99usize, -2i64, 99usize);
        assert!(kui_selection_ends(ctx, &mut ai, &mut ab, &mut fi, &mut fb));
        assert_eq!(
            (ai, ab, fi, fb),
            (-1, 0, -1, 0),
            "a click places both ends together"
        );
        kui_input_modifiers(ctx, KUI_KMOD_SHIFT);
        kui_input_cursor(ctx, 20.0, 45.0);
        kui_input_mouse(ctx, true, 1);
        kui_input_mouse(ctx, false, 1);
        kui_input_modifiers(ctx, 0);
        assert!(kui_selection_ends(ctx, &mut ai, &mut ab, &mut fi, &mut fb));
        assert_eq!((ai, ab), (-1, 0), "the anchor stayed");
        assert!(fb > 0, "and the focus moved: {fb}");
        let mut text = KuiStr {
            ptr: std::ptr::null(),
            len: 0,
        };
        assert!(kui_selection_text(ctx, &mut text));
        assert!(kstr(text).starts_with("one\ntwo\n"), "{:?}", kstr(text));
        assert!(
            kui_selection_ends(
                ctx,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut()
            ),
            "NULL outs are fine"
        );

        // The wheel over the grid: two and a half lines is two, a half
        // carried; the next half is the carried half made whole.
        build(ctx);
        kui_input_cursor(ctx, 20.0, 60.0 + 20.0);
        kui_input_scroll(ctx, 0.0, -45.0);
        kui_input_scroll(ctx, 0.0, -9.0);
        let mut ev = KuiEvent::default();
        let mut lines = Vec::new();
        while kui_poll_event(ctx, &mut ev) {
            let mut kind = KuiStr {
                ptr: std::ptr::null(),
                len: 0,
            };
            kui_value_as_str(kui_value_get(ev.payload, ks("kind")), &mut kind);
            if &*kstr(kind) != "scroll" {
                continue;
            }
            let mut n = 0i64;
            assert!(kui_value_as_int(
                kui_value_get(ev.payload, ks("lines")),
                &mut n
            ));
            let mut t = KuiStr {
                ptr: std::ptr::null(),
                len: 0,
            };
            kui_value_as_str(
                kui_value_get(kui_value_get(ev.payload, ks("tag")), ks("kind")),
                &mut t,
            );
            assert_eq!(&*kstr(t), "term");
            lines.push(n);
        }
        assert_eq!(lines, vec![2, 1]);
        kui_value_free(tag);
        kui_ctx_free(ctx);
    }

    /// The three doors B1a's table closed for C: a grid's
    /// selection reads back through `kui_cell_selection` as absolute
    /// lines and columns, directed, and false while the window's
    /// selection is not a grid's; the devtools readers answer what the
    /// setters took; and `kui_nodes` is the frame's node list once
    /// `kui_set_inspect` asked for it, as maps the value readers walk.
    #[test]
    fn the_cell_selection_the_devtools_readers_and_the_node_list_cross() {
        let ctx = kui_ctx_new();
        let build = |ctx: *mut KuiCtx| {
            kui_frame_begin(ctx, 300.0, 200.0, 1.0);
            let screen = [KuiCell {
                ch: 'x' as u32,
                fg: 0xffffffff,
                bg: 0,
                flags: 0,
                ul: 0,
            }; 33];
            let mut mono = unsafe { std::mem::zeroed::<KuiTextStyle>() };
            mono.size = 13.0;
            mono.family = 2; // KUI_FONT_MONO
            mono.line_height = 18.0;
            let mut term = fixed(200.0, 54.0);
            term.selectable = 1;
            kui_cells(
                ctx,
                ks("term"),
                3,
                11,
                screen.as_ptr(),
                33,
                &mono,
                &term,
                NONE,
                NONE,
                NONE,
                0,
                0,
                0,
                0,
                700,
            );
            kui_frame_finish(ctx);
        };
        build(ctx);
        let mut out = (0u64, 0u64, 0usize, 0u64, 0usize, true);
        let read = |ctx: *mut KuiCtx, out: &mut (u64, u64, usize, u64, usize, bool)| {
            kui_cell_selection(
                ctx, &mut out.0, &mut out.1, &mut out.2, &mut out.3, &mut out.4, &mut out.5,
            )
        };
        assert!(!read(ctx, &mut out), "nothing selected yet");
        // A drag from row 1 col 4 back to row 0 col 1: the cell width is
        // the mono `M`, read through the grid's own metrics.
        let mut m = KuiTextMetrics::default();
        let mut mono = unsafe { std::mem::zeroed::<KuiTextStyle>() };
        mono.size = 13.0;
        mono.family = 2;
        mono.line_height = 18.0;
        assert!(kui_measure_text(ctx, ks("M"), &mono, -1.0, &mut m));
        let at = |r: f32, c: f32| (m.width * (c + 0.5), 18.0 * (r + 0.5));
        let (x, y) = at(1.0, 4.0);
        kui_input_cursor(ctx, x, y);
        kui_input_mouse(ctx, true, 1);
        let (x, y) = at(0.0, 1.0);
        kui_input_cursor(ctx, x, y);
        kui_input_mouse(ctx, false, 1);
        assert!(read(ctx, &mut out));
        assert_eq!(out.0, kui_key_of(ctx, ks("term")));
        assert_eq!((out.1, out.2), (701, 4), "the press, row 1 of line 700");
        assert_eq!((out.3, out.4), (700, 1), "the pointer, backwards");
        assert!(!out.5, "linewise");
        assert!(
            kui_cell_selection(
                ctx,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut()
            ),
            "NULL outs are fine"
        );

        // The devtools readers.
        assert!(!kui_devtools(ctx));
        kui_set_devtools(ctx, true);
        assert!(kui_devtools(ctx));
        assert!(kui_set_devtools_dock(ctx, ks("left")));
        let mut dock = KuiStr {
            ptr: std::ptr::null(),
            len: 0,
        };
        assert!(kui_devtools_dock(ctx, &mut dock));
        assert_eq!(&*kstr(dock), "left");
        assert!(kui_set_devtools_theme(ctx, ks("dark"), 0xff8800ff));
        assert!(!kui_set_devtools_theme(ctx, ks("sepia"), 0), "not a base");
        assert!(kui_set_devtools_theme(ctx, ks(""), 0), "the app's own");
        let keys = [ks("⌘K"), ks("Esc")];
        let what = [ks("palette"), ks("close")];
        kui_set_devtools_legend(ctx, keys.as_ptr(), what.as_ptr(), 2);
        let mut key = KuiStr {
            ptr: std::ptr::null(),
            len: 0,
        };
        assert!(kui_devtools_key(ctx, &mut key));
        assert_eq!(&*kstr(key), "ctrl+shift+i", "the default");
        assert!(kui_set_devtools_key(ctx, ks("F12")));
        assert!(!kui_set_devtools_key(ctx, ks("f99")), "not a key");
        assert!(kui_devtools_key(ctx, &mut key));
        assert_eq!(&*kstr(key), "f12", "set, and a bad spelling left it");
        // The facts a declared tab reads, and the writer.
        assert_eq!(kui_devtools_selected(ctx), 0);
        assert_eq!(kui_devtools_hovered(ctx), 0);
        assert_eq!(kui_devtools_picked(ctx), 0);
        let grid_key = kui_key_of(ctx, ks("term"));
        assert_ne!(grid_key, 0);
        kui_set_devtools_selected(ctx, grid_key);
        assert_eq!(
            kui_devtools_selected(ctx),
            grid_key,
            "selected from outside the panel"
        );
        kui_set_devtools_selected(ctx, 0);
        assert_eq!(kui_devtools_selected(ctx), 0, "and cleared");
        assert!(!kui_devtools_picking(ctx));
        kui_set_devtools_pick(ctx, true);
        assert!(kui_devtools_picking(ctx), "the picker raised from outside");
        kui_set_devtools_pick(ctx, false);
        assert!(!kui_devtools_picking(ctx));
        // The tab, selected from outside and read back.
        let mut tab = KuiStr {
            ptr: std::ptr::null(),
            len: 0,
        };
        assert!(kui_devtools_current_tab(ctx, &mut tab));
        assert_eq!(&*kstr(tab), "tree", "the pick showed the tree tab");
        assert!(
            kui_set_devtools_tab(ctx, ks("facts")),
            "one of the panel's own"
        );
        assert!(kui_devtools_current_tab(ctx, &mut tab));
        assert_eq!(&*kstr(tab), "facts");
        assert!(
            !kui_set_devtools_tab(ctx, ks("nobody")),
            "a name no frame declared: kept, not listed"
        );
        assert!(kui_devtools_current_tab(ctx, &mut tab));
        assert_eq!(&*kstr(tab), "facts", "the strip falls back");
        kui_set_devtools(ctx, false);

        // The node list: empty until asked for, then one map per node
        // with the grid's label and its `cells` kind among them.
        let list = kui_nodes(ctx);
        assert!(!list.is_null());
        assert_eq!(kui_value_len(list), 0, "not inspecting yet");
        kui_set_inspect(ctx, true);
        build(ctx);
        let list = kui_nodes(ctx);
        let n = kui_value_len(list);
        assert!(n >= 2, "a root and a grid: {n}");
        let mut labels = Vec::new();
        for i in 0..n {
            let node = kui_value_at(list, i);
            let mut label = KuiStr {
                ptr: std::ptr::null(),
                len: 0,
            };
            if kui_value_as_str(kui_value_get(node, ks("label")), &mut label) {
                labels.push(kstr(label).into_owned());
            }
            assert!(
                !kui_value_get(node, ks("events")).is_null(),
                "every node carries its events map"
            );
        }
        assert!(labels.iter().any(|l| l == "term"), "{labels:?}");
        kui_ctx_free(ctx);
    }

    /// `kui_host_rect`: the frame's host viewport with its
    /// origin, which under a left dock starts at the pane's width and
    /// under a bottom one is the window less the strip; scaled, it is the
    /// box the host's own quads land in.
    #[test]
    fn the_host_rect_is_the_frame_s_viewport_with_its_origin() {
        let ctx = kui_ctx_new();
        let frame = |ctx: *mut KuiCtx, scale: f32| {
            kui_frame_begin(ctx, 1040.0, 720.0, scale);
            let mut fill = unsafe { std::mem::zeroed::<KuiSpec>() };
            fill.width = KuiSizing { tag: 1, value: 1.0 };
            fill.height = KuiSizing { tag: 1, value: 1.0 };
            fill.bg = 0xff00ffff;
            kui_open(ctx, &fill, std::ptr::null_mut());
            kui_close(ctx);
            kui_frame_finish(ctx);
        };
        let read = |ctx: *mut KuiCtx| {
            let mut r = KuiLayoutRect::default();
            assert!(kui_host_rect(ctx, &mut r));
            (r.x, r.y, r.w, r.h)
        };
        assert_eq!(read(ctx), (0.0, 0.0, 0.0, 0.0), "before the first frame");
        frame(ctx, 1.0);
        assert_eq!(read(ctx), (0.0, 0.0, 1040.0, 720.0), "no dock: the window");
        kui_set_devtools(ctx, true);
        assert!(kui_set_devtools_dock(ctx, ks("left")));
        frame(ctx, 2.0);
        let (x, y, w, h) = read(ctx);
        assert_eq!((x, y, w, h), (340.0, 0.0, 700.0, 720.0), "left of the pane");
        let mut draw = KuiDrawData::default();
        assert!(kui_draw_data(ctx, &mut draw));
        let quads = unsafe { std::slice::from_raw_parts(draw.quads, draw.quad_count) };
        let fill = quads
            .iter()
            .find(|q| q.color == [1.0, 0.0, 1.0, 1.0])
            .expect("the host's fill");
        assert_eq!(
            (fill.x, fill.y, fill.w, fill.h),
            (x * 2.0, y * 2.0, w * 2.0, h * 2.0),
            "the host's quad is the rect, in physical px"
        );
        assert!(kui_set_devtools_dock(ctx, ks("bottom")));
        frame(ctx, 1.0);
        assert_eq!(read(ctx), (0.0, 0.0, 1040.0, 440.0), "above the strip");
        assert!(!kui_host_rect(ctx, std::ptr::null_mut()), "a NULL out");
        kui_ctx_free(ctx);
    }
}

/// `kui_run_with`'s two halves that need no window: the
/// `KuiRunConfig` reading, and the context's core changing hands.
#[cfg(test)]
mod run_config_headless {
    use super::*;

    #[test]
    fn a_run_config_reads_as_the_launcher_s_options_and_refuses_a_word_it_lacks() {
        // NULL and every zero are the same: the launcher's own defaults.
        assert_eq!(run_options_of(None).unwrap(), RunOptions::default());
        let zero = KuiRunConfig::default();
        assert_eq!(run_options_of(Some(&zero)).unwrap(), RunOptions::default());

        let full = KuiRunConfig {
            width: 800.0,
            height: 600.0,
            min_w: 400.0,
            min_h: 0.0,
            max_w: 0.0,
            max_h: 900.0,
            chrome: KUI_CHROME_CUSTOM,
            text_aa: KUI_TEXT_AA_GRAYSCALE,
            diagnostics: KUI_DIAG_OFF,
            frame_latency: 1,
            backdrop: 3, // KUI_BACKDROP_TINTED
        };
        assert_eq!(
            run_options_of(Some(&full)).unwrap(),
            RunOptions {
                size: Some((800.0, 600.0)),
                // A zero side of a bound is unbounded, as Node's lone
                // `minWidth` / `maxHeight` are.
                min_size: Some((400.0, 0.0)),
                max_size: Some((UNBOUNDED_SIZE, 900.0)),
                chrome: KUI_CHROME_CUSTOM,
                text_aa: KUI_TEXT_AA_GRAYSCALE,
                diagnostics: Some(false),
                frame_latency: Some(1),
                backdrop: kui_core::Backdrop::Tinted,
            }
        );
        assert_eq!(
            run_options_of(Some(&KuiRunConfig {
                diagnostics: KUI_DIAG_ON,
                ..zero
            }))
            .unwrap()
            .diagnostics,
            Some(true)
        );

        // Refused with the reason, not degraded: a window that opened
        // native when asked for a chrome this build lacks would draw its
        // titlebar under the OS's.
        let refused = |c: KuiRunConfig| run_options_of(Some(&c)).unwrap_err();
        assert!(
            refused(KuiRunConfig { chrome: 3, ..zero }).contains("KUI_CHROME_BORDERLESS, not 3")
        );
        assert!(
            refused(KuiRunConfig { text_aa: 9, ..zero }).contains("KUI_TEXT_AA_SUBPIXEL, not 9")
        );
        assert!(
            refused(KuiRunConfig {
                diagnostics: 3,
                ..zero
            })
            .contains("KUI_DIAG_OFF, not 3")
        );
        assert!(
            refused(KuiRunConfig {
                width: 320.0,
                ..zero
            })
            .contains("width and height go together")
        );
        assert!(
            refused(KuiRunConfig {
                min_h: -1.0,
                ..zero
            })
            .contains("min_h must be")
        );
        assert!(
            refused(KuiRunConfig {
                max_w: f32::NAN,
                ..zero
            })
            .contains("max_w must be")
        );
        assert!(
            refused(KuiRunConfig {
                backdrop: 4,
                ..zero
            })
            .contains("KUI_BACKDROP_TINTED, not 4")
        );
    }

    #[test]
    fn a_context_s_core_changes_hands_and_the_context_stays_a_context() {
        let ctx = kui_ctx_new();
        let c = unsafe { &mut *ctx };
        let image = c.core().resources.add_image(2, 2, vec![0; 16]);
        c.core().set_devtools(true);

        let mut taken = c.take_core().expect("a standalone context owns its core");
        assert_eq!(
            taken.resources.image_size(image),
            Some((2, 2)),
            "the registration went with the core"
        );
        assert!(taken.devtools());

        // What is left is a context that has registered nothing, and is
        // still one: a frame builds on it and it frees.
        assert_eq!(c.core().resources.image_size(image), None);
        assert!(!c.core().devtools());
        assert!(
            !c.core().diagnostics(),
            "off until asked, as kui_ctx_new leaves it"
        );
        kui_frame_begin(ctx, 100.0, 100.0, 1.0);
        let text = KuiStr {
            ptr: "still here".as_ptr(),
            len: 10,
        };
        kui_text(ctx, text, std::ptr::null());
        kui_frame_finish(ctx);
        let mut dd = KuiDrawData {
            size: std::mem::size_of::<KuiDrawData>() as u32,
            ..Default::default()
        };
        assert!(kui_draw_data(ctx, &mut dd));
        assert!(dd.quad_count > 0);
        // A borrowing context has no core of its own to hand over.
        let mut borrowing = KuiCtx::borrowing(&mut taken);
        assert!(borrowing.take_core().is_none());
        kui_ctx_free(ctx);
    }

    /// RG20: an eased scroll (F80) is owed like any leg — its own
    /// `KUI_OWED_SCROLL` bit — so `kui_animating` is still `kui_owed !=
    /// 0` and a host scheduling on the bits does not freeze mid-glide.
    #[test]
    fn an_eased_scroll_is_its_own_owed_bit() {
        use kui_core::{Easing, NodeSpec, Size, Transition};
        let ctx = kui_ctx_new();
        let c = unsafe { &mut *ctx };
        let build = |core: &mut kui_core::Core, reveal: bool| {
            let mut ui = core.frame(Size::new(100.0, 50.0), 1.0);
            ui.configure_root(NodeSpec::column().fill());
            let spec = NodeSpec::row()
                .fill()
                .scroll_x()
                .transition_with(Transition::ms(100.0).easing(Easing::Linear));
            let mut last = None;
            ui.with_keyed("row", spec, |ui| {
                for i in 0..4 {
                    let w = NodeSpec::column().width(100.0).grow_height();
                    last = Some(ui.leaf_keyed(&format!("b{i}"), w));
                }
            });
            if reveal {
                ui.reveal(last.expect("four boxes"));
            }
            ui.finish();
        };
        c.core().set_time(0.0);
        build(c.core(), false);
        build(c.core(), true);
        let owed = kui_owed(ctx);
        assert_ne!(owed & KUI_OWED_SCROLL, 0, "the leg has its bit: {owed}");
        assert!(kui_animating(ctx), "kui_animating is kui_owed != 0");
        c.core().set_time(0.2);
        build(c.core(), false);
        assert_eq!(kui_owed(ctx), 0, "landed");
        kui_ctx_free(ctx);
    }

    /// F111: a frame's reasons as bits — the input handed in, the host's
    /// note, `KUI_FRAME_CAUSE_OWED` — and, traced, whether it drew what
    /// the frame before drew.
    #[test]
    fn a_frame_cause_is_bits_and_an_unchanged_frame_is_one() {
        use kui_core::{NodeSpec, Size};
        let ctx = kui_ctx_new();
        let c = unsafe { &mut *ctx };
        let frame = |core: &mut kui_core::Core, ask: bool| {
            let mut ui = core.frame(Size::new(100.0, 50.0), 1.0);
            ui.leaf_keyed("b", NodeSpec::column().size(10.0, 10.0));
            if ask {
                ui.request_frame();
            }
            ui.finish();
        };
        kui_set_frame_trace(ctx, true);
        frame(c.core(), false);
        assert_eq!(kui_frame_unchanged(ctx), -1, "nothing to compare with");
        kui_input_cursor(ctx, 5.0, 5.0);
        kui_note_frame_cause(ctx, KUI_FRAME_CAUSE_WAKE);
        frame(c.core(), true);
        assert_eq!(
            kui_frame_cause(ctx),
            KUI_FRAME_CAUSE_POINTER_MOVE | KUI_FRAME_CAUSE_WAKE
        );
        assert_eq!(kui_frame_unchanged(ctx), 1);
        frame(c.core(), false);
        assert_eq!(kui_frame_cause(ctx), KUI_FRAME_CAUSE_OWED);
        kui_set_frame_trace(ctx, false);
        assert_eq!(kui_frame_unchanged(ctx), -1);
        kui_ctx_free(ctx);
    }

    /// RG124: a C fragment that declares `KuiSpec.tooltip` draws its hint
    /// while hovered, in both forms, as `kui_open` and the leaf doors do
    /// and as JSX and Lua always did. It was hover-tracked and spoken, and
    /// nothing floated.
    #[test]
    fn a_fragments_tooltip_is_drawn_while_hovered() {
        let ctx = kui_ctx_new();
        let frag = kui_fragment_add(
            ctx,
            ks(
                "fn fragment(in: FragmentIn, params: array<vec4<f32>, 4>) -> vec4<f32> {\n    return params[0];\n}\n",
            ),
        );
        assert_ne!(frag, 0);
        let quads = |open: bool| {
            kui_frame_begin(ctx, 320.0, 240.0, 1.0);
            let root: KuiSpec = unsafe { std::mem::zeroed() };
            kui_root(ctx, &root);
            let mut card: KuiSpec = unsafe { std::mem::zeroed() };
            card.width = KuiSizing {
                tag: 2,
                value: 100.0,
            };
            card.height = KuiSizing {
                tag: 2,
                value: 60.0,
            };
            card.tooltip = ks("a hint");
            if open {
                kui_fragment_open(ctx, ks("card"), frag, std::ptr::null(), 0, &card);
                kui_close(ctx);
            } else {
                kui_fragment(ctx, frag, std::ptr::null(), 0, &card);
            }
            kui_frame_finish(ctx);
            let mut draw = KuiDrawData::default();
            kui_draw_data(ctx, &mut draw);
            draw.quad_count
        };
        for open in [false, true] {
            kui_input_cursor(ctx, 300.0, 230.0);
            let away = quads(open);
            kui_input_cursor(ctx, 20.0, 20.0);
            quads(open);
            let over = quads(open);
            assert!(
                over > away,
                "open={open}: {over} quads hovered, {away} away — the hint floats"
            );
        }
        kui_ctx_free(ctx);
    }
}

/// `KuiSpec.min_w`'s three spellings: 0 is undeclared, as a
/// zeroed struct leaves it — the content's floor in a share's row —
/// `KUI_MIN_NONE` a declared 0, `KUI_MIN_FIT` the fit floor; and the spec
/// the conversion builds keeps undeclared undeclared.
#[test]
fn a_min_of_zero_is_undeclared_and_min_none_declares_zero() {
    use crate::convert::min_of;
    use crate::types::{KUI_MIN_FIT, KUI_MIN_NONE};
    assert!(min_of(0.0).is_auto());
    let none = min_of(KUI_MIN_NONE);
    assert!(!none.is_auto() && !none.is_fit());
    assert_eq!(none.resolved(), 0.0);
    assert!(min_of(KUI_MIN_FIT).is_fit());
    assert_eq!(min_of(12.0).resolved(), 12.0);
}

/// What a door hands out stays where kui.h says it stays: each of these
/// was freed by some other call first (the 2026-10-07 round's audit), and
/// each is pinned here by the pointer the host was given still being the
/// one the context holds, or by what was left waiting.
#[cfg(test)]
mod borrows {
    use super::*;

    fn zspec() -> KuiSpec {
        unsafe { std::mem::zeroed() }
    }
    fn none() -> KuiStr {
        KuiStr {
            ptr: std::ptr::null(),
            len: 0,
        }
    }

    /// Two `kui_draw_data` calls in a frame hand out the same arrays; the
    /// second one freed the first's. A frame with none is NULL, not an
    /// empty Vec's dangling pointer.
    #[test]
    fn draw_data_twice_in_a_frame_hands_out_the_same_arrays() {
        let ctx = kui_ctx_new();
        let frame = |polygon: bool| {
            kui_frame_begin(ctx, 200.0, 200.0, 1.0);
            kui_root(ctx, &zspec());
            if polygon {
                let mut s = zspec();
                s.bg = 0xff0000ff;
                let xy = [[0.0f32, 0.0], [50.0, 0.0], [25.0, 40.0]];
                let null = std::ptr::null_mut();
                kui_polygon(ctx, none(), xy.as_ptr().cast(), 3, &s, null, null, null);
            }
            kui_frame_finish(ctx);
        };
        frame(true);
        let (mut a, mut b) = (KuiDrawData::default(), KuiDrawData::default());
        assert!(kui_draw_data(ctx, &mut a));
        assert!(a.fragment_count >= 1, "a polygon is a fragment draw");
        let first = unsafe { *a.fragments };
        assert!(kui_draw_data(ctx, &mut b));
        assert_eq!(a.fragments, b.fragments, "the same array, not a new one");
        assert_eq!(unsafe { (*a.fragments).fragment }, first.fragment);
        assert_eq!(a.fragments, unsafe { (*ctx).fragment_draws.as_ptr() });
        // The next frame transcribes again, and an empty list is NULL.
        frame(false);
        assert!(kui_draw_data(ctx, &mut a));
        assert_eq!((a.fragments, a.fragment_count), (std::ptr::null(), 0));
        assert_eq!((a.textures, a.texture_count), (std::ptr::null(), 0));
        kui_ctx_free(ctx);
    }

    /// A `kui_draw_data` while the frame builds reads the list
    /// `kui_frame_begin` emptied; the one after `kui_frame_finish` reads
    /// what the frame drew, its fragment draws with it, where it handed
    /// out the empty arrays the first call cached.
    #[test]
    fn draw_data_mid_build_leaves_the_finished_frame_its_fragments() {
        let ctx = kui_ctx_new();
        kui_frame_begin(ctx, 200.0, 200.0, 1.0);
        let mut draw = KuiDrawData::default();
        assert!(kui_draw_data(ctx, &mut draw));
        assert_eq!(draw.fragment_count, 0);
        kui_root(ctx, &zspec());
        let mut s = zspec();
        s.bg = 0xff0000ff;
        let xy = [[0.0f32, 0.0], [50.0, 0.0], [25.0, 40.0]];
        let null = std::ptr::null_mut();
        kui_polygon(ctx, none(), xy.as_ptr().cast(), 3, &s, null, null, null);
        kui_frame_finish(ctx);
        assert!(kui_draw_data(ctx, &mut draw));
        assert!(draw.fragment_count >= 1, "a polygon is a fragment draw");
        assert!(!draw.fragments.is_null());
        kui_ctx_free(ctx);
    }

    /// `kui_access_runs` leaves the strings `kui_access_tree` handed out
    /// alone; it replaced the whole tree they live in.
    #[test]
    fn access_runs_leave_the_tree_its_strings() {
        let ctx = kui_ctx_new();
        let mut spec = zspec();
        spec.width = KuiSizing {
            tag: 2,
            value: 300.0,
        };
        spec.label = ks("Document name");
        kui_frame_begin(ctx, 400.0, 200.0, 1.0);
        let key = kui_text_edit(
            ctx,
            ks("doc"),
            ks("hello world"),
            std::ptr::null(),
            2,
            &spec,
        );
        kui_frame_finish(ctx);
        let mut nodes = [unsafe { std::mem::zeroed::<KuiAccessNode>() }; 4];
        let n = kui_access_tree(ctx, nodes.as_mut_ptr(), nodes.len());
        let ed = *nodes[..n].iter().find(|n| n.key == key).unwrap();
        assert_eq!(&*kstr(ed.name), "Document name");
        let mut runs = [unsafe { std::mem::zeroed::<KuiAccessRun>() }; 4];
        assert!(kui_access_runs(ctx, key, runs.as_mut_ptr(), runs.len()) >= 1);
        assert_eq!(&*kstr(runs[0].text), "hello world");
        let held = unsafe { &(*ctx).last_access };
        let node = held.get(Key(key)).unwrap();
        assert_eq!(
            node.name.as_deref().unwrap().as_ptr(),
            ed.name.ptr,
            "still the tree handed out"
        );
        assert_eq!(node.value.as_deref().unwrap().as_ptr(), ed.value.ptr);
        kui_ctx_free(ctx);
    }

    /// A taken menu action's text outlives reading a menu row and asking
    /// for a copy, which wrote over it; and an `out` the door cannot write
    /// leaves the action queued, where it was dropped.
    #[test]
    fn a_menu_actions_text_outlives_reading_a_menu() {
        let ctx = kui_ctx_new();
        kui_frame_begin(ctx, 200.0, 100.0, 1.0);
        kui_root(ctx, &zspec());
        let key = kui_open_keyed(ctx, ks("t"), &zspec(), std::ptr::null_mut());
        kui_close(ctx);
        kui_frame_finish(ctx);
        kui_set_clipboard(ctx, ks("copied text payload"), none());
        assert!(!kui_take_menu_action(ctx, std::ptr::null_mut()), "no out");
        let mut a: KuiMenuAction = unsafe { std::mem::zeroed() };
        a.size = std::mem::size_of::<KuiMenuAction>() as u32;
        assert!(kui_take_menu_action(ctx, &mut a), "still queued");
        assert_eq!(a.kind, KUI_MENU_ACTION_SET_CLIPBOARD);
        let item = KuiMenuItem {
            label: ks("Row"),
            role: KUI_MENU_CUSTOM,
            enabled: 1,
            id: std::ptr::null(),
            accel: none(),
            checked: 0,
            submenu: std::ptr::null(),
            submenu_count: 0,
        };
        assert!(kui_open_menu(ctx, key, 10.0, 10.0, &item, 1));
        let mut label = none();
        let null = std::ptr::null_mut();
        assert!(kui_menu_item(
            ctx,
            0,
            &mut label,
            std::ptr::null_mut(),
            null,
            null
        ));
        assert_eq!(&*kstr(label), "Row");
        let mut copy = none();
        kui_request_copy(ctx, &mut copy);
        assert_eq!(a.text.ptr, unsafe { (*ctx).menu_text.as_ptr() });
        assert_eq!(&*kstr(a.text), "copied text payload");
        kui_ctx_free(ctx);
    }

    /// A C row's submenu reaches the core in all three menus — the context
    /// menu, the select and the bar — nested, and a row that is its own
    /// submenu is refused rather than read until the stack runs out
    /// (backlog F128).
    #[test]
    fn a_c_submenu_reaches_every_menu_and_a_cycle_is_refused() {
        let row = |label: &'static str| KuiMenuItem {
            label: ks(label),
            role: KUI_MENU_CUSTOM,
            enabled: 1,
            id: std::ptr::null(),
            accel: none(),
            checked: 0,
            submenu: std::ptr::null(),
            submenu_count: 0,
        };
        let deeper = [row("Oldest first")];
        let mut date = row("Date");
        date.submenu = deeper.as_ptr();
        date.submenu_count = 1;
        let sort = [row("Name"), date];
        let mut sort_by = row("Sort by");
        sort_by.submenu = sort.as_ptr();
        sort_by.submenu_count = 2;
        let rows = [row("Open"), sort_by];

        let ctx = kui_ctx_new();
        kui_frame_begin(ctx, 200.0, 100.0, 1.0);
        kui_root(ctx, &zspec());
        let key = kui_open_keyed(ctx, ks("t"), &zspec(), std::ptr::null_mut());
        kui_close(ctx);
        let select = kui_select(ctx, ks("Order"), rows.as_ptr(), rows.len(), 0);
        let menus = [KuiMenu {
            label: ks("View"),
            items: rows.as_ptr(),
            count: rows.len(),
            enabled: 1,
        }];
        assert!(kui_menu_bar(ctx, menus.as_ptr(), 1));
        kui_frame_finish(ctx);
        assert_ne!(select, 0, "the select takes a row with a submenu");

        assert!(kui_open_menu(
            ctx,
            key,
            10.0,
            10.0,
            rows.as_ptr(),
            rows.len()
        ));
        let c = unsafe { &mut *ctx };
        let menu = c.core().menu().expect("open").clone();
        let inner = kui_core::MenuItem::at_path(&menu.items, &[1, 1, 0]).expect("three deep");
        assert_eq!(inner.label, "Oldest first");
        let bar = c.core().menu_bar().expect("declared").clone();
        assert_eq!(
            bar.item_at(0, &[1, 0]).map(|r| r.label.as_str()),
            Some("Name")
        );
        let path = [1usize, 1];
        assert_eq!(kui_menu_submenu_count(ctx, path.as_ptr(), 2), 1);
        assert_eq!(kui_menu_bar_submenu_count(ctx, 0, path.as_ptr(), 2), 1);
        assert_eq!(
            kui_menu_submenu_count(ctx, std::ptr::null(), 1),
            0,
            "a NULL path with a depth reads nothing"
        );
        assert!(!kui_activate_menu_path(ctx, std::ptr::null(), 0));
        kui_close_menu(ctx);

        // A row whose submenu is itself.
        let mut cycle = [row("Again")];
        cycle[0].submenu = cycle.as_ptr();
        cycle[0].submenu_count = 1;
        assert!(
            !kui_open_menu(ctx, key, 10.0, 10.0, cycle.as_ptr(), 1),
            "refused, as an unknown role is"
        );
        assert_eq!(
            kui_menu_item_count(
                ctx,
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                std::ptr::null_mut()
            ),
            0
        );
        kui_ctx_free(ctx);
    }

    /// `KuiSpec.backdrop_blur` reaches the display list as a
    /// `KUI_QUAD_BACKDROP` before the node's background, its radius in
    /// physical px (backlog F129).
    #[test]
    fn a_backdrop_blur_draws_a_backdrop_quad() {
        let ctx = kui_ctx_new();
        kui_frame_begin(ctx, 200.0, 100.0, 2.0);
        kui_root(ctx, &zspec());
        let mut spec = zspec();
        spec.width = KuiSizing {
            tag: 2,
            value: 80.0,
        };
        spec.height = KuiSizing {
            tag: 2,
            value: 40.0,
        };
        spec.bg = 0xffffff40;
        spec.backdrop_blur = 10.0;
        kui_open_keyed(ctx, ks("glass"), &spec, std::ptr::null_mut());
        kui_close(ctx);
        kui_frame_finish(ctx);
        let mut dd: KuiDrawData = unsafe { std::mem::zeroed() };
        dd.size = std::mem::size_of::<KuiDrawData>() as u32;
        assert!(kui_draw_data(ctx, &mut dd));
        let quads = unsafe { std::slice::from_raw_parts(dd.quads, dd.quad_count) };
        let backdrop = kui_core::QuadKind::Backdrop as u32;
        let i = quads
            .iter()
            .position(|q| q.kind == backdrop)
            .expect("a KUI_QUAD_BACKDROP");
        assert_eq!(backdrop, 9, "KUI_QUAD_BACKDROP");
        assert_eq!(quads[i].blur, 20.0, "10 logical px at scale 2");
        assert_eq!(
            quads[i + 1].kind,
            kui_core::QuadKind::Solid as u32,
            "the bg over it"
        );
        kui_ctx_free(ctx);
    }

    /// `kui_take_warnings` with a short `cap` leaves the rest for the next
    /// call, as kui.h says; it dropped them.
    #[test]
    fn warnings_past_cap_wait() {
        let ctx = kui_ctx_new();
        kui_set_diagnostics(ctx, true);
        kui_frame_begin(ctx, 200.0, 100.0, 1.0);
        kui_root(ctx, &zspec());
        for name in ["a", "a", "b", "b", "c", "c"] {
            kui_slot(ctx, ks(name), std::ptr::null());
        }
        kui_frame_finish(ctx);
        let mut out = [unsafe { std::mem::zeroed::<KuiWarning>() }; 8];
        let mut all = Vec::new();
        loop {
            let n = kui_take_warnings(ctx, out.as_mut_ptr(), 1);
            if n == 0 {
                break;
            }
            assert_eq!(n, 1);
            all.push(kstr(out[0].code).into_owned());
        }
        assert!(all.len() >= 3, "every warning, one a call: {all:?}");
        kui_ctx_free(ctx);
    }
}
