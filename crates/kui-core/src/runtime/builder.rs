//! The frame builder: what a view calls between `begin_frame` and
//! `finish_frame` to declare the tree — open, close, text, editors,
//! images — with each spec eased against its transitions and keyframes
//! as it is opened, and the hover / press queries a view styles by.

use super::*;

/// A node's keyframes flattened per slot for `ease_spec`, built once per
/// node per frame (only for nodes that declare keyframes).
struct Tracks {
    width: Option<Vec<(f32, [f32; 4])>>,
    height: Option<Vec<(f32, [f32; 4])>>,
    bg: Option<Vec<(f32, [f32; 4])>>,
    radius: Option<Vec<(f32, [f32; 4])>>,
    opacity: Option<Vec<(f32, [f32; 4])>>,
}

impl Tracks {
    fn of(spec: &NodeSpec) -> Self {
        let frames = &spec.anim().keyframes;
        let offsets = keyframes::offsets(frames);
        let one = |v: f32| [v, 0.0, 0.0, 0.0];
        let sizing = |base: Sizing, pick: fn(&Keyframe) -> Option<Sizing>| {
            let base = base.amount()?;
            keyframes::track(frames, &offsets, one(base), |k| pick(k)?.amount().map(one))
        };
        let bg = spec.style.bg;
        Tracks {
            width: sizing(spec.layout.width, |k| k.width),
            height: sizing(spec.layout.height, |k| k.height),
            bg: keyframes::track(frames, &offsets, [bg.r, bg.g, bg.b, bg.a], |k| {
                k.bg.map(|c| [c.r, c.g, c.b, c.a])
            }),
            radius: keyframes::track(frames, &offsets, spec.style.radius, |k| {
                k.radius.map(|r| [r; 4])
            }),
            opacity: keyframes::track(frames, &offsets, one(spec.style.opacity), |k| {
                k.opacity.map(one)
            }),
        }
    }

    fn get(&self, slot: Slot) -> Option<&Track> {
        match slot {
            Slot::Width => self.width.as_deref(),
            Slot::Height => self.height.as_deref(),
            Slot::Bg => self.bg.as_deref(),
            Slot::Radius => self.radius.as_deref(),
            Slot::Opacity => self.opacity.as_deref(),
            // Keyframing a shadow would need stops for four more numbers
            // and a color; a shadow tweens with `transition` and no more.
            Slot::Border | Slot::Pos | Slot::Shadow | Slot::ShadowColor => None,
        }
    }
}

impl Core {
    // -- Frame builder ------------------------------------------------------
    // Flat, non-panicking, callable through FFI. Misuse (close past the root,
    // building outside a frame) is ignored rather than UB or panic.

    /// Tags subsequently created nodes with an origin (set by the runner
    /// before handing the frame to an extension).
    pub fn set_origin(&mut self, origin: OriginId) {
        self.origin = origin;
    }

    /// Replaces the implicit root's spec (e.g. to make the top level a row).
    /// Root sizing is resolved against the viewport regardless.
    pub fn configure_root(&mut self, mut spec: NodeSpec) {
        if !self.tree.is_empty() {
            self.ease_spec(Key::ROOT, &mut spec);
            if spec.events().on_layout.is_some() {
                self.any_layout = true;
            }
            if spec.anim().exit.is_some() && spec.transition.is_some() {
                self.any_exit = true;
            }
            self.tree.specs[0] = spec;
        }
    }

    /// Replaces a transitioning node's animatable values with this frame's
    /// eased ones. Nodes without a transition cost one branch — inlined at
    /// the call site, so it is a branch and not a call that returns (C15).
    /// A slot the node's keyframes name is sampled from its cycle instead
    /// of tweened.
    #[inline]
    fn ease_spec(&mut self, key: Key, spec: &mut NodeSpec) {
        if let Some(t) = spec.transition {
            self.ease_transitioning(key, spec, t);
        }
    }

    #[inline(never)]
    fn ease_transitioning(&mut self, key: Key, spec: &mut NodeSpec, t: crate::anim::Transition) {
        // One lookup for the whole node. Every slot below used to reach
        // `AnimStore` by key on its own, which was seven to nine hashes and
        // probes of the same entry per transitioning node, per frame.
        let mut anim = self.anim.node(key);
        let tracks = (!spec.anim().keyframes.is_empty()).then(|| Tracks::of(spec));
        let track = |slot: Slot| tracks.as_ref().and_then(|k| k.get(slot));
        let enter = spec.anim().enter.unwrap_or_default();
        let mut sizing = |slot: Slot, s: Sizing, from: Option<Sizing>| {
            let Some(v) = s.amount() else {
                return s;
            };
            let from = from.and_then(|f| f.amount()).map(|f| [f, 0.0, 0.0, 0.0]);
            let eased = match track(slot) {
                Some(track) => anim.sample(track, t).map_or(v, |v| v[0]),
                None => anim.drive(slot, from, [v, 0.0, 0.0, 0.0], t, true)[0],
            };
            s.with_amount(eased)
        };
        spec.layout.width = sizing(Slot::Width, spec.layout.width, enter.width);
        spec.layout.height = sizing(Slot::Height, spec.layout.height, enter.height);
        let mut color = |slot: Slot, c: Color, from: Option<Color>| {
            let target = [c.r, c.g, c.b, c.a];
            let from = from.map(|f| [f.r, f.g, f.b, f.a]);
            let v = match track(slot) {
                Some(track) => anim.sample(track, t).unwrap_or(target),
                None => anim.drive(slot, from, target, t, true),
            };
            Color {
                r: v[0],
                g: v[1],
                b: v[2],
                a: v[3],
            }
        };
        spec.style.bg = color(Slot::Bg, spec.style.bg, enter.bg);
        spec.style.border_color = color(Slot::Border, spec.style.border_color, None);
        spec.style.shadow.color = color(Slot::ShadowColor, spec.style.shadow.color, None);
        let sh = spec.style.shadow;
        let geom = anim.drive(
            Slot::Shadow,
            None,
            [sh.dx, sh.dy, sh.blur, sh.spread],
            t,
            true,
        );
        spec.style.shadow.dx = geom[0];
        spec.style.shadow.dy = geom[1];
        spec.style.shadow.blur = geom[2].max(0.0);
        spec.style.shadow.spread = geom[3];
        spec.style.opacity = match track(Slot::Opacity) {
            Some(track) => anim.sample(track, t).map_or(spec.style.opacity, |v| v[0]),
            None => anim.drive(
                Slot::Opacity,
                enter.opacity.map(|o| [o, 0.0, 0.0, 0.0]),
                [spec.style.opacity, 0.0, 0.0, 0.0],
                t,
                true,
            )[0],
        }
        .clamp(0.0, 1.0);
        spec.style.radius = match track(Slot::Radius) {
            Some(track) => anim.sample(track, t).unwrap_or(spec.style.radius),
            None => anim.drive(
                Slot::Radius,
                enter.radius.map(|r| [r; 4]),
                spec.style.radius,
                t,
                true,
            ),
        };
    }

    /// The root node's key — for hover/press queries or `set_key_focus` when
    /// the root itself declares the interaction (e.g. a root-level key sink).
    pub fn root_key(&self) -> Key {
        self.tree.keys.first().copied().unwrap_or(Key(0))
    }

    fn current(&self) -> u32 {
        self.stack.last().copied().unwrap_or(0)
    }

    /// The key a child of the current node is derived from: the node's own
    /// key, except inside a slot fill at the depth the fill began, where it
    /// is the fill's namespace (`Core::fill`, ADR 0014 decision 4). One
    /// compare on the auto-key path; `ns_depth` is `usize::MAX` outside a
    /// fill.
    #[inline]
    pub(crate) fn parent_key(&self) -> Key {
        if self.stack.len() == self.ns_depth {
            self.ns_key
        } else {
            self.tree.keys[self.current() as usize]
        }
    }

    #[inline]
    pub(crate) fn auto_key(&mut self) -> Key {
        let parent = self.parent_key();
        let i = self.counters.last().copied().unwrap_or(0);
        if let Some(c) = self.counters.last_mut() {
            *c += 1;
        }
        parent.index(i)
    }

    /// The key a child labeled `label` would get — usable before creating it,
    /// e.g. to check hover state for styling.
    pub fn child_key(&self, label: &str) -> Key {
        self.parent_key().str(label)
    }

    /// The key the `i`th child gets from auto-keying — what `open_indexed`
    /// opens with, usable before the node exists.
    pub fn child_key_index(&self, i: u64) -> Key {
        self.parent_key().index(i)
    }

    pub fn is_hovered(&self, key: Key) -> bool {
        self.interaction.is_hovered(key)
    }

    pub fn is_pressed(&self, key: Key) -> bool {
        self.interaction.is_pressed(key)
    }

    /// Whether any member of hover group `group` (see
    /// `NodeSpec::hover_group`) is hovered.
    pub fn is_group_hovered(&self, group: u64) -> bool {
        self.interaction.is_group_hovered(group)
    }

    /// Whether hover group `group` is pressed (press started on a member,
    /// pointer still over one).
    pub fn is_group_pressed(&self, group: u64) -> bool {
        self.interaction.is_group_pressed(group)
    }

    /// Events raised outside `handle_input`: the `resize` a changed
    /// viewport produced at `begin_frame`, and `on_hover` enter/leave
    /// caused by a finished frame changing what sits under a still cursor.
    /// Frame drivers route these after `finish_frame`; they also ride along
    /// with the next `handle_input` result, so a driver that never calls
    /// this merely sees them a little later.
    pub fn take_pending_events(&mut self) -> Vec<UiEvent> {
        let mut out = std::mem::take(&mut self.pending);
        out.append(&mut self.interaction.take_pending());
        self.stamp(&mut out);
        out
    }

    /// Swaps in the hover / pressed / focus background the spec declares
    /// for the node's (or its group's) current state: pressed wins over
    /// keyboard-visible focus wins over hover. A disabled node keeps its
    /// plain `bg`. Runs before easing so a `transition` tweens between
    /// the states.
    #[inline]
    fn resolve_hover_style(&self, key: Key, spec: &mut NodeSpec) {
        // Runs for every node of every frame, and almost every node declares
        // none of this — so the early-out is one null check on the boxed
        // group rather than three `Option`s read out of the spec, and it is
        // inlined so the check is a branch rather than a call (C15).
        if spec.interact.is_some() {
            self.resolve_declared_hover_style(key, spec);
        }
    }

    #[inline(never)]
    fn resolve_declared_hover_style(&self, key: Key, spec: &mut NodeSpec) {
        let Some(interact) = spec.interact.as_deref() else {
            return;
        };
        let (hover_bg, pressed_bg, focus_bg, group) = (
            interact.hover_bg,
            interact.pressed_bg,
            interact.focus_bg,
            interact.hover_group,
        );
        if spec.disabled || (hover_bg.is_none() && pressed_bg.is_none() && focus_bg.is_none()) {
            return;
        }
        let pressed = self.interaction.is_pressed(key)
            || group.is_some_and(|g| self.interaction.is_group_pressed(g));
        let hovered = pressed
            || self.interaction.is_hovered(key)
            || group.is_some_and(|g| self.interaction.is_group_hovered(g));
        let focused = self.focus_visible && self.focus == Some(key);
        if pressed && let Some(c) = pressed_bg {
            spec.style.bg = c;
        } else if focused && let Some(c) = focus_bg {
            spec.style.bg = c;
        } else if hovered && let Some(c) = hover_bg {
            spec.style.bg = c;
        }
    }

    /// Physical modifier state as of the last `InputEvent::Modifiers`.
    pub fn modifiers(&self) -> crate::input::KeyMods {
        self.interaction.modifiers()
    }

    // The open chain is inlined end to end (`Ui::open` → here →
    // `open_with_key` → `Tree::push`): a `NodeSpec` is 224 bytes and moved
    // by value at every step, and each step that is a real call is a copy
    // of all of them. Inlined, the spec the view built travels by pointer
    // and is copied once, into the tree (C15).
    #[inline]
    pub fn open(&mut self, spec: NodeSpec) -> Key {
        let key = self.auto_key();
        self.open_with_key(key, spec);
        key
    }

    #[inline]
    pub fn open_keyed(&mut self, label: &str, spec: NodeSpec) -> Key {
        let key = self.child_key(label);
        self.open_with_key(key, spec);
        self.key_labels.push(key, label);
        key
    }

    /// The key of the node opened under `label` (`open_keyed`; a `key`
    /// prop in JSX or a Lua table) in the last finished frame — or, while
    /// a frame is being built, in it so far and then in the last one. The
    /// door for a caller that holds only strings: keys are hashes of the
    /// path from the root, and that path runs through auto-keyed
    /// ancestors nothing outside the build can spell, so "focus the node
    /// I just declared" is this and not `child_key`. None when no node
    /// declared the label. Labels are unique among siblings, not across a
    /// tree, so two nodes may share one under different parents: the
    /// first in tree order wins and an `ambiguous-key` warning says so.
    pub fn key_of(&mut self, label: &str) -> Option<Key> {
        let (first, count) = {
            let mut hits = self.key_labels.find(label);
            match hits.next() {
                Some(k) => (k, 1 + hits.count()),
                None if self.building => {
                    let mut hits = self.key_labels_last.find(label);
                    (hits.next()?, 1 + hits.count())
                }
                None => return None,
            }
        };
        if count > 1 {
            self.diag
                .raise(crate::diag::ambiguous_key(label, first, count));
        }
        Some(first)
    }

    /// `open_keyed` in the sibling-index namespace: the key auto-keying
    /// would have given the `i`th child. A list that builds only rows
    /// 900..930 opens each with its *data* index, so row 900 keeps the key
    /// it has when the whole list is built — hover, focus, edit buffers and
    /// tweens follow the row instead of the slot it happens to occupy.
    #[inline]
    pub fn open_indexed(&mut self, i: u64, spec: NodeSpec) -> Key {
        let key = self.child_key_index(i);
        let at = self.tree.len() as u32;
        self.open_with_key(key, spec);
        // Remembered for the node that was actually pushed, so a selection
        // inside a virtual row can be ordered by the row's place in the
        // *data* when the row itself is not built (ADR 0017, tier 3).
        if self.tree.len() as u32 > at {
            self.tree.indexed.push((at, i));
        }
        key
    }

    #[inline]
    fn open_with_key(&mut self, key: Key, spec: NodeSpec) {
        self.open_content(key, spec, NodeContent::Container);
    }

    /// `open_with_key` for a node that is a box in every way but what it
    /// paints: the caller supplies the content and closes the node. Every
    /// `any_*` flag below is a box's, and a `fragment` earns all of them
    /// because it clips, floats, fades and animates like one.
    fn open_content(&mut self, key: Key, mut spec: NodeSpec, content: NodeContent) {
        if self.tree.is_empty() {
            return;
        }
        // The one paint the environment decides (`accent`): the OS colour
        // where the host reported one, the declared `bg` where it did not.
        // Before the hover resolution below, so a node that declares both
        // still hovers to what it declared.
        if spec.accent
            && let Some(accent) = self.env.system.accent
        {
            spec.style.bg = accent;
        }
        self.resolve_hover_style(key, &mut spec);
        self.ease_spec(key, &mut spec);
        if spec.layout.clips() {
            self.any_clip = true;
            self.any_rounded_clip |= spec.style.radius != crate::display::SQUARE;
        }
        if spec.style.opacity < 1.0 {
            self.any_opacity = true;
        }
        if spec.layout.float.is_some() {
            self.any_float = true;
        }
        if spec.animate {
            // One node asking is the whole window asking; the flag is
            // cleared when the frame is taken, like any other request.
            self.frame_requested = true;
        }
        // Each boxed group is tested once, not once per flag it can set: a
        // node declaring no events and no animation reaches `Tree::push`
        // after two null checks.
        if let Some(events) = spec.events.as_deref() {
            if events.modal.is_some() {
                self.any_modal = true;
            }
            if events.on_layout.is_some() {
                self.any_layout = true;
            }
        }
        if spec.transition.is_some() {
            match spec.anim.as_deref() {
                Some(anim) => {
                    if spec.slide || anim.enter.is_some_and(|e| e.offsets()) {
                        self.any_slide = true;
                    }
                    if anim.exit.is_some() {
                        self.any_exit = true;
                    }
                }
                None => self.any_slide |= spec.slide,
            }
        }
        let parent = self.current();
        let idx = self.tree.push(parent, key, self.origin, spec, content);
        self.stack.push(idx);
        self.counters.push(0);
    }

    #[inline]
    pub fn close(&mut self) {
        if self.stack.len() > 1 {
            self.stack.pop();
            self.counters.pop();
        }
    }

    pub fn text_node(&mut self, content: &str, style: TextStyle) {
        if self.tree.is_empty() {
            return;
        }
        let tid = {
            let sess = &mut *self.session.state();
            self.text
                .add(content, &style, &sess.resources, &mut sess.fonts)
        };
        let key = self.auto_key();
        let parent = self.current();
        self.tree.push(
            parent,
            key,
            self.origin,
            NodeSpec::default(),
            NodeContent::Text(tid),
        );
    }

    /// A cell grid as one leaf node, sized `cols × cell_w` by `rows ×
    /// cell_h` (backlog C20; see `crate::cells`). `spec` is the node's:
    /// an `on_key` makes it the terminal's sink, an `on_click` / `on_drag`
    /// carry `cell: {row, col}` on their events.
    pub fn cells(&mut self, grid: &crate::cells::CellGrid<'_>, spec: NodeSpec) {
        let key = self.auto_key();
        self.cells_at(key, grid, spec);
    }

    /// [`Self::cells`] under a declared key.
    pub fn cells_keyed(&mut self, label: &str, grid: &crate::cells::CellGrid<'_>, spec: NodeSpec) {
        let key = self.child_key(label);
        self.key_labels.push(key, label);
        self.cells_at(key, grid, spec);
    }

    /// [`Self::cells`] under a data index; see [`Self::open_indexed`].
    pub fn cells_indexed(&mut self, i: u64, grid: &crate::cells::CellGrid<'_>, spec: NodeSpec) {
        let key = self.child_key_index(i);
        self.cells_at(key, grid, spec);
    }

    fn cells_at(&mut self, key: Key, grid: &crate::cells::CellGrid<'_>, spec: NodeSpec) {
        if self.tree.is_empty() {
            return;
        }
        let cid = self.cells.add(grid);
        let parent = self.current();
        self.tree
            .push(parent, key, self.origin, spec, NodeContent::Cells(cid));
    }

    /// An editable text node. State (buffer, cursor, selection) is retained
    /// by key across frames; edits arrive via `handle_input` and come back to
    /// the host as "changed"/"submit" events. Read with `edit_text`.
    pub fn text_edit(
        &mut self,
        label: &str,
        initial: &str,
        opts: &EditOptions,
        mut spec: NodeSpec,
    ) -> Key {
        if self.tree.is_empty() {
            return Key::ROOT;
        }
        let key = self.child_key(label);
        self.ease_spec(key, &mut spec);
        if spec.events().on_layout.is_some() {
            self.any_layout = true;
        }
        if spec.anim().exit.is_some() && spec.transition.is_some() {
            self.any_exit = true;
        }
        {
            let origin = self.origin;
            let scale = self.scale;
            let sess = &mut *self.session.state();
            // A `set_edit_text` by label, held for the frame that would
            // declare the name (backlog F32): claimed here, where the
            // label and its key are both in hand, and before `declare`,
            // which is what turns it into this key's seed.
            self.edit
                .claim_label(key, label, &mut sess.fonts, &sess.resources);
            self.edit.declare(
                key,
                initial,
                opts,
                origin,
                scale,
                &mut sess.fonts,
                &sess.resources,
            );
        }
        // Autofocus takes the keyboard only while nothing holds it — never
        // from a control Tab landed on.
        if opts.autofocus && self.focus.is_none() && !spec.disabled {
            self.set_focus(Some(key));
        }
        let parent = self.current();
        self.tree
            .push(parent, key, self.origin, spec, NodeContent::Edit(key));
        // A leaf keyed by its label, like `open_keyed`: `key_of` must find
        // the editor an app wants to focus by name.
        self.key_labels.push(key, label);
        key
    }

    /// A registered image (see `Resources::add_image`). Fit sizing takes
    /// the image's pixel dimensions as logical px; a Fit height against a
    /// resolved width preserves the aspect ratio. `style.radius` rounds the
    /// corners.
    pub fn image_node(&mut self, id: crate::resources::ImageId, mut spec: NodeSpec) {
        if self.tree.is_empty() {
            return;
        }
        let key = self.auto_key();
        self.resolve_hover_style(key, &mut spec);
        self.ease_spec(key, &mut spec);
        if spec.events().on_layout.is_some() {
            self.any_layout = true;
        }
        if spec.anim().exit.is_some() && spec.transition.is_some() {
            self.any_exit = true;
        }
        let parent = self.current();
        self.tree
            .push(parent, key, self.origin, spec, NodeContent::Image(id));
    }

    /// A box a registered WGSL function paints
    /// (`docs/adr/0015-a-fragment-element-and-the-painter-it-is-not.md`).
    ///
    /// An ordinary node in every other respect: it lays out where it is
    /// declared, sizes from `spec`, rounds by `radius`, clips, fades with
    /// its subtree's opacity, takes input like any box, and may hold
    /// children — which paint over it, so a gradient card is a `fragment`
    /// with a title and buttons inside it.
    ///
    /// It has **no intrinsic size**: unlike an image there is nothing to
    /// measure, so a fragment with no `width` / `height` / `fill` is zero
    /// by zero and draws nothing. Size it.
    ///
    /// `params` is up to sixteen numbers, positional, zero-padded, read by
    /// the shader as four `vec4<f32>`; more than sixteen are dropped with
    /// a `fragment-params-truncated` warning. A handle that is not live in
    /// this session draws nothing, as every resource kind does.
    pub fn fragment_node(
        &mut self,
        id: crate::resources::FragmentId,
        params: &[f32],
        spec: NodeSpec,
    ) -> Key {
        let key = self.open_fragment(id, params, spec);
        self.close();
        key
    }

    /// Opens a fragment as a parent: its children paint over it, which is
    /// what a gradient card with a title and buttons in it is. Balance it
    /// with [`Self::close`], or use `Ui::fragment_with`.
    pub fn open_fragment(
        &mut self,
        id: crate::resources::FragmentId,
        params: &[f32],
        spec: NodeSpec,
    ) -> Key {
        if self.tree.is_empty() {
            return Key::ROOT;
        }
        let key = self.auto_key();
        self.fragment_with_key(key, id, params, spec);
        key
    }

    /// [`Self::fragment_node`] under a label key, for a fragment that
    /// transitions or exits and needs a stable identity across frames.
    pub fn fragment_node_keyed(
        &mut self,
        label: &str,
        id: crate::resources::FragmentId,
        params: &[f32],
        spec: NodeSpec,
    ) -> Key {
        let key = self.open_fragment_keyed(label, id, params, spec);
        self.close();
        key
    }

    /// [`Self::open_fragment`] under a label key.
    pub fn open_fragment_keyed(
        &mut self,
        label: &str,
        id: crate::resources::FragmentId,
        params: &[f32],
        spec: NodeSpec,
    ) -> Key {
        if self.tree.is_empty() {
            return Key::ROOT;
        }
        let key = self.child_key(label);
        self.fragment_with_key(key, id, params, spec);
        self.key_labels.push(key, label);
        key
    }

    /// [`Self::open_fragment`] under a data index; see [`Self::open_indexed`].
    pub fn open_fragment_indexed(
        &mut self,
        i: u64,
        id: crate::resources::FragmentId,
        params: &[f32],
        spec: NodeSpec,
    ) -> Key {
        if self.tree.is_empty() {
            return Key::ROOT;
        }
        let key = self.child_key_index(i);
        self.fragment_with_key(key, id, params, spec);
        key
    }

    fn fragment_with_key(
        &mut self,
        key: Key,
        id: crate::resources::FragmentId,
        params: &[f32],
        spec: NodeSpec,
    ) {
        let (params, dropped) = crate::fragment::params_of(params);
        if dropped > 0 {
            self.diag.raise(Warning {
                code: crate::diag::FRAGMENT_PARAMS_TRUNCATED,
                key,
                message: format!(
                    "a fragment takes sixteen params and {} were declared, so the last {dropped}                      were dropped; pack what the shader needs into the sixteen it has",
                    params.len() + dropped
                ),
            });
        }
        let draw = self
            .fragments
            .push(crate::display::FragmentDraw { id, params });
        self.open_content(key, spec, NodeContent::Fragment(draw));
    }

    /// A stroke through `points` in the parent's box space: one round-capped
    /// segment for two points, a polyline for more, a smooth curve through
    /// them with [`Stroke::curve`] (`docs/adr/0010-a-segment-primitive.md`).
    ///
    /// Never in layout. The node is a float sized to the stroke's padded
    /// bounding box, so it takes no room in a row or column, and `spec`'s
    /// sizing, clamps, padding, gap and alignment are ignored. What `spec`
    /// carries that matters: `transition` (the colour eases — it rides in
    /// the `bg` slot — and `slide`, `enter` and `exit` offsets move the
    /// float), `opacity`, `on_layout` (reports the bounding box), a
    /// declared `float` whose *anchor* is kept (`FloatAnchor::Viewport`
    /// reads the points in viewport space), and `role` / `label`, which are
    /// honoured like any node's; without them a line has no access row. It
    /// takes no pointer input: `on_click`, `on_drag`, `on_key`, `on_hover`,
    /// `hoverable` and `focusable` are ignored, with a
    /// `line-ignores-input` warning. Fewer than two points draw nothing.
    ///
    /// Consecutive segments overlap at their round caps, which is the
    /// join: exact for an opaque stroke, and a translucent one
    /// double-blends there, the way a faded subtree shows its seams.
    pub fn line_node(&mut self, points: &[Vec2], stroke: Stroke, spec: NodeSpec) {
        if self.tree.is_empty() {
            return;
        }
        let key = self.auto_key();
        self.line_with_key(key, points, stroke, spec);
    }

    /// [`Self::line_node`] under a label key, for a stroke that transitions
    /// or exits and needs a stable identity across frames.
    pub fn line_node_keyed(
        &mut self,
        label: &str,
        points: &[Vec2],
        stroke: Stroke,
        spec: NodeSpec,
    ) {
        if self.tree.is_empty() {
            return;
        }
        let key = self.child_key(label);
        self.line_with_key(key, points, stroke, spec);
    }

    /// [`Self::line_node`] under a data index; see [`Self::open_indexed`].
    pub fn line_node_indexed(&mut self, i: u64, points: &[Vec2], stroke: Stroke, spec: NodeSpec) {
        if self.tree.is_empty() {
            return;
        }
        let key = self.child_key_index(i);
        self.line_with_key(key, points, stroke, spec);
    }

    fn line_with_key(&mut self, key: Key, points: &[Vec2], stroke: Stroke, mut spec: NodeSpec) {
        let Some((id, rect)) = self.lines.push(points, stroke) else {
            return;
        };
        let ev = spec.events();
        if spec.hoverable
            || spec.focusable
            || ev.on_click.is_some()
            || ev.on_drag.is_some()
            || ev.on_key.is_some()
            || ev.on_hover.is_some()
        {
            self.diag.raise(Warning {
                code: crate::diag::LINE_IGNORES_INPUT,
                key,
                message: "a line takes no pointer input, so the interaction it declares does \
                          nothing; put it on the nodes the line connects"
                    .to_string(),
            });
        }
        // The stroke colour rides in the slot backgrounds tween through, so
        // `transition`, `enter` and `exit` reach it with no slot of its own;
        // nothing else of the box vocabulary applies to a stroke.
        spec.style.bg = stroke.color;
        spec.style.border_w = 0.0;
        spec.style.border_color = Color::TRANSPARENT;
        spec.style.shadow = crate::spec::Shadow::default();
        self.ease_spec(key, &mut spec);
        // The box is the stroke's own, and the points are stored relative
        // to it: it is not a size the view chose or a tween may lag.
        let anchor = spec
            .layout
            .float
            .map_or(crate::spec::FloatAnchor::Parent, |f| f.anchor);
        spec.layout.float = Some(crate::spec::FloatConfig {
            anchor,
            offset: crate::spec::Vec2Offset {
                x: rect.x,
                y: rect.y,
            },
            ..crate::spec::FloatConfig::default()
        });
        spec.layout.width = Sizing::Fixed(rect.w);
        spec.layout.height = Sizing::Fixed(rect.h);
        spec.layout.min_w = crate::spec::Min::px(0.0);
        spec.layout.max_w = f32::INFINITY;
        spec.layout.min_h = crate::spec::Min::px(0.0);
        spec.layout.max_h = f32::INFINITY;
        spec.layout.clip = false;
        spec.layout.scroll_x = false;
        spec.layout.scroll_y = false;
        self.any_float = true;
        if spec.style.opacity < 1.0 {
            self.any_opacity = true;
        }
        if let Some(events) = spec.events.as_deref()
            && events.on_layout.is_some()
        {
            self.any_layout = true;
        }
        if spec.transition.is_some() {
            match spec.anim.as_deref() {
                Some(anim) => {
                    if spec.slide || anim.enter.is_some_and(|e| e.offsets()) {
                        self.any_slide = true;
                    }
                    if anim.exit.is_some() {
                        self.any_exit = true;
                    }
                }
                None => self.any_slide |= spec.slide,
            }
        }
        let parent = self.current();
        self.tree
            .push(parent, key, self.origin, spec, NodeContent::Line(id));
    }

    /// A paragraph of styled spans, shaped and wrapped as one flow.
    pub fn rich_text_node(&mut self, spans: &[Span<'_>], base: TextStyle) {
        if self.tree.is_empty() {
            return;
        }
        let tid = {
            let sess = &mut *self.session.state();
            self.text
                .add_rich(spans, &base, &sess.resources, &mut sess.fonts)
        };
        let key = self.auto_key();
        let parent = self.current();
        self.tree.push(
            parent,
            key,
            self.origin,
            NodeSpec::default(),
            NodeContent::Text(tid),
        );
    }
}
