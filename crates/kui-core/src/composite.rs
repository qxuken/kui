//! Composite keyboard patterns: one Tab stop, arrows inside it
//! (`docs/adr/0007-composite-keyboard-patterns.md`).
//!
//! A tab list, a radio group, a menu and a picker list are *composites*: on
//! every platform they are one Tab stop, and the arrow keys move an inner
//! selection. kui derives that shape rather than taking it as a
//! declaration — a container role whose items are focusable is one, and a
//! `group` of buttons is not — for the reason that already made
//! `pos_in_set` derived: a declared flag is a second source of truth that
//! can disagree with what is on screen.
//!
//! Everything here is a function of the frame's tree. The two consumers are
//! [`crate::access`] (which numbers the items "3 of 7" and reports the
//! container's orientation) and [`crate::runtime`] (which collapses the Tab
//! ring and moves focus inside it). They share [`items`] so that the set a
//! reader is told about and the set the arrows walk cannot drift apart.

use crate::access::{Orientation, Role, focusable};
use crate::spec::Dir;
use crate::tree::{NodeContent, Tree};

/// The container / item pairs, and nothing else. `group`, a row of links
/// and a bar of icon buttons stay ordinary Tab rings. The fifth is the
/// table's (ADR 0070, backlog C90): its items are its cells — a header
/// cell among them ([`is_item`]) — whichever `row` holds them, and its
/// arrows move in two dimensions.
pub(crate) const PAIRS: [(Role, Role); 5] = [
    (Role::RadioGroup, Role::Radio),
    (Role::TabList, Role::Tab),
    (Role::Menu, Role::MenuItem),
    (Role::List, Role::ListItem),
    (Role::Table, Role::Cell),
];

/// The item role a container role holds, or None for a role that is not a
/// composite container.
pub(crate) fn item_role(container: Role) -> Option<Role> {
    PAIRS
        .iter()
        .find(|(c, _)| *c == container)
        .map(|(_, item)| *item)
}

/// Whether motion past either end comes round to the other (decision 8).
///
/// A `radioGroup`, `tabList` and `menu` are a choice among *n*, so past the
/// end is the next choice. A `list` clamps, and the reason is specific to
/// lists rather than a taste: **a list can be windowed.** With 4,000 rows
/// and 40 in the tree, the last item is not the last row, so wrapping from
/// the first to "the last" would land on row 40 and claim it was row 4,000.
pub(crate) fn wraps(container: Role) -> bool {
    // A table is a list of rows, and windowed the same way.
    !matches!(container, Role::List | Role::Table)
}

/// Whether moving focus onto an item also emits its activation payload
/// (decision 11). `radio` and `tab` *define* selection as following focus —
/// arrows check a radio, and a tab bar's automatic activation is what every
/// platform does — while a menu that ran whatever you passed over would be
/// unusable, and manual activation is the norm for a list.
pub(crate) fn activates_on_motion(item: Role) -> bool {
    matches!(item, Role::Radio | Role::Tab)
}

/// The items of the container at `i`: its descendants carrying `item`, in
/// tree order. One walk, so that "3 of 7" and the order the arrows take are
/// the same seven in the same order — two functions that agreed only by
/// inspection would eventually announce a lie.
///
/// It skips whole subtrees three ways, each matching what the access tree
/// does with them: a `role="none"` subtree is decoration, a nested
/// composite of the same kind owns its own items, and a presentational
/// node's descendants are read as part of it (which covers an item nested
/// in an item). Fills `out` rather than returning, so the ring walk pays
/// one allocation per frame instead of one per composite.
pub(crate) fn items(tree: &Tree, container: usize, item: Role, out: &mut Vec<usize>) {
    out.clear();
    let kind = role(tree, container);
    let end = tree.subtree_end(container);
    let mut i = container + 1;
    while i < end {
        // A hidden node is nothing to a reader or the ring (ADR 0069), so
        // nothing to count or step onto either.
        if tree.is_hidden(i) {
            i = tree.subtree_end(i);
            continue;
        }
        let role = role(tree, i);
        if role.is_some_and(|r| is_item(r, item)) {
            out.push(i);
            i = tree.subtree_end(i);
            continue;
        }
        if role == Some(Role::None)
            || role == kind
            || crate::access::derived_role(tree, i).is_some_and(Role::presentational)
        {
            i = tree.subtree_end(i);
            continue;
        }
        i += 1;
    }
}

/// Whether the container at `i` is a composite: at least one of its items
/// is focusable (decision 1). The `list` half of decision 2 falls out of
/// this without a prop of its own — a navigation list is rows *containing*
/// links, so the focusable node is the link and the rows are not items in
/// the ring's sense, while a picker's rows say `focusable` themselves.
pub(crate) fn is_composite(tree: &Tree, i: usize, items: &[usize]) -> bool {
    let _ = i;
    items.iter().any(|&j| focusable(tree, j))
}

/// The innermost composite container the node at `i` is an item of: the
/// nearest ancestor whose role pairs with `i`'s own. None when `i` is not
/// an item, or its container holds no focusable item.
pub(crate) fn owner(tree: &Tree, i: usize, scratch: &mut Vec<usize>) -> Option<usize> {
    let item = item_of(role(tree, i)?);
    let mut j = i;
    loop {
        j = match tree.parent[j] {
            crate::tree::NIL => return None,
            p => p as usize,
        };
        if role(tree, j).and_then(item_role) == Some(item) {
            items(tree, j, item, scratch);
            return (scratch.contains(&i) && is_composite(tree, j, scratch)).then_some(j);
        }
    }
}

/// A composite container's orientation, from its own `dir` (decision 7).
/// A table has none: it is arranged both ways.
pub(crate) fn orientation(tree: &Tree, i: usize) -> Option<Orientation> {
    let role = role(tree, i)?;
    item_role(role)?;
    if role == Role::Table {
        return None;
    }
    Some(match tree.specs[i].layout.dir {
        Dir::Row => Orientation::Horizontal,
        Dir::Column => Orientation::Vertical,
    })
}

/// Which wrap line the item at `i` sits on, within the container at
/// `container`. The layout writes `line` on a wrapping container's own
/// children, so an item nested deeper takes the line of the ancestor that
/// is one — which is the only shape a wrapped composite has.
pub(crate) fn line_of(tree: &Tree, container: usize, mut i: usize) -> u32 {
    loop {
        match tree.parent[i] {
            crate::tree::NIL => return 0,
            p if p as usize == container => return tree.line[i],
            p => i = p as usize,
        }
    }
}

/// The row a table's cell at `i` sits in: the nearest `row` between it and
/// the table at `container`, or the table itself for a cell outside every
/// row (a one-row table needs no `row`). ADR 0070.
pub(crate) fn row_of(tree: &Tree, container: usize, i: usize) -> usize {
    let mut j = i;
    loop {
        j = match tree.parent[j] {
            crate::tree::NIL => return container,
            p => p as usize,
        };
        if j == container || role(tree, j) == Some(Role::Row) {
            return j;
        }
    }
}

/// The role a composite reads for node `i`: the one it declares, else the
/// one a layout table implies for it ([`implied_role`]). Every composite
/// walk asks here rather than reading the spec, so a `table()` of
/// focusable cells is a composite with no role spelled, and navigation and
/// a reader see the same table.
pub(crate) fn role(tree: &Tree, i: usize) -> Option<Role> {
    let spec = &tree.specs[i];
    spec.access().role.or_else(|| {
        let plain = spec.events().on_click().is_none()
            && spec.window.is_none()
            && spec.events().modal().is_none()
            && !(spec.layout.scroll_x || spec.layout.scroll_y);
        plain.then(|| implied_role(tree, i)).flatten()
    })
}

/// The role a layout table implies for node `i`, which declares none (ADR
/// 0070): the table itself is a `table`, its in-flow `Row` children
/// (`layout::is_table_row`) are `row`s, and their in-flow container
/// children `cell`s — where the row is a row, so a row that declared
/// itself a `listItem` holds no cells. A text or an image straight in a
/// row is a cell too, as the layout counts it, named by its own text or
/// label. Asked only where nothing else made
/// the node something — `access::derived_role` asks it after a click, a
/// scroller, a modal and chrome, and [`role`] checks the same — so a
/// clickable row is still a button. Free on a frame with no table.
pub(crate) fn implied_role(tree: &Tree, i: usize) -> Option<Role> {
    if !tree.any_table {
        return None;
    }
    let leaf = match tree.content[i] {
        NodeContent::Container => false,
        NodeContent::Text(_) | NodeContent::Image(..) => true,
        _ => return None,
    };
    if !leaf && tree.specs[i].layout.is_table() {
        return Some(Role::Table);
    }
    if !leaf && crate::layout::is_table_row(tree, i as u32) {
        return Some(Role::Row);
    }
    let p = tree.parent[i];
    (p != crate::tree::NIL
        && tree.specs[i].layout.float.is_none()
        && crate::layout::is_table_row(tree, p)
        && role(tree, p as usize) == Some(Role::Row))
    .then_some(Role::Cell)
}

/// Whether a node of role `role` is an item of a composite whose items are
/// `item`: that role, or — in a table — a header cell, which navigation
/// treats as the cell it is.
pub(crate) fn is_item(role: Role, item: Role) -> bool {
    role == item || (item == Role::Cell && matches!(role, Role::ColumnHeader | Role::RowHeader))
}

/// The item role a node of role `role` is an item as: a header cell is a
/// cell.
pub(crate) fn item_of(role: Role) -> Role {
    match role {
        Role::ColumnHeader | Role::RowHeader => Role::Cell,
        r => r,
    }
}

/// The data index node `i` was opened with (`index`, `Ui::open_indexed`),
/// if any: where a virtual list's row sits in the data, built or not.
pub(crate) fn data_index(tree: &Tree, i: usize) -> Option<u64> {
    tree.indexed
        .iter()
        .find(|(at, _)| *at as usize == i)
        .map(|&(_, index)| index)
}

/// The `rowCount` a composite's rows are counted by: the nearest
/// declaration from `from` up to and including the container at
/// `container` — on the list itself, or on the scroller between it and its
/// rows, which is where a virtual list's helper puts it (ADR 0070).
pub(crate) fn row_count(tree: &Tree, from: usize, container: usize) -> Option<u64> {
    if tree.row_counts.is_empty() {
        return None;
    }
    let mut j = from;
    loop {
        if let Some(&(_, n)) = tree.row_counts.iter().find(|(at, _)| *at as usize == j) {
            return Some(n);
        }
        if j == container {
            return None;
        }
        j = match tree.parent[j] {
            crate::tree::NIL => return None,
            p => p as usize,
        };
    }
}
