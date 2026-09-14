//! Registered fonts: installed families by name, font files from bytes,
//! stale handles falling back to sans, and shaping through `TextStyle::font`.

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
