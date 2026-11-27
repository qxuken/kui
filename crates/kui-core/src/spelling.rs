//! Misspelling ranges supplied by the host, in UTF-8 offsets in the checked text.
use std::ops::Range;

/// The native host supplies a non-blocking dictionary. Embedders may supply their own.
pub trait Spellchecker {
    /// Return disjoint ranges in ascending byte order for this exact text.
    fn check(&mut self, text: &str) -> Option<&[Range<usize>]>;
    /// Publish completed checks before declarations and return whether results changed.
    fn collect(&mut self) -> bool;
    fn pending(&self) -> bool;
}

#[derive(Default)]
pub struct Spelling {
    factory: Option<fn() -> Box<dyn Spellchecker>>,
    provider: Option<Box<dyn Spellchecker>>,
    revision: u64,
}

impl Spelling {
    /// Construct a host dictionary only when an editor first requests a check.
    pub fn with_provider(factory: fn() -> Box<dyn Spellchecker>) -> Self {
        Self {
            factory: Some(factory),
            ..Self::default()
        }
    }

    /// A launcher installs a default on a handed-in core only if the embedder
    /// has supplied none. An already running checker keeps its cached answers.
    pub fn install_default(&mut self, factory: fn() -> Box<dyn Spellchecker>) {
        self.factory.get_or_insert(factory);
    }

    /// Ranges for exactly this text, or None while the result is pending.
    /// A missing dictionary supplies an empty result.
    pub fn check(&mut self, text: &str) -> Option<&[Range<usize>]> {
        if text.is_empty() {
            return Some(&[]);
        }
        let Some(factory) = self.factory else {
            return Some(&[]);
        };
        self.provider.get_or_insert_with(factory).check(text)
    }

    /// Include this in memoized paint dependencies when spelling is enabled.
    pub fn revision(&self) -> u64 {
        self.revision
    }

    pub(crate) fn begin_frame(&mut self) {
        if self.provider.as_mut().is_some_and(|p| p.collect()) {
            self.revision = self.revision.wrapping_add(1);
        }
    }

    pub(crate) fn pending(&self) -> bool {
        self.provider.as_ref().is_some_and(|p| p.pending())
    }
}
