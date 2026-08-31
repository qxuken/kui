//! Retained scroll state: offsets keyed by widget `Key`, surviving the
//! per-frame tree rebuild. The layout pass clamps each offset to the current
//! content overflow, so state stays valid as content changes.

use rustc_hash::FxHashMap;

use crate::geom::Vec2;
use crate::key::Key;

#[derive(Default)]
pub struct ScrollStore {
    offsets: FxHashMap<Key, Vec2>,
}

impl ScrollStore {
    pub fn offset(&self, key: Key) -> Vec2 {
        self.offsets.get(&key).copied().unwrap_or(Vec2::ZERO)
    }

    /// Adds a delta (positive = scroll content further down/right).
    /// Clamping happens in the next layout pass.
    pub fn scroll_by(&mut self, key: Key, delta: Vec2) {
        let e = self.offsets.entry(key).or_insert(Vec2::ZERO);
        e.x += delta.x;
        e.y += delta.y;
    }

    pub fn set(&mut self, key: Key, offset: Vec2) {
        self.offsets.insert(key, offset);
    }

    /// Clamps the stored offset to [0, max] per axis and returns it.
    pub fn clamp(&mut self, key: Key, max: Vec2) -> Vec2 {
        match self.offsets.get_mut(&key) {
            Some(e) => {
                e.x = e.x.clamp(0.0, max.x.max(0.0));
                e.y = e.y.clamp(0.0, max.y.max(0.0));
                *e
            }
            None => Vec2::ZERO,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accumulates_and_clamps() {
        let mut s = ScrollStore::default();
        let k = Key::ROOT.str("list");
        s.scroll_by(k, Vec2::new(0.0, 120.0));
        s.scroll_by(k, Vec2::new(0.0, 500.0));
        assert_eq!(s.clamp(k, Vec2::new(0.0, 300.0)), Vec2::new(0.0, 300.0));
        s.scroll_by(k, Vec2::new(0.0, -1000.0));
        assert_eq!(s.clamp(k, Vec2::new(0.0, 300.0)), Vec2::new(0.0, 0.0));
        // Content shrinking re-clamps existing offsets.
        s.scroll_by(k, Vec2::new(0.0, 250.0));
        assert_eq!(s.clamp(k, Vec2::new(0.0, 100.0)), Vec2::new(0.0, 100.0));
    }

    #[test]
    fn unknown_key_is_zero() {
        let mut s = ScrollStore::default();
        assert_eq!(s.offset(Key::ROOT.str("x")), Vec2::ZERO);
        assert_eq!(s.clamp(Key::ROOT.str("x"), Vec2::new(0.0, 100.0)), Vec2::ZERO);
    }
}
