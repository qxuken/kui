use super::{Mapping, Revision, Snapshots};
use std::collections::VecDeque;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::{Arc, OnceLock, mpsc};

pub(super) struct Source {
    state: Arc<State>,
}

struct State {
    path: PathBuf,
    indices: Vec<u32>,
    revision: Option<Revision>,
    mapping: OnceLock<Option<Mapping>>,
    priority: AtomicU8,
}

impl Source {
    pub(super) fn new(path: PathBuf, indices: Vec<u32>, revision: Option<Revision>) -> Self {
        Self {
            state: Arc::new(State {
                path,
                indices,
                revision,
                mapping: OnceLock::new(),
                priority: AtomicU8::new(2),
            }),
        }
    }

    pub(super) fn revision(&self) -> Option<&Revision> {
        self.state.revision.as_ref()
    }

    pub(super) fn failed(&self) -> bool {
        matches!(self.state.mapping.get(), Some(None))
    }

    #[cfg(test)]
    pub(super) fn prepared(&self) -> bool {
        self.state.mapping.get().is_some_and(Option::is_some)
    }

    pub(super) fn request(&self, urgent: bool) {
        if self.state.mapping.get().is_some() {
            return;
        }
        let priority = u8::from(!urgent);
        if self.state.priority.fetch_min(priority, Ordering::Relaxed) <= priority {
            return;
        }
        if let Some(sender) = requests() {
            let _ = sender.send(Request {
                source: self.state.clone(),
                urgent,
            });
        } else {
            self.state.load(&mut Snapshots::default());
        }
    }

    pub(super) fn prepare(&self) -> bool {
        self.request(true);
        self.state.mapping.wait().is_some()
    }
}

impl AsRef<[u8]> for Source {
    fn as_ref(&self) -> &[u8] {
        self.prepare();
        self.state
            .mapping
            .wait()
            .as_ref()
            .map_or(&[], |mapping| mapping.bytes.as_ref().as_ref())
    }
}

impl State {
    fn load(&self, snapshots: &mut Snapshots) {
        self.mapping.get_or_init(|| {
            super::map(&self.path, &self.indices, None, snapshots).filter(|mapping| {
                (self.revision.is_none() || mapping.revision == self.revision)
                    && mapping.measurable == self.indices
            })
        });
    }
}

struct Request {
    source: Arc<State>,
    urgent: bool,
}

impl Drop for Request {
    fn drop(&mut self) {
        // A stopped worker also completes its queued requests. A caller
        // waiting for font bytes must be able to fall back after failure.
        let _ = self.source.mapping.set(None);
    }
}

fn requests() -> Option<&'static mpsc::Sender<Request>> {
    static REQUESTS: OnceLock<Option<mpsc::Sender<Request>>> = OnceLock::new();
    REQUESTS
        .get_or_init(|| {
            let (sender, receiver) = mpsc::channel();
            std::thread::Builder::new()
                .name("kui-fonts".into())
                .spawn(move || {
                    let mut pending = VecDeque::new();
                    let mut snapshots = Snapshots::default();
                    while let Some(request) = next_request(&mut pending, &receiver) {
                        request.source.load(&mut snapshots);
                    }
                })
                .ok()
                .map(|_| sender)
        })
        .as_ref()
}

fn next_request(
    pending: &mut VecDeque<Request>,
    receiver: &mpsc::Receiver<Request>,
) -> Option<Request> {
    if pending.is_empty() {
        pending.push_back(receiver.recv().ok()?);
    }
    pending.extend(receiver.try_iter());
    let index = pending
        .iter()
        .position(|request| request.urgent)
        .unwrap_or(0);
    pending.remove(index)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn snapshots_wait_for_a_request_and_survive_source_truncation() {
        let bytes = include_bytes!("../../../../kui-cli/assets/fonts/NotoSans-Regular.ttf");
        let mut file = tempfile::NamedTempFile::new().unwrap();
        file.write_all(bytes).unwrap();
        let source = Source::new(
            file.path().to_owned(),
            vec![0],
            Revision::read(file.as_file()),
        );
        assert!(!source.prepared());
        assert_eq!(source.as_ref(), bytes);
        assert!(source.prepared());
        file.as_file().set_len(0).unwrap();
        assert_eq!(source.as_ref(), bytes);
    }

    #[test]
    fn requested_fonts_run_ahead_of_prefetches() {
        let low = Source::new("low".into(), vec![0], None);
        let high = Source::new("high".into(), vec![0], None);
        let (sender, receiver) = mpsc::channel();
        sender
            .send(Request {
                source: low.state.clone(),
                urgent: false,
            })
            .unwrap();
        sender
            .send(Request {
                source: high.state.clone(),
                urgent: true,
            })
            .unwrap();
        let mut pending = VecDeque::new();
        let next = next_request(&mut pending, &receiver).unwrap();
        assert!(Arc::ptr_eq(&next.source, &high.state));
        assert!(Arc::ptr_eq(
            &next_request(&mut pending, &receiver).unwrap().source,
            &low.state
        ));
    }

    #[test]
    fn cancelled_requests_release_waiters() {
        let source = Source::new("missing".into(), vec![0], None);
        drop(Request {
            source: source.state.clone(),
            urgent: true,
        });
        assert!(source.state.mapping.wait().is_none());
    }

    #[test]
    fn changed_sources_fail_without_exposing_a_different_revision() {
        let mut file = tempfile::NamedTempFile::new().unwrap();
        file.write_all(include_bytes!(
            "../../../../kui-cli/assets/fonts/NotoSans-Regular.ttf"
        ))
        .unwrap();
        let source = Source::new(
            file.path().to_owned(),
            vec![0],
            Revision::read(file.as_file()),
        );
        file.as_file().set_len(0).unwrap();
        assert!(source.as_ref().is_empty());
        assert!(source.failed());
    }
}
