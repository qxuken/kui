//! Each window opens its platform dictionary on a dedicated thread on first use.
use kui_core::spelling::Spellchecker;
use std::collections::{HashMap, HashSet, VecDeque};
use std::ops::Range;
use std::sync::Arc;
use std::sync::mpsc::{self, Receiver, SyncSender, TryRecvError};

type Answer = (Arc<str>, Vec<Range<usize>>);
const CACHE_SIZE: usize = 2048;
const QUEUE_SIZE: usize = 64;
const CACHE_BYTES: usize = 8 * 1024 * 1024;

struct Worker {
    requests: SyncSender<Arc<str>>,
    answers: Receiver<Answer>,
    pending: HashSet<Arc<str>>,
    cache: HashMap<Arc<str>, Vec<Range<usize>>>,
    order: VecDeque<Arc<str>>,
    cache_bytes: usize,
    stopped: bool,
}

// A native worker lives for the lifetime of the window. Drain AppKit's
// temporary objects after each dictionary operation on its background thread.
fn dictionary_call<T>(f: impl FnOnce() -> T) -> T {
    #[cfg(target_os = "macos")]
    {
        objc2::rc::autoreleasepool(|_| f())
    }
    #[cfg(not(target_os = "macos"))]
    {
        f()
    }
}

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

impl Worker {
    pub fn new() -> Self {
        let (requests, inbox) = mpsc::sync_channel::<Arc<str>>(QUEUE_SIZE);
        let (outbox, answers) = mpsc::channel();
        let thread = std::thread::Builder::new()
            .name("kui-spelling".into())
            .spawn(move || {
                let checkers = dictionary_call(dictionaries);
                if checkers.is_empty() {
                    return;
                }
                for text in inbox {
                    let ranges = dictionary_call(|| check_text(&checkers, &text));
                    if outbox.send((text, ranges)).is_err() {
                        break;
                    }
                }
                dictionary_call(|| drop(checkers));
            });
        Self {
            requests,
            answers,
            pending: Default::default(),
            cache: Default::default(),
            order: VecDeque::new(),
            cache_bytes: 0,
            stopped: thread.is_err(),
        }
    }
}

impl Spellchecker for Worker {
    fn pending(&self) -> bool {
        !self.pending.is_empty()
    }

    fn check(&mut self, text: &str) -> Option<&[Range<usize>]> {
        if let Some(ranges) = self.cache.get(text) {
            return Some(ranges);
        }
        if self.stopped {
            return Some(&[]);
        }
        if self.pending.len() < QUEUE_SIZE && !self.pending.contains(text) {
            let text: Arc<str> = Arc::from(text);
            if self.requests.try_send(Arc::clone(&text)).is_ok() {
                self.pending.insert(text);
            }
        }
        None
    }

    fn collect(&mut self) -> bool {
        let mut changed = false;
        loop {
            match self.answers.try_recv() {
                Ok((text, ranges)) => {
                    self.pending.remove(&text);
                    // Completing empty results can also admit work that
                    // didn't fit the queue on a memoized view's last pass.
                    changed = true;
                    let bytes = text.len() + std::mem::size_of_val(ranges.as_slice());
                    while self.order.len() >= CACHE_SIZE || self.cache_bytes + bytes > CACHE_BYTES {
                        let Some(old) = self.order.pop_front() else {
                            break;
                        };
                        if let Some(old_ranges) = self.cache.remove(&old) {
                            self.cache_bytes -=
                                old.len() + std::mem::size_of_val(old_ranges.as_slice());
                        }
                    }
                    self.cache_bytes += bytes;
                    self.order.push_back(Arc::clone(&text));
                    self.cache.insert(text, ranges);
                }
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    changed |= !self.stopped;
                    self.stopped = true;
                    self.pending.clear();
                    break;
                }
            }
        }
        changed
    }
}

pub(super) fn provider() -> Box<dyn Spellchecker> {
    Box::new(Worker::new())
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
    }
}
