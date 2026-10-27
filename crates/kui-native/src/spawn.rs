//! One child process started at a time, whichever thread asks (backlog
//! F223): how kui starts every process — the runner's `kui.process`, the
//! tools [`crate::show_in_folder`] and [`crate::open_external`] run, the
//! devtools' "open folder" — and the door an app's own spawns take to
//! be safe beside them.
//!
//! A child inherits every descriptor of this process that is not
//! close-on-exec at the moment it is made, and on macOS a pipe is not
//! born that way: std makes it with `pipe()` and marks its ends with an
//! `fcntl` after (there is no `pipe2`). A process started by another
//! thread between the two calls keeps the unmarked ends for as long as
//! it lives, and what waits on the pipe waits that long: a `kui.process
//! .run` from a worker whose output never ends because a `spawn` on the
//! app's thread took its write end, a long-lived child holding it. So
//! making a child's pipes and starting it are one step under one lock,
//! as Go's `syscall.ForkLock`: when a child is made, every other spawn's
//! pipes are not made yet or marked already. It is held for the start
//! alone, never for the child's life. Linux makes a pipe close-on-exec
//! as it makes it (`pipe2`) and std on Windows spawns under a lock of its
//! own, so there it costs a lock and saves nothing.
//!
//! A host that already serializes its spawns under a lock of its own
//! hands kui that way instead ([`set_spawner`]): then every start here
//! is the host's, and kui's children are inside its lock too.

use std::io;
use std::process::{Child, Command, ExitStatus, Output, Stdio};
use std::sync::{Arc, Mutex, RwLock};

/// The host's way of starting a child ([`set_spawner`]).
type Spawner = Arc<dyn Fn(&mut Command) -> io::Result<Child> + Send + Sync>;

static SPAWNER: RwLock<Option<Spawner>> = RwLock::new(None);

/// What every start here is made under while no host has its own.
static LOCK: Mutex<()> = Mutex::new(());

/// Starts every child kui starts with `spawn` (backlog F222), where kui
/// held its own lock (F223): for a host that starts every child one way,
/// under a lock of its own against macOS's pipes, so kui's are inside it
/// too. Rust's: a C, Odin or Node host's processes are its own, and kui's
/// in their process are started under kui's lock.
pub fn set_spawner(spawn: impl Fn(&mut Command) -> io::Result<Child> + Send + Sync + 'static) {
    *SPAWNER.write().unwrap_or_else(|e| e.into_inner()) = Some(Arc::new(spawn));
}

/// `command.spawn()`, no other of kui's spawns under way: the host's
/// spawner when it set one, else under kui's lock.
pub fn spawn(command: &mut Command) -> io::Result<Child> {
    let spawner = SPAWNER.read().unwrap_or_else(|e| e.into_inner()).clone();
    match spawner {
        Some(spawn) => spawn(command),
        None => {
            let _held = LOCK.lock().unwrap_or_else(|e| e.into_inner());
            command.spawn()
        }
    }
}

/// `command.status()`: started as [`spawn`] starts it, waited for
/// outside the lock.
pub fn status(command: &mut Command) -> io::Result<ExitStatus> {
    spawn(command)?.wait()
}

/// `command.output()`: started as [`spawn`] starts it, read outside the
/// lock. Its stdin is nothing, its stdout and stderr what it returns,
/// as std's `output` sets them.
pub fn output(command: &mut Command) -> io::Result<Output> {
    command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    spawn(command)?.wait_with_output()
}

/// Back to kui's own lock, for a test that set a spawner.
#[cfg(test)]
pub(crate) fn reset_spawner() {
    *SPAWNER.write().unwrap_or_else(|e| e.into_inner()) = None;
}

/// Held by a test that sets a spawner, or depends on there being none:
/// the spawner is the process's.
#[cfg(test)]
pub(crate) static TEST_SPAWNER: Mutex<()> = Mutex::new(());

#[cfg(all(test, unix))]
mod tests {
    use std::process::{Child, Command, Stdio};
    use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
    use std::sync::{Arc, Mutex};
    use std::thread;
    use std::time::{Duration, Instant};

    /// Threads starting long-lived children, and how many each a round.
    const SERVERS: usize = 2;
    const EACH: usize = 100;
    /// Threads running short jobs, one after another, for as long as the
    /// servers are at it.
    const JOBBERS: usize = 8;
    const ROUNDS: usize = 10;
    /// A job's `true` is a few milliseconds; this is a loaded machine's
    /// worth of patience, and far short of the `sleep`.
    const BOUND: Duration = Duration::from_secs(20);

    /// Long-lived children started by some threads — a `kui.process
    /// .spawn` — while others run short jobs through [`super::output`] —
    /// a worker's `kui.process.run`: every job reports its output while
    /// the long-lived ones are all still alive. Without the lock, on
    /// macOS, a `sleep` started between a job's `pipe()` and its `fcntl`
    /// holds the pipe's write end, and the job's output never ends until
    /// the `sleep` is killed — what the watchdog here sees as a job not
    /// done within [`BOUND`].
    ///
    /// A race, not a certainty: there is no hook between std's two calls
    /// to hold a thread at. The numbers are kawoosh's, whose same test
    /// lost 40 times of 40 without its lock on a 12-core Mac and passes in
    /// about half a second with it. On Linux a pipe is born close-on-exec
    /// and this passes either way.
    #[test]
    fn a_job_ends_whatever_was_started_beside_it() {
        let _one = super::TEST_SPAWNER
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let (mut jobs, mut made) = (0, 0);
        for _ in 0..ROUNDS {
            let (done, beside, held) = round();
            jobs += done;
            made += beside;
            assert!(
                !held,
                "a job was not done in {BOUND:?}: one of the long-lived children started \
                 beside it held its pipe ({made} of them so far, {jobs} jobs done)"
            );
        }
        assert!(jobs > 0, "no job ran beside the {made} children");
    }

    /// One round: the servers start their `sleep`s while the jobbers run
    /// `true` through `output`, a watchdog watching each job under way;
    /// then every `sleep` killed. The jobs done, the children made, and
    /// whether a job was held.
    fn round() -> (u64, usize, bool) {
        let started = Instant::now();
        let alive: Arc<Mutex<Vec<Child>>> = Arc::default();
        let serving = Arc::new(AtomicUsize::new(SERVERS));
        let give_up = Arc::new(AtomicBool::new(false));
        let servers: Vec<_> = (0..SERVERS)
            .map(|_| {
                let (alive, serving, give_up) = (alive.clone(), serving.clone(), give_up.clone());
                thread::spawn(move || {
                    for _ in 0..EACH {
                        if give_up.load(Ordering::Relaxed) {
                            break;
                        }
                        let child = super::spawn(
                            Command::new("sleep")
                                .arg("600")
                                .stdin(Stdio::null())
                                .stdout(Stdio::null())
                                .stderr(Stdio::null()),
                        )
                        .expect("sleep");
                        alive.lock().unwrap().push(child);
                    }
                    serving.fetch_sub(1, Ordering::Relaxed);
                })
            })
            .collect();
        // When each jobber's job under way was started, in milliseconds
        // since `started`, from 1; 0 between jobs.
        let under_way: Arc<Vec<AtomicU64>> =
            Arc::new((0..JOBBERS).map(|_| AtomicU64::new(0)).collect());
        let jobbers: Vec<_> = (0..JOBBERS)
            .map(|n| {
                let (serving, give_up, under_way) =
                    (serving.clone(), give_up.clone(), under_way.clone());
                thread::spawn(move || {
                    let mut jobs = 0u64;
                    while serving.load(Ordering::Relaxed) > 0 && !give_up.load(Ordering::Relaxed) {
                        under_way[n]
                            .store(started.elapsed().as_millis() as u64 + 1, Ordering::Relaxed);
                        let out = super::output(&mut Command::new("true")).expect("true");
                        assert!(out.status.success());
                        under_way[n].store(0, Ordering::Relaxed);
                        jobs += 1;
                    }
                    jobs
                })
            })
            .collect();
        // The watchdog: a job under way for longer than the bound is one
        // a `sleep` holds, and killing them lets it go.
        let mut held = false;
        while !held && !jobbers.iter().all(|j| j.is_finished()) {
            let now = started.elapsed().as_millis() as u64 + 1;
            held = under_way.iter().any(|at| {
                let at = at.load(Ordering::Relaxed);
                at != 0 && now.saturating_sub(at) > BOUND.as_millis() as u64
            });
            thread::sleep(Duration::from_millis(1));
        }
        give_up.store(held, Ordering::Relaxed);
        for s in servers {
            s.join().unwrap();
        }
        let made = {
            let mut alive = alive.lock().unwrap();
            for c in alive.iter_mut() {
                let _ = c.kill();
                let _ = c.wait();
            }
            alive.len()
        };
        let done = jobbers.into_iter().map(|j| j.join().unwrap()).sum();
        (done, made, held)
    }
}
