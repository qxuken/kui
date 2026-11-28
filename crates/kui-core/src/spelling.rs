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
    factory: Option<Box<dyn FnOnce() -> Box<dyn Spellchecker>>>,
    provider: Option<Box<dyn Spellchecker>>,
    revision: u64,
}

impl Spelling {
    /// Construct a host dictionary only when an editor first requests a check.
    pub fn with_provider(factory: impl FnOnce() -> Box<dyn Spellchecker> + 'static) -> Self {
        Self {
            factory: Some(Box::new(factory)),
            ..Self::default()
        }
    }

    /// A launcher installs a default on a handed-in core only if the embedder
    /// has supplied none. An already running checker keeps its cached answers.
    pub fn install_default(&mut self, factory: impl FnOnce() -> Box<dyn Spellchecker> + 'static) {
        if self.provider.is_none() && self.factory.is_none() {
            self.factory = Some(Box::new(factory));
        }
    }

    fn provider(&mut self) -> Option<&mut (dyn Spellchecker + 'static)> {
        if self.provider.is_none() {
            self.provider = self.factory.take().map(|open| open());
        }
        self.provider.as_deref_mut()
    }

    /// Ranges for exactly this text, or None while the result is pending.
    /// A missing dictionary supplies an empty result.
    pub fn check(&mut self, text: &str) -> Option<&[Range<usize>]> {
        if text.is_empty() {
            return Some(&[]);
        }
        match self.provider() {
            Some(provider) => provider.check(text),
            None => Some(&[]),
        }
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
        if word.is_empty() {
            return Some(SpellingChoices::default());
        }
        match self.provider() {
            Some(p) => p.choices(word),
            None => Some(SpellingChoices::default()),
        }
    }

    /// Called only after the user chooses to learn the word.
    pub fn learn(&mut self, word: &str) {
        if !word.is_empty()
            && let Some(p) = self.provider()
        {
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
