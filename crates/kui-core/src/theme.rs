//! The named colours a view paints with, derived from what the OS said —
//! `docs/adr/0019-a-theme-derived-from-appearance-and-accent.md`.
//!
//! [`Appearance`] and the OS accent have been in [`crate::env::SystemEnv`]
//! since they were plumbed, and `env`'s own doc is firm that the core acts
//! on neither: "a dark appearance does not repaint anything: the view
//! decides, because only it knows which of its colours is the background".
//! That is still true. What was missing is the other half — a view that
//! *wants* to decide had nothing to decide *with*, so every one of them
//! wrote the same two dozen hex literals again, and the stock widgets in
//! this crate wrote them a third time. A [`Theme`] is that missing half:
//! plain data, derived from the two facts, and read rather than obeyed.
//!
//! Three ways to have one, which is [`ThemeSource`]:
//!
//! - **Derived** (the default): the OS's appearance and the OS's accent.
//!   A host that reports neither gets exactly what kui painted before this
//!   module existed, which is what makes it safe to be the default.
//! - **Derived with an accent**: the OS's light/dark, the app's brand
//!   colour. What most apps with a colour of their own actually want.
//! - **Pinned**: a [`Theme`] the app built, followed by nothing.
//!
//! The roles are the ones the codebase had already voted for: three
//! examples arrived independently at `bg` / `panel` / `border` / `fg` /
//! `dim` / `faint` / `accent`, with the same values. This is that set,
//! spelled once.

use crate::color::Color;
use crate::env::{Appearance, SystemEnv};

/// WCAG's contrast ratio between two opaque colours, 1:1 to 21:1 — the
/// number "4.5:1" and "3:1" are ratios of. Kept here rather than on
/// [`Color`] because it is a *palette* question: the roles are checked
/// against each other, and a view that wants the number has
/// [`Color::luminance`] to build it from.
pub(crate) fn contrast(a: Color, b: Color) -> f32 {
    let (hi, lo) = (
        a.luminance().max(b.luminance()),
        a.luminance().min(b.luminance()),
    );
    (hi + 0.05) / (lo + 0.05)
}

/// Every colour the stock widgets and the core's own chrome paint with,
/// as roles rather than values. Plain data and [`Copy`]: a view reads it
/// off `ui.theme()` and may keep, mutate or replace its own copy.
///
/// Roles, not a ramp. `surface` is not "grey 800" — it is *the colour a
/// card is*, and under a light theme it is nearly white. A view that
/// wants "one step lighter than this" has [`Color::mix`] and
/// [`Theme::raise`] for that, and nothing here promises an ordering
/// beyond the one the names carry.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Theme {
    /// Which base this was built from. `Unknown` means the host never
    /// said, and the dark base stands — see [`Theme::derive`].
    pub appearance: Appearance,

    // -- surfaces, back to front --------------------------------------
    /// The window behind everything.
    pub bg: Color,
    /// A card, panel or list sitting on `bg`.
    pub surface: Color,
    /// A surface that floats *above* content: a menu, a tooltip, a
    /// popover. Separate from `surface` because it has to read as
    /// nearer, and under a light theme "nearer" is not "lighter" — a
    /// float on a white page separates by its border.
    pub raised: Color,
    /// A well cut *into* a surface: a text field, a code block, a track.
    pub sunken: Color,

    // -- lines ---------------------------------------------------------
    /// The hairline between two surfaces.
    pub border: Color,
    /// A border that has to be seen — a float's edge, a focused field.
    pub border_strong: Color,

    // -- text ----------------------------------------------------------
    /// Body text. What a `TextStyle` with no colour of its own resolves
    /// to, which is what makes `<text>hello</text>` legible on both bases.
    pub fg: Color,
    /// Secondary text: captions, hints, an accelerator beside a label.
    pub muted: Color,
    /// Text that is barely there: a placeholder, a gutter number.
    pub faint: Color,

    // -- accent --------------------------------------------------------
    /// The one saturated colour: the OS accent when the host reports one,
    /// the app's when it pinned one, and kui's blue otherwise.
    pub accent: Color,
    /// `accent` under a pointer, and under a press.
    pub accent_hover: Color,
    pub accent_pressed: Color,
    /// Black or white — whichever a reader can see on `accent`.
    pub on_accent: Color,
    /// The accent as a *wash* rather than a fill: what a selected menu
    /// row, a chosen tab or a highlighted list item is painted with.
    ///
    /// Translucent on purpose. A row filled with the solid accent needs
    /// its label to flip to `on_accent` in the same frame the fill lands,
    /// and `hover_bg` is resolved by the core after the view has already
    /// chosen that label — so on a light theme the row would spend a
    /// frame as dark-on-blue. A wash keeps `fg` readable over both bases
    /// and stays declarative.
    pub accent_soft: Color,
    /// What a text selection is painted under. Translucent: the glyphs
    /// under it keep their own colour.
    pub selection: Color,
    /// The default keyboard focus ring (ADR 0002).
    pub focus_ring: Color,

    // -- neutral interaction -------------------------------------------
    /// A translucent wash over a neutral control that is hovered, and one
    /// over a pressed one. Overlays, not fills: they go *on* whatever
    /// surface the control sits on, so one pair works for every surface.
    /// `pressed` is the firmer of the two on both bases.
    pub hover: Color,
    pub pressed: Color,
    /// What a disabled control's opacity is multiplied by.
    pub disabled_opacity: f32,

    // -- status --------------------------------------------------------
    pub success: Color,
    pub warning: Color,
    pub danger: Color,

    // -- chrome --------------------------------------------------------
    /// The scrollbar thumb at rest, and while hovered or dragged.
    pub scrollbar: Color,
    pub scrollbar_active: Color,
}

impl Default for Theme {
    /// The dark base with kui's own accent: what this crate painted
    /// before themes existed.
    fn default() -> Self {
        Self::dark()
    }
}

impl Theme {
    /// The text colour kui painted before it could ask the OS anything,
    /// and the dark base's `fg`. What an unresolved style falls back to.
    pub const DEFAULT_FG: Color = Color {
        r: 0xe8 as f32 / 255.0,
        g: 0xe8 as f32 / 255.0,
        b: 0xea as f32 / 255.0,
        a: 1.0,
    };

    /// kui's own accent, and the stock button's background since there
    /// was a stock button. Stands in wherever no accent is known.
    pub const ACCENT: Color = Color {
        r: 0x3b as f32 / 255.0,
        g: 0x5b as f32 / 255.0,
        b: 0xd4 as f32 / 255.0,
        a: 1.0,
    };

    /// The theme for what the OS said: `system.appearance` picks the base,
    /// `system.accent` recolours it.
    ///
    /// An `Unknown` appearance takes the **dark** base. Not a guess about
    /// the user — the honest answer to "what did kui paint before it could
    /// ask" — and the reason a host that reports nothing sees no change at
    /// all. A view that would rather guess light has [`Theme::light`].
    pub fn derive(appearance: Appearance, accent: Option<Color>) -> Self {
        let base = match appearance {
            Appearance::Light => Self::light(),
            Appearance::Dark | Appearance::Unknown => Self::dark(),
        };
        let base = Theme { appearance, ..base };
        match accent {
            Some(c) => base.with_accent(c),
            None => base,
        }
    }

    /// [`derive`](Theme::derive) from a whole [`SystemEnv`], which is how
    /// the core does it every frame.
    pub fn from_system(sys: &SystemEnv) -> Self {
        Self::derive(sys.appearance, sys.accent)
    }

    /// The dark base. Every value here is one the crate already painted:
    /// the muted grey 16 files were writing out, the field background the
    /// stock input had, the ring ADR 0002 nailed down.
    ///
    /// The accent family is hand-picked rather than run through
    /// [`with_accent`](Theme::with_accent), for the reason
    /// [`crate::widgets::button_spec`] gives for its own trio: so that a
    /// host which reports no accent paints exactly what it always did,
    /// down to the byte. Hand an accent in and the arithmetic takes over.
    pub fn dark() -> Self {
        Self {
            appearance: Appearance::Dark,
            bg: Color::rgb8(0x14, 0x16, 0x1e),
            surface: Color::rgb8(0x1a, 0x1d, 0x27),
            raised: Color::rgb8(0x24, 0x27, 0x33),
            sunken: Color::rgb8(0x0e, 0x10, 0x16),
            border: Color::rgb8(0x2a, 0x2d, 0x3a),
            border_strong: Color::rgb8(0x3a, 0x3e, 0x4e),
            fg: Self::DEFAULT_FG,
            muted: Color::rgb8(0x8a, 0x8f, 0xa3),
            faint: Color::rgb8(0x6e, 0x75, 0x8a),
            accent: Self::ACCENT,
            accent_hover: Color::rgb8(0x47, 0x6c, 0xe0),
            accent_pressed: Color::rgb8(0x2f, 0x54, 0xc4),
            on_accent: Color::WHITE,
            accent_soft: Self::ACCENT.with_alpha(0.30),
            selection: Color::rgba8(0x3b, 0x5b, 0xd4, 0x66),
            focus_ring: Color::rgb8(0x7f, 0x9c, 0xf5),
            hover: Color::rgba(1.0, 1.0, 1.0, 0.08),
            pressed: Color::rgba(1.0, 1.0, 1.0, 0.14),
            disabled_opacity: 0.5,
            success: Color::rgb8(0x73, 0xd9, 0x8c),
            warning: Color::rgb8(0xd9, 0xa1, 0x4d),
            danger: Color::rgb8(0xe8, 0x5d, 0x5d),
            scrollbar: Color::rgba(1.0, 1.0, 1.0, 0.18),
            scrollbar_active: Color::rgba(1.0, 1.0, 1.0, 0.40),
        }
    }

    /// The light base: the same roles, mirrored rather than inverted.
    ///
    /// Mirrored, because inverting is wrong twice. A float above content
    /// is *lighter* than the page on a dark base and no lighter than
    /// white on a light one, so it separates by border instead; and the
    /// accent does not flip at all — a blue button is a blue button, and
    /// only its ring and its selection tint have to move, because the
    /// pale ring that reads on `#14161e` is invisible on `#f7f8fa`.
    pub fn light() -> Self {
        let accent = Self::ACCENT;
        Self {
            appearance: Appearance::Light,
            bg: Color::rgb8(0xf6, 0xf7, 0xf9),
            surface: Color::rgb8(0xff, 0xff, 0xff),
            raised: Color::rgb8(0xff, 0xff, 0xff),
            sunken: Color::rgb8(0xec, 0xee, 0xf2),
            border: Color::rgb8(0xdd, 0xe1, 0xe8),
            border_strong: Color::rgb8(0xb4, 0xbb, 0xc8),
            fg: Color::rgb8(0x1b, 0x1e, 0x27),
            muted: Color::rgb8(0x5b, 0x61, 0x71),
            faint: Color::rgb8(0x76, 0x7d, 0x8d),
            accent,
            accent_hover: accent.mix(Color::WHITE, 0.09),
            accent_pressed: accent.mix(Color::BLACK, 0.10),
            on_accent: Color::WHITE,
            accent_soft: accent.with_alpha(0.16),
            selection: accent.with_alpha(0.28),
            focus_ring: accent,
            hover: Color::rgba(0.0, 0.0, 0.0, 0.06),
            pressed: Color::rgba(0.0, 0.0, 0.0, 0.12),
            disabled_opacity: 0.5,
            success: Color::rgb8(0x1a, 0x7a, 0x3e),
            warning: Color::rgb8(0x8a, 0x5c, 0x08),
            danger: Color::rgb8(0xc0, 0x2b, 0x2b),
            scrollbar: Color::rgba(0.0, 0.0, 0.0, 0.22),
            scrollbar_active: Color::rgba(0.0, 0.0, 0.0, 0.42),
        }
    }

    /// This theme with `accent` in place of its own, and everything that
    /// comes *off* the accent recomputed with it: the two button shades,
    /// the label that goes on top, the selection tint and the ring.
    ///
    /// The shades are [`crate::widgets::button_palette`]'s arithmetic, so
    /// an accent-painted button reads as the same control in a different
    /// colour rather than as a different control. The ring keeps each
    /// base's habit and is then held to [`Theme::ring_for`]'s promise.
    pub fn with_accent(self, accent: Color) -> Self {
        let ring = self.ring_for(accent);
        Self {
            accent,
            accent_hover: accent.mix(Color::WHITE, 0.09),
            accent_pressed: accent.mix(Color::BLACK, 0.10),
            on_accent: crate::widgets::readable_on(accent),
            accent_soft: accent.with_alpha(match self.appearance {
                Appearance::Light => 0.16,
                _ => 0.30,
            }),
            selection: accent.with_alpha(match self.appearance {
                Appearance::Light => 0.28,
                _ => 0.40,
            }),
            focus_ring: ring,
            ..self
        }
    }

    /// A focus ring in `accent` that can actually be *seen* on this
    /// theme's `bg`: the accent moved toward the front of the base —
    /// white on a dark one, black on a light one — until it clears the
    /// 3:1 ADR 0002 asks of a focus indicator.
    ///
    /// Each base's habit is where it starts: the dark one lifts a
    /// saturated ring that would otherwise sink into the page, and the
    /// light one takes the accent as it is, because most accents are
    /// already dark enough on a near-white page. The loop is what turns
    /// that from a hope into a promise — a *light* accent on the light
    /// base is the case it exists for. macOS's yellow taken verbatim is
    /// 1.49:1 on `#f6f7f9`, which is not a ring, it is a rumour.
    pub fn ring_for(self, accent: Color) -> Color {
        let toward = if self.is_dark() {
            Color::WHITE
        } else {
            Color::BLACK
        };
        let mut t = if self.is_dark() { 0.35 } else { 0.0 };
        loop {
            let ring = accent.mix(toward, t);
            // `toward` itself always clears 3:1 on its own base, so the
            // cap is a floor and not a give-up.
            if t >= 1.0 || contrast(ring, self.bg) >= 3.0 {
                return ring;
            }
            t = (t + 0.05).min(1.0);
        }
    }

    /// Whether this is a dark theme — the question a view asks when it has
    /// a decision of its own to make (which of two images, how heavy a
    /// shadow). `Unknown` answers the way [`derive`](Theme::derive) does.
    pub fn is_dark(self) -> bool {
        self.appearance != Appearance::Light
    }

    /// `c` moved `t` of the way toward the *front* of this theme: lighter
    /// on a dark one, darker on a light one. The arithmetic behind
    /// "one step up from this surface", written once so a view does not
    /// have to branch on the appearance to get it right.
    pub fn raise(self, c: Color, t: f32) -> Color {
        c.mix(
            if self.is_dark() {
                Color::WHITE
            } else {
                Color::BLACK
            },
            t,
        )
    }

    /// Black or white, whichever a reader can see on `bg`
    /// ([`crate::widgets::readable_on`]).
    pub fn on(self, bg: Color) -> Color {
        crate::widgets::readable_on(bg)
    }
}

/// Where a [`Core`](crate::runtime::Core)'s theme comes from, re-read at
/// the start of every frame. `Derived` is the default.
///
/// `Pinned` makes this as big as a whole [`Theme`], which is the point:
/// there is one of these per window, read once a frame, and boxing it to
/// save three hundred bytes would cost the `Copy` that lets a view hold
/// the answer without borrowing the core.
#[allow(clippy::large_enum_variant)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum ThemeSource {
    /// [`Theme::from_system`] on whatever `env.system` currently says, so
    /// the app follows the OS without writing a line about it.
    #[default]
    Derived,
    /// The OS's appearance, this accent. What an app with a brand colour
    /// wants: it should still go light when the user does.
    DerivedWithAccent(Color),
    /// Exactly this, following nothing.
    Pinned(Theme),
}

impl ThemeSource {
    /// The theme this source resolves to under `sys`.
    pub fn resolve(&self, sys: &SystemEnv) -> Theme {
        match self {
            Self::Derived => Theme::from_system(sys),
            Self::DerivedWithAccent(c) => Theme::derive(sys.appearance, Some(*c)),
            Self::Pinned(t) => *t,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The reason this is safe to turn on for everyone: a host that
    /// cannot ask the OS anything gets the palette kui always painted.
    #[test]
    fn an_unknown_appearance_is_what_kui_always_painted() {
        let t = Theme::derive(Appearance::Unknown, None);
        assert_eq!(t.fg, Color::rgb8(0xe8, 0xe8, 0xea), "the old text colour");
        assert_eq!(t.accent, Theme::ACCENT, "the old button blue");
        assert_eq!(
            t.focus_ring,
            Color::rgb8(0x7f, 0x9c, 0xf5),
            "ADR 0002's ring"
        );
        assert_eq!(t.selection, Color::rgba8(0x3b, 0x5b, 0xd4, 0x66));
        assert_eq!(t.muted, Color::rgb8(0x8a, 0x8f, 0xa3));
        // And it is the dark base, without claiming the user chose it.
        assert_eq!(t.bg, Theme::dark().bg);
        assert_eq!(t.appearance, Appearance::Unknown);
    }

    /// Both bases have to be readable, which is the one thing a palette
    /// can be checked for rather than argued about. WCAG AA is 4.5:1 for
    /// body text and 3:1 for large text and UI edges.
    #[test]
    fn every_text_role_is_readable_on_every_surface() {
        for t in [Theme::dark(), Theme::light()] {
            let name = if t.is_dark() { "dark" } else { "light" };
            for (sn, surface) in [
                ("bg", t.bg),
                ("surface", t.surface),
                ("sunken", t.sunken),
                ("raised", t.raised),
            ] {
                let fg = contrast(t.fg, surface);
                assert!(fg >= 4.5, "{name}: fg on {sn} is {fg:.2}:1");
                let muted = contrast(t.muted, surface);
                assert!(muted >= 4.5, "{name}: muted on {sn} is {muted:.2}:1");
                // Faint is the placeholder tier: large-text/UI grade.
                let faint = contrast(t.faint, surface);
                assert!(faint >= 3.0, "{name}: faint on {sn} is {faint:.2}:1");
            }
            let label = contrast(t.on_accent, t.accent);
            assert!(label >= 4.5, "{name}: the button label is {label:.2}:1");
            // A ring nobody can see is not a focus indicator (ADR 0002).
            let ring = contrast(t.focus_ring, t.bg);
            assert!(ring >= 3.0, "{name}: the focus ring is {ring:.2}:1");
            for (sn, status) in [
                ("success", t.success),
                ("warning", t.warning),
                ("danger", t.danger),
            ] {
                let c = contrast(status, t.surface);
                assert!(c >= 4.5, "{name}: {sn} on a surface is {c:.2}:1");
            }
        }
    }

    /// An accent recolours everything that comes off it, and a light
    /// accent flips the label the way the stock button always did.
    #[test]
    fn an_accent_carries_the_whole_family_with_it() {
        let yellow = Color::hex(0xffc409ff);
        let t = Theme::derive(Appearance::Dark, Some(yellow));
        assert_eq!(t.accent, yellow);
        assert_eq!(t.on_accent, Color::BLACK, "white on yellow is not a button");
        assert_ne!(t.accent_hover, t.accent);
        assert_ne!(t.accent_pressed, t.accent);
        assert_eq!(t.selection.a, 0.40, "still a tint, not a fill");
        // And the surfaces are untouched: an accent is not a repaint.
        assert_eq!(t.bg, Theme::dark().bg);
        assert_eq!(t.fg, Theme::dark().fg);
    }

    /// The middle source is the one an app with a brand colour wants.
    #[test]
    fn a_source_can_follow_the_os_light_dark_and_not_its_accent() {
        let brand = Color::hex(0xd2691eff);
        let src = ThemeSource::DerivedWithAccent(brand);
        let mut sys = SystemEnv {
            accent: Some(Color::hex(0x007affff)),
            appearance: Appearance::Light,
            ..SystemEnv::default()
        };
        let t = src.resolve(&sys);
        assert_eq!(t.accent, brand, "the app's colour, not the OS's");
        assert_eq!(t.bg, Theme::light().bg, "the OS's light, not the app's");
        sys.appearance = Appearance::Dark;
        assert_eq!(src.resolve(&sys).bg, Theme::dark().bg);
        // Pinned follows nothing at all.
        let pinned = ThemeSource::Pinned(Theme::light());
        assert_eq!(pinned.resolve(&sys).bg, Theme::light().bg);
    }

    /// The two stock bases are checked above, but the ring is the one
    /// role that is *derived* from a colour kui does not choose — so it
    /// has to hold for whatever the OS reports, not only for kui's blue.
    /// The light base is where a verbatim accent fails: macOS's yellow
    /// is 1.49:1 on `#f6f7f9`, which no one would find.
    #[test]
    fn a_derived_focus_ring_is_visible_whatever_the_accent_is() {
        for hex in [
            0x3b5bd4ff, // kui's own
            0x007affff, // macOS blue
            0xffc409ff, // macOS yellow — the light one
            0xf74f9eff, // macOS pink
            0x2f7d4fff, // a dark brand green
            0xffffffff, // and the two ends
            0x000000ff,
        ] {
            for appearance in [Appearance::Light, Appearance::Dark, Appearance::Unknown] {
                let t = Theme::derive(appearance, Some(Color::hex(hex)));
                let c = contrast(t.focus_ring, t.bg);
                assert!(c >= 3.0, "{appearance:?} + {hex:08x}: the ring is {c:.2}:1");
            }
        }
        // And the case that used to ship: the ring is no longer the
        // accent itself here, because the accent itself was invisible.
        let yellow = Color::hex(0xffc409ff);
        let light = Theme::derive(Appearance::Light, Some(yellow));
        assert_ne!(light.focus_ring, yellow);
        assert!(contrast(yellow, light.bg) < 1.6, "which is why");
    }

    /// `raise` is the branch a view would otherwise write by hand.
    #[test]
    fn raise_goes_toward_the_front_of_whichever_base() {
        let dark = Theme::dark();
        assert!(dark.raise(dark.surface, 0.1).luminance() > dark.surface.luminance());
        let light = Theme::light();
        assert!(light.raise(light.surface, 0.1).luminance() < light.surface.luminance());
    }
}
