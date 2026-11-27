//! One thread-affine dictionary per interested window. Checking text and
//! requesting a context menu never wait for a system spelling service.
use kui_core::spelling::{Spellchecker, SpellingChoices};
use std::collections::{HashMap, HashSet, VecDeque};
use std::ops::Range;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};
use std::sync::{Arc, Mutex};

#[cfg(any(target_os = "windows", target_os = "macos", target_os = "linux"))]
#[path = "spelling/desktop.rs"]
mod backend;
#[cfg(all(target_os = "ios", feature = "mobile"))]
#[path = "spelling/ios.rs"]
mod backend;

pub(crate) trait Dictionary {
    fn check(&self, text: &str) -> Vec<Range<usize>>;
    fn choices(&self, word: &str) -> SpellingChoices;
    fn learn(&mut self, word: &str) -> bool;
}

const CACHE_SIZE: usize = 2048;
const QUEUE_SIZE: usize = 64;
const CACHE_BYTES: usize = 8 * 1024 * 1024;
static GENERATION: AtomicU64 = AtomicU64::new(0);
// Personal dictionaries can retain an in-memory word list. Serialize the
// reload and write so concurrent additions use the latest dictionary.
static LEARN_LOCK: Mutex<()> = Mutex::new(());

enum Request {
    Check(Arc<str>),
    Choices(Arc<str>),
    Learn(String),
}

// A menu should wait for at most the check already in progress, even when a
// newly opened document has queued dozens of paragraphs. Only the current
// menu can receive suggestions. Explicit dictionary additions are all kept.
fn next_request(inbox: &Receiver<Request>, queue: &mut VecDeque<Request>) -> Option<Request> {
    if queue.is_empty() {
        queue.push_back(inbox.recv().ok()?);
    }
    for request in inbox.try_iter() {
        if matches!(request, Request::Choices(_)) {
            queue.retain(|r| !matches!(r, Request::Choices(_)));
        }
        queue.push_back(request);
    }
    let index = queue
        .iter()
        .position(|r| !matches!(r, Request::Check(_)))
        .unwrap_or(0);
    queue.remove(index)
}

enum Answer {
    Checked(u64, Arc<str>, Vec<Range<usize>>),
    Choices(u64, Arc<str>, SpellingChoices),
    Learned,
}

struct Worker {
    requests: Sender<Request>,
    answers: Receiver<Answer>,
    pending: HashSet<Arc<str>>,
    menu: Option<(Arc<str>, Option<SpellingChoices>)>,
    learning: usize,
    cache: HashMap<Arc<str>, Vec<Range<usize>>>,
    order: VecDeque<Arc<str>>,
    cache_bytes: usize,
    generation: u64,
    stopped: bool,
}

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

pub(crate) fn worker<D: Dictionary>(
    open: impl Fn() -> Option<D> + Send + 'static,
) -> Box<dyn Spellchecker> {
    let (requests, inbox) = mpsc::channel();
    let (outbox, answers) = mpsc::channel();
    let generation = GENERATION.load(Ordering::Acquire);
    let thread = std::thread::Builder::new()
        .name("kui-spelling".into())
        .spawn(move || {
            let mut seen = generation;
            let Some(mut dictionary) = dictionary_call(&open) else {
                return;
            };
            let mut queue = VecDeque::new();
            while let Some(request) = next_request(&inbox, &mut queue) {
                let _learn_lock = matches!(request, Request::Learn(_))
                    .then(|| LEARN_LOCK.lock().unwrap_or_else(|e| e.into_inner()));
                let latest = GENERATION.load(Ordering::Acquire);
                if seen != latest {
                    // In particular, Hunspell loads its personal list when opened.
                    let Some(new) = dictionary_call(&open) else {
                        break;
                    };
                    dictionary_call(|| drop(std::mem::replace(&mut dictionary, new)));
                    seen = latest;
                }
                let answer = dictionary_call(|| match request {
                    Request::Check(text) => {
                        let ranges = dictionary.check(&text);
                        Answer::Checked(seen, text, ranges)
                    }
                    Request::Choices(word) => {
                        let choices = dictionary.choices(&word);
                        Answer::Choices(seen, word, choices)
                    }
                    Request::Learn(word) => {
                        if dictionary.learn(&word) {
                            GENERATION.fetch_add(1, Ordering::AcqRel);
                        } else {
                            // A backend may have changed its in-memory word list before
                            // discovering a write failure. Reopen before its next use.
                            seen = latest.wrapping_sub(1);
                        }
                        Answer::Learned
                    }
                });
                if outbox.send(answer).is_err() {
                    break;
                }
            }
            dictionary_call(|| drop(dictionary));
        });
    Box::new(Worker {
        requests,
        answers,
        generation,
        stopped: thread.is_err(),
        pending: Default::default(),
        menu: None,
        learning: 0,
        cache: Default::default(),
        order: Default::default(),
        cache_bytes: 0,
    })
}

impl Spellchecker for Worker {
    fn dictionary_revision(&self) -> u64 {
        self.generation
    }

    fn pending(&self) -> bool {
        self.generation != GENERATION.load(Ordering::Acquire)
            || !self.pending.is_empty()
            || self.learning != 0
            || self
                .menu
                .as_ref()
                .is_some_and(|(_, answer)| answer.is_none())
    }

    fn choices(&mut self, word: &str) -> Option<SpellingChoices> {
        if self.stopped {
            return Some(Default::default());
        }
        if let Some((last, answer)) = &self.menu
            && &**last == word
        {
            return answer.clone();
        }
        let word: Arc<str> = Arc::from(word);
        if self
            .requests
            .send(Request::Choices(Arc::clone(&word)))
            .is_err()
        {
            return Some(Default::default());
        }
        self.menu = Some((word, None));
        None
    }

    fn learn(&mut self, word: &str) {
        if !self.stopped && self.requests.send(Request::Learn(word.to_owned())).is_ok() {
            self.learning += 1;
        }
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
            if self
                .requests
                .send(Request::Check(Arc::clone(&text)))
                .is_ok()
            {
                self.pending.insert(text);
            }
        }
        None
    }

    fn collect(&mut self) -> bool {
        let mut changed = false;
        loop {
            match self.answers.try_recv() {
                Ok(Answer::Checked(epoch, text, ranges)) => {
                    self.pending.remove(&text);
                    // Completing even a stale check admits previously deferred work.
                    changed = true;
                    if epoch != self.generation {
                        continue;
                    }
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
                Ok(Answer::Choices(epoch, word, choices)) => {
                    if let Some((waiting, answer)) = &mut self.menu
                        && *waiting == word
                    {
                        if epoch == self.generation {
                            *answer = Some(choices);
                        } else {
                            self.menu = None;
                        }
                    }
                    changed = true;
                }
                Ok(Answer::Learned) => {
                    self.learning -= 1;
                    changed = true;
                }
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    changed |= !self.stopped;
                    self.stopped = true;
                    self.pending.clear();
                    self.menu = None;
                    self.learning = 0;
                    break;
                }
            }
        }
        // Read after receiving completions, including a successful Learn.
        let latest = GENERATION.load(Ordering::Acquire);
        if self.generation != latest {
            self.generation = latest;
            self.cache.clear();
            self.order.clear();
            self.cache_bytes = 0;
            self.menu = None;
            changed = true;
        }
        changed
    }
}

#[cfg(any(
    target_os = "windows",
    target_os = "macos",
    target_os = "linux",
    all(target_os = "ios", feature = "mobile")
))]
pub(super) fn provider() -> Box<dyn Spellchecker> {
    worker(backend::Dictionary::open)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};

    fn wait<T>(mut poll: impl FnMut() -> Option<T>) -> T {
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            if let Some(value) = poll() {
                return value;
            }
            assert!(Instant::now() < deadline, "dictionary timed out");
            std::thread::sleep(Duration::from_millis(1));
        }
    }

    #[test]
    fn the_current_menu_overtakes_queued_paragraphs() {
        let (tx, rx) = mpsc::channel();
        let mut queue = VecDeque::new();
        tx.send(Request::Check("long paragraph".into())).unwrap();
        tx.send(Request::Choices("old".into())).unwrap();
        tx.send(Request::Learn("word".into())).unwrap();
        tx.send(Request::Choices("current".into())).unwrap();
        assert!(matches!(
            next_request(&rx, &mut queue),
            Some(Request::Learn(_))
        ));
        assert!(
            matches!(next_request(&rx, &mut queue), Some(Request::Choices(w)) if &*w == "current")
        );
        assert!(matches!(
            next_request(&rx, &mut queue),
            Some(Request::Check(_))
        ));
    }

    #[test]
    fn learning_in_two_windows_refreshes_shared_words_without_losing_either() {
        #[derive(Clone, Default)]
        struct PersonalList {
            disk: Arc<Mutex<Vec<String>>>,
            loaded: Vec<String>,
        }
        impl Dictionary for PersonalList {
            fn check(&self, text: &str) -> Vec<Range<usize>> {
                if self.loaded.iter().any(|s| s == text) {
                    vec![]
                } else {
                    std::iter::once(0..text.len()).collect()
                }
            }
            fn choices(&self, _: &str) -> SpellingChoices {
                SpellingChoices {
                    suggestions: vec!["suggestion".into()],
                    can_learn: true,
                }
            }
            fn learn(&mut self, word: &str) -> bool {
                self.loaded.push(word.into());
                self.disk.lock().unwrap().clone_from(&self.loaded);
                true
            }
        }
        let list = PersonalList::default();
        let window = || {
            let seed = list.clone();
            worker(move || {
                let mut opened = seed.clone();
                opened.loaded = seed.disk.lock().unwrap().clone();
                Some(opened)
            })
        };
        let (mut a, mut b) = (window(), window());
        for w in [&mut a, &mut b] {
            let result = wait(|| {
                w.collect();
                w.check("abc").map(<[_]>::to_vec)
            });
            assert_eq!(result.len(), 1);
            assert_eq!(result[0], 0..3);
        }
        assert_eq!(
            wait(|| {
                a.collect();
                a.choices("abc")
            })
            .suggestions,
            ["suggestion"]
        );
        a.learn("abc");
        b.learn("xyz");
        wait(|| {
            a.collect();
            b.collect();
            (!a.pending() && !b.pending()).then_some(())
        });
        let mut disk = list.disk.lock().unwrap().clone();
        disk.sort();
        assert_eq!(disk, ["abc", "xyz"]);
        for w in [&mut a, &mut b] {
            for word in ["abc", "xyz"] {
                let ranges = wait(|| {
                    w.collect();
                    w.check(word).map(<[_]>::to_vec)
                });
                assert!(ranges.is_empty());
            }
        }
    }
}
