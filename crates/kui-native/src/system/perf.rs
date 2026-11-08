use kui_core::kui_trace::{Record, Trace, Writer, adapters::Format};
use kui_core::perf::{PerfClock, PerfHost, TraceOutput};
use std::fs::File;
use std::io::{self, Write};
use std::rc::Rc;
use std::sync::OnceLock;
use std::sync::mpsc::{self, Receiver, SyncSender};
use std::thread::JoinHandle;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

struct Clock;

impl PerfClock for Clock {
    fn now_us(&self) -> u64 {
        static EPOCH: OnceLock<Instant> = OnceLock::new();
        EPOCH.get_or_init(Instant::now).elapsed().as_micros() as u64
    }

    #[cfg(unix)]
    fn cpu_us(&self) -> Option<u64> {
        let mut time = libc::timespec {
            tv_sec: 0,
            tv_nsec: 0,
        };
        // SAFETY: clock_gettime fills the timespec supplied by the caller.
        let result = unsafe { libc::clock_gettime(libc::CLOCK_THREAD_CPUTIME_ID, &mut time) };
        (result == 0).then(|| time.tv_sec as u64 * 1_000_000 + time.tv_nsec as u64 / 1_000)
    }

    fn wall_ms(&self) -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |time| time.as_millis() as u64)
    }
}

pub(super) fn host() -> PerfHost {
    let host = PerfHost::new(Some(Rc::new(Clock)), Some(open));
    #[cfg(feature = "tracing")]
    let host = host.with_observer(Rc::new(Tracing));
    host
}

/// Gives a caller-supplied core native recording capabilities and
/// applies the native launch option for file recording.
pub fn configure_recording(core: &mut kui_core::Core) {
    core.supply_perf_host(host());
    if let Some(path) = std::env::var_os("KUI_PERF_TRACE") {
        let path = path.to_string_lossy();
        if let Err(error) = core.perf_trace_to(&path) {
            core.report_perf_trace_error(&path, &error);
        }
    }
}

enum FileOutput {
    Stream {
        writer: Writer<StreamFile>,
        error: Option<io::Error>,
    },
    Convert {
        path: String,
        format: Format,
        trace: Trace,
        dirty: bool,
    },
}

fn open(path: &str) -> io::Result<Box<dyn TraceOutput>> {
    let file = File::create(path)?;
    let output = match Format::from_path(path) {
        Format::Kuitrace => FileOutput::Stream {
            writer: Writer::new(StreamFile::new(file)?)?,
            error: None,
        },
        format => FileOutput::Convert {
            path: path.to_owned(),
            format,
            trace: Trace::new(),
            dirty: false,
        },
    };
    Ok(Box::new(output))
}

impl TraceOutput for FileOutput {
    fn record(&mut self, record: &Record) -> io::Result<()> {
        match self {
            Self::Stream { writer, error } => {
                if let Some(error) = error {
                    return Err(io::Error::new(error.kind(), error.to_string()));
                }
                writer.write(record).map_err(|cause| {
                    let failure = io::Error::new(cause.kind(), cause.to_string());
                    *error = Some(cause);
                    failure
                })
            }
            Self::Convert { trace, dirty, .. } => {
                trace.push(record.clone());
                *dirty = true;
                Ok(())
            }
        }
    }

    fn drain(&mut self) -> io::Result<()> {
        match self {
            Self::Stream { writer, error } => match error {
                Some(error) => Err(io::Error::new(error.kind(), error.to_string())),
                None => writer.output_mut().send_batch(),
            },
            Self::Convert { .. } => Ok(()),
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        match self {
            Self::Stream { writer, error } => match error {
                Some(error) => Err(io::Error::new(error.kind(), error.to_string())),
                None => writer.flush(),
            },
            Self::Convert {
                path,
                format,
                trace,
                dirty,
            } => {
                if *dirty {
                    std::fs::write(path, kui_core::kui_trace::adapters::write(trace, *format))?;
                    *dirty = false;
                }
                Ok(())
            }
        }
    }
}

impl Drop for FileOutput {
    fn drop(&mut self) {
        if matches!(self, Self::Convert { .. }) {
            let _ = self.flush();
        }
    }
}

const BATCH_BYTES: usize = 8 * 1024;
const QUEUED_BATCHES: usize = 4;

enum Job {
    Write(Vec<u8>),
    Flush(SyncSender<io::Result<()>>),
    Finish,
}

/// The encoder fills a reusable buffer on the recording thread. Only
/// whole buffers go to the writer, with backpressure if the disk falls behind.
struct StreamFile {
    buffer: Vec<u8>,
    jobs: SyncSender<Job>,
    recycled: Receiver<Vec<u8>>,
    thread: Option<JoinHandle<()>>,
}

impl StreamFile {
    fn new(mut file: File) -> io::Result<Self> {
        let (jobs, work) = mpsc::sync_channel(QUEUED_BATCHES);
        let (reuse, recycled) = mpsc::sync_channel(QUEUED_BATCHES);
        let thread = std::thread::Builder::new()
            .name("kui-trace-writer".into())
            .spawn(move || {
                let mut error = None;
                for job in work {
                    match job {
                        Job::Write(mut bytes) => {
                            if error.is_none() {
                                error = file.write_all(&bytes).err();
                            }
                            bytes.clear();
                            let _ = reuse.try_send(bytes);
                        }
                        Job::Flush(done) => {
                            let result = match &error {
                                Some(e) => Err(io::Error::new(e.kind(), e.to_string())),
                                None => file.flush(),
                            };
                            let _ = done.send(result);
                        }
                        Job::Finish => break,
                    }
                }
            })?;
        Ok(Self {
            buffer: Vec::with_capacity(BATCH_BYTES),
            jobs,
            recycled,
            thread: Some(thread),
        })
    }

    fn send(&self, job: Job) -> io::Result<()> {
        self.jobs.send(job).map_err(|_| writer_stopped())
    }

    fn send_batch(&mut self) -> io::Result<()> {
        if self.buffer.is_empty() {
            return Ok(());
        }
        let empty = self
            .recycled
            .try_recv()
            .unwrap_or_else(|_| Vec::with_capacity(BATCH_BYTES));
        let batch = std::mem::replace(&mut self.buffer, empty);
        self.send(Job::Write(batch))
    }
}

impl Write for StreamFile {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if self.buffer.len() + bytes.len() > BATCH_BYTES {
            self.send_batch()?;
        }
        self.buffer.extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        self.send_batch()?;
        let (done, result) = mpsc::sync_channel(1);
        self.send(Job::Flush(done))?;
        result.recv().map_err(|_| writer_stopped())?
    }
}

impl Drop for StreamFile {
    fn drop(&mut self) {
        let _ = self.flush();
        let _ = self.send(Job::Finish);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

fn writer_stopped() -> io::Error {
    io::Error::new(io::ErrorKind::BrokenPipe, "trace writer stopped")
}

#[cfg(feature = "tracing")]
struct Tracing;

#[cfg(feature = "tracing")]
impl kui_core::perf::PerfObserver for Tracing {
    fn enabled(&self) -> bool {
        tracing::enabled!(target: "kui", tracing::Level::TRACE)
    }

    fn enter(&self, name: &str, node: u64) -> Box<dyn std::any::Any> {
        Box::new(tracing::trace_span!(target: "kui", "kui", name = name, node = node).entered())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn idle_driver_turn_publishes_a_short_recording() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("short.kuitrace");
        let mut core = kui_core::Core::new();
        core.supply_perf_host(host());
        core.perf_trace_to(path.to_str().unwrap()).unwrap();
        core.perf_mark("last input before idle");
        core.perf_drain().unwrap();
        let deadline = Instant::now() + std::time::Duration::from_secs(2);
        loop {
            let bytes = std::fs::read(&path).unwrap();
            if bytes
                .windows(b"last input before idle".len())
                .any(|bytes| bytes == b"last input before idle")
            {
                assert!(Trace::from_bytes(&bytes).is_ok());
                break;
            }
            assert!(
                Instant::now() < deadline,
                "idle trace records never reached the file"
            );
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
    }

    #[test]
    fn stream_flush_and_drop_publish_records_in_order() {
        let path = std::env::temp_dir().join(format!("kui-stream-{}.kuitrace", std::process::id()));
        let file = File::create(&path).unwrap();
        let mut writer = Writer::new(StreamFile::new(file).unwrap()).unwrap();
        let names = (1..=96).map(|id| (id, format!("{id}: {}", "x".repeat(128))));
        for (id, text) in names.clone() {
            writer.write(&Record::Name { id, text }).unwrap();
        }
        writer.flush().unwrap();
        let trace = Trace::from_bytes(&std::fs::read(&path).unwrap()).unwrap();
        for (id, text) in names {
            assert_eq!(trace.name(id), text);
        }
        writer
            .write(&Record::Name {
                id: 97,
                text: "written at shutdown".into(),
            })
            .unwrap();
        drop(writer);
        let bytes = std::fs::read(&path).unwrap();
        std::fs::remove_file(path).unwrap();
        assert_eq!(
            Trace::from_bytes(&bytes).unwrap().name(97),
            "written at shutdown"
        );
    }
}
