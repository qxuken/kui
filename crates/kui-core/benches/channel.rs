//! What a channel's reduction costs (backlog C95): a hundred thousand rows
//! of a series bucketed for a chart's pixel columns, the first, least,
//! greatest and last of each — what Kit's line chart did in Lua at about
//! 1.5 ms a series, copying the column first.
//!
//! Run: cargo bench -p kui-core --bench channel

use kui_core::channel::{Bucket, Channel, Column};

fn main() {
    divan::main();
}

/// A noisy wave of `n` rows, and every 97th row NaN when `gaps` — a
/// sensor that drops readings.
fn series(n: usize, gaps: bool) -> Channel {
    let mut seed = 0x9e37_79b9u32;
    let ys = (0..n)
        .map(|i| {
            seed ^= seed << 13;
            seed ^= seed >> 17;
            seed ^= seed << 5;
            if gaps && i % 97 == 0 {
                f64::NAN
            } else {
                (i as f64 * 0.001).sin() * 50.0 + (seed % 1000) as f64 / 100.0
            }
        })
        .collect();
    let ints = (0..n as i64).map(|i| (i * 7919) % 1000).collect();
    Channel::table(vec![
        ("y".into(), Column::F64(ys)),
        ("n".into(), Column::I64(ints)),
    ])
    .unwrap()
}

fn reduce(b: divan::Bencher, ch: &Channel, col: usize, buckets: usize) {
    let mut out: Vec<Bucket> = Vec::new();
    b.bench_local(|| {
        ch.reduce_into(col, 0, ch.len(), divan::black_box(buckets), &mut out)
            .unwrap();
        out.len()
    });
}

/// A 720-px plot at scale 2 of 100k rows: 139 rows a bucket.
#[divan::bench]
fn reduce_100k_into_720(b: divan::Bencher) {
    reduce(b, &series(100_000, false), 0, 720);
}

#[divan::bench]
fn reduce_100k_with_nan_into_720(b: divan::Bencher) {
    reduce(b, &series(100_000, true), 0, 720);
}

#[divan::bench]
fn reduce_100k_integers_into_720(b: divan::Bencher) {
    reduce(b, &series(100_000, false), 1, 720);
}

/// A million rows into the same plot.
#[divan::bench]
fn reduce_1m_into_720(b: divan::Bencher) {
    reduce(b, &series(1_000_000, false), 0, 720);
}

/// Zoomed in: 300 of the 100k rows, a bucket a row.
#[divan::bench]
fn reduce_300_rows_of_100k_into_720(b: divan::Bencher) {
    let ch = series(100_000, false);
    let mut out: Vec<Bucket> = Vec::new();
    b.bench_local(|| {
        ch.reduce_into(0, 50_000, 50_300, 720, &mut out).unwrap();
        out.len()
    });
}
