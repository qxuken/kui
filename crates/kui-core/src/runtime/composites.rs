//! Arrow-key motion inside a composite:
//! a tab list, a radio
//! group, a menu or a picker list is one Tab stop, and these move focus
//! within it — arrows, Home/End, type-ahead, and the slider nudge.

use super::*;

impl Core {
    // -- Composites (docs/adr/0007-composite-keyboard-patterns.md) ------
    // A tab list, a radio group, a menu and a picker list are one Tab stop
    // with the arrows moving inside. The core moves that focus itself,
    // because focus is core state and this is the motion Tab already
    // performs with a narrower scope — every app in every binding would
    // otherwise reimplement the same ordered walk over a tree the core
    // computes and does not expose, and a Lua app could not do it at all.
    // It never writes `selected`: that is app state, re-declared each
    // frame, so a view that wants selection to follow focus writes
    // `selected(ui.is_focused(key))`.

    /// One arrow, Home, End, Page Up or Page Down step inside the composite
    /// holding node `i`. Nothing happens when `i` is not a composite item,
    /// or when the step runs off the end of one that clamps. A step to a
    /// row a windowed list or table has not built reaches for it instead
    /// (ADR 0070): the scroller moves toward it, and the frame that builds
    /// it lands the focus there.
    pub(crate) fn composite_step(
        &mut self,
        i: usize,
        ek: EditKey,
        mods: crate::input::Mods,
        out: &mut Vec<UiEvent>,
    ) {
        let mut items = std::mem::take(&mut self.items_scratch);
        let target = self.composite_target(i, ek, mods, &mut items);
        self.items_scratch = items;
        match target {
            Some(Target::Land(j, item)) => self.move_within_composite(i, j, item, out),
            Some(Target::Reach(reach, toward)) => self.reach_unbuilt(i, reach, toward),
            None => {}
        }
    }

    /// Lands focus on item `j` of a composite, having come from `i`, and
    /// activates it where the pattern says selection follows focus. The
    /// landing is exactly `focus_next`'s: focus moves, it shows, and the
    /// item scrolls into view.
    fn move_within_composite(
        &mut self,
        i: usize,
        j: usize,
        item: crate::access::Role,
        out: &mut Vec<UiEvent>,
    ) {
        self.focus_visible = true;
        if j == i {
            // A clamped End on the last item, or a search that matched
            // where focus already is: the motion happened, so the focus
            // shows, but nothing moved and nothing is activated again.
            return;
        }
        self.land_focus(j);
        // `radio` and `tab` define selection as following focus, and the
        // event is the one Enter already emits — so an app that handles
        // clicks on its tabs handles arrows on them with no new code.
        if crate::composite::activates_on_motion(item) {
            self.click_node(self.tree.keys[j], out);
        }
    }

    /// Where a step from item `i` lands, with the item role it lands on,
    /// or the unbuilt row it reaches for. Fills `items` with the
    /// composite's items on the way (see `composite::items`).
    fn composite_target(
        &self,
        i: usize,
        ek: EditKey,
        mods: crate::input::Mods,
        items: &mut Vec<usize>,
    ) -> Option<Target> {
        use crate::access::Role;
        let container = crate::composite::owner(&self.tree, i, items)?;
        let role = crate::composite::role(&self.tree, container)?;
        let item = crate::composite::item_role(role)?;
        // A disabled item is not an item here, as it is not in the ring —
        // it keeps its ordinal in "3 of 7" and the arrows step over it.
        let live: Vec<usize> = items
            .iter()
            .copied()
            .filter(|&j| crate::access::focusable(&self.tree, j))
            .collect();
        if role == Role::Table {
            return self.table_target(container, i, ek, mods, items);
        }
        let at = live.iter().position(|&j| j == i)?;
        // A list whose rows carry their data index under a `rowCount` is
        // windowed: its length is the count, and a step is by index, to a
        // row the frame may not have built (ADR 0070).
        if role == Role::List
            && let Some((k, n)) = self.window_of(container, i)
        {
            let page = self.page_rows(container, i, &live);
            let t = match ek {
                EditKey::Left | EditKey::Up => k.checked_sub(1)?,
                EditKey::Right | EditKey::Down => (k + 1 < n).then_some(k + 1)?,
                EditKey::Home => 0,
                EditKey::End => n.checked_sub(1)?,
                EditKey::PageUp => k.saturating_sub(page as u64),
                EditKey::PageDown => (k + page as u64).min(n.saturating_sub(1)),
                _ => return None,
            };
            return self.land_or_reach(container, i, &live, t, None, item);
        }
        let wrap = crate::composite::wraps(role);
        let layout = self.tree.specs[container].layout;
        // The one place the two arrow pairs differ: in a wrapped container
        // the cross-axis pair moves by a line rather than by one item,
        // which is what a wrapped grid of items needs. Wrapping is
        // rows-only, so the cross pair is always Up / Down.
        let by_line = layout.wrap && layout.dir == crate::spec::Dir::Row;
        let j = match ek {
            EditKey::Home => live[0],
            EditKey::End => live[live.len() - 1],
            EditKey::Up | EditKey::Down if by_line => {
                let down = ek == EditKey::Down;
                live[self.line_step(container, &live, at, down, wrap)?]
            }
            // Both pairs move, whatever the derived orientation says: the
            // perpendicular pair costs nothing, while refusing it turns a
            // mis-derived axis into a keyboard dead end that only a screen
            // reader user finds.
            EditKey::Left | EditKey::Up => live[step(at, -1, live.len(), wrap)?],
            EditKey::Right | EditKey::Down => live[step(at, 1, live.len(), wrap)?],
            // A list pages by what its scroller shows (ADR 0070); a choice
            // among a few has no pages.
            EditKey::PageUp | EditKey::PageDown if role == Role::List => {
                let page = self.page_rows(container, i, &live);
                if ek == EditKey::PageUp {
                    live[at.saturating_sub(page)]
                } else {
                    live[(at + page).min(live.len() - 1)]
                }
            }
            _ => return None,
        };
        Some(Target::Land(j, item))
    }

    /// A step inside a table (ADR 0070, backlog C90): Left / Right along
    /// the row, Up / Down to the row before or after keeping the column,
    /// Home / End to the row's ends — the table's first and last cell with
    /// Ctrl (or the primary) held — and Page Up / Page Down by the rows its
    /// scroller shows. It clamps at every edge, a table being a list of
    /// rows. Rows a windowed table has not built are reached for by their
    /// data index.
    fn table_target(
        &self,
        container: usize,
        i: usize,
        ek: EditKey,
        mods: crate::input::Mods,
        cells: &[usize],
    ) -> Option<Target> {
        use crate::access::Role;
        let live = |j: usize| crate::access::focusable(&self.tree, j);
        // The rows in tree order, each with every cell it holds, live or
        // not, so a disabled cell keeps its column.
        let mut rows: Vec<(usize, Vec<usize>)> = Vec::new();
        for &c in cells {
            let r = crate::composite::row_of(&self.tree, container, c);
            match rows.last_mut() {
                Some((at, row)) if *at == r => row.push(c),
                _ => rows.push((r, vec![c])),
            }
        }
        let ri = rows.iter().position(|(_, row)| row.contains(&i))?;
        let ci = rows[ri].1.iter().position(|&c| c == i)?;
        let along = |row: &[usize], from: usize, back: bool| -> Option<usize> {
            if back {
                row[..from].iter().rev().copied().find(|&c| live(c))
            } else {
                row[from + 1..].iter().copied().find(|&c| live(c))
            }
        };
        let to_table_end = mods.ctrl || mods.doc;
        let land = |j: usize| Some(Target::Land(j, Role::Cell));
        match ek {
            EditKey::Left => return land(along(&rows[ri].1, ci, true)?),
            EditKey::Right => return land(along(&rows[ri].1, ci, false)?),
            EditKey::Home if !to_table_end => {
                return land(rows[ri].1.iter().copied().find(|&c| live(c))?);
            }
            EditKey::End if !to_table_end => {
                return land(rows[ri].1.iter().rev().copied().find(|&c| live(c))?);
            }
            EditKey::Up | EditKey::Down | EditKey::PageUp | EditKey::PageDown => {}
            EditKey::Home | EditKey::End => {}
            _ => return None,
        }
        // The column a row motion keeps: this one, or the first / last for
        // the table's own ends.
        let column = match ek {
            EditKey::Home => 0,
            EditKey::End => usize::MAX,
            _ => ci,
        };
        let units: Vec<usize> = rows.iter().map(|(r, _)| *r).collect();
        if let Some((k, n)) = self.window_of(container, rows[ri].0) {
            let page = self.page_rows(container, rows[ri].0, &units) as u64;
            let t = match ek {
                EditKey::Up => k.checked_sub(1)?,
                EditKey::Down => (k + 1 < n).then_some(k + 1)?,
                EditKey::PageUp => k.saturating_sub(page),
                EditKey::PageDown => (k + page).min(n.saturating_sub(1)),
                EditKey::Home => 0,
                _ => n.checked_sub(1)?,
            };
            if let Some((_, row)) = rows
                .iter()
                .find(|(r, _)| crate::composite::data_index(&self.tree, *r) == Some(t))
            {
                return land(self.cell_at_column(row, column)?);
            }
            return Some(self.reach(container, rows[ri].0, &units, t, Some(column)));
        }
        let with_live = |r: usize| rows[r].1.iter().any(|&c| live(c));
        let last = rows.len() - 1;
        let page = self.page_rows(container, rows[ri].0, &units);
        let (start, back) = match ek {
            EditKey::Up => (ri.checked_sub(1)?, true),
            EditKey::Down => ((ri < last).then_some(ri + 1)?, false),
            EditKey::PageUp => (ri.saturating_sub(page), true),
            EditKey::PageDown => ((ri + page).min(last), false),
            EditKey::Home => (0, false),
            _ => (last, true),
        };
        // Past a row with nothing live in it, the way the step went.
        let mut r = start;
        while !with_live(r) {
            r = if back {
                r.checked_sub(1)?
            } else {
                (r < last).then_some(r + 1)?
            };
        }
        land(self.cell_at_column(&rows[r].1, column)?)
    }

    /// The live cell of `row` nearest column `column` (`usize::MAX` for the
    /// row's last): that column itself when it is live, else the nearest
    /// on either side, the earlier on a tie.
    fn cell_at_column(&self, row: &[usize], column: usize) -> Option<usize> {
        let live = |j: usize| crate::access::focusable(&self.tree, j);
        let c = column.min(row.len().checked_sub(1)?);
        (0..row.len()).find_map(|d| {
            [c.checked_sub(d), Some(c + d).filter(|&x| x < row.len())]
                .into_iter()
                .flatten()
                .map(|x| row[x])
                .find(|&j| live(j))
        })
    }

    /// The data index of the composite unit at `unit` — a list's item, a
    /// table's row — and the count of rows the composite at `container` has,
    /// built or not, when it is windowed: the unit carries an `index` and a
    /// `rowCount` is declared between them (ADR 0070). An index a step is
    /// already reaching for in the same composite stands in for the unit's,
    /// so a key held faster than frames come keeps going.
    fn window_of(&self, container: usize, unit: usize) -> Option<(u64, u64)> {
        let index = crate::composite::data_index(&self.tree, unit)?;
        let n = crate::composite::row_count(&self.tree, unit, container)?;
        let key = self.tree.keys[container];
        let k = self
            .composite_reach
            .filter(|r| r.container == key)
            .map_or(index, |r| r.index);
        Some((k.min(n.saturating_sub(1)), n))
    }

    /// A windowed list's step to data index `t`: onto the built item with
    /// that index when the frame has one, else a reach for it.
    fn land_or_reach(
        &self,
        container: usize,
        i: usize,
        live: &[usize],
        t: u64,
        column: Option<usize>,
        item: crate::access::Role,
    ) -> Option<Target> {
        if crate::composite::data_index(&self.tree, i) == Some(t) {
            return Some(Target::Land(i, item));
        }
        if let Some(&j) = live
            .iter()
            .find(|&&j| crate::composite::data_index(&self.tree, j) == Some(t))
        {
            return Some(Target::Land(j, item));
        }
        Some(self.reach(container, i, live, t, column))
    }

    /// A reach for the unbuilt row `t`, from the built unit `from` among
    /// `units`: where the row would be, from the pitch the built ones are
    /// laid at, for the scroller to move toward.
    fn reach(
        &self,
        container: usize,
        from: usize,
        units: &[usize],
        t: u64,
        column: Option<usize>,
    ) -> Target {
        let toward = self.estimate_row(from, units, t);
        Target::Reach(
            Reach {
                container: self.tree.keys[container],
                index: t,
                column,
                tries: 0,
                since: 0,
            },
            toward,
        )
    }

    /// Where row `t` would sit, from the built unit `from` and the pitch
    /// the built `units` are laid at: a virtual list's rows are uniform,
    /// which is what lets its spacers stand in for the rest.
    fn estimate_row(&self, from: usize, units: &[usize], t: u64) -> Rect {
        let rect = |j: usize| Rect::from_pos_size(self.tree.pos[j], self.tree.size[j]);
        let at = crate::composite::data_index(&self.tree, from).unwrap_or(0);
        let pitch = self.pitch(units, from);
        let d = t as f32 - at as f32;
        let r = rect(from);
        Rect::new(r.x + pitch.x * d, r.y + pitch.y * d, r.w, r.h)
    }

    /// The distance from one row to the next: across the built `units` that
    /// carry data indices, the first to the last over the indices between;
    /// else between the first and last in tree order; else `unit`'s own
    /// height, rows being stacked down.
    fn pitch(&self, units: &[usize], unit: usize) -> Vec2 {
        let pos = |j: usize| self.tree.pos[j];
        let indexed: Vec<(usize, u64)> = units
            .iter()
            .filter_map(|&j| Some((j, crate::composite::data_index(&self.tree, j)?)))
            .collect();
        let span = match (indexed.first(), indexed.last()) {
            (Some(&(a, ia)), Some(&(b, ib))) if ib != ia => Some((a, b, ib as f32 - ia as f32)),
            _ => match (units.first(), units.last()) {
                (Some(&a), Some(&b)) if a != b => Some((a, b, (units.len() - 1) as f32)),
                _ => None,
            },
        };
        match span {
            Some((a, b, n)) => Vec2::new((pos(b).x - pos(a).x) / n, (pos(b).y - pos(a).y) / n),
            None => Vec2::new(0.0, self.tree.size[unit].h),
        }
    }

    /// How many rows a page is: what the scroller around `unit` shows of
    /// them along the way they are stacked, at least one — or the composite's
    /// own box, with no scroller between.
    fn page_rows(&self, container: usize, unit: usize, units: &[usize]) -> usize {
        let pitch = self.pitch(units, unit);
        let mut a = self.tree.parent[unit];
        let mut view = None;
        while a != NIL {
            let s = &self.tree.specs[a as usize].layout;
            if s.scroll_x || s.scroll_y {
                view = Some(self.tree.size[a as usize]);
                break;
            }
            if a as usize == container {
                break;
            }
            a = self.tree.parent[a as usize];
        }
        let view = view.unwrap_or(self.tree.size[container]);
        let (extent, step) = if pitch.y.abs() >= pitch.x.abs() {
            (view.h, pitch.y.abs())
        } else {
            (view.w, pitch.x.abs())
        };
        if step <= 0.0 {
            return 1;
        }
        ((extent / step).floor() as usize).max(1)
    }

    /// Starts a reach (ADR 0070): the focus shows, the scroller moves
    /// toward where the row would be, and a frame is owed — the one whose
    /// view builds the rows the scroller now shows, after which
    /// [`Self::resolve_composite_reach`] lands the focus.
    fn reach_unbuilt(&mut self, from: usize, reach: Reach, toward: Rect) {
        self.focus_visible = true;
        self.scroll_rect_into_view(from, toward, false);
        self.composite_reach = Some(Reach {
            since: self.frame_no,
            ..reach
        });
        self.owe_frame("composite_reach");
    }

    /// At a frame's end: lands a reach on the row it was for, now that the
    /// view had the chance to build it — a list's item with that index, a
    /// table's cell in that row nearest the column kept. A row still not
    /// built (rows that are not uniform, so the estimate missed) is
    /// reached for again from the rows this frame did build, a few times,
    /// and then let go; so is one whose composite is gone. Focus moving
    /// any other way meanwhile has already let it go.
    pub(crate) fn resolve_composite_reach(&mut self) {
        use crate::access::Role;
        const TRIES: u8 = 3;
        let Some(mut reach) = self.composite_reach.take() else {
            return;
        };
        if reach.since >= self.frame_no {
            // Begun at this frame's end, and scrolled already: the frame it
            // owes lands it.
            self.composite_reach = Some(reach);
            return;
        }
        let Some(container) = self.tree.index_of(reach.container) else {
            return;
        };
        let Some(role) = crate::composite::role(&self.tree, container) else {
            return;
        };
        let Some(item) = crate::composite::item_role(role) else {
            return;
        };
        let mut items = std::mem::take(&mut self.items_scratch);
        crate::composite::items(&self.tree, container, item, &mut items);
        let units: Vec<usize> = if role == Role::Table {
            let mut rows: Vec<usize> = Vec::new();
            for &c in &items {
                let r = crate::composite::row_of(&self.tree, container, c);
                if rows.last() != Some(&r) {
                    rows.push(r);
                }
            }
            rows
        } else {
            items.clone()
        };
        let hit = units
            .iter()
            .copied()
            .find(|&u| crate::composite::data_index(&self.tree, u) == Some(reach.index));
        let landing = hit.and_then(|u| {
            if role == Role::Table {
                let row: Vec<usize> = items
                    .iter()
                    .copied()
                    .filter(|&c| crate::composite::row_of(&self.tree, container, c) == u)
                    .collect();
                self.cell_at_column(&row, reach.column.unwrap_or(0))
            } else {
                Some(u).filter(|&j| crate::access::focusable(&self.tree, j))
            }
        });
        // Reached, and the row takes no focus: the nearest that does
        // (backlog C132).
        let landing = match (landing, hit) {
            (None, Some(_)) => self.nearest_live(
                container,
                role == Role::Table,
                &items,
                &units,
                reach.index,
                reach.column.unwrap_or(0),
            ),
            (l, _) => l,
        };
        self.items_scratch = items;
        if let Some(j) = landing {
            self.land_focus(j);
            self.focus_visible = true;
            return;
        }
        if hit.is_some() || reach.tries + 1 >= TRIES {
            return;
        }
        // Again, from the built row nearest the one wanted.
        let near = units.iter().copied().min_by_key(|&u| {
            crate::composite::data_index(&self.tree, u)
                .map_or(u64::MAX, |x| x.abs_diff(reach.index))
        });
        if let Some(from) = near.filter(|&u| crate::composite::data_index(&self.tree, u).is_some())
        {
            let toward = self.estimate_row(from, &units, reach.index);
            self.scroll_rect_into_view(from, toward, false);
            reach.tries += 1;
            self.composite_reach = Some(reach);
            self.owe_frame("composite_reach");
        }
    }

    /// A cross-axis step in a wrapped container: to the adjacent wrap line,
    /// keeping the position within the line (clamped to that line's
    /// length). Returns an index into `live`.
    fn line_step(
        &self,
        container: usize,
        live: &[usize],
        at: usize,
        down: bool,
        wrap: bool,
    ) -> Option<usize> {
        let lines: Vec<u32> = live
            .iter()
            .map(|&j| crate::composite::line_of(&self.tree, container, j))
            .collect();
        let cur = lines[at];
        let col = lines[..at].iter().filter(|&&l| l == cur).count();
        let next = if down {
            lines.iter().copied().filter(|&l| l > cur).min()
        } else {
            lines.iter().copied().filter(|&l| l < cur).max()
        };
        let target = match next {
            Some(l) => l,
            None if wrap => {
                let l = if down {
                    lines.iter().min()
                } else {
                    lines.iter().max()
                };
                *l?
            }
            None => return None,
        };
        let row: Vec<usize> = (0..live.len()).filter(|&k| lines[k] == target).collect();
        row.get(col.min(row.len().checked_sub(1)?)).copied()
    }

    /// One printable keystroke inside a composite: extends the search
    /// buffer and moves focus to the next item whose name starts with it,
    /// wrapping. Returns whether the composite took the text — `false`
    /// leaves Space to press the item, which is what it does with no
    /// search under way.
    pub(crate) fn type_ahead(&mut self, i: usize, text: &str, out: &mut Vec<UiEvent>) -> bool {
        let typed: String = text.chars().filter(|c| !c.is_control()).collect();
        // Aged here as well as at the frame: a window parks between
        // frames, so after a quiet second the keystroke can come before
        // any frame has aged the buffer, and the driver stamped the clock
        // for it (backlog F139). Before the Space test, so a Space after
        // the pause presses rather than extending a search gone stale.
        self.age_type_ahead();
        if typed.is_empty() || (typed == " " && self.type_ahead.is_empty()) {
            return false;
        }
        let mut items = std::mem::take(&mut self.items_scratch);
        let found = self.type_ahead_target(i, &typed, &mut items);
        self.items_scratch = items;
        match found {
            Some((j, item)) => {
                self.move_within_composite(i, j, item, out);
                true
            }
            // Not a composite item: the text is not ours, so Space still
            // presses. A search that matched nothing *is* ours — the
            // buffer holds it, and the next character extends it rather
            // than starting over — and a composite that asked is asked
            // for the row its data names (ADR 0070).
            None => {
                if self.type_ahead.is_empty() {
                    return false;
                }
                self.ask_typeahead(i, out);
                true
            }
        }
    }

    /// A search the built rows did not answer, asked of the composite item
    /// `i` belongs to when it declared `on_typeahead` (ADR 0070): the
    /// buffer, whole, as `typeahead: {text}` in its message.
    fn ask_typeahead(&mut self, i: usize, out: &mut Vec<UiEvent>) {
        let mut items = std::mem::take(&mut self.items_scratch);
        let container = crate::composite::owner(&self.tree, i, &mut items);
        self.items_scratch = items;
        let Some(c) = container else {
            return;
        };
        let Some(tag) = self.tree.specs[c].events().on_typeahead() else {
            return;
        };
        let data = Value::map([("text", Value::Str(self.type_ahead.clone()))]);
        out.push(UiEvent::declared(
            EventKind::Typeahead,
            self.tree.origins[c],
            self.tree.keys[c],
            Some(tag),
            data,
        ));
        self.typeahead_asked = Some(self.tree.keys[c]);
    }

    /// The app's answer to a `typeahead` event (ADR 0070): the data index
    /// of the row its search named, in the composite `key` that asked. The
    /// core goes there as a step does — onto the row when the frame built
    /// it (in a table, the cell in the column focus is in), else scrolling
    /// toward it and landing when the frame that builds it finishes. False
    /// when `key` asked nothing (an answer is taken once), or focus has
    /// left the composite since, which makes the answer stale.
    ///
    /// An answer given while a view is building — from a draw, which is
    /// where an extension with no env in its event handler answers — is
    /// taken (true) and applied when the frame ends, against the tree that
    /// frame built, as a reach is landed (backlog C104); it is dropped
    /// there if focus has left the composite by then.
    pub fn answer_typeahead(&mut self, key: Key, index: u64) -> bool {
        if self.typeahead_asked != Some(key) {
            return false;
        }
        self.typeahead_asked = None;
        if self.building {
            self.typeahead_answer = Some((key, index));
            return true;
        }
        self.apply_typeahead_answer(key, index)
    }

    /// An answer taken while the view built, applied now that the frame's
    /// tree is whole and laid out (backlog C104).
    pub(crate) fn resolve_typeahead_answer(&mut self) {
        if let Some((key, index)) = self.typeahead_answer.take() {
            self.apply_typeahead_answer(key, index);
        }
    }

    /// Goes to row `index` of composite `key`, if focus is still inside it.
    fn apply_typeahead_answer(&mut self, key: Key, index: u64) -> bool {
        let (Some(c), Some(f)) = (self.tree.index_of(key), self.focus_index()) else {
            return false;
        };
        let mut items = std::mem::take(&mut self.items_scratch);
        let owner = crate::composite::owner(&self.tree, f, &mut items);
        self.items_scratch = items;
        if owner != Some(c) {
            return false;
        }
        let column = (crate::composite::role(&self.tree, c) == Some(crate::access::Role::Table))
            .then(|| self.column_of(c, f));
        self.reach_index(c, f, index, column);
        true
    }

    // -- Actionable mode (ADR 0070) ------------------------------------
    // -- Item keys (backlog C120) ---------------------------------------
    // A composite hears the keys its focused item leaves: Delete on a
    // checklist's row. A sink cannot do it — one on the container makes
    // the container a Tab stop of its own, and one on an item takes the
    // composite's arrows — so the handler is the container's, and the
    // walk that finds it is the walk `enclosing_sink` makes, stopping at
    // a sink that is nearer.

    /// The composite whose `on_item_key` hears a key the focused node `i`
    /// does not claim, with the item it reports: `i` itself when it is an
    /// item, else the nearest item above it (a control inside a row). An
    /// item whose composite declares no handler passes the walk on to the
    /// composite around that one. None with a key sink nearer than the
    /// composite — that sink hears the key, as it always did — or with
    /// nothing that declared one; and a disabled or modal-shut composite
    /// hears nothing.
    pub(crate) fn item_key_owner(&self, i: usize) -> Option<(usize, usize)> {
        let mut scratch = Vec::new();
        let sink = |j: usize| self.tree.specs[j].events().on_key().is_some();
        let mut n = i;
        loop {
            if n != i && (sink(n) || !self.interactive(n)) {
                return None;
            }
            if let Some(c) = crate::composite::owner(&self.tree, n, &mut scratch)
                && self.tree.specs[c].events().on_item_key().is_some()
            {
                // A sink between the item and its container is nearer.
                let mut m = self.tree.parent[n];
                while m != crate::tree::NIL && m as usize != c {
                    if sink(m as usize) {
                        return None;
                    }
                    m = self.tree.parent[m as usize];
                }
                let spec = &self.tree.specs[c];
                return (!spec.disabled && self.interactive(c)).then_some((c, n));
            }
            n = match self.tree.parent[n] {
                crate::tree::NIL => return None,
                p => p as usize,
            };
        }
    }

    /// Where item `item` sits in the composite at `c`, for an `itemkey`:
    /// its data index (a table's row's) when one was declared, else its
    /// place among the composite's items — a table's among its rows — and,
    /// in a table, its column.
    pub(crate) fn item_place(&self, c: usize, item: usize) -> (u64, Option<u64>) {
        use crate::access::Role;
        let role = crate::composite::role(&self.tree, c);
        let table = role == Some(Role::Table);
        let unit = self.composite_unit(c, item);
        let column = table.then(|| self.column_of(c, item) as u64);
        if let Some(index) = crate::composite::data_index(&self.tree, unit) {
            return (index, column);
        }
        let Some(item_role) = role.and_then(crate::composite::item_role) else {
            return (0, column);
        };
        let mut items = Vec::new();
        crate::composite::items(&self.tree, c, item_role, &mut items);
        let place = if table {
            let mut rows: Vec<usize> = Vec::new();
            for &j in &items {
                let r = crate::composite::row_of(&self.tree, c, j);
                if !rows.contains(&r) {
                    rows.push(r);
                }
            }
            rows.iter().position(|&r| r == unit)
        } else {
            items.iter().position(|&j| j == item)
        };
        (place.unwrap_or(0) as u64, column)
    }

    // A table's cell can hold controls — a checkbox, a button, a field.
    // The cell is the composite's roving stop; its controls are out of the
    // ring (`focusable-inside-item` does not fire for them). Enter or F2
    // moves focus into the cell, Tab cycles among its controls, and Escape
    // brings focus back to the cell, where the arrows walk the table again.

    /// Whether node `c` is a cell of a composite table.
    fn is_table_cell(&self, c: usize) -> bool {
        crate::composite::role(&self.tree, c)
            .is_some_and(|r| crate::composite::is_item(r, crate::access::Role::Cell))
            && crate::composite::owner(&self.tree, c, &mut Vec::new()).is_some()
    }

    /// The controls inside the table cell `c`, in tree order — the
    /// focusable nodes of its subtree, the cell itself not among them.
    /// Empty for a node that is not such a cell.
    pub(crate) fn cell_controls(&self, c: usize) -> Vec<usize> {
        if !self.is_table_cell(c) {
            return Vec::new();
        }
        let end = self.tree.subtree_end(c);
        let mut out = Vec::new();
        let mut j = c + 1;
        while j < end {
            if self.tree.is_hidden(j) {
                j = self.tree.subtree_end(j);
                continue;
            }
            if crate::access::focusable(&self.tree, j) {
                out.push(j);
            }
            j += 1;
        }
        out
    }

    /// The one control a table cell `c` holds, when it holds exactly one
    /// that presses — not an editor, which is entered, never pressed.
    pub(crate) fn sole_control(&self, c: usize) -> Option<usize> {
        let controls = self.cell_controls(c);
        let editor = |j: usize| {
            matches!(self.tree.content[j], NodeContent::Edit(_))
                || crate::access::is_custom_editor(&self.tree, j)
        };
        match controls.as_slice() {
            [j] if !editor(*j) => Some(*j),
            _ => None,
        }
    }

    /// Enter (or, with `always`, F2) on the focused table cell `c`: its one
    /// control is pressed (Enter only), or focus moves to its first
    /// control. False for a cell with no controls, or a node that is no
    /// cell, which Enter then presses as it presses any control.
    pub(crate) fn enter_cell(&mut self, c: usize, always: bool, out: &mut Vec<UiEvent>) -> bool {
        let controls = self.cell_controls(c);
        let Some(&first) = controls.first() else {
            return false;
        };
        self.focus_visible = true;
        match self.sole_control(c) {
            Some(j) if !always => self.click_node(self.tree.keys[j], out),
            _ => self.land_focus(first),
        }
        true
    }

    /// The composite table cell focus is inside, strictly — focus on one of
    /// its controls.
    fn cell_around_focus(&self) -> Option<usize> {
        let f = self.focus_index()?;
        let mut n = self.tree.parent[f];
        while n != NIL {
            let j = n as usize;
            if self.is_table_cell(j) {
                return Some(j);
            }
            n = self.tree.parent[j];
        }
        None
    }

    /// Tab inside a table cell: the next (or, back, the previous) of the
    /// cell's controls, wrapping. False when focus is not inside a cell.
    pub(crate) fn cycle_in_cell(&mut self, forward: bool) -> bool {
        let Some(c) = self.cell_around_focus() else {
            return false;
        };
        let controls = self.cell_controls(c);
        let Some(at) = self
            .focus_index()
            .and_then(|f| controls.iter().position(|&j| j == f))
        else {
            return false;
        };
        let n = controls.len();
        let next = if forward {
            (at + 1) % n
        } else {
            (at + n - 1) % n
        };
        self.land_focus(controls[next]);
        true
    }

    /// Escape from a control inside a table cell: focus back on the cell.
    /// False when focus is not inside one.
    pub(crate) fn leave_cell(&mut self) -> bool {
        let Some(c) = self.cell_around_focus() else {
            return false;
        };
        self.land_focus(c);
        true
    }

    /// The column of item `j` of the composite at `container`: a table
    /// cell's place in its row, 0 for a list's item.
    pub(crate) fn column_of_entry(&self, container: usize, j: usize) -> usize {
        if self.composite_unit(container, j) == j {
            0
        } else {
            self.column_of(container, j)
        }
    }

    /// The column of the cell at `cell` in the table at `table`: its place
    /// among its row's cells.
    pub(crate) fn column_of(&self, table: usize, cell: usize) -> usize {
        let mut items = Vec::new();
        crate::composite::items(&self.tree, table, crate::access::Role::Cell, &mut items);
        let row = crate::composite::row_of(&self.tree, table, cell);
        items
            .iter()
            .filter(|&&j| crate::composite::row_of(&self.tree, table, j) == row)
            .position(|&j| j == cell)
            .unwrap_or(0)
    }

    /// Goes to data row `index` of the composite at `container`, from its
    /// built item `from`: onto the row when the frame has it (a table's
    /// cell nearest `column`), else a reach for it, as a step to an
    /// unbuilt row makes. An index past `rowCount` is the last row.
    pub(crate) fn reach_index(
        &mut self,
        container: usize,
        from: usize,
        index: u64,
        column: Option<usize>,
    ) {
        use crate::access::Role;
        let Some(role) = crate::composite::role(&self.tree, container) else {
            return;
        };
        let Some(item) = crate::composite::item_role(role) else {
            return;
        };
        let index = match crate::composite::row_count(&self.tree, from, container) {
            Some(n) => index.min(n.saturating_sub(1)),
            None => index,
        };
        let mut items = std::mem::take(&mut self.items_scratch);
        crate::composite::items(&self.tree, container, item, &mut items);
        let table = role == Role::Table;
        let unit_of = |j: usize| {
            if table {
                crate::composite::row_of(&self.tree, container, j)
            } else {
                j
            }
        };
        let mut units: Vec<usize> = Vec::new();
        for &j in &items {
            let u = unit_of(j);
            if units.last() != Some(&u) {
                units.push(u);
            }
        }
        let built = units
            .iter()
            .copied()
            .find(|&u| crate::composite::data_index(&self.tree, u) == Some(index));
        let landing = built.and_then(|u| {
            if table {
                let row: Vec<usize> = items.iter().copied().filter(|&j| unit_of(j) == u).collect();
                self.cell_at_column(&row, column.unwrap_or(0))
            } else {
                Some(u).filter(|&j| crate::access::focusable(&self.tree, j))
            }
        });
        let from_unit = unit_of(from);
        // The row is built but takes no focus — disabled, or a table row
        // with no live cell: the nearest built row that does, as an arrow
        // steps past it (backlog C132).
        let landing = match (landing, built) {
            (None, Some(_)) => {
                self.nearest_live(container, table, &items, &units, index, column.unwrap_or(0))
            }
            (l, _) => l,
        };
        self.items_scratch = items;
        match landing {
            Some(j) => {
                self.focus_visible = true;
                self.land_focus(j);
            }
            None if built.is_none() => {
                let toward = self.estimate_row(from_unit, &units, index);
                let reach = Reach {
                    container: self.tree.keys[container],
                    index,
                    column,
                    tries: 0,
                    since: 0,
                };
                self.reach_unbuilt(from_unit, reach, toward);
            }
            None => {}
        }
    }

    // -- Focus kept in a composite (backlog C125) ------------------------
    // A checklist's Delete removes the focused row. The row is gone the
    // next frame, and focus was left on a key no node has: the keyboard
    // dropped out of the list, and an app could not name the row before
    // it, items being keyed by index. So the core remembers where the
    // focused item sat, and the frame that no longer has it lands focus
    // on the item now at that place — else the one before it, as roving
    // focus does in ARIA practice — and `focus_item` names an item by its
    // index.

    /// The units of the composite at `c` whose items are `items`: a
    /// table's rows in order, a list's items themselves.
    fn units_of(&self, c: usize, table: bool, items: &[usize]) -> Vec<usize> {
        let mut units: Vec<usize> = Vec::new();
        for &j in items {
            let u = if table {
                crate::composite::row_of(&self.tree, c, j)
            } else {
                j
            };
            if units.last() != Some(&u) {
                units.push(u);
            }
        }
        units
    }

    /// Where focus lands on unit `u` of a composite: a list's item when
    /// it is live, a table's row's live cell nearest `column`.
    fn landing_in(
        &self,
        c: usize,
        table: bool,
        items: &[usize],
        u: usize,
        column: usize,
    ) -> Option<usize> {
        if table {
            let row: Vec<usize> = items
                .iter()
                .copied()
                .filter(|&j| crate::composite::row_of(&self.tree, c, j) == u)
                .collect();
            self.cell_at_column(&row, column)
        } else {
            Some(u).filter(|&j| crate::access::focusable(&self.tree, j))
        }
    }

    /// Where focus lands on the built unit nearest data row `index` that
    /// takes it — a list's live item, a table's row with a live cell — the
    /// row after first on a tie, as roving focus moves on past a removed
    /// one (backlog C132): a row asked for or recovered to that is disabled
    /// is stepped past, as an arrow steps past it.
    fn nearest_live(
        &self,
        c: usize,
        table: bool,
        items: &[usize],
        units: &[usize],
        index: u64,
        column: usize,
    ) -> Option<usize> {
        let mut order: Vec<(u64, bool, usize)> = units
            .iter()
            .filter_map(|&u| {
                let d = crate::composite::data_index(&self.tree, u)?;
                Some((d.abs_diff(index), d < index, u))
            })
            .collect();
        order.sort_unstable();
        order
            .into_iter()
            .find_map(|(_, _, u)| self.landing_in(c, table, items, u, column))
    }

    /// The composite at `c`'s role, whether it is a table, and its items,
    /// when it is a composite at all.
    fn composite_parts(&mut self, c: usize) -> Option<(bool, Vec<usize>)> {
        let role = crate::composite::role(&self.tree, c)?;
        let item = crate::composite::item_role(role)?;
        let mut items = std::mem::take(&mut self.items_scratch);
        crate::composite::items(&self.tree, c, item, &mut items);
        let now = items.clone();
        self.items_scratch = items;
        Some((role == crate::access::Role::Table, now))
    }

    /// Reads where the focused item sits in its composite, for
    /// [`Self::keep_focus_in_composite`] to land on if it vanishes: run
    /// when focus moves between frames and at each frame's end, against
    /// the tree the focus is on. Focus on a control inside an item (a
    /// table cell's checkbox) is placed by that item. None when focus is
    /// on no composite item, or on a node the tree does not have.
    pub(crate) fn note_focus_place(&mut self) {
        self.focus_place = None;
        let (Some(focus), Some(f)) = (self.focus, self.focus_index()) else {
            return;
        };
        let item_role = |n: usize| {
            crate::composite::role(&self.tree, n).is_some_and(|r| {
                crate::composite::PAIRS
                    .iter()
                    .any(|&(_, item)| crate::composite::is_item(r, item))
            })
        };
        let mut scratch = std::mem::take(&mut self.items_scratch);
        let mut n = f;
        let found = loop {
            if item_role(n)
                && let Some(c) = crate::composite::owner(&self.tree, n, &mut scratch)
            {
                break Some((c, n));
            }
            n = match self.tree.parent[n] {
                NIL => break None,
                p => p as usize,
            };
        };
        let place = found.and_then(|(c, item)| {
            // `owner` left the composite's items in `scratch`.
            let table = crate::composite::role(&self.tree, c) == Some(crate::access::Role::Table);
            let units = self.units_of(c, table, &scratch);
            let unit = self.composite_unit(c, item);
            let place = units.iter().position(|&u| u == unit)?;
            let index = crate::composite::row_count(&self.tree, unit, c)
                .and_then(|_| crate::composite::data_index(&self.tree, unit));
            Some(ItemPlace {
                item: focus,
                container: self.tree.keys[c],
                place,
                index,
                column: if table { self.column_of(c, item) } else { 0 },
            })
        });
        self.items_scratch = scratch;
        self.focus_place = place;
    }

    /// At a frame's end: when the composite item focus was on is gone
    /// from this frame and its composite is not, focus moves to the item
    /// now at its place — by data index in a windowed composite, reaching
    /// for the row when the frame did not build it — else the one before
    /// it, else the first. A composite gone or empty leaves focus as it
    /// was. The focus shows as it did: a removal is no new input.
    pub(crate) fn keep_focus_in_composite(&mut self) {
        let Some(place) = self.focus_place.take() else {
            return;
        };
        if self.focus != Some(place.item) || self.focus_index().is_some() {
            return;
        }
        let Some(c) = self.tree.index_of(place.container) else {
            return;
        };
        let Some((table, items)) = self.composite_parts(c) else {
            return;
        };
        let units = self.units_of(c, table, &items);
        let Some(&first) = units.first() else {
            return;
        };
        let visible = self.focus_visible;
        if let Some(index) = place.index
            && let Some(n) = crate::composite::row_count(&self.tree, first, c)
        {
            // A windowed composite keys its rows by data index, so a row
            // removed before the end leaves its key to the row after it,
            // and focus never left. A row still counted but not built is
            // only scrolled out of the window: focus stays on it, as ADR
            // 0070 has it, and pulling the scroller back would fight the
            // wheel. What is left is a row past the new end: the last row,
            // reached for when the frame did not build it.
            let built = units
                .iter()
                .any(|&u| crate::composite::data_index(&self.tree, u) == Some(index));
            if index < n && !built {
                return;
            }
            self.reach_index(c, items[0], index, table.then_some(place.column));
            if self.composite_reach.is_none() {
                self.focus_visible = visible;
            }
            return;
        }
        let at = place.place.min(units.len() - 1);
        let landing = (0..=at)
            .rev()
            .chain(at + 1..units.len())
            .find_map(|k| self.landing_in(c, table, &items, units[k], place.column));
        if let Some(j) = landing {
            self.land_focus(j);
            self.focus_visible = visible;
        }
    }

    /// Asks for focus on item `index` of the composite `key`, landed when
    /// the frame ends: the item whose data index is `index` — a row a
    /// windowed composite did not build is reached for, scrolled to and
    /// landed on the frame that builds it, an index past `rowCount` being
    /// the last row — else, in a composite whose items carry no index, the
    /// item at that place; a table's row lands on its first live cell.
    /// Nothing happens for a key that names no composite, an index with
    /// no live item, or a composite a modal shuts out. It is the app
    /// saying where focus goes, as `set_focus` is: it stands against a
    /// closing modal's restore, and the focus shows as the last input left
    /// it (backlog C125).
    pub fn focus_item(&mut self, key: Key, index: u64) {
        self.pending_item_focus = Some((key, index));
        self.owe_frame("focus_item");
    }

    /// Lands a `focus_item` asked for since the last frame's end, against
    /// the tree this frame built.
    pub(crate) fn resolve_item_focus(&mut self) {
        let Some((key, index)) = self.pending_item_focus.take() else {
            return;
        };
        let Some(c) = self.tree.index_of(key) else {
            return;
        };
        if !self.interactive(c) {
            return;
        }
        let Some((table, items)) = self.composite_parts(c) else {
            return;
        };
        if !crate::composite::is_composite(&self.tree, c, &items) {
            return;
        }
        let units = self.units_of(c, table, &items);
        let Some(&first) = units.first() else {
            return;
        };
        let visible = self.focus_visible;
        let before = self.focus;
        if crate::composite::row_count(&self.tree, first, c).is_some() {
            self.reach_index(c, items[0], index, table.then_some(0));
        } else {
            let indexed = |u: usize| crate::composite::data_index(&self.tree, u);
            let unit = match units.iter().copied().find(|&u| indexed(u) == Some(index)) {
                Some(u) => Some(u),
                None if units.iter().all(|&u| indexed(u).is_none()) => usize::try_from(index)
                    .ok()
                    .and_then(|k| units.get(k).copied()),
                None => None,
            };
            let landing = unit.and_then(|u| {
                self.landing_in(c, table, &items, u, 0).or_else(|| {
                    // A dead item at that place: the nearest live one
                    // (backlog C132).
                    let at = units.iter().position(|&x| x == u)?;
                    let mut order: Vec<(usize, bool, usize)> = units
                        .iter()
                        .enumerate()
                        .map(|(k, &x)| (k.abs_diff(at), k < at, x))
                        .collect();
                    order.sort_unstable();
                    order
                        .into_iter()
                        .find_map(|(_, _, x)| self.landing_in(c, table, &items, x, 0))
                })
            });
            if let Some(j) = landing {
                self.land_focus(j);
            }
        }
        if self.composite_reach.is_none() {
            self.focus_visible = visible;
        }
        if self.focus != before || self.composite_reach.is_some() {
            self.focus_asked = true;
        }
    }

    /// The item the search buffer, extended by `typed`, now names.
    fn type_ahead_target(
        &mut self,
        i: usize,
        typed: &str,
        items: &mut Vec<usize>,
    ) -> Option<(usize, crate::access::Role)> {
        let container = crate::composite::owner(&self.tree, i, items)?;
        let item = crate::composite::item_role(crate::composite::role(&self.tree, container)?)?;
        // With no clock every keystroke starts a fresh search: the buffer
        // has no way to age, and a stale one would be worse than none.
        let now = self.anim.time();
        if now.is_none() {
            self.type_ahead.clear();
        }
        self.type_ahead.push_str(&typed.to_lowercase());
        self.type_ahead_at = now;
        let live: Vec<usize> = items
            .iter()
            .copied()
            .filter(|&j| crate::access::focusable(&self.tree, j))
            .collect();
        let at = live.iter().position(|&j| j == i)?;
        let names = self.item_names(&live);
        let buf = self.type_ahead.clone();
        // From the item after the focused one, wrapping — so the focused
        // item is the last one tried, which is what makes a second
        // character refine the match instead of jumping off it.
        (1..=live.len()).find_map(|d| {
            let k = (at + d) % live.len();
            let name = names[k].as_deref()?;
            name.to_lowercase()
                .starts_with(&buf)
                .then_some((live[k], item))
        })
    }

    /// What a reader announces for each of `items`, in order — which is
    /// what type-ahead searches.
    ///
    /// Usually the access name, but not every item role has one: `tab`,
    /// `radio` and `menuItem` are named by the text inside them, while a
    /// `listItem` is a *container* of content and takes no name from it
    /// (giving a row a label as well would have it read twice). So a row
    /// falls back to the text a reader reads out for it — its own
    /// `staticText` descendants, in order — and a picker list is
    /// searchable without every row having to carry a `label`.
    fn item_names(&mut self, items: &[usize]) -> Vec<Option<String>> {
        let keys: Vec<Key> = items.iter().map(|&j| self.tree.keys[j]).collect();
        let tree = self.access_tree();
        keys.iter()
            .map(|k| {
                let at = tree.nodes.iter().position(|n| n.key == *k)?;
                if let Some(name) = &tree.nodes[at].name {
                    return Some(name.clone());
                }
                // The access tree is preorder, so a node's descendants are
                // the run that follows it.
                let mut inside = vec![*k];
                let mut text = String::new();
                for n in &tree.nodes[at + 1..] {
                    if !n.parent.is_some_and(|p| inside.contains(&p)) {
                        break;
                    }
                    inside.push(n.key);
                    if n.role == crate::access::Role::StaticText
                        && let Some(t) = &n.name
                    {
                        if !text.is_empty() {
                            text.push(' ');
                        }
                        text.push_str(t);
                    }
                }
                (!text.is_empty()).then_some(text)
            })
            .collect()
    }

    /// Clears a type-ahead buffer that has gone stale. Run at the start of
    /// a frame, where a frame with nothing typed pays one comparison, and
    /// before each keystroke, against the clock the driver stamped for it
    /// (backlog F139).
    pub(crate) fn age_type_ahead(&mut self) {
        if self.type_ahead.is_empty() {
            return;
        }
        if let (Some(now), Some(at)) = (self.anim.time(), self.type_ahead_at)
            && now - at > TYPE_AHEAD_SECS
        {
            self.type_ahead.clear();
            self.type_ahead_at = None;
        }
    }

    /// A slider move on node `i`. A slider that declared `on_change` has
    /// the core do the arithmetic: the move is
    /// worked from its declared `value_now`, range and step, and proposed
    /// as `{change: {value, phase: "end"}}` in the `onChange` message —
    /// nothing when it lands where the slider already is. A slider without
    /// `on_change` does not move and raises `slider-without-handler`, and
    /// a node that is not a slider is not nudged at all: an assistive
    /// increment reaches this for any node it names.
    pub(crate) fn nudge(
        &mut self,
        i: usize,
        mv: crate::slider::SliderMove,
        out: &mut Vec<UiEvent>,
    ) {
        use crate::slider::{SliderRange, change_event};
        let spec = &self.tree.specs[i];
        if spec.disabled || spec.access().role != Some(crate::access::Role::Slider) {
            return;
        }
        if let Some(tag) = spec.events().on_change() {
            let ax = spec.access();
            let Some(range) = SliderRange::of(ax) else {
                return;
            };
            let value = range.moved(ax.value_now, mv);
            if ax.value_now.map(crate::slider::exact) != Some(value) {
                out.push(change_event(
                    self.tree.origins[i],
                    self.tree.keys[i],
                    value,
                    "end",
                    tag,
                ));
            }
            return;
        }
        // A slider with no `onChange` is read-only to the keyboard and to
        // assistive technology alike: there is no message to propose a
        // value in (ADR 0047, decision 5).
        self.note_slider_without_handler(i);
    }

    /// Raises [`crate::diag::SLIDER_WITHOUT_HANDLER`] for slider `i`.
    fn note_slider_without_handler(&mut self, i: usize) {
        let key = self.tree.keys[i];
        self.warn(crate::diag::slider_without_handler(key));
    }

    /// A reader's `SetValue` on slider `i`: `value` as the number it
    /// asked for. Windows' UI Automation moves a slider
    /// only this way — its RangeValue pattern has no increment — and the
    /// request was dropped, so a UIA client read a writable slider whose
    /// writes did nothing. With `on_change` the core proposes `value`
    /// snapped to the step and clamped, as the pointer does; without, it
    /// moves nothing, as the increments do not. Text that is not a number — the
    /// Value pattern handing over a `value_text` such as "25 minutes" —
    /// is nothing.
    pub(crate) fn set_slider(&mut self, i: usize, value: &str, out: &mut Vec<UiEvent>) {
        use crate::slider::{SliderRange, change_event};
        let spec = &self.tree.specs[i];
        if spec.disabled || spec.access().role != Some(crate::access::Role::Slider) {
            return;
        }
        let Some(asked) = value.trim().parse::<f64>().ok().filter(|v| v.is_finite()) else {
            return;
        };
        if let Some(tag) = spec.events().on_change() {
            let ax = spec.access();
            let Some(range) = SliderRange::of(ax) else {
                return;
            };
            let value = range.snap(asked);
            if ax.value_now.map(crate::slider::exact) != Some(value) {
                out.push(change_event(
                    self.tree.origins[i],
                    self.tree.keys[i],
                    value,
                    "end",
                    tag,
                ));
            }
            return;
        }
        self.note_slider_without_handler(i);
    }
}

/// Where the focused composite item sat (backlog C125): the focused key,
/// its composite, its unit's place among the composite's units (a table's
/// rows, a list's items), the unit's data index when the composite is
/// windowed, and the column in a table.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ItemPlace {
    pub item: Key,
    pub container: Key,
    pub place: usize,
    pub index: Option<u64>,
    pub column: usize,
}

/// Where a composite step goes: onto a built item, or reaching for a row
/// a windowed list or table has not built, toward where it would be.
enum Target {
    Land(usize, crate::access::Role),
    Reach(Reach, Rect),
}

/// A composite step to a row the frame did not build (ADR 0070, backlog
/// C89): the composite, the row's data index, and for a table the column
/// to land in (`usize::MAX` for the row's last). Held from the key to the
/// frame that builds the row; focus moving any other way lets it go.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Reach {
    pub container: Key,
    pub index: u64,
    pub column: Option<usize>,
    pub tries: u8,
    /// The frame number current when the reach began: a reach begun at a
    /// frame's end (a `focus_item`, a removed item's focus, a type-ahead
    /// answer) has scrolled already, and is landed by the frame it owes,
    /// not reached for twice in the one it began in (backlog C125).
    pub since: u64,
}
