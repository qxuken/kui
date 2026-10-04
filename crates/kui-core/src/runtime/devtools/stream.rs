//! The events tab: the stream as a virtual list of rows that open into
//! the payload as data, and the one-line printing
//! of a `Value` the rows and the inspector share.

#[cfg(test)]
use super::lines;
use super::panel::{filter_field, small_button};
use super::{Entry, EntryKind, STREAM_CAP, State, action, fmt_value};
use crate::geom::Vec2;
use crate::spec::TextStyle;
use crate::spec::{Align, Min, NodeSpec};
use crate::theme::Theme;
use crate::ui::Ui;
use crate::value::Value;
use crate::widgets::{self, RowHeights};
use crate::window::WindowId;

/// A closed row's height, and each line of an open payload's.
pub(super) const ROW_H: f32 = super::STREAM_ROW_H;
const LINE_H: f32 = 15.0;
/// What an open row adds around its lines.
const OPEN_PAD: f32 = 6.0;

/// `v` as `(indent, line)` pairs, the tree `lines` counts.
fn tree_lines(v: &Value, indent: usize, out: &mut Vec<(usize, String)>) {
    match v {
        Value::Map(entries) => {
            for (k, v) in entries {
                push_line(k, v, indent, out);
            }
        }
        Value::List(items) => {
            for (i, v) in items.iter().enumerate() {
                push_line(&i.to_string(), v, indent, out);
            }
        }
        other => out.push((indent, fmt_value(other))),
    }
}

fn push_line(k: &str, v: &Value, indent: usize, out: &mut Vec<(usize, String)>) {
    match v {
        Value::Map(entries) => {
            out.push((indent, format!("{k}: {{{}}}", entries.len())));
            tree_lines(v, indent + 1, out);
        }
        Value::List(items) => {
            out.push((indent, format!("{k}: [{}]", items.len())));
            tree_lines(v, indent + 1, out);
        }
        other => out.push((indent, format!("{k}: {}", fmt_value(other)))),
    }
}

fn row_height(e: &Entry, open: bool) -> f32 {
    if open {
        ROW_H + e.lines as f32 * LINE_H + OPEN_PAD
    } else {
        ROW_H
    }
}

/// The row's one line, without the frame: the window when not the main,
/// the node, and the payload.
fn summary(e: &Entry) -> String {
    let window = if e.window == WindowId::MAIN {
        String::new()
    } else {
        format!("w{} ", e.window.0)
    };
    match e.kind {
        EntryKind::Note => format!("{window}{}", fmt_value(&e.payload)),
        EntryKind::Warning => format!(
            "{window}warning [{}] {}",
            e.payload.get_str("code").unwrap_or(""),
            e.payload.get_str("message").unwrap_or("")
        ),
        EntryKind::Event => {
            let who = match &e.label {
                Some(l) => l.clone(),
                None => format!("{:08x}", e.key.0 as u32),
            };
            let origin = if e.origin.0 == 0 {
                String::new()
            } else {
                format!("ext{} ", e.origin.0)
            };
            format!("{window}{origin}{who} {}", fmt_value(&e.payload))
        }
    }
}

pub(super) fn events_tab(ui: &mut Ui<'_>, st: &mut State, t: &Theme) {
    // The toolbar: the count, the filter, and the three toggles.
    ui.with(
        NodeSpec::row()
            .grow_width()
            .min_height(Min::FIT)
            .cross_align(Align::Center)
            .gap(6.0),
        |ui| {
            ui.text(
                &format!("{} of {STREAM_CAP}", st.stream.len()),
                TextStyle::new(11.0).color(t.muted).nowrap(),
            );
            let filter = filter_field(ui, t, "stream-filter", "filter");
            if filter != st.stream_filter {
                st.stream_filter = filter;
                st.stream_dirty = true;
            }
            small_button(
                ui,
                t,
                "pause",
                if st.paused { "paused" } else { "pause" },
                "stop logging, keeping what is here",
                st.paused,
            );
            small_button(
                ui,
                t,
                "follow",
                "follow",
                "keep the newest row in view · a wheel up turns it off",
                st.follow,
            );
            small_button(
                ui,
                t,
                "clear",
                "clear",
                "Ctrl+Shift+C · empty the stream",
                false,
            );
        },
    );
    // The rows in view, rebuilt when anything changed, and their heights
    // forgotten with them: the measure below is arithmetic on the entry,
    // exact for every row it is asked about, so the list never corrects.
    if st.stream_dirty {
        let needle = st.stream_filter.to_lowercase();
        st.rows = st
            .stream
            .iter()
            .enumerate()
            .filter(|(_, e)| needle.is_empty() || summary(e).to_lowercase().contains(&needle))
            .map(|(i, _)| i)
            .collect();
        st.heights = RowHeights::new(st.rows.len(), ROW_H);
        st.stream_dirty = false;
    }
    let State {
        stream,
        rows,
        heights,
        expanded,
        ..
    } = st;
    let spec = NodeSpec::column()
        .fill()
        .min_height(60.0)
        .bg(t.sunken)
        .radius(6.0)
        .pad(6.0);
    if rows.is_empty() {
        ui.text_in_keyed(
            "kui-devtools/stream",
            spec,
            if stream.is_empty() {
                "events arrive here as data"
            } else {
                "nothing matches the filter"
            },
            TextStyle::new(11.0).color(t.faint),
        );
        return;
    }
    let list = widgets::list(
        ui,
        "kui-devtools/stream",
        spec.scrollbar(crate::spec::ScrollbarMode::Auto),
        heights,
        |_, i, _| {
            let e = &stream[rows[i]];
            row_height(e, expanded.contains(&e.seq))
        },
        |ui, i| {
            let e = &stream[rows[i]];
            let open = expanded.contains(&e.seq);
            row(ui, t, e, open);
        },
    );
    // Follow the newest row: the retained offset is pinned past the end
    // whenever the travel changed — the stream grew, a row opened, the
    // pane resized — and left alone otherwise, because `set_scroll` asks
    // for a frame and a pin every frame is a window that never idles
    // again once it has shown one event (~130 frames/s, found on screen
    // building ADR 0029). A wheel moves the offset back inside a travel
    // that did *not* change, and that is the user taking over until
    // `follow` is pressed again (which forces one more pin through
    // `stream_grew`).
    if st.follow {
        let forced = std::mem::take(&mut st.stream_grew);
        let max = ui.scroll_geometry(list).map_or(0.0, |g| g.max_offset.y);
        let travel_changed = max != st.followed_max;
        if forced || travel_changed {
            ui.set_scroll(list, Vec2::new(0.0, f32::MAX));
            st.followed_max = max;
        } else if ui.scroll_offset(list).y + 1.0 < max {
            st.follow = false;
        }
    }
}

/// One entry: its line, and the payload's lines under it when open.
fn row(ui: &mut Ui<'_>, t: &Theme, e: &Entry, open: bool) {
    let color = match e.kind {
        EntryKind::Event => t.fg,
        EntryKind::Warning => t.warning,
        EntryKind::Note => t.muted,
    };
    ui.with_keyed(
        &format!("row:{}", e.seq),
        NodeSpec::column()
            .fill()
            .radius(3.0)
            .bg(if open { t.raised } else { t.sunken })
            .hover_bg(if open { t.raised } else { t.hover })
            .on_click(action(format!("row:{}", e.seq)))
            .label(summary(e).as_str())
            .expanded(open),
        |ui| {
            ui.with(
                NodeSpec::row()
                    .grow_width()
                    .height(ROW_H)
                    .pad_xy(4.0, 0.0)
                    .gap(6.0)
                    .cross_align(Align::Center)
                    .clip(),
                |ui| {
                    ui.text(
                        if open { "▾" } else { "▸" },
                        TextStyle::new(10.0).color(t.faint),
                    );
                    ui.text(
                        &format!("{:>4}", e.frame),
                        TextStyle::new(11.0).color(t.faint).mono(),
                    );
                    ui.text(
                        &summary(e),
                        TextStyle::new(11.0).color(color).mono().nowrap(),
                    );
                },
            );
            if open {
                let mut lines = Vec::new();
                tree_lines(&e.payload, 0, &mut lines);
                ui.with(
                    NodeSpec::column().grow_width().pad_xy(28.0, OPEN_PAD / 2.0),
                    |ui| {
                        for (indent, line) in lines {
                            ui.text_in(
                                NodeSpec::row()
                                    .grow_width()
                                    .height(LINE_H)
                                    .pad_xy(indent as f32 * 12.0, 0.0)
                                    .cross_align(Align::Center)
                                    .clip(),
                                &line,
                                TextStyle::new(11.0).color(t.fg).mono().nowrap(),
                            );
                        }
                    },
                );
            }
        },
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn values_print_as_one_line_of_data() {
        let v = Value::map([
            ("kind", Value::str("click")),
            ("n", Value::Int(3)),
            (
                "at",
                Value::List(vec![Value::Float(1.5), Value::Float(2.0)]),
            ),
            ("text", Value::str("two words")),
        ]);
        assert_eq!(
            fmt_value(&v),
            "{kind: click, n: 3, at: [1.50, 2], text: \"two words\"}"
        );
        // And as a tree: one line per entry, the list's items under it.
        assert_eq!(lines(&v), 6);
        let mut out = Vec::new();
        tree_lines(&v, 0, &mut out);
        assert_eq!(
            out,
            vec![
                (0, "kind: click".to_string()),
                (0, "n: 3".to_string()),
                (0, "at: [2]".to_string()),
                (1, "0: 1.50".to_string()),
                (1, "1: 2".to_string()),
                (0, "text: \"two words\"".to_string()),
            ]
        );
        assert_eq!(lines(&Value::str("note")), 1);
    }
}
