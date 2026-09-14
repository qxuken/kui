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
use crate::value::Value;

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
        .map(|e| match e.payload.get("kind").and_then(Value::as_str) {
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
    liga_font::build()
}

mod liga_font {
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

    fn head() -> Vec<u8> {
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
        w.u16(0); // macStyle
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

    fn os2() -> Vec<u8> {
        let mut w = W(Vec::new());
        w.u16(4); // version
        w.i16(500); // xAvgCharWidth
        w.u16(400); // usWeightClass
        w.u16(5); // usWidthClass
        w.u16(0); // fsType
        for v in [650, 600, 0, 75, 650, 600, 0, 350, 50, 350] {
            w.i16(v); // subscript / superscript / strikeout metrics
        }
        w.i16(0); // sFamilyClass
        w.bytes(&[0; 10]); // panose
        w.u32(1); // ulUnicodeRange1: Basic Latin
        w.u32(0);
        w.u32(0);
        w.u32(0);
        w.bytes(b"KUI "); // achVendID
        w.u16(0x0040); // fsSelection: REGULAR
        w.u16(FIRST); // usFirstCharIndex
        w.u16(LAST); // usLastCharIndex
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
    /// share the square.
    fn cmap() -> Vec<u8> {
        let chars = usize::from(LAST - FIRST + 1);
        let segments: u16 = 2;
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
        w.u16(4); // searchRange
        w.u16(1); // entrySelector
        w.u16(0); // rangeShift
        w.u16(LAST); // endCode
        w.u16(0xFFFF);
        w.u16(0); // reservedPad
        w.u16(FIRST); // startCode
        w.u16(0xFFFF);
        w.i16(0); // idDelta: the array's ids are final
        w.i16(1);
        w.u16(2 * segments); // idRangeOffset: the glyph array starts right after
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

    fn name() -> Vec<u8> {
        let strings: [(u16, &str); 4] = [
            (1, "Kui Liga"),
            (2, "Regular"),
            (4, "Kui Liga"),
            (6, "KuiLiga"),
        ];
        let mut w = W(Vec::new());
        w.u16(0); // format
        w.u16(strings.len() as u16);
        w.u16(6 + 12 * strings.len() as u16); // stringOffset
        let mut pool = W(Vec::new());
        for (id, s) in strings {
            let start = pool.0.len() as u16;
            for unit in s.encode_utf16() {
                pool.u16(unit);
            }
            w.u16(3); // Windows
            w.u16(1); // Unicode BMP
            w.u16(0x0409); // en-US
            w.u16(id);
            w.u16((s.len() * 2) as u16);
            w.u16(start);
        }
        w.bytes(&pool.0);
        w.0
    }

    fn post() -> Vec<u8> {
        let mut w = W(Vec::new());
        w.u32(0x0003_0000); // no glyph names
        w.u32(0); // italicAngle
        w.i16(-100); // underlinePosition
        w.i16(50); // underlineThickness
        w.u32(0); // isFixedPitch
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

    pub(super) fn build() -> Vec<u8> {
        let (glyf, loca) = glyf_and_loca();
        // The directory wants its records sorted by tag.
        let tables: [(&[u8; 4], Vec<u8>); 11] = [
            (b"GSUB", gsub()),
            (b"OS/2", os2()),
            (b"cmap", cmap()),
            (b"glyf", glyf),
            (b"head", head()),
            (b"hhea", hhea()),
            (b"hmtx", hmtx()),
            (b"loca", loca),
            (b"maxp", maxp()),
            (b"name", name()),
            (b"post", post()),
        ];
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
