//! Bounded copies of longer plain-text nodes' quad runs (F284). Only the
//! paint call is skipped: placement, hit regions, selection resolution,
//! clip construction, focus rings and frame-level passes still run.

use super::emit::Paint;
use crate::display::{Clip, Quad};
use crate::geom::Rect;
use crate::key::Key;
use crate::spec::VisualStyle;
use crate::text::PaintTextStamp;

const MAX_QUADS: usize = 32768;

#[derive(Clone, Copy, PartialEq)]
pub(super) enum Content {
    Text(PaintTextStamp),
}

#[derive(Clone, Copy, PartialEq)]
struct Input {
    key: Key,
    rect: Rect,
    style: VisualStyle,
    paint: Paint,
    clip: Clip,
    content: Content,
}

#[derive(Default)]
struct Entry {
    input: Option<Input>,
    valid: bool,
    quads: Vec<Quad>,
}

#[derive(Default)]
pub(super) struct PaintReuse {
    pub(super) enabled: bool,
    pub(super) reused: usize,
    entries: Vec<Entry>,
    capacity: usize,
}

impl PaintReuse {
    pub(super) fn invalidate(&mut self) {
        // Shaping can rebuild an entry under the same text/style key
        // without resetting the atlas (for example a family reweight).
        // A later node sharing that entry must not reuse old-font quads.
        for e in &mut self.entries {
            e.valid = false;
            e.input = None;
        }
    }

    #[cfg(test)]
    pub(super) fn retained_capacity(&self) -> usize {
        self.capacity
    }

    pub(super) fn begin(&mut self, count: usize, enabled: bool) {
        self.enabled = enabled;
        self.reused = 0;
        if !enabled {
            self.entries.clear();
            self.capacity = 0;
            return;
        }
        self.entries.resize_with(count, Entry::default);
        self.capacity = self.entries.iter().map(|e| e.quads.capacity()).sum();
    }

    #[allow(clippy::too_many_arguments)]
    #[inline(never)]
    pub(super) fn reuse(
        &mut self,
        i: usize,
        key: Key,
        rect: Rect,
        style: &VisualStyle,
        paint: Paint,
        clip: Clip,
        content: Option<Content>,
        out: &mut Vec<Quad>,
    ) -> bool {
        let e = &mut self.entries[i];
        let Some(content) = content else {
            e.input = None;
            e.valid = false;
            return false;
        };
        let input = Input {
            key,
            rect,
            style: *style,
            paint,
            clip,
            content,
        };
        if e.valid && e.input == Some(input) {
            out.extend_from_slice(&e.quads);
            self.reused += 1;
            return true;
        }
        e.input = Some(input);
        e.valid = false;
        false
    }

    #[inline(never)]
    pub(super) fn save(&mut self, i: usize, quads: &[Quad]) {
        let e = &mut self.entries[i];
        if e.input.is_none() {
            return;
        }
        let without = self.capacity - e.quads.capacity();
        if quads.len() > MAX_QUADS.saturating_sub(without) {
            self.capacity = without;
            e.quads = Vec::new();
            e.input = None;
            return;
        }
        if quads.len() > e.quads.capacity() {
            e.quads = Vec::with_capacity(quads.len());
        } else {
            e.quads.clear();
        }
        self.capacity = without + e.quads.capacity();
        e.quads.extend_from_slice(quads);
        e.valid = true;
    }
}
