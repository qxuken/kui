//! A host's questions about the pointer and the keyboard (backlog F233):
//! a game, a map or a 3D view drawing the UI over a world of its own asks
//! whether an input is the UI's before it decides whose it is — Dear
//! ImGui's `WantCaptureMouse` / `WantCaptureKeyboard`, egui's
//! `wants_pointer_input`. Each answers as of the last finished frame and
//! the input fed since.

use super::*;

impl Core {
    /// The topmost node at `(x, y)` (logical px, as the cursor is fed)
    /// that blocks a host's world, as of the last finished frame: a node
    /// kui takes the pointer on, or a box that paints — a background, a
    /// border, an image — unless it says `host_hit: pass`, or any box
    /// that says `host_hit: block` (see [`crate::spec::HostHit`]). `None`
    /// over bare text, over a box that paints nothing, and outside the
    /// window: the world's.
    ///
    /// The painted boxes are recorded only once a host has asked (this or
    /// [`Core::pointer_wants`]) or armed them ([`Core::arm_host_hit`]),
    /// so a window nobody asks about pays nothing for them. The first ask
    /// arms them and owes a frame; until that frame, only the nodes kui
    /// takes the pointer on block. C's and Odin's own contexts
    /// (`kui_ctx_new`, `kui_ctx_new_in`) are armed from the start.
    pub fn hit(&self, x: f32, y: f32) -> Option<Key> {
        self.arm_host_hit();
        self.interaction.host_hit_at(Vec2::new(x, y))
    }

    /// Records, from the next frame on, the painted boxes [`Core::hit`]
    /// and [`Core::pointer_wants`] count as blocking — what their first
    /// call does. A host that asks about its very first frame arms before
    /// drawing it.
    pub fn arm_host_hit(&self) {
        self.host_asked
            .store(true, std::sync::atomic::Ordering::Relaxed);
    }

    /// Armed with no frame recorded since: the frame [`Core::owed`] owes
    /// so the next answer is whole.
    pub(crate) fn host_hit_pending(&self) -> bool {
        !self.blockers_live && self.host_asked.load(std::sync::atomic::Ordering::Relaxed)
    }

    /// Whether the pointer is the UI's now: over a node [`Core::hit`]
    /// answers for, or held by something kui started — a press, a drag,
    /// a slider, a scrollbar's thumb, a selection — whose release is
    /// kui's wherever it lands; and everywhere while a menu is open
    /// (a press outside closes it) or a `modal` is in effect. A host
    /// asks before it hands a press, a drag or a wheel to its world, and
    /// feeds kui every pointer event either way.
    pub fn pointer_wants(&self) -> bool {
        self.arm_host_hit();
        self.menu.is_some()
            || self.menu_bar_open.is_some()
            || self.modal.is_some()
            || self.interaction.pointer_held()
            || self.edit.dragging.is_some()
            || self.select_dragging.is_some()
            || self
                .interaction
                .cursor()
                .is_some_and(|p| self.interaction.host_hit_at(p).is_some())
    }

    /// Whether the keyboard is the UI's now: an editor has focus, the
    /// focused node is a key sink (`on_key`), or a menu is open. A host
    /// keeps its own keys (movement, hotkeys) while this is false. A
    /// focused button still takes Tab, Enter and Space, and a sink above
    /// it the keys that bubble, so a host feeds kui every key either way.
    pub fn keyboard_wants(&self) -> bool {
        self.menu.is_some()
            || self.menu_bar_open.is_some()
            || self.edit.focused().is_some()
            || self.focus.is_some_and(|key| {
                self.tree
                    .index_of(key)
                    .is_some_and(|i| self.tree.specs[i].events().on_key().is_some())
            })
    }
}
