use cosmic_text::{fontdb, Attrs, Family, FontSystem, Weight};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

struct CountedBytes {
    bytes: &'static [u8],
    reads: Arc<AtomicUsize>,
}

impl AsRef<[u8]> for CountedBytes {
    fn as_ref(&self) -> &[u8] {
        self.reads.fetch_add(1, Ordering::Relaxed);
        self.bytes
    }
}

#[test]
fn matching_keeps_unrelated_font_bytes_unread() {
    let bytes = include_bytes!("../fonts/Inter-Regular.ttf");
    let reads = Arc::new(AtomicUsize::new(0));
    let mut db = fontdb::Database::new();
    db.load_font_data(bytes.to_vec());
    let counted = db.load_font_source(fontdb::Source::Binary(Arc::new(CountedBytes {
        bytes,
        reads: reads.clone(),
    })))[0];
    let mut unrelated = db.face(counted).unwrap().clone();
    db.remove_face(counted);
    for (family, _) in &mut unrelated.families {
        *family = "Unrequested family".into();
    }
    unrelated.weight = Weight::BOLD;
    db.push_face_info(unrelated);
    let mut fonts = FontSystem::new_with_locale_and_db("en-US".into(), db);
    reads.store(0, Ordering::Relaxed);

    assert!(!fonts
        .get_font_matches(&Attrs::new().family(Family::Name("Inter")))
        .is_empty());
    assert_eq!(reads.load(Ordering::Relaxed), 0);
}

#[test]
fn monospace_yields_the_default_before_preparing_the_remaining_candidates() {
    let bytes = include_bytes!("../fonts/Inter-Regular.ttf");
    let reads = Arc::new(AtomicUsize::new(0));
    let mut db = fontdb::Database::new();
    db.load_font_data(bytes.to_vec());
    let primary = db.faces().next().unwrap().id;
    let counted = db.load_font_source(fontdb::Source::Binary(Arc::new(CountedBytes {
        bytes,
        reads: reads.clone(),
    })))[0];
    let mut fallback = db.face(counted).unwrap().clone();
    db.remove_face(counted);
    for (family, _) in &mut fallback.families {
        *family = "Deferred mono".into();
    }
    fallback.monospaced = true;
    let fallback = db.push_face_info(fallback);
    db.set_monospace_family("Inter");
    let mut fonts = FontSystem::new_with_locale_and_db("en-US".into(), db);
    reads.store(0, Ordering::Relaxed);
    let matches = fonts.get_font_matches(&Attrs::new().family(Family::Monospace));
    let mut candidates = cosmic_text::fallback::FontFallbackIter::new(
        &mut fonts,
        &matches,
        &[&Family::Monospace],
        &[],
        "Hello\u{10FFFF}",
        Weight::NORMAL,
    );

    assert_eq!(candidates.next().unwrap().id(), primary);
    assert_eq!(reads.load(Ordering::Relaxed), 0);
    assert_eq!(candidates.next().unwrap().id(), fallback);
    assert!(reads.load(Ordering::Relaxed) > 0);
    assert_eq!(
        candidates.map(|font| font.id()).collect::<Vec<_>>(),
        [primary, fallback]
    );
}
