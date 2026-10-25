//! What a waiting call waits on (ADR 0064, decision 4): an operation the
//! Rust side runs on another thread and answers by id. The answer goes in
//! the inbox, the loop is woken, and the Lua side resumes the task that
//! waited on that id — or, outside a task, the caller blocks on the inbox
//! until its own answer is there.
//!
//! One `Ops` per Lua state: the app's, woken through the window's (or the
//! headless runner's) `Waker`, and each worker's, woken by its own
//! condition variable.

use std::collections::VecDeque;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Condvar, Mutex};

use kui_core::Value;

/// An operation's answer: a plain value, or what went wrong.
pub type Answer = Result<Value, String>;

/// Something that arrived for the Lua state besides an answer: a message a
/// worker dispatched, a request the server took, a file that changed.
pub enum Inbound {
    /// A message for the app's `update` (from a worker, by its id).
    Dispatch { from: Option<u64>, msg: Value },
    /// A request the server took: the handler runs as a task.
    Request { server: u64, id: u64, req: Value },
    /// A worker's state changed: it threw, or its module returned.
    WorkerExit { id: u64, error: Option<String> },
    /// A message for a worker's `on_message`, from the app.
    Message(Value),
}

#[derive(Default)]
struct Inbox {
    answers: Vec<(u64, Answer)>,
    inbound: VecDeque<Inbound>,
}

/// How the loop that owns the Lua state is told something arrived.
pub type Wake = Box<dyn Fn() + Send + Sync>;

struct Inner {
    next: AtomicU64,
    inbox: Mutex<Inbox>,
    ready: Condvar,
    wake: Mutex<Option<Wake>>,
}

/// The inbox of one Lua state; cheap to clone into the threads that answer.
#[derive(Clone)]
pub struct Ops(Arc<Inner>);

impl Default for Ops {
    fn default() -> Self {
        Ops(Arc::new(Inner {
            next: AtomicU64::new(1),
            inbox: Mutex::new(Inbox::default()),
            ready: Condvar::new(),
            wake: Mutex::new(None),
        }))
    }
}

impl Ops {
    /// A fresh id for an operation about to start.
    pub fn id(&self) -> u64 {
        self.0.next.fetch_add(1, Ordering::Relaxed)
    }

    /// What rings the owning loop: the window's waker, set once it exists.
    pub fn set_wake(&self, wake: Wake) {
        *self.0.wake.lock().unwrap() = Some(wake);
    }

    fn ring(&self) {
        self.0.ready.notify_all();
        if let Some(w) = self.0.wake.lock().unwrap().as_ref() {
            w();
        }
    }

    /// Answers operation `id`, from any thread.
    pub fn answer(&self, id: u64, answer: Answer) {
        self.0.inbox.lock().unwrap().answers.push((id, answer));
        self.ring();
    }

    /// Leaves something for the Lua state, from any thread.
    pub fn send(&self, msg: Inbound) {
        self.0.inbox.lock().unwrap().inbound.push_back(msg);
        self.ring();
    }

    /// Rings the loop with nothing in the inbox: a dispatch queued from Lua
    /// wants the next frame.
    pub fn poke(&self) {
        self.ring();
    }

    /// Everything that arrived since the last call.
    pub fn take(&self) -> (Vec<(u64, Answer)>, VecDeque<Inbound>) {
        let mut inbox = self.0.inbox.lock().unwrap();
        (
            std::mem::take(&mut inbox.answers),
            std::mem::take(&mut inbox.inbound),
        )
    }

    /// Blocks until operation `id` is answered, leaving every other answer
    /// where it is: a waiting call made outside a task.
    pub fn block_on(&self, id: u64) -> Answer {
        let mut inbox = self.0.inbox.lock().unwrap();
        loop {
            if let Some(i) = inbox.answers.iter().position(|(a, _)| *a == id) {
                return inbox.answers.remove(i).1;
            }
            inbox = self.0.ready.wait(inbox).unwrap();
        }
    }

    /// Blocks until anything arrives or `timeout` passes: a worker's idle.
    pub fn wait_any(&self, timeout: std::time::Duration) {
        let inbox = self.0.inbox.lock().unwrap();
        if inbox.answers.is_empty() && inbox.inbound.is_empty() {
            let _ = self.0.ready.wait_timeout(inbox, timeout);
        }
    }
}

/// Runs `work` on a thread of its own and answers `id` with what it
/// returns: the shape of every blocking call (a request, a process, a
/// sleep) the modules make.
pub fn run(ops: &Ops, id: u64, work: impl FnOnce() -> Answer + Send + 'static) {
    let ops = ops.clone();
    std::thread::spawn(move || {
        let answer = work();
        ops.answer(id, answer);
    });
}
