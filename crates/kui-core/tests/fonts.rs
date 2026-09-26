//! Registered fonts: installed families by name, font files from bytes,
//! stale handles falling back to sans, shaping through `TextStyle::font`,
//! and what the font database says each family is.

use kui_core::{Core, NodeSpec, Size, TextStyle};

fn glyph_quads(core: &mut Core, style: TextStyle) -> usize {
    let mut ui = core.frame(Size::new(400.0, 100.0), 1.0);
    ui.with(NodeSpec::column(), |ui| ui.text("Fonts", style));
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
    ui.with(NodeSpec::column(), |ui| {
        ui.text("字 a", TextStyle::new(14.0).family(FontFamily::Mono))
    });
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
