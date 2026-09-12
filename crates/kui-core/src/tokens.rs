//! Tokens: named colours and lengths an app declares beside the theme and
//! the metrics, and references a colour or length prop by name
//! (`docs/adr/0027-tokens-beside-the-theme.md`).
//!
//! A [`Theme`] is twenty-three roles the stock widgets paint from; a
//! [`Metrics`] is sixteen lengths they are built from. Both are closed,
//! and an app whose palette *is* the design — LCARS peach, tangerine, a
//! sidebar width — has a vocabulary neither names. [`Tokens`] is that
//! vocabulary: a colour token carries a light and a dark half (the same
//! value twice, in the common case) **or a recipe over an earlier token**
//! (`docs/adr/0028-derived-tokens.md`: a source and a chain of
//! [`ColorOp`]s, folded on read), a length token is one number in
//! logical px, and the app declares them once, whole. Where a role is
//! read as `ui.theme().surface`, a token is referenced by name in the
//! prop — `bg = "$peach"` — and the binding that lowers the node resolves
//! it through [`TokenLookup`] before the core sees a `NodeSpec`, so the
//! core's open path knows nothing about names.
//!
//! One table per origin. A host's names are the host's and an
//! extension's are its own: a lookup reads the table of the origin whose
//! view is running, then the host's, so a guest can paint in the host's
//! vocabulary without the host passing it and still name a grey of its
//! own, and a plugin's declaration never replaces the host's palette.
//! The theme's roles and the metrics' roles are reachable by the same
//! spelling — `$surface`, `$radius` — through a reserved range in front of
//! the app's, so a declared token that takes a role's name is refused
//! ([`crate::diag::RESERVED_TOKEN`]) rather than shadowing it.

use rustc_hash::FxHashMap;

use crate::color::Color;
use crate::env::Appearance;
use crate::metrics::Metrics;
use crate::schema::{METRIC_ROLES, THEME_ROLES};
use crate::theme::Theme;

/// A colour token: one value per base, or a recipe over an earlier token
/// or a theme role (ADR 0028). [`ColorToken::same`] is the unthemed case,
/// and what a declaration with one colour builds; a derived one is built
/// by [`Tokens::derive`], since its source is an index into the table
/// that holds it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ColorToken {
    Value {
        light: Color,
        dark: Color,
    },
    /// `from`, with the ops at `ops` in [`Tokens`]' chain applied in
    /// order. `from` is a colour token declared before this one in the
    /// same table, or a theme role — never a later token, so the chain is
    /// acyclic by construction and a read recurses at most the table's
    /// length.
    Derived {
        from: TokenRef,
        ops: OpRange,
    },
}

impl ColorToken {
    pub fn same(c: Color) -> Self {
        Self::Value { light: c, dark: c }
    }

    pub fn themed(light: Color, dark: Color) -> Self {
        Self::Value { light, dark }
    }

    /// The declared halves of a value token; `None` for a derived one,
    /// whose halves are whatever its recipe makes of its source's
    /// ([`Tokens::halves`]).
    pub fn halves(&self) -> Option<(Color, Color)> {
        match *self {
            Self::Value { light, dark } => Some((light, dark)),
            Self::Derived { .. } => None,
        }
    }

    pub fn is_derived(&self) -> bool {
        matches!(self, Self::Derived { .. })
    }
}

/// One step of a derived token's recipe, as declared: a verb and its
/// operands, a colour named by its token or role. Each is a method the
/// core already paints with — `lift` and `darken` are [`Color::mix`]
/// toward white and black, `raise` is [`Theme::raise`] (toward the front
/// of whichever base is in effect), `alpha` is [`Color::with_alpha`],
/// `mix` is [`Color::mix`] toward another token, and `readable` is
/// [`Color::toward_contrast`] toward black or white — whichever reads on
/// the named colour — until it clears the ratio on it.
#[derive(Clone, Debug, PartialEq)]
pub enum ColorOp {
    Lift(f32),
    Darken(f32),
    Raise(f32),
    Alpha(f32),
    Mix(String, f32),
    Readable(String, f32),
}

impl ColorOp {
    /// The verbs, in the order C numbers them (`KuiColorOp.op`).
    pub const VERBS: [&'static str; 6] = ["lift", "darken", "raise", "alpha", "mix", "readable"];

    /// The verb's spelling, as every binding writes it in a tuple's first
    /// slot and as the devtools print it.
    pub fn verb(&self) -> &'static str {
        match self {
            Self::Lift(_) => "lift",
            Self::Darken(_) => "darken",
            Self::Raise(_) => "raise",
            Self::Alpha(_) => "alpha",
            Self::Mix(..) => "mix",
            Self::Readable(..) => "readable",
        }
    }

    /// Whether `verb` takes a colour operand before its number (`mix`,
    /// `readable`); `None` for a verb that is none of the six.
    pub fn takes_color(verb: &str) -> Option<bool> {
        match verb {
            "lift" | "darken" | "raise" | "alpha" => Some(false),
            "mix" | "readable" => Some(true),
            _ => None,
        }
    }

    /// A step from its spelling: the verb, the colour operand a verb that
    /// takes one names, and the number. `None` for an unknown verb or a
    /// colour given to a verb that takes none (and the reverse).
    pub fn parse(verb: &str, color: Option<&str>, t: f32) -> Option<Self> {
        Some(match (verb, color) {
            ("lift", None) => Self::Lift(t),
            ("darken", None) => Self::Darken(t),
            ("raise", None) => Self::Raise(t),
            ("alpha", None) => Self::Alpha(t),
            ("mix", Some(c)) => Self::Mix(c.to_string(), t),
            ("readable", Some(c)) => Self::Readable(c.to_string(), t),
            _ => return None,
        })
    }
}

/// A step with its operand resolved to the table's index or a role: what
/// the chain stores, so a read follows indices and never a name.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Step {
    Lift(f32),
    Darken(f32),
    Raise(f32),
    Alpha(f32),
    Mix(TokenRef, f32),
    Readable(TokenRef, f32),
}

/// Where a derived token's steps are in [`Tokens`]' chain: the table owns
/// the ops, so the token stays `Copy`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OpRange {
    start: u16,
    len: u16,
}

/// Why a derived token was dropped at declaration: the name it was given
/// and the source or operand that resolved to no colour token declared
/// before it and no theme role.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Unresolved {
    pub token: String,
    pub source: String,
}

/// What kind of value a token holds, and which prop slots it fits.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TokenKind {
    Color,
    Length,
}

impl TokenKind {
    pub fn name(self) -> &'static str {
        match self {
            TokenKind::Color => "color",
            TokenKind::Length => "length",
        }
    }
}

/// Where a name resolved to: a role, or an app token by index.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TokenRef {
    ColorRole(u16),
    LengthRole(u16),
    Color(u16),
    Length(u16),
}

impl TokenRef {
    pub fn kind(self) -> TokenKind {
        match self {
            TokenRef::ColorRole(_) | TokenRef::Color(_) => TokenKind::Color,
            TokenRef::LengthRole(_) | TokenRef::Length(_) => TokenKind::Length,
        }
    }

    /// The index a binding writes on the wire for this reference: roles
    /// first, the app's after them, one space per kind
    /// ([`COLOR_ROLES`] / [`LENGTH_ROLES`] wide).
    pub fn index(self) -> u16 {
        match self {
            TokenRef::ColorRole(i) | TokenRef::LengthRole(i) => i,
            TokenRef::Color(i) => COLOR_ROLES + i,
            TokenRef::Length(i) => LENGTH_ROLES + i,
        }
    }
}

/// How many indices the theme's roles take in front of the app's colour
/// tokens: `$surface` is index 1 whatever the app declared.
pub const COLOR_ROLES: u16 = THEME_ROLES.len() as u16;
/// The same for the metrics' roles in front of the app's lengths.
pub const LENGTH_ROLES: u16 = METRIC_ROLES.len() as u16;

/// Why a name did not resolve.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TokenError {
    /// Nothing declared it, and it is no role.
    Unknown(String),
    /// It exists, as the other kind: a length in a colour slot or the
    /// reverse.
    Kind {
        name: String,
        is: TokenKind,
        wanted: TokenKind,
    },
}

impl std::fmt::Display for TokenError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TokenError::Unknown(name) => write!(
                f,
                "`${name}` names no token: nothing declared it and it is not a theme or metrics \
                 role, so this declaration paints nothing"
            ),
            TokenError::Kind { name, is, wanted } => write!(
                f,
                "`${name}` is a {} token, and this slot takes a {}",
                is.name(),
                wanted.name()
            ),
        }
    }
}

/// One origin's declared tokens, in declaration order. Built with the
/// chaining constructors and handed to [`crate::Core::set_tokens`] whole;
/// each call replaces the caller's table.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Tokens {
    colors: Vec<(String, ColorToken)>,
    lengths: Vec<(String, f32)>,
    by_name: FxHashMap<String, TokenRef>,
    /// Every derived token's steps, end to end; a token holds its range.
    ops: Vec<Step>,
    /// Names refused because a role owns them, kept so the core can warn
    /// once per name when the table is declared.
    reserved: Vec<String>,
    /// Derived tokens dropped because a source did not resolve, kept for
    /// the same reason.
    unresolved: Vec<Unresolved>,
}

impl Tokens {
    pub fn new() -> Self {
        Self::default()
    }

    /// A colour with one value for both bases.
    pub fn color(self, name: impl Into<String>, c: Color) -> Self {
        self.color_token(name, ColorToken::same(c))
    }

    /// A colour with a value per base.
    pub fn color_themed(self, name: impl Into<String>, light: Color, dark: Color) -> Self {
        self.color_token(name, ColorToken::themed(light, dark))
    }

    pub fn color_token(mut self, name: impl Into<String>, token: ColorToken) -> Self {
        let name = name.into();
        if is_role(&name) {
            self.reserved.push(name);
            return self;
        }
        match self.by_name.get(&name) {
            Some(TokenRef::Color(i)) => self.colors[*i as usize].1 = token,
            _ => {
                let i = self.colors.len() as u16;
                self.colors.push((name.clone(), token));
                self.by_name.insert(name, TokenRef::Color(i));
            }
        }
        self
    }

    /// A colour computed from another (ADR 0028): `from` is a colour
    /// token already in this table or a theme role, and `ops` the steps
    /// applied to it in order, each colour operand likewise a name
    /// declared before this one. A source that resolves to nothing —
    /// undeclared, a length, this token itself, or one declared after it
    /// — drops the declaration and is reported by [`Tokens::unresolved`];
    /// the core raises `unknown-token` for each when the table is set. An
    /// empty chain is an alias.
    pub fn derive(
        mut self,
        name: impl Into<String>,
        from: &str,
        ops: impl IntoIterator<Item = ColorOp>,
    ) -> Self {
        let name = name.into();
        if is_role(&name) {
            self.reserved.push(name);
            return self;
        }
        // The index this token will have — its own if it is being
        // re-declared, else the next — and every source sits below it.
        let own = match self.by_name.get(&name) {
            Some(TokenRef::Color(i)) => *i,
            _ => self.colors.len() as u16,
        };
        let source = |t: &Self, s: &str| -> Option<TokenRef> {
            match role_ref(s) {
                Some(r @ TokenRef::ColorRole(_)) => Some(r),
                Some(_) => None,
                None => match t.color_id(s) {
                    Some(r @ TokenRef::Color(i)) if i < own => Some(r),
                    _ => None,
                },
            }
        };
        let unresolved = |s: &str| Unresolved {
            token: name.clone(),
            source: s.to_string(),
        };
        let Some(from) = source(&self, from) else {
            self.unresolved.push(unresolved(from));
            return self;
        };
        let mut steps = Vec::new();
        for op in ops {
            let step = match op {
                ColorOp::Lift(t) => Step::Lift(t),
                ColorOp::Darken(t) => Step::Darken(t),
                ColorOp::Raise(t) => Step::Raise(t),
                ColorOp::Alpha(a) => Step::Alpha(a),
                ColorOp::Mix(ref other, t) => match source(&self, other) {
                    Some(r) => Step::Mix(r, t),
                    None => {
                        self.unresolved.push(unresolved(other));
                        return self;
                    }
                },
                ColorOp::Readable(ref on, ratio) => match source(&self, on) {
                    Some(r) => Step::Readable(r, ratio),
                    None => {
                        self.unresolved.push(unresolved(on));
                        return self;
                    }
                },
            };
            steps.push(step);
        }
        let range = OpRange {
            start: self.ops.len() as u16,
            len: steps.len() as u16,
        };
        self.ops.extend(steps);
        self.color_token(name, ColorToken::Derived { from, ops: range })
    }

    /// A length in logical px, before `env.scale`.
    pub fn length(mut self, name: impl Into<String>, px: f32) -> Self {
        let name = name.into();
        if is_role(&name) {
            self.reserved.push(name);
            return self;
        }
        match self.by_name.get(&name) {
            Some(TokenRef::Length(i)) => self.lengths[*i as usize].1 = px,
            _ => {
                let i = self.lengths.len() as u16;
                self.lengths.push((name.clone(), px));
                self.by_name.insert(name, TokenRef::Length(i));
            }
        }
        self
    }

    /// The reference a declared name resolves to in this table alone —
    /// what a Rust app holds instead of the name.
    pub fn id(&self, name: &str) -> Option<TokenRef> {
        self.by_name.get(name).copied()
    }

    pub fn color_id(&self, name: &str) -> Option<TokenRef> {
        self.id(name).filter(|r| r.kind() == TokenKind::Color)
    }

    pub fn length_id(&self, name: &str) -> Option<TokenRef> {
        self.id(name).filter(|r| r.kind() == TokenKind::Length)
    }

    /// The colours, in declaration order.
    pub fn colors(&self) -> &[(String, ColorToken)] {
        &self.colors
    }

    /// The lengths, in declaration order.
    pub fn lengths(&self) -> &[(String, f32)] {
        &self.lengths
    }

    /// The names a role owns that this declaration tried to take.
    pub fn reserved(&self) -> &[String] {
        &self.reserved
    }

    /// The derived tokens this declaration dropped, with the source that
    /// did not resolve.
    pub fn unresolved(&self) -> &[Unresolved] {
        &self.unresolved
    }

    pub fn is_empty(&self) -> bool {
        self.colors.is_empty() && self.lengths.is_empty()
    }

    /// The colour token at `i` under `theme`: a value's half, or a
    /// derived token's recipe folded over its source's — recursing
    /// through a derived source, which is always an earlier index. A
    /// derived colour comes back rounded to eight bits a channel, so it is
    /// exactly what a reader gets through `0xRRGGBBAA`.
    pub fn resolve_color(&self, i: u16, theme: &Theme) -> Color {
        match self.colors[i as usize].1 {
            ColorToken::Value { light, dark } => {
                if theme.is_dark() {
                    dark
                } else {
                    light
                }
            }
            ColorToken::Derived { from, ops } => {
                let mut c = self.source_color(from, theme);
                for step in &self.ops[ops.start as usize..(ops.start + ops.len) as usize] {
                    c = match *step {
                        Step::Lift(t) => c.mix(Color::WHITE, t),
                        Step::Darken(t) => c.mix(Color::BLACK, t),
                        Step::Raise(t) => theme.raise(c, t),
                        Step::Alpha(a) => c.with_alpha(a),
                        Step::Mix(other, t) => c.mix(self.source_color(other, theme), t),
                        Step::Readable(on, ratio) => {
                            let on = self.source_color(on, theme);
                            c.toward_contrast(crate::widgets::readable_on(on), on, ratio, 0.0)
                        }
                    };
                }
                // Rounded to eight bits a channel: the value every binding
                // paints is the one `to_hex` reads back, so a C host that
                // reads a derived colour and writes it (it has no reference)
                // lowers the same quad as a `$name` does.
                Color::hex(c.to_hex())
            }
        }
    }

    fn source_color(&self, r: TokenRef, theme: &Theme) -> Color {
        match r {
            TokenRef::ColorRole(i) => (THEME_ROLES[i as usize].get)(theme),
            TokenRef::Color(i) => self.resolve_color(i, theme),
            TokenRef::LengthRole(_) | TokenRef::Length(_) => unreachable!("a colour source"),
        }
    }

    /// Both halves of the colour token at `i`, the light first: a value's
    /// as declared; a derived token's computed under `theme` for the half
    /// in effect and under a theme of the other appearance with the same
    /// accent for the other — what the devtools show beside the swatch.
    pub fn halves(&self, i: u16, theme: &Theme) -> (Color, Color) {
        if let Some(h) = self.colors[i as usize].1.halves() {
            return h;
        }
        let other = Theme::derive(
            if theme.is_dark() {
                Appearance::Light
            } else {
                Appearance::Dark
            },
            Some(theme.accent),
        );
        let here = self.resolve_color(i, theme);
        let there = self.resolve_color(i, &other);
        if theme.is_dark() {
            (there, here)
        } else {
            (here, there)
        }
    }

    /// A derived token's recipe as the devtools print it — `peach → lift
    /// 0.3 → alpha 0.5` — or `None` for a value.
    pub fn recipe(&self, i: u16) -> Option<String> {
        let ColorToken::Derived { from, ops } = self.colors[i as usize].1 else {
            return None;
        };
        let name = |r: TokenRef| -> &str {
            match r {
                TokenRef::ColorRole(i) => THEME_ROLES[i as usize].node,
                TokenRef::Color(i) => &self.colors[i as usize].0,
                _ => unreachable!("a colour source"),
            }
        };
        let mut out = name(from).to_string();
        for step in &self.ops[ops.start as usize..(ops.start + ops.len) as usize] {
            out.push_str(" → ");
            out.push_str(&match *step {
                Step::Lift(t) => format!("lift {t}"),
                Step::Darken(t) => format!("darken {t}"),
                Step::Raise(t) => format!("raise {t}"),
                Step::Alpha(a) => format!("alpha {a}"),
                Step::Mix(r, t) => format!("mix {} {t}", name(r)),
                Step::Readable(r, ratio) => format!("readable {} {ratio}", name(r)),
            });
        }
        Some(out)
    }
}

/// Whether a name is a theme or metrics role under either spelling
/// (`border_strong` and `borderStrong` both are).
pub fn is_role(name: &str) -> bool {
    role_ref(name).is_some()
}

/// The role a name is, under either spelling.
pub fn role_ref(name: &str) -> Option<TokenRef> {
    if let Some(i) = THEME_ROLES
        .iter()
        .position(|r| r.name == name || r.node == name)
    {
        return Some(TokenRef::ColorRole(i as u16));
    }
    METRIC_ROLES
        .iter()
        .position(|r| r.name == name || r.node == name)
        .map(|i| TokenRef::LengthRole(i as u16))
}

/// A frame's view of the tokens a lowering can reference: the running
/// origin's table over the host's, and the roles in front of both.
/// Borrowed from the core for the length of a lowering
/// ([`crate::Core::token_lookup`]).
#[derive(Clone, Copy)]
pub struct TokenLookup<'a> {
    pub(crate) own: Option<&'a Tokens>,
    pub(crate) host: Option<&'a Tokens>,
    pub(crate) theme: &'a Theme,
    pub(crate) metrics: &'a Metrics,
}

impl<'a> TokenLookup<'a> {
    /// The table a by-index reference reads: the running origin's, or
    /// the host's when it declared none.
    fn table(&self) -> Option<&'a Tokens> {
        self.own.or(self.host)
    }

    /// What a name resolves to: a role under either spelling, then the
    /// running origin's table, then the host's.
    pub fn resolve(&self, name: &str) -> Option<TokenRef> {
        if let Some(r) = role_ref(name) {
            return Some(r);
        }
        if let Some(r) = self.own.and_then(|t| t.id(name)) {
            return Some(r);
        }
        self.host.and_then(|t| t.id(name))
    }

    /// The colour a name resolves to this frame.
    pub fn color(&self, name: &str) -> Result<Color, TokenError> {
        match self.resolve(name) {
            None => Err(TokenError::Unknown(name.to_string())),
            Some(r) if r.kind() != TokenKind::Color => Err(TokenError::Kind {
                name: name.to_string(),
                is: r.kind(),
                wanted: TokenKind::Color,
            }),
            Some(TokenRef::ColorRole(i)) => Ok((THEME_ROLES[i as usize].get)(self.theme)),
            Some(TokenRef::Color(i)) => Ok(self.color_in(name, i)),
            Some(_) => unreachable!(),
        }
    }

    /// The length a name resolves to this frame.
    pub fn length(&self, name: &str) -> Result<f32, TokenError> {
        match self.resolve(name) {
            None => Err(TokenError::Unknown(name.to_string())),
            Some(r) if r.kind() != TokenKind::Length => Err(TokenError::Kind {
                name: name.to_string(),
                is: r.kind(),
                wanted: TokenKind::Length,
            }),
            Some(TokenRef::LengthRole(i)) => Ok((METRIC_ROLES[i as usize].get)(self.metrics)),
            Some(TokenRef::Length(i)) => Ok(self.length_in(name, i)),
            Some(_) => unreachable!(),
        }
    }

    // A by-name hit came from whichever table holds the name; the index
    // is only meaningful in that table, so it is re-read by name.
    fn color_in(&self, name: &str, i: u16) -> Color {
        let own = self.own.filter(|t| t.id(name) == Some(TokenRef::Color(i)));
        let t = own.or(self.host).expect("resolved in a table");
        t.resolve_color(i, self.theme)
    }

    fn length_in(&self, name: &str, i: u16) -> f32 {
        let own = self.own.filter(|t| t.id(name) == Some(TokenRef::Length(i)));
        let t = own.or(self.host).expect("resolved in a table");
        t.lengths[i as usize].1
    }

    /// A colour by wire index ([`TokenRef::index`]): a role below
    /// [`COLOR_ROLES`], the running origin's table above. `None` past the
    /// end — a reference to a token the table no longer holds.
    pub fn color_at(&self, index: u32) -> Option<Color> {
        if index < COLOR_ROLES as u32 {
            return Some((THEME_ROLES[index as usize].get)(self.theme));
        }
        let i = index - COLOR_ROLES as u32;
        self.table()
            .filter(|t| (i as usize) < t.colors.len())
            .map(|t| t.resolve_color(i as u16, self.theme))
    }

    /// A length by wire index, the same way over [`LENGTH_ROLES`].
    pub fn length_at(&self, index: u32) -> Option<f32> {
        if index < LENGTH_ROLES as u32 {
            return Some((METRIC_ROLES[index as usize].get)(self.metrics));
        }
        let i = (index - LENGTH_ROLES as u32) as usize;
        self.table().and_then(|t| t.lengths.get(i)).map(|(_, v)| *v)
    }

    /// Every colour token a reader in this origin sees, resolved for the
    /// frame: the running origin's over the host's, by name, in the host's
    /// order then the guest's additions. Roles are not listed — they are
    /// the theme's reading.
    pub fn colors(&self) -> Vec<(&'a str, Color)> {
        let mut out: Vec<(&'a str, Color)> = Vec::new();
        for t in [self.host, self.own].into_iter().flatten() {
            for (i, (name, _)) in t.colors.iter().enumerate() {
                // A name later declared as a length leaves its colour
                // entry behind; the name binds the length, so the
                // listing does too.
                if t.id(name) != Some(TokenRef::Color(i as u16)) {
                    continue;
                }
                let c = t.resolve_color(i as u16, self.theme);
                match out.iter_mut().find(|(n, _)| *n == name.as_str()) {
                    Some(slot) => slot.1 = c,
                    None => out.push((name.as_str(), c)),
                }
            }
        }
        out
    }

    /// The same for lengths.
    pub fn lengths(&self) -> Vec<(&'a str, f32)> {
        let mut out: Vec<(&'a str, f32)> = Vec::new();
        for t in [self.host, self.own].into_iter().flatten() {
            for (i, (name, v)) in t.lengths.iter().enumerate() {
                if t.id(name) != Some(TokenRef::Length(i as u16)) {
                    continue;
                }
                match out.iter_mut().find(|(n, _)| *n == name.as_str()) {
                    Some(slot) => slot.1 = *v,
                    None => out.push((name.as_str(), *v)),
                }
            }
        }
        out
    }

    /// The names a colour paints under this frame, for an inspector
    /// printing a value: every token (own over host) whose resolved
    /// colour is `c`. Two names with one value are both listed.
    pub fn color_names(&self, c: Color) -> Vec<&'a str> {
        self.colors()
            .into_iter()
            .filter(|(_, v)| *v == c)
            .map(|(n, _)| n)
            .collect()
    }

    /// The same for a length.
    pub fn length_names(&self, v: f32) -> Vec<&'a str> {
        self.lengths()
            .into_iter()
            .filter(|(_, x)| *x == v)
            .map(|(n, _)| n)
            .collect()
    }
}

/// A `$name` reference as a prop value spells it: the name without the
/// sigil, or `None` for a value that is not a reference.
pub fn reference(s: &str) -> Option<&str> {
    s.strip_prefix('$').filter(|n| !n.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_role_name_is_refused_and_remembered() {
        let t = Tokens::new()
            .color("surface", Color::WHITE)
            .length("radius", 3.0)
            .color("peach", Color::WHITE);
        assert_eq!(t.reserved(), ["surface", "radius"]);
        assert_eq!(t.colors().len(), 1);
        assert!(t.lengths().is_empty());
        // Both spellings are the role's.
        assert!(is_role("border_strong"));
        assert!(is_role("borderStrong"));
        assert!(is_role("controlPadX"));
    }

    #[test]
    fn redeclaring_a_name_keeps_its_index() {
        let t = Tokens::new()
            .color("a", Color::WHITE)
            .color("b", Color::BLACK)
            .color("a", Color::BLACK);
        assert_eq!(t.id("a"), Some(TokenRef::Color(0)));
        assert_eq!(t.colors()[0].1, ColorToken::same(Color::BLACK));
        assert_eq!(t.colors().len(), 2);
    }

    /// A name declared as both kinds binds the later one; the earlier
    /// entry stays reachable by its index (the wire's contract) but is
    /// not what the name lists as.
    #[test]
    fn a_name_declared_twice_lists_once() {
        let t = Tokens::new().color("gap", Color::WHITE).length("gap", 6.0);
        let theme = Theme::default();
        let metrics = Metrics::default();
        let look = TokenLookup {
            own: Some(&t),
            host: None,
            theme: &theme,
            metrics: &metrics,
        };
        assert_eq!(look.colors(), vec![]);
        assert_eq!(look.lengths(), vec![("gap", 6.0)]);
        assert_eq!(look.color_names(Color::WHITE), Vec::<&str>::new());
        assert_eq!(
            look.color_at(COLOR_ROLES as u32),
            Some(Color::WHITE),
            "by index still"
        );
    }

    #[test]
    fn wire_indices_put_the_roles_first() {
        assert_eq!(TokenRef::ColorRole(1).index(), 1);
        assert_eq!(TokenRef::Color(0).index(), COLOR_ROLES);
        assert_eq!(TokenRef::Length(2).index(), LENGTH_ROLES + 2);
    }

    #[test]
    fn a_reference_is_a_dollar_and_a_name() {
        assert_eq!(reference("$peach"), Some("peach"));
        assert_eq!(reference("$"), None);
        assert_eq!(reference("#fff"), None);
    }

    // -- derived tokens (ADR 0028) ----------------------------------------

    const PEACH: Color = Color {
        r: 1.0,
        g: 0.8,
        b: 0.6,
        a: 1.0,
    };

    /// A derived colour is rounded to eight bits a channel, so it is
    /// compared as hex — against an expectation built the same way.
    fn same(a: Color, b: Color) -> bool {
        a.to_hex() == b.to_hex()
    }

    fn q(c: Color) -> Color {
        Color::hex(c.to_hex())
    }

    /// A chain folds in order over the source, and the same two steps the
    /// other way round give a different colour when they do not commute.
    #[test]
    fn a_chain_folds_in_declaration_order() {
        let t = Tokens::new()
            .color("peach", PEACH)
            .color("black", Color::BLACK)
            .derive("lit", "peach", [ColorOp::Lift(0.3)])
            .derive("wash", "peach", [ColorOp::Lift(0.3), ColorOp::Alpha(0.5)])
            .derive(
                "lit_then_read",
                "peach",
                [ColorOp::Lift(0.3), ColorOp::Readable("black".into(), 4.5)],
            )
            .derive(
                "read_then_lit",
                "peach",
                [ColorOp::Readable("black".into(), 4.5), ColorOp::Lift(0.3)],
            );
        let theme = Theme::dark();
        let hover = t.resolve_color(2, &theme);
        assert!(same(hover, PEACH.mix(Color::WHITE, 0.3)));
        let wash = t.resolve_color(3, &theme);
        assert!(same(wash, q(PEACH.mix(Color::WHITE, 0.3)).with_alpha(0.5)));
        // Peach already reads on black, so the readable step is a no-op
        // after the lift; before it, the lift still applies after.
        let a = t.resolve_color(4, &theme);
        let b = t.resolve_color(5, &theme);
        assert!(same(a, PEACH.mix(Color::WHITE, 0.3)));
        assert!(same(b, PEACH.mix(Color::WHITE, 0.3)));
        assert_eq!(t.recipe(3).as_deref(), Some("peach → lift 0.3 → alpha 0.5"));
        assert_eq!(t.recipe(0), None);
    }

    /// A derived token follows its source's half, and a role source
    /// follows the theme; `raise` goes toward the front of the base.
    #[test]
    fn a_derived_token_follows_the_appearance_with_its_source() {
        let t = Tokens::new()
            .color_themed("ink", Color::BLACK, Color::WHITE)
            .derive("ink_soft", "ink", [ColorOp::Alpha(0.5)])
            .derive("accent_up", "accent", [ColorOp::Raise(0.2)])
            .derive("alias", "accent", []);
        let (light, dark) = (Theme::light(), Theme::dark());
        assert!(same(
            t.resolve_color(1, &light),
            Color::BLACK.with_alpha(0.5)
        ));
        assert!(same(
            t.resolve_color(1, &dark),
            Color::WHITE.with_alpha(0.5)
        ));
        assert!(same(
            t.resolve_color(2, &dark),
            dark.accent.mix(Color::WHITE, 0.2)
        ));
        assert!(same(
            t.resolve_color(2, &light),
            light.accent.mix(Color::BLACK, 0.2)
        ));
        assert_eq!(t.resolve_color(3, &dark), dark.accent);
        assert_eq!(t.recipe(2).as_deref(), Some("accent → raise 0.2"));
        // The devtools' halves: the one in effect is the frame's, the
        // other under the other base with the same accent.
        let (l, d) = t.halves(1, &dark);
        assert_eq!((l.to_hex() & 0xff, d.to_hex() & 0xff), (0x80, 0x80));
        assert_eq!((l.r, d.r), (0.0, 1.0));
    }

    /// A derived token may derive from a derived one; `mix` reaches
    /// another token; `readable` moves toward whichever of black and
    /// white reads on its operand.
    #[test]
    fn sources_chain_and_readable_reaches() {
        let t = Tokens::new()
            .color("peach", PEACH)
            .color("white", Color::WHITE)
            .derive("lit", "peach", [ColorOp::Lift(0.3)])
            .derive("lit2", "lit", [ColorOp::Lift(0.35)])
            .derive("halfway", "peach", [ColorOp::Mix("white".into(), 0.5)])
            .derive(
                "on_white",
                "peach",
                [ColorOp::Readable("white".into(), 4.5)],
            );
        let theme = Theme::dark();
        let pressed = t.resolve_color(3, &theme);
        assert!(same(
            pressed,
            q(PEACH.mix(Color::WHITE, 0.3)).mix(Color::WHITE, 0.35)
        ));
        assert!(same(
            t.resolve_color(4, &theme),
            PEACH.mix(Color::WHITE, 0.5)
        ));
        let on_white = t.resolve_color(5, &theme);
        assert!(
            on_white.contrast(Color::WHITE) >= 4.5,
            "moved toward black until it read"
        );
        assert_eq!(t.recipe(5).as_deref(), Some("peach → readable white 4.5"));
    }

    /// A source that is undeclared, a length, a later token, or the
    /// token itself drops the declaration and says which name failed.
    #[test]
    fn an_unresolved_source_drops_the_token_and_is_reported() {
        let t = Tokens::new()
            .color("peach", PEACH)
            .length("gap", 6.0)
            .derive("a", "peech", [])
            .derive("b", "gap", [])
            .derive("c", "later", [])
            .color("later", Color::WHITE)
            .derive("d", "peach", [ColorOp::Mix("nothing".into(), 0.5)])
            .derive("peach", "peach", [ColorOp::Lift(0.1)]);
        assert_eq!(t.colors().len(), 2, "peach and later");
        assert_eq!(t.id("a"), None);
        assert_eq!(
            t.colors()[0].1,
            ColorToken::same(PEACH),
            "peach is not its own lift"
        );
        let dropped: Vec<(&str, &str)> = t
            .unresolved()
            .iter()
            .map(|u| (u.token.as_str(), u.source.as_str()))
            .collect();
        assert_eq!(
            dropped,
            [
                ("a", "peech"),
                ("b", "gap"),
                ("c", "later"),
                ("d", "nothing"),
                ("peach", "peach")
            ]
        );
    }

    /// A derived token re-declared as a value, and a value as derived,
    /// keep the index; a lookup resolves either by index and by name.
    #[test]
    fn a_derived_token_resolves_through_the_lookup() {
        let t = Tokens::new()
            .color("peach", PEACH)
            .color("lit", Color::BLACK)
            .derive("lit", "peach", [ColorOp::Lift(0.3)]);
        assert_eq!(t.id("lit"), Some(TokenRef::Color(1)));
        assert!(t.colors()[1].1.is_derived());
        let theme = Theme::dark();
        let metrics = Metrics::default();
        let look = TokenLookup {
            own: Some(&t),
            host: None,
            theme: &theme,
            metrics: &metrics,
        };
        let want = PEACH.mix(Color::WHITE, 0.3);
        assert!(same(look.color("lit").unwrap(), want));
        assert!(same(look.color_at(COLOR_ROLES as u32 + 1).unwrap(), want));
        assert_eq!(look.color_names(q(want)), vec!["lit"]);
    }

    #[test]
    fn a_verb_parses_with_the_operands_it_takes() {
        assert_eq!(ColorOp::parse("lift", None, 0.3), Some(ColorOp::Lift(0.3)));
        assert_eq!(
            ColorOp::parse("mix", Some("ink"), 0.5),
            Some(ColorOp::Mix("ink".into(), 0.5))
        );
        assert_eq!(ColorOp::parse("lift", Some("ink"), 0.3), None);
        assert_eq!(ColorOp::parse("mix", None, 0.5), None);
        assert_eq!(ColorOp::parse("glow", None, 0.5), None);
        assert_eq!(ColorOp::takes_color("readable"), Some(true));
        assert_eq!(ColorOp::takes_color("glow"), None);
        assert_eq!(ColorOp::VERBS.iter().position(|v| *v == "alpha"), Some(3));
    }
}
