//! Props by condition at run time (ADR 0069): a node's `when` entries
//! resolved as it opens, what each node matched kept across frames, the
//! `query` events, and the lazy branches' decisions.
//!
//! The window, environment and state entries resolve in `prepare_spec`,
//! after the `hoverBg` / `pressedBg` / `focusBg` shorthands — which are
//! entries ahead of the list, in that order — and before the transition
//! eases the spec, so an entry's props tween as a state background does.
//! The entries over the node's own size are left for layout
//! (`rules::settle_axis`), which starts from what they matched last frame.

use super::*;
use crate::rules::{Base, Facts, Platform, RulesSpec, field, state};

/// What one node matched, kept by key across frames: each window and
/// own-size range's last answer (for the band), the entries it matched,
/// and the frame it was last seen.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct RuleMemo {
    win_w: u64,
    win_h: u64,
    held_w: u64,
    held_h: u64,
    static_ok: u64,
    /// The own-size and window entries matched, as `query` reported them.
    reported: u64,
    /// Every entry matched.
    matched: u64,
    seen: u64,
}

/// What a lazy branch decided, kept by key: the room the last layout
/// offered and the parent's size there, the query's range answers, and
/// whether it built `render`.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct LazyMemo {
    pub room: Size,
    pub parent: Size,
    pub held: u8,
    pub decided: bool,
    pub seen: u64,
}

impl Core {
    /// The facts a condition other than the node's own size reads, with
    /// `state` the node's state bits.
    pub(crate) fn rule_facts(&self, state: u8) -> Facts {
        Facts {
            viewport: Size::new(self.dt_area.w, self.dt_area.h),
            scale: self.scale,
            dark: self.theme.appearance == crate::env::Appearance::Dark,
            reduced_motion: self.env.system.motion.is_reduced(),
            contrast: self.env.system.contrast,
            platform: Platform::current(),
            state,
        }
    }

    /// Node `key`'s state bits, as `hoverBg` reads them: hovered is the
    /// pointer over it or anything inside it, or over its hover group, or
    /// a press it holds.
    fn state_bits(&self, key: Key, spec: &NodeSpec) -> u8 {
        let group = spec.interact().hover_group;
        let pressed = self.interaction.is_pressed(key)
            || group.is_some_and(|g| self.interaction.is_group_pressed(g));
        let hovered = pressed
            || self.interaction.is_hovered(key)
            || self.hover_chain.contains(&key)
            || group.is_some_and(|g| self.interaction.is_group_hovered(g));
        let focused = self.focus == Some(key);
        let mut s = 0;
        if hovered {
            s |= state::HOVERED;
        }
        if pressed {
            s |= state::PRESSED;
        }
        if focused {
            s |= state::FOCUSED;
        }
        if focused && self.focus_visible {
            s |= state::FOCUS_VISIBLE;
        }
        if spec.disabled {
            s |= state::DISABLED;
        }
        if spec.access().selected {
            s |= state::SELECTED;
        }
        s
    }

    /// The node's `when` entries, resolved for this frame. One null check
    /// for a node that declares none.
    #[inline]
    pub(super) fn resolve_rules(&mut self, key: Key, spec: &mut NodeSpec) {
        if spec.rules.is_some() {
            self.resolve_declared_rules(key, spec);
        }
    }

    #[inline(never)]
    fn resolve_declared_rules(&mut self, key: Key, spec: &mut NodeSpec) {
        let Some(mut rules) = spec.rules.take() else {
            return;
        };
        let memo = self
            .rule_memo
            .get(&key)
            .copied()
            .filter(|m| m.seen + 1 >= self.frame_no)
            .unwrap_or_default();
        let reads_state = rules.entries().iter().any(|r| r.cond.reads_state());
        let facts = self.rule_facts(if reads_state {
            self.state_bits(key, spec)
        } else {
            0
        });
        for (k, rule) in rules.entries().iter().enumerate() {
            if !rule.refused.is_empty() {
                let refused = rule.refused.join(", ");
                self.diag
                    .raise_once(crate::diag::RULE_PROP_REFUSED, key, || {
                        format!(
                            "`when` entry {} ({}) names what it may not set, left out: {refused}",
                            k,
                            rule.cond.describe()
                        )
                    });
            }
        }
        // A node that reads its own size in an axis is a container there
        // (decision 8): its size there cannot come from its content.
        if rules.size_w != 0 && spec.layout.width == Sizing::Fit {
            spec.layout.width = Sizing::Grow(1.0);
            self.query_on_fit(key, "width");
        }
        if rules.size_h != 0 && spec.layout.height == Sizing::Fit {
            spec.layout.height = Sizing::Grow(1.0);
            self.query_on_fit(key, "height");
        }
        let base = Base::of_parts(spec, &rules);
        let sized = rules.sized();
        let (mut win_w, mut win_h, mut static_ok) = (memo.win_w, memo.win_h, 0u64);
        for (k, rule) in rules.entries().iter().enumerate() {
            let bit = 1u64 << k;
            let mut held = ((win_w >> k) & 1) as u8 | ((((win_h >> k) & 1) as u8) << 1);
            if rule.cond.holds_static(&facts, &mut held) {
                static_ok |= bit;
            }
            win_w = (win_w & !bit) | (u64::from(held & 1) << k);
            win_h = (win_h & !bit) | (u64::from((held >> 1) & 1) << k);
        }
        // Every entry that matches, in list order: the ones over the
        // node's own size are laid by layout, the rest here.
        for (k, rule) in rules.entries().iter().enumerate() {
            let bit = 1u64 << k;
            if static_ok & bit != 0 && sized & bit == 0 {
                rule.patch.apply(spec, !field::MIN_COLUMN);
            }
        }
        let mut min_column = base.min_column();
        for (k, rule) in rules.entries().iter().enumerate() {
            if static_ok & (1 << k) != 0
                && sized & (1 << k) == 0
                && let Some(n) = rule.patch.min_column()
            {
                min_column = n;
            }
        }
        rules.min_column = min_column;
        let mut hide = Vec::new();
        for (k, rule) in rules.entries().iter().enumerate() {
            for label in rule.patch.hides() {
                hide.push((k as u8, key.str(label)));
            }
        }
        let changed = memo.seen != 0 && memo.static_ok != static_ok;
        let live = &mut rules.live;
        live.static_ok = static_ok;
        live.win_w = win_w;
        live.win_h = win_h;
        live.held_w = memo.held_w;
        live.held_h = memo.held_h;
        live.base = base;
        live.hide = hide;
        live.height_moved = false;
        rules.live.matched = rules.matching();
        spec.rules = Some(rules);
        if sized != 0 {
            RulesSpec::relay(spec);
        }
        // Under reduced motion a match change jumps (decision 10).
        if changed
            && facts.reduced_motion
            && let Some(t) = spec.transition.as_mut()
        {
            t.duration_ms = 0.0;
        }
    }

    fn query_on_fit(&mut self, key: Key, axis: &'static str) {
        self.diag.raise_once(crate::diag::QUERY_ON_FIT, key, || {
            format!(
                "this node reads its own {axis} but sizes its {axis} to its content (`fit`), \
                 which would loop; it takes the room its parent offers instead, as `grow` \
                 would — give it a {axis} that is not `fit`"
            )
        });
    }

    /// After layout: what each node with entries matched, kept for the
    /// next frame, and a `query` event where its own-size and window
    /// matches moved.
    pub(super) fn settle_rules(&mut self) {
        let frame_no = self.frame_no;
        let ruled = std::mem::take(&mut self.tree.ruled);
        for &i in &ruled {
            let i = i as usize;
            let key = self.tree.keys[i];
            let Some(rules) = self.tree.specs[i].rules.as_deref() else {
                continue;
            };
            if rules.entries().is_empty() {
                continue;
            }
            let live = &rules.live;
            let reads = rules.sized() | rules.window;
            let reported = live.matched & reads;
            let last = self.rule_memo.get(&key).copied();
            let fresh = last.is_none_or(|m| m.seen + 1 < frame_no);
            self.rule_memo.insert(
                key,
                RuleMemo {
                    win_w: live.win_w,
                    win_h: live.win_h,
                    held_w: live.held_w,
                    held_h: live.held_h,
                    static_ok: live.static_ok,
                    reported,
                    matched: live.matched,
                    seen: frame_no,
                },
            );
            let moved = fresh || last.is_some_and(|m| m.reported != reported);
            if reads == 0 || !moved {
                continue;
            }
            let Some(tag) = self.tree.specs[i].events().on_query().cloned() else {
                continue;
            };
            let matched: Vec<Value> = (0..rules.entries().len())
                .filter(|k| live.matched & (1 << k) != 0)
                .map(|k| Value::Int(k as i64))
                .collect();
            let size = self.tree.size[i];
            let data = Value::map([
                ("matched", Value::List(matched)),
                ("width", Value::Float(size.w as f64)),
                ("height", Value::Float(size.h as f64)),
            ]);
            self.pending.push(UiEvent::declared(
                EventKind::Query,
                self.tree.origins[i],
                key,
                Some(&tag),
                data,
            ));
        }
        self.tree.ruled = ruled;
        self.settle_lazy();
        if let Some(cutoff) = crate::retain::sweep_cutoff(frame_no) {
            self.rule_memo.retain(|_, m| m.seen >= cutoff);
            self.lazy_memo.retain(|_, m| m.seen >= cutoff);
        }
    }

    /// After layout: each lazy branch's room, the decision it gives, and
    /// whether that is the branch the view built — a crossing asks the
    /// runner to build the frame again before it draws it (decision 15),
    /// once; a second in the same frame waits for the next and warns.
    fn settle_lazy(&mut self) {
        let frame_no = self.frame_no;
        let facts = self.rule_facts(0);
        let mut crossed = None;
        for k in 0..self.tree.ruled.len() {
            let i = self.tree.ruled[k] as usize;
            let Some(lazy) = self.tree.specs[i].rules.as_deref().and_then(|r| r.lazy()) else {
                continue;
            };
            let lazy = lazy.clone();
            let key = self.tree.keys[i];
            let p = self.tree.parent[i];
            let (room, parent) = if p == NIL {
                (self.viewport, self.viewport)
            } else {
                (self.lazy_room(i, p as usize), self.tree.size[p as usize])
            };
            let last = self
                .lazy_memo
                .get(&key)
                .copied()
                .filter(|m| m.seen + 1 >= frame_no);
            let mut held = last.map_or(0, |m| m.held);
            let decided = lazy.decide(room, parent, &facts, &mut held);
            self.lazy_memo.insert(
                key,
                LazyMemo {
                    room,
                    parent,
                    held,
                    decided,
                    seen: frame_no,
                },
            );
            if lazy.built != Some(decided) {
                crossed.get_or_insert(key);
            }
            let tag = lazy
                .on_query
                .clone()
                .or_else(|| self.tree.specs[i].events().on_query().cloned());
            if let Some(tag) = tag
                && last.is_none_or(|m| m.decided != decided)
            {
                let matched = if decided {
                    vec![Value::Int(0)]
                } else {
                    Vec::new()
                };
                let data = Value::map([
                    ("matched", Value::List(matched)),
                    ("width", Value::Float(room.w as f64)),
                    ("height", Value::Float(room.h as f64)),
                ]);
                self.pending.push(UiEvent::declared(
                    EventKind::Query,
                    self.tree.origins[i],
                    key,
                    Some(&tag),
                    data,
                ));
            }
        }
        // What the next build of a binding that decides for itself reads.
        let hosted: Vec<Option<bool>> = self
            .lazy_hosted
            .iter()
            .map(|k| self.lazy_memo.get(k).map(|m| m.decided))
            .collect();
        self.lazy_hosted_choices = hosted;
        let Some(key) = crossed else {
            return;
        };
        if self.in_rerun {
            self.diag.raise_once(crate::diag::QUERY_UNSETTLED, key, || {
                "this lazy branch's decision disagreed with the layout again after the \
                     frame was built a second time: its room depends on itself; the next frame \
                     decides again"
                    .into()
            });
        } else {
            self.rerun_owed = true;
        }
        self.owe_frame("lazy");
    }

    /// The room a lazy branch's parent `p` offers branch `i`: its content
    /// box less its other in-flow children and the gaps the branch would
    /// add, along its main axis — a child that grows counted at its floor,
    /// so the room never depends on what the branch itself took — and its
    /// content box across.
    fn lazy_room(&self, i: usize, p: usize) -> Size {
        let tree = &self.tree;
        let l = &tree.specs[p].layout;
        let row = l.dir == crate::spec::Dir::Row;
        let content = Size::new(
            (tree.size[p].w - l.padding.x()).max(0.0),
            (tree.size[p].h - l.padding.y()).max(0.0),
        );
        let (mut used, mut n) = (0.0f32, 0u32);
        let mut c = tree.first_child[p];
        while c != NIL {
            let ci = c as usize;
            let out = tree.is_hidden(ci) || tree.specs[ci].layout.float.is_some();
            if ci != i && !out {
                let cl = &tree.specs[ci].layout;
                let (sizing, min, laid) = if row {
                    (cl.width, cl.min_w, tree.size[ci].w)
                } else {
                    (cl.height, cl.min_h, tree.size[ci].h)
                };
                used += match sizing {
                    Sizing::Grow(_) => min.resolved(),
                    _ => laid,
                };
                n += 1;
            }
            c = tree.next_sibling[ci];
        }
        let main = ((if row { content.w } else { content.h }) - used - l.gap * n as f32).max(0.0);
        if row {
            Size::new(main, content.h)
        } else {
            Size::new(content.w, main)
        }
    }

    /// Whether a lazy branch's decision disagreed with this frame's
    /// layout (ADR 0069, decision 15): the runner builds the frame once
    /// more — `begin_frame`, the view, `finish` — before it draws it, and
    /// the branch is built from this layout's room. A runner that does
    /// not ask gets the corrected frame next, which the core asks for.
    pub fn rerun_owed(&self) -> bool {
        self.rerun_owed
    }

    /// The entries node `key` matched in the last finished frame, as a bit
    /// set over its `when` list; 0 for a node with none.
    pub fn matched_rules(&self, key: Key) -> u64 {
        self.rule_memo.get(&key).map_or(0, |m| m.matched)
    }
}
