//! The frame builder: what a view calls between `begin_frame` and
//! `finish_frame` to declare the tree — open, close, text, editors,
//! images — with each spec eased against its transitions and keyframes
//! as it is opened, and the hover / press queries a view styles by.

use super::*;

use crate::schema::{Identity, PropsOut};
use crate::slots::one;

/// What a node opened through [`Core::open_from`] holds: a box (left open
/// for its children), a fragment (likewise), a `cells` grid or a stroke
/// (leaves, closed by the door).
pub enum Content<'a> {
    Box,
    /// A fragment: the function (and its image, if any) and the params.
    Fragment(crate::fragment::FragmentRef, &'a [f32]),
    Cells(&'a crate::cells::CellGrid<'a>),
    Line(&'a [Vec2], Stroke),
    /// A filled polygon (ADR 0025, decision 6); the fill is the spec's `bg`.
    Polygon(&'a [Vec2]),
}

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
        // Each slot's stops in lanes, the node's own value at the ends the
        // stops do not reach; a sizing with no amount (`fit`) has no track.
        let track = |slot: Slot, base: Option<[f32; 4]>| {
            keyframes::track(frames, &offsets, base?, |k| k.lanes(slot))
        };
        Tracks {
            width: track(Slot::Width, spec.layout.width.amount().map(one)),
            height: track(Slot::Height, spec.layout.height.amount().map(one)),
            bg: track(Slot::Bg, Some(spec.style.bg.lanes())),
            radius: track(Slot::Radius, Some(spec.style.radius)),
            opacity: track(Slot::Opacity, Some(one(spec.style.opacity))),
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
        if let Some(app) = self.dt_app {
            // The host's tree is wrapped for the devtools (ADR 0024,
            // decision 2): the spec is split between the two nodes.
            self.devtools_configure_root(app, spec);
            return;
        }
        if !self.tree.is_empty() {
            self.ease_spec(Key::ROOT, &mut spec);
            self.tree.note(&spec, &NodeContent::Container);
            self.tree.specs[0] = spec;
        }
    }

    /// Replaces a transitioning node's animatable values with this frame's
    /// eased ones. Nodes without a transition cost one branch — inlined at
    /// the call site, so it is a branch and not a call that returns (C15).
    /// A slot the node's keyframes name is sampled from its cycle instead
    /// of tweened.
    #[inline]
    pub(super) fn ease_spec(&mut self, key: Key, spec: &mut NodeSpec) {
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
        // Each slot: sampled from its track when keyframed, else tweened
        // toward its declared value from where the entrance says it starts.
        let mut ease = |slot: Slot, target: [f32; 4]| match track(slot) {
            Some(track) => anim.sample(track, t).unwrap_or(target),
            None => anim.drive(slot, enter.lanes(slot), target, t, true),
        };
        let mut sizing = |slot: Slot, s: Sizing| match s.amount() {
            Some(v) => s.with_amount(ease(slot, one(v))[0]),
            None => s,
        };
        spec.layout.width = sizing(Slot::Width, spec.layout.width);
        spec.layout.height = sizing(Slot::Height, spec.layout.height);
        let mut color = |slot: Slot, c: Color| Color::from_lanes(ease(slot, c.lanes()));
        spec.style.bg = color(Slot::Bg, spec.style.bg);
        spec.style.border_color = color(Slot::Border, spec.style.border_color);
        spec.style.shadow.color = color(Slot::ShadowColor, spec.style.shadow.color);
        let sh = spec.style.shadow;
        let geom = ease(Slot::Shadow, [sh.dx, sh.dy, sh.blur, sh.spread]);
        spec.style.shadow.dx = geom[0];
        spec.style.shadow.dy = geom[1];
        spec.style.shadow.blur = geom[2].max(0.0);
        spec.style.shadow.spread = geom[3];
        spec.style.opacity = ease(Slot::Opacity, one(spec.style.opacity))[0].clamp(0.0, 1.0);
        spec.style.radius = ease(Slot::Radius, spec.style.radius);
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
            // Outside a frame — or in the devtools' own window, where the
            // root is deferred (ADR 0024, decision 6) — there is no node
            // to derive from, and the open that follows is a no-op.
            self.tree
                .keys
                .get(self.current() as usize)
                .copied()
                .unwrap_or(Key::ROOT)
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
        self.devtools_consume(&mut out);
        self.devtools_translate(&mut out);
        self.stamp(&mut out);
        self.devtools_log(&out);
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

    /// Where the pointer is, in this window's logical viewport
    /// coordinates, as of the last `CursorMoved` — `None` once it has
    /// left the window. What a view that follows the pointer reads (the
    /// devtools' picker outlines the node under it); a control that wants
    /// to *react* to the pointer declares `hoverable` or `on_hover` and
    /// lets the core do the hit test.
    pub fn cursor(&self) -> Option<Vec2> {
        self.interaction.cursor().map(|p| p.minus(self.dt_shift()))
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
        if self.tree.is_empty() {
            // No frame to open into (the same no-op as `open_with_key`),
            // and so no node for the label to name.
            return key;
        }
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
    /// The inverse of [`Self::key_of`]: the label `key` was opened under
    /// — in the frame being built so far, else in the last one — or
    /// `None` for an auto-keyed node or a key no frame has declared. What
    /// a reader holding a key from an event or from `focus()` turns back
    /// into the name the view gave it.
    pub fn label_of(&self, key: Key) -> Option<&str> {
        self.key_labels
            .label_of(key)
            .or_else(|| self.key_labels_last.label_of(key))
    }

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

    /// Declares how many indexed rows the *open* node's virtual list has,
    /// built or not (`rowCount`): what Select All inside a `selectable`
    /// virtual list spans, since the built rows are all the core can see
    /// (ADR 0017, tier 3). `widgets::virtual_column` and `virtual_rows`
    /// call it on their container; a list composed by hand calls it
    /// inside the container's `with`. Nothing, outside any node.
    pub fn row_count(&mut self, n: u64) {
        if self.tree.is_empty() || self.stack.is_empty() {
            return;
        }
        let at = self.current();
        self.tree.row_counts.push((at, n));
    }

    #[inline]
    pub(crate) fn open_with_key(&mut self, key: Key, spec: NodeSpec) {
        self.open_content(key, spec, NodeContent::Container);
    }

    /// `open_with_key` for a node that is a box in every way but what it
    /// paints: the caller supplies the content and closes the node. What
    /// the node asks of the frame is noted by `Tree::push`, the same for a
    /// box, a `fragment` and every leaf.
    fn open_content(&mut self, key: Key, mut spec: NodeSpec, content: NodeContent) {
        if self.tree.is_empty() {
            return;
        }
        self.prepare_spec(key, &mut spec);
        let parent = self.current();
        let idx = self.tree.push(parent, key, self.origin, spec, content);
        self.stack.push(idx);
        self.counters.push(0);
    }

    /// What every node's spec goes through between the door and the tree,
    /// in this order — one pipeline for a box, a leaf, a stroke and a
    /// fill alike (AR16: five doors ran five subsets of it, and a wedge's
    /// `hover_bg` never painted). The one paint the environment decides
    /// (`accent`): the theme's accent, which is the OS's where the host
    /// reported one, the app's where it pinned one, and kui's otherwise
    /// (ADR 0019 — before it, this read `env.system.accent` and a host
    /// that reported none left the declared `bg`, which is the same answer
    /// by a shorter route); before the hover resolution, so a node that
    /// declares both still hovers to what it declared. Then the hover /
    /// pressed / focus background for the node's state, then the eased
    /// values a transition, entrance or keyframes put over the declared
    /// ones.
    #[inline]
    fn prepare_spec(&mut self, key: Key, spec: &mut NodeSpec) {
        if spec.accent && self.has_accent() {
            spec.style.bg = self.theme.accent;
        }
        self.resolve_hover_style(key, spec);
        self.ease_spec(key, spec);
    }

    /// The layout of a node placed by its own geometry — a stroke, a fill
    /// (ADR 0010 decision 5): never in layout, a float at `rect` in the
    /// parent's box space sized exactly to it, the declared float's
    /// *anchor* kept and every sizing, clamp and scroll row overridden,
    /// since the box is the shape's own and not a size the view chose or
    /// a tween may lag.
    fn float_box_for(spec: &mut NodeSpec, rect: Rect) {
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
    }

    #[inline]
    pub fn close(&mut self) {
        // The tooltip prop's third effect, for the node being closed: its
        // hint floats below it as its last child while it is hovered. One
        // length check per close for a frame that declared no hints.
        if let Some((depth, _, _)) = self.hints.last()
            && *depth == self.stack.len()
        {
            let (_, key, hint) = self.hints.pop().unwrap();
            if self.is_hovered(key) {
                crate::widgets::tooltip(&mut Ui::wrap(self), &hint);
            }
        }
        if self.stack.len() > 1 {
            self.stack.pop();
            self.counters.pop();
        }
    }

    /// Records the hover hint of the node just opened (the top of the
    /// stack): `close` floats `widgets::tooltip` below it while it is
    /// hovered. The one place that decides *when* a tooltip shows, so a
    /// binding that parsed the string cannot show it some other way.
    pub fn hint(&mut self, key: Key, text: impl Into<String>) {
        self.hints.push((self.stack.len(), key, text.into()));
    }

    /// Opens a node the way a parsed prop list says — under the data
    /// index, the label or the next auto key; taking keyboard focus when
    /// `keyFocus` asked; floating its `tooltip` on `close` while hovered —
    /// with whatever the node holds. The one door for every binding that
    /// lowers props, so the identity match, the focus edge and the hint
    /// are not re-derived per binding per element (they were, eight, five
    /// and four times). A box or a fragment is left open for its children;
    /// a `cells` grid, a `line` and a `polygon` are leaves and take no
    /// hint, since a stroke and a fill take no input and a grid draws its
    /// own. Returns the key.
    pub fn open_from(&mut self, props: PropsOut, content: Content<'_>) -> Key {
        let PropsOut {
            spec,
            key: label,
            index,
            row_count,
            key_focus,
            tooltip,
            ..
        } = props;
        let identity = match (index, &label) {
            (Some(i), _) => Identity::Index(i),
            (None, Some(label)) => Identity::Label(label),
            (None, None) => Identity::Auto,
        };
        let key = match identity {
            Identity::Auto => self.auto_key(),
            Identity::Label(l) => self.child_key(l),
            Identity::Index(i) => self.child_key_index(i),
        };
        let at = self.tree.len() as u32;
        let leaf = match content {
            Content::Box => {
                self.open_with_key(key, spec);
                false
            }
            Content::Fragment(frag, params) => {
                self.fragment_with_key(key, frag, params, spec);
                false
            }
            Content::Cells(grid) => {
                self.cells_at(key, grid, spec);
                true
            }
            Content::Line(points, stroke) => {
                self.line_with_key(key, points, stroke, spec);
                true
            }
            Content::Polygon(points) => {
                self.polygon_with_key(key, points, spec);
                true
            }
        };
        // Bookkeeping for the node that was actually pushed: the label
        // `key_of` resolves through, or the data index a selection inside
        // a virtual row is ordered by when the row is not built (ADR 0017).
        if self.tree.len() as u32 > at {
            match identity {
                Identity::Label(l) => self.key_labels.push(key, l),
                Identity::Index(i) => self.tree.indexed.push((at, i)),
                Identity::Auto => {}
            }
            if let Some(n) = row_count {
                self.tree.row_counts.push((at, n));
            }
        }
        if key_focus {
            self.set_key_focus(Some(key));
        }
        if let Some(hint) = tooltip
            && !leaf
        {
            self.hint(key, hint);
        }
        key
    }

    /// The root the way a parsed prop list says: its title, whether it
    /// wants the window on top, the windows it declares, its spec, and
    /// keyboard focus on it when asked — what a binding's root op does,
    /// once.
    pub fn configure_root_from(&mut self, props: PropsOut) {
        if let Some(title) = &props.title {
            self.set_window_title(title);
        }
        if props.always_on_top {
            self.set_always_on_top(true);
        }
        for (name, cfg) in &props.windows {
            self.declare_window(name, *cfg);
        }
        self.configure_root(props.spec);
        if props.key_focus {
            let root = self.root_key();
            self.set_key_focus(Some(root));
        }
    }

    pub fn text_node(&mut self, content: &str, style: TextStyle) {
        if self.tree.is_empty() {
            return;
        }
        // A style that named no colour takes the theme's foreground here,
        // at the one door text comes through, so the shaping cache, the
        // display list and every binding downstream see a real colour
        // (ADR 0019).
        let style = style.or_fg(self.theme.fg);
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
        if self.tree.is_empty() {
            return;
        }
        let key = self.child_key(label);
        self.cells_at(key, grid, spec);
        // Like every other keyed door: the label after the node, so a
        // frame with no root records no name (AR16).
        self.key_labels.push(key, label);
    }

    /// [`Self::cells`] under a data index; see [`Self::open_indexed`].
    pub fn cells_indexed(&mut self, i: u64, grid: &crate::cells::CellGrid<'_>, spec: NodeSpec) {
        let key = self.child_key_index(i);
        self.cells_at(key, grid, spec);
    }

    fn cells_at(&mut self, key: Key, grid: &crate::cells::CellGrid<'_>, mut spec: NodeSpec) {
        if self.tree.is_empty() {
            return;
        }
        // The node's box is a box like any leaf's: its `hoverBg` lights
        // and its `transition` tweens the bg, the opacity, the size. The
        // cells inside it are a picture the app redraws, and nothing here
        // touches them (AR5).
        self.prepare_spec(key, &mut spec);
        let cid = self.cells.add(key, grid);
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
        self.prepare_spec(key, &mut spec);
        // The same stamp the two text funnels make: an editor that named
        // no text colour and no selection tint takes the theme's, so a
        // field and a label beside it agree on both (ADR 0019).
        let opts = &EditOptions {
            style: opts.style.or_fg(self.theme.fg),
            accent: Some(opts.accent.unwrap_or(self.theme.selection)),
            ..opts.clone()
        };
        let edge = {
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
            )
        };
        // Autofocus takes the keyboard only while nothing holds it — never
        // from a control Tab landed on — and only on the frame the editor
        // starts being declared (`docs/adr/0022`, decision 9): asked every
        // frame, it would take focus straight back from every blur, and an
        // app with an autofocus field could never have nothing focused.
        if opts.autofocus && edge && self.focus.is_none() && !spec.disabled {
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
    /// corners. Linear sampling, stretched to the box: [`Self::image_node_with`]
    /// takes the two rows that say otherwise.
    pub fn image_node(&mut self, id: crate::resources::ImageId, spec: NodeSpec) {
        self.image_node_with(id, crate::resources::ImageOpts::default(), spec);
    }

    /// [`Self::image_node`] with its `sampling` and `fit` rows (ADR 0025,
    /// decision 4): how texels are read between pixels, and how the pixels
    /// meet a box of another aspect. The box — its layout, hit region and
    /// access rect — is the same in every mode.
    pub fn image_node_with(
        &mut self,
        id: crate::resources::ImageId,
        opts: crate::resources::ImageOpts,
        mut spec: NodeSpec,
    ) {
        if self.tree.is_empty() {
            return;
        }
        let key = self.auto_key();
        self.prepare_spec(key, &mut spec);
        let parent = self.current();
        self.tree
            .push(parent, key, self.origin, spec, NodeContent::Image(id, opts));
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
    /// this session draws nothing, as every resource kind does — and so
    /// does one whose `image` (`FragmentId::with_image`) is not, which is
    /// the removal order ADR 0015 decision 9 asked to be pinned before an
    /// image input existed: the image goes, the fragment reading it draws
    /// the fallback, and the handle it kept is a `foreign-resource` miss
    /// like any other.
    pub fn fragment_node(
        &mut self,
        frag: impl Into<crate::fragment::FragmentRef>,
        params: &[f32],
        spec: NodeSpec,
    ) -> Key {
        let key = self.open_fragment(frag, params, spec);
        self.close();
        key
    }

    /// Opens a fragment as a parent: its children paint over it, which is
    /// what a gradient card with a title and buttons in it is. Balance it
    /// with [`Self::close`], or use `Ui::fragment_with`.
    pub fn open_fragment(
        &mut self,
        frag: impl Into<crate::fragment::FragmentRef>,
        params: &[f32],
        spec: NodeSpec,
    ) -> Key {
        if self.tree.is_empty() {
            return Key::ROOT;
        }
        let key = self.auto_key();
        self.fragment_with_key(key, frag.into(), params, spec);
        key
    }

    /// [`Self::fragment_node`] under a label key, for a fragment that
    /// transitions or exits and needs a stable identity across frames.
    pub fn fragment_node_keyed(
        &mut self,
        label: &str,
        frag: impl Into<crate::fragment::FragmentRef>,
        params: &[f32],
        spec: NodeSpec,
    ) -> Key {
        let key = self.open_fragment_keyed(label, frag, params, spec);
        self.close();
        key
    }

    /// [`Self::open_fragment`] under a label key.
    pub fn open_fragment_keyed(
        &mut self,
        label: &str,
        frag: impl Into<crate::fragment::FragmentRef>,
        params: &[f32],
        spec: NodeSpec,
    ) -> Key {
        if self.tree.is_empty() {
            return Key::ROOT;
        }
        let key = self.child_key(label);
        self.fragment_with_key(key, frag.into(), params, spec);
        self.key_labels.push(key, label);
        key
    }

    /// [`Self::open_fragment`] under a data index; see [`Self::open_indexed`].
    pub fn open_fragment_indexed(
        &mut self,
        i: u64,
        frag: impl Into<crate::fragment::FragmentRef>,
        params: &[f32],
        spec: NodeSpec,
    ) -> Key {
        if self.tree.is_empty() {
            return Key::ROOT;
        }
        let key = self.child_key_index(i);
        self.fragment_with_key(key, frag.into(), params, spec);
        key
    }

    fn fragment_with_key(
        &mut self,
        key: Key,
        frag: crate::fragment::FragmentRef,
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
        let draw = self.fragments.push(crate::fragment::Draw {
            id: frag.id,
            image: frag.image,
            params,
        });
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
    /// honoured like any node's; without them a line has no access row —
    /// unless it takes input, when it derives one as a box would. Input
    /// is hit by *shape* (ADR 0026): a press within half the stroke's
    /// width of any piece (at least `MIN_STROKE_GRAB` wide) hits it, and
    /// a press elsewhere in its box falls through to what is under it.
    /// Fewer than two points draw nothing.
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
        // Like every other keyed door: the label `key_of` resolves through.
        self.key_labels.push(key, label);
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
        // The stroke colour rides in the slot backgrounds tween through, so
        // `transition`, `enter` and `exit` reach it with no slot of its own;
        // nothing else of the box vocabulary applies to a stroke.
        spec.style.bg = stroke.color;
        spec.style.border_w = 0.0;
        spec.style.border_color = Color::TRANSPARENT;
        spec.style.shadow = crate::spec::Shadow::default();
        // The same pipeline as a box's, so a stroke's `hover_bg` is the
        // colour it takes under the pointer and `accent` is honoured.
        self.prepare_spec(key, &mut spec);
        // The box is the stroke's own, and the points are stored relative
        // to it.
        Self::float_box_for(&mut spec, rect);
        let parent = self.current();
        self.tree
            .push(parent, key, self.origin, spec, NodeContent::Line(id));
    }

    /// A filled polygon through `points` in the parent's box space
    /// (`docs/adr/0025-the-image-is-the-canvas.md`, decision 6): up to
    /// eight vertices, the fill in `spec`'s `bg`, painted by the stock
    /// polygon fragment the core registers itself.
    ///
    /// Placed exactly as a line is (ADR 0010, decision 5): never in
    /// layout, a float sized to the points' bounding box inflated by a
    /// logical pixel for the edge ramp, so it takes no room in a row or
    /// column and `spec`'s sizing, clamps, padding, gap and alignment are
    /// ignored. `transition` eases the fill through the `bg` slot, and
    /// `slide`, `enter` and `exit` move the float; a declared `float`
    /// keeps its *anchor*; `role` and `label` are honoured, and without
    /// them a polygon has no access row unless it takes input, when it
    /// derives one as a box would (a clickable wedge is a button). Input
    /// is hit by *shape* (ADR 0026): a press inside the outline hits it,
    /// one in its box but outside the outline falls through to what is
    /// under. Fewer than three points draw nothing; a ninth and later are
    /// dropped with `polygon-points-truncated`. The outline may be
    /// concave; a self-intersecting one fills even-odd, its overlaps
    /// unfilled.
    pub fn polygon_node(&mut self, points: &[Vec2], spec: NodeSpec) {
        if self.tree.is_empty() {
            return;
        }
        let key = self.auto_key();
        self.polygon_with_key(key, points, spec);
    }

    /// [`Self::polygon_node`] under a label key.
    pub fn polygon_node_keyed(&mut self, label: &str, points: &[Vec2], spec: NodeSpec) {
        if self.tree.is_empty() {
            return;
        }
        let key = self.child_key(label);
        self.polygon_with_key(key, points, spec);
        self.key_labels.push(key, label);
    }

    /// [`Self::polygon_node`] under a data index; see [`Self::open_indexed`].
    pub fn polygon_node_indexed(&mut self, i: u64, points: &[Vec2], spec: NodeSpec) {
        if self.tree.is_empty() {
            return;
        }
        let key = self.child_key_index(i);
        self.polygon_with_key(key, points, spec);
    }

    /// The stock polygon fragment's handle, registered on first use and
    /// again after `remove_fragment` forgot it.
    fn stock_polygon(&mut self) -> Option<crate::resources::FragmentId> {
        if let Some(id) = self.stock_polygon {
            return Some(id);
        }
        let id = self.add_fragment(crate::fragment::POLYGON);
        self.stock_polygon = id;
        id
    }

    fn polygon_with_key(&mut self, key: Key, points: &[Vec2], mut spec: NodeSpec) {
        if points.len() < 3 {
            return;
        }
        if points.len() > crate::fragment::POLYGON_MAX_POINTS {
            self.diag.raise(Warning {
                code: crate::diag::POLYGON_POINTS_TRUNCATED,
                key,
                message: format!(
                    "a polygon takes {} points and {} were declared, so the last {} were \
                     dropped; split it in two",
                    crate::fragment::POLYGON_MAX_POINTS,
                    points.len(),
                    points.len() - crate::fragment::POLYGON_MAX_POINTS
                ),
            });
        }
        let points = &points[..points.len().min(crate::fragment::POLYGON_MAX_POINTS)];
        let Some(id) = self.stock_polygon() else {
            return;
        };
        // The box: the points' bounds, a logical pixel out on every side
        // so the one-pixel edge ramp is never cut by the quad's own edge.
        let (mut x0, mut y0, mut x1, mut y1) = (f32::MAX, f32::MAX, f32::MIN, f32::MIN);
        for p in points {
            x0 = x0.min(p.x);
            y0 = y0.min(p.y);
            x1 = x1.max(p.x);
            y1 = y1.max(p.y);
        }
        let rect = Rect::new(x0 - 1.0, y0 - 1.0, x1 - x0 + 2.0, y1 - y0 + 2.0);
        // The vertices, normalised to that box; the last repeated to pad,
        // which the stock source reads as a zero-length edge and skips.
        let mut params = [0.0f32; 16];
        let last = points[points.len() - 1];
        for i in 0..crate::fragment::POLYGON_MAX_POINTS {
            let p = points.get(i).copied().unwrap_or(last);
            params[i * 2] = (p.x - rect.x) / rect.w;
            params[i * 2 + 1] = (p.y - rect.y) / rect.h;
        }
        let draw = self.fragments.push(crate::fragment::Draw {
            id,
            image: None,
            params,
        });
        // The fill rides in `bg`, which `transition`, `enter` and `exit`
        // already ease — and which `hover_bg` and `accent` swap, through
        // the same pipeline as a box's; nothing else of the box vocabulary
        // applies.
        spec.style.border_w = 0.0;
        spec.style.border_color = Color::TRANSPARENT;
        spec.style.shadow = crate::spec::Shadow::default();
        self.prepare_spec(key, &mut spec);
        Self::float_box_for(&mut spec, rect);
        let parent = self.current();
        self.tree
            .push(parent, key, self.origin, spec, NodeContent::Polygon(draw));
    }

    /// A paragraph of styled spans, shaped and wrapped as one flow.
    pub fn rich_text_node(&mut self, spans: &[Span<'_>], base: TextStyle) {
        if self.tree.is_empty() {
            return;
        }
        // The paragraph's own colour, which each span falls back to.
        let base = base.or_fg(self.theme.fg);
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
