use super::SpellingChoices;
use std::ops::Range;
/// Windows' display language list can omit a writing language enabled in Settings.
#[cfg(windows)]
fn preferred_languages() -> Vec<String> {
    use windows::{
        System::UserProfile::GlobalizationPreferences, Win32::Globalization::ResolveLocaleName,
    };
    let Ok(languages) = GlobalizationPreferences::Languages() else {
        return sys_locale::get_locales().collect();
    };
    (0..languages.Size().unwrap_or(0))
        .filter_map(|i| {
            let tag = languages.GetAt(i).ok()?;
            // SpellCheckerFactory takes specific tags, but Settings also returns
            // neutral tags such as "ru". Let Windows choose the matching locale.
            let mut specific = [0_u16; 85];
            let n = unsafe { ResolveLocaleName(&tag, Some(&mut specific)) };
            Some(if n > 1 {
                String::from_utf16_lossy(&specific[..n as usize - 1])
            } else {
                tag.to_string_lossy()
            })
        })
        .collect()
}

#[cfg(not(windows))]
fn preferred_languages() -> Vec<String> {
    sys_locale::get_locales().collect()
}

fn dictionaries() -> Vec<spellkit::Checker> {
    let mut checkers: Vec<_> = spellkit::Checker::new().into_iter().collect();
    for tag in preferred_languages() {
        let known = |locale: &str| {
            checkers.iter().any(|c| {
                c.locale()
                    .replace('_', "-")
                    .eq_ignore_ascii_case(&locale.replace('_', "-"))
            })
        };
        if !known(&tag)
            && let Ok(checker) = spellkit::Checker::with_locale(&tag)
            && !known(checker.locale())
        {
            checkers.push(checker);
        }
    }
    checkers
}

/// Only bytes rejected by every configured dictionary are misspelled.
/// Token boundaries can differ between languages (e.g. a hyphenated word).
fn common_errors(a: &[Range<usize>], b: &[Range<usize>]) -> Vec<Range<usize>> {
    let (mut i, mut j) = (0, 0);
    let mut errors = Vec::new();
    while i < a.len() && j < b.len() {
        let overlap = a[i].start.max(b[j].start)..a[i].end.min(b[j].end);
        if !overlap.is_empty() {
            errors.push(overlap);
        }
        if a[i].end <= b[j].end {
            i += 1;
        } else {
            j += 1;
        }
    }
    errors
}

fn check_text(dictionaries: &[spellkit::Checker], text: &str) -> Vec<Range<usize>> {
    let mut answers = dictionaries.iter().map(|c| {
        let mut ranges: Vec<_> = c.check(text).map(|e| e.range()).collect();
        ranges.sort_unstable_by_key(|r| r.start);
        ranges
    });
    let mut errors = answers.next().unwrap_or_default();
    while !errors.is_empty()
        && let Some(other) = answers.next()
    {
        errors = common_errors(&errors, &other);
    }
    errors
}

pub(super) struct Dictionary(Vec<spellkit::Checker>);

impl Dictionary {
    pub(super) fn open() -> Option<Self> {
        let checkers = dictionaries();
        (!checkers.is_empty()).then_some(Self(checkers))
    }
}

impl super::Dictionary for Dictionary {
    fn check(&self, text: &str) -> Vec<Range<usize>> {
        check_text(&self.0, text)
    }

    fn choices(&self, word: &str) -> SpellingChoices {
        // Interleave native rankings: the user's second writing language
        // also gets a first suggestion, even when the first has many.
        let lists: Vec<_> = self.0.iter().map(|c| c.suggest(word)).collect();
        let mut suggestions = Vec::new();
        for rank in 0..lists.iter().map(Vec::len).max().unwrap_or(0) {
            for list in &lists {
                if let Some(s) = list.get(rank)
                    && s != word
                    && !s.is_empty()
                    && !suggestions.contains(s)
                {
                    suggestions.push(s.clone());
                    if suggestions.len() == 6 {
                        break;
                    }
                }
            }
            if suggestions.len() == 6 {
                break;
            }
        }
        SpellingChoices {
            suggestions,
            can_learn: true,
        }
    }

    fn learn(&mut self, word: &str) -> bool {
        // One explicit addition to the user's default personal dictionary.
        // Acceptance by any configured writing language clears the mark.
        self.0[0].learn(word).is_ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn alternate_languages_can_accept_words_or_parts_of_a_compound() {
        let primary = [0..4, 6..12, 18..23];
        assert_eq!(
            common_errors(&primary, &[2..8, 18..23]),
            [2..4, 6..8, 18..23]
        );
        assert!(common_errors(&primary, &[]).is_empty());
    }
    #[cfg(windows)]
    #[test]
    fn mixed_russian_and_english_with_native_dictionaries() {
        let (Ok(en), Ok(ru)) = (
            spellkit::Checker::with_locale("en-US"),
            spellkit::Checker::with_locale("ru-RU"),
        ) else {
            return;
        };
        let text = "Книги и заметки. The books and notes. Превед wrldd.";
        use super::super::Dictionary as _;
        let sample = [en, ru];
        let errors = check_text(&sample, text);
        let preferred = preferred_languages();
        if sample.iter().all(|c| {
            preferred
                .iter()
                .any(|tag| tag.eq_ignore_ascii_case(c.locale()))
        }) {
            assert_eq!(check_text(&dictionaries(), text), errors);
        }
        assert_eq!(
            errors.iter().map(|r| &text[r.clone()]).collect::<Vec<_>>(),
            ["Превед", "wrldd"]
        );
        let choices = Dictionary(sample.into_iter().collect()).choices("wrldd");
        assert!(choices.can_learn && !choices.suggestions.is_empty());
    }
    #[cfg(target_os = "linux")]
    #[test]
    fn hunspell_suggests_and_persists_a_word_in_an_isolated_dictionary() {
        use super::super::Dictionary as _;
        const CHILD: &str = "KUI_SPELLING_TEST_CHILD";
        if std::env::var_os(CHILD).is_some() {
            let open = || Dictionary(vec![spellkit::Checker::with_locale("en-US").unwrap()]);
            let mut dictionary = open();
            assert!(!dictionary.check("helo").is_empty());
            assert!(
                dictionary
                    .choices("helo")
                    .suggestions
                    .iter()
                    .any(|s| s == "hello")
            );
            assert!(!dictionary.check("kuizzz").is_empty());
            assert!(dictionary.learn("kuizzz"));
            assert!(
                open().check("kuizzz").is_empty(),
                "fresh handle reloaded the personal word list"
            );
            return;
        }
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("en_US.aff"),
            "SET UTF-8\nTRY abcdefghijklmnopqrstuvwxyz\n",
        )
        .unwrap();
        std::fs::write(dir.path().join("en_US.dic"), "2\nhello\nworld\n").unwrap();
        let status=std::process::Command::new(std::env::current_exe().unwrap())
            .arg("--exact").arg("system::spelling::backend::tests::hunspell_suggests_and_persists_a_word_in_an_isolated_dictionary")
            .env("DICPATH",dir.path()).env("XDG_DATA_HOME",dir.path()).env(CHILD,"1").status().unwrap();
        assert!(status.success());
        assert!(
            std::fs::read_dir(dir.path().join("spellkit"))
                .unwrap()
                .next()
                .is_some()
        );
    }
    #[cfg(target_os = "macos")]
    #[test]
    #[ignore = "exercises the personal dictionary of a disposable macOS test account"]
    fn appkit_suggestions_and_learning() {
        use super::super::Dictionary as _;
        // Run on the same background thread and with the same autorelease pool
        // used by the host. The CI account is disposable; clean up its word too.
        std::thread::spawn(|| {
            objc2::rc::autoreleasepool(|_| {
                let open = || Dictionary(vec![spellkit::Checker::with_locale("en-US").unwrap()]);
                let mut dictionary = open();
                assert!(
                    dictionary
                        .choices("speling")
                        .suggestions
                        .iter()
                        .any(|s| s == "spelling")
                );
                let fresh = "kuizzzspellingregression";
                assert!(!dictionary.check(fresh).is_empty());
                assert!(dictionary.learn(fresh));
                let accepted = open().check(fresh).is_empty();
                dictionary.0[0].unlearn(fresh).unwrap();
                assert!(accepted);
            })
        })
        .join()
        .unwrap();
    }
}
