//! Channels (ADR 0065): bytes read as column specs — a kind byte, a name
//! length and name, a row count and rows — built into a channel, then read
//! back by every row and column, resolved from its handle, published again
//! and dropped. The properties: nothing panics, a channel built is the
//! length its columns are, every cell reads as the kind its column is, and
//! a handle nobody holds resolves to nothing. Then what reads it in Rust
//! (C147, C148, ADR 0075), driven by the same bytes from the back: an
//! append under a cap, a sort, a range and a string filter, a take, and
//! lanes read a window at a time — a sort a permutation, a filter's rows
//! in its range, a take as long as its rows, an appended revision read
//! whole as it reads row by row, and a run's row inside the channel.

use kui_core::channel::{Cell, Channel, Column, Match, Strs};

fn take<'a>(data: &mut &'a [u8], n: usize) -> Option<&'a [u8]> {
    if data.len() < n {
        return None;
    }
    let (head, rest) = data.split_at(n);
    *data = rest;
    Some(head)
}

fn columns(mut data: &[u8]) -> Vec<(String, Column)> {
    let mut out = Vec::new();
    while let Some(&[kind, name_len, rows]) = take(&mut data, 3) {
        let Some(name) = take(&mut data, usize::from(name_len % 8)) else {
            break;
        };
        let name = String::from_utf8_lossy(name).into_owned();
        let rows = usize::from(rows % 32);
        let column = match kind % 3 {
            0 => Column::F64(
                (0..rows)
                    .map(|_| take(&mut data, 1).map_or(0.0, |b| f64::from(b[0]) - 128.0))
                    .collect(),
            ),
            1 => Column::I64(
                (0..rows)
                    .map(|_| take(&mut data, 1).map_or(0, |b| i64::from(b[0])))
                    .collect(),
            ),
            _ => {
                let mut s = Strs::default();
                for _ in 0..rows {
                    let n = take(&mut data, 1).map_or(0, |b| usize::from(b[0] % 6));
                    let bytes = take(&mut data, n).unwrap_or_default();
                    s.push(&String::from_utf8_lossy(bytes));
                }
                Column::Str(s)
            }
        };
        out.push((name, column));
    }
    out
}

pub fn run(data: &[u8]) {
    let cols = columns(data);
    let Ok(ch) = Channel::table(cols.clone()) else {
        return;
    };
    let len = cols.first().map_or(0, |(_, c)| c.len());
    assert_eq!(ch.len(), len);
    for (c, (name, column)) in cols.iter().enumerate() {
        // A channel refuses a name given twice, so each names its own.
        assert_eq!(ch.col(name), Some(c));
        for row in 0..=len {
            let cell = ch.get(row, c);
            let ok = match (column, cell) {
                (_, Cell::Null) => row >= len,
                (Column::F64(_), Cell::F64(_)) | (Column::I64(_), Cell::I64(_)) => true,
                (Column::Str(_), Cell::Str(_)) => true,
                _ => false,
            };
            assert!(ok, "row {row} of `{name}` read {cell:?}");
        }
    }
    let handle = ch.handle();
    assert_eq!(Channel::resolve(&handle).map(|c| c.len()), Some(len));
    reads(&ch, &cols, data);
    if let Ok(next) = ch.publish(cols) {
        assert_eq!(next.rev(), ch.rev() + 1);
    }
    drop(ch);
    assert!(Channel::resolve(&handle).is_none());
}

fn reads(ch: &Channel, cols: &[(String, Column)], data: &[u8]) {
    let n = cols.len();
    if n == 0 {
        return;
    }
    let len = ch.len();
    let mut ops = data.iter().rev().copied();
    let mut next = move || ops.next().unwrap_or(0);
    let cap = usize::from(next() % 40);
    if let Ok(a) = ch.append(cols.to_vec(), (cap > 0).then_some(cap)) {
        let want = if cap > 0 { (2 * len).min(cap) } else { 2 * len };
        assert_eq!(a.len(), want, "appended {len} rows under a cap of {cap}");
        for c in 0..n {
            let whole = a.column(c).expect("every column reads whole");
            assert_eq!(whole.len(), a.len());
            for r in 0..a.len() {
                assert_eq!(
                    whole.get(r),
                    a.get(r, c),
                    "row {r} of column {c}, whole and by row"
                );
            }
        }
    }
    let c = usize::from(next()) % n;
    if let Ok(rows) = ch.sort(c, next() & 1 == 1, None) {
        let mut seen = rows.clone();
        seen.sort_unstable();
        assert!(seen.into_iter().eq(0..len), "a sort is a permutation");
    }
    let (lo, hi) = (f64::from(next()) - 128.0, f64::from(next()) - 128.0);
    let range = Match::Range {
        min: Some(lo),
        max: Some(hi),
    };
    if let Ok(rows) = ch.filter(c, &range, None) {
        for &r in &rows {
            let v = match ch.get(r, c) {
                Cell::F64(v) => v,
                Cell::I64(v) => v as f64,
                other => panic!("row {r} of a number filter reads {other:?}"),
            };
            assert!(v >= lo && v <= hi, "{v} is outside {lo}..={hi}");
        }
        if let Ok(t) = ch.take(&rows) {
            assert_eq!(t.len(), rows.len());
        }
    }
    let text = Match::Contains {
        text: "a".into(),
        case: next() & 1 == 1,
    };
    if let Ok(rows) = ch.filter(c, &text, None) {
        assert!(rows.iter().all(|&r| r < len));
    }
    let pick = |b: u8| usize::from(b) % n;
    let (s, l, d) = (pick(next()), pick(next()), pick(next()));
    if let Ok(lanes) = ch.lanes(s, Some(l), Some(d)) {
        let mut out = Vec::new();
        for lane in 0..=lanes.count().min(8) {
            let v0 = f64::from(next()) - 128.0;
            let v1 = v0 + f64::from(next());
            lanes.window(lane, v0, v1, f64::from(next()) / 16.0, &mut out);
            assert!(out.iter().all(|r| r.row.is_none_or(|row| row < len)));
        }
    }
}
