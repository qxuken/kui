//! The table (ADR 0033): `NodeSpec::table()` is a column whose rows'
//! children line up in columns, each column as wide as its widest cell.
//! No width is picked by hand and nothing is measured: the name column
//! sits at the longest name, the size column at the widest size, and the
//! kind column grows into the rest because its cells say `grow`.
//!
//! The rows are rows — this one's are clickable, with a hover wash and a
//! selected background, and the header's cells are clickable too, sorting
//! the rows by the column pressed — and a number right-aligns inside its
//! column with a `main_align: end` row around the text, as it would
//! anywhere.
//!
//! Run: cargo run -p kui-native --example table [-- --headless]

use kui_devtools::{Drive, Example};
use kui_native::{Align, App, Core, NodeSpec, Sizing, TextStyle, Ui, UiEvent, Value};

/// Name, size in bytes, kind.
const FILES: [(&str, u64, &str); 6] = [
    ("Cargo.toml", 1_204, "manifest"),
    ("src", 0, "directory"),
    ("README.md", 18_930, "markdown"),
    ("target", 0, "directory"),
    ("a-rather-long-file-name.rs", 402, "rust source"),
    ("LICENSE", 1_067, "text"),
];

#[derive(Clone, Copy, PartialEq)]
enum Sort {
    Name,
    Size,
    Kind,
}

struct Table {
    sort: Sort,
    selected: Option<usize>,
}

impl Default for Table {
    fn default() -> Self {
        Self {
            sort: Sort::Name,
            selected: None,
        }
    }
}

impl Table {
    fn order(&self) -> Vec<usize> {
        let mut rows: Vec<usize> = (0..FILES.len()).collect();
        match self.sort {
            Sort::Name => rows.sort_by_key(|&i| FILES[i].0),
            Sort::Size => rows.sort_by_key(|&i| std::cmp::Reverse(FILES[i].1)),
            Sort::Kind => rows.sort_by_key(|&i| (FILES[i].2, FILES[i].0)),
        }
        rows
    }
}

fn size(bytes: u64) -> String {
    if bytes == 0 {
        "—".to_string()
    } else if bytes < 10_000 {
        format!("{bytes} B")
    } else {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    }
}

impl App for Table {
    fn view(&mut self, ui: &mut Ui<'_>) {
        let t = ui.theme();
        ui.with(NodeSpec::column().fill().pad(24.0).gap(12.0), |ui| {
            ui.text(
                "a table: each column as wide as its widest cell, the last one growing",
                TextStyle::new(12.0).color(t.muted),
            );
            ui.with_keyed(
                "files",
                NodeSpec::table()
                    .grow_width()
                    .gap(2.0)
                    .bg(t.sunken)
                    .radius(6.0)
                    .pad(4.0),
                |ui| {
                    // The header: a row of three clickable cells, each
                    // sorting by its column; the sorted one in the accent.
                    ui.with(
                        NodeSpec::row().grow_width().gap(16.0).pad_xy(8.0, 4.0),
                        |ui| {
                            for (label, sort, align) in [
                                ("name", Sort::Name, Align::Start),
                                ("size", Sort::Size, Align::End),
                                ("kind", Sort::Kind, Align::Start),
                            ] {
                                let color = if self.sort == sort { t.accent } else { t.muted };
                                let width = if sort == Sort::Kind {
                                    Sizing::Grow(1.0)
                                } else {
                                    Sizing::Fit
                                };
                                ui.text_in_keyed(
                                    &format!("sort-{label}"),
                                    NodeSpec::row()
                                        .width(width)
                                        .main_align(align)
                                        .on_click(Value::map([
                                            ("kind", Value::str("sort")),
                                            ("by", Value::str(label)),
                                        ]))
                                        .label(format!("sort by {label}").as_str()),
                                    label,
                                    TextStyle::new(11.0).color(color),
                                );
                            }
                        },
                    );
                    for i in self.order() {
                        let (name, bytes, kind) = FILES[i];
                        let selected = self.selected == Some(i);
                        ui.with_keyed(
                            name,
                            NodeSpec::row()
                                .grow_width()
                                .gap(16.0)
                                .pad_xy(8.0, 3.0)
                                .radius(4.0)
                                .bg(if selected { t.accent } else { t.sunken })
                                .hover_bg(if selected { t.accent } else { t.hover })
                                .on_click(Value::map([
                                    ("kind", Value::str("select")),
                                    ("row", Value::Int(i as i64)),
                                ]))
                                .label(name),
                            |ui| {
                                let fg = if selected { t.on_accent } else { t.fg };
                                // A bare text is a cell, held to its column.
                                ui.text(name, TextStyle::new(13.0).color(fg).nowrap());
                                // A number sits at the column's right edge.
                                ui.text_in_keyed(
                                    "size",
                                    NodeSpec::row().main_align(Align::End),
                                    &size(bytes),
                                    TextStyle::new(13.0).color(fg).mono().nowrap(),
                                );
                                ui.text(
                                    kind,
                                    TextStyle::new(13.0).color(if selected {
                                        t.on_accent
                                    } else {
                                        t.muted
                                    }),
                                );
                            },
                        );
                    }
                },
            );
            let picked = self
                .selected
                .map(|i| format!("selected: {}", FILES[i].0))
                .unwrap_or_else(|| "click a row to select it, a header to sort by it".to_string());
            ui.text(&picked, TextStyle::new(12.0).color(t.faint).mono());
        });
    }

    fn on_event(&mut self, ev: UiEvent) {
        match ev.kind() {
            Some("select") => {
                self.selected = ev.payload.get_int("row").map(|i| i as usize);
            }
            Some("sort") => {
                self.sort = match ev.payload.get_str("by") {
                    Some("size") => Sort::Size,
                    Some("kind") => Sort::Kind,
                    _ => Sort::Name,
                };
            }
            _ => {}
        }
    }
}

impl Example for Table {
    fn window(&self) -> kui_devtools::Window {
        kui_devtools::Window::default().size(560.0, 320.0)
    }

    /// Every row's size cell starts at one x whatever its name's length,
    /// the size column is as wide as its widest size, and the kind column
    /// ends at the table's edge; a click selects a row, a header re-sorts.
    fn headless(&mut self, core: &mut Core) -> Result<(), String> {
        core.set_inspect(true);
        let mut d = Drive::new(core, 560.0, 320.0);
        d.frame(self);
        let cells = |d: &Drive<'_>, label: &str| -> Vec<kui_native::Rect> {
            d.core
                .nodes()
                .iter()
                .filter(|n| n.label.as_deref() == Some(label))
                .map(|n| n.rect)
                .collect()
        };
        let sizes = cells(&d, "size");
        d.check(sizes.len() == FILES.len(), "a size cell per row")?;
        let x = sizes[0].x;
        d.check(
            sizes.iter().all(|r| r.x == x && r.w == sizes[0].w),
            "every size cell starts at one x and is one width",
        )?;
        let table = d.key_of("files").ok_or("no table")?;
        let table_rect = d
            .core
            .nodes()
            .iter()
            .find(|n| n.key == table)
            .map(|n| n.rect)
            .ok_or("the table is not in the snapshot")?;
        let long = d
            .key_of("a-rather-long-file-name.rs")
            .ok_or("no long row")?;
        let long_rect = d.rect_of(long).ok_or("the row is not clickable")?;
        d.check(
            long_rect.w == table_rect.w - 8.0,
            "a row is as wide as the table's content",
        )?;
        let evs = d.click_key(self, long);
        d.check(evs.len() == 1, "a row's click is the app's")?;
        d.check(self.selected == Some(4), "and selects it")?;
        d.frame(self);
        let before = d.rect_of(long).ok_or("the row after the click")?;
        let header = d.key_of("sort-kind").ok_or("no kind header")?;
        d.click_key(self, header);
        d.check(
            self.sort == Sort::Kind,
            "a header's click sorts by its column",
        )?;
        d.frame(self);
        let after = d.rect_of(long).ok_or("the row after the sort")?;
        d.check(after.y != before.y, "and the rows moved")?;
        let sizes = cells(&d, "size");
        d.check(
            sizes.iter().all(|r| r.x == x),
            "the columns stayed where they were",
        )?;
        let warnings = d.core.take_warnings();
        d.check(warnings.is_empty(), "no warnings")
    }
}

kui_devtools::main!(Table::default());
