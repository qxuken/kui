//! Tokens: named colours and lengths an app declares beside the theme and
//! the metrics, and references a colour or length prop by name
//! (`docs/adr/0027-tokens-beside-the-theme.md`).
//!
//! A [`Theme`] is twenty-three roles the stock widgets paint from; a
//! [`Metrics`] is sixteen lengths they are built from. Both are closed,
//! and an app whose palette *is* the design — LCARS peach, tangerine, a
//! sidebar width — has a vocabulary neither names. [`Tokens`] is that
//! vocabulary: a colour token carries a light and a dark half (the same
//! value twice, in the common case), a length token is one number in
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
use crate::metrics::Metrics;
use crate::schema::{METRIC_ROLES, THEME_ROLES};
use crate::theme::Theme;

/// A colour token: one value per base. [`ColorToken::same`] is the
/// unthemed case, and what a declaration with one colour builds.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ColorToken {
    pub light: Color,
    pub dark: Color,
}

impl ColorToken {
    pub fn same(c: Color) -> Self {
        Self { light: c, dark: c }
    }

    pub fn themed(light: Color, dark: Color) -> Self {
        Self { light, dark }
    }

    /// The half a theme paints with: the dark one on the dark base and on
    /// an unknown appearance, which is the dark base without claiming the
    /// user chose it (ADR 0019, decision 4).
    pub fn resolve(&self, theme: &Theme) -> Color {
        if theme.is_dark() {
            self.dark
        } else {
            self.light
        }
    }
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
    /// Names refused because a role owns them, kept so the core can warn
    /// once per name when the table is declared.
    reserved: Vec<String>,
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

    pub fn is_empty(&self) -> bool {
        self.colors.is_empty() && self.lengths.is_empty()
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
        t.colors[i as usize].1.resolve(self.theme)
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
        let i = (index - COLOR_ROLES as u32) as usize;
        self.table()
            .and_then(|t| t.colors.get(i))
            .map(|(_, tok)| tok.resolve(self.theme))
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
            for (name, tok) in &t.colors {
                let c = tok.resolve(self.theme);
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
            for (name, v) in &t.lengths {
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
}
