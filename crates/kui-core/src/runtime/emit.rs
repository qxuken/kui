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
/// An `auto` bar (`ScrollbarMode::Auto`): how long it stays after the
/// scroll state last changed, then how long it takes to fade. Seconds of
/// the driver's clock.
const SCROLLBAR_HOLD: f64 = 1.0;
const SCROLLBAR_FADE: f64 = 0.25;
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

/// The disjoint borrows one box's paint needs, split off the core field by
/// field — a macro rather than a method so the tree, the stroke list and a
/// ghost's points can stay borrowed beside them.
macro_rules! painter {
    ($core:expr) => {
        Painter {
            display: &mut $core.display,
            text: &mut $core.text,
            edit: &mut $core.edit,
            cells: &mut $core.cells,
            atlas: &mut $core.atlas,
            session: &$core.session,
        }
    };
}

impl Core {
    /// The scrollbar thumb's colour at rest and while hovered or dragged:
    /// the node's own where it declared one, else the theme's. A wash over
    /// whatever it sits on rather than a fill, which is why the theme's
    /// are two translucent colours and not one with an alpha applied.
    fn thumb_color(&self, bar: &crate::spec::Scrollbar, active: bool) -> Color {
        let t = self.theme();
        if active {
            bar.active_color.unwrap_or(t.scrollbar_active)
        } else {
            bar.color.unwrap_or(t.scrollbar)
        }
    }

    /// Node `i`'s hit region, for a node that tracks the pointer. Out of
    /// line: a box that takes no input — most of a frame — does not carry
    /// the context-menu walk, the slider's track or the shape in
    /// `emit_node`'s saved registers and stack frame (backlog C48).
    #[inline(never)]
    fn push_hit(
        &mut self,
        i: usize,
        rect: Rect,
        clip: Rect,
        drop: Option<crate::input::DropOwner>,
        hits: &mut Vec<HitRegion>,
    ) {
        let spec = &self.tree.specs[i];
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
        // The menu this region opens is its own or an ancestor's; the
        // walk is skipped on a frame where no node offers one (T1).
        let context_menu = if self.tree.any_context_menu {
            self.enclosing_menu(i).map(|j| crate::input::MenuOwner {
                key: self.tree.keys[j],
                origin: self.tree.origins[j],
                tag: self.tree.specs[j]
                    .events()
                    .on_context_menu
                    .clone()
                    .expect("enclosing_menu returns a node that offers one"),
            })
        } else {
            None
        };
        // A slider that asked for its changes reads the pointer along
        // its content box (ADR 0034, decision 4); a disabled one, or a
        // range that is not one, reads nothing.
        let slider = match spec.events().on_change.as_ref() {
            Some(tag) if live && spec.access().role == Some(crate::access::Role::Slider) => {
                crate::slider::SliderRange::of(spec.access()).map(|range| {
                    Box::new(crate::slider::SliderTrack::new(
                        rect,
                        spec.layout.padding,
                        spec.layout.dir == crate::spec::Dir::Column,
                        range,
                        tag.clone(),
                    ))
                })
            }
            _ => None,
        };
        // The shape past the rect (ADR 0026): a stroke's pieces, a
        // fill's outline, a rounded box's corners; a plain box none.
        let shape = match self.tree.content[i] {
            NodeContent::Line(id) => {
                let (run, points) = self.lines.run(id);
                self.hit_shapes.segments(points, run.width)
            }
            NodeContent::Polygon(id) => {
                let draw = self.fragments.get(id);
                let mut pts = [Vec2::ZERO; crate::fragment::POLYGON_MAX_POINTS];
                for (k, p) in pts.iter_mut().enumerate() {
                    *p = Vec2::new(draw.params[k * 2] * rect.w, draw.params[k * 2 + 1] * rect.h);
                }
                self.hit_shapes.polygon(&pts)
            }
            _ if spec.style.radius != crate::display::SQUARE => {
                crate::input::HitShape::Rounded(spec.style.radius)
            }
            _ => crate::input::HitShape::Rect,
        };
        hits.push(HitRegion {
            key: self.tree.keys[i],
            origin: self.tree.origins[i],
            rect,
            clip,
            shape,
            payload: spec.events().on_click.clone().filter(|_| live),
            drag: spec.events().on_drag.clone().filter(|_| live),
            parent_rect,
            key_sink: spec.events().on_key.clone().filter(|_| live),
            key_up: spec.events().key_up,
            context_menu,
            drop,
            focusable: crate::access::focusable(&self.tree, i),
            edit_origin: None,
            select_scope: self.scope_of(i).filter(|_| live),
            window: spec.window,
            hover: spec.events().on_hover.clone(),
            group: spec.interact().hover_group,
            click_sound: spec.interact().click_sound.filter(|_| live),
            hover_sound: spec.interact().hover_sound,
            cursor: spec.cursor,
            slider,
        });
    }

    /// An editor's own hit region, whose origin is where its glyphs sit
    /// (shifted by what a field is scrolled). Out of line for the reason
    /// `push_hit` is (backlog C48).
    #[inline(never)]
    #[allow(clippy::too_many_arguments)]
    fn push_edit_hit(
        &mut self,
        i: usize,
        key: Key,
        rect: Rect,
        clip: Rect,
        scale: f32,
        drop: Option<crate::input::DropOwner>,
        hits: &mut Vec<HitRegion>,
    ) {
        let spec = &self.tree.specs[i];
        let pad = spec.layout.padding;
        let content_origin = Vec2::new(rect.x + pad.l, rect.y + pad.t);
        let inner_w = (rect.w - pad.x()).max(0.0);
        // A single-line field scrolls its own text (F41). Resolved
        // here, where the box is known, and read back by the hit
        // region and the access runs so all three agree on where the
        // glyphs are; the painter reads the same stored offset.
        let offset = {
            let sess = &mut *self.session.state();
            self.edit.line_offset(key, inner_w * scale, &mut sess.fonts)
        };
        hits.push(HitRegion {
            key,
            origin: self.tree.origins[i],
            rect,
            clip,
            // A field's corners round its hit too (ADR 0026).
            shape: if spec.style.radius != crate::display::SQUARE {
                crate::input::HitShape::Rounded(spec.style.radius)
            } else {
                crate::input::HitShape::Rect
            },
            payload: None,
            drag: None,
            parent_rect: rect,
            // Shifted by what the field is scrolled: a click lands on
            // the character under the pointer.
            edit_origin: Some(Vec2::new(
                content_origin.x - offset / scale,
                content_origin.y,
            )),
            // An editor is its own selection scope: a press in it
            // places a caret and drags a selection through the
            // editor's own path, not the scope's.
            select_scope: None,
            key_sink: None,
            key_up: false,
            context_menu: None,
            // A field inside a zone is the zone's: files dropped on
            // it land there.
            drop,
            focusable: !spec.disabled,
            window: None,
            hover: None,
            group: None,
            click_sound: None,
            hover_sound: None,
            // The editor's own node carries any override.
            cursor: spec.cursor,
            slider: None,
        });
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
        let Paint { clip, scale, .. } = paint;
        // Behind a modal a node still draws, and stops taking input.
        let interactive = self.interactive(i);
        let spec = &self.tree.specs[i];
        let style = spec.style;
        // The zone this node's regions belong to — its own `on_drop` or
        // an ancestor's — by the context menu's walk (ADR 0031, decision
        // 2); skipped wholesale on a frame with no zone.
        let drop = if self.tree.any_drop && interactive {
            self.enclosing_drop(i).map(|j| crate::input::DropOwner {
                key: self.tree.keys[j],
                origin: self.tree.origins[j],
                tag: self.tree.specs[j]
                    .events()
                    .on_drop
                    .clone()
                    .expect("enclosing_drop returns a node that declares one"),
            })
        } else {
            None
        };
        // A stroke emits no hit region: it takes no input (ADR 0010,
        // decision 7).
        if spec.hover_tracked() && interactive {
            self.push_hit(i, rect, clip.rect, drop.clone(), hits);
        }
        let spec = &self.tree.specs[i];
        // An `on_scroll` node takes the wheel the way a container does —
        // one list, one paint-order rule (ADR 0029, decision 4).
        let handler = self.tree.any_scroll_handler && spec.events().on_scroll.is_some();
        if spec.layout.scroll_x || spec.layout.scroll_y || handler {
            // A container behind a modal keeps its scrollbar drawn and
            // refuses the wheel and the thumb.
            scroll_regions.push(ScrollRegion {
                key: self.tree.keys[i],
                node: i as u32,
                rect,
                clip: clip.rect,
                inert: !interactive,
                handler,
            });
        }
        if let NodeContent::Edit(key) = self.tree.content[i]
            && interactive
        {
            self.push_edit_hit(i, key, rect, clip.rect, scale, drop, hits);
        }
        // The content, resolved to what the painter needs — a text node's
        // selection and its place, an editor's focus — then painted by the
        // one step a ghost also paints through.
        let leaf = match self.tree.content[i] {
            NodeContent::Text(tid) => {
                // The keys above it, nearest first, so a query by the
                // `line` row (or a wrapper) finds the runs inside it.
                let ancestry = self.text_ancestors(i);
                self.text.place(
                    self.tree.keys[i],
                    &ancestry,
                    tid,
                    self.tree.pos[i],
                    self.scope_of(i),
                    true,
                );
                Leaf::Text {
                    tid,
                    sel: self.sel_range(i, tid),
                }
            }
            NodeContent::Cells(cid) => Leaf::Cells {
                cid,
                at: self.cells_origin(i),
                // The window's selection, when it is in this very grid.
                sel: self
                    .cell_selection
                    .as_ref()
                    .filter(|s| s.node == self.tree.keys[i] && !s.is_empty()),
                tint: self.theme.selection,
            },
            NodeContent::Edit(key) => Leaf::Edit {
                key,
                focused: self.edit.focused() == Some(key),
                pad: self.tree.specs[i].layout.padding,
            },
            NodeContent::Image(id, opts) => Leaf::Image(id, opts),
            NodeContent::Fragment(id) => Leaf::Fragment(self.fragments.get(id)),
            NodeContent::Polygon(id) => Leaf::Polygon(self.fragments.get(id)),
            NodeContent::Line(id) => {
                let (run, points) = self.lines.run(id);
                Leaf::Line {
                    points,
                    width: run.width,
                }
            }
            NodeContent::Container => Leaf::Container,
        };
        painter!(self).paint_box(rect, &style, &paint, leaf);
        // A table's grid rules, with its box and under its cells. Out of
        // `emit_node`: a rare path kept off its codegen (C48).
        if self.tree.any_table
            && let Some(c) = self.tree.specs[i].interact().rules
            && self.tree.specs[i].layout.is_table()
        {
            self.emit_rules(i, rect, c, &paint);
        }
    }

    /// The `rules` of table `i` (backlog DX21): a line down the middle of
    /// each gap between the columns of its widest row, from its first
    /// row's top to its last row's bottom, and one across the middle of
    /// each gap between its rows, the content box wide. On whole pixels,
    /// so a 1 px rule is one crisp pixel line at any scale.
    #[cold]
    #[inline(never)]
    fn emit_rules(&mut self, i: usize, rect: Rect, color: Color, paint: &Paint) {
        let spec = &self.tree.specs[i];
        let w = match spec.interact().rule_w {
            w if w > 0.0 => w,
            _ => 1.0,
        };
        let pad = spec.layout.padding;
        let row_of = |j: usize| Rect::from_pos_size(self.tree.pos[j], self.tree.size[j]);
        // The table's rows: its in-flow row children.
        let mut rows: Vec<usize> = Vec::new();
        let mut c = self.tree.first_child[i];
        while c != NIL {
            let j = c as usize;
            if self.tree.specs[j].layout.dir == crate::spec::Dir::Row
                && self.tree.specs[j].layout.float.is_none()
            {
                rows.push(j);
            }
            c = self.tree.next_sibling[j];
        }
        let (Some(&first), Some(&last)) = (rows.first(), rows.last()) else {
            return;
        };
        let mut lines: Vec<Rect> = Vec::new();
        for pair in rows.windows(2) {
            let (a, b) = (row_of(pair[0]), row_of(pair[1]));
            let y = (a.y + a.h + b.y) / 2.0;
            let x = rect.x + pad.l;
            lines.push(Rect::new(x, y - w / 2.0, rect.w - pad.l - pad.r, w));
        }
        // The columns: the in-flow cells of the row with the most of them.
        let cells_of = |row: usize| {
            let mut out = Vec::new();
            let mut c = self.tree.first_child[row];
            while c != NIL {
                let j = c as usize;
                if self.tree.specs[j].layout.float.is_none() {
                    out.push(row_of(j));
                }
                c = self.tree.next_sibling[j];
            }
            out
        };
        let widest = rows
            .iter()
            .map(|&r| cells_of(r))
            .max_by_key(|cells| cells.len())
            .unwrap_or_default();
        let (top, bottom) = (row_of(first).y, {
            let r = row_of(last);
            r.y + r.h
        });
        for pair in widest.windows(2) {
            let x = (pair[0].x + pair[0].w + pair[1].x) / 2.0;
            lines.push(Rect::new(x - w / 2.0, top, w, bottom - top));
        }
        let color = Color {
            a: color.a * paint.opacity,
            ..color
        };
        for line in lines {
            self.display.quads.push(Quad {
                rect: line.scaled(paint.scale).on_pixels(),
                color,
                border_color: Color::TRANSPARENT,
                radius: [0.0; 4],
                border_w: 0.0,
                blur: 0.0,
                kind: QuadKind::Solid,
                clip: paint.clip_id,
                uv: [0; 4],
            });
        }
    }

    /// Runs layout and emission into `output()`, and installs this frame's
    /// hit and scroll regions for input handling. The last step of
    /// `Ui::finish`, which runs the extension fills, the devtools panel
    /// and the open menu first — and crate-private for that reason
    /// (backlog AR37): a host that called this directly got a frame where
    /// `open_menu` drew nothing and `KUI_DEVTOOLS` did nothing, with no
    /// warning. A driver with a bare `Core` finishes through
    /// `Ui::wrap(core).finish()`.
    pub(crate) fn finish_frame(&mut self) {
        self.layout_frame();
        self.emit_frame();
        self.building = false;
        // A focus the view moved, or a focused node the frame declared
        // `on_focus` on or dropped (backlog DX18).
        let mut out = std::mem::take(&mut self.pending);
        self.report_focus("program", &mut out);
        self.pending = out;
        self.snapshot_nodes();
        self.devtools_after_frame();
        // Between frames the host is who talks to the core: a driver that
        // tagged the last nodes with an extension's origin by hand (rather
        // than through `fill`, which restores it) must not leave its
        // `set_tokens` landing in that extension's table (ADR 0027).
        self.origin = crate::tree::OriginId::HOST;
    }

    /// The frame's first half: layout, then everything that resolves
    /// against it before a quad is emitted — the caret and reveal nudges,
    /// the `layout` events and the diagnostics, the declared window set,
    /// the modal scope and the Tab step a view asked for.
    fn layout_frame(&mut self) {
        // Tolerate unclosed containers (an FFI caller may have bailed early).
        self.stack.truncate(1);
        self.counters.truncate(1);

        self.tree.host_area = self.dt_area;
        if !self.pending_scroll_labels.is_empty() {
            self.resolve_scroll_labels();
        }
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
        // A container a view sliced its rows by (`scroll_geometry`) that
        // came out otherwise — taller, scrolled elsewhere — owes a frame
        // built against this layout, or the slice stays a frame behind
        // until the next event (`ScrollStore::resliced`).
        if self.scroll.take_resliced() {
            self.frame_requested = true;
        }
        if self.tree.any_slide {
            self.ease_positions();
        }
        // Positions are final: report the rects views asked about, and
        // look for the misconfigurations that would otherwise fail silently.
        if self.tree.any_layout {
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
        self.modal = if self.tree.any_modal {
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
            // The row itself is built, on runs no end's key names: a Select
            // All's placeholder end (`select_all_in` puts it on the scope
            // with the row's index and `ROW_END`) after the list scrolled
            // that row into the built window. It lands in that row's own
            // runs — the end of its last one, or the byte into its first —
            // rather than at the boundary an unbuilt row would take.
            let mut in_row = built.iter().filter(|(_, r, _)| *r == Some(row));
            if let Some(first_run) = in_row.next() {
                return Some(if end.byte >= crate::select::ROW_END {
                    let last_run = in_row.next_back().unwrap_or(first_run);
                    (last_run.0, last_run.2)
                } else {
                    (first_run.0, end.byte.min(first_run.2))
                });
            }
            // Against the built runs that *carry* a row, not the first and
            // last of everything built: a scope can hold plain labels
            // beside virtual rows — a header, a footer — and a label says
            // nothing about where a row sits in the data. Comparing
            // against one puts an end below the list at the top of it.
            let hi = built.iter().rev().find_map(|(_, r, _)| *r)?;
            if crate::select::unbuilt_row_is_after(row, Some(hi)) {
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
    /// The keys above text node `i`, nearest first, as many as a place
    /// remembers; how many there are; and the depth of the nearest
    /// `role="none"` ancestor among them, which a query from above it
    /// does not reach (backlog AR30). A `line` row further up than the
    /// place can remember raises `text-beyond-line`, once per text.
    fn text_ancestors(&mut self, i: usize) -> crate::text::Ancestry {
        use crate::access::Role;
        let mut ancestors = [Key::ROOT; crate::text::PLACE_ANCESTORS];
        let mut depth = 0;
        let mut none_at = None;
        let mut p = self.tree.parent[i];
        while p != NIL && depth < ancestors.len() {
            let j = p as usize;
            ancestors[depth] = self.tree.keys[j];
            if none_at.is_none() && self.tree.specs[j].access().role == Some(Role::None) {
                none_at = Some(depth);
            }
            depth += 1;
            p = self.tree.parent[j];
        }
        // Past the reach: a `line` row still above is one the text cannot
        // be found from, and the app should hear it.
        if p != NIL && self.tree.any_line {
            let mut q = p;
            while q != NIL {
                let j = q as usize;
                if self.tree.specs[j].access().role == Some(Role::Line) {
                    self.diag.raise(crate::diag::text_beyond_line(
                        self.tree.keys[i],
                        self.tree.keys[j],
                        crate::text::PLACE_ANCESTORS,
                    ));
                    break;
                }
                q = self.tree.parent[j];
            }
        }
        crate::text::Ancestry {
            keys: ancestors,
            depth,
            none_at,
        }
    }

    /// The frame's second half: the laid-out tree into the display list,
    /// in paint order — the in-flow layer, then one layer per floating
    /// subtree in the order they opened, each with the departed subtrees
    /// that were painted among it and its own chrome (scrollbars, the
    /// ring) at its end (ADR 0023) — and the hit and scroll regions the
    /// next input is tested against, in the same order.
    fn emit_frame(&mut self) {
        let scale = self.scale;
        let mut hits: Vec<HitRegion> = self.interaction.take_hit_buffer();
        self.hit_shapes = self.interaction.take_shape_buffer();
        let mut scroll_regions: Vec<ScrollRegion> = Vec::new();
        self.display.viewport = Size::new(self.viewport.w * scale, self.viewport.h * scale);
        self.display.scale = scale;
        self.display.time = self.anim.time().unwrap_or(0.0) as f32;

        // Read once each: what the frame declared, noted by `Tree::push`
        // (and by `configure_root`, whose spec replaces the root's).
        let any_clip = self.tree.any_clip;
        let any_rounded_clip = self.tree.any_rounded_clip;
        let any_opacity = self.tree.any_opacity;
        let any_float = self.tree.any_float;
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
        self.float_root.clear();
        if any_float {
            self.float_root.resize(self.tree.len(), NIL);
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
        // paint order; parents precede children). `float_root[i]` is the
        // nearest floating ancestor-or-self, `NIL` in flow: the layer a
        // node paints in (ADR 0023, decision 1). Every float root goes into
        // `roots`, in tree order, for the stack to sort.
        let mut roots: Vec<u32> = Vec::new();
        for i in 0..self.tree.len() {
            let parent = self.tree.parent[i];
            let floats_here = any_float && self.tree.specs[i].layout.float.is_some();
            if any_float {
                self.float_root[i] = if floats_here {
                    roots.push(i as u32);
                    i as u32
                } else if parent != NIL {
                    self.float_root[parent as usize]
                } else {
                    NIL
                };
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
            // A parent-anchored float that declared `clip` belongs to the
            // parent's content as a child does, so the parent's clip holds
            // it: a node on a `clip` canvas panned past the canvas's edge
            // is cut there (F90), and a graph beside a scrolled list at
            // the list's edge like the rows it draws over, since the core
            // sets the bit on every stroke and polygon (F78, ADR 0010
            // decision 5). Any other float (a tooltip, a menu, a stroke
            // anchored to the viewport) escapes. Only the clip is the
            // parent's: the node still paints in its float layer.
            let drawn_in_parent = floats_here
                && parent != NIL
                && self.tree.specs[i]
                    .layout
                    .float
                    .is_some_and(|f| f.clipped_by_parent());
            let (clip, clip_id) = if !any_clip {
                (Clip::NONE, no_clip)
            } else {
                // Floating nodes escape ancestor clips.
                let (clip, id) = if parent == NIL || (floats_here && !drawn_in_parent) {
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
            if any_float && self.float_root[i] != NIL {
                continue; // deferred to its layer, in the float pass
            }
            // A departing subtree painted just under this node last time
            // goes first, so it stays under it.
            if any_ghost && replay.may_precede(self.tree.keys[i]) {
                let key = self.tree.keys[i];
                replay.paint(At::UnderInFlow(key), |g, play| {
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
                    let ancestry = self.text_ancestors(i);
                    self.text.place(
                        self.tree.keys[i],
                        &ancestry,
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
            // layer, still under its chrome and under every float.
            replay.paint(At::InFlowEnd, |g, play| self.emit_ghost(g, play, scale));
        }

        // The in-flow layer's chrome — its scrollers' bars and, if the
        // focused node is in flow, the ring — above its content and under
        // every float (ADR 0023, decision 2).
        let mut scrollbars: Vec<ScrollbarRegion> = Vec::new();
        let mut chrome_from = 0usize;
        self.emit_layer_chrome(
            NIL,
            &scroll_regions[chrome_from..],
            &mut scrollbars,
            hits.len(),
            scale,
        );
        chrome_from = scroll_regions.len();

        // Pass 2: the float layers, bottom to top in the order they opened
        // (ADR 0023, decision 3); each one's chrome at its end. Their hit
        // regions land in the same order, so `hit_at` reads the stack.
        if any_float {
            let order = self.stack_floats(&roots);
            for &r in &order {
                let root = r as usize;
                let root_key = self.tree.keys[root];
                if any_ghost && replay.may_precede(root_key) {
                    // A departed float that was under this one stays under
                    // it: a whole layer, painted before this layer starts.
                    replay.paint(At::UnderLayer(root_key), |g, play| {
                        self.emit_ghost(g, play, scale)
                    });
                }
                let end = self.tree.subtree_end(root);
                for i in root..end {
                    if self.float_root[i] != r {
                        continue; // a nested float: its own layer, later
                    }
                    if any_ghost && replay.may_precede(self.tree.keys[i]) {
                        let key = self.tree.keys[i];
                        replay.paint(At::UnderInLayer(key), |g, play| {
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
                if any_ghost {
                    replay.paint(At::LayerEnd(root_key), |g, play| {
                        self.emit_ghost(g, play, scale)
                    });
                }
                // A layer with no scroller and no ring to draw has no
                // chrome: the common float is a leaf, and ten thousand of
                // them (a `line` each) pay a call apiece otherwise (C29).
                if scroll_regions.len() > chrome_from || self.focus_visible {
                    self.emit_layer_chrome(
                        r,
                        &scroll_regions[chrome_from..],
                        &mut scrollbars,
                        hits.len(),
                        scale,
                    );
                    chrome_from = scroll_regions.len();
                }
            }
            self.check_layers_over_modal(&order);
        } else if !self.float_stack.is_empty() {
            // No floats this frame: every layer closed.
            self.float_stack.clear();
        }

        if any_ghost {
            // A ghost whose whole layer is gone: on top, the only place
            // left that is under nothing it was under.
            replay.paint(At::Top, |g, play| self.emit_ghost(g, play, scale));
        }
        if took_ghosts {
            self.depart.end_replay(replay);
        }

        // Rounded span backgrounds, joined into one shape with the ones
        // they meet now that every text is painted (backlog F101).
        let joins = self.text.take_joins();
        if !joins.is_empty() {
            let sess = &mut *self.session.state();
            let id = sess.resources.add_fragment(crate::fragment::JOIN);
            if let Some(source) = sess.resources.fragment(id).cloned() {
                crate::join::shape(&mut self.display, &joins, id, &source, scale);
            }
        }

        let shapes = std::mem::take(&mut self.hit_shapes);
        self.interaction.set_hits_shaped(hits, shapes);
        // A button held on a node this frame no longer declares has no one
        // to hear its release (backlog F104).
        let tree = &self.tree;
        self.interaction
            .drop_gone_buttons(|key| tree.index_of(key).is_some());
        // A new frame can move a hover-sound node under a still cursor.
        self.flush_sound_requests();
        // Against this window's mounts only (AR7): a popup or a second
        // window finishing a frame with no `<audio>` in it says nothing
        // about the main window's loop.
        let window = self.env.window.id;
        self.session.state().audio.reconcile(window);
        self.interaction.scroll_regions = scroll_regions;
        self.interaction.scrollbars = scrollbars;
        self.ime_rect = self.focused_caret_rect();
        self.note_sink_caret();
        // The atlas refused a glyph for room this frame rather than drop
        // a slot the frame had already used (F99): the next frame starts
        // on an empty page and draws it, and has to come — an
        // input-driven app would keep the short frame until the next
        // event. A page that only grew is right as it is presented.
        if self.atlas.short() {
            self.frame_requested = true;
        }
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
                // The node, and its parent: an exit plays only where the
                // parent is still declared (backlog DX19), so the diff has
                // to know about that key too.
                let p = self.prev_tree.parent[i];
                let keys = [
                    Some(self.prev_tree.keys[i]),
                    (p != NIL).then(|| self.prev_tree.keys[p as usize]),
                ];
                for k in keys.into_iter().flatten() {
                    watch.insert(k);
                    mask |= 1u64 << (k.0 & 63);
                }
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
            // Its parent went too, and the parent declared no exit that
            // would have carried it (it would have swallowed it above): the
            // node went with its ancestor, not on its own, and plays nothing
            // — a tab switched away does not fade out every column that
            // fades when it closes (backlog DX19). CSS removes the subtree;
            // React's `AnimatePresence` plays the exits of its direct
            // children only. This is that rule.
            let p = self.prev_tree.parent[i];
            if p != NIL && !live.contains(&self.prev_tree.keys[p as usize]) {
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
                .place(&self.prev_tree, i, &self.float_stack);
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
            // nothing. Same rule as the live pass, from the root down —
            // a parent-anchored float with `clip`, a stroke or polygon
            // included, is held by the parent's clip as a child is (F78,
            // F90; the ghost pass kept the old escape, RG26).
            let escapes = node
                .spec
                .layout
                .float
                .is_some_and(|f| !f.clipped_by_parent());
            let (clip, clip_id) = if node.parent == NIL || escapes {
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
            let leaf = match node.content {
                GhostContent::Container => Leaf::Container,
                // None once the shaped buffer has been evicted: a ghost
                // older than the text cache draws no text rather than
                // somebody else's. A departing subtree takes no input and
                // holds no selection; it records no place either.
                GhostContent::Text { cache_key, color } => {
                    match self.text.readd(cache_key, color) {
                        Some(tid) => Leaf::Text { tid, sel: None },
                        None => Leaf::Container,
                    }
                }
                // Never focused: the departing subtree gave the keyboard
                // up the frame it stopped being declared.
                GhostContent::Edit(key) => Leaf::Edit {
                    key,
                    focused: false,
                    pad: node.spec.layout.padding,
                },
                GhostContent::Image(id, opts) => Leaf::Image(id, opts),
                // The picture is frozen at departure — the parameters are
                // the ones the node last declared — while the box eases
                // and the group opacity fades it.
                GhostContent::Fragment(draw) => Leaf::Fragment(draw),
                GhostContent::Polygon(draw) => Leaf::Polygon(draw),
                // The points are the ghost's own copy; the colour is the
                // `bg` slot, which `play.bg` eases on the root.
                GhostContent::Line { first, len, width } => Leaf::Line {
                    points: &g.points[first as usize..(first + len) as usize],
                    width,
                },
            };
            let paint = Paint {
                clip,
                clip_id,
                scale,
                opacity,
            };
            painter!(self).paint_box(rect, &style, &paint, leaf);
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
    /// `{kind="layout", x, y, w, h, parent, scale, tag}`, pending like a
    /// `resize`.
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
            let payload = Value::map([
                ("kind", Value::str("layout")),
                ("x", Value::Float(rect.x as f64)),
                ("y", Value::Float(rect.y as f64)),
                ("w", Value::Float(rect.w as f64)),
                ("h", Value::Float(rect.h as f64)),
                ("parent", rect_value(parent_rect)),
                // Physical px per logical px at this node — the number a
                // view multiplies `w`/`h` by to know how many pixels to
                // render before `update_image` (ADR 0025, decision 5).
                // The frame's today; where a zoom would compose in.
                ("scale", Value::Float(self.scale as f64)),
            ]);
            self.pending
                .push(UiEvent::on(self.tree.origins[i], key, payload).tagged(Some(tag)));
        }
    }

    /// The chrome of one layer, after its content: the bars of every
    /// scroller the layer emitted (`regions`), then the ring if the focused
    /// node is in this layer (ADR 0023, decision 2). `layer` is the float
    /// root's index, or `NIL` for the in-flow layer; `above` is the hit
    /// list's length now, which is what a bar records so a press can tell
    /// a region under it from one in a layer over it (decision 4).
    fn emit_layer_chrome(
        &mut self,
        layer: u32,
        regions: &[ScrollRegion],
        scrollbars: &mut Vec<ScrollbarRegion>,
        above: usize,
        scale: f32,
    ) {
        let cursor = self.interaction.cursor();
        let above = above as u32;
        for r in regions {
            let i = r.node as usize;
            // The node's own bar style, if it declared one: hidden means
            // no thumb and no track, and the wheel still works because the
            // scroll region is already pushed. A handler has no offset to
            // draw a bar for.
            let style = self.tree.specs[i].interact().scrollbar;
            if r.handler || style.mode == crate::spec::ScrollbarMode::Hidden {
                continue;
            }
            let rest_w = style.width.unwrap_or(SCROLLBAR_W);
            let active_w = rest_w + (SCROLLBAR_ACTIVE_W - SCROLLBAR_W);
            // The grabbable gutter: the stock one, or enough for a wide
            // thumb and its inset.
            let hit_w = SCROLLBAR_HIT_W.max(active_w + 2.0 * SCROLLBAR_INSET);
            let max = self.tree.scroll_max[i];
            let offset = self.scroll.drawn(r.key);
            let clip_id = self.clip_ids.get(i).copied().unwrap_or(NO_CLIP_ID);
            let mut opacity = self.opacity.get(i).copied().unwrap_or(1.0);
            // The tracks, before either bar: an `auto` bar is held while
            // the pointer is on either track, and the two tracks share the
            // one quiet clock.
            let track_y = Rect::new(
                r.rect.x + r.rect.w - hit_w,
                r.rect.y + SCROLLBAR_INSET,
                hit_w,
                r.rect.h - 2.0 * SCROLLBAR_INSET,
            );
            let track_x = Rect::new(
                r.rect.x + SCROLLBAR_INSET,
                r.rect.y + r.rect.h - hit_w,
                r.rect.w - 2.0 * SCROLLBAR_INSET,
                hit_w,
            );
            if style.mode == crate::spec::ScrollbarMode::Auto
                && let Some(now) = self.anim.time()
            {
                let held = self.interaction.is_scrollbar_dragging(r.key, ScrollAxis::Y)
                    || self.interaction.is_scrollbar_dragging(r.key, ScrollAxis::X)
                    || cursor.is_some_and(|p| {
                        (max.y > 0.0 && track_y.contains(p)) || (max.x > 0.0 && track_x.contains(p))
                    });
                let idle = self.scroll.bar_idle(r.key, now, held);
                let shown = if idle < SCROLLBAR_HOLD {
                    1.0
                } else {
                    (1.0 - (idle - SCROLLBAR_HOLD) / SCROLLBAR_FADE).max(0.0)
                };
                if shown <= 0.0 {
                    continue; // faded out: no thumb, and no track to press
                }
                // Something to settle: the hold running out, or the fade.
                // Not while held — that is input's to end, and a frame a
                // hover would ask for every 8 ms is the idle CPU C27 fought.
                if !held {
                    self.frame_requested = true;
                }
                opacity *= shown as f32;
            }
            for (axis, track) in [(ScrollAxis::Y, track_y), (ScrollAxis::X, track_x)] {
                let (max_a, offset_a) = match axis {
                    ScrollAxis::Y => (max.y, offset.y),
                    ScrollAxis::X => (max.x, offset.x),
                };
                if max_a <= 0.0 {
                    continue;
                }
                let active = self.interaction.is_scrollbar_dragging(r.key, axis)
                    || cursor.is_some_and(|p| track.contains(p));
                let w = if active { active_w } else { rest_w };
                let (thumb, bar_len) = thumb_along(axis, r.rect, track, max_a, offset_a, w);
                let mut bar =
                    scrollbar_quad(thumb, scale, clip_id, self.thumb_color(&style, active));
                bar.color.a *= opacity;
                self.display.quads.push(bar);
                scrollbars.push(ScrollbarRegion {
                    key: r.key,
                    axis,
                    thumb,
                    track,
                    bar_len,
                    max: max_a,
                    inert: r.inert,
                    above,
                });
            }
        }
        self.emit_focus_ring(layer, scale);
    }

    /// The layer node `i` paints in: its float root's index, `NIL` in flow
    /// — and `NIL` for every node of a frame that floats nothing, where the
    /// map is not even sized.
    #[inline]
    fn layer_of(&self, i: usize) -> u32 {
        self.float_root.get(i).copied().unwrap_or(NIL)
    }

    /// This frame's float layers in paint order, from `roots` (the float
    /// roots in tree order) and the stack the last frame left: a root the
    /// stack knows keeps its place, one it does not is appended, in tree
    /// order, and a root the frame no longer declares is dropped. Writes
    /// the stack back for the next frame (ADR 0023, decision 3).
    ///
    /// The steady state — the same roots as last frame — is one pass over
    /// the stack and no allocation beyond the order itself: each entry
    /// remembers its root's rank in tree order, so the check and the
    /// answer are the same read.
    fn stack_floats(&mut self, roots: &[u32]) -> Vec<u32> {
        let keys = &self.tree.keys;
        let stack = &mut self.float_stack;
        let steady = stack.len() == roots.len()
            && stack
                .iter()
                .all(|&(k, rank)| keys[roots[rank as usize] as usize] == k);
        let order: Vec<u32> = if steady {
            stack
                .iter()
                .map(|&(_, rank)| roots[rank as usize])
                .collect()
        } else {
            // A float opened or closed: the ranks are found again, by key.
            let rank_of: FxHashMap<Key, u32> = roots
                .iter()
                .enumerate()
                .map(|(rank, &r)| (keys[r as usize], rank as u32))
                .collect();
            let mut placed = vec![false; roots.len()];
            let mut next: Vec<(Key, u32)> = Vec::with_capacity(roots.len());
            for &(k, _) in stack.iter() {
                if let Some(&rank) = rank_of.get(&k) {
                    placed[rank as usize] = true;
                    next.push((k, rank));
                }
            }
            for (rank, &r) in roots.iter().enumerate() {
                if !placed[rank] {
                    next.push((keys[r as usize], rank as u32));
                }
            }
            *stack = next;
            stack
                .iter()
                .map(|&(_, rank)| roots[rank as usize])
                .collect()
        };
        // A nested float is above the float it is in — by construction,
        // since its key derives from its parent's and so cannot have been
        // opened first; said here so the construction cannot drift.
        debug_assert!(order.iter().enumerate().all(|(pos, &r)| {
            let parent = self.tree.parent[r as usize];
            parent == NIL || {
                let outer = self.float_root[parent as usize];
                outer == NIL || order[..pos].contains(&outer)
            }
        }));
        order
    }

    /// ADR 0003's `modal-behind-content`, for the stack: a float layer
    /// above the modal's whose root is outside the modal's scope is inert
    /// and drawn over the one surface that takes input, which is the same
    /// defect the in-flow check names (ADR 0023, decision 6). Only a layer
    /// with something in it that *would* take input is the defect — a
    /// control the user sees and cannot press. A picture over the dialog
    /// (a HUD, the devtools' inspector outline) is not, and is not named.
    fn check_layers_over_modal(&mut self, order: &[u32]) {
        let Some((start, end, modal_key)) = self.modal else {
            return;
        };
        let modal_layer = self.layer_of(start);
        if modal_layer == NIL {
            return; // `diag::check_modal` has this case
        }
        let Some(at) = order.iter().position(|&r| r == modal_layer) else {
            return;
        };
        let over = order[at + 1..].iter().any(|&r| {
            let root = r as usize;
            !(start..end).contains(&root)
                && (root..self.tree.subtree_end(root)).any(|i| {
                    self.float_root[i] == r
                        && (self.tree.specs[i].hover_tracked()
                            || crate::access::focusable(&self.tree, i))
                })
        });
        if over {
            self.diag.raise(crate::diag::modal_under_layer(modal_key));
        }
    }

    /// The default focus ring around the keyboard-visibly focused node, at
    /// the end of the layer the node is in — above every sibling that could
    /// touch it, under every layer over it — in the same display list
    /// every binding draws. Not for editors (the caret shows focus), key
    /// sinks (an app surface styles itself, through `is_focused` /
    /// `focus_visible`) or nodes declaring `focus_bg`.
    fn emit_focus_ring(&mut self, layer: u32, scale: f32) {
        if !self.focus_visible {
            return;
        }
        let Some(i) = self.focus_index() else {
            return;
        };
        if self.layer_of(i) != layer {
            return;
        }
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

    /// The stock editor `key`'s node and its caret rect in viewport
    /// coordinates, from the frame laid out: the editor's own caret
    /// (physical px inside its text box) placed at the node's content
    /// origin. The one place this arithmetic lives (backlog AR45) — the
    /// IME anchor and the scroll-into-view both read it. None when no
    /// node of this frame is that editor, or it has no caret.
    pub(crate) fn stock_caret_viewport_rect(&mut self, key: Key) -> Option<(usize, Rect)> {
        let i = (0..self.tree.len()).find(|&i| self.tree.content[i] == NodeContent::Edit(key))?;
        let caret = self.edit_with_fonts(|edit, fs| edit.caret_rect(key, fs))?;
        let pad = self.tree.specs[i].layout.padding;
        Some((
            i,
            Rect::new(
                self.tree.pos[i].x + pad.l + caret.x / self.scale,
                self.tree.pos[i].y + pad.t + caret.y / self.scale,
                caret.w / self.scale,
                caret.h / self.scale,
            ),
        ))
    }

    fn focused_caret_rect(&mut self) -> Option<Rect> {
        if let Some(key) = self.edit.focused() {
            return self.stock_caret_viewport_rect(key).map(|(_, r)| r);
        }
        // A custom editor (backlog C17): the focused node's subtree holds
        // the `line` rows it draws, and the one carrying `caret` says
        // where the caret is — a byte offset into that line's runs, which
        // is the question `caret_rect` answers. This runs after the text
        // pass, so the places it reads are this frame's.
        let (l, caret, _) = self.sink_caret_line()?;
        self.text.caret_at(self.tree.keys[l], caret as usize, false)
    }

    /// The `line` under the focused node that declares `caret`, the
    /// offset it declares, and whether it declares the caret
    /// `caret_solid` — a custom editor's caret, in the frame just built.
    /// None with a stock editor focused, or nothing declaring one.
    fn sink_caret_line(&self) -> Option<(usize, u32, bool)> {
        if !self.tree.any_line {
            return None;
        }
        // The editor whose caret this is: the focused sink, or the sink
        // enclosing a focused control inside it — the node keys and
        // commits already go to (`key_target`, `sink_event`). Read from
        // the focused node alone, focus on a pane button inside a custom
        // editor un-armed the blink clock and lost the IME its anchor
        // while the keys kept arriving (backlog AR29).
        let i = self.focus_index()?;
        let i = if self.tree.specs[i].events().on_key.is_some() {
            i
        } else {
            self.enclosing_sink(i)?
        };
        // The candidates are the editor's lines as the access tree reads
        // them — `role="none"` subtrees (a gutter) skipped, a line's own
        // subtree not descended into — and the *last* one declaring a
        // caret is the caret, as `custom_editor` reads it.
        let l = crate::access::lines_under(&self.tree, i)
            .into_iter()
            .rev()
            .find(|&l| self.tree.specs[l].access().caret.is_some())?;
        let access = self.tree.specs[l].access();
        Some((l, access.caret?, access.caret_solid))
    }

    /// Remembers this frame's custom-editor caret and bumps the stamp when
    /// it is not last frame's (backlog C35): the blink clock reads both.
    /// Whether it is solid is kept beside it, not in it: a caret going
    /// from bar to block has not moved, and the clock re-arms on the
    /// way back from `has_caret` alone.
    fn note_sink_caret(&mut self) {
        let (now, solid) = if self.edit.focused().is_some() {
            (None, false)
        } else {
            match self.sink_caret_line() {
                Some((l, offset, solid)) => (Some((self.tree.keys[l], offset)), solid),
                None => (None, false),
            }
        };
        if now != self.sink_caret {
            self.sink_caret = now;
            self.sink_caret_stamp += 1;
        }
        self.sink_caret_solid = solid;
    }

    // -- The caret's blink --------------------------------------------
    // The clock is the driver's (a frame twice a second is a decision
    // about the window, not the tree); what the core keeps is whether
    // there is a caret to blink, when it moved, and the phase the driver
    // last set — for the stock editor, which paints its own caret on the
    // phase, and for a custom one, which reads it (backlog C35).

    /// Whether there is a caret to blink: a focused stock editor's, or the
    /// `caret` a `line` under the focused custom editor declares — unless
    /// that line declares it `caret_solid`, which is a caret to anchor
    /// the IME and read to assistive technology but not one to blink. A
    /// driver arms its blink clock while this is true and leaves the
    /// caret solid otherwise.
    pub fn has_caret(&self) -> bool {
        self.edit.focused().is_some() || (self.sink_caret.is_some() && !self.sink_caret_solid)
    }

    /// Changes whenever the caret moved or focus changed — the stock
    /// editor's caret through typing or a click, a custom editor's
    /// through the `caret` row it declares — so a driver comparing it
    /// across frames re-arms the blink with the caret solid, the way a
    /// caret that just moved is never mid-blink.
    pub fn caret_stamp(&self) -> u64 {
        self.edit.caret_stamp().wrapping_add(self.sink_caret_stamp)
    }

    /// The blink phase, as the driver last set it: `true` draws the
    /// caret. The stock editor reads it itself; a custom editor reads it
    /// in `view` (`Ui::caret_visible`) and skips its caret node on the
    /// off phase, so the two blink in step — and a window without the
    /// keyboard, where the driver parks it hidden, shows neither.
    /// Headless it stays `true`.
    pub fn caret_visible(&self) -> bool {
        self.edit.blink_visible()
    }

    /// Sets the blink phase; the driver's, on its clock. A frame is the
    /// caller's to ask for.
    pub fn set_caret_visible(&mut self, visible: bool) {
        self.edit.set_blink_visible(visible);
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

    fn baseline(&mut self, id: crate::tree::TextId) -> f32 {
        self.text.baseline(id)
    }

    fn edit_baseline(&mut self, key: Key) -> f32 {
        self.edit.baseline(key)
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
/// What a box holds, resolved to what painting it needs: the live pass
/// resolves a text node's selection and an editor's focus from the frame,
/// a ghost resolves nothing (no selection, never focused, its text re-added
/// from the cache) — and both hand the result here.
enum Leaf<'a> {
    Container,
    Text {
        tid: crate::tree::TextId,
        sel: Option<((usize, usize), Color)>,
    },
    Cells {
        cid: crate::cells::CellsId,
        at: Vec2,
        sel: Option<&'a crate::select::CellSelection>,
        tint: Color,
    },
    Edit {
        key: Key,
        focused: bool,
        /// The box's padding: the text starts inside it.
        pad: crate::geom::Edges,
    },
    Image(crate::resources::ImageId, crate::resources::ImageOpts),
    Fragment(crate::fragment::Draw),
    /// A polygon's draw; the fill is the node's `bg`, put through the
    /// fragment quad's colour rather than a box under it.
    Polygon(crate::fragment::Draw),
    Line {
        points: &'a [Vec2],
        width: f32,
    },
}

/// One box's paint: its shadow, its fill and border, its content, faded by
/// the group opacity — written once for the live node and the ghost, which
/// differ in what they *record* (hit regions, scroll regions, a text's
/// place) and not in what they draw. The two were the same hundred and
/// twenty lines until F41 had to be fixed in both.
struct Painter<'a> {
    display: &'a mut DisplayList,
    text: &'a mut TextSystem,
    edit: &'a mut EditStore,
    cells: &'a mut crate::cells::CellStore,
    atlas: &'a mut GlyphAtlas,
    session: &'a Session,
}

impl Painter<'_> {
    /// Inlined into its two callers: a call per node with the borrows
    /// packed into a struct measured +2.5% on `frame_10k_rects` (C15).
    /// What a leaf draws, and a shadow, are calls (`paint_leaf`,
    /// `shadow_quad`): inlined as well, they made every box pay for them
    /// (C48).
    #[inline(always)]
    fn paint_box(
        &mut self,
        rect: Rect,
        style: &crate::spec::VisualStyle,
        paint: &Paint,
        leaf: Leaf<'_>,
    ) {
        let Paint {
            clip,
            clip_id,
            scale,
            opacity,
        } = *paint;
        let clip_px = clip.scaled(scale);
        let first_quad = self.display.quads.len();
        if style.shadow.is_visible() {
            self.display
                .quads
                .push(shadow_quad(style, rect, clip_id, scale));
        }
        // A stroke's `bg` is its colour, not a box to fill (ADR 0010,
        // decision 7), and a polygon's is its fill (ADR 0025, decision 6)
        // — for the ghost of one as much as for the live one.
        let is_line = matches!(leaf, Leaf::Line { .. } | Leaf::Polygon(_));
        if !is_line
            && (style.bg.is_visible() || (style.border_w > 0.0 && style.border_color.is_visible()))
        {
            // Where layout put it, or on whole pixels when it asked
            // (`pixelSnap`), from the same numbers a text's backgrounds are.
            let px = rect.scaled(scale);
            self.display.quads.push(Quad {
                rect: if style.pixel_snap { px.on_pixels() } else { px },
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
        if !matches!(leaf, Leaf::Container) {
            self.paint_leaf(rect, style, paint, clip_px, leaf);
        }
        if opacity < 1.0 {
            fade(&mut self.display.quads[first_quad..], opacity);
        }
    }

    /// What a leaf draws inside its box: text, cells, an editor, an
    /// image, a fragment, a polygon's fill or a stroke. Out of line, so the
    /// kinds a plain box never takes do not weigh on every node's
    /// `emit_node` — its saved registers and its stack frame (backlog C48,
    /// as C41 was for the segment loop).
    #[inline(never)]
    fn paint_leaf(
        &mut self,
        rect: Rect,
        style: &crate::spec::VisualStyle,
        paint: &Paint,
        clip_px: crate::display::Clip,
        leaf: Leaf<'_>,
    ) {
        let Paint { clip_id, scale, .. } = *paint;
        match leaf {
            Leaf::Container => {}
            Leaf::Text { tid, sel } => {
                let sess = &mut *self.session.state();
                self.text.emit(
                    tid,
                    Vec2::new(rect.x, rect.y),
                    Size::new(rect.w, rect.h),
                    clip_px,
                    clip_id,
                    &mut self.display.clips,
                    &sess.resources,
                    &mut sess.fonts,
                    self.atlas,
                    &mut self.display.quads,
                    sel,
                );
            }
            Leaf::Cells { cid, at, sel, tint } => {
                let sess = &mut *self.session.state();
                self.cells.emit(
                    cid,
                    at,
                    clip_px,
                    clip_id,
                    &sess.resources,
                    &mut sess.fonts,
                    self.text.raster_mut(),
                    self.atlas,
                    &mut self.display.quads,
                    sel.map(|s| (s, tint)),
                );
            }
            Leaf::Edit { key, focused, pad } => {
                let origin = Vec2::new(
                    crate::geom::snap_px((rect.x + pad.l) * scale),
                    crate::geom::snap_px((rect.y + pad.t) * scale),
                );
                // A field bounds its own text horizontally — it is what
                // makes scrolling one legible rather than a line running
                // out over its neighbours (F41). Horizontally only: the
                // ancestors own the vertical clip, and a descender or a
                // caret is not what a field is trying to cut off.
                // Narrowing the clip makes a new one, so it needs an entry
                // of its own; an editor that folds to its width — a
                // document, or a field with `wrap` (F44) — keeps the node's.
                let (edit_clip, edit_clip_id) = if self.edit.folds(key) {
                    (clip_px, clip_id)
                } else {
                    let narrowed = clip_px.intersect(
                        Rect::new(
                            origin.x,
                            clip_px.rect.y,
                            (rect.w - pad.x()).max(0.0) * scale,
                            clip_px.rect.h,
                        ),
                        crate::display::SQUARE,
                    );
                    (narrowed, self.display.intern_clip(narrowed))
                };
                let sess = &mut *self.session.state();
                self.edit.emit(
                    key,
                    origin,
                    focused,
                    edit_clip,
                    edit_clip_id,
                    &mut sess.fonts,
                    self.text,
                    self.atlas,
                    &mut self.display.quads,
                );
            }
            Leaf::Image(id, opts) => {
                let sess = self.session.state();
                if let Some(entry) = sess.resources.image(id) {
                    // Atlas-backed unless the entry says otherwise — or
                    // unless the atlas cannot take it after all, which
                    // used to draw nothing (ADR 0025, decision 2).
                    let slot =
                        match entry.backing {
                            crate::resources::ImageBacking::Atlas => self
                                .atlas
                                .get_or_insert_image(id, entry.width, entry.height, &entry.rgba),
                            crate::resources::ImageBacking::Texture => None,
                        };
                    let (kind, uv) = match slot {
                        Some(slot) => (QuadKind::Image, [slot.x, slot.y, slot.w, slot.h]),
                        None => (
                            QuadKind::Texture,
                            [self.display.textures.len() as u32, 0, 0, 0],
                        ),
                    };
                    let mut uv = uv;
                    let (rect, tex_uv) = fit_image(
                        opts.fit,
                        rect,
                        Size::new(entry.width as f32, entry.height as f32),
                        [0, 0, entry.width, entry.height],
                    );
                    if kind == QuadKind::Image {
                        // The crop, if any, applied inside the atlas slot.
                        uv = [uv[0] + tex_uv[0], uv[1] + tex_uv[1], tex_uv[2], tex_uv[3]];
                    } else {
                        self.display
                            .textures
                            .push(crate::display::TextureDraw { id, uv: tex_uv });
                        self.display
                            .texture_pixels
                            .push(crate::display::TexturePixels {
                                width: entry.width,
                                height: entry.height,
                                rev: entry.rev,
                                rgba: entry.rgba.clone(),
                            });
                    }
                    self.display.quads.push(Quad {
                        rect: rect.scaled(scale),
                        // White = untinted; radius rounds like a solid.
                        color: Color::WHITE,
                        border_color: Color::TRANSPARENT,
                        radius: style.radius.map(|r| r * scale),
                        // The sampling flag rides the slot an image never
                        // had a border in (decision 4).
                        border_w: match opts.sampling {
                            crate::resources::Sampling::Linear => 0.0,
                            crate::resources::Sampling::Nearest => 1.0,
                        },
                        blur: 0.0,
                        kind,
                        clip: clip_id,
                        uv,
                    });
                }
            }
            Leaf::Fragment(draw) => {
                // On whole pixels when the node asked (`pixelSnap`), as its
                // background is, so a stack of fragments meets seamlessly.
                let px = rect.scaled(scale);
                push_fragment(
                    self.display,
                    self.atlas,
                    &self.session.state().resources,
                    draw,
                    if style.pixel_snap { px.on_pixels() } else { px },
                    style.radius.map(|r| r * scale),
                    clip_id,
                    Color::WHITE,
                );
            }
            Leaf::Polygon(draw) => {
                if style.bg.is_visible() {
                    push_fragment(
                        self.display,
                        self.atlas,
                        &self.session.state().resources,
                        draw,
                        rect.scaled(scale),
                        crate::display::SQUARE,
                        clip_id,
                        style.bg,
                    );
                }
            }
            Leaf::Line { points, width } => {
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
    }
}

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
/// keep its place (see `depart::Place`): the layer each node painted in,
/// and for the in-flow layer the next node at or after each index that
/// this frame still declares. Built once per frame that has a departure,
/// from the previous tree and the stack as that frame left it.
struct PaintOrder {
    /// The previous tree's float roots, by node: `NIL` in flow.
    float_root: Vec<u32>,
    /// By previous-tree index: the next live in-flow node at or after it;
    /// one past the end reads `NIL`.
    next_live: Vec<u32>,
    /// This frame's keys.
    live: FxHashSet<Key>,
    /// This frame's float roots, by key.
    roots: FxHashSet<Key>,
}

impl PaintOrder {
    fn of(prev: &Tree, tree: &Tree) -> Self {
        let n = prev.len();
        let live: FxHashSet<Key> = tree.keys.iter().copied().collect();
        let roots: FxHashSet<Key> = (0..tree.len())
            .filter(|&i| tree.specs[i].layout.float.is_some())
            .map(|i| tree.keys[i])
            .collect();
        let mut float_root = vec![NIL; n];
        for i in 0..n {
            let parent = prev.parent[i];
            float_root[i] = if prev.specs[i].layout.float.is_some() {
                i as u32
            } else if parent != NIL {
                float_root[parent as usize]
            } else {
                NIL
            };
        }
        let mut next_live = vec![NIL; n + 1];
        for j in (0..n).rev() {
            next_live[j] = next_live[j + 1];
            if float_root[j] == NIL && live.contains(&prev.keys[j]) {
                next_live[j] = j as u32;
            }
        }
        Self {
            float_root,
            next_live,
            live,
            roots,
        }
    }

    /// The place the subtree rooted at `root` of the previous frame painted
    /// in: its layer, and what was painted right after it there that is
    /// still here. `stack` is the previous frame's float stack, bottom to
    /// top — where a departing float finds the layer that was over it.
    fn place(&self, prev: &Tree, root: usize, stack: &[(Key, u32)]) -> Place {
        let end = prev.subtree_end(root);
        let layer = self.float_root[root];
        if layer == NIL {
            let after = self.next_live[end];
            return Place::InFlow {
                before: (after != NIL).then(|| prev.keys[after as usize]),
            };
        }
        if layer as usize != root {
            // Inside a float: the next live node of the same layer, found
            // by a scan bounded by that layer's subtree — a departure is
            // rare and a float is small.
            let layer_end = prev.subtree_end(layer as usize);
            let before = (end..layer_end)
                .find(|&j| self.float_root[j] == layer && self.live.contains(&prev.keys[j]))
                .map(|j| prev.keys[j]);
            return Place::InLayer {
                layer: prev.keys[layer as usize],
                before,
            };
        }
        // A float root: under the first layer above it in the stack that
        // is still a float this frame.
        let key = prev.keys[root];
        let at = stack.iter().position(|&(k, _)| k == key);
        let before = at.and_then(|at| {
            stack[at + 1..]
                .iter()
                .map(|&(k, _)| k)
                .find(|k| self.roots.contains(k))
        });
        Place::Layer { before }
    }
}

/// One [`QuadKind::Segment`] per straight piece of a stroke: `points` are
/// relative to `origin` (the node's box, logical px) and `width` is
/// logical; everything on the quad is physical. The rect is the piece's
/// bounding box padded by half the width plus two logical px, so the
/// backend's edge ramp is never cut by the quad's own edge, and the
/// endpoints ride in `uv` (see [`Quad::segment_ends`]).
///
/// Kept out of line on purpose (backlog C41): inlined into `emit_node`,
/// whose size moves with every prop a hit region grows, the loop's carried
/// point lost its register to the stack once the drop-zone commit tipped
/// the allocator — a store and a reload on every segment, +10% on
/// `frame_1k_curves`. On its own the loop keeps every value in a register.
#[inline(never)]
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
/// silhouette instead of sprouting corners. Never inlined: in
/// `emit_node` its arithmetic took two more saved float registers for
/// every node, shadow or not (backlog C48).
#[inline(never)]
fn shadow_quad(style: &crate::spec::VisualStyle, rect: Rect, clip_id: ClipId, scale: f32) -> Quad {
    let sh = style.shadow;
    let blur = sh.blur.max(0.0);
    let shape = Rect::new(
        rect.x + sh.dx - sh.spread,
        rect.y + sh.dy - sh.spread,
        (rect.w + 2.0 * sh.spread).max(0.0),
        (rect.h + 2.0 * sh.spread).max(0.0),
    );
    let rect = if style.pixel_snap {
        // Snapped with its box, so it stays under it; the blur around it.
        let s = shape.scaled(scale).on_pixels();
        let b = blur * scale;
        Rect::new(s.x - b, s.y - b, s.w + 2.0 * b, s.h + 2.0 * b)
    } else {
        Rect::new(
            (shape.x - blur) * scale,
            (shape.y - blur) * scale,
            (shape.w + 2.0 * blur) * scale,
            (shape.h + 2.0 * blur) * scale,
        )
    };
    Quad {
        rect,
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

/// The thumb of a scrollbar along `axis`, inset from the far edge of the
/// scroller's `rect`, `w` thick, and its length along the track: the
/// track's share of the content that is visible, never shorter than
/// `SCROLLBAR_MIN`, placed by how far the content has scrolled. One
/// geometry for both bars — the Y bar and the X bar were the same thirty
/// lines with the axes swapped.
fn thumb_along(
    axis: ScrollAxis,
    rect: Rect,
    track: Rect,
    max: f32,
    offset: f32,
    w: f32,
) -> (Rect, f32) {
    let t = (offset / max).clamp(0.0, 1.0);
    match axis {
        ScrollAxis::Y => {
            let bar = (track.h * rect.h / (rect.h + max)).max(SCROLLBAR_MIN);
            let thumb = Rect::new(
                rect.x + rect.w - w - SCROLLBAR_INSET,
                track.y + t * (track.h - bar),
                w,
                bar,
            );
            (thumb, bar)
        }
        ScrollAxis::X => {
            let bar = (track.w * rect.w / (rect.w + max)).max(SCROLLBAR_MIN);
            let thumb = Rect::new(
                track.x + t * (track.w - bar),
                rect.y + rect.h - w - SCROLLBAR_INSET,
                bar,
                w,
            );
            (thumb, bar)
        }
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
/// Resolves the `fit` row: the rect the pixels paint into (logical px)
/// and the texel rect of the image they come from. `fill` stretches the
/// whole image to the box; `contain` shrinks the painted rect to the
/// image's aspect, centred; `cover` keeps the box and crops the texels,
/// centred (ADR 0025, decision 4). A zero-sized image or box falls back to
/// `fill`, which paints nothing visible either way.
pub(crate) fn fit_image(
    fit: crate::resources::ImageFit,
    rect: Rect,
    image: Size,
    texels: [u32; 4],
) -> (Rect, [u32; 4]) {
    use crate::resources::ImageFit;
    if image.w <= 0.0 || image.h <= 0.0 || rect.w <= 0.0 || rect.h <= 0.0 {
        return (rect, texels);
    }
    let box_aspect = rect.w / rect.h;
    let image_aspect = image.w / image.h;
    match fit {
        ImageFit::Fill => (rect, texels),
        ImageFit::Contain => {
            let (w, h) = if image_aspect > box_aspect {
                (rect.w, rect.w / image_aspect)
            } else {
                (rect.h * image_aspect, rect.h)
            };
            (
                Rect::new(
                    rect.x + (rect.w - w) * 0.5,
                    rect.y + (rect.h - h) * 0.5,
                    w,
                    h,
                ),
                texels,
            )
        }
        ImageFit::Cover => {
            // Whole texels: a crop is a rect on the texture, and a
            // half-texel edge would sample the neighbour.
            let (w, h) = if image_aspect > box_aspect {
                ((image.h * box_aspect).round().max(1.0), image.h)
            } else {
                (image.w, (image.w / box_aspect).round().max(1.0))
            };
            let x = ((image.w - w) * 0.5).floor();
            let y = ((image.h - h) * 0.5).floor();
            (
                rect,
                [
                    texels[0] + x as u32,
                    texels[1] + y as u32,
                    w as u32,
                    h as u32,
                ],
            )
        }
    }
}

/// One fragment quad and its side entry: the draw, the source a backend
/// compiles, and the colour the function reads as `in.color` — white for
/// a `fragment`, the fill for a `polygon` (ADR 0025).
///
/// A draw naming an `image` resolves it here, where the window's atlas
/// is: an atlas-backed image goes into the atlas as an `image` node's
/// would and the draw carries its slot; a texture-backed one takes an
/// entry of the texture side list — the same entry an `image` node of it
/// would — and the draw carries the index, so the backend binds that
/// texture for this one quad as it does for a texture quad. An image
/// handle that is not live draws nothing, the fallback every resource
/// kind has, and the lookup is what records a foreign one.
#[allow(clippy::too_many_arguments)]
fn push_fragment(
    display: &mut DisplayList,
    atlas: &mut GlyphAtlas,
    resources: &crate::resources::Resources,
    draw: crate::fragment::Draw,
    rect: Rect,
    radius: [f32; 4],
    clip_id: ClipId,
    color: Color,
) {
    let Some(source) = resources.fragment(draw.id) else {
        return;
    };
    let image = match draw.image {
        None => crate::display::FragmentImage::None,
        Some(id) => {
            let Some(entry) = resources.image(id) else {
                return;
            };
            let slot = match entry.backing {
                crate::resources::ImageBacking::Atlas => {
                    atlas.get_or_insert_image(id, entry.width, entry.height, &entry.rgba)
                }
                crate::resources::ImageBacking::Texture => None,
            };
            match slot {
                Some(slot) => {
                    crate::display::FragmentImage::Atlas([slot.x, slot.y, slot.w, slot.h])
                }
                None => {
                    let index = display.textures.len() as u32;
                    let uv = [0, 0, entry.width, entry.height];
                    display
                        .textures
                        .push(crate::display::TextureDraw { id, uv });
                    display.texture_pixels.push(crate::display::TexturePixels {
                        width: entry.width,
                        height: entry.height,
                        rev: entry.rev,
                        rgba: entry.rgba.clone(),
                    });
                    crate::display::FragmentImage::Texture { index, uv }
                }
            }
        }
    };
    let index = display.fragments.len() as u32;
    display.fragments.push(crate::display::FragmentDraw {
        id: draw.id,
        params: draw.params,
        image,
    });
    display.fragment_sources.push(source.clone());
    display.quads.push(Quad {
        rect,
        // White on a `fragment` — the function returns its own colour and
        // reads this as `in.color` if it wants one — and the fill on a
        // `polygon`; `a` is the fill's alpha times the group opacity, which
        // the fade pass multiplies in after the node's quads are pushed.
        color,
        border_color: Color::TRANSPARENT,
        radius,
        border_w: 0.0,
        blur: 0.0,
        kind: QuadKind::Fragment,
        clip: clip_id,
        uv: [index, 0, 0, 0],
    });
}
