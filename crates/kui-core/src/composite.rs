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
use crate::tree::Tree;

/// The four container / item pairs, and nothing else. `group`, a row of
/// links and a bar of icon buttons stay ordinary Tab rings.
pub(crate) const PAIRS: [(Role, Role); 4] = [
    (Role::RadioGroup, Role::Radio),
    (Role::TabList, Role::Tab),
    (Role::Menu, Role::MenuItem),
    (Role::List, Role::ListItem),
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
    container != Role::List
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
    let kind = tree.specs[container].access().role;
    let end = tree.subtree_end(container);
    let mut i = container + 1;
    while i < end {
        let role = tree.specs[i].access().role;
        if role == Some(item) {
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
    let item = tree.specs[i].access().role?;
    let mut j = i;
    loop {
        j = match tree.parent[j] {
            crate::tree::NIL => return None,
            p => p as usize,
        };
        if tree.specs[j].access().role.and_then(item_role) == Some(item) {
            items(tree, j, item, scratch);
            return (scratch.contains(&i) && is_composite(tree, j, scratch)).then_some(j);
        }
    }
}

/// A composite container's orientation, from its own `dir` (decision 7).
pub(crate) fn orientation(tree: &Tree, i: usize) -> Option<Orientation> {
    item_role(tree.specs[i].access().role?)?;
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
