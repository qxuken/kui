//! The frame's back half: layout, then emission into the display list.
//!
//! `finish_frame` runs the passes; the rest are the passes — one node's
//! quads, the departed subtrees replayed as ghosts, position easing, the
//! `layout` events, the default focus ring — and the quad helpers they
//! share. Everything here reads the tree `builder` built and writes
//! `display`; the input side of the same frame is `dispatch`.

use super::*;

const SCROLLBAR_W: f32 = 4.0;
/// Thumb width while hovered or dragged.
const SCROLLBAR_ACTIVE_W: f32 = 6.0;
/// Grabbable gutter width (wider than the drawn thumb).
const SCROLLBAR_HIT_W: f32 = 10.0;
const SCROLLBAR_INSET: f32 = 2.0;
const SCROLLBAR_MIN: f32 = 24.0;
/// The default focus ring (see `docs/adr/0002-keyboard-focus-as-data.md`):
/// drawn this far outside the focused node, this thick, when focus is
/// keyboard-visible and the node styles nothing itself.
///
/// The *colour* is `theme.focus_ring` (ADR 0019, which revisits ADR
/// 0002's "a constant in the core, not a theme value"): the geometry is
/// still not a prop and still not negotiable, but a ring that cannot be
/// seen is not a focus indicator, and the pale blue that reads on a dark
/// page is invisible on a light one.
const FOCUS_RING_GAP: f32 = 2.0;
const FOCUS_RING_W: f32 = 2.0;

impl Core {
    /// The scrollbar thumb's colour at rest and while hovered or dragged.
    /// A wash over whatever it sits on rather than a fill, which is why
    /// it is two translucent colours and not one with an alpha applied.
    fn thumb_color(&self, active: bool) -> Color {
        let t = self.theme();
        if active {
            t.scrollbar_active
        } else {
            t.scrollbar
        }
    }

    /// Emits one node's quads and registers its hit/scroll regions.
    fn emit_node(
        &mut self,
        i: usize,
        rect: Rect,
        paint: Paint,
        hits: &mut Vec<HitRegion>,
        scroll_regions: &mut Vec<ScrollRegion>,
    ) {
        let Paint {
            clip,
            clip_id,
            scale,
            opacity,
        } = paint;
        let clip_px = clip.scaled(scale);
        // Behind a modal a node still draws, and stops taking input.
        let interactive = self.interactive(i);
        let spec = &self.tree.specs[i];
        let style = spec.style;
        let first_quad = self.display.quads.len();
        // A stroke's `bg` is its colour, not a box to fill, and it emits no
        // hit region: it takes no input (ADR 0010, decision 7).
        let is_line = matches!(self.tree.content[i], NodeContent::Line(_));
        if style.shadow.is_visible() {
            self.display
                .quads
                .push(shadow_quad(&style, rect, clip_id, scale));
        }
        if !is_line
            && (style.bg.is_visible() || (style.border_w > 0.0 && style.border_color.is_visible()))
        {
            self.display.quads.push(Quad {
                rect: rect.scaled(scale),
                color: style.bg,
                border_color: style.border_color,
                radius: style.radius.map(|r| r * scale),
                border_w: style.border_w * scale,
                blur: 0.0,
                kind: QuadKind::Solid,
                clip: clip_id,
                uv: [0; 4],
            });
        }
        if spec.hover_tracked() && interactive && !is_line {
            let parent = self.tree.parent[i];
            let parent_rect = if parent == NIL {
                Rect::new(0.0, 0.0, self.viewport.w, self.viewport.h)
            } else {
                let p = parent as usize;
                Rect::from_pos_size(self.tree.pos[p], self.tree.size[p])
            };
            // A disabled node keeps hover (a tooltip can say why) and loses
            // every interaction: it emits nothing and takes no focus.
            let live = !spec.disabled;
            hits.push(HitRegion {
                key: self.tree.keys[i],
                origin: self.tree.origins[i],
                rect,
                clip: clip.rect,
                payload: spec.events().on_click.clone().filter(|_| live),
                drag: spec.events().on_drag.clone().filter(|_| live),
                parent_rect,
                key_sink: spec.events().on_key.clone().filter(|_| live),
                key_up: spec.events().key_up,
                context_menu: spec.events().on_context_menu.clone().filter(|_| live),
                focusable: crate::access::focusable(&self.tree, i),
                edit_origin: None,
                select_scope: self.scope_of(i).filter(|_| live),
                window: spec.window,
                hover: spec.events().on_hover.clone(),
                group: spec.interact().hover_group,
                click_sound: spec.interact().click_sound.filter(|_| live),
                hover_sound: spec.interact().hover_sound,
                cursor: spec.cursor,
            });
        }
        if spec.layout.scroll_x || spec.layout.scroll_y {
            // A container behind a modal keeps its scrollbar drawn and
            // refuses the wheel and the thumb.
            scroll_regions.push(ScrollRegion {
                key: self.tree.keys[i],
                rect,
                clip: clip.rect,
                inert: !interactive,
            });
        }
        match self.tree.content[i] {
            NodeContent::Text(tid) => {
                let sel = self.sel_range(i, tid);
                let sess = &mut *self.session.state();
                self.text.emit(
                    tid,
                    self.tree.pos[i],
                    self.tree.size[i],
                    clip_px,
                    clip_id,
                    &mut self.display.clips,
                    &sess.resources,
                    &mut sess.fonts,
                    &mut self.atlas,
                    &mut self.display.quads,
                    sel,
                );
                // The keys above it, nearest first, so a query by the
                // `line` row (or a wrapper) finds the runs inside it.
                let (ancestors, depth) = self.text_ancestors(i);
                self.text.place(
                    self.tree.keys[i],
                    &ancestors[..depth],
                    tid,
                    self.tree.pos[i],
                    self.scope_of(i),
                    true,
                );
            }
            NodeContent::Cells(cid) => {
                // The window's selection, when it is in this very grid.
                let sel = self
                    .cell_selection
                    .filter(|s| s.node == self.tree.keys[i] && !s.is_empty());
                let at = self.cells_origin(i);
                let tint = self.theme.selection;
                let sess = &mut *self.session.state();
                self.cells.emit(
                    cid,
                    at,
                    clip_px,
                    clip_id,
                    &sess.resources,
                    &mut sess.fonts,
                    self.text.raster_mut(),
                    &mut self.atlas,
                    &mut self.display.quads,
                    sel.as_ref().map(|s| (s, tint)),
                );
            }
            NodeContent::Edit(key) => {
                let pad = spec.layout.padding;
                let content_origin = Vec2::new(rect.x + pad.l, rect.y + pad.t);
                let inner_w = (rect.w - pad.x()).max(0.0);
                // A single-line field scrolls its own text (F41). Resolved
                // here, where the box is known, and read back by the hit
                // region and the access runs so all three agree on where
                // the glyphs are.
                let offset = {
                    let sess = &mut *self.session.state();
                    self.edit.line_offset(key, inner_w * scale, &mut sess.fonts)
                };
                if interactive {
                    hits.push(HitRegion {
                        key,
                        origin: self.tree.origins[i],
                        rect,
                        clip: clip.rect,
                        payload: None,
                        drag: None,
                        parent_rect: rect,
                        // Shifted by what the field is scrolled: a click
                        // lands on the character under the pointer.
                        edit_origin: Some(Vec2::new(
                            content_origin.x - offset / scale,
                            content_origin.y,
                        )),
                        // An editor is its own selection scope: a press
                        // in it places a caret and drags a selection
                        // through the editor's own path, not the scope's.
                        select_scope: None,
                        key_sink: None,
                        key_up: false,
                        context_menu: None,
                        focusable: !spec.disabled,
                        window: None,
                        hover: None,
                        group: None,
                        click_sound: None,
                        hover_sound: None,
                        // The editor's own node carries any override.
                        cursor: spec.cursor,
                    });
                }
                let focused = self.edit.focused() == Some(key);
                let origin_phys = Vec2::new(
                    crate::geom::snap_px(content_origin.x * scale),
                    crate::geom::snap_px(content_origin.y * scale),
                );
                // A field bounds its own text horizontally — it is what
                // makes scrolling one legible rather than a line running
                // out over its neighbours. Horizontally only: the
                // ancestors own the vertical clip, and a descender or a
                // caret is not what a field is trying to cut off.
                // Narrowing the clip makes a new one, so it needs an entry
                // of its own; a multiline editor keeps the node's.
                let (edit_clip, edit_clip_id) = if self.edit.is_multiline(key) {
                    (clip_px, clip_id)
                } else {
                    let narrowed = clip_px.intersect(
                        Rect::new(
                            origin_phys.x,
                            clip_px.rect.y,
                            inner_w * scale,
                            clip_px.rect.h,
                        ),
                        crate::display::SQUARE,
                    );
                    (narrowed, self.display.intern_clip(narrowed))
                };
                let sess = &mut *self.session.state();
                self.edit.emit(
                    key,
                    origin_phys,
                    focused,
                    edit_clip,
                    edit_clip_id,
                    &mut sess.fonts,
                    &mut self.text,
                    &mut self.atlas,
                    &mut self.display.quads,
                );
            }
            NodeContent::Image(id) => {
                let sess = self.session.state();
                if let Some(entry) = sess.resources.image(id)
                    && let Some(slot) =
                        self.atlas
                            .get_or_insert_image(id, entry.width, entry.height, &entry.rgba)
                {
                    self.display.quads.push(Quad {
                        rect: rect.scaled(scale),
                        // White = untinted; radius rounds like a solid.
                        color: Color::WHITE,
                        border_color: Color::TRANSPARENT,
                        radius: spec.style.radius.map(|r| r * scale),
                        border_w: 0.0,
                        blur: 0.0,
                        kind: QuadKind::Image,
                        clip: clip_id,
                        uv: [slot.x, slot.y, slot.w, slot.h],
                    });
                }
            }
            NodeContent::Fragment(id) => {
                let draw = self.fragments.get(id);
                push_fragment(
                    &mut self.display.quads,
                    &mut self.display.fragments,
                    &mut self.display.fragment_sources,
                    &self.session.state().resources,
                    draw,
                    rect.scaled(scale),
                    spec.style.radius.map(|r| r * scale),
                    clip_id,
                );
            }
            NodeContent::Line(id) => {
                let (run, points) = self.lines.run(id);
                push_segments(
                    &mut self.display.quads,
                    self.tree.pos[i],
                    points,
                    run.width,
                    style.bg,
                    clip_id,
                    scale,
                );
            }
            NodeContent::Container => {}
        }
        if opacity < 1.0 {
            fade(&mut self.display.quads[first_quad..], opacity);
        }
    }

    /// Runs layout and emission into `output()`, and installs this frame's
    /// hit and scroll regions for input handling.
    pub fn finish_frame(&mut self) {
        self.layout_frame();
        self.emit_frame();
        self.building = false;
        self.snapshot_nodes();
    }

    /// The frame's first half: layout, then everything that resolves
    /// against it before a quad is emitted — the caret and reveal nudges,
    /// the `layout` events and the diagnostics, the declared window set,
    /// the modal scope and the Tab step a view asked for.
    fn layout_frame(&mut self) {
        // Tolerate unclosed containers (an FFI caller may have bailed early).
        self.stack.truncate(1);
        self.counters.truncate(1);

        {
            let sess = &mut *self.session.state();
            let mut measure = Measure {
                text: &mut self.text,
                cells: &mut self.cells,
                fonts: &mut sess.fonts,
                edit: &mut self.edit,
                resources: &sess.resources,
            };
            layout::compute(
                &mut self.tree,
                &mut measure,
                &mut self.scroll,
                self.viewport,
                self.scale,
            );
        }
        self.scroll_caret_into_view();
        // An explicit `reveal` after the caret nudge: the app asked for
        // this one, so it wins the offset if both want to move it.
        self.apply_pending_reveal();
        if self.any_slide {
            self.ease_positions();
        }
        // Positions are final: report the rects views asked about, and
        // look for the misconfigurations that would otherwise fail silently.
        if self.any_layout {
            self.emit_layout_events();
        }
        self.diag
            .check(&self.tree, &self.text, &self.edit, self.frame_no);
        // Text set for a key nothing had declared yet was held for this
        // frame (backlog F25). What it declared has taken its seed; what
        // is left named an editor no view draws, so drop it and say so.
        // Drained whatever `diag.enabled` says, so the gate changes what
        // is reported and never what is retained.
        for seed in self.edit.take_unclaimed_seeds() {
            self.diag.raise(match seed {
                crate::edit::Unclaimed::Key(key) => crate::diag::edit_text_without_editor(key),
                crate::edit::Unclaimed::Label(label) => {
                    crate::diag::edit_text_without_editor_label(&label)
                }
            });
        }
        // The declared window set, diffed against the session's: a frame
        // that declared a new name queues its `Open` here.
        self.sync_windows();
        // The frame's modal scope, and the focus it moves: emission reads
        // it (everything outside is inert) and so does the Tab ring.
        self.modal = if self.any_modal {
            self.modal_scope()
        } else {
            None
        };
        self.resolve_modal_focus();
        // Then the regions: a `focus_region` asked for during the build,
        // the region following a focus the build declared, and a region
        // that went away handing focus back (`docs/adr/0022`).
        self.resolve_regions();
        // The ring exists now: laid out, and scoped to the modal if there
        // is one, else to the region in effect. A step asked for during
        // the build lands here, so it wins over both the modal's own focus
        // move and a same-frame `set_focus`. Like a real Tab press, the
        // scroll it triggers shows on the next frame.
        if let Some(forward) = self.pending_focus_step.take() {
            self.focus_next(forward);
        }
    }

    /// The frame's second half: the laid-out tree into the display list,
    /// in paint order — in-flow content, then floating subtrees, each with
    /// the departed subtrees that were painted among them, then the
    /// Numbers the text nodes of the selection's scope in tree order and
    /// resolves the two ends against those ordinals. Run once per frame,
    /// after the scope map and before emission, because a run's range has
    /// to be known when the run is drawn and its *offset* cannot be —
    /// the runs after it have not been placed yet, and an end may be one
    /// of them (ADR 0017, decision 2).
    fn resolve_selection(&mut self) {
        self.sel_ords.clear();
        self.sel_ends = None;
        let Some(sel) = self.selection else { return };
        self.sel_ords.resize(self.tree.len(), u32::MAX);
        let (mut anchor, mut focus) = (None, None);
        // The built runs, in order, with the virtualised row each is in:
        // what an end whose own node is *not* built is placed against.
        let mut built: Vec<(u32, Option<u64>, usize)> = Vec::new();
        let mut ord = 0u32;
        for i in 0..self.tree.len() {
            if self.scopes.get(i).copied().flatten() != Some(sel.scope)
                || !matches!(self.tree.content[i], NodeContent::Text(_))
            {
                continue;
            }
            self.sel_ords[i] = ord;
            let key = self.tree.keys[i];
            let len = match self.tree.content[i] {
                NodeContent::Text(tid) => self.text.content_len(tid),
                _ => 0,
            };
            built.push((ord, self.rows.get(i).copied().flatten(), len));
            if key == sel.anchor.node {
                anchor = Some((ord, sel.anchor.byte));
            }
            if key == sel.focus.node {
                focus = Some((ord, sel.focus.byte));
            }
            ord += 1;
        }
        // An end this frame did not build is placed by its row's index in
        // the data: before everything built, after it, or at the boundary
        // it falls on. That is what lets the built middle paint while a
        // virtual list scrolls under a selection (ADR 0017, tier 3).
        let place = |end: &crate::select::Endpoint| -> Option<(u32, usize)> {
            let row = end.row?;
            let (start, last) = (built.first()?, built.last()?);
            // Against the built runs that *carry* a row, not the first and
            // last of everything built: a scope can hold plain labels
            // beside virtual rows — a header, a footer — and a label says
            // nothing about where a row sits in the data. Comparing
            // against one puts an end below the list at the top of it.
            let hi = built.iter().rev().find_map(|(_, r, _)| *r)?;
            if row > hi {
                return Some((last.0, last.2));
            }
            // Below the first row, or inside the built range without being
            // built — a hole, which a contiguous virtual window does not
            // have. Either way the start is the nearest honest boundary.
            Some((start.0, 0))
        };
        let anchor = anchor.or_else(|| place(&sel.anchor));
        let focus = focus.or_else(|| place(&sel.focus));
        if let (Some(a), Some(f)) = (anchor, focus) {
            self.sel_ends = Some(crate::select::Ends::ordered(a, f));
        }
    }

    /// The bytes of node `i`'s text the selection covers, with the colour
    /// to paint under them. `None` on every node of every frame that has
    /// no selection.
    fn sel_range(&self, i: usize, tid: crate::tree::TextId) -> Option<((usize, usize), Color)> {
        let ends = self.sel_ends?;
        let ord = self.sel_ords.get(i).copied()?;
        if ord == u32::MAX {
            return None;
        }
        let range = ends.range_in(ord, self.text.content_len(tid))?;
        Some((range, self.theme().selection))
    }

    /// The innermost selection scope node `i` is inside, if any. Empty on
    /// every frame that declares no `selectable` at all, where the map is
    /// not even sized.
    #[inline]
    fn scope_of(&self, i: usize) -> Option<Key> {
        self.scopes.get(i).copied().flatten()
    }

    /// The keys above node `i`, nearest first, as many as a `TextPlace`
    /// remembers — what lets a query by a `line` row or a wrapper find
    /// the runs inside it.
    fn text_ancestors(&self, i: usize) -> ([Key; 4], usize) {
        let mut ancestors = [Key::ROOT; 4];
        let mut depth = 0;
        let mut p = self.tree.parent[i];
        while p != NIL && depth < ancestors.len() {
            ancestors[depth] = self.tree.keys[p as usize];
            depth += 1;
            p = self.tree.parent[p as usize];
        }
        (ancestors, depth)
    }

    /// scrollbars and the focus ring on top — and the hit and scroll
    /// regions the next input is tested against.
    fn emit_frame(&mut self) {
        let scale = self.scale;
        let mut hits: Vec<HitRegion> = self.interaction.take_hit_buffer();
        let mut scroll_regions: Vec<ScrollRegion> = Vec::new();
        self.display.viewport = Size::new(self.viewport.w * scale, self.viewport.h * scale);
        self.display.scale = scale;
        self.display.time = self.anim.time().unwrap_or(0.0) as f32;

        // configure_root can also introduce a clipper, or a fade.
        let root_clips = !self.tree.is_empty() && self.tree.specs[0].layout.clips();
        let any_clip = self.any_clip || root_clips;
        let any_rounded_clip = self.any_rounded_clip
            || (root_clips && self.tree.specs[0].style.radius != crate::display::SQUARE);
        let any_opacity =
            self.any_opacity || (!self.tree.is_empty() && self.tree.specs[0].style.opacity < 1.0);
        let any_float = self.any_float;
        // Read once: every per-node selection check below is behind it.
        let any_selectable = self.tree.any_selectable;

        // inherited clip per node (logical): ancestors only, not the node
        // itself. Only materialized when something actually clips.
        // Index 0 is the clip that clips nothing, so an unclipped frame
        // interns once and every quad on it names entry zero. Seeded
        // rather than found, so `clip_of` has something to answer with
        // even on a frame that emitted no quad at all.
        let no_clip = self.display.intern_clip(Clip::NONE.scaled(scale));
        self.clips.clear();
        self.clip_ids.clear();
        if any_clip {
            self.clips.resize(self.tree.len(), Clip::NONE);
            self.clip_ids.resize(self.tree.len(), no_clip);
        }
        self.opacity.clear();
        if any_opacity {
            self.opacity.resize(self.tree.len(), 1.0);
        }
        self.in_float.clear();
        if any_float {
            self.in_float.resize(self.tree.len(), false);
        }
        // The innermost `selectable` above each node (ADR 0017). Parents
        // precede their children in the tree array, so one forward pass
        // inherits it; a node declaring `selectable` inside another scope
        // takes the text under it and is warned about, because two scopes
        // over one run would each think they own it.
        self.scopes.clear();
        self.rows.clear();
        if any_selectable {
            self.scopes.resize(self.tree.len(), None);
            self.rows.resize(self.tree.len(), None);
            // The rows a virtual list built, by node, so the walk below
            // can carry each one down to the text inside it.
            for &(node, index) in &self.tree.indexed {
                if let Some(slot) = self.rows.get_mut(node as usize) {
                    *slot = Some(index);
                }
            }
            for i in 0..self.tree.len() {
                let parent = self.tree.parent[i];
                // A floating subtree escapes the scope it floats out of,
                // the way it escapes the clip: a popover over a card is
                // not part of the card's paragraph, and a float is
                // emitted in a later pass than its tree position, which
                // would put its runs out of reading order anyway.
                let outer = if parent == NIL || self.tree.specs[i].layout.float.is_some() {
                    None
                } else {
                    self.scopes[parent as usize]
                };
                // An inner scope takes the text under it; the nesting
                // itself is reported by `diag`, over the finished tree.
                //
                // A `cells` grid is its own scope and never joins the one
                // around it: it selects in cells, and a selection that ran
                // from a paragraph into a terminal screen would be two
                // kinds of selection at once (ADR 0017, decision 4).
                let grid = matches!(self.tree.content[i], NodeContent::Cells(_));
                self.scopes[i] = if self.tree.specs[i].interact().selectable {
                    Some(self.tree.keys[i])
                } else if grid {
                    None
                } else {
                    outer
                };
                // A row's index reaches the text inside it: the node that
                // declared it keeps its own, everything under it inherits.
                if self.rows[i].is_none() && parent != NIL {
                    self.rows[i] = self.rows[parent as usize];
                }
            }
        }
        self.resolve_selection();

        // Exits: what the previous frame declared and this one does not is
        // copied out of the tree the previous frame left behind, and every
        // departing subtree still in flight is taken out for the passes,
        // which paint each one where its node was (see `depart`).
        if !self.prev_tree.is_empty() || !self.depart.is_empty() {
            self.collect_departures();
        }
        let mut replay = Replay::default();
        let mut took_ghosts = false;
        if !self.depart.is_empty()
            && let Some(now) = self.anim.time()
        {
            replay = self.depart.begin_replay(now);
            took_ghosts = true;
        }
        let any_ghost = !replay.is_empty();

        // Pass 1: clip/float propagation + in-flow emission (preorder =
        // paint order; parents precede children).
        for i in 0..self.tree.len() {
            let parent = self.tree.parent[i];
            let floats_here = any_float && self.tree.specs[i].layout.float.is_some();
            if any_float {
                self.in_float[i] = floats_here || (parent != NIL && self.in_float[parent as usize]);
            }
            let rect = Rect::from_pos_size(self.tree.pos[i], self.tree.size[i]);
            // Opacity multiplies down the tree, floats included: a tooltip
            // inside a fading panel fades with it.
            let opacity = if !any_opacity {
                1.0
            } else {
                let inherited = if parent == NIL {
                    1.0
                } else {
                    self.opacity[parent as usize]
                };
                let o = inherited * self.tree.specs[i].style.opacity;
                self.opacity[i] = o;
                o
            };
            let (clip, clip_id) = if !any_clip {
                (Clip::NONE, no_clip)
            } else {
                // Floating nodes escape ancestor clips.
                let (clip, id) = if parent == NIL || floats_here {
                    (Clip::NONE, no_clip)
                } else {
                    let p = parent as usize;
                    if self.tree.specs[p].layout.clips() {
                        // A clipper with a radius rounds what it clips, so
                        // the children of a rounded card stay inside its
                        // corners (see `display::Clip`).
                        let box_rect = Rect::from_pos_size(self.tree.pos[p], self.tree.size[p]);
                        let box_radius = if any_rounded_clip {
                            self.tree.specs[p].style.radius
                        } else {
                            crate::display::SQUARE
                        };
                        let clip = self.clips[p].intersect(box_rect, box_radius);
                        (clip, self.display.intern_clip(clip.scaled(scale)))
                    } else {
                        // The overwhelming case: the clip is the parent's,
                        // so the entry is too, and nothing is compared.
                        (self.clips[p], self.clip_ids[p])
                    }
                };
                self.clips[i] = clip;
                self.clip_ids[i] = id;
                (clip, id)
            };
            if any_float && self.in_float[i] {
                continue; // deferred to the float pass
            }
            // A departing subtree painted just under this node last time
            // goes first, so it stays under it.
            if any_ghost && replay.may_precede(self.tree.keys[i]) {
                let key = self.tree.keys[i];
                replay.paint(Pass::InFlow, Some(key), |g, play| {
                    self.emit_ghost(g, play, scale)
                });
            }
            // Entirely clipped away: skip drawing and hit-testing.
            // Rect, not rounded: a node that survives only in a corner's
            // arc is drawn and clipped rather than culled.
            let visible = rect.intersect(&clip.rect);
            if visible.w <= 0.0 || visible.h <= 0.0 {
                // Culled — but a text run inside a selection scope keeps
                // its place in the order and its content reachable, so a
                // selection can run past the edge of a scroller (ADR
                // 0017, tier 2). Marked undrawn: no hit region, no
                // `text_hit`, nothing a pointer can find.
                //
                // Behind the frame's own flag, hoisted out of the loop:
                // a frame that declares no scope does not pay a lookup
                // per culled node to find that out (C15).
                if any_selectable
                    && let Some(scope) = self.scope_of(i)
                    && let NodeContent::Text(tid) = self.tree.content[i]
                {
                    let (anc, depth) = self.text_ancestors(i);
                    self.text.place(
                        self.tree.keys[i],
                        &anc[..depth],
                        tid,
                        self.tree.pos[i],
                        Some(scope),
                        false,
                    );
                }
                continue;
            }
            let paint = Paint {
                clip,
                clip_id,
                scale,
                opacity,
            };
            self.emit_node(i, rect, paint, &mut hits, &mut scroll_regions);
        }

        if any_ghost {
            // The in-flow ghosts whose place is gone: the end of their
            // pass, still under every float.
            replay.paint(Pass::InFlow, None, |g, play| {
                self.emit_ghost(g, play, scale)
            });
        }

        // Pass 2: floating subtrees, on top of all in-flow content (their
        // hit regions land last too, so they're topmost for input).
        if any_float {
            for i in 0..self.tree.len() {
                if !self.in_float[i] {
                    continue;
                }
                if any_ghost && replay.may_precede(self.tree.keys[i]) {
                    let key = self.tree.keys[i];
                    replay.paint(Pass::Float, Some(key), |g, play| {
                        self.emit_ghost(g, play, scale)
                    });
                }
                let rect = Rect::from_pos_size(self.tree.pos[i], self.tree.size[i]);
                let clip = if any_clip { self.clips[i] } else { Clip::NONE };
                let clip_id = if any_clip { self.clip_ids[i] } else { no_clip };
                let opacity = if any_opacity { self.opacity[i] } else { 1.0 };
                let visible = rect.intersect(&clip.rect);
                if visible.w <= 0.0 || visible.h <= 0.0 {
                    continue;
                }
                let paint = Paint {
                    clip,
                    clip_id,
                    scale,
                    opacity,
                };
                self.emit_node(i, rect, paint, &mut hits, &mut scroll_regions);
            }
        }

        if any_ghost {
            // The float ghosts whose place is gone, and every ghost of a
            // float in a frame with no floats to paint among.
            replay.paint(Pass::Float, None, |g, play| self.emit_ghost(g, play, scale));
        }
        if took_ghosts {
            self.depart.end_replay(replay);
        }

        // Scrollbars, on top of content: indicator quads plus the hit
        // regions that make their thumbs draggable.
        let cursor = self.interaction.cursor();
        let mut scrollbars: Vec<ScrollbarRegion> = Vec::new();
        for r in &scroll_regions {
            let i = self
                .tree
                .keys
                .iter()
                .position(|k| *k == r.key)
                .expect("scroll region from this frame");
            let max = self.tree.scroll_max[i];
            let offset = self.scroll.offset(r.key);
            let clip_id = self.clip_ids[i];
            let opacity = self.opacity.get(i).copied().unwrap_or(1.0);
            if max.y > 0.0 {
                let track_h = r.rect.h - 2.0 * SCROLLBAR_INSET;
                let bar_h = (track_h * r.rect.h / (r.rect.h + max.y)).max(SCROLLBAR_MIN);
                let t = (offset.y / max.y).clamp(0.0, 1.0);
                let track = Rect::new(
                    r.rect.x + r.rect.w - SCROLLBAR_HIT_W,
                    r.rect.y + SCROLLBAR_INSET,
                    SCROLLBAR_HIT_W,
                    track_h,
                );
                let active = self.interaction.is_scrollbar_dragging(r.key, ScrollAxis::Y)
                    || cursor.is_some_and(|p| track.contains(p));
                let w = if active {
                    SCROLLBAR_ACTIVE_W
                } else {
                    SCROLLBAR_W
                };
                let thumb = Rect::new(
                    r.rect.x + r.rect.w - w - SCROLLBAR_INSET,
                    r.rect.y + SCROLLBAR_INSET + t * (track_h - bar_h),
                    w,
                    bar_h,
                );
                let mut bar = scrollbar_quad(thumb, scale, clip_id, self.thumb_color(active));
                bar.color.a *= opacity;
                self.display.quads.push(bar);
                scrollbars.push(ScrollbarRegion {
                    key: r.key,
                    axis: ScrollAxis::Y,
                    thumb,
                    track,
                    bar_len: bar_h,
                    max: max.y,
                    inert: r.inert,
                });
            }
            if max.x > 0.0 {
                let track_w = r.rect.w - 2.0 * SCROLLBAR_INSET;
                let bar_w = (track_w * r.rect.w / (r.rect.w + max.x)).max(SCROLLBAR_MIN);
                let t = (offset.x / max.x).clamp(0.0, 1.0);
                let track = Rect::new(
                    r.rect.x + SCROLLBAR_INSET,
                    r.rect.y + r.rect.h - SCROLLBAR_HIT_W,
                    track_w,
                    SCROLLBAR_HIT_W,
                );
                let active = self.interaction.is_scrollbar_dragging(r.key, ScrollAxis::X)
                    || cursor.is_some_and(|p| track.contains(p));
                let w = if active {
                    SCROLLBAR_ACTIVE_W
                } else {
                    SCROLLBAR_W
                };
                let thumb = Rect::new(
                    r.rect.x + SCROLLBAR_INSET + t * (track_w - bar_w),
                    r.rect.y + r.rect.h - w - SCROLLBAR_INSET,
                    bar_w,
                    w,
                );
                let mut bar = scrollbar_quad(thumb, scale, clip_id, self.thumb_color(active));
                bar.color.a *= opacity;
                self.display.quads.push(bar);
                scrollbars.push(ScrollbarRegion {
                    key: r.key,
                    axis: ScrollAxis::X,
                    thumb,
                    track,
                    bar_len: bar_w,
                    max: max.x,
                    inert: r.inert,
                });
            }
        }

        self.emit_focus_ring(scale);

        self.interaction.set_hits(hits);
        // A new frame can move a hover-sound node under a still cursor.
        self.flush_sound_requests();
        self.session.state().audio.reconcile();
        self.interaction.scroll_regions = scroll_regions;
        self.interaction.scrollbars = scrollbars;
        self.ime_rect = self.focused_caret_rect();
    }

    /// The exit diff: every key the previous frame declared an `exit` on
    /// and this frame does not becomes a departing subtree, copied out of
    /// `prev_tree` — the frame that still had it — and handed to the store
    /// with the place it painted in, so its ghost keeps it.
    /// A ghost whose key came back is retired here too: the live node wins.
    ///
    /// Only two kinds of key are interesting (the previous frame's
    /// exit-declaring roots, and the roots already departing), and both are
    /// few, so the walk over *this* frame's keys — the part that scales
    /// with the frame — is one AND against a 64-bit membership mask per
    /// node, and a hash lookup only for the handful that collide with it.
    fn collect_departures(&mut self) {
        let Some(now) = self.anim.time() else {
            // No clock: every transition snaps, and an exit that snaps is
            // the plain disappearance it has always been.
            self.depart.clear();
            return;
        };
        // The steady state: the view declared the same nodes in the same
        // order, so nothing left and nothing came back. Two flat arrays of
        // u64 compared is cheaper than anything that looks at the keys one
        // at a time, and it is the case almost every frame is.
        if self.prev_tree.keys == self.tree.keys {
            return;
        }
        let mut watch: FxHashSet<Key> = FxHashSet::default();
        let mut mask = 0u64;
        for k in self.depart.keys() {
            watch.insert(k);
            mask |= 1u64 << (k.0 & 63);
        }
        let mut candidates: Vec<usize> = Vec::new();
        for i in 0..self.prev_tree.len() {
            if crate::depart::can_depart(&self.prev_tree.specs[i]) {
                candidates.push(i);
                let k = self.prev_tree.keys[i];
                watch.insert(k);
                mask |= 1u64 << (k.0 & 63);
            }
        }
        if watch.is_empty() {
            return;
        }
        let mut live: FxHashSet<Key> = FxHashSet::default();
        for &k in &self.tree.keys {
            if mask & (1u64 << (k.0 & 63)) != 0 && watch.contains(&k) {
                live.insert(k);
            }
        }
        self.depart.retire_returned(&live);
        // The frame's removal, whole, before any of it is copied: the roots
        // that actually left, in tree order so a departing subtree swallows
        // the exits nested inside it rather than drawing them a second time
        // on top, and how many nodes they come to together.
        let mut roots: Vec<usize> = Vec::new();
        let mut wanted = 0usize;
        let mut swallowed_until = 0usize;
        for i in candidates {
            if i < swallowed_until || live.contains(&self.prev_tree.keys[i]) {
                continue;
            }
            swallowed_until = self.prev_tree.subtree_end(i);
            wanted += swallowed_until - i;
            roots.push(i);
        }
        if roots.is_empty() {
            return;
        }
        // ADR 0012, decisions 2 and 3: the removal animates whole or not at
        // all, and takes the room it needs from the oldest ghosts in flight
        // before it is refused. Refused means every node of it vanishes at
        // once — what a node with no `exit` does — and one warning for the
        // frame, keyed by its first departing root, says how much did.
        if !self.depart.admit(wanted) {
            self.diag.raise(Warning {
                code: crate::diag::EXIT_BUDGET,
                key: self.prev_tree.keys[roots[0]],
                message: format!(
                    "this frame removed {wanted} nodes declaring `exit` and the exit store \
                     holds {}, so none of that removal animated: every departing node \
                     vanished at once, as a node with no `exit` does; `exit` is per node, \
                     and a list that drops many rows at once wants it on the list, not on \
                     every row",
                    crate::depart::MAX_NODES
                ),
            });
            return;
        }
        // The previous frame's paint order, built on the first departure:
        // a frame with one is a frame that changed shape and paid for a
        // layout, and the frames that did not never get here.
        let mut order: Option<PaintOrder> = None;
        for i in roots {
            let place = order
                .get_or_insert_with(|| PaintOrder::of(&self.prev_tree, &self.tree))
                .place(&self.prev_tree, i);
            // The group opacity the root inherited from ancestors that are
            // now gone: a subtree already half-faded departs from there.
            let mut base = 1.0;
            let mut a = self.prev_tree.parent[i];
            while a != NIL {
                base *= self.prev_tree.specs[a as usize].style.opacity;
                a = self.prev_tree.parent[a as usize];
            }
            self.depart.depart(
                &self.prev_tree,
                i,
                now,
                base,
                place,
                &self.text,
                &self.lines,
                &self.fragments,
            );
        }
    }

    /// One departing subtree's quads: frozen rects moved by however far
    /// its `exit` has got, outside every clip (its ancestors may be gone).
    /// A smaller `emit_node`: the parts a picture has (shadow, background,
    /// border, its content) and none of the parts a node has — no hit
    /// region, no scroll region, no access row.
    fn emit_ghost(&mut self, g: &Ghost, play: &Playback, scale: f32) {
        // Whole physical pixels, for the reason a slide's is: a ghost is
        // mostly text, and it moves for its whole life.
        let offset = play.offset.snapped(scale);
        self.ghost_opacity.clear();
        self.ghost_opacity.resize(g.nodes.len(), 1.0);
        self.ghost_clip.clear();
        self.ghost_clip.resize(g.nodes.len(), Clip::NONE);
        self.ghost_clip_ids.clear();
        self.ghost_clip_ids.resize(g.nodes.len(), NO_CLIP_ID);
        self.ghost_rect.clear();
        self.ghost_rect
            .resize(g.nodes.len(), Rect::new(0.0, 0.0, 0.0, 0.0));
        for (i, node) in g.nodes.iter().enumerate() {
            let mut rect = Rect::new(
                node.rect.x + offset.x,
                node.rect.y + offset.y,
                node.rect.w,
                node.rect.h,
            );
            let mut style = node.spec.style;
            let inherited = if node.parent == NIL {
                // The root carries the eased slots; an `exit` says nothing
                // about the subtree under it, which fades and moves with
                // its root and no more.
                if let Some(bg) = play.bg {
                    style.bg = bg;
                }
                if let Some(radius) = play.radius {
                    style.radius = radius;
                }
                if let Some((w, h)) = play.size {
                    // The root's own box only: the subtree inside it is a
                    // picture, and re-laying it out is the one thing a
                    // frozen ghost must not do.
                    rect.w = w.unwrap_or(rect.w);
                    rect.h = h.unwrap_or(rect.h);
                }
                style.opacity = play.opacity;
                play.base_opacity
            } else {
                self.ghost_opacity[node.parent as usize]
            };
            let opacity = (inherited * style.opacity).clamp(0.0, 1.0);
            self.ghost_opacity[i] = opacity;
            // The clip is the subtree's own: a scroll box or `clip` node
            // inside the picture still bounds what it held (the rows a
            // virtual list built past its edge stay past it), while the
            // ancestors outside the picture, which may be gone, clip
            // nothing. Same rule as the live pass, from the root down.
            let (clip, clip_id) = if node.parent == NIL || node.spec.layout.float.is_some() {
                (Clip::NONE, NO_CLIP_ID)
            } else {
                let p = node.parent as usize;
                let inherited = self.ghost_clip[p];
                if g.nodes[p].spec.layout.clips() {
                    let clip =
                        inherited.intersect(self.ghost_rect[p], g.nodes[p].spec.style.radius);
                    (clip, self.display.intern_clip(clip.scaled(scale)))
                } else {
                    (inherited, self.ghost_clip_ids[p])
                }
            };
            self.ghost_clip[i] = clip;
            self.ghost_clip_ids[i] = clip_id;
            self.ghost_rect[i] = rect;
            let visible = rect.intersect(&clip.rect);
            if visible.w <= 0.0 || visible.h <= 0.0 {
                continue;
            }
            let clip_px = clip.scaled(scale);
            let first_quad = self.display.quads.len();
            if style.shadow.is_visible() {
                self.display
                    .quads
                    .push(shadow_quad(&style, rect, clip_id, scale));
            }
            if style.bg.is_visible() || (style.border_w > 0.0 && style.border_color.is_visible()) {
                self.display.quads.push(Quad {
                    rect: rect.scaled(scale),
                    color: style.bg,
                    border_color: style.border_color,
                    radius: style.radius.map(|r| r * scale),
                    border_w: style.border_w * scale,
                    blur: 0.0,
                    kind: QuadKind::Solid,
                    clip: clip_id,
                    uv: [0; 4],
                });
            }
            match node.content {
                GhostContent::Container => {}
                GhostContent::Text { cache_key, color } => {
                    // None once the shaped buffer has been evicted: a ghost
                    // older than the text cache draws no text rather than
                    // somebody else's.
                    let sess = &mut *self.session.state();
                    if let Some(tid) = self.text.readd(cache_key, color) {
                        self.text.emit(
                            tid,
                            Vec2::new(rect.x, rect.y),
                            Size::new(rect.w, rect.h),
                            clip_px,
                            clip_id,
                            &mut self.display.clips,
                            &sess.resources,
                            &mut sess.fonts,
                            &mut self.atlas,
                            &mut self.display.quads,
                            // A departing subtree takes no input and
                            // holds no selection; it records no place
                            // either, so there is nothing to resolve.
                            None,
                        );
                    }
                }
                GhostContent::Edit(key) => {
                    let pad = node.spec.layout.padding;
                    let origin = Vec2::new(
                        crate::geom::snap_px((rect.x + pad.l) * scale),
                        crate::geom::snap_px((rect.y + pad.t) * scale),
                    );
                    // A field leaves scrolled where it was scrolled to, so
                    // it takes its own horizontal clip with it (F41).
                    let clip_px = if self.edit.is_multiline(key) {
                        clip_px
                    } else {
                        clip_px.intersect(
                            Rect::new(
                                origin.x,
                                clip_px.rect.y,
                                (rect.w - pad.x()).max(0.0) * scale,
                                clip_px.rect.h,
                            ),
                            crate::display::SQUARE,
                        )
                    };
                    // Never focused: the departing subtree gave the
                    // keyboard up the frame it stopped being declared.
                    let sess = &mut *self.session.state();
                    self.edit.emit(
                        key,
                        origin,
                        false,
                        clip_px,
                        clip_id,
                        &mut sess.fonts,
                        &mut self.text,
                        &mut self.atlas,
                        &mut self.display.quads,
                    );
                }
                GhostContent::Image(id) => {
                    let sess = self.session.state();
                    if let Some(entry) = sess.resources.image(id)
                        && let Some(slot) = self.atlas.get_or_insert_image(
                            id,
                            entry.width,
                            entry.height,
                            &entry.rgba,
                        )
                    {
                        self.display.quads.push(Quad {
                            rect: rect.scaled(scale),
                            color: Color::WHITE,
                            border_color: Color::TRANSPARENT,
                            radius: style.radius.map(|r| r * scale),
                            border_w: 0.0,
                            blur: 0.0,
                            kind: QuadKind::Image,
                            clip: clip_id,
                            uv: [slot.x, slot.y, slot.w, slot.h],
                        });
                    }
                }
                GhostContent::Fragment(draw) => {
                    // The picture is frozen at departure — the parameters
                    // are the ones the node last declared — while the box
                    // eases and the group opacity fades it.
                    push_fragment(
                        &mut self.display.quads,
                        &mut self.display.fragments,
                        &mut self.display.fragment_sources,
                        &self.session.state().resources,
                        draw,
                        rect.scaled(scale),
                        style.radius.map(|r| r * scale),
                        clip_id,
                    );
                }
                GhostContent::Line { first, len, width } => {
                    // The points are the ghost's own copy; the colour is
                    // the `bg` slot, which `play.bg` eases on the root.
                    let points = &g.points[first as usize..(first + len) as usize];
                    push_segments(
                        &mut self.display.quads,
                        Vec2::new(rect.x, rect.y),
                        points,
                        width,
                        style.bg,
                        clip_id,
                        scale,
                    );
                }
            }
            if opacity < 1.0 {
                fade(&mut self.display.quads[first_quad..], opacity);
            }
        }
    }

    /// After layout: nodes that `slide` ease from last frame's position
    /// toward where layout put them, carrying their subtree along (hit
    /// regions come from the same positions, so input follows the motion).
    /// Nodes whose `enter` has an offset start that far away on first
    /// sight and ease in the same way; without `slide` that entrance is
    /// all their position ever eases. Preorder means a parent shifts before
    /// its children are visited, so nested sliders ease relative to an
    /// already-eased parent.
    fn ease_positions(&mut self) {
        for i in 0..self.tree.len() {
            let spec = &self.tree.specs[i];
            let Some(t) = spec.transition else {
                continue;
            };
            let enter = spec.anim().enter.filter(|e| e.offsets());
            if !spec.slide && enter.is_none() {
                continue;
            }
            let key = self.tree.keys[i];
            let target = self.tree.pos[i];
            let from = enter.map(|e| [target.x + e.dx, target.y + e.dy, 0.0, 0.0]);
            let v = self.anim.drive(
                key,
                Slot::Pos,
                from,
                [target.x, target.y, 0.0, 0.0],
                t,
                spec.slide,
            );
            // Whole physical pixels, so the text inside moves with its box
            // rather than wobbling in it (`Vec2::snapped`). Rounding here
            // and not at the end means the last frame lands exactly on the
            // layout position, as it did before.
            let d = Vec2::new(v[0] - target.x, v[1] - target.y).snapped(self.scale);
            if d.x == 0.0 && d.y == 0.0 {
                continue;
            }
            let end = self.tree.subtree_end(i);
            for p in &mut self.tree.pos[i..end] {
                p.x += d.x;
                p.y += d.y;
            }
        }
    }

    /// After layout: every `on_layout` node whose rect differs from the one
    /// last reported for its key — or that was not seen last frame — posts
    /// `{kind="layout", x, y, w, h, parent, tag}`, pending like a `resize`.
    /// A frame that leaves a node where it was posts nothing, so a view
    /// that stores the rect in its model and redraws does not loop.
    fn emit_layout_events(&mut self) {
        let frame_no = self.frame_no;
        for i in 0..self.tree.len() {
            let Some(tag) = &self.tree.specs[i].events().on_layout else {
                continue;
            };
            let key = self.tree.keys[i];
            let rect = Rect::from_pos_size(self.tree.pos[i], self.tree.size[i]);
            let changed = match self.layouts.get(&key) {
                Some((last, seen)) if *seen + 1 == frame_no => *last != rect,
                _ => true,
            };
            self.layouts.insert(key, (rect, frame_no));
            if !changed {
                continue;
            }
            let parent = self.tree.parent[i];
            let parent_rect = if parent == NIL {
                Rect::new(0.0, 0.0, self.viewport.w, self.viewport.h)
            } else {
                let p = parent as usize;
                Rect::from_pos_size(self.tree.pos[p], self.tree.size[p])
            };
            let rect_value = |r: Rect| {
                Value::map([
                    ("x", Value::Float(r.x as f64)),
                    ("y", Value::Float(r.y as f64)),
                    ("w", Value::Float(r.w as f64)),
                    ("h", Value::Float(r.h as f64)),
                ])
            };
            let mut entries = vec![
                ("kind".to_string(), Value::str("layout")),
                ("x".to_string(), Value::Float(rect.x as f64)),
                ("y".to_string(), Value::Float(rect.y as f64)),
                ("w".to_string(), Value::Float(rect.w as f64)),
                ("h".to_string(), Value::Float(rect.h as f64)),
                ("parent".to_string(), rect_value(parent_rect)),
            ];
            if *tag != Value::Null {
                entries.push(("tag".to_string(), tag.clone()));
            }
            self.pending.push(UiEvent {
                origin: self.tree.origins[i],
                window: WindowId::MAIN,
                key,
                payload: Value::Map(entries),
            });
        }
    }

    /// After content and scrollbars: the default focus ring around the
    /// keyboard-visibly focused node, on top of everything, in the same
    /// display list every binding draws. Not for editors (the caret shows
    /// focus), key sinks (an app surface styles itself, through
    /// `is_focused` / `focus_visible`) or nodes declaring `focus_bg`.
    fn emit_focus_ring(&mut self, scale: f32) {
        if !self.focus_visible {
            return;
        }
        let Some(i) = self.focus_index() else {
            return;
        };
        let spec = &self.tree.specs[i];
        let editor = matches!(self.tree.content[i], NodeContent::Edit(_))
            || spec
                .access()
                .role
                .is_some_and(crate::access::Role::is_editor);
        if editor
            || spec.events().on_key.is_some()
            || spec.interact().focus_bg.is_some()
            || spec.disabled
        {
            return;
        }
        let node = Rect::from_pos_size(self.tree.pos[i], self.tree.size[i]);
        let rect = Rect::new(
            node.x - FOCUS_RING_GAP,
            node.y - FOCUS_RING_GAP,
            node.w + 2.0 * FOCUS_RING_GAP,
            node.h + 2.0 * FOCUS_RING_GAP,
        );
        let clip = self.clips.get(i).copied().unwrap_or(Clip::NONE);
        // No entry means nothing clipped this frame, which is entry zero.
        let clip_id = self.clip_ids.get(i).copied().unwrap_or(NO_CLIP_ID);
        let visible = rect.intersect(&clip.rect);
        if visible.w <= 0.0 || visible.h <= 0.0 {
            return;
        }
        let mut ring = self.theme().focus_ring;
        ring.a *= self.opacity.get(i).copied().unwrap_or(1.0);
        self.display.quads.push(Quad {
            rect: rect.scaled(scale),
            color: Color::TRANSPARENT,
            border_color: ring,
            radius: spec.style.radius.map(|r| (r + FOCUS_RING_GAP) * scale),
            border_w: FOCUS_RING_W * scale,
            blur: 0.0,
            kind: QuadKind::Solid,
            clip: clip_id,
            uv: [0; 4],
        });
    }

    /// See the `ime_rect` field. None when nothing with a caret is
    /// focused: neither a stock editor nor a sink holding a `line` that
    /// declares one.
    pub fn ime_rect(&self) -> Option<Rect> {
        self.ime_rect
    }

    fn focused_caret_rect(&mut self) -> Option<Rect> {
        if let Some(key) = self.edit.focused() {
            let i =
                (0..self.tree.len()).find(|&i| self.tree.content[i] == NodeContent::Edit(key))?;
            let caret = self.edit_with_fonts(|edit, fs| edit.caret_rect(key, fs))?;
            let pad = self.tree.specs[i].layout.padding;
            return Some(Rect::new(
                self.tree.pos[i].x + pad.l + caret.x / self.scale,
                self.tree.pos[i].y + pad.t + caret.y / self.scale,
                caret.w / self.scale,
                caret.h / self.scale,
            ));
        }
        // A custom editor (backlog C17): the focused node's subtree holds
        // the `line` rows it draws, and the one carrying `caret` says
        // where the caret is — a byte offset into that line's runs, which
        // is the question `caret_rect` answers. This runs after the text
        // pass, so the places it reads are this frame's.
        let i = self.focus_index()?;
        let end = self.tree.subtree_end(i);
        let l = (i..end).find(|&l| {
            let a = self.tree.specs[l].access();
            a.role == Some(crate::access::Role::Line) && a.caret.is_some()
        })?;
        let caret = self.tree.specs[l].access().caret? as usize;
        self.text.caret_at(self.tree.keys[l], caret, false)
    }
}

/// Combined measurer handed to the layout pass: static text through the
/// shape cache, editors through the edit store (sharing one FontSystem),
/// images through the resource registry.
struct Measure<'a> {
    text: &'a mut TextSystem,
    cells: &'a mut crate::cells::CellStore,
    fonts: &'a mut cosmic_text::FontSystem,
    edit: &'a mut EditStore,
    resources: &'a Resources,
}

impl TextMeasure for Measure<'_> {
    fn intrinsic(&mut self, id: crate::tree::TextId) -> Size {
        self.text.intrinsic(id)
    }

    fn wrapped(&mut self, id: crate::tree::TextId, max_w: f32) -> Size {
        self.text.wrapped(id, max_w, self.fonts)
    }

    fn edit_intrinsic(&mut self, key: Key) -> Size {
        self.edit.intrinsic(key, self.fonts)
    }

    fn edit_wrapped(&mut self, key: Key, max_w: f32) -> Size {
        self.edit.wrapped(key, max_w, self.fonts)
    }

    fn cells_size(&mut self, id: crate::cells::CellsId) -> Size {
        self.cells.size(id, self.resources, self.fonts)
    }

    fn image_size(&mut self, id: crate::resources::ImageId) -> Size {
        self.resources
            .image(id)
            .map_or(Size::ZERO, |e| Size::new(e.width as f32, e.height as f32))
    }
}

/// What a node inherits at emission time: the frame's scale, the clip its
/// ancestors imposed (logical px), and the group opacity its own `opacity`
/// and every ancestor's multiply out to.
#[derive(Clone, Copy)]
struct Paint {
    clip: Clip,
    /// `clip`, scaled and interned: what the node's quads name.
    clip_id: ClipId,
    scale: f32,
    /// Multiplied into the alpha of every quad the node emits.
    opacity: f32,
}

/// As much of the previous frame's paint order as a departure needs to
/// keep its place (see `depart::Place`): the pass each node painted in,
/// and for each index the next node at or after it in each pass that this
/// frame still declares. Two linear passes over the previous tree, once
/// per frame that has a departure.
struct PaintOrder {
    in_float: Vec<bool>,
    /// Indexed by pass (in flow, float), then by previous-tree index; one
    /// past the end reads `NIL`.
    next_live: [Vec<u32>; 2],
}

impl PaintOrder {
    fn of(prev: &Tree, tree: &Tree) -> Self {
        let n = prev.len();
        let live: FxHashSet<Key> = tree.keys.iter().copied().collect();
        let mut in_float = vec![false; n];
        for i in 0..n {
            let parent = prev.parent[i];
            in_float[i] = prev.specs[i].layout.float.is_some()
                || (parent != NIL && in_float[parent as usize]);
        }
        let mut next_live = [vec![NIL; n + 1], vec![NIL; n + 1]];
        for j in (0..n).rev() {
            next_live[0][j] = next_live[0][j + 1];
            next_live[1][j] = next_live[1][j + 1];
            if live.contains(&prev.keys[j]) {
                next_live[in_float[j] as usize][j] = j as u32;
            }
        }
        Self {
            in_float,
            next_live,
        }
    }

    /// The place the subtree rooted at `root` of the previous frame painted
    /// in: its pass, and the node painted right after it in that pass that
    /// is still here.
    fn place(&self, prev: &Tree, root: usize) -> Place {
        let float = self.in_float[root];
        let after = self.next_live[float as usize][prev.subtree_end(root)];
        Place {
            pass: if float { Pass::Float } else { Pass::InFlow },
            before: (after != NIL).then(|| prev.keys[after as usize]),
        }
    }
}

/// One [`QuadKind::Segment`] per straight piece of a stroke: `points` are
/// relative to `origin` (the node's box, logical px) and `width` is
/// logical; everything on the quad is physical. The rect is the piece's
/// bounding box padded by half the width plus two logical px, so the
/// backend's edge ramp is never cut by the quad's own edge, and the
/// endpoints ride in `uv` (see [`Quad::segment_ends`]).
fn push_segments(
    quads: &mut Vec<Quad>,
    origin: Vec2,
    points: &[Vec2],
    width: f32,
    color: Color,
    clip_id: ClipId,
    scale: f32,
) {
    let pad = crate::line::pad(width) * scale;
    let w = width.max(0.0) * scale;
    for pair in points.windows(2) {
        let a = Vec2::new(
            (origin.x + pair[0].x) * scale,
            (origin.y + pair[0].y) * scale,
        );
        let b = Vec2::new(
            (origin.x + pair[1].x) * scale,
            (origin.y + pair[1].y) * scale,
        );
        let (x0, x1) = (a.x.min(b.x) - pad, a.x.max(b.x) + pad);
        let (y0, y1) = (a.y.min(b.y) - pad, a.y.max(b.y) + pad);
        quads.push(Quad {
            rect: Rect::new(x0, y0, x1 - x0, y1 - y0),
            color,
            border_color: Color::TRANSPARENT,
            radius: crate::display::SQUARE,
            border_w: w,
            blur: 0.0,
            kind: QuadKind::Segment,
            clip: clip_id,
            uv: Quad::segment_uv([a.x, a.y, b.x, b.y]),
        });
    }
}

/// The drop shadow behind one node, in physical pixels. The quad is the
/// shadow's own shape — the node's rect moved by `dx`/`dy` and grown by
/// `spread` — inflated by `blur` on every side, because that is how far
/// the blurred edge reaches; the backend insets by `blur` again to find
/// the shape. Radii grow with the spread so a rounded box keeps its
/// silhouette instead of sprouting corners.
fn shadow_quad(style: &crate::spec::VisualStyle, rect: Rect, clip_id: ClipId, scale: f32) -> Quad {
    let sh = style.shadow;
    let blur = sh.blur.max(0.0);
    let shape = Rect::new(
        rect.x + sh.dx - sh.spread,
        rect.y + sh.dy - sh.spread,
        (rect.w + 2.0 * sh.spread).max(0.0),
        (rect.h + 2.0 * sh.spread).max(0.0),
    );
    Quad {
        rect: Rect::new(
            (shape.x - blur) * scale,
            (shape.y - blur) * scale,
            (shape.w + 2.0 * blur) * scale,
            (shape.h + 2.0 * blur) * scale,
        ),
        color: sh.color,
        border_color: Color::TRANSPARENT,
        radius: style.radius.map(|r| (r + sh.spread).max(0.0) * scale),
        border_w: 0.0,
        blur: blur * scale,
        kind: QuadKind::Shadow,
        clip: clip_id,
        uv: [0; 4],
    }
}

/// Multiplies a group opacity into a run of quads. Alpha only: every quad
/// kind reads `color.a` as its coverage, so one multiply fades a
/// background, a border, a glyph and an image alike.
fn fade(quads: &mut [Quad], opacity: f32) {
    for q in quads {
        q.color.a *= opacity;
        q.border_color.a *= opacity;
    }
}

fn scrollbar_quad(bar: Rect, scale: f32, clip_id: ClipId, color: Color) -> Quad {
    Quad {
        rect: bar.scaled(scale),
        color,
        border_color: Color::TRANSPARENT,
        radius: [bar.w.min(bar.h) / 2.0 * scale; 4],
        border_w: 0.0,
        blur: 0.0,
        kind: QuadKind::Solid,
        clip: clip_id,
        uv: [0; 4],
    }
}

/// One `fragment` node's quad, live or ghost.
///
/// The handle is resolved first: a removed or foreign one draws nothing,
/// which is the documented fallback for every resource kind, and the
/// lookup is what records the `foreign-resource` warning. The quad carries
/// the node's own rect, radii and clip — a fragment rounds and clips like
/// a solid — and an opaque white `color`, whose alpha the group-opacity
/// pass then multiplies into; the shader reads that alpha and nothing else
/// of the colour, because a fragment returns its own.
#[allow(clippy::too_many_arguments)]
fn push_fragment(
    quads: &mut Vec<Quad>,
    fragments: &mut Vec<crate::display::FragmentDraw>,
    sources: &mut Vec<std::sync::Arc<str>>,
    resources: &crate::resources::Resources,
    draw: crate::display::FragmentDraw,
    rect: Rect,
    radius: [f32; 4],
    clip_id: ClipId,
) {
    let Some(source) = resources.fragment(draw.id) else {
        return;
    };
    let index = fragments.len() as u32;
    fragments.push(draw);
    sources.push(source.clone());
    quads.push(Quad {
        rect,
        // `rgb` is unused on this kind; `a` is the group opacity, which
        // the fade pass multiplies in after the node's quads are pushed.
        color: Color::WHITE,
        border_color: Color::TRANSPARENT,
        radius,
        border_w: 0.0,
        blur: 0.0,
        kind: QuadKind::Fragment,
        clip: clip_id,
        uv: [index, 0, 0, 0],
    });
}
