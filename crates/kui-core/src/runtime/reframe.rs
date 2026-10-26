//! A frame built from the last tree (backlog C138).
//!
//! A frame whose only cause is an animation the core runs — a keyframe
//! cycle, a transition, a departure, an eased scroll — asks nothing of the
//! host's view that the last build did not already answer: no input
//! reached the window, no wake, no resize. Yet every one ran the host's
//! whole view again, at the display's rate: a Kit chart's slide drew the
//! runner app's `view` 57 times a second where its data changed ten.
//!
//! So while something of the core's animates, a build keeps what it needs
//! to be built again without the view: the spec each node whose look the
//! frame decides (a `transition`, `keyframes`, a `when` list) declared
//! before the frame touched it, and every fact of the frame the build
//! read, whoever read it — the host's view, a fresh fill, the core's own
//! widgets. A replayed fill's reads are its kept fill's, checked as a
//! replay checks them. When the next frame is one only that animation asks
//! for, every fact still reads the same, and the host says its own state
//! has not moved (`Extension::root_state` for an extension's root, the
//! driver's `App` for its view), the driver calls [`Core::reframe`]: the
//! tree stays, those nodes go through the frame's pipeline again — their
//! hover, rules and easing resolved for this frame, so the cycles and
//! transitions are sampled at this frame's clock — and the frame is laid
//! out and emitted as any. Everything else the build declared stands: the
//! title, the windows, the labels, the instances placed.
//!
//! A frame with the devtools in the window, a lazy branch's second build
//! owed, or a room kept for a leaving node is built as always.

use std::cell::{Cell, RefCell};

use super::*;
use crate::FrameCause;
use crate::runtime::replay::Read;

/// What a build kept to be built again from its tree.
#[derive(Default)]
pub(crate) struct Reframe {
    /// This build keeps what follows: the last frame owed an animation
    /// of the core's.
    pub(crate) logging: bool,
    /// The finished tree can be built again.
    ready: bool,
    /// Every fact of the frame the build read, as `replay::Read`s.
    pub(crate) reads: RefCell<Vec<Read>>,
    /// The `ONCE_*` reads noted.
    pub(crate) once: Cell<u8>,
    /// The spec each node the frame's pipeline changes declared, by key.
    specs: Vec<(Key, NodeSpec)>,
    /// The root's, as `configure_root` was handed it.
    root: Option<NodeSpec>,
    /// The viewport and scale the tree was laid out for.
    at: (Size, f32),
}

impl Core {
    /// At `begin_frame`: this build keeps what a frame built from it
    /// needs while the last frame owed an animation of the core's own,
    /// and the devtools are not in the window.
    pub(super) fn reframe_begin(&mut self) {
        let logging = self.core_animated() && self.dt_app.is_none();
        let r = &mut self.reframe;
        r.ready = false;
        r.reads.get_mut().clear();
        r.once.set(0);
        r.specs.clear();
        r.root = None;
        r.logging = logging;
    }

    /// `prepare_spec`'s first act while the build keeps: a node whose
    /// look the frame decides keeps the spec it was declared with.
    #[cold]
    #[inline(never)]
    pub(super) fn reframe_note(&mut self, key: Key, spec: &NodeSpec) {
        self.reframe.specs.push((key, spec.clone()));
    }

    /// `configure_root`'s, for the root.
    pub(super) fn reframe_note_root(&mut self, spec: &NodeSpec) {
        if self.reframe.logging {
            self.reframe.root = Some(spec.clone());
        }
    }

    /// At `finish_frame`'s end: the tree may be built again.
    /// The log ends with the build: a read between frames, or while a
    /// frame is built again, is no fact of this tree's.
    pub(super) fn reframe_finish(&mut self) {
        if self.reframe.logging {
            self.reframe.logging = false;
            self.reframe.ready = !self.rerun_owed && self.dt_app.is_none();
            self.reframe.at = (self.viewport, self.scale);
        } else if self.reframe.ready {
            // A frame built again: its own layout may owe a build.
            self.reframe.ready = !self.rerun_owed;
        }
    }

    /// Builds the frame about to begin from the last tree rather than
    /// from the host's view, when that frame is one only an animation of
    /// the core's asks for and nothing the last build read has moved —
    /// and answers whether it did; `false` leaves everything as it was,
    /// for the driver to build the frame as ever (backlog C138).
    ///
    /// The driver calls it where it would begin the frame, after setting
    /// the clock and noting the frame's causes, and only when its host
    /// says its own state has not moved since the last build: the core
    /// cannot see what a view reads beyond its own doors (a model, a
    /// timer's data). The frame is then laid out and emitted, and its
    /// events, hits and display list are what a built frame's are.
    pub fn reframe(&mut self, viewport: Size, scale: f32) -> bool {
        if !self.reframe_possible(viewport, scale) {
            return false;
        }
        // `begin_frame`'s bookkeeping, less everything a build declares.
        if let (Some(due), Some(now)) = (self.frame_due, self.anim.time())
            && due <= now
        {
            self.frame_due = None;
        }
        self.trace_begin_frame();
        self.perf_frame_start();
        self.age_type_ahead();
        self.recording = None;
        self.keep_next = None;
        // Nothing departs from a tree built again: its departures were
        // taken from the build that dropped them.
        self.prev_tree.clear();
        let unread = (!self.output_read).then(|| {
            const MOST: usize = 4096;
            let mut t = std::mem::take(&mut self.display.dropped_textures);
            let mut f = std::mem::take(&mut self.display.dropped_fragments);
            t.drain(..t.len().saturating_sub(MOST));
            f.drain(..f.len().saturating_sub(MOST));
            (t, f)
        });
        self.display.clear();
        if let Some((t, f)) = unread {
            self.display.dropped_textures = t;
            self.display.dropped_fragments = f;
        }
        self.output_read = false;
        self.sync_dropped();
        self.atlas.begin_frame();
        self.text.begin_reframe();
        self.anim.begin_frame(self.frame_no);
        self.depart.begin_frame(self.frame_no);
        self.frame_requested = false;
        self.trace_forget_requests();
        self.building = true;
        self.perf_frame_begun();
        // The nodes whose look the frame decides, through its pipeline
        // again: their hover, rules and easing for this frame's clock.
        if let Some(root) = self.reframe.root.clone() {
            let mut spec = root;
            self.resolve_rules(Key::ROOT, &mut spec);
            self.ease_spec(Key::ROOT, &mut spec);
            self.tree.note(&spec, &NodeContent::Container);
            self.tree.specs[0] = spec;
        }
        let specs = std::mem::take(&mut self.reframe.specs);
        for (key, declared) in &specs {
            let Some(i) = self.tree.index_of(*key) else {
                continue;
            };
            let mut spec = declared.clone();
            self.prepare_spec_again(*key, &mut spec);
            let content = self.tree.content[i];
            self.tree.note(&spec, &content);
            self.tree.specs[i] = spec;
        }
        self.reframe.specs = specs;
        self.perf_build_done();
        self.finish_frame();
        true
    }

    /// Whether [`Self::reframe`] may build the frame about to begin.
    fn reframe_possible(&self, viewport: Size, scale: f32) -> bool {
        if !self.reframe.ready
            || self.reframe.at != (viewport, scale)
            || self.dt_app.is_some()
            || self.rerun_owed
            || !self.rooms.is_empty()
            || self.tree.is_empty()
        {
            return false;
        }
        // The causes the frame would begin with, as `begin_frame` takes
        // them, and what the last frame owed.
        let owed = self.owed();
        let mut cause = self.trace_pending_cause();
        if owed.any() {
            cause |= FrameCause::OWED;
        }
        let quiet = FrameCause::OWED
            .union(FrameCause::RETRY)
            .union(FrameCause::OVERDUE);
        if !cause.contains(FrameCause::OWED)
            || !quiet.contains(cause)
            || owed.requested
            || owed.autoscroll
            || !(owed.transition || owed.cycle || owed.depart || owed.scroll)
        {
            return false;
        }
        self.reads_hold(&self.reframe.reads.borrow()) && self.kept_reads_hold()
    }
}
