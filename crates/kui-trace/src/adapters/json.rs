//! JSON written by hand: strings escaped as RFC 8259 says, numbers as
//! JSON has them, and a value an adapter carries as an argument.

use std::fmt::Write;

use super::Val;

/// `s` as a JSON string: quotes, backslashes and control characters
/// escaped, U+2028 and U+2029 too (a JavaScript reader's line breaks),
/// the rest as UTF-8.
pub fn string(out: &mut String, s: &str) {
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{8}' => out.push_str("\\b"),
            '\u{c}' => out.push_str("\\f"),
            c if (c as u32) < 0x20 || c == '\u{2028}' || c == '\u{2029}' => {
                let _ = write!(out, "\\u{:04x}", c as u32);
            }
            c => out.push(c),
        }
    }
    out.push('"');
}

/// `v` as a JSON number; `null` for one JSON has not (NaN, ±∞).
pub fn number(out: &mut String, v: f64) {
    if !v.is_finite() {
        out.push_str("null");
    } else if v.fract() == 0.0 && v.abs() < 1e15 {
        let _ = write!(out, "{}", v as i64);
    } else {
        // Rust writes the shortest digits that read back, never an
        // exponent: valid JSON.
        let _ = write!(out, "{v}");
    }
}

/// An argument's value.
pub fn val(out: &mut String, v: &Val) {
    match v {
        Val::Str(s) => string(out, s),
        Val::U(u) => {
            let _ = write!(out, "{u}");
        }
        Val::I(i) => {
            let _ = write!(out, "{i}");
        }
        Val::F(f) => number(out, *f),
        Val::B(b) => out.push_str(if *b { "true" } else { "false" }),
    }
}

/// `{"key":value,…}` of `args`.
pub fn object(out: &mut String, args: &[(&str, Val)]) {
    out.push('{');
    for (i, (k, v)) in args.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        string(out, k);
        out.push(':');
        val(out, v);
    }
    out.push('}');
}

/// `µs` as ms, for a format that counts in them.
pub fn ms(out: &mut String, us: u64) {
    number(out, us as f64 / 1000.0);
}
