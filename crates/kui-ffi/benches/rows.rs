//! What a row costs a C host (backlog F281, F282): a log list of a
//! thousand rows built through the C ABI the way a C host builds it —
//! `kui_open_keyed` with a padded, rounded, hover-tracking box and an
//! `on_click` message from `kui_msg_int`, one `kui_text`, `kui_close` —
//! inside `kui_frame_begin` .. `kui_frame_finish`, steady state over one
//! context. Beside it the same frame through the Rust builder, so the gap
//! is what the boundary costs, and the same rows as components a loaded
//! extension provides, every one replayed.
//!
//! Run: cargo bench -p kui-ffi --bench rows

use std::ffi::c_void;

use kui_core::{Color, Core, Edges, NodeSpec, Size, TextStyle, Value};
use kui_ffi::*;

const ROWS: usize = 1000;

fn ks(s: &str) -> KuiStr {
    KuiStr {
        ptr: s.as_ptr(),
        len: s.len(),
    }
}

/// The former header-only `kui_msg_int`, retained as a same-binary baseline.
fn msg_int_inline(kind: &str, field: &str, n: i64) -> *mut KuiValue {
    let m = kui_value_map();
    kui_value_map_set(m, ks("kind"), kui_value_str(ks(kind)));
    kui_value_map_set(m, ks(field), kui_value_int(n));
    m
}

/// What the rows say and are keyed by, made once: a C host formats them
/// into buffers it keeps, and the formatting is not the library's cost.
struct Log {
    labels: Vec<String>,
    lines: Vec<String>,
}

impl Log {
    fn new() -> Self {
        Self {
            labels: (0..ROWS).map(|i| format!("row{i}")).collect(),
            lines: (0..ROWS)
                .map(|i| format!("{i:5} INFO  request served in {} ms", 3 + i % 40))
                .collect(),
        }
    }
}

fn zspec() -> KuiSpec {
    unsafe { std::mem::zeroed() }
}

fn row_spec(selected: bool) -> KuiSpec {
    let mut s = zspec();
    s.dir = 1;
    s.width = KuiSizing { tag: 1, value: 1.0 };
    s.pad_l = 8.0;
    s.pad_r = 8.0;
    s.pad_t = 2.0;
    s.pad_b = 2.0;
    s.radius = 4.0;
    s.bg = 0x20202aff;
    s.hover_bg = 0x30303cff;
    s.selected = selected as u32;
    s
}

fn text_style() -> KuiTextStyle {
    let mut t: KuiTextStyle = unsafe { std::mem::zeroed() };
    t.size = 13.0;
    t.color = 0xd0d0d0ff;
    t
}

#[derive(Clone, Copy)]
struct Parts {
    boxes: bool,
    texts: bool,
    inline_message: bool,
}

const BOTH: Parts = Parts {
    boxes: true,
    texts: true,
    inline_message: false,
};

const BOTH_INLINE: Parts = Parts {
    inline_message: true,
    ..BOTH
};

fn begin(ctx: *mut KuiCtx) {
    kui_frame_begin(ctx, 1280.0, 720.0, 2.0);
    let mut root = zspec();
    root.width = KuiSizing { tag: 1, value: 1.0 };
    root.height = KuiSizing { tag: 1, value: 1.0 };
    kui_root(ctx, &root);
}

fn build(ctx: *mut KuiCtx, log: &Log, parts: Parts) {
    let style = text_style();
    let spec = row_spec(false);
    let selected = row_spec(true);
    for i in 0..ROWS {
        if parts.boxes {
            let s = if i.is_multiple_of(17) {
                &selected
            } else {
                &spec
            };
            let message = if parts.inline_message {
                msg_int_inline("row", "line", i as i64)
            } else {
                kui_msg_int(c"row".as_ptr(), c"line".as_ptr(), i as i64)
            };
            kui_open_keyed(ctx, ks(&log.labels[i]), s, message);
        }
        if parts.texts {
            kui_text(ctx, ks(&log.lines[i]), &style);
        }
        if parts.boxes {
            kui_close(ctx);
        }
    }
}

fn frame(ctx: *mut KuiCtx, log: &Log, parts: Parts) {
    begin(ctx);
    build(ctx, log, parts);
    kui_frame_finish(ctx);
}

fn quads(ctx: *mut KuiCtx) -> usize {
    let mut d = KuiDrawData::default();
    kui_draw_data(ctx, &mut d);
    d.quad_count
}

fn rows_bench(bencher: divan::Bencher, parts: Parts) {
    let log = Log::new();
    let ctx = kui_ctx_new();
    frame(ctx, &log, parts);
    bencher.bench_local(|| frame(ctx, &log, parts));
    kui_ctx_free(ctx);
}

/// The whole frame: build, layout, shaping (warm), the access tree's
/// digest and the draw data.
#[divan::bench]
fn ffi_rows_1k(bencher: divan::Bencher) {
    rows_bench(bencher, BOTH);
}

/// The same frame with the old message helper, for interleaved comparison.
#[divan::bench]
fn ffi_rows_1k_inline_message(bencher: divan::Bencher) {
    rows_bench(bencher, BOTH_INLINE);
}

#[divan::bench(sample_size = 10000)]
fn message_int_direct(bencher: divan::Bencher) {
    bencher.bench_local(|| {
        kui_value_free(divan::black_box(kui_msg_int(
            c"row".as_ptr(),
            c"line".as_ptr(),
            42,
        )));
    });
}

#[divan::bench(sample_size = 10000)]
fn message_int_inline(bencher: divan::Bencher) {
    bencher.bench_local(|| {
        kui_value_free(divan::black_box(msg_int_inline("row", "line", 42)));
    });
}

/// The boxes alone, no text in them.
#[divan::bench]
fn ffi_rows_1k_boxes_only(bencher: divan::Bencher) {
    rows_bench(
        bencher,
        Parts {
            boxes: true,
            texts: false,
            inline_message: false,
        },
    );
}

/// The texts alone, straight under the root.
#[divan::bench]
fn ffi_rows_1k_texts_only(bencher: divan::Bencher) {
    rows_bench(
        bencher,
        Parts {
            boxes: false,
            texts: true,
            inline_message: false,
        },
    );
}

/// Only the build calls of `ffi_rows_1k`: the frame begins and finishes
/// outside the timing, one frame a sample.
#[divan::bench(sample_size = 1, sample_count = 2000)]
fn ffi_rows_1k_build(bencher: divan::Bencher) {
    build_bench(bencher, BOTH);
}

#[divan::bench(sample_size = 1, sample_count = 2000)]
fn ffi_rows_1k_build_inline_message(bencher: divan::Bencher) {
    build_bench(bencher, BOTH_INLINE);
}

fn build_bench(bencher: divan::Bencher, parts: Parts) {
    let log = Log::new();
    let ctx = kui_ctx_new();
    frame(ctx, &log, parts);
    let mut open = false;
    bencher
        .with_inputs(|| {
            if open {
                kui_frame_finish(ctx);
            }
            begin(ctx);
            open = true;
        })
        .bench_local_values(|()| build(ctx, &log, parts));
    if open {
        kui_frame_finish(ctx);
    }
    kui_ctx_free(ctx);
}

/// Only `kui_frame_finish` of `ffi_rows_1k`: the build outside the timing.
#[divan::bench(sample_size = 1, sample_count = 2000)]
fn ffi_rows_1k_finish(bencher: divan::Bencher) {
    let log = Log::new();
    let ctx = kui_ctx_new();
    frame(ctx, &log, BOTH);
    bencher
        .with_inputs(|| {
            begin(ctx);
            build(ctx, &log, BOTH);
        })
        .bench_local_values(|()| kui_frame_finish(ctx));
    kui_ctx_free(ctx);
}

/// `ffi_rows_1k` through the Rust builder: the same specs, keys, messages
/// and texts, no boundary. A C host's row costs this plus the gap.
#[divan::bench]
fn core_rows_1k(bencher: divan::Bencher) {
    let log = Log::new();
    let mut core = Core::new();
    core.set_diagnostics(false);
    let row = |selected: bool| {
        NodeSpec::row()
            .grow_width()
            .padding(Edges {
                l: 8.0,
                r: 8.0,
                t: 2.0,
                b: 2.0,
            })
            .radius(4.0)
            .bg(Color::hex(0x20202aff))
            .hover_bg(Color::hex(0x30303cff))
            .selected(selected)
    };
    let style = TextStyle::new(13.0).color(Color::hex(0xd0d0d0ff));
    let run = |core: &mut Core| {
        let mut ui = core.frame(Size::new(1280.0, 720.0), 2.0);
        ui.configure_root(NodeSpec::column().grow_width().grow_height());
        for i in 0..ROWS {
            let msg = Value::map([("kind", "row".into()), ("line", (i as i64).into())]);
            ui.open_keyed(&log.labels[i], row(i.is_multiple_of(17)).on_click(msg));
            ui.text(&log.lines[i], style);
            ui.close();
        }
        ui.finish();
    };
    run(&mut core);
    bencher.bench_local(|| run(&mut core));
}

// -- Components (ADR 0060) ----------------------------------------------------
// The same rows as a component a loaded extension provides, placed with
// `kui_component` and props built the way a C host builds them
// (`kui_value_map` and four `kui_value_map_set`). The props never change,
// so after the first frame every one replays: the extension is not asked,
// and what is left is the placement, the props compare and the journal's
// nodes pushed again. Lua provides it because a script loads through the
// public door with no compiler; a replay never runs it either way.

const ROW_LUA: &str = r#"
components = {
  row = function(env, props, inst)
    return row {
      width = "grow", pad = { l = 8, r = 8, t = 2, b = 2 }, radius = 4, bg = 0x20202aff, hover_bg = 0x30303cff,
      selected = props.selected,
      on_click = { kind = "row", line = props.line },
      text(props.text, { size = 13, color = 0xd0d0d0ff }),
    }
  end,
}
"#;

fn row_props(log: &Log, i: usize) -> *mut KuiValue {
    let m = kui_value_map();
    kui_value_map_set(m, ks("key"), kui_value_str(ks(&log.labels[i])));
    kui_value_map_set(m, ks("line"), kui_value_int(i as i64));
    kui_value_map_set(m, ks("text"), kui_value_str(ks(&log.lines[i])));
    kui_value_map_set(m, ks("selected"), kui_value_bool(i.is_multiple_of(17)));
    m
}

fn components_ctx() -> *mut KuiCtx {
    let dir = std::env::temp_dir().join(format!("kui-bench-rows-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("log.lua");
    std::fs::write(&path, ROW_LUA).unwrap();
    let ctx = kui_ctx_new();
    assert!(
        kui_ctx_add_extension(ctx, ks("log"), ks(path.to_str().unwrap())),
        "the row script loads"
    );
    ctx
}

/// What one frame of the plain rows draws, a box's quad and a text's
/// glyphs a row: what a replayed frame of components must draw too.
fn row_quads(log: &Log) -> usize {
    let ctx = kui_ctx_new();
    frame(ctx, log, BOTH);
    let n = quads(ctx);
    kui_ctx_free(ctx);
    n
}

/// The first frame, which runs the script: it must draw without a
/// warning, or the replays after it would be timing a row that is not
/// there.
fn first_frame(ctx: *mut KuiCtx, props: &[*mut KuiValue]) {
    kui_set_diagnostics(ctx, true);
    components_frame(ctx, props);
    let mut out = [unsafe { std::mem::zeroed::<KuiWarning>() }; 1];
    assert_eq!(
        kui_take_warnings(ctx, out.as_mut_ptr(), 1),
        0,
        "the row script draws cleanly"
    );
    kui_set_diagnostics(ctx, false);
}

/// One frame of placed rows; how many replayed.
fn components_frame(ctx: *mut KuiCtx, props: &[*mut KuiValue]) -> usize {
    begin(ctx);
    let mut replayed = 0;
    for p in props {
        let fill = kui_component(ctx, ks("log/row"), *p, None, std::ptr::null_mut::<c_void>());
        replayed += (fill == 0) as usize;
    }
    kui_frame_finish(ctx);
    replayed
}

/// A thousand replayed components, their props built every frame as a C
/// host builds them.
#[divan::bench]
fn ffi_components_1k_replayed(bencher: divan::Bencher) {
    let log = Log::new();
    let ctx = components_ctx();
    let run = || {
        let props: Vec<_> = (0..ROWS).map(|i| row_props(&log, i)).collect();
        let replayed = components_frame(ctx, &props);
        for p in props {
            kui_value_free(p);
        }
        replayed
    };
    let props: Vec<_> = (0..ROWS).map(|i| row_props(&log, i)).collect();
    first_frame(ctx, &props);
    for p in props {
        kui_value_free(p);
    }
    assert_eq!(run(), ROWS, "every row replays");
    assert_eq!(quads(ctx), row_quads(&log), "and draws what the rows draw");
    bencher.bench_local(run);
    kui_ctx_free(ctx);
}

/// The same with the props built once and kept: what the placement and
/// the replay cost apart from building the maps.
#[divan::bench]
fn ffi_components_1k_replayed_props_kept(bencher: divan::Bencher) {
    let log = Log::new();
    let ctx = components_ctx();
    let props: Vec<_> = (0..ROWS).map(|i| row_props(&log, i)).collect();
    first_frame(ctx, &props);
    assert_eq!(components_frame(ctx, &props), ROWS, "every row replays");
    assert_eq!(quads(ctx), row_quads(&log), "and draws what the rows draw");
    bencher.bench_local(|| components_frame(ctx, &props));
    for p in props {
        kui_value_free(p);
    }
    kui_ctx_free(ctx);
}

fn main() {
    divan::main();
}
