//! Size expressions (backlog F109): CSS's `min()`, `max()` and `clamp()`
//! over lengths and percentages, resolved by layout against the parent's
//! content box — the same box a `Percent` sizing takes its cut of.
//!
//! ```text
//! size  := number ["px"] | number "%" | fn "(" size ("," size)* ")"
//! fn    := "min" | "max" | "clamp"          -- clamp takes exactly three
//! ```
//!
//! `"clamp(400px, 80%, 1000px)"` is 80% of the room, never under 400 nor
//! over 1000 — and, as CSS has it, the minimum wins when it is over the
//! maximum. An expression with no percentage in it is a length
//! (`"min(300px, 400)"` is `Fixed(300)`), a bare percentage is a
//! `Percent`, and only what depends on the room becomes a [`Calc`].
//!
//! The same expression as data, for a binding that would rather not
//! spell it ([`from_value`]): a number is px, `{ pct = N }` or
//! `{ percent = N }` a percentage, `{ px = N }` a length, and a function
//! a one-key table of its arguments — `{ clamp = { 400, { pct = 80 },
//! 1000 } }` in Lua, `{ clamp: [400, { percent: 80 }, 1000] }` in JS. A
//! string may stand anywhere an argument does. C builds one with
//! `kui_size_clamp(kui_size_px(400), kui_size_pct(80), kui_size_px(1000))`
//! and Rust with [`Expr`] and [`intern`]. A spelling a frame declares
//! again is found by its text before it is parsed, so a string costs a
//! lookup after the first frame.
//!
//! A [`Calc`] is a handle — `LayoutSpec` is `Copy` and copied per node per
//! frame, so the tree it names lives in a process-wide table, one entry
//! per distinct expression (equal by structure), which a frame that
//! declares the same expression again finds rather than adds to. The
//! table holds at most [`MAX_CALCS`] entries: expressions come from a
//! view's source, so a program that reaches the cap is spelling a new one
//! per frame (`format!("clamp({n}px, …)")`), which a parse then refuses.

use std::collections::HashMap;
use std::fmt;
use std::sync::{Arc, OnceLock, RwLock};

/// How many distinct expressions the table keeps.
pub const MAX_CALCS: usize = 1 << 16;

/// How deep an expression nests: at most this many functions inside one
/// another, whatever built it — the grammar, data, prefix code, C's
/// builders or a Rust tree handed to [`intern`]. Every walk of a tree
/// (evaluating, hashing, comparing, spelling, dropping) recurses, so a
/// tree the table keeps is one those walks can finish; a spelling of
/// `"min("` a hundred thousand times aborted the process on the parser's
/// stack before this (backlog RG79).
pub const MAX_DEPTH: u32 = 32;

fn too_deep(what: &str) -> String {
    format!("bad size{what}: nested past {MAX_DEPTH}")
}

/// A number an expression may hold: finite, and `-0` as `0`. The table
/// finds an entry by its numbers' bits, and `NaN` is equal to nothing —
/// not even itself — so `{ min = { 0/0, { pct = 50 } } }` declared each
/// frame was a new entry each frame, towards the cap every view shares;
/// and `-0` and `0` were two entries for one expression (backlog RG80).
/// Neither infinity means anything a room can be cut to either.
fn finite(v: f32) -> Result<f32, String> {
    if v.is_finite() {
        // `-0.0 == 0.0`, so this is `0.0` for either.
        Ok(if v == 0.0 { 0.0 } else { v })
    } else {
        Err(format!("bad size: {v} is not a finite number"))
    }
}

/// Holds a tree to what the table keeps — nested no deeper than
/// [`MAX_DEPTH`], its numbers [`finite`] and `-0` made `0` — at depth
/// `depth`: what [`intern`] and [`norm`] make of a tree built by hand (C's
/// builders, a Rust `Expr`), stopping at the first level past the cap
/// rather than walking the rest (backlog RG79).
fn canon(e: &mut Expr, depth: u32) -> Result<(), String> {
    if depth > MAX_DEPTH {
        return Err(too_deep(""));
    }
    match e {
        Expr::Px(v) | Expr::Pct(v) => {
            *v = finite(*v)?;
            Ok(())
        }
        Expr::Min(xs) | Expr::Max(xs) => xs.iter_mut().try_for_each(|x| canon(x, depth + 1)),
        Expr::Clamp(a, b, c) => {
            canon(a, depth + 1)?;
            canon(b, depth + 1)?;
            canon(c, depth + 1)
        }
    }
}

/// A parsed expression: lengths in logical px, percentages as fractions.
#[derive(Clone, Debug, PartialEq)]
pub enum Expr {
    Px(f32),
    /// A fraction of the room (`"50%"` is `Pct(0.5)`).
    Pct(f32),
    Min(Vec<Expr>),
    Max(Vec<Expr>),
    /// `clamp(min, target, max)`.
    Clamp(Box<Expr>, Box<Expr>, Box<Expr>),
}

impl Expr {
    /// The expression in logical px of `room` px, never below zero.
    pub fn resolve(&self, room: f32) -> f32 {
        self.eval(room).max(0.0)
    }

    fn eval(&self, room: f32) -> f32 {
        match self {
            Expr::Px(px) => *px,
            Expr::Pct(f) => room * f,
            Expr::Min(xs) => xs
                .iter()
                .map(|x| x.eval(room))
                .fold(f32::INFINITY, f32::min),
            Expr::Max(xs) => xs
                .iter()
                .map(|x| x.eval(room))
                .fold(f32::NEG_INFINITY, f32::max),
            // CSS's order: the minimum over the maximum.
            Expr::Clamp(lo, target, hi) => target.eval(room).min(hi.eval(room)).max(lo.eval(room)),
        }
    }

    /// Whether the value depends on the room: a percentage anywhere in it.
    pub fn relative(&self) -> bool {
        match self {
            Expr::Px(_) => false,
            Expr::Pct(_) => true,
            Expr::Min(xs) | Expr::Max(xs) => xs.iter().any(Expr::relative),
            Expr::Clamp(a, b, c) => a.relative() || b.relative() || c.relative(),
        }
    }
}

/// Structural equality and hashing by the numbers' bits: what the table
/// finds an entry by, so data and prefix code find theirs without being
/// spelled out first.
impl Eq for Expr {}

impl std::hash::Hash for Expr {
    fn hash<H: std::hash::Hasher>(&self, h: &mut H) {
        match self {
            Expr::Px(v) => (0u8, v.to_bits()).hash(h),
            Expr::Pct(v) => (1u8, v.to_bits()).hash(h),
            Expr::Min(xs) => (2u8, xs).hash(h),
            Expr::Max(xs) => (3u8, xs).hash(h),
            Expr::Clamp(a, b, c) => (4u8, a, b, c).hash(h),
        }
    }
}

impl fmt::Display for Expr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fn list(f: &mut fmt::Formatter<'_>, name: &str, xs: &[&Expr]) -> fmt::Result {
            write!(f, "{name}(")?;
            for (i, x) in xs.iter().enumerate() {
                if i > 0 {
                    write!(f, ", ")?;
                }
                write!(f, "{x}")?;
            }
            write!(f, ")")
        }
        match self {
            Expr::Px(px) => write!(f, "{px}px"),
            Expr::Pct(p) => write!(f, "{}%", p * 100.0),
            Expr::Min(xs) => list(f, "min", &xs.iter().collect::<Vec<_>>()),
            Expr::Max(xs) => list(f, "max", &xs.iter().collect::<Vec<_>>()),
            Expr::Clamp(a, b, c) => list(f, "clamp", &[a, b, c]),
        }
    }
}

/// An expression that depends on the room, by its place in the table.
/// `Copy`, so a `Sizing` holding one still is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Calc(u32);

impl Calc {
    /// The expression in logical px of `room` px — evaluated under the
    /// table's read lock, since layout asks once per node that has one.
    pub fn resolve(self, room: f32) -> f32 {
        table()
            .read()
            .ok()
            .and_then(|t| t.exprs.get(self.0 as usize).map(|e| e.resolve(room)))
            .unwrap_or(0.0)
    }

    /// The tree the handle names.
    pub fn expr(self) -> Option<Arc<Expr>> {
        table().read().ok()?.exprs.get(self.0 as usize).cloned()
    }

    /// The handle's number: what a binding carries across (the C ABI's
    /// `KUI_CALC` sizing holds it in its `value`).
    pub fn id(self) -> u32 {
        self.0
    }

    /// A handle by number, when the table has one there.
    pub fn from_id(id: u32) -> Option<Calc> {
        let t = table().read().ok()?;
        ((id as usize) < t.exprs.len()).then_some(Calc(id))
    }

    /// The canonical spelling (`clamp(400px, 80%, 1000px)`), for a reader.
    pub fn describe(self) -> String {
        self.expr()
            .map_or_else(|| "calc(?)".into(), |e| e.to_string())
    }
}

#[derive(Default)]
struct Table {
    exprs: Vec<Arc<Expr>>,
    by_expr: HashMap<Expr, u32>,
    /// What a spelling came to, by the text a binding handed in — so the
    /// view that declares `"clamp(400px, 80%, 1000px)"` every frame parses
    /// it once. Bounded with the rest ([`MAX_CALCS`]); past it, a spelling
    /// is parsed each time and not kept.
    by_input: HashMap<String, Norm>,
    /// The same for prefix code, by its slots' bytes: a Node view that
    /// declares `{ clamp: [...] }` every frame builds the tree once.
    by_code: HashMap<Box<[u8]>, Norm>,
}

/// An expression reduced to what it needs to be.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Norm {
    Px(f32),
    Pct(f32),
    Calc(Calc),
}

fn norm(mut e: Expr) -> Result<Norm, String> {
    // Before `relative` and `resolve` walk it: a tree from C's builders
    // or a Rust caller has had no cap on the way in.
    canon(&mut e, 0)?;
    Ok(match e {
        Expr::Pct(f) => Norm::Pct(f),
        e if !e.relative() => Norm::Px(e.resolve(0.0)),
        e => Norm::Calc(intern_checked(e)?),
    })
}

/// A spelling reduced, through the input cache.
fn norm_str(s: &str) -> Result<Norm, String> {
    if let Some(n) = table().read().ok().and_then(|t| t.by_input.get(s).copied()) {
        return Ok(n);
    }
    let n = norm(parse(s)?)?;
    if let Ok(mut t) = table().write()
        && t.by_input.len() < MAX_CALCS
    {
        t.by_input.insert(s.to_string(), n);
    }
    Ok(n)
}

fn norm_sizing(n: Norm) -> crate::spec::Sizing {
    use crate::spec::Sizing;
    match n {
        Norm::Px(px) => Sizing::Fixed(px),
        Norm::Pct(f) => Sizing::Percent(f),
        Norm::Calc(c) => Sizing::Calc(c),
    }
}

fn norm_bound(n: Norm) -> Result<crate::spec::Bound, String> {
    use crate::spec::Bound;
    Ok(match n {
        Norm::Px(px) => Bound::Px(px),
        // A percentage clamp needs the room as much as a calc does.
        Norm::Pct(f) => Bound::Calc(intern(Expr::Pct(f))?),
        Norm::Calc(c) => Bound::Calc(c),
    })
}

/// A size expression as data (see the module's doc): a number, a
/// string, `{ pct }` / `{ percent }` / `{ px }`, or a one-key
/// `{ min | max | clamp = [args] }`.
pub fn from_value(v: &crate::value::Value) -> Result<Expr, String> {
    value_at(v, 0)
}

fn value_at(v: &crate::value::Value, depth: u32) -> Result<Expr, String> {
    use crate::value::Value;
    if depth > MAX_DEPTH {
        return Err(too_deep(""));
    }
    match v {
        Value::Int(_) | Value::Float(_) => {
            Ok(Expr::Px(finite(v.as_float().unwrap_or(0.0) as f32)?))
        }
        Value::Str(s) => parse(s),
        Value::Map(m) => {
            let mut it = m.iter();
            let (Some((k, arg)), None) = (it.next(), it.next()) else {
                return Err(
                    "bad size: a table names one of pct, percent, px, min, max, clamp".into(),
                );
            };
            let num = || {
                arg.as_float()
                    .map(|n| n as f32)
                    .ok_or_else(|| format!("bad size: {k} takes a number"))
                    .and_then(finite)
            };
            let args = || -> Result<Vec<Expr>, String> {
                let Value::List(xs) = arg else {
                    return Err(format!("bad size: {k} takes a list"));
                };
                if xs.is_empty() {
                    return Err(format!("bad size: {k} takes at least one"));
                }
                xs.iter().map(|x| value_at(x, depth + 1)).collect()
            };
            match k.as_str() {
                "pct" | "percent" => Ok(Expr::Pct(finite(num()? / 100.0)?)),
                "px" => Ok(Expr::Px(num()?)),
                "min" => Ok(Expr::Min(args()?)),
                "max" => Ok(Expr::Max(args()?)),
                "clamp" => match <[Expr; 3]>::try_from(args()?) {
                    Ok([a, b, c]) => Ok(Expr::Clamp(Box::new(a), Box::new(b), Box::new(c))),
                    Err(_) => Err("bad size: clamp takes three: clamp(MIN, TARGET, MAX)".into()),
                },
                _ => Err(format!(
                    "bad size: no {k:?} (pct, percent, px, min, max, clamp)"
                )),
            }
        }
        _ => Err("bad size: a number, a string or a table".into()),
    }
}

/// A size expression in prefix code, the form a transport that carries
/// only numbers sends (the Node wire's `SIZE_MODE_TREE`, v19): `1 px`,
/// `2 fraction`, `3 n args…` (min), `4 n args…` (max), `5 a b c`
/// (clamp). The whole slice is one expression.
pub fn from_code(code: &[f64]) -> Result<Expr, String> {
    fn one(code: &[f64], at: &mut usize, depth: u32) -> Result<Expr, String> {
        let mut next = || -> Result<f64, String> {
            let v = code.get(*at).copied().ok_or("bad size code: truncated")?;
            *at += 1;
            Ok(v)
        };
        if depth > MAX_DEPTH {
            return Err(too_deep(" code"));
        }
        Ok(match next()? as u32 {
            1 => Expr::Px(finite(next()? as f32)?),
            2 => Expr::Pct(finite(next()? as f32)?),
            op @ (3 | 4) => {
                let n = next()? as usize;
                if n == 0 || n > code.len() {
                    return Err("bad size code: an argument count out of range".into());
                }
                let args = (0..n)
                    .map(|_| one(code, at, depth + 1))
                    .collect::<Result<Vec<_>, _>>()?;
                if op == 3 {
                    Expr::Min(args)
                } else {
                    Expr::Max(args)
                }
            }
            5 => {
                let a = one(code, at, depth + 1)?;
                let b = one(code, at, depth + 1)?;
                let c = one(code, at, depth + 1)?;
                Expr::Clamp(Box::new(a), Box::new(b), Box::new(c))
            }
            op => return Err(format!("bad size code: no op {op}")),
        })
    }
    let mut at = 0;
    let e = one(code, &mut at, 0)?;
    if at != code.len() {
        return Err("bad size code: slots left over".into());
    }
    Ok(e)
}

/// Prefix code reduced, through the code cache.
fn norm_code(code: &[f64]) -> Result<Norm, String> {
    // SAFETY: an `f64` slice is initialised bytes, and a `u8` view of it
    // has no alignment to keep; the view lives only for the lookup.
    let bytes =
        unsafe { std::slice::from_raw_parts(code.as_ptr().cast::<u8>(), size_of_val(code)) };
    if let Some(n) = table()
        .read()
        .ok()
        .and_then(|t| t.by_code.get(bytes).copied())
    {
        return Ok(n);
    }
    let n = norm(from_code(code)?)?;
    if let Ok(mut t) = table().write()
        && t.by_code.len() < MAX_CALCS
    {
        t.by_code.insert(bytes.into(), n);
    }
    Ok(n)
}

/// A size expression in prefix code, as a sizing.
pub fn sizing_code(code: &[f64]) -> Result<crate::spec::Sizing, String> {
    norm_code(code).map(norm_sizing)
}

/// A size expression in prefix code, as a clamp.
pub fn bound_code(code: &[f64]) -> Result<crate::spec::Bound, String> {
    norm_bound(norm_code(code)?)
}

/// A size expression as data, as a sizing.
pub fn sizing_value(v: &crate::value::Value) -> Result<crate::spec::Sizing, String> {
    match v {
        crate::value::Value::Str(s) => sizing(s),
        v => Ok(norm_sizing(norm(from_value(v)?)?)),
    }
}

/// A size expression as data, as a clamp.
pub fn bound_value(v: &crate::value::Value) -> Result<crate::spec::Bound, String> {
    match v {
        crate::value::Value::Str(s) => bound(s),
        v => norm_bound(norm(from_value(v)?)?),
    }
}

/// An expression tree built by hand, as a sizing: C's builders and a
/// Rust view that composes one.
pub fn sizing_of(e: Expr) -> Result<crate::spec::Sizing, String> {
    Ok(norm_sizing(norm(e)?))
}

fn table() -> &'static RwLock<Table> {
    static TABLE: OnceLock<RwLock<Table>> = OnceLock::new();
    TABLE.get_or_init(Default::default)
}

/// The handle for `expr`: the one an equal expression already has, or a
/// new one.
pub fn intern(mut expr: Expr) -> Result<Calc, String> {
    canon(&mut expr, 0)?;
    intern_checked(expr)
}

/// [`intern`] for a tree [`canon`] has passed.
fn intern_checked(expr: Expr) -> Result<Calc, String> {
    if let Some(&id) = table()
        .read()
        .map_err(|e| e.to_string())?
        .by_expr
        .get(&expr)
    {
        return Ok(Calc(id));
    }
    let mut t = table().write().map_err(|e| e.to_string())?;
    if let Some(&id) = t.by_expr.get(&expr) {
        return Ok(Calc(id));
    }
    if t.exprs.len() >= MAX_CALCS {
        return Err(format!(
            "too many distinct size expressions ({MAX_CALCS}): \"{expr}\" is not kept — declare \
             one per layout, not one per frame"
        ));
    }
    let id = t.exprs.len() as u32;
    t.exprs.push(Arc::new(expr.clone()));
    t.by_expr.insert(expr, id);
    Ok(Calc(id))
}

/// Parses a size expression; the public grammar, which a host validating
/// its own settings reuses so what it accepts is what kui draws.
pub fn parse(s: &str) -> Result<Expr, String> {
    let mut p = Parser {
        s: s.as_bytes(),
        at: 0,
    };
    let e = p.expr(0)?;
    p.skip_ws();
    if p.at < p.s.len() {
        return Err(p.error("the end"));
    }
    Ok(e)
}

/// What a spelling is as a sizing: a length, a percentage, or a
/// [`Calc`] for anything else.
pub fn sizing(s: &str) -> Result<crate::spec::Sizing, String> {
    norm_str(s).map(norm_sizing)
}

/// What a spelling is as a clamp: a length, or a [`Calc`] for one that
/// depends on the room.
pub fn bound(s: &str) -> Result<crate::spec::Bound, String> {
    norm_bound(norm_str(s)?)
}

struct Parser<'a> {
    s: &'a [u8],
    at: usize,
}

impl Parser<'_> {
    fn skip_ws(&mut self) {
        while self.s.get(self.at).is_some_and(|c| c.is_ascii_whitespace()) {
            self.at += 1;
        }
    }

    fn error(&self, wanted: &str) -> String {
        let rest = String::from_utf8_lossy(&self.s[self.at.min(self.s.len())..]);
        if rest.is_empty() {
            format!("bad size: {wanted} expected at the end")
        } else {
            format!("bad size: {wanted} expected at {rest:?}")
        }
    }

    fn eat(&mut self, word: &str) -> bool {
        self.skip_ws();
        if self.s[self.at..].starts_with(word.as_bytes()) {
            self.at += word.len();
            true
        } else {
            false
        }
    }

    /// One argument, `depth` functions in: past [`MAX_DEPTH`] it is
    /// refused before it is read, so the recursion is bounded by the cap
    /// and not by the input (backlog RG79).
    fn expr(&mut self, depth: u32) -> Result<Expr, String> {
        if depth > MAX_DEPTH {
            return Err(too_deep(""));
        }
        self.skip_ws();
        for name in ["clamp", "min", "max"] {
            if self.s[self.at..].starts_with(name.as_bytes()) {
                self.at += name.len();
                if !self.eat("(") {
                    return Err(self.error("\"(\""));
                }
                let mut args = vec![self.expr(depth + 1)?];
                while self.eat(",") {
                    args.push(self.expr(depth + 1)?);
                }
                if !self.eat(")") {
                    return Err(self.error("\",\" or \")\""));
                }
                return match name {
                    "clamp" => match <[Expr; 3]>::try_from(args) {
                        Ok([a, b, c]) => Ok(Expr::Clamp(Box::new(a), Box::new(b), Box::new(c))),
                        Err(_) => {
                            Err("bad size: clamp takes three: clamp(MIN, TARGET, MAX)".into())
                        }
                    },
                    "min" => Ok(Expr::Min(args)),
                    _ => Ok(Expr::Max(args)),
                };
            }
        }
        let start = self.at;
        while self
            .s
            .get(self.at)
            .is_some_and(|c| c.is_ascii_digit() || *c == b'.')
        {
            self.at += 1;
        }
        let n: f32 = std::str::from_utf8(&self.s[start..self.at])
            .ok()
            .and_then(|t| t.parse().ok())
            .ok_or_else(|| {
                self.at = start;
                self.error("a number, \"N%\", \"Npx\", min(…), max(…) or clamp(…)")
            })?;
        // Digits alone can still overflow an `f32` (forty of them do):
        // an infinity is refused as `NaN` is from data (backlog RG80).
        let n = finite(n)?;
        if self.eat("%") {
            Ok(Expr::Pct(finite(n / 100.0)?))
        } else {
            self.eat("px");
            Ok(Expr::Px(n))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::spec::{Bound, Sizing};

    fn px(s: &str, room: f32) -> f32 {
        parse(s).unwrap().resolve(room)
    }

    #[test]
    fn expressions_resolve_against_the_room() {
        let c = "clamp(400px, 80%, 1000px)";
        assert_eq!(px(c, 300.0), 400.0, "the minimum");
        assert_eq!(px(c, 1000.0), 800.0, "the target");
        assert_eq!(px(c, 2000.0), 1000.0, "the maximum");
        assert_eq!(
            px("clamp(500, 10%, 200)", 1000.0),
            500.0,
            "the minimum over the maximum"
        );
        assert_eq!(px("min(720px, 100%)", 500.0), 500.0);
        assert_eq!(px("max(50%, 300)", 400.0), 300.0);
        assert_eq!(px("min(clamp(1, 50%, 900), 30%)", 1000.0), 300.0, "nested");
    }

    #[test]
    fn only_what_depends_on_the_room_is_a_calc() {
        assert_eq!(sizing("720px").unwrap(), Sizing::Fixed(720.0));
        assert_eq!(sizing("min(300px, 400)").unwrap(), Sizing::Fixed(300.0));
        assert_eq!(sizing("50%").unwrap(), Sizing::Percent(0.5));
        let Sizing::Calc(a) = sizing("clamp(400px,80%,1000px)").unwrap() else {
            panic!("a calc");
        };
        let Sizing::Calc(b) = sizing(" clamp( 400 , 80% , 1000px ) ").unwrap() else {
            panic!("a calc");
        };
        assert_eq!(a, b, "one entry per expression, however spelled");
        assert_eq!(a.describe(), "clamp(400px, 80%, 1000px)");
        assert_eq!(bound("300").unwrap(), Bound::Px(300.0));
        assert!(matches!(bound("50%").unwrap(), Bound::Calc(_)));
    }

    #[test]
    fn the_same_expression_as_data() {
        use crate::value::Value;
        let list = |xs: Vec<Value>| Value::List(xs);
        let map = |k: &str, v: Value| Value::Map([(k.to_string(), v)].into_iter().collect());
        let v = map(
            "clamp",
            list(vec![
                Value::Int(400),
                map("pct", Value::Int(80)),
                Value::Str("1000px".into()),
            ]),
        );
        let Sizing::Calc(a) = sizing_value(&v).unwrap() else {
            panic!("a calc");
        };
        assert_eq!(
            Some(a),
            match sizing("clamp(400px, 80%, 1000px)").unwrap() {
                Sizing::Calc(c) => Some(c),
                _ => None,
            },
            "one entry, spelled or built"
        );
        assert_eq!(
            sizing_value(&map("percent", Value::Int(50))).unwrap(),
            Sizing::Percent(0.5)
        );
        assert_eq!(
            sizing_value(&map("min", list(vec![Value::Int(300), Value::Int(400)]))).unwrap(),
            Sizing::Fixed(300.0)
        );
        assert!(
            sizing_value(&map("clamp", list(vec![Value::Int(1)])))
                .unwrap_err()
                .contains("three")
        );
        assert!(
            sizing_value(&map("wide", Value::Int(1)))
                .unwrap_err()
                .contains("no \"wide\"")
        );
    }

    #[test]
    fn the_same_expression_in_prefix_code() {
        let code = [5.0, 1.0, 400.0, 2.0, 0.8, 3.0, 2.0, 1.0, 1000.0, 2.0, 1.0];
        assert_eq!(
            from_code(&code).unwrap().to_string(),
            "clamp(400px, 80%, min(1000px, 100%))"
        );
        assert!(from_code(&code[..4]).unwrap_err().contains("truncated"));
        assert!(
            from_code(&[1.0, 3.0, 9.0])
                .unwrap_err()
                .contains("left over")
        );
        assert!(from_code(&[9.0]).unwrap_err().contains("no op 9"));
    }

    #[test]
    fn a_bad_one_says_where() {
        assert_eq!(
            parse("80 %x").unwrap_err(),
            "bad size: the end expected at \"x\""
        );
        assert!(parse("clamp(1, 2)").unwrap_err().contains("three"));
        assert!(parse("wide").unwrap_err().contains("at \"wide\""));
        assert!(parse("min(1, 2").unwrap_err().contains("at the end"));
    }

    /// `n` functions nested, spelled, as data and as prefix code.
    fn nested(n: usize) -> (String, crate::value::Value, Vec<f64>, Expr) {
        use crate::value::Value;
        let spelled = format!("{}50%{}", "min(".repeat(n), ")".repeat(n));
        let mut data = Value::Map(vec![("pct".into(), Value::Int(50))]);
        let mut code = [3.0, 1.0].repeat(n);
        code.extend([2.0, 0.5]);
        let mut built = Expr::Pct(0.5);
        for _ in 0..n {
            data = Value::Map(vec![("min".into(), Value::List(vec![data]))]);
            built = Expr::Min(vec![built]);
        }
        (spelled, data, code, built)
    }

    /// Nesting stops at [`MAX_DEPTH`] whatever builds the tree: the
    /// parser and data recursed as deep as the input went, and `"min("`
    /// a hundred thousand times overflowed the stack (backlog RG79).
    #[test]
    fn nesting_stops_at_the_cap() {
        let deep = parse(&"min(".repeat(100_000)).unwrap_err();
        assert_eq!(deep, "bad size: nested past 32");
        let (s, v, code, e) = nested(MAX_DEPTH as usize);
        assert!(parse(&s).is_ok(), "32 is allowed");
        assert!(from_value(&v).is_ok());
        assert!(from_code(&code).is_ok());
        assert!(intern(e).is_ok());
        let (s, v, code, e) = nested(MAX_DEPTH as usize + 1);
        assert_eq!(parse(&s).unwrap_err(), "bad size: nested past 32");
        assert_eq!(from_value(&v).unwrap_err(), "bad size: nested past 32");
        assert_eq!(
            from_code(&code).unwrap_err(),
            "bad size code: nested past 32"
        );
        assert_eq!(intern(e.clone()).unwrap_err(), "bad size: nested past 32");
        assert!(sizing_of(e).is_err(), "a tree built by hand is held to it");
        assert!(sizing(&s).is_err());
    }

    /// `NaN` equals nothing, so an expression holding one was a new entry
    /// each time it was declared — a view declaring it every frame filled
    /// the table — and `-0` was an entry apart from `0` (backlog RG80).
    /// Every way in refuses a number that is not finite and reads `-0`
    /// as `0`.
    #[test]
    fn only_finite_numbers_and_one_zero() {
        use crate::value::Value;
        let map = |k: &str, v: Value| Value::Map(vec![(k.to_string(), v)]);
        let with = |n: f64| {
            map(
                "min",
                Value::List(vec![Value::Float(n), map("pct", Value::Int(50))]),
            )
        };
        for n in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY, 1e300] {
            let e = sizing_value(&with(n)).unwrap_err();
            assert!(e.contains("not a finite number"), "{n}: {e}");
            assert!(sizing_value(&map("px", Value::Float(n))).is_err());
            assert!(sizing_value(&map("pct", Value::Float(n))).is_err());
            assert!(from_code(&[3.0, 2.0, 1.0, n, 2.0, 0.5]).is_err());
            assert!(from_code(&[2.0, n]).is_err());
        }
        assert!(bound_value(&with(f64::NAN)).is_err());
        assert!(intern(Expr::Min(vec![Expr::Px(f32::NAN), Expr::Pct(0.5)])).is_err());
        assert!(sizing_of(Expr::Pct(f32::INFINITY)).is_err());
        let digits = format!("min(1{}px, 50%)", "0".repeat(40));
        assert!(parse(&digits).unwrap_err().contains("not a finite number"));
        assert!(parse(&format!("1{}%", "0".repeat(40))).is_err());

        let zero = sizing_value(&with(0.0)).unwrap();
        assert_eq!(sizing_value(&with(-0.0)).unwrap(), zero, "-0 is 0");
        assert_eq!(
            sizing_of(Expr::Min(vec![Expr::Px(-0.0), Expr::Pct(0.5)])).unwrap(),
            zero
        );
        assert_eq!(sizing_code(&[3.0, 2.0, 1.0, -0.0, 2.0, 0.5]).unwrap(), zero);
        assert_eq!(zero.describe(), "min(0px, 50%)");
    }
}
