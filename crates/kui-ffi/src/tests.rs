//! Headless tests of the entry points, over a standalone context.

use super::*;

#[cfg(test)]
mod widgets_headless {
    use super::*;

    fn ks(s: &str) -> KuiStr {
        KuiStr {
            ptr: s.as_ptr(),
            len: s.len(),
        }
    }

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

    /// The metrics cross the boundary both ways (backlog T2): the stock
    /// set reads back as the constants, a set the host wrote is what the
    /// next button is built from, NULL restores the stock set, and a short
    /// reservation is refused as every [out] struct's is.
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
}

#[cfg(test)]
mod window_commands_headless {
    use super::*;

    /// `MAIN`, which the header spells for C.
    const MAIN: u32 = WindowId::MAIN.0;

    fn ks(s: &str) -> KuiStr {
        KuiStr {
            ptr: s.as_ptr(),
            len: s.len(),
        }
    }

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
    /// than only described (ADR 0006; `abi.rs`'s note on ABI 7).
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

    /// ADR 0004 decision 9 through the C surface: the kind and the anchor
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
        assert_eq!(
            unsafe { &(*ev.payload).0 }
                .get("kind")
                .and_then(Value::as_str),
            Some("window")
        );
        assert!(!kui_poll_event(ctx, &raw mut ev));

        kui_window_dismissed(ctx, 1, KUI_DISMISS_ESCAPE);
        assert!(kui_poll_event(ctx, &raw mut ev));
        let payload = unsafe { &(*ev.payload).0 };
        assert_eq!(payload.get("kind").and_then(Value::as_str), Some("dismiss"));
        assert_eq!(
            payload.get("reason").and_then(Value::as_str),
            Some("escape")
        );
        assert_eq!(payload.get("name").and_then(Value::as_str), Some("menu"));
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
        assert_eq!(payload.0.get("kind").and_then(Value::as_str), Some("sound"));
        assert_eq!(payload.0.get("tag").and_then(Value::as_str), Some("music"));
        assert_eq!(
            payload.0.get("playback").and_then(Value::as_int),
            Some(music as i64)
        );
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
}

#[cfg(test)]
mod queries_headless {
    use super::*;

    fn ks(s: &str) -> KuiStr {
        KuiStr {
            ptr: s.as_ptr(),
            len: s.len(),
        }
    }

    /// A host that never saw an event from a node names it by the label it
    /// opened it under (backlog F5): `kui_key_of` hands back the key the
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
        while kui_poll_event(ctx, &mut ev) {
            let get = |k: &str| {
                let v = kui_value_get(ev.payload, ks(k));
                let mut out = KuiStr {
                    ptr: std::ptr::null(),
                    len: 0,
                };
                kui_value_as_str(v, &mut out).then(|| kstr(out).into_owned())
            };
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
        assert_eq!(
            unsafe { &*ev.payload }
                .0
                .get("kind")
                .and_then(Value::as_str),
            Some("changed")
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
        assert_eq!(
            payload.0.get("kind").and_then(Value::as_str),
            Some("access")
        );
        assert_eq!(
            payload.0.get("action").and_then(Value::as_str),
            Some("increment")
        );
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
        assert_eq!(
            payload.0.get("kind").and_then(Value::as_str),
            Some("layout")
        );
        assert_eq!(payload.0.get("w").and_then(Value::as_float), Some(300.0));
        assert_eq!(payload.0.get("tag").and_then(Value::as_str), Some("panel"));
        // The same rect as a query (backlog C26 step 2), and nothing for
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

    fn ks(s: &str) -> KuiStr {
        KuiStr {
            ptr: s.as_ptr(),
            len: s.len(),
        }
    }

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

    /// A null context is a no-op, like every other entry point.
    #[test]
    fn a_null_context_is_survivable() {
        kui_env_set_system(std::ptr::null_mut(), 1, 0, 1, ks("en"));
        kui_env_set_audio(std::ptr::null_mut(), 2, 1);
    }
}

#[cfg(test)]
mod parity_headless {
    use super::*;

    fn ks(s: &str) -> KuiStr {
        KuiStr {
            ptr: s.as_ptr(),
            len: s.len(),
        }
    }

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
            },
            KuiMenuItem {
                label: ks("Wrap"),
                role: KUI_MENU_CUSTOM,
                enabled: 1,
                id: std::ptr::null(),
                accel: ks("⌥Z"),
                checked: 1,
            },
            KuiMenuItem {
                label: ks("Gone"),
                role: KUI_MENU_CUSTOM,
                enabled: 0,
                id: std::ptr::null(),
                accel: ks(""),
                checked: 0,
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
        assert_eq!(
            read(1),
            (
                "Wrap".into(),
                "⌥Z".into(),
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
}
