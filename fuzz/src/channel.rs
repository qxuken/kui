//! Channels (ADR 0065): bytes read as column specs — a kind byte, a name
//! length and name, a row count and rows — built into a channel, then read
//! back by every row and column, resolved from its handle, published again
//! and dropped. The properties: nothing panics, a channel built is the
//! length its columns are, every cell reads as the kind its column is, and
//! a handle nobody holds resolves to nothing.

use kui_core::channel::{Cell, Channel, Column, Strs};

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
    if let Ok(next) = ch.publish(cols) {
        assert_eq!(next.rev(), ch.rev() + 1);
    }
    drop(ch);
    assert!(Channel::resolve(&handle).is_none());
}
