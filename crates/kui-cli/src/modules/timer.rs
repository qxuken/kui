//! `kui.sleep`'s clock: one thread holding every deadline in a heap, which
//! answers each sleep's operation when its time comes — so a hundred timers
//! are one thread, not a hundred. `kui.timer.after` and `every` are Lua over
//! `kui.sleep` (lua/runtime.lua).

use std::collections::BinaryHeap;
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

use kui_core::Value;
use mlua::{Lua, Table};

use crate::ops::Ops;

use super::shared;

#[derive(PartialEq, Eq)]
struct Due(Instant, u64);

impl Ord for Due {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        // A min-heap on the deadline.
        other.0.cmp(&self.0).then(other.1.cmp(&self.1))
    }
}

impl PartialOrd for Due {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

struct Clock {
    heap: Mutex<BinaryHeap<Due>>,
    bell: Condvar,
}

/// The state's timers; the thread starts with the first sleep.
pub struct Timers {
    ops: Ops,
    clock: Arc<Clock>,
    started: std::sync::Once,
}

impl Timers {
    pub fn new(ops: Ops) -> Self {
        Timers {
            ops,
            clock: Arc::new(Clock {
                heap: Mutex::new(BinaryHeap::new()),
                bell: Condvar::new(),
            }),
            started: std::sync::Once::new(),
        }
    }

    /// Answers `id` with nothing after `ms`.
    pub fn after(&self, id: u64, ms: f64) {
        self.started.call_once(|| {
            let clock = self.clock.clone();
            let ops = self.ops.clone();
            std::thread::Builder::new()
                .name("kui-timers".into())
                .spawn(move || run(clock, ops))
                .expect("spawn the timer thread");
        });
        let at = Instant::now() + Duration::from_secs_f64(ms.max(0.0) / 1000.0);
        self.clock.heap.lock().unwrap().push(Due(at, id));
        self.clock.bell.notify_one();
    }
}

fn run(clock: Arc<Clock>, ops: Ops) {
    let mut heap = clock.heap.lock().unwrap();
    loop {
        let now = Instant::now();
        while heap.peek().is_some_and(|d| d.0 <= now) {
            let Due(_, id) = heap.pop().unwrap();
            ops.answer(id, Ok(Value::Null));
        }
        heap = match heap.peek() {
            Some(d) => {
                let wait = d.0.saturating_duration_since(Instant::now());
                clock.bell.wait_timeout(heap, wait).unwrap().0
            }
            None => clock.bell.wait(heap).unwrap(),
        };
    }
}

pub fn install(lua: &Lua, kui: &Table) -> mlua::Result<()> {
    kui.set(
        "__sleep",
        lua.create_function(|lua, ms: f64| {
            let s = shared(lua);
            let id = s.ops.id();
            s.timers.after(id, ms);
            Ok(id)
        })?,
    )?;
    Ok(())
}
