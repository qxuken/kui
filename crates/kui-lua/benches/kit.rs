//! Kit's M0 budgets (addons/kit, the M0 brief): a whole host frame with
//! Kit's fills in it — the host's view, the script's views, the walk from
//! its tables to the tree, layout and emission — against what each
//! component is allowed.
//!
//! - `buttons`: twenty buttons in one toolbar fill, as twenty slots of one
//!   each, and as those slots replayed (ADR 0045): what one instance costs.
//! - `table_*`: 100,000 rows, still and scrolling 37 px a frame
//!   (budget 1.5 ms).
//! - `waterfall_*`: 18,000 bars fully zoomed out, at a 2 s window, and
//!   panning across one (budget 1.5 ms).
//! - `handover`: the frame on which the rows and bars cross to the script
//!   again, by a new revision.
//! - `channel_*` (ADR 0065): the same 100,000 rows as a channel — built
//!   once from Rust vectors, and a frame of a Lua table component that
//!   reads the forty rows it draws by index, asked fresh every frame —
//!   against `handover`'s conversion.
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
#[divan::bench(
    consts = [0, 20, 200],
    args = ["native", "one_fill", "slots", "slots_replayed", "slots_replayed_kept", "components"]
)]
fn buttons<const N: usize>(bencher: divan::Bencher, how: &str) {
    let mut s = only(false, false);
    s.n_buttons = N;
    s.buttons = match how {
        "native" => Buttons::Native,
        "one_fill" => Buttons::OneFill,
        "slots" => Buttons::Slots,
        "slots_replayed" => Buttons::SlotsReplayed,
        "components" => Buttons::Components,
        _ => Buttons::SlotsReplayedKept,
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
    s.settings_replayed = true;
    let (mut core, mut exts) = setup(&mut s);
    bencher.bench_local(|| frame(&mut core, &mut s, &mut exts));
}

/// 200 replayed slots with kept params and nothing else Kit fills fresh:
/// the replay path alone, for a profiler (`--bench replay_only`).
#[divan::bench]
fn replay_only(bencher: divan::Bencher) {
    let mut s = only(false, false);
    s.n_buttons = 200;
    s.buttons = Buttons::SlotsReplayedKept;
    s.settings_replayed = true;
    let (mut core, mut exts) = setup(&mut s);
    bencher.bench_local(|| frame(&mut core, &mut s, &mut exts));
}

#[divan::bench]
fn table_still(bencher: divan::Bencher) {
    let mut s = only(true, false);
    let (mut core, mut exts) = setup(&mut s);
    bencher.bench_local(|| frame(&mut core, &mut s, &mut exts));
}

#[divan::bench]
fn table_scroll(bencher: divan::Bencher) {
    let mut s = only(true, false);
    let (mut core, mut exts) = setup(&mut s);
    let mut y = 0.0;
    bencher.bench_local(|| {
        y = (y + 37.0) % 2_000_000.0;
        s.scroll_top = Some(y);
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
        s.rev += 1;
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

fn main() {
    divan::main();
}
