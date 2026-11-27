//! Misspelling ranges supplied by the host, in UTF-8 offsets in the checked text.
use std::ops::Range;

/// Actions the host dictionary can offer for one exact misspelling.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SpellingChoices {
    pub suggestions: Vec<String>,
    pub can_learn: bool,
}

/// The native host supplies a non-blocking dictionary. Embedders may supply their own.
pub trait Spellchecker {
    /// Return disjoint ranges in ascending byte order for this exact text.
    fn check(&mut self, text: &str) -> Option<&[Range<usize>]>;
    /// Request corrections only when opening a menu. None means pending.
    fn choices(&mut self, _word: &str) -> Option<SpellingChoices> {
        Some(SpellingChoices::default())
    }
    /// Save an explicitly chosen word in the host's personal dictionary.
    fn learn(&mut self, _word: &str) {}
    /// Changes when previously completed checks must be performed again.
    fn dictionary_revision(&self) -> u64 {
        0
    }
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

    /// The provider's dictionary generation. Retained highlights for old
    /// generations must be checked again, even for unchanged source text.
    pub fn dictionary_revision(&self) -> u64 {
        self.provider
            .as_ref()
            .map_or(0, |p| p.dictionary_revision())
    }

    /// Ask for the menu actions available for this word.
    pub fn choices(&mut self, word: &str) -> Option<SpellingChoices> {
        match &mut self.provider {
            Some(p) => p.choices(word),
            None => Some(SpellingChoices::default()),
        }
    }

    /// Called only after the user chooses to learn the word.
    pub fn learn(&mut self, word: &str) {
        if let Some(p) = &mut self.provider {
            p.learn(word);
        }
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
