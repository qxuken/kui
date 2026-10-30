//! What a component saves when its view is the expensive part (ADR 0060,
//! backlog F282/F283): a screen drawn by a Lua script - a header and a
//! table of units, each row four cells formatted from the unit's numbers -
//! placed with `kui_component` through the C ABI:
//!
//!   - fresh every frame (`fresh: true`): the script runs every frame;
//!   - replayed: the props never change, so the script runs once;
//!   - built directly through the C ABI, the same nodes, the host
//!     formatting the cells: what a frame costs with no script at all.
//!
//! And the same rows as eight panel components, one panel's data changing
//! every frame: what a screen pays when one part of it moves.
//!
//! Run: cargo bench -p kui-ffi --bench screens

use std::ffi::c_void;

use kui_ffi::*;

const ROWS: usize = 240;
const PANELS: usize = 8;

fn ks(s: &str) -> KuiStr {
    KuiStr {
        ptr: s.as_ptr(),
        len: s.len(),
    }
}

fn zspec() -> KuiSpec {
    unsafe { std::mem::zeroed() }
}

const SCREEN_LUA: &str = r#"
local cell = { size = 13, color = 0xd0d0d0ff }
local head = { size = 20, color = 0xffffffff, bold = true }

-- A unit's numbers, as a game's model would hand them over: derived here
-- so the script does what a real view does - read, compute, format.
local function unit(i, seed)
  local v = (i * 7919 + seed * 104729) % 1000
  return v, v / 7, (v % 3 == 0) and "ready" or (v % 3 == 1 and "busy" or "lost")
end

local function rows_of(t, from, to, seed)
  for i = from, to do
    local v, hp, state = unit(i, seed)
    t[#t + 1] = row {
      width = "grow", gap = 8, pad = { l = 6, r = 6, t = 2, b = 2 }, hover_bg = 0x30303cff,
      on_click = { kind = "pick", row = i },
      text(string.format("%3d", i), cell),
      text(string.format("unit-%04d", v), cell),
      text(string.format("%6.1f hp", hp), cell),
      text(state, cell),
    }
  end
  return t
end

components = {
  screen = function(env, props, inst)
    local t = { width = "grow", gap = 2 }
    t[1] = row { gap = 12, text("Fleet", head), text(string.format("%d units", props.rows), cell) }
    return column(rows_of(t, 1, props.rows, props.seed))
  end,
  panel = function(env, props, inst)
    local t = { width = "grow", gap = 2 }
    t[1] = text(string.format("Wing %d", props.panel), head)
    return column(rows_of(t, props.from, props.to, props.seed))
  end,
}
"#;

fn screens_ctx() -> *mut KuiCtx {
    let dir = std::env::temp_dir().join(format!("kui-bench-screens-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("fleet.lua");
    std::fs::write(&path, SCREEN_LUA).unwrap();
    let ctx = kui_ctx_new();
    assert!(
        kui_ctx_add_extension(ctx, ks("fleet"), ks(path.to_str().unwrap())),
        "the screen script loads"
    );
    ctx
}

fn begin(ctx: *mut KuiCtx) {
    kui_frame_begin(ctx, 1280.0, 2400.0, 2.0);
    let mut root = zspec();
    root.width = KuiSizing { tag: 1, value: 1.0 };
    root.height = KuiSizing { tag: 1, value: 1.0 };
    kui_root(ctx, &root);
}

fn quads(ctx: *mut KuiCtx) -> usize {
    let mut d = KuiDrawData::default();
    kui_draw_data(ctx, &mut d);
    d.quad_count
}

fn nodes(ctx: *mut KuiCtx) -> usize {
    kui_access_tree(ctx, std::ptr::null_mut(), 0)
}

fn screen_props(seed: i64, fresh: bool) -> *mut KuiValue {
    let m = kui_value_map();
    kui_value_map_set(m, ks("key"), kui_value_str(ks("screen")));
    kui_value_map_set(m, ks("rows"), kui_value_int(ROWS as i64));
    kui_value_map_set(m, ks("seed"), kui_value_int(seed));
    if fresh {
        kui_value_map_set(m, ks("fresh"), kui_value_bool(true));
    }
    m
}

/// One frame of the screen component; its KUI_FILL_* answer.
fn screen_frame(ctx: *mut KuiCtx, props: *mut KuiValue) -> i32 {
    begin(ctx);
    let fill = kui_component(
        ctx,
        ks("fleet/screen"),
        props,
        None,
        std::ptr::null_mut::<c_void>(),
    );
    kui_frame_finish(ctx);
    fill
}

/// The first frame, which runs the script: it must draw cleanly.
fn checked_first(ctx: *mut KuiCtx, frame: impl FnOnce()) {
    kui_set_diagnostics(ctx, true);
    frame();
    let mut out = [unsafe { std::mem::zeroed::<KuiWarning>() }; 1];
    assert_eq!(
        kui_take_warnings(ctx, out.as_mut_ptr(), 1),
        0,
        "the script draws cleanly"
    );
    kui_set_diagnostics(ctx, false);
}

/// The screen with the script asked every frame.
#[divan::bench]
fn lua_screen_fresh(bencher: divan::Bencher) {
    let ctx = screens_ctx();
    let props = screen_props(0, true);
    checked_first(ctx, || {
        screen_frame(ctx, props);
    });
    assert_eq!(
        screen_frame(ctx, props),
        1,
        "fresh: filled again (KUI_FILL_NOT_KEPT)"
    );
    bencher.bench_local(|| screen_frame(ctx, props));
    kui_value_free(props);
    kui_ctx_free(ctx);
}

/// The screen replayed: the script ran on the first frame only.
#[divan::bench]
fn lua_screen_replayed(bencher: divan::Bencher) {
    let ctx = screens_ctx();
    let props = screen_props(0, false);
    checked_first(ctx, || {
        screen_frame(ctx, props);
    });
    assert_eq!(screen_frame(ctx, props), 0, "replayed (KUI_FILL_REPLAYED)");
    bencher.bench_local(|| screen_frame(ctx, props));
    kui_value_free(props);
    kui_ctx_free(ctx);
}

/// The same nodes built through the C ABI, the host formatting the cells
/// every frame as the script does: the screen with no script and no replay.
#[divan::bench]
fn ffi_screen_direct(bencher: divan::Bencher) {
    let ctx = kui_ctx_new();
    let mut cell: KuiTextStyle = unsafe { std::mem::zeroed() };
    cell.size = 13.0;
    cell.color = 0xd0d0d0ff;
    let mut head = cell;
    head.size = 20.0;
    head.color = 0xffffffff;
    head.bold = 1;
    let mut col = zspec();
    col.width = KuiSizing { tag: 1, value: 1.0 };
    col.gap = 2.0;
    let mut header = zspec();
    header.dir = 1;
    header.gap = 12.0;
    let mut row = zspec();
    row.dir = 1;
    row.width = KuiSizing { tag: 1, value: 1.0 };
    row.gap = 8.0;
    row.pad_l = 6.0;
    row.pad_r = 6.0;
    row.pad_t = 2.0;
    row.pad_b = 2.0;
    row.hover_bg = 0x30303cff;
    let run = || {
        begin(ctx);
        kui_open(ctx, &col, std::ptr::null_mut());
        kui_open(ctx, &header, std::ptr::null_mut());
        kui_text(ctx, ks("Fleet"), &head);
        kui_text(ctx, ks(&format!("{ROWS} units")), &cell);
        kui_close(ctx);
        for i in 1..=ROWS as i64 {
            let v = (i * 7919) % 1000;
            let state = match v % 3 {
                0 => "ready",
                1 => "busy",
                _ => "lost",
            };
            let m = kui_value_map();
            kui_value_map_set(m, ks("kind"), kui_value_str(ks("pick")));
            kui_value_map_set(m, ks("row"), kui_value_int(i));
            kui_open(ctx, &row, m);
            kui_text(ctx, ks(&format!("{i:3}")), &cell);
            kui_text(ctx, ks(&format!("unit-{v:04}")), &cell);
            kui_text(ctx, ks(&format!("{:6.1} hp", v as f64 / 7.0)), &cell);
            kui_text(ctx, ks(state), &cell);
            kui_close(ctx);
        }
        kui_close(ctx);
        kui_frame_finish(ctx);
    };
    run();
    bencher.bench_local(run);
    kui_ctx_free(ctx);
}

fn panel_props(p: usize, seed: i64) -> *mut KuiValue {
    let per = ROWS / PANELS;
    let m = kui_value_map();
    kui_value_map_set(m, ks("key"), kui_value_str(ks(&format!("wing{p}"))));
    kui_value_map_set(m, ks("panel"), kui_value_int(p as i64));
    kui_value_map_set(m, ks("from"), kui_value_int((p * per + 1) as i64));
    kui_value_map_set(m, ks("to"), kui_value_int(((p + 1) * per) as i64));
    kui_value_map_set(m, ks("seed"), kui_value_int(seed));
    m
}

/// One frame of the eight panels, each with its seed; how many replayed.
fn panels_frame(ctx: *mut KuiCtx, seeds: &[i64; PANELS]) -> usize {
    begin(ctx);
    let mut replayed = 0;
    for (p, seed) in seeds.iter().enumerate() {
        let props = panel_props(p, *seed);
        let fill = kui_component(
            ctx,
            ks("fleet/panel"),
            props,
            None,
            std::ptr::null_mut::<c_void>(),
        );
        replayed += usize::from(fill == 0);
        kui_value_free(props);
    }
    kui_frame_finish(ctx);
    replayed
}

/// Eight panels, nothing changing: all replayed.
#[divan::bench]
fn lua_panels_all_replayed(bencher: divan::Bencher) {
    let ctx = screens_ctx();
    let seeds = [0; PANELS];
    checked_first(ctx, || {
        panels_frame(ctx, &seeds);
    });
    assert_eq!(panels_frame(ctx, &seeds), PANELS);
    bencher.bench_local(|| panels_frame(ctx, &seeds));
    kui_ctx_free(ctx);
}

/// Eight panels, one panel's data changing every frame: one fill, seven
/// replays.
#[divan::bench]
fn lua_panels_one_changes(bencher: divan::Bencher) {
    let ctx = screens_ctx();
    let mut seeds = [0; PANELS];
    checked_first(ctx, || {
        panels_frame(ctx, &seeds);
    });
    let mut frame = 0i64;
    let mut run = || {
        frame += 1;
        seeds[(frame as usize) % PANELS] = frame;
        panels_frame(ctx, &seeds)
    };
    assert_eq!(run(), PANELS - 1, "one panel fills, the rest replay");
    bencher.bench_local(run);
    kui_ctx_free(ctx);
}

/// Eight panels, every one's data changing every frame: all filled.
#[divan::bench]
fn lua_panels_all_change(bencher: divan::Bencher) {
    let ctx = screens_ctx();
    let mut seeds = [0; PANELS];
    checked_first(ctx, || {
        panels_frame(ctx, &seeds);
    });
    let mut frame = 0i64;
    let mut run = || {
        frame += 1;
        seeds = [frame; PANELS];
        panels_frame(ctx, &seeds)
    };
    assert_eq!(run(), 0, "every panel fills");
    bencher.bench_local(run);
    kui_ctx_free(ctx);
}

/// How many nodes the screen is, printed once: what a replay pushes.
#[divan::bench(sample_count = 1, sample_size = 1)]
fn screen_node_count(bencher: divan::Bencher) {
    let ctx = screens_ctx();
    let props = screen_props(0, false);
    screen_frame(ctx, props);
    screen_frame(ctx, props);
    eprintln!("screen: {} access nodes, {} quads", nodes(ctx), quads(ctx));
    bencher.bench_local(|| ());
    kui_value_free(props);
    kui_ctx_free(ctx);
}

/// Where a frame's time goes, printed once: the frame's begin (last
/// frame's tree dropped), the placement (the script, or the replay), and
/// the finish (layout, text, emit), medians over 400 frames, for the
/// screen fresh and replayed and for the direct build's finish.
#[divan::bench(sample_count = 1, sample_size = 1)]
fn screen_phases(bencher: divan::Bencher) {
    use std::time::Instant;
    for fresh in [true, false] {
        let ctx = screens_ctx();
        let props = screen_props(0, fresh);
        screen_frame(ctx, props);
        screen_frame(ctx, props);
        let mut t = [Vec::new(), Vec::new(), Vec::new()];
        for _ in 0..400 {
            let a = Instant::now();
            begin(ctx);
            let b = Instant::now();
            kui_component(
                ctx,
                ks("fleet/screen"),
                props,
                None,
                std::ptr::null_mut::<c_void>(),
            );
            let c = Instant::now();
            kui_frame_finish(ctx);
            let d = Instant::now();
            t[0].push((b - a).as_secs_f64() * 1e6);
            t[1].push((c - b).as_secs_f64() * 1e6);
            t[2].push((d - c).as_secs_f64() * 1e6);
        }
        for v in &mut t {
            v.sort_by(f64::total_cmp);
        }
        eprintln!(
            "screen {}: begin {:.0} us, placement {:.0} us, finish {:.0} us",
            if fresh { "fresh" } else { "replayed" },
            t[0][200],
            t[1][200],
            t[2][200]
        );
        kui_value_free(props);
        kui_ctx_free(ctx);
    }
    bencher.bench_local(|| ());
}

fn main() {
    divan::main();
}
