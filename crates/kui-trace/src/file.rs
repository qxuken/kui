//! A recording's file as a [`Transport`] (backlog F249, ADR 0075 decisions
//! 5 and 6): what was appended since the last poll, or the whole again when
//! the file was rewritten — shorter than what was read, or its first bytes
//! changed; and a [`Watch`] that checks the file every so often on a thread
//! of its own and calls back only when its size or its modification time
//! moved, so a reader idles at no cost.

use std::fs::File;
use std::io::{self, Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, SystemTime};

use crate::stream::{Polled, Transport};

/// How many of a file's first bytes say it is the recording read so far.
const HEAD: usize = 64;
/// How much one poll reads at most, so a reader of a long file draws while
/// it reads.
const CHUNK: usize = 8 << 20;

/// A recording's file, read as it grows.
pub struct FileTransport {
    path: PathBuf,
    file: File,
    offset: u64,
    head: Vec<u8>,
    follow: bool,
}

impl FileTransport {
    /// The file at `path`, from its start. With `follow`, its end is
    /// [`Polled::Idle`] — it may still be written — and not
    /// [`Polled::Ended`].
    pub fn open(path: impl AsRef<Path>, follow: bool) -> io::Result<Self> {
        let path = path.as_ref().to_path_buf();
        Ok(FileTransport {
            file: File::open(&path)?,
            path,
            offset: 0,
            head: Vec::new(),
            follow,
        })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Whether the file is still the one read so far: as long as what was
    /// read, its first bytes the same.
    fn same(&mut self, len: u64) -> io::Result<bool> {
        if len < self.offset {
            return Ok(false);
        }
        if self.head.is_empty() {
            return Ok(true);
        }
        let mut now = vec![0; self.head.len()];
        self.file.seek(SeekFrom::Start(0))?;
        self.file.read_exact(&mut now)?;
        Ok(now == self.head)
    }

    fn read_from(&mut self, at: u64, buf: &mut Vec<u8>) -> io::Result<usize> {
        self.file.seek(SeekFrom::Start(at))?;
        let before = buf.len();
        (&mut self.file).take(CHUNK as u64).read_to_end(buf)?;
        let n = buf.len() - before;
        // Reads run on from the start, so the head is filled while what
        // is read follows straight on from it.
        if self.head.len() < HEAD && at == self.head.len() as u64 {
            let take = (HEAD - self.head.len()).min(n);
            self.head.extend_from_slice(&buf[before..before + take]);
        }
        self.offset = at + n as u64;
        Ok(n)
    }
}

impl Transport for FileTransport {
    fn poll(&mut self, buf: &mut Vec<u8>) -> io::Result<Polled> {
        // A file replaced under its name is another inode: open the name.
        let file = File::open(&self.path)?;
        self.file = file;
        let len = self.file.metadata()?.len();
        if !self.same(len)? {
            self.offset = 0;
            self.head.clear();
            self.read_from(0, buf)?;
            return Ok(Polled::Reset);
        }
        if len == self.offset {
            return Ok(if self.follow {
                Polled::Idle
            } else {
                Polled::Ended
            });
        }
        let n = self.read_from(self.offset, buf)?;
        Ok(Polled::More(n))
    }
}

/// A file watched on a thread of its own: `on_change` runs there whenever
/// the file's size or modification time moved since the last look. The
/// thread stops when the `Watch` drops.
pub struct Watch {
    stop: Arc<AtomicBool>,
}

impl Watch {
    /// Watches `path` every `every` (ADR 0075: 250 ms), as it stands now.
    pub fn spawn(
        path: impl AsRef<Path>,
        every: Duration,
        on_change: impl Fn() + Send + 'static,
    ) -> io::Result<Watch> {
        let path = path.as_ref().to_path_buf();
        let stop = Arc::new(AtomicBool::new(false));
        let stopped = stop.clone();
        let stamp = |p: &Path| -> Option<(u64, SystemTime)> {
            let m = std::fs::metadata(p).ok()?;
            Some((m.len(), m.modified().ok()?))
        };
        let mut last = stamp(&path);
        std::thread::Builder::new()
            .name("kui-trace-watch".into())
            .spawn(move || {
                while !stopped.load(Ordering::Relaxed) {
                    std::thread::sleep(every);
                    if stopped.load(Ordering::Relaxed) {
                        break;
                    }
                    let now = stamp(&path);
                    if now != last {
                        last = now;
                        on_change();
                    }
                }
            })?;
        Ok(Watch { stop })
    }
}

impl Drop for Watch {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}
