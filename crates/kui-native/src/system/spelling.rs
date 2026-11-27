//! One thread-affine dictionary per interested window. Checking text and
//! requesting a context menu never wait for a system spelling service.
use kui_core::spelling::{Spellchecker, SpellingChoices};
use std::collections::{HashMap, HashSet, VecDeque};
use std::ops::Range;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};

#[cfg(any(target_os = "windows", target_os = "macos", target_os = "linux"))]
#[path = "spelling/desktop.rs"]
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

enum Request {
    Check(Arc<str>),
    Choices(Arc<str>),
    Learn(String),
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
    #[cfg(any(target_os = "macos", target_os = "ios"))]
    {
        objc2::rc::autoreleasepool(|_| f())
    }
    #[cfg(not(any(target_os = "macos", target_os = "ios")))]
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
            for request in inbox {
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
        !self.pending.is_empty()
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
    target_os = "ios"
))]
pub(super) fn provider() -> Box<dyn Spellchecker> {
    worker(backend::Dictionary::open)
}
