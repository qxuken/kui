//! A pane multiplexer: nested splits, tabs, and focus hopping, with the app
//! — not kui — owning the pane tree and the chord keymap. Splits are nested
//! Grow containers rebuilt from a plain enum tree each frame; a pane is just
//! its number here (the modal_editor example shows what real pane content
//! looks like). The whole keyboard arrives through one `on_key` sink as
//! `{kind="key"}` events, so the same chord dispatch would work verbatim
//! from Lua or C.
//!
//! Run: cargo run -p kui --example splitmux
//!
//! Keys — Alt is ⌥ Option on macOS: Alt-v/s split · Alt-o hop panes ·
//! Alt-w close · Alt-t new tab · Alt-1..9 jump to tab. Click a pane to
//! focus it. Drag the strip between panes to resize a split; drag a tab
//! along the bar to reorder it (both are plain `on_drag` data — the drag
//! payload's parent rect gives the divider its ratio, and hover during the
//! drag gives tabs their live reorder). Splits ease into place: the two
//! halves carry a `transition`, so a new split slides open and a keyboard
//! resize glides — except while a divider drags, when the ratio must track
//! the cursor exactly.
//!
//! Hold ⌘ (Ctrl elsewhere) and drag a pane to move it: the modifier state
//! arrives as a `{kind="modifiers"}` event, the model keeps it, and while it
//! is held the view floats five drop-zone overlays (edges + center) over
//! every pane. They are the drag sources *and* the drop targets — no core
//! knowledge of "pane moving" at all, and without ⌘ the overlays don't
//! exist, so plain clicks still reach the pane. Hover over a zone during
//! the drag names the destination; dropping on an edge splits the target
//! on that side, on the center swaps the two panes. A ghost label follows
//! the cursor as a viewport-anchored float.

use kui::widgets;
use kui::{
    Align, App, Color, Easing, FloatConfig, KeyMods, NodeSpec, Sizing, TextStyle, Ui, UiEvent,
    Value, WindowCommand,
};

const TABBAR_H: f32 = 30.0;
/// How long a split takes to ease into a new ratio.
const SPLIT_MS: f32 = 180.0;

#[cfg(target_os = "macos")]
const ALT: &str = "⌥";
#[cfg(not(target_os = "macos"))]
const ALT: &str = "Alt-";
#[cfg(target_os = "macos")]
const PRIMARY: &str = "⌘";
#[cfg(not(target_os = "macos"))]
const PRIMARY: &str = "Ctrl-";

// ---------------------------------------------------------------- palette

#[derive(Clone, Copy)]
struct Pal {
    bg: Color,
    bg2: Color,
    panel: Color,
    border: Color,
    border_focus: Color,
    fg: Color,
    dim: Color,
    faint: Color,
    accent: Color,
}

impl Default for Pal {
    fn default() -> Self {
        Self {
            bg: Color::rgb8(0x0f, 0x11, 0x17),
            bg2: Color::rgb8(0x13, 0x15, 0x1d),
            panel: Color::rgb8(0x14, 0x16, 0x1e),
            border: Color::rgb8(0x22, 0x25, 0x31),
            border_focus: Color::rgb8(0x3b, 0x5b, 0xd4),
            fg: Color::rgb8(0xd6, 0xd8, 0xe0),
            dim: Color::rgb8(0x8a, 0x8f, 0xa3),
            faint: Color::rgb8(0x50, 0x55, 0x66),
            accent: Color::rgb8(0x6a, 0x8b, 0xff),
        }
    }
}

// ---------------------------------------------------------------- model

#[derive(Clone, Copy, PartialEq)]
enum SplitDir {
    /// Side by side.
    H,
    /// Stacked.
    V,
}

#[derive(Clone)]
enum Node {
    Pane(u64),
    Split {
        dir: SplitDir,
        ratio: f32,
        a: Box<Node>,
        b: Box<Node>,
        /// Just created: rendered once with the new half collapsed so the
        /// transition has somewhere to slide it open from (a node's first
        /// frame snaps).
        fresh: Fresh,
    },
}

impl Node {
    fn panes(&self, out: &mut Vec<u64>) {
        match self {
            Node::Pane(id) => out.push(*id),
            Node::Split { a, b, .. } => {
                a.panes(out);
                b.panes(out);
            }
        }
    }

    /// Replaces the leaf `target` with a split of it and `new_id`.
    fn split(&mut self, target: u64, dir: SplitDir, new_id: u64) -> bool {
        match self {
            Node::Pane(id) if *id == target => {
                let old = Node::Pane(*id);
                *self = Node::Split {
                    dir,
                    ratio: 0.5,
                    a: Box::new(old),
                    b: Box::new(Node::Pane(new_id)),
                    fresh: Fresh::B,
                };
                true
            }
            Node::Pane(_) => false,
            Node::Split { a, b, .. } => {
                a.split(target, dir, new_id) || b.split(target, dir, new_id)
            }
        }
    }

    /// Clears every `fresh` flag (after the collapsed first frame drew).
    fn settle(&mut self) {
        if let Node::Split { a, b, fresh, .. } = self {
            *fresh = Fresh::No;
            a.settle();
            b.settle();
        }
    }

    /// Replaces the leaf `target` with a split of it and `node`, `node`
    /// going first when `before`.
    fn split_with(&mut self, target: u64, dir: SplitDir, node: Node, before: bool) -> bool {
        match self {
            Node::Pane(id) if *id == target => {
                let old = Node::Pane(*id);
                let (a, b, fresh) = if before {
                    (node, old, Fresh::A)
                } else {
                    (old, node, Fresh::B)
                };
                *self = Node::Split {
                    dir,
                    ratio: 0.5,
                    a: Box::new(a),
                    b: Box::new(b),
                    fresh,
                };
                true
            }
            Node::Pane(_) => false,
            Node::Split { a, b, .. } => {
                // Descend into the side that holds the target so `node`
                // is moved, not cloned.
                if a.contains(target) {
                    a.split_with(target, dir, node, before)
                } else {
                    b.split_with(target, dir, node, before)
                }
            }
        }
    }

    fn contains(&self, target: u64) -> bool {
        match self {
            Node::Pane(id) => *id == target,
            Node::Split { a, b, .. } => a.contains(target) || b.contains(target),
        }
    }

    /// Exchanges two leaves in place.
    fn swap(&mut self, x: u64, y: u64) {
        match self {
            Node::Pane(id) if *id == x => *id = y,
            Node::Pane(id) if *id == y => *id = x,
            Node::Pane(_) => {}
            Node::Split { a, b, .. } => {
                a.swap(x, y);
                b.swap(x, y);
            }
        }
    }

    /// The split at `path` ("a"/"b" steps from the root), for divider drags.
    fn ratio_mut(&mut self, path: &str) -> Option<&mut f32> {
        match self {
            Node::Pane(_) => None,
            Node::Split { ratio, a, b, .. } => match path.split_at_checked(1) {
                None => Some(ratio),
                Some(("a", rest)) => a.ratio_mut(rest),
                Some((_, rest)) => b.ratio_mut(rest),
            },
        }
    }
}

/// Removes a leaf, collapsing its split; None if the tree became empty.
fn without(node: Node, target: u64) -> Option<Node> {
    match node {
        Node::Pane(id) if id == target => None,
        Node::Pane(id) => Some(Node::Pane(id)),
        Node::Split {
            dir,
            ratio,
            a,
            b,
            fresh,
        } => match (without(*a, target), without(*b, target)) {
            (Some(a), Some(b)) => Some(Node::Split {
                dir,
                ratio,
                a: Box::new(a),
                b: Box::new(b),
                fresh,
            }),
            (Some(x), None) | (None, Some(x)) => Some(x),
            (None, None) => None,
        },
    }
}

/// Which half of a split is new (see `Node::Split::fresh`).
#[derive(Clone, Copy, PartialEq, Eq)]
enum Fresh {
    No,
    A,
    B,
}

/// Where a dragged pane lands on its target: an edge splits the target on
/// that side, the center swaps the two.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Zone {
    Left,
    Right,
    Top,
    Bottom,
    Center,
}

impl Zone {
    const ALL: [Zone; 5] = [
        Zone::Left,
        Zone::Right,
        Zone::Top,
        Zone::Bottom,
        Zone::Center,
    ];

    fn label(self) -> &'static str {
        match self {
            Zone::Left => "zl",
            Zone::Right => "zr",
            Zone::Top => "zt",
            Zone::Bottom => "zb",
            Zone::Center => "zc",
        }
    }

    /// The overlay: a percent-sized float pinned to the pane's edge (or its
    /// middle). Later zones paint and hit-test on top, so the center wins
    /// over the edges and top/bottom win in the corners.
    fn spec(self) -> NodeSpec {
        let (ax, ay, w, h) = match self {
            Zone::Left => (Align::Start, Align::Start, 0.25, 1.0),
            Zone::Right => (Align::End, Align::Start, 0.25, 1.0),
            Zone::Top => (Align::Start, Align::Start, 1.0, 0.25),
            Zone::Bottom => (Align::Start, Align::End, 1.0, 0.25),
            Zone::Center => (Align::Center, Align::Center, 0.5, 0.5),
        };
        NodeSpec::column()
            .float(FloatConfig::parent().at(ax, ay).self_at(ax, ay))
            .width(Sizing::Percent(w))
            .height(Sizing::Percent(h))
    }
}

/// One tab: its split tree plus a stable id, so the tab's node key survives
/// reordering and its position transition has something to slide.
struct Tab {
    id: u64,
    root: Node,
}

// ---------------------------------------------------------------- app

struct Splitmux {
    pal: Pal,
    next_pane: u64,
    /// One split tree per tab — the whole layout is this data.
    tabs: Vec<Tab>,
    next_tab: u64,
    tab: usize,
    focused: u64,
    /// Path of the divider being dragged, for active styling while the
    /// cursor is off the strip.
    dragging: Option<String>,
    /// Tab drag in flight: (current slot, sign of the last horizontal
    /// motion). The sign gates reorder direction so unequal-width tabs
    /// can't oscillate around the cursor.
    tab_drag: Option<(usize, f32)>,
    /// Physical modifiers, straight from `{kind="modifiers"}` events.
    mods: KeyMods,
    /// ⌘-drag of a pane in flight: (pane id, cursor x, cursor y).
    pane_drag: Option<(u64, f32, f32)>,
    /// Where the pane drag would land, recomputed by the view from which
    /// zone overlay is hovered (hover is last frame's layout, as always).
    drop_target: Option<(u64, Zone)>,
    quit: bool,
}

impl Splitmux {
    fn new() -> Self {
        // Launch looking like the app it wants to be: pane | (pane / pane).
        let root = Node::Split {
            dir: SplitDir::H,
            ratio: 0.5,
            a: Box::new(Node::Pane(1)),
            b: Box::new(Node::Split {
                dir: SplitDir::V,
                ratio: 0.5,
                a: Box::new(Node::Pane(2)),
                b: Box::new(Node::Pane(3)),
                fresh: Fresh::No,
            }),
            fresh: Fresh::No,
        };
        Self {
            pal: Pal::default(),
            next_pane: 4,
            tabs: vec![Tab { id: 1, root }],
            next_tab: 2,
            tab: 0,
            focused: 1,
            dragging: None,
            tab_drag: None,
            mods: KeyMods::default(),
            pane_drag: None,
            drop_target: None,
            quit: false,
        }
    }

    /// Whether the panes wear their drop-zone overlays: while the primary
    /// modifier is held, and for the whole of a pane drag (the modifier may
    /// be released mid-drag).
    fn overlays_on(&self) -> bool {
        self.mods.primary() || self.pane_drag.is_some()
    }

    /// Lands `src` on `dst`: an edge zone splits `dst` on that side, the
    /// center swaps them.
    fn move_pane(&mut self, src: u64, dst: u64, zone: Zone) {
        if src == dst {
            return;
        }
        if zone == Zone::Center {
            self.tabs[self.tab].root.swap(src, dst);
            return;
        }
        let root = std::mem::replace(&mut self.tabs[self.tab].root, Node::Pane(u64::MAX));
        let Some(mut root) = without(root, src) else {
            return;
        };
        let (dir, before) = match zone {
            Zone::Left => (SplitDir::H, true),
            Zone::Right => (SplitDir::H, false),
            Zone::Top => (SplitDir::V, true),
            Zone::Bottom => (SplitDir::V, false),
            Zone::Center => unreachable!(),
        };
        root.split_with(dst, dir, Node::Pane(src), before);
        self.tabs[self.tab].root = root;
        self.focused = src;
    }

    fn pane_ids(&self) -> Vec<u64> {
        let mut ids = Vec::new();
        self.tabs[self.tab].root.panes(&mut ids);
        ids
    }

    fn refocus(&mut self) {
        self.focused = self.pane_ids().first().copied().unwrap_or(0);
    }

    fn cycle_pane(&mut self) {
        let ids = self.pane_ids();
        if let Some(i) = ids.iter().position(|id| *id == self.focused) {
            self.focused = ids[(i + 1) % ids.len()];
        } else if let Some(id) = ids.first() {
            self.focused = *id;
        }
    }

    fn split(&mut self, dir: SplitDir) {
        let id = self.next_pane;
        self.next_pane += 1;
        self.tabs[self.tab].root.split(self.focused, dir, id);
        self.focused = id;
    }

    fn close_pane(&mut self) {
        let root = std::mem::replace(&mut self.tabs[self.tab].root, Node::Pane(u64::MAX));
        match without(root, self.focused) {
            Some(root) => {
                self.tabs[self.tab].root = root;
                self.refocus();
            }
            None => {
                self.tabs.remove(self.tab);
                if self.tabs.is_empty() {
                    self.quit = true;
                    return;
                }
                self.tab = self.tab.min(self.tabs.len() - 1);
                self.refocus();
            }
        }
    }

    fn new_tab(&mut self) {
        let id = self.next_pane;
        self.next_pane += 1;
        self.tabs.push(Tab {
            id: self.next_tab,
            root: Node::Pane(id),
        });
        self.next_tab += 1;
        self.tab = self.tabs.len() - 1;
        self.focused = id;
    }

    // ------------------------------------------------------------ keymap

    /// Alt chords, straight off the `code` string. Anything else would go to
    /// the focused pane's content — here panes have none, so it's dropped.
    fn chord(&mut self, code: &str) {
        match code {
            "o" => self.cycle_pane(),
            "v" => self.split(SplitDir::H),
            "s" => self.split(SplitDir::V),
            "w" => self.close_pane(),
            "t" => self.new_tab(),
            _ => {
                if let Ok(n @ 1..=9) = code.parse::<usize>()
                    && n <= self.tabs.len()
                {
                    self.tab = n - 1;
                    self.refocus();
                }
            }
        }
    }

    // ------------------------------------------------------------ view

    fn tab_bar(&mut self, ui: &mut Ui<'_>) {
        let pal = self.pal;
        ui.with(
            NodeSpec::row()
                .width(Sizing::Grow(1.0))
                .height(Sizing::Fixed(TABBAR_H))
                .bg(pal.bg2)
                .pad_xy(8.0, 0.0)
                .gap(4.0)
                .cross_align(Align::Center),
            |ui| {
                // Live reorder: while a tab drags, hovering another tab in
                // the direction of motion moves it there. Hover comes from
                // the previous frame's layout; the direction gate keeps
                // unequal widths from swap-oscillating under a still cursor.
                // Each tab wears a full-height hover column during the
                // drag (a transparent float below it), so the cursor's x
                // keeps reordering after it has left the bar vertically.
                let dragging_tab = self.tab_drag.is_some();
                let column_h = ui.viewport().h;
                if let Some((from, sign)) = self.tab_drag
                    && sign != 0.0
                    && from < self.tabs.len()
                {
                    let to = (0..self.tabs.len()).find(|&j| {
                        let tab = ui.child_key(&format!("tab{}", self.tabs[j].id));
                        j != from
                            && (ui.is_hovered(tab) || ui.is_hovered(tab.str("col")))
                            && ((j > from && sign > 0.0) || (j < from && sign < 0.0))
                    });
                    if let Some(j) = to {
                        let node = self.tabs.remove(from);
                        self.tabs.insert(j, node);
                        self.tab = if self.tab == from {
                            j
                        } else if from < self.tab && j >= self.tab {
                            self.tab - 1
                        } else if from > self.tab && j <= self.tab {
                            self.tab + 1
                        } else {
                            self.tab
                        };
                        self.tab_drag = Some((j, sign));
                    }
                }
                for i in 0..self.tabs.len() {
                    let active = i == self.tab;
                    let lifted = self.tab_drag.is_some_and(|(s, _)| s == i);
                    let (bg, fg) = if active {
                        (pal.panel, pal.fg)
                    } else {
                        (Color::TRANSPARENT, pal.dim)
                    };
                    let mut ids = Vec::new();
                    self.tabs[i].root.panes(&mut ids);
                    // Keyed by tab id, not slot: a reordered tab keeps its
                    // identity, so `slide` eases it (and the tabs it
                    // displaced) into the new order instead of snapping.
                    let mut spec = NodeSpec::row()
                        .pad_xy(10.0, 4.0)
                        .radius(6.0)
                        .bg(bg)
                        .transition(220.0)
                        .easing(Easing::Spring)
                        .slide()
                        .on_click(Value::map([
                            ("kind", "tab".into()),
                            ("tab", Value::Int(i as i64)),
                        ]))
                        .on_drag(Value::map([
                            ("kind", "tabdrag".into()),
                            ("tab", Value::Int(i as i64)),
                        ]));
                    if lifted {
                        spec = spec.border(1.0, pal.border_focus);
                    }
                    ui.with_keyed(&format!("tab{}", self.tabs[i].id), spec, |ui| {
                        ui.text(
                            &format!("{}  {} pane(s)", i + 1, ids.len()),
                            TextStyle::new(12.0).color(fg),
                        );
                        if dragging_tab {
                            // Hangs from the tab's bottom edge: the tab
                            // keeps its own hover, so a press-release on
                            // it is still a click.
                            ui.with_keyed(
                                "col",
                                NodeSpec::column()
                                    .float(FloatConfig::parent().at(Align::Start, Align::End))
                                    .width(Sizing::Percent(1.0))
                                    .height(Sizing::Fixed(column_h))
                                    .hoverable(),
                                |_| {},
                            );
                        }
                    });
                }
                ui.with_keyed(
                    "tab+",
                    NodeSpec::row()
                        .pad_xy(8.0, 4.0)
                        .radius(6.0)
                        .on_click(Value::map([("kind", "tabnew".into())])),
                    |ui| ui.text("+", TextStyle::new(12.0).color(pal.faint)),
                );
                ui.with(NodeSpec::row().width(Sizing::Grow(1.0)), |_| {});
                ui.text(
                    &format!(
                        "{ALT}v/{ALT}s split · {ALT}o hop · {ALT}w close · {ALT}t tab · {PRIMARY}drag moves a pane"
                    ),
                    TextStyle::new(11.0).color(pal.faint),
                );
            },
        );
    }

    fn render_node(&mut self, ui: &mut Ui<'_>, node: &Node, path: &str) {
        match node {
            Node::Pane(id) => self.render_pane(ui, *id),
            Node::Split {
                dir,
                ratio,
                a,
                b,
                fresh,
            } => {
                let pal = self.pal;
                let spec = match dir {
                    SplitDir::H => NodeSpec::row(),
                    SplitDir::V => NodeSpec::column(),
                };
                ui.with(spec.fill(), |ui| {
                    // A fresh split draws once with the new half collapsed;
                    // the transition then slides it open to the real ratio.
                    // That first frame snaps, so nothing is mid-flight yet
                    // to keep frames coming: ask for the next one.
                    if *fresh != Fresh::No {
                        ui.request_frame();
                    }
                    let (wa, wb) = match fresh {
                        Fresh::A => (0.0, 1.0),
                        Fresh::B => (1.0, 0.0),
                        Fresh::No => (ratio.clamp(0.05, 0.95), 1.0 - ratio.clamp(0.05, 0.95)),
                    };
                    // The halves ease between ratios, except under a divider
                    // drag, where the ratio has to follow the cursor exactly
                    // (the core snaps a transition that skipped a frame, so
                    // nothing replays when the drag ends).
                    let dragging = self.dragging.as_deref() == Some(path);
                    let grow = |f: f32| {
                        let spec = match dir {
                            SplitDir::H => NodeSpec::column()
                                .width(Sizing::Grow(f))
                                .height(Sizing::Grow(1.0)),
                            SplitDir::V => NodeSpec::column()
                                .width(Sizing::Grow(1.0))
                                .height(Sizing::Grow(f)),
                        };
                        if dragging {
                            spec
                        } else {
                            spec.transition(SPLIT_MS)
                        }
                    };
                    ui.with_keyed("a", grow(wa), |ui| {
                        self.render_node(ui, a, &format!("{path}a"))
                    });
                    // The divider: a grabbable strip that drags the ratio.
                    // Its drag events carry the parent (this split) rect, so
                    // the handler turns absolute x/y into a ratio directly.
                    let divider = ui.child_key("divider");
                    let active = ui.is_hovered(divider)
                        || ui.is_pressed(divider)
                        || self.dragging.as_deref() == Some(path);
                    let bar = match dir {
                        SplitDir::H => NodeSpec::column()
                            .width(Sizing::Fixed(5.0))
                            .height(Sizing::Grow(1.0)),
                        SplitDir::V => NodeSpec::column()
                            .width(Sizing::Grow(1.0))
                            .height(Sizing::Fixed(5.0)),
                    };
                    ui.with_keyed(
                        "divider",
                        bar.bg(if active { pal.border_focus } else { pal.bg2 })
                            .on_drag(Value::map([
                                ("kind", "split".into()),
                                ("path", Value::str(path)),
                                (
                                    "dir",
                                    Value::str(match dir {
                                        SplitDir::H => "h",
                                        SplitDir::V => "v",
                                    }),
                                ),
                            ])),
                        |_| {},
                    );
                    ui.with_keyed("b", grow(wb), |ui| {
                        self.render_node(ui, b, &format!("{path}b"))
                    });
                });
            }
        }
    }

    /// A pane is just its number — swap this fn for an editor, a terminal,
    /// whatever; the tree around it doesn't change.
    fn render_pane(&mut self, ui: &mut Ui<'_>, id: u64) {
        let pal = self.pal;
        let focused = self.focused == id;
        let overlays = self.overlays_on();
        let drag = self.pane_drag;
        let border = if focused {
            pal.border_focus
        } else {
            pal.border
        };
        ui.with_keyed(
            &format!("pane{id}"),
            NodeSpec::column()
                .fill()
                .center()
                .gap(8.0)
                .bg(pal.panel)
                .border(1.0, border)
                .clip()
                .on_click(Value::map([
                    ("kind", "focus".into()),
                    ("pane", Value::Int(id as i64)),
                ])),
            |ui| {
                let color = if focused { pal.accent } else { pal.faint };
                ui.text(&format!("{id}"), TextStyle::new(48.0).color(color));
                ui.text(
                    if focused { "focused" } else { "click to focus" },
                    TextStyle::new(12.0).color(if focused { pal.fg } else { pal.dim }),
                );
                ui.text(
                    &format!("{ALT}v splits me · {ALT}w closes me"),
                    TextStyle::new(11.0).color(pal.faint),
                );
                if !overlays {
                    return;
                }
                // Drop-zone overlays: drag sources (any zone starts a drag
                // of this pane) and drop targets (the hovered zone during
                // a drag is the destination). Floats hit-test above the
                // pane's own on_click, so ⌘-clicks never focus by accident.
                for zone in Zone::ALL {
                    let key = ui.child_key(zone.label());
                    let hovered = ui.is_hovered(key);
                    let mut lit = false;
                    // Dropping a pane on itself is a no-op; don't advertise it.
                    if let Some((src, ..)) = drag
                        && hovered
                        && src != id
                    {
                        self.drop_target = Some((id, zone));
                        lit = true;
                    }
                    let bg = if lit {
                        Color {
                            a: 0.35,
                            ..pal.accent
                        }
                    } else {
                        Color::TRANSPARENT
                    };
                    ui.with_keyed(
                        zone.label(),
                        zone.spec()
                            .bg(bg)
                            .transition(80.0)
                            .hoverable()
                            .on_drag(Value::map([
                                ("kind", "panedrag".into()),
                                ("pane", Value::Int(id as i64)),
                                ("zone", Value::str(zone.label())),
                            ])),
                        |_| {},
                    );
                }
            },
        );
    }

    /// The ghost that follows the cursor during a pane drag: a viewport
    /// float placed from the drag payload's cursor position.
    fn render_ghost(&self, ui: &mut Ui<'_>) {
        let Some((id, x, y)) = self.pane_drag else {
            return;
        };
        let pal = self.pal;
        // Hang off the cursor's bottom-right, or its bottom-left once the
        // right edge is near (the float's `fit` only clamps for viewport
        // anchors; flipping sides is the host's call).
        let vw = ui.viewport().w;
        let float = if x + 300.0 < vw {
            FloatConfig::viewport().offset(x + 14.0, y + 14.0)
        } else {
            FloatConfig::viewport()
                .at(Align::End, Align::Start)
                .self_at(Align::End, Align::Start)
                .offset(x - vw - 14.0, y + 14.0)
        }
        .fit();
        let target = self
            .drop_target
            .map(|(dst, zone)| match zone {
                Zone::Center => format!("swap with {dst}"),
                Zone::Left => format!("left of {dst}"),
                Zone::Right => format!("right of {dst}"),
                Zone::Top => format!("above {dst}"),
                Zone::Bottom => format!("below {dst}"),
            })
            .unwrap_or_else(|| "drop on a pane".to_string());
        ui.with_keyed(
            "ghost",
            NodeSpec::row()
                .float(float)
                .pad_xy(10.0, 6.0)
                .gap(6.0)
                .radius(6.0)
                .bg(Color {
                    a: 0.92,
                    ..pal.panel
                })
                .border(1.0, pal.border_focus)
                .cross_align(Align::Center),
            |ui| {
                ui.text(&format!("pane {id}"), TextStyle::new(13.0).color(pal.fg));
                ui.text(&format!("→ {target}"), TextStyle::new(12.0).color(pal.dim));
            },
        );
    }
}

impl App for Splitmux {
    fn view(&mut self, ui: &mut Ui<'_>) {
        // Closing the last pane empties `tabs`, so don't build a frame from
        // them — just ask the runner to close and emit nothing.
        if self.quit {
            ui.window_command(WindowCommand::Close);
            return;
        }
        let pal = self.pal;
        ui.configure_root(NodeSpec::column().fill().bg(pal.bg));
        widgets::titlebar(
            ui,
            "splitmux — the app owns the pane tree, kui owns the pixels",
        );
        self.tab_bar(ui);

        let root = self.tabs[self.tab].root.clone();
        // The view names the drop target from hover; start each frame blank
        // so a cursor that left every zone means "nowhere".
        self.drop_target = None;
        let sink = ui.with_keyed(
            "main",
            NodeSpec::column()
                .width(Sizing::Grow(1.0))
                .height(Sizing::Grow(1.0))
                .on_key(Value::Null),
            |ui| self.render_node(ui, &root, ""),
        );
        ui.take_key_focus(sink);
        self.tabs[self.tab].root.settle();
        self.render_ghost(ui);

        widgets::latency_hud(ui);
    }

    fn on_event(&mut self, ev: UiEvent) {
        match ev.payload.get("kind").and_then(Value::as_str) {
            Some("key") => {
                // The chord map: Alt (⌥ Option on macOS) + a letter or digit.
                let alt = ev
                    .payload
                    .get("alt")
                    .and_then(Value::as_bool)
                    .unwrap_or(false);
                let ctrl = ev
                    .payload
                    .get("ctrl")
                    .and_then(Value::as_bool)
                    .unwrap_or(false);
                if alt
                    && !ctrl
                    && let Some(code) = ev.payload.get("code").and_then(Value::as_str)
                {
                    let code = code.to_string();
                    self.chord(&code);
                } else if ev.payload.get("code").and_then(Value::as_str) == Some("escape") {
                    // Abandon a pane drag; the pointer capture runs on
                    // until release, but its end lands on nothing.
                    self.pane_drag = None;
                }
            }
            Some("modifiers") => {
                let flag = |k| ev.payload.get(k).and_then(Value::as_bool).unwrap_or(false);
                self.mods = KeyMods {
                    shift: flag("shift"),
                    ctrl: flag("ctrl"),
                    alt: flag("alt"),
                    super_key: flag("super"),
                };
            }
            Some("focus") => {
                if let Some(id) = ev.payload.get("pane").and_then(Value::as_int) {
                    self.focused = id as u64;
                }
            }
            Some("tab") => {
                if let Some(i) = ev.payload.get("tab").and_then(Value::as_int) {
                    self.tab = i as usize;
                    self.refocus();
                }
            }
            Some("tabnew") => self.new_tab(),
            Some("drag") => {
                let tag = ev.payload.get("tag");
                match tag.and_then(|t| t.get("kind")).and_then(Value::as_str) {
                    Some("tabdrag") => match ev.payload.get("phase").and_then(Value::as_str) {
                        Some("start") => {
                            if let Some(i) = tag.and_then(|t| t.get("tab")).and_then(Value::as_int)
                            {
                                self.tab_drag = Some((i as usize, 0.0));
                            }
                        }
                        Some("move") => {
                            let dx = ev
                                .payload
                                .get("dx")
                                .and_then(Value::as_float)
                                .unwrap_or(0.0);
                            if let Some((_, sign)) = self.tab_drag.as_mut()
                                && dx != 0.0
                            {
                                *sign = dx as f32;
                            }
                        }
                        Some("end") => self.tab_drag = None,
                        _ => {}
                    },
                    Some("split") => self.split_drag(&ev),
                    Some("panedrag") => self.pane_drag_event(&ev),
                    _ => {}
                }
            }
            _ => {}
        }
    }
}

impl Splitmux {
    /// ⌘-drag of a pane: the payload's cursor drives the ghost, the view's
    /// hover bookkeeping names the target, and release performs the move.
    fn pane_drag_event(&mut self, ev: &UiEvent) {
        let num = |k| ev.payload.get(k).and_then(Value::as_float).unwrap_or(0.0) as f32;
        let tag = ev.payload.get("tag");
        let Some(pane) = tag.and_then(|t| t.get("pane")).and_then(Value::as_int) else {
            return;
        };
        match ev.payload.get("phase").and_then(Value::as_str) {
            Some("start") => self.pane_drag = Some((pane as u64, num("x"), num("y"))),
            Some("move") => {
                if let Some((_, x, y)) = self.pane_drag.as_mut() {
                    *x = num("x");
                    *y = num("y");
                }
            }
            Some("end") => {
                if let (Some((src, ..)), Some((dst, zone))) = (self.pane_drag, self.drop_target) {
                    self.move_pane(src, dst, zone);
                }
                self.pane_drag = None;
                self.drop_target = None;
            }
            _ => {}
        }
    }

    /// Divider drags: absolute cursor position over the split's own rect
    /// (carried in the payload) is the new ratio directly.
    fn split_drag(&mut self, ev: &UiEvent) {
        let tag = ev.payload.get("tag");
        let Some(path) = tag.and_then(|t| t.get("path")).and_then(Value::as_str) else {
            return;
        };
        match ev.payload.get("phase").and_then(Value::as_str) {
            Some("end") => self.dragging = None,
            Some(_) => {
                // Absolute cursor position over the split's own rect
                // (carried in the payload) is the new ratio directly.
                let horizontal =
                    tag.and_then(|t| t.get("dir")).and_then(Value::as_str) == Some("h");
                let parent = ev.payload.get("parent");
                let get = |m: Option<&Value>, k| {
                    m.and_then(|v| v.get(k))
                        .and_then(Value::as_float)
                        .unwrap_or(0.0)
                };
                let ratio = if horizontal {
                    let w = get(parent, "w").max(1.0);
                    (get(Some(&ev.payload), "x") - get(parent, "x")) / w
                } else {
                    let h = get(parent, "h").max(1.0);
                    (get(Some(&ev.payload), "y") - get(parent, "y")) / h
                };
                let path = path.to_string();
                self.dragging = Some(path.clone());
                if let Some(r) = self.tabs[self.tab].root.ratio_mut(&path) {
                    *r = (ratio as f32).clamp(0.05, 0.95);
                }
            }
            None => {}
        }
    }
}

fn main() {
    kui::app("splitmux")
        .custom_titlebar()
        .size(1100.0, 720.0)
        .run(Splitmux::new())
        .unwrap();
}
