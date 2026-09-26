//! Bold and regular in the family a style names (backlog F100).
//!
//! kui asks cosmic-text for two weights, 400 and 700 on the CSS scale, and
//! cosmic-text takes a family's face for a weight only when the face is
//! that weight or is variable with a `wght` axis spanning it. Otherwise it
//! looks in the platform's fallback list and then in every face installed,
//! so bold of a family with no bold face was drawn in some other family —
//! a proportional one, one glyph to a cell in a terminal. It then reads
//! the CSS number as the `wght` coordinate, which is right only for a face
//! whose axis is on that scale: Berkeley Mono Variable's runs from 100
//! (its Regular) to 150 (its Bold), so 400 clamped to the Bold and 700
//! fell outside it altogether.
//!
//! So each registered family is asked at weights it has ([`Weights`]):
//! a weight it has no face for is asked at its nearest face's, and bold
//! asked of a lighter face marks its glyphs [`SYNTHETIC_BOLD`]. The
//! rasterizer reads a face's `wght` coordinate for a CSS weight off its
//! named instances when the axis is not on the CSS scale ([`WghtAxis`]),
//! draws a marked glyph at the axis's bold, and emboldens the outline of
//! a marked glyph whose face has no heavier instance.

use cosmic_text::{Attrs, CacheKeyFlags, Weight, fontdb};

/// kui's own bit in a glyph's cache key (backlog F100): the glyph's span
/// asked for bold and its family was matched at a lighter face, so the
/// rasterizer draws it bold — at the face's bold instance when it is
/// variable, else emboldened. cosmic-text carries the key's flags through
/// shaping untouched and uses the low three bits itself; a test holds
/// that this one stays clear of every flag it defines.
pub(crate) const SYNTHETIC_BOLD: CacheKeyFlags = CacheKeyFlags::from_bits_retain(1 << 31);

/// The weights a family is asked at for kui's regular (400) and bold
/// (700): each the weight itself when the family has a face cosmic-text
/// takes for it, else the weight of the family's nearest face, so the
/// family is never passed over for another (backlog F100).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Weights {
    pub(crate) regular: Weight,
    pub(crate) bold: Weight,
}

impl Weights {
    /// The weights as asked: a generic family, and a family whose faces
    /// cover both.
    pub(crate) const CSS: Self = Self {
        regular: Weight::NORMAL,
        bold: Weight::BOLD,
    };

    /// What `family`'s faces in `db` cover. A family with no face in the
    /// database is asked as [`Self::CSS`]; cosmic-text falls back for it
    /// whatever it is asked.
    pub(crate) fn of(db: &fontdb::Database, family: &str) -> Self {
        let faces: Vec<(u16, Option<(f32, f32)>)> = db
            .faces()
            .filter(|face| face.families.iter().any(|(name, _)| name == family))
            .map(|face| (face.weight.0, wght_range(db, face.id)))
            .collect();
        if faces.is_empty() {
            return Self::CSS;
        }
        // cosmic-text's own test, `FontFallbackIter::default_font_match_key`:
        // the face's weight, or a `wght` axis that spans the one asked.
        let serve = |asked: u16| -> Weight {
            let covered = faces.iter().any(|&(weight, range)| {
                weight == asked
                    || range.is_some_and(|(lo, hi)| (lo..=hi).contains(&f32::from(asked)))
            });
            if covered {
                return Weight(asked);
            }
            Weight(css_match(asked, faces.iter().map(|&(weight, _)| weight)).unwrap_or(asked))
        };
        Self {
            regular: serve(Weight::NORMAL.0),
            bold: serve(Weight::BOLD.0),
        }
    }

    /// `attrs` at the family's regular or its bold; a bold asked of a
    /// face lighter than semibold carries [`SYNTHETIC_BOLD`].
    pub(crate) fn apply(self, attrs: Attrs<'_>, bold: bool) -> Attrs<'_> {
        if !bold {
            return attrs.weight(self.regular);
        }
        let flags = attrs.cache_key_flags;
        let attrs = attrs.weight(self.bold);
        if self.bold.0 < 600 {
            attrs.cache_key_flags(flags | SYNTHETIC_BOLD)
        } else {
            attrs
        }
    }
}

/// The face weight CSS font matching takes for `asked` among `weights`
/// (CSS Fonts 4, § 5.2, step 4): for 400 to 500, the weights from it up
/// to 500 in ascending order, then those below it in descending order,
/// then those above 500; below 400, those at or below it descending, then
/// those above ascending; above 500, those at or above it ascending, then
/// those below descending. So regular with faces at 300 and 500 is the
/// 500, and bold with faces at 500 and 900 the 900 (RG59).
fn css_match(asked: u16, weights: impl Iterator<Item = u16> + Clone) -> Option<u16> {
    let up = |lo: u16, hi: u16| weights.clone().filter(|w| (lo..=hi).contains(w)).min();
    let down = |lo: u16, hi: u16| weights.clone().filter(|w| (lo..=hi).contains(w)).max();
    match asked {
        400..=500 => up(asked, 500)
            .or_else(|| down(0, asked - 1))
            .or_else(|| up(501, u16::MAX)),
        0..400 => down(0, asked).or_else(|| up(asked + 1, u16::MAX)),
        _ => up(asked, u16::MAX).or_else(|| down(0, asked - 1)),
    }
}

/// The span of a face's `wght` axis, if it is variable along one.
fn wght_range(db: &fontdb::Database, id: fontdb::ID) -> Option<(f32, f32)> {
    use cosmic_text::skrifa::{FontRef, MetadataProvider, Tag};
    db.with_face_data(id, |data, index| {
        let font = FontRef::from_index(data, index).ok()?;
        let axis = font.axes().get_by_tag(Tag::new(b"wght"))?;
        Some((axis.min_value(), axis.max_value()))
    })
    .flatten()
}

/// A face's `wght` axis as the rasterizer reads it: where a CSS weight
/// lies along it. On the CSS scale — the axis's default where the face's
/// `OS/2` weight says — a weight is its own coordinate, as cosmic-text
/// takes it. Otherwise the coordinate is read off the named instances
/// whose names say a weight ("Regular", "Bold", "SemiBold"…), with the
/// default standing for the `OS/2` weight when no instance names that
/// weight, and between two of them it is linear; past the last it stays
/// there (backlog F100). An instance goes first: a face whose `OS/2`
/// weight is 400 and whose default is its Thin, with a Regular at 400,
/// draws regular at the Regular (RG59).
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct WghtAxis {
    min: f32,
    max: f32,
    /// (CSS weight, coordinate), by weight; empty on the CSS scale.
    points: Vec<(f32, f32)>,
}

impl WghtAxis {
    /// The axis of `font`, a face whose `OS/2` weight is `weight`; `None`
    /// without one.
    pub(crate) fn read(font: swash::FontRef<'_>, weight: u16) -> Option<Self> {
        let axis = font
            .variations()
            .find_by_tag(swash::Tag::from_be_bytes(*b"wght"))?;
        let (min, max, default) = (axis.min_value(), axis.max_value(), axis.default_value());
        let mut points: Vec<(f32, f32)> = Vec::new();
        if (default - f32::from(weight)).abs() >= 0.5 {
            for instance in font.instances() {
                let Some(css) = instance
                    .name(None)
                    .and_then(|name| css_weight(&name.to_string()))
                else {
                    continue;
                };
                let Some(at) = instance.values().nth(axis.index()) else {
                    continue;
                };
                if !points.iter().any(|&(w, _)| w == css) {
                    points.push((css, at));
                }
            }
            if !points.iter().any(|&(w, _)| w == f32::from(weight)) {
                points.push((f32::from(weight), default));
            }
            points.sort_by(|a, b| a.0.total_cmp(&b.0));
        }
        Some(Self { min, max, points })
    }

    /// The coordinate for `css`, within the axis.
    pub(crate) fn coordinate(&self, css: f32) -> f32 {
        let at = match self.points[..] {
            [] => css,
            [(_, only)] => only,
            [(w0, c0), ..] if css <= w0 => c0,
            [.., (wn, cn)] if css >= wn => cn,
            _ => self
                .points
                .windows(2)
                .find(|pair| css <= pair[1].0)
                .map_or(css, |pair| {
                    let ((w0, c0), (w1, c1)) = (pair[0], pair[1]);
                    c0 + (c1 - c0) * (css - w0) / (w1 - w0)
                }),
        };
        at.clamp(self.min, self.max)
    }
}

/// The CSS weight a named instance's subfamily says ("Bold", "Semi Bold
/// Italic", "ExtraLight"), if it says one.
fn css_weight(name: &str) -> Option<f32> {
    let key: String = name
        .chars()
        .filter(|c| c.is_ascii_alphanumeric())
        .collect::<String>()
        .to_ascii_lowercase()
        .replace("italic", "")
        .replace("oblique", "");
    Some(match key.as_str() {
        "thin" | "hairline" => 100.0,
        "extralight" | "ultralight" => 200.0,
        "light" => 300.0,
        "" | "regular" | "normal" | "book" | "roman" => 400.0,
        "medium" => 500.0,
        "semibold" | "demibold" => 600.0,
        "bold" => 700.0,
        "extrabold" | "ultrabold" => 800.0,
        "black" | "heavy" => 900.0,
        _ => return None,
    })
}

/// How far a synthesized bold grows an outline, in pixels at `size`: a
/// twenty-fourth of the size up to 9 px, a thirty-second from 36 px, and
/// between the two in between — the ratios Skia's fake bold strokes with.
pub(crate) fn embolden_strength(size: f32) -> f32 {
    let t = ((size - 9.0) / (36.0 - 9.0)).clamp(0.0, 1.0);
    size * (1.0 / 24.0 + (1.0 / 32.0 - 1.0 / 24.0) * t)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::{font_face, variable_face};
    use cosmic_text::fontdb::{Database, Source};

    fn db(faces: Vec<Vec<u8>>) -> Database {
        let mut db = Database::new();
        for bytes in faces {
            db.load_font_source(Source::Binary(std::sync::Arc::new(bytes)));
        }
        db
    }

    const BERKELEY: [(&str, u16); 2] = [("Regular", 100), ("Bold", 150)];

    #[test]
    fn the_flag_is_none_of_cosmic_texts() {
        assert!(!CacheKeyFlags::all().intersects(SYNTHETIC_BOLD));
        assert_eq!(SYNTHETIC_BOLD.bits(), 1 << 31);
    }

    #[test]
    fn a_family_is_asked_at_weights_it_has() {
        let db = db(vec![
            font_face("Kui Both", 400, false, true),
            font_face("Kui Both", 700, false, true),
            font_face("Kui Regular", 400, false, true),
            font_face("Kui Light", 300, false, true),
            variable_face("Kui Var", false, [100, 100, 150], &BERKELEY),
            variable_face("Kui Css", false, [100, 400, 900], &[]),
        ]);
        let w = |family| Weights::of(&db, family);
        assert_eq!(w("Kui Both"), Weights::CSS);
        assert_eq!(w("Kui Css"), Weights::CSS, "the axis spans 400 and 700");
        assert_eq!(w("No Such Family"), Weights::CSS);
        let only = |weight| Weights {
            regular: Weight(weight),
            bold: Weight(weight),
        };
        assert_eq!(w("Kui Regular"), only(400));
        assert_eq!(w("Kui Light"), only(300));
        // 400 is its OS/2 weight; 700 is past its axis, 100 to 150.
        assert_eq!(w("Kui Var"), only(400));
    }

    #[test]
    fn bold_of_a_lighter_face_is_marked_and_bold_of_a_bold_face_is_not() {
        let base = Attrs::new();
        let marked = |w: Weights| w.apply(base.clone(), true).cache_key_flags;
        assert_eq!(marked(Weights::CSS), CacheKeyFlags::empty());
        let regular = Weights {
            regular: Weight::NORMAL,
            bold: Weight::NORMAL,
        };
        assert_eq!(marked(regular), SYNTHETIC_BOLD);
        assert_eq!(regular.apply(base.clone(), true).weight, Weight::NORMAL);
        assert_eq!(
            regular.apply(base.clone(), false).cache_key_flags,
            CacheKeyFlags::empty()
        );
        let semibold = Weights {
            regular: Weight::NORMAL,
            bold: Weight::SEMIBOLD,
        };
        assert_eq!(marked(semibold), CacheKeyFlags::empty());
        // A flag already set stays.
        let italic = base.cache_key_flags(CacheKeyFlags::FAKE_ITALIC);
        assert_eq!(
            regular.apply(italic, true).cache_key_flags,
            CacheKeyFlags::FAKE_ITALIC | SYNTHETIC_BOLD
        );
    }

    fn axis(bytes: &[u8]) -> WghtAxis {
        let font = swash::FontRef::from_index(bytes, 0).expect("the fixture parses");
        WghtAxis::read(font, 400).expect("a wght axis")
    }

    /// Berkeley Mono Variable's axis: its Regular at 100, its Bold at 150,
    /// its `OS/2` weight 400. Regular is 100 and bold 150, not the 150 and
    /// nothing a CSS number clamped to the axis gives.
    #[test]
    fn an_axis_off_the_css_scale_is_read_off_its_instances() {
        let a = axis(&variable_face("Kui Var", false, [100, 100, 150], &BERKELEY));
        assert_eq!(a.coordinate(400.0), 100.0);
        assert_eq!(a.coordinate(700.0), 150.0);
        assert_eq!(a.coordinate(550.0), 125.0);
        assert_eq!(a.coordinate(100.0), 100.0);
        assert_eq!(a.coordinate(900.0), 150.0);
        // Without a named bold, the default is all there is.
        let bare = axis(&variable_face("Kui Bare", false, [100, 100, 150], &[]));
        assert_eq!(bare.coordinate(700.0), 100.0);
    }

    /// RG59: a named instance at the `OS/2` weight is where that weight
    /// draws, not the default: a face whose `OS/2` weight is 400 and
    /// whose default is its Thin draws regular at its Regular. The
    /// default's point used to go first and shadow it, and regular drew
    /// Thin.
    #[test]
    fn an_instance_at_the_os2_weight_is_not_shadowed_by_the_default() {
        let a = axis(&variable_face(
            "Kui Thin Default",
            false,
            [100, 100, 900],
            &[("Thin", 100), ("Regular", 400)],
        ));
        assert_eq!(a.coordinate(400.0), 400.0);
        assert_eq!(a.coordinate(100.0), 100.0);
    }

    /// RG59: the nearest face is CSS's (CSS Fonts 4, § 5.2): regular
    /// looks up to 500 before it looks down, and below 400 or above 500
    /// looks away from 400–500 first.
    #[test]
    fn the_nearest_face_is_the_one_css_matching_takes() {
        let m = |asked, weights: &[u16]| css_match(asked, weights.iter().copied());
        assert_eq!(m(400, &[300, 500]), Some(500), "up to 500 first");
        assert_eq!(m(400, &[300, 600]), Some(300), "then down");
        assert_eq!(m(400, &[100, 600]), Some(100), "down before past 500");
        assert_eq!(m(400, &[600, 900]), Some(600));
        assert_eq!(m(700, &[500, 900]), Some(900), "bold looks up first");
        assert_eq!(m(700, &[300, 500]), Some(500), "then down");
        assert_eq!(m(300, &[200, 400]), Some(200), "light looks down first");
        assert_eq!(m(300, &[400, 500]), Some(400));
        assert_eq!(m(400, &[]), None);
        let db = db(vec![
            font_face("Kui Between", 300, false, true),
            font_face("Kui Between", 500, false, true),
        ]);
        assert_eq!(
            Weights::of(&db, "Kui Between"),
            Weights {
                regular: Weight(500),
                bold: Weight(500),
            }
        );
    }

    #[test]
    fn an_axis_on_the_css_scale_is_its_own_coordinate() {
        let a = axis(&variable_face(
            "Kui Css",
            false,
            [200, 400, 700],
            &[("Regular", 400), ("Bold", 700)],
        ));
        assert_eq!(a.coordinate(400.0), 400.0);
        assert_eq!(a.coordinate(700.0), 700.0);
        assert_eq!(a.coordinate(900.0), 700.0);
    }

    #[test]
    fn instance_names_say_their_weight() {
        for (name, css) in [
            ("Regular", Some(400.0)),
            ("Italic", Some(400.0)),
            ("Bold", Some(700.0)),
            ("Bold Italic", Some(700.0)),
            ("SemiBold", Some(600.0)),
            ("Semi Bold", Some(600.0)),
            ("Extra-Light", Some(200.0)),
            ("Heavy", Some(900.0)),
            ("Condensed", None),
        ] {
            assert_eq!(css_weight(name), css, "{name}");
        }
    }
}
