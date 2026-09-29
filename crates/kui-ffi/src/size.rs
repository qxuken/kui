//! Size expressions for C (backlog F109): built from parts, so a host
//! parses no text — `kui_size_clamp(kui_size_px(400), kui_size_pct(80),
//! kui_size_px(1000))` — or parsed from a spelling (`kui_size_parse`).
//! Each hands back a `KuiSizing` already reduced: a length is
//! `KUI_FIXED`, a lone percentage `KUI_PERCENT`, and what depends on the
//! room `KUI_CALC`, its `value` the expression's number in the core's
//! table. The result goes in a `width` / `height`, or in one of the
//! `*_size` clamps `KuiSpec` carries since ABI 22.

use kui_core::calc::{self, Expr};
use kui_core::{Bound, Calc, Sizing};

use crate::convert::kstr;
use crate::types::{KuiSizing, KuiStr};

const FIT: KuiSizing = KuiSizing { tag: 0, value: 0.0 };

/// The expression a sizing stands for, when it is one: a length, a
/// percentage or a calc.
fn expr_of(s: KuiSizing) -> Option<Expr> {
    match s.tag {
        2 => Some(Expr::Px(s.value)),
        3 => Some(Expr::Pct(s.value)),
        4 => Calc::from_id(s.value as u32)
            .and_then(|c| c.expr())
            .map(|e| (*e).clone()),
        _ => None,
    }
}

/// A sizing as C carries it.
pub(crate) fn c_sizing(s: Sizing) -> KuiSizing {
    match s {
        Sizing::Fit => FIT,
        Sizing::Grow(v) => KuiSizing { tag: 1, value: v },
        Sizing::Fixed(v) => KuiSizing { tag: 2, value: v },
        Sizing::Percent(v) => KuiSizing { tag: 3, value: v },
        Sizing::Calc(c) => KuiSizing {
            tag: 4,
            value: c.id() as f32,
        },
    }
}

/// A `KuiSpec` `*_size` clamp: `KUI_FIXED`, `KUI_PERCENT` or `KUI_CALC`;
/// anything else (zeroed, `KUI_FIT`) is none, and the float clamp holds.
pub(crate) fn bound_of(s: KuiSizing) -> Option<Bound> {
    match s.tag {
        2 => Some(Bound::Px(s.value)),
        3 | 4 => {
            let e = expr_of(s)?;
            calc::intern(e).ok().map(Bound::Calc)
        }
        _ => None,
    }
}

fn reduced(e: Option<Expr>) -> KuiSizing {
    e.and_then(|e| calc::sizing_of(e).ok())
        .map_or(FIT, c_sizing)
}

/// `px` logical pixels: `KUI_FIXED`.
#[unsafe(no_mangle)]
pub extern "C" fn kui_size_px(px: f32) -> KuiSizing {
    KuiSizing { tag: 2, value: px }
}

/// `percent` percent of the room (80 for 80%): `KUI_PERCENT`.
#[unsafe(no_mangle)]
pub extern "C" fn kui_size_pct(percent: f32) -> KuiSizing {
    KuiSizing {
        tag: 3,
        value: percent / 100.0,
    }
}

fn args(ptr: *const KuiSizing, n: usize) -> Option<Vec<Expr>> {
    if ptr.is_null() || n == 0 {
        return None;
    }
    // SAFETY: the caller hands `n` sizings at `ptr`, read and not kept.
    let xs = unsafe { std::slice::from_raw_parts(ptr, n) };
    xs.iter().map(|s| expr_of(*s)).collect()
}

/// The smallest of `n` sizings (each a length, a percentage or a calc);
/// `KUI_FIT` when one is anything else.
#[unsafe(no_mangle)]
pub extern "C" fn kui_size_min(args_ptr: *const KuiSizing, n: usize) -> KuiSizing {
    reduced(args(args_ptr, n).map(Expr::Min))
}

/// The largest of `n` sizings; `KUI_FIT` when one is not a size.
#[unsafe(no_mangle)]
pub extern "C" fn kui_size_max(args_ptr: *const KuiSizing, n: usize) -> KuiSizing {
    reduced(args(args_ptr, n).map(Expr::Max))
}

/// `target` held between `lo` and `hi`, `lo` winning over `hi` as CSS's
/// `clamp()` has it; `KUI_FIT` when one is not a size.
#[unsafe(no_mangle)]
pub extern "C" fn kui_size_clamp(lo: KuiSizing, target: KuiSizing, hi: KuiSizing) -> KuiSizing {
    reduced((|| {
        Some(Expr::Clamp(
            Box::new(expr_of(lo)?),
            Box::new(expr_of(target)?),
            Box::new(expr_of(hi)?),
        ))
    })())
}

/// A sizing's spelling — `"fit"`, `"grow"`, `"120"`, `"50%"`,
/// `"clamp(400px, 80%, 1000px)"` — into `*out`; false (and `*out`
/// untouched) for one that is not.
#[unsafe(no_mangle)]
pub extern "C" fn kui_size_parse(s: KuiStr, out: *mut KuiSizing) -> bool {
    let Ok(sizing) = kui_core::schema::sizing_str(&kstr(s)) else {
        return false;
    };
    if !out.is_null() {
        // SAFETY: the caller hands a writable `KuiSizing`.
        unsafe { *out = c_sizing(sizing) };
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn built_and_spelled_are_one() {
        let built = kui_size_clamp(kui_size_px(400.0), kui_size_pct(80.0), kui_size_px(1000.0));
        let mut spelled = FIT;
        let s = "clamp(400px, 80%, 1000px)";
        assert!(kui_size_parse(
            KuiStr {
                ptr: s.as_ptr() as _,
                len: s.len()
            },
            &mut spelled
        ));
        assert_eq!((built.tag, built.value), (spelled.tag, spelled.value));
        assert_eq!(built.tag, 4);
        let lengths = [kui_size_px(300.0), kui_size_px(400.0)];
        let m = kui_size_min(lengths.as_ptr(), 2);
        assert_eq!((m.tag, m.value), (2, 300.0), "no percentage: a length");
        let bad = kui_size_clamp(FIT, kui_size_pct(50.0), kui_size_px(9.0));
        assert_eq!(bad.tag, 0, "fit is not a size");
    }

    /// A negative `KUI_FIXED` ceiling is a ceiling of 0, not the calc a
    /// negative `max_w` otherwise stands for (backlog RG78).
    #[test]
    fn a_negative_px_ceiling_is_zero() {
        let l = kui_core::NodeSpec::column()
            .with_bounds(
                None,
                bound_of(kui_size_px(-1.0)),
                None,
                bound_of(kui_size_px(f32::NAN)),
            )
            .layout;
        assert_eq!((l.max_w, l.max_h), (0.0, 0.0));
    }
}
