//! Private harness for the shaper embedded in kui-core.

extern crate alloc;

#[doc(hidden)]
#[path = "../../kui-core/vendor/cosmic-text/src/lib.rs"]
pub mod cosmic_text;

pub use cosmic_text::*;

// Upstream's tests and direct benchmarks discover desktop fonts. The
// embedded runtime receives its catalog from kui's host instead.
#[allow(clippy::new_without_default)]
impl FontSystem {
    pub fn new() -> Self {
        Self::new_with_fonts(core::iter::empty())
    }

    pub fn new_with_fonts(fonts: impl IntoIterator<Item = fontdb::Source>) -> Self {
        let locale = sys_locale::get_locale().unwrap_or_else(|| "en-US".into());
        let mut db = fontdb::Database::new();
        db.load_system_fonts();
        for source in fonts {
            db.load_font_source(source);
        }
        db.set_monospace_family("Noto Sans Mono");
        db.set_sans_serif_family("Open Sans");
        db.set_serif_family("DejaVu Serif");
        Self::new_with_locale_and_db_and_fallback(locale, db, PlatformFallback)
    }
}
