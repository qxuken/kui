//! The runner's version and the apps it runs unchanged (ADR 0068,
//! decisions 6 and 7). An app pins the runner it was written against in
//! its `kui.config.lua` (`kui = "0.1.0-alpha.68"`); `kui run` warns when
//! the pin is neither this runner nor one listed here, and `kui pack`
//! packs the pinned runner, not its own.
//!
//! A release adds its predecessor here — and keeps the entries before it —
//! unless its CHANGELOG lists a break in the runner or in Kit, in which
//! case the list starts over empty (docs/releasing.md). Pre-1.0 most
//! releases break something, so the list is short.

/// This runner's version.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Earlier runners whose apps this one runs unchanged, newest first.
pub const RUNS: &[&str] = &[];

/// Whether an app pinned to `pin` runs here as it was written.
pub fn runs(pin: &str) -> bool {
    pin == VERSION || RUNS.contains(&pin)
}

/// The warning `kui run` prints for a pin this runner does not cover.
pub fn warning(pin: &str) -> Option<String> {
    (!runs(pin)).then(|| {
        format!(
            "this app pins kui {pin}, and this is kui {VERSION}, which does not list it as \
             compatible: it may not run as written — `kui upgrade {pin}` gets that runner"
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_running_version_is_compatible_with_itself_and_not_with_others() {
        assert!(runs(VERSION));
        assert!(warning(VERSION).is_none());
        assert!(
            warning("0.0.0-nope")
                .unwrap()
                .contains("kui upgrade 0.0.0-nope")
        );
    }

    #[test]
    fn the_table_never_lists_this_version_and_has_no_duplicates() {
        assert!(
            !RUNS.contains(&VERSION),
            "the table lists earlier runners only"
        );
        let mut seen = std::collections::HashSet::new();
        assert!(
            RUNS.iter().all(|v| seen.insert(v)),
            "a version listed twice"
        );
    }
}
