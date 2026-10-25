//! DTCG token documents (ADR 0062): bytes as JSON through
//! [`kui_core::json::parse`], and as a token file or an inline-only
//! resolver through [`kui_core::dtcg::from_str`] (no folder, so a file
//! `$ref` is refused rather than read), then the loaded table added to a
//! core under both appearances. Byte 0 picks the options: bit 0 a prefix,
//! bit 1 another rem, bit 2 an input for a modifier named `density`.

use kui_core::dtcg::{self, DtcgOptions};
use kui_core::{Core, Size, Theme};

pub fn run(data: &[u8]) {
    let [flags, rest @ ..] = data else {
        return;
    };
    let Ok(text) = std::str::from_utf8(rest) else {
        return;
    };
    let _ = kui_core::json::parse(text);
    let opts = DtcgOptions {
        prefix: if flags & 1 != 0 {
            "brand".into()
        } else {
            String::new()
        },
        rem_px: if flags & 2 != 0 { 10.0 } else { 16.0 },
        inputs: if flags & 4 != 0 {
            vec![("density".into(), "compact".into())]
        } else {
            Vec::new()
        },
        ..DtcgOptions::default()
    };
    let Ok(loaded) = dtcg::from_str(text, None, &opts) else {
        return;
    };
    let mut core = Core::new();
    core.add_dtcg(&loaded);
    for theme in [Theme::light(), Theme::dark()] {
        core.set_theme(theme);
        let mut ui = core.frame(Size::new(64.0, 64.0), 1.0);
        for n in loaded.names.iter().take(64) {
            let _ = ui.token_color(&n.name);
            let _ = ui.token_length(&n.name);
        }
        ui.finish();
    }
}
