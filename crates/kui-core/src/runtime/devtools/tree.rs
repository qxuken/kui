//! The tree tab: the main window's last frame as a collapsible, filterable
//! list; the picker that finds a node from the app; the inspector for the
//! selected one; and the outlines the main window paints over them
//! (ADR 0024, decisions 9 and 10).

use rustc_hash::FxHashMap;

use super::panel::{filter_field, small_button, small_button_iconed};
use super::{APP_KEY, DEVTOOLS_KEY, Dock, Place, State, action, fmt_value};
use crate::color::Color;
use crate::geom::{Rect, Vec2};
use crate::key::Key;
use crate::runtime::inspect::NodeInfo;
use crate::spec::TextStyle;
use crate::spec::{Align, FloatConfig, Min, NodeSpec};
use crate::theme::Theme;
use crate::tokens::TokenKind;
use crate::tree::OriginId;
use crate::ui::Ui;
use crate::widgets;

const ROW_H: f32 = 18.0;

/// One row of the list: which node, at what depth once the app container
/// is folded out, and whether it is only there as a match's ancestor.
struct Row {
    idx: usize,
    depth: u16,
    dim: bool,
}

/// The nodes the tab lists — the app's, never the panel's own — with the
/// ones under a collapsed row left out, or, while a filter is typed, the
/// matches and their ancestors.
fn rows(nodes: &[NodeInfo], st: &State) -> Vec<Row> {
    let dt_key = Key::ROOT.str(DEVTOOLS_KEY);
    let app_key = Key::ROOT.str(APP_KEY);
    let index: FxHashMap<Key, usize> = nodes.iter().enumerate().map(|(i, n)| (n.key, i)).collect();
    // What each node is under the panel's subtree or the container: a
    // node's parent has been seen before it (preorder), so one pass.
    let mut hidden = vec![false; nodes.len()];
    let mut under_app = vec![false; nodes.len()];
    for (i, n) in nodes.iter().enumerate() {
        if let Some(p) = n.parent.and_then(|p| index.get(&p).copied()) {
            hidden[i] = hidden[p] || n.parent == Some(dt_key);
            under_app[i] = under_app[p] || n.parent == Some(app_key);
        }
        hidden[i] |= n.key == dt_key;
    }
    let needle = st.tree_filter.trim().to_lowercase();
    if !needle.is_empty() {
        // A match, or the ancestor of one: marked bottom-up, which in
        // preorder is a reverse pass.
        let mut keep = vec![false; nodes.len()];
        let mut matched = vec![false; nodes.len()];
        for i in (0..nodes.len()).rev() {
            if hidden[i] || nodes[i].key == app_key {
                continue;
            }
            if matches(&nodes[i], &needle) {
                matched[i] = true;
                keep[i] = true;
            }
            if keep[i]
                && let Some(p) = nodes[i].parent.and_then(|p| index.get(&p).copied())
            {
                keep[p] = true;
            }
        }
        return nodes
            .iter()
            .enumerate()
            .filter(|(i, n)| keep[*i] && n.key != app_key)
            .map(|(i, n)| Row {
                idx: i,
                depth: n.depth - u16::from(under_app[i]),
                dim: !matched[i],
            })
            .collect();
    }
    let mut out = Vec::with_capacity(nodes.len());
    let mut folded = vec![false; nodes.len()];
    for (i, n) in nodes.iter().enumerate() {
        if hidden[i] || n.key == app_key {
            continue;
        }
        if let Some(p) = n.parent.and_then(|p| index.get(&p).copied()) {
            // The container's children answer to the root's fold.
            let pp = if nodes[p].key == app_key {
                nodes[p].parent.and_then(|k| index.get(&k).copied())
            } else {
                Some(p)
            };
            folded[i] = pp.is_some_and(|p| folded[p] || st.collapsed.contains(&nodes[p].key));
        }
        if folded[i] {
            continue;
        }
        out.push(Row {
            idx: i,
            depth: n.depth - u16::from(under_app[i]),
            dim: false,
        });
    }
    out
}

fn matches(n: &NodeInfo, needle: &str) -> bool {
    n.kind.name().contains(needle)
        || n.label
            .as_deref()
            .is_some_and(|l| l.to_lowercase().contains(needle))
        || n.text
            .as_deref()
            .is_some_and(|t| t.to_lowercase().contains(needle))
        || n.role.is_some_and(|r| r.name().contains(needle))
        || n.flags.iter().any(|f| f.contains(needle))
        || format!("{:08x}", n.key.0 as u32).contains(needle)
}

/// How many nodes are under `i`: its children and theirs, which in
/// preorder are the run of deeper nodes that follows it.
fn descendants(nodes: &[NodeInfo], i: usize) -> usize {
    let d = nodes[i].depth;
    nodes[i + 1..].iter().take_while(|n| n.depth > d).count()
}

fn name_of(n: &NodeInfo) -> String {
    match (&n.label, &n.text) {
        _ if n.key == Key::ROOT => "root".into(),
        (Some(l), _) => l.clone(),
        (None, Some(txt)) => format!("“{txt}”"),
        (None, None) => String::new(),
    }
}

/// How many of `nodes` are the app's: not the panel's, not the container.
fn app_count(nodes: &[NodeInfo]) -> usize {
    let dt_key = Key::ROOT.str(DEVTOOLS_KEY);
    let app_key = Key::ROOT.str(APP_KEY);
    let mut hidden_below: Option<u16> = None;
    let mut n = 0;
    for node in nodes {
        if let Some(d) = hidden_below {
            if node.depth > d {
                continue;
            }
            hidden_below = None;
        }
        if node.key == dt_key {
            hidden_below = Some(node.depth);
            continue;
        }
        if node.key != app_key {
            n += 1;
        }
    }
    n
}

pub(super) fn tree_tab(
    ui: &mut Ui<'_>,
    st: &mut State,
    nodes: &[NodeInfo],
    t: &Theme,
    place: Place,
) {
    // What the last input asked of the rows: every fold at once, or the
    // ancestors of a node to be shown opened.
    if std::mem::take(&mut st.fold_all) {
        for n in nodes {
            if n.children > 0 && n.key != Key::ROOT {
                st.collapsed.insert(n.key);
            }
        }
    }
    let reveal = st.reveal.take();
    if let Some(key) = reveal {
        let index: FxHashMap<Key, usize> =
            nodes.iter().enumerate().map(|(i, n)| (n.key, i)).collect();
        let mut k = index.get(&key).and_then(|&i| nodes[i].parent);
        while let Some(p) = k {
            st.collapsed.remove(&p);
            k = index.get(&p).and_then(|&i| nodes[i].parent);
        }
        st.tree_filter.clear();
        ui.set_edit_text(ui.child_key("kui-devtools/tree-filter"), "");
    }
    // The toolbar.
    ui.with(
        NodeSpec::row()
            .grow_width()
            .min_height(Min::FIT)
            .cross_align(Align::Center)
            .gap(6.0),
        |ui| {
            small_button_iconed(
                ui,
                t,
                "pick",
                super::icons::Icon::Pick,
                "pick",
                "Ctrl+Shift+P · pick a node from the app; Escape leaves",
                st.pick,
            );
            let filter = filter_field(ui, t, "tree-filter", "find");
            if filter != st.tree_filter {
                st.tree_filter = filter;
            }
            small_button(ui, t, "fold-all", "−", "collapse every row", false);
            small_button(ui, t, "unfold-all", "+", "expand every row", false);
        },
    );
    let mut rows = rows(nodes, st);
    // The keyboard (backlog D1a): the list is one sink with a cursor of its
    // own, and a key it heard last frame moves that cursor over the rows
    // this build has — Up/Down by one, Home/End to the ends, PageUp/Down
    // by a screenful, Left folds the row (or goes to its parent), Right
    // unfolds it (or goes to its first child), Enter and Space select it
    // as a click would. A fold or an unfold changes the rows, so they are
    // rebuilt after.
    let mut keep_in_view = None;
    if let Some(code) = st.tree_key.take() {
        let index: FxHashMap<Key, usize> =
            nodes.iter().enumerate().map(|(i, n)| (n.key, i)).collect();
        let at = st
            .tree_cursor
            .and_then(|k| rows.iter().position(|r| nodes[r.idx].key == k));
        let page = 12;
        let go = |i: usize| Some(nodes[rows[i].idx].key);
        let last = rows.len().saturating_sub(1);
        let next = match (code.as_str(), at) {
            (_, _) if rows.is_empty() => None,
            ("down", None) | ("up", None) | ("home", _) => go(0),
            ("end", _) => go(last),
            ("down", Some(i)) => go((i + 1).min(last)),
            ("up", Some(i)) => go(i.saturating_sub(1)),
            ("pagedown", Some(i)) => go((i + page).min(last)),
            ("pageup", Some(i)) => go(i.saturating_sub(page)),
            ("pagedown", None) | ("pageup", None) => go(0),
            ("enter", Some(i)) | ("space", Some(i)) => {
                let k = nodes[rows[i].idx].key;
                st.selected = if st.selected == Some(k) {
                    None
                } else {
                    Some(k)
                };
                Some(k)
            }
            ("left", Some(i)) => {
                let n = &nodes[rows[i].idx];
                if n.children > 0 && !st.collapsed.contains(&n.key) {
                    st.collapsed.insert(n.key);
                    Some(n.key)
                } else {
                    // The parent row, skipping the app container the
                    // rows never show.
                    let mut p = n.parent;
                    while let Some(k) = p {
                        if rows.iter().any(|r| nodes[r.idx].key == k) {
                            break;
                        }
                        p = index.get(&k).and_then(|&i| nodes[i].parent);
                    }
                    p.or(Some(n.key))
                }
            }
            ("right", Some(i)) => {
                let n = &nodes[rows[i].idx];
                if n.children > 0 && st.collapsed.remove(&n.key) {
                    Some(n.key)
                } else if n.children > 0 {
                    rows.get(i + 1).map(|r| nodes[r.idx].key)
                } else {
                    Some(n.key)
                }
            }
            _ => st.tree_cursor,
        };
        if next.is_some() {
            st.tree_cursor = next;
            keep_in_view = next;
        }
        rows = self::rows(nodes, st);
    }
    let selected = st.selected;
    let collapsed = &st.collapsed;
    let count = rows.len();
    let mut hovered_row: Option<Key> = None;
    ui.text(
        &format!(
            "{} of {} nodes · click one to inspect it",
            count,
            app_count(nodes)
        ),
        TextStyle::new(11.0).color(t.muted),
    );
    let list_key = ui.child_key("kui-devtools/nodes");
    let list_focused = ui.is_focused(list_key);
    let cursor = st.tree_cursor;
    let list = widgets::uniform_list(
        ui,
        "kui-devtools/nodes",
        NodeSpec::column()
            .fill()
            .min_height(80.0)
            .bg(t.sunken)
            .radius(6.0)
            .pad(4.0)
            .scrollbar(crate::spec::ScrollbarMode::Auto)
            // The list is the keyboard's one stop and its own sink: the
            // rows are clickable and not Tab stops, and the arrows reach
            // this rather than a row (backlog D1a, ADR 0011).
            .on_key(action("tree-key"))
            .role(crate::access::Role::List)
            .label("nodes")
            .border(1.0, if list_focused { t.focus_ring } else { t.sunken }),
        count,
        ROW_H,
        |ui, i| {
            let row = &rows[i];
            let n = &nodes[row.idx];
            let on = Some(n.key) == selected;
            let at = list_focused && Some(n.key) == cursor;
            let folded = collapsed.contains(&n.key);
            let row_key = format!("node:{:016x}", n.key.0);
            if ui.is_hovered(ui.child_key(&row_key)) {
                hovered_row = Some(n.key);
            }
            let fg = if row.dim { t.faint } else { t.fg };
            ui.with_keyed(
                &row_key,
                NodeSpec::row()
                    .fill()
                    .pad_xy(4.0, 0.0)
                    .radius(3.0)
                    .bg(if on { t.accent_soft } else { t.sunken })
                    .hover_bg(if on { t.accent_soft } else { t.hover })
                    .border(1.0, if at { t.focus_ring } else { Color::TRANSPARENT })
                    .cross_align(Align::Center)
                    .gap(4.0)
                    .on_click(action(format!("node:{:016x}", n.key.0)))
                    .role(crate::access::Role::ListItem)
                    .selected(on)
                    .label(format!("{} {}", n.kind.name(), name_of(n)).as_str()),
                |ui| {
                    ui.leaf(NodeSpec::row().width(row.depth as f32 * 10.0));
                    // The disclosure: its own hit region inside the row's,
                    // so a press on it folds and does not select.
                    if n.children > 0 {
                        ui.text_in_keyed(
                            &format!("fold:{:016x}", n.key.0),
                            NodeSpec::row()
                                .width(12.0)
                                .grow_height()
                                .center()
                                .on_click(action(format!("fold:{:016x}", n.key.0)))
                                .label(if folded { "expand" } else { "collapse" }),
                            if folded { "▸" } else { "▾" },
                            TextStyle::new(10.0).color(t.muted),
                        );
                    } else {
                        ui.leaf(NodeSpec::row().width(12.0));
                    }
                    ui.text(n.kind.name(), TextStyle::new(11.0).color(t.accent).mono());
                    let what = name_of(n);
                    let mut rest = String::new();
                    if !n.flags.is_empty() {
                        rest.push_str(" · ");
                        rest.push_str(&n.flags.join(" "));
                    }
                    if folded {
                        rest.push_str(&format!(" · +{}", descendants(nodes, row.idx)));
                    }
                    ui.text(
                        &what,
                        TextStyle::new(11.0)
                            .color(if what.is_empty() { t.faint } else { fg })
                            .mono()
                            .nowrap(),
                    );
                    if !rest.is_empty() {
                        ui.text(&rest, TextStyle::new(11.0).color(t.muted).mono().nowrap());
                    }
                },
            );
        },
    );
    if st.hovered_row != hovered_row {
        st.hovered_row = hovered_row;
        st.redraw_main = true;
    }
    if let Some(key) = reveal
        && let Some(i) = rows.iter().position(|r| nodes[r.idx].key == key)
    {
        // The row into the top third of the list, so what is around it
        // is legible too.
        let above = (ui.scroll_geometry(list).map_or(200.0, |g| g.rect.h) / 3.0).max(0.0);
        ui.set_scroll(list, Vec2::new(0.0, (i as f32 * ROW_H - above).max(0.0)));
    } else if let Some(key) = keep_in_view
        && let Some(i) = rows.iter().position(|r| nodes[r.idx].key == key)
        && let Some(g) = ui.scroll_geometry(list)
    {
        // The cursor's row stays on screen and the list moves as little
        // as it must: a row above the top scrolls up to it, one below the
        // bottom scrolls down to it, one in view leaves the list alone.
        let top = i as f32 * ROW_H;
        let bottom = top + ROW_H;
        let view_h = (g.rect.h - 8.0).max(ROW_H);
        if top < g.offset.y {
            ui.set_scroll(list, Vec2::new(0.0, top));
        } else if bottom > g.offset.y + view_h {
            ui.set_scroll(list, Vec2::new(0.0, bottom - view_h));
        }
    }
    if let Some(n) = selected.and_then(|k| nodes.iter().find(|n| n.key == k)) {
        inspector(ui, st, nodes, n, t, place);
    }
}

/// The selected node in detail: its ancestors as a breadcrumb, and the
/// rest in groups.
fn inspector(
    ui: &mut Ui<'_>,
    st: &State,
    nodes: &[NodeInfo],
    n: &NodeInfo,
    t: &Theme,
    place: Place,
) {
    let index: FxHashMap<Key, usize> = nodes.iter().enumerate().map(|(i, n)| (n.key, i)).collect();
    let app_key = Key::ROOT.str(APP_KEY);
    // The ancestors, root first, the container left out.
    let mut path = Vec::new();
    let mut k = n.parent;
    while let Some(p) = k {
        let Some(&i) = index.get(&p) else { break };
        if nodes[i].key != app_key {
            path.push(i);
        }
        k = nodes[i].parent;
    }
    path.reverse();
    let parent = path.last().map(|&i| &nodes[i]);
    // The names a value paints under (ADR 0027, decision 7): every token
    // the node's origin sees — its own and the host's — whose resolved
    // value is this one. Two names with one value are both printed; a
    // value no token holds prints as it always did.
    let sees = |tok: &super::TokenFact| tok.origin == n.origin || tok.origin == OriginId::HOST;
    let named = |names: Vec<&str>| {
        if names.is_empty() {
            String::new()
        } else {
            format!(" · {}", names.join(" "))
        }
    };
    let color_names = |c: Color| {
        named(
            st.facts
                .tokens
                .iter()
                .filter(|tok| sees(tok) && tok.kind == TokenKind::Color && tok.resolved == c)
                .map(|tok| tok.name.as_str())
                .collect(),
        )
    };
    let length_names = |v: f32| {
        named(
            st.facts
                .tokens
                .iter()
                .filter(|tok| sees(tok) && tok.kind == TokenKind::Length && tok.length == v)
                .map(|tok| tok.name.as_str())
                .collect(),
        )
    };
    let hex = |c: Color| {
        if c.a == 0.0 {
            "none".to_string()
        } else {
            format!("#{:08x}{}", c.to_hex(), color_names(c))
        }
    };
    let px = |v: f32| format!("{v:.0}{}", length_names(v));
    let opt_px = |v: Option<f32>| v.map_or("—".to_string(), |v| format!("{v:.0}"));
    let align = |a: Align| a.name();
    let is = |k: Option<Key>| k == Some(n.key);
    let mut state = Vec::new();
    if is(st.facts.hovered) {
        state.push("hovered");
    }
    if is(st.facts.pressed) {
        state.push("pressed");
    }
    if is(st.facts.focus) {
        state.push(if st.facts.focus_visible {
            "focused · ring"
        } else {
            "focused"
        });
    }
    if is(st.facts.region) {
        state.push("region");
    }
    let origin = match n.origin {
        OriginId::HOST => "host".to_string(),
        OriginId::DEVTOOLS => "devtools".to_string(),
        o => format!("extension {}", o.0),
    };
    // `(group, rows)`: a row is a name, a value, and what a click on it
    // selects, if anything.
    type Rows = Vec<(&'static str, String, Option<Key>)>;
    let groups: Vec<(&str, Rows)> = vec![
        (
            "node",
            vec![
                ("key", format!("{:016x}", n.key.0), None),
                ("label", n.label.clone().unwrap_or_else(|| "—".into()), None),
                ("kind", n.kind.name().into(), None),
                (
                    "role",
                    n.role.map_or("—".into(), |r| r.name().to_string()),
                    None,
                ),
                ("origin", origin, None),
                ("text", n.text.clone().unwrap_or_else(|| "—".into()), None),
                (
                    "parent",
                    parent.map_or("—".into(), |p| {
                        format!("{} {}", p.kind.name(), name_of(p))
                    }),
                    parent.map(|p| p.key),
                ),
                ("children", n.children.to_string(), None),
                (
                    "layer",
                    if n.layer == 0 {
                        "flow".into()
                    } else {
                        format!("float {}{}", n.layer, if n.float { " (root)" } else { "" })
                    },
                    None,
                ),
            ],
        ),
        (
            "box",
            vec![
                (
                    "rect",
                    format!(
                        "{}, {} · {}×{}",
                        px(n.rect.x),
                        px(n.rect.y),
                        px(n.rect.w),
                        px(n.rect.h)
                    ),
                    None,
                ),
                (
                    "size",
                    format!("{} × {}", n.width.describe(), n.height.describe()),
                    None,
                ),
                (
                    "min",
                    format!(
                        "{} × {}",
                        n.min_w.map_or("fit".into(), px),
                        n.min_h.map_or("fit".into(), px)
                    ),
                    None,
                ),
                (
                    "max",
                    format!("{} × {}", opt_px(n.max_w), opt_px(n.max_h)),
                    None,
                ),
                (
                    "scroll",
                    n.scroll
                        .map_or("—".into(), |o| format!("{}, {}", px(o.x), px(o.y))),
                    None,
                ),
            ],
        ),
        (
            "layout",
            vec![
                (
                    "dir",
                    format!(
                        "{}{}{}",
                        format!("{:?}", n.dir).to_lowercase(),
                        if n.table { " · table" } else { "" },
                        if n.wrap { " · wrap" } else { "" }
                    ),
                    None,
                ),
                (
                    "padding",
                    format!(
                        "{} {} {} {}",
                        px(n.padding.t),
                        px(n.padding.r),
                        px(n.padding.b),
                        px(n.padding.l)
                    ),
                    None,
                ),
                ("gap", px(n.gap), None),
                (
                    "align",
                    format!("{} · {}", align(n.main_align), align(n.cross_align)),
                    None,
                ),
            ],
        ),
        (
            "paint",
            vec![
                ("bg", hex(n.bg), None),
                (
                    "border",
                    if n.border_w > 0.0 {
                        format!("{} {}", px(n.border_w), hex(n.border_color))
                    } else {
                        "—".into()
                    },
                    None,
                ),
                (
                    "radius",
                    if n.radius.iter().all(|r| *r == n.radius[0]) {
                        px(n.radius[0])
                    } else {
                        n.radius
                            .iter()
                            .map(|r| px(*r))
                            .collect::<Vec<_>>()
                            .join(" ")
                    },
                    None,
                ),
                ("opacity", format!("{:.2}", n.opacity), None),
            ],
        ),
        (
            "events",
            if n.events.is_empty() {
                vec![("—", String::new(), None)]
            } else {
                n.events
                    .iter()
                    .map(|(name, v)| (*name, fmt_value(v), None))
                    .collect()
            },
        ),
        (
            "state",
            vec![
                (
                    "flags",
                    if n.flags.is_empty() {
                        "—".into()
                    } else {
                        n.flags.join(" ")
                    },
                    None,
                ),
                (
                    "now",
                    if state.is_empty() {
                        "—".into()
                    } else {
                        state.join(" · ")
                    },
                    None,
                ),
            ],
        ),
    ];
    let _ = place;
    ui.with_keyed(
        "kui-devtools/inspector",
        NodeSpec::column()
            .fill()
            .min_height(60.0)
            .max_height(300.0)
            .gap(4.0)
            .pad(6.0)
            .bg(t.raised)
            .radius(6.0)
            .border(1.0, t.border)
            .scroll_y()
            .scrollbar(crate::spec::ScrollbarMode::Auto),
        |ui| {
            // The breadcrumb: each ancestor a button that selects it.
            ui.with(
                NodeSpec::row()
                    .grow_width()
                    .min_height(Min::FIT)
                    .gap(2.0)
                    .wrap()
                    .cross_align(Align::Center),
                |ui| {
                    for &i in &path {
                        let a = &nodes[i];
                        let what = if a.key == Key::ROOT {
                            "root".to_string()
                        } else {
                            let name = name_of(a);
                            if name.is_empty() {
                                a.kind.name().to_string()
                            } else {
                                name
                            }
                        };
                        ui.text_in_keyed(
                            &format!("crumb:{:016x}", a.key.0),
                            NodeSpec::row()
                                .pad_xy(4.0, 1.0)
                                .radius(3.0)
                                .hover_bg(t.hover)
                                .on_click(action(format!("goto:{:016x}", a.key.0)))
                                .label(what.as_str()),
                            &what,
                            TextStyle::new(10.0).color(t.muted).mono().nowrap(),
                        );
                        ui.text("›", TextStyle::new(10.0).color(t.faint));
                    }
                    let name = name_of(n);
                    ui.text(
                        if name.is_empty() {
                            n.kind.name()
                        } else {
                            &name
                        },
                        TextStyle::new(10.0).color(t.fg).mono().nowrap(),
                    );
                },
            );
            // Each group is a table (ADR 0033): its name column at the
            // longest name in that group, a row that goes somewhere a
            // clickable row like any other.
            for (group, rows) in groups {
                ui.with(
                    NodeSpec::column()
                        .grow_width()
                        .min_height(Min::FIT)
                        .gap(1.0),
                    |ui| {
                        ui.text(group, TextStyle::new(10.0).color(t.accent));
                        ui.with(NodeSpec::table().grow_width().gap(1.0), |ui| {
                            for (k, v, goto) in rows {
                                let spec = NodeSpec::row()
                                    .grow_width()
                                    .gap(8.0)
                                    .pad_xy(2.0, 0.0)
                                    .radius(3.0);
                                let spec = match goto {
                                    Some(key) => spec
                                        .hover_bg(t.hover)
                                        .on_click(action(format!("goto:{:016x}", key.0)))
                                        .label(format!("{k} {v}").as_str()),
                                    None => spec,
                                };
                                ui.with(spec, |ui| {
                                    ui.text(k, TextStyle::new(11.0).color(t.muted));
                                    ui.text(
                                        &v,
                                        TextStyle::new(11.0)
                                            .color(if goto.is_some() { t.accent } else { t.fg })
                                            .mono(),
                                    );
                                });
                            }
                        });
                    },
                );
            }
        },
    );
}

/// The app's area in the main window, in viewport coordinates: what the
/// picker covers, and what the outlines are drawn within.
fn app_area(st: &State, dock: Dock) -> Rect {
    let vp = st.facts.viewport;
    let (w, h) = (
        super::pane_w(st.side_w, vp.w),
        super::pane_h(st.bottom_h, vp.h),
    );
    match dock {
        Dock::Left => Rect::new(w, 0.0, (vp.w - w).max(0.0), vp.h),
        Dock::Right => Rect::new(0.0, 0.0, (vp.w - w).max(0.0), vp.h),
        Dock::Bottom => Rect::new(0.0, 0.0, vp.w, (vp.h - h).max(0.0)),
        _ => Rect::new(0.0, 0.0, vp.w, vp.h),
    }
}

/// The node under `p` (decision 9): among the app's nodes whose rect
/// holds it and whose scrolling or clipping ancestors all do too, the one
/// in the topmost layer, and the last in preorder within it — the deepest
/// of those painted over one another.
pub(super) fn pick_target(nodes: &[NodeInfo], p: Vec2) -> Option<Key> {
    let dt_key = Key::ROOT.str(DEVTOOLS_KEY);
    let app_key = Key::ROOT.str(APP_KEY);
    let index: FxHashMap<Key, usize> = nodes.iter().enumerate().map(|(i, n)| (n.key, i)).collect();
    let mut hidden = vec![false; nodes.len()];
    let mut clipped = vec![false; nodes.len()];
    let mut best: Option<(u16, usize)> = None;
    for (i, n) in nodes.iter().enumerate() {
        if let Some(pi) = n.parent.and_then(|k| index.get(&k).copied()) {
            hidden[i] = hidden[pi] || n.parent == Some(dt_key) || n.key == dt_key;
            let clips = nodes[pi]
                .flags
                .iter()
                .any(|f| *f == "scroll" || *f == "clip");
            clipped[i] =
                clipped[pi] || (clips && !nodes[pi].rect.contains(p) && n.layer == nodes[pi].layer);
        }
        // A text node is its box's: the box is what has a spec to read.
        if hidden[i]
            || clipped[i]
            || n.key == app_key
            || n.key == Key::ROOT
            || n.kind == crate::runtime::inspect::NodeKind::Text
        {
            continue;
        }
        if n.rect.contains(p) && best.is_none_or(|(layer, _)| n.layer >= layer) {
            best = Some((n.layer, i));
        }
    }
    best.map(|(_, i)| nodes[i].key)
}

/// What the main window paints over the app for the panel: the outline of
/// the selected node and of the row under the pointer, and, while the
/// picker is up, the overlay that takes the press, the outline of the node
/// under the pointer and a badge naming it.
pub(super) fn overlays(
    ui: &mut Ui<'_>,
    st: &mut State,
    nodes: &[NodeInfo],
    t: &Theme,
    place: Place,
) {
    let Place::Main(dock) = place else { return };
    let find = |k: Option<Key>| k.and_then(|k| nodes.iter().find(|n| n.key == k));
    let mut outlines: Vec<(&NodeInfo, &'static str, bool)> = Vec::new();
    if let Some(n) = find(st.selected) {
        outlines.push((n, "kui-devtools/outline-selected", true));
    }
    if let Some(n) = find(st.hovered_row) {
        outlines.push((n, "kui-devtools/outline-hover", false));
    }
    if st.pick {
        let area = app_area(st, dock);
        let cursor = st.facts.cursor.filter(|p| area.contains(*p));
        st.pick_hover = cursor.and_then(|p| pick_target(nodes, p));
        // The overlay: over the app's area only, so the panel's own
        // controls — the crosshair that leaves — stay pressable.
        ui.leaf_keyed(
            "kui-devtools/picker",
            NodeSpec::row()
                .float(
                    FloatConfig::viewport()
                        .inside(Align::Start, Align::Start)
                        .offset(area.x, area.y),
                )
                .size(area.w.max(1.0), area.h.max(1.0))
                .bg(t.accent.with_alpha(0.04))
                .hoverable()
                .cursor(crate::cursor::CursorShape::Pointer)
                .on_click(action("picked"))
                .label("pick a node"),
        );
        if let Some(n) = find(st.pick_hover) {
            outlines.push((n, "kui-devtools/outline-pick", true));
            // The badge: what it is, above its top-left corner, or below
            // it when the corner is at the top of the window.
            let name = name_of(n);
            let badge = format!(
                "{} {} · {}×{}",
                n.kind.name(),
                name,
                n.rect.w.round(),
                n.rect.h.round()
            );
            let y = if n.rect.y >= 20.0 {
                n.rect.y - 20.0
            } else {
                n.rect.y + n.rect.h
            };
            ui.text_in_keyed(
                "kui-devtools/badge",
                NodeSpec::row()
                    .float(
                        FloatConfig::viewport()
                            .inside(Align::Start, Align::Start)
                            .offset(n.rect.x.max(0.0), y.max(0.0)),
                    )
                    .pad_xy(6.0, 2.0)
                    .radius(3.0)
                    .bg(t.fg)
                    .role(crate::access::Role::None),
                &badge,
                TextStyle::new(10.0).color(t.bg).mono().nowrap(),
            );
        }
    } else {
        st.pick_hover = None;
    }
    // The outlines: viewport floats, since the rects are viewport px and
    // the app is a sibling. The border is the foreground, not the accent:
    // half the nodes worth outlining are accent-coloured buttons, on which
    // an accent border is nothing.
    for (n, label, strong) in outlines {
        ui.leaf_keyed(
            label,
            NodeSpec::row()
                .float(
                    FloatConfig::viewport()
                        .inside(Align::Start, Align::Start)
                        .offset(n.rect.x, n.rect.y),
                )
                .size(n.rect.w.max(1.0), n.rect.h.max(1.0))
                .border(
                    if strong { 2.0 } else { 1.0 },
                    t.fg.with_alpha(if strong { 0.9 } else { 0.5 }),
                )
                .bg(t.accent.with_alpha(if strong { 0.10 } else { 0.05 }))
                .role(crate::access::Role::None),
        );
    }
}
