#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Color {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

impl Color {
    pub const TRANSPARENT: Color = Color {
        r: 0.0,
        g: 0.0,
        b: 0.0,
        a: 0.0,
    };
    pub const WHITE: Color = Color {
        r: 1.0,
        g: 1.0,
        b: 1.0,
        a: 1.0,
    };
    pub const BLACK: Color = Color {
        r: 0.0,
        g: 0.0,
        b: 0.0,
        a: 1.0,
    };

    pub fn rgba(r: f32, g: f32, b: f32, a: f32) -> Self {
        Self { r, g, b, a }
    }

    pub fn rgb8(r: u8, g: u8, b: u8) -> Self {
        Self::rgba8(r, g, b, 255)
    }

    pub fn rgba8(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self {
            r: r as f32 / 255.0,
            g: g as f32 / 255.0,
            b: b as f32 / 255.0,
            a: a as f32 / 255.0,
        }
    }

    /// 0xRRGGBBAA
    /// The `0xRRGGBBAA` this colour is, rounded to eight bits a channel:
    /// what `hex` reads.
    pub fn to_hex(&self) -> u32 {
        let ch = |v: f32| (v.clamp(0.0, 1.0) * 255.0).round() as u32;
        (ch(self.r) << 24) | (ch(self.g) << 16) | (ch(self.b) << 8) | ch(self.a)
    }

    pub fn hex(v: u32) -> Self {
        Self::rgba8((v >> 24) as u8, (v >> 16) as u8, (v >> 8) as u8, v as u8)
    }

    pub fn with_alpha(mut self, a: f32) -> Self {
        self.a = a;
        self
    }

    pub fn is_visible(&self) -> bool {
        self.a > 0.0
    }

    /// This colour moved `t` of the way toward `other`, per channel, with
    /// `self`'s alpha kept — `mix(WHITE, 0.1)` is "a little lighter" and
    /// `mix(BLACK, 0.1)` "a little darker", which is how a palette is
    /// built from one colour (`widgets::button_palette`).
    ///
    /// Straight sRGB, not a perceptual space: it is the interpolation the
    /// animation slots already do channel by channel, and the one a view
    /// gets if it lerps two colours itself. `t` outside 0..=1 extrapolates
    /// rather than clamping, so a caller can overshoot on purpose.
    /// The colour as a tween's four lanes.
    #[inline]
    pub(crate) fn lanes(self) -> [f32; 4] {
        [self.r, self.g, self.b, self.a]
    }

    /// The inverse of [`Self::lanes`].
    #[inline]
    pub(crate) fn from_lanes(v: [f32; 4]) -> Color {
        Color {
            r: v[0],
            g: v[1],
            b: v[2],
            a: v[3],
        }
    }

    /// Every channel, alpha included, `t` of the way to `other` — what a
    /// tween does to a colour. [`Self::mix`] keeps this colour's alpha.
    pub fn lerp(self, other: Color, t: f32) -> Color {
        Color::from_lanes(std::array::from_fn(|i| {
            self.lanes()[i] + (other.lanes()[i] - self.lanes()[i]) * t
        }))
    }

    pub fn mix(self, other: Color, t: f32) -> Color {
        Color {
            r: self.r + (other.r - self.r) * t,
            g: self.g + (other.g - self.g) * t,
            b: self.b + (other.b - self.b) * t,
            a: self.a,
        }
    }

    /// WCAG relative luminance, 0 (black) to 1 (white): the number a
    /// contrast decision is made from — "is this background light enough
    /// to want dark text" — rather than the average of the channels, which
    /// would call a saturated blue and a saturated green equally bright.
    /// Alpha is not in it; a translucent colour's luminance is the
    /// luminance of what it would be over nothing.
    pub fn luminance(self) -> f32 {
        let ch = |c: f32| {
            let c = c.clamp(0.0, 1.0);
            if c <= 0.03928 {
                c / 12.92
            } else {
                ((c + 0.055) / 1.055).powf(2.4)
            }
        };
        0.2126 * ch(self.r) + 0.7152 * ch(self.g) + 0.0722 * ch(self.b)
    }

    /// WCAG's contrast ratio between two opaque colours, 1:1 to 21:1 —
    /// the number "4.5:1" (body text) and "3:1" (large text, UI edges, a
    /// focus ring) are ratios of. Symmetric. The arithmetic a palette is
    /// *checked* with; which pairs to check is the theme's business.
    pub fn contrast(self, other: Color) -> f32 {
        let (a, b) = (self.luminance(), other.luminance());
        (a.max(b) + 0.05) / (a.min(b) + 0.05)
    }

    /// `self` moved toward `toward` in steps of 0.05, starting `from` of
    /// the way there, until it clears `ratio` on `on` — the one loop
    /// behind "an accent that can be seen on this base". Reaches
    /// `toward` itself at the end, so pick one that clears the ratio on
    /// its own (black or white on any base does), and the cap is a floor
    /// rather than a give-up. A colour that already reads at `from` comes
    /// back as that mix, so `from = 0.0` paints one verbatim.
    pub fn toward_contrast(self, toward: Color, on: Color, ratio: f32, from: f32) -> Color {
        let mut t = from.clamp(0.0, 1.0);
        loop {
            let c = self.mix(toward, t);
            if t >= 1.0 || c.contrast(on) >= ratio {
                return c;
            }
            t = (t + 0.05).min(1.0);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A mix keeps its own alpha, lands on each end at 0 and 1, and is
    /// the arithmetic a palette is built with.
    #[test]
    fn a_mix_walks_between_two_colors() {
        let a = Color::rgba(0.2, 0.4, 0.6, 0.5);
        assert_eq!(a.mix(Color::WHITE, 0.0), a);
        let end = a.mix(Color::WHITE, 1.0);
        assert_eq!((end.r, end.g, end.b), (1.0, 1.0, 1.0));
        assert_eq!(end.a, 0.5, "the alpha is this colour's, not the other's");
        let half = a.mix(Color::BLACK, 0.5);
        assert!((half.r - 0.1).abs() < 1e-6 && (half.b - 0.3).abs() < 1e-6);
    }

    /// Black on white is WCAG's 21:1, a colour on itself 1:1, and the
    /// ratio does not care which side is which.
    #[test]
    fn contrast_is_wcags_ratio_and_symmetric() {
        assert!((Color::BLACK.contrast(Color::WHITE) - 21.0).abs() < 1e-3);
        assert!((Color::WHITE.contrast(Color::BLACK) - 21.0).abs() < 1e-3);
        let c = Color::hex(0x3b5bd4ff);
        assert!((c.contrast(c) - 1.0).abs() < 1e-6);
        assert_eq!(c.contrast(Color::WHITE), Color::WHITE.contrast(c));
    }

    /// The loop stops at the first step that reads, paints a colour that
    /// already reads verbatim when started from 0, and ends on `toward`
    /// when nothing short of it does.
    #[test]
    fn toward_contrast_stops_at_the_first_step_that_reads() {
        let navy = Color::hex(0x1e2a4aff);
        let dark = Color::hex(0x1a1d27ff);
        assert!(
            navy.contrast(dark) < 1.5,
            "the case: an accent near the base"
        );
        let ink = navy.toward_contrast(Color::WHITE, dark, 3.0, 0.0);
        assert!(ink.contrast(dark) >= 3.0);
        assert!(
            ink.contrast(dark) < 3.5,
            "the first step that clears, not a leap"
        );
        let already = Color::hex(0xf5d67fff);
        assert_eq!(
            already.toward_contrast(Color::WHITE, dark, 3.0, 0.0),
            already
        );
        assert_eq!(
            navy.toward_contrast(Color::WHITE, dark, 30.0, 0.0),
            navy.mix(Color::WHITE, 1.0)
        );
        // `from` is where it starts: a start that already reads is the
        // answer, even when a smaller mix would have read too.
        let lifted = navy.toward_contrast(Color::WHITE, dark, 3.0, 0.35);
        assert_eq!(lifted, navy.mix(Color::WHITE, 0.35));
        assert!(ink.r < lifted.r, "from 0 the loop stopped short of 0.35");
    }

    /// The number a contrast decision is made from: white is 1, black 0,
    /// and green outweighs blue at the same channel value — which is the
    /// whole reason it is not the average of the three.
    #[test]
    fn luminance_is_weighted_the_way_eyes_are() {
        assert!((Color::WHITE.luminance() - 1.0).abs() < 1e-4);
        assert!(Color::BLACK.luminance().abs() < 1e-6);
        let green = Color::rgb8(0, 255, 0).luminance();
        let blue = Color::rgb8(0, 0, 255).luminance();
        assert!(green > blue * 5.0, "{green} vs {blue}");
        // The two accents the stock button's split was checked against.
        assert!(Color::hex(0x007affff).luminance() < 0.4, "macOS blue");
        assert!(Color::hex(0xffc409ff).luminance() > 0.4, "macOS yellow");
    }
}
