//! Size expressions (backlog F109): what one costs to hand in, by form,
//! and what it costs layout. A binding turns a prop into a `Sizing` once
//! per node per frame, so the conversions are per-node numbers:
//!
//! - `spelled_cold`: parsing the text and finding its entry — the first
//!   frame a spelling is seen (`calc::parse` + `sizing_of`);
//! - `spelled_cached`: the same text on every later frame, found by its
//!   text in the input cache (`schema::sizing_str`);
//! - `data`: the expression as a `Value` tree (`{ clamp = { … } }` from Lua,
//!   a JSON-ish object), walked and matched to its entry (`sizing_value`);
//! - `code`: the Node wire's prefix code, numbers only (`sizing_code`);
//! - `px` and `percent`: what a plain `720` and `"80%"` cost, for scale.
//!
//! And a laid-out frame of 1000 rows sized `Percent` against the same
//! rows sized by a `Calc`, and by a calc clamp, for layout's share.
//!
//! Run: cargo bench -p kui-core --bench sizes

use kui_core::calc;
use kui_core::schema::{max_str, sizing_str};
use kui_core::value::Value;
use kui_core::{Core, NodeSpec, Size, Sizing};

const SPELLED: &str = "clamp(400px, 80%, 1000px)";

fn map(k: &str, v: Value) -> Value {
    Value::Map(vec![(k.to_string(), v)])
}

fn tree() -> Value {
    map(
        "clamp",
        Value::List(vec![
            Value::Int(400),
            map("pct", Value::Int(80)),
            Value::Int(1000),
        ]),
    )
}

const CODE: [f64; 7] = [5.0, 1.0, 400.0, 2.0, 0.8, 1.0, 1000.0];

#[divan::bench]
fn px() -> Sizing {
    calc::sizing_value(divan::black_box(&Value::Int(720))).unwrap()
}

#[divan::bench]
fn percent() -> Sizing {
    sizing_str(divan::black_box("80%")).unwrap()
}

#[divan::bench]
fn spelled_cold() -> Sizing {
    calc::sizing_of(calc::parse(divan::black_box(SPELLED)).unwrap()).unwrap()
}

#[divan::bench]
fn spelled_cached() -> Sizing {
    sizing_str(divan::black_box(SPELLED)).unwrap()
}

#[divan::bench]
fn data(bencher: divan::Bencher) {
    let v = tree();
    bencher.bench_local(|| calc::sizing_value(divan::black_box(&v)).unwrap());
}

#[divan::bench]
fn code() -> Sizing {
    calc::sizing_code(divan::black_box(&CODE)).unwrap()
}

const ROWS: usize = 1000;

fn frame(core: &mut Core, width: Sizing, max: Option<kui_core::Bound>) {
    let mut ui = core.frame(Size::new(1600.0, 1000.0), 1.0);
    ui.with(NodeSpec::column().width(Sizing::GROW), |ui| {
        for _ in 0..ROWS {
            let mut row = NodeSpec::row().width(width).height(Sizing::Fixed(4.0));
            if let Some(m) = max {
                row = row.max_width(m);
            }
            ui.with(row, |_| {});
        }
    });
    ui.finish();
}

#[divan::bench(args = ["percent", "calc", "percent + calc max"])]
fn layout_1000_rows(bencher: divan::Bencher, form: &str) {
    let (width, max) = match form {
        "percent" => (Sizing::Percent(0.8), None),
        "calc" => (sizing_str(SPELLED).unwrap(), None),
        _ => (
            Sizing::Percent(0.8),
            Some(max_str("min(1000px, 90%)").unwrap()),
        ),
    };
    let mut core = Core::new();
    frame(&mut core, width, max);
    bencher.bench_local(|| frame(&mut core, width, max));
}

fn main() {
    divan::main();
}
