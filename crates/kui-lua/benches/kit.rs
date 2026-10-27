//! Kit's budgets (addons/kit): a whole host frame with Kit's components in
//! it — the host's view, the script's functions, the walk from its tables
//! to the tree, layout and emission — against what each is allowed.
//!
//! - `buttons`: stock buttons, Kit buttons in a row, and Kit buttons as the
//!   host's children of a Kit toolbar, at 0, 20 and 200: what one instance
//!   costs is the slope.
//! - `table_*`: 100,000 rows from a channel, still and following a
//!   selection that moves a row a frame (budget 1.5 ms).
//! - `waterfall_*`: 18,000 spans fully zoomed out, at a 2 s window, and
//!   panning across one (budget 1.5 ms).
//! - `handover`: the frame after the rows are published again as a new
//!   revision.
//! - `channel_*` (ADR 0065): the same 100,000 rows as a channel — built
//!   once from Rust vectors, and a frame of a Lua table component that
//!   reads the forty rows it draws by index, asked fresh every frame.
//!
//! Run: cargo bench -p kui-lua --bench kit

#[path = "../../../examples/lua/features/kit/scene.rs"]
#[allow(dead_code)]
mod scene;

use kui_core::{Core, Extensions, Size};
use scene::{Buttons, Scene};

fn frame(core: &mut Core, scene: &mut Scene, exts: &mut Extensions) {
    let mut ui = core.frame_with(Size::new(1280.0, 820.0), 1.0, exts);
    scene.view(&mut ui);
    ui.finish();
}

fn setup(scene: &mut Scene) -> (Core, Extensions) {
    let mut core = Core::new();
    let mut exts = Extensions::new();
    exts.push_as("kit", Box::new(scene::load_kit())).unwrap();
    for _ in 0..3 {
        frame(&mut core, scene, &mut exts);
    }
    let w = core.take_warnings();
    assert!(w.is_empty(), "the scene is clean: {w:?}");
    (core, exts)
}

fn only(table: bool, waterfall: bool) -> Scene {
    let mut s = Scene::new(
        if table { 100_000 } else { 0 },
        if waterfall { 3_000 } else { 0 },
    );
    s.table = table;
    s.waterfall = waterfall;
    s.n_buttons = 5;
    s
}

/// Each placement at 0, 20 and 200 buttons: the frame around them is the
/// 0 row, so what one instance costs is the slope, not the total over 20.
#[divan::bench(consts = [0, 20, 200], args = ["native", "components", "toolbar"])]
fn buttons<const N: usize>(bencher: divan::Bencher, how: &str) {
    let mut s = only(false, false);
    s.n_buttons = N;
    s.buttons = match how {
        "native" => Buttons::Native,
        "components" => Buttons::Components,
        _ => Buttons::Toolbar,
    };
    let (mut core, mut exts) = setup(&mut s);
    bencher.bench_local(|| frame(&mut core, &mut s, &mut exts));
}

/// 200 components, replayed by default, and nothing else Kit fills fresh:
/// the instance path alone, for a profiler (`--bench components_only`).
#[divan::bench]
fn components_only(bencher: divan::Bencher) {
    let mut s = only(false, false);
    s.n_buttons = 200;
    s.buttons = Buttons::Components;
    let (mut core, mut exts) = setup(&mut s);
    bencher.bench_local(|| frame(&mut core, &mut s, &mut exts));
}

#[divan::bench]
fn table_still(bencher: divan::Bencher) {
    let mut s = only(true, false);
    let (mut core, mut exts) = setup(&mut s);
    bencher.bench_local(|| frame(&mut core, &mut s, &mut exts));
}

/// The selection moving a row a frame, the table scrolling to keep it in
/// view once it reaches the edge.
#[divan::bench]
fn table_follow(bencher: divan::Bencher) {
    let mut s = only(true, false);
    let (mut core, mut exts) = setup(&mut s);
    let mut i = 0i64;
    bencher.bench_local(|| {
        i = (i + 1) % 99_000;
        s.selected = Some(i);
        frame(&mut core, &mut s, &mut exts);
    });
}

#[divan::bench]
fn waterfall_full(bencher: divan::Bencher) {
    let mut s = only(false, true);
    let (mut core, mut exts) = setup(&mut s);
    bencher.bench_local(|| frame(&mut core, &mut s, &mut exts));
}

#[divan::bench]
fn waterfall_2s(bencher: divan::Bencher) {
    let mut s = only(false, true);
    s.window = Some((10_000.0, 2_000.0));
    let (mut core, mut exts) = setup(&mut s);
    bencher.bench_local(|| frame(&mut core, &mut s, &mut exts));
}

#[divan::bench]
fn waterfall_pan(bencher: divan::Bencher) {
    let mut s = only(false, true);
    let (mut core, mut exts) = setup(&mut s);
    let mut start = 0.0;
    bencher.bench_local(|| {
        start = (start + 7.0) % 45_000.0;
        s.window = Some((start, 2_000.0));
        frame(&mut core, &mut s, &mut exts);
    });
}

#[divan::bench(sample_count = 10, sample_size = 1)]
fn handover(bencher: divan::Bencher) {
    let mut s = only(true, true);
    let (mut core, mut exts) = setup(&mut s);
    bencher.bench_local(|| {
        s.republish();
        frame(&mut core, &mut s, &mut exts);
    });
}

/// The 100,000 rows the M0 table draws, as a channel's columns.
fn channel_columns() -> Vec<(String, kui_core::channel::Column)> {
    use kui_core::channel::Column;
    let n = 100_000;
    vec![
        (
            "name".into(),
            Column::strs((0..n).map(|i| format!("GET /api/items/{i}"))),
        ),
        (
            "ms".into(),
            Column::F64((0..n).map(|i| (i % 997) as f64 * 0.25).collect()),
        ),
        (
            "status".into(),
            Column::I64(
                (0..n)
                    .map(|i| if i % 13 == 0 { 500 } else { 200 })
                    .collect(),
            ),
        ),
    ]
}

#[divan::bench(sample_count = 20, sample_size = 1)]
fn channel_build_100k(bencher: divan::Bencher) {
    bencher
        .with_inputs(channel_columns)
        .bench_local_values(|cols| kui_core::channel::Channel::table(cols).unwrap());
}

/// The same, the columns built from nothing in the timing: what a host
/// that holds its rows in some other shape pays to hand them over.
#[divan::bench(sample_count = 20, sample_size = 1)]
fn channel_from_scratch_100k(bencher: divan::Bencher) {
    bencher.bench_local(|| kui_core::channel::Channel::table(channel_columns()).unwrap());
}

const CHANNEL_TABLE: &str = r#"
    components = {
      table = function(env, props)
        local rows, first = props.rows, props.first or 0
        local kids = {}
        for i = first, math.min(first + 39, #rows - 1) do
          kids[#kids + 1] = row { height = 20, gap = 8,
            text(rows:get(i, "name"), { size = 12 }),
            text(tostring(rows:get(i, "ms")), { size = 12 }),
            text(tostring(rows:get(i, "status")), { size = 12 }),
          }
        end
        return column(kids)
      end,
    }
"#;

#[divan::bench]
fn channel_table_frame_100k(bencher: divan::Bencher) {
    use kui_core::Value;
    let ch = kui_core::channel::Channel::table(channel_columns()).unwrap();
    let mut exts = Extensions::new();
    exts.push_as(
        "t",
        Box::new(kui_lua::LuaExtension::from_source("t.lua", CHANNEL_TABLE).unwrap()),
    )
    .unwrap();
    let mut core = Core::new();
    let mut first = 0i64;
    bencher.bench_local(|| {
        first = (first + 37) % 99_000;
        let props = Value::map([
            ("rows", ch.handle()),
            ("first", Value::Int(first)),
            ("fresh", Value::Bool(true)),
        ]);
        let mut ui = core.frame_with(Size::new(1280.0, 820.0), 1.0, &mut exts);
        ui.component("t/table", &props);
        ui.finish();
    });
}

/// A line chart's columns: `n` samples of two series against `t`, from
/// sample `from` on.
fn chart_columns(from: usize, n: usize) -> Vec<(String, kui_core::channel::Column)> {
    use kui_core::channel::Column;
    let at = |i: usize| (from + i) as f64;
    vec![
        (
            "t".into(),
            Column::F64((0..n).map(|i| at(i) * 0.1).collect()),
        ),
        (
            "a".into(),
            Column::F64(
                (0..n)
                    .map(|i| 40.0 + 25.0 * (at(i) * 0.09).sin() + (hash(at(i) as u64) % 12) as f64)
                    .collect(),
            ),
        ),
        (
            "b".into(),
            Column::F64(
                (0..n)
                    .map(|i| {
                        55.0 + 10.0 * (at(i) * 0.021).sin() + (hash(at(i) as u64 + 7) % 4) as f64
                    })
                    .collect(),
            ),
        ),
    ]
}

fn hash(i: u64) -> u64 {
    let x = i.wrapping_mul(0x9E37_79B9_7F4A_7C15);
    x ^ (x >> 29)
}

fn chart_props(ch: &kui_core::channel::Channel, window: Option<f64>) -> kui_core::Value {
    use kui_core::Value;
    let series = |f: &'static str| Value::map([("field", Value::str(f)), ("name", Value::str(f))]);
    let mut props = vec![
        ("data", ch.handle()),
        ("x", Value::str("t")),
        ("area", Value::Bool(true)),
        ("height", Value::Float(360.0)),
        ("series", Value::list([series("a"), series("b")])),
    ];
    if let Some(w) = window {
        props.push(("window", Value::Float(w)));
    }
    Value::map(props)
}

fn chart_frame(core: &mut Core, exts: &mut Extensions, props: &kui_core::Value) {
    let mut ui = core.frame_with(Size::new(1400.0, 500.0), 1.0, exts);
    ui.component("kit/line_chart", props);
    ui.finish();
}

/// A live line chart: 300 samples of two series, its area filled, a new
/// revision every frame with the oldest sample gone and a new one on —
/// what the Kit app's Charts page draws on every tick (budget 1 ms).
#[divan::bench]
fn line_chart_live_300(bencher: divan::Bencher) {
    let mut ch = kui_core::channel::Channel::table(chart_columns(0, 300)).unwrap();
    let mut exts = Extensions::new();
    exts.push_as("kit", Box::new(scene::load_kit())).unwrap();
    let mut core = Core::new();
    for _ in 0..3 {
        chart_frame(&mut core, &mut exts, &chart_props(&ch, None));
    }
    let w = core.take_warnings();
    assert!(w.is_empty(), "the chart is clean: {w:?}");
    let mut from = 0;
    bencher.bench_local(|| {
        from += 1;
        ch = ch.publish(chart_columns(from, 300)).unwrap();
        chart_frame(&mut core, &mut exts, &chart_props(&ch, None));
    });
}

/// A live window (the last 30 s of 300 samples, one every 100 ms, on a
/// frame clock): `sample` is the frame a sample lands in — a new revision,
/// the chart asked again and its slide started — and `between` a frame
/// between two, which the slide's keyframe cycle draws with no view run
/// (the fill replayed, the core moving the content); `reframed` the same
/// frame built from the last tree with no view and no fill at all, as a
/// driver whose app says its state held builds it (`Core::reframe`,
/// backlog C138).
#[divan::bench(args = ["sample", "between", "reframed"])]
fn line_chart_live_window(bencher: divan::Bencher, frame: &str) {
    let mut ch = kui_core::channel::Channel::table(chart_columns(0, 300)).unwrap();
    let mut exts = Extensions::new();
    exts.push_as("kit", Box::new(scene::load_kit())).unwrap();
    let mut core = Core::new();
    let mut t = 1.0;
    let mut from = 0;
    for _ in 0..5 {
        from += 1;
        t += 0.1;
        ch = ch.publish(chart_columns(from, 300)).unwrap();
        core.set_time(t);
        chart_frame(&mut core, &mut exts, &chart_props(&ch, Some(30.0)));
    }
    let w = core.take_warnings();
    assert!(w.is_empty(), "the chart is clean: {w:?}");
    let props = chart_props(&ch, Some(30.0));
    if frame == "sample" {
        bencher.bench_local(|| {
            from += 1;
            t += 0.1;
            ch = ch.publish(chart_columns(from, 300)).unwrap();
            core.set_time(t);
            chart_frame(&mut core, &mut exts, &chart_props(&ch, Some(30.0)));
        });
    } else if frame == "between" {
        // A sixtieth of the pace a frame, a fresh slide each hundred
        // milliseconds' worth of frames' clock, never a new sample.
        let mut k = 0u32;
        bencher.bench_local(|| {
            k = (k + 1) % 5;
            core.set_time(t + 0.016 * f64::from(k + 1));
            chart_frame(&mut core, &mut exts, &props);
        });
    } else {
        // The frame after one built with the view keeps what it needs,
        // once a driver has asked.
        core.set_time(t + 0.004);
        let _ = core.reframe(Size::new(1400.0, 500.0), 1.0);
        core.set_time(t + 0.008);
        chart_frame(&mut core, &mut exts, &props);
        let mut k = 0u32;
        bencher.bench_local(|| {
            k = (k + 1) % 5;
            core.set_time(t + 0.016 * f64::from(k + 1));
            assert!(core.reframe(Size::new(1400.0, 500.0), 1.0), "built again");
        });
    }
}

/// 100,000 samples of two series in view, the chart asked again every
/// frame (its window moves by a sample): the cut to two points a pixel
/// is the cost, and Kit's plan gave it 1 ms.
#[divan::bench]
fn line_chart_100k(bencher: divan::Bencher) {
    let ch = kui_core::channel::Channel::table(chart_columns(0, 100_000)).unwrap();
    let mut exts = Extensions::new();
    exts.push_as("kit", Box::new(scene::load_kit())).unwrap();
    let mut core = Core::new();
    for _ in 0..3 {
        chart_frame(&mut core, &mut exts, &chart_props(&ch, Some(10_000.0)));
    }
    let mut odd = false;
    bencher.bench_local(|| {
        odd = !odd;
        let window = if odd { 9_999.9 } else { 10_000.0 };
        chart_frame(&mut core, &mut exts, &chart_props(&ch, Some(window)));
    });
}

fn main() {
    divan::main();
}
