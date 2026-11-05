//! Progress is delivered on the request's operation id, before its final
//! answer. The Lua waiter consumes these events on its own state.

use std::io::{self, Read};
use std::time::{Duration, Instant};

use kui_core::Value;

use crate::ops::Ops;

#[derive(Clone)]
pub(super) struct Target {
    pub ops: Ops,
    pub id: u64,
}

impl Target {
    pub fn reader<R: Read>(
        &self,
        inner: R,
        direction: &'static str,
        total: Option<u64>,
    ) -> Reader<R> {
        let mut reader = Reader {
            inner,
            target: self.clone(),
            direction,
            loaded: 0,
            total: total.filter(|n| *n <= i64::MAX as u64),
            last: Instant::now(),
            done: false,
        };
        reader.emit(false);
        if direction == "upload" && total == Some(0) {
            reader.finish();
        }
        reader
    }
}

pub(super) struct Reader<R> {
    inner: R,
    target: Target,
    direction: &'static str,
    loaded: u64,
    total: Option<u64>,
    last: Instant,
    done: bool,
}

impl<R> Reader<R> {
    fn emit(&mut self, done: bool) {
        let mut fields = vec![
            ("__http_progress", Value::Bool(true)),
            ("direction", Value::Str(self.direction.into())),
            ("loaded", Value::Int(self.loaded as i64)),
            ("done", Value::Bool(done)),
        ];
        if let Some(total) = self.total {
            fields.push(("total", Value::Int(total as i64)));
            let fraction = if total == 0 {
                if done { 1.0 } else { 0.0 }
            } else {
                self.loaded as f64 / total as f64
            };
            fields.push(("fraction", Value::Float(fraction)));
        }
        self.target
            .ops
            .answer(self.target.id, Ok(Value::map(fields)));
        self.last = Instant::now();
    }

    pub fn finish(&mut self) {
        if !self.done {
            self.done = true;
            self.emit(true);
        }
    }
}

impl<R: Read> Read for Reader<R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let n = self.inner.read(buf)?;
        self.loaded += n as u64;
        if self.direction == "upload" && self.total == Some(self.loaded) {
            self.finish();
        } else if n > 0 && !self.done && self.last.elapsed() >= Duration::from_millis(50) {
            self.emit(false);
        }
        Ok(n)
    }
}

impl<R> Drop for Reader<R> {
    fn drop(&mut self) {
        // Preserve the last byte count on failure even when an intermediate
        // update was throttled. Failed transfers never emit done=true.
        if !self.done && self.loaded > 0 {
            self.emit(false);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_slow_transfer_emits_intermediate_progress_before_completion() {
        let ops = Ops::default();
        let id = ops.id();
        let target = Target {
            ops: ops.clone(),
            id,
        };
        let mut reader = target.reader(io::Cursor::new(b"abcdefgh"), "download", Some(8));
        std::thread::sleep(Duration::from_millis(60));
        let mut first = [0; 4];
        reader.read_exact(&mut first).unwrap();
        let (events, _) = ops.take();
        assert!(events.iter().any(|(operation, event)| {
            let event = event.as_ref().unwrap();
            *operation == id
                && event.get_int("loaded") == Some(4)
                && event.get_bool("done") == Some(false)
        }));
        reader.read_to_end(&mut Vec::new()).unwrap();
        reader.finish();
        let (events, _) = ops.take();
        let event = events.last().unwrap().1.as_ref().unwrap();
        assert_eq!(event.get_int("loaded"), Some(8));
        assert_eq!(event.get_bool("done"), Some(true));
    }
}
