//! The three ways a store outlives the frame, each stated once.
//!
//! kui is immediate mode: nothing is retained between frames except what
//! has to be, and each retained thing is kept by a named mechanism that
//! says why (ADR 0016). Those mechanisms are three, and before this module
//! each was written by hand in every store that used it:
//!
//! - **A kept frame list** ([`Kept`]): this frame's entries, and the
//!   previous frame's on a condition the caller states — the text list,
//!   the strokes, the fragment draws keep theirs exactly when `Core` keeps
//!   the previous tree, since a departing subtree's nodes index the list
//!   that frame filled; the text places and the cell grids keep theirs
//!   always, since a query during a build answers from the frame that
//!   finished.
//! - **A ceiling on the undeclared** ([`evict_undeclared`]): a by-key
//!   store that retains state across a key's absence (editors, scroll
//!   offsets) caps how many *undeclared* entries it holds and drops the
//!   longest-undeclared past the cap — a ceiling, not a prune, so
//!   retention still holds (backlog F26).
//! - **A sweep by last use** ([`sweep_cutoff`]): a store that stamps each
//!   entry with the frame it was last driven (tweens, ghosts, the shaped
//!   text cache) walks itself every so many frames and drops what nothing
//!   has touched for a while — a backstop for a driver whose clock stops
//!   while frames keep coming.

use rustc_hash::FxHashMap;

use crate::key::Key;

/// This frame's list, and the previous frame's while it is wanted. Derefs
/// to the current list, so a store pushes into and indexes it as a `Vec`;
/// [`Self::prev`] is the frame before.
pub(crate) struct Kept<T> {
    cur: Vec<T>,
    prev: Vec<T>,
}

// By hand rather than derived: a derived `Default` would ask `T: Default`,
// and an empty list needs nothing of its element.
impl<T> Default for Kept<T> {
    fn default() -> Self {
        Self {
            cur: Vec::new(),
            prev: Vec::new(),
        }
    }
}

impl<T> Kept<T> {
    /// Starts a frame. With `keep` the list just finished becomes the
    /// previous one — the two buffers swap roles, which costs an allocation
    /// that already existed and no copying; without it the spare is
    /// emptied, so a stale list can never be read as last frame's.
    pub(crate) fn begin(&mut self, keep: bool) {
        if keep {
            std::mem::swap(&mut self.cur, &mut self.prev);
        } else {
            self.prev.clear();
        }
        self.cur.clear();
    }

    /// The previous frame's list: empty when it was not kept.
    pub(crate) fn prev(&self) -> &[T] {
        &self.prev
    }
}

impl<T> std::ops::Deref for Kept<T> {
    type Target = Vec<T>;
    fn deref(&self) -> &Vec<T> {
        &self.cur
    }
}

impl<T> std::ops::DerefMut for Kept<T> {
    fn deref_mut(&mut self) -> &mut Vec<T> {
        &mut self.cur
    }
}

/// Drops the longest-undeclared entries of `map` down to `cap` undeclared
/// ones. `declared_at` is the frame that just ended: an entry whose
/// `stamp` is at least that was declared and is never evicted, and neither
/// is one `pinned` says the store still points at. Oldest first, and by
/// key inside a frame, so the same view evicts the same entries whatever
/// order the map iterated in; `removed` hears each key that went. A store
/// inside its budget should not call this at all — the length test at the
/// call site is what an ordinary frame pays.
pub(crate) fn evict_undeclared<V>(
    map: &mut FxHashMap<Key, V>,
    cap: usize,
    declared_at: u64,
    stamp: impl Fn(&V) -> u64,
    pinned: impl Fn(Key) -> bool,
    mut removed: impl FnMut(Key),
) {
    let mut undeclared: Vec<(u64, Key)> = map
        .iter()
        .filter(|(k, v)| stamp(v) < declared_at && !pinned(**k))
        .map(|(k, v)| (stamp(v), *k))
        .collect();
    let Some(excess) = undeclared.len().checked_sub(cap) else {
        return;
    };
    if excess == 0 {
        return;
    }
    undeclared.sort_unstable();
    for (_, key) in &undeclared[..excess] {
        map.remove(key);
        removed(*key);
    }
}

/// A store that sweeps by last use walks itself every this many frames…
pub(crate) const SWEEP_EVERY: u64 = 240;
/// …and drops what has not been driven for this many.
pub(crate) const KEEP_FOR: u64 = 300;

/// On a frame the store sweeps, the stamp below which an entry is stale
/// (`last_used < cutoff` goes); `None` on every other frame. The two
/// numbers were spelled in three stores with no shared name.
#[inline]
pub(crate) fn sweep_cutoff(frame_no: u64) -> Option<u64> {
    frame_no
        .is_multiple_of(SWEEP_EVERY)
        .then(|| frame_no.saturating_sub(KEEP_FOR))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_kept_list_keeps_the_previous_frame_only_when_asked() {
        let mut k: Kept<u32> = Kept::default();
        k.push(1);
        k.begin(true);
        assert_eq!(k.prev(), &[1]);
        assert!(k.is_empty());
        k.push(2);
        k.begin(false);
        assert!(k.prev().is_empty(), "not kept: the spare is emptied");
        assert!(k.is_empty());
    }

    #[test]
    fn eviction_is_a_ceiling_on_the_undeclared_and_spares_the_pinned() {
        let mut map: FxHashMap<Key, u64> = FxHashMap::default();
        for i in 0..6u64 {
            // Stamped with the frame each was last declared: 0..6.
            map.insert(Key::ROOT.index(i), i);
        }
        let pinned = Key::ROOT.index(0);
        let mut gone = Vec::new();
        // Frame 5 just ended: entry 5 is declared, 0..5 are undeclared, and
        // the cap is two undeclared.
        evict_undeclared(&mut map, 2, 5, |s| *s, |k| k == pinned, |k| gone.push(k));
        assert_eq!(
            map.len(),
            4,
            "the declared one, the pinned one, and the two newest undeclared"
        );
        assert!(map.contains_key(&pinned));
        assert!(map.contains_key(&Key::ROOT.index(5)));
        assert_eq!(
            gone,
            vec![Key::ROOT.index(1), Key::ROOT.index(2)],
            "the two oldest unpinned undeclared went, oldest first"
        );
    }

    #[test]
    fn a_sweep_happens_every_so_many_frames() {
        assert_eq!(sweep_cutoff(1), None);
        assert_eq!(sweep_cutoff(SWEEP_EVERY), Some(0));
        assert_eq!(
            sweep_cutoff(SWEEP_EVERY * 2),
            Some(SWEEP_EVERY * 2 - KEEP_FOR)
        );
    }
}
