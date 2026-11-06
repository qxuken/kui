//! Resources owned by one app run, shared with its workers (ADR 0072).
//! The test controller supplies time; ordinary apps keep the system clock.
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use crate::modules::AppInfo;
use crate::ops::{Answer, Ops};

struct Scheduled {
    at: f64,
    ops: Ops,
    id: u64,
    work: Box<dyn FnOnce() -> Answer + Send>,
}

pub struct Clock {
    time: Mutex<Option<f64>>,
    scheduled: Mutex<Vec<Scheduled>>,
}

impl Clock {
    fn new(time: Option<f64>) -> Self {
        Self {
            time: Mutex::new(time),
            scheduled: Mutex::new(Vec::new()),
        }
    }
    pub fn virtual_time(&self) -> Option<f64> {
        *self.time.lock().unwrap()
    }
    pub fn now_ms(&self) -> f64 {
        self.virtual_time()
            .map(|t| 1_700_000_000_000.0 + t)
            .unwrap_or_else(|| {
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs_f64()
                    * 1000.0
            })
    }
    pub fn schedule(&self, ops: &Ops, id: u64, ms: f64, answer: Answer) {
        let at = self.virtual_time().expect("virtual scheduler") + ms.max(0.0);
        self.scheduled.lock().unwrap().push(Scheduled {
            at,
            ops: ops.clone(),
            id,
            work: Box::new(move || answer),
        });
    }
    pub fn schedule_with(
        &self,
        ops: &Ops,
        id: u64,
        ms: f64,
        work: impl FnOnce() -> Answer + Send + 'static,
    ) {
        let at = self.virtual_time().expect("virtual scheduler") + ms.max(0.0);
        self.scheduled.lock().unwrap().push(Scheduled {
            at,
            ops: ops.clone(),
            id,
            work: Box::new(work),
        });
    }
    pub fn advance(&self, ms: f64) -> Result<(), String> {
        if !ms.is_finite() || ms < 0.0 {
            return Err("time advancement must be finite and nonnegative".into());
        }
        let now = {
            let mut time = self.time.lock().unwrap();
            let t = time.as_mut().ok_or("this app uses the system clock")?;
            *t += ms;
            *t
        };
        let due = {
            let mut q = self.scheduled.lock().unwrap();
            let mut due = Vec::new();
            let mut i = 0;
            while i < q.len() {
                if q[i].at <= now {
                    due.push(q.remove(i));
                } else {
                    i += 1;
                }
            }
            due.sort_by(|a, b| a.at.total_cmp(&b.at));
            due
        };
        for d in due {
            d.ops.answer(d.id, (d.work)());
        }
        Ok(())
    }
    pub fn next_due(&self) -> Option<f64> {
        self.scheduled
            .lock()
            .unwrap()
            .iter()
            .map(|d| d.at)
            .min_by(f64::total_cmp)
    }
    pub fn clear(&self) {
        self.scheduled.lock().unwrap().clear();
    }
}

pub struct RunContext {
    pub clock: Clock,
    pub closed: std::sync::atomic::AtomicBool,
    pub args: Mutex<Vec<String>>,
    pub name: Option<String>,
    pub case_name: Mutex<Option<String>>,
    pub app: Option<AppInfo>,
    pub http: Arc<crate::modules::http::Client>,
    pub children: crate::modules::process::Children,
    pub background: Mutex<Vec<std::thread::JoinHandle<()>>>,
    pub databases: Mutex<Vec<rusqlite::InterruptHandle>>,
    pub clients: Mutex<Vec<Arc<crate::modules::http::Client>>>,
    pub secrets: Arc<Mutex<std::collections::HashMap<(String, String), String>>>,
    pub temporary_root: Option<PathBuf>,
    pub temporary: Mutex<Vec<PathBuf>>,
    pub retained: Mutex<Vec<PathBuf>>,
    pub testing: bool,
    pub effects: Arc<crate::effects::Effects>,
    pub journal: Mutex<Option<PathBuf>>,
    pub journal_error: Mutex<Option<String>>,
    pub step: std::sync::atomic::AtomicU64,
}

impl RunContext {
    pub fn new() -> Arc<Self> {
        Self::build(
            None,
            crate::modules::arguments(),
            crate::modules::name(),
            crate::modules::app().cloned(),
            None,
        )
    }
    pub fn test(args: Vec<String>, name: String, app: AppInfo, root: PathBuf) -> Arc<Self> {
        Self::build(Some(0.0), args, Some(name), Some(app), Some(root))
    }
    fn build(
        time: Option<f64>,
        args: Vec<String>,
        name: Option<String>,
        app: Option<AppInfo>,
        root: Option<PathBuf>,
    ) -> Arc<Self> {
        Arc::new(Self {
            clock: Clock::new(time),
            closed: std::sync::atomic::AtomicBool::new(false),
            args: Mutex::new(args),
            name,
            app,
            case_name: Mutex::default(),
            http: crate::modules::http::new_default(),
            children: Default::default(),
            background: Mutex::default(),
            databases: Mutex::default(),
            clients: Mutex::default(),
            secrets: Default::default(),
            temporary_root: root,
            temporary: Mutex::default(),
            retained: Mutex::default(),
            testing: time.is_some(),
            effects: Arc::new(crate::effects::Effects::new(time.is_some())),
            journal: Mutex::default(),
            journal_error: Mutex::default(),
            step: std::sync::atomic::AtomicU64::new(0),
        })
    }
    pub fn restart(&self, args: Vec<String>) -> Arc<Self> {
        let mut fresh = Self::build(
            self.clock.virtual_time(),
            args,
            self.name.clone(),
            self.app.clone(),
            self.temporary_root.clone(),
        );
        let new = Arc::get_mut(&mut fresh).unwrap();
        *new.case_name.lock().unwrap() = self.case_name.lock().unwrap().clone();
        new.effects = self.effects.clone();
        new.secrets = self.secrets.clone();
        *new.journal.lock().unwrap() = self.journal.lock().unwrap().clone();
        *new.journal_error.lock().unwrap() = self.journal_error.lock().unwrap().clone();
        *new.temporary.lock().unwrap() = std::mem::take(&mut *self.temporary.lock().unwrap());
        *new.retained.lock().unwrap() = std::mem::take(&mut *self.retained.lock().unwrap());
        new.step.store(
            self.step.load(std::sync::atomic::Ordering::Relaxed),
            std::sync::atomic::Ordering::Relaxed,
        );
        self.clock.clear();
        fresh
    }
    pub fn record(&self, kind: &str, actor: Option<u64>, data: serde_json::Value) {
        self.record_at(
            kind,
            actor,
            self.step.load(std::sync::atomic::Ordering::Relaxed),
            data,
        );
    }
    pub fn record_at(&self, kind: &str, actor: Option<u64>, step: u64, data: serde_json::Value) {
        use std::io::Write;
        let journal = self.journal.lock().unwrap();
        if let Some(path) = journal.as_ref() {
            let event = serde_json::json!({"kind":kind,"case":self.case_name.lock().unwrap().clone(),"actor":actor,"step":step,"time":self.clock.virtual_time(),"data":data});
            if let Err(error) = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(path)
                .and_then(|mut f| writeln!(f, "{}", event))
            {
                self.journal_error
                    .lock()
                    .unwrap()
                    .get_or_insert_with(|| format!("cannot record {}: {error}", path.display()));
            }
        }
    }
    pub fn retain(&self, path: PathBuf) -> Result<(), String> {
        self.retained.lock().unwrap().push(path.clone());
        if let Some(journal) = self.journal.lock().unwrap().as_ref() {
            let manifest = journal.with_file_name("retained.json");
            let mut paths = std::fs::read(&manifest)
                .ok()
                .and_then(|v| serde_json::from_slice::<Vec<PathBuf>>(&v).ok())
                .unwrap_or_default();
            paths.push(path);
            crate::testing::artifact::atomic_json(&manifest, &paths)?;
        }
        Ok(())
    }
    pub fn cleanup(&self) {
        self.closed
            .store(true, std::sync::atomic::Ordering::Relaxed);
        self.clock.clear();
        self.children.kill_all();
        for db in self.databases.lock().unwrap().drain(..) {
            db.interrupt();
        }

        crate::modules::http::shutdown(&self.http);
        for client in self.clients.lock().unwrap().drain(..) {
            crate::modules::http::shutdown(&client);
        }
        for thread in self.background.lock().unwrap().drain(..) {
            let _ = thread.join();
        }
        for p in self.temporary.lock().unwrap().drain(..).rev() {
            cleanup_except(&p, &self.retained.lock().unwrap());
        }
    }
}
impl Drop for RunContext {
    fn drop(&mut self) {
        self.cleanup();
    }
}

pub fn cleanup_except(path: &std::path::Path, retained: &[PathBuf]) {
    if retained.iter().any(|keep| path.starts_with(keep)) {
        return;
    }
    if retained.iter().any(|keep| keep.starts_with(path)) {
        if let Ok(entries) = std::fs::read_dir(path) {
            for entry in entries.flatten() {
                cleanup_except(&entry.path(), retained);
            }
        }
    } else if path.is_dir() {
        let _ = std::fs::remove_dir_all(path);
    } else {
        let _ = std::fs::remove_file(path);
    }
}
