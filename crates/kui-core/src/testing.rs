//! The headless driver the crate's own tests drive a [`Core`] with. Test
//! infrastructure, behind the `conformance` feature like the corpus, so no
//! shipped binary carries it; the self dev-dependency turns it on for
//! `cargo test`.
//!
//! Every integration test used to re-derive these at the shallowest
//! interface there is — three `handle_input` calls per click, in nine
//! named copies and seventy-odd inline triples — so a change to the click
//! protocol was a hunt through the test corpus. These are the verbs once:
//! input as a driver sends it, and the two readbacks every test wants (the
//! `kind` of each event, the quads of one kind). They are plain functions
//! over a borrowed `Core` rather than a struct, since a test already holds
//! the core and builds its own frames.

use crate::display::{Quad, QuadKind};
use crate::geom::Vec2;
use crate::input::{EditKey, InputEvent, KeyCode, KeyMods, KeyPress, Mods, UiEvent};
use crate::runtime::Core;

/// A primary click at `at`: the pointer moves there, presses once and
/// releases. Every event the three inputs produced, in order — a test
/// that wants only the release's reads the tail.
pub fn click(core: &mut Core, at: Vec2) -> Vec<UiEvent> {
    let mut out = core.handle_input(InputEvent::CursorMoved(at));
    out.extend(core.handle_input(InputEvent::mouse_down(1)));
    out.extend(core.handle_input(InputEvent::mouse_up()));
    out
}

/// [`click`] by coordinates.
pub fn click_at(core: &mut Core, x: f32, y: f32) -> Vec<UiEvent> {
    click(core, Vec2::new(x, y))
}

/// The pointer moves to `at` and presses, and stays down: the start of a
/// drag. [`release`] ends it.
pub fn press(core: &mut Core, at: Vec2) -> Vec<UiEvent> {
    let mut out = core.handle_input(InputEvent::CursorMoved(at));
    out.extend(core.handle_input(InputEvent::mouse_down(1)));
    out
}

/// The primary button released where the pointer is.
pub fn release(core: &mut Core) -> Vec<UiEvent> {
    core.handle_input(InputEvent::mouse_up())
}

/// The pointer moves to `at`, nothing pressed.
pub fn hover(core: &mut Core, at: Vec2) -> Vec<UiEvent> {
    core.handle_input(InputEvent::CursorMoved(at))
}

/// A Tab (or Shift-Tab) on the editor channel — the one every driver
/// sends from `KeyPress::edit_event`, and the one the Tab ring reads.
pub fn tab(core: &mut Core, shift: bool) -> Vec<UiEvent> {
    edit_key(
        core,
        EditKey::Tab,
        Mods {
            shift,
            ..Mods::default()
        },
    )
}

/// An editing key on the editor channel.
pub fn edit_key(core: &mut Core, key: EditKey, mods: Mods) -> Vec<UiEvent> {
    core.handle_input(InputEvent::Key(key, mods))
}

/// Feeds every input in turn; every event they produced, in order.
pub fn drive(core: &mut Core, events: &[InputEvent]) -> Vec<UiEvent> {
    let mut out = Vec::new();
    for ev in events {
        out.extend(core.handle_input(ev.clone()));
    }
    out
}

/// The `kind` of each event, in order: what a payload map's `kind` says,
/// a bare string payload as itself, and `"?"` for anything else.
pub fn kinds(evs: &[UiEvent]) -> Vec<&str> {
    evs.iter()
        .map(|e| match e.kind() {
            Some(k) => k,
            None => e.payload.as_str().unwrap_or("?"),
        })
        .collect()
}

/// The code of each warning, in order.
pub fn codes(ws: &[crate::diag::Warning]) -> Vec<&'static str> {
    ws.iter().map(|w| w.code).collect()
}

/// The last frame's quads of one kind, in emission order.
pub fn quads_of(core: &mut Core, kind: QuadKind) -> Vec<Quad> {
    let (dl, _) = core.output();
    dl.quads
        .iter()
        .filter(|q| q.kind == kind)
        .cloned()
        .collect()
}

/// The last frame's solid quads (fills and borders), in emission order.
pub fn solids(core: &mut Core) -> Vec<Quad> {
    quads_of(core, QuadKind::Solid)
}

/// A raw key going down, unmodified — `InputEvent::KeyDown`, the keymap
/// channel, not the editing one [`edit_key`] drives — as the event, for a
/// [`drive`] batch.
pub fn key_press(code: KeyCode) -> InputEvent {
    InputEvent::KeyDown(KeyPress::new(code, KeyMods::default()))
}

/// The same key coming back up, as the event.
pub fn key_release(code: KeyCode) -> InputEvent {
    InputEvent::KeyUp(KeyPress::new(code, KeyMods::default()))
}

/// [`key_press`] with modifiers, sent: what the focused sink hears.
pub fn key_down(core: &mut Core, code: KeyCode, mods: KeyMods) -> Vec<UiEvent> {
    core.handle_input(InputEvent::KeyDown(KeyPress::new(code, mods)))
}

/// [`key_release`] with modifiers, sent.
pub fn key_up(core: &mut Core, code: KeyCode, mods: KeyMods) -> Vec<UiEvent> {
    core.handle_input(InputEvent::KeyUp(KeyPress::new(code, mods)))
}

/// What each event names, in order: a bare string payload as itself, else
/// its map's `tag` when that is a string. Events with neither are skipped
/// — the reading a test of "which node heard it" wants, where [`kinds`]
/// is the reading for "what happened".
pub fn tags(evs: &[UiEvent]) -> Vec<&str> {
    evs.iter()
        .filter_map(|e| {
            e.payload
                .as_str()
                .or_else(|| e.payload.get("tag")?.as_str())
        })
        .collect()
}

/// A font file the tests can register without one being installed
/// (backlog AR48): a TrueType face, family **"Kui Liga"**, that maps every
/// printable ASCII character to a filled square and carries one `liga`
/// ligature — `f` followed by `i` shapes as one wider glyph unless the
/// feature is off. Built here rather than checked in as bytes, so the
/// tables are readable and the fixture cannot rot into a blob nobody can
/// regenerate; ~1.5 KB, built in microseconds.
///
/// Five glyphs: `.notdef` (empty), the square, `f`, `i` and `fi`; 1000
/// units per em, ascender 800, descender −200, every advance 500 but the
/// ligature's 700. The GSUB has one lookup, `liga` under `DFLT` and
/// `latn`, so the shaper applies it by default the way a real font's is.
pub fn liga_font() -> Vec<u8> {
    liga_font::build(&liga_font::Face::LIGA)
}

/// The [`liga_font`] fixture as a face of another family — `family`, at
/// `weight` on the CSS scale (the `OS/2` table's `usWeightClass`), italic
/// or upright, and saying it is fixed-pitch or not (the `post` table's
/// `isFixedPitch`) — so a test can stock a font database with families
/// whose faces differ in what the database reads off them (backlog F97).
/// The glyphs are the fixture's whatever the flag says.
pub fn font_face(family: &str, weight: u16, italic: bool, fixed_pitch: bool) -> Vec<u8> {
    liga_font::build(&liga_font::Face {
        family,
        weight,
        italic,
        fixed_pitch,
        ..liga_font::Face::LIGA
    })
}

/// The [`liga_font`] fixture as a proportional face of `family` that maps
/// 字 (U+5B57) to its square besides printable ASCII: a face Han text can
/// fall back to, measured like any other (backlog F98).
pub fn han_face(family: &str) -> Vec<u8> {
    liga_font::build(&liga_font::Face {
        family,
        han: true,
        ..liga_font::Face::LIGA
    })
}

/// A face whose glyph advances cannot be measured (backlog F98): the
/// [`han_face`] fixture, fixed-pitch, without its `head`, `hhea` and
/// `hmtx` tables — no units per em, no horizontal metrics — and without
/// its outlines, the way macOS's GB18030 Bitmap has none of them (it
/// carries Apple's `bhed` and bitmap tables instead, and this carries no
/// bitmaps either). The font database takes it, since its records come
/// from `name`, `OS/2` and `post`; the shaper reads its units per em as 0
/// and so its every advance as infinite.
pub fn unmeasurable_face(family: &str) -> Vec<u8> {
    liga_font::build(&liga_font::Face {
        family,
        fixed_pitch: true,
        han: true,
        metrics: false,
        ..liga_font::Face::LIGA
    })
}

/// A variable face of `family` (backlog F100): the [`liga_font`] fixture,
/// fixed-pitch, whose `OS/2` says weight 400 and which carries a `wght`
/// axis from `wght[0]` to `wght[2]`, its default at `wght[1]`, with the
/// named instances `instances` (subfamily name, coordinate). At the axis's
/// maximum every square's right edge is 100 units further right (`gvar`),
/// so a glyph drawn at the heavy end is 500 units wide where the default's
/// is 400; the advances stay 500. `[100, 100, 150]` with "Regular" at 100
/// and "Bold" at 150 is Berkeley Mono Variable's axis, which is not on the
/// CSS scale its `OS/2` weight is.
pub fn variable_face(
    family: &str,
    italic: bool,
    wght: [u16; 3],
    instances: &[(&str, u16)],
) -> Vec<u8> {
    liga_font::build(&liga_font::Face {
        family,
        italic,
        fixed_pitch: true,
        wght: Some(liga_font::Wght {
            axis: wght,
            instances,
        }),
        ..liga_font::Face::LIGA
    })
}

mod liga_font {
    /// What the tables say the face is: its family, weight and style.
    pub(super) struct Face<'a> {
        pub(super) family: &'a str,
        pub(super) weight: u16,
        pub(super) italic: bool,
        pub(super) fixed_pitch: bool,
        /// Maps 字 (U+5B57) to the square as well as printable ASCII.
        pub(super) han: bool,
        /// Carries `head`, `hhea`, `hmtx` and the outlines; without them
        /// nothing says how wide a glyph is.
        pub(super) metrics: bool,
        /// A `wght` axis (`fvar`, `gvar`): a variable face.
        pub(super) wght: Option<Wght<'a>>,
    }

    /// A variable face's weight axis: minimum, default and maximum, and
    /// the named instances along it.
    pub(super) struct Wght<'a> {
        pub(super) axis: [u16; 3],
        pub(super) instances: &'a [(&'a str, u16)],
    }

    impl Face<'static> {
        pub(super) const LIGA: Self = Self {
            family: "Kui Liga",
            weight: 400,
            italic: false,
            fixed_pitch: false,
            han: false,
            metrics: true,
            wght: None,
        };
    }

    impl Face<'_> {
        /// The `name` table's subfamily, the way a family's faces spell it.
        fn subfamily(&self) -> &'static str {
            match (self.weight >= 600, self.italic) {
                (false, false) => "Regular",
                (true, false) => "Bold",
                (false, true) => "Italic",
                (true, true) => "Bold Italic",
            }
        }
    }

    struct W(Vec<u8>);
    impl W {
        fn u8(&mut self, v: u8) {
            self.0.push(v);
        }
        fn u16(&mut self, v: u16) {
            self.0.extend_from_slice(&v.to_be_bytes());
        }
        fn i16(&mut self, v: i16) {
            self.0.extend_from_slice(&v.to_be_bytes());
        }
        fn u32(&mut self, v: u32) {
            self.0.extend_from_slice(&v.to_be_bytes());
        }
        fn i64(&mut self, v: i64) {
            self.0.extend_from_slice(&v.to_be_bytes());
        }
        fn bytes(&mut self, b: &[u8]) {
            self.0.extend_from_slice(b);
        }
        fn pad4(&mut self) {
            while !self.0.len().is_multiple_of(4) {
                self.0.push(0);
            }
        }
    }

    // Glyph 0 is `.notdef`, the empty one.
    const BOX: u16 = 1;
    const F: u16 = 2;
    const I: u16 = 3;
    const FI: u16 = 4;
    const GLYPHS: u16 = 5;
    const FIRST: u16 = 0x20;
    const LAST: u16 = 0x7E;
    /// 字, the one ideograph a `han` face maps.
    const HAN: u16 = 0x5B57;

    /// A simple glyph: one contour, a square from (50, 0) to (`right`, 700).
    fn square(right: i16) -> Vec<u8> {
        let mut w = W(Vec::new());
        w.i16(1); // contours
        w.i16(50);
        w.i16(0);
        w.i16(right);
        w.i16(700);
        w.u16(3); // last point of the contour
        w.u16(0); // no instructions
        for _ in 0..4 {
            w.u8(0x01); // on-curve, both coordinates as i16
        }
        // x deltas, then y deltas: (50,0) (50,700) (right,700) (right,0).
        for dx in [50, 0, right - 50, 0] {
            w.i16(dx);
        }
        for dy in [0, 700, 0, -700] {
            w.i16(dy);
        }
        w.pad4();
        w.0
    }

    fn glyf_and_loca() -> (Vec<u8>, Vec<u8>) {
        let glyphs: [Vec<u8>; GLYPHS as usize] = [
            Vec::new(),
            square(450),
            square(450),
            square(450),
            square(650),
        ];
        let mut glyf = W(Vec::new());
        let mut loca = W(Vec::new());
        for g in &glyphs {
            loca.u32(glyf.0.len() as u32);
            glyf.bytes(g);
        }
        loca.u32(glyf.0.len() as u32);
        (glyf.0, loca.0)
    }

    fn head(face: &Face) -> Vec<u8> {
        let mut w = W(Vec::new());
        w.u32(0x0001_0000); // version
        w.u32(0x0001_0000); // fontRevision
        w.u32(0); // checkSumAdjustment (nothing here verifies it)
        w.u32(0x5F0F_3CF5); // magic
        w.u16(0x000B); // flags
        w.u16(1000); // unitsPerEm
        w.i64(0); // created
        w.i64(0); // modified
        w.i16(0); // xMin
        w.i16(0); // yMin
        w.i16(700); // xMax
        w.i16(700); // yMax
        // macStyle: bold, italic.
        w.u16(u16::from(face.weight >= 600) | u16::from(face.italic) << 1);
        w.u16(8); // lowestRecPPEM
        w.i16(2); // fontDirectionHint
        w.i16(1); // indexToLocFormat: long
        w.i16(0); // glyphDataFormat
        w.0
    }

    fn hhea() -> Vec<u8> {
        let mut w = W(Vec::new());
        w.u32(0x0001_0000);
        w.i16(800); // ascender
        w.i16(-200); // descender
        w.i16(0); // lineGap
        w.u16(700); // advanceWidthMax
        w.i16(0); // minLeftSideBearing
        w.i16(0); // minRightSideBearing
        w.i16(700); // xMaxExtent
        w.i16(1); // caretSlopeRise
        w.i16(0); // caretSlopeRun
        w.i16(0); // caretOffset
        for _ in 0..4 {
            w.i16(0);
        }
        w.i16(0); // metricDataFormat
        w.u16(GLYPHS); // numberOfHMetrics
        w.0
    }

    fn maxp() -> Vec<u8> {
        let mut w = W(Vec::new());
        w.u32(0x0001_0000);
        w.u16(GLYPHS);
        w.u16(4); // maxPoints
        w.u16(1); // maxContours
        w.u16(0); // maxCompositePoints
        w.u16(0); // maxCompositeContours
        w.u16(2); // maxZones
        for _ in 0..8 {
            w.u16(0);
        }
        w.0
    }

    fn os2(face: &Face) -> Vec<u8> {
        let mut w = W(Vec::new());
        w.u16(4); // version
        w.i16(500); // xAvgCharWidth
        w.u16(face.weight); // usWeightClass
        w.u16(5); // usWidthClass
        w.u16(0); // fsType
        for v in [650, 600, 0, 75, 650, 600, 0, 350, 50, 350] {
            w.i16(v); // subscript / superscript / strikeout metrics
        }
        w.i16(0); // sFamilyClass
        w.bytes(&[0; 10]); // panose
        w.u32(1); // ulUnicodeRange1: Basic Latin
        // ulUnicodeRange2: CJK Unified Ideographs (bit 59) for a `han` face.
        w.u32(u32::from(face.han) << 27);
        w.u32(0);
        w.u32(0);
        w.bytes(b"KUI "); // achVendID
        // fsSelection: ITALIC, BOLD, or REGULAR when neither.
        w.u16(match (face.weight >= 600, face.italic) {
            (false, false) => 0x0040,
            (bold, italic) => u16::from(italic) | u16::from(bold) << 5,
        });
        w.u16(FIRST); // usFirstCharIndex
        w.u16(if face.han { HAN } else { LAST }); // usLastCharIndex
        w.i16(800); // sTypoAscender
        w.i16(-200); // sTypoDescender
        w.i16(0); // sTypoLineGap
        w.u16(800); // usWinAscent
        w.u16(200); // usWinDescent
        w.u32(1); // ulCodePageRange1: Latin 1
        w.u32(0);
        w.i16(500); // sxHeight
        w.i16(700); // sCapHeight
        w.u16(0); // usDefaultChar
        w.u16(0x20); // usBreakChar
        w.u16(2); // usMaxContext: the ligature's length
        debug_assert_eq!(w.0.len(), 96);
        w.0
    }

    fn hmtx() -> Vec<u8> {
        let mut w = W(Vec::new());
        for g in 0..GLYPHS {
            w.u16(if g == FI { 700 } else { 500 });
            w.i16(0);
        }
        w.0
    }

    /// One format-4 subtable: every printable ASCII character through the
    /// glyph array, so `f` and `i` can be their own glyphs while the rest
    /// share the square, and a `han` face's 字 to the square by delta.
    fn cmap(face: &Face) -> Vec<u8> {
        let chars = usize::from(LAST - FIRST + 1);
        // By endCode: ASCII, 字 when mapped, and the closing 0xFFFF.
        let han: &[u16] = if face.han { &[HAN] } else { &[] };
        let segments = 2 + han.len() as u16;
        let entry_selector = segments.ilog2() as u16;
        let search_range = 2 << entry_selector;
        let mut w = W(Vec::new());
        w.u16(0); // version
        w.u16(1); // one encoding record
        w.u16(3); // Windows
        w.u16(1); // Unicode BMP
        w.u32(12); // its subtable follows the header
        let length = 14 + 8 * usize::from(segments) + 2 + 2 * chars;
        w.u16(4); // format
        w.u16(length as u16);
        w.u16(0); // language
        w.u16(segments * 2);
        w.u16(search_range);
        w.u16(entry_selector);
        w.u16(segments * 2 - search_range); // rangeShift
        w.u16(LAST); // endCode
        han.iter().for_each(|&c| w.u16(c));
        w.u16(0xFFFF);
        w.u16(0); // reservedPad
        w.u16(FIRST); // startCode
        han.iter().for_each(|&c| w.u16(c));
        w.u16(0xFFFF);
        w.i16(0); // idDelta: the array's ids are final
        han.iter().for_each(|&c| w.u16(BOX.wrapping_sub(c)));
        w.i16(1);
        w.u16(2 * segments); // idRangeOffset: the glyph array starts right after
        han.iter().for_each(|_| w.u16(0));
        w.u16(0);
        for c in FIRST..=LAST {
            w.u16(match c {
                0x66 => F,
                0x69 => I,
                _ => BOX,
            });
        }
        debug_assert_eq!(w.0.len(), 12 + length);
        w.0
    }

    fn name(face: &Face) -> Vec<u8> {
        let full = match face.subfamily() {
            "Regular" => face.family.to_string(),
            sub => format!("{} {sub}", face.family),
        };
        let postscript: String = full.chars().filter(|c| !c.is_whitespace()).collect();
        let mut strings: Vec<(u16, &str)> = vec![
            (1, face.family),
            (2, face.subfamily()),
            (4, &full),
            (6, &postscript),
        ];
        // A variable face's axis name at 256 and its instances' after it.
        if let Some(wght) = &face.wght {
            strings.push((AXIS_NAME, "Weight"));
            for (i, (name, _)) in wght.instances.iter().enumerate() {
                strings.push((AXIS_NAME + 1 + i as u16, name));
            }
        }
        let mut w = W(Vec::new());
        w.u16(0); // format
        w.u16(strings.len() as u16);
        w.u16(6 + 12 * strings.len() as u16); // stringOffset
        let mut pool = W(Vec::new());
        for (id, s) in strings.iter().copied() {
            let start = pool.0.len() as u16;
            for unit in s.encode_utf16() {
                pool.u16(unit);
            }
            w.u16(3); // Windows
            w.u16(1); // Unicode BMP
            w.u16(0x0409); // en-US
            w.u16(id);
            w.u16((s.encode_utf16().count() * 2) as u16);
            w.u16(start);
        }
        w.bytes(&pool.0);
        w.0
    }

    fn post(face: &Face) -> Vec<u8> {
        let mut w = W(Vec::new());
        w.u32(0x0003_0000); // no glyph names
        // italicAngle, 16.16: an italic leans 12 degrees to the right.
        w.u32(if face.italic {
            (-12i32 << 16) as u32
        } else {
            0
        });
        w.i16(-100); // underlinePosition
        w.i16(50); // underlineThickness
        w.u32(u32::from(face.fixed_pitch)); // isFixedPitch
        for _ in 0..4 {
            w.u32(0);
        }
        w.0
    }

    /// `liga` under `DFLT` and `latn`: one ligature-substitution lookup,
    /// `f` + `i` → `fi`.
    fn gsub() -> Vec<u8> {
        let mut w = W(Vec::new());
        w.u32(0x0001_0000);
        w.u16(10); // ScriptList
        w.u16(36); // FeatureList
        w.u16(50); // LookupList
        // ScriptList: two records sharing one Script table.
        w.u16(2);
        w.bytes(b"DFLT");
        w.u16(14);
        w.bytes(b"latn");
        w.u16(14);
        w.u16(4); // Script: DefaultLangSys at 4
        w.u16(0); // no LangSysRecords
        w.u16(0); // LangSys: lookupOrder
        w.u16(0xFFFF); // no required feature
        w.u16(1); // one feature
        w.u16(0); // FeatureIndex 0
        // FeatureList.
        w.u16(1);
        w.bytes(b"liga");
        w.u16(8);
        w.u16(0); // Feature: no params
        w.u16(1); // one lookup
        w.u16(0); // LookupListIndex 0
        // LookupList.
        w.u16(1);
        w.u16(4);
        w.u16(4); // Lookup: type 4, ligature substitution
        w.u16(0); // flag
        w.u16(1); // one subtable
        w.u16(8); // at 8
        w.u16(1); // LigatureSubstFormat1
        w.u16(8); // Coverage at 8
        w.u16(1); // one LigatureSet
        w.u16(14); // at 14
        w.u16(1); // Coverage format 1
        w.u16(1); // one glyph
        w.u16(F);
        w.u16(1); // LigatureSet: one Ligature
        w.u16(4); // at 4
        w.u16(FI); // Ligature: the glyph
        w.u16(2); // two components
        w.u16(I); // the second
        debug_assert_eq!(w.0.len(), 86);
        w.0
    }

    /// The `name` id of a variable face's axis; its instances' follow.
    const AXIS_NAME: u16 = 256;

    fn fixed(v: u16) -> u32 {
        u32::from(v) << 16
    }

    /// One `wght` axis and its named instances.
    fn fvar(wght: &Wght) -> Vec<u8> {
        let mut w = W(Vec::new());
        w.u32(0x0001_0000);
        w.u16(16); // axesArrayOffset
        w.u16(2); // reserved
        w.u16(1); // axisCount
        w.u16(20); // axisSize
        w.u16(wght.instances.len() as u16);
        w.u16(8); // instanceSize: name id, flags, one coordinate
        w.bytes(b"wght");
        for v in wght.axis {
            w.u32(fixed(v)); // min, default, max
        }
        w.u16(0); // flags
        w.u16(AXIS_NAME);
        for (i, &(_, at)) in wght.instances.iter().enumerate() {
            w.u16(AXIS_NAME + 1 + i as u16);
            w.u16(0);
            w.u32(fixed(at));
        }
        w.0
    }

    /// Every square's right edge 100 units further right at the axis's
    /// maximum (a peak of +1.0), the phantom points — the advance — still.
    fn gvar() -> Vec<u8> {
        // One tuple over all points: the square's four and the four
        // phantoms. x: 0 0 100 100 0 0 0 0, y: all 0.
        let mut data = W(Vec::new());
        data.u16(1); // tupleVariationCount
        data.u16(10); // dataOffset: past this header and the tuple's
        data.u16(7); // variationDataSize
        data.u16(0xA000); // EMBEDDED_PEAK_TUPLE | PRIVATE_POINT_NUMBERS
        data.u16(0x4000); // peak: +1.0 in F2Dot14
        data.u8(0); // point numbers: all of them
        data.bytes(&[0x81, 0x01, 100, 100, 0x83]); // x deltas
        data.u8(0x87); // y deltas: eight zeros
        data.pad4();
        let per_glyph = data.0;
        let mut w = W(Vec::new());
        w.u16(1);
        w.u16(0);
        w.u16(1); // axisCount
        w.u16(0); // sharedTupleCount
        let array = 20 + 4 * (u32::from(GLYPHS) + 1);
        w.u32(array); // sharedTuplesOffset: none, at the array
        w.u16(GLYPHS);
        w.u16(1); // flags: 32-bit offsets
        w.u32(array); // glyphVariationDataArrayOffset
        // .notdef has no variation data; the squares each have one.
        w.u32(0);
        for g in 1..=GLYPHS {
            w.u32((per_glyph.len() * usize::from(g - 1)) as u32);
        }
        for _ in 1..GLYPHS {
            w.bytes(&per_glyph);
        }
        w.0
    }

    /// What a face without metrics leaves out: what says how wide a glyph
    /// is, and the outlines `loca` finds by `head`'s index format.
    const METRICS: [&[u8; 4]; 5] = [b"glyf", b"head", b"hhea", b"hmtx", b"loca"];

    fn checksum(table: &[u8]) -> u32 {
        table
            .chunks(4)
            .map(|c| {
                let mut word = [0u8; 4];
                word[..c.len()].copy_from_slice(c);
                u32::from_be_bytes(word)
            })
            .fold(0u32, u32::wrapping_add)
    }

    pub(super) fn build(face: &Face) -> Vec<u8> {
        let (glyf, loca) = glyf_and_loca();
        // The directory wants its records sorted by tag.
        let mut tables: Vec<(&[u8; 4], Vec<u8>)> = [
            (b"GSUB", gsub()),
            (b"OS/2", os2(face)),
            (b"cmap", cmap(face)),
            (b"glyf", glyf),
            (b"head", head(face)),
            (b"hhea", hhea()),
            (b"hmtx", hmtx()),
            (b"loca", loca),
            (b"maxp", maxp()),
            (b"name", name(face)),
            (b"post", post(face)),
        ]
        .into_iter()
        .filter(|(tag, _)| face.metrics || !METRICS.contains(tag))
        .collect();
        if let Some(wght) = &face.wght {
            tables.push((b"fvar", fvar(wght)));
            tables.push((b"gvar", gvar()));
            tables.sort_by_key(|(tag, _)| **tag);
        }
        let n = tables.len() as u16;
        let mut w = W(Vec::new());
        w.u32(0x0001_0000);
        w.u16(n);
        let mut search_range = 16;
        let mut entry_selector = 0;
        while search_range * 2 <= 16 * n {
            search_range *= 2;
            entry_selector += 1;
        }
        w.u16(search_range);
        w.u16(entry_selector);
        w.u16(16 * n - search_range);
        let mut offset = 12 + 16 * usize::from(n);
        for (tag, table) in &tables {
            w.bytes(*tag);
            w.u32(checksum(table));
            w.u32(offset as u32);
            w.u32(table.len() as u32);
            offset += table.len().div_ceil(4) * 4;
        }
        for (_, table) in &tables {
            w.bytes(table);
            w.pad4();
        }
        w.0
    }
}
