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
/// drawn this far outside the focused node, this thick, in this colour,
/// when focus is keyboard-visible and the node styles nothing itself.
const FOCUS_RING_GAP: f32 = 2.0;
const FOCUS_RING_W: f32 = 2.0;
const FOCUS_RING: Color = Color {
    r: 0x7f as f32 / 255.0,
    g: 0x9c as f32 / 255.0,
    b: 0xf5 as f32 / 255.0,
    a: 1.0,
};

impl Core {
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
                .push(shadow_quad(&style, rect, clip, scale));
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
                uv: [0; 4],
                clip: clip_px.rect,
                clip_radius: clip_px.radius,
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
                let sess = &mut *self.session.state();
                self.text.emit(
                    tid,
                    self.tree.pos[i],
                    self.tree.size[i],
                    clip_px,
                    &mut sess.fonts,
                    &mut self.atlas,
                    &mut self.display.quads,
                );
            }
            NodeContent::Edit(key) => {
                let pad = spec.layout.padding;
                let content_origin = Vec2::new(rect.x + pad.l, rect.y + pad.t);
                if interactive {
                    hits.push(HitRegion {
                        key,
                        origin: self.tree.origins[i],
                        rect,
                        clip: clip.rect,
                        payload: None,
                        drag: None,
                        parent_rect: rect,
                        edit_origin: Some(content_origin),
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
                    (content_origin.x * scale).round(),
                    (content_origin.y * scale).round(),
                );
                let sess = &mut *self.session.state();
                self.edit.emit(
                    key,
                    origin_phys,
                    focused,
                    clip_px,
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
                        uv: [slot.x, slot.y, slot.w, slot.h],
                        clip: clip_px.rect,
                        clip_radius: clip_px.radius,
                    });
                }
            }
            NodeContent::Line(id) => {
                let (run, points) = self.lines.run(id);
                push_segments(
                    &mut self.display.quads,
                    self.tree.pos[i],
                    points,
                    run.width,
                    style.bg,
                    clip_px,
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
                fonts: &mut sess.fonts,
                edit: &mut self.edit,
                resources: &sess.resources,
            };
            layout::compute(
                &mut self.tree,
                &mut measure,
                &mut self.scroll,
                self.viewport,
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
        // The ring exists now: laid out, and scoped to the modal if there
        // is one. A step asked for during the build lands here, so it wins
        // over both the modal's own focus move and a same-frame
        // `set_focus`. Like a real Tab press, the scroll it triggers shows
        // on the next frame.
        if let Some(forward) = self.pending_focus_step.take() {
            self.focus_next(forward);
        }
    }

    /// The frame's second half: the laid-out tree into the display list,
    /// in paint order — in-flow content, then floating subtrees, each with
    /// the departed subtrees that were painted among them, then the
    /// scrollbars and the focus ring on top — and the hit and scroll
    /// regions the next input is tested against.
    fn emit_frame(&mut self) {
        let scale = self.scale;
        let mut hits: Vec<HitRegion> = self.interaction.take_hit_buffer();
        let mut scroll_regions: Vec<ScrollRegion> = Vec::new();
        self.display.viewport = Size::new(self.viewport.w * scale, self.viewport.h * scale);
        self.display.scale = scale;

        // configure_root can also introduce a clipper, or a fade.
        let root_clips = !self.tree.is_empty() && self.tree.specs[0].layout.clips();
        let any_clip = self.any_clip || root_clips;
        let any_rounded_clip = self.any_rounded_clip
            || (root_clips && self.tree.specs[0].style.radius != crate::display::SQUARE);
        let any_opacity =
            self.any_opacity || (!self.tree.is_empty() && self.tree.specs[0].style.opacity < 1.0);
        let any_float = self.any_float;

        // inherited clip per node (logical): ancestors only, not the node
        // itself. Only materialized when something actually clips.
        self.clips.clear();
        if any_clip {
            self.clips.resize(self.tree.len(), Clip::NONE);
        }
        self.opacity.clear();
        if any_opacity {
            self.opacity.resize(self.tree.len(), 1.0);
        }
        self.in_float.clear();
        if any_float {
            self.in_float.resize(self.tree.len(), false);
        }

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
            let floats_here = self.tree.specs[i].layout.float.is_some();
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
            let clip = if !any_clip {
                Clip::NONE
            } else {
                // Floating nodes escape ancestor clips.
                let clip = if parent == NIL || floats_here {
                    Clip::NONE
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
                        self.clips[p].intersect(box_rect, box_radius)
                    } else {
                        self.clips[p]
                    }
                };
                self.clips[i] = clip;
                clip
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
                continue;
            }
            let paint = Paint {
                clip,
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
                let opacity = if any_opacity { self.opacity[i] } else { 1.0 };
                let visible = rect.intersect(&clip.rect);
                if visible.w <= 0.0 || visible.h <= 0.0 {
                    continue;
                }
                let paint = Paint {
                    clip,
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
            let clip = self.clips[i].scaled(scale);
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
                let mut bar = scrollbar_quad(thumb, scale, clip, active);
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
                let mut bar = scrollbar_quad(thumb, scale, clip, active);
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
            let spec = &self.prev_tree.specs[i];
            if spec.anim().exit.is_some() && spec.transition.is_some() {
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
        // The previous frame's paint order, built on the first departure:
        // a frame with one is a frame that changed shape and paid for a
        // layout, and the frames that did not never get here.
        let mut order: Option<PaintOrder> = None;
        // In tree order, so a departing subtree swallows the exits nested
        // inside it rather than drawing them a second time on top.
        let mut swallowed_until = 0usize;
        for i in candidates {
            if i < swallowed_until || live.contains(&self.prev_tree.keys[i]) {
                continue;
            }
            swallowed_until = self.prev_tree.subtree_end(i);
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
            );
        }
        if let Some(key) = self.depart.refused.take() {
            self.diag.raise(Warning {
                code: crate::diag::EXIT_BUDGET,
                key,
                message: format!(
                    "more than {} nodes are departing at once, so this subtree was dropped \
                     instead of animating out; `exit` is per node, and a list that drops \
                     many rows at once wants it on the list, not on every row",
                    crate::depart::MAX_NODES
                ),
            });
        }
    }

    /// One departing subtree's quads: frozen rects moved by however far
    /// its `exit` has got, outside every clip (its ancestors may be gone).
    /// A smaller `emit_node`: the parts a picture has (shadow, background,
    /// border, its content) and none of the parts a node has — no hit
    /// region, no scroll region, no access row.
    fn emit_ghost(&mut self, g: &Ghost, play: &Playback, scale: f32) {
        self.ghost_opacity.clear();
        self.ghost_opacity.resize(g.nodes.len(), 1.0);
        for (i, node) in g.nodes.iter().enumerate() {
            let mut rect = Rect::new(
                node.rect.x + play.offset.x,
                node.rect.y + play.offset.y,
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
            let first_quad = self.display.quads.len();
            if style.shadow.is_visible() {
                self.display
                    .quads
                    .push(shadow_quad(&style, rect, Clip::NONE, scale));
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
                    uv: [0; 4],
                    clip: NO_CLIP.scaled(scale),
                    clip_radius: crate::display::SQUARE,
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
                            Clip::NONE.scaled(scale),
                            &mut sess.fonts,
                            &mut self.atlas,
                            &mut self.display.quads,
                        );
                    }
                }
                GhostContent::Edit(key) => {
                    let pad = node.spec.layout.padding;
                    let origin = Vec2::new(
                        ((rect.x + pad.l) * scale).round(),
                        ((rect.y + pad.t) * scale).round(),
                    );
                    // Never focused: the departing subtree gave the
                    // keyboard up the frame it stopped being declared.
                    let sess = &mut *self.session.state();
                    self.edit.emit(
                        key,
                        origin,
                        false,
                        Clip::NONE.scaled(scale),
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
                            uv: [slot.x, slot.y, slot.w, slot.h],
                            clip: NO_CLIP.scaled(scale),
                            clip_radius: crate::display::SQUARE,
                        });
                    }
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
                        Clip::NONE.scaled(scale),
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
            let d = Vec2::new(v[0] - target.x, v[1] - target.y);
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
        let visible = rect.intersect(&clip.rect);
        if visible.w <= 0.0 || visible.h <= 0.0 {
            return;
        }
        let mut ring = FOCUS_RING;
        ring.a *= self.opacity.get(i).copied().unwrap_or(1.0);
        self.display.quads.push(Quad {
            rect: rect.scaled(scale),
            color: Color::TRANSPARENT,
            border_color: ring,
            radius: spec.style.radius.map(|r| (r + FOCUS_RING_GAP) * scale),
            border_w: FOCUS_RING_W * scale,
            blur: 0.0,
            kind: QuadKind::Solid,
            uv: [0; 4],
            clip: clip.rect.scaled(scale),
            clip_radius: clip.radius.map(|r| r * scale),
        });
    }

    /// See the `ime_rect` field. None when no editor is focused.
    pub fn ime_rect(&self) -> Option<Rect> {
        self.ime_rect
    }

    fn focused_caret_rect(&mut self) -> Option<Rect> {
        let key = self.edit.focused()?;
        let i = (0..self.tree.len()).find(|&i| self.tree.content[i] == NodeContent::Edit(key))?;
        let caret = self.edit_with_fonts(|edit, fs| edit.caret_rect(key, fs))?;
        let pad = self.tree.specs[i].layout.padding;
        Some(Rect::new(
            self.tree.pos[i].x + pad.l + caret.x / self.scale,
            self.tree.pos[i].y + pad.t + caret.y / self.scale,
            caret.w / self.scale,
            caret.h / self.scale,
        ))
    }
}

/// Combined measurer handed to the layout pass: static text through the
/// shape cache, editors through the edit store (sharing one FontSystem),
/// images through the resource registry.
struct Measure<'a> {
    text: &'a mut TextSystem,
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
    clip: Clip,
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
            uv: Quad::segment_uv([a.x, a.y, b.x, b.y]),
            clip: clip.rect,
            clip_radius: clip.radius,
        });
    }
}

/// The drop shadow behind one node, in physical pixels. The quad is the
/// shadow's own shape — the node's rect moved by `dx`/`dy` and grown by
/// `spread` — inflated by `blur` on every side, because that is how far
/// the blurred edge reaches; the backend insets by `blur` again to find
/// the shape. Radii grow with the spread so a rounded box keeps its
/// silhouette instead of sprouting corners.
fn shadow_quad(style: &crate::spec::VisualStyle, rect: Rect, clip: Clip, scale: f32) -> Quad {
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
        uv: [0; 4],
        clip: clip.rect.scaled(scale),
        clip_radius: clip.radius.map(|r| r * scale),
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

fn scrollbar_quad(bar: Rect, scale: f32, clip: Clip, active: bool) -> Quad {
    Quad {
        rect: bar.scaled(scale),
        color: Color::rgba(1.0, 1.0, 1.0, if active { 0.4 } else { 0.18 }),
        border_color: Color::TRANSPARENT,
        radius: [bar.w.min(bar.h) / 2.0 * scale; 4],
        border_w: 0.0,
        blur: 0.0,
        kind: QuadKind::Solid,
        uv: [0; 4],
        clip: clip.rect,
        clip_radius: clip.radius,
    }
}
