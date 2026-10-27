//! Design tokens from a DTCG file (ADR 0062): a W3C Design Tokens
//! Community Group format file, 2025.10, or a Resolver Module document
//! that composes several, read into kui's own [`Tokens`] — colours with a
//! light and a dark half, lengths in logical px, family fallback lists.
//!
//! One call takes either kind of file and says which it was: a document
//! with a `resolutionOrder` is a resolver, anything else a token file. A
//! resolver is resolved twice, once per half, through the modifier the
//! options name (`theme` by default, with contexts `light` and `dark`) —
//! or, when it has no modifier of that name, the one modifier whose
//! contexts are called `light` and `dark`, so a resolver exported as
//! `mode` or `appearance` loads with no options. Every other modifier
//! takes the context the options give it, else its `default`.
//!
//! ```rust
//! use kui_core::dtcg::{self, DtcgOptions};
//!
//! let file = r##"{
//!   "color": {
//!     "$type": "color",
//!     "ink": { "$value": { "colorSpace": "srgb", "components": [0.08, 0.08, 0.07] } },
//!     "accent": { "$value": "#ffd400" },
//!     "accentText": { "$value": "{color.ink}" }
//!   },
//!   "space": { "gutter": { "$type": "dimension", "$value": { "value": 1, "unit": "rem" } } },
//!   "font": { "code": { "$type": "fontFamily", "$value": ["IBM Plex Mono", "monospace"] } }
//! }"##;
//! let loaded = dtcg::from_str(file, None, &DtcgOptions::default()).unwrap();
//! assert!(loaded.warnings.is_empty(), "{:?}", loaded.warnings);
//! let names: Vec<_> = loaded.names.iter().map(|n| n.name.as_str()).collect();
//! assert_eq!(names, ["colorInk", "colorAccent", "colorAccentText", "spaceGutter", "fontCode"]);
//! assert_eq!(loaded.names[3].path, "space.gutter");
//! ```
//!
//! What kui has no kind for — `shadow`, `border`, `typography`,
//! `gradient`, `transition`, `strokeStyle`, `duration`, `cubicBezier`,
//! `fontWeight` — is skipped with a [`DtcgWarning`] naming the token, as
//! are a colour clamped into sRGB and two paths that make the same name.
//! Legacy spellings that real exports still carry are read too: a colour
//! as a `"#rrggbb"` string, a dimension as `"12px"` or `"1.5rem"`.

use std::path::{Path, PathBuf};

use crate::color::Color;
use crate::json;
use crate::tokens::Tokens;
use crate::value::Value;

/// How a DTCG file becomes kui's tokens.
#[derive(Clone, Debug, PartialEq)]
pub struct DtcgOptions {
    /// The resolver modifier whose contexts are the two halves.
    pub modifier: String,
    /// Its context for the light half.
    pub light: String,
    /// Its context for the dark half.
    pub dark: String,
    /// A context for any other modifier, by name; a modifier left out
    /// takes its `default`.
    pub inputs: Vec<(String, String)>,
    /// Logical px in one `rem`.
    pub rem_px: f32,
    /// Put in front of every name, so a brand's tokens cannot collide with
    /// an app's: `"brand"` makes `color.ink` `brandColorInk`.
    pub prefix: String,
}

impl Default for DtcgOptions {
    fn default() -> Self {
        DtcgOptions {
            modifier: "theme".into(),
            light: "light".into(),
            dark: "dark".into(),
            inputs: Vec::new(),
            rem_px: 16.0,
            prefix: String::new(),
        }
    }
}

/// What a loaded token became, with its value: what a binding hands back
/// to an app that wants the table as data before declaring it.
#[derive(Clone, Debug, PartialEq)]
pub enum DtcgValue {
    /// A colour, each half; the same twice for a token file.
    Color { light: Color, dark: Color },
    /// A length in logical px (a `dimension`) or a plain `number`.
    Length(f32),
    /// A family fallback list.
    Family(Vec<String>),
}

/// One token as it was loaded: the name kui knows it by, the DTCG path it
/// came from, and its value.
#[derive(Clone, Debug, PartialEq)]
pub struct DtcgName {
    pub name: String,
    pub path: String,
    pub value: DtcgValue,
}

/// A token that was skipped or changed on the way in, by its DTCG path.
#[derive(Clone, Debug, PartialEq)]
pub struct DtcgWarning {
    pub path: String,
    pub message: String,
}

impl std::fmt::Display for DtcgWarning {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.path, self.message)
    }
}

/// What a load gives back: the table to declare, in file order, and what
/// each name came from.
#[derive(Debug)]
pub struct Dtcg {
    pub tokens: Tokens,
    pub names: Vec<DtcgName>,
    pub warnings: Vec<DtcgWarning>,
    /// Whether the document was a resolver (two halves) or a token file.
    pub resolver: bool,
}

/// Why nothing was loaded.
#[derive(Clone, Debug, PartialEq)]
pub struct DtcgError {
    pub message: String,
}

impl std::fmt::Display for DtcgError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for DtcgError {}

fn fail(message: impl Into<String>) -> DtcgError {
    DtcgError {
        message: message.into(),
    }
}

/// Loads the DTCG file at `path`: a token file, or a resolver whose `$ref`s
/// resolve relative to it.
pub fn from_path(path: impl AsRef<Path>, opts: &DtcgOptions) -> Result<Dtcg, DtcgError> {
    let path = path.as_ref();
    let src =
        std::fs::read_to_string(path).map_err(|e| fail(format!("{}: {e}", path.display())))?;
    let base = path.parent().map(Path::to_path_buf);
    from_str_named(&src, base.as_deref(), &path.display().to_string(), opts)
}

/// Loads a DTCG document from text; a resolver's relative `$ref`s resolve
/// against `base`, and are refused without one.
pub fn from_str(src: &str, base: Option<&Path>, opts: &DtcgOptions) -> Result<Dtcg, DtcgError> {
    from_str_named(src, base, "the document", opts)
}

fn from_str_named(
    src: &str,
    base: Option<&Path>,
    name: &str,
    opts: &DtcgOptions,
) -> Result<Dtcg, DtcgError> {
    let doc = json::parse(src).map_err(|e| fail(format!("{name}: {e}")))?;
    let mut warnings = Vec::new();
    let mut files = Files::new(base);
    let (light, dark, resolver) = if doc.get("resolutionOrder").is_some() {
        let modifier = pick_modifier(&doc, opts)?;
        let half = |ctx: &str, files: &mut Files| -> Result<Flat, DtcgError> {
            let mut inputs = opts.inputs.clone();
            if let Some(m) = &modifier {
                inputs.retain(|(k, _)| !k.eq_ignore_ascii_case(m));
                inputs.push((m.clone(), ctx.to_string()));
            }
            resolve(&doc, &inputs, files)
        };
        let l = half(&opts.light, &mut files)?;
        let d = half(&opts.dark, &mut files)?;
        if modifier.is_none() {
            warnings.push(DtcgWarning {
                path: String::new(),
                message: format!(
                    "no modifier named `{}` with contexts `{}` and `{}`: both halves are the same",
                    opts.modifier, opts.light, opts.dark
                ),
            });
        }
        (l, d, true)
    } else {
        let mut flat = Flat::default();
        flatten(&doc, &mut Vec::new(), None, &mut flat);
        (flat.clone(), flat, false)
    };
    let mut out = convert(&light, &dark, opts, &mut warnings);
    out.resolver = resolver;
    out.warnings.splice(0..0, warnings);
    Ok(out)
}

/// The modifier whose contexts are the halves: the one the options name,
/// else the one modifier that has both context names.
fn pick_modifier(doc: &Value, opts: &DtcgOptions) -> Result<Option<String>, DtcgError> {
    let Some(Value::Map(mods)) = doc.get("modifiers") else {
        return Ok(None);
    };
    let has_both = |m: &Value| {
        m.get("contexts")
            .is_some_and(|c| c.get(&opts.light).is_some() && c.get(&opts.dark).is_some())
    };
    if let Some((k, m)) = mods
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case(&opts.modifier))
    {
        if !has_both(m) {
            return Err(fail(format!(
                "modifier `{k}` has no contexts `{}` and `{}`",
                opts.light, opts.dark
            )));
        }
        return Ok(Some(k.clone()));
    }
    Ok(mods
        .iter()
        .find(|(_, m)| has_both(m))
        .map(|(k, _)| k.clone()))
}

/// A token as the file declared it, its `$type` inherited from its groups.
#[derive(Clone, Debug)]
struct Def {
    ty: Option<String>,
    value: Value,
}

/// Every token of a resolution, by path, in first-declared order; a later
/// declaration replaces the value and keeps the place.
#[derive(Clone, Debug, Default)]
struct Flat {
    order: Vec<String>,
    defs: rustc_hash::FxHashMap<String, Def>,
}

impl Flat {
    fn put(&mut self, path: String, def: Def) {
        if self.defs.insert(path.clone(), def).is_none() {
            self.order.push(path);
        }
    }
}

fn flatten(v: &Value, path: &mut Vec<String>, ty: Option<&str>, out: &mut Flat) {
    let Value::Map(m) = v else { return };
    let ty = m
        .iter()
        .find(|(k, _)| k == "$type")
        .and_then(|(_, t)| t.as_str())
        .or(ty);
    if let Some((_, value)) = m.iter().find(|(k, _)| k == "$value") {
        out.put(
            path.join("."),
            Def {
                ty: ty.map(str::to_owned),
                value: value.clone(),
            },
        );
        return;
    }
    for (k, child) in m {
        if k.starts_with('$') {
            continue;
        }
        path.push(k.clone());
        flatten(child, path, ty, out);
        path.pop();
    }
}

/// The documents a resolver names, read once each.
struct Files {
    base: Option<PathBuf>,
    cache: Vec<(PathBuf, Value)>,
}

impl Files {
    fn new(base: Option<&Path>) -> Self {
        Files {
            base: base.map(Path::to_path_buf),
            cache: Vec::new(),
        }
    }

    fn load(&mut self, rel: &str) -> Result<Value, DtcgError> {
        if rel.contains("://") {
            return Err(fail(format!(
                "`{rel}`: a remote $ref is not read; save the file beside the resolver"
            )));
        }
        let Some(base) = &self.base else {
            return Err(fail(format!(
                "`{rel}`: a file $ref needs the resolver's folder; load it from its path"
            )));
        };
        let path = base.join(rel);
        if let Some((_, v)) = self.cache.iter().find(|(p, _)| *p == path) {
            return Ok(v.clone());
        }
        let src =
            std::fs::read_to_string(&path).map_err(|e| fail(format!("{}: {e}", path.display())))?;
        let v = json::parse(&src).map_err(|e| fail(format!("{}: {e}", path.display())))?;
        self.cache.push((path, v.clone()));
        Ok(v)
    }
}

/// A JSON pointer (`/sets/base`) into `v`.
fn pointer<'a>(v: &'a Value, ptr: &str) -> Option<&'a Value> {
    let mut at = v;
    for raw in ptr.split('/').skip(1) {
        let key = raw.replace("~1", "/").replace("~0", "~");
        at = match at {
            Value::Map(_) => at.get(&key)?,
            Value::List(l) => l.get(key.parse::<usize>().ok()?)?,
            _ => return None,
        };
    }
    Some(at)
}

/// A `$ref` entry's target, with the entry's sibling keys laid over it
/// (shallowly, as the Resolver Module says).
fn deref(doc: &Value, entry: &Value, files: &mut Files, depth: usize) -> Result<Value, DtcgError> {
    if depth > 32 {
        return Err(fail("$ref chain too deep (circular?)"));
    }
    let Some(r) = entry.get("$ref").and_then(Value::as_str) else {
        return Ok(entry.clone());
    };
    let target = match r.split_once('#') {
        Some(("", ptr)) => pointer(doc, ptr)
            .cloned()
            .ok_or_else(|| fail(format!("$ref `{r}` points at nothing")))?,
        Some((file, ptr)) => {
            let d = files.load(file)?;
            pointer(&d, ptr)
                .cloned()
                .ok_or_else(|| fail(format!("$ref `{r}` points at nothing")))?
        }
        None => files.load(r)?,
    };
    let mut target = deref(doc, &target, files, depth + 1)?;
    if let (Value::Map(over), Value::Map(t)) = (entry, &mut target) {
        for (k, v) in over {
            if k == "$ref" {
                continue;
            }
            match t.iter_mut().find(|(tk, _)| tk == k) {
                Some(slot) => slot.1 = v.clone(),
                None => t.push((k.clone(), v.clone())),
            }
        }
    }
    Ok(target)
}

fn add_sources(
    doc: &Value,
    sources: &Value,
    files: &mut Files,
    out: &mut Flat,
) -> Result<(), DtcgError> {
    let Value::List(list) = sources else {
        return Err(fail("`sources` must be a list"));
    };
    for s in list {
        let s = deref(doc, s, files, 0)?;
        flatten(&s, &mut Vec::new(), None, out);
    }
    Ok(())
}

/// One resolution of a resolver document under `inputs` (Resolver Module
/// §5): sets contribute their sources, modifiers their chosen context,
/// later entries over earlier ones.
fn resolve(doc: &Value, inputs: &[(String, String)], files: &mut Files) -> Result<Flat, DtcgError> {
    let Some(Value::List(order)) = doc.get("resolutionOrder") else {
        return Err(fail("`resolutionOrder` must be a list"));
    };
    let mut out = Flat::default();
    for entry in order {
        let (kind, name, item) = match entry.get("$ref").and_then(Value::as_str) {
            Some(r) if r.starts_with("#/sets/") => {
                ("set", r[7..].to_string(), deref(doc, entry, files, 0)?)
            }
            Some(r) if r.starts_with("#/modifiers/") => (
                "modifier",
                r[12..].to_string(),
                deref(doc, entry, files, 0)?,
            ),
            Some(r) => {
                return Err(fail(format!(
                    "resolutionOrder: `{r}` is neither a set nor a modifier"
                )));
            }
            None => {
                let kind = match entry.get_str("type") {
                    Some("set") => "set",
                    Some("modifier") => "modifier",
                    _ => {
                        return Err(fail(
                            "resolutionOrder: an inline entry needs `type` \"set\" or \"modifier\"",
                        ));
                    }
                };
                (
                    kind,
                    entry.get_str("name").unwrap_or("").to_string(),
                    entry.clone(),
                )
            }
        };
        if kind == "set" {
            let sources = item
                .get("sources")
                .ok_or_else(|| fail(format!("set `{name}` has no `sources`")))?;
            add_sources(doc, sources, files, &mut out)?;
            continue;
        }
        let Some(Value::Map(contexts)) = item.get("contexts") else {
            return Err(fail(format!("modifier `{name}` has no `contexts`")));
        };
        let chosen = inputs
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(&name))
            .map(|(_, v)| v.clone())
            .or_else(|| item.get_str("default").map(str::to_owned))
            .ok_or_else(|| {
                fail(format!(
                    "modifier `{name}` has no default; choose one of {:?} in the options' inputs",
                    contexts.iter().map(|(k, _)| k.as_str()).collect::<Vec<_>>()
                ))
            })?;
        let (_, sources) = contexts
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(&chosen))
            .ok_or_else(|| fail(format!("modifier `{name}` has no context `{chosen}`")))?;
        add_sources(doc, sources, files, &mut out)?;
    }
    Ok(out)
}

/// `path`'s value with aliases followed, and the type found on the way.
fn follow(flat: &Flat, path: &str) -> Result<(Value, Option<String>), String> {
    let mut seen: Vec<String> = Vec::new();
    let mut at = path.to_string();
    let mut ty: Option<String> = None;
    loop {
        let def = flat
            .defs
            .get(&at)
            .ok_or_else(|| format!("alias to `{at}`, which is not a token"))?;
        if ty.is_none() {
            ty = def.ty.clone();
        }
        match def
            .value
            .as_str()
            .and_then(|s| s.strip_prefix('{')?.strip_suffix('}'))
        {
            Some(next) => {
                if seen.iter().any(|p| p == next) || next == path {
                    return Err(format!("circular alias through `{next}`"));
                }
                seen.push(at);
                at = next.to_string();
            }
            None => {
                if ty.is_none() {
                    ty = def.ty.clone();
                }
                return Ok((def.value.clone(), ty));
            }
        }
    }
}

/// `path` as a kui name: segments split on anything not a letter or digit,
/// joined in camelCase, behind `prefix`.
fn kui_name(prefix: &str, path: &str) -> String {
    let mut out = String::new();
    for word in path
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
    {
        let mut cs = word.chars();
        let first = cs.next().unwrap_or_default();
        if out.is_empty() && prefix.is_empty() {
            out.extend(first.to_lowercase());
        } else {
            out.extend(first.to_uppercase());
        }
        out.push_str(cs.as_str());
    }
    if prefix.is_empty() {
        out
    } else {
        format!("{prefix}{out}")
    }
}

enum Converted {
    Color(Color),
    Length(f32),
    Family(Vec<String>),
}

fn convert_one(
    v: &Value,
    ty: Option<&str>,
    opts: &DtcgOptions,
    warn: &mut Vec<String>,
) -> Option<Converted> {
    match ty {
        Some("color") => color(v, warn).map(Converted::Color),
        Some("dimension") => dimension(v, opts.rem_px, warn).map(Converted::Length),
        Some("number") => v
            .as_float()
            .map(|n| Converted::Length(n as f32))
            .or_else(|| {
                warn.push("a number token that is not a number".into());
                None
            }),
        Some("fontFamily") => match v {
            Value::Str(s) => Some(Converted::Family(vec![s.clone()])),
            Value::List(l) => Some(Converted::Family(
                l.iter()
                    .filter_map(|s| s.as_str().map(str::to_owned))
                    .collect(),
            )),
            _ => {
                warn.push("a fontFamily that is neither a name nor a list of names".into());
                None
            }
        },
        Some(other) => {
            warn.push(format!("`{other}` has no kui kind yet; skipped"));
            None
        }
        None => {
            warn.push("no `$type` on the token or its groups; skipped".into());
            None
        }
    }
}

fn dimension(v: &Value, rem: f32, warn: &mut Vec<String>) -> Option<f32> {
    let (value, unit) = match v {
        Value::Map(_) => (
            v.get("value").and_then(Value::as_float)?,
            v.get_str("unit").unwrap_or("px").to_string(),
        ),
        Value::Int(_) | Value::Float(_) => (v.as_float()?, "px".to_string()),
        Value::Str(s) => {
            let s = s.trim();
            let split = s.find(|c: char| c.is_alphabetic()).unwrap_or(s.len());
            (
                s[..split].trim().parse::<f64>().ok()?,
                s[split..].to_string(),
            )
        }
        _ => return None,
    };
    match unit.as_str() {
        "px" | "" => Some(value as f32),
        "rem" => Some(value as f32 * rem),
        other => {
            warn.push(format!(
                "dimension unit `{other}` is not px or rem; skipped"
            ));
            None
        }
    }
}

fn color(v: &Value, warn: &mut Vec<String>) -> Option<Color> {
    let (rgb, alpha) = match v {
        Value::Str(s) => {
            return hex(s).or_else(|| {
                warn.push(format!(
                    "colour `{s}` is not a hex string or a colour object"
                ));
                None
            });
        }
        Value::Map(_) => {
            let space = v.get_str("colorSpace").unwrap_or("srgb");
            let comps: Vec<f64> = match v.get("components") {
                Some(Value::List(l)) => l.iter().map(|c| c.as_float().unwrap_or(0.0)).collect(),
                _ => Vec::new(),
            };
            let alpha = v.get("alpha").and_then(Value::as_float).unwrap_or(1.0);
            match (comps.len() == 3)
                .then(|| to_srgb(space, [comps[0], comps[1], comps[2]]))
                .flatten()
            {
                Some(rgb) => (rgb, alpha),
                None => match v.get_str("hex").and_then(hex) {
                    Some(c) => return Some(c.with_alpha(alpha as f32)),
                    None => {
                        warn.push(format!(
                            "colour space `{space}` is not read and the token has no `hex`"
                        ));
                        return None;
                    }
                },
            }
        }
        _ => return None,
    };
    let out = rgb.iter().any(|c| !(-1e-3..=1.0 + 1e-3).contains(c));
    if out {
        warn.push("outside sRGB; clamped".into());
    }
    let c = |x: f64| x.clamp(0.0, 1.0) as f32;
    Some(Color::rgba(c(rgb[0]), c(rgb[1]), c(rgb[2]), c(alpha)))
}

fn hex(s: &str) -> Option<Color> {
    let h = s.strip_prefix('#')?;
    // Hex digits alone, before anything is sliced: a byte index inside a
    // character panicked (`#1̠413`, backlog FZ16), and `from_str_radix`
    // takes a leading `+` (`#+fff`).
    if !h.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    let n = |i: usize, w: usize| u8::from_str_radix(&h[i..i + w], 16).ok();
    match h.len() {
        3 | 4 => {
            let d = |i| n(i, 1).map(|v| v * 17);
            Some(Color::rgba8(
                d(0)?,
                d(1)?,
                d(2)?,
                if h.len() == 4 { d(3)? } else { 255 },
            ))
        }
        6 | 8 => Some(Color::rgba8(
            n(0, 2)?,
            n(2, 2)?,
            n(4, 2)?,
            if h.len() == 8 { n(6, 2)? } else { 255 },
        )),
        _ => None,
    }
}

type M3 = [[f64; 3]; 3];

fn mul(m: &M3, v: [f64; 3]) -> [f64; 3] {
    [
        m[0][0] * v[0] + m[0][1] * v[1] + m[0][2] * v[2],
        m[1][0] * v[0] + m[1][1] * v[1] + m[1][2] * v[2],
        m[2][0] * v[0] + m[2][1] * v[1] + m[2][2] * v[2],
    ]
}

// The CSS Color 4 matrices, D65 unless named.
const XYZ_TO_LIN_SRGB: M3 = [
    [3.2409699419045226, -1.537383177570094, -0.4986107602930034],
    [-0.9692436362808796, 1.8759675015077202, 0.04155505740717559],
    [
        0.05563007969699366,
        -0.20397695888897652,
        1.0569715142428786,
    ],
];
const LIN_P3_TO_XYZ: M3 = [
    [0.4865709486482162, 0.26566769316909306, 0.1982172852343625],
    [0.2289745640697488, 0.6917385218365064, 0.079286914093745],
    [0.0, 0.04511338185890264, 1.043944368900976],
];
const LIN_2020_TO_XYZ: M3 = [
    [0.6369580483012914, 0.14461690358620832, 0.1688809751641721],
    [0.2627002120112671, 0.6779980715188708, 0.05930171646986196],
    [0.0, 0.028072693049087428, 1.060985057710791],
];
const LIN_A98_TO_XYZ: M3 = [
    [0.5766690429101305, 0.1855582379065463, 0.1882286462349947],
    [0.29734497525053605, 0.6273635662554661, 0.0752914584939978],
    [0.02703136138641234, 0.07068885253582723, 0.9913375368376388],
];
const LIN_PROPHOTO_TO_XYZ_D50: M3 = [
    [0.7977666449006423, 0.13518129740053308, 0.0313477341283922],
    [0.2880748288194013, 0.711835234241873, 0.00008993693872564],
    [0.0, 0.0, 0.8251046025104602],
];
const D50_TO_D65: M3 = [
    [0.955473421488075, -0.02309845494876471, 0.06325924320057072],
    [
        -0.0283697093338637,
        1.0099953980813041,
        0.021041441191917323,
    ],
    [
        0.012314014864481998,
        -0.020507649298898964,
        1.330365926242124,
    ],
];

fn srgb_encode(c: f64) -> f64 {
    let a = c.abs();
    let v = if a <= 0.0031308 {
        12.92 * a
    } else {
        1.055 * a.powf(1.0 / 2.4) - 0.055
    };
    v.copysign(c)
}

fn srgb_decode(c: f64) -> f64 {
    let a = c.abs();
    let v = if a <= 0.04045 {
        a / 12.92
    } else {
        ((a + 0.055) / 1.055).powf(2.4)
    };
    v.copysign(c)
}

fn lab_to_xyz_d50(l: f64, a: f64, b: f64) -> [f64; 3] {
    const K: f64 = 24389.0 / 27.0;
    const E: f64 = 216.0 / 24389.0;
    const WHITE: [f64; 3] = [0.3457 / 0.3585, 1.0, (1.0 - 0.3457 - 0.3585) / 0.3585];
    let f1 = (l + 16.0) / 116.0;
    let f0 = a / 500.0 + f1;
    let f2 = f1 - b / 200.0;
    let x = if f0.powi(3) > E {
        f0.powi(3)
    } else {
        (116.0 * f0 - 16.0) / K
    };
    let y = if l > K * E { f1.powi(3) } else { l / K };
    let z = if f2.powi(3) > E {
        f2.powi(3)
    } else {
        (116.0 * f2 - 16.0) / K
    };
    [x * WHITE[0], y * WHITE[1], z * WHITE[2]]
}

fn oklab_to_lin_srgb(l: f64, a: f64, b: f64) -> [f64; 3] {
    let l_ = (l + 0.3963377774 * a + 0.2158037573 * b).powi(3);
    let m_ = (l - 0.1055613458 * a - 0.0638541728 * b).powi(3);
    let s_ = (l - 0.0894841775 * a - 1.2914855480 * b).powi(3);
    [
        4.0767416621 * l_ - 3.3077115913 * m_ + 0.2309699292 * s_,
        -1.2684380046 * l_ + 2.6097574011 * m_ - 0.3413193965 * s_,
        -0.0041960863 * l_ - 0.7034186147 * m_ + 1.7076147010 * s_,
    ]
}

fn hsl(h: f64, s: f64, l: f64) -> [f64; 3] {
    let a = s * l.min(1.0 - l);
    let f = |n: f64| {
        let k = (n + h / 30.0).rem_euclid(12.0);
        l - a * (k - 3.0).min(9.0 - k).clamp(-1.0, 1.0)
    };
    [f(0.0), f(8.0), f(4.0)]
}

/// Gamma-encoded sRGB in `0..1` (possibly outside it) for a DTCG colour
/// space and its three components, as the 2025.10 Color module ranges
/// them; `None` for a space this does not read.
fn to_srgb(space: &str, c: [f64; 3]) -> Option<[f64; 3]> {
    let lin_to = |lin: [f64; 3]| lin.map(srgb_encode);
    let xyz65 = |xyz: [f64; 3]| lin_to(mul(&XYZ_TO_LIN_SRGB, xyz));
    let polar = |l: f64, ch: f64, h: f64| {
        let r = h.to_radians();
        (l, ch * r.cos(), ch * r.sin())
    };
    Some(match space {
        "srgb" => c,
        "srgb-linear" => lin_to(c),
        "display-p3" => xyz65(mul(&LIN_P3_TO_XYZ, c.map(srgb_decode))),
        "rec2020" => {
            const A: f64 = 1.09929682680944;
            const B: f64 = 0.018053968510807;
            let dec = |v: f64| {
                let a = v.abs();
                (if a < B * 4.5 {
                    a / 4.5
                } else {
                    ((a + A - 1.0) / A).powf(1.0 / 0.45)
                })
                .copysign(v)
            };
            xyz65(mul(&LIN_2020_TO_XYZ, c.map(dec)))
        }
        "a98-rgb" => xyz65(mul(
            &LIN_A98_TO_XYZ,
            c.map(|v: f64| v.abs().powf(563.0 / 256.0).copysign(v)),
        )),
        "prophoto-rgb" => {
            let dec = |v: f64| {
                let a = v.abs();
                (if a <= 16.0 / 512.0 {
                    a / 16.0
                } else {
                    a.powf(1.8)
                })
                .copysign(v)
            };
            xyz65(mul(&D50_TO_D65, mul(&LIN_PROPHOTO_TO_XYZ_D50, c.map(dec))))
        }
        "xyz-d65" | "xyz" => xyz65(c),
        "xyz-d50" => xyz65(mul(&D50_TO_D65, c)),
        "lab" => xyz65(mul(&D50_TO_D65, lab_to_xyz_d50(c[0], c[1], c[2]))),
        "lch" => {
            let (l, a, b) = polar(c[0], c[1], c[2]);
            xyz65(mul(&D50_TO_D65, lab_to_xyz_d50(l, a, b)))
        }
        "oklab" => lin_to(oklab_to_lin_srgb(c[0], c[1], c[2])),
        "oklch" => {
            let (l, a, b) = polar(c[0], c[1], c[2]);
            lin_to(oklab_to_lin_srgb(l, a, b))
        }
        "hsl" => hsl(c[0], c[1] / 100.0, c[2] / 100.0),
        "hwb" => {
            let (w, b) = (c[1] / 100.0, c[2] / 100.0);
            if w + b >= 1.0 {
                let g = w / (w + b);
                [g, g, g]
            } else {
                hsl(c[0], 1.0, 0.5).map(|v| v * (1.0 - w - b) + w)
            }
        }
        _ => return None,
    })
}

fn convert(light: &Flat, dark: &Flat, opts: &DtcgOptions, warnings: &mut Vec<DtcgWarning>) -> Dtcg {
    let mut tokens = Tokens::new();
    let mut names: Vec<DtcgName> = Vec::new();
    let mut warns: Vec<DtcgWarning> = Vec::new();
    let mut order: Vec<&String> = light.order.iter().collect();
    for p in &dark.order {
        if !light.defs.contains_key(p) {
            order.push(p);
        }
    }
    for path in order {
        let mut w: Vec<String> = Vec::new();
        let one = |flat: &Flat, w: &mut Vec<String>| -> Option<Converted> {
            if !flat.defs.contains_key(path) {
                return None;
            }
            match follow(flat, path) {
                Ok((v, ty)) => convert_one(&v, ty.as_deref(), opts, w),
                Err(e) => {
                    w.push(e);
                    None
                }
            }
        };
        let l = one(light, &mut w);
        let mut dw = Vec::new();
        let d = if std::ptr::eq(light, dark) {
            None
        } else {
            one(dark, &mut dw)
        };
        for m in w.into_iter().chain(dw) {
            if !warns.iter().any(|x| x.path == *path && x.message == m) {
                warns.push(DtcgWarning {
                    path: path.clone(),
                    message: m,
                });
            }
        }
        let name = kui_name(&opts.prefix, path);
        if name.is_empty() {
            continue;
        }
        if let Some(prev) = names.iter().find(|n| n.name == name) {
            warns.push(DtcgWarning {
                path: path.clone(),
                message: format!("its name `{name}` is `{}`'s already; skipped", prev.path),
            });
            continue;
        }
        let value = match (l, d) {
            (Some(Converted::Color(a)), Some(Converted::Color(b))) => {
                DtcgValue::Color { light: a, dark: b }
            }
            (Some(Converted::Color(a)), _) | (None, Some(Converted::Color(a))) => {
                DtcgValue::Color { light: a, dark: a }
            }
            (Some(Converted::Length(px)), _) | (None, Some(Converted::Length(px))) => {
                DtcgValue::Length(px)
            }
            (Some(Converted::Family(list)), _) | (None, Some(Converted::Family(list))) => {
                DtcgValue::Family(list)
            }
            _ => continue,
        };
        tokens = match &value {
            DtcgValue::Color { light, dark } if light == dark => tokens.color(&name, *light),
            DtcgValue::Color { light, dark } => tokens.color_themed(&name, *light, *dark),
            DtcgValue::Length(px) => tokens.length(&name, *px),
            DtcgValue::Family(list) => tokens.family(&name, list.clone()),
        };
        names.push(DtcgName {
            name,
            path: path.clone(),
            value,
        });
    }
    warnings.extend(warns);
    Dtcg {
        tokens,
        names,
        warnings: Vec::new(),
        resolver: false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("kui-dtcg-{tag}-{}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    #[test]
    fn names_are_camel_case_behind_the_prefix() {
        assert_eq!(
            kui_name("", "background.elevation-1"),
            "backgroundElevation1"
        );
        assert_eq!(
            kui_name("brand", "background.elevation-1"),
            "brandBackgroundElevation1"
        );
        assert_eq!(kui_name("", "Text.secondary_muted"), "textSecondaryMuted");
    }

    /// A character past ASCII in a hex colour is not one, and is no
    /// panic: the parser sliced it by byte (backlog FZ16, from `dtcg`).
    #[test]
    fn a_hex_colour_is_hex_digits_alone() {
        assert_eq!(hex("#1\u{320}413"), None);
        assert_eq!(hex("#1\u{19f}413"), None);
        assert_eq!(hex("#é00"), None);
        assert_eq!(hex("#+fff"), None);
        assert_eq!(hex("#ff00ff"), Some(Color::rgba8(255, 0, 255, 255)));
        assert_eq!(hex("#0f08"), Some(Color::rgba8(0, 255, 0, 136)));
    }

    #[test]
    fn colour_spaces_land_on_srgb() {
        let near = |a: [f64; 3], b: [f64; 3]| a.iter().zip(b).all(|(x, y)| (x - y).abs() < 2e-3);
        assert!(near(
            to_srgb("srgb-linear", [0.2140, 0.2140, 0.2140]).unwrap(),
            [0.5, 0.5, 0.5]
        ));
        assert!(near(
            to_srgb("display-p3", [1.0, 1.0, 1.0]).unwrap(),
            [1.0, 1.0, 1.0]
        ));
        assert!(near(
            to_srgb("oklch", [1.0, 0.0, 0.0]).unwrap(),
            [1.0, 1.0, 1.0]
        ));
        assert!(near(
            to_srgb("lab", [100.0, 0.0, 0.0]).unwrap(),
            [1.0, 1.0, 1.0]
        ));
        assert!(near(
            to_srgb("hsl", [0.0, 100.0, 50.0]).unwrap(),
            [1.0, 0.0, 0.0]
        ));
        assert!(near(
            to_srgb("hwb", [120.0, 0.0, 0.0]).unwrap(),
            [0.0, 1.0, 0.0]
        ));
        // An sRGB red as oklab, from the CSS Color 4 sample values.
        assert!(near(
            to_srgb("oklab", [0.62796, 0.22486, 0.12585]).unwrap(),
            [1.0, 0.0, 0.0]
        ));
        // Display-p3 green is outside sRGB.
        let g = to_srgb("display-p3", [0.0, 1.0, 0.0]).unwrap();
        assert!(g[0] < 0.0);
    }

    #[test]
    fn a_token_file_loads_with_aliases_legacy_spellings_and_warnings() {
        let src = r##"{
          "color": { "$type": "color",
            "base": { "$value": { "colorSpace": "srgb", "components": [1, 0, 0], "alpha": 0.5 } },
            "legacy": { "$value": "#00ff00" },
            "alias": { "$value": "{color.base}" },
            "p3": { "$value": { "colorSpace": "display-p3", "components": [0, 1, 0] } }
          },
          "size": { "small": { "$type": "dimension", "$value": "0.5rem" }, "line": { "$type": "number", "$value": 1.4 } },
          "shadow": { "card": { "$type": "shadow", "$value": {} } },
          "loop": { "a": { "$type": "color", "$value": "{loop.b}" }, "b": { "$type": "color", "$value": "{loop.a}" } }
        }"##;
        let d = from_str(src, None, &DtcgOptions::default()).unwrap();
        assert!(!d.resolver);
        let names: Vec<_> = d.names.iter().map(|n| n.name.as_str()).collect();
        assert_eq!(
            names,
            [
                "colorBase",
                "colorLegacy",
                "colorAlias",
                "colorP3",
                "sizeSmall",
                "sizeLine"
            ]
        );
        let base = Color::rgba(1.0, 0.0, 0.0, 0.5);
        assert_eq!(
            d.names[0].value,
            DtcgValue::Color {
                light: base,
                dark: base
            }
        );
        assert_eq!(
            d.names[2].value, d.names[0].value,
            "the alias is its target"
        );
        assert_eq!(d.names[4].value, DtcgValue::Length(8.0));
        let w: Vec<_> = d.warnings.iter().map(|w| w.to_string()).collect();
        assert!(
            w.iter().any(|w| w.starts_with("color.p3: outside sRGB")),
            "{w:?}"
        );
        assert!(
            w.iter()
                .any(|w| w.starts_with("shadow.card: `shadow` has no kui kind")),
            "{w:?}"
        );
        assert!(
            w.iter().any(|w| w.starts_with("loop.a: circular alias")),
            "{w:?}"
        );
    }

    #[test]
    fn a_resolver_gives_each_colour_two_halves_from_its_files() {
        let dir = temp("resolver");
        std::fs::write(
            dir.join("base.json"),
            r##"{ "space": { "gap": { "$type": "dimension", "$value": { "value": 8, "unit": "px" } } } }"##,
        )
        .unwrap();
        std::fs::write(
            dir.join("light.json"),
            r##"{ "bg": { "$type": "color", "$value": "#ffffff" } }"##,
        )
        .unwrap();
        std::fs::write(
            dir.join("dark.json"),
            r##"{ "bg": { "$type": "color", "$value": "#101010" } }"##,
        )
        .unwrap();
        std::fs::write(
            dir.join("brand.resolver.json"),
            r##"{
              "version": "2025-10-01",
              "sets": { "base": { "sources": [{ "$ref": "base.json" }] } },
              "modifiers": {
                "mode": { "contexts": { "light": [{ "$ref": "light.json" }], "dark": [{ "$ref": "dark.json" }] }, "default": "light" },
                "density": { "contexts": { "comfy": [], "tight": [{ "space": { "gap": { "$type": "dimension", "$value": "4px" } } }] }, "default": "comfy" }
              },
              "resolutionOrder": [{ "$ref": "#/sets/base" }, { "$ref": "#/modifiers/mode" }, { "$ref": "#/modifiers/density" }]
            }"##,
        )
        .unwrap();
        // `mode`, not `theme`: found by its contexts.
        let d = from_path(dir.join("brand.resolver.json"), &DtcgOptions::default()).unwrap();
        assert!(d.resolver && d.warnings.is_empty(), "{:?}", d.warnings);
        let get = |d: &Dtcg, n: &str| {
            d.names
                .iter()
                .find(|x| x.name == n)
                .map(|x| x.value.clone())
        };
        assert_eq!(
            get(&d, "bg"),
            Some(DtcgValue::Color {
                light: Color::hex(0xffffffff),
                dark: Color::hex(0x101010ff)
            })
        );
        assert_eq!(get(&d, "spaceGap"), Some(DtcgValue::Length(8.0)));
        let tight = DtcgOptions {
            inputs: vec![("density".into(), "tight".into())],
            ..DtcgOptions::default()
        };
        let d = from_path(dir.join("brand.resolver.json"), &tight).unwrap();
        assert_eq!(get(&d, "spaceGap"), Some(DtcgValue::Length(4.0)));
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn a_resolver_without_its_folder_says_how_to_load_it() {
        let src = r#"{ "resolutionOrder": [{ "type": "set", "name": "x", "sources": [{ "$ref": "a.json" }] }] }"#;
        let e = from_str(src, None, &DtcgOptions::default()).unwrap_err();
        assert!(e.message.contains("load it from its path"), "{e}");
    }
}
