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

    /// One arrow, Home or End step inside the composite holding node `i`.
    /// Nothing happens when `i` is not a composite item, or when the step
    /// runs off the end of one that clamps.
    pub(crate) fn composite_step(&mut self, i: usize, ek: EditKey, out: &mut Vec<UiEvent>) {
        let mut items = std::mem::take(&mut self.items_scratch);
        let target = self.composite_target(i, ek, &mut items);
        self.items_scratch = items;
        if let Some((j, item)) = target {
            self.move_within_composite(i, j, item, out);
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

    /// Where a step from item `i` lands, with the item role it lands on.
    /// Fills `items` with the composite's items on the way (see
    /// `composite::items`).
    fn composite_target(
        &self,
        i: usize,
        ek: EditKey,
        items: &mut Vec<usize>,
    ) -> Option<(usize, crate::access::Role)> {
        let container = crate::composite::owner(&self.tree, i, items)?;
        let role = self.tree.specs[container].access().role?;
        let item = crate::composite::item_role(role)?;
        // A disabled item is not an item here, as it is not in the ring —
        // it keeps its ordinal in "3 of 7" and the arrows step over it.
        let live: Vec<usize> = items
            .iter()
            .copied()
            .filter(|&j| crate::access::focusable(&self.tree, j))
            .collect();
        let at = live.iter().position(|&j| j == i)?;
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
            _ => return None,
        };
        Some((j, item))
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
            // than starting over.
            None => !self.type_ahead.is_empty(),
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
        let item = crate::composite::item_role(self.tree.specs[container].access().role?)?;
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
    /// as `{kind="change", value, phase="end", tag}` — nothing when it
    /// lands where the slider already is. Any other slider reaches the app
    /// as `{kind="access", action, tag}`, since the core cannot know what
    /// a step means there, and only for a single step: its Page, Home and
    /// End keys were never its own.
    pub(crate) fn nudge(
        &mut self,
        i: usize,
        mv: crate::slider::SliderMove,
        out: &mut Vec<UiEvent>,
    ) {
        use crate::slider::{SliderMove, SliderRange, change_event};
        let spec = &self.tree.specs[i];
        if spec.disabled {
            return;
        }
        if let Some(tag) = spec.events().on_change.as_ref() {
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
        let action = match mv {
            SliderMove::Step(n) if n > 0 => crate::access::AccessAction::Increment,
            SliderMove::Step(_) => crate::access::AccessAction::Decrement,
            _ => return,
        };
        let payload = Value::map([
            ("kind", Value::str("access")),
            ("action", Value::str(action.name())),
        ]);
        out.push(
            UiEvent::on(self.tree.origins[i], self.tree.keys[i], payload)
                .tagged(self.access_tag(i).as_ref()),
        );
    }

    /// A reader's `SetValue` on slider `i`: `value` as the number it
    /// asked for. Windows' UI Automation moves a slider
    /// only this way — its RangeValue pattern has no increment — and the
    /// request was dropped, so a UIA client read a writable slider whose
    /// writes did nothing. With `on_change` the core proposes `value`
    /// snapped to the step and clamped, as the pointer does; without, the
    /// app hears `{kind="access", action="setValue", value}` beside the
    /// increments it already hears. Text that is not a number — the
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
        if let Some(tag) = spec.events().on_change.as_ref() {
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
        let payload = Value::map([
            ("kind", Value::str("access")),
            (
                "action",
                Value::str(crate::access::AccessAction::SetValue.name()),
            ),
            ("value", Value::Float(asked)),
        ]);
        out.push(
            UiEvent::on(self.tree.origins[i], self.tree.keys[i], payload)
                .tagged(self.access_tag(i).as_ref()),
        );
    }
}
