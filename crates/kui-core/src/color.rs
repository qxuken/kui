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
