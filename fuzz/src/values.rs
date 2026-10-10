//! The parsers that read a prop as data — a [`Value`] tree, which is what
//! a Lua table, a JS object and a C host's JSON all arrive as: gradients,
//! keyframes, entrances, window and menu declarations, file dialogs and
//! size expressions as data or as the Node wire's prefix code.
//!
//! The tree is built from the bytes by [`value`], its map keys mostly
//! drawn from the words these parsers look for so the fuzzer reaches past
//! "not an object".

use arbitrary::{Result, Unstructured};
use kui_core::{
    FileDialog, MenuBar, MenuItem, Value, WindowConfig, calc, enter, gradient, keyframes,
};

pub fn run(data: &[u8]) {
    let mut u = Unstructured::new(data);
    let Ok(which) = u.arbitrary::<u8>() else {
        return;
    };
    if which % 8 == 7 {
        let Ok(code) = u.arbitrary::<Vec<f64>>() else {
            return;
        };
        size_code(&code);
        return;
    }
    let Ok(v) = value(&mut u, 0) else {
        return;
    };
    match which % 8 {
        0 => {
            let _ = gradient::parse(&v);
        }
        1 => {
            let _ = keyframes::parse(&v);
        }
        2 => {
            let _ = enter::parse(&v);
        }
        3 => {
            let _ = WindowConfig::from_value(&v);
        }
        4 => {
            let _ = MenuItem::from_value(&v);
            let _ = MenuBar::from_value(&v);
        }
        5 => {
            let _ = FileDialog::from_value(&v);
        }
        _ => size_value(&v),
    }
}

/// `calc::from_value`, and the reductions after it: what reads as an
/// expression reduces to a sizing and a clamp, short of a full table.
fn size_value(v: &Value) {
    let Ok(e) = calc::from_value(v) else {
        return;
    };
    for room in [0.0, 640.0] {
        let r = e.resolve(room);
        assert!(!r.is_nan() && r >= 0.0, "{v:?} at {room}: {r}");
    }
    if let Err(err) = calc::sizing_value(v) {
        assert!(
            calc::is_full(&err),
            "{v:?} reads but sizing_value says {err}"
        );
    }
    if let Err(err) = calc::bound_value(v) {
        assert!(
            calc::is_full(&err),
            "{v:?} reads but bound_value says {err}"
        );
    }
}

/// The Node wire's prefix code for a size (`SIZE_MODE_TREE`).
fn size_code(code: &[f64]) {
    let Ok(e) = calc::from_code(code) else {
        return;
    };
    let r = e.resolve(640.0);
    assert!(!r.is_nan() && r >= 0.0, "{code:?}: {r}");
    if let Err(err) = calc::sizing_code(code) {
        assert!(
            calc::is_full(&err),
            "{code:?} reads but sizing_code says {err}"
        );
    }
}

/// The words the data parsers read, so a map key is one of them more
/// often than chance would make it.
const KEYS: &[&str] = &[
    "accel",
    "activates",
    "alpha",
    "anchor",
    "angle",
    "at",
    "bg",
    "blur",
    "by",
    "checked",
    "clamp",
    "color",
    "colour",
    "config",
    "d",
    "default",
    "delay",
    "duration",
    "dx",
    "dy",
    "ease",
    "enabled",
    "extensions",
    "fileName",
    "file_name",
    "files",
    "filters",
    "from",
    "h",
    "height",
    "id",
    "items",
    "kind",
    "label",
    "max",
    "min",
    "mode",
    "multiple",
    "name",
    "offset",
    "opacity",
    "origin",
    "owner",
    "pct",
    "percent",
    "px",
    "radial",
    "radius",
    "role",
    "rotate",
    "scale",
    "side",
    "size",
    "stops",
    "submenu",
    "t",
    "tag",
    "title",
    "to",
    "w",
    "width",
    "x",
    "y",
];

/// The string values they compare against.
const WORDS: &[&str] = &[
    "",
    "open",
    "save",
    "folder",
    "popup",
    "toast",
    "window",
    "sidebar",
    "separator",
    "quit",
    "copy",
    "paste",
    "selectAll",
    "mod+s",
    "ctrl+shift+p",
    "top",
    "bottom",
    "left right",
    "linear",
    "radial",
    "#ff0000",
    "#00ff0080",
    "red",
    "50%",
    "clamp(10px, 50%, 1000px)",
    "$accent",
    "$gap",
];

/// A value at most a few levels deep. Numbers include the non-finite and
/// the out-of-range, which every binding can send.
pub fn value(u: &mut Unstructured<'_>, depth: u32) -> Result<Value> {
    let leaf = depth >= 5;
    Ok(match u.int_in_range(0..=if leaf { 4 } else { 6 })? {
        0 => Value::Null,
        1 => Value::Bool(u.arbitrary()?),
        2 => Value::Int(match u.int_in_range(0..=3)? {
            0 => u.int_in_range(-4..=4)?,
            1 => u.int_in_range(0..=1000)?,
            _ => u.arbitrary()?,
        }),
        3 => Value::Float(match u.int_in_range(0..=7)? {
            0 => f64::NAN,
            1 => f64::INFINITY,
            2 => f64::NEG_INFINITY,
            3 => -0.0,
            4 => u.int_in_range(0..=100)? as f64 / 100.0,
            5 => u.int_in_range(-2000..=2000)? as f64 / 4.0,
            _ => u.arbitrary()?,
        }),
        4 => Value::Str(string(u)?),
        5 => {
            let n = u.int_in_range(0..=6)?;
            Value::List((0..n).map(|_| value(u, depth + 1)).collect::<Result<_>>()?)
        }
        _ => {
            let n = u.int_in_range(0..=6)?;
            let mut fields = Vec::with_capacity(n);
            for _ in 0..n {
                let key = if u.ratio(7, 8)? {
                    (*u.choose(KEYS)?).to_string()
                } else {
                    u.arbitrary()?
                };
                fields.push((key, value(u, depth + 1)?));
            }
            Value::Map(fields)
        }
    })
}

fn string(u: &mut Unstructured<'_>) -> Result<String> {
    if u.ratio(1, 2)? {
        Ok((*u.choose(WORDS)?).to_string())
    } else {
        u.arbitrary()
    }
}
