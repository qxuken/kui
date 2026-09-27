//! Registered fonts: installed families by name, font files from bytes,
//! stale handles falling back to sans, shaping through `TextStyle::font`,
//! and what the font database says each family is.

use kui_core::{Core, NodeSpec, Size, TextStyle};

fn glyph_quads(core: &mut Core, style: TextStyle) -> usize {
    let mut ui = core.frame(Size::new(400.0, 100.0), 1.0);
    ui.text_in(NodeSpec::column(), "Fonts", style);
    ui.finish();
    let (dl, _) = core.output();
    dl.quads
        .iter()
        .filter(|q| q.kind != kui_core::QuadKind::Solid)
        .count()
}

#[test]
fn installed_family_registers_by_name_and_unknown_is_rejected() {
    let mut core = Core::new();
    assert!(core.add_system_font("kui-no-such-family-2026").is_none());
    let families = core.system_font_families();
    let Some(name) = families.first().cloned() else {
        // The one font test the fixture face cannot stand in for: it is
        // about the *installed* set, which a fontless CI image has none of.
        eprintln!("skipped: no installed font on this machine");
        return;
    };
    let id = core
        .add_system_font(&name)
        .expect("installed family registers");
    assert_eq!(core.font_family(id), Some(name.as_str()));
    assert!(glyph_quads(&mut core, TextStyle::new(20.0).font(id)) > 0);

    core.remove_font(id);
    assert!(core.font_family(id).is_none());
    // A stale handle still shapes (as sans-serif) rather than failing.
    assert!(glyph_quads(&mut core, TextStyle::new(20.0).font(id)) > 0);
}

#[test]
fn font_file_bytes_register_and_garbage_is_rejected() {
    let mut core = Core::new();
    assert!(core.add_font_data(vec![0u8; 64]).is_none());
    // The face the tests carry with them (backlog AR48): this used to look
    // for a system font at four paths and pass green where none was.
    let id = core
        .add_font_data(kui_core::testing::liga_font())
        .expect("a real font file registers");
    let family = core
        .font_family(id)
        .expect("family name recorded")
        .to_string();
    assert_eq!(family, "Kui Liga");
    assert!(glyph_quads(&mut core, TextStyle::new(20.0).font(id)) > 0);
    // Two styles differing only in font must not share a shaping cache entry.
    let a = glyph_quads(&mut core, TextStyle::new(20.0));
    let b = glyph_quads(&mut core, TextStyle::new(20.0).font(id));
    assert!(a > 0 && b > 0);
    core.remove_font(id);
    assert!(core.font_family(id).is_none());
}

#[test]
fn font_folders_and_files_load_by_path_and_names_resolve_idempotently() {
    let mut core = Core::new();
    assert!(core.load_font_file("/no/such/font.ttf").is_none());
    assert_eq!(core.load_fonts_dir("/no/such/dir"), 0);
    // A private folder with one font in it, the "bundled fonts/ dir" case:
    // the fixture face written out, so the path doors are exercised on a
    // machine with no font of its own.
    let dir = std::env::temp_dir().join(format!("kui-fonts-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("bundled.ttf");
    std::fs::write(&file, kui_core::testing::liga_font()).unwrap();

    let by_file = core.load_font_file(&file).expect("file loads by path");
    let family = core.font_family(by_file).unwrap().to_string();
    assert!(glyph_quads(&mut core, TextStyle::new(20.0).font(by_file)) > 0);

    let mut fresh = Core::new();
    assert!(fresh.load_fonts_dir(&dir) >= 1);
    let a = fresh
        .add_system_font(&family)
        .expect("loaded family resolves by name");
    let b = fresh.add_system_font(&family).unwrap();
    assert_eq!(a, b, "the same family keeps one handle");
    assert!(glyph_quads(&mut fresh, TextStyle::new(20.0).font(a)) > 0);
    std::fs::remove_dir_all(&dir).ok();
}

/// What the font database knows of each family, listed without loading or
/// shaping anything (backlog F97): whether every face is fixed-pitch, the
/// weights, an italic. Families of the fixture face stand in for installed
/// ones: a monospaced one in three faces, a proportional one, and one whose
/// faces disagree, which is not monospaced.
#[test]
fn system_fonts_say_what_each_family_is() {
    use kui_core::SystemFont;
    use kui_core::testing::font_face;
    let mut core = Core::new();
    let faces = [
        font_face("Kui F97 Mono", 400, false, true),
        font_face("Kui F97 Mono", 700, false, true),
        font_face("Kui F97 Mono", 400, true, true),
        font_face("Kui F97 Sans", 300, false, false),
        font_face("Kui F97 Mixed", 700, false, false),
        font_face("Kui F97 Mixed", 400, false, true),
    ];
    let ids: Vec<_> = faces
        .into_iter()
        .map(|bytes| core.add_font_data(bytes).expect("the fixture registers"))
        .collect();

    let fonts = core.system_fonts();
    let names: Vec<&str> = fonts.iter().map(|f| f.family.as_str()).collect();
    let mut sorted = names.clone();
    sorted.sort();
    sorted.dedup();
    assert_eq!(names, sorted, "sorted by family, one row each");
    assert_eq!(
        core.system_font_families(),
        names,
        "the same families system_font_families names"
    );

    let find = |family: &str| fonts.iter().find(|f| f.family == family).cloned();
    assert_eq!(
        find("Kui F97 Mono"),
        Some(SystemFont {
            family: "Kui F97 Mono".into(),
            monospaced: true,
            weights: vec![400, 700],
            italic: true,
        })
    );
    assert_eq!(
        find("Kui F97 Sans"),
        Some(SystemFont {
            family: "Kui F97 Sans".into(),
            monospaced: false,
            weights: vec![300],
            italic: false,
        })
    );
    let mixed = find("Kui F97 Mixed").expect("listed");
    assert!(!mixed.monospaced, "one proportional face and it is not");
    assert_eq!(
        mixed.weights,
        vec![400, 700],
        "sorted, whatever the load order"
    );

    // Each family's name is one add_system_font takes.
    let mono = core.add_system_font("Kui F97 Mono").expect("resolves");
    assert_eq!(core.font_family(mono), Some("Kui F97 Mono"));

    for id in ids {
        core.remove_font(id);
    }
    assert!(
        !core
            .system_fonts()
            .iter()
            .any(|f| f.family.starts_with("Kui F97")),
        "gone with their faces"
    );
}

/// A face whose glyph advances cannot be measured — no `head`, `hhea` or
/// `hmtx`, the way macOS's GB18030 Bitmap has none — is refused wherever
/// a face comes in (backlog F98): from bytes, from a file, from a folder,
/// so it is never a family to list, name or fall back to.
#[test]
fn a_face_whose_advances_cannot_be_measured_is_refused() {
    use kui_core::testing::{han_face, unmeasurable_face};
    let mut core = Core::new();
    assert!(
        core.add_font_data(unmeasurable_face("Kui F98 Bitmap"))
            .is_none(),
        "no usable face in the bytes"
    );

    let dir = std::env::temp_dir().join(format!("kui-f98-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let bitmap = dir.join("bitmap.ttf");
    std::fs::write(&bitmap, unmeasurable_face("Kui F98 Bitmap")).unwrap();
    std::fs::write(dir.join("han.ttf"), han_face("Kui F98 Han")).unwrap();
    assert!(core.load_font_file(&bitmap).is_none(), "nor in the file");
    assert_eq!(core.load_fonts_dir(&dir), 1, "the folder's other face");
    std::fs::remove_dir_all(&dir).ok();

    let families = core.system_font_families();
    assert!(families.iter().any(|f| f == "Kui F98 Han"));
    assert!(
        !families.iter().any(|f| f == "Kui F98 Bitmap"),
        "not offered in the list"
    );
    assert!(core.add_system_font("Kui F98 Bitmap").is_none());
}

/// Han text in `mono` on the fonts this machine has (backlog F98): on a
/// Mac, 字 fell back to GB18030 Bitmap, whose advances are infinite, and
/// the frame overflowed in a debug build or placed what followed at
/// infinity. Every glyph lands at a finite place, left to right.
#[test]
fn han_in_mono_draws_at_finite_places() {
    use kui_core::{FontFamily, QuadKind};
    let mut core = Core::new();
    let mut ui = core.frame(Size::new(400.0, 100.0), 1.0);
    ui.text_in(
        NodeSpec::column(),
        "字 a",
        TextStyle::new(14.0).family(FontFamily::Mono),
    );
    ui.finish();
    let (dl, _) = core.output();
    let glyphs: Vec<_> = dl
        .quads
        .iter()
        .filter(|q| q.kind != QuadKind::Solid)
        .map(|q| q.rect)
        .collect();
    for r in &glyphs {
        assert!(
            r.x.is_finite() && r.y.is_finite() && r.w.is_finite() && r.h.is_finite(),
            "a glyph at {r:?}"
        );
        assert!(
            r.x >= 0.0 && r.x + r.w <= 400.0,
            "a glyph off the line: {r:?}"
        );
    }
    // Where some installed face maps 字, it is drawn and the `a` starts past it.
    if let [han, a] = glyphs[..] {
        assert!(han.x + han.w <= a.x, "字 before a: {glyphs:?}");
    }
}

/// The glyph quads of `spans` in `style`, drawn as one rich text: each
/// glyph's x and width.
fn rich_glyphs(core: &mut Core, spans: &[kui_core::Span<'_>], style: TextStyle) -> Vec<(f32, f32)> {
    let mut ui = core.frame(Size::new(400.0, 100.0), 1.0);
    ui.with(NodeSpec::column(), |ui| ui.rich_text(spans, style));
    ui.finish();
    let (dl, _) = core.output();
    dl.quads
        .iter()
        .filter(|q| q.kind != kui_core::QuadKind::Solid)
        .map(|q| (q.rect.x, q.rect.w))
        .collect()
}

/// The glyph quads of one row of cells in `style`, each `flags`: each
/// glyph's x and width.
fn cell_glyphs(core: &mut Core, text: &str, flags: u8, style: TextStyle) -> Vec<(f32, f32)> {
    use kui_core::cells::{Cell, CellGrid};
    let cells: Vec<Cell> = text
        .chars()
        .map(|ch| Cell {
            flags,
            ..Cell::new(ch, 0xffffffff, 0)
        })
        .collect();
    let mut ui = core.frame(Size::new(400.0, 100.0), 1.0);
    ui.configure_root(NodeSpec::column().fill());
    ui.cells(
        &CellGrid {
            rows: 1,
            cols: cells.len(),
            cells: &cells,
            style,
            cursor: None,
            origin_line: 0,
        },
        NodeSpec::default(),
    );
    ui.finish();
    let (dl, _) = core.output();
    dl.quads
        .iter()
        .filter(|q| q.kind != kui_core::QuadKind::Solid)
        .map(|q| (q.rect.x, q.rect.w))
        .collect()
}

/// Every glyph `advance` from the one before it and `width` wide: glyphs
/// of the fixture family, whose advance is half an em.
fn assert_fixture(glyphs: &[(f32, f32)], advance: f32, width: f32, what: &str) {
    assert_eq!(glyphs.len(), 4, "{what}: {glyphs:?}");
    for pair in glyphs.windows(2) {
        assert_eq!(pair[1].0 - pair[0].0, advance, "{what}: {glyphs:?}");
    }
    for &(_, w) in glyphs {
        assert_eq!(w, width, "{what}: {glyphs:?}");
    }
}

/// A variable family whose `wght` axis is not on the CSS scale — Berkeley
/// Mono Variable's, 100 (Regular) to 150 (Bold) with an `OS/2` weight of
/// 400 — draws its Regular for regular and its Bold for bold, in text and
/// in cells (backlog F100). Bold, asked at 700, fell outside the axis and
/// was drawn in another family, one glyph to a cell in a terminal; regular
/// took 400 as the coordinate and was drawn at the Bold. The fixture's
/// square is 400 units wide at the Regular and 500 at the Bold; a family
/// with a bold face is registered beside it, where bold used to go.
#[test]
fn bold_of_a_variable_family_off_the_css_scale_is_its_bold_instance() {
    use kui_core::Span;
    use kui_core::cells::flags;
    use kui_core::testing::{font_face, variable_face};
    let mut core = Core::new();
    let axis = [("Regular", 100), ("Bold", 150)];
    let id = core
        .add_font_data(variable_face("Kui F100 Var", false, [100, 100, 150], &axis))
        .expect("the variable fixture registers");
    core.add_font_data(variable_face("Kui F100 Var", true, [100, 100, 150], &axis))
        .expect("its italic");
    core.add_font_data(font_face("Kui F100 Decoy", 700, false, false))
        .expect("a bold face of another family");
    // 20 px: an advance of 10, a square of 8 at the Regular and 10 at the Bold.
    let style = TextStyle::new(20.0).font(id).line_height(24.0);
    let plain = Span::new("aaaa");
    let bold = Span {
        bold: true,
        ..plain
    };
    let both = Span {
        italic: true,
        ..bold
    };
    assert_fixture(
        &rich_glyphs(&mut core, &[plain], style),
        10.0,
        8.0,
        "regular",
    );
    assert_fixture(&rich_glyphs(&mut core, &[bold], style), 10.0, 10.0, "bold");
    assert_fixture(
        &cell_glyphs(&mut core, "aaaa", 0, style),
        10.0,
        8.0,
        "regular cells",
    );
    assert_fixture(
        &cell_glyphs(&mut core, "aaaa", flags::BOLD, style),
        10.0,
        10.0,
        "bold cells",
    );
    // Bold italic from the italic face (upright squares too, so no lean
    // is synthesized) at its Bold.
    let flags = flags::BOLD | flags::ITALIC;
    assert_fixture(
        &rich_glyphs(&mut core, &[both], style),
        10.0,
        10.0,
        "bold italic",
    );
    assert_fixture(
        &cell_glyphs(&mut core, "aaaa", flags, style),
        10.0,
        10.0,
        "bold italic cells",
    );
}

/// A family of one regular face draws bold in that face, its outline
/// grown, and italic in that face, leaning — never in another family
/// that has the weight or the style (backlog F100). Bold used to go to
/// the other family's bold face, the fixture's square at its own width.
#[test]
fn bold_and_italic_of_a_family_without_them_are_synthesized_in_it() {
    use kui_core::Span;
    use kui_core::cells::flags;
    use kui_core::testing::font_face;
    let mut core = Core::new();
    let id = core
        .add_font_data(font_face("Kui F100 Mono", 400, false, true))
        .expect("the fixture registers");
    core.add_font_data(font_face("Kui F100 Decoy", 700, false, false))
        .expect("a bold face of another family");
    core.add_font_data(font_face("Kui F100 Decoy", 400, true, false))
        .expect("an italic face of another family");
    let style = TextStyle::new(20.0).font(id).line_height(24.0);
    let plain = Span::new("aaaa");
    let regular = rich_glyphs(&mut core, &[plain], style);
    assert_fixture(&regular, 10.0, 8.0, "regular");
    let cases = [
        (
            Span {
                bold: true,
                ..plain
            },
            flags::BOLD,
            "bold",
        ),
        (
            Span {
                italic: true,
                ..plain
            },
            flags::ITALIC,
            "italic",
        ),
    ];
    for (span, flag, what) in cases {
        let text = rich_glyphs(&mut core, &[span], style);
        let cells = cell_glyphs(&mut core, "aaaa", flag, style);
        for glyphs in [&text, &cells] {
            assert_eq!(glyphs.len(), 4, "{what}: {glyphs:?}");
            assert!(
                glyphs.windows(2).all(|p| p[1].0 - p[0].0 == 10.0),
                "{what}, a glyph to a cell: {glyphs:?}"
            );
            assert!(
                glyphs.iter().all(|g| g.1 > 8.0),
                "{what} wider than regular: {glyphs:?}"
            );
        }
    }
}

/// A family with no 400 face — a Light-only file — draws in that family
/// in plain text and in an editor, as it does in a span and a cell: every
/// path asks it at its own regular (the regression pass over F100: only
/// spans and cells did, so one style drew in two families).
#[test]
fn a_family_with_no_400_face_draws_in_it_everywhere() {
    use kui_core::testing::font_face;
    use kui_core::{EditOptions, Span};
    let mut core = Core::new();
    let id = core
        .add_font_data(font_face("Kui F100 Light", 300, false, false))
        .expect("the fixture registers");
    let style = TextStyle::new(20.0).font(id).line_height(24.0);
    assert_fixture(
        &rich_glyphs(&mut core, &[Span::new("aaaa")], style),
        10.0,
        8.0,
        "span",
    );
    assert_fixture(
        &cell_glyphs(&mut core, "aaaa", 0, style),
        10.0,
        8.0,
        "cells",
    );
    let glyphs = |core: &mut Core| -> Vec<(f32, f32)> {
        let (dl, _) = core.output();
        dl.quads
            .iter()
            .filter(|q| q.kind == kui_core::QuadKind::GlyphMask)
            .map(|q| (q.rect.x, q.rect.w))
            .collect()
    };
    let mut ui = core.frame(Size::new(400.0, 100.0), 1.0);
    ui.text_in(NodeSpec::column(), "aaaa", style);
    ui.finish();
    assert_fixture(&glyphs(&mut core), 10.0, 8.0, "plain text");
    let mut ui = core.frame(Size::new(400.0, 100.0), 1.0);
    ui.text_edit(
        "field",
        "aaaa",
        &EditOptions {
            style,
            ..Default::default()
        },
        NodeSpec::column().width(kui_core::Sizing::Fixed(300.0)),
    );
    ui.finish();
    assert_fixture(&glyphs(&mut core), 10.0, 8.0, "editor");
}

/// A face of a registered family loaded or removed under text already
/// shaped in it reaches that text: plain text, spans, cells and an
/// editor shape again at the weights the family is asked at now (RG59).
/// They were keyed by content and style alone, so text shaped before a
/// family gained its regular kept the face it had, and bold synthesized
/// before the family's Bold was loaded stayed synthetic — or stayed that
/// Bold after it was removed.
#[test]
fn a_face_loaded_or_removed_under_shaped_text_reaches_it() {
    use kui_core::testing::{font_face, han_face};
    use kui_core::{EditOptions, Span};
    let mut core = Core::new();
    // A Light face with no 字: the family's regular is 300, and 字 falls
    // back to another family.
    let light = core
        .add_font_data(font_face("Kui RG59", 300, false, false))
        .expect("the fixture registers");
    let style = TextStyle::new(20.0).font(light).line_height(24.0);
    let glyphs = |core: &mut Core| -> Vec<(f32, f32)> {
        let (dl, _) = core.output();
        dl.quads
            .iter()
            .filter(|q| q.kind == kui_core::QuadKind::GlyphMask)
            .map(|q| (q.rect.x, q.rect.w))
            .collect()
    };
    // Each path's glyphs of 字字字字.
    let every_path = |core: &mut Core| -> Vec<(&str, Vec<(f32, f32)>)> {
        let han = "字字字字";
        let mut ui = core.frame(Size::new(400.0, 100.0), 1.0);
        ui.text_in(NodeSpec::column(), han, style);
        ui.finish();
        let plain = glyphs(core);
        let spans = rich_glyphs(core, &[Span::new(han)], style);
        let cells = cell_glyphs(core, han, 0, style);
        let mut ui = core.frame(Size::new(400.0, 100.0), 1.0);
        ui.text_edit(
            "field",
            han,
            &EditOptions {
                style,
                ..Default::default()
            },
            NodeSpec::column().width(kui_core::Sizing::Fixed(300.0)),
        );
        ui.finish();
        let editor = glyphs(core);
        vec![
            ("plain text", plain),
            ("span", spans),
            ("cells", cells),
            ("editor", editor),
        ]
    };
    let bold = Span {
        bold: true,
        ..Span::new("aaaa")
    };
    every_path(&mut core);
    let synthetic = rich_glyphs(&mut core, &[bold], style);
    assert!(synthetic.iter().all(|g| g.1 > 8.0), "{synthetic:?}");
    // A Regular with 字 comes: the family's regular is 400, and 字 is
    // the fixture's, half an em apart.
    core.add_font_data(han_face("Kui RG59"))
        .expect("a regular face of it");
    let paths = every_path(&mut core);
    let square = paths[0].1.first().map(|g| g.1);
    for (what, glyphs) in paths {
        assert_eq!(glyphs.len(), 4, "{what}: {glyphs:?}");
        assert!(
            glyphs.windows(2).all(|p| p[1].0 - p[0].0 == 10.0)
                && glyphs.iter().all(|g| Some(g.1) == square),
            "{what} shaped at the family's new regular: {glyphs:?}"
        );
    }
    // Its Bold comes: bold is that face, drawn as it is.
    let bold_face = core
        .add_font_data(font_face("Kui RG59", 700, false, false))
        .expect("a bold face of it");
    let real = rich_glyphs(&mut core, &[bold], style);
    assert_fixture(&real, 10.0, 8.0, "bold, the family's Bold");
    // And goes: bold is synthesized again.
    core.remove_font(bold_face);
    let synthetic = rich_glyphs(&mut core, &[bold], style);
    assert!(
        synthetic.iter().all(|g| g.1 > 8.0),
        "bold synthesized again: {synthetic:?}"
    );
}
