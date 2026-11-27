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

impl Worker {
    pub fn new() -> Self {
        let (requests, inbox) = mpsc::sync_channel::<Arc<str>>(QUEUE_SIZE);
        let (outbox, answers) = mpsc::channel();
        let thread = std::thread::Builder::new()
            .name("kui-spelling".into())
            .spawn(move || {
                let Ok(checker) = spellkit::Checker::new() else {
                    return;
                };
                for text in inbox {
                    let mut ranges: Vec<_> =
                        checker.check(&text).map(|error| error.range()).collect();
                    ranges.sort_unstable_by_key(|r| r.start);
                    if outbox.send((text, ranges)).is_err() {
                        break;
                    }
                }
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
