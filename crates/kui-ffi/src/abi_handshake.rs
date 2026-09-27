use super::*;

fn ks(s: &str) -> KuiStr {
    KuiStr {
        ptr: s.as_ptr(),
        len: s.len(),
    }
}

/// A finished frame holding one clickable scroll container, and the
/// click: enough for every out-param below to have something real to
/// refuse to write.
fn ctx_with_a_scroller_and_a_pending_event() -> *mut KuiCtx {
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
    spec.overflow = 1 | 4; // KUI_CLIP | KUI_SCROLL_Y
    kui_frame_begin(ctx, 200.0, 100.0, 1.0);
    kui_open_keyed(ctx, ks("scroller"), &spec, kui_value_str(ks("hit")));
    kui_close(ctx);
    kui_frame_finish(ctx);
    kui_input_cursor(ctx, 10.0, 10.0);
    kui_input_mouse(ctx, true, 1);
    kui_input_mouse(ctx, false, 1);
    ctx
}

fn window_cmds(ctx: *mut KuiCtx) -> Vec<KuiWindowCommand> {
    let mut out = Vec::new();
    let mut cmd = KuiWindowCommand::default();
    while kui_take_window_command(ctx, &raw mut cmd) {
        out.push(KuiWindowCommand {
            size: cmd.size,
            kind: cmd.kind,
            window: cmd.window,
            origin: cmd.origin,
            config: cmd.config,
            width: cmd.width,
            height: cmd.height,
            owner: cmd.owner,
        });
    }
    out
}

/// `(kind, phase)` of every `window` event queued.
fn window_events(ctx: *mut KuiCtx) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let mut ev = KuiEvent::default();
    while kui_poll_event(ctx, &raw mut ev) {
        let payload = unsafe { &(*ev.payload).0 };
        let kind = payload.get_str("kind").unwrap_or("-");
        if kind != "window" {
            continue;
        }
        let phase = payload.get_str("phase").unwrap_or("-");
        let name = payload.get_str("name").unwrap_or("-");
        out.push((phase.to_string(), name.to_string()));
    }
    out
}

fn frame_declaring(ctx: *mut KuiCtx, declare: &[(f32, f32)]) {
    kui_frame_begin(ctx, 320.0, 240.0, 1.0);
    for (w, h) in declare {
        let cfg = KuiWindowConfig {
            kind: KUI_WINDOW_KIND_NORMAL,
            width: *w,
            height: *h,
            activates: 1,
            ..Default::default()
        };
        kui_window_declare(ctx, ks("palette"), &cfg);
    }
    kui_frame_finish(ctx);
}

/// ADR 0004's step 3 through the C surface: a declaration opens a
/// window (once, with the first config, warning about the second),
/// an OS close reported back keeps it closed while declared, and the
/// declaration lapsing and starting again opens it anew.
#[test]
fn a_declared_window_opens_closes_and_warns_through_the_c_api() {
    let ctx = kui_ctx_new();
    kui_set_diagnostics(ctx, true);
    assert_eq!(kui_ctx_window(ctx), 0);
    let mut name = KuiStr {
        ptr: std::ptr::null(),
        len: 0,
    };
    assert!(kui_ctx_window_name(ctx, &raw mut name));
    assert_eq!(&*kstr(name), "main");

    frame_declaring(ctx, &[(400.0, 300.0), (500.0, 500.0)]);
    let cmds = window_cmds(ctx);
    assert_eq!(cmds.len(), 1);
    assert_eq!(
        (cmds[0].kind, cmds[0].window, cmds[0].origin),
        (KUI_CMD_OPEN, 1, 0)
    );
    assert_eq!(
        (
            cmds[0].config.width,
            cmds[0].config.height,
            cmds[0].config.activates
        ),
        (400.0, 300.0, 1),
        "the first declaration's config, not the second's"
    );
    assert_eq!(cmds[0].size, std::mem::size_of::<KuiWindowCommand>() as u32);
    assert_eq!(
        window_events(ctx),
        vec![("opened".into(), "palette".into())]
    );

    // The user closes it.
    kui_window_closed(ctx, 1);
    assert_eq!(
        window_events(ctx),
        vec![("closed".into(), "palette".into())]
    );
    // Still declared: nothing reopens, and the diagnostics say why.
    frame_declaring(ctx, &[(400.0, 300.0)]);
    assert!(window_cmds(ctx).is_empty());
    let mut warnings = [KuiWarning {
        code: ks(""),
        key: 0,
        message: ks(""),
    }; 8];
    let n = kui_take_warnings(ctx, warnings.as_mut_ptr(), 8);
    let codes: Vec<String> = warnings[..n]
        .iter()
        .map(|w| kstr(w.code).into_owned())
        .collect();
    assert_eq!(
        codes,
        vec![
            "duplicate-window-config".to_string(),
            "window-declared-while-closed".to_string()
        ]
    );

    // The declaration lapses, then starts again: a new window, new id.
    frame_declaring(ctx, &[]);
    assert!(window_cmds(ctx).is_empty());
    frame_declaring(ctx, &[(400.0, 300.0)]);
    let cmds = window_cmds(ctx);
    assert_eq!(cmds.len(), 1);
    assert_eq!((cmds[0].kind, cmds[0].window), (KUI_CMD_OPEN, 2));
    // And stops: the diff closes it.
    frame_declaring(ctx, &[]);
    let cmds = window_cmds(ctx);
    assert_eq!(cmds.len(), 1);
    assert_eq!((cmds[0].kind, cmds[0].window), (KUI_CMD_CLOSE, 2));
    assert_eq!(
        window_events(ctx),
        vec![
            ("opened".into(), "palette".into()),
            ("closed".into(), "palette".into())
        ]
    );
    kui_ctx_free(ctx);
}

/// The size handshake on the new [out] struct, the way `kui_poll_event`
/// has it: a reservation below the layout is refused before anything
/// is popped, so the command is still there for a proper call.
#[test]
fn a_short_window_command_reservation_is_refused_and_keeps_the_command() {
    let ctx = kui_ctx_new();
    frame_declaring(ctx, &[(400.0, 300.0)]);
    let mut short = KuiWindowCommand {
        size: 4,
        kind: 0xdead,
        ..Default::default()
    };
    assert!(!kui_take_window_command(ctx, &raw mut short));
    assert_eq!(short.kind, 0xdead, "nothing was written");
    let cmds = window_cmds(ctx);
    assert_eq!(cmds.len(), 1, "the refused command is still queued");
    assert_eq!(cmds[0].kind, KUI_CMD_OPEN);
    kui_ctx_free(ctx);
}

/// A context standing in for a declared window: the id
/// `kui_env_set_window` gives it is what its events carry and what
/// its name resolves through.
#[test]
fn env_set_window_names_the_context_and_stamps_its_events() {
    let ctx = kui_ctx_new();
    frame_declaring(ctx, &[(400.0, 300.0)]);
    window_cmds(ctx);
    window_events(ctx);
    kui_env_set_window(ctx, 1, false, false, false, 0.0, 0.0);
    assert_eq!(kui_ctx_window(ctx), 1);
    let mut name = KuiStr {
        ptr: std::ptr::null(),
        len: 0,
    };
    assert!(kui_ctx_window_name(ctx, &raw mut name));
    assert_eq!(&*kstr(name), "palette");
    // A frame at another viewport raises a resize, stamped with the id.
    kui_frame_begin(ctx, 200.0, 100.0, 1.0);
    kui_window_declare(ctx, ks("palette"), std::ptr::null());
    kui_frame_finish(ctx);
    let mut ev = KuiEvent::default();
    assert!(kui_poll_event(ctx, &raw mut ev));
    assert_eq!(ev.window, 1);
    kui_ctx_free(ctx);
}

#[test]
fn the_exported_version_is_the_one_the_header_states() {
    // The C side of this is a _Static_assert in mod abi_parity; this is
    // the half that survives the header being absent.
    assert_eq!(kui_abi_version(), KUI_ABI_VERSION);
}

/// The whole point of the size field, on a struct that has already
/// grown: a caller that reserved only the first two fields gets them,
/// and the third — the appended one — is left exactly as it was.
///
/// This was written before any real [out] struct had grown, so that
/// the truncating path would not be first exercised by the change that
/// depends on it. `KuiEvent` has since grown `window`, and
/// [`an_abi_3_host_polls_events_without_seeing_the_appended_window`]
/// runs the same path through the public API — this one stays as the
/// unit-level statement of the rule, on a struct with nothing else
/// going on.
#[test]
fn a_short_reservation_is_filled_only_as_far_as_it_goes() {
    #[repr(C)]
    #[derive(Clone, Copy)]
    struct Grown {
        size: u32,
        was_always_here: u32,
        appended_later: u32,
    }
    // SAFETY: repr(C) with `size: u32` first.
    unsafe impl OutParam for Grown {
        const ABI_V1_SIZE: u32 = abi_through!(Grown, was_always_here, u32);
        fn size_mut(&mut self) -> &mut u32 {
            &mut self.size
        }
    }

    // A host built before `appended_later` existed: it reserved the
    // whole struct it knew, which is the ABI-1 layout.
    let mut old = Grown {
        size: Grown::ABI_V1_SIZE,
        was_always_here: 0,
        appended_later: 0xdeadbeef,
    };
    assert!(write_out(
        &raw mut old,
        Grown {
            size: 0,
            was_always_here: 7,
            appended_later: 9,
        },
    ));
    assert_eq!(old.was_always_here, 7);
    assert_eq!(
        old.appended_later, 0xdeadbeef,
        "the library wrote past what the caller reserved"
    );
    assert_eq!(old.size, Grown::ABI_V1_SIZE, "size reports what was filled");

    // A host built after: it gets everything, and `size` says so.
    let mut new = Grown {
        size: std::mem::size_of::<Grown>() as u32,
        was_always_here: 0,
        appended_later: 0,
    };
    assert!(write_out(
        &raw mut new,
        Grown {
            size: 0,
            was_always_here: 7,
            appended_later: 9,
        },
    ));
    assert_eq!((new.was_always_here, new.appended_later), (7, 9));
    assert_eq!(new.size, std::mem::size_of::<Grown>() as u32);

    // And `size` coming back as the filled count is idempotent, so a
    // loop reusing one struct clamps to the same prefix every time.
    assert!(write_out(
        &raw mut old,
        Grown {
            size: 0,
            was_always_here: 11,
            appended_later: 0,
        },
    ));
    assert_eq!((old.size, old.was_always_here), (Grown::ABI_V1_SIZE, 11));
    assert_eq!(old.appended_later, 0xdeadbeef);
}

/// The same rule on the real struct, through the real entry point:
/// `KuiEvent.window` is ABI 4's append, and a host that predates it —
/// one whose `KuiEvent` ends after `payload`, which is the ABI-1 floor
/// `out_accepts` measures against — keeps polling events and simply
/// never sees the new field.
///
/// The old host is spelled as a reservation rather than as a second
/// struct because that is all the library ever sees of it: four bytes
/// of `size`, and a promise about what lies behind them.
#[test]
fn an_abi_3_host_polls_events_without_seeing_the_appended_window() {
    let ctx = ctx_with_a_scroller_and_a_pending_event();
    let abi3 = KuiEvent::ABI_V1_SIZE;
    assert!(
        (abi3 as usize) < std::mem::size_of::<KuiEvent>(),
        "`window` must sit past the ABI-1 layout, or this proves nothing"
    );

    let mut ev = KuiEvent {
        size: abi3,
        window: 0xdead,
        ..Default::default()
    };
    assert!(kui_poll_event(ctx, &raw mut ev), "the event still arrives");
    assert_ne!(ev.key, 0, "and the fields it knows are filled");
    assert!(!ev.payload.is_null());
    assert_eq!(ev.size, abi3, "`size` reports the prefix that was filled");
    assert_eq!(
        ev.window, 0xdead,
        "the library wrote past what an ABI 3 host reserved"
    );

    kui_ctx_free(ctx);
}

/// And a host built against this ABI gets the field, which is 0 until
/// ADR 0004's step 3 opens a second window.
#[test]
fn a_current_host_sees_window_and_it_is_the_main_one() {
    let ctx = ctx_with_a_scroller_and_a_pending_event();
    let mut ev = KuiEvent::default();
    assert!(kui_poll_event(ctx, &raw mut ev));
    assert_eq!(ev.window, 0);
    assert_eq!(ev.size, std::mem::size_of::<KuiEvent>() as u32);
    kui_ctx_free(ctx);
}

/// A reservation smaller than ABI 1 is refused rather than guessed at —
/// and refused *before* the queue moves, so the event is still there
/// for a caller that asks properly.
#[test]
fn an_unreadable_reservation_refuses_without_dropping_the_event() {
    let ctx = ctx_with_a_scroller_and_a_pending_event();
    // 4 bytes: what an un-set `size`, or one from a host predating the
    // field, looks like to this library.
    let mut stale = KuiEvent {
        size: 4,
        ..Default::default()
    };
    assert!(!kui_poll_event(ctx, &raw mut stale));
    assert_eq!(stale.key, 0, "nothing was written");

    let mut ev = KuiEvent::default();
    assert!(
        kui_poll_event(ctx, &raw mut ev),
        "the event was not dropped"
    );
    assert!(!ev.payload.is_null());
    kui_ctx_free(ctx);
}

/// The same refusal on the other three, so the rule is the family's and
/// not `kui_poll_event`'s.
#[test]
fn every_out_param_refuses_a_reservation_it_cannot_honour() {
    let ctx = ctx_with_a_scroller_and_a_pending_event();

    // A key with real geometry behind it, so the refusal below is the
    // reservation being rejected and not the lookup missing.
    let scroller = kui_child_key(ctx, ks("scroller"));
    let mut geom = KuiScrollGeometry::default();
    assert!(kui_scroll_geometry(ctx, scroller, &raw mut geom));
    assert!(geom.h > 0.0);
    let mut short = KuiScrollGeometry {
        size: 2,
        ..Default::default()
    };
    assert!(!kui_scroll_geometry(ctx, scroller, &raw mut short));
    assert_eq!(short.h, 0.0, "nothing was written");

    let mut m = KuiTextMetrics {
        size: 0,
        ..Default::default()
    };
    let style: KuiTextStyle = unsafe { std::mem::zeroed() };
    assert!(!kui_measure_text(ctx, ks("hello"), &style, 0.0, &raw mut m));
    assert_eq!(m.width, 0.0);

    let mut draw = KuiDrawData {
        size: 1,
        ..Default::default()
    };
    assert!(!kui_draw_data(ctx, &raw mut draw));
    assert!(draw.quads.is_null());
    // Refused, so the atlas is still owed to whoever asks next.
    let mut good = KuiDrawData::default();
    assert!(kui_draw_data(ctx, &raw mut good));
    assert_eq!(good.size, std::mem::size_of::<KuiDrawData>() as u32);

    // NULL is the older half of the same rule and still holds.
    assert!(!kui_poll_event(ctx, std::ptr::null_mut()));
    assert!(!kui_draw_data(ctx, std::ptr::null_mut()));
    kui_ctx_free(ctx);
}
